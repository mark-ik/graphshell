use super::super::harness::TestRegistry;
use crate::shell::desktop::workbench::ux_tree;

#[test]
fn ux_tree_diff_gate_policy_matches_contract_defaults() {
    let mut harness = TestRegistry::new();
    let node = harness.add_node("https://scenario-uxtree-diff.example");
    harness.open_node_tab(node);

    let baseline = ux_tree::build_snapshot(&harness.tiles_tree, &harness.app, 9);

    let mut semantic_changed = baseline.clone();
    semantic_changed.semantic_nodes[0].label = "Workbench Contract Shift".to_string();
    let semantic_gate = ux_tree::classify_snapshot_diff_gate(&baseline, &semantic_changed, false);
    assert!(semantic_gate.blocking_failure, "semantic diffs must be blocking");

    let mut presentation_changed = baseline.clone();
    presentation_changed.presentation_nodes[0]
        .transient_flags
        .push("hover:fade");
    let presentation_gate =
        ux_tree::classify_snapshot_diff_gate(&baseline, &presentation_changed, false);
    assert!(
        !presentation_gate.blocking_failure,
        "presentation diffs should remain informational by default"
    );

    let promoted_gate = ux_tree::classify_snapshot_diff_gate(&baseline, &presentation_changed, true);
    assert!(
        promoted_gate.blocking_failure,
        "promoted presentation diffs should become blocking"
    );
}
