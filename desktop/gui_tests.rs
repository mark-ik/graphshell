use super::*;
use egui_tiles::{Container, Tile, TileId, Tiles, Tree};
use crate::window::GraphSemanticEventKind;

/// Create a unique WebViewId for testing.
fn test_webview_id() -> servo::WebViewId {
        thread_local! {
            static NS_INSTALLED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
        }
        NS_INSTALLED.with(|cell| {
            if !cell.get() {
                base::id::PipelineNamespace::install(base::id::PipelineNamespaceId(44));
                cell.set(true);
            }
        });
        servo::WebViewId::new(base::id::PainterId::next())
}

fn event(kind: GraphSemanticEventKind) -> GraphSemanticEvent {
        static NEXT_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        GraphSemanticEvent {
            seq: NEXT_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            kind,
        }
}

fn tree_with_graph_root() -> Tree<TileKind> {
        let mut tiles = Tiles::default();
        let graph_tile_id = tiles.insert_pane(TileKind::Graph(crate::app::GraphViewId::default()));
        Tree::new("test_tree", graph_tile_id, tiles)
}

fn webview_tile_count(tiles_tree: &Tree<TileKind>, node_key: NodeKey) -> usize {
        tiles_tree
            .tiles
            .iter()
            .filter(
                |(_, tile)| matches!(tile, Tile::Pane(TileKind::WebView(key)) if *key == node_key),
            )
            .count()
}

fn has_any_webview_tiles_in(tiles_tree: &Tree<TileKind>) -> bool {
        tile_runtime::has_any_webview_tiles(tiles_tree)
}

fn open_or_focus_webview_tile(tiles_tree: &mut Tree<TileKind>, node_key: NodeKey) {
        tile_view_ops::open_or_focus_webview_tile(tiles_tree, node_key);
}

fn open_or_focus_webview_tile_with_mode(
        tiles_tree: &mut Tree<TileKind>,
        node_key: NodeKey,
        mode: TileOpenMode,
) {
        tile_view_ops::open_or_focus_webview_tile_with_mode(tiles_tree, node_key, mode);
}

fn all_webview_tile_nodes(tiles_tree: &Tree<TileKind>) -> HashSet<NodeKey> {
        tile_runtime::all_webview_tile_nodes(tiles_tree)
}

fn remove_all_webview_tiles(tiles_tree: &mut Tree<TileKind>) {
        tile_runtime::remove_all_webview_tiles(tiles_tree);
}

fn remove_webview_tile_for_node(tiles_tree: &mut Tree<TileKind>, node_key: NodeKey) {
        tile_runtime::remove_webview_tile_for_node(tiles_tree, node_key);
}

fn prune_stale_webview_tile_keys_only(
        tiles_tree: &mut Tree<TileKind>,
        graph_app: &GraphBrowserApp,
) {
        tile_runtime::prune_stale_webview_tile_keys_only(tiles_tree, graph_app);
}

fn focused_webview_id_for_tree(
        tiles_tree: &Tree<TileKind>,
        graph_app: &GraphBrowserApp,
        focused_hint: Option<WebViewId>,
) -> Option<WebViewId> {
        tile_compositor::focused_webview_id_for_tree(tiles_tree, graph_app, focused_hint)
}

fn webview_for_frame_activation(
        tiles_tree: &Tree<TileKind>,
        graph_app: &GraphBrowserApp,
        focused_hint: Option<WebViewId>,
) -> Option<WebViewId> {
        tile_compositor::webview_for_frame_activation(tiles_tree, graph_app, focused_hint)
}

#[test]
fn test_open_webview_tile_creates_tabs_container() {
        let mut tree = tree_with_graph_root();
        let node_key = NodeKey::new(1);

        open_or_focus_webview_tile(&mut tree, node_key);

        assert!(has_any_webview_tiles_in(&tree));
        let root_id = tree.root().expect("root tile should exist");
        match tree.tiles.get(root_id) {
            Some(Tile::Container(Container::Tabs(tabs))) => {
                assert_eq!(tabs.children.len(), 2);
            },
            _ => panic!("expected tabs container root"),
        }
}

#[test]
fn test_open_duplicate_tile_focuses_existing() {
        let mut tree = tree_with_graph_root();
        let node_key = NodeKey::new(7);

        open_or_focus_webview_tile(&mut tree, node_key);
        open_or_focus_webview_tile(&mut tree, node_key);

        assert_eq!(webview_tile_count(&tree, node_key), 1);
}

#[test]
fn test_open_webview_tile_split_creates_horizontal_root() {
        let mut tree = tree_with_graph_root();
        let node_key = NodeKey::new(42);

        open_or_focus_webview_tile_with_mode(&mut tree, node_key, TileOpenMode::SplitHorizontal);

        let root_id = tree.root().expect("root tile should exist");
        match tree.tiles.get(root_id) {
            Some(Tile::Container(Container::Linear(linear))) => {
                assert_eq!(linear.children.len(), 2);
            },
            _ => panic!("expected horizontal split container root"),
        }
}

#[test]
fn test_open_webview_tile_split_reuses_existing_linear_root() {
        let mut tree = tree_with_graph_root();
        let a = NodeKey::new(40);
        let b = NodeKey::new(41);

        open_or_focus_webview_tile_with_mode(&mut tree, a, TileOpenMode::SplitHorizontal);
        let root_before = tree.root().expect("root should exist");
        open_or_focus_webview_tile_with_mode(&mut tree, b, TileOpenMode::SplitHorizontal);
        let root_after = tree.root().expect("root should exist");

        assert_eq!(root_before, root_after);
        match tree.tiles.get(root_after) {
            Some(Tile::Container(Container::Linear(linear))) => {
                assert_eq!(linear.children.len(), 3);
            },
            _ => panic!("expected linear root container"),
        }
}

#[test]
fn test_close_last_webview_tile_leaves_graph_only() {
        let mut tree = tree_with_graph_root();
        let node_key = NodeKey::new(3);
        open_or_focus_webview_tile(&mut tree, node_key);

        remove_all_webview_tiles(&mut tree);

        assert!(!has_any_webview_tiles_in(&tree));
        let has_graph_pane = tree
            .tiles
            .iter()
            .any(|(_, tile)| matches!(tile, Tile::Pane(TileKind::Graph(_))));
        assert!(has_graph_pane);
}

#[test]
fn test_all_webview_tile_nodes_tracks_correctly() {
        let mut tree = tree_with_graph_root();
        let a = NodeKey::new(1);
        let b = NodeKey::new(2);
        open_or_focus_webview_tile(&mut tree, a);
        open_or_focus_webview_tile(&mut tree, b);

        let nodes = all_webview_tile_nodes(&tree);
        assert_eq!(nodes.len(), 2);
        assert!(nodes.contains(&a));
        assert!(nodes.contains(&b));
}

#[test]
fn test_focused_webview_id_for_tree_prefers_active_hint() {
        let mut app = GraphBrowserApp::new_for_testing();
        let a = app.add_node_and_sync("https://a.example".into(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("https://b.example".into(), Point2D::new(10.0, 0.0));
        let a_id = test_webview_id();
        let b_id = test_webview_id();
        app.map_webview_to_node(a_id, a);
        app.map_webview_to_node(b_id, b);

        let mut tiles = Tiles::default();
        let a_tile = tiles.insert_pane(TileKind::WebView(a));
        let b_tile = tiles.insert_pane(TileKind::WebView(b));
        let root = tiles.insert_horizontal_tile(vec![a_tile, b_tile]);
        let tree = Tree::new("focus_hint_test", root, tiles);

        let focused = focused_webview_id_for_tree(&tree, &app, Some(b_id));
        assert_eq!(focused, Some(b_id));
}

#[test]
fn test_focused_webview_id_for_tree_prefers_hint_when_tile_still_present() {
        let mut app = GraphBrowserApp::new_for_testing();
        let a = app.add_node_and_sync("https://a.example".into(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("https://b.example".into(), Point2D::new(10.0, 0.0));
        let a_id = test_webview_id();
        let b_id = test_webview_id();
        app.map_webview_to_node(a_id, a);
        app.map_webview_to_node(b_id, b);

        let mut tiles = Tiles::default();
        let a_tile = tiles.insert_pane(TileKind::WebView(a));
        let b_tile = tiles.insert_pane(TileKind::WebView(b));
        let root = tiles.insert_tab_tile(vec![a_tile, b_tile]);
        let mut tree = Tree::new("focus_hint_tab_test", root, tiles);
        let _ = tree.make_active(|tile_id, _| tile_id == a_tile);

        let focused = focused_webview_id_for_tree(&tree, &app, Some(b_id));
        assert_eq!(focused, Some(b_id));
}

#[test]
fn test_get_focused_webview_semantics_use_tile_focused_hint() {
        let mut app = GraphBrowserApp::new_for_testing();
        let a = app.add_node_and_sync("https://a.example".into(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("https://b.example".into(), Point2D::new(10.0, 0.0));
        let a_id = test_webview_id();
        let b_id = test_webview_id();
        app.map_webview_to_node(a_id, a);
        app.map_webview_to_node(b_id, b);

        let mut tiles = Tiles::default();
        let a_tile = tiles.insert_pane(TileKind::WebView(a));
        let b_tile = tiles.insert_pane(TileKind::WebView(b));
        let root = tiles.insert_horizontal_tile(vec![a_tile, b_tile]);
        let tree = Tree::new("webdriver_get_focused_semantics", root, tiles);

        // Mirrors WebDriver GetFocusedWebView semantics in desktop graphshell:
        // preferred input target should resolve to the tile-focused webview.
        let focused = focused_webview_id_for_tree(&tree, &app, Some(b_id));
        assert_eq!(focused, Some(b_id));
}

#[test]
fn test_focused_webview_id_for_tree_graph_only_returns_none() {
        let app = GraphBrowserApp::new_for_testing();
        let tree = tree_with_graph_root();
        let stale_hint = test_webview_id();

        let focused = focused_webview_id_for_tree(&tree, &app, Some(stale_hint));
        assert_eq!(focused, None);
}

#[test]
fn test_webview_for_frame_activation_prefers_active_hint() {
        let mut app = GraphBrowserApp::new_for_testing();
        let a = app.add_node_and_sync("https://a.example".into(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("https://b.example".into(), Point2D::new(10.0, 0.0));
        let a_id = test_webview_id();
        let b_id = test_webview_id();
        app.map_webview_to_node(a_id, a);
        app.map_webview_to_node(b_id, b);

        let mut tiles = Tiles::default();
        let a_tile = tiles.insert_pane(TileKind::WebView(a));
        let b_tile = tiles.insert_pane(TileKind::WebView(b));
        let root = tiles.insert_horizontal_tile(vec![a_tile, b_tile]);
        let tree = Tree::new("frame_activation_hint_test", root, tiles);

        let chosen = webview_for_frame_activation(&tree, &app, Some(b_id));
        assert_eq!(chosen, Some(b_id));
}

#[test]
fn test_webview_for_frame_activation_prefers_hint_when_tile_still_present() {
        let mut app = GraphBrowserApp::new_for_testing();
        let a = app.add_node_and_sync("https://a.example".into(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("https://b.example".into(), Point2D::new(10.0, 0.0));
        let a_id = test_webview_id();
        let b_id = test_webview_id();
        app.map_webview_to_node(a_id, a);
        app.map_webview_to_node(b_id, b);

        let mut tiles = Tiles::default();
        let a_tile = tiles.insert_pane(TileKind::WebView(a));
        let b_tile = tiles.insert_pane(TileKind::WebView(b));
        let root = tiles.insert_tab_tile(vec![a_tile, b_tile]);
        let mut tree = Tree::new("frame_activation_active_test", root, tiles);
        let _ = tree.make_active(|tile_id, _| tile_id == a_tile);

        let chosen = webview_for_frame_activation(&tree, &app, Some(b_id));
        assert_eq!(chosen, Some(b_id));
}

#[test]
fn test_split_layout_keeps_both_webview_tiles_visible_when_focus_changes() {
        let mut app = GraphBrowserApp::new_for_testing();
        let a = app.add_node_and_sync("https://a.example".into(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("https://b.example".into(), Point2D::new(10.0, 0.0));
        let a_id = test_webview_id();
        let b_id = test_webview_id();
        app.map_webview_to_node(a_id, a);
        app.map_webview_to_node(b_id, b);

        let mut tree = tree_with_graph_root();
        open_or_focus_webview_tile_with_mode(&mut tree, a, TileOpenMode::SplitHorizontal);
        open_or_focus_webview_tile_with_mode(&mut tree, b, TileOpenMode::SplitHorizontal);

        let initial_nodes = all_webview_tile_nodes(&tree);
        assert_eq!(initial_nodes.len(), 2);
        assert!(initial_nodes.contains(&a));
        assert!(initial_nodes.contains(&b));

        let first = webview_for_frame_activation(&tree, &app, Some(a_id));
        let second = webview_for_frame_activation(&tree, &app, Some(b_id));
        assert_eq!(first, Some(a_id));
        assert_eq!(second, Some(b_id));

        let after_nodes = all_webview_tile_nodes(&tree);
        assert_eq!(after_nodes, initial_nodes);
}

#[test]
fn test_split_layout_reports_two_active_webview_tile_rect_entries() {
        let mut tree = tree_with_graph_root();
        let a = NodeKey::new(61);
        let b = NodeKey::new(62);
        open_or_focus_webview_tile_with_mode(&mut tree, a, TileOpenMode::SplitHorizontal);
        open_or_focus_webview_tile_with_mode(&mut tree, b, TileOpenMode::SplitHorizontal);

        // In unit tests without an egui layout pass, rectangles are unset. We still
        // assert tile presence as the precondition for two-pane compositing.
        let nodes = all_webview_tile_nodes(&tree);
        assert_eq!(nodes.len(), 2);
        assert!(nodes.contains(&a));
        assert!(nodes.contains(&b));
}

#[test]
fn test_user_grouped_intents_for_tab_group_moves_emits_on_group_change() {
        let moved = NodeKey::new(70);
        let anchor = NodeKey::new(71);
        let old_group = TileId::from_u64(1);
        let new_group = TileId::from_u64(2);

        let mut before = HashMap::new();
        before.insert(moved, old_group);

        let mut after = HashMap::new();
        after.insert(moved, new_group);

        let mut after_nodes = HashMap::new();
        after_nodes.insert(new_group, vec![anchor, moved]);
        let moved_nodes = HashSet::from([moved]);

        let intents = tile_grouping::user_grouped_intents_for_tab_group_moves(
            &before,
            &after,
            &after_nodes,
            &moved_nodes,
        );
        assert_eq!(intents.len(), 1);
        match intents[0] {
            GraphIntent::CreateUserGroupedEdge { from, to } => {
                assert_eq!(from, moved);
                assert_eq!(to, anchor);
            },
            _ => panic!("expected CreateUserGroupedEdge"),
        }
}

#[test]
fn test_user_grouped_intents_for_tab_group_moves_ignores_same_group_or_no_peer() {
        let node = NodeKey::new(80);
        let group = TileId::from_u64(3);

        let mut before = HashMap::new();
        before.insert(node, group);

        let mut after = HashMap::new();
        after.insert(node, group);

        let mut after_nodes = HashMap::new();
        after_nodes.insert(group, vec![node]);
        let moved_nodes = HashSet::from([node]);

        let intents = tile_grouping::user_grouped_intents_for_tab_group_moves(
            &before,
            &after,
            &after_nodes,
            &moved_nodes,
        );
        assert!(intents.is_empty());

        let moved_group = TileId::from_u64(4);
        after.insert(node, moved_group);
        after_nodes.insert(moved_group, vec![node]);
        let intents = tile_grouping::user_grouped_intents_for_tab_group_moves(
            &before,
            &after,
            &after_nodes,
            &moved_nodes,
        );
        assert!(intents.is_empty());
}

#[test]
fn test_open_or_focus_sets_active_tile_to_target_node() {
        let mut tree = tree_with_graph_root();
        let a = NodeKey::new(10);
        let b = NodeKey::new(11);
        open_or_focus_webview_tile(&mut tree, a);
        open_or_focus_webview_tile(&mut tree, b);

        assert_eq!(Gui::active_webview_tile_node(&tree), Some(b));

        open_or_focus_webview_tile(&mut tree, a);
        assert_eq!(Gui::active_webview_tile_node(&tree), Some(a));
        assert_eq!(webview_tile_count(&tree, a), 1);
        assert_eq!(webview_tile_count(&tree, b), 1);
}

#[test]
fn test_remove_webview_tile_for_node_preserves_other_tiles() {
        let mut tree = tree_with_graph_root();
        let a = NodeKey::new(20);
        let b = NodeKey::new(21);
        open_or_focus_webview_tile(&mut tree, a);
        open_or_focus_webview_tile(&mut tree, b);

        remove_webview_tile_for_node(&mut tree, a);
        let nodes = all_webview_tile_nodes(&tree);
        assert!(!nodes.contains(&a));
        assert!(nodes.contains(&b));
}

#[test]
fn test_stale_node_cleanup_removes_tile() {
        let mut app = GraphBrowserApp::new_for_testing();
        let alive_key =
            app.add_node_and_sync("https://alive.example".into(), Point2D::new(0.0, 0.0));
        let stale_key = NodeKey::new(9999);
        let mut tree = tree_with_graph_root();
        open_or_focus_webview_tile(&mut tree, alive_key);
        open_or_focus_webview_tile(&mut tree, stale_key);

        prune_stale_webview_tile_keys_only(&mut tree, &app);
        let nodes = all_webview_tile_nodes(&tree);
        assert!(nodes.contains(&alive_key));
        assert!(!nodes.contains(&stale_key));
}

#[test]
fn test_tile_layout_serde_roundtrip() {
        let mut tree = tree_with_graph_root();
        let a = NodeKey::new(5);
        let b = NodeKey::new(6);
        open_or_focus_webview_tile(&mut tree, a);
        open_or_focus_webview_tile(&mut tree, b);

        let json = serde_json::to_string(&tree).expect("serialize tree");
        let restored: Tree<TileKind> = serde_json::from_str(&json).expect("deserialize tree");
        let nodes = all_webview_tile_nodes(&restored);

        assert_eq!(nodes.len(), 2);
        assert!(nodes.contains(&a));
        assert!(nodes.contains(&b));
}

#[test]
fn test_startup_session_restore_prefers_bundle_over_legacy_tile_layout_json() {
        let temp = tempfile::TempDir::new().expect("temp dir");
        let mut app = GraphBrowserApp::new_from_dir(temp.path().to_path_buf());
        let bundle_node =
            app.add_node_and_sync("https://bundle.example".into(), Point2D::new(0.0, 0.0));
        let legacy_node =
            app.add_node_and_sync("https://legacy.example".into(), Point2D::new(10.0, 0.0));

        let mut bundle_tree = tree_with_graph_root();
        open_or_focus_webview_tile(&mut bundle_tree, bundle_node);
        crate::desktop::persistence_ops::save_named_workspace_bundle(
            &mut app,
            GraphBrowserApp::SESSION_WORKSPACE_LAYOUT_NAME,
            &bundle_tree,
        )
        .expect("save session bundle");

        let mut legacy_tree = tree_with_graph_root();
        open_or_focus_webview_tile(&mut legacy_tree, legacy_node);
        let legacy_json = serde_json::to_string(&legacy_tree).expect("serialize legacy tree");
        app.save_tile_layout_json(&legacy_json);

        let mut startup_tree = tree_with_graph_root();
        let restored = restore_startup_session_workspace_if_available(&mut app, &mut startup_tree);
        assert!(restored, "expected startup restore to succeed");

        let restored_nodes = all_webview_tile_nodes(&startup_tree);
        assert!(restored_nodes.contains(&bundle_node));
        assert!(!restored_nodes.contains(&legacy_node));

        let restored_runtime_json = serde_json::to_string(&startup_tree).expect("serialize restored tree");
        assert_eq!(
            app.last_session_workspace_layout_json(),
            Some(restored_runtime_json.as_str())
        );
    }

#[test]
fn test_invariant_check_detects_desync() {
        let mut app = GraphBrowserApp::new_for_testing();
        let node_key = app.add_node_and_sync("https://example.test".into(), Point2D::new(0.0, 0.0));
        let mut tree = tree_with_graph_root();
        open_or_focus_webview_tile(&mut tree, node_key);

        let contexts: HashMap<NodeKey, Rc<OffscreenRenderingContext>> = HashMap::new();
        let violations = tile_invariants::collect_tile_invariant_violations(&tree, &app, &contexts);

        assert!(
            violations
                .iter()
                .any(|v| v.contains("missing webview mapping"))
        );
        assert!(
            violations
                .iter()
                .any(|v| v.contains("missing rendering context"))
        );
}

#[test]
fn test_refresh_graph_search_matches_updates_active_index() {
        let mut app = GraphBrowserApp::new_for_testing();
        let github = app.add_node_and_sync("https://github.com".into(), Point2D::new(0.0, 0.0));
        let _example = app.add_node_and_sync("https://example.com".into(), Point2D::new(10.0, 0.0));

        let mut matches = Vec::new();
        let mut active = None;
        refresh_graph_search_matches(&app, "gthub", &mut matches, &mut active);

        assert_eq!(matches.first().copied(), Some(github));
        assert_eq!(active, Some(0));

        refresh_graph_search_matches(&app, "", &mut matches, &mut active);
        assert!(matches.is_empty());
        assert_eq!(active, None);
}

#[test]
fn test_step_graph_search_active_match_wraps() {
        let matches = vec![NodeKey::new(1), NodeKey::new(2), NodeKey::new(3)];
        let mut active = Some(2);
        step_graph_search_active_match(&matches, &mut active, 1);
        assert_eq!(active, Some(0));

        step_graph_search_active_match(&matches, &mut active, -1);
        assert_eq!(active, Some(2));
}

#[test]
fn test_active_graph_search_match_returns_current_key() {
        let matches = vec![NodeKey::new(10), NodeKey::new(11)];
        assert_eq!(
            active_graph_search_match(&matches, Some(1)),
            Some(NodeKey::new(11))
        );
        assert_eq!(active_graph_search_match(&matches, Some(2)), None);
        assert_eq!(active_graph_search_match(&matches, None), None);
}

#[test]
fn test_parse_data_dir_input_trims_quotes_and_whitespace() {
        let parsed = Gui::parse_data_dir_input("  \"C:\\\\tmp\\\\graph data\"  ")
            .expect("should parse quoted path");
        assert_eq!(parsed, PathBuf::from("C:\\tmp\\graph data"));

        let parsed_single = Gui::parse_data_dir_input(" 'C:\\\\tmp\\\\graph' ")
            .expect("should parse single-quoted path");
        assert_eq!(parsed_single, PathBuf::from("C:\\tmp\\graph"));
}

#[test]
fn test_parse_data_dir_input_empty_is_none() {
        assert!(Gui::parse_data_dir_input("").is_none());
        assert!(Gui::parse_data_dir_input("   ").is_none());
        assert!(Gui::parse_data_dir_input("\"\"").is_none());
}

#[test]
fn test_graph_intents_from_semantic_events_preserves_order_and_variants() {
        let w1 = test_webview_id();
        let w2 = test_webview_id();
        let w3 = test_webview_id();
        let events = vec![
            event(GraphSemanticEventKind::UrlChanged {
                webview_id: w1,
                new_url: "https://a.com".to_string(),
            }),
            event(GraphSemanticEventKind::HistoryChanged {
                webview_id: w2,
                entries: vec!["https://x.com".to_string()],
                current: 0,
            }),
            event(GraphSemanticEventKind::PageTitleChanged {
                webview_id: w1,
                title: Some("A".to_string()),
            }),
            event(GraphSemanticEventKind::CreateNewWebView {
                parent_webview_id: w1,
                child_webview_id: w3,
                initial_url: Some("https://child.com".to_string()),
            }),
        ];

        let intents = graph_intents_from_semantic_events(events);
        assert_eq!(intents.len(), 4);
        assert!(matches!(
            &intents[0],
            GraphIntent::WebViewUrlChanged { webview_id, new_url }
                if *webview_id == w1 && new_url == "https://a.com"
        ));
        assert!(matches!(
            &intents[1],
            GraphIntent::WebViewHistoryChanged { webview_id, entries, current }
                if *webview_id == w2 && entries.len() == 1 && *current == 0
        ));
        assert!(matches!(
            &intents[2],
            GraphIntent::WebViewTitleChanged { webview_id, title }
                if *webview_id == w1 && title.as_deref() == Some("A")
        ));
        assert!(matches!(
            &intents[3],
            GraphIntent::WebViewCreated { parent_webview_id, child_webview_id, initial_url }
                if *parent_webview_id == w1
                    && *child_webview_id == w3
                    && initial_url.as_deref() == Some("https://child.com")
        ));
}

#[test]
fn test_graph_intents_from_semantic_events_maps_webview_crashed() {
        let wv = test_webview_id();
        let events = vec![event(GraphSemanticEventKind::WebViewCrashed {
            webview_id: wv,
            reason: "renderer panic".to_string(),
            has_backtrace: true,
        })];

        let intents = graph_intents_from_semantic_events(events);
        assert_eq!(intents.len(), 1);
        assert!(matches!(
            &intents[0],
            GraphIntent::WebViewCrashed {
                webview_id,
                reason,
                has_backtrace
            } if *webview_id == wv && reason == "renderer panic" && *has_backtrace
        ));
}

#[test]
fn test_graph_intents_and_responsive_from_events_redirect_like_sequence_preserves_order() {
        let wv = test_webview_id();
        let events = vec![
            event(GraphSemanticEventKind::UrlChanged {
                webview_id: wv,
                new_url: "https://redirect-a.example".into(),
            }),
            event(GraphSemanticEventKind::UrlChanged {
                webview_id: wv,
                new_url: "https://redirect-b.example".into(),
            }),
            event(GraphSemanticEventKind::PageTitleChanged {
                webview_id: wv,
                title: Some("Final".into()),
            }),
            event(GraphSemanticEventKind::HistoryChanged {
                webview_id: wv,
                entries: vec![
                    "https://start.example".into(),
                    "https://redirect-b.example".into(),
                ],
                current: 1,
            }),
        ];

        let (intents, created_children, responsive_webviews) =
            graph_intents_and_responsive_from_events(events);

        assert!(created_children.is_empty());
        assert!(responsive_webviews.contains(&wv));
        assert_eq!(intents.len(), 4);
        assert!(matches!(
            &intents[0],
            GraphIntent::WebViewUrlChanged { webview_id, new_url }
                if *webview_id == wv && new_url == "https://redirect-a.example"
        ));
        assert!(matches!(
            &intents[1],
            GraphIntent::WebViewUrlChanged { webview_id, new_url }
                if *webview_id == wv && new_url == "https://redirect-b.example"
        ));
        assert!(matches!(
            &intents[2],
            GraphIntent::WebViewTitleChanged { webview_id, title }
                if *webview_id == wv && title.as_deref() == Some("Final")
        ));
        assert!(matches!(
            &intents[3],
            GraphIntent::WebViewHistoryChanged { webview_id, current, .. }
                if *webview_id == wv && *current == 1
        ));
}

#[test]
fn test_graph_intents_and_responsive_from_events_create_new_is_prioritized() {
        let parent = test_webview_id();
        let child = test_webview_id();
        let events = vec![
            event(GraphSemanticEventKind::UrlChanged {
                webview_id: parent,
                new_url: "https://parent.example".into(),
            }),
            event(GraphSemanticEventKind::CreateNewWebView {
                parent_webview_id: parent,
                child_webview_id: child,
                initial_url: Some("https://child.example".into()),
            }),
            event(GraphSemanticEventKind::PageTitleChanged {
                webview_id: parent,
                title: Some("Parent".into()),
            }),
        ];

        let (intents, created_children, responsive_webviews) =
            graph_intents_and_responsive_from_events(events);

        assert_eq!(created_children, vec![child]);
        assert!(responsive_webviews.contains(&parent));
        assert!(responsive_webviews.contains(&child));
        assert_eq!(intents.len(), 3);
        assert!(matches!(
            &intents[0],
            GraphIntent::WebViewCreated { parent_webview_id, child_webview_id, .. }
                if *parent_webview_id == parent && *child_webview_id == child
        ));
        assert!(matches!(
            &intents[1],
            GraphIntent::WebViewUrlChanged { webview_id, .. } if *webview_id == parent
        ));
        assert!(matches!(
            &intents[2],
            GraphIntent::WebViewTitleChanged { webview_id, .. } if *webview_id == parent
        ));
}

#[test]
fn test_semantic_events_to_intents_apply_to_graph_state() {
        let mut app = GraphBrowserApp::new_for_testing();
        let parent = app.add_node_and_sync("https://parent.com".into(), Point2D::new(10.0, 20.0));
        let parent_wv = test_webview_id();
        let child_wv = test_webview_id();
        app.map_webview_to_node(parent_wv, parent);

        let events = vec![
            event(GraphSemanticEventKind::UrlChanged {
                webview_id: parent_wv,
                new_url: "https://parent-updated.com".into(),
            }),
            event(GraphSemanticEventKind::HistoryChanged {
                webview_id: parent_wv,
                entries: vec!["https://a.com".into(), "https://b.com".into()],
                current: 1,
            }),
            event(GraphSemanticEventKind::PageTitleChanged {
                webview_id: parent_wv,
                title: Some("Updated Parent".into()),
            }),
            event(GraphSemanticEventKind::CreateNewWebView {
                parent_webview_id: parent_wv,
                child_webview_id: child_wv,
                initial_url: Some("https://child.com".into()),
            }),
        ];

        let intents = graph_intents_from_semantic_events(events);
        app.apply_intents_with_services(crate::app::default_app_services(), intents);

        let parent_node = app.graph.get_node(parent).unwrap();
        assert_eq!(parent_node.url, "https://parent-updated.com");
        assert_eq!(parent_node.title, "Updated Parent");
        assert_eq!(parent_node.history_entries.len(), 2);
        assert_eq!(parent_node.history_index, 1);

        let child = app.get_node_for_webview(child_wv).unwrap();
        assert_eq!(app.graph.get_node(child).unwrap().url, "https://child.com");
}

#[test]
fn test_graph_intent_for_thumbnail_result_accepts_matching_url() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app.add_node_and_sync("https://thumb.com".to_string(), Point2D::new(0.0, 0.0));
        let webview_id = test_webview_id();
        app.map_webview_to_node(webview_id, key);

        let result = ThumbnailCaptureResult {
            webview_id,
            requested_url: "https://thumb.com".to_string(),
            png_bytes: Some(vec![1, 2, 3, 4]),
            width: 2,
            height: 2,
        };

        let intent = graph_intent_for_thumbnail_result(&app, &result);
        assert!(matches!(
            intent,
            Some(GraphIntent::SetNodeThumbnail { key: k, width, height, .. })
                if k == key && width == 2 && height == 2
        ));
}

#[test]
fn test_graph_intent_for_thumbnail_result_rejects_stale_or_empty() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app.add_node_and_sync("https://thumb.com".to_string(), Point2D::new(0.0, 0.0));
        let webview_id = test_webview_id();
        app.map_webview_to_node(webview_id, key);

        let stale = ThumbnailCaptureResult {
            webview_id,
            requested_url: "https://other.com".to_string(),
            png_bytes: Some(vec![1, 2, 3, 4]),
            width: 2,
            height: 2,
        };
        assert!(graph_intent_for_thumbnail_result(&app, &stale).is_none());

        let empty_png = ThumbnailCaptureResult {
            webview_id,
            requested_url: "https://thumb.com".to_string(),
            png_bytes: None,
            width: 2,
            height: 2,
        };
        assert!(graph_intent_for_thumbnail_result(&app, &empty_png).is_none());
}

#[test]
fn test_reset_runtime_webview_state_clears_tiles_and_texture_caches() {
        let mut tree = tree_with_graph_root();
        let node_key = NodeKey::new(77);
        open_or_focus_webview_tile(&mut tree, node_key);

        let mut tile_rendering_contexts: HashMap<NodeKey, Rc<OffscreenRenderingContext>> =
            HashMap::new();
        let mut tile_favicon_textures: HashMap<NodeKey, (u64, egui::TextureHandle)> =
            HashMap::new();
        let mut favicon_textures: HashMap<
            WebViewId,
            (egui::TextureHandle, egui::load::SizedTexture),
        > = HashMap::new();

        let ctx = egui::Context::default();
        let image = egui::ColorImage::from_rgba_unmultiplied([1, 1], &[255, 255, 255, 255]);
        let handle = ctx.load_texture("test-reset-favicon", image, Default::default());
        tile_favicon_textures.insert(node_key, (123, handle.clone()));
        let wv_id = test_webview_id();
        let sized = egui::load::SizedTexture::new(handle.id(), egui::vec2(1.0, 1.0));
        favicon_textures.insert(wv_id, (handle, sized));

        Gui::reset_runtime_webview_state(
            &mut tree,
            &mut tile_rendering_contexts,
            &mut tile_favicon_textures,
            &mut favicon_textures,
        );

        assert!(!has_any_webview_tiles_in(&tree));
        assert!(tile_rendering_contexts.is_empty());
        assert!(tile_favicon_textures.is_empty());
        assert!(favicon_textures.is_empty());
}
