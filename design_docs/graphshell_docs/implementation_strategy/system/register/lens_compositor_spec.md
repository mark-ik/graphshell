# Lens Compositor Spec

**Doc role:** Canonical registry spec for `lens_compositor`.
**Status:** Active / canonical
**Kind:** Cross-domain compositor
**Related docs:**
- [../2026-02-22_registry_layer_plan.md](../2026-02-22_registry_layer_plan.md) (registry ecosystem and ownership model)
- [SYSTEM_REGISTER.md](SYSTEM_REGISTER.md) (register hub and routing boundary)

**Policy authority**: This file is the canonical policy authority for `lens_compositor` semantics and boundaries.
Policy in this file should be distilled from canonical specs and accepted research conclusions.

## Model boundary (inherits UX Contract Register §3B)

- `GraphId` = truth boundary.
- `GraphViewId` = scoped view state.
- file tree = graph-backed hierarchical projection.
- workbench = arrangement boundary.

Lens composition is GraphView-scoped and must not assume workbench arrangement ownership.

## Contract template (inherits UX Contract Register §2A)

Normative lens contracts use: intent, trigger, preconditions, semantic result, focus result, visual result, degradation result, owner, verification.

## Terminology lock (inherits UX Contract Register §3C)

- Tile/frame arrangement is not content hierarchy.
- File tree is not content truth authority.
- Physics presets are not camera modes.

## Registry Policies

1. **Compositional-lens policy**: Lens composition combines existing domain outputs and does not replace subsystem ownership boundaries.
2. **Layout-then-presentation policy**: Lens resolution preserves canonical sequencing (layout before presentation).
3. **Scope-boundary policy**: Lens scope remains graph-view configuration and excludes workbench/session authority.
4. **Fallback-clarity policy**: Lens resolution/fallback behavior is explicit, diagnosable, and test-backed.

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
