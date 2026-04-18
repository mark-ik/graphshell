/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! iced-facing `HostPorts` implementation — M5 skeleton.
//!
//! Sibling of [`super::egui_host_ports`]. Currently a placeholder bundle
//! whose trait impls are no-ops; its purpose is to prove that
//! `GraphshellRuntime::tick` accepts a second host and to give M5 follow-on
//! work a concrete wiring target.
//!
//! As M5 phases land, each placeholder here swaps in an iced-native
//! delegation (native clipboard, iced-side toast queue, texture cache
//! against iced's image handle, accesskit bridge via iced_accesskit,
//! etc.). Until then the bundle is intentionally thin so `cargo check
//! --features iced-host` exercises the trait coverage without requiring
//! a full iced dependency tree.

use std::sync::Arc;

use crate::graph::NodeKey;
use crate::shell::desktop::render_backend::{BackendGraphicsContext, BackendViewportInPixels};
use crate::shell::desktop::ui::frame_model::ToastSpec;
use crate::shell::desktop::ui::host_ports::{
    HostAccessibilityPort, HostClipboardPort, HostInputPort, HostPaintPort, HostSurfacePort,
    HostTexturePort, HostToastPort,
};
use crate::shell::desktop::workbench::ux_replay::{HostEvent, ModifiersState};
use servo::WebViewId;

/// iced-side bundle of host port implementations.
///
/// An empty unit struct during M5 bring-up. Fields land as iced adapter
/// state matures (clipboard handle, toast queue, texture cache, etc.).
/// Downstream callers should construct the bundle fresh per tick, as
/// `EguiHostPorts` does, so there's no long-lived state to keep in sync.
pub(crate) struct IcedHostPorts;

// ---------------------------------------------------------------------------
// HostInputPort
// ---------------------------------------------------------------------------

impl HostInputPort for IcedHostPorts {
    fn poll_events(&mut self) -> Vec<HostEvent> {
        // todo(m5): translate iced events into HostEvent — mirror of
        // `HostEvent::from_egui_event`, consuming iced's keyboard / mouse /
        // window event types.
        Vec::new()
    }

    fn pointer_hover_position(&self) -> Option<egui::Pos2> {
        // todo(m5): read from iced's cursor tracking. (Note: the port
        // signature still leaks `egui::Pos2`; the M3.5 design flagged
        // this as a cosmetic leak iced will satisfy via a trivial
        // `iced::Point -> egui::Pos2` conversion.)
        None
    }

    fn wants_keyboard_input(&self) -> bool {
        // todo(m5): iced's text input focus tracking.
        false
    }

    fn wants_pointer_input(&self) -> bool {
        // todo(m5): iced's pointer capture state.
        false
    }

    fn modifiers(&self) -> ModifiersState {
        // todo(m5): read iced's keyboard modifier state.
        ModifiersState::default()
    }
}

// ---------------------------------------------------------------------------
// HostSurfacePort
// ---------------------------------------------------------------------------

impl HostSurfacePort for IcedHostPorts {
    fn present_surface(&mut self, _node_key: NodeKey) {
        // todo(m5): bump ViewerSurfaceRegistry and request an iced redraw.
    }

    fn retire_surface(&mut self, _node_key: NodeKey) {
        // todo(m5): retire the node's content surface via the shared
        // compositor adapter.
    }

    fn register_content_callback(
        &mut self,
        _node_key: NodeKey,
        _callback: Arc<dyn Fn(&BackendGraphicsContext, BackendViewportInPixels) + Send + Sync>,
    ) {
        // todo(m5): register on the shared CompositorAdapter; the callback
        // signature is already host-neutral (BackendGraphicsContext) so no
        // adapter work is needed here — just storage + invocation wiring.
    }

    fn unregister_content_callback(&mut self, _node_key: NodeKey) {
        // todo(m5): paired unregister.
    }
}

// ---------------------------------------------------------------------------
// HostPaintPort
// ---------------------------------------------------------------------------

impl HostPaintPort for IcedHostPorts {
    fn draw_overlay_stroke(
        &mut self,
        _node_key: NodeKey,
        _rect: egui::Rect,
        _stroke: egui::Stroke,
        _rounding: f32,
    ) {
        // todo(m5): route to iced's canvas/primitive stack. Per M3.5
        // design the egui::* overlay descriptors survive as a "cosmetic
        // leak" iced converts at the boundary.
    }

    fn draw_dashed_overlay_stroke(
        &mut self,
        _node_key: NodeKey,
        _rect: egui::Rect,
        _stroke: egui::Stroke,
    ) {
        // todo(m5): dashed-stroke equivalent on iced.
    }

    fn draw_overlay_glyphs(
        &mut self,
        _node_key: NodeKey,
        _rect: egui::Rect,
        _glyphs: &[crate::registries::atomic::lens::GlyphOverlay],
        _color: egui::Color32,
    ) {
        // todo(m5): glyph overlay on iced's text primitive.
    }

    fn draw_overlay_chrome_markers(
        &mut self,
        _node_key: NodeKey,
        _rect: egui::Rect,
        _stroke: egui::Stroke,
    ) {
        // todo(m5): tile-edge chrome markers on iced.
    }

    fn draw_degraded_receipt(&mut self, _rect: egui::Rect, _message: &str) {
        // todo(m5): in-tile receipt banner via iced text layer.
    }
}

// ---------------------------------------------------------------------------
// HostTexturePort
// ---------------------------------------------------------------------------

impl HostTexturePort for IcedHostPorts {
    // Placeholder handle type until the iced texture cache lands.
    // Chosen as `()` so M5 callers can detect "no textures yet" without
    // the iced crate being pulled in.
    type TextureHandle = ();

    fn load_texture(
        &mut self,
        _key: &str,
        _width: u32,
        _height: u32,
        _rgba: &[u8],
    ) -> Self::TextureHandle {
        // todo(m5): upload into iced's image cache and return a handle.
        unimplemented!("IcedHostPorts::load_texture wiring lands alongside texture cache")
    }

    fn texture(&self, _key: &str) -> Option<Self::TextureHandle> {
        // todo(m5): lookup in iced texture cache.
        None
    }

    fn drop_texture(&mut self, _key: &str) {
        // todo(m5): release from iced texture cache.
    }
}

// ---------------------------------------------------------------------------
// HostClipboardPort
// ---------------------------------------------------------------------------

impl HostClipboardPort for IcedHostPorts {
    fn get_text(&mut self) -> Option<String> {
        // todo(m5): iced exposes clipboard via `iced::clipboard::read`
        // inside update loops. M5 wiring will either propagate iced's
        // clipboard handle onto IcedHostPorts or route through an arboard
        // holder as the egui adapter does.
        None
    }

    fn set_text(&mut self, _text: &str) -> Result<(), String> {
        // todo(m5): as above.
        Err("iced clipboard not yet wired".to_string())
    }
}

// ---------------------------------------------------------------------------
// HostToastPort
// ---------------------------------------------------------------------------

impl HostToastPort for IcedHostPorts {
    fn enqueue(&mut self, _toast: ToastSpec) {
        // todo(m5): iced has no first-class toast system; M5 will pick
        // between iced_aw::notification and a hand-rolled overlay layer.
    }
}

// ---------------------------------------------------------------------------
// HostAccessibilityPort
// ---------------------------------------------------------------------------

impl HostAccessibilityPort for IcedHostPorts {
    fn inject_tree_update(
        &mut self,
        _webview_id: WebViewId,
        _update: servo::accesskit::TreeUpdate,
    ) {
        // M6 §5.2 follow-on: iced's accesskit integration is not yet
        // wired. Until it is, runtime-driven webview a11y tree updates
        // are dropped on the iced host. This is documented as the
        // blocker preventing chrome surfaces from landing in iced —
        // see 2026-04-17_chrome_port_cleanup_plan.md §5.2.
        //
        // The egui host's shape (buffer on `IcedHost` + drain in frame
        // prelude, mirroring `EguiHost::pending_webview_a11y_updates`)
        // is the target when the bridge lands.
    }

    fn request_focus(&mut self, _node_id: accesskit::NodeId) {
        // M6 §5.2 follow-on: same as above — iced focus requests
        // through winit's accesskit adapter land with the bridge.
    }
}

// ---------------------------------------------------------------------------
// HostPorts composite auto-satisfied via the blanket impl in host_ports.rs
// (HostTexturePort is not part of the composite because of its associated
// type). Call sites needing texture access bound `T: HostPorts +
// HostTexturePort` explicitly.
// ---------------------------------------------------------------------------
