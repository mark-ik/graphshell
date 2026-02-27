# Workbench / Frame / Tile Interaction Spec

**Date**: 2026-02-27  
**Status**: Canonical interaction contract (UX baseline aligned)  
**Priority**: Immediate implementation guidance

**Related**:
- `PLANNING_REGISTER.md`
- `2026-02-27_ux_baseline_done_definition.md`
- `SYSTEM_REGISTER.md`
- `../technical_architecture/GRAPHSHELL_AS_BROWSER.md`
- `../../TERMINOLOGY.md`
- `../design/KEYBINDINGS.md`

---

## 1) Canonical Structure

1. One `GraphId` maps to one Workbench context.
2. Workbench is the persistent host for that graph context.
3. Workbench tracks an ordered set of Frames in the workbench bar.
4. A Frame is a persisted branch/subtree of the workbench tile tree.
5. A Frame contains arranged Tiles.
6. A Tile is the primary arrangeable unit (tab-like affordance, canonical term: tile).
7. A Pane is the content presentation surface rendered within a Tile.
8. A Node is canonical graph content identity/state.

Hierarchy: `Node → Pane → Tile → Frame → Workbench`.

---

## 2) Interaction Semantics

### 2.1 Selection vs Activation

- Single click: select/focus UI element (node/edge/tile/frame).
- Double click: activate UI element.
- For non-activatable elements, double click is a no-op beyond maintaining selection.

### 2.2 Node Open Path (Intermediate-Step Model)

- Double-clicking a graph node opens/focuses the node's Tile first (no mandatory frame picker).
- Users can then compose structure by grouping/rearranging Tiles into Frames.
- Command palette remains the explicit power path for routing a node to a specific frame.

### 2.3 Workbench Bar Controls

- Workbench bar exposes explicit `Create Tile` and `Create Frame` actions.
- Drag interactions:
  - Tile → tile handle: group into same Frame.
  - Tile → tile viewport: enter split-arrangement targeting UI.
  - Tile between tile selectors: reorder selectors.

### 2.4 Split Orientation + Drop-Zone Semantics

- Vertical divider produces left/right regions.
- Horizontal divider produces top/bottom regions.
- Viewport drop-zones must preview target outcome before commit:
  - left/right edge zone → vertical divider split,
  - top/bottom edge zone → horizontal divider split,
  - center zone → group/stack in same frame branch.
- Invalid drops (unsupported target/surface) must cancel with explicit visual feedback.

---

## 3) Node-to-Frame Routing Rules

When opening a node through explicit routing actions:

1. If node exists in exactly one Frame, open/focus there.
2. If node exists in multiple Frames, default to last-active Frame.
3. Provide explicit override in command palette to select another Frame.
4. If node has no existing Frame membership, create a new Frame seeded with that node Tile.

Duplicate/open policy:

- If target frame already contains the node tile, focus that existing tile by default (do not duplicate silently).
- Explicit duplicate action may create another tile for the same node only through command palette/explicit command.

Open feedback:

- If default routing opens in a non-active frame, show a jump affordance that identifies the destination frame.
- If routing falls back (no membership), show a creation notice for the new frame/tile destination.

Closing a Tile/Frame never deletes node identity from the graph.

### 3.1 Empty-State and Close Semantics

- Empty frame state must remain visible and actionable (create tile, open node, or close frame).
- Closing the last tile in a frame must not delete graph nodes.
- Closing/removing a frame must require deterministic handoff to next frame context (or empty workbench state) with no focus ambiguity.

---

## 4) Graph Meaning vs Workbench Arrangement

1. Edges and Traversals are graph truth (global within a `GraphId`).
2. Frames/Tiles are arrangement truth (workbench-local context).
3. Connected nodes are not required to co-exist in one Frame.
4. Node can appear in multiple Frames without duplicating graph identity.

---

## 5) History Model (Dual-Stream)

Frame history is derived from two streams:

- **Traversal Stream**: edge-attached navigation events (graph semantics).
- **Workbench Stream**: tile/frame/split/reorder/open/close operations (arrangement semantics).

Frame history view is a merged timeline over both streams.

Merge requirements:

- Traversal events carry optional origin/destination frame context metadata.
- Workbench events carry structural operation type and affected tile/frame IDs.
- Merged ordering uses monotonic event time plus deterministic tie-breaker policy.

---

## 6) Undo/Redo Interaction Contract

Two scopes are required:

1. **Back / Forward**: tile navigation history (traversal-driven).
2. **Undo / Redo**: workbench structural edits (global within active workbench context).

`Ctrl+Z` interaction contract:

- Hold `Ctrl+Z`: show undo preview indicator (non-committed).
- While holding `Ctrl`, release `Z`: commit one undo step.
- Releasing `Ctrl+Z` without releasing `Z` commit path cancels preview.
- Holding `Ctrl` and re-pressing `Z` steps preview backward one additional operation per repeat.

Implementation note: if key event limitations require adaptation, preserve the same user-visible semantics through nearest equivalent interaction.

---

## 7) Frame Identity

- Each Frame has a stable identity (`Frame N`) and editable display label.
- Auto-suggest labels may be derived from member tiles, but user label is authoritative.
- Workbench bar should show order + active frame + compact context metadata (e.g., tile count).

---

## 8) Accessibility + Performance Requirements

Accessibility:

- Every drag operation must have keyboard-equivalent commands.
- Selection and activation state changes must be clearly announced and visible.
- History previews and undo scope indicators must be keyboard-accessible.
- Keyboard-only users must be able to: create tile/frame, group tiles into frames, split by orientation, reorder tiles, and switch frames.

Performance:

- Frame switches and tile open should remain responsive under quick smoke workloads.
- Inactive tile content may degrade (`Warm/Cold`) with explicit user-visible state.
- Degradation and fallback reasons must be observable in diagnostics.

---

## 9) Acceptance Criteria (Spec Parity)

1. Terminology docs and active strategy docs use `Tile` as canonical term (tab as UI affordance only).
2. Node double-click follows open-tile-first behavior.
3. Workbench bar includes explicit create actions for tile/frame.
4. Frame grouping/reorder/split drag semantics are implemented and test-covered.
5. Merged history view (Traversal + Workbench streams) is represented in design and diagnostics.
6. Undo preview interaction contract is documented and exposed in keybinding/help surfaces.
7. Accessibility parity exists for non-pointer users.
8. Split-drop previews and invalid-drop feedback are deterministic and test-covered.
9. Duplicate-tile policy and open-feedback behavior are deterministic and test-covered.
