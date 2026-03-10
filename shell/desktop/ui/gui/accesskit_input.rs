/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

pub(super) fn handle_accesskit_initial_tree_requested(egui_ctx: &egui::Context) -> bool {
    set_accesskit_enabled(egui_ctx, true);
    true
}

pub(super) fn handle_accesskit_action_requested(
    egui_winit: &mut egui_winit::State,
    req: &egui::accesskit::ActionRequest,
) -> bool {
    forward_accesskit_action_request(egui_winit, req);
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
