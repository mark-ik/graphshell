use super::super::harness::TestRegistry;

#[test]
fn test_capture_undo_checkpoint_pushes_and_clears_redo() {
    let mut harness = TestRegistry::new();
    let _node_a = harness.add_node("https://example.com/a");

    // Capture first checkpoint
    harness.app.capture_undo_checkpoint(None);
    assert_eq!(harness.app.undo_stack_len(), 1);
    assert_eq!(harness.app.redo_stack_len(), 0);

    // Add another node and capture
    let _node_b = harness.add_node("https://example.com/b");
    harness.app.capture_undo_checkpoint(None);
    assert_eq!(harness.app.undo_stack_len(), 2);
    assert_eq!(harness.app.redo_stack_len(), 0);

    // Undo to create redo stack
    harness.app.perform_undo(None);
    assert_eq!(harness.app.undo_stack_len(), 1);
    assert_eq!(harness.app.redo_stack_len(), 1);

    // Capture again should clear redo stack
    let _node_c = harness.add_node("https://example.com/c");
    harness.app.capture_undo_checkpoint(None);
    assert_eq!(harness.app.undo_stack_len(), 2);
    assert_eq!(
        harness.app.redo_stack_len(),
        0,
        "redo stack should be cleared after capture"
    );
}

#[test]
fn test_undo_stack_trimmed_at_max() {
    let mut harness = TestRegistry::new();

    // Capture 129 checkpoints (more than MAX_UNDO_STEPS = 128)
    for i in 0..129 {
        let _node = harness.add_node(&format!("https://example.com/{}", i));
        harness.app.capture_undo_checkpoint(None);
    }

    // Stack should be trimmed to 128
    assert!(
        harness.app.undo_stack_len() <= 128,
        "undo_stack should be trimmed to max 128, got {}",
        harness.app.undo_stack_len()
    );
}

#[test]
fn test_new_action_clears_redo_stack() {
    let mut harness = TestRegistry::new();
    let _node_a = harness.add_node("https://example.com/a");

    // Set up: capture, add node, capture, undo to create redo
    harness.app.capture_undo_checkpoint(None);
    let _node_b = harness.add_node("https://example.com/b");
    harness.app.capture_undo_checkpoint(None);
    harness.app.perform_undo(None);

    assert_eq!(harness.app.redo_stack_len(), 1);

    // New action (adding a node and capturing) should clear redo
    let _node_c = harness.add_node("https://example.com/c");
    harness.app.capture_undo_checkpoint(None);

    assert_eq!(
        harness.app.redo_stack_len(),
        0,
        "redo_stack should be empty after new action"
    );
}

#[test]
fn test_perform_undo_reverts_to_previous_graph() {
    let mut harness = TestRegistry::new();

    // Add first node
    let node_a = harness.add_node("https://example.com/a");
    harness.app.capture_undo_checkpoint(None);

    // Add second node
    let node_b = harness.add_node("https://example.com/b");

    // Graph should have 2 nodes
    assert_eq!(harness.app.workspace.graph.node_count(), 2);
    assert!(harness.app.workspace.graph.get_node(node_a).is_some());
    assert!(harness.app.workspace.graph.get_node(node_b).is_some());

    // Undo to previous state
    let result = harness.app.perform_undo(None);
    assert!(result, "perform_undo should succeed");

    // Graph should only have first node
    assert_eq!(
        harness.app.workspace.graph.node_count(),
        1,
        "after undo, should have 1 node"
    );
    assert!(harness.app.workspace.graph.get_node(node_a).is_some());
    assert!(harness.app.workspace.graph.get_node(node_b).is_none());
}

#[test]
fn test_perform_redo_reapplies_after_undo() {
    let mut harness = TestRegistry::new();

    // Add first node
    let node_a = harness.add_node("https://example.com/a");
    harness.app.capture_undo_checkpoint(None);

    // Add second node
    let node_b = harness.add_node("https://example.com/b");

    // Undo
    harness.app.perform_undo(None);
    assert_eq!(harness.app.workspace.graph.node_count(), 1);

    // Redo should restore second node
    let result = harness.app.perform_redo(None);
    assert!(result, "perform_redo should succeed");

    assert_eq!(
        harness.app.workspace.graph.node_count(),
        2,
        "after redo, should have 2 nodes"
    );
    assert!(harness.app.workspace.graph.get_node(node_a).is_some());
    assert!(harness.app.workspace.graph.get_node(node_b).is_some());
}

#[test]
fn test_undo_returns_false_when_stack_empty() {
    let mut harness = TestRegistry::new();

    // With no captures, undo should fail
    let result = harness.app.perform_undo(None);
    assert!(!result, "perform_undo should return false on empty stack");

    // Add a capture, undo, should succeed
    harness.add_node("https://example.com/a");
    harness.app.capture_undo_checkpoint(None);
    let result = harness.app.perform_undo(None);
    assert!(result, "perform_undo should succeed after capture");

    // Try to undo again, should fail
    let result = harness.app.perform_undo(None);
    assert!(
        !result,
        "perform_undo should return false when undo_stack is empty"
    );
}
