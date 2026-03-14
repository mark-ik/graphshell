use super::super::harness::TestRegistry;
use crate::app::{GraphBrowserApp, GraphIntent};

#[test]
fn set_node_pinned_intent_syncs_pin_tag() {
    let mut harness = TestRegistry::new();
    let node = harness.add_node("https://example.com");

    harness
        .app
        .apply_reducer_intents([GraphIntent::SetNodePinned {
            key: node,
            is_pinned: true,
        }]);
    assert!(
        harness
            .app
            .node_has_canonical_tag(node, GraphBrowserApp::TAG_PIN)
    );

    harness
        .app
        .apply_reducer_intents([GraphIntent::SetNodePinned {
            key: node,
            is_pinned: false,
        }]);
    assert!(
        !harness
            .app
            .node_has_canonical_tag(node, GraphBrowserApp::TAG_PIN)
    );
}

#[test]
fn tag_node_pin_updates_pinned_state() {
    let mut harness = TestRegistry::new();
    let node = harness.add_node("https://example.com");

    harness.app.apply_reducer_intents([GraphIntent::TagNode {
        key: node,
        tag: GraphBrowserApp::TAG_PIN.to_string(),
    }]);
    assert!(
        harness
            .app
            .workspace
            .domain
            .graph
            .get_node(node)
            .is_some_and(|n| n.is_pinned)
    );

    harness.app.apply_reducer_intents([GraphIntent::UntagNode {
        key: node,
        tag: GraphBrowserApp::TAG_PIN.to_string(),
    }]);
    assert!(
        harness
            .app
            .workspace
            .domain
            .graph
            .get_node(node)
            .is_some_and(|n| !n.is_pinned)
    );
}

#[test]
fn tag_node_canonicalizes_valid_knowledge_tags_and_accepts_user_defined_tags() {
    let mut harness = TestRegistry::new();
    let node = harness.add_node("https://example.com");

    harness.app.apply_reducer_intents([GraphIntent::TagNode {
        key: node,
        tag: "519.6".to_string(),
    }]);

    assert!(harness.app.node_has_canonical_tag(node, "udc:519.6"));

    harness.app.apply_reducer_intents([GraphIntent::TagNode {
        key: node,
        tag: "unknown-subject".to_string(),
    }]);

    assert!(harness.app.node_has_canonical_tag(node, "unknown-subject"));
}

#[test]
fn tag_node_lowercases_reserved_hash_tags() {
    let mut harness = TestRegistry::new();
    let node = harness.add_node("https://example.com");

    harness.app.apply_reducer_intents([GraphIntent::TagNode {
        key: node,
        tag: "#PIN".to_string(),
    }]);

    assert!(
        harness
            .app
            .node_has_canonical_tag(node, GraphBrowserApp::TAG_PIN)
    );
}
