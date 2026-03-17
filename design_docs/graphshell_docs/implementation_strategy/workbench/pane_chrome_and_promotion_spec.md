# Pane Chrome and Opening Semantics — Interaction Spec

**Date**: 2026-02-28 (revised 2026-03-15)
**Status**: Canonical interaction contract
**Priority**: Implementation-ready

**Related**:

- `WORKBENCH.md`
- `workbench_frame_tile_interaction_spec.md`
- `pane_presentation_and_locking_spec.md` — **canonical authority for `PaneLock`** (§7 here defers to it)
- `2026-03-03_pane_opening_mode_and_simplification_suppressed_plan.md` — canonical authority for `PaneOpeningMode` and `SimplificationSuppressed`
- `2026-02-22_workbench_tab_semantics_overlay_and_promotion_plan.md` — `FrameTabSemantics` overlay plan (§7 canonicalized there)
- `../canvas/node_badge_and_tagging_spec.md` — badge strip rendering contract referenced in §3.1
- `../../../TERMINOLOGY.md` — `Tiled Pane`, `Docked Pane`, `Pane Presentation Mode`, `Tile`

---

## 1. Scope

This spec defines the canonical contracts for:

1. **Pane Presentation Mode** — the three chrome modes and their behavioral rules.
2. **Tab selector overlay** — when and how the tile-selection chrome renders.
3. **Pane opening mode boundary** — where graph-citizenship decisions stop and chrome behavior begins.
4. **Presentation-mode transitions** — moving panes between Tiled, Docked, and Floating presentation.
5. **Tab ordering and reorder** — drag-reorder semantics within a Tile.
6. **Pane locking** — preventing accidental reflow.
7. **Floating pane promotion** — the canonical path from ephemeral overlay pane to graph-backed tile.

This spec does **not** define duplicated cross-context appearances as
presentation-instances of one shared node. Reuse across frames/graphlets is
handled by explicit node operations (`Move`, `Associate`, `Copy`) in graph /
workbench authority. Navigator lifecycle acts on node-bearing container entries,
not on bare pane instances.

---

## 2. Pane Presentation Mode Contract

Every pane has a **Pane Presentation Mode** (also called **Pane Chrome Mode**) that controls its chrome, mobility, and locking behavior. This is distinct from both the pane's content and the pane's **Pane Opening Mode** (the graph-citizenship decision that determines whether the pane exists only as an ephemeral open surface or as a graph-backed tile).

```
PanePresentationMode =
  | Tiled      -- full chrome; normal tile-tree mobility
  | Docked     -- reduced chrome; position-locked
  | Floating   -- chromeless overlay; ephemeral by default; promotable to Tiled
  | Fullscreen -- content-only; all chrome hidden (future; not in current scope)
```

**Invariant**: `PanePresentationMode` is workbench-owned state. It does not affect graph content identity. Changing the mode of a pane must not create or delete graph nodes, write addresses, append traversal history, or otherwise mutate graph data.

**Exception — promotion**: transitioning a `Floating` pane to `Tiled` via the
promote action (§2.3, §5.3) is the one presentation-mode transition that *does*
cross the graph-citizenship boundary. It is the canonical **Promotion** event:
graph citizenship is written, a node is created or reused, and a `PaneId` plus
any required container membership are assigned. All other mode transitions
remain graph-neutral.

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

### 2.3 Floating Pane

A `Floating` pane is a chromeless overlay pane rendered at a fractional size (`QuarterPane` = 25%, `HalfPane` = 50% of the parent tile area) over the tile surface. It is **ephemeral by default**: it has no graph citizenship, no `PaneId`, and no `ArrangementRelation` edge until the user explicitly promotes it.

**Chrome contract**:

- No top bar, tab strip, or title.
- No drag handle. Edge-drag resize is allowed (the user can adjust dimensions freely after open).
- `SimplificationSuppressed` is set automatically when the pane opens and cleared on promote or dismiss.
- Two **hover-only overlay controls** rendered as translucent buttons in a thin band along the top edge of the pane. They are invisible when the cursor is outside the pane rect and fade in (~80 ms) on cursor entry. They must not intercept pointer events to pane content outside the top-edge band.

| Control | Position | Icon | Action |
|---------|----------|------|--------|
| **Promote** | Top-left | ▣ (expand square) | Promotes pane to graph-backed `Tiled` mode (see §5.3) |
| **Dismiss** | Top-right | ✕ | Closes and discards pane without graph write |

**Lifetime contract**:

- A `Floating` pane's lifetime is scoped to the hosting surface/context it
  overlays. That host may be a tile surface, graph surface, frame context, or
  split context. If that host closes, the floating pane is discarded without any
  graph write — it was never graph-citizened.
- A `Floating` pane that is dismissed via ✕ produces no graph node, no address write, and no traversal edge.
- A `Floating` pane is not a member of any Tile or tile tree container. It floats above the tile tree render layer until promoted or dismissed.

**Rationale**: This formalises the "chromeless dialog/temp window" UX that previously appeared as an undefined side effect. The `Floating` mode makes it a managed, intentional surface with predictable lifecycle and a clear upgrade path to full graph citizenship.

---

## 3. Tab Selector Overlay Contract

### 3.1 Tab Bar Rendering

Each Tile that contains multiple node entries renders a tab bar strip (legacy
term: **Workbar** for frames; now: frame tabs in the **Workbench Sidebar** or
per-tile tab strips for multi-node tiles). The tab strip:

- Shows one tab entry per child tile with: title, badge strip (compact, per `../canvas/node_badge_and_tagging_spec.md §3.5`), close button.
- Active tile's tab is highlighted.
- Tab bar is scrollable horizontally if tab count overflows available width.

### 3.2 Tab Overlay on Hover

When the cursor hovers over a non-active pane within a multi-node Tile, a
**tile-selection affordance** is shown:

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
2. Creating graph citizenship (for example, writing a pane address into the graph and turning it into a graph-backed tile) is an opening-mode concern, not a chrome-mode concern — **except** for `Floating` pane promotion (§5.3), which is the explicit crossing of this boundary by user intent.
3. Moving a node into another frame, associating it with another graphlet, or copying it into another context are explicit node operations and are outside the scope of pane chrome. This spec must not describe those operations as generic "open elsewhere" behavior.
4. Once a pane is already graph-backed, switching between `Tiled` and `Docked` changes only presentation and lock affordances.
5. Internal surfaces that are graph-backed at creation time (for example `verso://tool/*`, `verso://view/*`, `verso://frame/<FrameId>`) are already across the opening-mode boundary before this spec applies.
6. `Floating` panes opened in `QuarterPane` or `HalfPane` opening mode begin as ephemeral. Their `PaneOpeningMode` transitions to `Tile` only when the user triggers promotion (§5.3).

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

**Invariant**: Presentation-mode transitions (§5.1, §5.2) never move or remove the pane from the tile tree. They only change the presentation mode. Content and graph state are unaffected. The sole exception is §5.3 (Floating → Tiled promotion), which is a graph-citizenship transition by design.

### 5.3 Promote Floating Pane to Tile: Floating -> Tiled

**Trigger**: User clicks the ▣ promote control on a `Floating` pane (hover-only, top-left corner).

**Effect** (in order):

1. Emit `PromoteFloatingPane { target_tile_context }`.
2. In `apply_intents()`:
   - Resolve the pane's content address and write it through the canonical graph write path.
   - Create or reuse a graph node according to address-as-identity rules.
   - Assign a stable `PaneId`.
   - Assert an `ArrangementRelation` edge (sub-kind determined by target tile context — see below).
   - Transition `PaneOpeningMode` from `QuarterPane`/`HalfPane` to `Tile`.
3. In `reconcile_webview_lifecycle()` / workbench mutation:
   - Insert the pane into the tile tree at the target position (see placement rules below).
   - Discard the floating geometry.
   - Switch `PanePresentationMode` to `Tiled`.
   - Clear `SimplificationSuppressed`.
4. Workbench emits `PanePresentationModeChanged` signal.

**Placement rules**:

| Context the floating pane overlays | Placement on promotion |
|-------------------------------------|------------------------|
| Inside or over an existing Tile | New node entry in that Tile; `ArrangementRelation` sub-kind `tile-member` when tile membership is graph-rooted for that context |
| Over a split (horizontal or vertical tile, no enclosing multi-node Tile) | New split at current tile tree level; `ArrangementRelation` sub-kind `split-pair` |
| Over the bare graph canvas (no workbench tiles open) | New solo tile; no default `ArrangementRelation` edge is required |

**Invariant**: The floating pane's content and address are preserved through promotion. No content reload occurs. The pane receives its tab handle at its insertion position in the tile tree.

### 5.4 Dismiss Floating Pane

**Trigger**: User clicks the ✕ dismiss control on a `Floating` pane (hover-only, top-right corner), or closes the enclosing surface.

**Effect**:

1. Pane is removed from the workbench render layer.
2. No graph node is created. No address is written. No traversal edge is appended.
3. `SimplificationSuppressed` is cleared.
4. If the enclosing surface closed (not an explicit dismiss click), the floating pane is discarded silently — no signal, no undo entry.
5. If the user clicked ✕ explicitly, workbench emits `PaneDiscarded` signal (for observability; no undo entry since the pane was never graph-backed).

**Non-goal clarification**: `Dismiss` here is pane-surface discard only. It is
not the same as Navigator `DismissNode`, which removes a node from its current
container and may demote or delete that node depending on lifecycle state.

---

## 6. Tab Reorder Contract

Within a Tile, tabs may be reordered by drag-and-drop.

### 6.1 Drag Semantics

- Drag target: the tab entry in the tab strip (not the pane body).
- Drop target: any position in the same tab strip (reorder within the same Tile).
- Cross-Tile drag: drops a tab into a different Tile (moves the tile membership, not just reorders).

**Invariant**: Tab reorder within a Tile only changes the ordered node-entry list in that container. It does not change tile tree depth or split geometry. No graph data is affected.

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
| Pane presentation change does not move pane in tile tree | Test: switch `Docked -> Tiled` → pane `TileId` remains at same tree position |
| Pane presentation change does not affect graph data | Test: switch `Docked <-> Tiled` → no graph node create/delete, address writes, or traversal appends |
| Tab reorder changes `Vec<TileId>` order only | Test: drag tab to new position → only container child order changed; no depth change |
| Docked pane is not user-draggable | Test: mode = `Docked` → drag attempt has no effect |
| Active tab indicator visible without animation | Test: `prefers-reduced-motion` set → active tab indicator renders distinctly |
| Cross-Tile drop moves tab membership | Test: drag tab to different Tile → node entry moves to new container |
| `PanePresentationModeChanged` signal emitted on mode switch | Test: switch presentation mode → `PanePresentationModeChanged` signal present in signal log |
| Floating pane renders no top bar, tab strip, or title | Test: mode = `Floating` → no chrome elements rendered outside the top-edge hover band |
| Floating pane hover controls appear only on cursor entry | Test: cursor outside pane rect → controls not visible; cursor enters → controls visible within 80 ms |
| Floating pane hover controls do not intercept content pointer events | Test: click on pane content area (not top-edge band) → event reaches pane content, not intercepted by chrome |
| Floating pane dismiss produces no graph write | Test: click ✕ on `Floating` pane → no graph node created, no address written, no traversal edge appended |
| Floating pane dismissed when enclosing surface closes | Test: close host Tile containing a `Floating` pane → pane is discarded; no graph write |
| Floating pane promotion creates graph node and `PaneId` | Test: click ▣ on `Floating` pane → graph node created, `PaneId` assigned, `ArrangementRelation` edge asserted |
| Promoted pane inserted into Tile as new tab | Test: promote `Floating` pane overlaying a Tile → pane appears as a new tab in that tile with tab handle |
| Promoted pane loses floating geometry | Test: promote `Floating` pane → floating overlay removed; pane occupies tile tree position |
| `SimplificationSuppressed` cleared after promotion | Test: promote `Floating` pane → `SimplificationSuppressed` not set on resulting `Tiled` pane |
| `SimplificationSuppressed` cleared after dismiss | Test: dismiss `Floating` pane → `SimplificationSuppressed` cleared before removal |
