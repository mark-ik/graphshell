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

use std::sync::Arc;

use arboard::Clipboard;

use crate::graph::NodeKey;
use crate::shell::desktop::render_backend::{BackendGraphicsContext, BackendViewportInPixels};
use crate::shell::desktop::ui::frame_model::{ToastSeverity, ToastSpec};
use crate::shell::desktop::ui::host_ports::{
    HostAccessibilityPort, HostClipboardPort, HostInputPort, HostPaintPort, HostSurfacePort,
    HostTexturePort, HostToastPort,
};
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
}

// ---------------------------------------------------------------------------
// HostInputPort
// ---------------------------------------------------------------------------

impl<'a> HostInputPort for EguiHostPorts<'a> {
    fn poll_events(&mut self) -> Vec<HostEvent> {
        // todo(m4.5): translate egui's accumulated input into HostEvent vocabulary.
        Vec::new()
    }

    fn pointer_hover_position(&self) -> Option<egui::Pos2> {
        // todo(m4.5): read from egui::Context::input(|i| i.pointer.hover_pos()).
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
        _rect: egui::Rect,
        _stroke: egui::Stroke,
        _rounding: f32,
    ) {
        // todo(m4.5): delegate to CompositorAdapter::draw_overlay_stroke.
    }

    fn draw_dashed_overlay_stroke(
        &mut self,
        _node_key: NodeKey,
        _rect: egui::Rect,
        _stroke: egui::Stroke,
    ) {
        // todo(m4.5): delegate to CompositorAdapter::draw_dashed_overlay_stroke.
    }

    fn draw_overlay_glyphs(
        &mut self,
        _node_key: NodeKey,
        _rect: egui::Rect,
        _glyphs: &[crate::registries::atomic::lens::GlyphOverlay],
        _color: egui::Color32,
    ) {
        // todo(m4.5): delegate to CompositorAdapter::draw_overlay_glyphs.
    }

    fn draw_overlay_chrome_markers(
        &mut self,
        _node_key: NodeKey,
        _rect: egui::Rect,
        _stroke: egui::Stroke,
    ) {
        // todo(m4.5): delegate to CompositorAdapter::draw_overlay_chrome_markers.
    }

    fn draw_degraded_receipt(&mut self, _rect: egui::Rect, _message: &str) {
        // todo(m4.5): implement via egui::Painter on a foreground layer.
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
    fn inject_tree_update(
        &mut self,
        _webview_id: WebViewId,
        _update: servo::accesskit::TreeUpdate,
    ) {
        // todo(m4.5): insert into EguiHost::pending_webview_a11y_updates and
        // flush into egui's accesskit surface on next frame.
    }

    fn request_focus(&mut self, _node_id: accesskit::NodeId) {
        // todo(m4.5): send an accesskit focus request through egui_winit.
    }
}

// ---------------------------------------------------------------------------
// HostPorts composite is auto-satisfied via the blanket impl in host_ports.rs
// once all six non-texture traits above are implemented.
// ---------------------------------------------------------------------------
