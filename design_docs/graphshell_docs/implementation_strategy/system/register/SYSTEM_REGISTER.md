<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# System Register (RegisterRuntime + Control Panel + Signal Routing)

**Doc role:** Canonical hub / index for the Register layer and its component specs; subordinate to the top-level system architecture.
**Status:** Active / canonical (maintained; historical CP1/CP2 implementation details preserved below)
**Short label:** `system_register`
**Primary runtime types:** `RegistryRuntime`, `ControlPanel`
**Related docs:**
- [../system_architecture_spec.md](../system_architecture_spec.md) (top-level system architecture parent spec)
- [../register_layer_spec.md](../register_layer_spec.md) (canonical Register layer spec)
- [../registry_runtime_spec.md](../registry_runtime_spec.md) (`RegistryRuntime` component spec)
- [../control_panel_spec.md](../control_panel_spec.md) (`ControlPanel` component spec)
- [../signal_bus_spec.md](../signal_bus_spec.md) (`SignalBus` / signal-routing component spec)
- [../coop_session_spec.md](../coop_session_spec.md) (`Coop` host-led co-presence contract; distinct from device sync)
- [../aspect_render/2026-02-20_embedder_decomposition_plan.md](../aspect_render/2026-02-20_embedder_decomposition_plan.md) (embedder decomposition context)
- [../2026-02-21_lifecycle_intent_model.md](../2026-02-21_lifecycle_intent_model.md) (intent schema and reducer boundary)
- [../2026-02-22_registry_layer_plan.md](../2026-02-22_registry_layer_plan.md) (registry architecture and provider wiring)
- [../../PLANNING_REGISTER.md](../../PLANNING_REGISTER.md) (cross-subsystem sequencing / backlog)

**Policy authority**: This file is the canonical policy authority for Register hub rules and register-spec family coordination.
Policy in this file should be distilled from canonical specs and accepted research conclusions.

## Register Hub Policies

1. **Family-coordination policy**: Register-wide routing and boundary policy is coordinated here; component-local semantics remain in component specs.
2. **Authority-routing policy**: Direct call vs signal vs intent routing must follow the canonical decision table and authority boundaries.
3. **Misroute-visibility policy**: Boundary misroutes and no-op fallthroughs should surface during development via explicit warnings/diagnostics.
4. **No-catch-all policy**: This hub does not replace subsystem or component authority docs; it reconciles them.
5. **Convergence policy**: Transitional routing patterns must converge toward explicit typed signal and provider-routed contracts.

---

## Register Component Spec Family

The Register layer is split into separate canonical specs:

- [../register_layer_spec.md](../register_layer_spec.md)
- [../registry_runtime_spec.md](../registry_runtime_spec.md)
- [../control_panel_spec.md](../control_panel_spec.md)
- [../signal_bus_spec.md](../signal_bus_spec.md)

This hub remains the navigation/index surface and historical implementation guide for Register-local material.

## Purpose

The **Control Panel** is the async adapter layer that allows concurrent background producers — network device sync, prefetch scheduler, memory monitor, mod loader lifecycle — to feed intents into the synchronous two-phase reducer without compromising determinism or testability.

The Control Panel is a core component of **The Register** (implemented today as `RegistryRuntime` + `ControlPanel`, with a transitional signal-routing layer and a planned `SignalBus`-class abstraction).

**Key principle:** The reducer stays 100% synchronous and testable. All I/O and background work happens in supervised tokio tasks that communicate exclusively via the current intent queue. This is an async *adapter layer* around a deterministic sync core, not an async rewrite.

### Sync Terminology Contract

- **Device Sync**: durable workspace replication between trusted devices (remote delta carrier intents, peer status, version-vector convergence).
- **Coop**: collaborative/co-presence behavior (live follow/presence/shared browsing context) and not implied by Device Sync.
- UI and docs should use explicit labels (`Sync Devices`, `Start Coop`) instead of plain `Sync` when both concepts are present.

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
- Runtime-owned content-pipeline completion: URI-aware protocol MIME inference, cancellable
  content-type probes, viewer capability description, viewer-surface profile resolution, and
  content-aware lens composition
- Folded viewer/surface capability-conformance declarations with runtime diagnostics inspection hooks
- Runtime-owned canvas, physics, layout-domain, and presentation-domain profile resolution paths
- Runtime-owned diagnostics, knowledge, and index authorities with semantic lifecycle signaling and
  omnibox submit-path search fanout

Gaps / active architectural work:
- Signal/event routing is still transitional (no dedicated `SignalBus` abstraction/API yet)
- Canonical docs/terminology wording still needs tightening around `Signal` vs `Intent` vs direct calls (routing rules are defined here but not yet propagated everywhere)
- Some authority-boundary misroutes are still too silent in fallback/no-op paths and should surface more explicitly during development
- Layout execution still uses `egui_graphs` as the widget substrate, but algorithm ownership is now
  registry-owned through `LayoutRegistry` + `app/graph_layout.rs`; remaining work here is
  stabilization, not missing authority structure
- Omnibar suggestion-dropdown UI still has a legacy local candidate pipeline; only the submit/action
  path is currently unified through `IndexRegistry`
- `index:timeline` remains future history work; the provider shape is planned, but no live timeline
  index source exists yet

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
| **Current reducer carrier (`GraphIntent` / reducer-intent path)** | Mutation of reducer-owned semantic graph data model; must be deterministic, testable, WAL-logged | Graph Reducer boundary |
| **`WorkbenchIntent` (frame-loop intercept)** | Mutation of tile-tree shape (`egui_tiles`); workbench authority owns layout | Workbench Mutation Authority |
| **Signal / `SignalBus`** | Decoupled cross-registry or cross-subsystem notification; emitter must not know observer | Register-owned signal layer |

### The two-authority model (explicit)

The architecture has **two distinct mutation authorities** — not one:

**1. Graph Reducer** (`apply_reducer_intents` in `graph_app.rs`)

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
intercept path (`handle_tool_pane_intents` in `shell/desktop/ui/gui.rs`). This is correct
architecture — not a gap — **provided the intercept is documented and
consistent**.

**Current status (2026-03-10):**
- The old silent-no-op gap has been addressed by a reducer-side
  warning/classification seam for graph-carrier intents that are actually
  workbench-authority bridges.
- `WorkbenchSurfaceRegistry` now exists and is the concrete workbench authority
  object reached by the frame-loop adapter path.
- The practical bridge seam is still not raw `WorkbenchIntent` reaching
  `apply_intents()` by type; it is graph-carrier bridge intents such as
  `RouteGraphViewToWorkbench` reaching reducer ingress before being forwarded to
  workbench authority.
- Workflow lifecycle changes now publish through the Register signal-routing
  layer, and Sector D now provides runtime-stateful canvas/physics/layout
  authorities for those activations.
- Semantic-index changes now also publish through the Register signal-routing
  layer, and the GUI-side observer path re-resolves registry-backed view lenses
  when those lifecycle notifications arrive.

### Routing anti-patterns to avoid

- **Do not call `apply_intents` for tile-tree mutations.** Tile layout is
  workbench authority; routing it through the graph reducer couples layout
  to the WAL and makes tests harder.
- **Do not use direct calls across registry boundaries.** Two registries that
  need to coordinate should use a Signal or delegate to `ControlPanel`, not
  call each other's internal methods.
- **Do not accumulate workbench state in `GraphBrowserApp` workspace fields.**
  Legacy panel booleans have been removed; pane-open/close state must live in
  the tile tree.
- **Do not bypass `ControlPanel` for background intent producers.** All
  background tasks that produce current reducer-carrier values must go through
  `ControlPanel`'s supervised worker model, not spawn independent threads.

Carrier interpretation note:

- `GraphIntent` is the active bridge carrier across much of the current runtime
- this hub should not be read as freezing the top-level architecture around a permanently universal `GraphIntent`
- Register-layer routing and authority rules remain valid even if the carrier surface later evolves into `AppCommand` / planner / transaction layers

## Implementation Roadmap (Register-Local)

### SR1: Normalize Register Hub Boundaries (near-term)

Goals:
- Keep `RegistryRuntime` as the composition root and `ControlPanel` as the async coordinator (avoid collapsing terms prematurely)
- Remove doc/code mismatches that imply a concrete `SignalBus` implementation where none exists
- Make signal/event routing a named internal layer owned by the Register (even if still direct/transitional)

Done gates:
- [x] Hub docs and terminology consistently describe `SignalBus` as planned / equivalent abstraction (`register_layer_spec.md`, `signal_bus_spec.md`, `TERMINOLOGY.md`)
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
- [x] `Signal` type families and source metadata contract documented (`runtime/registries/signal_routing.rs`)
- [x] Register-owned routing API skeleton exists (facade/trait/module)
- [x] At least one producer + two observers integrated through the routing contract (navigation producer + observer integration tests)

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
- [x] Register signal bus/fabric implementation replaces transitional direct routing in selected paths (navigation path)
- [x] Diagnostics channels report signal routing health (`register.signal_routing.*`)
- [x] Mod-triggered cross-registry workflow path uses signal routing instead of direct wiring (mod lifecycle route via `phase3_route_mod_lifecycle_event`)
- [x] Subsystem health propagation path uses signal routing (or equivalent observer fabric) (`phase3_propagate_subsystem_health_memory_pressure`)

### SR4: Register Runtime Authoritative Dispatch Cleanup

Goals:
- Complete migration away from remaining legacy desktop dispatch paths
- Make Register-owned provider wiring and event routing the canonical runtime path
- Keep `ControlPanel` focused on worker orchestration, not cross-registry policy dispatch

Done gates:
- [x] Legacy dispatch callsites removed or wrapped behind Register APIs (phase0/phase2/phase3 runtime dispatch wrappers)
- [x] Legacy dispatch callsites removed or wrapped behind Register APIs (phase0 navigation/provider-wired runtime dispatch slice; see `#82`)
- [x] Internal settings workbench routes (`verso://settings/*` canonical, legacy `graphshell://settings/*` alias) are pane-authority and no longer reducer-panel owned
- [x] `RegistryRuntime` + signal routing layer responsibilities are documented and tested (SR2/SR3 routing tests + runtime dispatch tests)
- [x] `ControlPanel` API surface reflects coordinator/process-host role only (queue internals private; coordination exposed via spawn/drain/shutdown and command/lifecycle policy APIs)

---

## Architecture

### The Two-Layer Model

```
┌─────────────────────────────────────────────────────────┐
│                   Frame Loop (sync)                     │
│  drain intent_rx → sort by causality → apply_reducer_intents()  │
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
│  p2p_sync_worker         (in progress)                 │
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
    /// Intent queue internals remain private; frame loop uses drain API.
    intent_tx: mpsc::Sender<QueuedIntent>,
    intent_rx: mpsc::Receiver<QueuedIntent>,
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
    all_intents.extend(control_panel.drain_pending());

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
- `shell/desktop/mod.rs` — expose `control_panel` module

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

**Status:** ✅ Complete (2026-03-05)

Goals:
- Periodically emit `PromoteNodeToActive { cause: SelectedPrewarm }` based on graph heuristics
- Subscribes to `watch::Receiver<LifecyclePolicy>` for memory budget awareness
- Uses exponential backoff on channel congestion

Done gates:
- [x] `prefetch_scheduler_worker` implemented and spawned (supervised worker)
- [x] `LifecyclePolicy` watch channel wired to scheduler (GUI frame updates policy each tick)

**Implementation Notes (2026-03-05):**
- Scheduler emits `GraphIntent::PromoteNodeToActive { cause: SelectedPrewarm }` for the current prewarm target instead of placeholder `Noop` intents.
- CP3 policy now carries selected-node prewarm target + memory pressure level; warning/critical pressure slows or disables prefetch cadence.
- Congestion policy remains exponential backoff bounded by `PREFETCH_MAX_INTERVAL`.

### Phase CP4: P2P Device Sync

**Status:** In progress (worker scaffold wired; reducer sync semantics pending) — see [`system/2026-03-05_cp4_p2p_sync_plan.md`](../2026-03-05_cp4_p2p_sync_plan.md)

Goals:

- P2P device-sync worker consumes peer deltas and queues remote-sync carrier intents with version vector stamps
- Network failures emit explicit offline signaling (target `MarkPeerOffline`; never silent)
- Causality ordering via version vectors ensures convergence across all peers

Done gates:

- [ ] Peer discovery and rendezvous design complete (covered in Verso Tier 1 plan §2–3)
- [x] `p2p_sync_worker` implemented and supervised under `ControlPanel`
- [ ] Remote-sync reducer carrier semantics completed (runtime `ApplyRemoteLogEntries` alias and/or CP4 target `ApplyRemoteDelta` naming)
- [ ] Explicit peer-offline reducer path completed (`MarkPeerOffline` target behavior or equivalent status intent)
- [ ] Version vector persistence wired into workspace state (note: "Lamport clock" in prior wording — version vectors are the correct mechanism per Verso sync plan §4.3; see CP4 plan §5.3)

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
            // CP4 runtime scaffold: remote batches arrive through a carrier intent.
            // Full CP4 convergence will derive ordering from version-vector metadata.
            GraphIntent::ApplyRemoteLogEntries { .. } => (0, self.source),
            // Local intents have implicit clock 0 (applied first)
            _ => (0, self.source),
        }
    }
}
```

Local UI intents (clock 0) always apply before async producer intents, preserving responsiveness. CP4 target convergence uses version-vector causality; runtime scaffold currently preserves worker batch order.

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
| **CP3** | ✅ Done | Prefetch scheduler + `LifecyclePolicy` watch + selected-prewarm promotion intents |
| **CP4** | In progress | P2P device-sync worker scaffold landed; reducer sync semantics and persistence follow-up |

---

## References

- [2026-02-20_embedder_decomposition_plan.md](../../aspect_render/2026-02-20_embedder_decomposition_plan.md) — Stage 5 overview
- [2026-02-21_lifecycle_intent_model.md](2026-02-21_lifecycle_intent_model.md) — Intent schema and lifecycle state machine
- [2026-02-22_registry_layer_plan.md](2026-02-22_registry_layer_plan.md) — The Register architecture and provider wiring
- [PLANNING_REGISTER.md](PLANNING_REGISTER.md) — sequencing and backlog for Register/runtime follow-ups
- Crates: `tokio`, `tokio-util` (CancellationToken), `tokio::task::JoinSet`

## Registry Spec Index

### Atomic Registries
- [protocol_registry_spec.md](protocol_registry_spec.md)
- [index_registry_spec.md](index_registry_spec.md)
- [viewer_registry_spec.md](viewer_registry_spec.md)
- [layout_registry_spec.md](layout_registry_spec.md)
- [theme_registry_spec.md](theme_registry_spec.md)
- [physics_profile_registry_spec.md](physics_profile_registry_spec.md)
- [action_registry_spec.md](action_registry_spec.md)
- [identity_registry_spec.md](identity_registry_spec.md)
- [mod_registry_spec.md](mod_registry_spec.md)
- [agent_registry_spec.md](agent_registry_spec.md)
- [diagnostics_registry_spec.md](diagnostics_registry_spec.md)
- [knowledge_registry_spec.md](knowledge_registry_spec.md)

### Surface and Domain Registries
- [input_registry_spec.md](input_registry_spec.md)
- [canvas_registry_spec.md](canvas_registry_spec.md)
- [workbench_surface_registry_spec.md](workbench_surface_registry_spec.md)
- [viewer_surface_registry_spec.md](viewer_surface_registry_spec.md)
- [layout_domain_registry_spec.md](layout_domain_registry_spec.md)
- [presentation_domain_registry_spec.md](presentation_domain_registry_spec.md)
- [lens_compositor_spec.md](lens_compositor_spec.md)
- [workflow_registry_spec.md](workflow_registry_spec.md)
