# Mod Registry Spec

**Doc role:** Canonical registry spec for `mod_registry`.
**Status:** Active / canonical
**Kind:** Atomic registry
**Related docs:**
- [../2026-02-22_registry_layer_plan.md](../2026-02-22_registry_layer_plan.md) (registry ecosystem and ownership model)
- [SYSTEM_REGISTER.md](SYSTEM_REGISTER.md) (register hub and routing boundary)

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
- Core seeds keep the app functional with no mods loaded.

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
