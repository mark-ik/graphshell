# Pane Chrome and Opening Semantics — Interaction Spec

**Date**: 2026-02-28 (revised 2026-04-06)
**Status**: Canonical interaction contract
**Priority**: Implementation-ready

**Related**:

- `WORKBENCH.md`
- `workbench_frame_tile_interaction_spec.md`
- `pane_presentation_and_locking_spec.md` — **canonical authority for `PaneLock`** (§8 here defers to it)
- `2026-03-03_pane_opening_mode_and_simplification_suppressed_plan.md` — canonical authority for `PaneOpeningMode` and `SimplificationSuppressed`
- `../../archive_docs/checkpoint_2026-04-02/graphshell_docs/implementation_strategy/workbench/2026-02-22_workbench_tab_semantics_overlay_and_promotion_plan.md` — archived execution note for the completed `FrameTabSemantics` rollout; canonical semantic-tab contract lives in `../graph/multi_view_pane_spec.md §7`
- `../canvas/node_badge_and_tagging_spec.md` — badge strip rendering contract referenced in §4.1
- `../shell/2026-04-03_shell_command_bar_execution_plan.md` — the Shell command bar cleanup that motivated the graduated chrome model
- `../../../TERMINOLOGY.md` — `Tiled Pane`, `Docked Pane`, `Pane Presentation Mode`, `Tile`

---

## 1. Scope

This spec defines the canonical contracts for:

1. **Pane Presentation Mode** — the graduated chrome model and its per-mode rendering rules.
2. **Tile Viewer Chrome Strip** — the per-pane viewer toolbar rendered for Tiled panes.
3. **Compatibility mode** — Wry as a local tile chrome affordance for web compat fallback.
4. **Tab selector overlay** — when and how the tile-selection chrome renders.
5. **Pane opening mode boundary** — where graph-citizenship decisions stop and chrome behavior begins.
6. **Presentation-mode transitions** — moving panes between Tiled, Docked, and Floating presentation.
7. **Tab ordering and reorder** — drag-reorder semantics within a Tile.
8. **Pane locking** — preventing accidental reflow.
9. **Floating pane promotion** — the canonical path from ephemeral overlay pane to graph-backed tile.

This spec does **not** define duplicated cross-context appearances as
presentation-instances of one shared node. Reuse across frames/graphlets is
handled by explicit node operations (`Move`, `Associate`, `Copy`) in graph /
workbench authority. Navigator lifecycle acts on node-bearing container entries,
not on bare pane instances.

### 1.1 Graduated Chrome Principle

Chrome affordances graduate with a pane's lifecycle status in the graph-backed
workbench. A **Pane** is an ephemeral content carrier, not yet a graph citizen.
A **Tile** is a graph-backed workbench citizen. Only Tiles expose viewer
affordances. The Shell command bar and workbench host chrome are for navigation
structure, arrangement, frames, and context — not per-viewer command ownership.

Per-viewer controls (Back, Forward, Reload, Zoom, compatibility mode) live in
the **Tile Viewer Chrome Strip** (§3), rendered per-pane and scoped to the
pane's own content. This replaces any prior model where viewer controls were
hosted in a global toolbar.

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
promote action (§2.3, §6.3) is the one presentation-mode transition that *does*
cross the graph-citizenship boundary. It is the canonical **Promotion** event:
graph citizenship is written, a node is created or reused, and a `PaneId` plus
any required container membership are assigned. All other mode transitions
remain graph-neutral.

### 2.1 Tiled Pane

The primary presentation mode. Tiled panes are graph-backed workbench citizens
with full viewer affordances.

```
┌─ Tab Bar (full: favicon, title, frame chip, close) ──┐
├─ Tile Viewer Chrome Strip ───────────────────────────┤
│  [<] [>] [R]  example.com/page   [1:1] [-][+] [Compat]  │
├─ Viewer Content Area ────────────────────────────────┤
│                                                       │
│       (Servo composited texture or Wry overlay)       │
│                                                       │
└───────────────────────────────────────────────────────┘
```

- Renders with **tile-selector chrome**: tab bar strip, split/close affordances, resize handles.
- Renders a **Tile Viewer Chrome Strip** (§3) between the tab bar and the viewer content area, carrying per-pane navigation, zoom, and compatibility mode controls.
- Participates in all tile-tree mobility operations: split, move, reorder, close, open into a separate frame.
- Normal drag-and-drop target and source.

### 2.2 Docked Pane

Docked panes are graph-backed but present with reduced chrome. They are intended
for reference content, pinned panels, and secondary views that should not
compete visually with the active browsing context.

```
┌─ Tab Bar (compact: title + close) ────────────┐
├───────────────────────────────────────────────┤
│  (content, no viewer chrome strip)            │
│                                               │
└───────────────────────────────────────────────┘
```

- Renders with **compact tab chrome**: title, favicon, and close button. No frame chip, split offer, or resize handles.
- **No Tile Viewer Chrome Strip**. Navigation, zoom, and compatibility mode controls are not rendered. Users access these via keyboard shortcuts, the command palette, or the graph.
- **Position-locked**: drag-to-reorder is disabled. User cannot accidentally drag a docked pane out of its position.
- Docked panes are still closeable via their title-bar close button or keyboard shortcut.
- Docked panes are eligible for focus; focus behavior follows the Focus subsystem contract.
- A docked pane may be explicitly restored to `Tiled` presentation by the user (see §6.1).

**Rationale**: Docked presentation reduces visual noise and accidental reflow for auxiliary surfaces (e.g., a persistent diagnostics panel, a side-by-side reference node). Omitting the viewer chrome strip reinforces the visual distinction between the active browsing context (Tiled) and reference content (Docked).

### 2.3 Floating Pane

A `Floating` pane is an ephemeral content carrier rendered over the tile
surface. It is **not a graph citizen**: it has no `PaneId` and no
`ArrangementRelation` edge until the user explicitly promotes it.

```
┌───────────────────────────────────────────────┐
│  (content, chromeless)                        │
│                                               │
│                       [Promote ↑] [Dismiss ×] │
└───────────────────────────────────────────────┘
```

**Chrome contract**:

- No tab bar, tab strip, or title.
- No Tile Viewer Chrome Strip. No Back/Forward, no zoom, no compatibility toggle.
- No drag handle. Edge-drag resize is allowed (the user can adjust dimensions freely after open).
- `SimplificationSuppressed` is set automatically when the pane opens and cleared on promote or dismiss.
- Two affordances only:

| Control | Label | Action |
|---------|-------|--------|
| **Promote** | Promote | Promotes pane to graph-backed `Tiled` mode via `PromoteEphemeralPane` (see §6.3) |
| **Dismiss** | X | Demotes node to cold and closes pane without graph write |

These controls render as a compact bar above the content area. They are always
visible (not hover-gated) to ensure discoverability in the prototype.

**Lifetime contract**:

- A `Floating` pane's lifetime is scoped to the hosting surface/context it
  overlays. That host may be a tile surface, graph surface, frame context, or
  split context. If that host closes, the floating pane is discarded without any
  graph write — it was never graph-citizened.
- A `Floating` pane that is dismissed via X produces no graph node, no address write, and no traversal edge.
- A `Floating` pane is not a member of any Tile or tile tree container. It floats above the tile tree render layer until promoted or dismissed.
- Use cases: link-follows, previews, ephemeral content inspection.

**Rationale**: This formalises the "chromeless dialog/temp window" UX that previously appeared as an undefined side effect. The `Floating` mode makes it a managed, intentional surface with predictable lifecycle and a clear upgrade path to full graph citizenship.

---

## 3. Tile Viewer Chrome Strip

The Tile Viewer Chrome Strip is a per-pane horizontal toolbar rendered between
the tab bar and the viewer content area. It is the canonical home for
per-viewer navigation and rendering controls. It renders only in `Tiled`
presentation mode.

### 3.1 Layout

The strip renders as a single `ui.horizontal()` row. Left-aligned controls
provide navigation; right-aligned controls provide zoom and compatibility mode.
A compact URL display occupies the center.

```
[<] [>] [R]  |  example.com/path/to/page  |  [1:1] [-] [+]  [Compat]
 ← nav →     │        ← url →             │     ← zoom →    ← compat →
              separator                    right-to-left layout
```

### 3.2 Navigation Controls

| Control | Label | Command | Hover text |
|---------|-------|---------|------------|
| Back | `<` | `BrowserCommand::Back` | "Back" |
| Forward | `>` | `BrowserCommand::Forward` | "Forward" |
| Reload | `R` | `BrowserCommand::Reload` | "Reload" |

Navigation commands are routed via
`graph_app.request_browser_command(ChromeProjection { fallback_node: Some(node_key) }, command)`.
This targets the pane's own node without requiring `EmbedderWindow` access from
the tile behavior context.

### 3.3 URL Display

The pane's current node URL is shown as a compact, read-only label truncated to
fit the available width. The URL is for orientation only; address editing is
handled by the Shell omnibar.

### 3.4 Zoom Controls

| Control | Label | Command | Hover text |
|---------|-------|---------|------------|
| Reset | `1:1` | `BrowserCommand::ZoomReset` | "Reset zoom" |
| Zoom out | `-` | `BrowserCommand::ZoomOut` | "Zoom out" |
| Zoom in | `+` | `BrowserCommand::ZoomIn` | "Zoom in" |

Zoom commands use the same `ChromeProjection` routing as navigation.

### 3.5 Compatibility Mode Toggle (Wry)

Wry is framed as a **compatibility mode** — a local tile chrome affordance for
sites that don't render correctly in the default Servo-based renderer.

| State | Label | Tooltip |
|-------|-------|---------|
| Inactive | `Compat` | "Load in compatibility mode (Wry) for sites that don't render correctly" |
| Active | `Compat *` | "Using compatibility renderer (Wry). Click to switch back." |
| Unavailable | `Compat` (disabled) | Reason from `wry_unavailable_reason` (feature disabled, capability missing, or preference) |

Clicking the toggle swaps between the Wry viewer backend (`viewer:wry`) and
clearing the viewer override (returning to automatic resolution). The toggle
replaces the prior "Render With: Auto/WebView/Wry" selector.

### 3.6 NativeOverlay Constraint

When `TileRenderMode::NativeOverlay` is active (Wry overlay), the OS window
covers the pane's content rect. The Tile Viewer Chrome Strip is rendered by
egui *above* the compositor rect allocation — it is not covered by the
overlay. The content area's `allocate_exact_size(ui.available_size())` call
naturally shrinks to accommodate the strip.

### 3.7 Implementation Anchor

The chrome strip is rendered by `render_tile_viewer_chrome_strip()` in
`shell/desktop/workbench/tile_behavior/node_pane_ui.rs`, called from the
`PanePresentationMode::Tiled` branch of `render_node_pane_impl()`.

---

## 4. Tab Selector Overlay Contract

### 4.1 Tab Bar Rendering

Each Tile that contains multiple node entries renders a tab bar strip (legacy
term: **Workbar** for frames; now: frame tabs in a workbench-scoped Navigator
host or per-tile tab strips for multi-node tiles). The tab strip:

- Shows one tab entry per child tile with: title, badge strip (compact, per `../canvas/node_badge_and_tagging_spec.md §3.5`), close button.
- Active tile's tab is highlighted.
- Tab bar is scrollable horizontally if tab count overflows available width.

### 4.2 Tab Overlay on Hover

When the cursor hovers over a non-active pane within a multi-node Tile, a
**tile-selection affordance** is shown:

- A hover ring or highlight border on the pane boundary.
- The tab bar scrolls to make the hovered pane's tab entry visible.
- Clicking the pane body (not an interactive element within it) activates that pane.

**Invariant**: The hover affordance must not interfere with content interaction within the pane. Pointer events must be forwarded to the pane content when hovering over content areas.

### 4.3 Active Tab Indicator

The active tab renders a distinct visual indicator (accent underline or fill) that is visible at all zoom levels and in reduced-motion mode. The indicator must not rely on animation alone to convey active state.

---

## 5. Pane Opening Mode Boundary

This spec does not define the full Pane Opening Mode model. It defines the boundary between opening semantics and chrome semantics so they do not get conflated.

Canonical boundary:

- **Pane Opening Mode** decides whether opening content creates graph citizenship.
- **Pane Presentation Mode** decides how an already-open pane renders and how much tile chrome it exposes.

Rules:

1. Opening a pane in an ephemeral mode may create a visible pane without writing any graph node.
2. Creating graph citizenship (for example, writing a pane address into the graph and turning it into a graph-backed tile) is an opening-mode concern, not a chrome-mode concern — **except** for `Floating` pane promotion (§6.3), which is the explicit crossing of this boundary by user intent.
3. Moving a node into another frame, associating it with another graphlet, or copying it into another context are explicit node operations and are outside the scope of pane chrome. This spec must not describe those operations as generic "open elsewhere" behavior.
4. Once a pane is already graph-backed, switching between `Tiled` and `Docked` changes only presentation and lock affordances.
5. Internal surfaces that are graph-backed at creation time (for example `verso://tool/*`, `verso://view/*`, `verso://frame/<FrameId>`) are already across the opening-mode boundary before this spec applies.
6. `Floating` panes opened in `QuarterPane` or `HalfPane` opening mode begin as ephemeral. Their `PaneOpeningMode` transitions to `Tile` only when the user triggers promotion (§6.3).

Compatibility note:

- Older docs may still use `graphshell://...` for these same internal surfaces.
- Treat `graphshell://...` as the legacy alias; runtime canonical formatting is `verso://...`.

This separation is required by the address-as-identity model in `TERMINOLOGY.md`: graph citizenship follows address write and node existence, not the presence or absence of tab chrome.

---

## 6. Presentation-Mode Transition Contract

### 6.1 Restore Full Tile Chrome: Docked -> Tiled

Triggers:

- Right-click on a docked pane's title bar -> context menu "Show Tile Chrome"
- Keyboard shortcut (configurable; default unbound)
- Command palette: "Show Tile Chrome"

Effect:

1. Pane `PanePresentationMode` changes to `Tiled`.
2. Pane is inserted into the tile tree at its current position (it was already in the tree; only chrome mode changes).
3. Full tile-selector chrome — including the Tile Viewer Chrome Strip (§3) — is restored.
4. Workbench emits a `PanePresentationModeChanged` signal for observability.

### 6.2 Reduce Chrome: Tiled -> Docked

Triggers:

- Right-click on a tab entry -> context menu "Dock pane"
- Keyboard shortcut (configurable; default unbound)
- Command palette: "Dock pane"

Effect:

1. Pane `PanePresentationMode` changes to `Docked`.
2. Position in tile tree is preserved (pane is not moved; only chrome mode changes).
3. Tile Viewer Chrome Strip is removed; compact tab chrome applied; drag affordances hidden.
4. Workbench emits a `PanePresentationModeChanged` signal for observability.

**Invariant**: Presentation-mode transitions (§6.1, §6.2) never move or remove the pane from the tile tree. They only change the presentation mode. Content and graph state are unaffected. The sole exception is §6.3 (Floating → Tiled promotion), which is a graph-citizenship transition by design.

### 6.3 Promote Floating Pane to Tile: Floating -> Tiled

**Trigger**: User clicks the Promote control on a `Floating` pane.

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

### 6.4 Dismiss Floating Pane

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

## 7. Tab Reorder Contract

Within a Tile, tabs may be reordered by drag-and-drop.

### 7.1 Drag Semantics

- Drag target: the tab entry in the tab strip (not the pane body).
- Drop target: any position in the same tab strip (reorder within the same Tile).
- Cross-Tile drag: drops a tab into a different Tile (moves the tile membership, not just reorders).

**Invariant**: Tab reorder within a Tile only changes the ordered node-entry list in that container. It does not change tile tree depth or split geometry. No graph data is affected.

### 7.2 Docked Panes and Reorder

Docked panes are **not draggable** by the user. The drag affordance is hidden in docked chrome. Programmatic reorder (via intent) is still possible; only the user-interactive drag is blocked.

### 7.3 Dropped Tab Feedback

When a drag completes successfully, the tab animates to its new position (120 ms ease-out; respects `prefers-reduced-motion`). If the drag is cancelled (Esc or released outside a valid drop zone), the tab returns to its original position.

---

## 8. Pane Locking Contract

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

## 9. Acceptance Criteria

### 9.1 Graduated Chrome

| Criterion | Verification |
|-----------|-------------|
| Tiled pane renders Tile Viewer Chrome Strip | Test: mode = `Tiled` → chrome strip with nav, URL, zoom, compat visible between tab bar and content |
| Docked pane does not render Tile Viewer Chrome Strip | Test: mode = `Docked` → no chrome strip; compact tab bar only |
| Floating pane renders only Promote + Dismiss | Test: mode = `Floating` → only Promote and Dismiss controls rendered; no nav, zoom, or compat |
| Chrome strip does not render for Fullscreen | Test: mode = `Fullscreen` → no chrome strip rendered |

### 9.2 Tile Viewer Chrome Strip (§3)

| Criterion | Verification |
|-----------|-------------|
| Back button sends `BrowserCommand::Back` to pane node | Test: click `<` → `request_browser_command(ChromeProjection { fallback_node: node_key }, Back)` called |
| Forward button sends `BrowserCommand::Forward` to pane node | Test: click `>` → `request_browser_command(ChromeProjection { fallback_node: node_key }, Forward)` called |
| Reload button sends `BrowserCommand::Reload` to pane node | Test: click `R` → `request_browser_command(ChromeProjection { fallback_node: node_key }, Reload)` called |
| URL display shows truncated node URL | Test: node URL longer than display limit → truncated with ellipsis |
| Zoom in/out/reset route to pane node | Test: click `+`/`-`/`1:1` → corresponding `BrowserCommand` sent to pane node |
| Compat toggle activates Wry viewer | Test: click `Compat` when inactive → `viewer_id_override` set to `viewer:wry` |
| Compat toggle deactivates Wry viewer | Test: click `Compat *` when active → `viewer_id_override` cleared |
| Compat toggle disabled when Wry unavailable | Test: `wry_unavailable_reason` returns `Some` → toggle disabled with reason tooltip |
| NativeOverlay chrome strip visible above overlay | Test: `TileRenderMode::NativeOverlay` → chrome strip renders above compositor rect; not covered by OS overlay |

### 9.3 Presentation-Mode Transitions and Tab Behavior

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

### 9.4 Floating Pane Lifecycle

| Criterion | Verification |
|-----------|-------------|
| Floating pane renders no tab bar or viewer chrome strip | Test: mode = `Floating` → no tab bar, no chrome strip |
| Floating pane dismiss produces no graph write | Test: click Dismiss on `Floating` pane → no graph node created, no address written, no traversal edge appended |
| Floating pane dismissed when enclosing surface closes | Test: close host Tile containing a `Floating` pane → pane is discarded; no graph write |
| Floating pane promotion creates graph node and `PaneId` | Test: click Promote on `Floating` pane → graph node created, `PaneId` assigned, `ArrangementRelation` edge asserted |
| Promoted pane inserted into Tile as new tab | Test: promote `Floating` pane overlaying a Tile → pane appears as a new tab in that tile with tab handle |
| Promoted pane loses floating geometry | Test: promote `Floating` pane → floating overlay removed; pane occupies tile tree position |
| `SimplificationSuppressed` cleared after promotion | Test: promote `Floating` pane → `SimplificationSuppressed` not set on resulting `Tiled` pane |
| `SimplificationSuppressed` cleared after dismiss | Test: dismiss `Floating` pane → `SimplificationSuppressed` cleared before removal |
