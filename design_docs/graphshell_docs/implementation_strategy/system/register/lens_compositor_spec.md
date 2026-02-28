# Lens Compositor Spec

**Doc role:** Canonical registry spec for `lens_compositor`.
**Status:** Active / canonical
**Kind:** Cross-domain compositor
**Related docs:**
- [../2026-02-22_registry_layer_plan.md](../2026-02-22_registry_layer_plan.md) (registry ecosystem and ownership model)
- [SYSTEM_REGISTER.md](SYSTEM_REGISTER.md) (register hub and routing boundary)

## Purpose and Scope

Composes named lenses from graph-surface, presentation, and knowledge/filter configuration.

In scope:
- lens composition and naming
- layout-first then presentation sequencing for graph views
- knowledge filter integration into graph view configuration

Out of scope:
- workbench layout profiles
- global session/workflow activation
- direct tile-tree control

## Canonical Model

This registry is a named capability surface within the Register. It should expose stable lookup and capability contracts, keep failures explicit, and avoid hidden fallback semantics.

Canonical interfaces:
- `resolve_lens(id) -> LensProfile`
- `compose(parts) -> LensProfile`
- `describe_lens(id) -> LensCapability`

## Normative Core

- A Lens is a graph view configuration, not a full session mode.
- Lens resolution composes registry outputs; it does not replace them.
- Workbench layout stays outside the Lens boundary.

## Planned Extensions

- lens authoring presets
- stronger validation between graph, presentation, and knowledge filters

## Prospective Capabilities

- shareable mod-defined lenses
- adaptive lens switching

## Acceptance Criteria

- Registration, lookup, and fallback behavior are covered by registry contract tests.
- At least one harness or scenario path exercises the registry through real app behavior.
- Diagnostics channels emitted by this registry follow the Register diagnostics contract.
- Registry ownership remains explicit and does not collapse into widget-local or ad hoc fallback logic.
