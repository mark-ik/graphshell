/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Graph-search phase — per-tick request drain + window rendering +
//! match-set maintenance.
//!
//! Split out of `gui_orchestration.rs` as part of M6 §4.1. Absorbs
//! the former `graph_search_orchestration` sibling submodule; its
//! internals are now private helpers of this module. Public entry
//! points:
//!
//! - [`run_graph_search_phase`] — called once per tick during the
//!   keyboard+graph-search phase. Drains `take_pending_graph_search_request`,
//!   pushes history, resolves origin / anchor / depth, runs the
//!   `graph_search_flow` pipeline, and notifies via toasts.
//! - [`run_graph_search_window_phase`] — renders the search window UI
//!   if appropriate (toolbar hidden, search open, graph view).
//! - [`active_graph_search_match`] — idx → NodeKey lookup helper.
//! - [`graph_search_toast_message`] — formats the "Search applied: ..."
//!   toast message for origin+depth variants.

use std::collections::HashSet;

use crate::app::{
    GraphBrowserApp, GraphIntent, GraphSearchHistoryEntry, GraphSearchOrigin, GraphSearchRequest,
    SearchDisplayMode, WorkbenchIntent,
};
use crate::graph::NodeKey;
use crate::shell::desktop::ui::graph_search_flow::{self, GraphSearchFlowArgs};
use crate::shell::desktop::ui::graph_search_ui::{self, GraphSearchUiArgs};
use crate::shell::desktop::ui::gui_state::{LocalFocusTarget, ToolbarState};

// ---------------------------------------------------------------------------
// Public entries
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
pub(crate) fn run_graph_search_phase(
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
    let graph_search_available = !has_active_node_pane;

    if let Some(request) = graph_app.take_pending_graph_search_request() {
        if request.record_history {
            maybe_push_graph_search_history(graph_app, &request);
        }
        *graph_search_query = request.query;
        *graph_search_filter_mode = request.filter_mode;
        crate::shell::desktop::ui::gui::apply_graph_search_local_focus_state(
            graph_search_open,
            local_widget_focus,
            !graph_search_query.trim().is_empty(),
        );
        graph_app.workspace.graph_runtime.active_graph_search_origin = request.origin;
        graph_app
            .workspace
            .graph_runtime
            .active_graph_search_neighborhood_anchor = request.neighborhood_anchor;
        graph_app
            .workspace
            .graph_runtime
            .active_graph_search_neighborhood_depth = request.neighborhood_depth;
        if let Some(message) = request.toast_message {
            toasts.success(message);
        }
        graph_app.workspace.graph_runtime.egui_state_dirty = true;
    }

    graph_app.workspace.graph_runtime.active_graph_search_query =
        graph_search_query.trim().to_string();
    graph_app.workspace.graph_runtime.search_display_mode = if *graph_search_filter_mode {
        SearchDisplayMode::Filter
    } else {
        SearchDisplayMode::Highlight
    };

    let output = graph_search_flow::handle_graph_search_flow(
        GraphSearchFlowArgs {
            ctx,
            graph_app,
            graph_search_open,
            local_widget_focus,
            graph_search_query,
            graph_search_filter_mode,
            graph_search_matches,
            graph_search_active_match_index,
            location: &mut toolbar_state.location,
            location_dirty: &mut toolbar_state.location_dirty,
            frame_intents,
            graph_search_available,
        },
        |graph_app, query, matches, active_index| {
            refresh_graph_search_matches(graph_app, query, matches, active_index);
        },
        |matches, active_index, delta| {
            step_graph_search_active_match(matches, active_index, delta);
        },
        |matches, active_index| active_graph_search_match(matches, active_index),
    );

    if !*graph_search_open && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        graph_app.enqueue_workbench_intent(WorkbenchIntent::ClearTileSelection);
    }
    graph_app
        .workspace
        .graph_runtime
        .active_graph_search_match_count = graph_search_matches.len();
    output
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
    if should_render_graph_search_window(toolbar_visible, graph_search_open, is_graph_view) {
        graph_search_ui::render_graph_search_window(
            GraphSearchUiArgs {
                ctx,
                graph_app,
                graph_search_query,
                graph_search_filter_mode,
                graph_search_matches,
                graph_search_active_match_index,
                local_widget_focus,
                focus_graph_search_field: &mut graph_search_output.focus_graph_search_field,
            },
            |graph_app, query, matches, active_index| {
                refresh_graph_search_matches(graph_app, query, matches, active_index);
            },
        );
    }
}

pub(crate) fn active_graph_search_match(
    matches: &[NodeKey],
    active_index: Option<usize>,
) -> Option<NodeKey> {
    let idx = active_index?;
    matches.get(idx).copied()
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

// ---------------------------------------------------------------------------
// Internals
// ---------------------------------------------------------------------------

fn should_render_graph_search_window(
    toolbar_visible: bool,
    graph_search_open: bool,
    is_graph_view: bool,
) -> bool {
    !toolbar_visible && graph_search_open && is_graph_view
}

pub(crate) fn refresh_graph_search_matches(
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
            .filter_map(|result| match result.kind {
                crate::shell::desktop::runtime::registries::index::SearchResultKind::Node(key) => {
                    Some(key)
                }
                crate::shell::desktop::runtime::registries::index::SearchResultKind::HistoryUrl(
                    _,
                )
                | crate::shell::desktop::runtime::registries::index::SearchResultKind::KnowledgeTag {
                    ..
                } => None,
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

pub(crate) fn maybe_push_graph_search_history(
    graph_app: &mut GraphBrowserApp,
    request: &GraphSearchRequest,
) {
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

fn sync_graph_search_active_index(matches: &[NodeKey], active_index: &mut Option<usize>) {
    if matches.is_empty() {
        *active_index = None;
    } else if active_index.is_none_or(|idx| idx >= matches.len()) {
        *active_index = Some(0);
    }
}

pub(crate) fn step_graph_search_active_match(
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
