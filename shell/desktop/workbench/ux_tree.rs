/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};

use egui_tiles::{Container, Tile, TileId, Tree};

use crate::app::workbench_layout_policy::AnchorEdge;
use crate::app::{
    GraphBrowserApp, GraphViewId, PendingConnectedOpenScope, PendingTileOpenMode, SurfaceHostId,
    WorkbenchLayoutConstraint,
};
use crate::graph::{EdgeFamily, NodeKey, RelationSelector};
use crate::render::radial_menu::latest_semantic_snapshot;

use super::pane_model::TileRenderMode;
use super::tile_kind::TileKind;
use crate::shell::desktop::workbench::pane_model::PanePresentationMode;

pub(crate) const UX_TREE_SEMANTIC_SCHEMA_VERSION: u32 = 2;
pub(crate) const UX_TREE_PRESENTATION_SCHEMA_VERSION: u32 = 1;
pub(crate) const UX_TREE_TRACE_SCHEMA_VERSION: u32 = 1;
pub(crate) const UX_TREE_WORKBENCH_ROOT_ID: &str = "uxnode://workbench/root";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UxNodeRole {
    Workbench,
    SplitContainer,
    TabContainer,
    GraphSurface,
    GraphNode,
    NodePane,
    RadialPalette,
    RadialTierRing,
    RadialSector,
    RadialSummary,
    GraphViewLensScope,
    NavigatorProjection,
    FileTreeProjection,
    RouteOpenBoundary,
    #[cfg(feature = "diagnostics")]
    ToolPane,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UxAction {
    Focus,
    Close,
    SplitHorizontal,
    Select,
    Open,
    Navigate,
    Dismiss,
    Invoke,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum UxDomainIdentity {
    Workbench,
    GraphView {
        graph_view_id: GraphViewId,
    },
    Node {
        node_key: NodeKey,
    },
    #[cfg(feature = "diagnostics")]
    Tool {
        tool_kind: &'static str,
    },
    RadialSector {
        action_id: String,
        enabled: bool,
        tier: u8,
        rail_position: f32,
        hover_scale: f32,
        angle_rad: f32,
        page: usize,
    },
    RadialTierRing {
        tier: u8,
        visible_count: usize,
        page: usize,
        page_count: usize,
    },
    RadialSummary {
        tier1_visible_count: usize,
        tier2_visible_count: usize,
        tier2_page: usize,
        tier2_page_count: usize,
        overflow_hidden_entries: usize,
        label_pre_collisions: usize,
        label_post_collisions: usize,
        fallback_to_palette: bool,
        fallback_reason: Option<String>,
    },
    GraphViewLensScope {
        graph_view_id: GraphViewId,
        lens_name: String,
        lens_id: Option<String>,
        physics_name: String,
        layout_mode: String,
        theme_name: Option<String>,
        filter_count: usize,
        dimension: String,
        position_fit_locked: bool,
        zoom_fit_locked: bool,
        focused_view: bool,
        selection_count: usize,
    },
    NavigatorProjection {
        host: SurfaceHostId,
        anchor_edge: AnchorEdge,
        form_factor: String,
        scope: String,
        containment_relation_source: String,
        sort_mode: String,
        root_filter: Option<String>,
        row_count: usize,
        selected_count: usize,
        expanded_count: usize,
        collapsed_count: usize,
        workbench_group_count: usize,
        workbench_member_count: usize,
        unrelated_count: usize,
        recent_count: usize,
    },
    FileTreeProjection {
        containment_relation_source: String,
        sort_mode: String,
        root_filter: Option<String>,
        row_count: usize,
        selected_count: usize,
        expanded_count: usize,
        collapsed_count: usize,
    },
    RouteOpenBoundary {
        pending_node_context_target: Option<NodeKey>,
        pending_open_node: Option<(NodeKey, String)>,
        pending_open_connected: Option<(NodeKey, String, String)>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct UxNodeState {
    pub(crate) focused: bool,
    pub(crate) selected: bool,
    pub(crate) blocked: bool,
    pub(crate) degraded: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct UxSemanticNode {
    pub(crate) ux_node_id: String,
    pub(crate) parent_ux_node_id: Option<String>,
    pub(crate) role: UxNodeRole,
    pub(crate) label: String,
    pub(crate) state: UxNodeState,
    pub(crate) allowed_actions: Vec<UxAction>,
    pub(crate) domain: UxDomainIdentity,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct UxPresentationNode {
    pub(crate) ux_node_id: String,
    pub(crate) bounds: Option<[f32; 4]>,
    pub(crate) render_mode: Option<TileRenderMode>,
    pub(crate) z_pass: &'static str,
    pub(crate) style_flags: Vec<&'static str>,
    pub(crate) transient_flags: Vec<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UxTraceNode {
    pub(crate) ux_node_id: String,
    pub(crate) event_route: &'static str,
    pub(crate) backend_path: &'static str,
    pub(crate) diagnostics_counter: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UxTraceSummary {
    pub(crate) build_duration_us: u64,
    pub(crate) route_events_observed: u64,
    pub(crate) diagnostics_events_observed: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct UxTreeSnapshot {
    pub(crate) semantic_version: u32,
    pub(crate) presentation_version: u32,
    pub(crate) trace_version: u32,
    pub(crate) semantic_nodes: Vec<UxSemanticNode>,
    pub(crate) presentation_nodes: Vec<UxPresentationNode>,
    pub(crate) trace_nodes: Vec<UxTraceNode>,
    pub(crate) trace_summary: UxTraceSummary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UxDiffGateSeverity {
    Blocking,
    Informational,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UxSnapshotDiffGate {
    pub(crate) semantic_changed: bool,
    pub(crate) presentation_changed: bool,
    pub(crate) trace_changed: bool,
    pub(crate) semantic_severity: UxDiffGateSeverity,
    pub(crate) presentation_severity: UxDiffGateSeverity,
    pub(crate) trace_severity: UxDiffGateSeverity,
    pub(crate) blocking_failure: bool,
}

pub(crate) fn classify_snapshot_diff_gate(
    baseline: &UxTreeSnapshot,
    current: &UxTreeSnapshot,
    promote_presentation: bool,
) -> UxSnapshotDiffGate {
    let semantic_changed = baseline.semantic_version != current.semantic_version
        || baseline.semantic_nodes != current.semantic_nodes;
    let presentation_changed = baseline.presentation_version != current.presentation_version
        || baseline.presentation_nodes != current.presentation_nodes;
    let trace_changed = baseline.trace_version != current.trace_version
        || baseline.trace_nodes != current.trace_nodes
        || baseline.trace_summary != current.trace_summary;

    let semantic_severity = UxDiffGateSeverity::Blocking;
    let presentation_severity = if promote_presentation {
        UxDiffGateSeverity::Blocking
    } else {
        UxDiffGateSeverity::Informational
    };
    let trace_severity = UxDiffGateSeverity::Informational;

    let blocking_failure = semantic_changed
        || (presentation_changed && matches!(presentation_severity, UxDiffGateSeverity::Blocking));

    UxSnapshotDiffGate {
        semantic_changed,
        presentation_changed,
        trace_changed,
        semantic_severity,
        presentation_severity,
        trace_severity,
        blocking_failure,
    }
}

static LATEST_UX_TREE_SNAPSHOT: OnceLock<Mutex<Option<UxTreeSnapshot>>> = OnceLock::new();

fn snapshot_cache() -> &'static Mutex<Option<UxTreeSnapshot>> {
    LATEST_UX_TREE_SNAPSHOT.get_or_init(|| Mutex::new(None))
}

fn presentation_style_flags_for_mode(
    base_flags: &[&'static str],
    presentation_mode: PanePresentationMode,
) -> Vec<&'static str> {
    let mut flags = base_flags.to_vec();
    flags.push(match presentation_mode {
        PanePresentationMode::Tiled => "presentation:tiled",
        PanePresentationMode::Docked => "presentation:docked",
        PanePresentationMode::Floating => "presentation:floating",
        PanePresentationMode::Fullscreen => "presentation:fullscreen",
    });
    flags
}

pub(crate) fn publish_snapshot(snapshot: &UxTreeSnapshot) {
    if let Ok(mut slot) = snapshot_cache().lock() {
        *slot = Some(snapshot.clone());
    }
}

pub(crate) fn latest_snapshot() -> Option<UxTreeSnapshot> {
    snapshot_cache().lock().ok().and_then(|slot| slot.clone())
}

pub(crate) fn build_snapshot(
    tiles_tree: &Tree<TileKind>,
    graph_app: &GraphBrowserApp,
    build_duration_us: u64,
) -> UxTreeSnapshot {
    build_snapshot_with_rects(tiles_tree, graph_app, build_duration_us, &HashMap::new())
}

pub(crate) fn build_snapshot_with_rects(
    tiles_tree: &Tree<TileKind>,
    graph_app: &GraphBrowserApp,
    build_duration_us: u64,
    node_rects: &HashMap<NodeKey, egui::Rect>,
) -> UxTreeSnapshot {
    let active: HashSet<TileId> = tiles_tree.active_tiles().into_iter().collect();

    let mut semantic_nodes = vec![UxSemanticNode {
        ux_node_id: UX_TREE_WORKBENCH_ROOT_ID.to_string(),
        parent_ux_node_id: None,
        role: UxNodeRole::Workbench,
        label: "Workbench".to_string(),
        state: UxNodeState {
            focused: true,
            selected: false,
            blocked: false,
            degraded: false,
        },
        allowed_actions: vec![UxAction::Focus],
        domain: UxDomainIdentity::Workbench,
    }];

    let mut presentation_nodes = vec![UxPresentationNode {
        ux_node_id: UX_TREE_WORKBENCH_ROOT_ID.to_string(),
        bounds: None,
        render_mode: None,
        z_pass: "workbench:root",
        style_flags: vec!["spine:egui_tiles"],
        transient_flags: Vec::new(),
    }];

    let mut trace_nodes = vec![UxTraceNode {
        ux_node_id: UX_TREE_WORKBENCH_ROOT_ID.to_string(),
        event_route: "workbench.frame_loop",
        backend_path: "egui_tiles",
        diagnostics_counter: 0,
    }];

    if let Some(root) = tiles_tree.root() {
        push_nodes(
            tiles_tree,
            graph_app,
            root,
            &active,
            Some(UX_TREE_WORKBENCH_ROOT_ID),
            node_rects,
            &mut semantic_nodes,
            &mut presentation_nodes,
            &mut trace_nodes,
        );
    }

    append_radial_palette_nodes(
        &mut semantic_nodes,
        &mut presentation_nodes,
        &mut trace_nodes,
    );
    append_workbench_semantics_nodes(
        graph_app,
        &mut semantic_nodes,
        &mut presentation_nodes,
        &mut trace_nodes,
    );

    UxTreeSnapshot {
        semantic_version: UX_TREE_SEMANTIC_SCHEMA_VERSION,
        presentation_version: UX_TREE_PRESENTATION_SCHEMA_VERSION,
        trace_version: UX_TREE_TRACE_SCHEMA_VERSION,
        semantic_nodes,
        presentation_nodes,
        trace_nodes,
        trace_summary: UxTraceSummary {
            build_duration_us,
            route_events_observed: 0,
            diagnostics_events_observed: 0,
        },
    }
}

fn current_frame_tab_container_label(
    tiles_tree: &Tree<TileKind>,
    graph_app: &GraphBrowserApp,
    tile_id: TileId,
    child_count: usize,
) -> Option<String> {
    (tiles_tree.root() == Some(tile_id))
        .then(|| graph_app.current_frame_name())
        .flatten()
        .map(|frame_name| format!("Frame: {frame_name} ({child_count})"))
}

fn append_workbench_semantics_nodes(
    graph_app: &GraphBrowserApp,
    semantic_nodes: &mut Vec<UxSemanticNode>,
    presentation_nodes: &mut Vec<UxPresentationNode>,
    trace_nodes: &mut Vec<UxTraceNode>,
) {
    fn navigator_hosts_for_snapshot(graph_app: &GraphBrowserApp) -> Vec<SurfaceHostId> {
        let mut hosts = graph_app
            .workspace
            .workbench_session
            .active_layout_constraints
            .keys()
            .filter(|host| matches!(host, SurfaceHostId::Navigator(_)))
            .cloned()
            .collect::<Vec<_>>();
        if hosts.is_empty() {
            hosts.push(SurfaceHostId::Navigator(
                crate::app::workbench_layout_policy::NavigatorHostId::Right,
            ));
        }
        hosts.sort_by_key(|host| match host {
            SurfaceHostId::Navigator(crate::app::workbench_layout_policy::NavigatorHostId::Top) => {
                0
            }
            SurfaceHostId::Navigator(
                crate::app::workbench_layout_policy::NavigatorHostId::Bottom,
            ) => 1,
            SurfaceHostId::Navigator(
                crate::app::workbench_layout_policy::NavigatorHostId::Left,
            ) => 2,
            SurfaceHostId::Navigator(
                crate::app::workbench_layout_policy::NavigatorHostId::Right,
            ) => 3,
            SurfaceHostId::Role(_) => 4,
        });
        hosts.dedup();
        hosts
    }

    fn default_anchor_edge_for_host(surface_host: &SurfaceHostId) -> AnchorEdge {
        match surface_host {
            SurfaceHostId::Navigator(crate::app::workbench_layout_policy::NavigatorHostId::Top) => {
                AnchorEdge::Top
            }
            SurfaceHostId::Navigator(
                crate::app::workbench_layout_policy::NavigatorHostId::Bottom,
            ) => AnchorEdge::Bottom,
            SurfaceHostId::Navigator(
                crate::app::workbench_layout_policy::NavigatorHostId::Left,
            ) => AnchorEdge::Left,
            SurfaceHostId::Navigator(
                crate::app::workbench_layout_policy::NavigatorHostId::Right,
            )
            | SurfaceHostId::Role(_) => AnchorEdge::Right,
        }
    }

    fn default_form_factor_for_edge(anchor_edge: AnchorEdge) -> String {
        match anchor_edge {
            AnchorEdge::Top | AnchorEdge::Bottom => "toolbar".to_string(),
            AnchorEdge::Left | AnchorEdge::Right => "sidebar".to_string(),
        }
    }

    fn recent_traversal_node_timestamps(graph_app: &GraphBrowserApp) -> HashMap<NodeKey, u64> {
        let mut by_node: HashMap<NodeKey, u64> = HashMap::new();
        for edge in graph_app.domain_graph().edges() {
            let Some(edge_key) = graph_app.domain_graph().find_edge_key(edge.from, edge.to) else {
                continue;
            };
            let Some(payload) = graph_app.domain_graph().get_edge(edge_key) else {
                continue;
            };
            if !payload.has_relation(RelationSelector::Family(EdgeFamily::Traversal)) {
                continue;
            }
            let timestamp = payload.metrics().last_navigated_at.unwrap_or(0);
            if timestamp == 0 {
                continue;
            }
            by_node
                .entry(edge.to)
                .and_modify(|current| *current = (*current).max(timestamp))
                .or_insert(timestamp);
        }
        by_node
    }

    for (view_id, view_state) in &graph_app.workspace.graph_runtime.views {
        let selection_count = graph_app.selection_for_view(*view_id).len();
        let focused_view = graph_app.workspace.graph_runtime.focused_view == Some(*view_id);
        let lens_scope_id = format!("uxnode://workbench/graph/{view_id:?}/lens-scope");
        semantic_nodes.push(UxSemanticNode {
            ux_node_id: lens_scope_id.clone(),
            parent_ux_node_id: Some(UX_TREE_WORKBENCH_ROOT_ID.to_string()),
            role: UxNodeRole::GraphViewLensScope,
            label: format!("Graph View Lens/Scope {:?}", view_id),
            state: UxNodeState {
                focused: focused_view,
                selected: false,
                blocked: false,
                degraded: false,
            },
            allowed_actions: vec![UxAction::Navigate],
            domain: UxDomainIdentity::GraphViewLensScope {
                graph_view_id: *view_id,
                lens_name: view_state.lens.name.clone(),
                lens_id: view_state.lens.lens_id.clone(),
                physics_name: view_state.lens.physics.name.clone(),
                layout_mode: format!("{:?}", view_state.lens.layout),
                theme_name: view_state
                    .lens
                    .theme
                    .as_ref()
                    .map(|theme| crate::registries::atomic::lens::theme_data_id(theme).to_string()),
                filter_count: view_state.lens.filter_expr.is_some() as usize
                    + view_state.lens.filters_legacy.len(),
                dimension: format!("{:?}", view_state.dimension),
                position_fit_locked: view_state.position_fit_locked,
                zoom_fit_locked: view_state.zoom_fit_locked,
                focused_view,
                selection_count,
            },
        });
        presentation_nodes.push(UxPresentationNode {
            ux_node_id: lens_scope_id.clone(),
            bounds: None,
            render_mode: Some(TileRenderMode::EmbeddedEgui),
            z_pass: "workbench.graph.lens_scope",
            style_flags: vec!["surface:graph-lens-scope"],
            transient_flags: Vec::new(),
        });
        trace_nodes.push(UxTraceNode {
            ux_node_id: lens_scope_id,
            event_route: "graph.lens_scope_route",
            backend_path: "egui_graphs",
            diagnostics_counter: selection_count as u64,
        });
    }

    let navigator_projection = graph_app.navigator_projection_state();
    let arrangement_groups = graph_app.arrangement_projection_groups();
    let workbench_group_count = arrangement_groups.len();
    let mut arranged_nodes = HashSet::new();
    let mut workbench_member_count = 0usize;
    for group in arrangement_groups {
        workbench_member_count += group.member_keys.len();
        for node_key in group.member_keys {
            arranged_nodes.insert(node_key);
        }
    }
    let recent_timestamps = recent_traversal_node_timestamps(graph_app);
    let recent_count = recent_timestamps
        .keys()
        .filter(|key| !arranged_nodes.contains(key))
        .count();
    let unrelated_count = graph_app
        .domain_graph()
        .nodes()
        .filter(|(node_key, _)| {
            !arranged_nodes.contains(node_key) && !recent_timestamps.contains_key(node_key)
        })
        .count();
    for navigator_host in navigator_hosts_for_snapshot(graph_app) {
        let (anchor_edge, form_factor) = match graph_app
            .workspace
            .workbench_session
            .active_layout_constraints
            .get(&navigator_host)
        {
            Some(WorkbenchLayoutConstraint::AnchoredSplit { anchor_edge, .. }) => {
                (*anchor_edge, default_form_factor_for_edge(*anchor_edge))
            }
            _ => {
                let anchor_edge = default_anchor_edge_for_host(&navigator_host);
                (anchor_edge, default_form_factor_for_edge(anchor_edge))
            }
        };
        let configured_scope = graph_app.navigator_host_scope(&navigator_host);
        let navigator_projection_node_id = format!(
            "uxnode://workbench/navigator/projection/{}",
            navigator_host.to_string().replace(':', "/")
        );
        semantic_nodes.push(UxSemanticNode {
            ux_node_id: navigator_projection_node_id.clone(),
            parent_ux_node_id: Some(UX_TREE_WORKBENCH_ROOT_ID.to_string()),
            role: UxNodeRole::NavigatorProjection,
            label: format!("Navigator Projection {}", navigator_host),
            state: UxNodeState {
                focused: false,
                selected: !navigator_projection.selected_rows.is_empty(),
                blocked: navigator_projection.row_targets.is_empty(),
                degraded: false,
            },
            allowed_actions: vec![UxAction::Navigate],
            domain: UxDomainIdentity::NavigatorProjection {
                host: navigator_host.clone(),
                anchor_edge,
                form_factor,
                scope: configured_scope.as_str().to_string(),
                containment_relation_source: format!(
                    "{:?}",
                    navigator_projection.containment_relation_source
                ),
                sort_mode: format!("{:?}", navigator_projection.sort_mode),
                root_filter: navigator_projection.root_filter.clone(),
                row_count: navigator_projection.row_targets.len(),
                selected_count: navigator_projection.selected_rows.len(),
                expanded_count: navigator_projection.expanded_rows.len(),
                collapsed_count: navigator_projection.collapsed_rows.len(),
                workbench_group_count,
                workbench_member_count,
                unrelated_count,
                recent_count,
            },
        });
        presentation_nodes.push(UxPresentationNode {
            ux_node_id: navigator_projection_node_id.clone(),
            bounds: None,
            render_mode: Some(TileRenderMode::EmbeddedEgui),
            z_pass: "workbench.navigator.projection",
            style_flags: vec!["surface:navigator"],
            transient_flags: Vec::new(),
        });
        trace_nodes.push(UxTraceNode {
            ux_node_id: navigator_projection_node_id,
            event_route: "workbench.navigator_route",
            backend_path: "egui",
            diagnostics_counter: navigator_projection.row_targets.len() as u64,
        });
    }

    let route_node_id = "uxnode://workbench/route-open/boundary".to_string();
    let pending_open_node = graph_app.pending_open_node_request().map(|pending| {
        (
            pending.key,
            pending_tile_mode_label(pending.mode).to_string(),
        )
    });
    let pending_open_connected =
        graph_app
            .pending_open_connected_from()
            .map(|(source, mode, scope)| {
                (
                    source,
                    pending_tile_mode_label(mode).to_string(),
                    pending_connected_scope_label(scope).to_string(),
                )
            });
    let pending_count = usize::from(graph_app.pending_node_context_target().is_some())
        + usize::from(pending_open_node.is_some())
        + usize::from(pending_open_connected.is_some());
    semantic_nodes.push(UxSemanticNode {
        ux_node_id: route_node_id.clone(),
        parent_ux_node_id: Some(UX_TREE_WORKBENCH_ROOT_ID.to_string()),
        role: UxNodeRole::RouteOpenBoundary,
        label: "Route/Open Boundary".to_string(),
        state: UxNodeState {
            focused: false,
            selected: false,
            blocked: false,
            degraded: false,
        },
        allowed_actions: vec![UxAction::Open, UxAction::Navigate],
        domain: UxDomainIdentity::RouteOpenBoundary {
            pending_node_context_target: graph_app.pending_node_context_target(),
            pending_open_node,
            pending_open_connected,
        },
    });
    presentation_nodes.push(UxPresentationNode {
        ux_node_id: route_node_id.clone(),
        bounds: None,
        render_mode: Some(TileRenderMode::EmbeddedEgui),
        z_pass: "workbench.route_open.boundary",
        style_flags: vec!["surface:route-open"],
        transient_flags: Vec::new(),
    });
    trace_nodes.push(UxTraceNode {
        ux_node_id: route_node_id,
        event_route: "workbench.route_open_route",
        backend_path: "egui",
        diagnostics_counter: pending_count as u64,
    });
}

fn pending_tile_mode_label(mode: PendingTileOpenMode) -> &'static str {
    match mode {
        PendingTileOpenMode::Tab => "tab",
        PendingTileOpenMode::SplitHorizontal => "split-horizontal",
        PendingTileOpenMode::QuarterPane => "quarter-pane",
        PendingTileOpenMode::HalfPane => "half-pane",
    }
}

fn pending_connected_scope_label(scope: PendingConnectedOpenScope) -> &'static str {
    match scope {
        PendingConnectedOpenScope::Neighbors => "neighbors",
        PendingConnectedOpenScope::Connected => "connected",
    }
}

fn append_radial_palette_nodes(
    semantic_nodes: &mut Vec<UxSemanticNode>,
    presentation_nodes: &mut Vec<UxPresentationNode>,
    trace_nodes: &mut Vec<UxTraceNode>,
) {
    let Some(snapshot) = latest_semantic_snapshot() else {
        return;
    };

    let radial_root_id = "uxnode://command/radial/root".to_string();
    semantic_nodes.push(UxSemanticNode {
        ux_node_id: radial_root_id.clone(),
        parent_ux_node_id: Some(UX_TREE_WORKBENCH_ROOT_ID.to_string()),
        role: UxNodeRole::RadialPalette,
        label: "Radial Palette".to_string(),
        state: UxNodeState {
            focused: true,
            selected: false,
            blocked: false,
            degraded: false,
        },
        allowed_actions: vec![UxAction::Focus, UxAction::Dismiss, UxAction::Navigate],
        domain: UxDomainIdentity::Workbench,
    });
    presentation_nodes.push(UxPresentationNode {
        ux_node_id: radial_root_id.clone(),
        bounds: None,
        render_mode: Some(TileRenderMode::EmbeddedEgui),
        z_pass: "command.radial",
        style_flags: vec!["surface:radial"],
        transient_flags: vec!["mode:radial"],
    });
    trace_nodes.push(UxTraceNode {
        ux_node_id: radial_root_id.clone(),
        event_route: "command.radial_route",
        backend_path: "egui",
        diagnostics_counter: snapshot.sectors.len() as u64,
    });

    let tier1_ring_id = "uxnode://command/radial/tier-1-ring".to_string();
    semantic_nodes.push(UxSemanticNode {
        ux_node_id: tier1_ring_id.clone(),
        parent_ux_node_id: Some(radial_root_id.clone()),
        role: UxNodeRole::RadialTierRing,
        label: "Tier-1 Ring".to_string(),
        state: UxNodeState {
            focused: snapshot
                .sectors
                .iter()
                .any(|sector| sector.tier == 1 && sector.hover_scale > 1.0),
            selected: false,
            blocked: snapshot.summary.tier1_visible_count == 0,
            degraded: false,
        },
        allowed_actions: vec![UxAction::Navigate],
        domain: UxDomainIdentity::RadialTierRing {
            tier: 1,
            visible_count: snapshot.summary.tier1_visible_count,
            page: 0,
            page_count: 1,
        },
    });
    presentation_nodes.push(UxPresentationNode {
        ux_node_id: tier1_ring_id.clone(),
        bounds: None,
        render_mode: Some(TileRenderMode::EmbeddedEgui),
        z_pass: "command.radial.tier1",
        style_flags: vec!["surface:radial-tier"],
        transient_flags: Vec::new(),
    });
    trace_nodes.push(UxTraceNode {
        ux_node_id: tier1_ring_id.clone(),
        event_route: "command.radial_tier1_route",
        backend_path: "egui",
        diagnostics_counter: snapshot.summary.tier1_visible_count as u64,
    });

    let tier2_ring_id = "uxnode://command/radial/tier-2-ring".to_string();
    semantic_nodes.push(UxSemanticNode {
        ux_node_id: tier2_ring_id.clone(),
        parent_ux_node_id: Some(radial_root_id.clone()),
        role: UxNodeRole::RadialTierRing,
        label: "Tier-2 Ring".to_string(),
        state: UxNodeState {
            focused: snapshot
                .sectors
                .iter()
                .any(|sector| sector.tier == 2 && sector.hover_scale > 1.0),
            selected: false,
            blocked: snapshot.summary.tier2_visible_count == 0,
            degraded: snapshot.summary.fallback_to_palette,
        },
        allowed_actions: vec![UxAction::Navigate],
        domain: UxDomainIdentity::RadialTierRing {
            tier: 2,
            visible_count: snapshot.summary.tier2_visible_count,
            page: snapshot.summary.tier2_page,
            page_count: snapshot.summary.tier2_page_count,
        },
    });
    presentation_nodes.push(UxPresentationNode {
        ux_node_id: tier2_ring_id.clone(),
        bounds: None,
        render_mode: Some(TileRenderMode::EmbeddedEgui),
        z_pass: "command.radial.tier2",
        style_flags: vec!["surface:radial-tier"],
        transient_flags: Vec::new(),
    });
    trace_nodes.push(UxTraceNode {
        ux_node_id: tier2_ring_id.clone(),
        event_route: "command.radial_tier2_route",
        backend_path: "egui",
        diagnostics_counter: snapshot.summary.tier2_visible_count as u64,
    });

    let radial_summary_id = "uxnode://command/radial/summary".to_string();
    semantic_nodes.push(UxSemanticNode {
        ux_node_id: radial_summary_id.clone(),
        parent_ux_node_id: Some(radial_root_id.clone()),
        role: UxNodeRole::RadialSummary,
        label: "Radial Layout Summary".to_string(),
        state: UxNodeState {
            focused: false,
            selected: false,
            blocked: false,
            degraded: snapshot.summary.fallback_to_palette,
        },
        allowed_actions: vec![UxAction::Navigate],
        domain: UxDomainIdentity::RadialSummary {
            tier1_visible_count: snapshot.summary.tier1_visible_count,
            tier2_visible_count: snapshot.summary.tier2_visible_count,
            tier2_page: snapshot.summary.tier2_page,
            tier2_page_count: snapshot.summary.tier2_page_count,
            overflow_hidden_entries: snapshot.summary.overflow_hidden_entries,
            label_pre_collisions: snapshot.summary.label_pre_collisions,
            label_post_collisions: snapshot.summary.label_post_collisions,
            fallback_to_palette: snapshot.summary.fallback_to_palette,
            fallback_reason: snapshot.summary.fallback_reason.clone(),
        },
    });
    presentation_nodes.push(UxPresentationNode {
        ux_node_id: radial_summary_id.clone(),
        bounds: None,
        render_mode: Some(TileRenderMode::EmbeddedEgui),
        z_pass: "command.radial.summary",
        style_flags: vec!["surface:radial-summary"],
        transient_flags: Vec::new(),
    });
    trace_nodes.push(UxTraceNode {
        ux_node_id: radial_summary_id,
        event_route: "command.radial_summary_route",
        backend_path: "egui",
        diagnostics_counter: snapshot.summary.overflow_hidden_entries as u64,
    });

    for (idx, sector) in snapshot.sectors.iter().enumerate() {
        let sector_id = format!(
            "uxnode://command/radial/tier{}/domain/{}/sector/{}",
            sector.tier,
            sector.domain_label.to_ascii_lowercase(),
            idx
        );
        semantic_nodes.push(UxSemanticNode {
            ux_node_id: sector_id.clone(),
            parent_ux_node_id: Some(match sector.tier {
                1 => tier1_ring_id.clone(),
                _ => tier2_ring_id.clone(),
            }),
            role: UxNodeRole::RadialSector,
            label: format!("{} [{}]", sector.action_id, sector.domain_label),
            state: UxNodeState {
                focused: sector.hover_scale > 1.0,
                selected: false,
                blocked: !sector.enabled,
                degraded: false,
            },
            allowed_actions: vec![UxAction::Invoke, UxAction::Navigate],
            domain: UxDomainIdentity::RadialSector {
                action_id: sector.action_id.clone(),
                enabled: sector.enabled,
                tier: sector.tier,
                rail_position: sector.rail_position,
                hover_scale: sector.hover_scale,
                angle_rad: sector.angle_rad,
                page: sector.page,
            },
        });
        presentation_nodes.push(UxPresentationNode {
            ux_node_id: sector_id.clone(),
            bounds: None,
            render_mode: Some(TileRenderMode::EmbeddedEgui),
            z_pass: "command.radial.sector",
            style_flags: vec!["surface:radial-sector"],
            transient_flags: if sector.hover_scale > 1.0 {
                vec!["hovered"]
            } else {
                Vec::new()
            },
        });
        trace_nodes.push(UxTraceNode {
            ux_node_id: sector_id,
            event_route: "command.radial_sector_route",
            backend_path: "egui",
            diagnostics_counter: u64::from(sector.enabled),
        });
    }
}

fn push_nodes(
    tiles_tree: &Tree<TileKind>,
    graph_app: &GraphBrowserApp,
    tile_id: TileId,
    active: &HashSet<TileId>,
    parent_ux_node_id: Option<&str>,
    node_rects: &HashMap<NodeKey, egui::Rect>,
    semantic_nodes: &mut Vec<UxSemanticNode>,
    presentation_nodes: &mut Vec<UxPresentationNode>,
    trace_nodes: &mut Vec<UxTraceNode>,
) {
    let Some(tile) = tiles_tree.tiles.get(tile_id) else {
        return;
    };

    let ux_node_id = ux_node_id_for_tile(tile_id, tile);
    let focused = active.contains(&tile_id);
    let tile_selected = graph_app
        .workbench_tile_selection()
        .selected_tile_ids
        .contains(&tile_id);

    match tile {
        Tile::Pane(TileKind::Pane(
            crate::shell::desktop::workbench::pane_model::PaneViewState::Graph(view_ref),
        )) => {
            let focused_selection = graph_app.focused_selection();
            semantic_nodes.push(UxSemanticNode {
                ux_node_id: ux_node_id.clone(),
                parent_ux_node_id: parent_ux_node_id.map(str::to_string),
                role: UxNodeRole::GraphSurface,
                label: format!("Graph View {:?}", view_ref.graph_view_id),
                state: UxNodeState {
                    focused,
                    selected: tile_selected,
                    blocked: false,
                    degraded: false,
                },
                allowed_actions: vec![UxAction::Focus, UxAction::Navigate],
                domain: UxDomainIdentity::GraphView {
                    graph_view_id: view_ref.graph_view_id,
                },
            });
            presentation_nodes.push(UxPresentationNode {
                ux_node_id: ux_node_id.clone(),
                bounds: None,
                render_mode: Some(TileRenderMode::EmbeddedEgui),
                z_pass: "workbench.content",
                style_flags: vec!["surface:graph", "backend:egui_graphs"],
                transient_flags: Vec::new(),
            });
            trace_nodes.push(UxTraceNode {
                ux_node_id: ux_node_id.clone(),
                event_route: "graph.input_route",
                backend_path: "egui_graphs",
                diagnostics_counter: graph_app.domain_graph().node_count() as u64,
            });

            for (node_key, node) in graph_app.domain_graph().nodes() {
                let graph_node_ux_id = format!(
                    "uxnode://workbench/graph/{:?}/node/{}",
                    view_ref.graph_view_id, node.id
                );
                let selected = focused_selection.contains(&node_key);
                let focused_graph_node = focused_selection.primary() == Some(node_key);
                let blocked = graph_app.runtime_block_state_for_node(node_key).is_some();
                let degraded = graph_app.runtime_crash_state_for_node(node_key).is_some();

                semantic_nodes.push(UxSemanticNode {
                    ux_node_id: graph_node_ux_id.clone(),
                    parent_ux_node_id: Some(ux_node_id.clone()),
                    role: UxNodeRole::GraphNode,
                    label: if node.title.is_empty() {
                        node.url().to_string()
                    } else {
                        node.title.clone()
                    },
                    state: UxNodeState {
                        focused: focused_graph_node,
                        selected,
                        blocked,
                        degraded,
                    },
                    allowed_actions: vec![UxAction::Select, UxAction::Open, UxAction::Navigate],
                    domain: UxDomainIdentity::Node { node_key },
                });
                presentation_nodes.push(UxPresentationNode {
                    ux_node_id: graph_node_ux_id.clone(),
                    bounds: None,
                    render_mode: Some(TileRenderMode::EmbeddedEgui),
                    z_pass: "graph.layer.node",
                    style_flags: vec!["surface:graph-node"],
                    transient_flags: Vec::new(),
                });
                trace_nodes.push(UxTraceNode {
                    ux_node_id: graph_node_ux_id,
                    event_route: "graph.node_route",
                    backend_path: "egui_graphs",
                    diagnostics_counter: u64::from(selected),
                });
            }
        }
        Tile::Pane(TileKind::Pane(
            crate::shell::desktop::workbench::pane_model::PaneViewState::Node(state),
        )) => {
            let focused_selection = graph_app.focused_selection();
            let blocked = graph_app.runtime_block_state_for_node(state.node).is_some();
            let degraded = matches!(state.render_mode, TileRenderMode::Placeholder);
            let selected = focused_selection.contains(&state.node);

            semantic_nodes.push(UxSemanticNode {
                ux_node_id: ux_node_id.clone(),
                parent_ux_node_id: parent_ux_node_id.map(str::to_string),
                role: UxNodeRole::NodePane,
                label: format!("Node Pane {:?}", state.node),
                state: UxNodeState {
                    focused,
                    selected: selected || tile_selected,
                    blocked,
                    degraded,
                },
                allowed_actions: vec![
                    UxAction::Focus,
                    UxAction::Open,
                    UxAction::Close,
                    UxAction::SplitHorizontal,
                ],
                domain: UxDomainIdentity::Node {
                    node_key: state.node,
                },
            });
            let bounds = node_rects
                .get(&state.node)
                .map(|r| [r.min.x, r.min.y, r.max.x, r.max.y]);
            presentation_nodes.push(UxPresentationNode {
                ux_node_id: ux_node_id.clone(),
                bounds,
                render_mode: Some(state.render_mode),
                z_pass: "workbench.content",
                style_flags: presentation_style_flags_for_mode(
                    &["surface:node"],
                    state.presentation_mode,
                ),
                transient_flags: Vec::new(),
            });
            trace_nodes.push(UxTraceNode {
                ux_node_id: ux_node_id.clone(),
                event_route: "workbench.node_route",
                backend_path: match state.render_mode {
                    TileRenderMode::CompositedTexture => "viewer.composited",
                    TileRenderMode::NativeOverlay => "viewer.native_overlay",
                    TileRenderMode::EmbeddedEgui => "viewer.embedded_egui",
                    TileRenderMode::Placeholder => "viewer.placeholder",
                },
                diagnostics_counter: u64::from(focused),
            });
        }
        #[cfg(feature = "diagnostics")]
        Tile::Pane(TileKind::Pane(
            crate::shell::desktop::workbench::pane_model::PaneViewState::Tool(tool),
        )) => {
            let tool_kind = tool.title();
            semantic_nodes.push(UxSemanticNode {
                ux_node_id: ux_node_id.clone(),
                parent_ux_node_id: parent_ux_node_id.map(str::to_string),
                role: UxNodeRole::ToolPane,
                label: format!("Tool Pane {tool_kind}"),
                state: UxNodeState {
                    focused,
                    selected: tile_selected,
                    blocked: false,
                    degraded: false,
                },
                allowed_actions: vec![UxAction::Focus, UxAction::Close],
                domain: UxDomainIdentity::Tool { tool_kind },
            });
        }
        Tile::Pane(TileKind::Graph(view_ref)) => {
            let focused_selection = graph_app.focused_selection();
            semantic_nodes.push(UxSemanticNode {
                ux_node_id: ux_node_id.clone(),
                parent_ux_node_id: parent_ux_node_id.map(str::to_string),
                role: UxNodeRole::GraphSurface,
                label: format!("Graph View {:?}", view_ref.graph_view_id),
                state: UxNodeState {
                    focused,
                    selected: tile_selected,
                    blocked: false,
                    degraded: false,
                },
                allowed_actions: vec![UxAction::Focus, UxAction::Navigate],
                domain: UxDomainIdentity::GraphView {
                    graph_view_id: view_ref.graph_view_id,
                },
            });
            presentation_nodes.push(UxPresentationNode {
                ux_node_id: ux_node_id.clone(),
                bounds: None,
                render_mode: Some(TileRenderMode::EmbeddedEgui),
                z_pass: "workbench.content",
                style_flags: presentation_style_flags_for_mode(
                    &["surface:graph", "backend:egui_graphs"],
                    view_ref.presentation_mode,
                ),
                transient_flags: Vec::new(),
            });
            trace_nodes.push(UxTraceNode {
                ux_node_id: ux_node_id.clone(),
                event_route: "graph.input_route",
                backend_path: "egui_graphs",
                diagnostics_counter: graph_app.domain_graph().node_count() as u64,
            });

            for (node_key, node) in graph_app.domain_graph().nodes() {
                let graph_node_ux_id = format!(
                    "uxnode://workbench/graph/{:?}/node/{}",
                    view_ref.graph_view_id, node.id
                );
                let selected = focused_selection.contains(&node_key);
                let focused_graph_node = focused_selection.primary() == Some(node_key);
                let blocked = graph_app.runtime_block_state_for_node(node_key).is_some();
                let degraded = graph_app.runtime_crash_state_for_node(node_key).is_some();

                semantic_nodes.push(UxSemanticNode {
                    ux_node_id: graph_node_ux_id.clone(),
                    parent_ux_node_id: Some(ux_node_id.clone()),
                    role: UxNodeRole::GraphNode,
                    label: if node.title.is_empty() {
                        node.url().to_string()
                    } else {
                        node.title.clone()
                    },
                    state: UxNodeState {
                        focused: focused_graph_node,
                        selected,
                        blocked,
                        degraded,
                    },
                    allowed_actions: vec![UxAction::Select, UxAction::Open, UxAction::Navigate],
                    domain: UxDomainIdentity::Node { node_key },
                });
                presentation_nodes.push(UxPresentationNode {
                    ux_node_id: graph_node_ux_id.clone(),
                    bounds: None,
                    render_mode: Some(TileRenderMode::EmbeddedEgui),
                    z_pass: "workbench.content",
                    style_flags: vec!["surface:graph-node", "backend:egui_graphs"],
                    transient_flags: Vec::new(),
                });
                trace_nodes.push(UxTraceNode {
                    ux_node_id: graph_node_ux_id,
                    event_route: "graph.node_route",
                    backend_path: "egui_graphs",
                    diagnostics_counter: u64::from(selected),
                });
            }
        }
        Tile::Pane(TileKind::Node(state)) => {
            let focused_selection = graph_app.focused_selection();
            let blocked = graph_app.runtime_block_state_for_node(state.node).is_some();
            let degraded = matches!(state.render_mode, TileRenderMode::Placeholder);
            let selected = focused_selection.contains(&state.node);

            semantic_nodes.push(UxSemanticNode {
                ux_node_id: ux_node_id.clone(),
                parent_ux_node_id: parent_ux_node_id.map(str::to_string),
                role: UxNodeRole::NodePane,
                label: format!("Node Pane {:?}", state.node),
                state: UxNodeState {
                    focused,
                    selected: selected || tile_selected,
                    blocked,
                    degraded,
                },
                allowed_actions: vec![
                    UxAction::Focus,
                    UxAction::Open,
                    UxAction::Close,
                    UxAction::SplitHorizontal,
                ],
                domain: UxDomainIdentity::Node {
                    node_key: state.node,
                },
            });
            let bounds = node_rects
                .get(&state.node)
                .map(|r| [r.min.x, r.min.y, r.max.x, r.max.y]);
            presentation_nodes.push(UxPresentationNode {
                ux_node_id: ux_node_id.clone(),
                bounds,
                render_mode: Some(state.render_mode),
                z_pass: "workbench.content",
                style_flags: presentation_style_flags_for_mode(
                    &["surface:node"],
                    state.presentation_mode,
                ),
                transient_flags: Vec::new(),
            });
            trace_nodes.push(UxTraceNode {
                ux_node_id: ux_node_id.clone(),
                event_route: "workbench.node_route",
                backend_path: match state.render_mode {
                    TileRenderMode::CompositedTexture => "viewer.composited",
                    TileRenderMode::NativeOverlay => "viewer.native_overlay",
                    TileRenderMode::EmbeddedEgui => "viewer.embedded_egui",
                    TileRenderMode::Placeholder => "viewer.placeholder",
                },
                diagnostics_counter: u64::from(focused),
            });
        }
        #[cfg(feature = "diagnostics")]
        Tile::Pane(TileKind::Tool(tool)) => {
            let tool_kind = tool.title();
            semantic_nodes.push(UxSemanticNode {
                ux_node_id: ux_node_id.clone(),
                parent_ux_node_id: parent_ux_node_id.map(str::to_string),
                role: UxNodeRole::ToolPane,
                label: format!("Tool Pane {tool_kind}"),
                state: UxNodeState {
                    focused,
                    selected: tile_selected,
                    blocked: false,
                    degraded: false,
                },
                allowed_actions: vec![UxAction::Focus, UxAction::Close],
                domain: UxDomainIdentity::Tool { tool_kind },
            });
            presentation_nodes.push(UxPresentationNode {
                ux_node_id: ux_node_id.clone(),
                bounds: None,
                render_mode: Some(TileRenderMode::EmbeddedEgui),
                z_pass: "workbench.tool",
                style_flags: presentation_style_flags_for_mode(
                    &["surface:tool"],
                    tool.presentation_mode,
                ),
                transient_flags: Vec::new(),
            });
            trace_nodes.push(UxTraceNode {
                ux_node_id: ux_node_id.clone(),
                event_route: "workbench.tool_route",
                backend_path: "egui",
                diagnostics_counter: 0,
            });
        }
        Tile::Container(Container::Tabs(tabs)) => {
            semantic_nodes.push(UxSemanticNode {
                ux_node_id: ux_node_id.clone(),
                parent_ux_node_id: parent_ux_node_id.map(str::to_string),
                role: UxNodeRole::TabContainer,
                label: current_frame_tab_container_label(
                    tiles_tree,
                    graph_app,
                    tile_id,
                    tabs.children.len(),
                )
                .unwrap_or_else(|| format!("Tabs ({})", tabs.children.len())),
                state: UxNodeState {
                    focused,
                    selected: false,
                    blocked: false,
                    degraded: false,
                },
                allowed_actions: vec![UxAction::Focus],
                domain: UxDomainIdentity::Workbench,
            });
            presentation_nodes.push(UxPresentationNode {
                ux_node_id: ux_node_id.clone(),
                bounds: None,
                render_mode: None,
                z_pass: "workbench.tabs",
                style_flags: vec!["container:tabs"],
                transient_flags: Vec::new(),
            });
            trace_nodes.push(UxTraceNode {
                ux_node_id: ux_node_id.clone(),
                event_route: "workbench.tabs_route",
                backend_path: "egui_tiles",
                diagnostics_counter: tabs.children.len() as u64,
            });

            for child in &tabs.children {
                push_nodes(
                    tiles_tree,
                    graph_app,
                    *child,
                    active,
                    Some(ux_node_id.as_str()),
                    node_rects,
                    semantic_nodes,
                    presentation_nodes,
                    trace_nodes,
                );
            }
        }
        Tile::Container(Container::Linear(linear)) => {
            semantic_nodes.push(UxSemanticNode {
                ux_node_id: ux_node_id.clone(),
                parent_ux_node_id: parent_ux_node_id.map(str::to_string),
                role: UxNodeRole::SplitContainer,
                label: format!("Split ({})", linear.children.len()),
                state: UxNodeState {
                    focused,
                    selected: false,
                    blocked: false,
                    degraded: false,
                },
                allowed_actions: vec![UxAction::Focus],
                domain: UxDomainIdentity::Workbench,
            });
            presentation_nodes.push(UxPresentationNode {
                ux_node_id: ux_node_id.clone(),
                bounds: None,
                render_mode: None,
                z_pass: "workbench.split",
                style_flags: vec!["container:linear"],
                transient_flags: Vec::new(),
            });
            trace_nodes.push(UxTraceNode {
                ux_node_id: ux_node_id.clone(),
                event_route: "workbench.split_route",
                backend_path: "egui_tiles",
                diagnostics_counter: linear.children.len() as u64,
            });

            for child in &linear.children {
                push_nodes(
                    tiles_tree,
                    graph_app,
                    *child,
                    active,
                    Some(ux_node_id.as_str()),
                    node_rects,
                    semantic_nodes,
                    presentation_nodes,
                    trace_nodes,
                );
            }
        }
        Tile::Container(Container::Grid(grid)) => {
            let grid_children_count = grid.children().count();
            semantic_nodes.push(UxSemanticNode {
                ux_node_id: ux_node_id.clone(),
                parent_ux_node_id: parent_ux_node_id.map(str::to_string),
                role: UxNodeRole::SplitContainer,
                label: format!("Grid ({})", grid_children_count),
                state: UxNodeState {
                    focused,
                    selected: false,
                    blocked: false,
                    degraded: false,
                },
                allowed_actions: vec![UxAction::Focus],
                domain: UxDomainIdentity::Workbench,
            });
            presentation_nodes.push(UxPresentationNode {
                ux_node_id: ux_node_id.clone(),
                bounds: None,
                render_mode: None,
                z_pass: "workbench.grid",
                style_flags: vec!["container:grid"],
                transient_flags: Vec::new(),
            });
            trace_nodes.push(UxTraceNode {
                ux_node_id: ux_node_id.clone(),
                event_route: "workbench.grid_route",
                backend_path: "egui_tiles",
                diagnostics_counter: grid_children_count as u64,
            });

            for child in grid.children() {
                push_nodes(
                    tiles_tree,
                    graph_app,
                    *child,
                    active,
                    Some(ux_node_id.as_str()),
                    node_rects,
                    semantic_nodes,
                    presentation_nodes,
                    trace_nodes,
                );
            }
        }
    }
}

fn ux_node_id_for_tile(tile_id: TileId, tile: &Tile<TileKind>) -> String {
    match tile {
        Tile::Pane(TileKind::Pane(
            crate::shell::desktop::workbench::pane_model::PaneViewState::Graph(view_ref),
        )) => {
            format!(
                "uxnode://workbench/tile/{tile_id:?}/graph/{:?}",
                view_ref.graph_view_id
            )
        }
        Tile::Pane(TileKind::Pane(
            crate::shell::desktop::workbench::pane_model::PaneViewState::Node(state),
        )) => {
            format!("uxnode://workbench/tile/{tile_id:?}/node/{:?}", state.node)
        }
        #[cfg(feature = "diagnostics")]
        Tile::Pane(TileKind::Pane(
            crate::shell::desktop::workbench::pane_model::PaneViewState::Tool(tool),
        )) => {
            format!("uxnode://workbench/tile/{tile_id:?}/tool/{}", tool.title())
        }
        Tile::Pane(TileKind::Graph(view_ref)) => {
            format!(
                "uxnode://workbench/tile/{tile_id:?}/graph/{:?}",
                view_ref.graph_view_id
            )
        }
        Tile::Pane(TileKind::Node(state)) => {
            format!("uxnode://workbench/tile/{tile_id:?}/node/{:?}", state.node)
        }
        #[cfg(feature = "diagnostics")]
        Tile::Pane(TileKind::Tool(tool)) => {
            format!("uxnode://workbench/tile/{tile_id:?}/tool/{}", tool.title())
        }
        Tile::Container(Container::Tabs(_)) => {
            format!("uxnode://workbench/tile/{tile_id:?}/tabs")
        }
        Tile::Container(Container::Linear(_)) => {
            format!("uxnode://workbench/tile/{tile_id:?}/split")
        }
        Tile::Container(Container::Grid(_)) => {
            format!("uxnode://workbench/tile/{tile_id:?}/grid")
        }
    }
}

pub(crate) fn semantic_ids(snapshot: &UxTreeSnapshot) -> HashSet<&str> {
    snapshot
        .semantic_nodes
        .iter()
        .map(|node| node.ux_node_id.as_str())
        .collect()
}

pub(crate) fn presentation_ids(snapshot: &UxTreeSnapshot) -> HashSet<&str> {
    snapshot
        .presentation_nodes
        .iter()
        .map(|node| node.ux_node_id.as_str())
        .collect()
}

pub(crate) fn presentation_id_consistency_violation(snapshot: &UxTreeSnapshot) -> Option<String> {
    let semantic = semantic_ids(snapshot);
    for node in &snapshot.presentation_nodes {
        if !semantic.contains(node.ux_node_id.as_str()) {
            return Some(format!(
                "uxtree invariant failed: presentation ux_node_id '{}' missing from semantic layer",
                node.ux_node_id
            ));
        }
    }
    None
}

pub(crate) fn node_pane_bounds_missing_count(snapshot: &UxTreeSnapshot) -> usize {
    let presentation_has_bounds: std::collections::HashMap<&str, bool> = snapshot
        .presentation_nodes
        .iter()
        .map(|n| (n.ux_node_id.as_str(), n.bounds.is_some()))
        .collect();
    snapshot
        .semantic_nodes
        .iter()
        .filter(|n| n.role == UxNodeRole::NodePane)
        .filter(|n| {
            !presentation_has_bounds
                .get(n.ux_node_id.as_str())
                .copied()
                .unwrap_or(false)
        })
        .count()
}

pub(crate) struct CoverageReport {
    /// Number of adjacent tile pairs with a gap > 1 px between their edges.
    pub(crate) gutter_pair_count: usize,
    /// Number of tile pairs whose rects have a non-zero area intersection.
    pub(crate) overlap_pair_count: usize,
}

/// Pure coverage analysis over a set of laid-out tile rects.
///
/// A *gutter* is a gap of more than 1 px between two rects that share an
/// axis-aligned edge direction (i.e. one rect's right edge is close to another
/// rect's left edge, or top/bottom equivalents) but do not actually touch.
///
/// An *overlap* is any pair of rects whose intersection has positive area.
pub(crate) fn run_coverage_analysis(
    rects: &std::collections::HashMap<crate::graph::NodeKey, egui::Rect>,
) -> CoverageReport {
    const GAP_THRESHOLD: f32 = 1.0;

    let rects: Vec<egui::Rect> = rects.values().copied().collect();
    let mut gutter_pair_count = 0usize;
    let mut overlap_pair_count = 0usize;

    for i in 0..rects.len() {
        for j in (i + 1)..rects.len() {
            let a = rects[i];
            let b = rects[j];

            // Overlap: intersection has positive area.
            let inter = a.intersect(b);
            if inter.width() > 0.0 && inter.height() > 0.0 {
                overlap_pair_count += 1;
                continue;
            }

            // Gutter: rects do not overlap but are "neighbours" along one axis
            // with a gap > GAP_THRESHOLD on the shared axis.
            //
            // Horizontal neighbours: x-projections overlap or are adjacent,
            // one rect's right is close to the other's left.
            let x_gap = if a.max.x <= b.min.x {
                b.min.x - a.max.x
            } else if b.max.x <= a.min.x {
                a.min.x - b.max.x
            } else {
                0.0
            };
            let y_gap = if a.max.y <= b.min.y {
                b.min.y - a.max.y
            } else if b.max.y <= a.min.y {
                a.min.y - b.max.y
            } else {
                0.0
            };

            // Only consider pairs that are neighbours along exactly one axis
            // (the other axis has overlapping or touching projections).
            let x_proj_overlap =
                a.min.x < b.max.x + GAP_THRESHOLD && b.min.x < a.max.x + GAP_THRESHOLD;
            let y_proj_overlap =
                a.min.y < b.max.y + GAP_THRESHOLD && b.min.y < a.max.y + GAP_THRESHOLD;

            if x_gap > GAP_THRESHOLD && y_proj_overlap {
                gutter_pair_count += 1;
            } else if y_gap > GAP_THRESHOLD && x_proj_overlap {
                gutter_pair_count += 1;
            }
        }
    }

    CoverageReport {
        gutter_pair_count,
        overlap_pair_count,
    }
}

#[cfg(test)]
pub(crate) fn snapshot_json_for_tests(snapshot: &UxTreeSnapshot) -> serde_json::Value {
    serde_json::json!({
        "semantic_version": snapshot.semantic_version,
        "presentation_version": snapshot.presentation_version,
        "trace_version": snapshot.trace_version,
        "semantic_nodes": snapshot.semantic_nodes.iter().map(|node| serde_json::json!({
            "ux_node_id": node.ux_node_id,
            "parent_ux_node_id": node.parent_ux_node_id,
            "role": format!("{:?}", node.role),
            "label": node.label,
            "focused": node.state.focused,
            "selected": node.state.selected,
            "blocked": node.state.blocked,
            "degraded": node.state.degraded,
            "allowed_actions": node.allowed_actions.iter().map(|a| format!("{:?}", a)).collect::<Vec<_>>(),
            "domain": format!("{:?}", node.domain),
        })).collect::<Vec<_>>(),
        "presentation_nodes": snapshot.presentation_nodes.iter().map(|node| serde_json::json!({
            "ux_node_id": node.ux_node_id,
            "z_pass": node.z_pass,
            "style_flags": node.style_flags,
            "transient_flags": node.transient_flags,
            "render_mode": node.render_mode.map(|mode| format!("{:?}", mode)),
        })).collect::<Vec<_>>(),
        "trace_nodes": snapshot.trace_nodes.iter().map(|node| serde_json::json!({
            "ux_node_id": node.ux_node_id,
            "event_route": node.event_route,
            "backend_path": node.backend_path,
            "diagnostics_counter": node.diagnostics_counter,
        })).collect::<Vec<_>>(),
        "trace_summary": {
            "build_duration_us": snapshot.trace_summary.build_duration_us,
            "route_events_observed": snapshot.trace_summary.route_events_observed,
            "diagnostics_events_observed": snapshot.trace_summary.diagnostics_events_observed,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{
        NavigatorContainmentRelationSource, PendingConnectedOpenScope, PendingTileOpenMode,
    };
    use crate::render::radial_menu::{
        RadialPaletteSemanticSnapshot, RadialPaletteSemanticSummary, RadialSectorSemanticMetadata,
        clear_semantic_snapshot, publish_semantic_snapshot,
    };
    use crate::shell::desktop::tests::harness::TestRegistry;

    #[test]
    fn snapshot_uses_single_canonical_id_space_across_layers() {
        let mut harness = TestRegistry::new();
        let node = harness.add_node("https://ux-tree-id.example");
        harness.open_node_tab(node);

        let snapshot = build_snapshot(&harness.tiles_tree, &harness.app, 12);
        let semantic = semantic_ids(&snapshot);
        let presentation = presentation_ids(&snapshot);

        assert!(
            presentation.is_subset(&semantic),
            "presentation ids must be subset of semantic ids"
        );
    }

    #[test]
    fn consistency_probe_flags_missing_semantic_id_for_presentation_node() {
        let mut harness = TestRegistry::new();
        let node = harness.add_node("https://ux-tree-probe.example");
        harness.open_node_tab(node);
        let mut snapshot = build_snapshot(&harness.tiles_tree, &harness.app, 5);

        snapshot.presentation_nodes.push(UxPresentationNode {
            ux_node_id: "uxnode://orphan/presentation".to_string(),
            bounds: None,
            render_mode: None,
            z_pass: "workbench.orphan",
            style_flags: Vec::new(),
            transient_flags: Vec::new(),
        });

        let violation = presentation_id_consistency_violation(&snapshot)
            .expect("probe should detect orphan presentation node");
        assert!(violation.contains("orphan/presentation"));
    }

    #[test]
    fn snapshot_projects_stable_parent_links_for_graph_surface_hierarchy() {
        let mut harness = TestRegistry::new();
        let node = harness.add_node("https://ux-tree-parent-links.example");
        harness.open_node_tab(node);
        harness.app.select_node(node, false);

        let snapshot = build_snapshot(&harness.tiles_tree, &harness.app, 5);
        let graph_surface = snapshot
            .semantic_nodes
            .iter()
            .find(|entry| entry.role == UxNodeRole::GraphSurface)
            .expect("graph surface should be present");
        let graph_surface_parent = snapshot
            .semantic_nodes
            .iter()
            .find(|entry| {
                Some(entry.ux_node_id.as_str()) == graph_surface.parent_ux_node_id.as_deref()
            })
            .expect("graph surface parent should be projected");
        assert_eq!(graph_surface_parent.role, UxNodeRole::TabContainer);

        let graph_node = snapshot
            .semantic_nodes
            .iter()
            .find(|entry| entry.role == UxNodeRole::GraphNode && matches!(entry.domain, UxDomainIdentity::Node { node_key } if node_key == node))
            .expect("selected graph node should be projected");
        assert_eq!(
            graph_node.parent_ux_node_id.as_deref(),
            Some(graph_surface.ux_node_id.as_str())
        );
    }

    #[test]
    fn snapshot_labels_root_tab_container_as_active_frame_when_frame_handle_is_open() {
        let mut harness = TestRegistry::new();
        let node = harness.add_node("https://ux-tree-frame.example");
        harness.open_node_tab(node);
        harness.app.note_frame_activated("alpha", [node]);

        let snapshot = build_snapshot(&harness.tiles_tree, &harness.app, 7);

        let frame_tabs = snapshot
            .semantic_nodes
            .iter()
            .find(|entry| {
                entry.role == UxNodeRole::TabContainer && entry.label.contains("Frame: alpha")
            })
            .expect("active frame tab container should be labeled explicitly");
        assert_eq!(
            frame_tabs.parent_ux_node_id.as_deref(),
            Some(UX_TREE_WORKBENCH_ROOT_ID)
        );
    }

    #[test]
    fn diff_gate_semantic_changes_are_blocking_by_default() {
        let mut harness = TestRegistry::new();
        let node = harness.add_node("https://ux-tree-diff-semantic.example");
        harness.open_node_tab(node);

        let baseline = build_snapshot(&harness.tiles_tree, &harness.app, 10);
        let mut current = baseline.clone();
        current.semantic_nodes[0].label = "Workbench Renamed".to_string();

        let gate = classify_snapshot_diff_gate(&baseline, &current, false);
        assert!(gate.semantic_changed);
        assert!(gate.blocking_failure);
        assert_eq!(gate.semantic_severity, UxDiffGateSeverity::Blocking);
    }

    #[test]
    fn diff_gate_presentation_changes_are_informational_by_default() {
        let mut harness = TestRegistry::new();
        let node = harness.add_node("https://ux-tree-diff-presentation.example");
        harness.open_node_tab(node);

        let baseline = build_snapshot(&harness.tiles_tree, &harness.app, 10);
        let mut current = baseline.clone();
        current.presentation_nodes[0]
            .transient_flags
            .push("anim:pulse");

        let gate = classify_snapshot_diff_gate(&baseline, &current, false);
        assert!(!gate.semantic_changed);
        assert!(gate.presentation_changed);
        assert!(!gate.blocking_failure);
        assert_eq!(
            gate.presentation_severity,
            UxDiffGateSeverity::Informational
        );
    }

    #[test]
    fn diff_gate_can_promote_presentation_changes_to_blocking() {
        let mut harness = TestRegistry::new();
        let node = harness.add_node("https://ux-tree-diff-promotion.example");
        harness.open_node_tab(node);

        let baseline = build_snapshot(&harness.tiles_tree, &harness.app, 10);
        let mut current = baseline.clone();
        current.presentation_nodes[0]
            .style_flags
            .push("promoted-style");

        let gate = classify_snapshot_diff_gate(&baseline, &current, true);
        assert!(gate.presentation_changed);
        assert!(gate.blocking_failure);
        assert_eq!(gate.presentation_severity, UxDiffGateSeverity::Blocking);
    }

    #[test]
    fn snapshot_projects_graph_nodes_into_semantic_layer() {
        let mut harness = TestRegistry::new();
        let node_a = harness.add_node("https://ux-tree-graph-a.example");
        let node_b = harness.add_node("https://ux-tree-graph-b.example");
        harness.open_node_tab(node_a);
        harness.app.select_node(node_b, false);

        let snapshot = build_snapshot(&harness.tiles_tree, &harness.app, 14);
        let graph_nodes = snapshot
            .semantic_nodes
            .iter()
            .filter(|entry| entry.role == UxNodeRole::GraphNode)
            .collect::<Vec<_>>();

        assert_eq!(graph_nodes.len(), 2);
        assert!(graph_nodes.iter().any(|entry| entry.state.selected));
    }

    #[test]
    fn snapshot_projects_radial_sector_metadata_when_available() {
        clear_semantic_snapshot();
        publish_semantic_snapshot(RadialPaletteSemanticSnapshot {
            sectors: vec![RadialSectorSemanticMetadata {
                tier: 2,
                domain_label: "Node".to_string(),
                action_id: "NodeDelete".to_string(),
                enabled: true,
                page: 0,
                rail_position: 0.15,
                angle_rad: 1.2,
                hover_scale: 1.5,
            }],
            summary: RadialPaletteSemanticSummary {
                tier1_visible_count: 4,
                tier2_visible_count: 1,
                tier2_page: 0,
                tier2_page_count: 1,
                overflow_hidden_entries: 0,
                label_pre_collisions: 2,
                label_post_collisions: 0,
                fallback_to_palette: false,
                fallback_reason: None,
            },
        });

        let harness = TestRegistry::new();
        let snapshot = build_snapshot(&harness.tiles_tree, &harness.app, 7);

        assert!(
            snapshot
                .semantic_nodes
                .iter()
                .any(|node| node.role == UxNodeRole::RadialPalette),
            "snapshot should include radial palette root when metadata is available"
        );
        assert!(
            snapshot.semantic_nodes.iter().any(|node| {
                node.role == UxNodeRole::RadialSector
                    && matches!(
                        &node.domain,
                        UxDomainIdentity::RadialSector {
                            action_id,
                            enabled,
                            tier,
                            ..
                        } if action_id == "NodeDelete" && *enabled && *tier == 2
                    )
            }),
            "snapshot should include radial sector action metadata"
        );
        assert!(
            snapshot.semantic_nodes.iter().any(|node| {
                node.role == UxNodeRole::RadialTierRing
                    && matches!(
                        &node.domain,
                        UxDomainIdentity::RadialTierRing {
                            tier,
                            visible_count,
                            ..
                        } if *tier == 1 && *visible_count == 4
                    )
            }),
            "snapshot should include explicit tier-1 ring container metadata"
        );
        assert!(
            snapshot.semantic_nodes.iter().any(|node| {
                node.role == UxNodeRole::RadialSummary
                    && matches!(
                        &node.domain,
                        UxDomainIdentity::RadialSummary {
                            label_pre_collisions,
                            label_post_collisions,
                            ..
                        } if *label_pre_collisions == 2 && *label_post_collisions == 0
                    )
            }),
            "snapshot should include radial overflow/readability summary metadata"
        );

        clear_semantic_snapshot();
    }

    #[test]
    fn snapshot_projects_lens_scope_navigator_and_route_open_boundary_nodes() {
        let mut harness = TestRegistry::new();
        let node = harness.add_node("https://ux-tree-route-open.example");
        harness.open_node_tab(node);

        let view_id = GraphViewId::default();
        harness.app.ensure_graph_view_registered(view_id);
        if let Some(view) = harness.app.workspace.graph_runtime.views.get_mut(&view_id) {
            view.lens.name = "Research Lens".to_string();
            view.lens.layout = crate::registries::atomic::lens::LayoutMode::Free;
            view.lens.filters_legacy = vec!["tag:#clip".to_string()];
        }
        harness.app.workspace.graph_runtime.focused_view = Some(view_id);

        harness.app.set_navigator_host_scope(
            SurfaceHostId::Navigator(crate::app::workbench_layout_policy::NavigatorHostId::Right),
            crate::app::workbench_layout_policy::NavigatorHostScope::WorkbenchOnly,
        );
        harness.app.set_navigator_containment_relation_source(
            NavigatorContainmentRelationSource::SavedViewCollections,
        );
        let row_key = format!("view:{}", view_id.as_uuid());
        harness.app.set_navigator_selected_rows([row_key]);

        harness.app.set_pending_node_context_target(Some(node));
        harness
            .app
            .request_open_node_tile_mode(node, PendingTileOpenMode::SplitHorizontal);
        harness.app.request_open_connected_from(
            node,
            PendingTileOpenMode::Tab,
            PendingConnectedOpenScope::Neighbors,
        );

        let snapshot = build_snapshot(&harness.tiles_tree, &harness.app, 9);

        assert!(
            snapshot.semantic_nodes.iter().any(|entry| {
                entry.role == UxNodeRole::GraphViewLensScope
                    && matches!(
                        &entry.domain,
                        UxDomainIdentity::GraphViewLensScope {
                            graph_view_id,
                            lens_name,
                            filter_count,
                            focused_view,
                            ..
                        } if *graph_view_id == view_id
                            && lens_name == "Research Lens"
                            && *filter_count == 1
                            && *focused_view
                    )
            }),
            "snapshot should include graph view lens/scope metadata"
        );
        assert!(
            snapshot.semantic_nodes.iter().any(|entry| {
                entry.role == UxNodeRole::NavigatorProjection
                    && matches!(
                        &entry.domain,
                        UxDomainIdentity::NavigatorProjection {
                            host,
                            anchor_edge,
                            form_factor,
                            scope,
                            containment_relation_source,
                            selected_count,
                            row_count,
                            ..
                        } if *host == SurfaceHostId::Navigator(crate::app::workbench_layout_policy::NavigatorHostId::Right)
                            && *anchor_edge == AnchorEdge::Right
                            && form_factor == "sidebar"
                            && scope == "workbench"
                            && containment_relation_source == "SavedViewCollections"
                            && *selected_count == 1
                            && *row_count >= 1
                    )
            }),
            "snapshot should include navigator projection metadata"
        );
        assert!(
            snapshot.semantic_nodes.iter().any(|entry| {
                entry.role == UxNodeRole::RouteOpenBoundary
                    && matches!(
                        &entry.domain,
                        UxDomainIdentity::RouteOpenBoundary {
                            pending_node_context_target,
                            pending_open_node,
                            pending_open_connected,
                        } if *pending_node_context_target == Some(node)
                            && pending_open_node
                                .as_ref()
                                .is_some_and(|(key, mode)| *key == node && mode == "split-horizontal")
                            && pending_open_connected
                                .as_ref()
                                .is_some_and(|(source, mode, scope)| {
                                    *source == node && mode == "tab" && scope == "neighbors"
                                })
                    )
            }),
            "snapshot should include route/open boundary pending intent metadata"
        );
    }

    #[test]
    fn snapshot_projects_multiple_navigator_hosts_from_profile_constraints() {
        let mut harness = TestRegistry::new();
        let view_id = GraphViewId::default();
        harness.app.ensure_graph_view_registered(view_id);
        harness.app.set_workbench_layout_constraint(
            SurfaceHostId::Navigator(crate::app::workbench_layout_policy::NavigatorHostId::Top),
            WorkbenchLayoutConstraint::AnchoredSplit {
                surface_host: SurfaceHostId::Navigator(
                    crate::app::workbench_layout_policy::NavigatorHostId::Top,
                ),
                anchor_edge: AnchorEdge::Top,
                anchor_size_fraction: 0.15,
                cross_axis_margin_start_px: 12.0,
                cross_axis_margin_end_px: 18.0,
                resizable: true,
            },
        );
        harness.app.set_workbench_layout_constraint(
            SurfaceHostId::Navigator(crate::app::workbench_layout_policy::NavigatorHostId::Bottom),
            WorkbenchLayoutConstraint::AnchoredSplit {
                surface_host: SurfaceHostId::Navigator(
                    crate::app::workbench_layout_policy::NavigatorHostId::Bottom,
                ),
                anchor_edge: AnchorEdge::Bottom,
                anchor_size_fraction: 0.14,
                cross_axis_margin_start_px: 0.0,
                cross_axis_margin_end_px: 0.0,
                resizable: false,
            },
        );

        let snapshot = build_snapshot(&harness.tiles_tree, &harness.app, 11);

        let projected_hosts = snapshot
            .semantic_nodes
            .iter()
            .filter_map(|entry| match &entry.domain {
                UxDomainIdentity::NavigatorProjection { host, .. } => Some(host.clone()),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert!(projected_hosts.contains(&SurfaceHostId::Navigator(
            crate::app::workbench_layout_policy::NavigatorHostId::Top,
        )));
        assert!(projected_hosts.contains(&SurfaceHostId::Navigator(
            crate::app::workbench_layout_policy::NavigatorHostId::Bottom,
        )));
    }
}
