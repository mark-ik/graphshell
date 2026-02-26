use super::super::harness::TestRegistry;
use crate::mods::native::verse::{SyncLog, SyncedIntent};
use crate::services::persistence::types::LogEntry;
use crate::registries::atomic::diagnostics;
use crate::shell::desktop::runtime::registries;

// Helper to generate a deterministic test NodeId without network I/O
fn test_peer_id() -> iroh::NodeId {
    iroh::SecretKey::generate(&mut rand::thread_rng()).public()
}

#[test]
fn two_instance_node_sync() {
    // Simulate peer A creating a node and peer B receiving it via sync log merge.
    let peer_a = test_peer_id();
    let peer_b = test_peer_id();

    let mut log_a = SyncLog::new("workspace-test".to_string());
    let mut log_b = SyncLog::new("workspace-test".to_string());

    // Peer A records an AddNode intent
    let add_node = SyncedIntent {
        log_entry: LogEntry::AddNode {
            node_id: "node-shared-1".to_string(),
            url: "https://example.com".to_string(),
            position_x: 10.0,
            position_y: 20.0,
        },
        authored_by: peer_a,
        authored_at_secs: 1000,
        sequence: 1,
    };

    assert!(log_a.record_intent(add_node.clone()), "peer A should record intent");

    // Peer B receives the delta from peer A
    let should_apply = log_b.should_apply(&add_node);
    assert!(should_apply, "node add should be applicable on peer B (no tombstone)");

    let recorded = log_b.record_intent(add_node);
    assert!(recorded, "peer B should record the new intent");

    // Verify version vectors advance correctly
    assert_eq!(log_b.version_vector.get(peer_a), 1);
    assert_eq!(log_b.version_vector.get(peer_b), 0);

    // A node from peer A must exist in B's intent history
    assert!(
        log_b.intents.iter().any(|i| matches!(
            &i.log_entry,
            LogEntry::AddNode { node_id, .. } if node_id == "node-shared-1"
        )),
        "node created on peer A should appear in peer B's sync log"
    );
}

#[test]
fn rename_conflict_resolves_deterministically_with_lww() {
    // Two peers rename the same node concurrently; the later timestamp wins.
    let peer_a = test_peer_id();
    let peer_b = test_peer_id();

    let mut log_a = SyncLog::new("workspace-test".to_string());

    // Peer A applies a title update at t=200 (the newer write)
    let intent_newer = SyncedIntent {
        log_entry: LogEntry::UpdateNodeTitle {
            node_id: "node-1".to_string(),
            title: "Title from A (t=200)".to_string(),
        },
        authored_by: peer_a,
        authored_at_secs: 200,
        sequence: 1,
    };
    assert!(log_a.should_apply(&intent_newer), "newer write should be accepted");
    log_a.record_intent(intent_newer);

    // Peer B's conflicting title update arrives at t=150 (older — should lose)
    let mut harness = TestRegistry::new();

    let intent_older = SyncedIntent {
        log_entry: LogEntry::UpdateNodeTitle {
            node_id: "node-1".to_string(),
            title: "Title from B (t=150)".to_string(),
        },
        authored_by: peer_b,
        authored_at_secs: 150,
        sequence: 1,
    };

    let applied = log_a.should_apply(&intent_older);
    assert!(!applied, "LWW: older write (t=150) should be rejected when t=200 is already recorded");

    // Verify the LWW winner is still the t=200 title
    assert_eq!(
        log_a.last_write_title.get("node-1").copied().unwrap_or(0),
        200,
        "last_write_title for node-1 should remain 200 after LWW resolution"
    );

    // Conflict diagnostics must have been emitted
    let snapshot = harness.snapshot();
    assert!(
        TestRegistry::channel_count(&snapshot, registries::CHANNEL_VERSE_SYNC_CONFLICT_DETECTED) > 0,
        "verse.sync.conflict_detected channel should be emitted on LWW conflict"
    );
    assert!(
        TestRegistry::channel_count(&snapshot, registries::CHANNEL_VERSE_SYNC_CONFLICT_RESOLVED) > 0,
        "verse.sync.conflict_resolved channel should be emitted after LWW resolution"
    );
}

#[test]
fn phase5_conflict_channels_registered_in_diagnostics() {
    let channels = diagnostics::phase5_required_channels();
    assert!(
        channels
            .iter()
            .any(|entry| entry.channel_id == registries::CHANNEL_VERSE_SYNC_CONFLICT_DETECTED),
        "phase5 channels must include verse.sync.conflict_detected"
    );
    assert!(
        channels
            .iter()
            .any(|entry| entry.channel_id == registries::CHANNEL_VERSE_SYNC_CONFLICT_RESOLVED),
        "phase5 channels must include verse.sync.conflict_resolved"
    );
    // All registered channels must have a positive schema version
    assert!(
        channels.iter().all(|entry| entry.schema_version > 0),
        "all phase5 channels must have a positive schema version"
    );
}
