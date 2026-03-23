/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Canvas visual helpers: presentation profiles, lifecycle colours, viewport
//! culling, search-hit colouring, and adjacency-set computation.

use crate::app::{GraphBrowserApp, GraphViewId, SearchDisplayMode};
use crate::graph::{NodeKey, NodeLifecycle};
use crate::model::graph::filter::{FacetExpr, FilterEvaluationSummary, evaluate_filter_result};
use crate::registries::domain::presentation::PresentationProfile;
use crate::shell::desktop::runtime::registries::{
    phase3_resolve_active_presentation_profile, phase3_resolve_active_theme,
};
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
    let theme_resolution = phase3_resolve_active_theme(app.default_registry_theme_id());
    let theme_tokens = &theme_resolution.tokens;
    let highlighted_endpoint_color = theme_tokens.edge_tokens.selection.foreground_color;
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
                    theme_tokens.graph_node_search_match_active
                } else {
                    theme_tokens.graph_node_search_match
                };
            } else if search_miss && matches!(search_mode, SearchDisplayMode::Highlight) {
                color = color.gamma_multiply(0.45);
            }

            if hovered.is_some() && !adjacency_set.contains(&key) {
                color = color.gamma_multiply(0.4);
            }
            if hovered == Some(key) {
                color = theme_tokens.graph_node_hover;
            }
            if let Some((from, to)) = highlighted_edge
                && (key == from || key == to)
            {
                color = highlighted_endpoint_color;
            }
            if selection.primary() == Some(key) {
                color = theme_tokens.graph_node_selection;
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
    let focus_ring_color = theme_tokens.graph_node_focus_ring;
    let hover_ring_color = theme_tokens.graph_node_hover_ring;
    for (key, color) in colors {
        if let Some(node) = state.graph.node_mut(key) {
            node.set_color(color);
            node.display_mut().set_focus_ring_color(focus_ring_color);
            node.display_mut().set_hover_ring_color(hover_ring_color);
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
        .map(|expr| evaluate_filter_result(app.domain_graph(), expr))
}

pub(super) fn filtered_graph_for_visible_nodes(
    app: &GraphBrowserApp,
    visible_nodes: &HashSet<NodeKey>,
) -> crate::graph::Graph {
    let mut filtered = app.domain_graph().clone();
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

    let tombstones_hidden = app
        .workspace
        .graph_runtime
        .views
        .get(&view_id)
        .is_some_and(|v| !v.tombstones_visible);
    let tombstone_filter: Option<HashSet<NodeKey>> = tombstones_hidden.then(|| {
        app.domain_graph()
            .nodes()
            .filter(|(_, node)| node.lifecycle != NodeLifecycle::Tombstone)
            .map(|(key, _)| key)
            .collect()
    });

    match (facet_matches, search_filter_matches, tombstone_filter) {
        (Some(facet), Some(search), Some(tombs)) => Some(
            facet
                .intersection(&search)
                .copied()
                .filter(|k| tombs.contains(k))
                .collect(),
        ),
        (Some(facet), Some(search), None) => {
            Some(facet.intersection(&search).copied().collect())
        }
        (Some(facet), None, Some(tombs)) => {
            Some(facet.intersection(&tombs).copied().collect())
        }
        (None, Some(search), Some(tombs)) => {
            Some(search.intersection(&tombs).copied().collect())
        }
        (Some(facet), None, None) => Some(facet),
        (None, Some(search), None) => Some(search),
        (None, None, Some(tombs)) => Some(tombs),
        (None, None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use euclid::default::Point2D;

    use super::*;
    use crate::app::GraphViewState;
    use crate::graph::egui_adapter::EguiGraphState;

    fn test_app() -> GraphBrowserApp {
        GraphBrowserApp::new_for_testing()
    }

    #[test]
    fn test_search_highlight_mode_dims_not_hides() {
        let mut app = test_app();
        let a = app.add_node_and_sync("alpha".into(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("beta".into(), Point2D::new(10.0, 0.0));
        app.workspace.graph_runtime.search_display_mode = SearchDisplayMode::Highlight;
        app.workspace.graph_runtime.egui_state =
            Some(EguiGraphState::from_graph_with_visual_state(
                &app.workspace.domain.graph,
                app.focused_selection(),
                app.focused_selection().primary(),
                &HashSet::new(),
            ));
        let matches = HashSet::from([a]);
        let selection = app.focused_selection().clone();
        apply_search_node_visuals(&mut app, &selection, &matches, Some(a), true);

        let state = app.workspace.graph_runtime.egui_state.as_ref().unwrap();
        assert!(state.graph.node(a).is_some());
        assert!(state.graph.node(b).is_some());
        let b_color = state.graph.node(b).unwrap().color().unwrap();
        let presentation =
            crate::registries::domain::presentation::PresentationDomainRegistry::default()
                .resolve_profile("physics:default", "theme:default")
                .profile;
        assert!(b_color != lifecycle_color(&presentation, NodeLifecycle::Cold));
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
    fn primary_selection_uses_theme_node_selection_color() {
        let mut app = test_app();
        let key = app.add_node_and_sync("alpha".into(), Point2D::new(0.0, 0.0));
        app.select_node(key, false);
        app.workspace.graph_runtime.egui_state =
            Some(EguiGraphState::from_graph_with_visual_state(
                &app.workspace.domain.graph,
                app.focused_selection(),
                app.focused_selection().primary(),
                &HashSet::new(),
            ));

        let selection = app.focused_selection().clone();
        apply_search_node_visuals(&mut app, &selection, &HashSet::new(), None, false);

        let state = app.workspace.graph_runtime.egui_state.as_ref().unwrap();
        let theme_resolution =
            crate::shell::desktop::runtime::registries::phase3_resolve_active_theme(
                app.default_registry_theme_id(),
            );
        assert_eq!(
            state.graph.node(key).unwrap().color(),
            Some(theme_resolution.tokens.graph_node_selection)
        );
    }

    #[test]
    fn hovered_node_uses_theme_hover_color() {
        let mut app = test_app();
        let key = app.add_node_and_sync("alpha".into(), Point2D::new(0.0, 0.0));
        app.workspace.graph_runtime.hovered_graph_node = Some(key);
        app.workspace.graph_runtime.egui_state =
            Some(EguiGraphState::from_graph_with_visual_state(
                &app.workspace.domain.graph,
                app.focused_selection(),
                app.focused_selection().primary(),
                &HashSet::new(),
            ));

        let selection = app.focused_selection().clone();
        apply_search_node_visuals(&mut app, &selection, &HashSet::new(), None, false);

        let state = app.workspace.graph_runtime.egui_state.as_ref().unwrap();
        let theme_resolution =
            crate::shell::desktop::runtime::registries::phase3_resolve_active_theme(
                app.default_registry_theme_id(),
            );
        assert_eq!(
            state.graph.node(key).unwrap().color(),
            Some(theme_resolution.tokens.graph_node_hover)
        );
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
