# Canvas Registry Spec

**Doc role:** Canonical registry spec for `canvas_registry`.
**Status:** Active / canonical
**Kind:** Surface registry
**Related docs:**
- [../2026-02-22_registry_layer_plan.md](../2026-02-22_registry_layer_plan.md) (registry ecosystem and ownership model)
- [SYSTEM_REGISTER.md](SYSTEM_REGISTER.md) (register hub and routing boundary)

**Policy authority**: This file is the canonical policy authority for `canvas_registry` semantics and boundaries.
Policy in this file should be distilled from canonical specs and accepted research conclusions.

## Registry Policies

1. **Canvas-territory policy**: Canvas registry owns graph-surface topology/layout/interaction policy, not unrelated subsystem semantics.
2. **Deterministic-layout policy**: Layout and interaction resolution must be reproducible under equivalent state/inputs.
3. **Physics-execution policy**: Physics execution occurs in canvas territory using selected profile parameters from presentation policies.
4. **No-hidden-authority policy**: Canvas callbacks must not bypass graph/workbench mutation authorities.

## Purpose and Scope

Defines graph-surface topology, layout, and interaction/rendering policy.

In scope:
- graph topology policy sets
- graph layout algorithm selection
- graph interaction/rendering policy and physics execution boundaries

Out of scope:
- tile-tree layout
- viewer MIME selection
- cross-app command semantics

## Canonical Model

This registry is a named capability surface within the Register. It should expose stable lookup and capability contracts, keep failures explicit, and avoid hidden fallback semantics.

Canonical interfaces:
- `resolve_topology(id)`
- `resolve_layout(id)`
- `resolve_interaction_policy(id)`

## Normative Core

- CanvasRegistry is the graph-domain surface authority.
- Topology, layout, and interaction/rendering are distinct sections and must not be conflated.
- Graph camera and graph interaction policy are Graphshell-owned semantics.

## Planned Extensions

- graph policy extraction if the surface grows too large
- camera fit-strength/lock defaults integration via explicit camera-policy contracts (not physics presets)

## Prospective Capabilities

- custom canvas backends and advanced graph view modes
- domain-specific graph surface packs

## Acceptance Criteria

- Registration, lookup, and fallback behavior are covered by registry contract tests.
- At least one harness or scenario path exercises the registry through real app behavior.
- Diagnostics channels emitted by this registry follow the Register diagnostics contract.
- Registry ownership remains explicit and does not collapse into widget-local or ad hoc fallback logic.
