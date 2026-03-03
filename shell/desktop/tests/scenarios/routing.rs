use super::super::harness::TestRegistry;
use crate::app::{GraphIntent, PendingNodeOpenRequest, PendingTileOpenMode, WorkspaceOpenAction};
use crate::util::{GraphshellAddress, GraphshellSettingsPath, NodeAddress, NoteAddress};
use std::collections::{BTreeSet, HashMap};

#[test]
fn open_node_frame_routed_falls_back_to_current_frame_for_zero_membership() {
    let mut harness = TestRegistry::new();
    let key = harness.add_node("https://example.com");

    harness
        .app
        .apply_intents([GraphIntent::OpenNodeFrameRouted {
            key,
            prefer_frame: None,
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
fn open_node_frame_routed_with_preferred_frame_requests_restore() {
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

    harness
        .app
        .apply_intents([GraphIntent::OpenNodeFrameRouted {
            key,
            prefer_frame: Some("alpha".to_string()),
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
fn remove_selected_nodes_clears_frame_membership_entry() {
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
fn resolve_frame_open_prefers_recent_membership() {
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
        WorkspaceOpenAction::RestoreFrame {
            name: "beta".to_string(),
            node: key,
        }
    );
}

#[test]
fn resolve_frame_open_honors_preferred_frame() {
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
        WorkspaceOpenAction::RestoreFrame {
            name: "alpha".to_string(),
            node: key,
        }
    );
}

#[test]
fn set_node_url_preserves_frame_membership() {
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
        &BTreeSet::from(["workspace-alpha".to_string(), "workspace-beta".to_string()])
    );
}

#[test]
fn open_settings_url_history_does_not_use_legacy_history_flag() {
    let mut harness = TestRegistry::new();
    let node = harness.add_node("https://example.com");
    harness.app.select_node(node, false);
    let was_running = harness.app.workspace.physics.base.is_running;

    harness.app.apply_intents([
        GraphIntent::SetNodeUrl {
            key: node,
            new_url: GraphshellAddress::settings(GraphshellSettingsPath::History).to_string(),
        },
        GraphIntent::OpenSettingsUrl {
            url: GraphshellAddress::settings(GraphshellSettingsPath::History).to_string(),
        },
    ]);

    assert_eq!(harness.app.workspace.physics.base.is_running, was_running);
}

#[test]
fn open_settings_url_physics_is_not_reducer_owned() {
    let mut harness = TestRegistry::new();
    let node = harness.add_node("https://example.com");
    harness.app.select_node(node, false);
    let was_running = harness.app.workspace.physics.base.is_running;

    harness.app.apply_intents([
        GraphIntent::SetNodeUrl {
            key: node,
            new_url: GraphshellAddress::settings(GraphshellSettingsPath::Physics).to_string(),
        },
        GraphIntent::OpenSettingsUrl {
            url: GraphshellAddress::settings(GraphshellSettingsPath::Physics).to_string(),
        },
    ]);

    assert_eq!(harness.app.workspace.physics.base.is_running, was_running);
}

#[test]
fn open_settings_url_persistence_does_not_use_legacy_persistence_flag() {
    let mut harness = TestRegistry::new();
    let node = harness.add_node("https://example.com");
    harness.app.select_node(node, false);
    let was_running = harness.app.workspace.physics.base.is_running;

    harness.app.apply_intents([
        GraphIntent::SetNodeUrl {
            key: node,
            new_url: GraphshellAddress::settings(GraphshellSettingsPath::Persistence).to_string(),
        },
        GraphIntent::OpenSettingsUrl {
            url: GraphshellAddress::settings(GraphshellSettingsPath::Persistence).to_string(),
        },
    ]);

    assert_eq!(harness.app.workspace.physics.base.is_running, was_running);
}

#[test]
fn open_clip_url_is_not_reducer_owned() {
    let mut harness = TestRegistry::new();
    let node = harness.add_node("https://example.com");
    harness.app.select_node(node, false);
    let node_count_before = harness.app.workspace.graph.node_count();

    harness.app.apply_intents([
        GraphIntent::SetNodeUrl {
            key: node,
            new_url: GraphshellAddress::clip("clip-123").to_string(),
        },
        GraphIntent::OpenClipUrl {
            url: GraphshellAddress::clip("clip-123").to_string(),
        },
    ]);

    assert_eq!(harness.app.workspace.graph.node_count(), node_count_before);
    assert_eq!(
        harness.app.workspace.graph.get_node(node).expect("node exists").url,
        GraphshellAddress::clip("clip-123").to_string()
    );
    assert!(harness.app.take_pending_open_clip_request().is_none());
}

#[test]
fn resolve_clip_route_accepts_legacy_scheme_and_normalizes() {
    let resolved = crate::app::GraphBrowserApp::resolve_clip_route("graphshell://clip/legacy-clip");
    assert_eq!(resolved.as_deref(), Some("legacy-clip"));

    let unresolved = crate::app::GraphBrowserApp::resolve_clip_route("verso://clip");
    assert!(unresolved.is_none());
}

#[test]
fn open_node_url_is_not_reducer_owned() {
    let mut harness = TestRegistry::new();
    let node = harness.add_node("https://example.com");
    harness.app.select_node(node, false);
    let node_id = harness
        .app
        .workspace
        .graph
        .get_node(node)
        .expect("node exists")
        .id;
    let node_count_before = harness.app.workspace.graph.node_count();
    let node_url = NodeAddress::node(node_id.to_string()).to_string();

    harness.app.apply_intents([
        GraphIntent::SetNodeUrl {
            key: node,
            new_url: node_url.clone(),
        },
        GraphIntent::OpenNodeUrl {
            url: node_url.clone(),
        },
    ]);

    assert_eq!(harness.app.workspace.graph.node_count(), node_count_before);
    assert_eq!(
        harness.app.workspace.graph.get_node(node).expect("node exists").url,
        node_url
    );
    assert!(harness.app.take_pending_open_node_request().is_none());
}

#[test]
fn open_note_url_is_not_reducer_owned() {
    let mut harness = TestRegistry::new();
    let node = harness.add_node("https://example.com");
    harness.app.select_node(node, false);
    let note_id = harness
        .app
        .create_note_for_node(node, Some("Routing note".to_string()))
        .expect("note should exist");
    let _ = harness.app.take_pending_open_note_request();
    let node_count_before = harness.app.workspace.graph.node_count();
    let note_url = NoteAddress::note(note_id.as_uuid().to_string()).to_string();

    harness.app.apply_intents([GraphIntent::OpenNoteUrl {
        url: note_url.clone(),
    }]);

    assert_eq!(harness.app.workspace.graph.node_count(), node_count_before);
    assert!(harness.app.take_pending_open_note_request().is_none());
}
