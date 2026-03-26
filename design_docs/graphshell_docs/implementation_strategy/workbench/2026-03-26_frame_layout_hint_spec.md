# Frame Layout Hint — Spec

**Date**: 2026-03-26
**Status**: Design — Pre-Implementation
**Lane**: `lane:layout-semantics` (`#99`)

**Related**:

- `graph_first_frame_semantics_spec.md` — canonical Frame lifecycle,
  `CloseFrameHandle` vs `DeleteFrame`, `OpenFrameHandle`
- `workbench_frame_tile_interaction_spec.md` — tile/frame interaction
  contracts, split/dock semantics
- `../canvas/frame_graph_representation_spec.md` — frame as graph-canvas
  bounding-box minimap; member node positioning
- `../canvas/layout_behaviors_and_physics_spec.md §4` — frame-affinity force,
  backdrop rendering
- `../canvas/2026-03-14_graph_relation_families.md §2.4` — `ArrangementRelation`
  edge family; `FrameMember`, `TileGroup`, `SplitPair` sub-kinds
- `../canvas/graph_node_edge_interaction_spec.md` — node selection, double-click
  navigation contract
- `../workbench/navigator_graph_isomorphism_spec.md` — Navigator ↔ graph
  selection coherence
- `../../TERMINOLOGY.md`

---

## 1. Purpose and Scope

This spec defines **Frame Layout Hints** — a durable annotation on graph frames
that records how a frame's members are arranged as splits in the workbench, and
how the workbench surfaces that arrangement back to the user when they next
interact with the frame or any of its members.

It also defines the **frame → tile group materialization contract**: opening a
frame opens its members as a named tile group, with any recorded split
arrangements as tabs within that group.

This spec does not redefine Frame identity or lifecycle (see
`graph_first_frame_semantics_spec.md`). It extends the frame data model
with layout-hint annotations and defines the interaction surfaces that read and
write those annotations.

---

## 2. Core Analogy

The frame ↔ tile-group relationship is the group-level analog of the existing
node ↔ tile relationship:

```
node   →  tile        (graph node gets a workbench viewport pane)
frame  →  tile group  (graph frame gets a workbench tiled arrangement)
```

A frame is to a tile group what a node is to a tile: graph-first identity,
workbench-first presentation. Closing the tile group does not close the frame.
Closing the tile does not close the node. The graph object is the truth; the
workbench object is the handle.

---

## 3. Data Model

### 3.1 FrameLayoutHint

A `FrameLayoutHint` records a split arrangement over a subset of a frame's
members. A frame may carry zero, one, or many layout hints — each hint
corresponds to one tab in the frame's tile group.

```rust
pub enum FrameLayoutHint {
    /// Two members arranged side by side. Left/right are node keys within
    /// the frame. Either may be None if the member was opted out.
    SplitVertical { left: NodeKey, right: NodeKey },

    /// Two members arranged top-to-bottom.
    SplitHorizontal { top: NodeKey, bottom: NodeKey },

    /// Four members in a 2×2 grid. Any quadrant may be None (opt-out).
    SplitQuartered {
        top_left:     NodeKey,
        top_right:    NodeKey,
        bottom_left:  NodeKey,
        bottom_right: NodeKey,
    },

    /// One member occupies a fixed corner overlay (25 % of viewport).
    /// Typically used as a reference/context pane alongside a primary tile.
    Corner {
        anchor:        CornerAnchor,
        member:        NodeKey,
        size_fraction: f32,   // clamped [0.15, 0.40]; default 0.25
    },
}

pub enum CornerAnchor {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}
```

### 3.2 Extended GraphFrame

```rust
pub struct GraphFrame {
    // ... existing fields ...
    pub layout_hints: Vec<FrameLayoutHint>,
    /// When true, the workbench will not offer split-arrangement promotion
    /// for this frame. Persisted on the frame; toggled via frame settings.
    pub split_offer_suppressed: bool,
}
```

`layout_hints` is durable — it persists in graph scope alongside frame identity
and membership. `split_offer_suppressed` is also durable and frame-specific.

### 3.3 ArrangementSubKind additions

`SplitPair` already exists as a session-durability sub-kind. Layout hints are
richer and durable; they are stored directly on `GraphFrame.layout_hints` rather
than as additional edge sub-kinds. The existing `FrameMember` edges remain the
canonical membership authority; hints are metadata *on the frame*, not additional
edges.

---

## 4. Frame → Tile Group Materialization

### 4.1 Default open behavior

Opening a frame (double-click the frame backdrop on the graph canvas, or
activating it via the Navigator) opens a **tile group** in the workbench
containing all of the frame's members. This is the default; opening a single
member tile from a frame is the exception (§6.2).

If the tile group is already open, the workbench focuses it (switches to it,
brings it forward) rather than opening a second instance. Frame identity is 1:1
with tile group identity, matching the node ↔ tile cardinality rule.

### 4.2 Tabs within the tile group

Each `FrameLayoutHint` in `GraphFrame.layout_hints` corresponds to one tab in
the tile group:

- A frame with no layout hints opens as a plain tile group — one tab per
  member, unsplit.
- A frame with one `SplitVertical` hint opens with one tab showing a
  vertical split of the two named members, plus additional tabs for any
  members not referenced by a hint.
- A frame with multiple hints opens with one tab per hint, plus spillover
  tabs for uncovered members.

Tab order follows the order of `layout_hints`; uncovered-member tabs are
appended last, sorted by membership order.

### 4.3 Recording layout hints

Layout hints are recorded when the user arranges tiles within an open frame
tile group into splits:

1. User opens a frame tile group (plain tiles, no hints yet).
2. User drags tile A and tile B into a vertical split within the tab.
3. The workbench emits `RecordFrameLayoutHint { frame_id, hint: SplitVertical { left: A, right: B } }`.
4. The intent is applied: `GraphFrame.layout_hints` gains the new entry.
5. Next time the frame is opened, that split tab is present.

Hints are **additive**: each new split arrangement appends to `layout_hints`.
The user can reorder or delete tabs (hints) via the tile group tab strip.

Removing a tab emits `RemoveFrameLayoutHint { frame_id, hint_index }`.

### 4.4 Opt-out of a member from a split

Within the split-arrangement UI, any member slot can be vacated:

- Right-click a tile in the split → "Remove from this split" → the slot
  becomes empty in the hint (stored as None for that position in
  `SplitQuartered`; removing from a `SplitVertical` or `SplitHorizontal`
  collapses it to a plain tile tab).
- The member remains in the frame; it is not removed from frame membership.
- The hint is updated: `RecordFrameLayoutHint` with the updated hint replaces
  the prior entry.

---

## 5. Interaction Contract: Graph Canvas

### 5.1 Frame interaction surfaces

| Gesture | Target | Result |
|---------|--------|--------|
| Single click | Frame backdrop | Select frame |
| Double click | Frame backdrop | Open tile group (or focus if open) |
| Single click | Node within frame | Select node (unchanged from unframed behavior) |
| Double click | Node within frame | Open tile group, focus that node's tile |
| Right click | Frame backdrop | Context menu: rename, settings, suppress split offer, delete |
| Right click | Node within frame | Context menu includes "Open this tile only" |

### 5.2 Frame selection → Navigator coherence

Selecting a frame on the graph canvas:

- **Highlights** the corresponding tile group in the Navigator, if the tile
  group is open and the workbench scope is visible.
- Does not select the tile group (highlight ≠ select; no focus transfer).

Focusing a tile within an open tile group (workbench-initiated):

- **Highlights** the frame backdrop on the graph canvas.
- Does not select the frame (highlight ≠ select; no graph focus transfer).

This is the group-level analog of the existing node ↔ tile highlight coherence:

```
Select node      → highlight tile in Navigator   (if tile is open)
Select frame     → highlight tile group in Navigator (if group is open)
Focus tile       → highlight node on graph
Focus tile group → highlight frame backdrop on graph
```

Selected state requires explicit user gesture (single click). Highlighted state
is derived from workbench/graph coherence and does not require user gesture.

### 5.3 Frame backdrop as hit target

The frame backdrop (frame-affinity region rendered below member nodes) acts as
an independent hit target for single-click and double-click. It does not
intercept clicks on member nodes — node hit testing takes priority when the
pointer is over a node within the frame area.

---

## 6. Interaction Contract: Workbench

### 6.1 Tile group header

The tile group header displays the frame's label and color token. It is distinct
from a regular tab group in that it has a frame identity chip (analogous to the
node chip in a single tile header) that routes back to the graph canvas frame.

### 6.2 Opening a single member tile

When the user wants to open one member without the full tile group:

- Right-click node in graph → "Open this tile only" → opens a single tile
  for that node, outside the frame's tile group.
- The tile is not a member of the frame's tile group for this session.
- Frame membership in the graph is unchanged.

### 6.3 Closing the tile group

`Ctrl+W` on the tile group (or frame header close button) →
`CloseFrameHandle(frame_id)` — non-destructive. Frame identity, membership,
and layout hints are preserved. Next open recreates the tile group from the
frame's current state.

---

## 7. Split Offer Lifecycle

When a user navigates to a node that has frame membership where the frame
carries one or more `FrameLayoutHint`s, the workbench may offer to open the
frame's tile group with the split arrangement active.

### 7.1 Offer states

| State | Trigger | Persistence |
|-------|---------|-------------|
| **Offered** | Navigation to framed node; frame has layout hints | None |
| **Dismissed (this time)** | User dismisses the offer affordance | None — offer reappears next navigation |
| **Dismissed for session** | User selects "not this session" option on the affordance | Session state; expires at session end |
| **Suppressed for frame** | User selects "never for this frame" or toggles in frame settings | Durable; persisted on `GraphFrame.split_offer_suppressed` |

Session-dismiss and frame-suppress are independently toggleable. Dismissing for
session does not suppress; suppressing does not affect the current session's
other frames.

### 7.2 Recovering a suppressed offer

`split_offer_suppressed` is a field on the frame. The canonical recovery surface
is **frame settings**, reached via:

- Right-click frame backdrop on graph → "Frame settings"
- Tile group header → frame identity chip → "Frame settings"

Frame settings shows: label, color, layout hints (listed, reorderable, deletable),
and "Re-enable split offer" toggle when suppressed.

Session-dismiss expires automatically; no recovery surface is needed.

### 7.3 Offer affordance form

The offer is non-intrusive — a transient affordance attached to the tile header
or workbench chrome, not a modal. It presents:

- Frame label + split type icon
- "Open as split" (primary action)
- "Not this session" (secondary)
- "Never for this frame" (tertiary, reached via overflow/disclosure)

---

## 8. Natural Member Count Cap

The geometry of split arrangements provides a natural cap on frame size without
requiring policy enforcement:

| Split type | Maximum meaningful members |
|------------|---------------------------|
| SplitVertical / SplitHorizontal | 2 |
| SplitQuartered | 4 |
| Corner | 2 (primary + corner overlay) |

Frames with more members than a given split type accommodates simply have
uncovered members in plain tile tabs. The workbench does not refuse to open
large frames; it just stops offering split promotion once no split type fits
the current member count. The UX pressure to keep frames small emerges from
usability rather than a hard limit.

---

## 9. Graph Canvas Representation Update

The frame bounding-box minimap (defined in `frame_graph_representation_spec.md`)
gains a **split-type indicator** when the frame carries layout hints:

- A small icon in the frame backdrop header area shows the dominant split type
  (vertical bar, horizontal bar, quad grid, corner-pip).
- When multiple hints exist, the icon shows the count of split tabs ("2 splits").
- When no hints exist, no split indicator is shown.

This makes the frame's workbench arrangement self-documenting on the graph
canvas, consistent with the existing rule that frame geometry is visible from
the graph.

---

## 10. Implementation Stages

### Stage 1 — Data model (immediate prerequisite)

- Add `layout_hints: Vec<FrameLayoutHint>` and `split_offer_suppressed: bool`
  to `GraphFrame`.
- Add `FrameLayoutHint` enum.
- Add WAL `LogEntry` variants: `RecordFrameLayoutHint`, `RemoveFrameLayoutHint`,
  `SetFrameSplitOfferSuppressed`.
- WAL replay handlers for all three.
- Snapshot round-trip coverage.

**Done gate**: a frame survives restart with its layout hints intact.

### Stage 2 — Tile group materialization

- `OpenFrameHandle` opens all members as a tile group.
- Tile group focuses existing group if already open (1:1 cardinality enforced).
- Double-click node within frame → open tile group, focus that node's tile.
- Right-click node → "Open this tile only" option.

**Done gate**: frame double-click opens tile group; node double-click opens tile
group with focus on that node; 1:1 cardinality holds.

### Stage 3 — Split recording

- Drag-to-split within a frame tile group emits `RecordFrameLayoutHint`.
- Tabs per hint are rendered in the tile group tab strip.
- `RemoveFrameLayoutHint` on tab close.

**Done gate**: split arrangements persist across frame close/reopen.

### Stage 4 — Selection coherence

- Select frame → highlight tile group in Navigator.
- Focus tile group → highlight frame backdrop on graph canvas.
- Bidirectional highlight, not selection transfer.

**Done gate**: frame↔tile-group highlight coherence matches existing
node↔tile highlight coherence.

### Stage 5 — Split offer and suppression

- Offer affordance appears on navigation to framed node with layout hints.
- Dismiss / session-dismiss / frame-suppress states wired.
- Frame settings panel exposes suppression toggle and hint management.

**Done gate**: suppression survives restart; session-dismiss expires; offer
affordance is non-modal and keyboard-accessible.

### Stage 6 — Graph canvas split indicator

- Split-type icon / count indicator in frame backdrop header.
- Driven directly from `GraphFrame.layout_hints`.

**Done gate**: a frame with layout hints shows a split indicator on the canvas;
a frame without hints shows none.

---

## 11. Acceptance Criteria

| Criterion | Verification |
|-----------|-------------|
| Frame → tile group is 1:1 | Test: open frame twice → second open focuses first, no duplicate group |
| Node double-click in frame opens tile group with focus | Test: double-click member → tile group open, that member's tile focused |
| Layout hints survive snapshot roundtrip | Test: record hint, restart, verify hint present |
| Session-dismiss does not suppress | Test: dismiss for session, new session starts, offer reappears |
| Frame suppress persists | Test: suppress, restart, offer does not appear |
| Recovery via frame settings | Test: suppress → open frame settings → toggle re-enables offer |
| Select frame → Navigator highlight | Test: select frame backdrop → tile group highlighted in Navigator (if open) |
| Focus tile group → graph highlight | Test: focus tile group → frame backdrop highlighted on canvas |
| Natural cap — no hard limit | Test: frame with 6 members opens without error; split offer not shown |
| Opt-out member remains in frame | Test: remove from split → member still in frame membership, not in hint slot |
| Split indicator on canvas | Test: frame with SplitVertical hint → vertical-bar icon visible in backdrop |
