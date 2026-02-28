# Physics Profile Registry Spec

**Doc role:** Canonical registry spec for `physics_profile_registry`.
**Status:** Active / canonical
**Kind:** Atomic registry
**Related docs:**
- [../2026-02-22_registry_layer_plan.md](../2026-02-22_registry_layer_plan.md) (registry ecosystem and ownership model)
- [SYSTEM_REGISTER.md](SYSTEM_REGISTER.md) (register hub and routing boundary)

## Purpose and Scope

Defines named force/physics parameter presets such as Liquid, Gas, and Solid.

In scope:
- physics preset registration
- named profile lookup
- cross-domain semantic labels over numeric parameters

Out of scope:
- physics engine execution
- layout algorithm selection
- camera command dispatch

## Canonical Model

This registry is a named capability surface within the Register. It should expose stable lookup and capability contracts, keep failures explicit, and avoid hidden fallback semantics.

Canonical interfaces:
- `get_profile(id)`
- `describe_profile(id) -> PhysicsCapability`
- `resolve_preset(mode) -> PhysicsProfile`

## Normative Core

- Profiles are parameters only; execution remains elsewhere.
- Named presets are user-facing semantic contracts.
- Preset lookup must be deterministic and overrideable.

## Planned Extensions

- shared preset mapping across camera/layout/physics domains
- user-configurable preset overrides

## Prospective Capabilities

- adaptive presets driven by workflow or content density
- mod-shipped simulation bundles

## Acceptance Criteria

- Registration, lookup, and fallback behavior are covered by registry contract tests.
- At least one harness or scenario path exercises the registry through real app behavior.
- Diagnostics channels emitted by this registry follow the Register diagnostics contract.
- Registry ownership remains explicit and does not collapse into widget-local or ad hoc fallback logic.
