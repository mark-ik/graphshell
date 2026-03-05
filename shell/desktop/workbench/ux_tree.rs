/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};

use egui_tiles::{Container, Tile, TileId, Tree};

use crate::app::{GraphBrowserApp, GraphViewId};
use crate::graph::NodeKey;

use super::pane_model::TileRenderMode;
use super::tile_kind::TileKind;

pub(crate) const UX_TREE_SEMANTIC_SCHEMA_VERSION: u32 = 1;
pub(crate) const UX_TREE_PRESENTATION_SCHEMA_VERSION: u32 = 1;
pub(crate) const UX_TREE_TRACE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UxNodeRole {
    Workbench,
    SplitContainer,
    TabContainer,
    GraphSurface,
    GraphNode,
    NodePane,
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct UxNodeState {
    pub(crate) focused: bool,
    pub(crate) selected: bool,
    pub(crate) blocked: bool,
    pub(crate) degraded: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UxSemanticNode {
    pub(crate) ux_node_id: String,
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
    let active: HashSet<TileId> = tiles_tree.active_tiles().into_iter().collect();

    let mut semantic_nodes = vec![UxSemanticNode {
        ux_node_id: "uxnode://workbench/root".to_string(),
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
        ux_node_id: "uxnode://workbench/root".to_string(),
        bounds: None,
        render_mode: None,
        z_pass: "workbench:root",
        style_flags: vec!["spine:egui_tiles"],
        transient_flags: Vec::new(),
    }];

    let mut trace_nodes = vec![UxTraceNode {
        ux_node_id: "uxnode://workbench/root".to_string(),
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
            &mut semantic_nodes,
            &mut presentation_nodes,
            &mut trace_nodes,
        );
    }

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

fn push_nodes(
    tiles_tree: &Tree<TileKind>,
    graph_app: &GraphBrowserApp,
    tile_id: TileId,
    active: &HashSet<TileId>,
    semantic_nodes: &mut Vec<UxSemanticNode>,
    presentation_nodes: &mut Vec<UxPresentationNode>,
    trace_nodes: &mut Vec<UxTraceNode>,
) {
    let Some(tile) = tiles_tree.tiles.get(tile_id) else {
        return;
    };

    let ux_node_id = ux_node_id_for_tile(tile_id, tile);
    let focused = active.contains(&tile_id);

    match tile {
        Tile::Pane(TileKind::Graph(view_id)) => {
            let focused_selection = graph_app.focused_selection();
            semantic_nodes.push(UxSemanticNode {
                ux_node_id: ux_node_id.clone(),
                role: UxNodeRole::GraphSurface,
                label: format!("Graph View {:?}", view_id),
                state: UxNodeState {
                    focused,
                    selected: false,
                    blocked: false,
                    degraded: false,
                },
                allowed_actions: vec![UxAction::Focus, UxAction::Navigate],
                domain: UxDomainIdentity::GraphView {
                    graph_view_id: *view_id,
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
                ux_node_id,
                event_route: "graph.input_route",
                backend_path: "egui_graphs",
                diagnostics_counter: graph_app.workspace.graph.node_count() as u64,
            });

            for (node_key, node) in graph_app.workspace.graph.nodes() {
                let graph_node_ux_id =
                    format!("uxnode://workbench/graph/{view_id:?}/node/{}", node.id);
                let selected = graph_app.workspace.selected_nodes.contains(&node_key);
                let focused_graph_node = focused_selection.primary() == Some(node_key);
                let blocked = graph_app.runtime_block_state_for_node(node_key).is_some();
                let degraded = graph_app.runtime_crash_state_for_node(node_key).is_some();

                semantic_nodes.push(UxSemanticNode {
                    ux_node_id: graph_node_ux_id.clone(),
                    role: UxNodeRole::GraphNode,
                    label: if node.title.is_empty() {
                        node.url.clone()
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
            let blocked = graph_app.runtime_block_state_for_node(state.node).is_some();
            let degraded = matches!(state.render_mode, TileRenderMode::Placeholder);
            let selected = graph_app.workspace.selected_nodes.contains(&state.node);

            semantic_nodes.push(UxSemanticNode {
                ux_node_id: ux_node_id.clone(),
                role: UxNodeRole::NodePane,
                label: format!("Node Pane {:?}", state.node),
                state: UxNodeState {
                    focused,
                    selected,
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
            presentation_nodes.push(UxPresentationNode {
                ux_node_id: ux_node_id.clone(),
                bounds: None,
                render_mode: Some(state.render_mode),
                z_pass: "workbench.content",
                style_flags: vec!["surface:node"],
                transient_flags: Vec::new(),
            });
            trace_nodes.push(UxTraceNode {
                ux_node_id,
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
                role: UxNodeRole::ToolPane,
                label: format!("Tool Pane {tool_kind}"),
                state: UxNodeState {
                    focused,
                    selected: false,
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
                style_flags: vec!["surface:tool"],
                transient_flags: Vec::new(),
            });
            trace_nodes.push(UxTraceNode {
                ux_node_id,
                event_route: "workbench.tool_route",
                backend_path: "egui",
                diagnostics_counter: 0,
            });
        }
        Tile::Container(Container::Tabs(tabs)) => {
            semantic_nodes.push(UxSemanticNode {
                ux_node_id: ux_node_id.clone(),
                role: UxNodeRole::TabContainer,
                label: format!("Tabs ({})", tabs.children.len()),
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
                ux_node_id,
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
                    semantic_nodes,
                    presentation_nodes,
                    trace_nodes,
                );
            }
        }
        Tile::Container(Container::Linear(linear)) => {
            semantic_nodes.push(UxSemanticNode {
                ux_node_id: ux_node_id.clone(),
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
                ux_node_id,
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
                ux_node_id,
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
        Tile::Pane(TileKind::Graph(view_id)) => {
            format!("uxnode://workbench/tile/{tile_id:?}/graph/{view_id:?}")
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

#[cfg(test)]
pub(crate) fn snapshot_json_for_tests(snapshot: &UxTreeSnapshot) -> serde_json::Value {
    serde_json::json!({
        "semantic_version": snapshot.semantic_version,
        "presentation_version": snapshot.presentation_version,
        "trace_version": snapshot.trace_version,
        "semantic_nodes": snapshot.semantic_nodes.iter().map(|node| serde_json::json!({
            "ux_node_id": node.ux_node_id,
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
        harness.app.workspace.selected_nodes.select(node_b, false);

        let snapshot = build_snapshot(&harness.tiles_tree, &harness.app, 14);
        let graph_nodes = snapshot
            .semantic_nodes
            .iter()
            .filter(|entry| entry.role == UxNodeRole::GraphNode)
            .collect::<Vec<_>>();

        assert_eq!(graph_nodes.len(), 2);
        assert!(graph_nodes.iter().any(|entry| entry.state.selected));
    }
}
