# System Architecture Spec (2026-02-28)

**Doc role:** Canonical top-level system spec for Graphshell.
**Status:** Active / canonical
**Short label:** `system_architecture`
**Related docs:**
- [2026-02-20_embedder_decomposition_plan.md](../aspect_render/2026-02-20_embedder_decomposition_plan.md) (embedder/runtime decomposition)
- [2026-02-21_lifecycle_intent_model.md](./2026-02-21_lifecycle_intent_model.md) (intent and reducer boundary)
- [2026-02-22_registry_layer_plan.md](./2026-02-22_registry_layer_plan.md) (registry ecosystem and capability model)
- [2026-02-24_universal_content_model_plan.md](../viewer/2026-02-24_universal_content_model_plan.md) (content model and identity surface)
- [register/SYSTEM_REGISTER.md](./register/SYSTEM_REGISTER.md) (register-layer runtime hub/spec)

**Policy authority**: This file is the canonical policy authority for top-level system construct boundaries and cross-construct ownership.
Policy in this file should be distilled from canonical specs and accepted research conclusions.

**Standards alignment**: The canonical adopted/referenced standard set for all Graphshell subsystems and registries is defined in [2026-03-04_standards_alignment_report.md](../../research/2026-03-04_standards_alignment_report.md). Every subsystem spec must cite the adopted standards that govern its domain. Validating against an adopted standard is the preferred validation target; internal contract tests translate the standard into Graphshell-specific assertions.

Adopted standards with system-wide scope:
- **OSGi R8 Service Registry** (conceptual model) — registry vocabulary, capability lifecycle, `namespace:name` key convention, mod manifest `requires`/`provides`.
- **RFC 3986** — URI syntax for all internal schemes (`verso://`, `notes://`, `graph://`, `node://`).
- **RFC 4122 UUID v4** — node identity; **UUID v7** — WAL/operation tokens only. These namespaces must not be conflated.
- **OpenTelemetry Semantic Conventions** — diagnostic channel naming and severity across all subsystems.

## System Component Policies

1. **Single-owner policy**: Each major behavior class has one subsystem authority; cross-subsystem implementation is allowed, cross-subsystem ownership is not.
2. **Mutation-authority policy**: Graph/model mutation stays in reducer authority; tile-tree/layout mutation stays in workbench authority. Current runtime carriers may still use `GraphIntent` / reducer-intent forms, but the target architecture should converge toward `AppCommand -> AppPlan -> AppTransaction -> AppEffect` without changing the underlying authority split.
3. **Routing policy**: Commands and framework events must route through Graphshell semantic authorities; host/widget layers cannot become policy owners.
4. **Register-boundary policy**: Register infrastructure composes capabilities and routes signals/intents but must not absorb product-surface semantics.
5. **Policy-change rule**: Any change to cross-construct ownership or boundaries must update this file in the same slice as affected child specs.

## Purpose and Scope

This document defines the top-level architecture of Graphshell:

- the major subsystems
- the authority boundaries between them
- the role of the Register layer
- the routing contracts between content, workbench, viewer, command, and system infrastructure

It is the system-level parent document for subsystem specs and register specs.

In scope:

- top-level decomposition of Graphshell into system, subsystem, and registry layers
- canonical ownership rules
- cross-subsystem bridges and mutation boundaries
- the distinction between runtime composition, UX surfaces, and capability registries

Out of scope:

- detailed UX contracts for specific surfaces
- per-registry provider details
- implementation sequencing and roadmap detail beyond architectural boundaries

## Canonical Model

Graphshell is organized in three layers:

1. **System layer**
- cross-cutting architecture, runtime composition, capability ecosystem, and global invariants

2. **Subsystem layer**
- user-facing and feature-facing domains such as Graph, Workbench, Viewer, Command, Focus, Control Surfaces, History, Diagnostics, and Storage

3. **Registry layer**
- named capability surfaces used to make subsystem behavior extensible, composable, and testable

The Register is part of the **system layer**. It is not the whole system. It is the runtime composition and capability-routing layer that hosts registries and coordination infrastructure.

Important interpretation rule:

- this document defines **subsystem and layer authority**
- state-container plans may separately define where concrete state is stored

These are different axes, not competing architectures.

For example:

- Graph subsystem may own graph-camera interaction policy while view-local camera state is stored in workbench-owned state containers.
- Focus subsystem may own focus rules while concrete focus/selection state lives in workbench-owned state containers.
- Runtime/Register layers may own composition and routing policy while runtime handles live in explicit runtime/service containers.

The existing `GraphWorkspace` / `AppServices` split and any future refinement into `DomainState` / `WorkbenchState` / `RuntimeState` should be read as state-container refinements under this same subsystem-ownership model, not as replacements for it.

Current execution note (2026-03-07):

- the first Phase B state-layer CLAT is now landed in code as `GraphWorkspace { domain: DomainState, ... }`
- the durable core moved into `DomainState { graph, notes, next_placeholder_id }`
- the first bounded bridge-reduction follow-on slice is also landed for the workbench consumer family's graph reads
- the temporary `GraphWorkspace -> DomainState` deref bridge remains active while remaining families are migrated

## Normative Core

### System decomposition

Graphshell is composed of these major system responsibilities:

- **Graph subsystem**
  - owns graph/content truth, node and edge semantics, graph interaction policy, and graph-space camera semantics/policy

- **Workbench subsystem**
  - owns arrangement truth, tile-tree layout, pane lifecycle, split/tab behavior, focus within workbench layout, and view-local camera state/preferences

- **Viewer subsystem**
  - owns viewer presentation behavior, fallback/placeholder semantics, embedded/composited viewer policy

- **Command subsystem**
  - owns canonical action invocation semantics across keyboard, palette, radial, context, and omnibar surfaces

- **Focus subsystem**
  - owns cross-surface focus authority, region navigation, and focus return-path rules

- **Control Surfaces subsystem**
  - owns settings, diagnostics panes, history manager surfaces, and other tool/control pages as app-owned surfaces

- **Cross-cutting infrastructure subsystems**
  - accessibility, diagnostics, history, storage, security

- **Register layer**
  - owns registry composition, capability routing, mod integration, async coordination boundaries, and signal/event distribution contracts

### Authority boundaries

The system has explicit mutation authorities:

- **Graph reducer authority**
  - synchronous graph/model mutations and deterministic state transitions
  - current reducer carriers may still be `GraphIntent` / `GraphReducerIntent`
  - target top-level pipeline should evolve toward `AppCommand -> AppPlan -> AppTransaction -> AppEffect`

- **Workbench authority**
  - tile-tree and pane arrangement mutations

- **Register/runtime authority**
  - capability composition, background worker supervision, and cross-registry routing

No subsystem should silently absorb another subsystem's authority.

### Routing boundaries

Canonical routing rules:

- graph semantics do not directly own tile-tree layout
- workbench layout does not own graph truth
- viewers do not choose graph semantics or workbench structure
- command surfaces do not redefine action meaning
- widget/framework code may render and report input, but may not become the semantic authority

### Register as a system component

The Register layer exists to:

- host registry composition
- expose stable capability contracts
- supervise async/background producers outside the reducer
- route cross-registry notifications

The Register is therefore a **system component** and has its own hub/spec, but it is subordinate to this document in scope.

## Planned Extensions

- stronger explicit `SignalBus`-class routing abstraction under the Register
- fuller `WorkflowRegistry`-driven session-mode composition
- deeper policy composition between camera, layout, physics, and presentation
- eventual custom canvas and renderer evolution without changing subsystem ownership rules

## Prospective Capabilities

- alternate renderer stacks and custom canvas backends
- richer workflow and automation orchestration
- broader mod-defined capability packs spanning multiple registries and subsystems
- distributed/peer-native capability providers

## Acceptance Criteria

- Every major feature surface has a clear subsystem owner.
- Every extensible capability surface has a clear registry owner.
- The Register is treated as the capability/runtime composition layer, not as a catch-all substitute for system architecture.
- Subsystem specs and registry specs can be read as children of this system-level decomposition without contradiction.
