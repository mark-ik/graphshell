# Workbench / Frame / Tile Interaction Spec

**Date**: 2026-02-27  
**Status**: Canonical interaction contract (UX baseline aligned)  
**Priority**: Immediate implementation guidance

**Related**:
- `PLANNING_REGISTER.md`
- `2026-02-27_ux_baseline_done_definition.md`
- `2026-02-28_ux_contract_register.md`
- `../canvas/graph_node_edge_interaction_spec.md`
- `WORKBENCH.md`
- `../canvas/CANVAS.md`
- `../subsystem_focus/focus_and_region_navigation_spec.md`
- `../viewer/viewer_presentation_and_fallback_spec.md`
- `../aspect_control/settings_and_control_surfaces_spec.md`
- `SYSTEM_REGISTER.md`
- `../technical_architecture/GRAPHSHELL_AS_BROWSER.md`
- `../../TERMINOLOGY.md`
- `../design/KEYBINDINGS.md`

---

## 1. Purpose and Scope

This spec defines the interaction contract for the **workbench layer** of Graphshell.

Within the architecture, this surface is the user-facing interaction layer of the
**Workbench subsystem**.

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

For graph-surface semantics (nodes, edges, canvas, camera), see:

- `../canvas/graph_node_edge_interaction_spec.md`

For cross-app focus rules, viewer-state clarity, and app-owned tool surfaces, see:

- `../subsystem_focus/focus_and_region_navigation_spec.md`
- `../viewer/viewer_presentation_and_fallback_spec.md`
- `../aspect_control/settings_and_control_surfaces_spec.md`

---

## 2. Canonical Structure

### 2.1 Canonical hierarchy

1. One `GraphId` maps to one workbench context.
2. Workbench is the persistent host for that graph context.
3. Workbench tracks an ordered set of Frames in the workbench bar.
4. A Frame is a persisted branch/subtree of the workbench tile tree.
5. A Frame contains arranged Tiles.
6. A Tile is the primary arrangeable unit (tab-like affordance; canonical term: tile).
7. A Pane is the content presentation surface rendered within a Tile.
8. A Node is canonical graph content identity/state.

Hierarchy:

`Node -> Pane -> Tile -> Frame -> Workbench`

### 2.2 What each layer is for

- **Workbench**: persistent context host for one graph.
- **Frame**: named and persisted arrangement context inside the workbench.
- **Tile**: primary arrangeable workspace unit users move, group, split, and focus.
- **Pane**: rendered content surface hosted by a tile.
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

### 2.4 Subsystem boundary

- The Workbench subsystem owns arrangement truth:
  - the tile tree,
  - frame branches,
  - split geometry,
  - tile activation,
  - pane hosting.
- The Graph subsystem does not own arrangement truth.
- The Graph subsystem may request routing into the workbench, but it does not define the
  workbench tree structure.

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

- Single click selects and focuses the target UI object (tile, frame, or related workbench element).
- Double click activates the target's primary action.
- If an object is non-activatable, double click is a no-op beyond maintaining selection.

### 3.3 Canonical guarantees

The workbench layer must make these user expectations reliable:

- opening content lands in a usable destination,
- arranging work never changes graph identity,
- closing presentation surfaces never deletes graph truth,
- focus handoff is deterministic during structural change,
- fallback behavior is explicit rather than silent.

---

## 4. Normative Core

This section defines the stable target behavior for the current workbench layer.

### 4.1 Workbench Creation and Switching

**What this domain is for**

- Create and select persistent arrangement contexts for the active graph.

**Core controls**

- Workbench bar exposes explicit `Create Tile` and `Create Frame` actions.
- Workbench bar shows:
  - frame order,
  - active frame,
  - compact context metadata (for example tile count).
- Selecting a frame changes the active frame context without changing graph identity.

**Who owns it**

- Graphshell workbench controller owns frame creation semantics, active-frame state, and persistence.
- `egui_tiles` may render tab strips and layout chrome, but it does not define frame meaning.

**State transitions**

- `Create Tile` adds a new tile in the active frame context or declared destination.
- `Create Frame` creates a new persistent frame context and makes it available as a destination.
- Switching frame changes active-frame focus and visible arrangement context only.

**Visual feedback**

- Active frame state must be obvious.
- Newly created frames and tiles must be visible as soon as they are created.
- Frame selection changes must be legible even when the underlying content is similar.

**Fallback / degraded behavior**

- If a frame or tile cannot be created, the failure must be explicit.
- Blank or ambiguous frame-switch outcomes are forbidden.

### 4.2 Tile Open, Focus, and Routing

**What this domain is for**

- Route graph content into the workbench and preserve one clear default open path.

**Core controls**

- Double-clicking a graph node opens or focuses the node's Tile first.
- There is no mandatory frame picker on the default path.
- The command palette remains the explicit power path for routing a node to a specific frame.

**Who owns it**

- Graphshell routing + lifecycle authority owns the open path and duplicate policy.
- The framework may only host the destination tile UI once Graphshell has chosen the destination.

**State transitions**

- Node activation triggers a routing decision.
- If a matching tile already exists in the chosen destination, that tile is focused.
- Otherwise, a new tile is created in the default destination.
- Explicit routing follows these rules:
  1. If the node exists in exactly one frame, open or focus it there.
  2. If the node exists in multiple frames, default to the last-active frame.
  3. Provide explicit override in the command palette to select another frame.
  4. If the node has no frame membership, create a new frame seeded with that node tile.

**Visual feedback**

- Opening must visibly land the user in the destination tile.
- If the target is in a non-active frame, the destination jump must be legible.
- If routing creates a new frame or tile, creation must be visible.

**Fallback / degraded behavior**

- Silent duplication is forbidden.
- If routing falls back to creation, the fallback must be explicit and deterministic.
- If a preferred destination is unavailable, Graphshell may choose a safe default, but it must not hide that decision.

### 4.3 Tile Arrangement: Group, Reorder, Split

**What this domain is for**

- Rearrange presentation structure without changing graph identity.

**Core controls**

- Drag Tile -> tile handle: group into the same frame branch.
- Drag Tile -> tile viewport: enter split-arrangement targeting UI.
- Drag Tile between tile selectors: reorder selectors.

**Who owns it**

- Graphshell workbench controller owns the meaning of grouping, splitting, and reorder effects.
- The framework may provide split geometry, drag surfaces, and tab-strip visuals.

**State transitions**

- Reorder changes tile order only.
- Group changes branch membership within the frame.
- Split creates or mutates a structural split container in the frame tree.

**Split orientation and drop-zone semantics**

- Vertical divider produces left/right regions.
- Horizontal divider produces top/bottom regions.
- Viewport drop-zones must preview target outcome before commit:
  - left/right edge zone -> vertical split
  - top/bottom edge zone -> horizontal split
  - center zone -> group or stack in the same frame branch

**Visual feedback**

- Drop targets must preview the resulting structure before commit.
- Group, reorder, and split outcomes must remain legible after commit.
- Invalid drops must visibly cancel.

**Fallback / degraded behavior**

- Unsupported targets or surfaces cancel safely with explicit feedback.
- Invalid structural operations must not leave behind ambiguous half-state.

### 4.4 Close, Empty-State, and Recovery

**What this domain is for**

- Preserve usability and context integrity when presentation surfaces disappear.

**Core controls**

- Close Tile
- Close Frame
- Empty-state actions:
  - create tile
  - open node
  - close frame

**Who owns it**

- Graphshell workbench/pane controller owns close semantics, handoff rules, and empty-state meaning.
- The framework may render empty-state surfaces, but it does not define the recovery path.

**State transitions**

- Tile close removes a presentation instance, not graph identity.
- Frame close removes an arrangement context, not graph identity.
- Closing the last tile in a frame must route deterministically to:
  - the next frame context, or
  - an explicit empty workbench state.
- Focus is reassigned deterministically.

**Visual feedback**

- The resulting target context must be immediately visible and usable.
- Empty-state surfaces must be visible and actionable.

**Fallback / degraded behavior**

- Blank ambiguous regions are forbidden.
- If no successor context exists, show an explicit empty-state surface.

### 4.5 History and Undo Semantics

**What this domain is for**

- Preserve both navigation meaning and structural editing meaning without conflating them.

**Core controls**

- **Back / Forward**: tile navigation history (traversal-driven)
- **Undo / Redo**: workbench structural edits (global within the active workbench context)

**Who owns it**

- Graphshell history authority owns event capture, scope, merge policy, and undo semantics.
- The framework may only surface history UI and preview affordances.

**State transitions**

Frame history is derived from two streams:

- **Traversal Stream**: edge-attached navigation events (graph semantics)
- **Workbench Stream**: tile/frame/split/reorder/open/close operations (arrangement semantics)

Merged history uses monotonic event time plus deterministic tie-breaker policy.

**Undo interaction contract**

- Hold `Ctrl+Z`: show undo preview indicator (non-committed).
- While holding `Ctrl`, release `Z`: commit one undo step.
- Releasing `Ctrl+Z` without the commit path cancels preview.
- Holding `Ctrl` and re-pressing `Z` steps preview backward one additional operation per repeat.

If key-event limitations require adaptation, preserve the same user-visible semantics through the nearest equivalent interaction.

**Visual feedback**

- History scope and preview state must be legible.
- Users must be able to tell whether they are previewing or committing.

**Fallback / degraded behavior**

- If merged history details are unavailable, Graphshell may show reduced history detail, but scope separation must remain explicit.
- Silent scope collapse between traversal and structural history is forbidden.

### 4.6 Workbench Boundaries, Visual Feedback, and Accessibility

**What this domain is for**

- Make the workbench trustworthy, teachable, and usable under normal and degraded conditions.

**Core semantic boundaries**

1. Edges and Traversals are graph truth (global within a `GraphId`).
2. Frames and Tiles are arrangement truth (workbench-local context).
3. Connected nodes are not required to co-exist in one frame.
4. A node may appear in multiple frames without duplicating graph identity.

**Frame identity**

- Each frame has a stable identity (`Frame N`) and editable display label.
- Auto-suggest labels may be derived from member tiles, but the user label is authoritative.

**Visual feedback**

- Active frame and active tile states must be legible.
- Split-drop previews must be visible before commit.
- Invalid drops must be explicit.
- Empty-state surfaces must be visible and actionable.
- Creation, routing, and recovery fallbacks must be visible.

**Accessibility**

- Every drag operation must have keyboard-equivalent commands.
- Selection and activation state changes must be clearly announced and visible.
- History previews and undo-scope indicators must be keyboard-accessible.
- Keyboard-only users must be able to:
  - create tile and frame,
  - group tiles into frames,
  - split by orientation,
  - reorder tiles,
  - switch frames.

**Performance**

- Frame switches and tile open should remain responsive under quick smoke workloads.
- Inactive tile content may degrade (`Warm` / `Cold`) with explicit user-visible state.
- Degradation and fallback reasons must be observable in diagnostics.

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

---

## 6. Prospective Capabilities

These are exploratory design directions. They are informative only and should not be treated as current implementation requirements.

### 6.1 Semantic frame workflows

- Frame presets based on graph task context
- Reusable frame templates
- Batch-opening selected nodes into structured frame arrangements

### 6.2 Richer structural semantics

- Semantic tile groups beyond basic split/tab structure
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
3. Workbench bar includes explicit create actions for tile and frame.
4. Frame grouping, reorder, and split semantics are implemented and test-covered.
5. Merged history (Traversal + Workbench streams) is represented in design and diagnostics.
6. Undo preview interaction contract is documented and exposed in help/keybinding surfaces.
7. Accessibility parity exists for non-pointer users.
8. Split-drop previews and invalid-drop feedback are deterministic and test-covered.
9. Duplicate-tile policy and open-feedback behavior are deterministic and test-covered.
10. Focus handoff and empty-state behavior are deterministic and visibly recoverable.


