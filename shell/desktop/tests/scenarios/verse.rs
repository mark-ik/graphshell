use super::super::harness::TestRegistry;
use crate::mods::native::verse::{AccessLevel, PeerRole, TrustedPeer, WorkspaceGrant};
use crate::shell::desktop::runtime::registries;

fn make_peer(node_id: iroh::EndpointId, workspace_id: &str, access: AccessLevel) -> TrustedPeer {
    TrustedPeer {
        node_id,
        display_name: "test-peer".to_string(),
        role: PeerRole::Friend,
        added_at: std::time::SystemTime::UNIX_EPOCH,
        last_seen: None,
        workspace_grants: vec![WorkspaceGrant {
            workspace_id: workspace_id.to_string(),
            access,
        }],
    }
}

// P2.a — RO/RW grant matrix

#[test]
fn verse_access_control_rw_peer_with_mutations_is_allowed() {
    let mut harness = TestRegistry::new();
    let peer_id = crate::mods::native::verse::generate_p2p_secret_key().public();
    let peers = vec![make_peer(peer_id, "workspace-w", AccessLevel::ReadWrite)];

    let allowed = registries::phase5_check_verse_workspace_sync_access_for_tests(
        &harness.diagnostics,
        &peers,
        peer_id,
        "workspace-w",
        true,
    );

    assert!(allowed, "ReadWrite peer with mutations should be allowed");

    let snapshot = harness.snapshot();
    assert_eq!(
        TestRegistry::channel_count(&snapshot, registries::CHANNEL_VERSE_SYNC_ACCESS_DENIED),
        0,
        "ReadWrite peer should not emit access_denied"
    );
}

#[test]
fn verse_access_control_ro_peer_with_mutations_emits_access_denied() {
    let mut harness = TestRegistry::new();
    let peer_id = crate::mods::native::verse::generate_p2p_secret_key().public();
    let peers = vec![make_peer(peer_id, "workspace-w", AccessLevel::ReadOnly)];

    let allowed = registries::phase5_check_verse_workspace_sync_access_for_tests(
        &harness.diagnostics,
        &peers,
        peer_id,
        "workspace-w",
        true,
    );

    assert!(!allowed, "ReadOnly peer with mutations should be denied");

    let snapshot = harness.snapshot();
    assert!(
        TestRegistry::channel_count(&snapshot, registries::CHANNEL_VERSE_SYNC_ACCESS_DENIED) > 0,
        "ReadOnly peer with mutations should emit access_denied"
    );
}

#[test]
fn verse_access_control_ro_peer_without_mutations_is_allowed() {
    let mut harness = TestRegistry::new();
    let peer_id = crate::mods::native::verse::generate_p2p_secret_key().public();
    let peers = vec![make_peer(peer_id, "workspace-w", AccessLevel::ReadOnly)];

    let allowed = registries::phase5_check_verse_workspace_sync_access_for_tests(
        &harness.diagnostics,
        &peers,
        peer_id,
        "workspace-w",
        false,
    );

    assert!(
        allowed,
        "ReadOnly peer without mutations should be allowed to receive"
    );

    let snapshot = harness.snapshot();
    assert_eq!(
        TestRegistry::channel_count(&snapshot, registries::CHANNEL_VERSE_SYNC_ACCESS_DENIED),
        0,
        "ReadOnly peer without mutations should not emit access_denied"
    );
}

// P2.c — revoke / forget / ungranted inbound sync

#[test]
fn verse_access_control_ungranted_peer_emits_access_denied() {
    let mut harness = TestRegistry::new();
    let peer_id = crate::mods::native::verse::generate_p2p_secret_key().public();
    // Peer is trusted but holds no grant for "workspace-w"
    let peer = TrustedPeer {
        node_id: peer_id,
        display_name: "no-grant-peer".to_string(),
        role: PeerRole::Friend,
        added_at: std::time::SystemTime::UNIX_EPOCH,
        last_seen: None,
        workspace_grants: vec![],
    };

    let allowed = registries::phase5_check_verse_workspace_sync_access_for_tests(
        &harness.diagnostics,
        &[peer],
        peer_id,
        "workspace-w",
        false,
    );

    assert!(!allowed, "Peer without workspace grant should be denied");

    let snapshot = harness.snapshot();
    assert!(
        TestRegistry::channel_count(&snapshot, registries::CHANNEL_VERSE_SYNC_ACCESS_DENIED) > 0,
        "Peer without workspace grant should emit access_denied"
    );
}

#[test]
fn verse_access_control_wrong_workspace_grant_emits_access_denied() {
    let mut harness = TestRegistry::new();
    let peer_id = crate::mods::native::verse::generate_p2p_secret_key().public();
    let peers = vec![make_peer(
        peer_id,
        "workspace-other",
        AccessLevel::ReadWrite,
    )];

    let allowed = registries::phase5_check_verse_workspace_sync_access_for_tests(
        &harness.diagnostics,
        &peers,
        peer_id,
        "workspace-w",
        true,
    );

    assert!(
        !allowed,
        "Peer granted on another workspace should be denied for workspace-w"
    );

    let snapshot = harness.snapshot();
    assert_eq!(
        TestRegistry::channel_count(&snapshot, registries::CHANNEL_VERSE_SYNC_ACCESS_DENIED),
        1,
        "Wrong-workspace grant should emit one access_denied event"
    );
}

#[test]
fn verse_access_control_target_workspace_ro_denies_mutations_even_with_rw_elsewhere() {
    let mut harness = TestRegistry::new();
    let peer_id = crate::mods::native::verse::generate_p2p_secret_key().public();
    let peers = vec![TrustedPeer {
        node_id: peer_id,
        display_name: "mixed-grant-peer".to_string(),
        role: PeerRole::Friend,
        added_at: std::time::SystemTime::UNIX_EPOCH,
        last_seen: None,
        workspace_grants: vec![
            WorkspaceGrant {
                workspace_id: "workspace-other".to_string(),
                access: AccessLevel::ReadWrite,
            },
            WorkspaceGrant {
                workspace_id: "workspace-w".to_string(),
                access: AccessLevel::ReadOnly,
            },
        ],
    }];

    let allowed = registries::phase5_check_verse_workspace_sync_access_for_tests(
        &harness.diagnostics,
        &peers,
        peer_id,
        "workspace-w",
        true,
    );

    assert!(
        !allowed,
        "Target-workspace ReadOnly grant should deny mutating syncs even if another workspace is ReadWrite"
    );

    let snapshot = harness.snapshot();
    assert_eq!(
        TestRegistry::channel_count(&snapshot, registries::CHANNEL_VERSE_SYNC_ACCESS_DENIED),
        1,
        "Target-workspace ReadOnly grant should emit one access_denied event"
    );
}

#[test]
fn verse_access_control_unknown_peer_emits_access_denied() {
    let mut harness = TestRegistry::new();
    let peer_id = crate::mods::native::verse::generate_p2p_secret_key().public();
    let peers: Vec<TrustedPeer> = vec![]; // empty trust store

    let allowed = registries::phase5_check_verse_workspace_sync_access_for_tests(
        &harness.diagnostics,
        &peers,
        peer_id,
        "workspace-w",
        false,
    );

    assert!(!allowed, "Peer absent from trust store should be denied");

    let snapshot = harness.snapshot();
    assert!(
        TestRegistry::channel_count(&snapshot, registries::CHANNEL_VERSE_SYNC_ACCESS_DENIED) > 0,
        "Peer absent from trust store should emit access_denied"
    );
}

#[test]
fn verse_access_control_revoke_removes_grant_and_emits_access_denied() {
    let mut harness = TestRegistry::new();
    let peer_id = crate::mods::native::verse::generate_p2p_secret_key().public();

    // Before revoke: peer has ReadWrite access
    let mut peers = vec![make_peer(peer_id, "workspace-w", AccessLevel::ReadWrite)];

    let allowed_before = registries::phase5_check_verse_workspace_sync_access_for_tests(
        &harness.diagnostics,
        &peers,
        peer_id,
        "workspace-w",
        true,
    );
    assert!(allowed_before, "Peer should be allowed before revoke");

    // Revoke: remove peer from the trust store
    peers.retain(|p| p.node_id != peer_id);

    let allowed_after = registries::phase5_check_verse_workspace_sync_access_for_tests(
        &harness.diagnostics,
        &peers,
        peer_id,
        "workspace-w",
        true,
    );
    assert!(!allowed_after, "Peer should be denied after revoke");

    let snapshot = harness.snapshot();
    assert!(
        TestRegistry::channel_count(&snapshot, registries::CHANNEL_VERSE_SYNC_ACCESS_DENIED) > 0,
        "Revoked peer should emit access_denied"
    );
}

/// Deny-path must not mutate graph state (P2.b behavioural guarantee).
#[test]
fn verse_access_control_deny_does_not_mutate_graph_state() {
    let mut harness = TestRegistry::new();

    let _node = harness.add_node("https://example.com");
    let node_count_before = harness.app.workspace.domain.graph.node_count();

    let peer_id = crate::mods::native::verse::generate_p2p_secret_key().public();
    let peers: Vec<TrustedPeer> = vec![];

    let allowed = registries::phase5_check_verse_workspace_sync_access_for_tests(
        &harness.diagnostics,
        &peers,
        peer_id,
        "workspace-w",
        true,
    );
    assert!(!allowed, "Denied peer should return false");

    assert_eq!(
        node_count_before,
        harness.app.workspace.domain.graph.node_count(),
        "Graph state must not be mutated when sync access is denied"
    );
}

