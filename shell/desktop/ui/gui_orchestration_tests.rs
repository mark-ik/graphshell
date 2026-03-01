use crate::app::{GraphBrowserApp, GraphIntent, GraphViewId, PendingTileOpenMode};
use crate::shell::desktop::ui::gui_orchestration;
use crate::shell::desktop::ui::gui_frame;
use crate::shell::desktop::workbench::pane_model::{NodePaneState, ToolPaneState};
use crate::shell::desktop::workbench::tile_kind::TileKind;
use base::id::{PIPELINE_NAMESPACE, PainterId, PipelineNamespace, TEST_NAMESPACE};
use egui_tiles::{Tile, Tiles, Tree};
use servo::WebViewId;

fn active_graph_count(tree: &Tree<TileKind>) -> usize {
    tree.active_tiles()
        .into_iter()
        .filter(|tile_id| {
            matches!(
                tree.tiles.get(*tile_id),
                Some(Tile::Pane(TileKind::Graph(_)))
            )
        })
        .count()
}

fn active_node_key(tree: &Tree<TileKind>) -> Option<crate::graph::NodeKey> {
    tree.active_tiles().into_iter().find_map(|tile_id| match tree.tiles.get(tile_id) {
        Some(Tile::Pane(TileKind::Node(state))) => Some(state.node),
        _ => None,
    })
}

fn node_pane_count(tree: &Tree<TileKind>) -> usize {
    tree.tiles
        .iter()
        .filter(|(_, tile)| matches!(tile, Tile::Pane(TileKind::Node(_))))
        .count()
}

fn test_webview_id() -> WebViewId {
    PIPELINE_NAMESPACE.with(|tls| {
        if tls.get().is_none() {
            PipelineNamespace::install(TEST_NAMESPACE);
        }
    });
    WebViewId::new(PainterId::next())
}

#[cfg(feature = "diagnostics")]
fn active_tool_pane(tree: &Tree<TileKind>, kind: ToolPaneState) -> bool {
    tree.active_tiles().into_iter().any(|tile_id| {
        matches!(
            tree.tiles.get(tile_id),
            Some(Tile::Pane(TileKind::Tool(tool_kind))) if *tool_kind == kind
        )
    })
}

#[test]
fn settings_history_url_intent_is_consumed_by_orchestration_authority() {
    let mut app = GraphBrowserApp::new_for_testing();
    let initial_view = GraphViewId::new();
    let mut tiles = Tiles::default();
    let root = tiles.insert_pane(TileKind::Graph(initial_view));
    let mut tree = Tree::new("graphshell_tiles", root, tiles);
    let mut intents = vec![GraphIntent::OpenSettingsUrl {
        url: "graphshell://settings/history".to_string(),
    }];

    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut intents);

    assert!(intents.is_empty());
}

#[test]
fn unknown_settings_url_intent_is_not_consumed_by_orchestration_authority() {
    let mut app = GraphBrowserApp::new_for_testing();
    let initial_view = GraphViewId::new();
    let mut tiles = Tiles::default();
    let root = tiles.insert_pane(TileKind::Graph(initial_view));
    let mut tree = Tree::new("graphshell_tiles", root, tiles);
    let unresolved_url = "graphshell://settings/not-a-real-route".to_string();
    let mut intents = vec![GraphIntent::OpenSettingsUrl {
        url: unresolved_url.clone(),
    }];

    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut intents);

    assert_eq!(intents.len(), 1);
    match &intents[0] {
        GraphIntent::OpenSettingsUrl { url } => assert_eq!(url, &unresolved_url),
        other => panic!("expected unresolved OpenSettingsUrl intent, got {other:?}"),
    }
}

#[test]
fn non_workbench_intent_is_preserved_by_orchestration_authority() {
    let mut app = GraphBrowserApp::new_for_testing();
    let initial_view = GraphViewId::new();
    let mut tiles = Tiles::default();
    let root = tiles.insert_pane(TileKind::Graph(initial_view));
    let mut tree = Tree::new("graphshell_tiles", root, tiles);
    let mut intents = vec![GraphIntent::ToggleCommandPalette];

    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut intents);

    assert_eq!(intents.len(), 1);
    assert!(matches!(intents[0], GraphIntent::ToggleCommandPalette));
}

#[cfg(feature = "diagnostics")]
#[test]
fn close_settings_tool_pane_restores_previous_graph_focus_via_orchestration() {
    let mut app = GraphBrowserApp::new_for_testing();
    let graph_view = GraphViewId::new();
    let mut tiles = Tiles::default();
    let graph = tiles.insert_pane(TileKind::Graph(graph_view));
    let root = tiles.insert_tab_tile(vec![graph]);
    let mut tree = Tree::new("graphshell_tiles", root, tiles);

    let mut open_intents = vec![GraphIntent::OpenSettingsUrl {
        url: "graphshell://settings/general".to_string(),
    }];
    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut open_intents);
    assert!(open_intents.is_empty());
    assert!(active_tool_pane(&tree, ToolPaneState::Settings));

    let mut close_intents = vec![GraphIntent::CloseToolPane {
        kind: ToolPaneState::Settings,
        restore_previous_focus: true,
    }];
    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut close_intents);

    assert!(close_intents.is_empty());
    assert!(active_graph_count(&tree) >= 1);
    assert!(tree.active_tiles().into_iter().any(|tile_id| {
        matches!(
            tree.tiles.get(tile_id),
            Some(Tile::Pane(TileKind::Graph(existing))) if *existing == graph_view
        )
    }));
}

#[cfg(feature = "diagnostics")]
#[test]
fn cycle_focus_region_intent_cycles_graph_node_tool_regions() {
    let graph_view = GraphViewId::new();
    let mut tiles = Tiles::default();
    let graph = tiles.insert_pane(TileKind::Graph(graph_view));
    let node_key = crate::graph::NodeKey::new(11);
    let node = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(node_key)));
    let tool = tiles.insert_pane(TileKind::Tool(ToolPaneState::Diagnostics));
    let root = tiles.insert_tab_tile(vec![graph, node, tool]);
    let mut tree = Tree::new("cycle_focus_orchestration", root, tiles);
    let mut app = GraphBrowserApp::new_for_testing();

    let _ = tree.make_active(|_, tile| matches!(tile, Tile::Pane(TileKind::Graph(_))));

    let mut intents = vec![GraphIntent::CycleFocusRegion];
    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut intents);
    assert!(intents.is_empty());
    assert!(tree.active_tiles().into_iter().any(|tile_id| {
        matches!(tree.tiles.get(tile_id), Some(Tile::Pane(TileKind::Node(_))))
    }));

    let mut intents = vec![GraphIntent::CycleFocusRegion];
    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut intents);
    assert!(intents.is_empty());
    assert!(tree.active_tiles().into_iter().any(|tile_id| {
        matches!(
            tree.tiles.get(tile_id),
            Some(Tile::Pane(TileKind::Tool(ToolPaneState::Diagnostics)))
        )
    }));

    let mut intents = vec![GraphIntent::CycleFocusRegion];
    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut intents);
    assert!(intents.is_empty());
    assert!(tree.active_tiles().into_iter().any(|tile_id| {
        matches!(
            tree.tiles.get(tile_id),
            Some(Tile::Pane(TileKind::Graph(existing))) if *existing == graph_view
        )
    }));
}

#[cfg(feature = "diagnostics")]
#[test]
fn close_history_tool_pane_restores_previous_node_focus_via_orchestration() {
    let mut app = GraphBrowserApp::new_for_testing();
    let graph_view = GraphViewId::new();
    let focus_node = crate::graph::NodeKey::new(77);
    let mut tiles = Tiles::default();
    let graph = tiles.insert_pane(TileKind::Graph(graph_view));
    let node = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(focus_node)));
    let root = tiles.insert_tab_tile(vec![graph, node]);
    let mut tree = Tree::new("restore_history_focus_node", root, tiles);

    let _ = tree.make_active(|_, tile| {
        matches!(tile, Tile::Pane(TileKind::Node(state)) if state.node == focus_node)
    });

    let mut open_intents = vec![GraphIntent::OpenToolPane {
        kind: ToolPaneState::HistoryManager,
    }];
    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut open_intents);
    assert!(open_intents.is_empty());
    assert!(active_tool_pane(&tree, ToolPaneState::HistoryManager));

    let mut close_intents = vec![GraphIntent::CloseToolPane {
        kind: ToolPaneState::HistoryManager,
        restore_previous_focus: true,
    }];
    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut close_intents);

    assert!(close_intents.is_empty());
    assert_eq!(active_node_key(&tree), Some(focus_node));
}

#[test]
fn open_pending_child_webviews_skips_unmapped_child_webview_ids() {
    let mut app = GraphBrowserApp::new_for_testing();
    let mapped_node = app.add_node_and_sync("https://example.com/mapped".to_string(), euclid::default::Point2D::new(0.0, 0.0));
    let mapped_webview = test_webview_id();
    let unmapped_webview = test_webview_id();
    app.map_webview_to_node(mapped_webview, mapped_node);

    let mut opened = Vec::new();
    let deferred = gui_frame::open_pending_child_webviews_for_tiles(
        &app,
        vec![mapped_webview, unmapped_webview],
        |node_key| opened.push(node_key),
    );

    assert_eq!(opened, vec![mapped_node]);
    assert_eq!(deferred, vec![unmapped_webview]);
}

#[test]
fn deferred_child_webview_retries_and_opens_via_frame_routed_intent() {
    let mut app = GraphBrowserApp::new_for_testing();
    let graph_view = GraphViewId::new();
    let mut tiles = Tiles::default();
    let graph = tiles.insert_pane(TileKind::Graph(graph_view));
    let root = tiles.insert_tab_tile(vec![graph]);
    let mut tree = Tree::new("graphshell_tiles", root, tiles);

    let child_webview = test_webview_id();
    let mut frame_intents = Vec::new();

    let deferred = super::open_pending_child_webview_nodes(
        &mut app,
        &mut frame_intents,
        vec![child_webview],
    );
    assert_eq!(deferred, vec![child_webview]);
    assert!(frame_intents.is_empty());

    let child_node = app.add_node_and_sync(
        "https://example.com/child".to_string(),
        euclid::default::Point2D::new(120.0, 80.0),
    );
    app.map_webview_to_node(child_webview, child_node);

    let deferred_after_mapping = super::open_pending_child_webview_nodes(
        &mut app,
        &mut frame_intents,
        deferred,
    );
    assert!(deferred_after_mapping.is_empty());
    assert!(frame_intents.iter().any(|intent| {
        matches!(
            intent,
            GraphIntent::OpenNodeFrameRouted {
                key,
                prefer_frame: None,
            } if *key == child_node
        )
    }));

    let mut open_node_tile_after_intents = None;
    super::apply_semantic_intents_and_pending_open(
        &mut app,
        &mut tree,
        &mut open_node_tile_after_intents,
        &mut frame_intents,
    );
    super::apply_semantic_intents_and_pending_open(
        &mut app,
        &mut tree,
        &mut open_node_tile_after_intents,
        &mut frame_intents,
    );

    let node_pane_exists = tree.tiles.iter().any(|(_, tile)| {
        matches!(
            tile,
            Tile::Pane(TileKind::Node(state)) if state.node == child_node
        )
    });
    assert!(node_pane_exists, "child node pane should open after routed retry");
}

#[test]
fn webview_created_child_open_routes_through_frame_routed_intent() {
    let mut app = GraphBrowserApp::new_for_testing();
    let parent_node = app.add_node_and_sync(
        "https://example.com/parent".to_string(),
        euclid::default::Point2D::new(30.0, 30.0),
    );
    let parent_webview = test_webview_id();
    let child_webview = test_webview_id();
    app.map_webview_to_node(parent_webview, parent_node);

    app.apply_intents([GraphIntent::WebViewCreated {
        parent_webview_id: parent_webview,
        child_webview_id: child_webview,
        initial_url: Some("https://example.com/child".to_string()),
    }]);

    assert_eq!(
        app.get_single_selected_node(),
        None,
        "webview creation should not directly mutate selection"
    );

    let child_node = app
        .get_node_for_webview(child_webview)
        .expect("child webview should map to a node");
    let mut frame_intents = Vec::new();
    let deferred = super::open_pending_child_webview_nodes(
        &mut app,
        &mut frame_intents,
        vec![child_webview],
    );

    assert!(deferred.is_empty());
    assert!(frame_intents.iter().any(|intent| {
        matches!(
            intent,
            GraphIntent::OpenNodeFrameRouted {
                key,
                prefer_frame: None,
            } if *key == child_node
        )
    }));
}

#[test]
fn pending_open_mode_is_one_shot_after_execution() {
    let mut app = GraphBrowserApp::new_for_testing();
    let selected = app.add_node_and_sync(
        "https://example.com/selected".to_string(),
        euclid::default::Point2D::new(10.0, 10.0),
    );
    app.select_node(selected, false);

    let mut tiles = Tiles::default();
    let graph = tiles.insert_pane(TileKind::Graph(GraphViewId::new()));
    let root = tiles.insert_tab_tile(vec![graph]);
    let mut tree = Tree::new("graphshell_tiles", root, tiles);
    let mut frame_intents = Vec::new();
    let mut open_node_tile_after_intents = Some(
        crate::shell::desktop::workbench::tile_view_ops::TileOpenMode::Tab,
    );

    super::handle_pending_open_node_after_intents(
        &mut app,
        &mut tree,
        &mut open_node_tile_after_intents,
        &mut frame_intents,
    );

    assert_eq!(open_node_tile_after_intents, None);
    assert_eq!(node_pane_count(&tree), 1);
    assert_eq!(active_node_key(&tree), Some(selected));
    let intents_after_first_pass = frame_intents.len();

    super::handle_pending_open_node_after_intents(
        &mut app,
        &mut tree,
        &mut open_node_tile_after_intents,
        &mut frame_intents,
    );

    assert_eq!(open_node_tile_after_intents, None);
    assert_eq!(node_pane_count(&tree), 1);
    assert_eq!(active_node_key(&tree), Some(selected));
    assert_eq!(frame_intents.len(), intents_after_first_pass);
}

#[test]
fn pending_open_request_split_mode_uses_split_route_and_focuses_node() {
    let mut app = GraphBrowserApp::new_for_testing();
    let selected = app.add_node_and_sync(
        "https://example.com/split".to_string(),
        euclid::default::Point2D::new(20.0, 20.0),
    );

    let mut tiles = Tiles::default();
    let graph = tiles.insert_pane(TileKind::Graph(GraphViewId::new()));
    let mut tree = Tree::new("graphshell_tiles", graph, tiles);
    let mut frame_intents = Vec::new();
    let mut open_node_tile_after_intents = None;

    app.request_open_node_tile_mode(selected, PendingTileOpenMode::SplitHorizontal);

    super::handle_pending_open_node_after_intents(
        &mut app,
        &mut tree,
        &mut open_node_tile_after_intents,
        &mut frame_intents,
    );

    assert_eq!(
        app.get_single_selected_node(),
        None,
        "selection should remain unchanged until reducer applies intents"
    );
    assert!(frame_intents.iter().any(|intent| {
        matches!(
            intent,
            GraphIntent::SelectNode {
                key,
                multi_select: false
            } if *key == selected
        )
    }));
    assert_eq!(active_node_key(&tree), Some(selected));
    assert_eq!(node_pane_count(&tree), 1);
    assert!(matches!(
        tree.root().and_then(|root| tree.tiles.get(root)),
        Some(Tile::Container(egui_tiles::Container::Linear(_)))
    ));

    app.apply_intents(std::mem::take(&mut frame_intents));
    assert_eq!(app.get_single_selected_node(), Some(selected));
}

#[test]
fn orchestration_preserves_semantic_intents_until_reducer_applies_them() {
    let mut app = GraphBrowserApp::new_for_testing();
    let node_key = app.add_node_and_sync(
        "https://before.example".to_string(),
        euclid::default::Point2D::new(5.0, 5.0),
    );

    let mut tiles = Tiles::default();
    let root = tiles.insert_pane(TileKind::Graph(GraphViewId::new()));
    let mut tree = Tree::new("graphshell_tiles", root, tiles);

    let mut intents = vec![GraphIntent::SetNodeUrl {
        key: node_key,
        new_url: "https://after.example".to_string(),
    }];

    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut intents);

    assert_eq!(intents.len(), 1);
    assert!(matches!(intents[0], GraphIntent::SetNodeUrl { .. }));
    assert_eq!(
        app.workspace
            .graph
            .get_node(node_key)
            .expect("node should exist")
            .url,
        "https://before.example"
    );

    app.apply_intents(intents);

    assert_eq!(
        app.workspace
            .graph
            .get_node(node_key)
            .expect("node should exist")
            .url,
        "https://after.example"
    );
}
