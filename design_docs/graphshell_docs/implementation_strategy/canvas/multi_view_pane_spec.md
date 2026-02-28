# Multi-View Pane — Interaction Spec

**Date**: 2026-02-28
**Status**: Canonical interaction contract
**Priority**: Active

**Related**:

- `CANVAS.md`
- `graph_node_edge_interaction_spec.md`
- `2026-02-22_multi_graph_pane_plan.md`
- `../workbench/WORKBENCH.md`
- `../workbench/workbench_frame_tile_interaction_spec.md`
- `../workbench/pane_chrome_and_promotion_spec.md`
- `../viewer/viewer_presentation_and_fallback_spec.md`
- `../system/register/canvas_registry_spec.md`
- `../system/register/workbench_surface_registry_spec.md`
- `../../TERMINOLOGY.md` — `Graph View`, `GraphViewId`, `Scope Isolation`, `Graph Scope`

---

## 1. Scope

This spec defines the canonical contracts for:

1. **Pane as universal host** — panes host view payloads; graph views are one kind.
2. **Graph view identity** — `GraphViewId`, per-view camera, per-view lens.
3. **Canonical vs Divergent layout** — multi-pane layout semantics.
4. **Scope isolation** — interaction independence between sibling panes.
5. **Semantic tab overlay** — `FrameTabSemantics`, promote/demote, demotion repair.

---

## 2. Pane as Universal Host Contract

The workbench does not special-case graph panes vs. viewer panes vs. tool panes at the layout layer. A pane is a host; its **payload** (`TileKind`) determines rendering and input behavior.

```
TileKind =
  | Graph(GraphViewId)       -- graph canvas view
  | Node(NodePaneState)      -- node viewer pane
  | Tool(ToolPaneState)      -- tool/subsystem pane
```

**Invariant**: Workbench layout operations (split, move, close, reorder) apply uniformly across all `TileKind` variants. The workbench must not branch on payload type for structural operations.

---

## 3. Graph View Identity Contract

### 3.1 GraphViewId

Each graph view pane has a stable `GraphViewId`. `GraphViewId` is the canonical identity for:

- Per-view camera state (pan offset, zoom level).
- Per-view `ViewDimension` (TwoD / ThreeD).
- Per-view Lens assignment.
- Per-view `LocalSimulation` (physics state for Divergent views).

`GraphViewId` is generated at pane creation and persisted as part of the frame snapshot. It does not change when the pane is moved, split, or reordered.

### 3.2 Per-View Camera

Camera state is per-`GraphViewId`. Camera commands (`fit`, `fit selection`, zoom, pan) target the focused graph view. Camera state from one graph view must not bleed into a sibling graph view.

### 3.3 Per-View Lens

A Lens may be assigned per-`GraphViewId` or inherited from the workspace default. Per-view Lens assignment overrides the workspace default for that view only. Lens resolution follows: `View → Workspace → User → Default` fallback chain.

---

## 4. Canonical vs Divergent Layout Contract

Multiple graph view panes may open simultaneously. Each pane is classified as either **Canonical** or **Divergent**.

```
GraphLayoutMode =
  | Canonical    -- participates in the shared workspace graph layout
  | Divergent    -- has its own independent LocalSimulation
```

### 4.1 Canonical Mode

- The pane uses the shared workspace graph layout (single physics simulation, shared node positions).
- Multiple Canonical panes in the same workbench share `(x, y)` node positions.
- Camera is per-view (independent pan/zoom), but positions are shared.
- This is the default mode for new graph view panes.

### 4.2 Divergent Mode

- The pane has its own `LocalSimulation` with independent node positions.
- Divergent layout does not affect Canonical pane positions.
- Divergent panes may use a different `LayoutId` (algorithm) than the Canonical layout.
- Divergent mode is explicitly user-activated (not automatic).

**Scope isolation**: Selection, camera, and gestures in a Divergent pane do not affect sibling panes unless an explicit bridge/sync rule is enabled (see §5).

---

## 5. Scope Isolation Contract

Graph scopes rendered in separate panes within the same workbench are **interaction-isolated by default**.

| Interaction type | Default behavior |
|-----------------|-----------------|
| Selection | Independent per pane; selecting a node in pane A does not change selection in pane B |
| Camera | Independent per pane |
| Gestures (lasso, drag) | Local to the pane that receives the gesture |
| Physics state | Shared (Canonical) or isolated (Divergent) |

**Explicit sync**: Scope sync (e.g., "mirror selection across panes") requires an explicit bridge rule. Bridge rules are future work; this spec records that isolation is the default.

---

## 6. Semantic Tab Overlay Contract

### 6.1 Purpose

`FrameTabSemantics` is an optional overlay on top of the `egui_tiles` structural tree. It persists semantic tab group membership so that meaning is not lost when `egui_tiles` simplification (`simplify()`) restructures the tree.

**Invariant**: The `egui_tiles` workbench tree is structural state, not semantic truth. `FrameTabSemantics` is semantic truth.

### 6.2 Data Model

```
FrameTabSemantics {
    version: u32,
    tab_groups: Vec<TabGroupMetadata>,
}

TabGroupMetadata {
    group_id: TabGroupId,    -- Uuid
    pane_ids: Vec<PaneId>,   -- ordered tab membership
    active_pane_id: Option<PaneId>,   -- must be a member, or None after repair
}
```

- A pane belongs to at most one semantic tab group.
- Persistence: serialized with rkyv into the frame bundle (redb). This is frame state, not WAL data — it must not appear in `LogEntry` variants.

### 6.3 Promote / Demote Contract

**Promote** (pane rest state → tab container): pane is hoisted into a `Container::Tabs` node in the tile tree. Semantic metadata is created or updated.

**Demote** (tab container → pane rest state): pane is unhoisted; tab container is removed. Semantic metadata is retained (the tab group still exists in `FrameTabSemantics`; only the visual container is removed). The pane can be re-promoted without losing group membership.

**Invariant**: Promote and demote are explicit `GraphIntent` variants (e.g., `PromotePane`, `DemotePane`). They must not be ad hoc tree rewrites at UI callsites. The `render/*` layer captures the user event and routes it to `app.rs` as an intent; `app.rs` is the authority for the promotion decision; `desktop/*` applies the workbench tree mutation.

### 6.4 Simplification Repair

When `egui_tiles::simplify()` runs and removes a tab container that has semantic metadata:

1. The semantic metadata (`TabGroupMetadata`) is preserved.
2. The pane's rest state (pane-only representation without a tab container) is valid for all tab-aware features.
3. The `active_pane_id` is validated on restore: if the previously active pane is no longer a member, `active_pane_id` is repaired to `None`.

**Invariant**: Graphshell must remain compatible with `egui_tiles` simplification. No semantic data may be stored in the tile tree shape alone.

---

## 7. Acceptance Criteria

| Criterion | Verification |
|-----------|-------------|
| Camera in pane A does not affect pane B | Test: pan in pane A → pane B camera unchanged |
| Selection in pane A does not affect pane B (isolation) | Test: select node in pane A → pane B selection unchanged |
| Canonical panes share node positions | Test: move node in Canonical pane A → same node at new position in Canonical pane B |
| Divergent pane has independent positions | Test: move node in Divergent pane → Canonical pane positions unchanged |
| `GraphViewId` persists across reorder | Test: reorder pane → `GraphViewId` unchanged |
| Promote creates `Container::Tabs` node | Test: promote pane → tile tree contains `Container::Tabs` parent |
| Demote removes container but retains semantic metadata | Test: demote → `TabGroupMetadata` still present in `FrameTabSemantics` |
| `simplify()` does not lose tab group membership | Test: simplify removes tab container → `TabGroupMetadata` intact; re-promote restores group |
| `active_pane_id` repaired when member removed | Test: remove pane from group → `active_pane_id` set to `None` |
| Promote/demote routed through `GraphIntent` | Architecture invariant: no direct tile tree mutation from render callsites for promote/demote |
