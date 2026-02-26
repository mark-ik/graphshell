use super::super::harness::TestRegistry;
use crate::app::GraphBrowserApp;
use crate::app::GraphIntent;
use crate::app::ToastAnchorPreference;
use crate::services::persistence::types::LogEntry;
use crate::services::persistence::GraphStore;
use std::collections::{BTreeSet, HashMap};
use std::time::Duration;
use tempfile::TempDir;
use uuid::Uuid;

#[test]
fn open_node_workspace_routed_preserves_unsaved_prompt_state_until_restore() {
    let mut harness = TestRegistry::new();
    let key = harness.add_node("https://example.com");
    let node_id = harness
        .app
        .workspace
        .graph
        .get_node(key)
        .expect("node should exist")
        .id;

    let mut index = HashMap::new();
    index.insert(node_id, BTreeSet::from(["workspace-alpha".to_string()]));
    harness.app.init_membership_index(index);
    harness.app.mark_current_workspace_synthesized();
    harness.app.apply_intents([GraphIntent::CreateNodeNearCenter]);

    assert!(harness.app.should_prompt_unsaved_workspace_save());

    harness.app.apply_intents([GraphIntent::OpenNodeWorkspaceRouted {
        key,
        prefer_workspace: None,
    }]);

    assert_eq!(
        harness.app.take_pending_restore_workspace_snapshot_named(),
        Some("workspace-alpha".to_string())
    );
    assert!(harness.app.should_prompt_unsaved_workspace_save());
}

#[test]
fn workspace_has_unsaved_changes_for_graph_mutations() {
    let mut harness = TestRegistry::new();
    harness.app.mark_current_workspace_synthesized();

    harness.app.apply_intents([GraphIntent::CreateNodeNearCenter]);

    assert!(harness.app.should_prompt_unsaved_workspace_save());
}

#[test]
fn workspace_modified_for_graph_mutations_even_when_not_synthesized() {
    let mut harness = TestRegistry::new();

    assert!(!harness.app.should_prompt_unsaved_workspace_save());
    harness.app.apply_intents([GraphIntent::CreateNodeNearCenter]);

    assert!(harness.app.should_prompt_unsaved_workspace_save());
}

#[test]
fn unsaved_prompt_warning_resets_on_additional_graph_mutation() {
    let mut harness = TestRegistry::new();
    harness.app.mark_current_workspace_synthesized();
    harness.app.apply_intents([GraphIntent::CreateNodeNearCenter]);

    assert!(harness.app.consume_unsaved_workspace_prompt_warning());
    assert!(!harness.app.consume_unsaved_workspace_prompt_warning());

    harness.app.apply_intents([GraphIntent::CreateNodeNearCenter]);

    assert!(harness.app.consume_unsaved_workspace_prompt_warning());
}

#[test]
fn save_named_workspace_clears_unsaved_prompt_state() {
    let dir = TempDir::new().expect("temp dir should be created");
    let mut app = GraphBrowserApp::new_from_dir(dir.path().to_path_buf());
    app.mark_current_workspace_synthesized();
    app.apply_intents([GraphIntent::CreateNodeNearCenter]);

    assert!(app.should_prompt_unsaved_workspace_save());
    assert!(app.consume_unsaved_workspace_prompt_warning());

    app.save_workspace_layout_json("workspace:user-saved", "{\"root\":null}");

    assert!(!app.should_prompt_unsaved_workspace_save());
    assert!(!app.consume_unsaved_workspace_prompt_warning());
}

#[test]
fn workspace_not_modified_for_non_graph_mutations() {
    let mut harness = TestRegistry::new();
    let key = harness.add_node("https://example.com");
    harness.app.mark_current_workspace_synthesized();

    harness.app.apply_intents([GraphIntent::SelectNode {
        key,
        multi_select: false,
    }]);

    assert!(!harness.app.should_prompt_unsaved_workspace_save());
}

#[test]
fn workspace_not_modified_for_set_node_position() {
    let mut harness = TestRegistry::new();
    let key = harness.add_node("https://example.com");
    harness.app.mark_current_workspace_synthesized();

    harness.app.apply_intents([GraphIntent::SetNodePosition {
        key,
        position: euclid::Point2D::new(42.0, 24.0),
    }]);

    assert!(!harness.app.should_prompt_unsaved_workspace_save());
}

#[test]
fn workspace_has_unsaved_changes_for_set_node_pinned() {
    let mut harness = TestRegistry::new();
    let key = harness.add_node("https://example.com");
    harness.app.mark_current_workspace_synthesized();

    harness.app.apply_intents([GraphIntent::SetNodePinned {
        key,
        is_pinned: true,
    }]);

    assert!(harness.app.should_prompt_unsaved_workspace_save());
}

#[test]
fn session_workspace_blob_autosave_uses_runtime_layout_hash_and_caches_runtime_layout() {
    let dir = TempDir::new().expect("temp dir should be created");
    let mut app = GraphBrowserApp::new_from_dir(dir.path().to_path_buf());
    app.set_workspace_autosave_interval_secs(1)
        .expect("autosave interval should be configurable");

    app.save_session_workspace_layout_blob_if_changed("bundle-json-v1", "runtime-layout-v1");
    assert_eq!(
        app.load_workspace_layout_json(GraphBrowserApp::SESSION_WORKSPACE_LAYOUT_NAME)
            .as_deref(),
        Some("bundle-json-v1")
    );
    assert_eq!(
        app.last_session_workspace_layout_json(),
        Some("runtime-layout-v1")
    );

    std::thread::sleep(Duration::from_millis(1100));
    app.save_session_workspace_layout_blob_if_changed("bundle-json-v2", "runtime-layout-v1");

    assert_eq!(
        app.load_workspace_layout_json(GraphBrowserApp::SESSION_WORKSPACE_LAYOUT_NAME)
            .as_deref(),
        Some("bundle-json-v1")
    );
    assert_eq!(
        app.last_session_workspace_layout_json(),
        Some("runtime-layout-v1")
    );
    assert_eq!(app.list_workspace_layout_names().len(), 1);
}

#[test]
fn session_workspace_blob_autosave_rotates_previous_latest_bundle_on_layout_change() {
    let dir = TempDir::new().expect("temp dir should be created");
    let mut app = GraphBrowserApp::new_from_dir(dir.path().to_path_buf());
    app.set_workspace_autosave_interval_secs(1)
        .expect("autosave interval should be configurable");
    app.set_workspace_autosave_retention(2)
        .expect("retention setting should succeed");

    app.save_session_workspace_layout_blob_if_changed("bundle-json-a", "runtime-layout-a");
    std::thread::sleep(Duration::from_millis(1100));
    app.save_session_workspace_layout_blob_if_changed("bundle-json-b", "runtime-layout-b");

    assert_eq!(
        app.load_workspace_layout_json(GraphBrowserApp::SESSION_WORKSPACE_LAYOUT_NAME)
            .as_deref(),
        Some("bundle-json-b")
    );
    let history_name = app
        .list_workspace_layout_names()
        .into_iter()
        .find(|name| name != GraphBrowserApp::SESSION_WORKSPACE_LAYOUT_NAME)
        .expect("rotating autosave should persist one history entry");
    assert_eq!(
        app.load_workspace_layout_json(&history_name).as_deref(),
        Some("bundle-json-a")
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
        });
    }
    {
        let mut store_b = GraphStore::open(path_b.clone()).expect("store b should open");
        store_b.log_mutation(&LogEntry::AddNode {
            node_id: Uuid::new_v4().to_string(),
            url: "https://from-b.com".to_string(),
            position_x: 3.0,
            position_y: 4.0,
        });
        store_b.log_mutation(&LogEntry::AddNode {
            node_id: Uuid::new_v4().to_string(),
            url: "about:blank#7".to_string(),
            position_x: 5.0,
            position_y: 6.0,
        });
    }

    let mut app = GraphBrowserApp::new_from_dir(path_a);
    assert!(app.workspace.graph.get_node_by_url("https://from-a.com").is_some());
    assert!(app.workspace.graph.get_node_by_url("https://from-b.com").is_none());

    app.switch_persistence_dir(path_b)
        .expect("switching persistence dir should succeed");

    assert!(app.workspace.graph.get_node_by_url("https://from-a.com").is_none());
    assert!(app.workspace.graph.get_node_by_url("https://from-b.com").is_some());
    assert!(app.workspace.selected_nodes.is_empty());

    let new_placeholder = app.create_new_node_near_center();
    assert_eq!(
        app.workspace
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
        reopened.workspace.toast_anchor_preference,
        ToastAnchorPreference::TopRight
    );
}
