# Register Layer Spec (2026-02-28)

**Doc role:** Canonical spec for the Register layer as a system component.
**Status:** Active / canonical
**Short label:** `register_layer`
**Related docs:**
- [system_architecture_spec.md](./system_architecture_spec.md) (top-level system decomposition)
- [2026-02-22_registry_layer_plan.md](./2026-02-22_registry_layer_plan.md) (registry ecosystem and capability model)
- [register/SYSTEM_REGISTER.md](./register/SYSTEM_REGISTER.md) (register-layer hub/index)
- [registry_runtime_spec.md](./registry_runtime_spec.md) (runtime composition root)
- [control_panel_spec.md](./control_panel_spec.md) (async worker/process host)
- [signal_bus_spec.md](./signal_bus_spec.md) (signal/event routing fabric)

## Purpose and Scope

The Register layer is the system-owned capability composition and routing layer.

It exists to:

- host registry composition
- expose stable capability contracts
- coordinate cross-registry wiring
- supervise async/background ingress into deterministic app state
- separate extensible capability infrastructure from product-surface semantics

In scope:

- registry composition and capability ownership
- runtime wiring boundaries between registries, mods, and system coordinators
- async ingress boundaries around the reducer
- cross-registry event and signal routing

Out of scope:

- subsystem-specific UX contracts
- individual registry semantics
- product-surface layout or viewer behavior

## Canonical Model

The Register layer is composed of four primary parts:

1. **RegistryRuntime**
- the composition root for registries and runtime services

2. **ControlPanel**
- the async worker/process host that produces intents or signals safely

3. **SignalBus**
- the typed routing fabric for cross-registry and cross-subsystem event distribution

4. **Registry surfaces**
- the atomic, surface, and domain registries defined in `system/register/`

The Register is not the reducer, not the workbench, and not the graph authority. It is the infrastructure layer that makes those systems composable and extensible.

## Normative Core

- The Register layer owns capability composition, not product semantics.
- Background work must cross into app state through explicit routing contracts.
- Registries do not silently call into one another through hidden coupling.
- Register-owned failures should surface explicitly through diagnostics and typed errors.
- The Register layer must remain subordinate to top-level system architecture, not a substitute for it.

## Planned Extensions

- stronger typed signal routing in place of transitional direct fanout
- richer mod capability negotiation and diagnostics
- clearer workflow/session composition over existing registry outputs

## Prospective Capabilities

- distributed capability providers
- remote or signed capability catalogs
- more explicit multi-runtime composition boundaries

## Acceptance Criteria

- Register ownership is distinguishable from graph, workbench, viewer, and command ownership.
- Registry composition and async ingress are documented as separate concerns.
- Each major Register component has its own canonical spec.
- Registry specs in `system/register/` can be read as children of this layer without ambiguity.

