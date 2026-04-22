use super::super::harness::TestRegistry;
use crate::shell::desktop::workbench::ux_tree::{self, UxNodeRole};

#[test]
fn uxtree_graph_semantic_parity_matches_graph_model_count() {
    let mut harness = TestRegistry::new();
    let node_a = harness.add_node("https://graph-parity-a.example");
    let node_b = harness.add_node("https://graph-parity-b.example");
    harness.open_node_tab(node_a);
    harness.open_node_tab(node_b);
    harness.app.select_node(node_b, false);

    let snapshot = ux_tree::build_snapshot(&harness.tiles_tree, &harness.app, None, 11);

    let graph_semantic_nodes = snapshot
        .semantic_nodes
        .iter()
        .filter(|node| node.role == UxNodeRole::GraphNode)
        .count();

    assert_eq!(
        graph_semantic_nodes,
        harness.app.workspace.domain.graph.node_count(),
        "graph semantic layer should project all graph nodes"
    );

    let selected_graph_semantic_nodes = snapshot
        .semantic_nodes
        .iter()
        .filter(|node| node.role == UxNodeRole::GraphNode && node.state.selected)
        .count();

    assert_eq!(
        selected_graph_semantic_nodes, 1,
        "selected graph node should be marked selected in semantic layer"
    );
}
