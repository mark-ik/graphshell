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

## Normative Core

### System decomposition

Graphshell is composed of these major system responsibilities:

- **Graph subsystem**
  - owns graph/content truth, node and edge semantics, graph interaction policy, graph camera policy

- **Workbench subsystem**
  - owns arrangement truth, tile-tree layout, pane lifecycle, split/tab behavior, focus within workbench layout

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
