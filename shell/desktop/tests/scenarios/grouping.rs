use super::super::harness::TestRegistry;
use crate::app::{GraphIntent, WorkbenchIntent};
use crate::graph::NodeLifecycle;
use crate::shell::desktop::runtime::registries::{self as registries};
use crate::shell::desktop::runtime::registries::action::{
    ACTION_GRAPH_NODE_REMOVE_FROM_GRAPHLET, ACTION_GRAPH_SELECTION_WARM_SELECT, ActionOutcome,
    ActionPayload, ActionRegistry,
};
use crate::shell::desktop::workbench::tile_kind::TileKind;
use crate::shell::desktop::workbench::tile_view_ops;
use egui_tiles::Tile;

// ── helpers ──────────────────────────────────────────────────────────────────

/// Find the `PaneId` for the node tile carrying `node_key` in the tree.
fn pane_id_for_node(
    harness: &TestRegistry,
    node_key: crate::graph::NodeKey,
) -> Option<crate::shell::desktop::workbench::pane_model::PaneId> {
    harness.tiles_tree.tiles.iter().find_map(|(_, tile)| match tile {
        Tile::Pane(TileKind::Node(state)) if state.node == node_key => Some(state.pane_id),
        _ => None,
    })
}

/// Count how many distinct `Container::Tabs` parents the given nodes' tiles have.
fn tab_container_count_for_nodes(
    harness: &TestRegistry,
    nodes: &[crate::graph::NodeKey],
) -> usize {
    use std::collections::HashSet;
    let mut containers: HashSet<egui_tiles::TileId> = HashSet::new();
    for &node in nodes {
        let tile_id = harness.tiles_tree.tiles.iter().find_map(|(tid, tile)| match tile {
            Tile::Pane(TileKind::Node(s)) if s.node == node => Some(*tid),
            _ => None,
        });
        if let Some(tid) = tile_id {
            if let Some(parent) = harness.tiles_tree.tiles.parent_of(tid) {
                containers.insert(parent);
            }
        }
    }
    containers.len()
}

/// Return true if a tile for `node_key` exists in the tree.
fn has_tile(harness: &TestRegistry, node_key: crate::graph::NodeKey) -> bool {
    harness.tiles_tree.tiles.iter().any(|(_, tile)| match tile {
        Tile::Pane(TileKind::Node(s)) => s.node == node_key,
        _ => false,
    })
}

// ── existing test ─────────────────────────────────────────────────────────────

#[test]
fn create_user_grouped_edge_from_primary_selection_creates_grouped_edge() {
    let mut harness = TestRegistry::new();
    let source = harness.add_node("https://a.com");
    let destination = harness.add_node("https://b.com");

    harness.app.select_node(destination, false);
    harness.app.select_node(source, true);

    harness
        .app
        .apply_reducer_intents([GraphIntent::CreateUserGroupedEdgeFromPrimarySelection]);

    let grouped_edge_count = harness
        .app
        .workspace
        .domain
        .graph
        .find_edge_key(source, destination)
        .and_then(|edge_key| harness.app.workspace.domain.graph.get_edge(edge_key))
        .map(|payload| {
            usize::from(payload.has_relation(crate::graph::RelationSelector::Semantic(
                crate::graph::SemanticSubKind::UserGrouped,
            )))
        })
        .unwrap_or(0);

    assert_eq!(
        grouped_edge_count, 1,
        "grouping action should create a single UserGrouped edge"
    );
}

// ── §12 acceptance criteria tests ─────────────────────────────────────────────

/// DismissTile closes the tile and demotes the node to Cold, but leaves all
/// graph edges intact (§5.3, acceptance criterion row 7).
#[test]
fn dismiss_tile_demotes_lifecycle_and_preserves_edges() {
    let mut harness = TestRegistry::new();
    let a = harness.add_node("https://a.test/");
    let b = harness.add_node("https://b.test/");

    harness.app.apply_reducer_intents([GraphIntent::CreateUserGroupedEdge {
        from: a,
        to: b,
        label: None,
    }]);

    tile_view_ops::open_or_focus_node_pane(&mut harness.tiles_tree, &harness.app, a);
    tile_view_ops::open_or_focus_node_pane(&mut harness.tiles_tree, &harness.app, b);

    assert!(has_tile(&harness, a), "a should have a tile before dismiss");

    let pane_id = pane_id_for_node(&harness, a).expect("a tile should have a pane id");
    registries::dispatch_workbench_surface_intent(
        &mut harness.app,
        &mut harness.tiles_tree,
        WorkbenchIntent::DismissTile { pane: pane_id },
    );

    assert!(
        !has_tile(&harness, a),
        "a's tile should be removed after DismissTile"
    );
    assert_eq!(
        harness.app.domain_graph().get_node(a).map(|n| n.lifecycle),
        Some(NodeLifecycle::Cold),
        "a's lifecycle should be Cold after DismissTile"
    );

    let edge_preserved = harness.app.domain_graph().edges().any(|e| {
        ((e.from == a && e.to == b) || (e.from == b && e.to == a))
            && harness
                .app
                .domain_graph()
                .find_edge_key(e.from, e.to)
                .and_then(|edge_key| harness.app.domain_graph().get_edge(edge_key))
                .is_some_and(|payload| {
                    payload.has_relation(crate::graph::RelationSelector::Semantic(
                        crate::graph::SemanticSubKind::UserGrouped,
                    ))
                })
    });
    assert!(
        edge_preserved,
        "UserGrouped edge must survive DismissTile — dismiss is not delete"
    );
}

/// After DismissTile, the node remains a durable graphlet peer of its partner
/// (acceptance criterion: "dismissed node remains in graphlet").
#[test]
fn dismissed_node_remains_in_durable_graphlet() {
    let mut harness = TestRegistry::new();
    let a = harness.add_node("https://a.test/");
    let b = harness.add_node("https://b.test/");

    harness.app.apply_reducer_intents([GraphIntent::CreateUserGroupedEdge {
        from: a,
        to: b,
        label: None,
    }]);

    tile_view_ops::open_or_focus_node_pane(&mut harness.tiles_tree, &harness.app, a);

    let pane_id = pane_id_for_node(&harness, a).expect("pane id");
    registries::dispatch_workbench_surface_intent(
        &mut harness.app,
        &mut harness.tiles_tree,
        WorkbenchIntent::DismissTile { pane: pane_id },
    );

    let peers_of_b = harness.app.durable_graphlet_peers(b);
    assert!(
        peers_of_b.contains(&a),
        "a must remain a durable graphlet peer of b after dismiss"
    );
}

/// Opening a node whose durable graphlet peer already has a warm tile routes
/// the new tile into the peer's tab container (§5.1, acceptance criteria rows 5–6).
#[test]
fn open_node_with_graphlet_routing_joins_warm_peer_tab_container() {
    let mut harness = TestRegistry::new();
    let a = harness.add_node("https://a.test/");
    let b = harness.add_node("https://b.test/");

    harness.app.apply_reducer_intents([GraphIntent::CreateUserGroupedEdge {
        from: a,
        to: b,
        label: None,
    }]);

    // Open a first — creates a tab container for it.
    tile_view_ops::open_or_focus_node_pane(&mut harness.tiles_tree, &harness.app, a);
    assert!(has_tile(&harness, a));

    // Open b with graphlet routing — should join a's tab container.
    tile_view_ops::open_node_with_graphlet_routing(&mut harness.tiles_tree, &harness.app, b);
    assert!(has_tile(&harness, b));

    assert_eq!(
        tab_container_count_for_nodes(&harness, &[a, b]),
        1,
        "a and b should be in the same tab container after graphlet-routed open"
    );
}

/// Activating a cold node (lifecycle Cold) and re-opening it brings it back
/// into the graphlet's tab group without touching graph edges (§5.5).
#[test]
fn cold_node_reactivated_joins_existing_tab_group() {
    let mut harness = TestRegistry::new();
    let a = harness.add_node("https://a.test/");
    let b = harness.add_node("https://b.test/");

    harness.app.apply_reducer_intents([GraphIntent::CreateUserGroupedEdge {
        from: a,
        to: b,
        label: None,
    }]);

    tile_view_ops::open_or_focus_node_pane(&mut harness.tiles_tree, &harness.app, a);
    tile_view_ops::open_or_focus_node_pane(&mut harness.tiles_tree, &harness.app, b);

    // Dismiss b — it becomes Cold, tile removed.
    let pane_b = pane_id_for_node(&harness, b).expect("pane id");
    registries::dispatch_workbench_surface_intent(
        &mut harness.app,
        &mut harness.tiles_tree,
        WorkbenchIntent::DismissTile { pane: pane_b },
    );
    assert!(!has_tile(&harness, b), "b should have no tile after dismiss");

    // Re-open b via graphlet routing — should rejoin a's container.
    tile_view_ops::open_node_with_graphlet_routing(&mut harness.tiles_tree, &harness.app, b);
    assert!(has_tile(&harness, b), "b should have a tile after reactivation");

    assert_eq!(
        tab_container_count_for_nodes(&harness, &[a, b]),
        1,
        "b should rejoin a's tab container on reactivation"
    );
}

/// ReconcileGraphletTiles merges warm tiles from different containers into one
/// after a UserGrouped edge is created between two already-warm nodes (§8).
#[test]
fn reconcile_graphlet_merges_tiles_from_different_tab_containers() {
    let mut harness = TestRegistry::new();
    let a = harness.add_node("https://a.test/");
    let b = harness.add_node("https://b.test/");

    // Build a tree where a and b are in separate tab containers under a split.
    let a_pane = harness.tiles_tree.tiles.insert_pane(TileKind::Node(a.into()));
    let a_tabs = harness.tiles_tree.tiles.insert_tab_tile(vec![a_pane]);
    let b_pane = harness.tiles_tree.tiles.insert_pane(TileKind::Node(b.into()));
    let b_tabs = harness.tiles_tree.tiles.insert_tab_tile(vec![b_pane]);
    let split = harness
        .tiles_tree
        .tiles
        .insert_horizontal_tile(vec![a_tabs, b_tabs]);
    harness.tiles_tree.root = Some(split);

    assert_eq!(
        tab_container_count_for_nodes(&harness, &[a, b]),
        2,
        "precondition: a and b start in separate containers"
    );

    // Create the edge.
    harness.app.apply_reducer_intents([GraphIntent::CreateUserGroupedEdge {
        from: a,
        to: b,
        label: None,
    }]);

    // Drain the queued ReconcileGraphletTiles intent (enqueued by intent_phases).
    let pending = harness.app.take_pending_workbench_intents();
    assert!(
        pending.iter().any(|i| matches!(i, WorkbenchIntent::ReconcileGraphletTiles { .. })),
        "CreateUserGroupedEdge should enqueue a ReconcileGraphletTiles intent"
    );
    for intent in pending {
        registries::dispatch_workbench_surface_intent(
            &mut harness.app,
            &mut harness.tiles_tree,
            intent,
        );
    }

    assert!(has_tile(&harness, a), "a tile must survive reconcile");
    assert!(has_tile(&harness, b), "b tile must survive reconcile");
    assert_eq!(
        tab_container_count_for_nodes(&harness, &[a, b]),
        1,
        "after reconcile, a and b should be in the same tab container"
    );
}

/// RemoveFromGraphlet retracts only UserGrouped / FrameMember edges; Hyperlink
/// edges are left intact (§5.4, acceptance criterion row 9).
#[test]
fn remove_from_graphlet_action_retracts_durable_edges_only() {
    let mut harness = TestRegistry::new();
    let a = harness.add_node("https://a.test/");
    let b = harness.add_node("https://b.test/");

    // Durable edge to retract.
    harness.app.apply_reducer_intents([GraphIntent::CreateUserGroupedEdge {
        from: a,
        to: b,
        label: None,
    }]);
    // Circumstantial edge that must NOT be retracted.
    let _ = harness.app.assert_relation_and_sync(
        a,
        b,
        crate::graph::EdgeAssertion::Semantic {
            sub_kind: crate::graph::SemanticSubKind::Hyperlink,
            label: None,
            decay_progress: None,
        },
    );

    harness.app.select_node(a, false);

    let action_registry = ActionRegistry::default();
    let outcome = action_registry.execute(
        ACTION_GRAPH_NODE_REMOVE_FROM_GRAPHLET,
        &harness.app,
        ActionPayload::GraphDeselectAll,
    );

    let dispatch = match outcome {
        ActionOutcome::Dispatch(d) => d,
        other => panic!("expected Dispatch, got: {other:?}"),
    };

    // Apply the graph intents from the dispatch.
    harness.app.apply_reducer_intents(dispatch.intents);

    // UserGrouped edge should be gone.
    let has_grouped = harness.app.domain_graph().edges().any(|e| {
        ((e.from == a && e.to == b) || (e.from == b && e.to == a))
            && harness
                .app
                .domain_graph()
                .find_edge_key(e.from, e.to)
                .and_then(|edge_key| harness.app.domain_graph().get_edge(edge_key))
                .is_some_and(|payload| {
                    payload.has_relation(crate::graph::RelationSelector::Semantic(
                        crate::graph::SemanticSubKind::UserGrouped,
                    ))
                })
    });
    assert!(!has_grouped, "UserGrouped edge should be retracted by RemoveFromGraphlet");

    // Hyperlink edge should remain.
    let has_hyperlink = harness.app.domain_graph().edges().any(|e| {
        e.from == a
            && e.to == b
            && harness
                .app
                .domain_graph()
                .find_edge_key(e.from, e.to)
                .and_then(|edge_key| harness.app.domain_graph().get_edge(edge_key))
                .is_some_and(|payload| {
                    payload.has_relation(crate::graph::RelationSelector::Semantic(
                        crate::graph::SemanticSubKind::Hyperlink,
                    ))
                })
    });
    assert!(
        has_hyperlink,
        "Hyperlink edge must not be retracted by RemoveFromGraphlet"
    );
}

/// The warm-select action dispatches OpenNodeInPane for each cold selected node.
/// Cold nodes are not opened; warm nodes are not re-opened (§5.5, §12 row 3).
#[test]
fn warm_select_action_dispatches_open_intent_for_cold_selected_nodes() {
    let mut harness = TestRegistry::new();
    let a = harness.add_node("https://a.test/");
    let b = harness.add_node("https://b.test/");
    let c = harness.add_node("https://c.test/");

    harness.app.apply_reducer_intents([GraphIntent::CreateUserGroupedEdge {
        from: a,
        to: b,
        label: None,
    }]);

    // Open a — warm. b and c remain Cold.
    tile_view_ops::open_or_focus_node_pane(&mut harness.tiles_tree, &harness.app, a);

    // Select cold b and cold c (a is warm but not selected for this test).
    harness.app.select_node(b, false);
    harness.app.select_node(c, true);

    let action_registry = ActionRegistry::default();
    let outcome = action_registry.execute(
        ACTION_GRAPH_SELECTION_WARM_SELECT,
        &harness.app,
        ActionPayload::GraphDeselectAll,
    );

    let dispatch = match outcome {
        ActionOutcome::Dispatch(d) => d,
        other => panic!("expected Dispatch, got: {other:?}"),
    };

    let open_nodes: Vec<_> = dispatch
        .workbench_intents
        .iter()
        .filter_map(|i| match i {
            WorkbenchIntent::OpenNodeInPane { node, .. } => Some(*node),
            _ => None,
        })
        .collect();

    assert!(
        open_nodes.contains(&b),
        "warm-select should include cold node b"
    );
    assert!(
        open_nodes.contains(&c),
        "warm-select should include cold node c"
    );
    assert_eq!(open_nodes.len(), 2, "warm-select should only target cold nodes");
}

/// Opening a new tile as a tab while a graphlet-member node is the primary
/// selection creates a durable UserGrouped edge between the new node and that
/// peer (Phase 5, §12).
#[test]
fn new_tile_as_tab_creates_durable_graphlet_edge() {
    use crate::app::PendingTileOpenMode;

    let mut harness = TestRegistry::new();
    let a = harness.add_node("https://a.test/");
    let b = harness.add_node("https://b.test/");

    harness.app.apply_reducer_intents([GraphIntent::CreateUserGroupedEdge {
        from: a,
        to: b,
        label: None,
    }]);

    // Select a — the primary selection determines the graphlet context.
    harness.app.select_node(a, false);

    // Create a new node opened as a tab into the graphlet context.
    harness
        .app
        .apply_reducer_intents([GraphIntent::CreateNodeNearCenterAndOpen {
            mode: PendingTileOpenMode::Tab,
        }]);

    // Find the newly created node (not a or b).
    let new_node = harness
        .app
        .domain_graph()
        .nodes()
        .map(|(k, _)| k)
        .find(|&k| k != a && k != b)
        .expect("a new node should have been created");

    let has_edge = harness.app.domain_graph().edges().any(|e| {
        ((e.from == new_node && e.to == a) || (e.from == a && e.to == new_node))
            && harness
                .app
                .domain_graph()
                .find_edge_key(e.from, e.to)
                .and_then(|edge_key| harness.app.domain_graph().get_edge(edge_key))
                .is_some_and(|payload| {
                    payload.has_relation(crate::graph::RelationSelector::Semantic(
                        crate::graph::SemanticSubKind::UserGrouped,
                    ))
                })
    });
    assert!(
        has_edge,
        "new node should have a UserGrouped edge to the active graphlet peer"
    );
    assert!(
        harness.app.durable_graphlet_peers(a).contains(&new_node),
        "new node should be a durable graphlet peer of a"
    );
}
