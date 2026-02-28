# Layout Domain Registry Spec

**Doc role:** Canonical registry spec for `layout_domain_registry`.
**Status:** Active / canonical
**Kind:** Domain registry
**Related docs:**
- [../2026-02-22_registry_layer_plan.md](../2026-02-22_registry_layer_plan.md) (registry ecosystem and ownership model)
- [SYSTEM_REGISTER.md](SYSTEM_REGISTER.md) (register hub and routing boundary)

## Purpose and Scope

Coordinates layout-first resolution across graph, workbench, and viewer surfaces.

In scope:
- domain-level layout sequencing
- coordination of Canvas, WorkbenchSurface, and ViewerSurface registries
- layout-first resolution contracts

Out of scope:
- theme tokens
- physics parameter sets as named presets
- action dispatch

## Canonical Model

This registry is a named capability surface within the Register. It should expose stable lookup and capability contracts, keep failures explicit, and avoid hidden fallback semantics.

Canonical interfaces:
- `resolve_layout_domain(context) -> LayoutDomainResolution`
- `describe_domain(id) -> LayoutDomainCapability`

## Normative Core

- Layout domain resolves structure and interaction before presentation styling.
- The graph surface and workbench surface are separate sovereign territories under one coordinating domain.
- Domain resolution is explicit and compositional.

## Planned Extensions

- richer cross-surface compatibility validation
- layout-domain profiles for named environments

## Prospective Capabilities

- dynamic domain reconfiguration by workflow
- cross-surface simulation constraints

## Acceptance Criteria

- Registration, lookup, and fallback behavior are covered by registry contract tests.
- At least one harness or scenario path exercises the registry through real app behavior.
- Diagnostics channels emitted by this registry follow the Register diagnostics contract.
- Registry ownership remains explicit and does not collapse into widget-local or ad hoc fallback logic.
