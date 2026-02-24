# Verse Tier 1: Direct P2P Sync — Implementation Plan

**Date**: 2026-02-23
**Status**: Implementation-Ready
**Phase**: Registry Phase 5
**Context**: Defines Verse as a native mod that provides zero-cost P2P sync between trusted devices. This is the concrete deliverable for Phase 5 — iroh transport, identity, pairing, delta sync, and workspace access control.

---

## 1. Overview

Verse Tier 1 enables **direct bilateral sync** between explicitly trusted peers. A user's phone and desktop, or a workspace shared with a friend. No tokens, no central server, no economic layer.

**Key characteristics**:
- Transport: iroh (QUIC + Noise + Magic Sockets)
- Topology: Bilateral, session-oriented
- Discovery: mDNS + manual pairing
- Trust model: Explicit pairing ceremony, stored in local trust store
- Conflict resolution: Intent-based LWW + ghost-node pattern
- Privacy: Private by default, end-to-end encrypted

---

## 2. Identity & Pairing

### 2.1 The P2P Identity

Each Graphshell instance has a single **Node Identity** — an Ed25519 keypair generated on first launch and stored in the OS keychain. This keypair's public key is the `NodeId` (a 32-byte hash used as the peer address in iroh).

```rust
/// Stored in OS keychain via `keyring` crate
/// Service: "graphshell", Key: "p2p-identity"
struct P2PIdentitySecret {
    secret_key: SecretKey,  // Ed25519 private key (32 bytes)
    created_at: SystemTime,
    device_name: String,    // Human-readable display name
}
```

The `NodeId` (the public key) is derived deterministically from the `SecretKey`. It is the canonical identifier for this device across all Verse interactions.

**Keychain access**: `keyring` crate handles OS Credential Store (Windows), Keychain (macOS), and Secret Service (Linux) uniformly. The secret key is never written to disk anywhere else.

### 2.2 IdentityRegistry Extension

The Verse mod extends `IdentityRegistry` with:

```rust
/// Added to IdentityRegistry by Verse mod on load
trait P2PIdentityExt {
    fn p2p_node_id(&self) -> NodeId;
    fn sign_sync_payload(&self, payload: &[u8]) -> Signature;
    fn verify_peer_signature(&self, peer: NodeId, payload: &[u8], sig: &Signature) -> bool;
    fn get_trusted_peers(&self) -> Vec<TrustedPeer>;
    fn trust_peer(&mut self, peer: TrustedPeer);
    fn revoke_peer(&mut self, node_id: NodeId);
}
```

### 2.3 The Trust Store

```rust
struct TrustedPeer {
    node_id: NodeId,
    display_name: String,
    role: PeerRole,
    added_at: SystemTime,
    last_seen: Option<SystemTime>,
    workspace_grants: Vec<WorkspaceGrant>,
}

enum PeerRole {
    /// Own device — full read/write on all personal workspaces.
    /// Established by: same root seed or explicit mutual pairing confirmation.
    Self_,
    /// Friend — explicitly added. Access is per-workspace, per grant.
    Friend,
}

struct WorkspaceGrant {
    workspace_id: WorkspaceId,
    access: AccessLevel,
}

enum AccessLevel { ReadOnly, ReadWrite }
```

Trusted peers are persisted in `user_registries.json` alongside other user registry state. The trust store is loaded by the Verse mod on startup.

### 2.4 Pairing Flows

Two peers establish trust through a **pairing ceremony** that exchanges `NodeAddr` (iroh's address record: `NodeId` + relay hint + direct addresses). After pairing, both peers store each other as `TrustedPeer`.

#### Flow A: Pairing Code (Cross-Network)

1. Device A: "Show Pairing Code" → generates a one-time iroh ticket encoded as:
   - A 6-word human phrase (e.g., `river-anchor-moon-cedar-pulse-nine`)
   - A QR code (binary encoding of the same data)
   - Ticket expires in 5 minutes
2. Device B: "Add Device" → "Enter Code" or "Scan QR"
3. iroh establishes QUIC connection (with relay fallback if NAT blocks direct)
4. Both show fingerprint confirmation: "Connecting to NodeId `a3f7...`. Allow?"
5. User names the device: "Marks-iPhone" → confirmed
6. Workspace access grant dialog (see §3.3)
7. Pairing stored in trust store on both sides

#### Flow B: Local Discovery (Same Network)

1. Desktop advertises via mDNS: `_graphshell-sync._udp.local`, TXT record includes `NodeId`
2. Sync Panel shows "Nearby Devices" section with discovered peers
3. User clicks "Pair" next to device name
4. iroh connects directly (same LAN, no relay needed)
5. Same fingerprint confirmation + workspace grant flow as Flow A

#### Flow C: Invite Link (Friend Sharing)

1. User clicks "Create Invite Link" for a specific workspace
2. App generates a one-time link encoding: `verse://pair?ticket=<iroh_ticket>&workspace=<id>&access=ro`
3. Friend clicks link in Graphshell → pairing + workspace grant in one step
4. Link is single-use and expires after 24 hours

---

## 3. Transport: iroh

### 3.1 Why iroh

iroh is a Rust-native QUIC transport library built by n0 (formerly Protocol Labs) specifically for "syncing bytes between devices." It provides:

- **QUIC transport** — multiplexed, low-latency, no head-of-line blocking
- **Magic Sockets** — transparent NAT traversal via hole punching + relay fallback
- **Noise protocol** — mutual authentication and encryption at the transport layer (no separate TLS handshake needed)
- **ALPN-based protocol dispatch** — multiple protocols over one endpoint

The iroh relay network (DERP-style relay servers) handles NAT traversal without requiring a central coordination server. Connections upgrade to direct QUIC once hole-punching succeeds.

### 3.2 Endpoint Setup

```rust
use iroh::{Endpoint, SecretKey, NodeAddr};

const SYNC_ALPN: &[u8] = b"graphshell-sync/1";

// In VerseMod::init():
async fn start_endpoint(secret_key: SecretKey) -> Result<Endpoint> {
    Endpoint::builder()
        .secret_key(secret_key)
        .alpns(vec![SYNC_ALPN.to_vec()])
        .bind()
        .await
}
```

### 3.3 NAT Traversal Strategy

iroh handles this transparently:

1. Try direct UDP QUIC to peer's socket address (works on LAN / non-NAT)
2. If blocked: route through iroh relay (relay has public IP, both peers connect outbound)
3. Continue hole-punching in background
4. Upgrade to direct connection if hole-punch succeeds (relay is just a fallback)

From the application's perspective: call `endpoint.connect(peer_addr)` and get back a `Connection`. The transport details are irrelevant.

### 3.4 Connection Model

Each sync session uses **one QUIC stream per workspace**. The stream is bidirectional — both peers can send on the same stream.

```
One QUIC connection per peer pair
  └─ Stream 0: workspace "Research" sync
  └─ Stream 1: workspace "Reading List" sync
  └─ Stream 2: workspace "Private" (only if granted)
  ...
```

---

## 4. The Sync Protocol

### 4.1 The SyncUnit (Wire Format)

The `SyncUnit` is the atomic unit of transfer. It carries a delta of `GraphIntent`s from one peer to another.

```rust
#[derive(Archive, Serialize, Deserialize)]
#[archive(check_bytes)]
struct SyncUnit {
    workspace_id: WorkspaceId,
    from_peer: NodeId,
    /// The sender's current version vector (so receiver can detect gaps)
    sender_vv: VersionVector,
    /// The VV at the start of this delta (sender's knowledge of receiver's state)
    base_vv: VersionVector,
    /// The intents the receiver has not yet seen
    intents: Vec<SyncedIntent>,
    /// Full snapshot included when receiver is too far behind for delta sync
    snapshot: Option<WorkspaceSnapshot>,
}

#[derive(Archive, Serialize, Deserialize)]
struct SyncedIntent {
    intent: GraphIntent,
    authored_by: NodeId,     // Which peer originated this intent
    authored_at: SystemTime, // Wall clock at origin (for LWW resolution)
    sequence: u64,           // Per-peer monotonic counter
}

#[derive(Archive, Serialize, Deserialize, Clone)]
struct VersionVector {
    /// Maps NodeId → highest sequence number seen from that peer
    clocks: HashMap<NodeId, u64>,
}
```

Serialization pipeline: `rkyv` → `zstd` → raw bytes on wire (no additional encryption needed — iroh Noise handles transport security).

### 4.2 The Exchange Protocol

Both peers simultaneously act as initiator and responder. After a QUIC connection is established, each peer sends a `SyncHello` and then computes what to send:

```
Peer A                                    Peer B
──────────────────────────────────────────────────────
→ SyncHello { my_vv, my_node_id }
                                ← SyncHello { my_vv, my_node_id }

→ SyncUnits { intents B hasn't seen }
                                ← SyncUnits { intents A hasn't seen }

→ SyncAck
                                ← SyncAck
```

Delta computation: Given `A_vv` and `B_vv`, Peer A sends intents where `intent.sequence > B_vv[A]`. Peer B sends intents where `intent.sequence > A_vv[B]`. Both can compute this independently from their local logs after exchanging version vectors.

**Full snapshot trigger**: If `B_vv[A] == 0` (B has never seen A's state for this workspace) or the delta would be >10,000 intents, send a `WorkspaceSnapshot` instead and B rebuilds from scratch.

### 4.3 Version Vectors

```rust
impl VersionVector {
    /// Merge two version vectors (take max per peer)
    fn merge(&self, other: &VersionVector) -> VersionVector {
        let mut merged = self.clocks.clone();
        for (peer, &seq) in &other.clocks {
            merged.entry(*peer).and_modify(|s| *s = (*s).max(seq)).or_insert(seq);
        }
        VersionVector { clocks: merged }
    }

    /// True if self has strictly seen more from every peer than other
    fn dominates(&self, other: &VersionVector) -> bool {
        other.clocks.iter().all(|(peer, &seq)| {
            self.clocks.get(peer).copied().unwrap_or(0) >= seq
        })
    }

    fn increment(&mut self, peer: NodeId) -> u64 {
        let seq = self.clocks.entry(peer).or_insert(0);
        *seq += 1;
        *seq
    }
}
```

### 4.4 Conflict Resolution by Intent Type

The edge traversal model (append-only traversals) makes most graph mutations naturally conflict-free. The remaining conflicts have defined resolution strategies:

| Intent | Concurrency Behavior | Strategy |
| --- | --- | --- |
| `AddNode` | No conflict — UUID-keyed | Always safe (different UUIDs) |
| `DeleteNode` | Concurrent with edge-from → ghost | Ghost-node pattern + conflict UI |
| `UpdateNodeTitle` | Concurrent edits | LWW: `authored_at` timestamp wins |
| `UpdateNodeTags` | Concurrent tag changes | CRDT: union of add-sets; intersection of remove-sets |
| `AddEdge` | No conflict — idempotent by (src, dst, type) | Always safe |
| `RemoveEdge` | Concurrent with add | Remove wins (conservative; user can re-add) |
| `AddTraversal` | No conflict — append-only log | Always safe ✓ |
| `SetNodePosition` | Concurrent moves | Local edit wins while user is interacting; physics reconciles |
| `SetWorkspaceName` | Concurrent rename | LWW |

#### The Ghost-Node Pattern

When `DeleteNode(X)` arrives but the receiver has live edges pointing to X:

1. Node X transitions to `NodeState::Ghost` — invisible in the normal graph view, but edges are preserved in the data model
2. A non-blocking conflict notice appears: "Marks-iPhone deleted 'React Docs', but you have 3 edges to it."
3. User options: Keep Node & Edges | Delete Node & Edges | Keep as Ghost | Decide Later
4. "Decide Later" leaves X as ghost; it can be cleaned up via Settings → Sync → Conflicts

This avoids data loss without blocking the sync operation.

---

## 5. The SyncWorker (Control Plane Integration)

### 5.1 SyncWorker as Supervised Task

The SyncWorker is a long-lived `tokio` task owned by `ControlPanel`. It holds the iroh `Endpoint` and runs the accept loop. The ControlPanel supervises it with a `CancellationToken` and restarts it if it panics.

```rust
struct SyncWorker {
    endpoint: Endpoint,
    identity: Arc<IdentityState>,      // Read-only handle to IdentityRegistry state
    intent_tx: mpsc::Sender<GraphIntent>, // Inject remote intents into app pipeline
    sync_log: Arc<RwLock<SyncLog>>,   // Persisted per-workspace intent log + VV
    rx: mpsc::Receiver<SyncCommand>,
}

enum SyncCommand {
    SyncWorkspace { peer: NodeId, workspace_id: WorkspaceId },
    AcceptIncoming(iroh::incoming::Incoming),
    PairDevice(PairingTicket),
    UpdateGrant { peer: NodeId, grant: WorkspaceGrant },
    RevokeAccess { peer: NodeId },
}
```

### 5.2 The Accept Loop

```rust
async fn run(mut self, cancel: CancellationToken) {
    loop {
        tokio::select! {
            biased;
            _ = cancel.cancelled() => break,
            incoming = self.endpoint.accept() => {
                if let Ok(conn) = incoming {
                    let worker = self.clone_handle();
                    tokio::spawn(worker.handle_incoming(conn));
                }
            },
            cmd = self.rx.recv() => {
                if let Some(cmd) = cmd {
                    self.handle_command(cmd).await;
                }
            },
        }
    }
    self.endpoint.close().await;
}
```

### 5.3 Integration with GraphIntent Pipeline

Remote intents enter the normal `GraphIntent` pipeline but are tagged with their origin:

```rust
// GraphIntent gains a source field (or wrapper):
enum IntentSource {
    LocalUser,
    RemotePeer { peer_id: NodeId, sequence: u64 },
}

// Remote intents:
// - Bypass the local undo stack (they are in the remote peer's log)
// - Update the local VV on successful application
// - Trigger a UI refresh via the existing reconcile path
// - Emit diagnostics: verse.sync.intent_applied
```

The intent reducer (`apply_intents`) treats remote intents identically to local ones except for undo history. This means the existing two-phase apply model (pure state → reconcile effects) works without modification.

### 5.4 The Sync Log

The SyncLog is a persisted per-workspace record of applied intents and the current VV:

```rust
struct SyncLog {
    workspace_id: WorkspaceId,
    entries: Vec<SyncLogEntry>,       // Append-only
    local_vv: VersionVector,          // Current VV including all applied intents
    peer_vv_cache: HashMap<NodeId, VersionVector>, // Last known VV per peer
}

struct SyncLogEntry {
    intent: SyncedIntent,
    applied_at: SystemTime,
}
```

The SyncLog is serialized via `rkyv` and stored alongside workspace persistence data.

### 5.5 Backpressure

The SyncWorker respects existing backpressure primitives:

- Max 5 concurrent incoming sync connections (configurable, checked before spawning handler)
- Large snapshots are chunked into batches of 500 intents and sent progressively
- If the intent pipeline's `intent_tx` channel is full (app under load), incoming intents are buffered in the SyncWorker — not dropped

---

## 6. UX Design

### 6.1 Sync Status Indicator

Persistent element in the toolbar, always visible when Verse mod is loaded:

```
[●] 2 peers · synced          ← connected peers, recently synced
[●] 2 peers · syncing...      ← sync in progress
[○] Offline                   ← no peers reachable
[!] Conflict                  ← unresolved conflict pending
```

Clicking the indicator opens the Sync Panel (§6.2). Absent when Verse mod is not loaded.

### 6.2 The Sync Panel (`graphshell://settings/sync`)

```
┌─────────────────────────────────────────────────────┐
│ Sync                                  [Sync Now ↺]  │
├─────────────────────────────────────────────────────┤
│ This Device                                         │
│   "Marks-Desktop"                                   │
│   ID: a3f7...b91c                                  │
│   [Show Pairing Code]   [Show QR Code]              │
├─────────────────────────────────────────────────────┤
│ Connected Devices                                   │
│  ●  Marks-iPhone  · now                            │
│     Research (RW) · Reading List (RO)               │
│     [Manage Access]  [Sync Now]  [Disconnect]       │
│                                                     │
│  ○  Work-Laptop  · 2 hours ago                     │
│     Research (RW)                                   │
│     [Manage Access]  [Remove Device]                │
├─────────────────────────────────────────────────────┤
│  ⚠ 1 Conflict Pending  [Review]                    │
├─────────────────────────────────────────────────────┤
│  [+ Add Device]                                     │
└─────────────────────────────────────────────────────┘
```

### 6.3 Pairing Flow UX

**"Show Pairing Code" path** (for remote / cross-network pairing):

```
Device A shows:                      Device B enters:
┌───────────────────┐                ┌────────────────────────┐
│ Pair New Device   │                │ Add Device             │
│                   │                │                        │
│ [QR CODE HERE]    │                │ Scan QR  or  Enter     │
│                   │                │ Code:                  │
│ river-anchor-moon │◄──────────────►│ [river-anchor-moon   ] │
│ -cedar-pulse-nine │                │ [-cedar-pulse-nine   ] │
│                   │                │                        │
│ Expires in 4:47   │                │ [Connect]              │
│ [Cancel]          │                └────────────────────────┘
└───────────────────┘
```

After connection:
```
┌───────────────────────────────────────┐
│ ✓ Connected to a3f7...b91c           │
│                                       │
│ Device name: [Marks-iPhone        ]   │
│                                       │
│ Share workspaces:                     │
│   ☑ Research          [Read/Write ▾] │
│   ☑ Reading List      [Read Only  ▾] │
│   ☐ Private                          │
│                                       │
│ [Cancel]              [Done Pairing]  │
└───────────────────────────────────────┘
```

**Local discovery path:**

- Sync panel shows "Nearby: Marks-iPhone [Pair]" under a "Nearby Devices" section
- Tap "Pair" → same fingerprint confirm + workspace grant flow
- No codes needed — mDNS handles discovery

### 6.4 Workspace Sharing Context Menu

Right-click workspace header (or three-dot menu):

```
Research  ▾
├── Sync Settings
│   ├── Share with Marks-iPhone (Read/Write)
│   ├── Share with Marks-iPhone (Read Only)
│   ├── Revoke Marks-iPhone Access
│   ────────────────────────────────
│   └── Create Invite Link...
```

"Create Invite Link" generates a one-time `verse://` link valid for 24 hours.

### 6.5 Conflict Resolution UI

Non-blocking notification bar (appears below toolbar):

```
┌──────────────────────────────────────────────────────────────┐
│ ⚠ Sync Conflict: Marks-iPhone deleted "React Docs"          │
│   but you have 3 edges pointing to it.                       │
│   [Keep Node]  [Delete Node & Edges]  [Decide Later]         │
└──────────────────────────────────────────────────────────────┘
```

For concurrent title edits (lower severity):

```
┌──────────────────────────────────────────────────────────────┐
│ ↕ Title resolved: "React API Reference" (from Marks-iPhone) │
│   replaced your edit "React Docs" (older by 2 minutes)      │
│   [Undo]                                                     │
└──────────────────────────────────────────────────────────────┘
```

---

## 7. Security & Encryption

### 7.1 Transport Security

iroh's Noise protocol provides:

- **Mutual authentication**: both peers prove ownership of their `NodeId` (Ed25519 keypair)
- **Forward secrecy**: per-session ephemeral keys
- **Encryption**: ChaCha20-Poly1305 on all QUIC packets

There is no additional application-level encryption on the wire — Noise handles it. Peers are authenticated by `NodeId` before any sync data is exchanged.

### 7.2 At-Rest Sync Cache

The `SyncLog` is encrypted at rest using the same pipeline as workspace persistence:
```
SyncLog → rkyv serialize → zstd compress → AES-256-GCM → disk
```
Encryption key: derived from the OS keychain `P2PIdentitySecret` via HKDF.

### 7.3 Trust Boundary

Only connections from `TrustedPeer`s (NodeIds in the trust store) are accepted:
```rust
fn is_trusted(&self, node_id: &NodeId) -> bool {
    self.identity.get_trusted_peers()
        .iter()
        .any(|p| &p.node_id == node_id)
}

// In accept loop:
if !self.is_trusted(&conn.remote_node_id()) {
    conn.close(0u32, b"not trusted").await;
    return;
}
```

### 7.4 The "No-Receipt" Trust Tier

Tier 1 sync does NOT use the Proof-of-Access economic model. The `skip_receipt = true` flag is implicit for all Tier 1 transfers. Bandwidth is tracked locally for user info only. No tokens, no ledger, no economic layer.

---

## 8. Registry Integration: The Verse Mod

### 8.1 ModManifest

```rust
ModManifest {
    mod_id: "verse",
    display_name: "Verse — Direct Sync",
    version: "0.1.0",
    mod_type: ModType::Native,
    provides: &[
        "identity:p2p",
        "protocol:verse",
        "action:verse.pair_device",
        "action:verse.sync_now",
        "action:verse.share_workspace",
        "action:verse.forget_device",
    ],
    requires: &[
        "IdentityRegistry",
        "ActionRegistry",
        "ProtocolRegistry",
        "ControlPanel",      // For SyncWorker supervision
        "DiagnosticsRegistry",
    ],
    capabilities: &["network", "identity"],
}
```

Registered via `inventory::submit!` at compile time — no dynamic loading, no sandboxing.

### 8.2 Initialization Sequence

```
AppServices::start()
  └─ ModRegistry::load_all_native()
       └─ VerseMod::init(registry_ctx)
            ├─ Load or generate Ed25519 keypair from OS keychain
            ├─ Register identity:p2p in IdentityRegistry
            ├─ Register verse.* actions in ActionRegistry
            ├─ Spawn SyncWorker task via ControlPanel
            └─ Start mDNS advertisement
```

Failure policy: if keychain is unavailable, log diagnostics and disable the Verse mod gracefully. The app continues without sync.

### 8.3 ActionRegistry Extensions

| Action ID | Trigger | Intent Emitted |
| --- | --- | --- |
| `verse.pair_device` | Command Palette, Sync Panel | Opens pairing dialog |
| `verse.sync_now` | Toolbar indicator, Sync Panel | Triggers SyncCommand::SyncAll |
| `verse.share_workspace` | Context menu, Sync Panel | Opens workspace grant dialog |
| `verse.forget_device` | Sync Panel → device menu | Removes TrustedPeer, emits diagnostics |

### 8.4 Diagnostics Channels

All channels follow `verse.sync.*` naming (scoped under the Verse mod, not under atomic registries):

| Channel | Emitted When |
| --- | --- |
| `verse.sync.peer_connected` | Incoming connection authenticated |
| `verse.sync.peer_disconnected` | Connection closed |
| `verse.sync.unit_sent` | SyncUnit transmitted to peer |
| `verse.sync.unit_received` | SyncUnit received from peer |
| `verse.sync.intent_applied` | Remote intent applied to workspace |
| `verse.sync.conflict_detected` | Non-commutative conflict requires resolution |
| `verse.sync.conflict_resolved` | User resolved a conflict |
| `verse.sync.pairing_started` | Pairing ceremony initiated |
| `verse.sync.pairing_succeeded` | Peer trusted and stored |
| `verse.sync.pairing_failed` | Pairing failed (timeout, mismatch, rejected) |
| `verse.sync.access_denied` | Incoming sync for a non-granted workspace |

### 8.5 Offline Graceful Degradation

**Verse mod not loaded:**

- No `verse.*` actions → Command Palette shows nothing sync-related
- No sync indicator in toolbar
- App is 100% functional offline graph organizer
- `ipfs://` and `activitypub://` not registered → those URLs resolve to "Protocol not available"

**Verse mod loaded, no peers reachable:**

- Sync indicator shows "○ Offline"
- `verse.sync_now` emits `verse.sync.unit_sent` with error and user-visible message: "No peers reachable"
- Local intents continue to journal — they will sync when peers reconnect
- mDNS continues advertising in background

---

## 9. Phase 5 Execution Plan

This is the concrete implementation plan for Registry Phase 5. Each step is a thin vertical slice with a testable done gate. See `2026-02-22_registry_layer_plan.md` §Phase 5 for alignment with the broader registry plan.

### Step 5.1: iroh Scaffold & Identity Bootstrap

**Goal**: Verse mod loads, iroh endpoint starts, NodeId is generated and stored.

- Add `iroh`, `keyring`, `qrcode-generator` (or `qrcode`) dependencies to `Cargo.toml`
- Define `VerseMod` struct with `ModManifest` (compile-time `inventory::submit!`)
- On first load: generate `SecretKey`, derive `NodeId`, store secret in OS keychain
- On subsequent loads: load secret from keychain
- Create iroh `Endpoint` with `SYNC_ALPN`
- Register `identity:p2p` persona in `IdentityRegistry` (NodeId available to other code)
- Add diagnostics: `verse.sync.peer_connected`, `verse.sync.pairing_started`
- **Done gate**: `cargo run` starts iroh endpoint; `DiagnosticsRegistry` shows `registry.mod.load_succeeded` for "verse". `IdentityRegistry::p2p_node_id()` returns the device NodeId.

### Step 5.2: TrustedPeer Store & IdentityRegistry Extension

**Goal**: Identity system knows about P2P keypairs and can persist/retrieve trusted peers.

- Extend `IdentityRegistry` with `P2PIdentityExt` trait (see §2.2)
- Implement `TrustedPeer` model with `PeerRole` and `WorkspaceGrant`
- Persist trust store in `user_registries.json` under `verse.trusted_peers`
- Add `sign_sync_payload` and `verify_peer_signature` implementations
- Add `SyncLog` struct with rkyv serialization + AES-256-GCM at-rest encryption
- Diagnostics: `registry.identity.p2p_key_loaded`, `verse.sync.pairing_succeeded`, `verse.sync.pairing_failed`
- **Done gate**: Contract tests cover: P2P persona creation, sign/verify round-trip, trust store persist/load round-trip, grant model serialization.

### Step 5.3: Pairing Ceremony & Settings UI

**Goal**: Two running instances can discover each other and complete a pairing ceremony.

- Implement `verse.pair_device` action: show 6-word code (encoded NodeAddr) + QR data
- Implement `verse.pair_device` receiver: accept 6-word code → resolve to `NodeAddr` → connect → confirm fingerprint
- Implement mDNS advertisement (`_graphshell-sync._udp.local`) and discovery
- Add Sync settings page (`graphshell://settings/sync`) with device list and "Add Device" flow
- Implement trust-on-confirm: after fingerprint confirmation, add to `TrustedPeer` store
- Implement workspace grant dialog (post-pairing)
- **Done gate**: Two instances launched on the same machine can pair via a printed 6-word code. After pairing, both show each other in Sync Panel's device list.

### Step 5.4: Delta Sync (The Core)

**Goal**: GraphIntents sync bidirectionally between paired peers.

- Implement `SyncWorker` as ControlPanel-supervised tokio task (accept loop + command channel)
- Implement `VersionVector` with `merge`, `dominates`, `increment`
- Implement `SyncUnit` with rkyv serialization + zstd compression
- Implement outbound sync: collect unsynced intents → serialize → send via iroh QUIC stream
- Implement inbound sync: receive `SyncUnit` → deserialize → apply via `GraphIntent::ApplyRemoteDelta`
- Implement delta computation (version vector diff)
- Implement full snapshot trigger (when delta > 10,000 intents or VV is 0)
- Implement LWW for `UpdateNodeTitle` / `SetWorkspaceName`
- Implement CRDT merge for `UpdateNodeTags` (union/intersection)
- Implement ghost-node pattern for `DeleteNode` conflicts
- Add sync status indicator to toolbar (●/○/!)
- Add non-blocking conflict notification bar
- Diagnostics: `verse.sync.unit_sent`, `verse.sync.unit_received`, `verse.sync.intent_applied`, `verse.sync.conflict_detected`
- **Done gate**: Create a node on instance A → it appears on instance B within 5 seconds. Rename node simultaneously on both → LWW resolves without crash. Harness scenario `verse_delta_sync_basic` passes.

### Step 5.5: Workspace Access Control

**Goal**: Per-workspace, per-peer access grants are enforced.

- Enforce `WorkspaceGrant` on inbound sync: reject `SyncUnit` for non-granted workspaces with `verse.sync.access_denied` diagnostic
- Enforce read-only grants: reject inbound intents that mutate state when peer has `ReadOnly`
- Implement workspace sharing context menu (right-click workspace → "Share with...")
- Implement "Manage Access" screen in Sync Panel (grant/revoke per device per workspace)
- Implement `verse.forget_device` action (revoke all grants + remove from trust store)
- **Done gate**: Peer A grants Peer B `ReadOnly` on workspace W. Peer B can receive nodes from W but its local mutations on W do not propagate to A. Harness scenario `verse_access_control` passes.

---

## 10. Crate Dependencies (Tier 1)

| Crate | Purpose |
| --- | --- |
| `iroh` | QUIC transport, NAT traversal, Noise auth |
| `keyring` | OS keychain (Windows Credential, macOS Keychain, Linux SecretService) |
| `mdns-sd` | Local device discovery and advertisement |
| `qrcode` | QR code generation for pairing UI |
| `rkyv` | Zero-copy binary serialization for SyncUnit |
| `zstd` | Compression for sync payloads |
| `aes-gcm` | At-rest encryption of SyncLog |
| `hkdf` + `sha2` | Key derivation for SyncLog encryption key |

---

## 11. Open Questions (Tier 1)

1. **Identity Scope**: Should each workspace have its own keypair, or is the device-level NodeId sufficient for Phase 5? (Device-level is simpler; workspace-level keys enable finer-grained revocation but add key management complexity. Recommendation: device-level for Phase 5.)

2. **Relay Infrastructure**: iroh's public relay network is operated by n0. Accept this dependency for Phase 5; evaluate hosting a dedicated relay for production resilience.

3. **Sync Trigger**: Auto-sync when peers are reachable (continuous), or user-initiated only? (Recommendation: continuous, with a 30-second quiescence window after the last local mutation to avoid syncing on every keystroke.)

4. **Conflict Accumulation**: Cap shown at 10 pending conflicts; overflow goes to "Review Conflicts" panel in Sync settings to avoid UI overwhelm.

5. **Version Vector Pruning**: VVs grow unbounded as peers interact. Prune entries for peers not seen in >30 days; emit `verse.sync.vv_pruned` diagnostic.

6. **Workspace Granularity**: Workspace is the atomic sync unit for Phase 5. Sub-workspace (node-set) sync is deferred.

---

## 12. Next Steps

After Tier 1 validation (Q2 2026), the architecture extends naturally to Tier 2:
- libp2p for public community swarms (complementary to iroh, not a replacement)
- Identity bridge: same Ed25519 keypair derives both iroh NodeId and libp2p PeerId
- VerseBlob content format (ported from SyncUnit design) for universal content addressing
- Index artifacts (tantivy segments) as publishable blobs
- Proof of Access economic layer (optional; Tier 1 continues to work offline)

See `2026-02-23_verse_tier2_architecture.md` for the long-horizon design.
