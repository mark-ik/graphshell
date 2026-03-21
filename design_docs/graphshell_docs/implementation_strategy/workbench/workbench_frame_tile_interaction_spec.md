# Workbench / Frame / Tile Interaction Spec

**Date**: 2026-02-27  
**Status**: Canonical interaction contract (UX baseline aligned)  
**Priority**: Immediate implementation guidance

**Related**:

- `../system/register/PLANNING_REGISTER.md`
- `../subsystem_ux_semantics/2026-03-01_ux_execution_control_plane.md`
- `2026-02-28_ux_contract_register.md`
- `../canvas/graph_node_edge_interaction_spec.md`
- `WORKBENCH.md`
- `../canvas/CANVAS.md`
- `../subsystem_focus/focus_and_region_navigation_spec.md`
- `../viewer/viewer_presentation_and_fallback_spec.md`
- `../aspect_control/settings_and_control_surfaces_spec.md`
- `../subsystem_ux_semantics/2026-03-13_chrome_scope_split_plan.md` — WorkbenchLayerState, ChromeExposurePolicy, Graph Bar vs Workbench Sidebar split
- `../canvas/2026-03-14_graph_relation_families.md` — ArrangementRelation edges backing frame/tile membership; Navigator projection sections
- `workbench_profile_and_workflow_composition_spec.md`
- `../subsystem_ux_semantics/2026-03-04_model_boundary_control_matrix.md`
- `../system/register/SYSTEM_REGISTER.md`
- `../technical_architecture/GRAPHSHELL_AS_BROWSER.md`
- `../../TERMINOLOGY.md`
- `../design/KEYBINDINGS.md`

**Adopted standards** (see [standards report](../../research/2026-03-04_standards_alignment_report.md) §§3.5, 3.6):

- **WCAG 2.2 Level AA** — SC 2.5.8 (tile/frame interactive targets), SC 2.4.3 (focus order in tile tree), SC 2.4.11 (focus appearance), SC 2.1.1 (keyboard equivalents for all drag operations)
- **OpenTelemetry Semantic Conventions** — diagnostics for routing fallbacks, focus-handoff failures

## Model boundary (inherits UX Contract Register §3B)

- `GraphId` = truth boundary.
- `GraphViewId` = scoped view state.
- Graph Bar = graph-scope chrome that names the active graph target and graph-view scope.
- **Navigator** (Workbench Sidebar projection) = graph-backed hierarchical projection over relation families (replaces "file tree" — see `canvas/2026-03-14_graph_relation_families.md §5`).
- workbench = arrangement boundary.

This spec owns arrangement semantics only; it must not define graph truth or Navigator content authority.

## Contract template (inherits UX Contract Register §2A)

Normative workbench contracts use: intent, trigger, preconditions, semantic result, focus result, visual result, degradation result, owner, verification.

## Terminology lock (inherits UX Contract Register §3C)

- Tile/frame arrangement is not content hierarchy.
- Navigator (Workbench Sidebar projection) is not content truth authority — it is a read-only projection.
- Physics presets are not camera modes.
- "File tree" is a legacy alias — use **Navigator** in new code and docs.
- Node and Tile are not synonyms: a node is graph identity; a tile is its workbench presentation/container.
- Graphlet and Tile Group are not synonyms: a graphlet is grouped graph arrangement; a tile group is its workbench presentation/container.

### Status update (2026-03-18)

- Runtime intent surfaces now use `Navigator*` carriers directly; legacy
  `FileTree*` intent variants are removed from active reducer/view-action paths.
- Navigator containment projection resolves from graph `ContainmentRelation`
  edges and is refreshed from graph deltas (node add/remove and URL updates),
  preserving the graph-truth-first contract described in this spec.

---

## 1. Purpose and Scope

This spec defines the interaction contract for the **workbench layer** of Graphshell.

Within the architecture, this surface is the user-facing interaction layer of the
**Workbench subsystem**.

The workbench is not a peer semantic owner beside the graph. It is the contextual
presentation layer under the currently active graph target named by the Graph Bar.

It explains:

- what the workbench, frames, tiles, and panes are for,
- what current workbench actions mean semantically,
- who owns each behavior,
- what state transitions those interactions imply,
- what visual feedback must accompany them,
- what fallback behavior must happen when the ideal path is unavailable,
- which controls are core, planned, and exploratory.

This is not the graph-canvas contract.

This spec governs:

- `Workbench`
- `Frame`
- `Tile`
- `Pane`
- the user-facing contract of the workbench tile tree subsystem

This spec does not define durable content hierarchy. That remains a Graph subsystem concern, even when graph-backed content is also exposed through hierarchical navigation surfaces.

For graph-surface semantics (nodes, edges, canvas, camera), see:

- `../canvas/graph_node_edge_interaction_spec.md`

For cross-app focus rules, viewer-state clarity, and app-owned tool surfaces, see:

- `../subsystem_focus/focus_and_region_navigation_spec.md`
- `../viewer/viewer_presentation_and_fallback_spec.md`
- `../aspect_control/settings_and_control_surfaces_spec.md`

---

## 2. Canonical Structure

### 2.1 Canonical hierarchy

1. `GraphId` is graph truth boundary.
2. `GraphViewId` is graph-owned scoped view identity within that graph truth.
3. The Graph Bar names the active graph target (`GraphId` / `GraphViewId`) one UI level above workbench hosting.
4. The workbench is the contextual arrangement layer for the leaves of the active branch.
5. A Frame is a persisted arrangement context within that workbench layer.
6. A Tile Group / Tile is a structural hosting unit inside the frame.
7. A Pane is the active presentation surface rendered for the selected hosted leaf.
8. A hosted leaf may be:
   - a graph-view pane presenting a `GraphViewId`
   - a node/document/media viewer pane
   - a tool surface
9. A Node remains canonical graph content identity/state regardless of how many hosted leaves present it.

UI-level ordering:

`Graph target (GraphId / GraphViewId) -> Workbench contextual arrangement -> Frame -> Tile/Group -> Pane -> Hosted leaf`

Additional structure rules:

- A `Tile` is the broad arrangement container term; a solo placement is a
  `Tile` with one node entry, and a grouped placement is a `Tile` with multiple
  node entries.
- Tabs belong to the `Tile` and enumerate node entries within that tile.
- A `Pane` is not the primary navigator/workbench container identity; it is the
  live rendered surface for the Tile's currently active node entry.
- A hosted graph view remains graph-owned semantic scope even while presented inside a pane.
- Nodes project as tiles in workbench chrome; graphlets project as tile groups.
- These are presentation correspondences, not term collapses.

### 2.2 What each layer is for

- **Graph Bar target**: the graph-scoped thing currently being named, steered, and configured.
- **Workbench**: contextual presentation layer for the current branch's leaves.
- **Frame**: named and persisted arrangement context inside the workbench.
- **Tile**: primary arrangeable workspace container users move, split, focus,
  and populate with one or more node entries.
- **Pane**: rendered presentation surface hosted by a tile for its currently
  active node entry.
- **GraphViewId**: graph-owned scoped view and lens-state target that may be presented in a pane or selected in the Graph Bar without changing ownership.
- **Node**: graph identity; never reduced to a tile instance.

### 2.3 Ownership model

- Graphshell owns workbench semantics, routing, focus, lifecycle, and persistence rules.
- The framework layer (currently `egui_tiles`) may own layout geometry and tab/split mechanics.
- The framework must not be the semantic owner of:
  - frame meaning,
  - tile destination rules,
  - focus handoff,
  - node-to-frame routing,
  - close/open semantics.
- Tiles and frames are not the canonical owner of content identity, saved hierarchy, or graph-backed containment semantics.

### 2.4 Subsystem boundary

- The Workbench subsystem owns arrangement interaction/session mutation truth:
  - the tile tree,
  - frame branches,
  - split geometry,
  - tile activation,
  - pane hosting.
- The Graph subsystem does not own arrangement truth.
- The Graph Bar and graph-view manager may name/select graph targets above the
  workbench layer without thereby creating or mutating workbench structure.
- The Graph subsystem may request routing into the workbench, but it does not define the
  workbench tree structure.
- Durable arrangement carriers (for example saved frame membership) may be
  graph-rooted through `ArrangementRelation`; the workbench remains the owner of
  interactive session mutation and structural realization.
- The Workbench subsystem may persist arrangement state and return-path memory, but that persistence is workspace state, not durable content hierarchy.
- The **Navigator** (Workbench Sidebar projection), when visible, is a Graph-owned read-only projection over relation families and is not part of workbench arrangement truth.

---

## 3. Canonical Interaction Model

### 3.1 Interaction categories

Workbench interactions fall into four semantic categories:

1. **Selection**
   - choose the current target without committing a structural action
2. **Activation**
   - invoke the primary action of the selected workbench object
3. **Arrangement**
   - change tile/frame structure or placement
4. **Routing**
   - decide where graph content appears in the workbench

### 3.2 Core selection vs activation rule

- Single click without a selection modifier selects and focuses the target UI
  object and replaces the prior selection set.
- `Ctrl`-click toggles the target object's membership in the current selection
  set.
- Workbench selections may mix nodes, tiles, and frames.
- Double click activates the target's primary action.
- If an object is non-activatable, double click is a no-op beyond maintaining selection.

### 3.3 Canonical guarantees

The workbench layer must make these user expectations reliable:

- opening content lands in a usable destination,
- arranging work never changes graph identity,
- closing presentation surfaces never deletes graph truth,
- focus handoff is deterministic during structural change,
- fallback behavior is explicit rather than silent,
- tile order, grouping, and frame membership do not define durable content hierarchy.

---

## 4. Normative Core

This section defines the stable target behavior for the current workbench layer.

### 4.1 Workbench Creation and Switching

Create and select persistent arrangement contexts for the active graph.

**Core controls**: Workbench chrome exposes explicit `Create Tile` and
`Create Frame` actions. In the desktop default, these actions live in the
Workbench Sidebar header and/or frame overflow affordances. A frame
chip/dropdown summarizes frame order and active frame, while the sidebar body
shows the full pane tree. Selecting a frame changes the active frame context
without changing graph identity.

**Owner**: Graphshell workbench controller owns frame creation semantics, active-frame state, and persistence. `egui_tiles` may render tab strips and layout chrome, but it does not define frame meaning.

**State transitions**: `Create Tile` adds a new tile in the active frame context or declared destination. `Create Frame` creates a new persistent frame context. Switching frame changes active-frame focus and visible arrangement context only.

**Visual feedback**: Active frame state must be obvious. Newly created frames
and tiles must be visible immediately. Frame selection changes must be legible
even when underlying content is similar. Frames and tiles may be summarized
compactly in chrome, but panes remain presentation surfaces rather than the
primary structural rows in the sidebar/tree projection.

**Fallback**: If a frame or tile cannot be created, the failure must be explicit. Blank or ambiguous frame-switch outcomes are forbidden.

### 4.1A Inter-Workbench Switch, Open, and Restore Contract

This section defines cross-workbench behavior across `WorkbenchId` boundaries.

**Canonical semantics**:

1. `SwitchWorkbench(target_workbench_id)` changes active arrangement authority to target workbench without mutating graph truth.
2. `OpenInWorkbench(target_workbench_id, open_payload)` routes the open action into target workbench authority, then focuses resulting destination pane/tile.
3. `RestoreWorkbench(target_workbench_id)` restores persisted frame/tile arrangement for the target workbench before focus assignment.
4. Cross-workbench close/return paths must restore focus to a deterministic source anchor when return policy requests it.

**Target resolution order** (open routing): (1) Explicit `target_workbench_id` from command/route intent; (2) last-active workbench for the current graph context; (3) current active workbench. If no target can be resolved, Graphshell must stay in current workbench and emit route/focus diagnostics — no silent no-op.

**Persistence boundaries**: Workbench arrangement state (`frames`, `tiles`, split tree) persists per `WorkbenchId`. Active workbench selection persists per workspace session boundary. Graph content identity and traversal truth do not move ownership when switching workbenches.

**Cross-workbench focus return**: On `OpenInWorkbench` with source context capture enabled, runtime records `(source_workbench_id, source_frame_id, source_tile_id, source_focus_region)`. On closing the routed pane (or explicit return command), focus router restores to the captured source anchor when still valid. If captured anchor is invalid, fallback order is: source frame root → source workbench root → active graph pane in current workbench.

**Route/focus diagnostics assertions (normative)**:

| Flow | Required diagnostics assertions |
| --- | --- |
| `SwitchWorkbench` success | Emit `ux:navigation_transition` with `operation=switch_workbench`, `from_workbench_id`, `to_workbench_id`, and post-switch focus owner |
| `OpenInWorkbench` resolution | Emit `ux:open_decision_path` and `ux:open_decision_reason` including requested and resolved `workbench_id` |
| Cross-workbench focus restore success | Emit `ux:navigation_transition` with `operation=focus_restore`, captured source anchor fields, and resolved focus target |
| Route/focus fallback triggered | Emit `ux:navigation_violation` (`Warn`) and `ux:contract_warning` with fallback reason and applied fallback target |

### 4.2 Tile Open, Focus, and Routing

Route graph content into the workbench and preserve one clear default open path.

**Core controls**: Double-clicking a graph node opens or focuses the node's Tile first. No mandatory frame picker on the default path. The command palette is the explicit power path for routing a node to a specific frame.

**Owner**: Graphshell routing + lifecycle authority owns the open path and duplicate policy. The framework may only host the destination tile UI once Graphshell has chosen the destination.

**State transitions**: Node activation triggers a routing decision. If a matching tile already exists in the chosen destination, that tile is focused; otherwise a new tile is created. Explicit routing priority: (1) if the node exists in exactly one frame, open or focus it there; (2) if the node exists in multiple frames, default to the last-active frame; (3) provide explicit override in the command palette; (4) if the node has no frame membership, create a new frame seeded with that node tile.

**Visual feedback**: Opening must visibly land the user in the destination tile. If the target is in a non-active frame, the destination jump must be legible. If routing creates a new frame or tile, creation must be visible.

**Fallback**: Silent duplication is forbidden. If routing falls back to creation, the fallback must be explicit and deterministic. If a preferred destination is unavailable, Graphshell may choose a safe default but must not hide that decision.

### 4.2A Cross-tree focus and selection integration

Workbench routing and tile activation must comply with the deterministic Focus subsystem contract.

- Selection truth remains owned by the active Graph View within the active Frame.
- Tile/frame activation updates semantic focus owner via the focus router; `egui_tiles` local focus is not semantic authority.
- Pane close and frame-switch flows must restore focus through the canonical return-path algorithm defined in:
  - `../subsystem_focus/focus_and_region_navigation_spec.md` (§4.7.3)
- Pointer hover and keyboard-target conflict handling must follow focus-spec arbitration rules:
  - `../subsystem_focus/focus_and_region_navigation_spec.md` (§4.7.4)

Implementation/test anchor (non-exhaustive):

- `shell/desktop/ui/gui.rs` focus-return and graph/node focus-state tests.
- `shell/desktop/ui/gui_orchestration_tests.rs` orchestration-level focus-return test.
- `shell/desktop/workbench/tile_view_ops.rs` region-cycle routing behavior.

### 4.3 Tile Arrangement: Group, Reorder, Split

Rearrange presentation structure without changing graph identity.

**Core controls**: Drag Tile → tile handle to group into the same frame branch; drag Tile → tile viewport to enter split-arrangement targeting UI; drag Tile between tile selectors to reorder.

**Owner**: Graphshell workbench controller owns grouping, splitting, and reorder semantics. The framework may provide split geometry, drag surfaces, and tab-strip visuals.

**State transitions**:

- Reorder changes tile order only.
- Group changes branch membership within the frame.
- Split creates or mutates a structural split container in the frame tree.

**Split orientation and drop-zone semantics**:

- Vertical divider produces left/right regions.
- Horizontal divider produces top/bottom regions.
- Viewport drop-zones must preview target outcome before commit:
  - left/right edge zone → vertical split
  - top/bottom edge zone → horizontal split
  - center zone → group or stack in the same frame branch

**Visual feedback**: Drop targets must preview the resulting structure before commit. Group, reorder, and split outcomes must remain legible after commit. Invalid drops must visibly cancel.

**Fallback**: Unsupported targets or surfaces cancel safely with explicit feedback. Invalid structural operations must not leave behind ambiguous half-state.

### 4.4 Close, Empty-State, and Recovery

Preserve usability and context integrity when presentation surfaces disappear.

**Core controls**: Close Tile, Close Frame, and empty-state actions (create tile, open node, close frame).

**Owner**: Graphshell workbench/pane controller owns close semantics, handoff rules, and empty-state meaning. The framework may render empty-state surfaces but does not define the recovery path.

**State transitions**: Tile close removes a presentation instance, not graph identity. Frame close removes an arrangement context, not graph identity. `Close` is reserved for presentation containers (tile/frame) — graph content mutation uses `Delete` semantics and is not a tile-close alias. Closing the last tile in a frame routes deterministically to: the next frame context, or an explicit empty workbench state. Focus is reassigned deterministically.

**Visual feedback**: The resulting target context must be immediately visible and usable. Empty-state surfaces must be visible and actionable.

**Fallback**: Blank ambiguous regions are forbidden. If no successor context exists, show an explicit empty-state surface.

### 4.5 History and Undo Semantics

Preserve both navigation meaning and structural editing meaning without conflating them.

**Core controls**: Back/Forward cover tile navigation history
(traversal-driven). Structural undo/redo remains a distinct workbench-history
concern and must never masquerade as pane navigation, regardless of where its
eventual UI is exposed.

**Owner**: Graphshell history authority owns event capture, scope, merge policy, and undo semantics. The framework may only surface history UI and preview affordances.

**State transitions**: Frame history is derived from two streams — Traversal Stream (edge-attached navigation events, graph semantics) and Workbench Stream (tile/frame/split/reorder/open/close operations, arrangement semantics). Merged history uses monotonic event time plus deterministic tie-breaker policy.

**Undo interaction contract**: Hold `Ctrl+Z` to show undo preview indicator (non-committed). While holding `Ctrl`, release `Z` to commit one undo step. Releasing `Ctrl+Z` without commit cancels preview. Holding `Ctrl` and re-pressing `Z` steps preview backward one operation per repeat. If key-event limitations require adaptation, preserve the same user-visible semantics through the nearest equivalent interaction.

**Visual feedback**: History scope and preview state must be legible. Users must be able to tell whether they are previewing or committing.

**Fallback**: If merged history details are unavailable, Graphshell may show reduced detail, but scope separation must remain explicit. Silent scope collapse between traversal and structural history is forbidden.

### 4.6 Workbench Boundaries, Visual Feedback, and Accessibility

Make the workbench trustworthy, teachable, and usable under normal and degraded conditions.

**Semantic boundaries**: (1) Edges and Traversals are graph truth (global within a `GraphId`). (2) Frames and Tiles are arrangement interaction/session context, with durable frame membership able to be graph-rooted via `ArrangementRelation`. (3) Connected nodes are not required to co-exist in one frame. (4) A node may appear in multiple frames without duplicating graph identity.

**Frame identity**: Each frame has a stable identity (`Frame N`) and editable display label. Auto-suggest labels may be derived from member tiles, but the user label is authoritative.

**Visual feedback**: Active frame and tile states must be legible. Split-drop previews must be visible before commit. Invalid drops must be explicit. Empty-state surfaces must be visible and actionable. Creation, routing, and recovery fallbacks must be visible.

**Accessibility**: Every drag operation must have keyboard-equivalent commands. Selection and activation state changes must be clearly announced and visible. History previews and undo-scope indicators must be keyboard-accessible. Keyboard-only users must be able to: create tile and frame, group tiles into frames, split by orientation, reorder tiles, and switch frames.

**Performance**: Frame switches and tile open should remain responsive under quick smoke workloads. Inactive tile content may degrade (`Warm` / `Cold`) with explicit user-visible state. Degradation and fallback reasons must be observable in diagnostics.

---

## 5. Planned Extensions

These are intended near-term behaviors. They are not yet required for baseline closure, but they are part of the immediate design direction.

### 5.1 Richer routing controls

- Explicit "open in split"
- Explicit "open in new frame"
- Explicit "open in specific frame"
- Explicit duplicate-tile command paths

### 5.2 Better structural previews

- Stronger live previews during grouping, reorder, and split drags
- Better invalid-drop explanation
- More legible structural container affordances

### 5.3 History and recovery refinement

- Richer merged-history surfacing
- Better preview visibility for undo scope
- More explicit structural-history diagnostics
- Smarter return-path behavior after disruptive structural changes

### 5.4 Per-domain Workspaces settings page

- default routing behavior, tile close/recovery policy, frame history depth
- exposed via the **Workspaces** settings category in `aspect_control/settings_and_control_surfaces_spec.md §4.2`

---

## 6. Prospective Capabilities

These are exploratory design directions. They are informative only and should not be treated as current implementation requirements.

### 6.1 Semantic frame workflows

- Frame presets based on graph task context
- Reusable frame templates
- Batch-opening selected nodes into structured frame arrangements

### 6.2 Richer structural semantics

- Semantic tile composition beyond basic split/tab structure
- Promotable and demotable structural groupings
- More expressive persistent workspace arrangements

### 6.3 Advanced history workflows

- Rich timeline filtering by traversal vs structural operations
- Named structural checkpoints
- Recoverable arrangement snapshots

---

## 7. Acceptance Criteria (Spec Parity)

1. Terminology docs and active strategy docs use `Tile` as the canonical term (`tab` only as UI-affordance wording).
2. Node double-click follows open-tile-first behavior.
3. Workbench chrome includes explicit create actions for tile and frame.
4. Frame grouping, reorder, and split semantics are implemented and test-covered.
5. Merged history (Traversal + Workbench streams) is represented in design and diagnostics.
6. Undo preview interaction contract is documented and exposed in help/keybinding surfaces.
7. Accessibility parity exists for non-pointer users.
8. Split-drop previews and invalid-drop feedback are deterministic and test-covered.
9. Duplicate-tile policy and open-feedback behavior are deterministic and test-covered.
10. Focus handoff and empty-state behavior are deterministic and visibly recoverable.
11. Inter-workbench switch/open/restore semantics are explicitly defined for `WorkbenchId` routing.
12. Cross-workbench focus return path and fallback order are deterministic and documented.
13. Route/focus diagnostics assertions for inter-workbench flows are defined and testable.
