# Viewer Surface Registry Spec

**Doc role:** Canonical registry spec for `viewer_surface_registry`.
**Status:** Active / canonical
**Kind:** Surface registry
**Related docs:**
- [../2026-02-22_registry_layer_plan.md](../2026-02-22_registry_layer_plan.md) (registry ecosystem and ownership model)
- [SYSTEM_REGISTER.md](SYSTEM_REGISTER.md) (register hub and routing boundary)

## Purpose and Scope

Defines viewport behavior and document-surface presentation policy for viewer panes.

In scope:
- viewer viewport behavior
- reader mode, zoom, and scroll policy
- viewer-surface capability metadata

Out of scope:
- viewer implementation selection
- tile-tree placement
- graph semantics

## Canonical Model

This registry is a named capability surface within the Register. It should expose stable lookup and capability contracts, keep failures explicit, and avoid hidden fallback semantics.

Canonical interfaces:
- `resolve_viewport_policy(id)`
- `describe_surface(id) -> ViewerSurfaceCapability`
- `validate_constraints(id, backend) -> Result<()>`

## Normative Core

- ViewerSurfaceRegistry governs how a selected viewer presents, not which viewer gets selected.
- Backend constraints are explicit and diagnosable.
- Viewport behavior remains separate from workbench layout and graph policy.

## Planned Extensions

- backend conformance profiles
- reader-mode variants and document navigation policies

## Prospective Capabilities

- cross-view synchronized viewport modes
- multi-surface linked reading

## Acceptance Criteria

- Registration, lookup, and fallback behavior are covered by registry contract tests.
- At least one harness or scenario path exercises the registry through real app behavior.
- Diagnostics channels emitted by this registry follow the Register diagnostics contract.
- Registry ownership remains explicit and does not collapse into widget-local or ad hoc fallback logic.
