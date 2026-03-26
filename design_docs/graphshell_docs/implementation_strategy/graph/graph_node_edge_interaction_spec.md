# Graph / Node / Edge Interaction Spec

**Date**: 2026-02-28  
**Status**: Canonical interaction contract (graph-surface baseline)  
**Priority**: Pre-renderer/WGPU required

**Related**:
- `PLANNING_REGISTER.md`
- `2026-02-28_ux_contract_register.md`
- `../subsystem_ux_semantics/2026-03-01_ux_execution_control_plane.md`
- `../workbench/workbench_frame_tile_interaction_spec.md`
- `GRAPH.md`
- `../workbench/WORKBENCH.md`
- `../aspect_command/command_surface_interaction_spec.md`
- `../subsystem_focus/focus_and_region_navigation_spec.md`
- `../viewer/viewer_presentation_and_fallback_spec.md`
- `../subsystem_ux_semantics/2026-03-04_model_boundary_control_matrix.md`
- `2026-02-23_graph_interaction_consistency_plan.md`
- `../design/KEYBINDINGS.md`
- `../../TERMINOLOGY.md`
- `2026-03-14_graph_relation_families.md` — relation family vocabulary; Navigator replaces "file tree" as the hierarchical projection surface
- `2026-03-14_edge_visual_encoding_spec.md` — per-family visual encoding and interaction affordances
- `2026-03-14_edge_operability_matrix.md` — per-family operability contract

**Adopted standards** (see [standards report](../../research/2026-03-04_standards_alignment_report.md) §§3.3, 3.5, 3.6):

- **WCAG 2.2 Level AA** — SC 2.5.8 (minimum target size for nodes/edges/affordances), SC 2.4.3 (focus order), SC 2.4.11 (focus appearance)
- **Fruchterman-Reingold 1991** — physics preset semantics (`Liquid` / `Gas` / `Solid`) documented against this model
- **OpenTelemetry Semantic Conventions** — diagnostics channels for blocked/degraded graph states

## Model boundary (inherits UX Contract Register §3B)

- `GraphId` = truth boundary.
- `GraphViewId` = scoped view state.
- **Navigator** = the section-structured projection of graph relations rendered
  through one or more Navigator hosts (replaces "file tree" — see
  `2026-03-14_graph_relation_families.md §5`).
- workbench = arrangement boundary.

This spec defines graph-surface semantics and must not redefine workbench arrangement ownership.

## Contract template (inherits UX Contract Register §2A)

Normative graph contracts use: intent, trigger, preconditions, semantic result, focus result, visual result, degradation result, owner, verification.

## Terminology lock (inherits UX Contract Register §3C)

- Tile/frame arrangement is not content hierarchy.
- Navigator, regardless of host edge or form factor, is not content truth
  authority — it is a read-only projection over graph relation families.
- Physics presets are not camera modes.
- "File tree" is a legacy alias — use **Navigator** in new code and docs.

---

## 1. Purpose and Scope

This spec defines the interaction contract for the **graph surface** of Graphshell.

Within the architecture, this surface is the user-facing interaction layer of the
**Graph subsystem**.

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
- **Navigator** (hierarchical projection over graph relation families rendered
  through Navigator hosts) — see `2026-03-14_graph_relation_families.md §5`
- camera and selection semantics inside the graph pane
- the user-facing contract of the graph structure subsystem

For workbench/frame/tile semantics, see:

- `../workbench/workbench_frame_tile_interaction_spec.md`

For command invocation, focus ownership, and viewer-state clarity, see:

- `../aspect_command/command_surface_interaction_spec.md`
- `../subsystem_focus/focus_and_region_navigation_spec.md`
- `../viewer/viewer_presentation_and_fallback_spec.md`

---

## 2. Canonical Surface Model

### 2.1 Interactive hierarchy

The graph surface is composed of four primary interactive layers:

1. **Graph Pane**
   - the active graph viewport and command context
2. **Canvas**
   - the navigable graph space inside the pane
3. **Node**
   - the primary graph entity users inspect, select, move, and open
4. **Edge**
   - the relationship or traversal surface between nodes

When the **Navigator** is active in any host, it is a section-structured
hierarchical projection over graph relation families — not a separate
content-truth authority. It reads graph edges and renders them as a tree; it
does not own graph identity or topology.

Selectable graph-surface objects are the interactable objects rendered on the
canvas: nodes, edges, arrangement objects (for example frames or minimized tile
objects), and minimap markers when they expose direct actions. Purely
informational hover-only UI (labels, passive badges, transient tooltips) is not
selectable.

### 2.2 What each layer is for

- **Graph Pane**: the semantic navigation and manipulation surface for graph work.
- **Canvas**: the continuous space users pan, zoom, lasso, and inspect.
- **Node**: the primary content entity users act on.
- **Edge**: the relationship surface users inspect and, when defined, traverse. Edges have families (Semantic, Traversal, Containment, Arrangement, Imported) — see `2026-03-14_graph_relation_families.md`.
- **Navigator**: a section-structured hierarchical projection over graph
  relation families rendered through one or more hosts. Sections may include
  Workbench (arrangement), Folders (containment/user-folder), Domain
  (containment/derived), Unrelated, Recent (traversal), and Imported.

### 2.3 Ownership model

- Graphshell owns camera policy, selection truth, activation semantics, routing, and graph interaction meaning.
- The framework layer may provide hit surfaces, event capture, and paint execution.
- The framework must not be the semantic owner of:
  - camera state,
  - selection truth,
  - lasso meaning,
  - node activation policy,
  - graph-to-workbench routing.
- The Navigator, when active in any host, is a read-only projection over graph
  relation families and must not become the owner of graph identity.

### 2.4 Subsystem boundary

- The Graph subsystem owns graph truth:
  - nodes,
  - edges,
  - graph-backed hierarchical containment relations where defined,
  - graphlets / connected groups,
  - graph selection,
  - graph camera target semantics,
  - `GraphViewId` scope and view-state semantics.
- The Workbench subsystem does not own graph truth.
- The Workbench subsystem may host graph presentation surfaces, but it only does so after
  Graphshell routing bridges graph content into a workbench destination.
- A `GraphViewId` is a scoped view instance within a `GraphId`; panes may host a `GraphViewId`, but they do not define its identity.

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
- Single click without a selection modifier replaces the current selection set
  with the target under the pointer.
- `Ctrl`-click (or configured additive-selection modifier) toggles the target's
  membership in the current selection set.
- Double click activates the target's primary action.
- If a target has no defined activation behavior, double click is a no-op beyond maintaining selection or inspection state.
- Hierarchical navigation actions, when invoked through the Navigator projection, must resolve through the same graph identity and routing rules as canvas-originated actions.
- Selection sets may mix any visible interactable graph-surface objects. Graphshell
  must not invent a hidden primary target inside a mixed selection.

### 3.3 Canonical guarantees

The graph surface must make these user expectations reliable:

- camera controls affect the active graph pane only,
- selection is explicit and visible,
- node activation routes through Graphshell open policy,
- graph gestures do not silently fight each other,
- blocked or degraded behavior is explicit rather than silent,
- Navigator navigation, regardless of host, and graph-canvas navigation resolve
  to the same underlying graph-backed identities.

---

## 4. Normative Core

This section defines the stable target behavior for the current graph surface.

### 4.0 Camera/Navigation Guardrail Checklist (normative)

The following guardrails are mandatory for camera/navigation changes:

1. Metadata/layout ID parity must be explicit.
   - Any key used to read/write graph metadata or layout state must match the key shape used by `egui_graphs` for the same surface.
   - Partial identity migrations (some callsites `None`, others custom id) are forbidden.
2. No per-frame implicit camera override.
   - Continuous fit/recenter loops must not silently overwrite manual pan/zoom every frame.
   - First-frame fit and explicit fit commands are allowed; implicit perpetual fit is not.
3. Camera command ownership remains explicit.
   - `CameraCommand::Fit` (and related fit-family commands) are Graphshell-owned semantic actions.
   - Framework helpers may execute drawing but do not define camera truth.
4. Coordinate-space invariant must hold.
   - `MetadataFrame.pan` and related camera state writes are in widget-local space.
   - Screen-space conversion is render-path-only.
5. Physics/camera boundaries must not blur.
   - Physics presets affect node simulation only.
   - Physics profile changes must not mutate camera lock policy or zoom ownership.
6. Multi-view identity policy must be all-or-nothing.
   - Shared-slot behavior is acceptable only when explicitly declared.
   - Per-view isolation requires consistent `GraphView` id usage across graph view creation, layout state, and metadata state.
7. Regression checks are mandatory before merge.
   - Pan, wheel zoom, fit command, and lock toggles must all be verified in the same patch lane.
   - Any change touching metadata/layout IDs must include targeted tests or scenario updates proving no dead-slot write path.

### 4.1 Camera and Viewport

**What this domain is for**

- Let the user move through graph space predictably and keep the active graph pane under user control.
- Define camera behavior as a Graphshell-owned policy surface, not a fixed widget behavior.

**Core controls**

- Drag selected node: move graph content.
- Wheel over graph pane: zoom at pointer.
- Pointer drag on empty canvas: pan the active graph pane.
- `Camera Fit`: fit relevant graph content.
- `Graphlet Fit`: fit the current connected cluster or explicitly targeted graph subset.
- `Focus Selection`: fit selected nodes only.
- `Zoom Reset`: restore canonical default zoom baseline.
- `C`: toggle position-fit lock (camera position follows fit target when enabled).
- `Z`: toggle zoom-fit lock (camera zoom follows fit target when enabled).
- Keyboard pan (`Arrow Keys` / `WASD`) is a camera control and must be configurable when enabled.
- Manual camera is the default policy: new graph views start with fit locks disabled and no startup auto-fit command.
- Physics presets (`Liquid`, `Gas`, `Solid`) affect node dynamics only and must not implicitly change camera behavior.

**Who owns it**

- Graphshell camera controller owns camera state, command targeting, zoom policy, fit-lock policy, and fit-strength policy.
- Graphshell graph/layout controller owns physics preset selection (`Liquid`, `Gas`, `Solid`) and node-dynamics behavior.
- The framework may only provide raw pointer and wheel events plus drawing surfaces.

**State transitions**

- Camera settings independently configure `position-fit lock`, `zoom-fit lock`, and fit strength for the active graph pane.
- `C` toggles `position-fit lock` for the active graph pane.
- `Z` toggles `zoom-fit lock` for the active graph pane.
- Panning changes viewport translation in the active graph pane.
- After manual pan input, camera translation may decay with slight inertia damping before settling.
- Wheel zoom changes viewport scale around the pointer target in the active graph pane.
- `Camera Fit` updates camera state to show relevant graph bounds.
- `Graphlet Fit` updates camera state to show the chosen graph subset bounds.
- `Focus Selection` updates camera state to show selected-node bounds.
- When `position-fit lock` is enabled, fit-family commands and graph-bounds changes may reconverge camera translation toward the current fit target.
- When `zoom-fit lock` is enabled, fit-family commands and graph-bounds changes may reconverge camera scale toward the current fit target.
- Node dragging changes graph bounds and may therefore update the fit target, but it must not silently disable manual pan or zoom.
- `Fit Strength` changes how strongly the camera converges toward the fit target over time.
- Physics preset changes update node simulation behavior only; they must not mutate camera locks or current camera state.

**Visual feedback**

- Camera movement must be immediate and legible.
- Zoom must visually anchor around the pointer, not drift arbitrarily.
- Fit operations must visibly land on the intended target set.
- If either fit lock is active, the user must be able to perceive that the camera is following graph bounds rather than being in a fully manual state.

**Camera invariants (pre-renderer/WGPU closure)**

- Zoom ownership is per active graph pane; a zoom request for one view must not mutate camera state in another view.
- Wheel zoom is pointer-relative when an anchor is available; missing anchor/metadata paths must defer safely or emit diagnostics rather than silently mutating camera state.
- `Zoom Reset` and fit-family commands resolve to deterministic camera targets under the active camera policy.
- Manual pan and manual zoom remain available regardless of active physics preset.
- Physics presets must never be used as implicit camera-mode selectors.

**Pointer-anchor and passive-input invariants (UX migration §5.2 closure)**

- Pointer-relative zoom anchor is resolved at wheel-sequence start from this order: hovered node anchor -> hovered edge midpoint anchor -> canvas pointer world position.
- If no valid world-space anchor can be resolved, wheel input is treated as passive for graph zoom (no camera mutation) and a diagnostics warning is emitted.
- Target lock for a wheel sequence is sticky: once anchor is chosen, subsequent wheel deltas in the same sequence must use the same anchor until sequence end.
- Graph zoom must not claim wheel input when a higher-priority input owner is active (`Modal`, `CommandPalette`, `TextEntry`, or non-active graph pane).
- Pointer-relative zoom is scoped to the active graph pane only; non-active panes may inspect hover but must not mutate camera state.

**Zoom diagnostics assertions (normative)**

| Flow | Required diagnostics assertions |
|---|---|
| Anchor resolved and zoom applied | `ux:navigation_transition` with `operation=zoom_pointer_relative`, `graph_view_id`, `anchor_kind`, and scale delta |
| Missing anchor / passive no-op | `ux:navigation_violation` (`Warn`) with `operation=zoom_pointer_relative` and `reason=anchor_unresolved` |
| Wheel denied due to input ownership | `ux:contract_warning` with `reason=zoom_input_owner_mismatch` and owning context |

**Fallback / degraded behavior**

- If input cannot be claimed, Graphshell must emit diagnostics and preserve existing camera state.
- Silent camera no-op behavior is forbidden.
- If a fit lock cannot be honored, Graphshell must preserve manual camera control and make the degraded fit-follow state explicit.

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

**Node manipulation command map (UX migration §5.3 closure)**

Default bindings are profile-configurable, but semantic action mapping is fixed:

| Action semantic | Default command/binding | Required behavior |
|---|---|---|
| Create node | `New Node` command (default `N`) | Create node in active graph context; selection moves to new node |
| Delete selected nodes | `Delete Selected` command (default `Delete`/`Backspace`) | Remove selected nodes via reducer-owned delete intent |
| Pin selected nodes | `Pin Selected` command (default `P`) | Set selected nodes to pinned state without changing selection scope |
| Unpin selected nodes | `Unpin Selected` command (`Shift+P`) | Clear pinned state for selected nodes |
| Group-move mode toggle | `Toggle Group Move` command (`G`) | While active, dragging any selected node moves all selected nodes as a cohort |

**Group-move invariants**

- Group-move applies only when selected set cardinality is greater than 1.
- Group-move drag preserves pairwise offsets among selected nodes for the duration of one drag sequence.
- Group-move must not mutate non-selected nodes except through physics side effects after drag commit.

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
- Single click edge: select the edge and make it the active inspection target.
- Double click edge: open the edge's family/category in History Manager and
  reveal the matching recency-ordered entry.
- Context action on edge: expose edge-scoped commands through the canonical palette shell.

**Who owns it**

- Graphshell graph interaction controller owns edge inspection meaning, traversal policy, and any edge-selection semantics.
- The framework may only report edge hits and paint edge state.

**State transitions**

- Edge hover changes inspection context only.
- Edge single-click replaces the current selection set with the target edge
  unless modifier selection is active.
- Edge double-click opens History Manager scoped to the relevant edge family and
  edge/event entry; it does not append traversal history by itself.
- Dismissing an edge updates the current `GraphViewId`'s `EdgePolicy` for that
  edge instance only, removing its presentation and graph effect from the
  current view without deleting the underlying relation/event truth.

**Edge-focus and traversal invariants**

- Edge focus (`SetHighlightedEdge` / `ClearHighlightedEdge`) is inspection-only state and must not append traversal history by itself.
- Traversal history mutation is owned by the history/reducer traversal append path, not by edge hover or single-click inspection.
- Clearing edge focus is explicit and deterministic when the active inspection target changes to none.
- Edge dismissal is view-local. Suppressing one edge in the current graph view
  must not hide other edges of the same family or delete provenance truth.

**Edge-management interaction parity (UX migration §5.4 closure)**

| Interaction | Canvas semantic result | History semantic result |
|---|---|---|
| Edge hover | Update inspection context only | No traversal append |
| Edge single-click | Select edge and set highlighted edge for inspection | No traversal append |
| Edge double-click | Open matching family/category entry in History Manager | No traversal append |
| Edge dismiss | Suppress this edge instance in current `GraphViewId` via `EdgePolicy` | Underlying truth remains visible to History Manager |

**Edge-context command map (C2.2 closure)**

Right-clicking an edge summons the canonical contextual shell with the **Edge**
category first. The minimum edge-context command set is:

| Action semantic | Default command/binding | Required behavior |
|---|---|---|
| Dismiss edge in this view | `Dismiss Edge` command (contextual) | Suppress only this edge instance in the current `GraphViewId` |
| Open edge family in history | `Open Edge History` command (default `Enter` on selected edge if double-click unavailable) | Open History Manager to the edge's family/category and reveal matching entry |
| Remove user edge | `Remove User Edge` command (default `Alt+G` for selected pair) | Remove user-grouped edge semantics for the active/selected pair |
| Connect source -> target | `Connect Pair` command (default `G` for selected pair) | Create one directed user-grouped edge from ordered pair source to target |
| Connect both directions | `Connect Both Directions` command (default `Shift+G` for selected pair) | Create user-grouped edges in both directions for the active/selected pair |

Edge-context invariants:

- Edge context is command-capable only when the resolved edge target maps to a valid pair context.
- Inspection-only traversal edges must still expose disabled edge commands with explicit reasons rather than silently hiding the category.
- Edge-context command invocation must route through the same `ActionRegistry` semantics used by keyboard and other palette modes.

**Edge diagnostics assertions (normative)**

| Flow | Required diagnostics assertions |
|---|---|
| Inspection-only edge focus update | `ux:navigation_transition` with `operation=edge_inspection` and `history_append=false` |
| Traversal-eligible edge activation | `ux:navigation_transition` with `operation=edge_traversal_activate` and resolver target |
| Edge activation blocked/fallback | `ux:navigation_violation` (`Warn`) plus `ux:contract_warning` with reason |

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
- Lasso gesture: select visible interactable graph-surface objects by region
  using the declared lasso binding.
- Empty-canvas context action: expose graph-scope commands.

**Who owns it**

- Graphshell graph interaction controller owns empty-canvas meaning, lasso semantics, and gesture precedence.
- The framework may only provide pointer events and paint transient lasso visuals.

**State transitions**

- Empty-canvas click clears the current selection set.
- Lasso updates the app-owned mixed selection set based on the chosen region and selection mode.
- Context commands do not directly mutate graph truth until a command is executed.

**Lasso invariants (pre-renderer/WGPU closure)**

- Lasso selection mode is deterministic from binding + modifiers: `Alt => Toggle`; otherwise `Add` when `Ctrl` is active or when the binding is `RightDrag` with `Shift`; otherwise `Replace`.
- Lasso candidate sets are canonicalized before intent dispatch (stable sort +
  deduplicate by selection-object key).
- Lasso, pan, and node drag are mutually exclusive gesture owners for a pointer sequence; ambiguity must be canceled explicitly with diagnostics.

**Lasso boundary semantics (UX migration §5.1 closure)**

- Boundary inclusion policy is intersection-inclusive: an interactable object is
  in the lasso candidate set when its hit bounds intersect the lasso
  polygon/rectangle by any positive area.
- Replace/Add/Toggle semantics apply over the candidate set after canonicalization (stable sort + dedupe).
- Lasso region sampling is deterministic: evaluation uses pointer-up finalized region, not intermediate hover jitter.
- If lasso starts on an eligible node-handle region, node drag owns the sequence and lasso does not arm.
- If lasso starts on empty canvas under declared lasso binding, lasso owns the sequence and pan is suppressed.

**Lasso diagnostics assertions (normative)**

| Flow | Required diagnostics assertions |
|---|---|
| Lasso commit success | `ux:navigation_transition` with `operation=lasso_commit`, `selection_mode`, and `candidate_count` |
| Lasso canceled due to owner ambiguity | `ux:navigation_violation` (`Warn`) with `reason=gesture_owner_ambiguous` |
| Lasso denied by higher-priority context | `ux:contract_warning` with owning context and denied operation |

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

### 4.8 LOD Semantic Zoom Policy (UX migration §6.3 closure)

**What this domain is for**

- Keep semantic zoom behavior predictable while preserving interaction clarity and UxTree signal quality.

**LOD tiers and thresholds (canonical defaults)**

| LOD tier | Scale range (`camera.scale`) | Node rendering contract | Semantic interaction contract |
|---|---|---|---|
| `Point` | `< 0.55` | Node points/minimal marks only | Node-level interactions suppressed; graph-level navigation remains active |
| `Compact` | `0.55 .. < 1.10` | Compact node glyph + key badge | Node selection/inspection available; reduced detail fields |
| `Expanded` | `>= 1.10` | Full node affordances and labels | Full node interaction and contextual commands |

**Transition and hysteresis policy**

- LOD transitions are threshold-based with hysteresis band `±0.05` around each threshold.
- A tier transition commits only when `camera.scale` exits the current tier's hysteresis band.
- For one zoom sequence, LOD recalculation runs after final camera-scale update for that event tick.

**UxTree cross-link (normative)**

- `GraphNode` semantic emission in UxTree must follow `ux_tree_and_probe_spec.md §3.1 C5`.
- At `Point` LOD, individual `GraphNode` children are omitted from UxTree and `GraphView` carries a `StatusIndicator` child labeled "Zoom in to interact with nodes.".
- At `Compact` and `Expanded` LOD, UxTree must emit node-semantic children for interactable nodes.

**Diagnostics assertions (normative)**

| Flow | Required diagnostics assertions |
|---|---|
| LOD tier transition committed | `ux:navigation_transition` with `operation=lod_tier_change`, `from_tier`, `to_tier`, `graph_view_id` |
| LOD transition suppressed by hysteresis | `ux:contract_warning` with `operation=lod_tier_change` and `reason=hysteresis_hold` |
| UxTree emission mismatch for active LOD | `ux:navigation_violation` (`Warn`) with expected vs observed semantic node-emission mode |

---

## 5. Planned Extensions

These are intended near-term behaviors. They are not yet required for baseline closure, but they are part of the immediate design direction.

### 5.1 Richer node and routing controls

- Explicit "open in split" and "open in frame" actions from node context
- Pin and unpin actions directly from the graph surface
- More explicit group-drag affordances

### 5.2 Richer relationship tooling

- Edge traversal previews
- Edge filtering or highlighting by relationship type (`UserGrouped`, `TraversalDerived`, `AgentDerived`)
- Better relationship-specific context actions
- Multi-kind edge visual controls (hide or emphasize specific kinds)

### 5.3 (deferred) Categorical edges and Mediator Nodes

When tag-based categorical relationships are implemented, edges of kind `Categorical` will route through a **Mediator Node** (a system-managed node representing the shared tag, e.g. `#Rust`). The graph interaction surface must handle:

- Mediator Node creation on first tag assignment and garbage collection when its degree drops to zero.
- `Categorical` edge rendering distinct from `UserGrouped` and `TraversalDerived` (e.g. dashed line).
- Mediator Node presentation as a non-content node type (non-activatable, distinct visual treatment).
- The interaction spec for `Categorical` edge kind and Mediator Node lifecycle belongs in this spec; the `EdgeKind` data-model extension belongs in `edge_traversal_spec.md`.

This is a prospective extension — do not implement until a dedicated design doc for categorical/tag-based edges is written.

### 5.4 Better graph navigation support

- Camera bookmarks
- Search-targeting affordances in graph space
- More explicit selection-to-navigation handoffs
- Configurable physics presets:
  - `Liquid` for more fluid node motion
  - `Gas` for more energetic / free-moving node motion
  - `Solid` for more damped / rigid node motion
- Configurable `Fit Strength` control for auto-fit behavior, from aggressive framing to gentle framing assistance
- Keyboard camera pan bindings (`Arrow Keys` and/or `WASD`)
- Explicit user-facing toggles for:
  - `position-fit lock` (`C`)
  - `zoom-fit lock` (`Z`)
  - cursor-driven camera behavior
  - auto-fit to graph
  - auto-fit to graphlet
  - follow-selection framing assistance

### 5.5 Per-domain Graph settings page

- node-dynamics preset selector (Liquid/Gas/Solid), physics engine coefficient controls, position-fit lock toggle, zoom-fit lock toggle, keyboard pan bindings, pan inertia controls
- exposed via the **Graph** settings category in `aspect_control/settings_and_control_surfaces_spec.md §4.2`

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

### 6.4 (prospective) Self-loop edges as node audit log carriers

Logical self-loops (edges where `source == target`) are currently forbidden by the traversal skip rule and must be excluded from physics simulation regardless of how they arise (see `edge_traversal_spec.md §2.6`). A future node audit log design may permit self-loop edges to carry `MetadataChange` or workbench-action events as a localized per-node event log. If implemented:

- Self-loop edges must remain headless (logical only) and excluded from physics.
- They must not render as circular lines on the canvas.
- The audit log design belongs in a dedicated `node_audit_log_spec.md` (see `subsystem_history/` deferred stub); it is not part of the edge traversal model.

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
10. Lasso modifier-mode resolution and key normalization rules are explicit and deterministic.
11. Edge-focus inspection is explicitly separated from traversal-history mutation semantics.
12. Pointer-anchor and passive-input zoom invariants are explicit and diagnostics-backed.
13. Node create/delete/pin/group-move semantic mapping is explicit and deterministic.
14. Lasso boundary inclusion policy and gesture-owner precedence are explicit and deterministic.
15. Edge management interactions are explicitly aligned with history append rules and diagnostics.
16. LOD semantic zoom thresholds and hysteresis rules are explicit and deterministic.
17. LOD-to-UxTree semantic emission behavior is explicitly cross-linked and diagnostics-backed.


