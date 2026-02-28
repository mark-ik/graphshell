# Presentation Domain Registry Spec

**Doc role:** Canonical registry spec for `presentation_domain_registry`.
**Status:** Active / canonical
**Kind:** Domain registry
**Related docs:**
- [../2026-02-22_registry_layer_plan.md](../2026-02-22_registry_layer_plan.md) (registry ecosystem and ownership model)
- [SYSTEM_REGISTER.md](SYSTEM_REGISTER.md) (register hub and routing boundary)

## Purpose and Scope

Coordinates appearance and motion semantics after layout has resolved.

In scope:
- presentation sequencing after layout
- theme and physics-profile coordination
- presentation capability metadata

Out of scope:
- graph topology
- tile-tree arrangement
- command routing

## Canonical Model

This registry is a named capability surface within the Register. It should expose stable lookup and capability contracts, keep failures explicit, and avoid hidden fallback semantics.

Canonical interfaces:
- `resolve_presentation(context) -> PresentationResolution`
- `describe_domain(id) -> PresentationCapability`

## Normative Core

- Presentation is applied after layout, never before.
- Theme and motion semantics are coordinated but still separable.
- Cross-domain presets like Liquid/Gas/Solid are resolved through explicit policy, not hidden defaults.

## Planned Extensions

- preset bundles spanning theme + physics + camera policy hooks
- presentation diagnostics and preview contracts

## Prospective Capabilities

- adaptive presentation based on content density or workflow
- per-pane presentation override stacks

## Acceptance Criteria

- Registration, lookup, and fallback behavior are covered by registry contract tests.
- At least one harness or scenario path exercises the registry through real app behavior.
- Diagnostics channels emitted by this registry follow the Register diagnostics contract.
- Registry ownership remains explicit and does not collapse into widget-local or ad hoc fallback logic.
