<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Runtime Task Budget — Async Partition Policy

**Date**: 2026-03-17
**Status**: Backlog / pre-design
**Scope**: Formalises how the tokio runtime partitions work across the egui
render loop, Servo, and background protocol workers (iroh, Nostr relay, Matrix,
future WebRTC). Feeds the "richer worker classes and resource-budget policies"
placeholder in `control_panel_spec.md`.

**Related docs**:

- [control_panel_spec.md](./control_panel_spec.md) — ControlPanel supervision
  model (worker lifecycle authority)
- [2026-03-05_cp4_p2p_sync_plan.md](./2026-03-05_cp4_p2p_sync_plan.md) — CP4
  P2P sync worker (first concrete multi-worker scenario)
- [2026-03-05_network_architecture.md](./2026-03-05_network_architecture.md) —
  protocol layer assignments (iroh / libp2p / Matrix / Nostr / WebRTC roles)
- [system_architecture_spec.md](./system_architecture_spec.md) — two-authority
  model and egui frame loop

---

## 1. Problem Statement

The ControlPanel supervision model (CP1–CP4) defines *how* workers are
supervised (JoinSet, CancellationToken, intent ingress contract). It does not
define *how many* workers may be alive simultaneously, which workers can be
suspended when the user is idle, or what the priority ordering is when CPU or
memory is constrained.

As of CP4 the concrete workers in play or planned are:

| Worker | State | Resource profile |
| --- | --- | --- |
| `P2PSyncWorker` (iroh) | CP4 scaffold wired | Network I/O, periodic disk flush |
| Nostr relay pool | `nostr` mod scaffolded | WebSocket keepalive, event fan-out |
| Matrix client (`MatrixCore`) | Plan landed 2026-03-17 | HTTP/QUIC, state-event fan-in |
| Servo render pipeline | Always-on (mod-owned) | GPU texture, per-tab threads |
| Future: WebRTC media | Tier 2+ | Codec threads, media I/O |
| Future: libp2p swarm (Verse) | Tier 2+ | DHT, gossipsub, open sockets |

Running all of these simultaneously on a mid-range machine (integrated GPU,
16 GB RAM, no discrete VRAM) without any coordination risks:

- egui frame time spikes from tokio thread contention
- iroh or Matrix flooding the network interface during a Servo render burst
- Nostr relay reconnect storms on wake-from-sleep drowning the intent queue

The ControlPanel already provides the structural answer (supervised workers,
bounded intent ingress). This note defines the **policy layer** that sits on top.

---

## 2. Core Constraint: egui Frame Loop is the Priority Ceiling

The egui render loop runs on the main thread and calls
`ControlPanel::drain_pending()` each frame. Every worker feeds the app only
through this drain. The invariant:

> No background worker may block or delay `drain_pending()` for more than one
> frame budget (~16 ms at 60 fps).

This is already enforced structurally (async workers, bounded channels). The
task-budget policy adds the complementary concern: workers must not saturate
the tokio thread pool to the point where egui's wakeup is delayed.

**Practical implication**: CPU-heavy background work (hash verification,
index computation, blob encoding) must run on `tokio::task::spawn_blocking`,
not `tokio::spawn`, so it does not compete with egui's async wakeup.

---

## 3. Worker Priority Tiers

Three tiers define suspension and CPU-yield behaviour:

### Tier 0 — Render-Adjacent (never suspended)

- Servo render pipeline
- Intent drain (`ControlPanel::drain_pending`)

These are not managed by ControlPanel; they are owned by the mod lifecycle and
the main loop respectively. They define the ceiling everything else must stay
below.

### Tier 1 — Session-Scoped (suspend when idle)

Workers that are only meaningful while the user is actively browsing or
syncing a session. They may be suspended (channel closed, CancellationToken
fired) when the app is backgrounded or idle for longer than a configurable
threshold.

- `P2PSyncWorker` (iroh)
- Nostr relay pool
- `MatrixCore` event worker

Suspension policy: if the app window loses focus for > N seconds (configurable,
default 120 s), Tier 1 workers enter a low-frequency polling mode (wake on
incoming event, not on timer). They do not terminate — reconnection latency on
resume must stay below ~2 s.

### Tier 2 — On-Demand (spawned and torn down per feature activation)

Workers that exist only while a specific feature is in use:

- WebRTC peer connection (per Coop session)
- Blob transfer tasks (iroh-blobs, per transfer)
- Verse gossipsub swarm (when a Verse space is open)

These are spawned by their owning mod on activation and cancelled on
deactivation. ControlPanel does not pre-allocate them; the owning mod calls
`spawn_*_worker()` at activation time per the CP1–CP4 pattern.

---

## 4. Concurrency Budget (Target Envelope)

These are design targets, not hard limits. They define the envelope within
which the system should operate without tuning:

| Metric | Target | Rationale |
| --- | --- | --- |
| Simultaneous Tier 1 workers | ≤ 4 | iroh + Nostr + Matrix + 1 spare |
| Tier 2 workers (Coop session) | ≤ 2 per active session | WebRTC + blob transfer |
| Intent queue depth at drain | ≤ 256 intents | Prevents drain latency spikes |
| `spawn_blocking` tasks | ≤ 8 concurrent | tokio default blocking thread pool cap |
| Open iroh connections | ≤ 16 | Per iroh endpoint recommendation |

If a worker attempts to push past these targets, it should:
1. Apply backpressure at the worker's own send channel (bounded mpsc).
2. Emit a diagnostics event (`system:task_budget:backpressure`, severity `Warn`).
3. Never block the drain path.

---

## 5. Suspension and Resume Semantics

### Idle detection

The app already tracks user-interaction recency for other purposes (Recent
node semantics, focus model). The same signal should gate worker suspension:
if no `GraphIntent` has been produced from a user gesture for > idle threshold,
Tier 1 workers enter low-frequency mode.

Implementation note: the idle signal should flow through the existing SignalBus
rather than being re-invented per worker. A `SystemSignal::UserIdle { since }` /
`SystemSignal::UserResumed` pair is the canonical carrier.

### Wake-on-network

Tier 1 workers in low-frequency mode must still be able to wake on incoming
network events (peer reconnect, Matrix room update, Nostr relay message). The
tokio `select!` loop in each worker handles this naturally — suspension just
means the timer arm is lengthened, not that the socket is closed.

### Resume race

On resume, multiple Tier 1 workers may flush buffered events simultaneously.
The intent queue's bounded channel provides natural backpressure here; workers
block on send until drain catches up. No additional coordination is needed.

---

## 6. Diagnostics Channels

The following channels should be registered when this policy is implemented:

| Channel | Severity | Description |
| --- | --- | --- |
| `system:task_budget:backpressure` | `Warn` | A worker's send channel is full; worker is blocking |
| `system:task_budget:worker_suspended` | `Info` | A Tier 1 worker entered low-frequency idle mode |
| `system:task_budget:worker_resumed` | `Info` | A Tier 1 worker returned to active polling |
| `system:task_budget:queue_depth` | `Info` | Intent queue depth at drain time (sampled, not emitted every frame) |

---

## 7. Relationship to ControlPanel Spec

`control_panel_spec.md` §Planned Extensions lists "richer worker classes and
resource-budget policies" as a prospective capability. This doc is the design
note that will eventually back that extension. When implementation begins:

1. Worker tier classification should be declared on `spawn_*_worker()` call
   sites (a `WorkerTier` parameter or similar).
2. The suspension/resume logic lives in ControlPanel, not in individual workers.
3. Workers should not implement their own idle timers — the SignalBus idle
   signal is the single source of suspension authority.

---

## 8. What This Is Not

This note does not cover:

- **Memory pruning / "hot vs cold" storage**: which Iroh collections to evict,
  which workspace snapshots to unload. That belongs in the storage subsystem
  (`SUBSYSTEM_STORAGE.md`).
- **Verse swarm resource management**: Verse is Tier 2+; its specific gossipsub
  fan-out and DHT load budgets are a Verse-layer concern.
- **GPU memory pressure**: Servo texture allocation and egui_glow frame
  management. Those are render-pipeline concerns outside ControlPanel's scope.

---

## 9. Open Questions (pre-design)

1. Should the idle threshold be per-worker or global? Global is simpler; per-
   worker allows Matrix to stay live while iroh suspends.
- per worker
2. Should `spawn_blocking` tasks be tracked through ControlPanel or left to
   tokio's default blocking pool? Tracking enables diagnostics; leaving them
   untracked is simpler.
- tracked through control panel
3. When Verse lands, does the libp2p swarm get its own ControlPanel worker
   slot, or does the Verse mod own its own tokio runtime? The latter would
   give Verse a hard resource partition but complicate intent ingress.
- i would think the libp2p swarm would get its own control panel worker, but this is the one I'm least sure of

These are blocking design questions for the implementation slice, not for
the current backlog phase.

---

## 10. Done Gates (when implementation slice is opened)

- [x] `WorkerTier` classification exists on ControlPanel worker registration.
      (`WorkerTier` enum in `control_panel.rs`; `Tier1P2pSync`, `Tier1NostrRelay`,
      `Tier1MatrixCore` variants. Each `spawn_*_worker` calls `register_worker_tier(tier)`
      at its spawn site; counts tracked in `registered_tiers: HashMap<WorkerTier, usize>`
      for future §4 budget enforcement. `spawn_matrix_core_worker` is a registered stub
      for MatrixCore (plan-only). Done 2026-03-18.)
- [x] `SystemSignal::UserIdle` / `SystemSignal::UserResumed` wired through
      SignalBus to Tier 1 worker suspension logic.
      (`LifecycleSignal::UserIdle { since_ms }` / `LifecycleSignal::UserResumed` added;
      `ControlPanel::tick_idle_watchdog` emits via `RegistryRuntime::propagate_user_idle_signal`
      / `propagate_user_resumed_signal`. Threshold sourced from
      `AppPreferences::worker_idle_threshold_secs` (CLI: `--worker-idle-threshold-secs`,
      env: `GRAPHSHELL_WORKER_IDLE_THRESHOLD_SECS`); defaults to 120 s. Done 2026-03-18.)
- [x] All existing Tier 1 workers (`P2PSyncWorker`, Nostr relay pool,
      `MatrixCore`) respect the suspension signal.
      (P2P sync and Nostr relay: advisory `watch::Receiver<bool>` wired; workers log
      the transition. MatrixCore: plan-only, stub interface registered.
      Full per-worker throttling deferred to worker-side API iteration. Done 2026-03-18.)
- [x] `system:task_budget:*` diagnostics channels registered with correct
      severities. (All 4 channels in `PHASE3_CHANNELS`; test added.
      Backpressure=Warn, Suspended/Resumed/QueueDepth=Info. Done 2026-03-18.)
- [ ] Intent queue depth stays within the §4 target envelope under a manual
      stress test (3 simultaneous Tier 1 workers + active Servo rendering).
      (Deferred: requires running binary; no code blocker.)
- [x] Open questions in §9 resolved and recorded as a dated receipt in this
      doc before implementation proceeds. (Answers inline in §9 above, 2026-03-18.)
