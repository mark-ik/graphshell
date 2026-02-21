# Control Plane: Async Scaling for Multi-Producer Intent Queuing

**Date:** 2026-02-21
**Status:** Prospective — Stage 5 design (not in Stages 1–4 implementation)
**Related Plans:** [2026-02-20_embedder_decomposition_plan.md](2026-02-20_embedder_decomposition_plan.md) (Stage 5), [2026-02-21_lifecycle_intent_model.md](2026-02-21_lifecycle_intent_model.md)

---

## Overview

Stage 5 extends graphshell's synchronous reducer model to support concurrent async producers (network, timers, background tasks) without sacrificing determinism or requiring a complete async rewrite. The two-phase model (`apply_intents()` + `reconcile_webview_lifecycle()`) remains the single authoritative decision boundary; background workers queue intents that are drained and ordered **before** applying.

**Key insight:** The reducer stays 100% synchronous and testable. Network I/O and background work happen in supervised tokio tasks that only communicate via intent queues. This is fundamentally different from making the entire app async—it's an async *adapter layer* around a deterministic sync core.

---

## Architecture: Sync Reducer + Async Producers

### Frame Loop (Synchronous)

```rust
pub fn run_frame(
    app: &mut GraphBrowserApp,
    intent_rx: &mpsc::Receiver<QueuedIntent>,
    // ... other args
) {
    // 1. Collect intents from all sources
    let mut all_intents = Vec::new();
    
    // Local UI intents (synchronous)
    all_intents.extend(collect_keyboard_intents());
    all_intents.extend(collect_mouse_intents());
    
    // Servo delegate events → intents (synchronous)
    all_intents.extend(graph_intents_from_pending_semantic_events());
    
    // Async producer intents (non-blocking drain)
    while let Ok(queued) = intent_rx.try_recv() {
        all_intents.push(queued.intent);
    }
    
    // 2. Sort by causality for determinism
    all_intents.sort_by_key(|intent| intent.causality_order());
    
    // 3. Apply atomically (pure state, no side effects)
    apply_intents(app, all_intents);
    
    // 4. Reconcile (side effects: webview creation/destruction)
    reconcile_webview_lifecycle(app);
    
    // 5. Render
    render(app);
}
```

### Background Workers (Asynchronous)

```rust
// P2P sync worker (example)
async fn p2p_sync_worker(
    peer_addr: PeerAddr,
    tx: mpsc::Sender<QueuedIntent>,
) {
    loop {
        match sync_peer_delta(peer_addr).await {
            Ok(peer_delta) => {
                // Convert peer mutation → GraphIntent
                let intent = QueuedIntent {
                    intent: GraphIntent::ApplyRemoteDelta {
                        from_peer: peer_addr,
                        delta: peer_delta.changes,
                        lamport_clock: peer_delta.lamport,
                    },
                    queued_at: Instant::now(),
                    source: IntentSource::P2pSync,
                };
                
                // Queue into main loop
                // (respects backpressure: blocks if queue full)
                tx.send(intent).await.ok();
            }
            Err(e) => {
                // Network failure → explicit intent
                let intent = QueuedIntent {
                    intent: GraphIntent::MarkPeerOffline {
                        peer: peer_addr,
                        retry_at: Instant::now() + Duration::from_secs(5),
                    },
                    queued_at: Instant::now(),
                    source: IntentSource::P2pSync,
                };
                tx.send(intent).await.ok();
            }
        }
    }
}

// Background prefetch scheduler (example)
async fn prefetch_scheduler(
    app_state: Arc<AppState>, // Shared read-only snapshot
    tx: mpsc::Sender<QueuedIntent>,
) {
    loop {
        tokio::time::sleep(Duration::from_secs(10)).await;
        
        // Decide what to prefetch based on memory + user context
        let candidates = decide_prefetch_candidates(&app_state);
        
        for node_key in candidates {
            let intent = QueuedIntent {
                intent: GraphIntent::PromoteNodeToActive {
                    key: node_key,
                    cause: LifecycleCause::BackgroundPrefetch,
                },
                queued_at: Instant::now(),
                source: IntentSource::PrefetchScheduler,
            };
            tx.send(intent).await.ok();
        }
    }
}
```

### Channel Design

```rust
// In app initialization (main.rs or app setup)
pub fn init_intent_channels() -> (
    mpsc::Sender<QueuedIntent>,
    mpsc::Receiver<QueuedIntent>,
) {
    // Capacity: prevents OOM flooding from malicious/broken workers
    // Typical: 256–512 pending intents before backpressure kicks in
    mpsc::channel::<QueuedIntent>(256)
}

// Each background worker gets a clone of the sender
let (intent_tx, intent_rx) = init_intent_channels();

tokio::spawn(p2p_sync_worker(peer_addr, intent_tx.clone()));
tokio::spawn(prefetch_scheduler(app_state_snapshot, intent_tx.clone()));
tokio::spawn(memory_monitor(intent_tx.clone()));

// Main loop drains the receiver
while is_running {
    run_frame(&mut app, &intent_rx);
}
```

### Intent Queue Metadata

```rust
/// Intent with source tracking and causality ordering.
#[derive(Debug, Clone)]
pub struct QueuedIntent {
    pub intent: GraphIntent,
    pub queued_at: Instant,
    pub source: IntentSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IntentSource {
    /// User keyboard/mouse input
    LocalUI,
    /// Servo browser delegate (navigation, load completion, etc.)
    ServoDelegate,
    /// P2P sync worker
    P2pSync,
    /// Background prefetch scheduler
    PrefetchScheduler,
    /// Memory/system monitor
    MemoryMonitor,
    /// Restore/replay from persistence
    Restore,
}

impl QueuedIntent {
    /// Causality ordering key for deterministic intent application.
    pub fn causality_order(&self) -> (u64, IntentSource) {
        match &self.intent {
            GraphIntent::ApplyRemoteDelta { lamport_clock, .. } => {
                (*lamport_clock, self.source)
            }
            // Local intents have implicit clock 0 (applied first)
            _ => (0, self.source),
        }
    }
}
```

---

## Causality Ordering: Deterministic Multi-Peer Convergence

### Problem: Concurrent Mutations from Multiple Peers

When two peers simultaneously modify the same node, they may apply mutations in different orders locally:

```
Peer A: SetNodeUrl(node_x, "a.com")  (arrives frame 1)
Peer B: SetNodeUrl(node_x, "b.com")  (arrives frame 1)

Without causality ordering:
  Frame 1 on A: sees B's mutation first → applies B, then A → final URL: "a.com" ✓
  Frame 1 on B: sees A's mutation first → applies A, then B → final URL: "b.com" ✗ (divergent!)
```

### Solution: Lamport Clock Causality

Each peer stamps mutations with a **Lamport clock**—a monotonic counter that increases with every local mutation:

```rust
pub struct GraphMutation {
    pub changes: Vec<IntentDelta>,
    pub lamport_clock: u64,  // This peer's clock at time of mutation
    pub from_peer: PeerAddr,
}

// Peer A makes mutation 1: lamport_clock = 42
// Peer B receives it, increments to max(own_clock, 42) + 1
// Peer B makes mutation 2: lamport_clock = 43
// When both mutations are queued, sort by:
//   1. lamport_clock (lower = earlier in causal history)
//   2. peer_id (tiebreaker: consistent across all peers)
```

### Intent Sorting Before Apply

```rust
fn run_frame(...) {
    let mut all_intents = Vec::new();
    
    // Collect from all sources...
    
    // CRITICAL: Sort before applying
    all_intents.sort_by(|a, b| {
        let a_order = a.causality_order();
        let b_order = b.causality_order();
        a_order.cmp(&b_order)
    });
    
    // Now apply in deterministic order
    apply_intents(&mut app, all_intents);
}
```

### Convergence Guarantee

With Lamport clock ordering:

```
Both peers apply in order:
  1. ApplyRemoteDelta { from: B, clock: 41, change: SetNodeUrl_B }
  2. ApplyRemoteDelta { from: A, clock: 42, change: SetNodeUrl_A }
  3. ApplyRemoteDelta { from: B, clock: 43, change: SetNodeUrl_B2 }

Result: Both peers end up with identical state ✓
```

---

## Backpressure & Flooding Prevention

### Channel Capacity as a Limit

```rust
// If worker sends faster than main loop drains:
let (tx, rx) = mpsc::channel::<QueuedIntent>(256);

async fn fast_worker(tx: mpsc::Sender<_>) {
    for i in 0..1000 {
        // First 256 sends succeed immediately
        // On send #257: call blocks until main loop drains
        match tx.send(intent).await {
            Ok(()) => { /* queued */ }
            Err(SendError(_)) => {
                // Channel closed (main loop exited)
                break;
            }
        }
    }
}
```

### Explicit Backoff for Slow Consumers

```rust
async fn network_worker_with_backoff(
    peer: PeerAddr,
    tx: mpsc::Sender<QueuedIntent>,
) {
    let mut backoff = ExponentialBuilder::default()
        .with_min_delay(Duration::from_millis(100))
        .with_max_delay(Duration::from_secs(10))
        .build();
    
    loop {
        match tx.send(intent).await {
            Ok(()) => {
                // Reset backoff on success
                backoff = ExponentialBuilder::default().build();
            }
            Err(SendError(_)) => {
                // Channel full or closed; apply exponential backoff
                let delay = backoff.next().unwrap_or(Duration::from_secs(10));
                tokio::time::sleep(delay).await;
            }
        }
    }
}
```

---

## Supervision & Cancellation

### Worker Lifecycle Management

```rust
pub struct ControlPlane {
    // Channels for producers
    intent_tx: mpsc::Sender<QueuedIntent>,
    intent_rx: mpsc::Receiver<QueuedIntent>,
    
    // Cancellation token for all workers
    cancel: CancellationToken,
    
    // Supervision of background workers
    workers: JoinSet<()>,
}

impl ControlPlane {
    pub async fn spawn_workers(&mut self) {
        // P2P sync
        let cancel = self.cancel.clone();
        let tx = self.intent_tx.clone();
        self.workers.spawn(async move {
            tokio::select! {
                _ = cancel.cancelled() => {
                    // Graceful shutdown: finish pending operations
                    flush_pending_peer_acks().await;
                }
                _ = p2p_sync_worker(tx) => {}
            }
        });
        
        // Prefetch scheduler
        let cancel = self.cancel.clone();
        let tx = self.intent_tx.clone();
        self.workers.spawn(async move {
            tokio::select! {
                _ = cancel.cancelled() => {
                    // Graceful shutdown
                }
                _ = prefetch_scheduler(tx) => {}
            }
        });
    }
    
    pub async fn shutdown(&mut self) {
        // Signal cancellation
        self.cancel.cancel();
        
        // Wait for all workers to finish
        while let Some(_) = self.workers.join_next().await {
            // Workers acknowledged cancellation
        }
        
        // Drain any remaining intents
        while let Ok(_) = self.intent_rx.try_recv() {
            // (typically none; workers stopped)
        }
    }
}
```

### Orphan Prevention

```rust
// In main event loop
async fn app_main() {
    let mut control_plane = ControlPlane::new();
    control_plane.spawn_workers().await;
    
    loop {
        tokio::select! {
            // Normal frame
            _ = frame_timer.tick() => {
                run_frame(...);
            }
            
            // Graceful shutdown on Ctrl+C
            _ = signal::ctrl_c() => {
                println!("Shutting down...");
                control_plane.shutdown().await;  // ← Waits for all workers
                break;
            }
        }
    }
}
```

---

## P2P Collaboration Example

### Workspace Sharing Scenario

```
User A and User B both have graphshell open, collaborative on shared graph.
Network: eventual consistency (both converge to same state).

Frame N on A:
  | User drags node X to (100, 200)
  → intent: SetNodePosition { key: X, pos: (100, 200), lamport: A_42 }
  → apply_intents(): A's graph updated
  → P2P worker broadcasts to B: "I set X to (100, 200) at clock 42"

Frame N on B (receives update from A):
  | P2P worker receives message
  → queues: ApplyRemoteDelta { from: A, delta: SetNodePosition_X, lamport: 42 }
  → Frame N+1: drain queue, sort by lamport
  | If B also moved a different node in frame N:
    →  all_intents = [
         SetNodePosition { Y, ..., lamport: 0 },  // Local
         ApplyRemoteDelta { X, from_A, lamport: 42 },  // Remote
       ]
  → sort: local intents first (lamport 0), then A's (clock 42)
  → apply_intents(): B's graph has both A's and B's changes
  → Serializer snapshots: B's persistent state includes A's changes

Result: Both A and B have identical graph after eventual sync ✓
```

### Conflict Resolution (Last-Write-Wins)

```rust
// When apply_intents() encounters two SetNodeUrl intents for the same node:
match (&intent_a, &intent_b) {
    (
        GraphIntent::SetNodeUrl { key, url: url_a },
        GraphIntent::SetNodeUrl { key, url: url_b },
    ) if key_a == key_b => {
        // Both intents modify same node
        // Last-write-wins: later lamport clock wins
        let (winner, loser) = if lamport_a > lamport_b {
            (intent_a, intent_b)  // A's change wins
        } else {
            (intent_b, intent_a)  // B's change wins
        };
        
        // Apply winner, ignore loser (create tombstone/audit entry if desired)
        apply_intent(winner);
    }
    _ => { /* apply both independently */ }
}
```

---

## Policy Snapshot Distribution (watch channel)

Some decisions need to be visible to background workers (memory limits, retention policies, etc.). Use `tokio::sync::watch`:

```rust
pub struct LifecyclePolicy {
    pub active_webview_limit: usize,
    pub warm_cache_limit: usize,
    pub memory_pressure_threshold: f32,
}

// In main app
let (policy_tx, policy_rx) = watch::channel(LifecyclePolicy::default());

// Prefetch worker subscribes to policy changes
async fn prefetch_scheduler(mut policy_rx: watch::Receiver<LifecyclePolicy>) {
    loop {
        // Wait for policy change OR timer
        tokio::select! {
            _ = policy_rx.changed() => {
                let policy = policy_rx.borrow();
                // Adjust prefetch aggressiveness based on memory policy
            }
            _ = timer.tick() => {
                let policy = policy_rx.borrow();
                // Use current policy for prefetch decisions
            }
        }
    }
}

// Main loop can update policy
policy_tx.send(LifecyclePolicy {
    active_webview_limit: 8,
    warm_cache_limit: 20,
    memory_pressure_threshold: 0.15,
}).ok();
```

---

## Testing Concurrent Ordering

### Property Test: Causality Invariant

```rust
#[cfg(test)]
mod tests {
    use proptest::prelude::*;
    
    #[test]
    fn prop_causality_ordering_is_deterministic() {
        proptest!(|(
            intents in prop::collection::vec(
                (0u64..1000, "a|b|c"),  // (lamport_clock, peer_id)
                10..100
            )
        )| {
            let mut intents1 = intents.clone();
            let mut intents2 = intents.clone();
            
            // Sort twice
            intents1.sort_by_key(|i| i.0);
            intents2.sort_by_key(|i| i.0);
            
            // Must be identical
            prop_assert_eq!(intents1, intents2);
        });
    }
    
    #[test]
    fn prop_concurrent_mutations_converge() {
        proptest!(|(
            mutations_a in prop::collection::vec(
                (0u64..10000, 0u64..1000000),  // (lamport, node_id)
                1..20
            ),
            mutations_b in prop::collection::vec(
                (0u64..10000, 0u64..1000000),
                1..20
            )
        )| {
            // Simulate A's changes
            let mut state_a = GraphBrowserApp::new();
            apply_mutations(&mut state_a, &mutations_a);
            
            // Simulate B receiving A's changes
            let mut state_b = GraphBrowserApp::new();
            let mut merged = mutations_b.clone();
            merged.extend(mutations_a.clone());
            merged.sort_by_key(|m| m.0);  // Sort by lamport clock
            apply_mutations(&mut state_b, &merged);
            
            // Both must have same nodes at same positions
            for node_id in 0..1000000 {
                let pos_a = state_a.get_node_position(node_id);
                let pos_b = state_b.get_node_position(node_id);
                prop_assert_eq!(pos_a, pos_b);
            }
        });
    }
}
```

### Golden Trace Test: Common Flow

```rust
#[test]
fn golden_p2p_workspace_restore() {
    let mut app = GraphBrowserApp::new();
    
    // Initial state: empty
    let trace = vec![
        QueuedIntent {
            intent: GraphIntent::CreateNodeAtUrl {
                url: "https://graphshell.dev".into(),
                position: Point2D::new(0.0, 0.0),
            },
            source: IntentSource::LocalUI,
            queued_at: Instant::now(),
        },
        QueuedIntent {
            intent: GraphIntent::ApplyRemoteDelta {
                from_peer: "peer-b".parse().unwrap(),
                delta: SetNodeUrl { node_x: ..., url: "https://example.com".into() },
                lamport_clock: 42,
            },
            source: IntentSource::P2pSync,
            queued_at: Instant::now(),
        },
    ];
    
    // Apply trace
    apply_intents(&mut app, trace);
    
    // Snapshot state
    let snapshot = insta::assert_snapshot!(format!("{:#?}", app));
    // Fine-grained assertions on specific mutations
    assert_eq!(app.graph.node_count(), 1);
}
```

---

## Actionable Insights & Implementation Guidelines

### 1. **Lamport Clock Initialization**

**Insight:** Each peer must have a persistent Lamport clock that survives crashes.

```rust
// On startup:
pub struct AppState {
    pub lamport_clock: u64,  // Persisted in graph metadata
}

impl AppState {
    pub fn next_lamport(&mut self) -> u64 {
        self.lamport_clock += 1;
        // Persist immediately to avoid rewind on crash
        self.persist_metadata();
        self.lamport_clock
    }
}
```

### 2. **Network Failure → Explicit Intent**

**Insight:** Don't silently drop network errors. Queue them as intents so the app can respond gracefully.

```rust
// Bad: ignore network failure
async fn sync_peer(addr, tx) {
    if let Ok(delta) = fetch_from_peer(addr).await {
        tx.send(intent).await.ok();  // Only success
    }
    // Failure is silent; peer is forgotten
}

// Good: explicit offline state
async fn sync_peer(addr, tx) {
    match fetch_from_peer(addr).await {
        Ok(delta) => tx.send(GraphIntent::ApplyRemoteDelta { ... }).await.ok(),
        Err(_) => tx.send(GraphIntent::MarkPeerOffline { 
            peer: addr, 
            retry_at: Instant::now() + Duration::from_secs(5) 
        }).await.ok(),
    }
}
```

### 3. **Serializer Snapshots After apply()**

**Insight:** Persistence must snapshot **after** apply_intents() completes but **before** reconcile_webview_lifecycle() runs, to ensure peer changes are persisted atomically.

```rust
fn run_frame(app, intent_rx, serializer_tx) {
    // Collect + apply
    let intents = collect_all_intents(intent_rx);
    apply_intents(app, intents);
    
    // SNAPSHOT WINDOW: peer changes are now in app state
    if should_autosave() {
        serializer_tx.send(SnapshotRequest {
            graph: app.graph.clone(),
            timestamp: Instant::now(),
        }).ok();
    }
    
    // Reconcile (side effects don't affect persistence)
    reconcile_webview_lifecycle(app);
    
    render(app);
}
```

### 4. **Intent Queue Capacity Planning**

**Insight:** Channel capacity is your defense against flooding attacks or broken workers.

| Capacity | Scenario | When to Use |
|----------|----------|-----------|
| 64 | Single P2P peer, low-latency LAN | Peer discovery, small collaborative teams |
| 256 | 2–5 P2P peers, moderate network latency | Default for graphshell |
| 512+ | Many producers (prefetch, memory monitor, multiple peers), high-latency WAN | High-load server deployments |

```rust
// Choose based on environment
let capacity = if is_server { 1024 } else { 256 };
let (tx, rx) = mpsc::channel::<QueuedIntent>(capacity);
```

### 5. **Debugging Causality Violations**

**Insight:** Log causality order violations to catch protocol bugs early.

```rust
fn run_frame(app, intent_rx, logger) {
    let mut all_intents = collect_intents(intent_rx);
    
    // Check for violations
    for i in 1..all_intents.len() {
        if all_intents[i].causality_order() < all_intents[i-1].causality_order() {
            logger.warn!(
                "Causality violation: intent {} (clock {}) before {} (clock {})",
                i-1, all_intents[i-1].causality_order().0,
                i, all_intents[i].causality_order().0
            );
        }
    }
    
    all_intents.sort_by_key(|i| i.causality_order());
    apply_intents(app, all_intents);
}
```

### 6. **CRDTs for Conflict-Free Resolution**

**Insight:** Last-write-wins works for simple cases (single node mutations). For more complex state (lists, sets, maps), consider CRDTs (Conflict-free Replicated Data Types).

**When to upgrade:**
- Multiple peers simultaneously reorder list of nodes (tab order, workspace tab order)
- Multiple peers simultaneously add/remove from sets (workspace membership, tags)
- Simple last-write-wins would lose concurrent additions

**Example:** Workspace membership should use a CRDT Set, not last-write-wins:

```rust
// Bad: last-write-wins loses concurrent adds
let mut members = set!["A", "B"];
apply(RemoveFromSet { "B" });  // Peer A removes
apply(AddToSet { "B" });        // Peer B adds (concurrent)  
// Result: depends on order; can lose concurrent add

// Good: CRDT-based (e.g., OR-Set)
let mut members_or = ORSet::new();
members_or.add("A");
members_or.add("B");
members_or.remove("B", timestamp: 42);  // Peer A removes at t=42
members_or.add("B", timestamp: 43);     // Peer B adds at t=43 (wins because later)
// Result: "B" is in set (concurrent add with higher timestamp wins)
```

### 7. **Test Coverage: Ordering, Convergence, Supervision**

**Insight:** Focus testing on three critical properties:

1. **Deterministic ordering:** Same intents always produce same result
2. **Eventual convergence:** All peers end up identical after sync
3. **Graceful degradation:** Network failures don't crash the app

```rust
#[test]
fn test_intent_ordering_deterministic() { /* prop test */ }

#[test]
fn test_p2p_eventual_convergence() { /* concurrent peers */ }

#[test]
fn test_network_failure_emits_offline_intent() { /* resilience */ }

#[test]
fn test_graceful_shutdown_waits_for_workers() { /* supervision */ }
```

---

## Integration Timeline: Stages 1–5

| Stage | When | What | Notes |
|-------|------|------|-------|
| **1–3** | Q1–Q2 2026 | Sync lifecycle, embedder split, GUI decomposition | Reducer is rock-solid before adding async |
| **4** | Q2 2026 | GUI decomposition complete | Single-threaded, testable without background tasks |
| **5 Design** | Q3 2026 | This doc + control-plane module skeleton | Async producers not yet active |
| **5 Prototype** | Q3 2026 | Single background worker (e.g., memory monitor) | Validate channel, backpressure, ordering |
| **P2P Design** | Q3–Q4 2026 | Peer discovery, Lamport clocks, causality | Separate design doc (see below) |
| **P2P Prototype** | Q4 2026 | Single-peer sync | Prove intent queuing works for network |
| **Multi-Peer** | Q1 2027 | N-peer convergence, CRDTs as needed | Production collab ready |

---

## Future: P2P Collaboration Plan

This doc handles the **control plane** (how async producers feed intents). The **P2P collaboration plan** (separate doc) will cover:

- Peer discovery and rendezvous (mDNS, DHT, or central coordinator)
- Replication model (eventual consistency vs. strong consistency)
- Conflict resolution policies (last-write-wins, CRDT, user prompts)
- Offline queueing (local mutations while peer unreachable)
- Bandwidth optimization (delta compression, selective sync)
- Encryption and trust model

---

## References

- [2026-02-20_embedder_decomposition_plan.md](2026-02-20_embedder_decomposition_plan.md) — Stage 5 overview and trigger criteria
- [2026-02-21_lifecycle_intent_model.md](2026-02-21_lifecycle_intent_model.md) — Intent schema and lifecycle state machine
- [2026-02-16_architecture_and_navigation_plan.md](2026-02-16_architecture_and_navigation_plan.md) — Two-phase apply model foundation
- Crates to evaluate: `tokio`, `tokio-util`, `darling` (for Lamport clocks), `rkyv`/`serde` (intent serialization for P2P)
