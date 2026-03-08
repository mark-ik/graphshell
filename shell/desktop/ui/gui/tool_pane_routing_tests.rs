use super::Gui;
use crate::shell::desktop::workbench::pane_model::ToolPaneState;
use crate::shell::desktop::workbench::tile_kind::TileKind;
use egui_tiles::{Tile, Tiles, Tree};

fn diagnostics_active(tree: &Tree<TileKind>) -> bool {
    tree.active_tiles().into_iter().any(|tile_id| {
        matches!(
            tree.tiles.get(tile_id),
            Some(Tile::Pane(TileKind::Tool(ToolPaneState::Diagnostics)))
        )
    })
}

#[test]
fn diagnostics_shortcut_focuses_existing_diagnostics_tool_pane() {
    let mut tiles = Tiles::default();
    let settings_id = tiles.insert_pane(TileKind::Tool(ToolPaneState::Settings));
    let diagnostics_id = tiles.insert_pane(TileKind::Tool(ToolPaneState::Diagnostics));
    let tabs_root = tiles.insert_tab_tile(vec![settings_id, diagnostics_id]);
    let mut tree = Tree::new("tool_tabs", tabs_root, tiles);

    let _ = tree.make_active(|_, tile| {
        matches!(tile, Tile::Pane(TileKind::Tool(ToolPaneState::Settings)))
    });
    assert!(!diagnostics_active(&tree));

    Gui::open_or_focus_diagnostics_tool_pane(&mut tree);
    assert!(diagnostics_active(&tree));
}

#[test]
fn diagnostics_shortcut_inserts_diagnostics_tool_pane_when_missing() {
    let mut tiles = Tiles::default();
    let settings_id = tiles.insert_pane(TileKind::Tool(ToolPaneState::Settings));
    let mut tree = Tree::new("tool_tabs", settings_id, tiles);

    Gui::open_or_focus_diagnostics_tool_pane(&mut tree);

    let diagnostics_count = tree
        .tiles
        .iter()
        .filter(|(_, tile)| {
            matches!(tile, Tile::Pane(TileKind::Tool(ToolPaneState::Diagnostics)))
        })
        .count();
    assert_eq!(diagnostics_count, 1);
    assert!(diagnostics_active(&tree));
}

#[test]
fn multiple_tool_panes_coexist_with_expected_titles() {
    let mut tiles = Tiles::default();
    let root = tiles.insert_pane(TileKind::Tool(ToolPaneState::Diagnostics));
    let mut tree = Tree::new("tool_tabs", root, tiles);

    Gui::open_or_focus_tool_pane(&mut tree, ToolPaneState::HistoryManager);
    Gui::open_or_focus_tool_pane(&mut tree, ToolPaneState::AccessibilityInspector);
    Gui::open_or_focus_tool_pane(&mut tree, ToolPaneState::Settings);

    let mut tool_titles: Vec<&'static str> = tree
        .tiles
        .iter()
        .filter_map(|(_, tile)| match tile {
            Tile::Pane(TileKind::Tool(tool)) => Some(tool.title()),
            _ => None,
        })
        .collect();
    tool_titles.sort_unstable();

    assert_eq!(
        tool_titles,
        vec!["Accessibility", "Diagnostics", "History", "Settings"]
    );
}

#[test]
fn diagnostics_shortcut_focuses_diagnostics_not_other_tool_pane() {
    let mut tiles = Tiles::default();
    let history_id = tiles.insert_pane(TileKind::Tool(ToolPaneState::HistoryManager));
    let diagnostics_id = tiles.insert_pane(TileKind::Tool(ToolPaneState::Diagnostics));
    let tabs_root = tiles.insert_tab_tile(vec![history_id, diagnostics_id]);
    let mut tree = Tree::new("tool_tabs", tabs_root, tiles);

    let _ = tree.make_active(|_, tile| {
        matches!(
            tile,
            Tile::Pane(TileKind::Tool(ToolPaneState::HistoryManager))
        )
    });

    Gui::open_or_focus_diagnostics_tool_pane(&mut tree);

    assert!(tree.active_tiles().into_iter().any(|tile_id| {
        matches!(
            tree.tiles.get(tile_id),
            Some(Tile::Pane(TileKind::Tool(ToolPaneState::Diagnostics)))
        )
    }));
    assert!(!tree.active_tiles().into_iter().any(|tile_id| {
        matches!(
            tree.tiles.get(tile_id),
            Some(Tile::Pane(TileKind::Tool(ToolPaneState::HistoryManager)))
        )
    }));
}
