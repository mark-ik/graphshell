use super::super::harness::TestHarness;
use crate::app::{GraphBrowserApp, GraphIntent};

#[test]
fn set_node_pinned_intent_syncs_pin_tag() {
    let mut harness = TestHarness::new();
    let node = harness.add_node("https://example.com");

    harness.app.apply_intents([GraphIntent::SetNodePinned {
        key: node,
        is_pinned: true,
    }]);
    assert!(
        harness
            .app
            .workspace
            .semantic_tags
            .get(&node)
            .is_some_and(|tags| tags.contains(GraphBrowserApp::TAG_PIN))
    );

    harness.app.apply_intents([GraphIntent::SetNodePinned {
        key: node,
        is_pinned: false,
    }]);
    assert!(
        harness
            .app
            .workspace
            .semantic_tags
            .get(&node)
            .is_none_or(|tags| !tags.contains(GraphBrowserApp::TAG_PIN))
    );
}

#[test]
fn tag_node_pin_updates_pinned_state() {
    let mut harness = TestHarness::new();
    let node = harness.add_node("https://example.com");

    harness.app.apply_intents([GraphIntent::TagNode {
        key: node,
        tag: GraphBrowserApp::TAG_PIN.to_string(),
    }]);
    assert!(
        harness
            .app
            .workspace
            .graph
            .get_node(node)
            .is_some_and(|n| n.is_pinned)
    );

    harness.app.apply_intents([GraphIntent::UntagNode {
        key: node,
        tag: GraphBrowserApp::TAG_PIN.to_string(),
    }]);
    assert!(
        harness
            .app
            .workspace
            .graph
            .get_node(node)
            .is_some_and(|n| !n.is_pinned)
    );
}
