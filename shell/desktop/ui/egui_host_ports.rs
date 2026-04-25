/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! egui-facing `HostPorts` implementation.
//!
//! This module is the **egui-side** bundle of host port implementations.
//! It gives the runtime access to egui-specific state (toasts, clipboard,
//! eventually texture caches, accesskit bridge) held on `EguiHost` through
//! the narrow port traits defined in [`super::host_ports`].
//!
//! ## Current wiring status
//!
//! - `HostToastPort::enqueue` — real: delegates to `egui_notify::Toasts`
//! - `HostClipboardPort::{get,set}_text` — real: delegates to `arboard::Clipboard`
//! - `HostSurfacePort::register_content_callback` — real: delegates to
//!   `CompositorAdapter::register_content_callback` with the GL callback bridge path.
//! - `HostSurfacePort::unregister_content_callback` — real: delegates to
//!   `CompositorAdapter::unregister_content_callback`.
//! - `HostInputPort::pointer_hover_position` — real: `ctx.input(|i| i.pointer.hover_pos())`.
//! - `HostInputPort::wants_keyboard_input` — real: `ctx.wants_keyboard_input()`.
//! - `HostInputPort::wants_pointer_input` — real: `ctx.wants_pointer_input()`.
//! - `HostInputPort::modifiers` — real: `ctx.input(|i| modifiers_from_egui(i.modifiers))`.
//! - `HostInputPort::poll_events` — intentional no-op: events enter via `FrameHostInput`,
//!   not through the port.
//! - `HostSurfacePort::present_surface` — real: enqueues the node key into
//!   `pending_present_requests`; the host drains this after tick returns and
//!   calls `ViewerSurfaceRegistry::bump_content_generation` for each entry.
//!   The deferred-queue pattern sidesteps the double-borrow on `runtime.tick`.
//! - `HostSurfacePort::retire_surface` — real: delegates to
//!   `CompositorAdapter::retire_node_content_resources` when `ui_render_backend`
//!   is `Some` (no-op in test contexts without a backend).
//! - All other ports — placeholders that compile but do not yet delegate.
//!   These wire up as each consuming phase migrates onto `runtime.tick`.
//!
//! Each `todo(m4.5)` comment marks a placeholder site awaiting wiring.

use std::collections::HashMap;
use std::sync::Arc;

use arboard::Clipboard;
use graphshell_runtime::{ToastSeverity, ToastSpec};

use crate::graph::NodeKey;
use crate::shell::desktop::render_backend::{BackendGraphicsContext, BackendViewportInPixels};
use crate::shell::desktop::ui::host_ports::{
    HostAccessibilityPort, HostClipboardPort, HostInputPort, HostPaintPort, HostSurfacePort,
    HostTexturePort, HostToastPort,
};
use crate::shell::desktop::render_backend::UiRenderBackendHandle;
use crate::shell::desktop::workbench::compositor_adapter::{
    CompositorAdapter, OverlayAffordanceStyle, PortablePoint, PortableRect,
    egui_rect_from_portable, egui_stroke_from_portable, portable_point_from_egui,
};
use crate::shell::desktop::workbench::ux_replay::{HostEvent, ModifiersState, modifiers_from_egui};
use servo::WebViewId;

/// egui-side bundle of host port implementations.
///
/// Holds the borrowed handles to egui-specific state (toasts, clipboard,
/// eventually texture caches and the accesskit bridge) the runtime needs to
/// drive. The struct is built fresh each tick at the host call site so each
/// port trait can delegate to live state without requiring `EguiHost` to
/// expose internals through wider visibility.
pub(crate) struct EguiHostPorts<'a> {
    /// Egui-side toast notification queue.
    pub(crate) toasts: &'a mut egui_notify::Toasts,

    /// Lazily-initialized arboard clipboard handle. The port lazy-inits
    /// on first write via `HostClipboardPort::set_text`.
    pub(crate) clipboard: &'a mut Option<Clipboard>,

    /// Per-webview accesskit tree updates that have been received but
    /// not yet injected into egui's accessibility surface. The host's
    /// prelude drains this map every frame via
    /// `accessibility::inject_webview_a11y_updates`.
    pub(crate) pending_webview_a11y_updates:
        &'a mut HashMap<WebViewId, servo::accesskit::TreeUpdate>,

    /// Accesskit focus requests emitted from the runtime this frame.
    /// The host drains these in its frame prelude and forwards through
    /// egui_winit's accesskit bridge. Empty until the runtime has a
    /// keyboard-nav code path that needs to move focus to a specific
    /// node; the port wiring lands now so the first caller doesn't
    /// need to revisit the port surface.
    pub(crate) pending_accesskit_focus_requests: &'a mut Vec<accesskit::NodeId>,

    /// Backend handle for GPU resource operations (native texture
    /// retirement, etc.). `None` in test contexts that don't need
    /// backend-backed surface management.
    pub(crate) ui_render_backend: Option<&'a mut UiRenderBackendHandle>,

    /// Deferred present requests accumulated during `runtime.tick`.
    ///
    /// `HostSurfacePort::present_surface` cannot call
    /// `ViewerSurfaceRegistry::bump_content_generation` directly because the
    /// registry lives on `GraphshellRuntime`, which is already mutably borrowed
    /// by `tick`. Instead, node keys are pushed here and the host drains the
    /// vec after tick returns, calling `bump_content_generation` for each.
    pub(crate) pending_present_requests: &'a mut Vec<NodeKey>,

    /// egui context used by `HostPaintPort` implementations.
    /// `egui::Context` is internally `Arc`-backed — clone is a cheap
    /// reference count bump. `None` in test contexts that don't exercise
    /// painting.
    pub(crate) ctx: Option<egui::Context>,
}

// ---------------------------------------------------------------------------
// HostInputPort
// ---------------------------------------------------------------------------

impl<'a> HostInputPort for EguiHostPorts<'a> {
    fn poll_events(&mut self) -> Vec<HostEvent> {
        // Intentionally empty: egui events are translated once per frame by
        // `build_frame_host_input` and passed to `runtime.tick` via
        // `FrameHostInput.events` before this port is constructed. There are
        // no mid-tick events to drain on the egui path.
        Vec::new()
    }

    fn pointer_hover_position(&self) -> Option<PortablePoint> {
        self.ctx
            .as_ref()?
            .input(|i| i.pointer.hover_pos())
            .map(portable_point_from_egui)
    }

    fn wants_keyboard_input(&self) -> bool {
        self.ctx
            .as_ref()
            .map(|ctx| ctx.wants_keyboard_input())
            .unwrap_or(false)
    }

    fn wants_pointer_input(&self) -> bool {
        self.ctx
            .as_ref()
            .map(|ctx| ctx.wants_pointer_input())
            .unwrap_or(false)
    }

    fn modifiers(&self) -> ModifiersState {
        self.ctx
            .as_ref()
            .map(|ctx| ctx.input(|i| modifiers_from_egui(i.modifiers)))
            .unwrap_or_default()
    }
}

// ---------------------------------------------------------------------------
// HostSurfacePort
// ---------------------------------------------------------------------------

impl<'a> HostSurfacePort for EguiHostPorts<'a> {
    fn present_surface(&mut self, node_key: NodeKey) {
        // Defer the bump_content_generation call: the registry lives on
        // GraphshellRuntime which is already mutably borrowed by tick. The host
        // drains this queue after tick returns and calls bump_content_generation
        // for each entry. Repaint scheduling is a host-side follow-on (M4.5).
        self.pending_present_requests.push(node_key);
    }

    fn retire_surface(&mut self, node_key: NodeKey) {
        if let Some(backend) = self.ui_render_backend.as_deref_mut() {
            CompositorAdapter::retire_node_content_resources(backend, node_key);
        }
    }

    fn register_content_callback(
        &mut self,
        node_key: NodeKey,
        callback: Arc<dyn Fn(&BackendGraphicsContext, BackendViewportInPixels) + Send + Sync>,
    ) {
        CompositorAdapter::register_content_callback(
            node_key,
            "gl.render_to_parent_callback",
            "gl_callback",
            callback,
        );
    }

    fn unregister_content_callback(&mut self, node_key: NodeKey) {
        CompositorAdapter::unregister_content_callback(node_key);
    }
}

// ---------------------------------------------------------------------------
// HostPaintPort
// ---------------------------------------------------------------------------

impl<'a> HostPaintPort for EguiHostPorts<'a> {
    fn draw_overlay_stroke(
        &mut self,
        node_key: NodeKey,
        rect: PortableRect,
        stroke: graph_canvas::packet::Stroke,
        rounding: f32,
    ) {
        let Some(ctx) = self.ctx.as_ref() else { return };
        CompositorAdapter::draw_overlay_stroke(
            ctx,
            node_key,
            egui_rect_from_portable(rect),
            rounding,
            egui_stroke_from_portable(stroke),
        );
    }

    fn draw_dashed_overlay_stroke(
        &mut self,
        node_key: NodeKey,
        rect: PortableRect,
        stroke: graph_canvas::packet::Stroke,
    ) {
        let Some(ctx) = self.ctx.as_ref() else { return };
        CompositorAdapter::draw_dashed_overlay_stroke(
            ctx,
            node_key,
            egui_rect_from_portable(rect),
            egui_stroke_from_portable(stroke),
        );
    }

    fn draw_overlay_glyphs(
        &mut self,
        node_key: NodeKey,
        rect: PortableRect,
        glyphs: &[crate::registries::atomic::lens::GlyphOverlay],
        color: graph_canvas::packet::Color,
    ) {
        let Some(ctx) = self.ctx.as_ref() else { return };
        let egui_color = egui::Color32::from_rgba_premultiplied(
            (color.r * 255.0).round().clamp(0.0, 255.0) as u8,
            (color.g * 255.0).round().clamp(0.0, 255.0) as u8,
            (color.b * 255.0).round().clamp(0.0, 255.0) as u8,
            (color.a * 255.0).round().clamp(0.0, 255.0) as u8,
        );
        CompositorAdapter::draw_overlay_glyphs(
            ctx,
            node_key,
            egui_rect_from_portable(rect),
            glyphs,
            egui_color,
            OverlayAffordanceStyle::RectStroke,
        );
    }

    fn draw_overlay_chrome_markers(
        &mut self,
        node_key: NodeKey,
        rect: PortableRect,
        stroke: graph_canvas::packet::Stroke,
    ) {
        let Some(ctx) = self.ctx.as_ref() else { return };
        CompositorAdapter::draw_overlay_chrome_markers(
            ctx,
            node_key,
            egui_rect_from_portable(rect),
            egui_stroke_from_portable(stroke),
        );
    }

    fn draw_degraded_receipt(&mut self, rect: PortableRect, message: &str) {
        let Some(ctx) = self.ctx.as_ref() else { return };
        let egui_rect = egui_rect_from_portable(rect);
        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("graphshell_degraded_receipt"),
        ));
        painter.rect_filled(
            egui_rect.shrink(2.0),
            4.0,
            egui::Color32::from_black_alpha(200),
        );
        painter.text(
            egui_rect.center(),
            egui::Align2::CENTER_CENTER,
            message,
            egui::FontId::proportional(11.0),
            egui::Color32::from_white_alpha(230),
        );
    }
}

// ---------------------------------------------------------------------------
// HostTexturePort
// ---------------------------------------------------------------------------

impl<'a> HostTexturePort for EguiHostPorts<'a> {
    type TextureHandle = egui::TextureHandle;

    fn load_texture(
        &mut self,
        _key: &str,
        _width: u32,
        _height: u32,
        _rgba: &[u8],
    ) -> Self::TextureHandle {
        // todo(m4.5): delegate to egui::Context::load_texture via a fixed namespace.
        unimplemented!("EguiHostPorts::load_texture wiring lands in M4.5")
    }

    fn texture(&self, _key: &str) -> Option<Self::TextureHandle> {
        // todo(m4.5): look up in the renderer_favicon_textures cache.
        None
    }

    fn drop_texture(&mut self, _key: &str) {
        // todo(m4.5): remove from cache + let egui reclaim.
    }
}

// ---------------------------------------------------------------------------
// HostClipboardPort
// ---------------------------------------------------------------------------

impl<'a> HostClipboardPort for EguiHostPorts<'a> {
    fn get_text(&mut self) -> Option<String> {
        let cb = self.clipboard.as_mut()?;
        cb.get_text().ok()
    }

    fn set_text(&mut self, text: &str) -> Result<(), String> {
        // Lazy-initialize on first write so iced-style hosts that pre-seed
        // the holder and egui-style hosts that defer construction behave
        // identically at the port boundary.
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

impl<'a> HostToastPort for EguiHostPorts<'a> {
    fn enqueue(&mut self, toast: ToastSpec) {
        let ToastSpec {
            severity,
            message,
            duration,
        } = toast;
        let entry = match severity {
            ToastSeverity::Info => self.toasts.info(message),
            ToastSeverity::Success => self.toasts.success(message),
            ToastSeverity::Warning => self.toasts.warning(message),
            ToastSeverity::Error => self.toasts.error(message),
        };
        // Honour explicit durations; `None` means "use egui_notify default",
        // which mirrors prior `toasts.error(...)` / `toasts.success(...)`
        // call-site behaviour that did not customise duration.
        if let Some(duration) = duration {
            entry.duration(Some(duration));
        }
    }
}

// ---------------------------------------------------------------------------
// HostAccessibilityPort
// ---------------------------------------------------------------------------

impl<'a> HostAccessibilityPort for EguiHostPorts<'a> {
    fn inject_tree_update(&mut self, webview_id: WebViewId, update: servo::accesskit::TreeUpdate) {
        enqueue_pending_webview_a11y_update(self.pending_webview_a11y_updates, webview_id, update);
    }

    fn request_focus(&mut self, node_id: accesskit::NodeId) {
        // Enqueue for host-side consumption. The egui frame prelude
        // forwards these through `egui_winit`'s accesskit adapter; iced
        // consumers drain the same queue shape through
        // `iced_accesskit` once that bridge is wired (M6 §5.2
        // follow-on).
        self.pending_accesskit_focus_requests.push(node_id);
    }
}

/// Insert or replace a pending webview accesskit tree update + record
/// the diagnostic channel entry. Shared helper so the port path and
/// the existing `EguiHost::notify_accessibility_tree_update` entry
/// point stay in lockstep — most-recent-wins semantics, identical
/// diagnostic recording.
pub(crate) fn enqueue_pending_webview_a11y_update(
    pending: &mut HashMap<WebViewId, servo::accesskit::TreeUpdate>,
    webview_id: WebViewId,
    update: servo::accesskit::TreeUpdate,
) {
    let replaced_existing = pending.insert(webview_id, update).is_some();
    let _ = replaced_existing;

    #[cfg(feature = "diagnostics")]
    if let Some(tree_update) = pending.get(&webview_id) {
        crate::shell::desktop::ui::gui::accessibility::record_webview_a11y_update_queued(
            webview_id,
            tree_update,
            replaced_existing,
            pending.len(),
        );
    }
}

// ---------------------------------------------------------------------------
// HostPorts composite is auto-satisfied via the blanket impl in host_ports.rs
// once all six non-texture traits above are implemented.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::desktop::ui::host_ports::HostAccessibilityPort;

    fn make_ports<'a>(
        toasts: &'a mut egui_notify::Toasts,
        clipboard: &'a mut Option<Clipboard>,
        a11y_updates: &'a mut HashMap<WebViewId, servo::accesskit::TreeUpdate>,
        focus_requests: &'a mut Vec<accesskit::NodeId>,
        present_requests: &'a mut Vec<NodeKey>,
    ) -> EguiHostPorts<'a> {
        EguiHostPorts {
            toasts,
            clipboard,
            pending_webview_a11y_updates: a11y_updates,
            pending_accesskit_focus_requests: focus_requests,
            ui_render_backend: None,
            pending_present_requests: present_requests,
            ctx: None,
        }
    }

    fn install_pipeline_namespace_for_tests() {
        use base::id::{PIPELINE_NAMESPACE, PipelineNamespace, TEST_NAMESPACE};
        PIPELINE_NAMESPACE.with(|tls| {
            if tls.get().is_none() {
                PipelineNamespace::install(TEST_NAMESPACE);
            }
        });
    }

    fn test_webview_id() -> WebViewId {
        use base::id::PainterId;
        install_pipeline_namespace_for_tests();
        WebViewId::new(PainterId::next())
    }

    /// Minimal tree update useful as a test fixture.
    fn stub_tree_update() -> servo::accesskit::TreeUpdate {
        servo::accesskit::TreeUpdate {
            nodes: Vec::new(),
            tree: None,
            tree_id: servo::accesskit::TreeId::ROOT,
            focus: servo::accesskit::NodeId(0),
        }
    }

    /// Guard that resets the global a11y bridge health state on drop so
    /// each test starts from a clean slate and doesn't leak queue size
    /// into later tests (notably
    /// `accessibility_bridge_health_snapshot_captures_health_metrics`
    /// which asserts `update_queue_size == 0`).
    struct A11yBridgeHealthGuard;
    impl A11yBridgeHealthGuard {
        fn new() -> Self {
            #[cfg(feature = "diagnostics")]
            crate::shell::desktop::ui::gui::accessibility::reset_webview_accessibility_bridge_health_state_for_tests();
            Self
        }
    }
    impl Drop for A11yBridgeHealthGuard {
        fn drop(&mut self) {
            #[cfg(feature = "diagnostics")]
            crate::shell::desktop::ui::gui::accessibility::reset_webview_accessibility_bridge_health_state_for_tests();
        }
    }

    #[test]
    fn inject_tree_update_enqueues_pending_update() {
        let _guard = A11yBridgeHealthGuard::new();
        let webview_id = test_webview_id();

        let mut toasts = egui_notify::Toasts::default();
        let mut clipboard: Option<Clipboard> = None;
        let mut pending: HashMap<WebViewId, servo::accesskit::TreeUpdate> = HashMap::new();
        let mut focus: Vec<accesskit::NodeId> = Vec::new();
        let mut present: Vec<NodeKey> = Vec::new();
        let mut ports = make_ports(&mut toasts, &mut clipboard, &mut pending, &mut focus, &mut present);

        ports.inject_tree_update(webview_id, stub_tree_update());

        assert_eq!(pending.len(), 1);
        assert!(pending.contains_key(&webview_id));
    }

    #[test]
    fn inject_tree_update_replaces_existing_update_for_same_webview() {
        let _guard = A11yBridgeHealthGuard::new();
        let webview_id = test_webview_id();

        let mut toasts = egui_notify::Toasts::default();
        let mut clipboard: Option<Clipboard> = None;
        let mut pending: HashMap<WebViewId, servo::accesskit::TreeUpdate> = HashMap::new();
        let mut focus: Vec<accesskit::NodeId> = Vec::new();
        let mut present: Vec<NodeKey> = Vec::new();
        let mut ports = make_ports(&mut toasts, &mut clipboard, &mut pending, &mut focus, &mut present);

        ports.inject_tree_update(webview_id, stub_tree_update());
        ports.inject_tree_update(webview_id, stub_tree_update());

        // Same webview id → most-recent-wins, still length 1.
        assert_eq!(pending.len(), 1);
    }

    #[test]
    fn request_focus_enqueues_node_id() {
        let mut toasts = egui_notify::Toasts::default();
        let mut clipboard: Option<Clipboard> = None;
        let mut pending: HashMap<WebViewId, servo::accesskit::TreeUpdate> = HashMap::new();
        let mut focus: Vec<accesskit::NodeId> = Vec::new();
        let mut present: Vec<NodeKey> = Vec::new();
        let mut ports = make_ports(&mut toasts, &mut clipboard, &mut pending, &mut focus, &mut present);

        ports.request_focus(accesskit::NodeId(17));
        ports.request_focus(accesskit::NodeId(42));

        assert_eq!(focus, vec![accesskit::NodeId(17), accesskit::NodeId(42)]);
    }

    #[test]
    fn present_surface_enqueues_node_key_for_host_drain() {
        use crate::shell::desktop::ui::host_ports::HostSurfacePort;

        let mut toasts = egui_notify::Toasts::default();
        let mut clipboard: Option<Clipboard> = None;
        let mut pending: HashMap<WebViewId, servo::accesskit::TreeUpdate> = HashMap::new();
        let mut focus: Vec<accesskit::NodeId> = Vec::new();
        let mut present: Vec<NodeKey> = Vec::new();
        let mut ports = make_ports(&mut toasts, &mut clipboard, &mut pending, &mut focus, &mut present);

        let key_a = NodeKey::new(42);
        let key_b = NodeKey::new(99);
        ports.present_surface(key_a);
        ports.present_surface(key_b);

        // Both keys queued in order; host drains after tick to call
        // ViewerSurfaceRegistry::bump_content_generation for each.
        assert_eq!(present, vec![key_a, key_b]);
    }

    #[test]
    fn host_input_port_returns_none_for_hover_position_without_ctx() {
        use crate::shell::desktop::ui::host_ports::HostInputPort;

        let mut toasts = egui_notify::Toasts::default();
        let mut clipboard: Option<Clipboard> = None;
        let mut pending: HashMap<WebViewId, servo::accesskit::TreeUpdate> = HashMap::new();
        let mut focus: Vec<accesskit::NodeId> = Vec::new();
        let mut present: Vec<NodeKey> = Vec::new();
        let ports = make_ports(&mut toasts, &mut clipboard, &mut pending, &mut focus, &mut present);

        // ctx is None in test contexts — graceful fallback expected.
        assert!(ports.pointer_hover_position().is_none());
        assert!(!ports.wants_keyboard_input());
        assert!(!ports.wants_pointer_input());
        assert_eq!(ports.modifiers(), ModifiersState::default());
    }

    #[test]
    fn host_input_port_reads_modifiers_from_egui_ctx() {
        use crate::shell::desktop::ui::host_ports::HostInputPort;

        let ctx = egui::Context::default();
        // Simulate a frame so egui processes input; inject shift modifier.
        ctx.begin_pass(egui::RawInput {
            modifiers: egui::Modifiers {
                shift: true,
                ..Default::default()
            },
            ..Default::default()
        });
        ctx.end_pass();

        let mut toasts = egui_notify::Toasts::default();
        let mut clipboard: Option<Clipboard> = None;
        let mut pending: HashMap<WebViewId, servo::accesskit::TreeUpdate> = HashMap::new();
        let mut focus: Vec<accesskit::NodeId> = Vec::new();
        let mut present: Vec<NodeKey> = Vec::new();
        let mut ports = make_ports(&mut toasts, &mut clipboard, &mut pending, &mut focus, &mut present);
        ports.ctx = Some(ctx);

        let modifiers = ports.modifiers();
        assert!(modifiers.shift, "shift modifier must propagate from egui ctx");
        assert!(!modifiers.ctrl);
        assert!(!modifiers.alt);
    }
}
