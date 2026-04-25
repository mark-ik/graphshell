/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! iced-facing `HostPorts` implementation.
//!
//! Sibling of [`super::egui_host_ports`]. A bundle of borrowed handles
//! into [`super::iced_host::IcedHost`]'s stateful fields (clipboard,
//! cursor cache, modifier cache, toast queue, texture cache, deferred
//! viewer-surface requests), constructed fresh each tick at the host
//! call site so the port traits can delegate to live iced state.
//!
//! Deliberately **iced-shaped**, not an egui mirror:
//!
//! - `HostPaintPort::draw_*` — no-ops. iced paints overlays inline
//!   inside `GraphCanvasProgram::draw`, not through a shared
//!   host-side painter. Retained as type-level trait-compile
//!   validators; intentionally empty in production.
//! - `IcedOverlayAffordancePainter` / `IcedContentPassPainter` —
//!   same story: type-level validators only. The iced production
//!   path consumes portable descriptors directly.
//! - `HostSurfacePort` — deferred-request pattern matches egui for
//!   `present_surface`. Content-callback registration is a stub
//!   pending the iced-native viewer content surface design.
//! - `HostTexturePort::TextureHandle` — `IcedTextureHandle` is an
//!   iced-dep-free opaque handle (key + dimensions). Iced production
//!   will upgrade to `iced::image::Handle` when the image feature lands.
//! - `HostToastPort::enqueue` — pushes into the host's `toast_queue`;
//!   `IcedApp::view` renders the queue as an iced-native overlay stack.
//! - `HostAccessibilityPort` — deferred until iced's accesskit bridge.

use std::collections::HashMap;

use graphshell_core::geometry::{PortablePoint, PortableRect};
use graphshell_core::host_event::{HostEvent, ModifiersState};
use graphshell_runtime::{
    BackendViewportInPixels, HostAccessibilityPort, HostInputPort, HostPaintPort, HostSurfacePort,
    HostTexturePort, ToastSpec,
};
use graphshell_runtime::ports::{RuntimeClipboardPort as HostClipboardPort, RuntimeToastPort as HostToastPort};

use crate::graph::NodeKey;

/// Cached texture payload: raw RGBA plus dimensions. iced's
/// `image::Handle::from_rgba(width, height, pixels)` can rehydrate
/// this on demand when the iced host grows an image-display surface.
///
/// 2026-04-25 servo-into-verso S3b.1: relocated here from
/// `iced_host.rs` so this module has no shell-side gated
/// dependencies. iced_host now imports `CachedTexture` from this
/// module instead.
#[derive(Debug, Clone)]
pub(crate) struct CachedTexture {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) rgba: std::sync::Arc<[u8]>,
}

/// iced-side bundle of host port implementations.
///
/// Constructed fresh at each `IcedHost::tick_with_input` call with
/// mutable refs into the host's stateful fields. Value-typed fields
/// (cursor_position, modifiers) are captured by copy so the bundle
/// does not hold additional borrows that would conflict with the
/// mutable ones.
pub(crate) struct IcedHostPorts<'a> {
    pub(crate) clipboard: &'a mut Option<arboard::Clipboard>,
    pub(crate) cursor_position: Option<iced::Point>,
    pub(crate) modifiers: iced::keyboard::Modifiers,
    pub(crate) toast_queue: &'a mut Vec<ToastSpec>,
    pub(crate) texture_cache: &'a mut HashMap<String, CachedTexture>,
    pub(crate) pending_present_requests: &'a mut Vec<NodeKey>,
}

/// Iced-dep-free texture handle. Carries the cache key plus
/// dimensions so callers can query metadata without pulling an iced
/// `image::Handle` through the trait surface.
///
/// Rationale: iced's `image::Handle` requires the `image` or
/// `image-without-codecs` feature. Since `HostTexturePort` currently
/// has no live consumers, we keep the handle iced-dep-free and let
/// the eventual production iced image-display surface convert via
/// `image::Handle::from_rgba(h.width, h.height, cached.rgba.to_vec())`
/// at the call site.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct IcedTextureHandle {
    pub(crate) key: String,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

// ---------------------------------------------------------------------------
// HostInputPort
// ---------------------------------------------------------------------------

impl<'a> HostInputPort for IcedHostPorts<'a> {
    fn poll_events(&mut self) -> Vec<HostEvent> {
        // Intentionally empty: iced events are translated by
        // `iced_events::from_iced_event` in `IcedApp::update` and
        // passed to `runtime.tick` via `FrameHostInput.events` before
        // this port is constructed. Matches the egui host's pattern.
        Vec::new()
    }

    fn pointer_hover_position(&self) -> Option<PortablePoint> {
        self.cursor_position
            .map(|p| PortablePoint::new(p.x, p.y))
    }

    fn wants_keyboard_input(&self) -> bool {
        // iced doesn't expose a widget-tree "text input focused"
        // signal on the application side. Production iced hosts that
        // care can wire this via a focus-registry message. Safe
        // default is `false` — runtime consumers interpret this as
        // "host doesn't need exclusive keyboard capture."
        false
    }

    fn wants_pointer_input(&self) -> bool {
        // Same rationale as `wants_keyboard_input`. iced widgets
        // handle pointer capture themselves via `Action::capture()`
        // in their `update` hooks, so the runtime doesn't need to
        // gate pointer dispatch.
        false
    }

    fn modifiers(&self) -> ModifiersState {
        ModifiersState {
            alt: self.modifiers.alt(),
            ctrl: self.modifiers.control(),
            shift: self.modifiers.shift(),
            mac_cmd: self.modifiers.logo(),
            command: self.modifiers.command(),
        }
    }
}

// ---------------------------------------------------------------------------
// HostSurfacePort
// ---------------------------------------------------------------------------

impl<'a> HostSurfacePort for IcedHostPorts<'a> {
    /// Iced paints inline through `canvas::Program::draw`; it has no
    /// borrowed graphics-context handle to hand to content callbacks.
    /// `()` is the canonical "no backend context" choice for the
    /// associated type.
    type BackendContext = ();

    fn present_surface(&mut self, node_key: NodeKey) {
        // Defer to post-tick drain on `IcedHost`; the registry lives
        // on `GraphshellRuntime`, which is already mutably borrowed
        // by `tick` when this port is active.
        self.pending_present_requests.push(node_key);
    }

    fn retire_surface(&mut self, _node_key: NodeKey) {
        // Viewer-surface retirement for iced hosts is tied to the
        // iced-native content-surface registry design, which hasn't
        // landed. Until it does, retirement is a no-op — the egui
        // host is the sole driver of actual surface lifecycle.
    }

    fn register_content_callback(
        &mut self,
        _node_key: NodeKey,
        _callback: std::sync::Arc<
            dyn Fn(&Self::BackendContext, BackendViewportInPixels) + Send + Sync,
        >,
    ) {
        // Iced's canvas primitive doesn't take wgpu callbacks the
        // way egui's compositor does. When iced hosts viewer content,
        // the content-surface bridge will likely go through the
        // compositor's native-texture path rather than this callback.
        // Intentional no-op for now.
    }

    fn unregister_content_callback(&mut self, _node_key: NodeKey) {
        // Paired with `register_content_callback` — no-op until the
        // iced-side content-surface bridge lands.
    }
}

// ---------------------------------------------------------------------------
// HostPaintPort
// ---------------------------------------------------------------------------

impl<'a> HostPaintPort for IcedHostPorts<'a> {
    // All methods below are intentionally empty: the iced host paints
    // overlays and chrome inline inside `GraphCanvasProgram::draw` and
    // `IcedApp::view`, not through a shared host-side painter. These
    // trait methods remain for type-level trait-compile validation.
    // See the module docstring for the full architectural rationale.

    fn draw_overlay_stroke(
        &mut self,
        _node_key: NodeKey,
        _rect: PortableRect,
        _stroke: graph_canvas::packet::Stroke,
        _rounding: f32,
    ) {
    }

    fn draw_dashed_overlay_stroke(
        &mut self,
        _node_key: NodeKey,
        _rect: PortableRect,
        _stroke: graph_canvas::packet::Stroke,
    ) {
    }

    fn draw_overlay_glyphs(
        &mut self,
        _node_key: NodeKey,
        _rect: PortableRect,
        _glyphs: &[crate::registries::atomic::lens::GlyphOverlay],
        _color: graph_canvas::packet::Color,
    ) {
    }

    fn draw_overlay_chrome_markers(
        &mut self,
        _node_key: NodeKey,
        _rect: PortableRect,
        _stroke: graph_canvas::packet::Stroke,
    ) {
    }

    fn draw_degraded_receipt(&mut self, _rect: PortableRect, _message: &str) {}
}

// ---------------------------------------------------------------------------
// HostTexturePort
// ---------------------------------------------------------------------------

impl<'a> HostTexturePort for IcedHostPorts<'a> {
    type TextureHandle = IcedTextureHandle;

    fn load_texture(
        &mut self,
        key: &str,
        width: u32,
        height: u32,
        rgba: &[u8],
    ) -> Self::TextureHandle {
        let cached = CachedTexture {
            width,
            height,
            rgba: std::sync::Arc::from(rgba.to_vec().into_boxed_slice()),
        };
        self.texture_cache.insert(key.to_string(), cached);
        IcedTextureHandle {
            key: key.to_string(),
            width,
            height,
        }
    }

    fn texture(&self, key: &str) -> Option<Self::TextureHandle> {
        self.texture_cache.get(key).map(|cached| IcedTextureHandle {
            key: key.to_string(),
            width: cached.width,
            height: cached.height,
        })
    }

    fn drop_texture(&mut self, key: &str) {
        self.texture_cache.remove(key);
    }
}

// ---------------------------------------------------------------------------
// HostClipboardPort
// ---------------------------------------------------------------------------

impl<'a> HostClipboardPort for IcedHostPorts<'a> {
    fn get_text(&mut self) -> Option<String> {
        let cb = self.clipboard.as_mut()?;
        cb.get_text().ok()
    }

    fn set_text(&mut self, text: &str) -> Result<(), String> {
        // Lazy init on first write, mirroring the egui host's pattern.
        if self.clipboard.is_none() {
            *self.clipboard = arboard::Clipboard::new().ok();
        }
        let Some(cb) = self.clipboard.as_mut() else {
            return Err("clipboard unavailable".to_string());
        };
        cb.set_text(text).map_err(|e| e.to_string())
    }
}

// ---------------------------------------------------------------------------
// HostToastPort
// ---------------------------------------------------------------------------

impl<'a> HostToastPort for IcedHostPorts<'a> {
    fn enqueue(&mut self, toast: ToastSpec) {
        // Push onto the host's queue; `IcedApp::view` renders entries
        // as an iced-native overlay stack. `IcedHost::tick_with_input`
        // trims the queue to `MAX_TOAST_QUEUE` after the runtime
        // returns so unbounded enqueue streams can't grow memory.
        self.toast_queue.push(toast);
    }
}

// ---------------------------------------------------------------------------
// HostAccessibilityPort
// ---------------------------------------------------------------------------

impl<'a> HostAccessibilityPort for IcedHostPorts<'a> {
    fn request_focus(&mut self, _node_id: accesskit::NodeId) {
        // Iced's accesskit bridge lands with the chrome-port cleanup
        // effort (§5.2 of 2026-04-17_chrome_port_cleanup_plan.md).
        // Until then, focus requests are dropped on the iced host.
    }
}

// Tree-update injection is gated on servo-engine (Servo's accesskit
// stream is keyed on `servo::WebViewId`). When servo-engine is on,
// implement the Servo-specific extension trait so the egui +
// runtime accesskit pipeline that already exists keeps working
// against this iced ports bundle. Iced's own accesskit consumer
// will land separately and consume `accesskit::TreeUpdate`
// directly without a `WebViewId` key.
#[cfg(feature = "servo-engine")]
impl<'a> crate::shell::desktop::ui::host_ports::ServoAccessibilityInjectionPort
    for IcedHostPorts<'a>
{
    fn inject_tree_update(
        &mut self,
        _webview_id: verso::servo_engine::WebViewId,
        _update: verso::servo_engine::accesskit::TreeUpdate,
    ) {
        // No-op stub for now.
    }
}

// ---------------------------------------------------------------------------
// Narrow painter-trait stubs — gated behind servo-engine
// ---------------------------------------------------------------------------
//
// The compositor exposes two narrow painter traits alongside the broader
// `HostPaintPort`: `OverlayAffordancePainter` (overlay strokes/glyphs) and
// `ContentPassPainter` (content-layer callback registration + native-texture
// placement). These were carved out of the egui host's compositor path in
// M3.5 so iced could plug its own painter implementation into the same
// static `CompositorAdapter` flow.
//
// **Iced-idiomatic deviation**: the iced host does NOT paint overlays or
// content through these traits in production. iced's canvas widget paints
// everything inline inside `canvas::Program::draw`. Overlay descriptors
// flow from `FrameViewModel.overlays` directly. The stubs below remain
// only to prove the trait surface is host-portable, but they consume
// `compositor_adapter` and `render_backend` types which are gated behind
// servo-engine by the S2b sweep — so the stubs themselves are
// servo-engine-gated for now. Decoupling them is part of S3b
// (compositor-side painter trait extraction).

#[cfg(feature = "servo-engine")]
mod servo_engine_painter_stubs {
    use super::*;
    use crate::shell::desktop::render_backend::BackendCustomPass;
    use crate::shell::desktop::workbench::compositor_adapter::{
        ContentPassPainter, OverlayAffordancePainter, OverlayStrokePass,
    };

    #[derive(Default)]
    pub(crate) struct IcedOverlayAffordancePainter {
        pub(crate) seen_count: usize,
    }

    impl OverlayAffordancePainter for IcedOverlayAffordancePainter {
        fn paint(&mut self, _overlay: &OverlayStrokePass) {
            self.seen_count += 1;
        }
    }

    #[derive(Default)]
    pub(crate) struct IcedContentPassPainter {
        pub(crate) registered_count: usize,
        pub(crate) native_painted_count: usize,
    }

    impl ContentPassPainter for IcedContentPassPainter {
        fn register_content_callback_on_layer(
            &mut self,
            _node_key: NodeKey,
            _tile_rect: PortableRect,
            _callback: BackendCustomPass,
        ) {
            self.registered_count += 1;
        }

        fn paint_native_content_texture(
            &mut self,
            _node_key: NodeKey,
            _tile_rect: PortableRect,
            _texture_token: crate::shell::desktop::render_backend::BackendTextureToken,
        ) {
            self.native_painted_count += 1;
        }
    }
}

#[cfg(feature = "servo-engine")]
pub(crate) use servo_engine_painter_stubs::{IcedContentPassPainter, IcedOverlayAffordancePainter};

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_ports<'a>(
        clipboard: &'a mut Option<arboard::Clipboard>,
        toast_queue: &'a mut Vec<ToastSpec>,
        texture_cache: &'a mut HashMap<String, CachedTexture>,
        pending_present: &'a mut Vec<NodeKey>,
    ) -> IcedHostPorts<'a> {
        IcedHostPorts {
            clipboard,
            cursor_position: None,
            modifiers: iced::keyboard::Modifiers::empty(),
            toast_queue,
            texture_cache,
            pending_present_requests: pending_present,
        }
    }

    #[test]
    fn pointer_hover_position_maps_cursor_to_portable_point() {
        let mut clip = None;
        let mut toasts = Vec::new();
        let mut tex = HashMap::new();
        let mut presents = Vec::new();
        let mut ports = empty_ports(&mut clip, &mut toasts, &mut tex, &mut presents);
        ports.cursor_position = Some(iced::Point::new(123.0, 456.0));

        let p = ports.pointer_hover_position().expect("cursor set");
        assert_eq!(p.x, 123.0);
        assert_eq!(p.y, 456.0);
    }

    #[test]
    fn modifiers_projects_iced_state() {
        let mut clip = None;
        let mut toasts = Vec::new();
        let mut tex = HashMap::new();
        let mut presents = Vec::new();
        let mut ports = empty_ports(&mut clip, &mut toasts, &mut tex, &mut presents);
        // `iced::keyboard::Modifiers` constructors are test-private;
        // CTRL + SHIFT via the public bitflag-style API:
        ports.modifiers = iced::keyboard::Modifiers::CTRL | iced::keyboard::Modifiers::SHIFT;

        let m = HostInputPort::modifiers(&ports);
        assert!(m.ctrl, "ctrl should propagate");
        assert!(m.shift, "shift should propagate");
        assert!(!m.alt, "alt should stay false");
    }

    #[test]
    fn toast_enqueue_pushes_to_queue() {
        let mut clip = None;
        let mut toasts = Vec::new();
        let mut tex = HashMap::new();
        let mut presents = Vec::new();
        let mut ports = empty_ports(&mut clip, &mut toasts, &mut tex, &mut presents);

        ports.enqueue(ToastSpec {
            severity: graphshell_runtime::ToastSeverity::Info,
            message: "hello".into(),
            duration: None,
        });
        drop(ports);

        assert_eq!(toasts.len(), 1);
        assert_eq!(toasts[0].message, "hello");
    }

    #[test]
    fn texture_roundtrip_through_port() {
        let mut clip = None;
        let mut toasts = Vec::new();
        let mut tex = HashMap::new();
        let mut presents = Vec::new();
        let mut ports = empty_ports(&mut clip, &mut toasts, &mut tex, &mut presents);

        let handle = ports.load_texture("favicon:example", 32, 32, &[0xff; 32 * 32 * 4]);
        assert_eq!(handle.width, 32);
        assert_eq!(handle.height, 32);
        assert_eq!(handle.key, "favicon:example");

        let fetched = ports.texture("favicon:example").expect("cached");
        assert_eq!(fetched, handle);

        ports.drop_texture("favicon:example");
        assert!(ports.texture("favicon:example").is_none());
    }

    #[test]
    fn present_surface_defers_to_pending_queue() {
        let mut clip = None;
        let mut toasts = Vec::new();
        let mut tex = HashMap::new();
        let mut presents = Vec::new();
        let mut ports = empty_ports(&mut clip, &mut toasts, &mut tex, &mut presents);

        ports.present_surface(NodeKey::new(7));
        drop(ports);

        assert_eq!(presents.len(), 1);
        assert_eq!(presents[0], NodeKey::new(7));
    }
}
