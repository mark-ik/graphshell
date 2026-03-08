/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use super::*;

pub(super) fn on_window_event(
    gui: &mut Gui,
    winit_window: &Window,
    event: &WindowEvent,
) -> EventResponse {
    let mut response = gui.context.handle_window_event(winit_window, event);

    // When no node-viewer tile is active, consume user input events so they
    // never reach an inactive/hidden runtime viewer.
    if !gui.has_active_node_pane() && should_consume_when_no_active_node(event) {
        response.consumed = true;
    }

    response
}

fn should_consume_when_no_active_node(event: &WindowEvent) -> bool {
    matches!(
        event,
        WindowEvent::KeyboardInput { .. }
            | WindowEvent::ModifiersChanged(_)
            | WindowEvent::MouseInput { .. }
            | WindowEvent::CursorMoved { .. }
            | WindowEvent::CursorLeft { .. }
            | WindowEvent::MouseWheel { .. }
            | WindowEvent::Touch(_)
            | WindowEvent::PinchGesture { .. }
    )
}