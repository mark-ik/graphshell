/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Canvas visual helpers: presentation profiles, lifecycle colours, viewport
//! culling, search-hit colouring, and adjacency-set computation.

use crate::app::{GraphBrowserApp, GraphViewId, SearchDisplayMode, VisibleNavigationRegionSet};
use crate::graph::{NodeKey, NodeLifecycle};
use crate::model::graph::filter::{FacetExpr, FilterEvaluationSummary, evaluate_filter_result};
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

pub(super) fn hovered_adjacency_set(
    app: &GraphBrowserApp,
    hovered: Option<NodeKey>,
) -> HashSet<NodeKey> {
    hovered
        .map(|hover_key| {
            app.render_graph()
                .out_neighbors(hover_key)
                .chain(app.render_graph().in_neighbors(hover_key))
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
        // Skip containment/organizational edges — they don't require both endpoints
        // to be rendered together and can cause nearly-all-nodes to be extended when
        // a domain anchor node is in the visible set.
        if matches!(
            edge.edge_type,
            crate::graph::EdgeType::ContainmentRelation(_)
        ) {
            continue;
        }
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
    let frame = app
        .workspace
        .graph_runtime
        .graph_view_frames
        .get(&view_id)?;
    let screen_rect = effective_graph_screen_rect(ui.max_rect(), app)?;
    let canvas_rect = canvas_rect_from_view_frame(screen_rect, *frame)?;

    viewport_culled_graph_for_canvas_rect(app.render_graph(), canvas_rect)
}

pub(super) fn graph_visible_screen_rects(
    screen_rect: egui::Rect,
    app: &GraphBrowserApp,
) -> VisibleNavigationRegionSet {
    let Some(geometry) = app
        .workspace
        .graph_runtime
        .workbench_navigation_geometry
        .as_ref()
    else {
        return VisibleNavigationRegionSet::singleton(screen_rect);
    };

    let visible_regions = geometry
        .visible_region_set_or_content()
        .clipped_to(screen_rect);

    if visible_regions.is_empty() && screen_rect.width() > 0.0 && screen_rect.height() > 0.0 {
        VisibleNavigationRegionSet::singleton(screen_rect)
    } else {
        visible_regions
    }
}

pub(super) fn effective_graph_screen_rect(
    screen_rect: egui::Rect,
    app: &GraphBrowserApp,
) -> Option<egui::Rect> {
    graph_visible_screen_rects(screen_rect, app).largest_rect()
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
    node_bounds_for_keys(app, selection.iter().copied())
}

pub(super) fn node_bounds_for_keys(
    app: &GraphBrowserApp,
    keys: impl Iterator<Item = crate::graph::NodeKey>,
) -> Option<(f32, f32, f32, f32)> {
    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_y = f32::NEG_INFINITY;

    for key in keys {
        if let Some(position) = app.render_graph().node_projected_position(key) {
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
    filtered_graph_for_visible_nodes(app, search_matches)
}

pub(super) fn active_view_filter_expr(
    app: &GraphBrowserApp,
    view_id: GraphViewId,
) -> Option<&FacetExpr> {
    app.workspace
        .graph_runtime
        .views
        .get(&view_id)
        .and_then(|view| view.effective_filter_expr())
}

pub(super) fn evaluate_active_view_filter(
    app: &GraphBrowserApp,
    view_id: GraphViewId,
) -> Option<FilterEvaluationSummary> {
    active_view_filter_expr(app, view_id)
        .map(|expr| evaluate_filter_result(app.render_graph(), expr))
}

pub(super) fn filtered_graph_for_visible_nodes(
    app: &GraphBrowserApp,
    visible_nodes: &HashSet<NodeKey>,
) -> crate::graph::Graph {
    let mut filtered = app.render_graph().clone();
    let to_remove: Vec<NodeKey> = filtered
        .nodes()
        .map(|(key, _)| key)
        .filter(|key| !visible_nodes.contains(key))
        .collect();
    for key in to_remove {
        filtered.remove_node(key);
    }
    filtered
}

pub(super) fn visible_nodes_for_view_filters(
    app: &GraphBrowserApp,
    view_id: GraphViewId,
    search_matches: &HashSet<NodeKey>,
    search_display_mode: SearchDisplayMode,
    search_query_active: bool,
) -> Option<HashSet<NodeKey>> {
    let facet_summary = evaluate_active_view_filter(app, view_id);
    let facet_matches = facet_summary.as_ref().map(|summary| {
        summary
            .result
            .matched_nodes
            .iter()
            .copied()
            .collect::<HashSet<_>>()
    });
    let search_filter_matches = (matches!(search_display_mode, SearchDisplayMode::Filter)
        && search_query_active)
        .then(|| search_matches.clone());

    let view_state = app.workspace.graph_runtime.views.get(&view_id);
    let tombstones_hidden = view_state.is_some_and(|v| !v.tombstones_visible);
    let tombstone_filter: Option<HashSet<NodeKey>> = tombstones_hidden.then(|| {
        app.render_graph()
            .nodes()
            .filter(|(_, node)| node.lifecycle != NodeLifecycle::Tombstone)
            .map(|(key, _)| key)
            .collect()
    });
    let graphlet_mask: Option<HashSet<NodeKey>> =
        view_state.and_then(|v| v.graphlet_node_mask.clone());
    let owned_node_mask: Option<HashSet<NodeKey>> =
        view_state.and_then(|v| v.owned_node_mask().cloned());

    // Intersect all active filters.  Each `Some` constrains the visible set.
    intersect_filters([
        facet_matches,
        search_filter_matches,
        tombstone_filter,
        owned_node_mask,
        graphlet_mask,
    ])
}

/// Intersect an array of optional node-set filters.
///
/// - `None` entries are ignored (no constraint from that filter).
/// - A single `Some` set is returned as-is.
/// - Multiple `Some` sets are intersected.
/// - If all entries are `None`, returns `None` (no filter active).
fn intersect_filters<const N: usize>(
    filters: [Option<HashSet<NodeKey>>; N],
) -> Option<HashSet<NodeKey>> {
    let mut result: Option<HashSet<NodeKey>> = None;
    for filter in filters {
        let Some(set) = filter else { continue };
        result = Some(match result {
            None => set,
            Some(acc) => acc.intersection(&set).copied().collect(),
        });
    }
    result
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use euclid::default::Point2D;

    use super::*;
    use crate::app::GraphViewState;

    fn test_app() -> GraphBrowserApp {
        GraphBrowserApp::new_for_testing()
    }

    #[test]
    fn test_search_filter_mode_hides_nodes() {
        let mut app = test_app();
        let a = app.add_node_and_sync("alpha".into(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("beta".into(), Point2D::new(10.0, 0.0));
        app.workspace.graph_runtime.search_display_mode = SearchDisplayMode::Filter;
        let matches = HashSet::from([a]);
        let filtered = filtered_graph_for_search(&app, &matches);
        assert!(filtered.get_node(a).is_some());
        assert!(filtered.get_node(b).is_none());
    }

    #[test]
    fn active_view_filter_matches_udc_descendants() {
        let mut app = test_app();
        let view_id = GraphViewId::new();
        app.workspace
            .graph_runtime
            .views
            .insert(view_id, GraphViewState::new_with_id(view_id, "Facet"));
        let descendant = app.add_node_and_sync(
            "https://example.com/numerical".into(),
            Point2D::new(0.0, 0.0),
        );
        let other =
            app.add_node_and_sync("https://example.com/music".into(), Point2D::new(10.0, 0.0));
        let _ = app
            .workspace
            .domain
            .graph
            .insert_node_tag(descendant, "udc:519.6".to_string());
        let _ = app
            .workspace
            .domain
            .graph
            .insert_node_tag(other, "udc:78".to_string());
        if let Some(view) = app.workspace.graph_runtime.views.get_mut(&view_id) {
            view.active_filter =
                crate::model::graph::filter::parse_omnibar_facet_token("facet:udc_classes=udc:51");
        }

        let summary = evaluate_active_view_filter(&app, view_id).expect("filter summary");
        let matches: HashSet<_> = summary.result.matched_nodes.iter().copied().collect();

        assert!(matches.contains(&descendant));
        assert!(!matches.contains(&other));
    }

    #[test]
    fn visible_nodes_for_view_filters_intersects_search_and_facet_sets() {
        let mut app = test_app();
        let view_id = GraphViewId::new();
        app.workspace
            .graph_runtime
            .views
            .insert(view_id, GraphViewState::new_with_id(view_id, "Facet"));
        let alpha = app.add_node_and_sync("alpha".into(), Point2D::new(0.0, 0.0));
        let beta = app.add_node_and_sync("beta".into(), Point2D::new(10.0, 0.0));
        if let Some(view) = app.workspace.graph_runtime.views.get_mut(&view_id) {
            view.active_filter = Some(crate::model::graph::filter::FacetExpr::Predicate(
                crate::model::graph::filter::FacetPredicate {
                    facet_key: crate::model::graph::filter::facet_keys::TITLE.to_string(),
                    operator: crate::model::graph::filter::FacetOperator::Eq,
                    operand: crate::model::graph::filter::FacetOperand::Scalar(
                        crate::model::graph::filter::FacetScalar::Text("alpha".to_string()),
                    ),
                },
            ));
        }

        let visible = visible_nodes_for_view_filters(
            &app,
            view_id,
            &HashSet::from([alpha, beta]),
            SearchDisplayMode::Filter,
            true,
        )
        .expect("visible set");

        assert_eq!(visible, HashSet::from([alpha]));
    }
}
