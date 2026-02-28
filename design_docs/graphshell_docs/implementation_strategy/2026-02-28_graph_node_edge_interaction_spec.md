# Graph / Node / Edge Interaction Spec

**Date**: 2026-02-28  
**Status**: Canonical interaction contract (graph-surface baseline)  
**Priority**: Immediate implementation guidance

**Related**:
- `PLANNING_REGISTER.md`
- `2026-02-28_ux_contract_register.md`
- `2026-02-27_ux_baseline_done_definition.md`
- `2026-02-27_workbench_frame_tile_interaction_spec.md`
- `2026-02-28_command_surface_interaction_spec.md`
- `2026-02-28_focus_and_region_navigation_spec.md`
- `2026-02-28_viewer_presentation_and_fallback_spec.md`
- `2026-02-23_graph_interaction_consistency_plan.md`
- `../design/KEYBINDINGS.md`
- `../../TERMINOLOGY.md`

---

## 1. Purpose and Scope

This spec defines the interaction contract for the **graph surface** of Graphshell.

It explains:

- what the graph pane is for,
- what node, edge, and canvas actions mean semantically,
- who owns each behavior,
- what state transitions those interactions imply,
- what visual feedback must accompany them,
- what fallback behavior must happen when the ideal path is unavailable,
- which controls are core, planned, and exploratory.

This is not the workbench arrangement contract.

This spec governs:

- `Graph Pane`
- `Node`
- `Edge`
- `Canvas`
- camera and selection semantics inside the graph pane

For workbench/frame/tile semantics, see:

- `2026-02-27_workbench_frame_tile_interaction_spec.md`

For command invocation, focus ownership, and viewer-state clarity, see:

- `2026-02-28_command_surface_interaction_spec.md`
- `2026-02-28_focus_and_region_navigation_spec.md`
- `2026-02-28_viewer_presentation_and_fallback_spec.md`

---

## 2. Canonical Surface Model

### 2.1 Interactive hierarchy

The graph surface is composed of four interactive layers:

1. **Graph Pane**
   - the active graph viewport and command context
2. **Canvas**
   - the navigable graph space inside the pane
3. **Node**
   - the primary graph entity users inspect, select, move, and open
4. **Edge**
   - the relationship or traversal surface between nodes

### 2.2 What each layer is for

- **Graph Pane**: the semantic navigation and manipulation surface for graph work.
- **Canvas**: the continuous space users pan, zoom, lasso, and inspect.
- **Node**: the primary content entity users act on.
- **Edge**: the relationship surface users inspect and, when defined, traverse.

### 2.3 Ownership model

- Graphshell owns camera policy, selection truth, activation semantics, routing, and graph interaction meaning.
- The framework layer may provide hit surfaces, event capture, and paint execution.
- The framework must not be the semantic owner of:
  - camera state,
  - selection truth,
  - lasso meaning,
  - node activation policy,
  - graph-to-workbench routing.

---

## 3. Canonical Interaction Model

### 3.1 Interaction categories

Graph interactions fall into five semantic categories:

1. **Selection**
   - choose graph targets without committing a structural action
2. **Activation**
   - invoke the primary action of the target
3. **Manipulation**
   - move nodes or the viewport
4. **Inspection**
   - reveal information without changing semantic ownership
5. **Routing**
   - open graph content into workbench structures

### 3.2 Core semantic rules

- Single click selects the target under the pointer.
- Modifier-click adjusts the current selection set.
- Double click activates the target's primary action.
- If a target has no defined activation behavior, double click is a no-op beyond maintaining selection or inspection state.

### 3.3 Canonical guarantees

The graph surface must make these user expectations reliable:

- camera controls affect the active graph pane only,
- selection is explicit and visible,
- node activation routes through Graphshell open policy,
- graph gestures do not silently fight each other,
- blocked or degraded behavior is explicit rather than silent.

---

## 4. Normative Core

This section defines the stable target behavior for the current graph surface.

### 4.1 Camera and Viewport

**What this domain is for**

- Let the user move through graph space predictably and keep the active graph pane under user control.

**Core controls**

- Drag on empty canvas: pan viewport.
- Wheel over graph pane: zoom at pointer.
- `Camera Fit`: fit relevant graph content.
- `Focus Selection`: fit selected nodes only.
- `Zoom Reset`: restore canonical default zoom baseline.

**Who owns it**

- Graphshell camera controller owns camera state, command targeting, and zoom policy.
- The framework may only provide raw pointer and wheel events plus drawing surfaces.

**State transitions**

- Panning changes viewport translation in the active graph pane only.
- Wheel zoom changes viewport scale around the pointer target in the active graph pane only.
- `Camera Fit` updates camera state to show relevant graph bounds.
- `Focus Selection` updates camera state to show selected-node bounds.

**Visual feedback**

- Camera movement must be immediate and legible.
- Zoom must visually anchor around the pointer, not drift arbitrarily.
- Fit operations must visibly land on the intended target set.

**Fallback / degraded behavior**

- If input cannot be claimed, Graphshell must emit diagnostics and preserve existing camera state.
- Silent camera no-op behavior is forbidden.

### 4.2 Node Interaction

**What this domain is for**

- Support selecting, inspecting, manipulating, and opening graph content.

**Core controls**

- Single click node: select node.
- Modifier-click node: add or remove the node from the current selection set.
- Double click node: activate the node's primary open path.
- Drag selected node: move node.
- Drag one selected node while multiple are selected: move the selected group when group-move mode is active.
- Hover node: inspect node metadata and show hover affordance.
- Context action on node: expose node-scoped commands.

**Who owns it**

- Graphshell selection and graph interaction controllers own node selection truth, drag semantics, and activation meaning.
- The framework may only report hits and render hover or selected visuals.

**State transitions**

- Single selection replaces the current selection set with the target node.
- Additive selection mutates the current selection set without changing graph identity.
- Node activation routes through the workbench open policy.
- Node drag changes node position and, when enabled, group position.

**Visual feedback**

- Selected nodes must look selected.
- Hovered nodes must look inspectable.
- Dragging must clearly show that movement is in progress.

**Fallback / degraded behavior**

- If a node cannot be opened, Graphshell must surface an explicit blocked-state or fallback target.
- Failed drag or selection claims must not leave hidden partial state.

### 4.3 Edge Interaction

**What this domain is for**

- Make relationships visible, inspectable, and traversable where defined.

**Core controls**

- Hover edge: inspect relationship or traversal information.
- Single click edge: set the edge as the active inspection target.
- Double click edge: invoke the edge's primary traversal action when that action is defined.

**Who owns it**

- Graphshell graph interaction controller owns edge inspection meaning, traversal policy, and any edge-selection semantics.
- The framework may only report edge hits and paint edge state.

**State transitions**

- Edge hover changes inspection context only.
- Edge single-click changes the active relationship target for inspection.
- Edge double-click may trigger traversal or related open behavior when the edge semantics define a primary action.

**Visual feedback**

- Hovered edges must visibly read as inspectable.
- If an edge is the active inspection target, that state must be legible.
- Traversal activation must visibly change focus or destination context.

**Fallback / degraded behavior**

- If an edge has no primary action, double click is a no-op beyond preserving inspection state.
- Edges must not pretend to be activatable when they are inspection-only in the current context.

### 4.4 Canvas Interaction

**What this domain is for**

- Provide predictable behavior for interacting with graph space rather than specific graph entities.

**Core controls**

- Empty-canvas click: clear selection when no other mode is active.
- Lasso gesture: select nodes by region using the declared lasso binding.
- Empty-canvas context action: expose graph-scope commands.

**Who owns it**

- Graphshell graph interaction controller owns empty-canvas meaning, lasso semantics, and gesture precedence.
- The framework may only provide pointer events and paint transient lasso visuals.

**State transitions**

- Empty-canvas click clears the current selection set.
- Lasso updates the app-owned selection set based on the chosen region and selection mode.
- Context commands do not directly mutate graph truth until a command is executed.

**Gesture precedence**

- Pointer-down on a node enters node interaction first.
- Pointer-down on empty canvas enters pan or lasso based on the declared binding.
- Pan, lasso, and node-drag must not silently claim the same gesture at the same time.

**Visual feedback**

- Lasso region must be visible while active.
- Empty-canvas clear must visibly remove prior selection state.

**Fallback / degraded behavior**

- If the app cannot determine a valid gesture owner, Graphshell must cancel safely and emit diagnostics.
- Ambiguous gesture fights are forbidden.

### 4.5 Graph-to-Workbench Routing

**What this domain is for**

- Bridge graph activation semantics to the workbench without bypassing Graphshell ownership.

**Core controls**

- Node activation uses the workbench spec's open-tile-first behavior.
- Explicit alternate destinations are command-driven, not hidden pointer-only gestures.

**Who owns it**

- Graphshell routing authority owns destination policy, tile reuse rules, and fallback behavior.
- The graph surface may request an open action, but it does not choose layout structure independently.

**State transitions**

- Default node activation requests open or focus for the node in the workbench.
- Existing tiles are focused before new tiles are created.
- Explicit alternate routes may create new tile or frame structure according to workbench rules.

**Visual feedback**

- Activation must visibly land in the destination context.
- Cross-frame jumps must be legible.

**Fallback / degraded behavior**

- Silent duplicate tile creation is forbidden.
- If Graphshell chooses a fallback destination, the fallback must be explicit and deterministic.

### 4.6 Keyboard and Command Semantics

**What this domain is for**

- Ensure graph actions are semantic commands first and bindings second.

**Core controls**

- `Zoom In`
- `Zoom Out`
- `Zoom Reset`
- `Camera Fit`
- `Focus Selection`
- `Delete Selected` where deletion is valid
- `New Node`
- graph-context command palette invocation

**Who owns it**

- Graphshell action registry and command dispatcher own semantic meaning and target resolution.
- The framework may render command surfaces and capture input, but it does not define command meaning.

**State transitions**

- Commands act on the active graph pane and current graph selection context.
- Commands may mutate camera state, selection state, or graph content according to their meaning.

**Visual feedback**

- Command execution must visibly change the graph surface or show a clear result surface.
- Failed commands must show explicit blocked-state feedback.

**Fallback / degraded behavior**

- If a command cannot run in the current context, the failure must be explicit.
- Hidden command no-op behavior is forbidden.

### 4.7 Visual Feedback, Diagnostics, Accessibility, and Performance

**What this domain is for**

- Make graph interactions visible, trustworthy, and usable under normal and degraded conditions.

**Visual feedback**

- Selected nodes must look selected.
- Hovered nodes and edges must look inspectable.
- Focused graph pane state must be legible.
- Lasso region must be visible while active.
- Active camera changes must be visually obvious.

**Diagnostics**

- Blocked states, degraded states, and fallback reasons must be observable.
- Missing or reduced interaction capability must be explicit rather than silent.

**Accessibility**

- Every critical graph action must eventually have a keyboard-accessible equivalent.
- Region focus and return paths must be deterministic.
- If a pointer-first action lacks a keyboard equivalent, it must be called out as incomplete rather than implied.

**Performance**

- Camera motion, node drag, and selection feedback must remain responsive under quick smoke workloads.
- Degradation may reduce fidelity, but it must not hide interaction ownership or current selection truth.

---

## 5. Planned Extensions

These are intended near-term behaviors. They are not yet required for baseline closure, but they are part of the immediate design direction.

### 5.1 Richer node and routing controls

- Explicit "open in split" and "open in frame" actions from node context
- Pin and unpin actions directly from the graph surface
- More explicit group-drag affordances

### 5.2 Richer relationship tooling

- Edge traversal previews
- Edge filtering or highlighting by relationship type
- Better relationship-specific context actions

### 5.3 Better graph navigation support

- Camera bookmarks
- Search-targeting affordances in graph space
- More explicit selection-to-navigation handoffs

---

## 6. Prospective Capabilities

These are exploratory design directions. They are informative only and should not be treated as current implementation requirements.

### 6.1 Alternate spatial modes

- 2.5D projection modes
- isometric views
- perspective or full 3D graph presentation

### 6.2 Richer graph-structure interaction

- Path-selection modes across chained edges
- Semantic subgraph collapse and expand
- Cluster-level operations and interaction modes

### 6.3 Advanced graph workflows

- Multiple layout modes beyond the default graph layout
- Graph-space lenses and semantic overlays
- Command-driven multi-node structural opening workflows

---

## 7. Acceptance Criteria (Spec Parity)

1. Camera semantics are defined for pan, wheel zoom, fit, and focus-selection.
2. Node single-click, modifier-click, double-click, hover, and drag semantics are explicit.
3. Edge hover, inspection, and conditional activation semantics are explicit.
4. Canvas gesture precedence (pan vs lasso vs node drag) is defined and deterministic.
5. Graph-to-workbench routing matches the workbench spec rather than bypassing it.
6. Command semantics are defined as app actions, not only as keybindings.
7. Visual feedback and degraded-state requirements are explicit.
8. Accessibility expectations and current incompleteness rules are explicit.
9. Planned and exploratory controls are separated from the canonical core.
