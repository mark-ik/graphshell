/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::{HashMap, HashSet};

use crate::app::{
    GraphBrowserApp, GraphIntent, GraphSearchHistoryEntry, GraphSearchOrigin, GraphSearchRequest,
    LifecycleCause, PendingTileOpenMode, SearchDisplayMode, UndoBoundaryReason, WorkbenchIntent,
};
use crate::graph::NodeKey;
use crate::shell::desktop::host::running_app_state::RunningAppState;
use crate::shell::desktop::host::window::EmbedderWindow;
use crate::shell::desktop::lifecycle::lifecycle_intents;
use crate::shell::desktop::lifecycle::webview_backpressure::WebviewCreationBackpressureState;
use crate::shell::desktop::runtime::control_panel::ControlPanel;
#[cfg(feature = "diagnostics")]
use crate::shell::desktop::runtime::diagnostics;
use crate::shell::desktop::runtime::diagnostics::{DiagnosticEvent, emit_event};
use crate::shell::desktop::runtime::registries::{
    CHANNEL_UX_CONTRACT_WARNING, CHANNEL_UX_DISPATCH_CONSUMED,
    CHANNEL_UX_DISPATCH_DEFAULT_PREVENTED, CHANNEL_UX_DISPATCH_PHASE, CHANNEL_UX_DISPATCH_STARTED,
    CHANNEL_UX_FOCUS_REALIZATION_MISMATCH, CHANNEL_UX_FOCUS_RETURN_FALLBACK,
    CHANNEL_UX_NAVIGATION_TRANSITION, CHANNEL_UX_NAVIGATION_VIOLATION,
};
use crate::shell::desktop::ui::graph_search_flow::{self, GraphSearchFlowArgs};
use crate::shell::desktop::ui::graph_search_ui::{self, GraphSearchUiArgs};
use crate::shell::desktop::ui::gui_frame::ToolbarDialogPhaseArgs;
use crate::shell::desktop::ui::gui_frame::{self};
use crate::shell::desktop::ui::gui_state::{
    LocalFocusTarget, RuntimeFocusAuthorityState, ToolbarState,
};
use crate::shell::desktop::ui::nav_targeting;
use crate::shell::desktop::ui::toolbar::toolbar_ui::OmnibarSearchSession;
use crate::shell::desktop::ui::toolbar_routing::{self, ToolbarOpenMode};
use crate::shell::desktop::workbench::pane_model::{PaneViewState, ToolPaneState};
use crate::shell::desktop::workbench::tile_kind::TileKind;
use crate::shell::desktop::workbench::tile_view_ops::{TileOpenMode, ToggleTileViewArgs};
use egui_tiles::{Tile, Tree};
use servo::WebViewId;
use servo::{OffscreenRenderingContext, WindowRenderingContext};
use std::rc::Rc;
use winit::window::Window;

#[path = "gui/clipboard_flow.rs"]
mod clipboard_flow;
#[path = "gui/focus_realizer.rs"]
mod focus_realizer;
#[path = "gui/graph_search_orchestration.rs"]
mod graph_search_orchestration;
#[path = "gui/pending_open_flow.rs"]
mod pending_open_flow;
#[path = "gui/pre_frame_flow.rs"]
mod pre_frame_flow;
#[path = "gui/semantic_lifecycle_flow.rs"]
mod semantic_lifecycle_flow;
#[path = "gui/toast_flow.rs"]
mod toast_flow;
#[path = "gui/toolbar_phase_flow.rs"]
mod toolbar_phase_flow;
#[path = "gui/workbench_dispatch_flow.rs"]
mod workbench_dispatch_flow;
#[path = "gui/workbench_intent_interceptor.rs"]
mod workbench_intent_interceptor;

pub(crate) use clipboard_flow::{
    CLIPBOARD_STATUS_EMPTY_TEXT, CLIPBOARD_STATUS_FAILURE_PREFIX,
    CLIPBOARD_STATUS_SUCCESS_TITLE_TEXT, CLIPBOARD_STATUS_SUCCESS_URL_TEXT,
    CLIPBOARD_STATUS_UNAVAILABLE_TEXT, ClipboardAdapter, clipboard_copy_failure_text,
    clipboard_copy_missing_node_failure_text, clipboard_copy_success_text,
    handle_pending_clipboard_copy_requests,
};
pub(crate) use pending_open_flow::{
    handle_pending_open_clip_after_intents, handle_pending_open_node_after_intents,
    handle_pending_open_note_after_intents,
};
pub(crate) use pre_frame_flow::{PreFramePhaseOutput, run_pre_frame_phase};
pub(crate) use semantic_lifecycle_flow::run_semantic_lifecycle_phase;
pub(crate) use toast_flow::{ToastsAdapter, handle_pending_node_status_notices};
pub(crate) use toolbar_phase_flow::{run_keyboard_phase, run_toolbar_phase};
pub(crate) use workbench_dispatch_flow::handle_tool_pane_intents;

use focus_realizer::FocusRealizer;

#[allow(clippy::too_many_arguments)]
pub(super) fn run_graph_search_phase(
    ctx: &egui::Context,
    graph_app: &mut GraphBrowserApp,
    toasts: &mut egui_notify::Toasts,
    graph_search_open: &mut bool,
    local_widget_focus: &mut Option<LocalFocusTarget>,
    graph_search_query: &mut String,
    graph_search_filter_mode: &mut bool,
    graph_search_matches: &mut Vec<NodeKey>,
    graph_search_active_match_index: &mut Option<usize>,
    toolbar_state: &mut ToolbarState,
    frame_intents: &mut Vec<GraphIntent>,
    has_active_node_pane: bool,
) -> graph_search_flow::GraphSearchFlowOutput {
    graph_search_orchestration::run_graph_search_phase(
        ctx,
        graph_app,
        toasts,
        graph_search_open,
        local_widget_focus,
        graph_search_query,
        graph_search_filter_mode,
        graph_search_matches,
        graph_search_active_match_index,
        toolbar_state,
        frame_intents,
        has_active_node_pane,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn run_graph_search_window_phase(
    ctx: &egui::Context,
    graph_app: &mut GraphBrowserApp,
    toolbar_visible: bool,
    graph_search_open: bool,
    is_graph_view: bool,
    local_widget_focus: &mut Option<LocalFocusTarget>,
    graph_search_query: &mut String,
    graph_search_filter_mode: &mut bool,
    graph_search_matches: &mut Vec<NodeKey>,
    graph_search_active_match_index: &mut Option<usize>,
    graph_search_output: &mut graph_search_flow::GraphSearchFlowOutput,
) {
    graph_search_orchestration::run_graph_search_window_phase(
        ctx,
        graph_app,
        toolbar_visible,
        graph_search_open,
        is_graph_view,
        local_widget_focus,
        graph_search_query,
        graph_search_filter_mode,
        graph_search_matches,
        graph_search_active_match_index,
        graph_search_output,
    );
}

pub(crate) fn active_graph_search_match(
    matches: &[NodeKey],
    active_index: Option<usize>,
) -> Option<NodeKey> {
    let idx = active_index?;
    matches.get(idx).copied()
}

fn refresh_graph_search_matches(
    graph_app: &GraphBrowserApp,
    query: &str,
    matches: &mut Vec<NodeKey>,
    active_index: &mut Option<usize>,
) {
    if clear_graph_search_matches_if_query_empty(query, matches, active_index) {
        return;
    }

    let mut ranked =
        crate::shell::desktop::runtime::registries::phase3_index_search(graph_app, query, 64)
            .into_iter()
            .filter_map(|result| {
                match result.kind {
        crate::shell::desktop::runtime::registries::index::SearchResultKind::Node(key) => Some(key),
        crate::shell::desktop::runtime::registries::index::SearchResultKind::HistoryUrl(_)
        | crate::shell::desktop::runtime::registries::index::SearchResultKind::KnowledgeTag {
            ..
        } => None,
    }
            })
            .collect::<Vec<_>>();
    ranked.dedup();
    extend_matches_with_active_anchor_neighborhood(graph_app, &mut ranked);
    *matches = ranked;
    sync_graph_search_active_index(matches, active_index);
}

fn extend_matches_with_active_anchor_neighborhood(
    graph_app: &GraphBrowserApp,
    matches: &mut Vec<NodeKey>,
) {
    let Some(anchor) = graph_app
        .workspace
        .graph_runtime
        .active_graph_search_neighborhood_anchor
    else {
        return;
    };
    if graph_app.domain_graph().get_node(anchor).is_none() {
        return;
    }

    let mut seen: HashSet<NodeKey> = matches.iter().copied().collect();
    let neighborhood_depth = graph_app
        .workspace
        .graph_runtime
        .active_graph_search_neighborhood_depth
        .clamp(1, 2);
    for neighbor in std::iter::once(anchor).chain(
        graph_app
            .domain_graph()
            .connected_candidates_with_depth(anchor, neighborhood_depth)
            .into_iter()
            .map(|(neighbor, _)| neighbor),
    ) {
        if seen.insert(neighbor) {
            matches.push(neighbor);
        }
    }
}

fn maybe_push_graph_search_history(graph_app: &mut GraphBrowserApp, request: &GraphSearchRequest) {
    let previous_query = graph_app
        .workspace
        .graph_runtime
        .active_graph_search_query
        .trim()
        .to_string();
    let previous_filter_mode = matches!(
        graph_app.workspace.graph_runtime.search_display_mode,
        SearchDisplayMode::Filter
    );
    let previous_origin = graph_app
        .workspace
        .graph_runtime
        .active_graph_search_origin
        .clone();
    let previous_neighborhood_anchor = graph_app
        .workspace
        .graph_runtime
        .active_graph_search_neighborhood_anchor;
    let previous_neighborhood_depth = graph_app
        .workspace
        .graph_runtime
        .active_graph_search_neighborhood_depth;

    if previous_query.is_empty() {
        return;
    }

    if previous_query == request.query
        && previous_filter_mode == request.filter_mode
        && previous_origin == request.origin
        && previous_neighborhood_anchor == request.neighborhood_anchor
        && previous_neighborhood_depth == request.neighborhood_depth
    {
        return;
    }

    let entry = GraphSearchHistoryEntry {
        query: previous_query,
        filter_mode: previous_filter_mode,
        origin: previous_origin,
        neighborhood_anchor: previous_neighborhood_anchor,
        neighborhood_depth: previous_neighborhood_depth,
    };

    graph_app
        .workspace
        .graph_runtime
        .graph_search_history
        .retain(|existing| existing != &entry);
    graph_app
        .workspace
        .graph_runtime
        .graph_search_history
        .push(entry);
    const GRAPH_SEARCH_HISTORY_LIMIT: usize = 5;
    if graph_app.workspace.graph_runtime.graph_search_history.len() > GRAPH_SEARCH_HISTORY_LIMIT {
        let overflow = graph_app.workspace.graph_runtime.graph_search_history.len()
            - GRAPH_SEARCH_HISTORY_LIMIT;
        graph_app
            .workspace
            .graph_runtime
            .graph_search_history
            .drain(0..overflow);
    }
}

fn clear_graph_search_matches_if_query_empty(
    query: &str,
    matches: &mut Vec<NodeKey>,
    active_index: &mut Option<usize>,
) -> bool {
    if query.trim().is_empty() {
        matches.clear();
        *active_index = None;
        return true;
    }

    false
}

pub(crate) fn graph_search_toast_message(
    origin: GraphSearchOrigin,
    query: &str,
    neighborhood_depth: u8,
) -> String {
    let action = match origin {
        GraphSearchOrigin::Manual => "Search applied",
        GraphSearchOrigin::SemanticTag => "Semantic slice applied",
        GraphSearchOrigin::AnchorSlice => "Anchor slice applied",
    };
    let scope = if neighborhood_depth > 1 {
        format!(" ({}-hop neighborhood)", neighborhood_depth)
    } else if neighborhood_depth == 1 {
        " (1-hop neighborhood)".to_string()
    } else {
        String::new()
    };
    format!("{action}: {query}{scope}")
}

fn sync_graph_search_active_index(matches: &[NodeKey], active_index: &mut Option<usize>) {
    if matches.is_empty() {
        *active_index = None;
    } else if active_index.is_none_or(|idx| idx >= matches.len()) {
        *active_index = Some(0);
    }
}

fn step_graph_search_active_match(
    matches: &[NodeKey],
    active_index: &mut Option<usize>,
    step: isize,
) {
    if matches.is_empty() {
        *active_index = None;
        return;
    }

    let current = active_index.unwrap_or(0) as isize;
    let len = matches.len() as isize;
    let next = (current + step).rem_euclid(len) as usize;
    *active_index = Some(next);
}


#[cfg(test)]
#[path = "gui_orchestration_tests.rs"]
mod gui_orchestration_tests;
