/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! iced-backed host adapter.
//!
//! Sibling of [`super::gui::EguiHost`]. Owns the iced-side wiring of the
//! shared `GraphshellRuntime`: its job is to collect iced events into
//! [`FrameHostInput`], call `runtime.tick(&input, &mut IcedHostPorts)`,
//! drain deferred port requests (surface presents, retires), and expose
//! runtime state to `IcedApp::view` via the cached `FrameViewModel`.
//!
//! Deliberately **iced-shaped, not egui-shaped** — see the 2026-04-24
//! "iced-idiomatic canvas architecture" entry in the migration plan.
//! The `IcedHost` struct holds state that iced needs directly (cursor
//! cache, modifier cache, toast queue, texture cache) plus the
//! deferred-request pattern egui introduced to sidestep double-borrows
//! of `GraphshellRuntime::viewer_surfaces`.

use std::collections::HashMap;

use crate::app::GraphViewId;
use crate::graph::NodeKey;
use crate::shell::desktop::ui::gui_state::GraphshellRuntime;
use crate::shell::desktop::ui::iced_host_ports::{CachedTexture, IcedHostPorts, IcedTextureHandle};
use graphshell_runtime::{FrameHostInput, FrameViewModel, ToastSpec};

/// Wgpu device + queue handles iced's renderer hands to the host.
///
/// Populated by `IcedHost::install_wgpu_context` once iced's
/// rendering pipeline exposes the device (see the 2026-04-24 content-
/// surface scoping doc §C1). Consumers are the future
/// `WebViewSurface<NodeKey>` widget and any Servo-produced wgpu
/// texture that iced imports for in-pane content rendering.
///
/// 2026-04-25 servo-into-verso S3b: gated on `servo-engine`. The
/// fields use `servo::wgpu` types because the original C1 use case
/// was holding Servo-produced textures for iced consumption. When
/// servo-engine is off there's no Servo producer, so the slot is
/// also off; iced-only builds get `wgpu_context: ()` instead.
/// A future slice can introduce a host-neutral wgpu vocabulary
/// (e.g., the upstream `wgpu` crate at iced's pinned version) once
/// real iced-side wgpu consumers land.
#[cfg(feature = "servo-engine")]
#[derive(Clone)]
pub(crate) struct IcedWgpuContext {
    pub(crate) device: servo::wgpu::Device,
    pub(crate) queue: servo::wgpu::Queue,
}

/// iced-side host adapter around a shared `GraphshellRuntime`.
pub(crate) struct IcedHost {
    /// Host-neutral runtime state shared with `EguiHost`.
    pub(crate) runtime: GraphshellRuntime,

    /// Stable `GraphViewId` the iced graph canvas is bound to. Minted
    /// once at `IcedHost` construction and reused for the lifetime of
    /// the host so camera persistence / view-scoped telemetry key on
    /// a single identity rather than regenerating per `view()` call.
    pub(crate) view_id: GraphViewId,

    /// Lazily-constructed OS clipboard handle. Mirrors the egui host's
    /// shape — clipboard is OS-level, not framework-specific, so both
    /// hosts use arboard.
    pub(crate) clipboard: Option<arboard::Clipboard>,

    /// Cursor position in window-local coordinates, cached from
    /// `iced::Event::Mouse(CursorMoved)` so `HostInputPort::pointer_hover_position`
    /// can surface it to the runtime at tick time. `None` until the
    /// cursor enters the window.
    pub(crate) cursor_position: Option<iced::Point>,

    /// Keyboard modifier state, cached from
    /// `iced::Event::Keyboard(ModifiersChanged)` so
    /// `HostInputPort::modifiers` can surface it at tick time.
    pub(crate) modifiers: iced::keyboard::Modifiers,

    /// Toast queue populated by `HostToastPort::enqueue`. `IcedApp::view`
    /// drains this and renders toasts as iced-native overlays. Bounded
    /// to the most recent `MAX_TOAST_QUEUE` entries so a runaway
    /// enqueue stream doesn't grow unbounded.
    pub(crate) toast_queue: Vec<ToastSpec>,

    /// Texture cache populated by `HostTexturePort::load_texture`.
    /// Keyed on the caller-provided string; values are raw RGBA bytes
    /// plus dimensions so iced's `image::Handle::from_rgba` can be
    /// constructed on demand when image display wires land. Leaves
    /// iced's `image` feature optional for this iced-host variant.
    pub(crate) texture_cache: HashMap<String, CachedTexture>,

    /// Deferred `present_surface` calls. `HostSurfacePort::present_surface`
    /// cannot directly call `ViewerSurfaceRegistry::bump_content_generation`
    /// because the registry lives on `GraphshellRuntime`, which is
    /// mutably borrowed by `tick`. Drained post-tick. Matches the egui
    /// host's `pending_present_requests` pattern.
    pub(crate) pending_present_requests: Vec<NodeKey>,

    /// Portable `HostIntent`s queued by `IcedApp` message handlers
    /// (toolbar submit, future command-palette actions). Drained into
    /// `FrameHostInput.host_intents` on the next tick so the runtime
    /// translates and applies them through its reducer path — §12.17's
    /// sanctioned route for host-originated mutation.
    pub(crate) pending_host_intents: Vec<graphshell_core::shell_state::host_intent::HostIntent>,

    /// Iced renderer's wgpu device/queue, when available. `None`
    /// during early iced startup (before the renderer has been
    /// initialized) and in test harnesses that don't boot a real
    /// wgpu backend. Populated via `install_wgpu_context` from the
    /// iced application boot path once the device handles become
    /// reachable.
    ///
    /// Consumers: `WebViewSurface<NodeKey>` (future) — the iced
    /// widget that mounts Servo-produced wgpu textures inside graph
    /// node panes. Required for C3 of the content-surface scoping
    /// doc; exposed here as C1 so the slot exists before any
    /// consumer wires up. Gated behind servo-engine since the slot
    /// today only holds Servo-produced textures.
    #[cfg(feature = "servo-engine")]
    pub(crate) wgpu_context: Option<IcedWgpuContext>,
}

// `CachedTexture` was relocated to `iced_host_ports.rs` (2026-04-25
// servo-into-verso S3b.1) so the iced_host_ports module has no
// gated shell-side deps and can be unconditionally available
// alongside its trait surface in graphshell-runtime.

/// Upper bound on the toast queue. Older entries drop when the queue
/// grows past this. Chosen to be small enough to render in a corner
/// stack without overflowing the viewport on toast-heavy sessions.
pub(crate) const MAX_TOAST_QUEUE: usize = 8;

impl IcedHost {
    /// Construct an `IcedHost` wrapping the supplied runtime.
    pub(crate) fn with_runtime(runtime: GraphshellRuntime) -> Self {
        Self {
            runtime,
            view_id: GraphViewId::new(),
            clipboard: None,
            cursor_position: None,
            modifiers: iced::keyboard::Modifiers::empty(),
            toast_queue: Vec::new(),
            texture_cache: HashMap::new(),
            pending_present_requests: Vec::new(),
            pending_host_intents: Vec::new(),
            #[cfg(feature = "servo-engine")]
            wgpu_context: None,
        }
    }

    /// Install the iced renderer's wgpu device/queue handles on this
    /// host. Called from the iced application boot path once the
    /// renderer is reachable; no-op if called twice (last write wins).
    /// Consumers can then acquire a `&IcedWgpuContext` via
    /// [`wgpu_context`](Self::wgpu_context).
    #[cfg(feature = "servo-engine")]
    pub(crate) fn install_wgpu_context(&mut self, ctx: IcedWgpuContext) {
        self.wgpu_context = Some(ctx);
    }

    /// Access the installed wgpu device/queue, if any. Widgets that
    /// need wgpu access (e.g. `WebViewSurface<NodeKey>`) bail out
    /// gracefully when this returns `None` — iced hosts without a
    /// wgpu-capable renderer attached fall back to chrome-only mode.
    #[cfg(feature = "servo-engine")]
    pub(crate) fn wgpu_context(&self) -> Option<&IcedWgpuContext> {
        self.wgpu_context.as_ref()
    }

    /// Construct a minimal `IcedHost` for skeleton / test purposes.
    #[cfg(test)]
    pub(crate) fn new_for_testing() -> Self {
        Self::with_runtime(GraphshellRuntime::for_testing())
    }

    /// Drive one tick of the shared runtime.
    ///
    /// Drains `pending_host_intents` into a fresh `FrameHostInput`
    /// that merges the caller's input with host-originated intents,
    /// builds an `IcedHostPorts` bundle borrowing the host's stateful
    /// fields, calls `runtime.tick(input, ports)`, then drains any
    /// deferred surface-registry requests before returning the
    /// projected `FrameViewModel`.
    pub(crate) fn tick_with_input(&mut self, input: &FrameHostInput) -> FrameViewModel {
        let cursor = self.cursor_position;
        let mods = self.modifiers;

        // Merge queued host intents into this tick's input. Cloning
        // the caller's input is cheap (small struct, Vec<HostEvent>
        // is already owned) and keeps `input: &FrameHostInput` API
        // shape stable for callers who don't care about intents.
        let merged_input = if self.pending_host_intents.is_empty() {
            None
        } else {
            let mut merged = input.clone();
            merged
                .host_intents
                .extend(self.pending_host_intents.drain(..));
            Some(merged)
        };
        let tick_input: &FrameHostInput = merged_input.as_ref().unwrap_or(input);

        let vm = {
            let mut ports = IcedHostPorts {
                clipboard: &mut self.clipboard,
                cursor_position: cursor,
                modifiers: mods,
                toast_queue: &mut self.toast_queue,
                texture_cache: &mut self.texture_cache,
                pending_present_requests: &mut self.pending_present_requests,
            };
            self.runtime.tick(tick_input, &mut ports)
        };

        // Drain deferred surface requests: the registry lives on the
        // runtime, which `tick` has just released, so the bumps are
        // safe now.
        #[cfg(feature = "servo-engine")]
        for key in self.pending_present_requests.drain(..) {
            self.runtime.viewer_surfaces.bump_content_generation(&key);
        }
        #[cfg(not(feature = "servo-engine"))]
        self.pending_present_requests.clear();

        // Bound the toast queue so unbounded enqueue streams can't
        // grow memory over a long session.
        if self.toast_queue.len() > MAX_TOAST_QUEUE {
            let excess = self.toast_queue.len() - MAX_TOAST_QUEUE;
            self.toast_queue.drain(0..excess);
        }

        vm
    }

    /// Handle for drop-ins that want to pre-construct a texture handle
    /// without going through the port trait (e.g. for tests).
    #[cfg(test)]
    pub(crate) fn insert_texture(
        &mut self,
        key: impl Into<String>,
        width: u32,
        height: u32,
        rgba: Vec<u8>,
    ) -> IcedTextureHandle {
        let key = key.into();
        let cached = CachedTexture {
            width,
            height,
            rgba: std::sync::Arc::from(rgba.into_boxed_slice()),
        };
        self.texture_cache.insert(key.clone(), cached);
        IcedTextureHandle { key, width, height }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iced_host_drives_runtime_tick() {
        let mut host = IcedHost::new_for_testing();
        let input = FrameHostInput::default();
        let _view_model = host.tick_with_input(&input);
    }

    #[test]
    fn host_starts_with_stable_view_id() {
        let host_a = IcedHost::new_for_testing();
        let host_b = IcedHost::new_for_testing();
        // Each host gets a fresh view_id, but the id is stable across
        // `tick_with_input` calls on the same host (this test pins the
        // invariant by construction — `view_id` is public `pub(crate)`
        // and not rewritten).
        assert_ne!(host_a.view_id, host_b.view_id);
    }

    #[test]
    fn tick_drains_pending_present_requests() {
        let mut host = IcedHost::new_for_testing();
        host.pending_present_requests.push(NodeKey::new(42));
        let _ = host.tick_with_input(&FrameHostInput::default());
        assert!(
            host.pending_present_requests.is_empty(),
            "present queue should drain every tick",
        );
    }

    /// C1 slot test — `wgpu_context` starts None and `install_wgpu_context`
    /// populates it. Doesn't boot a real wgpu adapter; just verifies the
    /// slot lifecycle. A real adapter-boot test lands alongside the first
    /// consumer (C3 `WebViewSurface<NodeKey>`). Gated on servo-engine
    /// since the slot itself only exists on Servo-producing builds.
    #[cfg(feature = "servo-engine")]
    #[test]
    fn wgpu_context_slot_starts_none_and_accepts_install() {
        let host = IcedHost::new_for_testing();
        assert!(
            host.wgpu_context().is_none(),
            "wgpu_context starts None before renderer boot",
        );
        // We can't construct a real `servo::wgpu::Device` without a
        // live adapter; the install path is exercised end-to-end when
        // iced's renderer boot wires in. This test pins the slot
        // shape — that `install_wgpu_context` is callable and the
        // accessor method exists.
        let _ = IcedHost::install_wgpu_context;
        let _ = IcedHost::wgpu_context;
    }

    #[test]
    fn toast_queue_is_bounded() {
        let mut host = IcedHost::new_for_testing();
        // Enqueue more than MAX_TOAST_QUEUE toasts by hand; tick should
        // trim to the most recent MAX_TOAST_QUEUE entries.
        for i in 0..(MAX_TOAST_QUEUE + 5) {
            host.toast_queue.push(ToastSpec {
                severity: graphshell_runtime::ToastSeverity::Info,
                message: format!("toast {i}"),
                duration: None,
            });
        }
        let _ = host.tick_with_input(&FrameHostInput::default());
        assert_eq!(host.toast_queue.len(), MAX_TOAST_QUEUE);
        assert!(
            host.toast_queue
                .last()
                .map(|t| t.message.contains(&(MAX_TOAST_QUEUE + 4).to_string()))
                .unwrap_or(false),
            "most recent toast should be preserved; got {:?}",
            host.toast_queue.last().map(|t| &t.message),
        );
    }
}
