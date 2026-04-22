/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Per-pane toolbar draft bookkeeping.
//!
//! Pulled off `EguiHost` in M6 §4.3 so iced can call the same logic.
//! The two entry points mirror the egui-era methods on
//! `EguiHost::persist_active_toolbar_draft` and
//! `EguiHost::sync_active_toolbar_draft`; they operate purely on
//! `GraphshellRuntime` state and a `next_active_pane` hint (which the
//! host resolves from its window/focus system).

use crate::shell::desktop::ui::gui_state::GraphshellRuntime;
use crate::shell::desktop::workbench::pane_model::PaneId;

/// Snapshot the current toolbar state into the active pane's draft slot.
///
/// Called before focus leaves the active pane so the user's in-progress
/// edits survive the pane switch and are restored when focus returns.
pub(crate) fn persist_active_toolbar_draft(runtime: &mut GraphshellRuntime) {
    let Some(active_pane) = runtime.focus_authority.pane_activation else {
        return;
    };
    runtime
        .toolbar_drafts
        .insert(active_pane, runtime.toolbar_state.editable.clone());
}

/// Switch the live toolbar state to the draft belonging to
/// `next_active_pane`, persisting the outgoing pane's draft first.
///
/// No-op when `next_active_pane` matches the current `pane_activation`.
pub(crate) fn sync_active_toolbar_draft(
    runtime: &mut GraphshellRuntime,
    next_active_pane: Option<PaneId>,
) {
    if runtime.focus_authority.pane_activation == next_active_pane {
        return;
    }

    persist_active_toolbar_draft(runtime);
    crate::shell::desktop::ui::gui::apply_pane_activation_focus_state(runtime, next_active_pane);

    let Some(active_pane) = next_active_pane else {
        return;
    };

    let draft = runtime
        .toolbar_drafts
        .entry(active_pane)
        .or_insert_with(|| runtime.toolbar_state.editable.clone())
        .clone();
    runtime.toolbar_state.editable = draft;
}
