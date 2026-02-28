use crate::app::{GraphBrowserApp, GraphIntent, GraphViewId};
use crate::shell::desktop::ui::gui_orchestration;
use crate::shell::desktop::workbench::pane_model::ToolPaneState;
use crate::shell::desktop::workbench::tile_kind::TileKind;
use egui_tiles::{Tile, Tiles, Tree};

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
