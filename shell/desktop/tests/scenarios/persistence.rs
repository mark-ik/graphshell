use super::super::harness::TestRegistry;
use crate::app::CommandPaletteShortcut;
use crate::app::GraphBrowserApp;
use crate::app::GraphIntent;
use crate::app::GraphViewId;
use crate::app::HelpPanelShortcut;
use crate::app::RadialMenuShortcut;
use crate::app::SelectionUpdateMode;
use crate::app::ToastAnchorPreference;
use crate::app::WorkbenchIntent;
use crate::registries::domain::layout::canvas::CanvasLassoBinding;
use crate::services::persistence::GraphStore;
use crate::services::persistence::types::LogEntry;
use crate::shell::desktop::runtime::registries::input::{
    GamepadButton, InputBinding, InputBindingRemap, InputContext,
};
use crate::shell::desktop::runtime::registries::workbench_surface::WorkbenchSurfaceRegistry;
use crate::shell::desktop::ui::persistence_ops::{
    load_named_workspace_bundle, restore_runtime_tree_from_workspace_bundle,
    save_named_workspace_bundle,
};
use crate::shell::desktop::workbench::pane_model::{GraphPaneRef, NodePaneState};
use crate::shell::desktop::workbench::tile_kind::TileKind;
use egui_tiles::{Container, Tile, Tiles, Tree};
use std::collections::{BTreeSet, HashMap};
use std::time::Duration;
use tempfile::TempDir;
use uuid::Uuid;

#[test]
fn open_node_frame_routed_preserves_unsaved_prompt_state_until_restore() {
    let mut harness = TestRegistry::new();
    let key = harness.add_node("https://example.com");
    let node_id = harness
        .app
        .workspace
        .domain
        .graph
        .get_node(key)
        .expect("node should exist")
        .id;

    let mut index = HashMap::new();
    index.insert(node_id, BTreeSet::from(["workspace-alpha".to_string()]));
    harness.app.init_membership_index(index);
    harness.app.mark_current_workspace_synthesized();
    harness
        .app
        .apply_reducer_intents([GraphIntent::CreateNodeNearCenter]);

    assert!(harness.app.should_prompt_unsaved_workspace_save());

    harness
        .app
        .apply_reducer_intents([GraphIntent::OpenNodeFrameRouted {
            key,
            prefer_frame: None,
        }]);

    assert_eq!(
        harness.app.take_pending_restore_workspace_snapshot_named(),
        Some("workspace-alpha".to_string())
    );
    assert!(harness.app.should_prompt_unsaved_workspace_save());
}

#[test]
fn frame_has_unsaved_changes_for_graph_mutations() {
    let mut harness = TestRegistry::new();
    harness.app.mark_current_workspace_synthesized();

    harness
        .app
        .apply_reducer_intents([GraphIntent::CreateNodeNearCenter]);

    assert!(harness.app.should_prompt_unsaved_workspace_save());
}

#[test]
fn frame_modified_for_graph_mutations_even_when_not_synthesized() {
    let mut harness = TestRegistry::new();

    assert!(!harness.app.should_prompt_unsaved_workspace_save());
    harness
        .app
        .apply_reducer_intents([GraphIntent::CreateNodeNearCenter]);

    assert!(harness.app.should_prompt_unsaved_workspace_save());
}

#[test]
fn unsaved_prompt_warning_resets_on_additional_graph_mutation() {
    let mut harness = TestRegistry::new();
    harness.app.mark_current_workspace_synthesized();
    harness
        .app
        .apply_reducer_intents([GraphIntent::CreateNodeNearCenter]);

    assert!(harness.app.consume_unsaved_workspace_prompt_warning());
    assert!(!harness.app.consume_unsaved_workspace_prompt_warning());

    harness
        .app
        .apply_reducer_intents([GraphIntent::CreateNodeNearCenter]);

    assert!(harness.app.consume_unsaved_workspace_prompt_warning());
}

#[test]
fn save_named_frame_clears_unsaved_prompt_state() {
    let dir = TempDir::new().expect("temp dir should be created");
    let mut app = GraphBrowserApp::new_from_dir(dir.path().to_path_buf());
    app.mark_current_workspace_synthesized();
    app.apply_reducer_intents([GraphIntent::CreateNodeNearCenter]);

    assert!(app.should_prompt_unsaved_workspace_save());
    assert!(app.consume_unsaved_workspace_prompt_warning());

    app.save_workspace_layout_json("workspace:user-saved", "{\"root\":null}");

    assert!(!app.should_prompt_unsaved_workspace_save());
    assert!(!app.consume_unsaved_workspace_prompt_warning());
}

#[test]
fn frame_not_modified_for_non_graph_mutations() {
    let mut harness = TestRegistry::new();
    let key = harness.add_node("https://example.com");
    harness.app.mark_current_workspace_synthesized();

    harness.app.apply_reducer_intents([GraphIntent::SelectNode {
        key,
        multi_select: false,
    }]);

    assert!(!harness.app.should_prompt_unsaved_workspace_save());
}

#[test]
fn frame_not_modified_for_set_node_position() {
    let mut harness = TestRegistry::new();
    let key = harness.add_node("https://example.com");
    harness.app.mark_current_workspace_synthesized();

    harness
        .app
        .apply_reducer_intents([GraphIntent::SetNodePosition {
            key,
            position: euclid::Point2D::new(42.0, 24.0),
        }]);

    assert!(!harness.app.should_prompt_unsaved_workspace_save());
}

#[test]
fn frame_has_unsaved_changes_for_set_node_pinned() {
    let mut harness = TestRegistry::new();
    let key = harness.add_node("https://example.com");
    harness.app.mark_current_workspace_synthesized();

    harness
        .app
        .apply_reducer_intents([GraphIntent::SetNodePinned {
            key,
            is_pinned: true,
        }]);

    assert!(harness.app.should_prompt_unsaved_workspace_save());
}

#[test]
fn session_frame_layout_autosave_uses_layout_hash_and_caches_runtime_layout() {
    let dir = TempDir::new().expect("temp dir should be created");
    let mut app = GraphBrowserApp::new_from_dir(dir.path().to_path_buf());
    app.set_workspace_autosave_interval_secs(1)
        .expect("autosave interval should be configurable");

    app.save_session_workspace_layout_json_if_changed("runtime-layout-v1");
    assert_eq!(
        app.load_workspace_layout_json(GraphBrowserApp::SESSION_WORKSPACE_LAYOUT_NAME)
            .as_deref(),
        Some("runtime-layout-v1")
    );
    assert_eq!(
        app.last_session_workspace_layout_json(),
        Some("runtime-layout-v1")
    );

    std::thread::sleep(Duration::from_millis(1100));
    app.save_session_workspace_layout_json_if_changed("runtime-layout-v1");

    assert_eq!(
        app.load_workspace_layout_json(GraphBrowserApp::SESSION_WORKSPACE_LAYOUT_NAME)
            .as_deref(),
        Some("runtime-layout-v1")
    );
    assert_eq!(
        app.last_session_workspace_layout_json(),
        Some("runtime-layout-v1")
    );
    assert_eq!(app.list_workspace_layout_names().len(), 1);
}

#[test]
fn session_frame_layout_autosave_rotates_previous_latest_on_layout_change() {
    let dir = TempDir::new().expect("temp dir should be created");
    let mut app = GraphBrowserApp::new_from_dir(dir.path().to_path_buf());
    app.set_workspace_autosave_interval_secs(1)
        .expect("autosave interval should be configurable");
    app.set_workspace_autosave_retention(2)
        .expect("retention setting should succeed");

    app.save_session_workspace_layout_json_if_changed("runtime-layout-a");
    std::thread::sleep(Duration::from_millis(1100));
    app.save_session_workspace_layout_json_if_changed("runtime-layout-b");

    assert_eq!(
        app.load_workspace_layout_json(GraphBrowserApp::SESSION_WORKSPACE_LAYOUT_NAME)
            .as_deref(),
        Some("runtime-layout-b")
    );
    let history_name = app
        .list_workspace_layout_names()
        .into_iter()
        .find(|name| name != GraphBrowserApp::SESSION_WORKSPACE_LAYOUT_NAME)
        .expect("rotating autosave should persist one history entry");
    assert_eq!(
        app.load_workspace_layout_json(&history_name).as_deref(),
        Some("runtime-layout-a")
    );
    assert_eq!(
        app.last_session_workspace_layout_json(),
        Some("runtime-layout-b")
    );
}

#[test]
fn switch_persistence_dir_reloads_graph_state() {
    let dir_a = TempDir::new().expect("temp dir a should be created");
    let path_a = dir_a.path().to_path_buf();
    let dir_b = TempDir::new().expect("temp dir b should be created");
    let path_b = dir_b.path().to_path_buf();

    {
        let mut store_a = GraphStore::open(path_a.clone()).expect("store a should open");
        store_a.log_mutation(&LogEntry::AddNode {
            node_id: Uuid::new_v4().to_string(),
            url: "https://from-a.com".to_string(),
            position_x: 1.0,
            position_y: 2.0,
            timestamp_ms: 0,
        });
    }
    {
        let mut store_b = GraphStore::open(path_b.clone()).expect("store b should open");
        store_b.log_mutation(&LogEntry::AddNode {
            node_id: Uuid::new_v4().to_string(),
            url: "https://from-b.com".to_string(),
            position_x: 3.0,
            position_y: 4.0,
            timestamp_ms: 0,
        });
        store_b.log_mutation(&LogEntry::AddNode {
            node_id: Uuid::new_v4().to_string(),
            url: "about:blank#7".to_string(),
            position_x: 5.0,
            position_y: 6.0,
            timestamp_ms: 0,
        });
    }

    let mut app = GraphBrowserApp::new_from_dir(path_a);
    assert!(
        app.workspace
            .domain
            .graph
            .get_node_by_url("https://from-a.com")
            .is_some()
    );
    assert!(
        app.workspace
            .domain
            .graph
            .get_node_by_url("https://from-b.com")
            .is_none()
    );

    app.switch_persistence_dir(path_b)
        .expect("switching persistence dir should succeed");

    assert!(
        app.workspace
            .domain
            .graph
            .get_node_by_url("https://from-a.com")
            .is_none()
    );
    assert!(
        app.workspace
            .domain
            .graph
            .get_node_by_url("https://from-b.com")
            .is_some()
    );
    assert!(app.focused_selection().is_empty());

    let new_placeholder = app.create_new_node_near_center();
    assert_eq!(
        app.workspace
            .domain
            .graph
            .get_node(new_placeholder)
            .expect("node should exist")
            .url,
        "about:blank#8"
    );
}

#[test]
fn set_toast_anchor_preference_persists_across_restart() {
    let dir = TempDir::new().expect("temp dir should be created");
    let path = dir.path().to_path_buf();

    let mut app = GraphBrowserApp::new_from_dir(path.clone());
    app.set_toast_anchor_preference(ToastAnchorPreference::TopRight);
    drop(app);

    let reopened = GraphBrowserApp::new_from_dir(path);
    assert_eq!(
        reopened.workspace.chrome_ui.toast_anchor_preference,
        ToastAnchorPreference::TopRight
    );
}

#[test]
fn set_shortcut_bindings_persist_across_restart() {
    let dir = TempDir::new().expect("temp dir should be created");
    let path = dir.path().to_path_buf();

    let mut app = GraphBrowserApp::new_from_dir(path.clone());
    app.set_command_palette_shortcut(CommandPaletteShortcut::CtrlK);
    app.set_help_panel_shortcut(HelpPanelShortcut::H);
    app.set_radial_menu_shortcut(RadialMenuShortcut::R);
    drop(app);

    let reopened = GraphBrowserApp::new_from_dir(path);
    assert_eq!(
        reopened.workspace.chrome_ui.command_palette_shortcut,
        CommandPaletteShortcut::CtrlK
    );
    assert_eq!(
        reopened.workspace.chrome_ui.help_panel_shortcut,
        HelpPanelShortcut::H
    );
    assert_eq!(
        reopened.workspace.chrome_ui.radial_menu_shortcut,
        RadialMenuShortcut::R
    );
}

#[test]
fn set_lasso_binding_preference_persists_across_restart() {
    let dir = TempDir::new().expect("temp dir should be created");
    let path = dir.path().to_path_buf();

    let mut app = GraphBrowserApp::new_from_dir(path.clone());
    app.set_lasso_binding_preference(CanvasLassoBinding::ShiftLeftDrag);
    drop(app);

    let reopened = GraphBrowserApp::new_from_dir(path);
    assert_eq!(
        reopened.lasso_binding_preference(),
        CanvasLassoBinding::ShiftLeftDrag
    );
}

#[test]
fn set_input_binding_remaps_persist_across_restart() {
    let dir = TempDir::new().expect("temp dir should be created");
    let path = dir.path().to_path_buf();
    let remaps = [InputBindingRemap {
        old: InputBinding::Gamepad {
            button: GamepadButton::South,
            modifier: None,
        },
        new: InputBinding::Gamepad {
            button: GamepadButton::East,
            modifier: None,
        },
        context: InputContext::GraphView,
    }];

    let mut app = GraphBrowserApp::new_from_dir(path.clone());
    app.set_input_binding_remaps(&remaps)
        .expect("remaps should persist");
    drop(app);

    // Verify persistence by reading remaps back from the reopened app's stored state
    // (avoids racing against other tests that share the global input registry).
    let reopened = GraphBrowserApp::new_from_dir(path);
    let loaded_remaps = reopened.input_binding_remaps();
    assert_eq!(
        loaded_remaps.len(),
        1,
        "reopened app should have one persisted remap"
    );
    assert_eq!(loaded_remaps[0].old, remaps[0].old);
    assert_eq!(loaded_remaps[0].new, remaps[0].new);
    assert_eq!(loaded_remaps[0].context, remaps[0].context);
}

#[test]
fn grouped_tiles_frame_bundle_round_trip_restores_group_and_members() {
    let dir = TempDir::new().expect("temp dir should be created");
    let mut app = GraphBrowserApp::new_from_dir(dir.path().to_path_buf());
    let registry = WorkbenchSurfaceRegistry::default();
    let view_id = GraphViewId::new();

    let left_node = app.add_node_and_sync(
        "https://group-left.example".into(),
        euclid::default::Point2D::new(0.0, 0.0),
    );
    let right_node = app.add_node_and_sync(
        "https://group-right.example".into(),
        euclid::default::Point2D::new(80.0, 0.0),
    );

    let mut tiles = Tiles::default();
    let graph_tile = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(view_id)));
    let left_tile = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(left_node)));
    let right_tile = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(right_node)));
    let graph_leaf = tiles.insert_tab_tile(vec![graph_tile]);
    let left_leaf = tiles.insert_tab_tile(vec![left_tile]);
    let right_leaf = tiles.insert_tab_tile(vec![right_tile]);
    let root = tiles.insert_horizontal_tile(vec![graph_leaf, left_leaf, right_leaf]);
    let mut tree = Tree::new("grouped_tiles_round_trip", root, tiles);

    registry.dispatch_intent(
        &mut app,
        &mut tree,
        WorkbenchIntent::UpdateTileSelection {
            tile_id: graph_tile,
            mode: SelectionUpdateMode::Replace,
        },
    );
    registry.dispatch_intent(
        &mut app,
        &mut tree,
        WorkbenchIntent::UpdateTileSelection {
            tile_id: left_tile,
            mode: SelectionUpdateMode::Add,
        },
    );
    registry.dispatch_intent(
        &mut app,
        &mut tree,
        WorkbenchIntent::UpdateTileSelection {
            tile_id: right_tile,
            mode: SelectionUpdateMode::Add,
        },
    );
    registry.dispatch_intent(&mut app, &mut tree, WorkbenchIntent::GroupSelectedTiles);

    let frame_name = "workspace-grouped-roundtrip";
    save_named_workspace_bundle(&mut app, frame_name, &tree)
        .expect("grouped frame bundle should save");
    let bundle = load_named_workspace_bundle(&app, frame_name)
        .expect("saved grouped frame bundle should load");
    let (restored, restored_nodes) = restore_runtime_tree_from_workspace_bundle(&app, &bundle)
        .expect("grouped frame bundle should restore runtime tree");

    assert!(
        restored.root().is_some(),
        "restored tree should have a root"
    );
    assert!(restored_nodes.contains(&left_node));
    assert!(restored_nodes.contains(&right_node));

    let graph_panes = restored
        .tiles
        .iter()
        .filter(|(_, tile)| matches!(tile, Tile::Pane(TileKind::Graph(_))))
        .count();
    let node_panes = restored
        .tiles
        .iter()
        .filter(|(_, tile)| matches!(tile, Tile::Pane(TileKind::Node(_))))
        .count();
    let grouped_tabs = restored.tiles.iter().any(|(_, tile)| {
        matches!(tile, Tile::Container(Container::Tabs(tabs)) if tabs.children.len() >= 3)
    });

    assert_eq!(graph_panes, 1, "restored tree should retain one graph pane");
    assert_eq!(
        node_panes, 2,
        "restored tree should retain grouped node panes"
    );
    assert!(
        grouped_tabs,
        "restored tree should include a grouped tabs container with selected members"
    );
}
