/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::time::{Duration, Instant};

use crate::app::{GraphBrowserApp, GraphViewId};
use crate::graph::NodeKey;
use crate::shell::desktop::ui::toolbar::toolbar_ui::OmnibarSearchSession;
use servo::LoadStatus;

pub(crate) struct ToolbarState {
    pub(crate) location: String,
    pub(crate) location_dirty: bool,
    pub(crate) location_submitted: bool,
    pub(crate) show_clear_data_confirm: bool,
    pub(crate) load_status: LoadStatus,
    pub(crate) status_text: Option<String>,
    pub(crate) can_go_back: bool,
    pub(crate) can_go_forward: bool,
}

pub(crate) struct GuiRuntimeState {
    pub(crate) graph_search_open: bool,
    pub(crate) graph_search_query: String,
    pub(crate) graph_search_filter_mode: bool,
    pub(crate) graph_search_matches: Vec<NodeKey>,
    pub(crate) graph_search_active_match_index: Option<usize>,
    pub(crate) focused_node_hint: Option<NodeKey>,
    pub(crate) graph_surface_focused: bool,
    pub(crate) focus_ring_node_key: Option<NodeKey>,
    pub(crate) focus_ring_started_at: Option<Instant>,
    pub(crate) focus_ring_duration: Duration,
    pub(crate) omnibar_search_session: Option<OmnibarSearchSession>,
    pub(crate) command_palette_toggle_requested: bool,
}

pub(crate) fn apply_node_focus_state(runtime_state: &mut GuiRuntimeState, node_key: Option<NodeKey>) {
    runtime_state.focused_node_hint = node_key;
    runtime_state.graph_surface_focused = false;
}

pub(crate) fn apply_graph_surface_focus_state(
    runtime_state: &mut GuiRuntimeState,
    graph_app: &mut GraphBrowserApp,
    active_graph_view: Option<GraphViewId>,
) {
    runtime_state.focused_node_hint = None;
    runtime_state.graph_surface_focused = true;
    graph_app.workspace.focused_view = active_graph_view;
}
