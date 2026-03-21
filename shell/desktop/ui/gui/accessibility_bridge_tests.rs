use std::collections::HashMap;

use accesskit::{Action, ActionRequest, Node, NodeId, Role, Tree, TreeUpdate};
use base::id::{PIPELINE_NAMESPACE, PainterId, PipelineNamespace, TEST_NAMESPACE};
use euclid::default::Point2D;
use servo::WebViewId;

use crate::app::{GraphBrowserApp, GraphReaderModeState, GraphViewId};
use crate::graph::NodeKey;
use crate::shell::desktop::workbench::tile_compositor::{
    LifecycleTreatment, TileAffordanceAnnotation,
};
use crate::shell::desktop::workbench::ux_tree::{
    UX_TREE_PRESENTATION_SCHEMA_VERSION, UX_TREE_SEMANTIC_SCHEMA_VERSION,
    UX_TREE_TRACE_SCHEMA_VERSION, UxAction, UxDomainIdentity, UxNodeRole, UxNodeState,
    UxSemanticNode, UxTraceSummary, UxTreeSnapshot,
};

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

#[test]
fn uxtree_a11y_graft_plan_projects_canonical_node_state_and_rendered_affordance() {
    let node_key = NodeKey::new(41);
    let snapshot = UxTreeSnapshot {
        semantic_version: UX_TREE_SEMANTIC_SCHEMA_VERSION,
        presentation_version: UX_TREE_PRESENTATION_SCHEMA_VERSION,
        trace_version: UX_TREE_TRACE_SCHEMA_VERSION,
        semantic_nodes: vec![UxSemanticNode {
            ux_node_id: "uxnode://workbench/tile/test/node/41".to_string(),
            parent_ux_node_id: None,
            role: UxNodeRole::GraphNode,
            label: "Node 41".to_string(),
            state: UxNodeState {
                focused: true,
                selected: true,
                blocked: false,
                degraded: false,
            },
            allowed_actions: vec![UxAction::Open],
            domain: UxDomainIdentity::Node { node_key },
        }],
        presentation_nodes: Vec::new(),
        trace_nodes: Vec::new(),
        trace_summary: UxTraceSummary {
            build_duration_us: 12,
            route_events_observed: 0,
            diagnostics_events_observed: 0,
        },
    };
    let graph_app = GraphBrowserApp::new_for_testing();
    let annotations = vec![TileAffordanceAnnotation {
        node_key,
        focus_ring_rendered: true,
        selection_ring_rendered: true,
        lifecycle_treatment: LifecycleTreatment::RuntimeBlocked,
        lens_glyphs_rendered: vec!["semantic".to_string(), "starred".to_string()],
        paint_callback_registered: true,
    }];

    let plan =
        super::accessibility::build_uxtree_a11y_graft_plan(&snapshot, &annotations, &graph_app);

    assert!(plan.anchor_label.contains("focused: Node 41"));
    assert_eq!(plan.nodes.len(), 1);

    let node = &plan.nodes[0];
    assert_eq!(node.role, egui::accesskit::Role::TreeItem);
    assert_eq!(node.label, "Node 41");
    assert!(node.selected);
    assert!(node.busy);
    assert!(!node.disabled);
    assert_eq!(
        node.description.as_deref(),
        Some("rendered lifecycle: runtime-blocked; rendered glyphs: semantic, starred")
    );
    let state_description = node
        .state_description
        .as_deref()
        .expect("node should expose canonical and rendered state");
    assert!(state_description.contains("focused"));
    assert!(state_description.contains("selected"));
    assert!(state_description.contains("rendered runtime-blocked"));
}

#[test]
fn uxtree_a11y_graft_plan_attaches_webview_anchor_and_graph_reader_under_host_hierarchy() {
    let mut graph_app = GraphBrowserApp::new_for_testing();
    let node_key =
        graph_app.add_node_and_sync("https://example.com".to_string(), Point2D::new(0.0, 0.0));
    let webview_id = test_webview_id();
    graph_app.map_webview_to_node(webview_id, node_key);

    let graph_view_id = GraphViewId::new();
    let graph_surface_id = format!("uxnode://workbench/tile/test/graph/{graph_view_id:?}");
    let node_pane_id = format!("uxnode://workbench/tile/test/node/{node_key:?}");
    let snapshot = UxTreeSnapshot {
        semantic_version: UX_TREE_SEMANTIC_SCHEMA_VERSION,
        presentation_version: UX_TREE_PRESENTATION_SCHEMA_VERSION,
        trace_version: UX_TREE_TRACE_SCHEMA_VERSION,
        semantic_nodes: vec![
            UxSemanticNode {
                ux_node_id: graph_surface_id.clone(),
                parent_ux_node_id: None,
                role: UxNodeRole::GraphSurface,
                label: "Graph Surface".to_string(),
                state: UxNodeState {
                    focused: true,
                    selected: false,
                    blocked: false,
                    degraded: false,
                },
                allowed_actions: vec![UxAction::Focus, UxAction::Navigate],
                domain: UxDomainIdentity::GraphView { graph_view_id },
            },
            UxSemanticNode {
                ux_node_id: node_pane_id.clone(),
                parent_ux_node_id: Some(graph_surface_id.clone()),
                role: UxNodeRole::NodePane,
                label: "Node Pane".to_string(),
                state: UxNodeState {
                    focused: false,
                    selected: false,
                    blocked: false,
                    degraded: false,
                },
                allowed_actions: vec![UxAction::Focus, UxAction::Open],
                domain: UxDomainIdentity::Node { node_key },
            },
        ],
        presentation_nodes: Vec::new(),
        trace_nodes: Vec::new(),
        trace_summary: UxTraceSummary {
            build_duration_us: 10,
            route_events_observed: 0,
            diagnostics_events_observed: 0,
        },
    };

    let plan = super::accessibility::build_uxtree_a11y_graft_plan(&snapshot, &[], &graph_app);

    let node_pane = plan
        .nodes
        .iter()
        .find(|node| node.ux_node_id == node_pane_id)
        .expect("node pane should be projected");
    assert_eq!(
        node_pane.attached_child_ids,
        vec![super::accessibility::webview_accessibility_anchor_id(
            webview_id
        )]
    );

    let map_root = plan
        .nodes
        .iter()
        .find(|node| node.label.starts_with("Graph Reader Map"))
        .expect("graph reader map root should be projected");
    assert_eq!(
        map_root.parent_ux_node_id.as_deref(),
        Some(graph_surface_id.as_str())
    );

    let map_item = plan
        .nodes
        .iter()
        .find(|node| node.parent_ux_node_id.as_deref() == Some(map_root.ux_node_id.as_str()))
        .expect("graph reader should include at least one map item");
    assert!(!map_item.label.is_empty());
}

#[test]
fn graph_reader_map_click_action_resolves_to_enter_room_dispatch() {
    let mut graph_app = GraphBrowserApp::new_for_testing();
    let node_key =
        graph_app.add_node_and_sync("https://example.com".to_string(), Point2D::new(0.0, 0.0));
    let graph_view_id = GraphViewId::new();
    graph_app.workspace.graph_runtime.focused_view = Some(graph_view_id);

    let graph_surface_id = format!("uxnode://workbench/tile/test/graph/{graph_view_id:?}");
    let snapshot = UxTreeSnapshot {
        semantic_version: UX_TREE_SEMANTIC_SCHEMA_VERSION,
        presentation_version: UX_TREE_PRESENTATION_SCHEMA_VERSION,
        trace_version: UX_TREE_TRACE_SCHEMA_VERSION,
        semantic_nodes: vec![UxSemanticNode {
            ux_node_id: graph_surface_id,
            parent_ux_node_id: None,
            role: UxNodeRole::GraphSurface,
            label: "Graph Surface".to_string(),
            state: UxNodeState {
                focused: true,
                selected: false,
                blocked: false,
                degraded: false,
            },
            allowed_actions: vec![UxAction::Focus, UxAction::Navigate],
            domain: UxDomainIdentity::GraphView { graph_view_id },
        }],
        presentation_nodes: Vec::new(),
        trace_nodes: Vec::new(),
        trace_summary: UxTraceSummary {
            build_duration_us: 10,
            route_events_observed: 0,
            diagnostics_events_observed: 0,
        },
    };

    let plan = super::accessibility::build_uxtree_a11y_graft_plan(&snapshot, &[], &graph_app);
    let map_item = plan
        .nodes
        .iter()
        .find(|node| {
            node.action_route
                == Some(
                    super::accessibility::UxTreeA11yActionRoute::GraphReaderMapItem { node_key },
                )
        })
        .expect("graph reader map item should be actionable");
    let req = ActionRequest {
        action: Action::Click,
        target: super::accessibility::accesskit_node_id_from_egui_id(
            super::accessibility::uxtree_accessibility_node_id(&map_item.ux_node_id),
        ),
        data: None,
    };

    let dispatch = super::accessibility::resolve_uxtree_accesskit_action_for_plan(&plan, &req);
    assert_eq!(
        dispatch,
        Some(super::accessibility::UxTreeAccesskitDispatch::EnterGraphReaderRoom { node_key })
    );
}

#[test]
fn graph_reader_map_focus_action_resolves_to_map_focus_dispatch() {
    let mut graph_app = GraphBrowserApp::new_for_testing();
    let node_key =
        graph_app.add_node_and_sync("https://example.com".to_string(), Point2D::new(0.0, 0.0));
    let graph_view_id = GraphViewId::new();
    graph_app.workspace.graph_runtime.focused_view = Some(graph_view_id);

    let graph_surface_id = format!("uxnode://workbench/tile/test/graph/{graph_view_id:?}");
    let snapshot = UxTreeSnapshot {
        semantic_version: UX_TREE_SEMANTIC_SCHEMA_VERSION,
        presentation_version: UX_TREE_PRESENTATION_SCHEMA_VERSION,
        trace_version: UX_TREE_TRACE_SCHEMA_VERSION,
        semantic_nodes: vec![UxSemanticNode {
            ux_node_id: graph_surface_id,
            parent_ux_node_id: None,
            role: UxNodeRole::GraphSurface,
            label: "Graph Surface".to_string(),
            state: UxNodeState {
                focused: true,
                selected: false,
                blocked: false,
                degraded: false,
            },
            allowed_actions: vec![UxAction::Focus, UxAction::Navigate],
            domain: UxDomainIdentity::GraphView { graph_view_id },
        }],
        presentation_nodes: Vec::new(),
        trace_nodes: Vec::new(),
        trace_summary: UxTraceSummary {
            build_duration_us: 10,
            route_events_observed: 0,
            diagnostics_events_observed: 0,
        },
    };

    let plan = super::accessibility::build_uxtree_a11y_graft_plan(&snapshot, &[], &graph_app);
    let map_item = plan
        .nodes
        .iter()
        .find(|node| {
            node.action_route
                == Some(
                    super::accessibility::UxTreeA11yActionRoute::GraphReaderMapItem { node_key },
                )
        })
        .expect("graph reader map item should be actionable");
    let req = ActionRequest {
        action: Action::Focus,
        target: super::accessibility::accesskit_node_id_from_egui_id(
            super::accessibility::uxtree_accessibility_node_id(&map_item.ux_node_id),
        ),
        data: None,
    };

    let dispatch = super::accessibility::resolve_uxtree_accesskit_action_for_plan(&plan, &req);
    assert_eq!(
        dispatch,
        Some(super::accessibility::UxTreeAccesskitDispatch::FocusGraphReaderMapItem { node_key })
    );
}

#[test]
fn graph_reader_room_root_click_action_resolves_to_return_to_map_dispatch() {
    let mut graph_app = GraphBrowserApp::new_for_testing();
    let node_key = graph_app.add_node_and_sync(
        "https://room-root.example".to_string(),
        Point2D::new(0.0, 0.0),
    );
    let graph_view_id = GraphViewId::new();
    graph_app.workspace.graph_runtime.focused_view = Some(graph_view_id);
    graph_app.graph_reader_enter_room(node_key);

    let graph_surface_id = format!("uxnode://workbench/tile/test/graph/{graph_view_id:?}");
    let snapshot = UxTreeSnapshot {
        semantic_version: UX_TREE_SEMANTIC_SCHEMA_VERSION,
        presentation_version: UX_TREE_PRESENTATION_SCHEMA_VERSION,
        trace_version: UX_TREE_TRACE_SCHEMA_VERSION,
        semantic_nodes: vec![UxSemanticNode {
            ux_node_id: graph_surface_id,
            parent_ux_node_id: None,
            role: UxNodeRole::GraphSurface,
            label: "Graph Surface".to_string(),
            state: UxNodeState {
                focused: true,
                selected: false,
                blocked: false,
                degraded: false,
            },
            allowed_actions: vec![UxAction::Focus, UxAction::Navigate],
            domain: UxDomainIdentity::GraphView { graph_view_id },
        }],
        presentation_nodes: Vec::new(),
        trace_nodes: Vec::new(),
        trace_summary: UxTraceSummary {
            build_duration_us: 10,
            route_events_observed: 0,
            diagnostics_events_observed: 0,
        },
    };

    let plan = super::accessibility::build_uxtree_a11y_graft_plan(&snapshot, &[], &graph_app);
    let room_root = plan
        .nodes
        .iter()
        .find(|node| {
            node.action_route
                == Some(super::accessibility::UxTreeA11yActionRoute::GraphReaderRoomRoot)
        })
        .expect("graph reader room root should be actionable");
    let req = ActionRequest {
        action: Action::Click,
        target: super::accessibility::accesskit_node_id_from_egui_id(
            super::accessibility::uxtree_accessibility_node_id(&room_root.ux_node_id),
        ),
        data: None,
    };

    let dispatch = super::accessibility::resolve_uxtree_accesskit_action_for_plan(&plan, &req);
    assert_eq!(
        dispatch,
        Some(super::accessibility::UxTreeAccesskitDispatch::ReturnGraphReaderToMap)
    );
}

#[test]
fn graph_reader_room_item_click_action_resolves_to_enter_room_dispatch() {
    let mut graph_app = GraphBrowserApp::new_for_testing();
    let room_node = graph_app.add_node_and_sync(
        "https://room-source.example".to_string(),
        Point2D::new(0.0, 0.0),
    );
    let neighbor = graph_app.add_node_and_sync(
        "https://room-neighbor.example".to_string(),
        Point2D::new(10.0, 0.0),
    );
    graph_app.apply_reducer_intents([crate::app::GraphIntent::CreateUserGroupedEdge {
        from: room_node,
        to: neighbor,
        label: None,
    }]);
    let graph_view_id = GraphViewId::new();
    graph_app.workspace.graph_runtime.focused_view = Some(graph_view_id);
    graph_app.graph_reader_enter_room(room_node);

    let graph_surface_id = format!("uxnode://workbench/tile/test/graph/{graph_view_id:?}");
    let snapshot = UxTreeSnapshot {
        semantic_version: UX_TREE_SEMANTIC_SCHEMA_VERSION,
        presentation_version: UX_TREE_PRESENTATION_SCHEMA_VERSION,
        trace_version: UX_TREE_TRACE_SCHEMA_VERSION,
        semantic_nodes: vec![UxSemanticNode {
            ux_node_id: graph_surface_id,
            parent_ux_node_id: None,
            role: UxNodeRole::GraphSurface,
            label: "Graph Surface".to_string(),
            state: UxNodeState {
                focused: true,
                selected: false,
                blocked: false,
                degraded: false,
            },
            allowed_actions: vec![UxAction::Focus, UxAction::Navigate],
            domain: UxDomainIdentity::GraphView { graph_view_id },
        }],
        presentation_nodes: Vec::new(),
        trace_nodes: Vec::new(),
        trace_summary: UxTraceSummary {
            build_duration_us: 10,
            route_events_observed: 0,
            diagnostics_events_observed: 0,
        },
    };

    let plan = super::accessibility::build_uxtree_a11y_graft_plan(&snapshot, &[], &graph_app);
    let room_item = plan
        .nodes
        .iter()
        .find(|node| {
            node.action_route
                == Some(
                    super::accessibility::UxTreeA11yActionRoute::GraphReaderRoomItem {
                        node_key: neighbor,
                    },
                )
        })
        .expect("graph reader room item should be actionable");
    let req = ActionRequest {
        action: Action::Click,
        target: super::accessibility::accesskit_node_id_from_egui_id(
            super::accessibility::uxtree_accessibility_node_id(&room_item.ux_node_id),
        ),
        data: None,
    };

    let dispatch = super::accessibility::resolve_uxtree_accesskit_action_for_plan(&plan, &req);
    assert_eq!(
        dispatch,
        Some(
            super::accessibility::UxTreeAccesskitDispatch::EnterGraphReaderRoom {
                node_key: neighbor
            }
        )
    );
}

#[test]
fn graph_reader_return_to_map_suppresses_room_projection_and_preserves_map_focus() {
    let mut graph_app = GraphBrowserApp::new_for_testing();
    let node_key = graph_app.add_node_and_sync(
        "https://return-map.example".to_string(),
        Point2D::new(0.0, 0.0),
    );
    let graph_view_id = GraphViewId::new();
    graph_app.workspace.graph_runtime.focused_view = Some(graph_view_id);
    graph_app.graph_reader_enter_room(node_key);
    graph_app.graph_reader_return_to_map();

    assert_eq!(
        graph_app.graph_reader_mode(),
        Some(GraphReaderModeState::Map {
            focused_node: Some(node_key)
        })
    );

    let graph_surface_id = format!("uxnode://workbench/tile/test/graph/{graph_view_id:?}");
    let snapshot = UxTreeSnapshot {
        semantic_version: UX_TREE_SEMANTIC_SCHEMA_VERSION,
        presentation_version: UX_TREE_PRESENTATION_SCHEMA_VERSION,
        trace_version: UX_TREE_TRACE_SCHEMA_VERSION,
        semantic_nodes: vec![UxSemanticNode {
            ux_node_id: graph_surface_id,
            parent_ux_node_id: None,
            role: UxNodeRole::GraphSurface,
            label: "Graph Surface".to_string(),
            state: UxNodeState {
                focused: true,
                selected: false,
                blocked: false,
                degraded: false,
            },
            allowed_actions: vec![UxAction::Focus, UxAction::Navigate],
            domain: UxDomainIdentity::GraphView { graph_view_id },
        }],
        presentation_nodes: Vec::new(),
        trace_nodes: Vec::new(),
        trace_summary: UxTraceSummary {
            build_duration_us: 10,
            route_events_observed: 0,
            diagnostics_events_observed: 0,
        },
    };

    let plan = super::accessibility::build_uxtree_a11y_graft_plan(&snapshot, &[], &graph_app);
    assert!(
        plan.nodes
            .iter()
            .any(|node| node.label.starts_with("Graph Reader Map"))
    );
    assert!(
        !plan
            .nodes
            .iter()
            .any(|node| node.label.starts_with("Graph Reader Room:"))
    );
}
