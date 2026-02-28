# Pane Chrome and Promotion — Interaction Spec

**Date**: 2026-02-28
**Status**: Canonical interaction contract
**Priority**: Implementation-ready

**Related**:

- `WORKBENCH.md`
- `workbench_frame_tile_interaction_spec.md`
- `2026-02-22_workbench_tab_semantics_overlay_and_promotion_plan.md`
- `../../TERMINOLOGY.md` — `Tiled Pane`, `Docked Pane`, `Pane Presentation Mode`, `Tab Group`, `Tile`

---

## 1. Scope

This spec defines the canonical contracts for:

1. **Pane Presentation Mode** — the three chrome modes and their behavioral rules.
2. **Tab selector overlay** — when and how the tile-selection chrome renders.
3. **Promotion and demotion** — moving panes between Tiled and Docked presentation.
4. **Tab ordering and reorder** — drag-reorder semantics within a Tab Group.
5. **Pane locking** — preventing accidental reflow.

---

## 2. Pane Presentation Mode Contract

Every pane has a **Pane Presentation Mode** (also called **Pane Chrome Mode**) that controls its chrome, mobility, and locking behavior. This is distinct from the pane's content.

```
PanePresentationMode =
  | Tiled      -- full chrome; normal tile-tree mobility
  | Docked     -- reduced chrome; position-locked
  | Fullscreen -- content-only; all chrome hidden (future; not in current scope)
```

**Invariant**: `PanePresentationMode` is workbench-owned state. It does not affect graph content identity. Changing the mode of a pane must not mutate any graph data (nodes, edges, traversal history).

### 2.1 Tiled Pane

- Renders with **tile-selector chrome**: tab bar strip, split/close affordances, resize handles.
- Participates in all tile-tree mobility operations: split, move, reorder, close, promote to new frame.
- Normal drag-and-drop target and source.

### 2.2 Docked Pane

- Renders with **reduced chrome**: title bar only; split/move affordances hidden.
- **Position-locked**: drag-to-reorder is disabled. User cannot accidentally drag a docked pane out of its position.
- Docked panes are still closeable via their title-bar close button or keyboard shortcut.
- Docked panes are eligible for focus; focus behavior follows the Focus subsystem contract.
- A docked pane may be explicitly **promoted** back to Tiled by the user (see §4).

**Rationale**: Docked presentation reduces visual noise and accidental reflow for auxiliary surfaces (e.g., a persistent diagnostics panel, a side-by-side reference node).

---

## 3. Tab Selector Overlay Contract

### 3.1 Tab Bar Rendering

Each Tab Group container renders a tab bar strip (the **Workbar** for frames; per-container tab strips for nested Tab Groups). The tab strip:

- Shows one tab entry per child tile with: title, badge strip (compact, per `node_badge_and_tagging_spec.md §3.5`), close button.
- Active tile's tab is highlighted.
- Tab bar is scrollable horizontally if tab count overflows available width.

### 3.2 Tab Overlay on Hover

When the cursor hovers over a non-active pane within a Tab Group that has multiple children, a **tile-selection affordance** is shown:

- A hover ring or highlight border on the pane boundary.
- The tab bar scrolls to make the hovered pane's tab entry visible.
- Clicking the pane body (not an interactive element within it) activates that pane.

**Invariant**: The hover affordance must not interfere with content interaction within the pane. Pointer events must be forwarded to the pane content when hovering over content areas.

### 3.3 Active Tab Indicator

The active tab renders a distinct visual indicator (accent underline or fill) that is visible at all zoom levels and in reduced-motion mode. The indicator must not rely on animation alone to convey active state.

---

## 4. Promotion and Demotion Contract

### 4.1 Promotion: Docked → Tiled

Triggers:
- Right-click on a docked pane's title bar → context menu "Promote to Tile"
- Keyboard shortcut (configurable; default unbound)
- Command palette: "Promote pane"

Effect:
1. Pane `PanePresentationMode` changes to `Tiled`.
2. Pane is inserted into the tile tree at its current position (it was already in the tree; only chrome mode changes).
3. Full tile-selector chrome is restored.
4. Workbench emits a `PanePromoted` signal for observability.

### 4.2 Demotion: Tiled → Docked

Triggers:
- Right-click on a tab entry → context menu "Dock pane"
- Keyboard shortcut (configurable; default unbound)
- Command palette: "Dock pane"

Effect:
1. Pane `PanePresentationMode` changes to `Docked`.
2. Position in tile tree is preserved (pane is not moved; only chrome mode changes).
3. Reduced chrome is applied; drag affordances are hidden.
4. Workbench emits a `PaneDocked` signal for observability.

**Invariant**: Promotion and demotion never move or remove the pane from the tile tree. They only change the presentation mode. Content and graph state are unaffected.

---

## 5. Tab Reorder Contract

Within a Tab Group, tabs may be reordered by drag-and-drop.

### 5.1 Drag Semantics

- Drag target: the tab entry in the tab strip (not the pane body).
- Drop target: any position in the same tab strip (reorder within the same Tab Group).
- Cross-Tab-Group drag: drops a tab into a different Tab Group (moves the tile, not just reorders).

**Invariant**: Tab reorder within a Tab Group only changes `Vec<TileId>` ordering in the container. It does not change tile tree depth or split geometry. No graph data is affected.

### 5.2 Docked Panes and Reorder

Docked panes are **not draggable** by the user. The drag affordance is hidden in docked chrome. Programmatic reorder (via intent) is still possible; only the user-interactive drag is blocked.

### 5.3 Dropped Tab Feedback

When a drag completes successfully, the tab animates to its new position (120 ms ease-out; respects `prefers-reduced-motion`). If the drag is cancelled (Esc or released outside a valid drop zone), the tab returns to its original position.

---

## 6. Pane Locking Contract

A pane may be **locked** to prevent all user-initiated reflow operations while preserving focus and interaction.

```
PaneLock =
  | Unlocked
  | PositionLocked    -- cannot be moved/reordered; can be closed
  | FullyLocked       -- cannot be moved, reordered, or closed by user
```

- `Docked` panes are implicitly `PositionLocked` from the user's perspective; their lock state is separate from `PanePresentationMode`.
- `FullyLocked` is reserved for system-owned panes (e.g., a required diagnostics pane during a critical operation). It is not user-assignable.
- Lock state is workbench-owned; it does not affect graph content.

---

## 7. Acceptance Criteria

| Criterion | Verification |
|-----------|-------------|
| Docked pane hides split/move affordances | Test: mode = `Docked` → split and drag handles not rendered |
| Docked pane is closeable | Test: mode = `Docked` → close button present and functional |
| Promote/demote does not move pane in tile tree | Test: promote docked pane → pane `TileId` remains at same tree position |
| Promote/demote does not affect graph data | Test: promote/demote → no `GraphIntent` mutations in intent log |
| Tab reorder changes `Vec<TileId>` order only | Test: drag tab to new position → only container child order changed; no depth change |
| Docked pane is not user-draggable | Test: mode = `Docked` → drag attempt has no effect |
| Active tab indicator visible without animation | Test: `prefers-reduced-motion` set → active tab indicator renders distinctly |
| Cross-Tab-Group drop moves tile | Test: drag tab to different Tab Group → tile moves to new container |
| `PanePromoted` signal emitted on promotion | Test: promote → `PanePromoted` signal present in signal log |
