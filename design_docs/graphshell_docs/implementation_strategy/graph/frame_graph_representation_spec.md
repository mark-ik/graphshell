# Frame Graph Representation: Spatial Minimap Node

**Date**: 2026-03-15
**Status**: Design — Pre-Implementation
**Purpose**: Define how Frames and Tiles are visually represented on the
graph canvas, establishing the rule that a Frame renders as a spatial minimap
bounding box where each member node is positioned to reflect its tile-center
coordinates within the frame's viewport layout.

**Related**:

- `../canvas/2026-03-14_graph_relation_families.md` — §2.4 Arrangement family,
  `frame-member` and `tile-member` sub-kinds
- `../workbench/workbench_frame_tile_interaction_spec.md` — §2.2 canonical
  hierarchy; Frame and Tile structure
- `../workbench/navigator_graph_isomorphism_spec.md` — interaction symmetry
  between canvas and Navigator
- `../subsystem_ux_semantics/2026-03-13_chrome_scope_split_plan.md` — §5.2
  frame chip and tile chip; §7 WorkbenchLayerState
- `../../TERMINOLOGY.md` — `Frame`, `Tile`, `ArrangementRelation`

---

## 1. The Core Principle

A Frame is a named, saved arrangement of tiles. Its spatial layout — which tile
is where, how wide, how tall — is real information that is otherwise invisible
on the graph canvas.

**Rule**: A Frame is represented on the graph canvas as a **bounding box
minimap**: a filled rectangle, proportionally scaled to the frame's viewport
aspect ratio, with each member node positioned inside it at the relative
coordinates of its tile's center within the frame layout.

This makes the frame's tile arrangement self-documenting in the graph. A user
can glance at a frame node and know: "this frame has two nodes side by side" or
"this frame has three nodes stacked vertically."

**Model boundary**: repeated contextual appearances are represented by copied
nodes, not shared presentation-instances of one node. A frame therefore
contains node-bearing tiles/nodes. Bare panes with no container-backed
node/tile representation are out of scope for this visualization and do not
appear in the Navigator.

**Arrangement-object rule**: Frames and tile-backed graphlets are arrangement
objects on the canvas. They may expand into minimap/cluster form or contract
into node-sized arrangement objects, and that expand/contract state remains
available even when the current `EdgePolicy` hides arrangement edges.

---

## 2. Frame Bounding Box

### 2.1 Shape and Size

The frame bounding box is rendered as a filled rectangle with a solid-color
background and a subtle border.

- **Aspect ratio**: matches the frame's recorded viewport aspect ratio at the
  time the frame was last saved. If the viewport aspect ratio is unknown (e.g.,
  a frame saved before this spec was implemented), default to 16:9.
- **Size on canvas**: fixed display size in canvas units, independent of the
  number of member nodes. The box is sized to be visually distinct from
  individual nodes but not so large that it dominates the graph. Suggested
  default: approximately 4× the width of a standard node icon, adjustable by
  the force-directed layout engine's node-size policy.
- **Color**: a distinct per-frame solid fill, drawn from the same palette used
  for frame tabs in the workbench-scoped Navigator host. The color is stable per frame
  identity (`FrameId`).

### 2.2 Label

The frame box displays its user-defined label (or auto-generated name if no
label has been assigned) centered below the bounding box, not inside it. The
label is always legible regardless of zoom.

### 2.3 Border

The border renders as a slightly darker shade of the fill color. No drop shadow.
The border thickness is constant in screen pixels (not canvas units), so it
remains visible at all zoom levels.

---

## 3. Member Node Positioning Within the Box

### 3.1 Position Derivation

For each member node in the frame, its position *within the bounding box* is
derived from the tile-center coordinates of its corresponding tile in the
frame's recorded layout:

```
node_position_in_box = (
    tile_center_x / frame_viewport_width,
    tile_center_y / frame_viewport_height
)
```

This is a relative value in [0, 1] × [0, 1] where (0, 0) is the top-left of
the frame box and (1, 1) is the bottom-right.

**Example — 50/50 horizontal split**:

If the frame has two tiles in a side-by-side horizontal split:
- Left tile center: x = 0.25, y = 0.5 → positioned at the left-center of the box
- Right tile center: x = 0.75, y = 0.5 → positioned at the right-center of the box

```
┌────────────────────────┐
│                        │
│   ● (node A)  ● (node B) │
│                        │
└────────────────────────┘
         Frame A
```

**Example — three-pane layout (top-left, top-right, bottom-full)**:

```
┌────────────────────────┐
│   ● (A)    ● (B)       │
│                        │
│       ● (C)            │
└────────────────────────┘
         Frame B
```

### 3.2 Node Markers Inside the Box

Member nodes inside the frame box are rendered as compact markers (not full
node cards). Each marker:
- Is a small circle or icon (same icon as the node's favicon/type icon, scaled
  down).
- Has a hover tooltip showing the node's title.
- Is single-clickable to select that node on the graph (same semantics as
  §2.1 of `navigator_graph_isomorphism_spec.md`).
- Is double-clickable to navigate to that node's resolved presentation target
  (same semantics as §3.2).

The markers are positioned according to §3.1. If two markers would overlap
(e.g., in a very crowded frame), a small radial offset is applied to each to
prevent complete occlusion.

### 3.3 Position Recalculation

The marker positions inside the box are recomputed whenever the frame layout
changes (tile resize, split change, new tile added, tile removed). Position
recalculation is triggered by the same events that would cause a
`PanePresentationModeChanged` or tile-tree mutation event.

Frames that have not been saved (session-only arrangement) do not render this
bounding box on the canvas. Only saved frames (durable `frame-member`
`ArrangementRelation` edges, §2.4 of `graph_relation_families.md`) render as
bounding boxes.

---

## 4. Tile Representation

A Tile (especially a multi-node Tile) is a lighter-weight arrangement structure
than a Frame. It does not have a named identity or durable graph-backed edges by
default — it is session-only until promoted to a saved frame.

### 4.1 Canvas Representation

A Tile is **not** rendered as a full frame bounding box. Instead, its member nodes
are drawn with a **shared color accent** on the graph canvas — a colored ring
or halo around each member node using a group-specific color. This signals
"these nodes are grouped together" without implying the spatial arrangement
semantics that a Frame has.

**Rationale**: A Tile conveys shared container context (which nodes are tabbed
together in one tile) but not the richer saved layout semantics of a Frame. The
bounding-box
minimap would be misleading — all multi-node-tile members share the same viewport
region by definition (one tile shows at a time). A color ring correctly
communicates "these are grouped" without false spatial information.

### 4.2 Color Assignment

Group colors are assigned per `TileId` of the Tile container, drawn from
a small stable palette. Colors are not user-configurable in this spec (that is
future work).

---

## 5. Interaction Model on the Frame Box

The frame box is itself a canvas object and participates in the same
interaction model as individual nodes, with one extension:

| Interaction | Target | Result |
|-------------|--------|--------|
| Single click on frame box (not on a marker) | Frame box | Expands/reveals this frame's contents in the Navigator; workbench does not switch active frame |
| Double click on frame box (not on a marker) | Frame box | Switches active frame in the workbench to this frame; opens the workbench-scoped Navigator host if not visible |
| Single click on a marker inside frame box | Node marker | Selects that node (same as §2.1 of navigator isomorphism spec) |
| Double click on a marker inside frame box | Node marker | Navigates to that node's resolved presentation target (same as §3.2) |
| Right click on frame box | Frame box | Context menu: Rename, Delete Frame, Open Frame, Duplicate Frame |
| Drag frame box | Frame box | Moves the frame box on the canvas (repositions the graph cluster, same as dragging any node) |

**Single-click on frame box reveals structure but does not switch active
workbench frame.** This follows the same structural-row rule as the Navigator:
containers reserve single click for expansion/reveal, while navigation is a
double-click action.

---

## 6. Physics Behavior

The frame bounding box participates in the force-directed layout as a single
physics body. Its member nodes' positions inside the box are *fixed relative to
the box* — they move with the box, not independently on the canvas.

The box's position on the canvas is influenced by the same forces as individual
nodes:
- `ArrangementRelation` edges connecting the frame box to its member nodes
  (if the arrangement-family physics weight is non-zero) draw the box toward
  semantically related clusters.
- Repulsion from other nodes and boxes prevents overlap.

This means a saved frame appears as a coherent, movable cluster on the canvas,
not as a fragmented set of disconnected nodes.

---

## 7. Visibility Rule

The frame bounding box is **visible by default** on the canvas when the
`WorkbenchLayerState` is `WorkbenchActive` or `WorkbenchPinned`. In
`GraphOnly` mode, frames may be hidden or rendered at reduced opacity (a
"workbench overlay" lens — see `graph_relation_families.md §2.4`).

The user can explicitly enable or disable frame box rendering via a canvas
overlay lens. The default-on-when-workbench-active behavior is the correct
baseline: the frame boxes make the workbench arrangement legible in the graph
without requiring the user to open a separate view.

---

## 8. Acceptance Criteria

| Criterion | Verification |
|-----------|-------------|
| Saved frame renders as bounding box on canvas | Test: save a frame → bounding box visible on graph canvas |
| Bounding box aspect ratio matches frame viewport aspect ratio | Test: frame with 16:9 viewport → box is 16:9 proportioned |
| Member node markers positioned at tile-center relative coordinates | Test: 50/50 H split → left node marker at x≈0.25, right at x≈0.75, both at y≈0.5 |
| Marker hover shows node title | Test: hover marker → tooltip shows node title |
| Single click on marker selects node on graph | Test: click marker → graph selection truth updated, no navigation occurs |
| Double click on marker navigates to node presentation target | Test: double-click marker → workbench if node live, graph if cold |
| Single click on frame box reveals frame contents without switching workbench | Test: click frame box → frame contents revealed in Navigator; active workbench frame unchanged |
| Double click on frame box switches active workbench frame | Test: double-click frame box → workbench active frame switches to this frame |
| Session-only arrangements do not render bounding box | Test: unsaved tile arrangement → no bounding box on canvas |
| Tile renders as color ring accent, not bounding box | Test: open multi-node tile → member nodes have shared color halo; no bounding box |
| Frame box moves as single physics body | Test: drag frame box → member node markers move with it; relative positions preserved |
| Frame box hidden in GraphOnly state by default | Test: `WorkbenchLayerState::GraphOnly` → frame boxes not rendered (or reduced opacity) |
