# Interaction & Semantic Design Schemes (2026-02-24)

**Status**: Research / Synthesis
**Context**: Leveraging the Registry Layer Architecture (`2026-02-22_registry_layer_plan.md`) to define high-level UX patterns.
**See Also**: `2026-02-22_multi_graph_pane_plan.md` (pane-hosted multi-view plan; graph-pane Canonical/Divergent semantics)

## Executive Summary

Graphshell's architecture has evolved from a monolithic application to a registry-driven ecosystem. This enables a shift in design philosophy: from "configuring a tool" to "applying a Lens". This document outlines the core and extended design schemes that leverage this architecture to provide semantic meaning through interaction and physics.

---

## 1. The Lens as the Atomic Unit of Experience

**Concept**: Users should not manage disparate settings (layout, theme, physics) individually. They should adopt **Lenses**. A Lens is a named configuration that defines the "Physics of Logic" for a viewport.

**Architecture Leverage**: `LensCompositor` (Coordinator) + `CanvasRegistry` (Topology/Layout) + `PhysicsProfileRegistry` (Motion) + `ThemeRegistry` (Visuals).

### Core Schemes
*   **The "File Explorer" Lens**:
    *   **Topology**: `topology:dag` (Strict hierarchy, no cycles).
    *   **Layout**: `layout:tree` (Indented list or node tree).
    *   **Physics**: `physics:solid` (Rigid, directional gravity).
    *   **Theme**: High contrast, structural lines.
*   **The "Brainstorm" Lens**:
    *   **Topology**: `topology:free` (Undirected, cycles allowed).
    *   **Layout**: `layout:force_directed` (Organic).
    *   **Physics**: `physics:liquid` (Clustering, languid motion).
    *   **Theme**: Dark/Soft, focus on content.

### Extended Schemes
*   **Progressive Lenses**: Automatically switching Lenses based on zoom level (LOD).
    *   *Zoom Out*: Switch to `physics:gas` (High repulsion) for overview/heatmap.
    *   *Zoom In*: Switch to `physics:liquid` (Clustering) for context.

---

## 2. Physics as Semantic Feedback ("States of Matter")

**Concept**: Motion conveys meaning. The way nodes move tells the user about their relationships and the nature of the data. Physics is not just for preventing overlap; it is a semantic channel.

**Architecture Leverage**: `PhysicsProfileRegistry` + `CanvasRegistry` (Interaction Policy).

### The Three States
1.  **Liquid (Organic/Clustering)**
    *   **Behavior**: Nodes move languidly, "pooling" together based on shared attributes.
    *   **Semantic Meaning**: "These things are related."
    *   **Mechanism**: Semantic attraction forces (`apply_semantic_clustering_forces` in `render/mod.rs`).
    *   **Use Case**: Knowledge graph, research, finding hidden connections.

2.  **Gas (Volumetric/Expansive)**
    *   **Behavior**: Nodes repel strongly, filling available space. High entropy.
    *   **Semantic Meaning**: "This is the scope of the dataset."
    *   **Mechanism**: High repulsion, zero gravity, high damping.
    *   **Use Case**: Search results, unconnected datasets, overview mode.

3.  **Solid (Structural/Rigid)**
    *   **Behavior**: Nodes lock into place relative to neighbors or a grid.
    *   **Semantic Meaning**: "This structure is authoritative."
    *   **Mechanism**: Stiff springs, directional gravity, grid snapping.
    *   **Use Case**: File system, timeline, organizational charts.

---

## 3. Semantic Layouts (Canonical vs. Divergent)

**Concept**: The same graph can be projected into different spatial arrangements without mutating the underlying data.

**Architecture Leverage**: `GraphViewState` (in `app.rs`) + `LocalSimulation`.

*   **Canonical View**: The "True" position of nodes, shared across the workspace. Driven by global physics or manual positioning.
*   **Divergent View**: A temporary or specialized projection.
    *   *Timeline View*: Project nodes onto an X-axis based on `created_at`.
    *   *Kanban View*: Project nodes into buckets based on `status` tag.
    *   *Map View*: Project nodes onto lat/long if geospatial data exists.

**UX Implication**: Users can "pivot" data instantly by opening a new Pane with a Divergent Lens, while keeping the Canonical view open for context.

---

## 4. Contextual Control & Navigation

**Concept**: Controls should appear *at the point of attention*, populated by the *context of the target*.

**Architecture Leverage**: `ActionRegistry` + `InputRegistry` + `KnowledgeRegistry`.

### Interaction Patterns

*   **Command Palette (Contextual and Global)**:
    *   The canonical name for the configurable action list. Two scope modes, one surface. *Contextual* scope: triggered by right-click / gamepad `A` on a focused node; filtered to actions relevant to the target via `ActionRegistry::list_actions_for_context`. *Global* scope: triggered by `Ctrl+K` / `F2` / gamepad `Start`; full searchable registry with context-relevant actions ranked first.
    *   Content: `ActionRegistry::list_actions_for_context(context)`, grouped by `ActionCategory`. Disabled actions shown greyed with tooltip — not hidden.
    *   *Example (contextual)*: Right-clicking a "PDF Node" with the PDF Mod shows "Extract Text".
    *   *Example (global)*: `Ctrl+K` → "Toggle Physics", "Switch to Dark Theme".
    *   **See**: `2026-02-24_control_ui_ux_plan.md` for full layout spec.
*   **Radial Menu (Directional)**:
    *   Trigger: Default in Gamepad mode (D-pad / stick navigation). Available in Mouse/KB mode via hotkey. 8-sector layout, one action per sector, labels outside the ring.
    *   Content: `ActionRegistry::list_actions_for_context(context)` — contextual, not hardcoded.
    *   *Example*: Focusing a node and pressing the gamepad confirm button opens the radial menu with node actions. A "PDF Node" with the PDF Mod loaded adds "Extract Text" to overflow.
    *   **See**: `2026-02-24_control_ui_ux_plan.md` for full layout spec and gamepad wiring.
*   **Semantic Navigation**:
    *   Navigation is graph traversal. "Back" follows `History` edges.
    *   "Focus" drives expansion. Selecting a node pulls related nodes (via `physics:liquid` attraction) into view.

---

## 5. Implementation Notes

This document is a UX design reference, not an implementation roadmap. Implementation is
covered by `2026-02-22_registry_layer_plan.md` (registries), `2026-02-22_multi_graph_pane_plan.md`
(pane-hosted multi-view dispatch + Canonical/Divergent graph views), and
`2026-02-24_physics_engine_extensibility_plan.md` (physics
presets and ExtraForce). This section notes gaps and ordering constraints.

**Registry status**: `ActionRegistry`, `InputRegistry`, `LensCompositor`, `PhysicsProfileRegistry`,
and `CanvasRegistry` are all defined in `2026-02-22_registry_layer_plan.md`. They are not
new concepts introduced here. The radial menu and command palette (§4) emit `ActionId`s through
`ActionRegistry::execute` — no new dispatch mechanism is needed.

**Sequencing constraints**:

1. Lens Resolution (`LensCompositor` fallback chain: Workspace → User → Default) — blocked
   on Phase 6.2 callsite migration completing. Do not begin physics-preset UI work until
   `LensCompositor.resolve_lens()` is the active code path.
2. Physics Tuning (Liquid/Gas/Solid parameter refinement) — can proceed in parallel with
   Phase 6.2. Target: perceptually distinct behaviors at default zoom (not just parameter
   differences). Coordinate with `PhysicsProfile` in `app.rs`.
3. Semantic Forces (`apply_semantic_clustering_forces` expansion beyond UDC to tag overlap,
   link density) — blocked on Level 2 ExtraForce implementation
   (`2026-02-24_physics_engine_extensibility_plan.md §Level 2`). The function referenced
   here may be aspirational; verify its existence before expanding it.
4. Visual Feedback (theme reinforcing physics states) — blocked on `ThemeRegistry` Phase 4
   being active. Low priority relative to physics correctness.

**Progressive Lenses (§1) — resolved**: trigger semantics are specified in
`2026-02-25_progressive_lens_and_physics_binding_plan.md`. Switching is threshold-based
(not continuous interpolation), governed by a `ProgressiveLensAutoSwitch` preference
(`Always / Ask / Never`, default `Ask`) with a ±10 % hysteresis band at each breakpoint.
The `Lens-physics binding preference` (`LensPhysicsBindingPreference`) is also formally
specified there and chains after the progressive-switch gate. Do not implement progressive
Lens switching or physics binding before the prerequisites in §4 of that plan are met.

**Divergent view types (§3)**: Timeline, Kanban, and Map projections are now tracked as
layout algorithm requirements in `2026-02-24_physics_engine_extensibility_plan.md §Layout
Algorithm Reference Table` and as Divergent use cases in `2026-02-22_multi_graph_pane_plan.md`.
