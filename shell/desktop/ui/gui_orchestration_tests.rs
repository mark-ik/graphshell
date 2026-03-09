use crate::app::{GraphBrowserApp, GraphIntent, GraphViewId, PendingTileOpenMode, WorkbenchIntent};
use crate::shell::desktop::runtime::registries::{
    CHANNEL_UX_CONTRACT_WARNING, CHANNEL_UX_DISPATCH_CONSUMED,
    CHANNEL_UX_DISPATCH_DEFAULT_PREVENTED, CHANNEL_UX_DISPATCH_PHASE, CHANNEL_UX_DISPATCH_STARTED,
    CHANNEL_UX_NAVIGATION_TRANSITION, CHANNEL_UX_NAVIGATION_VIOLATION,
    CHANNEL_UX_OPEN_DECISION_PATH, CHANNEL_UX_OPEN_DECISION_REASON,
};
use crate::shell::desktop::ui::gui_orchestration;
use crate::shell::desktop::workbench::pane_model::{NodePaneState, ToolPaneState};
use crate::shell::desktop::workbench::tile_kind::TileKind;
use base::id::{PIPELINE_NAMESPACE, PainterId, PipelineNamespace, TEST_NAMESPACE};
use egui_tiles::{Tile, Tiles, Tree};
use servo::WebViewId;
use tempfile::TempDir;

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
    tree.active_tiles()
        .into_iter()
        .find_map(|tile_id| match tree.tiles.get(tile_id) {
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
    let mut intents = vec![WorkbenchIntent::OpenSettingsUrl {
        url: crate::util::VersoAddress::settings(crate::util::GraphshellSettingsPath::History)
            .to_string(),
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
    let unresolved_url = crate::util::VersoAddress::settings(
        crate::util::GraphshellSettingsPath::Other("not-a-real-route".to_string()),
    )
    .to_string();
    let mut intents = vec![WorkbenchIntent::OpenSettingsUrl {
        url: unresolved_url.clone(),
    }];

    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut intents);

    assert_eq!(intents.len(), 1);
    match &intents[0] {
        WorkbenchIntent::OpenSettingsUrl { url } => assert_eq!(url, &unresolved_url),
        other => panic!("expected unresolved OpenSettingsUrl intent, got {other:?}"),
    }
}

#[test]
fn frame_url_intent_queues_frame_restore_via_orchestration_authority() {
    let mut app = GraphBrowserApp::new_for_testing();
    let initial_view = GraphViewId::new();
    let mut tiles = Tiles::default();
    let root = tiles.insert_pane(TileKind::Graph(initial_view));
    let mut tree = Tree::new("graphshell_tiles", root, tiles);
    let mut intents = vec![WorkbenchIntent::OpenFrameUrl {
        url: crate::util::VersoAddress::frame("frame-123").to_string(),
    }];

    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut intents);

    assert!(intents.is_empty());
    assert_eq!(
        app.take_pending_restore_frame_snapshot_named().as_deref(),
        Some("frame-123")
    );
}

#[test]
fn invalid_frame_url_intent_is_not_consumed_by_orchestration_authority() {
    let mut app = GraphBrowserApp::new_for_testing();
    let initial_view = GraphViewId::new();
    let mut tiles = Tiles::default();
    let root = tiles.insert_pane(TileKind::Graph(initial_view));
    let mut tree = Tree::new("graphshell_tiles", root, tiles);
    let unresolved_url = "verso://frame".to_string();
    let mut intents = vec![WorkbenchIntent::OpenFrameUrl {
        url: unresolved_url.clone(),
    }];

    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut intents);

    assert_eq!(intents.len(), 1);
    match &intents[0] {
        WorkbenchIntent::OpenFrameUrl { url } => assert_eq!(url, &unresolved_url),
        other => panic!("expected unresolved OpenFrameUrl intent, got {other:?}"),
    }
}

#[cfg(feature = "diagnostics")]
#[test]
fn tool_url_intent_opens_history_tool_via_orchestration_authority() {
    let mut app = GraphBrowserApp::new_for_testing();
    let initial_view = GraphViewId::new();
    let mut tiles = Tiles::default();
    let root = tiles.insert_pane(TileKind::Graph(initial_view));
    let mut tree = Tree::new("graphshell_tiles", root, tiles);
    let mut intents = vec![WorkbenchIntent::OpenToolUrl {
        url: crate::util::VersoAddress::tool("history", Some(2)).to_string(),
    }];

    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut intents);

    assert!(intents.is_empty());
    assert!(active_tool_pane(&tree, ToolPaneState::HistoryManager));
}

#[test]
fn unknown_tool_url_intent_is_not_consumed_by_orchestration_authority() {
    let mut app = GraphBrowserApp::new_for_testing();
    let initial_view = GraphViewId::new();
    let mut tiles = Tiles::default();
    let root = tiles.insert_pane(TileKind::Graph(initial_view));
    let mut tree = Tree::new("graphshell_tiles", root, tiles);
    let unresolved_url = crate::util::VersoAddress::tool("unknown-tool", None).to_string();
    let mut intents = vec![WorkbenchIntent::OpenToolUrl {
        url: unresolved_url.clone(),
    }];

    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut intents);

    assert_eq!(intents.len(), 1);
    match &intents[0] {
        WorkbenchIntent::OpenToolUrl { url } => assert_eq!(url, &unresolved_url),
        other => panic!("expected unresolved OpenToolUrl intent, got {other:?}"),
    }
}

#[cfg(feature = "diagnostics")]
#[test]
fn clip_url_intent_is_consumed_by_orchestration_authority() {
    let mut app = GraphBrowserApp::new_for_testing();
    let initial_view = GraphViewId::new();
    let mut tiles = Tiles::default();
    let root = tiles.insert_pane(TileKind::Graph(initial_view));
    let mut tree = Tree::new("graphshell_tiles", root, tiles);
    let mut intents = vec![WorkbenchIntent::OpenClipUrl {
        url: crate::util::VersoAddress::clip("clip-42").to_string(),
    }];

    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut intents);

    assert!(intents.is_empty());
    assert_eq!(
        app.take_pending_open_clip_request().as_deref(),
        Some("clip-42")
    );
    assert!(active_tool_pane(&tree, ToolPaneState::HistoryManager));
}

#[test]
fn invalid_clip_url_intent_is_not_consumed_by_orchestration_authority() {
    let mut app = GraphBrowserApp::new_for_testing();
    let initial_view = GraphViewId::new();
    let mut tiles = Tiles::default();
    let root = tiles.insert_pane(TileKind::Graph(initial_view));
    let mut tree = Tree::new("graphshell_tiles", root, tiles);
    let unresolved_url = "verso://clip".to_string();
    let mut intents = vec![WorkbenchIntent::OpenClipUrl {
        url: unresolved_url.clone(),
    }];

    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut intents);

    assert_eq!(intents.len(), 1);
    match &intents[0] {
        WorkbenchIntent::OpenClipUrl { url } => assert_eq!(url, &unresolved_url),
        other => panic!("expected unresolved OpenClipUrl intent, got {other:?}"),
    }
}

#[test]
fn view_url_intent_opens_graph_view_via_orchestration_authority() {
    let mut app = GraphBrowserApp::new_for_testing();
    let initial_view = GraphViewId::new();
    let mut tiles = Tiles::default();
    let root = tiles.insert_pane(TileKind::Graph(initial_view));
    let mut tree = Tree::new("graphshell_tiles", root, tiles);
    let route_uuid = uuid::Uuid::new_v4().to_string();
    let view_url = crate::util::VersoAddress::view(route_uuid).to_string();
    let expected_view =
        match GraphBrowserApp::resolve_view_route(&view_url).expect("view url should resolve") {
            crate::app::ViewRouteTarget::GraphPane(view_id) => view_id,
            other => panic!("expected legacy graph-pane route, got {other:?}"),
        };
    let mut intents = vec![WorkbenchIntent::OpenViewUrl { url: view_url }];

    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut intents);

    assert!(intents.is_empty());
    assert_eq!(
        crate::shell::desktop::workbench::tile_view_ops::active_graph_view_id(&tree),
        Some(expected_view)
    );
}

#[test]
fn open_graph_view_pane_intent_routes_to_workbench_pane_open() {
    let mut app = GraphBrowserApp::new_for_testing();
    let initial_view = GraphViewId::new();
    let new_view = GraphViewId::new();
    let mut tiles = Tiles::default();
    let root = tiles.insert_pane(TileKind::Graph(initial_view));
    let mut tree = Tree::new("graphshell_tiles", root, tiles);
    let mut intents = vec![WorkbenchIntent::OpenGraphViewPane {
        view_id: new_view,
        mode: crate::app::PendingTileOpenMode::SplitHorizontal,
    }];

    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut intents);

    assert!(intents.is_empty());
    assert_eq!(
        crate::shell::desktop::workbench::tile_view_ops::active_graph_view_id(&tree),
        Some(new_view)
    );
}

#[test]
fn invalid_view_url_intent_is_not_consumed_by_orchestration_authority() {
    let mut app = GraphBrowserApp::new_for_testing();
    let initial_view = GraphViewId::new();
    let mut tiles = Tiles::default();
    let root = tiles.insert_pane(TileKind::Graph(initial_view));
    let mut tree = Tree::new("graphshell_tiles", root, tiles);
    let unresolved_url = crate::util::VersoAddress::view("not-a-uuid").to_string();
    let mut intents = vec![WorkbenchIntent::OpenViewUrl {
        url: unresolved_url.clone(),
    }];

    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut intents);

    assert_eq!(intents.len(), 1);
    match &intents[0] {
        WorkbenchIntent::OpenViewUrl { url } => assert_eq!(url, &unresolved_url),
        other => panic!("expected unresolved OpenViewUrl intent, got {other:?}"),
    }
}

#[test]
fn note_view_url_intent_queues_note_open_via_orchestration_authority() {
    let mut app = GraphBrowserApp::new_for_testing();
    let initial_view = GraphViewId::new();
    let mut tiles = Tiles::default();
    let root = tiles.insert_pane(TileKind::Graph(initial_view));
    let mut tree = Tree::new("graphshell_tiles", root, tiles);
    let node_key = app.add_node_and_sync(
        "https://example.com/article".to_string(),
        euclid::default::Point2D::new(0.0, 0.0),
    );
    let note_id = app
        .create_note_for_node(node_key, Some("Article Note".to_string()))
        .expect("note should be created");
    let _ = app.take_pending_open_note_request();
    let _ = app.take_pending_open_node_request();
    let mut intents = vec![WorkbenchIntent::OpenViewUrl {
        url: crate::util::VersoAddress::view_note(note_id.as_uuid().to_string()).to_string(),
    }];

    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut intents);

    assert!(intents.is_empty());
    assert_eq!(app.take_pending_open_note_request(), Some(note_id));
}

#[test]
fn node_view_url_intent_opens_node_pane_via_orchestration_authority() {
    let mut app = GraphBrowserApp::new_for_testing();
    let initial_view = GraphViewId::new();
    let mut tiles = Tiles::default();
    let root = tiles.insert_pane(TileKind::Graph(initial_view));
    let mut tree = Tree::new("graphshell_tiles", root, tiles);
    let node_key = app.add_node_and_sync(
        "https://example.com/view-node".to_string(),
        euclid::default::Point2D::new(0.0, 0.0),
    );
    let node_id = app
        .workspace
        .domain
        .graph
        .get_node(node_key)
        .expect("node should exist")
        .id;
    let mut intents = vec![WorkbenchIntent::OpenViewUrl {
        url: crate::util::VersoAddress::view_node(node_id.to_string()).to_string(),
    }];

    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut intents);

    assert!(intents.is_empty());
    assert_eq!(active_node_key(&tree), Some(node_key));
}

#[test]
fn invalid_node_view_url_intent_is_not_consumed_by_orchestration_authority() {
    let mut app = GraphBrowserApp::new_for_testing();
    let initial_view = GraphViewId::new();
    let mut tiles = Tiles::default();
    let root = tiles.insert_pane(TileKind::Graph(initial_view));
    let mut tree = Tree::new("graphshell_tiles", root, tiles);
    let unresolved_url = crate::util::VersoAddress::view_node("not-a-uuid").to_string();
    let mut intents = vec![WorkbenchIntent::OpenViewUrl {
        url: unresolved_url.clone(),
    }];

    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut intents);

    assert_eq!(intents.len(), 1);
    match &intents[0] {
        WorkbenchIntent::OpenViewUrl { url } => assert_eq!(url, &unresolved_url),
        other => panic!("expected unresolved OpenViewUrl intent, got {other:?}"),
    }
}

#[test]
fn note_url_intent_queues_note_open_via_orchestration_authority() {
    let mut app = GraphBrowserApp::new_for_testing();
    let initial_view = GraphViewId::new();
    let mut tiles = Tiles::default();
    let root = tiles.insert_pane(TileKind::Graph(initial_view));
    let mut tree = Tree::new("graphshell_tiles", root, tiles);
    let node_key = app.add_node_and_sync(
        "https://example.com/note-url".to_string(),
        euclid::default::Point2D::new(0.0, 0.0),
    );
    let note_id = app
        .create_note_for_node(node_key, Some("Routed Note".to_string()))
        .expect("note should be created");
    let _ = app.take_pending_open_note_request();
    let mut intents = vec![WorkbenchIntent::OpenNoteUrl {
        url: crate::util::NoteAddress::note(note_id.as_uuid().to_string()).to_string(),
    }];

    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut intents);

    assert!(intents.is_empty());
    assert_eq!(app.take_pending_open_note_request(), Some(note_id));
}

#[test]
fn invalid_note_url_intent_is_not_consumed_by_orchestration_authority() {
    let mut app = GraphBrowserApp::new_for_testing();
    let initial_view = GraphViewId::new();
    let mut tiles = Tiles::default();
    let root = tiles.insert_pane(TileKind::Graph(initial_view));
    let mut tree = Tree::new("graphshell_tiles", root, tiles);
    let unresolved_url = "notes://not-a-uuid".to_string();
    let mut intents = vec![WorkbenchIntent::OpenNoteUrl {
        url: unresolved_url.clone(),
    }];

    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut intents);

    assert_eq!(intents.len(), 1);
    match &intents[0] {
        WorkbenchIntent::OpenNoteUrl { url } => assert_eq!(url, &unresolved_url),
        other => panic!("expected unresolved OpenNoteUrl intent, got {other:?}"),
    }
}

#[test]
fn graph_view_url_intent_queues_named_graph_restore_when_snapshot_exists() {
    let dir = TempDir::new().expect("temp dir should be created");
    let mut app = GraphBrowserApp::new_from_dir(dir.path().to_path_buf());
    app.add_node_and_sync(
        "https://example.com/graph-seed".to_string(),
        euclid::default::Point2D::new(0.0, 0.0),
    );
    app.save_named_graph_snapshot("graph-main")
        .expect("named graph snapshot should save");

    let initial_view = GraphViewId::new();
    let mut tiles = Tiles::default();
    let root = tiles.insert_pane(TileKind::Graph(initial_view));
    let mut tree = Tree::new("graphshell_tiles", root, tiles);
    let mut intents = vec![WorkbenchIntent::OpenViewUrl {
        url: crate::util::VersoAddress::view_graph("graph-main").to_string(),
    }];

    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut intents);

    assert!(intents.is_empty());
    assert_eq!(
        app.take_pending_restore_graph_snapshot_named().as_deref(),
        Some("graph-main")
    );
}

#[test]
fn unresolved_graph_view_url_intent_is_not_consumed_by_orchestration_authority() {
    let mut app = GraphBrowserApp::new_for_testing();
    let initial_view = GraphViewId::new();
    let mut tiles = Tiles::default();
    let root = tiles.insert_pane(TileKind::Graph(initial_view));
    let mut tree = Tree::new("graphshell_tiles", root, tiles);
    let unresolved_url = crate::util::VersoAddress::view_graph("missing-graph").to_string();
    let mut intents = vec![WorkbenchIntent::OpenViewUrl {
        url: unresolved_url.clone(),
    }];

    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut intents);

    assert_eq!(intents.len(), 1);
    match &intents[0] {
        WorkbenchIntent::OpenViewUrl { url } => assert_eq!(url, &unresolved_url),
        other => panic!("expected unresolved OpenViewUrl intent, got {other:?}"),
    }
}

#[test]
fn unresolved_note_view_url_intent_is_not_consumed_by_orchestration_authority() {
    let mut app = GraphBrowserApp::new_for_testing();
    let initial_view = GraphViewId::new();
    let mut tiles = Tiles::default();
    let root = tiles.insert_pane(TileKind::Graph(initial_view));
    let mut tree = Tree::new("graphshell_tiles", root, tiles);
    let unresolved_url =
        crate::util::VersoAddress::view_note(uuid::Uuid::new_v4().to_string()).to_string();
    let mut intents = vec![WorkbenchIntent::OpenViewUrl {
        url: unresolved_url.clone(),
    }];

    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut intents);

    assert_eq!(intents.len(), 1);
    match &intents[0] {
        WorkbenchIntent::OpenViewUrl { url } => assert_eq!(url, &unresolved_url),
        other => panic!("expected unresolved OpenViewUrl intent, got {other:?}"),
    }
}

#[test]
fn graph_url_intent_queues_named_graph_restore_when_snapshot_exists() {
    let dir = TempDir::new().expect("temp dir should be created");
    let mut app = GraphBrowserApp::new_from_dir(dir.path().to_path_buf());
    app.add_node_and_sync(
        "https://example.com/graph-seed".to_string(),
        euclid::default::Point2D::new(0.0, 0.0),
    );
    app.save_named_graph_snapshot("graph-main")
        .expect("named graph snapshot should save");

    let initial_view = GraphViewId::new();
    let mut tiles = Tiles::default();
    let root = tiles.insert_pane(TileKind::Graph(initial_view));
    let mut tree = Tree::new("graphshell_tiles", root, tiles);
    let mut intents = vec![WorkbenchIntent::OpenGraphUrl {
        url: crate::util::GraphAddress::graph("graph-main").to_string(),
    }];

    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut intents);

    assert!(intents.is_empty());
    assert_eq!(
        app.take_pending_restore_graph_snapshot_named().as_deref(),
        Some("graph-main")
    );
}

#[test]
fn unresolved_graph_url_intent_is_not_consumed_by_orchestration_authority() {
    let mut app = GraphBrowserApp::new_for_testing();
    let initial_view = GraphViewId::new();
    let mut tiles = Tiles::default();
    let root = tiles.insert_pane(TileKind::Graph(initial_view));
    let mut tree = Tree::new("graphshell_tiles", root, tiles);
    let unresolved_url = crate::util::GraphAddress::graph("missing-graph").to_string();
    let mut intents = vec![WorkbenchIntent::OpenGraphUrl {
        url: unresolved_url.clone(),
    }];

    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut intents);

    assert_eq!(intents.len(), 1);
    match &intents[0] {
        WorkbenchIntent::OpenGraphUrl { url } => assert_eq!(url, &unresolved_url),
        other => panic!("expected unresolved OpenGraphUrl intent, got {other:?}"),
    }
}

#[test]
fn pending_note_open_request_is_consumed_by_orchestration_semantic_phase() {
    let mut app = GraphBrowserApp::new_for_testing();
    let initial_view = GraphViewId::new();
    let mut tiles = Tiles::default();
    let root = tiles.insert_pane(TileKind::Graph(initial_view));
    let mut tree = Tree::new("graphshell_tiles", root, tiles);
    let node_key = app.add_node_and_sync(
        "https://example.com/semantic-note".to_string(),
        euclid::default::Point2D::new(0.0, 0.0),
    );
    let note_id = app
        .create_note_for_node(node_key, Some("Semantic Note".to_string()))
        .expect("note should be created");
    let _ = app.take_pending_open_node_request();
    let _ = app.take_pending_open_note_request();
    app.request_open_note_by_id(note_id);

    gui_orchestration::handle_pending_open_note_after_intents(&mut app, &mut tree);

    assert!(app.take_pending_open_note_request().is_none());
    assert!(node_pane_count(&tree) >= 1);
}

#[test]
fn pending_unknown_note_open_request_is_cleared_by_orchestration_semantic_phase() {
    let mut app = GraphBrowserApp::new_for_testing();
    let initial_view = GraphViewId::new();
    let mut tiles = Tiles::default();
    let root = tiles.insert_pane(TileKind::Graph(initial_view));
    let mut tree = Tree::new("graphshell_tiles", root, tiles);
    app.request_open_note_by_id(crate::app::NoteId::new());

    gui_orchestration::handle_pending_open_note_after_intents(&mut app, &mut tree);

    assert!(app.take_pending_open_note_request().is_none());
}

#[test]
fn pending_clip_open_request_is_consumed_by_orchestration_semantic_phase() {
    let mut app = GraphBrowserApp::new_for_testing();
    let initial_view = GraphViewId::new();
    let mut tiles = Tiles::default();
    let root = tiles.insert_pane(TileKind::Graph(initial_view));
    let mut tree = Tree::new("graphshell_tiles", root, tiles);
    app.request_open_clip_by_id("clip-semantic");

    gui_orchestration::handle_pending_open_clip_after_intents(&mut app, &mut tree);

    assert!(app.take_pending_open_clip_request().is_none());
    assert!(active_tool_pane(&tree, ToolPaneState::HistoryManager));
}

#[test]
fn pending_clip_open_request_is_noop_when_queue_empty() {
    let mut app = GraphBrowserApp::new_for_testing();
    let initial_view = GraphViewId::new();
    let mut tiles = Tiles::default();
    let root = tiles.insert_pane(TileKind::Graph(initial_view));
    let mut tree = Tree::new("graphshell_tiles", root, tiles);

    gui_orchestration::handle_pending_open_clip_after_intents(&mut app, &mut tree);

    assert!(app.take_pending_open_clip_request().is_none());
}

#[test]
fn node_url_intent_opens_node_pane_via_orchestration_authority() {
    let mut app = GraphBrowserApp::new_for_testing();
    let node_key = app.add_node_and_sync(
        "https://example.com/node".to_string(),
        euclid::default::Point2D::new(16.0, 24.0),
    );
    let node_id = app
        .workspace
        .domain
        .graph
        .get_node(node_key)
        .expect("node should exist")
        .id;

    let initial_view = GraphViewId::new();
    let mut tiles = Tiles::default();
    let root = tiles.insert_pane(TileKind::Graph(initial_view));
    let mut tree = Tree::new("graphshell_tiles", root, tiles);
    let mut intents = vec![WorkbenchIntent::OpenNodeUrl {
        url: crate::util::NodeAddress::node(node_id.to_string()).to_string(),
    }];

    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut intents);

    assert!(intents.is_empty());
    assert_eq!(active_node_key(&tree), Some(node_key));
}

#[test]
fn invalid_node_url_intent_is_not_consumed_by_orchestration_authority() {
    let mut app = GraphBrowserApp::new_for_testing();
    let initial_view = GraphViewId::new();
    let mut tiles = Tiles::default();
    let root = tiles.insert_pane(TileKind::Graph(initial_view));
    let mut tree = Tree::new("graphshell_tiles", root, tiles);
    let unresolved_url = "node://not-a-uuid".to_string();
    let mut intents = vec![WorkbenchIntent::OpenNodeUrl {
        url: unresolved_url.clone(),
    }];

    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut intents);

    assert_eq!(intents.len(), 1);
    match &intents[0] {
        WorkbenchIntent::OpenNodeUrl { url } => assert_eq!(url, &unresolved_url),
        other => panic!("expected unresolved OpenNodeUrl intent, got {other:?}"),
    }
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

    let mut open_intents = vec![WorkbenchIntent::OpenSettingsUrl {
        url: crate::util::VersoAddress::settings(crate::util::GraphshellSettingsPath::General)
            .to_string(),
    }];
    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut open_intents);
    assert!(open_intents.is_empty());
    assert!(active_tool_pane(&tree, ToolPaneState::Settings));

    let mut close_intents = vec![WorkbenchIntent::CloseToolPane {
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

    let mut intents = vec![WorkbenchIntent::CycleFocusRegion];
    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut intents);
    assert!(intents.is_empty());
    assert!(
        tree.active_tiles().into_iter().any(|tile_id| {
            matches!(tree.tiles.get(tile_id), Some(Tile::Pane(TileKind::Node(_))))
        })
    );

    let mut intents = vec![WorkbenchIntent::CycleFocusRegion];
    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut intents);
    assert!(intents.is_empty());
    assert!(tree.active_tiles().into_iter().any(|tile_id| {
        matches!(
            tree.tiles.get(tile_id),
            Some(Tile::Pane(TileKind::Tool(ToolPaneState::Diagnostics)))
        )
    }));

    let mut intents = vec![WorkbenchIntent::CycleFocusRegion];
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
fn workbench_intent_dispatch_emits_ux_dispatch_channels() {
    let mut diagnostics = crate::shell::desktop::runtime::diagnostics::DiagnosticsState::new();
    let graph_view = GraphViewId::new();
    let mut tiles = Tiles::default();
    let graph = tiles.insert_pane(TileKind::Graph(graph_view));
    let root = tiles.insert_tab_tile(vec![graph]);
    let mut tree = Tree::new("ux_dispatch_channels", root, tiles);
    let mut app = GraphBrowserApp::new_for_testing();

    let mut intents = vec![WorkbenchIntent::CycleFocusRegion];
    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut intents);

    diagnostics.force_drain_for_tests();
    let snapshot = diagnostics.snapshot_json_for_tests().to_string();
    assert!(
        snapshot.contains(CHANNEL_UX_DISPATCH_STARTED),
        "expected ux:dispatch_started channel"
    );
    assert!(
        snapshot.contains(CHANNEL_UX_DISPATCH_PHASE),
        "expected ux:dispatch_phase channel"
    );
    assert!(
        snapshot.contains(CHANNEL_UX_DISPATCH_CONSUMED),
        "expected ux:dispatch_consumed channel"
    );
    assert!(
        snapshot.contains(CHANNEL_UX_DISPATCH_DEFAULT_PREVENTED),
        "expected ux:dispatch_default_prevented channel"
    );
}

#[cfg(feature = "diagnostics")]
#[test]
fn unresolved_workbench_intent_emits_contract_warning_for_default_fallback() {
    let mut diagnostics = crate::shell::desktop::runtime::diagnostics::DiagnosticsState::new();
    let graph_view = GraphViewId::new();
    let mut tiles = Tiles::default();
    let graph = tiles.insert_pane(TileKind::Graph(graph_view));
    let root = tiles.insert_tab_tile(vec![graph]);
    let mut tree = Tree::new("ux_dispatch_contract_warning", root, tiles);
    let mut app = GraphBrowserApp::new_for_testing();

    let mut intents = vec![WorkbenchIntent::OpenSettingsUrl {
        url: crate::util::VersoAddress::settings(crate::util::GraphshellSettingsPath::Other(
            "unknown".to_string(),
        ))
        .to_string(),
    }];
    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut intents);

    diagnostics.force_drain_for_tests();
    let snapshot_value = diagnostics.snapshot_json_for_tests();
    let snapshot = snapshot_value.to_string();
    let dispatch_phase_count = snapshot_value
        .get("channels")
        .and_then(|channels| channels.get("message_counts"))
        .and_then(|counts| counts.get(CHANNEL_UX_DISPATCH_PHASE))
        .and_then(|value| value.as_u64())
        .unwrap_or(0);
    assert!(
        snapshot.contains(CHANNEL_UX_CONTRACT_WARNING),
        "expected ux:contract_warning when unresolved workbench intent falls back"
    );
    assert!(
        dispatch_phase_count >= 3,
        "expected at least capture/target/bubble phases for fallback path"
    );
    assert_eq!(
        intents.len(),
        1,
        "fallback intent should remain for default handling"
    );
}

#[cfg(feature = "diagnostics")]
#[test]
fn modal_isolation_consumes_non_modal_workbench_intent() {
    let mut diagnostics = crate::shell::desktop::runtime::diagnostics::DiagnosticsState::new();
    let graph_view = GraphViewId::new();
    let mut tiles = Tiles::default();
    let graph = tiles.insert_pane(TileKind::Graph(graph_view));
    let root = tiles.insert_tab_tile(vec![graph]);
    let mut tree = Tree::new("ux_dispatch_modal_isolation", root, tiles);
    let mut app = GraphBrowserApp::new_for_testing();
    app.workspace.show_command_palette = true;

    let mut intents = vec![WorkbenchIntent::CycleFocusRegion];
    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut intents);

    diagnostics.force_drain_for_tests();
    let snapshot = diagnostics.snapshot_json_for_tests().to_string();
    assert!(
        snapshot.contains(CHANNEL_UX_DISPATCH_CONSUMED),
        "expected ux:dispatch_consumed when active modal isolates non-modal intents"
    );
    assert!(
        snapshot.contains(CHANNEL_UX_DISPATCH_DEFAULT_PREVENTED),
        "expected ux:dispatch_default_prevented when active modal consumes intent"
    );
    assert!(
        intents.is_empty(),
        "non-modal workbench intent should be consumed while modal surface is active"
    );
}

#[cfg(feature = "diagnostics")]
#[test]
fn global_shortcut_toggle_command_palette_traverses_dispatch_phases() {}

#[cfg(feature = "diagnostics")]
#[test]
fn cycle_focus_region_failure_emits_ux_navigation_violation_channel() {
    let mut diagnostics = crate::shell::desktop::runtime::diagnostics::DiagnosticsState::new();
    let graph_view = GraphViewId::new();
    let mut tiles = Tiles::default();
    let graph = tiles.insert_pane(TileKind::Graph(graph_view));
    let mut tree = Tree::new("ux_navigation_violation", graph, tiles);
    let mut app = GraphBrowserApp::new_for_testing();

    tree.root = None;
    let mut intents = vec![WorkbenchIntent::CycleFocusRegion];
    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut intents);

    diagnostics.force_drain_for_tests();
    let snapshot = diagnostics.snapshot_json_for_tests().to_string();
    assert!(
        snapshot.contains(CHANNEL_UX_NAVIGATION_VIOLATION),
        "expected ux:navigation_violation when focus cycle cannot resolve a target"
    );
}

#[cfg(feature = "diagnostics")]
#[test]
fn cycle_focus_region_success_does_not_emit_ux_navigation_violation_channel() {
    let mut diagnostics = crate::shell::desktop::runtime::diagnostics::DiagnosticsState::new();
    let graph_view = GraphViewId::new();
    let mut tiles = Tiles::default();
    let graph = tiles.insert_pane(TileKind::Graph(graph_view));
    let root = tiles.insert_tab_tile(vec![graph]);
    let mut tree = Tree::new("ux_navigation_no_violation", root, tiles);
    let mut app = GraphBrowserApp::new_for_testing();

    let mut intents = vec![WorkbenchIntent::CycleFocusRegion];
    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut intents);

    diagnostics.force_drain_for_tests();
    let snapshot = diagnostics.snapshot_json_for_tests().to_string();
    assert!(
        snapshot.contains(CHANNEL_UX_NAVIGATION_TRANSITION),
        "expected ux:navigation_transition when focus cycle resolves successfully"
    );
    assert!(
        !snapshot.contains(CHANNEL_UX_NAVIGATION_VIOLATION),
        "did not expect ux:navigation_violation when focus cycle resolves successfully"
    );
}

#[cfg(feature = "diagnostics")]
#[test]
fn open_tool_pane_emits_ux_navigation_transition_channel() {
    let mut diagnostics = crate::shell::desktop::runtime::diagnostics::DiagnosticsState::new();
    let graph_view = GraphViewId::new();
    let mut tiles = Tiles::default();
    let graph = tiles.insert_pane(TileKind::Graph(graph_view));
    let root = tiles.insert_tab_tile(vec![graph]);
    let mut tree = Tree::new("ux_navigation_transition_open_tool", root, tiles);
    let mut app = GraphBrowserApp::new_for_testing();

    let mut intents = vec![WorkbenchIntent::OpenToolPane {
        kind: ToolPaneState::Settings,
    }];
    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut intents);

    diagnostics.force_drain_for_tests();
    let snapshot = diagnostics.snapshot_json_for_tests().to_string();
    assert!(
        snapshot.contains(CHANNEL_UX_NAVIGATION_TRANSITION),
        "expected ux:navigation_transition when opening a tool pane changes focus region"
    );
    assert!(
        !snapshot.contains(CHANNEL_UX_NAVIGATION_VIOLATION),
        "did not expect ux:navigation_violation when opening a tool pane succeeds"
    );
}

#[cfg(feature = "diagnostics")]
#[test]
fn open_settings_url_emits_ux_navigation_transition_channel() {
    let mut diagnostics = crate::shell::desktop::runtime::diagnostics::DiagnosticsState::new();
    let graph_view = GraphViewId::new();
    let mut tiles = Tiles::default();
    let graph = tiles.insert_pane(TileKind::Graph(graph_view));
    let root = tiles.insert_tab_tile(vec![graph]);
    let mut tree = Tree::new("ux_navigation_transition_open_settings_url", root, tiles);
    let mut app = GraphBrowserApp::new_for_testing();

    let mut intents = vec![WorkbenchIntent::OpenSettingsUrl {
        url: crate::util::VersoAddress::settings(crate::util::GraphshellSettingsPath::History)
            .to_string(),
    }];
    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut intents);

    diagnostics.force_drain_for_tests();
    let snapshot = diagnostics.snapshot_json_for_tests().to_string();
    assert!(
        snapshot.contains(CHANNEL_UX_NAVIGATION_TRANSITION),
        "expected ux:navigation_transition when opening settings route changes focus region"
    );
    assert!(
        !snapshot.contains(CHANNEL_UX_NAVIGATION_VIOLATION),
        "did not expect ux:navigation_violation when opening settings route succeeds"
    );
}

#[cfg(feature = "diagnostics")]
#[test]
fn open_settings_url_already_focused_does_not_emit_ux_navigation_transition_channel() {
    let mut diagnostics = crate::shell::desktop::runtime::diagnostics::DiagnosticsState::new();
    let mut tiles = Tiles::default();
    let settings = tiles.insert_pane(TileKind::Tool(ToolPaneState::Settings));
    let root = tiles.insert_tab_tile(vec![settings]);
    let mut tree = Tree::new(
        "ux_navigation_transition_open_settings_url_noop",
        root,
        tiles,
    );
    let mut app = GraphBrowserApp::new_for_testing();

    let _ = tree
        .make_active(|_, tile| matches!(tile, Tile::Pane(TileKind::Tool(ToolPaneState::Settings))));

    let mut intents = vec![WorkbenchIntent::OpenSettingsUrl {
        url: crate::util::VersoAddress::settings(crate::util::GraphshellSettingsPath::General)
            .to_string(),
    }];
    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut intents);

    diagnostics.force_drain_for_tests();
    let snapshot = diagnostics.snapshot_json_for_tests().to_string();
    assert!(
        !snapshot.contains(CHANNEL_UX_NAVIGATION_TRANSITION),
        "did not expect ux:navigation_transition when settings route target is already focused"
    );
    assert!(
        !snapshot.contains(CHANNEL_UX_NAVIGATION_VIOLATION),
        "did not expect ux:navigation_violation when no settings focus handoff is needed"
    );
}

#[cfg(feature = "diagnostics")]
#[test]
fn open_graph_url_emits_open_decision_diagnostics_for_routed_target() {
    let mut diagnostics = crate::shell::desktop::runtime::diagnostics::DiagnosticsState::new();
    let dir = TempDir::new().expect("temp dir should be created");
    let mut app = GraphBrowserApp::new_from_dir(dir.path().to_path_buf());
    app.add_node_and_sync(
        "https://example.com/graph-seed".to_string(),
        euclid::default::Point2D::new(0.0, 0.0),
    );
    app.save_named_graph_snapshot("graph-main")
        .expect("named graph snapshot should save");

    let graph_view = GraphViewId::new();
    let mut tiles = Tiles::default();
    let graph = tiles.insert_pane(TileKind::Graph(graph_view));
    let root = tiles.insert_tab_tile(vec![graph]);
    let mut tree = Tree::new("ux_open_decision_graph_routed", root, tiles);

    let mut intents = vec![WorkbenchIntent::OpenGraphUrl {
        url: crate::util::GraphAddress::graph("graph-main").to_string(),
    }];
    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut intents);

    diagnostics.force_drain_for_tests();
    let snapshot = diagnostics.snapshot_json_for_tests().to_string();
    assert!(snapshot.contains(CHANNEL_UX_OPEN_DECISION_PATH));
    assert!(snapshot.contains(CHANNEL_UX_OPEN_DECISION_REASON));
    assert!(intents.is_empty());
}

#[cfg(feature = "diagnostics")]
#[test]
fn unresolved_graph_url_emits_open_decision_diagnostics_for_fallback_path() {
    let mut diagnostics = crate::shell::desktop::runtime::diagnostics::DiagnosticsState::new();
    let mut app = GraphBrowserApp::new_for_testing();
    let graph_view = GraphViewId::new();
    let mut tiles = Tiles::default();
    let graph = tiles.insert_pane(TileKind::Graph(graph_view));
    let root = tiles.insert_tab_tile(vec![graph]);
    let mut tree = Tree::new("ux_open_decision_graph_unresolved", root, tiles);

    let mut intents = vec![WorkbenchIntent::OpenGraphUrl {
        url: crate::util::GraphAddress::graph("missing-graph").to_string(),
    }];
    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut intents);

    diagnostics.force_drain_for_tests();
    let snapshot = diagnostics.snapshot_json_for_tests().to_string();
    assert!(snapshot.contains(CHANNEL_UX_OPEN_DECISION_PATH));
    assert!(snapshot.contains(CHANNEL_UX_OPEN_DECISION_REASON));
    assert!(snapshot.contains(CHANNEL_UX_CONTRACT_WARNING));
    assert_eq!(
        intents.len(),
        1,
        "unresolved open intent should remain for fallback"
    );
}

#[cfg(feature = "diagnostics")]
#[test]
fn open_tool_pane_already_focused_does_not_emit_ux_navigation_transition_channel() {
    let mut diagnostics = crate::shell::desktop::runtime::diagnostics::DiagnosticsState::new();
    let mut tiles = Tiles::default();
    let settings = tiles.insert_pane(TileKind::Tool(ToolPaneState::Settings));
    let root = tiles.insert_tab_tile(vec![settings]);
    let mut tree = Tree::new("ux_navigation_transition_open_tool_noop", root, tiles);
    let mut app = GraphBrowserApp::new_for_testing();

    let _ = tree
        .make_active(|_, tile| matches!(tile, Tile::Pane(TileKind::Tool(ToolPaneState::Settings))));

    let mut intents = vec![WorkbenchIntent::OpenToolPane {
        kind: ToolPaneState::Settings,
    }];
    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut intents);

    diagnostics.force_drain_for_tests();
    let snapshot = diagnostics.snapshot_json_for_tests().to_string();
    assert!(
        !snapshot.contains(CHANNEL_UX_NAVIGATION_TRANSITION),
        "did not expect ux:navigation_transition when opening an already focused tool pane"
    );
    assert!(
        !snapshot.contains(CHANNEL_UX_NAVIGATION_VIOLATION),
        "did not expect ux:navigation_violation when no focus handoff is needed"
    );
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

    let _ = tree.make_active(
        |_, tile| matches!(tile, Tile::Pane(TileKind::Node(state)) if state.node == focus_node),
    );

    let mut open_intents = vec![WorkbenchIntent::OpenToolPane {
        kind: ToolPaneState::HistoryManager,
    }];
    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut open_intents);
    assert!(open_intents.is_empty());
    assert!(active_tool_pane(&tree, ToolPaneState::HistoryManager));

    let mut close_intents = vec![WorkbenchIntent::CloseToolPane {
        kind: ToolPaneState::HistoryManager,
        restore_previous_focus: true,
    }];
    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut close_intents);

    assert!(close_intents.is_empty());
    assert_eq!(active_node_key(&tree), Some(focus_node));
}

#[cfg(feature = "diagnostics")]
#[test]
fn close_tool_pane_restore_failure_emits_ux_navigation_violation_channel() {
    let mut diagnostics = crate::shell::desktop::runtime::diagnostics::DiagnosticsState::new();
    let mut app = GraphBrowserApp::new_for_testing();
    let mut tiles = Tiles::default();
    let tool = tiles.insert_pane(TileKind::Tool(ToolPaneState::Settings));
    let mut tree = Tree::new("restore_failure_violation", tool, tiles);

    app.set_pending_tool_surface_return_target(Some(crate::app::ToolSurfaceReturnTarget::Graph(
        GraphViewId::new(),
    )));

    let mut close_intents = vec![WorkbenchIntent::CloseToolPane {
        kind: ToolPaneState::Settings,
        restore_previous_focus: true,
    }];
    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut close_intents);

    diagnostics.force_drain_for_tests();
    let snapshot = diagnostics.snapshot_json_for_tests().to_string();
    assert!(
        !snapshot.contains(CHANNEL_UX_NAVIGATION_TRANSITION),
        "did not expect ux:navigation_transition when restore path cannot resolve a focus target"
    );
    assert!(
        snapshot.contains(CHANNEL_UX_NAVIGATION_VIOLATION),
        "expected ux:navigation_violation when restore path cannot resolve a focus target"
    );
}

#[cfg(feature = "diagnostics")]
#[test]
fn close_tool_pane_restore_success_does_not_emit_ux_navigation_violation_channel() {
    let mut diagnostics = crate::shell::desktop::runtime::diagnostics::DiagnosticsState::new();
    let mut app = GraphBrowserApp::new_for_testing();
    let graph_view = GraphViewId::new();
    let mut tiles = Tiles::default();
    let graph = tiles.insert_pane(TileKind::Graph(graph_view));
    let settings = tiles.insert_pane(TileKind::Tool(ToolPaneState::Settings));
    let root = tiles.insert_tab_tile(vec![graph, settings]);
    let mut tree = Tree::new("restore_success_no_violation", root, tiles);

    let _ = tree
        .make_active(|_, tile| matches!(tile, Tile::Pane(TileKind::Tool(ToolPaneState::Settings))));
    app.set_pending_tool_surface_return_target(Some(crate::app::ToolSurfaceReturnTarget::Graph(
        graph_view,
    )));

    let mut close_intents = vec![WorkbenchIntent::CloseToolPane {
        kind: ToolPaneState::Settings,
        restore_previous_focus: true,
    }];
    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut close_intents);

    diagnostics.force_drain_for_tests();
    let snapshot = diagnostics.snapshot_json_for_tests().to_string();
    assert!(
        snapshot.contains(CHANNEL_UX_NAVIGATION_TRANSITION),
        "expected ux:navigation_transition when restore path resolves successfully"
    );
    assert!(
        !snapshot.contains(CHANNEL_UX_NAVIGATION_VIOLATION),
        "did not expect ux:navigation_violation when restore path resolves successfully"
    );
}

#[cfg(feature = "diagnostics")]
#[test]
fn close_tool_pane_without_restore_clears_pending_return_target() {
    let mut app = GraphBrowserApp::new_for_testing();
    let graph_view = GraphViewId::new();
    let mut tiles = Tiles::default();
    let graph = tiles.insert_pane(TileKind::Graph(graph_view));
    let settings = tiles.insert_pane(TileKind::Tool(ToolPaneState::Settings));
    let root = tiles.insert_tab_tile(vec![graph, settings]);
    let mut tree = Tree::new("clear_pending_return_target", root, tiles);

    app.set_pending_tool_surface_return_target(Some(crate::app::ToolSurfaceReturnTarget::Graph(
        graph_view,
    )));

    let mut close_intents = vec![WorkbenchIntent::CloseToolPane {
        kind: ToolPaneState::Settings,
        restore_previous_focus: false,
    }];
    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut close_intents);

    assert!(app.pending_tool_surface_return_target().is_none());
}

#[cfg(feature = "diagnostics")]
#[test]
fn close_tool_pane_without_restore_keeps_pending_target_when_close_fails() {
    let mut app = GraphBrowserApp::new_for_testing();
    let graph_view = GraphViewId::new();
    let mut tiles = Tiles::default();
    let graph = tiles.insert_pane(TileKind::Graph(graph_view));
    let root = tiles.insert_tab_tile(vec![graph]);
    let mut tree = Tree::new("keep_pending_target_when_close_fails", root, tiles);

    app.set_pending_tool_surface_return_target(Some(crate::app::ToolSurfaceReturnTarget::Graph(
        graph_view,
    )));

    let mut close_intents = vec![WorkbenchIntent::CloseToolPane {
        kind: ToolPaneState::Settings,
        restore_previous_focus: false,
    }];
    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut close_intents);

    assert_eq!(
        app.pending_tool_surface_return_target(),
        Some(crate::app::ToolSurfaceReturnTarget::Graph(graph_view))
    );
}

#[cfg(feature = "diagnostics")]
#[test]
fn close_tool_pane_restore_requested_but_close_fails_emits_ux_navigation_violation() {
    let mut diagnostics = crate::shell::desktop::runtime::diagnostics::DiagnosticsState::new();
    let mut app = GraphBrowserApp::new_for_testing();
    let graph_view = GraphViewId::new();
    let mut tiles = Tiles::default();
    let graph = tiles.insert_pane(TileKind::Graph(graph_view));
    let root = tiles.insert_tab_tile(vec![graph]);
    let mut tree = Tree::new("restore_requested_close_fail_violation", root, tiles);

    let mut close_intents = vec![WorkbenchIntent::CloseToolPane {
        kind: ToolPaneState::Settings,
        restore_previous_focus: true,
    }];
    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut close_intents);

    diagnostics.force_drain_for_tests();
    let snapshot = diagnostics.snapshot_json_for_tests().to_string();
    assert!(
        snapshot.contains(CHANNEL_UX_NAVIGATION_VIOLATION),
        "expected ux:navigation_violation when restore was requested but close failed"
    );
}

#[cfg(feature = "diagnostics")]
#[test]
fn close_tool_pane_without_restore_and_close_fails_does_not_emit_ux_navigation_violation() {
    let mut diagnostics = crate::shell::desktop::runtime::diagnostics::DiagnosticsState::new();
    let mut app = GraphBrowserApp::new_for_testing();
    let graph_view = GraphViewId::new();
    let mut tiles = Tiles::default();
    let graph = tiles.insert_pane(TileKind::Graph(graph_view));
    let root = tiles.insert_tab_tile(vec![graph]);
    let mut tree = Tree::new("no_restore_close_fail_no_violation", root, tiles);

    let mut close_intents = vec![WorkbenchIntent::CloseToolPane {
        kind: ToolPaneState::Settings,
        restore_previous_focus: false,
    }];
    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut close_intents);

    diagnostics.force_drain_for_tests();
    let snapshot = diagnostics.snapshot_json_for_tests().to_string();
    assert!(
        !snapshot.contains(CHANNEL_UX_NAVIGATION_TRANSITION),
        "did not expect ux:navigation_transition when close fails without restore request"
    );
    assert!(
        !snapshot.contains(CHANNEL_UX_NAVIGATION_VIOLATION),
        "did not expect ux:navigation_violation when close failed without restore request"
    );
}

#[cfg(feature = "diagnostics")]
#[test]
fn close_tool_pane_without_restore_and_close_succeeds_emits_ux_navigation_transition() {
    let mut diagnostics = crate::shell::desktop::runtime::diagnostics::DiagnosticsState::new();
    let mut app = GraphBrowserApp::new_for_testing();
    let graph_view = GraphViewId::new();
    let mut tiles = Tiles::default();
    let graph = tiles.insert_pane(TileKind::Graph(graph_view));
    let settings = tiles.insert_pane(TileKind::Tool(ToolPaneState::Settings));
    let root = tiles.insert_tab_tile(vec![graph, settings]);
    let mut tree = Tree::new("close_without_restore_transition", root, tiles);

    let _ = tree
        .make_active(|_, tile| matches!(tile, Tile::Pane(TileKind::Tool(ToolPaneState::Settings))));

    let mut close_intents = vec![WorkbenchIntent::CloseToolPane {
        kind: ToolPaneState::Settings,
        restore_previous_focus: false,
    }];
    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut close_intents);

    diagnostics.force_drain_for_tests();
    let snapshot = diagnostics.snapshot_json_for_tests().to_string();
    assert!(
        snapshot.contains(CHANNEL_UX_NAVIGATION_TRANSITION),
        "expected ux:navigation_transition when close succeeds without restore and focus handoff resolves"
    );
    assert!(
        !snapshot.contains(CHANNEL_UX_NAVIGATION_VIOLATION),
        "did not expect ux:navigation_violation when close succeeds without restore"
    );
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
    let mut open_node_tile_after_intents =
        Some(crate::shell::desktop::workbench::tile_view_ops::TileOpenMode::Tab);

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

    app.apply_reducer_intents(std::mem::take(&mut frame_intents));
    assert_eq!(app.get_single_selected_node(), Some(selected));
}

#[test]
fn frame_loop_drains_workbench_intents_before_reducer_apply() {
    let mut app = GraphBrowserApp::new_for_testing();
    let node_key = app.add_node_and_sync(
        "https://before.example".to_string(),
        euclid::default::Point2D::new(5.0, 5.0),
    );

    let mut tiles = Tiles::default();
    let root = tiles.insert_pane(TileKind::Graph(GraphViewId::new()));
    let mut tree = Tree::new("graphshell_tiles", root, tiles);
    let mut open_node_tile_after_intents = None;

    app.enqueue_workbench_intent(WorkbenchIntent::CycleFocusRegion);

    let mut frame_intents = vec![GraphIntent::SetNodeUrl {
        key: node_key,
        new_url: "https://after.example".to_string(),
    }];

    super::apply_semantic_intents_and_pending_open(
        &mut app,
        &mut tree,
        false,
        &mut open_node_tile_after_intents,
        &mut frame_intents,
    );

    assert_eq!(app.pending_workbench_intent_count_for_tests(), 0);
    assert!(frame_intents.is_empty());

    assert_eq!(
        app.workspace
            .domain
            .graph
            .get_node(node_key)
            .expect("node should exist")
            .url,
        "https://after.example"
    );
}

#[test]
#[should_panic(
    expected = "workbench intents leaked past workbench-authority interception before reducer apply"
)]
fn frame_loop_panics_when_workbench_intent_leaks_past_interception() {
    let mut app = GraphBrowserApp::new_for_testing();

    let mut tiles = Tiles::default();
    let root = tiles.insert_pane(TileKind::Graph(GraphViewId::new()));
    let mut tree = Tree::new("graphshell_tiles", root, tiles);
    let mut open_node_tile_after_intents = None;
    let mut frame_intents = Vec::new();

    app.enqueue_workbench_intent(WorkbenchIntent::OpenSettingsUrl {
        url: "verso://settings/not-a-real-route".to_string(),
    });

    super::apply_semantic_intents_and_pending_open(
        &mut app,
        &mut tree,
        false,
        &mut open_node_tile_after_intents,
        &mut frame_intents,
    );
}

#[test]
fn clipboard_success_status_text_is_deterministic_per_copy_kind() {
    assert_eq!(
        super::clipboard_copy_success_text(crate::app::ClipboardCopyKind::Url),
        super::CLIPBOARD_STATUS_SUCCESS_URL_TEXT
    );
    assert_eq!(
        super::clipboard_copy_success_text(crate::app::ClipboardCopyKind::Title),
        super::CLIPBOARD_STATUS_SUCCESS_TITLE_TEXT
    );
}

#[test]
fn clipboard_status_messages_describe_outcomes_explicitly() {
    assert!(super::CLIPBOARD_STATUS_SUCCESS_URL_TEXT.contains("Copied"));
    assert!(super::CLIPBOARD_STATUS_SUCCESS_TITLE_TEXT.contains("Copied"));
    assert!(super::CLIPBOARD_STATUS_UNAVAILABLE_TEXT.contains("unavailable"));
    assert!(super::CLIPBOARD_STATUS_EMPTY_TEXT.contains("Nothing"));
    assert!(super::CLIPBOARD_STATUS_FAILURE_PREFIX.contains("failed"));
}

#[test]
fn clipboard_status_success_messages_identify_copied_subject() {
    assert!(super::CLIPBOARD_STATUS_SUCCESS_URL_TEXT.contains("URL"));
    assert!(super::CLIPBOARD_STATUS_SUCCESS_TITLE_TEXT.contains("title"));
}

#[test]
fn clipboard_failure_message_prefix_is_stable_and_identifiable() {
    let text = super::clipboard_copy_failure_text("permission denied");
    assert!(text.starts_with(super::CLIPBOARD_STATUS_FAILURE_PREFIX));
    assert!(text.contains("permission denied"));
}

#[test]
fn clipboard_missing_node_failure_message_is_explicit() {
    let text = super::clipboard_copy_missing_node_failure_text();
    assert!(text.contains("node no longer exists"));
    assert!(text.contains(super::CLIPBOARD_STATUS_FAILURE_PREFIX));
}

#[test]
fn clipboard_missing_node_failure_message_includes_recovery_suggestion() {
    let text = super::clipboard_copy_missing_node_failure_text();
    assert!(text.contains("try again"));
    assert!(text.contains("select a node"));
}
