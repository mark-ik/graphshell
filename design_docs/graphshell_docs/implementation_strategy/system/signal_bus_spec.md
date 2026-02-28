# Signal Bus Spec (2026-02-28)

**Doc role:** Canonical spec for `SignalBus` (or equivalent) as the Register layer's cross-registry event fabric.
**Status:** Active / canonical (planned abstraction; current direct routing remains transitional)
**Short label:** `signal_bus`
**Related docs:**
- [register_layer_spec.md](./register_layer_spec.md) (Register layer parent spec)
- [register/SYSTEM_REGISTER.md](./register/SYSTEM_REGISTER.md) (register-layer hub/index)
- [2026-02-22_registry_layer_plan.md](./2026-02-22_registry_layer_plan.md) (registry ecosystem and capability model)

## Purpose and Scope

`SignalBus` is the typed event-routing fabric for decoupled cross-registry and cross-subsystem notifications.

In scope:

- typed signals and envelopes
- publish/subscribe or equivalent observer-routing contracts
- source metadata, causality, and routing health
- explicit decoupling between emitters and observers

Out of scope:

- direct graph reducer mutations
- tile-tree mutation ownership
- ad hoc business logic hidden in event listeners

## Canonical Model

Signals are not intents:

- **Intents** request authoritative state mutation
- **Signals** notify observers without transferring ownership of mutation authority

The signal fabric belongs to the Register layer because it exists to decouple capabilities and system observers.

## Normative Core

- Signals must be typed and attributable to a source.
- Emitters should not need to know concrete observers.
- Signal handling must not become a hidden substitute for explicit mutation authorities.
- Routing failures, drops, and backpressure must be diagnosable.

## Planned Extensions

- concrete routing facade/API replacing transitional direct fanout
- queue depth, latency, and drop diagnostics
- stronger causality metadata for cross-registry workflows

## Prospective Capabilities

- scoped subscription classes
- priority-aware routing policies
- remote/distributed signal relays for future peer workflows

## Acceptance Criteria

- Signal semantics are distinguishable from direct calls and intents.
- At least one producer and multiple observers can be described through the same typed routing contract.
- Routing health is observable through diagnostics.

