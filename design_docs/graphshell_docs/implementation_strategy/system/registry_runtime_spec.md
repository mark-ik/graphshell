# RegistryRuntime Spec (2026-02-28)

**Doc role:** Canonical spec for `RegistryRuntime` as the Register layer's composition root.
**Status:** Active / canonical
**Short label:** `registry_runtime`
**Related docs:**
- [register_layer_spec.md](./register_layer_spec.md) (Register layer parent spec)
- [2026-02-22_registry_layer_plan.md](./2026-02-22_registry_layer_plan.md) (registry ecosystem and provider wiring)
- [register/SYSTEM_REGISTER.md](./register/SYSTEM_REGISTER.md) (register-layer hub/index)

**Policy authority**: This file is the canonical policy authority for `RegistryRuntime` behavior and boundaries.
Policy in this file should be distilled from canonical specs and accepted research conclusions.

**Adopted standards** (see [2026-03-04_standards_alignment_report.md](../../research/2026-03-04_standards_alignment_report.md) §3.7):
- **OSGi R8** — composition root model; capability availability, provider-routed dispatch, and service lifecycle follow OSGi service registry vocabulary

## RegistryRuntime Policies

1. **Composition-root policy**: `RegistryRuntime` is the canonical runtime composition root for registry capabilities.
2. **No-ui-semantics policy**: `RegistryRuntime` may expose provider-routed paths but must not own UI/product-surface semantics.
3. **Boundary-clarity policy**: `RegistryRuntime` does not assume `ControlPanel` worker supervision responsibilities.
4. **Capability-visibility policy**: Runtime capability availability must be discoverable via explicit descriptors and diagnostics.
5. **Provider-wiring policy**: Legacy direct dispatch paths should converge into explicit provider-routed runtime paths.

## Purpose and Scope

`RegistryRuntime` is the system-owned runtime composition root for the Register layer.

In scope:

- instantiating and wiring registries
- hosting runtime services that belong to capability composition
- exposing canonical provider-routed dispatch paths
- supervising Register-owned infrastructure relationships

Out of scope:

- async worker scheduling policy
- feature-specific business logic
- direct ownership of graph or workbench mutations

## Canonical Model

`RegistryRuntime` is responsible for assembling:

- atomic registries
- surface/domain registries
- mod/runtime services
- routing facades used by the application shell

It is the place where capability availability becomes concrete at runtime.

## Normative Core

- `RegistryRuntime` owns composition, not UI semantics.
- It should expose explicit provider-routed paths instead of leaking direct legacy dispatch.
- It should be the canonical place to understand what capabilities exist at runtime.
- It should not absorb `ControlPanel`'s worker/process-host role.

## Planned Extensions

- stronger typed composition diagnostics
- clearer runtime capability introspection and health surfaces
- tighter register-layer API boundaries for future renderer/runtime changes

## Prospective Capabilities

- multi-profile runtime assemblies
- hot capability graph inspection

## Acceptance Criteria

- Register-owned dispatch paths are routed through `RegistryRuntime` or its explicit delegates.
- Capability availability can be described without spelunking unrelated subsystems.
- Composition and supervision responsibilities stay distinct from `ControlPanel`.
