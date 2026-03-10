/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use super::*;

pub(super) fn handle_accesskit_event(
    gui: &mut Gui,
    event: &egui_winit::accesskit_winit::WindowEvent,
) -> bool {
    match event {
        egui_winit::accesskit_winit::WindowEvent::InitialTreeRequested => {
            accesskit_input::handle_accesskit_initial_tree_requested(gui.context.egui_context())
        }
        egui_winit::accesskit_winit::WindowEvent::ActionRequested(req) => {
            accesskit_input::handle_accesskit_action_requested(
                gui.context.egui_winit_state_mut(),
                req,
            )
        }
        egui_winit::accesskit_winit::WindowEvent::AccessibilityDeactivated => {
            accesskit_input::handle_accesskit_deactivated(gui.context.egui_context())
        }
    }
}
