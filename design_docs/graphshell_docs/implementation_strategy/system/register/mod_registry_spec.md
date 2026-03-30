# Mod Registry Spec

**Doc role:** Canonical registry spec for `mod_registry`.
**Status:** Active / canonical
**Kind:** Atomic registry
**Related docs:**
- [../2026-02-22_registry_layer_plan.md](../2026-02-22_registry_layer_plan.md) (registry ecosystem and ownership model)
- [SYSTEM_REGISTER.md](SYSTEM_REGISTER.md) (register hub and routing boundary)
- [../../../technical_architecture/2026-03-30_protocol_modularity_and_host_capability_model.md](../../../technical_architecture/2026-03-30_protocol_modularity_and_host_capability_model.md) (protocol packaging classes and host-aware modularity)

**Policy authority**: This file is the canonical policy authority for `mod_registry` semantics and boundaries.
Policy in this file should be distilled from canonical specs and accepted research conclusions.

## Registry Policies

1. **Manifest-gate policy**: Capability and dependency declarations are validated before load/activation.
2. **Activation-order policy**: Dependency resolution and activation ordering are deterministic and diagnosable.
3. **Containment policy**: Denial/quarantine paths are explicit and must prevent silent contract erosion.
4. **Lifecycle-integrity policy**: Load/unload transitions must preserve registry health and observability.

## Purpose and Scope

Manages native and WASM mod lifecycle, dependency resolution, and declared capabilities.

In scope:
- mod load/unload lifecycle
- dependency and capability resolution
- native vs WASM tier boundaries

Out of scope:
- feature semantics inside individual registries
- direct graph/workbench mutations outside declared APIs
- command UX

## Canonical Model

This registry is a named capability surface within the Register. It should expose stable lookup and capability contracts, keep failures explicit, and avoid hidden fallback semantics.

Canonical interfaces:
- `load_mod(path)`
- `unload_mod(id)`
- `resolve_dependencies()`
- `list_mods()`

## Normative Core

- Mods extend registry surfaces; they are not hidden hardcoded subsystems.
- Capability and dependency failures are explicit.
- Core built-ins keep the app functional offline; optional feature mods may be
  absent or denied without breaking the core floor.
- "Modular" means capability-scoped and host-aware. It does not require the
  same binary plugin or mod bundle to load in every host envelope.
- Portable protocol adapters and native feature mods are both valid extension
  units, but only the former are expected to span most hosts.
- Mod absence must degrade explicitly through registry state and diagnostics,
  not through silent fallback or flattening everything into core.

## Planned Extensions

- richer sandbox policy surfaces
- hot-reload diagnostics and capability diffing

## Prospective Capabilities

- signed mod distribution and trust policies
- remote capability catalogs

## Acceptance Criteria

- Registration, lookup, and fallback behavior are covered by registry contract tests.
- At least one harness or scenario path exercises the registry through real app behavior.
- Diagnostics channels emitted by this registry follow the Register diagnostics contract.
- Registry ownership remains explicit and does not collapse into widget-local or ad hoc fallback logic.
