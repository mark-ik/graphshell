# Pane Chrome and Opening Semantics — Interaction Spec

**Date**: 2026-02-28
**Status**: Canonical interaction contract
**Priority**: Implementation-ready

**Related**:

- `WORKBENCH.md`
- `workbench_frame_tile_interaction_spec.md`
- `pane_presentation_and_locking_spec.md` — **canonical authority for `PaneLock`** (§7 here defers to it)
- `2026-03-03_pane_opening_mode_and_simplification_suppressed_plan.md` — canonical authority for `PaneOpeningMode` and `SimplificationSuppressed`
- `2026-02-22_workbench_tab_semantics_overlay_and_promotion_plan.md` — `FrameTabSemantics` overlay plan (§7 canonicalized there)
- `../canvas/node_badge_and_tagging_spec.md` — badge strip rendering contract referenced in §3.1
- `../../../TERMINOLOGY.md` — `Tiled Pane`, `Docked Pane`, `Pane Presentation Mode`, `Tab Group`, `Tile`

---

## 1. Scope

This spec defines the canonical contracts for:

1. **Pane Presentation Mode** — the three chrome modes and their behavioral rules.
2. **Tab selector overlay** — when and how the tile-selection chrome renders.
3. **Pane opening mode boundary** — where graph-citizenship decisions stop and chrome behavior begins.
4. **Presentation-mode transitions** — moving panes between Tiled and Docked presentation.
5. **Tab ordering and reorder** — drag-reorder semantics within a Tab Group.
6. **Pane locking** — preventing accidental reflow.

---

## 2. Pane Presentation Mode Contract

Every pane has a **Pane Presentation Mode** (also called **Pane Chrome Mode**) that controls its chrome, mobility, and locking behavior. This is distinct from both the pane's content and the pane's **Pane Opening Mode** (the graph-citizenship decision that determines whether the pane exists only as an ephemeral open surface or as a graph-backed tile).

```
PanePresentationMode =
  | Tiled      -- full chrome; normal tile-tree mobility
  | Docked     -- reduced chrome; position-locked
  | Fullscreen -- content-only; all chrome hidden (future; not in current scope)
```

**Invariant**: `PanePresentationMode` is workbench-owned state. It does not affect graph content identity. Changing the mode of a pane must not create or delete graph nodes, write addresses, append traversal history, or otherwise mutate graph data.

### 2.1 Tiled Pane

- Renders with **tile-selector chrome**: tab bar strip, split/close affordances, resize handles.
- Participates in all tile-tree mobility operations: split, move, reorder, close, open into a separate frame.
- Normal drag-and-drop target and source.

### 2.2 Docked Pane

- Renders with **reduced chrome**: title bar only; split/move affordances hidden.
- **Position-locked**: drag-to-reorder is disabled. User cannot accidentally drag a docked pane out of its position.
- Docked panes are still closeable via their title-bar close button or keyboard shortcut.
- Docked panes are eligible for focus; focus behavior follows the Focus subsystem contract.
- A docked pane may be explicitly restored to `Tiled` presentation by the user (see §5).

**Rationale**: Docked presentation reduces visual noise and accidental reflow for auxiliary surfaces (e.g., a persistent diagnostics panel, a side-by-side reference node).

---

## 3. Tab Selector Overlay Contract

### 3.1 Tab Bar Rendering

Each Tab Group container renders a tab bar strip (the **Workbar** for frames; per-container tab strips for nested Tab Groups). The tab strip:

- Shows one tab entry per child tile with: title, badge strip (compact, per `../canvas/node_badge_and_tagging_spec.md §3.5`), close button.
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

## 4. Pane Opening Mode Boundary

This spec does not define the full Pane Opening Mode model. It defines the boundary between opening semantics and chrome semantics so they do not get conflated.

Canonical boundary:

- **Pane Opening Mode** decides whether opening content creates graph citizenship.
- **Pane Presentation Mode** decides how an already-open pane renders and how much tile chrome it exposes.

Rules:

1. Opening a pane in an ephemeral mode may create a visible pane without writing any graph node.
2. Creating graph citizenship (for example, writing a pane address into the graph and turning it into a graph-backed tile) is an opening-mode concern, not a chrome-mode concern.
3. Once a pane is already graph-backed, switching between `Tiled` and `Docked` changes only presentation and lock affordances.
4. Internal surfaces that are graph-backed at creation time (for example `verso://tool/*`, `verso://view/*`, `verso://frame/<FrameId>`) are already across the opening-mode boundary before this spec applies.

Compatibility note:

- Older docs may still use `graphshell://...` for these same internal surfaces.
- Treat `graphshell://...` as the legacy alias; runtime canonical formatting is `verso://...`.

This separation is required by the address-as-identity model in `TERMINOLOGY.md`: graph citizenship follows address write and node existence, not the presence or absence of tab chrome.

---

## 5. Presentation-Mode Transition Contract

### 5.1 Restore Full Tile Chrome: Docked -> Tiled

Triggers:
- Right-click on a docked pane's title bar -> context menu "Show Tile Chrome"
- Keyboard shortcut (configurable; default unbound)
- Command palette: "Show Tile Chrome"

Effect:
1. Pane `PanePresentationMode` changes to `Tiled`.
2. Pane is inserted into the tile tree at its current position (it was already in the tree; only chrome mode changes).
3. Full tile-selector chrome is restored.
4. Workbench emits a `PanePresentationModeChanged` signal for observability.

### 5.2 Reduce Chrome: Tiled -> Docked

Triggers:
- Right-click on a tab entry -> context menu "Dock pane"
- Keyboard shortcut (configurable; default unbound)
- Command palette: "Dock pane"

Effect:
1. Pane `PanePresentationMode` changes to `Docked`.
2. Position in tile tree is preserved (pane is not moved; only chrome mode changes).
3. Reduced chrome is applied; drag affordances are hidden.
4. Workbench emits a `PanePresentationModeChanged` signal for observability.

**Invariant**: Presentation-mode transitions never move or remove the pane from the tile tree. They only change the presentation mode. Content and graph state are unaffected.

---

## 6. Tab Reorder Contract

Within a Tab Group, tabs may be reordered by drag-and-drop.

### 6.1 Drag Semantics

- Drag target: the tab entry in the tab strip (not the pane body).
- Drop target: any position in the same tab strip (reorder within the same Tab Group).
- Cross-Tab-Group drag: drops a tab into a different Tab Group (moves the tile, not just reorders).

**Invariant**: Tab reorder within a Tab Group only changes `Vec<TileId>` ordering in the container. It does not change tile tree depth or split geometry. No graph data is affected.

### 6.2 Docked Panes and Reorder

Docked panes are **not draggable** by the user. The drag affordance is hidden in docked chrome. Programmatic reorder (via intent) is still possible; only the user-interactive drag is blocked.

### 6.3 Dropped Tab Feedback

When a drag completes successfully, the tab animates to its new position (120 ms ease-out; respects `prefers-reduced-motion`). If the drag is cancelled (Esc or released outside a valid drop zone), the tab returns to its original position.

---

## 7. Pane Locking Contract

> **Canonical authority**: `pane_presentation_and_locking_spec.md` owns the full `PaneLock` contract, invariants, diagnostics channel table, and test requirements. This section is a cross-reference summary only.

A pane may be **locked** to prevent user-initiated reflow operations while preserving focus and content interaction.

```text
PaneLock =
  | Unlocked
  | PositionLocked    -- cannot be moved/reordered; can be closed
  | FullyLocked       -- cannot be moved, reordered, or closed by user
```

Key rules (full contract in `pane_presentation_and_locking_spec.md §3`):

- `Docked` panes are implicitly `PositionLocked` from the user's perspective; their `PaneLock` field is nonetheless separate from `PanePresentationMode` and may be set independently.
- `FullyLocked` is reserved for system-owned panes (e.g., a required diagnostics pane during a critical operation). It is not user-assignable through normal settings.
- Lock state is workbench-owned; it does not affect graph content or node identity.
- Lock state changes must route through explicit `GraphIntent` variants; no direct field mutation from UI callsites.
- Forbidden operations on locked panes must produce explicit feedback — silent failure is forbidden.

---

## 8. Acceptance Criteria

| Criterion | Verification |
|-----------|-------------|
| Docked pane hides split/move affordances | Test: mode = `Docked` → split and drag handles not rendered |
| Docked pane is closeable | Test: mode = `Docked` → close button present and functional |
| Pane presentation change does not move pane in tile tree | Test: switch `Docked -> Tiled` -> pane `TileId` remains at same tree position |
| Pane presentation change does not affect graph data | Test: switch `Docked <-> Tiled` -> no graph node create/delete, address writes, or traversal appends |
| Tab reorder changes `Vec<TileId>` order only | Test: drag tab to new position → only container child order changed; no depth change |
| Docked pane is not user-draggable | Test: mode = `Docked` → drag attempt has no effect |
| Active tab indicator visible without animation | Test: `prefers-reduced-motion` set → active tab indicator renders distinctly |
| Cross-Tab-Group drop moves tile | Test: drag tab to different Tab Group → tile moves to new container |
| `PanePresentationModeChanged` signal emitted on mode switch | Test: switch presentation mode -> `PanePresentationModeChanged` signal present in signal log |
