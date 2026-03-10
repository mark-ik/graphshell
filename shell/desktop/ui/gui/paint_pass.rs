/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use super::*;

pub(super) fn paint(gui: &mut Gui, window: &Window) {
    begin_paint_pass(gui);
    gui.context.submit_frame(window);
    end_paint_pass(gui);
}

fn begin_paint_pass(gui: &Gui) {
    gui.rendering_context
        .make_current()
        .expect("Could not make RenderingContext current");
    gui.rendering_context
        .parent_context()
        .prepare_for_rendering();
}

fn end_paint_pass(gui: &Gui) {
    gui.rendering_context.parent_context().present();
}
