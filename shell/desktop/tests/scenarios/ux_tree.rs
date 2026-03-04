use super::super::harness::TestRegistry;
use crate::shell::desktop::workbench::ux_tree;

#[test]
fn uxtree_snapshot_and_probe_are_healthy_for_selected_node_flow() {
    let mut harness = TestRegistry::new();
    let node = harness.add_node("https://scenario-uxtree.example");
    harness.open_node_tab(node);
    harness.app.select_node(node, false);

    let snapshot = ux_tree::build_snapshot(&harness.tiles_tree, &harness.app, 7);
    let snapshot_json = ux_tree::snapshot_json_for_tests(&snapshot);

    assert_eq!(
        snapshot_json.get("semantic_version").and_then(|v| v.as_u64()),
        Some(1),
        "semantic schema version should be present"
    );
    assert_eq!(
        snapshot_json
            .get("presentation_version")
            .and_then(|v| v.as_u64()),
        Some(1),
        "presentation schema version should be present"
    );

    let semantic_nodes = snapshot_json
        .get("semantic_nodes")
        .and_then(|v| v.as_array())
        .expect("uxtree snapshot should expose semantic nodes");
    assert!(
        semantic_nodes
            .iter()
            .any(|node| node.get("domain").and_then(|v| v.as_str()).unwrap_or_default().contains("Node")),
        "expected semantic layer to include node-domain identity"
    );

    let violation = ux_tree::presentation_id_consistency_violation(&snapshot);
    assert!(
        violation.is_none(),
        "healthy selected-node flow should satisfy semantic/presentation consistency invariant"
    );
}
