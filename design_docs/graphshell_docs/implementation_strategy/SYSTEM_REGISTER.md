<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# System Register (RegisterRuntime + Control Panel + Signal Routing)

**Doc role:** Canonical hub / implementation guide for the Register runtime composition boundary.
**Status:** Active / canonical (maintained; historical CP1/CP2 implementation details preserved below)
**Short label:** `system_register`
**Primary runtime types:** `RegistryRuntime`, `ControlPanel`
**Related docs:**
- [2026-02-20_embedder_decomposition_plan.md](2026-02-20_embedder_decomposition_plan.md) (embedder decomposition context)
- [2026-02-20_settings_architecture_plan.md](2026-02-20_settings_architecture_plan.md) (settings/control coordination model; Register-adjacent orchestration surface)
- [2026-02-21_lifecycle_intent_model.md](2026-02-21_lifecycle_intent_model.md) (intent schema and reducer boundary)
- [2026-02-22_registry_layer_plan.md](2026-02-22_registry_layer_plan.md) (registry architecture and provider wiring)
- [2026-02-26_composited_viewer_pass_contract.md](2026-02-26_composited_viewer_pass_contract.md) (composited viewer/backend render-pass ownership and overlay policy; relevant to viewer/compositor routing boundaries)
- [PLANNING_REGISTER.md](PLANNING_REGISTER.md) (cross-subsystem sequencing / backlog)

---

## Purpose

The **Control Panel** is the async adapter layer that allows concurrent background producers — network sync, prefetch scheduler, memory monitor, mod loader lifecycle — to feed intents into the synchronous two-phase reducer without compromising determinism or testability.

The Control Panel is a core component of **The Register** (implemented today as `RegistryRuntime` + `ControlPanel`, with a transitional signal-routing layer and a planned `SignalBus`-class abstraction).

**Key principle:** The reducer stays 100% synchronous and testable. All I/O and background work happens in supervised tokio tasks that communicate exclusively via the intent queue. This is an async *adapter layer* around a deterministic sync core, not an async rewrite.

---

## Hub Scope

This hub document exists to keep the Register architecture current without forcing all runtime coordination material into subsystem docs.

In scope:
- Register composition boundary (`RegistryRuntime`, `ControlPanel`, signal/event routing layer)
- Async worker supervision and intent ingress policy
- Cross-registry event distribution strategy (`SignalBus` or equivalent)
- Ownership boundaries between registries, mods, subsystems, and runtime coordinators

Out of scope:
- Subsystem-specific contracts/validation (see subsystem guides)
- Feature-specific workers (covered in their feature/subsystem docs except for Register-facing integration points)

## Current Status & Gaps

Implemented:
- `ControlPanel` async worker supervision and multi-producer intent queueing
- Main GUI integration for control-panel worker lifecycle and intent draining
- RegistryRuntime composition root for atomic/domain registries and mod/runtime services
- RegistryRuntime provider-wired phase0 protocol/viewer dispatch paths and diagnostics coverage
- Folded viewer/surface capability-conformance declarations with runtime diagnostics inspection hooks

Gaps / active architectural work:
- Signal/event routing is still transitional (no dedicated `SignalBus` abstraction/API yet)
- Canonical docs/terminology wording still needs tightening around `Signal` vs `Intent` vs direct calls (routing rules are defined here but not yet propagated everywhere)
- Some authority-boundary misroutes are still too silent in fallback/no-op paths and should surface more explicitly during development

## Architecture Roles (Register vs Control Panel vs SignalBus)

- **The Register**: Runtime composition root / infrastructure host (`RegistryRuntime`) that owns registries, mod/runtime wiring, and supervises the `ControlPanel`.
- **Control Panel**: Async coordination/process host for workers that produce intents and background runtime tasks.
- **SignalBus (or equivalent)**: Typed event distribution fabric for decoupled publish/subscribe between registries, mods, subsystems, and observers. Architectural role is expected; concrete API is future work.

## Routing Decision Rules (Signal vs Intent vs Call)

This section defines the canonical decision rule for choosing between routing
mechanisms. Every cross-module interaction in the codebase should map cleanly to
one of these four rows.

### Decision table

| Mechanism | When to use | Authority boundary |
| --------- | ----------- | ------------------ |
| **Direct call** | Same module / same struct; synchronous, co-owned state | No boundary crossing |
| **`GraphIntent` → `apply_intents()`** | Mutation of graph/workspace data model; must be deterministic, testable, WAL-logged | Graph Reducer boundary |
| **`WorkbenchIntent` (frame-loop intercept)** | Mutation of tile-tree shape (`egui_tiles`); workbench authority owns layout | Workbench Mutation Authority |
| **Signal / `SignalBus`** | Decoupled cross-registry or cross-subsystem notification; emitter must not know observer | Register-owned signal layer |

### The two-authority model (explicit)

The architecture has **two distinct mutation authorities** — not one:

**1. Graph Reducer** (`apply_intents` in `app.rs`)

Authoritative for:

- Graph data model (nodes, edges, selections)
- Node/edge lifecycle transitions (Cold → Warm → Hot and reverse)
- Traversal history, WAL journal, undo/redo checkpoints
- WebView ↔ node mapping (`MapWebview`, `UnmapWebview`)

Properties: always synchronous, always logged, always testable in isolation.

**2. Workbench Authority** (Gui frame loop, `tile_behavior.rs`, `tile_view_ops.rs`)

Authoritative for:

- Tile-tree shape mutations (splits, tabs, pane open/close/focus)
- `TileKind` pane insertion, removal, and focus changes

The tile tree is an `egui_tiles` construct, not graph state. Tile mutations do
not need the WAL, the graph reducer, or `ControlPanel` involvement.

Intents that cross from workbench into the graph reducer (e.g. `OpenToolPane`
dispatched during a graph interaction) belong in the Workbench Authority
intercept path (`handle_tool_pane_intents` in `gui.rs`). This is correct
architecture — not a gap — **provided the intercept is documented and
consistent**.

**The current gap:** `apply_intents` silently no-ops on workbench-authority
intents (`OpenToolPane`, `SplitPane`, `SetPaneView`, `OpenNodeInPane`), making
authority mis-routing invisible. The fix (tracked as item E in the gap-analysis
plan) is to emit a `log::warn!` on these arms so mis-routing surfaces
immediately during development.

### Routing anti-patterns to avoid

- **Do not call `apply_intents` for tile-tree mutations.** Tile layout is
  workbench authority; routing it through the graph reducer couples layout
  to the WAL and makes tests harder.
- **Do not use direct calls across registry boundaries.** Two registries that
  need to coordinate should use a Signal or delegate to `ControlPanel`, not
  call each other's internal methods.
- **Do not accumulate workbench state in `GraphBrowserApp` workspace fields.**
  Legacy booleans like `show_history_manager` are a bridge for the migration
  period only; all new pane-open/close state lives in the tile tree.
- **Do not bypass `ControlPanel` for background intent producers.** All
  background tasks that produce `GraphIntent` values must go through
  `ControlPanel`'s supervised worker model, not spawn independent threads.

## Implementation Roadmap (Register-Local)

### SR1: Normalize Register Hub Boundaries (near-term)

Goals:
- Keep `RegistryRuntime` as the composition root and `ControlPanel` as the async coordinator (avoid collapsing terms prematurely)
- Remove doc/code mismatches that imply a concrete `SignalBus` implementation where none exists
- Make signal/event routing a named internal layer owned by the Register (even if still direct/transitional)

Done gates:
- [ ] Hub docs and terminology consistently describe `SignalBus` as planned / equivalent abstraction
- [x] `ControlPanel` APIs and comments avoid implying ownership of registries
- [x] `RegistryRuntime` integration issue follow-ups are linked from this hub (`#81`, `#82`)

### SR2: Introduce Signal Routing Layer Contract (typed signals, no hard pub/sub yet)

Goals:
- Define typed signal envelopes and ownership (`signal types`, payloads, source metadata, optional causality stamp)
- Define publisher/observer contracts for registries and subsystems
- Provide an internal Register-owned routing facade that can start as direct fanout

Why:
- Registries need to publish/observe changes without direct wiring
- Mods need to trigger cross-registry workflows without point-to-point coupling
- Subsystem health/diagnostics propagation benefits from decoupled observers
- Future async/event-heavy coordination will otherwise push coupling into `ControlPanel`

Done gates:
- [ ] `Signal` type families and source metadata contract documented
- [ ] Register-owned routing API skeleton exists (facade/trait/module)
- [ ] At least one producer + two observers integrated through the routing contract

### SR3: `SignalBus` (or Equivalent) Pub/Sub Implementation

Goals:
- Implement publish/subscribe event distribution fabric under the Register
- Support typed signals and decoupled observers
- Preserve deterministic reducer semantics by keeping signal handling outside direct state mutation paths (or routing to intents where needed)

Core requirements:
- Publish/subscribe
- Typed signals
- Decoupled observers
- Backpressure/error handling policy (drop/coalesce/retry diagnostics must be explicit)
- Diagnostics hooks (queue depth, dropped signals, handler failures, latency)

Done gates:
- [ ] Register signal bus/fabric implementation replaces transitional direct routing in selected paths
- [ ] Diagnostics channels report signal routing health
- [ ] Mod-triggered cross-registry workflow path uses signal routing instead of direct wiring
- [ ] Subsystem health propagation path uses signal routing (or equivalent observer fabric)

### SR4: Register Runtime Authoritative Dispatch Cleanup

Goals:
- Complete migration away from remaining legacy desktop dispatch paths
- Make Register-owned provider wiring and event routing the canonical runtime path
- Keep `ControlPanel` focused on worker orchestration, not cross-registry policy dispatch

Done gates:
- [ ] Legacy dispatch callsites removed or wrapped behind Register APIs
- [x] Legacy dispatch callsites removed or wrapped behind Register APIs (phase0 navigation/provider-wired runtime dispatch slice; see `#82`)
- [ ] `RegistryRuntime` + signal routing layer responsibilities are documented and tested
- [ ] `ControlPanel` API surface reflects coordinator/process-host role only

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

**Status:** ✅ Complete (2026-02-24). Integrated into Gui frame loop.

**Update (2026-02-24)** — ControlPanel fully wired into main app:
- Added `tokio_runtime: tokio::runtime::Runtime` and `control_panel: ControlPanel` fields to `Gui` struct
- Workers spawned in `Gui::new()` within runtime context: memory monitor, mod loader, sync worker
- Intent channel drained each frame via `control_panel.drain_pending()` before `apply_intents`
- Graceful shutdown in `Drop` via `tokio_runtime.block_on(control_panel.shutdown())`

Goals:
- Mod loader worker supervises mod load/unload lifecycle events
- Failed mod loads emit `GraphIntent::ModLoadFailed { mod_id, reason }` through the channel
- Successful mod activations emit `GraphIntent::ModActivated { mod_id }`
- Mod worker respects cancellation token for graceful shutdown

Done gates:
- [x] `ControlPanel::spawn_mod_loader()` implemented
- [x] mod lifecycle intents defined in `GraphIntent` (ModActivated, ModLoadFailed)
- [x] mod worker cancels cleanly on token signal
- [x] Coordinated with Registry Phase 2: `NativeModActivations` wired to `ModRegistry::activate_native_mod()`
- [x] Verso native mod defined and registered at compile time
- [x] Both Verse and Verso mods discoverable via `discover_native_mods()`

**Implementation Notes**:
- `discover_native_mods()` uses `inventory::collect!()` to gather `NativeModRegistration` entries
- `resolve_mod_load_order()` performs topological sort on mod dependencies
- `ModRegistry::load_all()` calls `activate_native_mod()` for each mod in order
- `NativeModActivations` dispatch table maps mod_id → activation function
- Mod activation is synchronous; long-running operations (I/O) would be spawned as sub-workers in future phases

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
| **CP2** | ✅ Done | Mod loader worker + mod lifecycle intents |
| **CP3** | Future | Prefetch scheduler + `LifecyclePolicy` watch |
| **CP4** | Future | P2P sync worker (separate design doc) |

---

## References

- [2026-02-20_embedder_decomposition_plan.md](2026-02-20_embedder_decomposition_plan.md) — Stage 5 overview
- [2026-02-21_lifecycle_intent_model.md](2026-02-21_lifecycle_intent_model.md) — Intent schema and lifecycle state machine
- [2026-02-22_registry_layer_plan.md](2026-02-22_registry_layer_plan.md) — The Register architecture and provider wiring
- [PLANNING_REGISTER.md](PLANNING_REGISTER.md) — sequencing and backlog for Register/runtime follow-ups
- Crates: `tokio`, `tokio-util` (CancellationToken), `tokio::task::JoinSet`
