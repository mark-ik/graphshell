use super::super::harness::TestRegistry;
use crate::app::{
    GraphIntent, PendingNodeOpenRequest, PendingTileOpenMode, WorkspaceOpenAction,
};
use std::collections::{BTreeSet, HashMap};

#[test]
fn open_node_workspace_routed_falls_back_to_current_workspace_for_zero_membership() {
    let mut harness = TestRegistry::new();
    let key = harness.add_node("https://example.com");

    harness.app.apply_intents([GraphIntent::OpenNodeWorkspaceRouted {
        key,
        prefer_workspace: None,
    }]);

    assert_eq!(harness.app.get_single_selected_node(), Some(key));
    assert_eq!(
        harness.app.take_pending_open_node_request(),
        Some(PendingNodeOpenRequest {
            key,
            mode: PendingTileOpenMode::Tab,
        })
    );
    assert!(
        harness
            .app
            .take_pending_restore_workspace_snapshot_named()
            .is_none()
    );
}

#[test]
fn open_node_workspace_routed_with_preferred_workspace_requests_restore() {
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
    index.insert(
        node_id,
        BTreeSet::from(["alpha".to_string(), "beta".to_string()]),
    );
    harness.app.init_membership_index(index);
    harness.app.note_workspace_activated("beta", [key]);

    harness.app.apply_intents([GraphIntent::OpenNodeWorkspaceRouted {
        key,
        prefer_workspace: Some("alpha".to_string()),
    }]);

    assert_eq!(
        harness.app.take_pending_restore_workspace_snapshot_named(),
        Some("alpha".to_string())
    );
    assert_eq!(
        harness.app.take_pending_workspace_restore_open_request(),
        Some(PendingNodeOpenRequest {
            key,
            mode: PendingTileOpenMode::Tab,
        })
    );
}

#[test]
fn remove_selected_nodes_clears_workspace_membership_entry() {
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
    index.insert(node_id, BTreeSet::from(["saved-workspace".to_string()]));
    harness.app.init_membership_index(index);

    harness.app.select_node(key, false);
    harness.app.remove_selected_nodes();

    assert!(harness.app.membership_for_node(node_id).is_empty());
}

#[test]
fn resolve_workspace_open_prefers_recent_membership() {
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
    index.insert(
        node_id,
        BTreeSet::from(["alpha".to_string(), "beta".to_string()]),
    );
    harness.app.init_membership_index(index);
    harness.app.note_workspace_activated("beta", [key]);

    assert_eq!(
        harness.app.resolve_workspace_open(key, None),
        WorkspaceOpenAction::RestoreWorkspace {
            name: "beta".to_string(),
            node: key,
        }
    );
}

#[test]
fn resolve_workspace_open_honors_preferred_workspace() {
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
    index.insert(
        node_id,
        BTreeSet::from(["alpha".to_string(), "beta".to_string()]),
    );
    harness.app.init_membership_index(index);
    harness.app.note_workspace_activated("beta", [key]);

    assert_eq!(
        harness.app.resolve_workspace_open(key, Some("alpha")),
        WorkspaceOpenAction::RestoreWorkspace {
            name: "alpha".to_string(),
            node: key,
        }
    );
}

#[test]
fn set_node_url_preserves_workspace_membership() {
    let mut harness = TestRegistry::new();
    let key = harness.add_node("https://before.example");
    let node_id = harness
        .app
        .workspace
        .graph
        .get_node(key)
        .expect("node should exist")
        .id;

    let mut index = HashMap::new();
    index.insert(
        node_id,
        BTreeSet::from(["workspace-alpha".to_string(), "workspace-beta".to_string()]),
    );
    harness.app.init_membership_index(index);

    harness.app.apply_intents([GraphIntent::SetNodeUrl {
        key,
        new_url: "https://after.example".to_string(),
    }]);

    assert_eq!(
        harness
            .app
            .workspace
            .graph
            .get_node(key)
            .expect("node should exist")
            .url,
        "https://after.example"
    );
    assert_eq!(
        harness.app.membership_for_node(node_id),
        &BTreeSet::from([
            "workspace-alpha".to_string(),
            "workspace-beta".to_string()
        ])
    );
}

#[test]
fn open_settings_url_history_activates_history_manager_surface() {
    let mut harness = TestRegistry::new();
    let node = harness.add_node("https://example.com");
    harness.app.select_node(node, false);

    harness.app.apply_intents([
        GraphIntent::SetNodeUrl {
            key: node,
            new_url: "graphshell://settings/history".to_string(),
        },
        GraphIntent::OpenSettingsUrl {
            url: "graphshell://settings/history".to_string(),
        },
    ]);

    assert!(harness.app.workspace.show_history_manager);
    assert!(!harness.app.workspace.show_physics_panel);
    assert!(!harness.app.workspace.show_persistence_panel);
}

#[test]
fn open_settings_url_physics_activates_physics_surface() {
    let mut harness = TestRegistry::new();
    let node = harness.add_node("https://example.com");
    harness.app.select_node(node, false);

    harness.app.apply_intents([
        GraphIntent::SetNodeUrl {
            key: node,
            new_url: "graphshell://settings/physics".to_string(),
        },
        GraphIntent::OpenSettingsUrl {
            url: "graphshell://settings/physics".to_string(),
        },
    ]);

    assert!(harness.app.workspace.show_physics_panel);
    assert!(!harness.app.workspace.show_history_manager);
    assert!(!harness.app.workspace.show_persistence_panel);
}
