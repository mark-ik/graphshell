<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Control Panel: Async Scaling & Multi-Producer Intent Queuing

**Date:** 2026-02-21 (revised 2026-02-23)
**Status:** In Progress — Phase CP1 implemented; Phase CP2 pending
**Related Plans:**
- [2026-02-20_embedder_decomposition_plan.md](2026-02-20_embedder_decomposition_plan.md) (Stage 5 trigger)
- [2026-02-21_lifecycle_intent_model.md](2026-02-21_lifecycle_intent_model.md)
- [2026-02-22_registry_layer_plan.md](2026-02-22_registry_layer_plan.md) (The Register)

---

## Purpose

The **Control Panel** is the async adapter layer that allows concurrent background producers — network sync, prefetch scheduler, memory monitor, mod loader lifecycle — to feed intents into the synchronous two-phase reducer without compromising determinism or testability.

The Control Panel is a core component of **The Register** (`RegistryRuntime` + `ControlPanel` + `SignalBus`).

**Key principle:** The reducer stays 100% synchronous and testable. All I/O and background work happens in supervised tokio tasks that communicate exclusively via the intent queue. This is an async *adapter layer* around a deterministic sync core, not an async rewrite.

---

## Architecture

### The Two-Layer Model

```
┌─────────────────────────────────────────────────────────┐
│                   Frame Loop (sync)                     │
│  drain intent_rx → sort by causality → apply_intents()  │
│  → reconcile_webview_lifecycle() → render()             │
└─────────────────────────────────────────────────────────┘
            ↑ intent_rx (non-blocking try_recv)
┌─────────────────────────────────────────────────────────┐
│                ControlPanel (async layer)               │
│  intent_tx  │  CancellationToken  │  JoinSet<workers>  │
│─────────────────────────────────────────────────────────│
│  memory_monitor_worker (active)                        │
│  mod_loader_worker       (stub)                        │
│  prefetch_scheduler      (future)                      │
│  p2p_sync_worker         (future)                      │
└─────────────────────────────────────────────────────────┘
```

### Core Structs

```rust
/// Intent with source tracking and causality ordering.
#[derive(Debug, Clone)]
pub struct QueuedIntent {
    pub intent: GraphIntent,
    pub queued_at: Instant,
    pub source: IntentSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum IntentSource {
    /// User keyboard/mouse input
    LocalUI,
    /// Servo browser delegate (navigation, load completion, etc.)
    ServoDelegate,
    /// Memory/system monitor
    MemoryMonitor,
    /// Mod loader lifecycle events
    ModLoader,
    /// Background prefetch scheduler
    PrefetchScheduler,
    /// P2P sync worker
    P2pSync,
    /// Restore/replay from persistence
    Restore,
}

pub struct ControlPanel {
    /// Sender cloned to each background worker.
    pub intent_tx: mpsc::Sender<QueuedIntent>,
    /// Receiver drained by the sync frame loop each tick.
    pub intent_rx: mpsc::Receiver<QueuedIntent>,
    /// Shared cancellation token — cancel() stops all workers.
    cancel: CancellationToken,
    /// Supervised set of background worker tasks.
    workers: JoinSet<()>,
}
```

### Frame Loop Integration

```rust
pub fn run_frame(
    app: &mut GraphBrowserApp,
    control_panel: &mut ControlPanel,
    // ... other args
) {
    let mut all_intents = Vec::new();

    // Local UI intents (synchronous)
    all_intents.extend(collect_keyboard_intents());
    all_intents.extend(collect_mouse_intents());

    // Servo delegate events → intents (synchronous)
    all_intents.extend(graph_intents_from_pending_semantic_events());

    // Async producer intents (non-blocking drain)
    while let Ok(queued) = control_panel.intent_rx.try_recv() {
        all_intents.push(queued.intent);
    }

    // Sort by causality for determinism
    all_intents.sort_by_key(|intent| intent.causality_order());

    // Apply atomically (pure state, no side effects)
    apply_intents(app, all_intents);

    // Reconcile (side effects: webview creation/destruction)
    reconcile_webview_lifecycle(app);

    render(app);
}
```

---

## Implementation Phases

### Phase CP1: Core Struct + Channel + Memory Monitor

**Status:** ✅ Complete (2026-02-23)

Goals:
- `QueuedIntent`, `IntentSource`, `ControlPanel` struct defined
- Intent channel initialized (`mpsc::channel` capacity 256)
- `CancellationToken` wired to all workers
- `JoinSet` supervises all background tasks
- `memory_monitor_worker` stub spawned (emits `DemoteNodeToCold` under pressure)
- `shutdown()` cancels all workers and awaits `JoinSet` drain

Files:
- `desktop/control_panel.rs` (new)
- `desktop/mod.rs` — expose `control_panel` module

Done gates:
- [x] `ControlPanel::new()` creates channel + token + empty `JoinSet`
- [x] `ControlPanel::spawn_memory_monitor()` spawns first supervised worker
- [x] `ControlPanel::shutdown()` gracefully stops all workers

### Phase CP2: Mod Loader Supervision

**Status:** Pending

Goals:
- Mod loader worker supervises mod load/unload lifecycle events
- Failed mod loads emit `GraphIntent::ModLoadFailed { mod_id, reason }` through the channel
- Successful mod activations emit `GraphIntent::ModActivated { mod_id }`
- Mod worker respects cancellation token for graceful shutdown

Done gates:
- [ ] `ControlPanel::spawn_mod_loader()` added
- [ ] mod lifecycle intents defined in `GraphIntent`
- [ ] mod worker cancels cleanly on token signal

### Phase CP3: Prefetch Scheduler

**Status:** Future

Goals:
- Periodically emit `PromoteNodeToActive { cause: SelectedPrewarm }` based on graph heuristics
- Subscribes to `watch::Receiver<LifecyclePolicy>` for memory budget awareness
- Uses exponential backoff on channel congestion

Done gates:
- [ ] `prefetch_scheduler_worker` implemented and spawned
- [ ] `LifecyclePolicy` watch channel wired to scheduler

### Phase CP4: P2P Sync

**Status:** Future (separate design doc required)

Goals:
- P2P worker syncs peer deltas and queues `ApplyRemoteDelta` intents with Lamport clock stamps
- Network failures emit `MarkPeerOffline` intents (never silent)
- Causality ordering ensures convergence across all peers

Done gates:
- [ ] Peer discovery and rendezvous design complete
- [ ] `p2p_sync_worker` implemented and supervised
- [ ] Lamport clock persistence wired into `GraphBrowserApp`

---

## Background Worker: Supervision & Cancellation

All workers follow the same cancellation contract:

```rust
impl ControlPanel {
    pub fn new() -> Self {
        let (intent_tx, intent_rx) = mpsc::channel(256);
        Self {
            intent_tx,
            intent_rx,
            cancel: CancellationToken::new(),
            workers: JoinSet::new(),
        }
    }

    pub fn spawn_memory_monitor(&mut self) {
        let cancel = self.cancel.clone();
        let tx = self.intent_tx.clone();
        self.workers.spawn(async move {
            tokio::select! {
                _ = cancel.cancelled() => {}
                _ = memory_monitor_worker(tx) => {}
            }
        });
    }

    pub async fn shutdown(&mut self) {
        self.cancel.cancel();
        while self.workers.join_next().await.is_some() {}
    }
}

async fn memory_monitor_worker(tx: mpsc::Sender<QueuedIntent>) {
    loop {
        tokio::time::sleep(Duration::from_secs(5)).await;
        let usage = estimate_memory_usage();
        if usage > MEMORY_PRESSURE_THRESHOLD {
            let _ = tx.send(QueuedIntent {
                intent: GraphIntent::DemoteNodeToCold {
                    key: pick_lru_candidate(),
                    cause: LifecycleCause::MemoryPressureWarning,
                },
                queued_at: Instant::now(),
                source: IntentSource::MemoryMonitor,
            }).await;
        }
    }
}
```

### Orphan Prevention

```rust
async fn app_main() {
    let mut control_panel = ControlPanel::new();
    control_panel.spawn_memory_monitor();

    loop {
        tokio::select! {
            _ = frame_timer.tick() => {
                run_frame(&mut app, &mut control_panel);
            }
            _ = signal::ctrl_c() => {
                control_panel.shutdown().await;
                break;
            }
        }
    }
}
```

---

## Mod Loader Supervision

The mod loader runs as a supervised worker under `ControlPanel`. Mods are loaded at startup and on demand, with lifecycle events emitted as intents:

```rust
async fn mod_loader_worker(tx: mpsc::Sender<QueuedIntent>) {
    // Phase CP2: scan mod directory and attempt loads
    for mod_path in scan_mod_directory() {
        match load_mod(&mod_path).await {
            Ok(mod_id) => {
                let _ = tx.send(QueuedIntent {
                    intent: GraphIntent::ModActivated { mod_id },
                    queued_at: Instant::now(),
                    source: IntentSource::ModLoader,
                }).await;
            }
            Err(reason) => {
                let _ = tx.send(QueuedIntent {
                    intent: GraphIntent::ModLoadFailed { mod_id: mod_path.to_string(), reason },
                    queued_at: Instant::now(),
                    source: IntentSource::ModLoader,
                }).await;
            }
        }
    }
}
```

---

## Backpressure & Flooding Prevention

Channel capacity is the primary defense against flooding from misbehaving workers:

| Capacity | Scenario |
|----------|----------|
| 64 | Single worker, local-only (testing) |
| 256 | Default: memory monitor + mod loader + prefetch |
| 512+ | Multi-peer P2P with high-latency WAN |

Workers respect backpressure via `.await` on `tx.send()`:

```rust
// Worker blocks if channel is full (main loop too slow to drain)
tx.send(intent).await.ok();

// For non-critical intents, prefer try_send + log drop:
if tx.try_send(intent).is_err() {
    log::debug!("control_panel: intent queue full, dropping non-critical intent");
}
```

---

## Causality Ordering

Async producer intents carry causality metadata for deterministic application:

```rust
impl QueuedIntent {
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

Local UI intents (clock 0) always apply before async producer intents, preserving responsiveness. Remote deltas sort by Lamport clock for cross-peer convergence.

---

## Policy Distribution (watch channel)

Background workers that need app policy (memory limits, retention) subscribe via `watch`:

```rust
pub struct LifecyclePolicy {
    pub active_webview_limit: usize,
    pub warm_cache_limit: usize,
    pub memory_pressure_threshold: f32,
}

// Wired in ControlPanel::new()
let (policy_tx, policy_rx) = watch::channel(LifecyclePolicy::default());

// Workers receive policy_rx clone at spawn time
async fn prefetch_scheduler(mut policy_rx: watch::Receiver<LifecyclePolicy>, tx: ...) {
    loop {
        tokio::select! {
            _ = policy_rx.changed() => { /* adjust on policy change */ }
            _ = timer.tick() => {
                let policy = policy_rx.borrow();
                // use policy.warm_cache_limit for prefetch decisions
            }
        }
    }
}
```

---

## Testing Plan

```rust
// CP1 (done gates)
#[test]
fn control_panel_new_creates_empty_joinset() { ... }

#[test]
fn control_panel_shutdown_stops_all_workers() { ... }

#[test]
fn memory_monitor_emits_demotion_intent_under_pressure() { ... }

// CP2
#[test]
fn mod_loader_emits_activated_intent_on_success() { ... }

#[test]
fn mod_loader_emits_failed_intent_on_load_error() { ... }

// CP3/CP4
#[test]
fn intent_ordering_deterministic_under_concurrent_producers() { ... }

#[test]
fn p2p_network_failure_emits_offline_intent_not_silent_drop() { ... }

#[test]
fn graceful_shutdown_drains_joinset_before_exit() { ... }
```

---

## Integration Timeline

| Phase | Status | What |
|-------|--------|------|
| **CP1** | ✅ Done | `ControlPanel` struct, channel, memory monitor stub, shutdown |
| **CP2** | Pending | Mod loader worker + mod lifecycle intents |
| **CP3** | Future | Prefetch scheduler + `LifecyclePolicy` watch |
| **CP4** | Future | P2P sync worker (separate design doc) |

---

## References

- [2026-02-20_embedder_decomposition_plan.md](2026-02-20_embedder_decomposition_plan.md) — Stage 5 overview
- [2026-02-21_lifecycle_intent_model.md](2026-02-21_lifecycle_intent_model.md) — Intent schema and lifecycle state machine
- [2026-02-22_registry_layer_plan.md](2026-02-22_registry_layer_plan.md) — The Register architecture (RegistryRuntime + ControlPanel + SignalBus)
- Crates: `tokio`, `tokio-util` (CancellationToken), `tokio::task::JoinSet`
