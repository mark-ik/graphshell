use std::collections::HashMap;

use accesskit::{Node, NodeId, Role, Tree, TreeUpdate};
use base::id::{PIPELINE_NAMESPACE, PainterId, PipelineNamespace, TEST_NAMESPACE};
use servo::WebViewId;

fn test_webview_id() -> WebViewId {
    PIPELINE_NAMESPACE.with(|tls| {
        if tls.get().is_none() {
            PipelineNamespace::install(TEST_NAMESPACE);
        }
    });
    WebViewId::new(PainterId::next())
}

#[test]
fn webview_a11y_anchor_id_is_stable_per_webview() {
    let id = test_webview_id();
    let a = super::accessibility::webview_accessibility_anchor_id(id);
    let b = super::accessibility::webview_accessibility_anchor_id(id);
    assert_eq!(a, b);
}

#[test]
fn webview_accessibility_label_prefers_focused_node_label() {
    let webview_id = test_webview_id();
    let mut focused = Node::new(Role::Document);
    focused.set_label("Focused title".to_string());
    let mut other = Node::new(Role::Paragraph);
    other.set_label("Other title".to_string());

    let update = TreeUpdate {
        nodes: vec![(NodeId(1), other), (NodeId(2), focused)],
        tree: Some(Tree::new(NodeId(1))),
        focus: NodeId(2),
    };

    let label = super::accessibility::webview_accessibility_label(webview_id, &update);
    assert!(label.contains("Focused title"));
}

#[test]
fn webview_accessibility_label_falls_back_when_no_labels_exist() {
    let webview_id = test_webview_id();
    let update = TreeUpdate {
        nodes: vec![(NodeId(5), Node::new(Role::Document))],
        tree: Some(Tree::new(NodeId(5))),
        focus: NodeId(5),
    };

    let label = super::accessibility::webview_accessibility_label(webview_id, &update);
    assert!(label.contains("Embedded web content"));
    assert!(label.contains("1 accessibility node update"));
}

#[test]
fn inject_webview_a11y_updates_drains_pending_map() {
    let webview_id = test_webview_id();
    let mut update_node = Node::new(Role::Document);
    update_node.set_label("Injected title".to_string());
    let update = TreeUpdate {
        nodes: vec![(NodeId(9), update_node)],
        tree: Some(Tree::new(NodeId(9))),
        focus: NodeId(9),
    };

    let mut pending = HashMap::from([(webview_id, update)]);
    let ctx = egui::Context::default();

    let _ = ctx.run(egui::RawInput::default(), |ctx| {
        super::accessibility::inject_webview_a11y_updates(ctx, &mut pending);
    });

    assert!(
        pending.is_empty(),
        "bridge injection should consume pending webview accessibility updates"
    );
}

#[test]
fn webview_a11y_graft_plan_includes_injectable_nodes_and_root() {
    let webview_id = test_webview_id();
    let mut root = Node::new(Role::Document);
    root.set_label("Page root".to_string());
    root.set_children(vec![NodeId(22)]);
    let mut child = Node::new(Role::Paragraph);
    child.set_label("Paragraph body".to_string());

    let update = TreeUpdate {
        nodes: vec![(NodeId(11), root), (NodeId(22), child)],
        tree: Some(Tree::new(NodeId(11))),
        focus: NodeId(11),
    };

    let plan = super::accessibility::build_webview_a11y_graft_plan(webview_id, &update);
    assert_eq!(plan.nodes.len(), 2);
    assert_eq!(plan.root_node_id, Some(NodeId(11)));
    assert_eq!(plan.dropped_node_count, 0);
    assert_eq!(plan.conversion_fallback_count, 0);
}

#[test]
fn webview_a11y_graft_plan_marks_reserved_ids_as_degraded() {
    let webview_id = test_webview_id();
    let mut reserved_root = Node::new(Role::Document);
    reserved_root.set_label("Reserved root".to_string());

    let update = TreeUpdate {
        nodes: vec![(NodeId(0), reserved_root)],
        tree: Some(Tree::new(NodeId(0))),
        focus: NodeId(0),
    };

    let plan = super::accessibility::build_webview_a11y_graft_plan(webview_id, &update);
    assert!(plan.nodes.is_empty());
    assert_eq!(plan.root_node_id, None);
    assert_eq!(plan.dropped_node_count, 1);
    assert_eq!(plan.conversion_fallback_count, 0);
}

#[test]
fn webview_a11y_graft_plan_tracks_role_conversion_fallbacks() {
    let webview_id = test_webview_id();
    let mut node = Node::new(Role::Article);
    node.set_label("Article root".to_string());
    let update = TreeUpdate {
        nodes: vec![(NodeId(44), node)],
        tree: Some(Tree::new(NodeId(44))),
        focus: NodeId(44),
    };

    let plan = super::accessibility::build_webview_a11y_graft_plan(webview_id, &update);
    assert_eq!(plan.nodes.len(), 1);
    assert_eq!(plan.conversion_fallback_count, 1);
}
