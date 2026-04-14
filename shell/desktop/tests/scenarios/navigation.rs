use super::super::harness::TestRegistry;
use crate::app::GraphIntent;
use crate::graph::NavigationTrigger;

#[test]
fn webview_url_changed_updates_existing_mapping() {
    let mut harness = TestRegistry::new();
    let key = harness.add_node("https://before.example");
    let webview_id = harness.map_test_webview_with_id(key);

    harness
        .app
        .apply_reducer_intents([GraphIntent::WebViewUrlChanged {
            webview_id,
            new_url: "https://after.example".to_string(),
        }]);

    let node = harness
        .app
        .workspace
        .domain
        .graph
        .get_node(key)
        .expect("mapped node should exist");
    assert_eq!(node.url(), "https://after.example");
    assert_eq!(harness.app.get_node_for_webview(webview_id), Some(key));
}

#[test]
fn webview_url_changed_appends_traversal_between_known_nodes_without_self_loop() {
    let mut harness = TestRegistry::new();
    let from = harness.add_node("https://a.example");
    let to = harness.add_node("https://b.example");
    let webview_id = harness.map_test_webview_with_id(from);

    harness
        .app
        .apply_reducer_intents([GraphIntent::WebViewUrlChanged {
            webview_id,
            new_url: "https://b.example".to_string(),
        }]);

    let edge_key = harness
        .app
        .workspace
        .domain
        .graph
        .find_edge_key(from, to)
        .expect("history traversal edge from source to destination should exist");
    let edge = harness
        .app
        .workspace
        .domain
        .graph
        .get_edge(edge_key)
        .expect("edge payload should exist");
    assert_eq!(edge.traversals().len(), 1);
    assert_eq!(edge.traversals()[0].trigger, NavigationTrigger::Unknown);
    assert!(
        harness
            .app
            .workspace
            .domain
            .graph
            .find_edge_key(to, to)
            .is_none(),
        "prior URL should be captured before node URL mutation to avoid self-loop"
    );
}

#[test]
fn webview_history_changed_clamps_index_to_entry_bounds() {
    let mut harness = TestRegistry::new();
    let key = harness.add_node("https://a.example");
    let webview_id = harness.map_test_webview_with_id(key);

    harness
        .app
        .apply_reducer_intents([GraphIntent::WebViewHistoryChanged {
            webview_id,
            entries: vec![
                "https://a.example".to_string(),
                "https://b.example".to_string(),
            ],
            current: 99,
        }]);

    let node = harness
        .app
        .workspace
        .domain
        .graph
        .get_node(key)
        .expect("mapped node should exist");
    assert_eq!(node.history_entries.len(), 2);
    assert_eq!(node.history_index, 1);
}

#[test]
fn webview_history_changed_adds_back_then_forward_traversals_with_repeat_counts() {
    let mut harness = TestRegistry::new();
    let a = harness.add_node("https://a.example");
    let b = harness.add_node("https://b.example");
    let webview_id = harness.map_test_webview_with_id(b);

    {
        let node = harness
            .app
            .workspace
            .domain
            .graph
            .get_node_mut(b)
            .expect("destination node should exist");
        node.history_entries = vec![
            "https://a.example".to_string(),
            "https://b.example".to_string(),
        ];
        node.history_index = 1;
    }

    harness
        .app
        .apply_reducer_intents([GraphIntent::WebViewHistoryChanged {
            webview_id,
            entries: vec![
                "https://a.example".to_string(),
                "https://b.example".to_string(),
            ],
            current: 0,
        }]);

    harness
        .app
        .apply_reducer_intents([GraphIntent::WebViewHistoryChanged {
            webview_id,
            entries: vec![
                "https://a.example".to_string(),
                "https://b.example".to_string(),
            ],
            current: 1,
        }]);

    harness
        .app
        .apply_reducer_intents([GraphIntent::WebViewHistoryChanged {
            webview_id,
            entries: vec![
                "https://a.example".to_string(),
                "https://b.example".to_string(),
            ],
            current: 0,
        }]);

    let back_edge_key = harness
        .app
        .workspace
        .domain
        .graph
        .find_edge_key(b, a)
        .expect("back traversal edge should exist");
    let back_edge = harness
        .app
        .workspace
        .domain
        .graph
        .get_edge(back_edge_key)
        .expect("back edge payload should exist");
    assert!(
        harness
            .app
            .workspace
            .domain
            .graph
            .get_edge(back_edge_key)
            .is_some_and(|payload| {
                payload.has_relation(crate::graph::RelationSelector::Family(
                    crate::graph::EdgeFamily::Traversal,
                ))
            })
    );
    assert_eq!(back_edge.traversals().len(), 2);
    assert_eq!(back_edge.traversals()[0].trigger, NavigationTrigger::Back);
    assert_eq!(back_edge.traversals()[1].trigger, NavigationTrigger::Back);

    let forward_edge_key = harness
        .app
        .workspace
        .domain
        .graph
        .find_edge_key(a, b)
        .expect("forward traversal edge should exist");
    let forward_edge = harness
        .app
        .workspace
        .domain
        .graph
        .get_edge(forward_edge_key)
        .expect("forward edge payload should exist");
    assert!(
        harness
            .app
            .workspace
            .domain
            .graph
            .get_edge(forward_edge_key)
            .is_some_and(|payload| {
                payload.has_relation(crate::graph::RelationSelector::Family(
                    crate::graph::EdgeFamily::Traversal,
                ))
            })
    );
    assert_eq!(forward_edge.traversals().len(), 1);
    assert_eq!(
        forward_edge.traversals()[0].trigger,
        NavigationTrigger::Forward
    );
}

#[test]
fn history_callback_is_authoritative_when_url_callback_stays_on_latest_entry() {
    let mut harness = TestRegistry::new();
    let step1 = harness.add_node("https://site.example/?step=1");
    let step2 = harness.add_node("https://site.example/?step=2");
    let webview_id = harness.map_test_webview_with_id(step2);

    {
        let node = harness
            .app
            .workspace
            .domain
            .graph
            .get_node_mut(step2)
            .expect("step2 node should exist");
        node.history_entries = vec![
            "https://site.example/?step=0".to_string(),
            "https://site.example/?step=1".to_string(),
            "https://site.example/?step=2".to_string(),
        ];
        node.history_index = 2;
    }

    harness.app.apply_reducer_intents([
        GraphIntent::WebViewUrlChanged {
            webview_id,
            new_url: "https://site.example/?step=2".to_string(),
        },
        GraphIntent::WebViewHistoryChanged {
            webview_id,
            entries: vec![
                "https://site.example/?step=0".to_string(),
                "https://site.example/?step=1".to_string(),
                "https://site.example/?step=2".to_string(),
            ],
            current: 1,
        },
    ]);

    let node = harness
        .app
        .workspace
        .domain
        .graph
        .get_node(step2)
        .expect("step2 node should exist");
    assert_eq!(node.history_index, 1);

    let has_history_edge = harness
        .app
        .workspace
        .domain
        .graph
        .find_edge_key(step2, step1)
        .and_then(|edge_key| harness.app.workspace.domain.graph.get_edge(edge_key))
        .is_some_and(|payload| {
            payload.has_relation(crate::graph::RelationSelector::Family(
                crate::graph::EdgeFamily::Traversal,
            ))
        });
    assert!(
        has_history_edge,
        "history callback should produce back traversal even when URL callback stays on latest"
    );
}

