# egui_tiles Retirement Strategy

**Date**: 2026-04-11
**Status**: Strategy — not yet started
**Scope**: Plan for removing the `egui_tiles` dependency from Graphshell after
GraphTree has assumed full semantic authority (Phases 0-6 of the GraphTree
implementation plan are complete).

**Related**:

- `2026-04-10_graph_tree_implementation_plan.md` — parent plan (Phases 0-6 done)
- `2026-04-11_graph_tree_egui_tiles_decoupling_follow_on_plan.md` — authority
  migration strategy (complete)
- `../../technical_architecture/graph_tree_spec.md` — GraphTree crate design
- `workbench_frame_tile_interaction_spec.md` — Workbench mutation semantics
- `../navigator/NAVIGATOR.md` — Navigator projection semantics

---

## 1. Current State

GraphTree is now the semantic authority for:

- membership (which nodes are in the tree)
- topology (parent-child relationships, provenance)
- activation / expansion / focus
- layout intent (taffy-backed `compute_layout()`)
- graphlet binding and reconciliation
- UxTree emission

`egui_tiles` remains the live rendering host. It is called at the main render
entry point (`tile_post_render::render_tile_tree_and_collect_outputs`) and the
`Behavior` trait implementation (`GraphshellTileBehavior`) provides:

- tab UI rendering (titles, favicons, close buttons)
- pane content rendering (`pane_ui()`)
- drag/drop interaction (tab detach/reattach)
- edit actions (drag stop, split)
- simplification options
- per-frame tile tree state management

---

## 2. Scale of Removal

### 2.1 File counts

| Metric | Count |
|--------|-------|
| Files with `egui_tiles` imports | 60 |
| `Tree<TileKind>` references | ~450 |
| `tiles_tree` variable references | ~1,565 |
| `TileId` references | ~142 |
| Lines in core tile modules | ~11,364 |

### 2.2 Modules to delete or gut

These modules exist only to serve `egui_tiles` and have GraphTree replacements:

| Module | Lines | GraphTree replacement |
|--------|-------|-----------------------|
| `tile_view_ops.rs` | 2,443 | `graph_tree_commands` + `NavAction::apply()` |
| `tile_kind.rs` | 135 | `MemberEntry` |
| `tile_grouping.rs` | 166 | `GraphletRef` |
| `tile_invariants.rs` | 149 | `apply()` postconditions + parity checks |
| `semantic_tabs.rs` | 248 | `TabEntry` from `LayoutResult` |

### 2.3 Modules to refactor heavily

| Module | Lines | What changes |
|--------|-------|-------------|
| `tile_behavior.rs` | 1,681 | Replace `Behavior<TileKind>` with direct `GraphTreeRenderer` calls |
| `tile_compositor.rs` | 2,916 | Remove `TileId` keying; use `NodeKey` directly |
| `tile_post_render.rs` | 1,205 | Replace `tiles_tree.ui(&mut behavior, ui)` with GraphTree-driven render |
| `tile_render_pass.rs` | 1,406 | Remove `tiles_tree` parameter threading |
| `tile_runtime.rs` | 1,015 | Replace tile-tree queries with GraphTree queries |
| `graph_tree_dual_write.rs` | ~186 | Delete entirely (transitional module) |
| `graph_tree_sync.rs` | varies | Delete or merge into persistence |

### 2.4 UI modules with tile-tree coupling

These modules pass `tiles_tree` through call chains:

- `gui.rs`, `gui_frame.rs`, `gui_orchestration.rs` — top-level frame plumbing
- `workbench_host.rs` — workbench rendering, tab bar, node labels
- `shell_layout_pass.rs` — WorkbenchArea slot rendering
- `focus_state.rs`, `focus_realizer.rs` — focus management
- `nav_targeting.rs` — navigation target resolution
- `persistence_ops.rs` — persist/restore tile tree
- `dialog_panels.rs`, `toolbar_ui.rs`, `overview_plane.rs`, `tag_panel.rs`
- `keyboard_phase.rs`, `toolbar_dialog.rs`

---

## 3. What egui_tiles Currently Provides That Must Be Replaced

### 3.1 The `Behavior<TileKind>` trait

The core rendering contract. `GraphshellTileBehavior` implements:

| Method | What it does | Replacement |
|--------|-------------|-------------|
| `pane_ui()` | Render pane content for each tile | GraphTree-driven content dispatch |
| `tab_ui()` | Render custom tab UI with favicon/lifecycle | `EguiGraphTreeRenderer::render_tree_tabs/flat_tabs` |
| `tab_title_for_pane()` | Node label resolution | Label closure (already wired in Phase 4a) |
| `is_tab_closable()` | Tab close policy | `MemberEntry` lifecycle rules |
| `on_tab_close()` | Close handler with successor activation | `NavAction::Dismiss` + focus cycle |
| `on_edit()` | Drag/split edit actions | `NavAction::Reparent` + layout overrides |
| `simplification_options()` | Tab merging/simplification policy | Not needed — GraphTree topology is explicit |

### 3.2 Tab drag/drop interaction

`egui_tiles` handles:

- tab drag initiation (from tab bar)
- drag overlay rendering (translucent tab follows cursor)
- drop target detection (split zones, tab bar insertion)
- tab reattach / container creation on drop

GraphTree replacement: implement a lightweight drag state machine in the
`graph_tree_adapter` that:

1. Detects drag start on tab widgets (egui `Sense::drag()`)
2. Renders a drag overlay during drag
3. Computes drop zones from `SplitBoundary` positions + tab bar rects
4. Emits `NavAction::Reparent` on drop

This is ~200-300 lines of new adapter code.

### 3.3 Container layout (linear/tabs/grid)

`egui_tiles` provides `Container::Linear`, `Container::Tabs`, and
`Container::Grid` layout modes with automatic rect subdivision.

GraphTree replacement: already done. `compute_layout()` with taffy handles
all layout modes (TreeStyleTabs, FlatTabs, SplitPanes). The `SplitBoundary`
system already provides interactive resize handles.

### 3.4 Floating panes

`egui_tiles` has no native floating pane support — Graphshell already
implements this as a custom overlay in `tile_render_pass.rs`
(`render_floating_pane_overlays`). No additional work needed.

### 3.5 Persistence

The tile tree is currently serialized as part of workbench persistence.
GraphTree already has its own serialization. The persistence layer must:

1. Stop serializing `Tree<TileKind>`
2. Rely on `GraphTree` persistence (already partially wired)
3. Migrate existing saved state on load (one-shot)

---

## 4. Risk Assessment

### 4.1 High risk: Behavior trait replacement

The `Behavior<TileKind>` implementation is the single most coupled surface.
Every frame, `tiles_tree.ui(&mut behavior, ui)` calls `pane_ui()` for each
visible pane, which dispatches to content renderers. Replacing this requires
a new content dispatch loop driven by GraphTree's `LayoutResult.pane_rects`.

Mitigation: Phase 4a already wired `EguiGraphTreeRenderer` alongside
`egui_tiles`. The GraphTree renderer can be promoted to primary while
`egui_tiles` rendering is demoted to a no-op, verified side by side.

### 4.2 Medium risk: tiles_tree parameter threading

1,565 `tiles_tree` references across 60 files means a wide refactor.

Mitigation: `graph_tree` is already threaded through most of the same call
chains. The refactor is mechanical: replace `tiles_tree` with `graph_tree`
in function signatures, then update the body to use `graph_tree` APIs.

### 4.3 Medium risk: test coupling

Multiple test scenarios (`grouping.rs`, `layout.rs`, `persistence.rs`,
`input_routing.rs`, etc.) construct `Tree<TileKind>` fixtures. These must
be rewritten to construct `GraphTree` fixtures instead.

Mitigation: the test harness (`harness.rs`) can be updated to construct
both trees during a transitional test phase, then remove tile-tree fixture
construction once all tests pass with GraphTree alone.

### 4.4 Low risk: PaneId migration

`PaneId` is currently a newtype over tile tree identity. In the GraphTree
world, `NodeKey` is the canonical member identity. `PaneId` can either:

- become a newtype over `NodeKey`, or
- be retired entirely in favor of `NodeKey`

The compositor already maps `NodeKey -> PaneId` at the boundary (the
migration bridge in `active_node_pane_rects_from_graph_tree`). Once
`egui_tiles` is removed, this mapping becomes unnecessary.

---

## 5. Recommended Execution Phases

### Phase 7A: Content dispatch without egui_tiles (2-3 days)

Replace the `tiles_tree.ui(&mut behavior, ui)` call with a GraphTree-driven
content dispatch loop:

```
for (node_key, rect) in layout.pane_rects {
    let clip = ui.clip_rect().intersect(rect);
    let mut pane_ui = ui.child_ui(rect, *ui.layout(), None);
    pane_ui.set_clip_rect(clip);
    render_pane_content(&mut pane_ui, node_key, graph_app, ...);
}
```

The `render_pane_content` function extracts the content-rendering logic from
`GraphshellTileBehavior::pane_ui()` into a standalone function.

**Done gate**: `tiles_tree.ui()` call removed; content renders from GraphTree
layout rects.

### Phase 7B: Tab drag/drop state machine (2-3 days)

Implement drag interaction in `graph_tree_adapter`:

1. Drag start detection on tab widgets
2. Drag overlay rendering (floating tab ghost)
3. Drop zone computation from `SplitBoundary` positions
4. `NavAction::Reparent` emission on valid drop

**Done gate**: tabs can be dragged and dropped to rearrange without
`egui_tiles` drag handling.

### Phase 7C: Remove tile_view_ops.rs (1 day)

All mutation paths already go through `graph_tree_commands` via dual-write.
Remove `tile_view_ops` functions and update dual-write to call
`graph_tree_commands` directly (which it already does — just remove the
`tile_view_ops` half).

Then delete `graph_tree_dual_write.rs` — it's no longer needed.

**Done gate**: `tile_view_ops.rs` and `graph_tree_dual_write.rs` deleted;
all mutations go through `graph_tree_commands`.

### Phase 7D: Remove tile_kind.rs and tile_grouping.rs (0.5 day)

Replace `TileKind` with `NodeKey` in all remaining signatures.
Replace `tile_grouping` with `GraphletRef` APIs.

**Done gate**: `tile_kind.rs` and `tile_grouping.rs` deleted.

### Phase 7E: Rekey compositor from TileId to NodeKey (1-2 days)

`tile_compositor.rs` currently uses `PaneId` and `TileId` for content
callback registration, GL state isolation, and overlay passes.

Replace `TileId` with `NodeKey` throughout. Remove the migration bridge
in `active_node_pane_rects_from_graph_tree` that maps `NodeKey -> PaneId
-> TileId`.

**Done gate**: compositor uses `NodeKey` directly; no `TileId` references
remain.

### Phase 7F: Update parameter threading (2-3 days)

Mechanical refactor: remove `tiles_tree` from all function signatures.
Update 60 files, ~1,565 variable references.

Approach: start from leaf functions (those that only read `tiles_tree`),
replace with `graph_tree`, then work up the call chain.

**Done gate**: no `tiles_tree` or `Tree<TileKind>` references remain.

### Phase 7G: Update test fixtures (1-2 days)

Rewrite test scenarios to construct `GraphTree` fixtures instead of
`Tree<TileKind>`. Update the test harness.

**Done gate**: all tests pass without `egui_tiles` test fixtures.

### Phase 7H: Remove egui_tiles dependency (0.5 day)

1. Remove `egui_tiles` from `Cargo.toml`
2. Delete remaining tile-only modules (`tile_invariants.rs`, `semantic_tabs.rs`)
3. `cargo test` green, `cargo clippy` clean

**Done gate**: zero `egui_tiles` references remain.

---

## 6. Estimated Total Effort

| Phase | Days |
|-------|------|
| 7A Content dispatch | 2-3 |
| 7B Tab drag/drop | 2-3 |
| 7C Remove tile_view_ops | 1 |
| 7D Remove tile_kind/grouping | 0.5 |
| 7E Rekey compositor | 1-2 |
| 7F Parameter threading | 2-3 |
| 7G Test fixtures | 1-2 |
| 7H Remove dependency | 0.5 |
| **Total** | **10-15** |

---

## 7. Prerequisites

Before starting Phase 7:

1. All decoupling plan phases (A-G) should be complete or deliberately deferred
2. GraphTree should be verified as the authority under real usage (not just
   tests) for at least one release cycle
3. The per-view persistence migration (decoupling Phase F) should be done so
   tile-tree persistence can be dropped cleanly

---

## 8. Non-Goals

This strategy does **not** cover:

- compositor pipeline redesign (separate GL->wgpu plan)
- Navigator interaction model changes
- new layout modes beyond what GraphTree already supports
- multi-window support (separate concern)

The scope is strictly: remove `egui_tiles` as a dependency while preserving
all current functionality through GraphTree equivalents.
