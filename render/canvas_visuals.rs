/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Canvas visual helpers: presentation profiles, lifecycle colours, viewport
//! culling, search-hit colouring, and adjacency-set computation.

use crate::app::{GraphBrowserApp, SearchDisplayMode};
use crate::graph::{NodeKey, NodeLifecycle};
use crate::registries::domain::presentation::PresentationProfile;
use crate::shell::desktop::runtime::registries::phase3_resolve_active_presentation_profile;
use egui::Color32;
use std::collections::HashSet;

use super::spatial_index::NodeSpatialIndex;
use crate::util::CoordBridge;

// ── Presentation ──────────────────────────────────────────────────────────────

pub(super) fn active_presentation_profile(app: &GraphBrowserApp) -> PresentationProfile {
    phase3_resolve_active_presentation_profile(app.default_registry_theme_id()).profile
}

pub(super) fn lifecycle_color(
    presentation: &PresentationProfile,
    lifecycle: NodeLifecycle,
) -> Color32 {
    match lifecycle {
        NodeLifecycle::Active => presentation.lifecycle_active.to_color32(),
        NodeLifecycle::Warm => presentation.lifecycle_warm.to_color32(),
        NodeLifecycle::Cold => presentation.lifecycle_cold.to_color32(),
        NodeLifecycle::Tombstone => presentation.lifecycle_tombstone.to_color32(),
    }
}

// ── Search visuals ────────────────────────────────────────────────────────────

pub(super) fn apply_search_node_visuals(
    app: &mut GraphBrowserApp,
    selection: &crate::app::SelectionState,
    search_matches: &HashSet<NodeKey>,
    active_search_match: Option<NodeKey>,
    search_query_active: bool,
) {
    let hovered = app.workspace.graph_runtime.hovered_graph_node;
    let highlighted_edge = app.workspace.graph_runtime.highlighted_graph_edge;
    let search_mode = app.workspace.graph_runtime.search_display_mode;
    let adjacency_set = hovered_adjacency_set(app, hovered);
    let presentation = active_presentation_profile(app);
    let colors: Vec<(NodeKey, Color32)> = app
        .workspace
        .domain
        .graph
        .nodes()
        .map(|(key, node)| {
            let mut color = lifecycle_color(&presentation, node.lifecycle);
            if app.is_crash_blocked(key) {
                color = presentation.crash_blocked.to_color32();
            }

            let search_match = search_query_active && search_matches.contains(&key);
            let search_miss = search_query_active && !search_matches.contains(&key);
            if search_match {
                color = if active_search_match == Some(key) {
                    presentation.search_match_active.to_color32()
                } else {
                    presentation.search_match.to_color32()
                };
            } else if search_miss && matches!(search_mode, SearchDisplayMode::Highlight) {
                color = color.gamma_multiply(0.45);
            }

            if hovered.is_some() && !adjacency_set.contains(&key) {
                color = color.gamma_multiply(0.4);
            }
            if hovered == Some(key) {
                color = presentation.hover_target.to_color32();
            }
            if let Some((from, to)) = highlighted_edge
                && (key == from || key == to)
            {
                color = presentation.edge_highlight_foreground.to_color32();
            }
            if selection.primary() == Some(key) {
                color = presentation.selection_primary.to_color32();
            } else if selection.contains(&key) && hovered != Some(key) {
                color = if app.is_crash_blocked(key) {
                    presentation.crash_blocked.to_color32()
                } else {
                    lifecycle_color(&presentation, node.lifecycle)
                };
            }
            (key, color)
        })
        .collect();

    let Some(state) = app.workspace.graph_runtime.egui_state.as_mut() else {
        return;
    };
    for (key, color) in colors {
        if let Some(node) = state.graph.node_mut(key) {
            node.set_color(color);
        }
    }

    let edge_dimming: Vec<_> = state
        .graph
        .edges_iter()
        .map(|(edge_key, _)| {
            let mut dim = false;
            if hovered.is_some()
                && let Some((from, to)) = state.graph.edge_endpoints(edge_key)
            {
                dim = !adjacency_set.contains(&from) && !adjacency_set.contains(&to);
            }
            if search_query_active
                && matches!(search_mode, SearchDisplayMode::Highlight)
                && let Some((from, to)) = state.graph.edge_endpoints(edge_key)
                && (!search_matches.contains(&from) || !search_matches.contains(&to))
            {
                dim = true;
            }
            (edge_key, dim)
        })
        .collect();
    for (edge_key, dim) in edge_dimming {
        if let Some(edge) = state.graph.edge_mut(edge_key) {
            edge.display_mut().set_dimmed(dim);
        }
    }
}

pub(super) fn hovered_adjacency_set(
    app: &GraphBrowserApp,
    hovered: Option<NodeKey>,
) -> HashSet<NodeKey> {
    hovered
        .map(|hover_key| {
            app.domain_graph()
                .out_neighbors(hover_key)
                .chain(app.domain_graph().in_neighbors(hover_key))
                .chain(std::iter::once(hover_key))
                .collect()
        })
        .unwrap_or_default()
}

// ── Viewport culling ──────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub(super) struct ViewportCullingSelection {
    pub(super) visible: HashSet<NodeKey>,
    pub(super) extended: HashSet<NodeKey>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ViewportCullingMetrics {
    pub(super) total_nodes: usize,
    pub(super) visible_nodes: usize,
    pub(super) submitted_nodes: usize,
    pub(super) removed_nodes: usize,
    pub(super) full_submission_units: usize,
    pub(super) culled_submission_units: usize,
}

pub(super) fn estimated_submission_units(graph: &crate::graph::Graph) -> usize {
    graph.node_count() + graph.edge_count()
}

pub(super) fn viewport_culling_selection_for_canvas_rect(
    graph: &crate::graph::Graph,
    canvas_rect: egui::Rect,
) -> Option<ViewportCullingSelection> {
    const DEFAULT_NODE_RADIUS: f32 = 5.0;
    let index = NodeSpatialIndex::build(graph.nodes().filter_map(|(key, _)| {
        graph.node_projected_position(key).map(|position| {
            let pos = position.to_pos2();
            (key, pos, DEFAULT_NODE_RADIUS)
        })
    }));

    let visible: HashSet<NodeKey> = index
        .nodes_in_canvas_rect(canvas_rect)
        .into_iter()
        .collect();
    if visible.is_empty() || visible.len() >= graph.node_count() {
        return None;
    }

    let mut extended = visible.clone();
    for edge in graph.edges() {
        if visible.contains(&edge.from) || visible.contains(&edge.to) {
            extended.insert(edge.from);
            extended.insert(edge.to);
        }
    }

    if extended.len() >= graph.node_count() {
        return None;
    }

    Some(ViewportCullingSelection { visible, extended })
}

pub(super) fn viewport_culling_metrics_for_canvas_rect(
    graph: &crate::graph::Graph,
    canvas_rect: egui::Rect,
) -> Option<ViewportCullingMetrics> {
    let selection = viewport_culling_selection_for_canvas_rect(graph, canvas_rect)?;
    let culled_edge_count = graph
        .edges()
        .filter(|edge| {
            selection.extended.contains(&edge.from) && selection.extended.contains(&edge.to)
        })
        .count();

    Some(ViewportCullingMetrics {
        total_nodes: graph.node_count(),
        visible_nodes: selection.visible.len(),
        submitted_nodes: selection.extended.len(),
        removed_nodes: graph.node_count().saturating_sub(selection.extended.len()),
        full_submission_units: estimated_submission_units(graph),
        culled_submission_units: selection.extended.len() + culled_edge_count,
    })
}

pub(super) fn viewport_culled_graph_for_canvas_rect(
    graph: &crate::graph::Graph,
    canvas_rect: egui::Rect,
) -> Option<crate::graph::Graph> {
    let selection = viewport_culling_selection_for_canvas_rect(graph, canvas_rect)?;

    let mut culled = graph.clone();
    let to_remove: Vec<NodeKey> = culled
        .nodes()
        .map(|(key, _)| key)
        .filter(|key| !selection.extended.contains(key))
        .collect();
    for key in to_remove {
        culled.remove_node(key);
    }

    Some(culled)
}

/// Compute a viewport-culled graph containing only the nodes visible in the
/// current frame's viewport (plus any additional nodes needed to keep edges
/// intact).  Returns `None` when culling is not applicable — e.g. when there
/// is no previous-frame camera metadata yet, when the graph is small enough
/// that culling has no effect, or when the canvas rect cannot be computed.
///
/// **Edge ghost-endpoint policy**: If either endpoint of an edge is visible,
/// both endpoints are included in the culled graph so that the renderer
/// never sees an edge with a missing endpoint.
pub(super) fn viewport_culled_graph(
    ui: &egui::Ui,
    app: &GraphBrowserApp,
    view_id: crate::app::GraphViewId,
) -> Option<crate::graph::Graph> {
    let frame = app.workspace.graph_runtime.graph_view_frames.get(&view_id)?;
    let canvas_rect = canvas_rect_from_view_frame(ui.max_rect(), *frame)?;

    viewport_culled_graph_for_canvas_rect(&app.workspace.domain.graph, canvas_rect)
}

pub(super) fn canvas_rect_from_view_frame(
    screen_rect: egui::Rect,
    frame: crate::app::GraphViewFrame,
) -> Option<egui::Rect> {
    if frame.zoom.abs() < f32::EPSILON {
        return None;
    }

    let pan = egui::vec2(frame.pan_x, frame.pan_y);
    let canvas_min = (screen_rect.min.to_vec2() - pan) / frame.zoom;
    let canvas_max = (screen_rect.max.to_vec2() - pan) / frame.zoom;
    let canvas_rect = egui::Rect::from_min_max(canvas_min.to_pos2(), canvas_max.to_pos2());
    Some(canvas_rect)
}

// ── Node bounds ───────────────────────────────────────────────────────────────

pub(super) fn node_bounds_for_selection(
    app: &GraphBrowserApp,
    selection: &crate::app::SelectionState,
) -> Option<(f32, f32, f32, f32)> {
    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_y = f32::NEG_INFINITY;

    for key in selection.iter().copied() {
        if let Some(position) = app.domain_graph().node_projected_position(key) {
            min_x = min_x.min(position.x);
            max_x = max_x.max(position.x);
            min_y = min_y.min(position.y);
            max_y = max_y.max(position.y);
        }
    }

    if !min_x.is_finite() || !max_x.is_finite() || !min_y.is_finite() || !max_y.is_finite() {
        return None;
    }

    Some((min_x, max_x, min_y, max_y))
}

// ── Search graph ─────────────────────────────────────────────────────────────

pub(super) fn filtered_graph_for_search(
    app: &GraphBrowserApp,
    search_matches: &HashSet<NodeKey>,
) -> crate::graph::Graph {
    let mut filtered = app.domain_graph().clone();
    let to_remove: Vec<NodeKey> = filtered
        .nodes()
        .map(|(key, _)| key)
        .filter(|key| !search_matches.contains(key))
        .collect();
    for key in to_remove {
        filtered.remove_node(key);
    }
    filtered
}
