# Workbench Surface Registry Spec

**Doc role:** Canonical registry spec for `workbench_surface_registry`.
**Status:** Active / canonical
**Kind:** Surface registry
**Related docs:**
- [../2026-02-22_registry_layer_plan.md](../2026-02-22_registry_layer_plan.md) (registry ecosystem and ownership model)
- [SYSTEM_REGISTER.md](SYSTEM_REGISTER.md) (register hub and routing boundary)

**Policy authority**: This file is the canonical policy authority for `workbench_surface_registry` semantics and boundaries.
Policy in this file should be distilled from canonical specs and accepted research conclusions.

## Model boundary (inherits UX Contract Register §3B)

- `GraphId` = truth boundary.
- `GraphViewId` = scoped view state.
- `Navigator` = graph-backed hierarchical projection over relation families. Legacy alias: "file tree".
- workbench = arrangement boundary.

This registry owns arrangement policy and must not redefine graph truth or Navigator semantic ownership.

## Contract template (inherits UX Contract Register §2A)

Normative workbench-surface contracts use: intent, trigger, preconditions, semantic result, focus result, visual result, degradation result, owner, verification.

## Terminology lock (inherits UX Contract Register §3C)

- Tile/frame arrangement is not content hierarchy.
- Navigator is not content truth authority.
- Physics presets are not camera modes.

## Registry Policies

1. **Tile-tree-authority policy**: Workbench surface registry owns pane/tile structural interaction policy.
2. **Graph-separation policy**: Workbench surface policy must not redefine graph semantic ownership.
3. **Locking-constraint policy**: Split/reorder/lock constraints are explicit contracts, not implicit framework defaults.
4. **Focus-handoff policy**: Surface-level transitions preserve deterministic focus and return-path rules.

## Purpose and Scope

Defines tile-tree layout policy and workbench interaction policy for pane hosts.

In scope:
- split/tab/container policy
- drag/drop and resize rules
- tile-tree simplification and arrangement contracts

Out of scope:
- graph semantics
- viewer MIME routing
- content ontology

## Canonical Model

This registry is a named capability surface within the Register. It should expose stable lookup and capability contracts, keep failures explicit, and avoid hidden fallback semantics.

Canonical interfaces:
- `resolve_layout_policy(id)`
- `resolve_interaction_policy(id)`
- `describe_surface(id) -> WorkbenchSurfaceCapability`

## Normative Core

- Workbench owns arrangement truth.
- Pane host behavior is distinct from pane payload behavior.
- Tile-tree mutations are workbench authority, not graph reducer authority.

## Planned Extensions

- richer container profile variants
- more explicit pane-structure and overlay policies

## Prospective Capabilities

- alternative workbench shells beyond the current tile tree
- non-rectilinear or scene-based workbench layouts

## Acceptance Criteria

- Registration, lookup, and fallback behavior are covered by registry contract tests.
- At least one harness or scenario path exercises the registry through real app behavior.
- Diagnostics channels emitted by this registry follow the Register diagnostics contract.
- Registry ownership remains explicit and does not collapse into widget-local or ad hoc fallback logic.
