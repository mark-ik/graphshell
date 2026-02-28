# Control Panel Spec (2026-02-28)

**Doc role:** Canonical spec for `ControlPanel` as the Register layer's async coordination surface.
**Status:** Active / canonical
**Short label:** `control_panel`
**Related docs:**
- [register_layer_spec.md](./register_layer_spec.md) (Register layer parent spec)
- [2026-02-21_lifecycle_intent_model.md](./2026-02-21_lifecycle_intent_model.md) (intent model and reducer boundary)
- [register/SYSTEM_REGISTER.md](./register/SYSTEM_REGISTER.md) (register-layer hub/index)

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

## Planned Extensions

- richer worker classes and resource-budget policies
- stronger lifecycle supervision tooling
- clearer interaction with typed signal routing

## Prospective Capabilities

- dynamic worker profiles by workflow
- stronger system health integration

## Acceptance Criteria

- Background producers are not silently bypassing the supervised async boundary.
- Control-panel responsibilities remain distinct from `RegistryRuntime` composition.
- Failure, shutdown, and backpressure behavior are explicit and testable.

