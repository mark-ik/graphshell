<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Sector H — Signal Infrastructure Development Plan

**Doc role:** Implementation plan for the signal routing and SignalBus infrastructure
**Status:** Active / planning
**Date:** 2026-03-08
**Parent:** [2026-03-08_registry_development_plan.md](2026-03-08_registry_development_plan.md)
**Registries / infrastructure covered:** `SignalRoutingLayer` → `SignalBus`
**Specs:** `system/signal_bus_spec.md` (primary), `SYSTEM_REGISTER.md` (routing policy)
**Lanes:** `lane:runtime-followon` (#91) — SR2/SR3 signal routing

---

## Purpose

Sector H completes the signal infrastructure that all other sectors depend on for decoupled
cross-registry coordination. The `SignalRoutingLayer` exists as a functional skeleton (SR2/SR3
done gates met) but is narrow: three signal topics, no async observers, no dead-letter policy,
no input or registry-event topics.

The SR3 → SR4 target is a `SignalBus`-class abstraction that replaces remaining direct
inter-registry wiring with typed publish/subscribe. Sector H does not need to complete before
other sectors begin — the existing skeleton is sufficient for basic cross-registry signals —
but the `SignalBus` upgrade must complete before cross-registry wiring in Sectors D and E
can be considered clean.

```
Current state:     SignalRoutingLayer (direct fanout, 3 topics, sync observers)
SR4 target:        SignalBus (typed, async observers, backpressure, dead-letter, full topic set)
```

---

## Current State

`SignalRoutingLayer` in `shell/desktop/runtime/registries/signal_routing.rs`:
- 3 topics: `Navigation`, `Lifecycle`, `Sync`.
- ~10 signal kind variants across the 3 topics.
- Sync `Box<dyn Fn>` observers per topic.
- Diagnostics tracking: published, routed, unrouted, failed counts.
- Tests cover multi-observer notification and failure scenarios.

**Gaps:**
- No `Input` or `RegistryEvent` topic.
- No async observer path (agents and `ControlPanel` workers need async notification).
- Failed signals are counted but silently dropped — no dead-letter visibility.
- No topic-level backpressure.
- Misrouting (signal with no observers) is counted but not warned.

---

## Phase H1 — Expand signal topic set

**Unlocks:** Agent signal subscription (Sector G G3.2); registry state change notifications.

### H1.1 — Add `RegistryEvent` topic

Registry state changes (lens update, theme switch, workflow activation, identity rotation, mod
load/unload) need to propagate to observers without direct inter-registry calls. This is the
`SignalBus`'s primary use case.

```rust
pub enum SignalKind {
    // Existing
    Navigation(NavigationSignal),
    Lifecycle(LifecycleSignal),
    Sync(SyncSignal),

    // New
    RegistryEvent(RegistryEventSignal),
    InputEvent(InputEventSignal),
}

pub enum RegistryEventSignal {
    ThemeChanged { new_theme_id: ThemeId },
    LensChanged { new_lens_id: LensId },
    WorkflowChanged { new_workflow_id: WorkflowId },
    PhysicsProfileChanged { new_profile_id: PhysicsProfileId },
    SemanticIndexUpdated,
    ModLoaded { mod_id: ModId },
    ModUnloaded { mod_id: ModId },
    AgentSpawned { agent_id: AgentId },
    IdentityRotated { identity_id: IdentityId },
}

pub enum InputEventSignal {
    ContextChanged { new_context: InputContext },
    BindingRemapped { action_id: ActionId },
}
```

**Done gates:**
- [ ] `RegistryEvent` and `InputEvent` topic variants added.
- [ ] `RegistryEventSignal` variants cover all state-changing registry operations above.
- [ ] All registry operations in Sectors A–G that change observable state emit the appropriate signal.
- [ ] Unit tests: each new signal kind published and received by an observer.

### H1.2 — `LifecycleSignal` additions

Extend the existing `Lifecycle` topic with signals needed by Sector F:

```rust
pub enum LifecycleSignal {
    // Existing
    NodeActivated { node_key: NodeKey },
    NodeDeactivated { node_key: NodeKey },
    WorkspaceRestored,

    // New
    SemanticIndexUpdated,   // from KnowledgeRegistry::reconcile_semantics()
    MimeResolved { node_key: NodeKey, mime: String },  // from Sector A probe
    WorkflowActivated { workflow_id: WorkflowId },
}
```

**Done gates:**
- [ ] `SemanticIndexUpdated` and `MimeResolved` variants added.
- [ ] `KnowledgeRegistry::reconcile_semantics()` emits `SemanticIndexUpdated`.
- [ ] Sector A MIME probe emits `MimeResolved`.

---

## Phase H2 — Dead-letter visibility and misroute warnings

**Implements SYSTEM_REGISTER misroute-visibility policy and signal bus spec backpressure requirements.**

### H2.1 — Warn on zero-observer signals

A signal published with no registered observers is currently silently counted as "unrouted".
This masks misroutes during development.

```rust
pub fn publish(&self, envelope: SignalEnvelope) {
    let observer_count = self.observer_count_for(envelope.kind.topic());
    if observer_count == 0 {
        log::warn!(
            "signal_routing: signal {:?} has no observers (source: {:?})",
            envelope.kind,
            envelope.source
        );
        self.diagnostics.unrouted += 1;
        return;
    }
    // ...
}
```

**Done gates:**
- [ ] `log::warn!` on zero-observer publish.
- [ ] `unrouted` counter increments on zero-observer.
- [ ] Test: publish with no observers logs warning and increments unrouted count.

### H2.2 — Observer error visibility

Observer call failures (panics, lock poisoning) currently increment `failed` counter silently.
Surfaces as `log::error!` with observer identity:

```rust
if let Err(e) = observer(envelope.clone()) {
    log::error!("signal_routing: observer {:?} failed on {:?}: {:?}", observer_id, kind, e);
    self.diagnostics.failed += 1;
}
```

**Done gates:**
- [ ] Observer errors logged with observer identity.
- [ ] `DIAG_SIGNAL_ROUTING` channel emits at `Error` severity on observer failure.

### H2.3 — Diagnostic channel for signal routing health

Register `DIAG_SIGNAL_ROUTING_*` channels with versioned payload schema (Sector F F1.2
dependency):

```
register.signal_routing.published   — Info
register.signal_routing.unrouted    — Warn
register.signal_routing.failed      — Error
register.signal_routing.queue_depth — Info (future: async queue)
```

**Done gates:**
- [ ] All 4 signal routing diagnostic channels registered.
- [ ] `SignalRoutingDiagnostics` fields emit through `DiagnosticsRegistry` at each tick.
- [ ] SR3 done gate (diagnostics channels report signal routing health) confirmed complete.

---

## Phase H3 — Async observer path

**Unlocks:** Agent signal subscription (Sector G G3.2); `ControlPanel` worker signal reception.

The current observer model uses synchronous `Box<dyn Fn>` callbacks. Agents and workers need
to receive signals without blocking the frame loop. The solution is a `tokio::broadcast` channel
per topic, alongside the existing sync observer map.

```rust
pub struct SignalRoutingLayer {
    // Sync path (existing — for in-frame-loop observers)
    sync_observers: HashMap<SignalTopic, Vec<SyncObserver>>,

    // Async path (new — for workers and agents)
    broadcast_tx: HashMap<SignalTopic, broadcast::Sender<SignalEnvelope>>,
}

impl SignalRoutingLayer {
    /// Subscribe to async signals for a topic. Returns a Receiver clone.
    pub fn subscribe_async(&self, topic: SignalTopic)
        -> broadcast::Receiver<SignalEnvelope>;

    /// Subscribe to all topics (for agents that need full signal stream).
    pub fn subscribe_all(&self) -> broadcast::Receiver<SignalEnvelope>;
}
```

`broadcast::Sender` has a fixed capacity; lagging receivers (slow agents) are dropped with
a `DIAG_SIGNAL_ROUTING` warn emission — this is the backpressure policy.

**Done gates:**
- [ ] `broadcast_tx` map added to `SignalRoutingLayer` with one channel per topic.
- [ ] `subscribe_async()` and `subscribe_all()` implemented.
- [ ] `publish()` sends to both sync observers and broadcast channel.
- [ ] Lagging receiver detection: `broadcast::SendError::Lagged` emits `Warn` diagnostic.
- [ ] Sector G G3.2 `AgentContext::signal_rx` wired from `subscribe_all()`.
- [ ] Test: async subscriber receives signal published from sync path.

---

## Phase H4 — SignalBus abstraction (SR4)

**Unlocks:** SR4 done gates; complete replacement of remaining direct inter-registry wiring.

The `signal_bus_spec.md` defines the `SignalBus` as the Register-owned publish/subscribe
fabric. The `SignalRoutingLayer` is the SR2/SR3 transitional implementation; `SignalBus` is
the stabilised SR4 API.

The key distinction: `SignalRoutingLayer` is a concrete struct with internal direct fanout.
`SignalBus` is a typed API facade — callers interact with it through trait methods, allowing
the internal implementation to evolve (e.g. move to an async message broker) without changing
callsites.

### H4.1 — Define `SignalBus` trait facade

```rust
pub trait SignalBus: Send + Sync {
    fn publish(&self, envelope: SignalEnvelope);
    fn subscribe_sync(&self, topic: SignalTopic, observer: SyncObserver) -> ObserverId;
    fn unsubscribe(&self, id: ObserverId);
    fn subscribe_async(&self, topic: SignalTopic) -> broadcast::Receiver<SignalEnvelope>;
    fn subscribe_all(&self) -> broadcast::Receiver<SignalEnvelope>;
    fn diagnostics(&self) -> SignalRoutingDiagnostics;
}
```

`SignalRoutingLayer` implements `SignalBus`. `RegistryRuntime` holds `Arc<dyn SignalBus>`.

**Done gates:**
- [ ] `SignalBus` trait defined.
- [ ] `SignalRoutingLayer` implements `SignalBus`.
- [ ] `RegistryRuntime` field changes from `SignalRoutingLayer` to `Arc<dyn SignalBus>`.
- [ ] All callsites updated to use `SignalBus` trait methods.

### H4.2 — Audit and remove remaining direct inter-registry wiring

Scan for direct registry-to-registry calls that should route through the `SignalBus`. Each
one that bypasses the bus is an SR4 violation. Replace with signal publication + observer
subscription.

Known candidates after Sectors A–G:
- `LensRegistry` reactivity to `KnowledgeRegistry` updates (Sector A A4.3 / Sector F F2.4).
- `PresentationDomainRegistry` reactivity to `ThemeRegistry` changes (Sector D D4.2).
- `WorkflowRegistry` cross-profile application (Sector E E2.2).

**Done gates:**
- [ ] Audit complete: all cross-registry coordination routes through `SignalBus`.
- [ ] No direct `Arc<OtherRegistry>` field references in any `XxxRegistry` struct (except `LayoutDomainRegistry` which is explicitly a coordinator by spec).
- [ ] SR4 done gate: legacy dispatch callsites removed or wrapped behind Register APIs.

---

## Acceptance Criteria (Sector H complete)

- [ ] `RegistryEvent` and `InputEvent` topics exist with full variant sets.
- [ ] All registry state changes emit the appropriate `RegistryEventSignal`.
- [ ] Zero-observer publish emits `log::warn!`; observer failures emit `log::error!`.
- [ ] `DIAG_SIGNAL_ROUTING_*` channels registered and emitting with correct severity.
- [ ] Async subscriber path exists; agents receive signals via `broadcast::Receiver`.
- [ ] `SignalBus` trait defined; `SignalRoutingLayer` implements it; `RegistryRuntime` uses `Arc<dyn SignalBus>`.
- [ ] No direct inter-registry wiring outside `LayoutDomainRegistry` coordinator role.
- [ ] SR2/SR3/SR4 done gates all confirmed complete.

---

## Related Documents

- `system/signal_bus_spec.md` — canonical `SignalBus` component spec
- [SYSTEM_REGISTER.md](SYSTEM_REGISTER.md) — SR1/SR2/SR3/SR4 roadmap and done gates
- [2026-03-08_sector_g_mod_agent_plan.md](2026-03-08_sector_g_mod_agent_plan.md) — AgentContext signal subscription
- [2026-03-08_sector_f_knowledge_index_plan.md](2026-03-08_sector_f_knowledge_index_plan.md) — SemanticIndexUpdated signal
- [2026-03-08_registry_development_plan.md](2026-03-08_registry_development_plan.md) — master index
