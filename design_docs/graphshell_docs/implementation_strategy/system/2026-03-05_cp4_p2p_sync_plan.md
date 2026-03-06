# CP4: P2P Device Sync — ControlPanel Integration Plan

**Date**: 2026-03-05
**Status**: In progress (worker scaffold wired; reducer sync semantics pending)
**Phase**: Control Panel Phase 4 (CP4)
**Context**: Defines how the `p2p_sync_worker` integrates into the `ControlPanel` intent pipeline. The Verso mod's device-sync protocol (iroh transport, identity, pairing, `SyncUnit` exchange, `SyncLog`) is specified in [`verso_tier1_sync_plan.md`](../../../../verse_docs/implementation_strategy/2026-02-23_verse_tier1_sync_plan.md). This doc covers the **ControlPanel boundary**: worker supervision, sync intent carrier naming, version vector persistence on `GraphBrowserApp`, and reducer handling of remote deltas.

**Related docs**:
- [`SYSTEM_REGISTER.md`](register/SYSTEM_REGISTER.md) — ControlPanel supervision model, CP1–CP3 pattern
- [`2026-02-23_verse_tier1_sync_plan.md`](../../../../verse_docs/implementation_strategy/2026-02-23_verse_tier1_sync_plan.md) — Verso device-sync protocol authority
- [`2026-02-21_lifecycle_intent_model.md`](2026-02-21_lifecycle_intent_model.md) — `GraphIntent` schema and reducer boundary
- [`coop_session_spec.md`](coop_session_spec.md) — Coop co-presence authority (host-led session semantics, roles, sharing, snapshot)
- [`SUBSYSTEM_SECURITY.md`](../subsystem_security/SUBSYSTEM_SECURITY.md) — trust/grant model (Phase 5.4/5.5)
- [`SUBSYSTEM_DIAGNOSTICS.md`](../subsystem_diagnostics/SUBSYSTEM_DIAGNOSTICS.md) — diagnostics channel conventions

---

## 1. Scope

CP4 adds one supervised worker and a remote-sync intent carrier path to the existing ControlPanel machinery established in CP1–CP3. The reducer stays 100% synchronous; all network I/O remains in the worker.

Runtime naming alignment (2026-03-06):

- Current runtime carrier: `GraphIntent::ApplyRemoteLogEntries { entries: Vec<u8> }` (stub-handled in reducer).
- Target CP4 semantic naming: `ApplyRemoteDelta` + explicit peer-offline signaling.
- This plan treats `ApplyRemoteLogEntries` as transitional naming until full CP4 reducer semantics are landed.

**Terminology lock (CP4 scope)**:
- "Sync" in this document means **Device Sync** (durable state replication across trusted devices).
- Collaborative/co-presence behavior is **Coop** and is out of scope for CP4 unless explicitly stated.

**CP4 delivers**:
- `spawn_p2p_sync_worker()` following the CP1–CP3 supervision pattern
- `GraphIntent` remote-sync carrier path (`ApplyRemoteLogEntries` runtime alias; `ApplyRemoteDelta` target naming)
- Explicit peer-offline signaling path (`MarkPeerOffline` target behavior; runtime equivalent may be emitted through diagnostics/status channels until intent variant lands)
- Version vector persistence on `GraphBrowserApp` (per-workspace `VersionVector` loaded/saved alongside workspace state)
- Reducer handling: deduplication, VV update, undo-stack bypass, diagnostics emission

**CP4 explicitly excludes** (covered in the Verso mod spec):
- iroh endpoint lifecycle, NAT traversal, ALPN dispatch
- `SyncUnit` wire format and exchange protocol
- Pairing ceremony and trust store
- `SyncLog` persistence format
- Conflict resolution UX (ghost-node pattern, conflict notice)
- Device Sync panel UX

---

## 2. Worker Supervision

### 2.1 ControlPanel extension

```rust
impl ControlPanel {
    /// CP4: Spawn the P2P sync worker (Verso mod calls this during activation).
    pub fn spawn_p2p_sync_worker(
        &mut self,
        sync_state: Arc<VersoSyncState>,  // Owned by Verso mod; holds iroh Endpoint + SyncLog
    ) -> mpsc::Sender<SyncCommand> {
        let cancel = self.cancel.clone();
        let intent_tx = self.intent_tx.clone();
        let (cmd_tx, cmd_rx) = mpsc::channel(64);

        self.workers.spawn(async move {
            let worker = P2PSyncWorker::new(sync_state, intent_tx, cmd_rx);
            worker.run(cancel).await;
        });

        cmd_tx  // Returned to Verso mod so it can send SyncCommands
    }
}
```

**Pattern alignment with CP1–CP3**:
- Worker supervised by `JoinSet` with `CancellationToken`
- Worker communicates exclusively via `intent_tx: mpsc::Sender<GraphIntent>` (no direct state mutation)
- Graceful shutdown: token cancellation → worker exits accept loop → `JoinSet` drain in `ControlPanel::shutdown()`

### 2.2 Worker spawn timing

The Verso mod calls `spawn_p2p_sync_worker()` during its activation sequence (after `ModActivated` intent lands and the iroh endpoint is initialized). This keeps the worker lifecycle tied to mod lifecycle — if the mod is deactivated, its `SyncCommand` sender is dropped and the worker exits cleanly on the next `select!` poll.

### 2.3 Worker restart policy

The `JoinSet` does not auto-restart panicking tasks by default. If `P2PSyncWorker::run` exits unexpectedly (non-cancellation):
- The worker absence is detected at the next `ControlPanel::drain_pending()` call (if needed via `JoinSet::try_join_next`)
- A `GraphIntent::MarkPeerOffline { peer_id: None, reason: SyncWorkerCrash }` is enqueued to surface the failure
- Restart is deferred to Verso mod re-activation (explicit user action or automatic mod reload)

This avoids silent failure and keeps restart policy in the mod layer, not the ControlPanel.

---

## 3. GraphIntent Carrier and Target Variants

Target CP4 naming adds two dedicated variants. Current runtime uses a transitional carrier.

```rust
/// CP4 additions to GraphIntent
pub enum GraphIntent {
    // ... existing variants ...

    /// A batch of intents received from a remote peer via the Verso sync worker.
    /// Applied in order; reducer updates the local VersionVector on success.
    /// These intents bypass the local undo stack.
    ApplyRemoteDelta {
        workspace_id: WorkspaceId,
        from_peer: NodeId,
        /// The peer's VV at the time of sending (used to detect gaps on next sync)
        peer_vv: VersionVector,
        /// Ordered batch of intents the local node has not yet applied
        intents: Vec<SyncedIntent>,
    },

    /// A peer has become unreachable. Emitted by the sync worker on connection
    /// failure, timeout, or graceful disconnect. Never omitted silently.
    MarkPeerOffline {
        peer_id: Option<NodeId>,   // None = sync worker itself crashed
        reason: PeerOfflineReason,
    },
}

pub enum PeerOfflineReason {
    ConnectionFailed,
    Timeout,
    GracefulDisconnect,
    SyncWorkerCrash,
    AccessRevoked,
}
```

### 3.1 Why a batched remote delta carrier (not one intent per `SyncedIntent`)

The Verso protocol exchanges `SyncUnit`s which may contain hundreds of `SyncedIntent`s from a catch-up batch. Enqueuing each as a separate reducer intent would:
- Fragment VV update atomicity (partial VV update if app exits mid-drain)
- Add overhead to `drain_pending()` for large syncs
- Complicate deduplication (must check mid-drain whether a later batch supersedes an earlier one)

Batching at the `SyncUnit` boundary preserves atomicity: either the whole batch lands or none of it does (reducer rolls back on error). Runtime `ApplyRemoteLogEntries` exists as this batch carrier while CP4 target naming converges.

### 3.2 Intent ordering guarantee

In the target CP4 model, the Verso worker enqueues `ApplyRemoteDelta` intents in `from_sequence` order per workspace. Current runtime staging may enqueue `ApplyRemoteLogEntries` as the carrier alias. The ControlPanel's `mpsc` channel preserves enqueue order, and the reducer applies batches in drain order. This is sufficient for VV-based convergence — out-of-order batches from different peers are handled by the VV deduplication check (§4.2), not by reordering.

---

## 4. Reducer Handling

`apply_intents()` handles the two new variants. The reducer remains synchronous and pure-state.

### 4.1 ApplyRemoteDelta

```rust
GraphIntent::ApplyRemoteDelta { workspace_id, from_peer, peer_vv, intents } => {
    let ws = state.workspace_mut(workspace_id)?;
    let local_vv = &mut ws.sync_state.local_vv;

    for synced in intents {
        // Deduplication: skip if we've already seen this sequence from this peer
        let already_seen = local_vv.clocks
            .get(&synced.authored_by)
            .copied()
            .unwrap_or(0) >= synced.sequence;

        if already_seen {
            continue;
        }

        // Apply the inner intent using the standard reducer path
        // (same code path as local intents — no special remote handling needed)
        apply_single_intent(state, synced.intent.clone(), IntentSource::RemotePeer {
            peer_id: synced.authored_by,
            sequence: synced.sequence,
        })?;

        // Advance VV for this peer
        local_vv.clocks
            .entry(synced.authored_by)
            .and_modify(|s| *s = (*s).max(synced.sequence))
            .or_insert(synced.sequence);
    }

    // Cache the peer's VV for next delta computation
    ws.sync_state.peer_vv_cache.insert(from_peer, peer_vv);

    // Mark VV as dirty for persistence
    ws.sync_state.vv_dirty = true;
}
```

**Undo-stack bypass**: `IntentSource::RemotePeer` is checked in the undo-stack push path. Remote intents are not pushed to the local undo stack — they are in the remote peer's log and the remote can replay them.

**Error handling**: If `apply_single_intent` returns an error for a specific `SyncedIntent` (e.g., referencing a node that doesn't exist), the intent is skipped with a `verse.sync.intent_rejected` diagnostics event (severity: `Warn`). The VV is NOT advanced for rejected intents — the gap will be re-synced on next connection.

### 4.2 MarkPeerOffline

```rust
GraphIntent::MarkPeerOffline { peer_id, reason } => {
    state.sync_panel_state.peer_status.insert(
        peer_id,
        PeerStatus::Offline { reason, since: SystemTime::now() },
    );
    // UI reconcile path will update the sync status indicator
}
```

This is a pure state update. The reconcile path (`reconcile_webview_lifecycle`) detects the status change and triggers a UI refresh of the device-sync indicator (§6.1 of the Verso plan).

---

## 5. Version Vector Persistence

### 5.1 Where it lives

`VersionVector` is per-workspace state. It lives on `WorkspaceSyncState`, a field added to the workspace runtime struct:

```rust
struct WorkspaceSyncState {
    /// The local node's current version vector — tracks the highest sequence
    /// number seen from each peer (including self) for this workspace.
    local_vv: VersionVector,
    /// Last known VV per peer, used to compute deltas for outgoing sync.
    peer_vv_cache: HashMap<NodeId, VersionVector>,
    /// True when local_vv has been updated and not yet persisted.
    vv_dirty: bool,
}
```

### 5.2 Persistence path

`WorkspaceSyncState` is serialized alongside workspace persistence data (the existing `services/persistence` path). On load:

1. `GraphBrowserApp::init()` loads workspace state from disk
2. `WorkspaceSyncState` is deserialized; if absent (new workspace or pre-CP4 data), `local_vv` defaults to an empty `VersionVector` (triggers full snapshot sync on first peer connection)

On save:
- The persistence service checks `vv_dirty` flag each persistence tick
- If dirty, serializes the updated `WorkspaceSyncState` and clears the flag
- Persistence is not triggered by every `ApplyRemoteDelta` — the flag batches writes

### 5.3 Terminology note (historical Lamport wording vs. version vectors)

Earlier CP4 wording used "Lamport clock" shorthand. The Verso sync plan uses **version vectors** (per-peer monotonic counters). These are related but distinct:

- A **Lamport clock** is a single monotonically increasing counter per node, advanced on send and `max(local, received)+1` on receive. It provides partial ordering but cannot detect concurrency.
- A **version vector** (used here) tracks the highest sequence number seen *per peer*. This enables precise gap detection and concurrent intent identification.

**Resolution**: CP4 uses version vectors as specified in the Verso plan. SYSTEM_REGISTER.md has been updated to use version-vector terminology for CP4 persistence and causality wording.

---

## 6. Diagnostics

CP4 registers the following diagnostics channels (all under the `verse.sync` namespace):

| Channel | Severity | Description |
| --- | --- | --- |
| `verse.sync.intent_applied` | `Info` | Remote intent successfully applied; payload: `(peer_id, sequence, intent_kind)` |
| `verse.sync.intent_rejected` | `Warn` | Remote intent skipped due to apply error; payload: `(peer_id, sequence, error)` |
| `verse.sync.peer_offline` | `Warn` | Peer became unreachable; payload: `(peer_id, reason)` |
| `verse.sync.vv_gap_detected` | `Warn` | Received delta with a sequence gap (intents out of order or lost); payload: `(peer_id, expected, got)` |
| `verse.sync.full_snapshot_applied` | `Info` | Full workspace snapshot received and applied (device-sync catchup) |
| `verse.sync.worker_crash` | `Error` | `P2PSyncWorker` exited unexpectedly; `MarkPeerOffline { peer_id: None }` enqueued |

All channels must include a `severity` field per `CLAUDE.md` guidelines.

---

## 7. Done Gates (aligned with SYSTEM_REGISTER CP4)

- [x] `ControlPanel::spawn_p2p_sync_worker()` implemented; worker supervised with `CancellationToken` + `JoinSet`
- [ ] Remote-sync reducer carrier is fully wired (`ApplyRemoteLogEntries` runtime alias and/or `ApplyRemoteDelta` target naming) with dedup + VV semantics
- [ ] Explicit peer-offline signal path (`MarkPeerOffline` or equivalent reducer-owned status intent) is defined and handled in `apply_intents()`
- [ ] Version vector loaded from persistence on workspace init; persisted on `vv_dirty` flag
- [ ] Deduplication: already-seen sequences are skipped without applying or erroring
- [ ] Undo-stack bypass: `IntentSource::RemotePeer` intents are not pushed to local undo history
- [ ] `verse.sync.*` diagnostics channels registered with correct severities
- [ ] Worker crash surfaces non-silent offline state (`MarkPeerOffline` target behavior or equivalent reducer-owned offline status path)
- [ ] `cargo check --package graphshell` clean; targeted tests for: (a) deduplication, (b) VV advancement, (c) peer-offline state update

---

## 8. Implementation Sequence

1. **Converge sync intent naming** — either: (a) rename runtime carrier from `ApplyRemoteLogEntries` to `ApplyRemoteDelta`, or (b) keep alias with explicit mapping docs. Add exhaustive match arms in `apply_intents()` (stubs returning `Ok(())` first to unblock compile).
2. **Add `WorkspaceSyncState`** — add field to workspace runtime struct; wire to persistence load/save.
3. **Implement reducer logic** — deduplication check, `apply_single_intent` call, VV update, undo bypass.
4. **Add diagnostics channels** — register `verse.sync.*` channels with correct severity in the diagnostics registry.
5. **Implement `spawn_p2p_sync_worker()`** — stub implementation first (worker that immediately returns); Verso mod wires the real `P2PSyncWorker` in its activation sequence.
6. **Wire worker crash detection** — detect unexpected `JoinSet` exit; enqueue `MarkPeerOffline` with `SyncWorkerCrash` reason.
7. **Tests** — deduplication invariant, VV advancement correctness, peer-offline state transition.

Steps 1–4 are pure Graphshell-side changes with no Verso dependency. Steps 5–7 require Verso mod integration.

---

## 9. Lane Assignment

- **Primary lane**: `lane:subsystem-hardening` (`#96`) — security/device-sync integrity
- **Cross-lane**: `lane:runtime-followon` (`#91`) — ControlPanel extension follows SR2/SR3 signal routing work
- **Blocker**: Phase 5.3 pairing (complete) — identity and trust store must be initialized before the sync worker can validate incoming connections
- **Hotspots**: `graph_app.rs` (intent enum + reducer), `shell/desktop/control_panel.rs` (worker spawn), `services/persistence/` (VV serialization), `mods/native/verso/mod.rs` (worker activation)
