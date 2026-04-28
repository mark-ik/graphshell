/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Re-export shim — real implementation lives in the ungated
//! `crate::shell::desktop::ui::gui_frame_inbox` so iced-host builds
//! can reach it without gating on `servo-engine`.

pub(crate) use crate::shell::desktop::ui::gui_frame_inbox::{
    GuiFrameInbox, spawn_gui_frame_inbox,
};
