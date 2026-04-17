/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Host capability ports — the service interfaces `GraphshellRuntime` uses to
//! drive whatever host (egui today, iced later) is presenting its output.
//!
//! Per the M3.5 runtime boundary design
//! (`design_docs/graphshell_docs/implementation_strategy/shell/2026-04-16_runtime_boundary_design.md`
//! §4), each port is a narrow trait scoped to one concern: input ingress,
//! surface mounting/presentation, overlay painting, texture caching,
//! clipboard, transient notifications, and accessibility.
//!
//! Host implementations bundle all seven trait impls behind one struct
//! (`EguiHostPorts`, eventually `IcedHostPorts`) that the runtime borrows
//! per-tick.
//!
//! ## M4.4 / M4.5 status
//!
//! Trait surfaces defined in M4.4. The egui-side implementation
//! ([`EguiHostPorts`](super::egui_host_ports::EguiHostPorts)) lands as a
//! scaffold in the same phase. M4.5 (in progress) renamed the host adapter
//! from `Gui` to `EguiHost`; the remaining work is routing the frame
//! pipeline through these ports instead of reaching into shell state
//! directly.

use std::sync::Arc;
use std::time::Duration;

use crate::graph::NodeKey;
use crate::shell::desktop::render_backend::{BackendGraphicsContext, BackendViewportInPixels};
use crate::shell::desktop::ui::frame_model::{ToastSeverity, ToastSpec};
use crate::shell::desktop::workbench::ux_replay::{HostEvent, ModifiersState};
use servo::WebViewId;

// ---------------------------------------------------------------------------
// HostInputPort — raw input ingress
// ---------------------------------------------------------------------------

/// The runtime queries this to read raw input each tick. The host translates
/// its native events (egui's `InputState`, iced's `Event`, etc.) into the
/// host-neutral [`HostEvent`] vocabulary.
pub(crate) trait HostInputPort {
    /// Drain input events accumulated since the last tick.
    fn poll_events(&mut self) -> Vec<HostEvent>;

    /// Current pointer hover position in screen coordinates, if any.
    fn pointer_hover_position(&self) -> Option<egui::Pos2>;

    /// Does a host-owned widget (text input, dialog field) currently want
    /// keyboard input? When true, the runtime should not route keyboard
    /// events to content.
    fn wants_keyboard_input(&self) -> bool;

    /// Does a host-owned widget currently want pointer input?
    fn wants_pointer_input(&self) -> bool;

    /// Active keyboard modifier state this tick.
    fn modifiers(&self) -> ModifiersState;
}

// ---------------------------------------------------------------------------
// HostSurfacePort — content-surface mounting and presentation
// ---------------------------------------------------------------------------

/// The host uses this to register content-surface callbacks that the runtime
/// will invoke when a surface needs painting (e.g., a Servo webview frame).
///
/// The callback signature deliberately uses `BackendGraphicsContext` — a
/// render-backend abstraction, not a framework-specific painter — so the
/// registered work is portable between egui and iced.
pub(crate) trait HostSurfacePort {
    /// Notify the host that a surface's content has changed and should be
    /// presented on the next paint. The host consults the
    /// `ViewerSurfaceRegistry` to resolve the node key to a concrete handle.
    fn present_surface(&mut self, node_key: NodeKey);

    /// Retire a surface (node closed, tombstoned, or moved off-screen).
    fn retire_surface(&mut self, node_key: NodeKey);

    /// Register a content callback invoked when a surface paints.
    fn register_content_callback(
        &mut self,
        node_key: NodeKey,
        callback: Arc<dyn Fn(&BackendGraphicsContext, BackendViewportInPixels) + Send + Sync>,
    );

    /// Unregister a previously-registered content callback.
    fn unregister_content_callback(&mut self, node_key: NodeKey);
}

// ---------------------------------------------------------------------------
// HostPaintPort — overlay painting
// ---------------------------------------------------------------------------

/// Overlay painting operations invoked by the runtime's compositor pass.
///
/// Overlays are described host-neutrally by `OverlayStrokePass` descriptors
/// (see [`crate::shell::desktop::workbench::compositor_adapter::OverlayStrokePass`]);
/// this port's methods translate descriptor intent into concrete draw calls
/// against whatever painter the host owns.
///
/// (The `egui::Rect` / `egui::Stroke` / `egui::Color32` types flowing through
/// these methods are the "cosmetic leaks" called out in the M3.5 design doc.
/// They do not block iced implementation — iced can pair each egui primitive
/// with a trivial conversion at the boundary.)
pub(crate) trait HostPaintPort {
    /// Paint a rectangular stroke outline for an overlay affordance (e.g.,
    /// focus ring, selection outline).
    fn draw_overlay_stroke(
        &mut self,
        node_key: NodeKey,
        rect: egui::Rect,
        stroke: egui::Stroke,
        rounding: f32,
    );

    /// Paint a dashed rectangular stroke (used for drag previews, ephemeral
    /// affordances).
    fn draw_dashed_overlay_stroke(
        &mut self,
        node_key: NodeKey,
        rect: egui::Rect,
        stroke: egui::Stroke,
    );

    /// Paint lens glyph overlays positioned relative to a tile rect.
    fn draw_overlay_glyphs(
        &mut self,
        node_key: NodeKey,
        rect: egui::Rect,
        glyphs: &[crate::registries::atomic::lens::GlyphOverlay],
        color: egui::Color32,
    );

    /// Paint chrome markers (tick/indicator lines at tile edges).
    fn draw_overlay_chrome_markers(
        &mut self,
        node_key: NodeKey,
        rect: egui::Rect,
        stroke: egui::Stroke,
    );

    /// Paint a degraded-mode receipt (small in-tile text banner).
    fn draw_degraded_receipt(&mut self, rect: egui::Rect, message: &str);
}

// ---------------------------------------------------------------------------
// HostTexturePort — favicon / image cache
// ---------------------------------------------------------------------------

/// Texture cache. The host owns the concrete handle type (egui's
/// `TextureHandle`, iced's equivalent); the runtime only names textures by
/// a stable key string.
pub(crate) trait HostTexturePort {
    /// Opaque handle type — caller treats it as a black box.
    type TextureHandle: Clone;

    /// Load or reuse a texture for `key` from raw pixel data. Returns a
    /// handle valid until `drop_texture(key)` is called or the host is
    /// destroyed.
    fn load_texture(&mut self, key: &str, width: u32, height: u32, rgba: &[u8])
        -> Self::TextureHandle;

    /// Look up a previously-loaded texture by key.
    fn texture(&self, key: &str) -> Option<Self::TextureHandle>;

    /// Release a texture. Subsequent lookups return `None`.
    fn drop_texture(&mut self, key: &str);
}

// ---------------------------------------------------------------------------
// HostClipboardPort — clipboard access
// ---------------------------------------------------------------------------

/// Clipboard get/set. Both egui and iced use `arboard` under the hood today,
/// so this port is essentially host-neutral — it exists so runtime code
/// doesn't reach directly into the host's clipboard holder.
pub(crate) trait HostClipboardPort {
    /// Read current clipboard text. Returns `None` if unavailable or empty.
    fn get_text(&mut self) -> Option<String>;

    /// Write text to the clipboard. The `Err(String)` case carries a
    /// short user-presentable description of the failure; callers may
    /// surface it via `HostToastPort` for feedback. The `Err` branch also
    /// covers "clipboard unavailable" — the port caller does not need to
    /// probe availability separately.
    fn set_text(&mut self, text: &str) -> Result<(), String>;
}

// ---------------------------------------------------------------------------
// HostToastPort — transient notifications
// ---------------------------------------------------------------------------

/// Transient notification delivery. The runtime pushes `ToastSpec` values;
/// the host renders them in whatever notification UI it has (egui_notify,
/// iced's toast system, a native macOS notification, etc.).
pub(crate) trait HostToastPort {
    /// Enqueue a toast for display.
    fn enqueue(&mut self, toast: ToastSpec);

    /// Convenience helper — build a `ToastSpec` and enqueue it.
    fn enqueue_message(
        &mut self,
        severity: ToastSeverity,
        message: impl Into<String>,
        duration: Option<Duration>,
    ) {
        self.enqueue(ToastSpec {
            severity,
            message: message.into(),
            duration,
        });
    }
}

// ---------------------------------------------------------------------------
// HostAccessibilityPort — accesskit bridging
// ---------------------------------------------------------------------------

/// Accessibility tree integration. Both egui and iced (with the right
/// backend) speak accesskit, but each framework injects the tree differently.
pub(crate) trait HostAccessibilityPort {
    /// Inject an accessibility tree update received from a runtime viewer
    /// (e.g., Servo's accesskit stream).
    fn inject_tree_update(
        &mut self,
        webview_id: WebViewId,
        update: servo::accesskit::TreeUpdate,
    );

    /// Request the host transfer programmatic focus to a particular node
    /// (e.g., when keyboard navigation lands somewhere chrome-owned).
    fn request_focus(&mut self, node_id: accesskit::NodeId);
}

// ---------------------------------------------------------------------------
// HostPorts composite — the bundle the runtime borrows per-tick
// ---------------------------------------------------------------------------

/// Composite bound — any type that implements the six non-texture ports
/// automatically qualifies as a `HostPorts`. Texture handling has an
/// associated type on its own trait (`HostTexturePort`) and is not part of
/// this composite; call sites that need textures bound `T: HostPorts +
/// HostTexturePort` explicitly.
pub(crate) trait HostPorts:
    HostInputPort
    + HostSurfacePort
    + HostPaintPort
    + HostClipboardPort
    + HostToastPort
    + HostAccessibilityPort
{
}

/// Any type that implements the six non-texture ports is a `HostPorts`.
impl<T> HostPorts for T where
    T: HostInputPort
        + HostSurfacePort
        + HostPaintPort
        + HostClipboardPort
        + HostToastPort
        + HostAccessibilityPort
{
}
