/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use super::{Gui, accessibility};

pub(super) fn handle_accesskit_initial_tree_requested(egui_ctx: &egui::Context) -> bool {
    set_accesskit_enabled(egui_ctx, true);
    true
}

pub(super) fn handle_accesskit_action_requested(
    gui: &mut Gui,
    req: &egui::accesskit::ActionRequest,
) -> bool {
    match accessibility::resolve_uxtree_accesskit_action(&gui.graph_app, req) {
        Some(accessibility::UxTreeAccesskitDispatch::FocusGraphSurface) => {
            gui.focus_graph_surface();
            crate::shell::desktop::render_backend::UiRenderBackendContract::egui_context(
                &gui.context,
            )
            .request_repaint();
        }
        Some(accessibility::UxTreeAccesskitDispatch::FocusGraphReaderMapItem { node_key }) => {
            gui.graph_app.graph_reader_focus_map_node(node_key);
            gui.focus_graph_surface();
            crate::shell::desktop::render_backend::UiRenderBackendContract::egui_context(
                &gui.context,
            )
            .request_repaint();
        }
        Some(accessibility::UxTreeAccesskitDispatch::EnterGraphReaderRoom { node_key }) => {
            gui.graph_app.graph_reader_enter_room(node_key);
            gui.focus_graph_surface();
            crate::shell::desktop::render_backend::UiRenderBackendContract::egui_context(
                &gui.context,
            )
            .request_repaint();
        }
        Some(accessibility::UxTreeAccesskitDispatch::ReturnGraphReaderToMap) => {
            gui.graph_app.graph_reader_return_to_map();
            gui.focus_graph_surface();
            crate::shell::desktop::render_backend::UiRenderBackendContract::egui_context(
                &gui.context,
            )
            .request_repaint();
        }
        Some(accessibility::UxTreeAccesskitDispatch::Unsupported) => {}
        None => forward_accesskit_action_request(
            crate::shell::desktop::render_backend::UiRenderBackendContract::egui_winit_state_mut(
                &mut gui.context,
            ),
            req,
        ),
    }
    true
}

pub(super) fn handle_accesskit_deactivated(egui_ctx: &egui::Context) -> bool {
    set_accesskit_enabled(egui_ctx, false);
    false
}

pub(super) fn clamp_zoom_factor(factor: f32) -> f32 {
    if factor.is_finite() {
        factor.clamp(0.25, 4.0)
    } else {
        1.0
    }
}

fn set_accesskit_enabled(egui_ctx: &egui::Context, enabled: bool) {
    if enabled {
        egui_ctx.enable_accesskit();
    } else {
        egui_ctx.disable_accesskit();
    }
}

fn forward_accesskit_action_request(
    egui_winit: &mut egui_winit::State,
    req: &egui::accesskit::ActionRequest,
) {
    egui_winit.on_accesskit_action_request(req.clone());
}

