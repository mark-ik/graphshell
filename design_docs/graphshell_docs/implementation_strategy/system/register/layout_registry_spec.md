# Layout Registry Spec

**Doc role:** Canonical registry spec for `layout_registry`.
**Status:** Active / canonical
**Kind:** Atomic registry
**Related docs:**
- [../2026-02-22_registry_layer_plan.md](../2026-02-22_registry_layer_plan.md) (registry ecosystem and ownership model)
- [SYSTEM_REGISTER.md](SYSTEM_REGISTER.md) (register hub and routing boundary)

## Purpose and Scope

Hosts named graph layout algorithms used by graph/canvas policies.

In scope:
- layout algorithm registration
- layout capability and parameter contracts
- algorithm lookup by `LayoutId`

Out of scope:
- graph topology rules
- physics preset naming
- tile-tree arrangement

## Canonical Model

This registry is a named capability surface within the Register. It should expose stable lookup and capability contracts, keep failures explicit, and avoid hidden fallback semantics.

Canonical interfaces:
- `compute_layout(graph) -> Positions`
- `describe_layout(id) -> LayoutCapability`
- `validate_params(id, params) -> Result<()>`

## Normative Core

- Layout algorithms produce positions only; they do not mutate graph semantics.
- Algorithm lookup is explicit and testable.
- Fallback behavior is diagnosed, not silent.

## Planned Extensions

- more advanced graph algorithms and constrained layouts
- parameter profile presets per algorithm

## Prospective Capabilities

- incremental and streaming layouts
- GPU-backed layout execution

## Acceptance Criteria

- Registration, lookup, and fallback behavior are covered by registry contract tests.
- At least one harness or scenario path exercises the registry through real app behavior.
- Diagnostics channels emitted by this registry follow the Register diagnostics contract.
- Registry ownership remains explicit and does not collapse into widget-local or ad hoc fallback logic.
