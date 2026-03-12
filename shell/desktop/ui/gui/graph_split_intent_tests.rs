use super::gui_orchestration;
use super::{apply_graph_surface_focus_state, apply_node_focus_state};
use crate::app::{
    CameraCommand, GraphBrowserApp, GraphIntent, GraphViewFrame, GraphViewId, GraphViewState,
    SettingsToolPage, WorkbenchIntent,
};
use crate::graph::NodeKey;
use crate::shell::desktop::ui::gui_state::GuiRuntimeState;
use crate::shell::desktop::workbench::pane_model::{
    GraphPaneRef, PaneId, SplitDirection, ToolPaneRef, ToolPaneState,
};
use crate::shell::desktop::workbench::tile_kind::TileKind;
use egui_tiles::{Tile, Tiles, Tree};
use std::time::Duration;

fn graph_pane(view_id: GraphViewId) -> TileKind {
    TileKind::Graph(GraphPaneRef::new(view_id))
}

fn tool_pane(kind: ToolPaneState) -> TileKind {
    TileKind::Tool(ToolPaneRef::new(kind))
}

fn is_tool_tile(tile: &Tile<TileKind>, kind: ToolPaneState) -> bool {
    matches!(tile, Tile::Pane(TileKind::Tool(tool_kind)) if tool_kind.kind == kind)
}

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

fn tool_pane_count(tree: &Tree<TileKind>, kind: ToolPaneState) -> usize {
    tree.tiles
        .iter()
        .filter(|(_, tile)| is_tool_tile(tile, kind.clone()))
        .count()
}

fn active_tool_pane(tree: &Tree<TileKind>, kind: ToolPaneState) -> bool {
    tree.active_tiles().into_iter().any(|tile_id| {
        tree.tiles
            .get(tile_id)
            .is_some_and(|tile| is_tool_tile(tile, kind.clone()))
    })
}

fn graph_pane_id(tree: &Tree<TileKind>, view_id: GraphViewId) -> PaneId {
    tree.tiles
        .iter()
        .find_map(|(_, tile)| match tile {
            Tile::Pane(TileKind::Graph(view_ref)) if view_ref.graph_view_id == view_id => {
                Some(view_ref.pane_id)
            }
            _ => None,
        })
        .expect("expected graph pane id")
}

#[test]
fn split_pane_intent_creates_new_graph_view_pane() {
    let mut app = GraphBrowserApp::new_for_testing();
    let initial_view = GraphViewId::new();
    let mut tiles = Tiles::default();
    let root = tiles.insert_pane(graph_pane(initial_view));
    let mut tree = Tree::new("graphshell_tiles", root, tiles);

    let mut intents = vec![WorkbenchIntent::SplitPane {
        source_pane: graph_pane_id(&tree, initial_view),
        direction: SplitDirection::Horizontal,
    }];

    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut intents);

    assert!(
        intents.is_empty(),
        "split intent should be consumed by workbench authority"
    );

    let graph_views: Vec<GraphViewId> = tree
        .tiles
        .iter()
        .filter_map(|(_, tile)| match tile {
            Tile::Pane(TileKind::Graph(view_ref)) => Some(view_ref.graph_view_id),
            _ => None,
        })
        .collect();

    assert_eq!(
        graph_views.len(),
        2,
        "split should produce a second graph pane"
    );
    assert!(graph_views.contains(&initial_view));
    assert!(graph_views.iter().any(|view_id| *view_id != initial_view));
    assert!(
        active_graph_count(&tree) >= 1,
        "a graph pane should remain active"
    );
}

#[test]
fn split_pane_intent_accepts_tool_pane_identity_as_source() {
    let mut app = GraphBrowserApp::new_for_testing();
    let settings_ref = ToolPaneRef::new(ToolPaneState::Settings);
    let mut tiles = Tiles::default();
    let root = tiles.insert_pane(TileKind::Tool(settings_ref.clone()));
    let mut tree = Tree::new("graphshell_tiles", root, tiles);

    let mut intents = vec![WorkbenchIntent::SplitPane {
        source_pane: settings_ref.pane_id,
        direction: SplitDirection::Horizontal,
    }];

    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut intents);

    assert!(intents.is_empty());
    assert_eq!(tool_pane_count(&tree, ToolPaneState::Settings), 1);
    let graph_count = tree
        .tiles
        .iter()
        .filter(|(_, tile)| matches!(tile, Tile::Pane(TileKind::Graph(_))))
        .count();
    assert_eq!(
        graph_count, 1,
        "split should add a graph pane beside the tool pane"
    );
    let root_id = tree.root().expect("split should preserve a root");
    let linear = match tree.tiles.get(root_id) {
        Some(Tile::Container(egui_tiles::Container::Linear(linear))) => linear,
        other => panic!("expected split root container, got {other:?}"),
    };
    assert_eq!(linear.children.len(), 2);
    for child in &linear.children {
        assert!(matches!(
            tree.tiles.get(*child),
            Some(Tile::Container(egui_tiles::Container::Tabs(_)))
        ));
    }
}

#[test]
fn settings_history_url_intent_is_consumed_by_workbench_authority() {
    let mut app = GraphBrowserApp::new_for_testing();
    let initial_view = GraphViewId::new();
    let mut tiles = Tiles::default();
    let root = tiles.insert_pane(graph_pane(initial_view));
    let mut tree = Tree::new("graphshell_tiles", root, tiles);
    let mut intents = vec![WorkbenchIntent::OpenSettingsUrl {
        url: crate::util::VersoAddress::settings(crate::util::GraphshellSettingsPath::History)
            .to_string(),
    }];

    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut intents);

    assert!(
        intents.is_empty(),
        "settings/history should be consumed by workbench authority"
    );
}

#[test]
fn settings_physics_url_intent_is_consumed_by_workbench_authority() {
    let mut app = GraphBrowserApp::new_for_testing();
    let initial_view = GraphViewId::new();
    let mut tiles = Tiles::default();
    let root = tiles.insert_pane(graph_pane(initial_view));
    let mut tree = Tree::new("graphshell_tiles", root, tiles);
    let mut intents = vec![WorkbenchIntent::OpenSettingsUrl {
        url: crate::util::VersoAddress::settings(crate::util::GraphshellSettingsPath::Physics)
            .to_string(),
    }];

    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut intents);

    assert!(
        intents.is_empty(),
        "settings/physics should be consumed by workbench authority"
    );
}

#[test]
fn settings_persistence_url_intent_is_consumed_by_workbench_authority() {
    let mut app = GraphBrowserApp::new_for_testing();
    let initial_view = GraphViewId::new();
    let mut tiles = Tiles::default();
    let root = tiles.insert_pane(graph_pane(initial_view));
    let mut tree = Tree::new("graphshell_tiles", root, tiles);
    let mut intents = vec![WorkbenchIntent::OpenSettingsUrl {
        url: crate::util::VersoAddress::settings(crate::util::GraphshellSettingsPath::Persistence)
            .to_string(),
    }];

    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut intents);

    assert!(
        intents.is_empty(),
        "settings/persistence should be consumed by workbench authority"
    );
}

#[test]
fn settings_sync_url_intent_is_consumed_by_workbench_authority() {
    let mut app = GraphBrowserApp::new_for_testing();
    let initial_view = GraphViewId::new();
    let mut tiles = Tiles::default();
    let root = tiles.insert_pane(graph_pane(initial_view));
    let mut tree = Tree::new("graphshell_tiles", root, tiles);
    let mut intents = vec![WorkbenchIntent::OpenSettingsUrl {
        url: crate::util::VersoAddress::settings(crate::util::GraphshellSettingsPath::Sync)
            .to_string(),
    }];

    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut intents);

    assert!(
        intents.is_empty(),
        "settings/sync should be consumed by workbench authority"
    );
}

#[test]
fn settings_root_url_opens_settings_tool_pane() {
    let mut app = GraphBrowserApp::new_for_testing();
    let initial_view = GraphViewId::new();
    let mut tiles = Tiles::default();
    let root = tiles.insert_pane(graph_pane(initial_view));
    let mut tree = Tree::new("graphshell_tiles", root, tiles);
    let mut intents = vec![WorkbenchIntent::OpenSettingsUrl {
        url: crate::util::VersoAddress::settings(crate::util::GraphshellSettingsPath::General)
            .to_string(),
    }];

    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut intents);

    assert!(intents.is_empty());
    assert_eq!(tool_pane_count(&tree, ToolPaneState::Settings), 1);
    assert!(active_tool_pane(&tree, ToolPaneState::Settings));
    assert_eq!(app.workspace.settings_tool_page, SettingsToolPage::General);
}

#[test]
fn settings_sync_url_focuses_existing_settings_tool_pane_without_duplication() {
    let mut app = GraphBrowserApp::new_for_testing();
    let mut tiles = Tiles::default();
    let settings = tiles.insert_pane(tool_pane(ToolPaneState::Settings));
    let history = tiles.insert_pane(tool_pane(ToolPaneState::HistoryManager));
    let tabs_root = tiles.insert_tab_tile(vec![history, settings]);
    let mut tree = Tree::new("graphshell_tiles", tabs_root, tiles);
    let _ = tree.make_active(|_, tile| is_tool_tile(tile, ToolPaneState::HistoryManager));
    let mut intents = vec![WorkbenchIntent::OpenSettingsUrl {
        url: crate::util::VersoAddress::settings(crate::util::GraphshellSettingsPath::Sync)
            .to_string(),
    }];

    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut intents);

    assert!(intents.is_empty());
    assert_eq!(tool_pane_count(&tree, ToolPaneState::Settings), 1);
    assert!(active_tool_pane(&tree, ToolPaneState::Settings));
}

#[test]
fn settings_history_url_focuses_existing_history_tool_pane_without_duplication() {
    let mut app = GraphBrowserApp::new_for_testing();
    let mut tiles = Tiles::default();
    let settings = tiles.insert_pane(tool_pane(ToolPaneState::Settings));
    let history = tiles.insert_pane(tool_pane(ToolPaneState::HistoryManager));
    let tabs_root = tiles.insert_tab_tile(vec![settings, history]);
    let mut tree = Tree::new("graphshell_tiles", tabs_root, tiles);
    let _ = tree.make_active(|_, tile| is_tool_tile(tile, ToolPaneState::Settings));
    let mut intents = vec![WorkbenchIntent::OpenSettingsUrl {
        url: crate::util::VersoAddress::settings(crate::util::GraphshellSettingsPath::History)
            .to_string(),
    }];

    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut intents);

    assert!(intents.is_empty());
    assert_eq!(tool_pane_count(&tree, ToolPaneState::HistoryManager), 1);
    assert!(active_tool_pane(&tree, ToolPaneState::HistoryManager));
}

#[test]
fn close_settings_tool_pane_restores_previous_graph_focus() {
    let mut app = GraphBrowserApp::new_for_testing();
    let graph_view = GraphViewId::new();
    let mut tiles = Tiles::default();
    let graph = tiles.insert_pane(graph_pane(graph_view));
    let root = tiles.insert_tab_tile(vec![graph]);
    let mut tree = Tree::new("graphshell_tiles", root, tiles);

    let mut open_intents = vec![WorkbenchIntent::OpenSettingsUrl {
        url: crate::util::VersoAddress::settings(crate::util::GraphshellSettingsPath::General)
            .to_string(),
    }];
    gui_orchestration::handle_tool_pane_intents(&mut app, &mut tree, &mut open_intents);
    assert!(open_intents.is_empty());

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
            Some(Tile::Pane(TileKind::Graph(existing))) if existing.graph_view_id == graph_view
        )
    }));
}

#[test]
fn ui_overlay_active_flags_include_radial_menu_capture() {
    assert!(!super::ui_overlay_active_from_flags(
        false, false, false, false
    ));
    assert!(super::ui_overlay_active_from_flags(
        true, false, false, false
    ));
    assert!(super::ui_overlay_active_from_flags(
        false, true, false, false
    ));
    assert!(super::ui_overlay_active_from_flags(
        false, false, true, false
    ));
    assert!(super::ui_overlay_active_from_flags(
        false, false, false, true
    ));
}

#[test]
fn node_focus_state_clears_graph_surface_focus() {
    let mut runtime_state = GuiRuntimeState {
        graph_search_open: false,
        graph_search_query: String::new(),
        graph_search_filter_mode: false,
        graph_search_matches: Vec::new(),
        graph_search_active_match_index: None,
        local_widget_focus: None,
        focused_node_hint: None,
        graph_surface_focused: true,
        focus_ring_node_key: None,
        focus_ring_started_at: None,
        focus_ring_duration: Duration::from_millis(500),
        omnibar_search_session: None,
        focus_authority: crate::shell::desktop::ui::gui_state::RuntimeFocusAuthorityState::default(
        ),
        toolbar_drafts: std::collections::HashMap::new(),
        command_palette_toggle_requested: false,
        deferred_open_child_webviews: Vec::new(),
    };

    let node = NodeKey::new(1);
    apply_node_focus_state(&mut runtime_state, Some(node));

    assert_eq!(runtime_state.focused_node_hint, Some(node));
    assert!(!runtime_state.graph_surface_focused);
}

#[test]
fn graph_surface_focus_state_clears_node_hint_and_syncs_focused_view() {
    let mut runtime_state = GuiRuntimeState {
        graph_search_open: false,
        graph_search_query: String::new(),
        graph_search_filter_mode: false,
        graph_search_matches: Vec::new(),
        graph_search_active_match_index: None,
        local_widget_focus: None,
        focused_node_hint: Some(NodeKey::new(2)),
        graph_surface_focused: false,
        focus_ring_node_key: None,
        focus_ring_started_at: None,
        focus_ring_duration: Duration::from_millis(500),
        omnibar_search_session: None,
        focus_authority: crate::shell::desktop::ui::gui_state::RuntimeFocusAuthorityState::default(
        ),
        toolbar_drafts: std::collections::HashMap::new(),
        command_palette_toggle_requested: false,
        deferred_open_child_webviews: Vec::new(),
    };
    let mut app = GraphBrowserApp::new_for_testing();
    let graph_view = GraphViewId::new();

    apply_graph_surface_focus_state(&mut runtime_state, &mut app, Some(graph_view));

    assert_eq!(runtime_state.focused_node_hint, None);
    assert!(runtime_state.graph_surface_focused);
    assert_eq!(app.workspace.focused_view, Some(graph_view));
}

#[test]
fn reconcile_workspace_graph_views_prunes_stale_state_and_preserves_active_focus() {
    let mut app = GraphBrowserApp::new_for_testing();
    let stale_view = GraphViewId::new();
    let live_view = GraphViewId::new();

    app.workspace
        .views
        .insert(stale_view, GraphViewState::new_with_id(stale_view, "Stale"));
    app.workspace
        .views
        .insert(live_view, GraphViewState::new_with_id(live_view, "Live"));
    app.workspace.graph_view_frames.insert(
        stale_view,
        GraphViewFrame {
            zoom: 1.0,
            pan_x: -100.0,
            pan_y: -100.0,
        },
    );

    app.workspace.focused_view = Some(stale_view);
    app.request_camera_command_for_view(Some(stale_view), CameraCommand::Fit);
    app.apply_reducer_intents(vec![GraphIntent::RequestZoomIn]);
    app.queue_pending_wheel_zoom_delta(stale_view, 1.0, Some((10.0, 20.0)));

    let mut tiles = Tiles::default();
    let live_graph_tile = tiles.insert_pane(graph_pane(live_view));
    let mut tree = Tree::new("graphshell_tiles", live_graph_tile, tiles);
    let _ = tree.make_active(
        |_, tile| matches!(tile, Tile::Pane(TileKind::Graph(existing)) if existing.graph_view_id == live_view),
    );

    super::pane_queries::reconcile_workspace_graph_views_from_tiles(&mut app, &tree);

    assert!(app.workspace.views.contains_key(&live_view));
    assert!(!app.workspace.views.contains_key(&stale_view));
    assert!(!app.workspace.graph_view_frames.contains_key(&stale_view));
    assert_eq!(app.workspace.focused_view, Some(live_view));
    assert!(app.pending_camera_command().is_none());
    assert!(app.take_pending_keyboard_zoom_request(stale_view).is_none());
    assert_eq!(app.pending_wheel_zoom_delta(stale_view), 0.0);
}
