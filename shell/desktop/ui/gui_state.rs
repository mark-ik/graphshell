/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::graph::NodeKey;
use crate::shell::desktop::workbench::pane_model::PaneId;
use crate::shell::desktop::ui::toolbar::toolbar_ui::OmnibarSearchSession;
use servo::{LoadStatus, WebViewId};

pub(super) struct ToolbarState {
    pub(super) location: String,
    pub(super) location_dirty: bool,
    pub(super) location_submitted: bool,
    pub(super) show_clear_data_confirm: bool,
    pub(super) load_status: LoadStatus,
    pub(super) status_text: Option<String>,
    pub(super) can_go_back: bool,
    pub(super) can_go_forward: bool,
}

#[derive(Clone)]
pub(super) struct ToolbarDraft {
    pub(super) location: String,
    pub(super) location_dirty: bool,
    pub(super) location_submitted: bool,
}

impl ToolbarDraft {
    pub(super) fn from_toolbar_state(toolbar_state: &ToolbarState) -> Self {
        Self {
            location: toolbar_state.location.clone(),
            location_dirty: toolbar_state.location_dirty,
            location_submitted: toolbar_state.location_submitted,
        }
    }

    pub(super) fn apply_to_toolbar_state(&self, toolbar_state: &mut ToolbarState) {
        toolbar_state.location = self.location.clone();
        toolbar_state.location_dirty = self.location_dirty;
        toolbar_state.location_submitted = self.location_submitted;
    }
}

pub(super) fn toolbar_location_input_id(active_toolbar_pane: Option<PaneId>) -> egui::Id {
    egui::Id::new(("location_input", active_toolbar_pane.map(|pane_id| pane_id.to_string())))
}

pub(super) struct GuiRuntimeState {
    pub(super) graph_search_open: bool,
    pub(super) graph_search_query: String,
    pub(super) graph_search_filter_mode: bool,
    pub(super) graph_search_matches: Vec<NodeKey>,
    pub(super) graph_search_active_match_index: Option<usize>,
    pub(super) focused_node_hint: Option<NodeKey>,
    pub(super) graph_surface_focused: bool,
    pub(super) focus_ring_node_key: Option<NodeKey>,
    pub(super) focus_ring_started_at: Option<Instant>,
    pub(super) focus_ring_duration: Duration,
    pub(super) omnibar_search_session: Option<OmnibarSearchSession>,
    pub(super) active_toolbar_pane: Option<PaneId>,
    pub(super) toolbar_drafts: HashMap<PaneId, ToolbarDraft>,
    pub(super) command_palette_toggle_requested: bool,
    pub(super) deferred_open_child_webviews: Vec<WebViewId>,
}
