use crate::shell::desktop::workbench::pane_model::{ToolPaneRef, ToolPaneState};
use crate::shell::desktop::workbench::tile_kind::TileKind;
use crate::shell::desktop::workbench::tile_view_ops;
use egui_tiles::{Tile, Tiles, Tree};

fn tool_pane(kind: ToolPaneState) -> TileKind {
    TileKind::Tool(ToolPaneRef::new(kind))
}

fn is_tool_tile(tile: &Tile<TileKind>, kind: ToolPaneState) -> bool {
    matches!(tile, Tile::Pane(TileKind::Tool(tool)) if tool.kind == kind)
}

fn diagnostics_active(tree: &Tree<TileKind>) -> bool {
    tree.active_tiles().into_iter().any(|tile_id| {
        tree.tiles
            .get(tile_id)
            .is_some_and(|tile| is_tool_tile(tile, ToolPaneState::Diagnostics))
    })
}

#[test]
fn diagnostics_shortcut_focuses_existing_diagnostics_tool_pane() {
    let mut tiles = Tiles::default();
    let settings_id = tiles.insert_pane(tool_pane(ToolPaneState::Settings));
    let diagnostics_id = tiles.insert_pane(tool_pane(ToolPaneState::Diagnostics));
    let tabs_root = tiles.insert_tab_tile(vec![settings_id, diagnostics_id]);
    let mut tree = Tree::new("tool_tabs", tabs_root, tiles);

    let _ = tree.make_active(|_, tile| is_tool_tile(tile, ToolPaneState::Settings));
    assert!(!diagnostics_active(&tree));

    tile_view_ops::open_or_focus_tool_pane(&mut tree, ToolPaneState::Diagnostics);
    assert!(diagnostics_active(&tree));
}

#[test]
fn diagnostics_shortcut_inserts_diagnostics_tool_pane_when_missing() {
    let mut tiles = Tiles::default();
    let settings_id = tiles.insert_pane(tool_pane(ToolPaneState::Settings));
    let mut tree = Tree::new("tool_tabs", settings_id, tiles);

    tile_view_ops::open_or_focus_tool_pane(&mut tree, ToolPaneState::Diagnostics);

    let diagnostics_count = tree
        .tiles
        .iter()
        .filter(|(_, tile)| is_tool_tile(tile, ToolPaneState::Diagnostics))
        .count();
    assert_eq!(diagnostics_count, 1);
    assert!(diagnostics_active(&tree));
}

#[test]
fn multiple_tool_panes_coexist_with_expected_titles() {
    let mut tiles = Tiles::default();
    let root = tiles.insert_pane(tool_pane(ToolPaneState::Diagnostics));
    let mut tree = Tree::new("tool_tabs", root, tiles);

    tile_view_ops::open_or_focus_tool_pane(&mut tree, ToolPaneState::HistoryManager);
    tile_view_ops::open_or_focus_tool_pane(&mut tree, ToolPaneState::AccessibilityInspector);
    tile_view_ops::open_or_focus_tool_pane(&mut tree, ToolPaneState::Settings);

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
    let history_id = tiles.insert_pane(tool_pane(ToolPaneState::HistoryManager));
    let diagnostics_id = tiles.insert_pane(tool_pane(ToolPaneState::Diagnostics));
    let tabs_root = tiles.insert_tab_tile(vec![history_id, diagnostics_id]);
    let mut tree = Tree::new("tool_tabs", tabs_root, tiles);

    let _ = tree.make_active(|_, tile| is_tool_tile(tile, ToolPaneState::HistoryManager));

    tile_view_ops::open_or_focus_tool_pane(&mut tree, ToolPaneState::Diagnostics);

    assert!(tree.active_tiles().into_iter().any(|tile_id| {
        tree.tiles
            .get(tile_id)
            .is_some_and(|tile| is_tool_tile(tile, ToolPaneState::Diagnostics))
    }));
    assert!(!tree.active_tiles().into_iter().any(|tile_id| {
        tree.tiles
            .get(tile_id)
            .is_some_and(|tile| is_tool_tile(tile, ToolPaneState::HistoryManager))
    }));
}

