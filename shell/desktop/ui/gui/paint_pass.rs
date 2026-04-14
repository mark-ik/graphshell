/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use super::*;

use crate::shell::desktop::render_backend::{
    begin_ui_render_backend_paint, end_ui_render_backend_paint,
};

pub(super) fn paint(gui: &mut Gui, window: &Window) {
    begin_paint_pass(gui);
    gui.context.submit_frame(window);
    end_paint_pass(gui);
}

fn begin_paint_pass(gui: &Gui) {
    begin_ui_render_backend_paint(gui.rendering_context.as_ref());
}

fn end_paint_pass(gui: &Gui) {
    end_ui_render_backend_paint(gui.rendering_context.as_ref());
}

