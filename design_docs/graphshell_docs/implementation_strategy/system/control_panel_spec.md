# Control Panel Spec (2026-02-28)

**Doc role:** Canonical spec for `ControlPanel` as the Register layer's async coordination surface.
**Status:** Active / canonical
**Short label:** `control_panel`
**Related docs:**
- [register_layer_spec.md](./register_layer_spec.md) (Register layer parent spec)
- [2026-02-21_lifecycle_intent_model.md](./2026-02-21_lifecycle_intent_model.md) (intent model and reducer boundary)
- [register/SYSTEM_REGISTER.md](./register/SYSTEM_REGISTER.md) (register-layer hub/index)

**Policy authority**: This file is the canonical policy authority for `ControlPanel` behavior and boundaries.
Policy in this file should be distilled from canonical specs and accepted research conclusions.

## ControlPanel Policies

1. **Supervised-worker policy**: Background producers that emit intents/signals must run under supervised lifecycle management.
2. **Sync-core policy**: `ControlPanel` is an async adapter; it must not turn reducer authority into async mutation logic.
3. **Ingress-contract policy**: Background work enters core state only through explicit queues/contracts, never side-channel mutation.
4. **Failure-visibility policy**: Worker failure, cancellation, and backpressure must be explicit and diagnosable.
5. **Role-separation policy**: `ControlPanel` coordinates work around registries; it does not become a composition root.

## Purpose and Scope

`ControlPanel` is the async coordination and process-host surface that allows background producers to feed the deterministic app core without compromising testability.

In scope:

- worker supervision
- cancellation and lifecycle management for background producers
- intent ingress from async/background tasks
- bounded, explicit routing from background work into the synchronous core

Out of scope:

- registry composition
- signal semantics definition
- direct ownership of graph or workbench authority

## Canonical Model

`ControlPanel` is a system adapter around a synchronous core:

- async workers run here
- the reducer does not
- background tasks communicate through explicit queueing and routing contracts

It is a process host, not a hidden second reducer.

## Normative Core

- All background tasks that produce intents should be supervised here or through an equivalent explicit host.
- The reducer remains synchronous and testable.
- Background failures must degrade explicitly through diagnostics or structured intents.
- `ControlPanel` does not own registries; it coordinates work around them.
- Short-lived host requests initiated from Shell/UI surfaces still count as
  background work and should use the same supervision boundary rather than raw
  detached threads.
- When a background task needs to update UI-visible Shell state directly, it
  should return through an explicit mailbox/result channel that Shell drains at
  frame boundaries.

## Planned Extensions

- richer worker classes and resource-budget policies — see
  [`2026-03-17_runtime_task_budget.md`](./2026-03-17_runtime_task_budget.md)
  for the pre-design policy note (worker tiers, concurrency envelope,
  suspension/resume semantics, diagnostics channels)
- stronger lifecycle supervision tooling
- clearer interaction with typed signal routing

## Prospective Capabilities

- dynamic worker profiles by workflow
- stronger system health integration

## Acceptance Criteria

- Background producers are not silently bypassing the supervised async boundary.
- Control-panel responsibilities remain distinct from `RegistryRuntime` composition.
- Failure, shutdown, and backpressure behavior are explicit and testable.
