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
//! - All other ports — placeholders that compile but do not yet delegate.
//!   These wire up as each consuming phase migrates onto `runtime.tick`.
//!
//! Each `todo(m4.5)` comment marks a placeholder site awaiting wiring.

use std::collections::HashMap;
use std::sync::Arc;

use arboard::Clipboard;

use crate::graph::NodeKey;
use crate::shell::desktop::render_backend::{BackendGraphicsContext, BackendViewportInPixels};
use crate::shell::desktop::ui::frame_model::{ToastSeverity, ToastSpec};
use crate::shell::desktop::ui::host_ports::{
    HostAccessibilityPort, HostClipboardPort, HostInputPort, HostPaintPort, HostSurfacePort,
    HostTexturePort, HostToastPort,
};
use crate::shell::desktop::workbench::compositor_adapter::{PortablePoint, PortableRect};
use crate::shell::desktop::workbench::ux_replay::{HostEvent, ModifiersState};
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
}

// ---------------------------------------------------------------------------
// HostInputPort
// ---------------------------------------------------------------------------

impl<'a> HostInputPort for EguiHostPorts<'a> {
    fn poll_events(&mut self) -> Vec<HostEvent> {
        // todo(m4.5): translate egui's accumulated input into HostEvent vocabulary.
        Vec::new()
    }

    fn pointer_hover_position(&self) -> Option<PortablePoint> {
        // todo(m4.5): read from egui::Context::input(|i| i.pointer.hover_pos())
        // and convert via `portable_point_from_egui`.
        None
    }

    fn wants_keyboard_input(&self) -> bool {
        // todo(m4.5): ctx.wants_keyboard_input().
        false
    }

    fn wants_pointer_input(&self) -> bool {
        // todo(m4.5): ctx.wants_pointer_input().
        false
    }

    fn modifiers(&self) -> ModifiersState {
        // todo(m4.5): read egui::Context::input(|i| i.modifiers) and translate.
        ModifiersState::default()
    }
}

// ---------------------------------------------------------------------------
// HostSurfacePort
// ---------------------------------------------------------------------------

impl<'a> HostSurfacePort for EguiHostPorts<'a> {
    fn present_surface(&mut self, _node_key: NodeKey) {
        // todo(m4.5): bump_content_generation on the ViewerSurfaceRegistry
        // and request a repaint from the egui context.
    }

    fn retire_surface(&mut self, _node_key: NodeKey) {
        // todo(m4.5): CompositorAdapter::retire_node_content_resources(...).
    }

    fn register_content_callback(
        &mut self,
        _node_key: NodeKey,
        _callback: Arc<dyn Fn(&BackendGraphicsContext, BackendViewportInPixels) + Send + Sync>,
    ) {
        // todo(m4.5): CompositorAdapter::register_content_callback(node_key, callback).
    }

    fn unregister_content_callback(&mut self, _node_key: NodeKey) {
        // todo(m4.5): CompositorAdapter::unregister_content_callback(node_key).
    }
}

// ---------------------------------------------------------------------------
// HostPaintPort
// ---------------------------------------------------------------------------

impl<'a> HostPaintPort for EguiHostPorts<'a> {
    fn draw_overlay_stroke(
        &mut self,
        _node_key: NodeKey,
        _rect: PortableRect,
        _stroke: graph_canvas::packet::Stroke,
        _rounding: f32,
    ) {
        // todo(m4.5): delegate to CompositorAdapter::draw_overlay_stroke via
        // egui_rect_from_portable / egui_stroke_from_portable.
    }

    fn draw_dashed_overlay_stroke(
        &mut self,
        _node_key: NodeKey,
        _rect: PortableRect,
        _stroke: graph_canvas::packet::Stroke,
    ) {
        // todo(m4.5): delegate to CompositorAdapter::draw_dashed_overlay_stroke
        // via the boundary helpers.
    }

    fn draw_overlay_glyphs(
        &mut self,
        _node_key: NodeKey,
        _rect: PortableRect,
        _glyphs: &[crate::registries::atomic::lens::GlyphOverlay],
        _color: graph_canvas::packet::Color,
    ) {
        // todo(m4.5): delegate to CompositorAdapter::draw_overlay_glyphs via
        // the boundary helpers.
    }

    fn draw_overlay_chrome_markers(
        &mut self,
        _node_key: NodeKey,
        _rect: PortableRect,
        _stroke: graph_canvas::packet::Stroke,
    ) {
        // todo(m4.5): delegate to CompositorAdapter::draw_overlay_chrome_markers
        // via the boundary helpers.
    }

    fn draw_degraded_receipt(&mut self, _rect: PortableRect, _message: &str) {
        // todo(m4.5): implement via egui::Painter on a foreground layer
        // after converting via egui_rect_from_portable.
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
    ) -> EguiHostPorts<'a> {
        EguiHostPorts {
            toasts,
            clipboard,
            pending_webview_a11y_updates: a11y_updates,
            pending_accesskit_focus_requests: focus_requests,
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
        let mut ports = make_ports(&mut toasts, &mut clipboard, &mut pending, &mut focus);

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
        let mut ports = make_ports(&mut toasts, &mut clipboard, &mut pending, &mut focus);

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
        let mut ports = make_ports(&mut toasts, &mut clipboard, &mut pending, &mut focus);

        ports.request_focus(accesskit::NodeId(17));
        ports.request_focus(accesskit::NodeId(42));

        assert_eq!(focus, vec![accesskit::NodeId(17), accesskit::NodeId(42)]);
    }
}
