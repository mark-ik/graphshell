# Workbench Surface Registry Spec

**Doc role:** Canonical registry spec for `workbench_surface_registry`.
**Status:** Active / canonical
**Kind:** Surface registry
**Related docs:**
- [../2026-02-22_registry_layer_plan.md](../2026-02-22_registry_layer_plan.md) (registry ecosystem and ownership model)
- [SYSTEM_REGISTER.md](SYSTEM_REGISTER.md) (register hub and routing boundary)

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
- more explicit promotion and overlay policies

## Prospective Capabilities

- alternative workbench shells beyond the current tile tree
- non-rectilinear or scene-based workbench layouts

## Acceptance Criteria

- Registration, lookup, and fallback behavior are covered by registry contract tests.
- At least one harness or scenario path exercises the registry through real app behavior.
- Diagnostics channels emitted by this registry follow the Register diagnostics contract.
- Registry ownership remains explicit and does not collapse into widget-local or ad hoc fallback logic.
