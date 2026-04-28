<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# egui_tiles Retirement Plan (2026-04-28)

**Status**: SUPERSEDED 2026-04-28 by
[2026-04-28 iced jump-ship plan](2026-04-28_iced_jump_ship_plan.md).
This doc preserved as a historical alternative (preservation-shaped
migration); it is no longer the active path. The jump-ship plan
treats egui as broken and builds iced to a refined UX target rather
than to egui parity.

**Lane** (historical): M1 sibling — finishes the tile-authority
shift the
[2026-04-14 iced host migration plan](2026-04-14_iced_host_migration_execution_plan.md)
marked done at the *semantic* level but left structurally incomplete.

**Related**:

- [2026-04-14 iced host migration execution plan](2026-04-14_iced_host_migration_execution_plan.md)
  §M1 (GraphTree authority shift) and §M7 (cleanup)
- `crates/graph-tree/src/tree.rs` — `compute_layout()` already produces
  the rect/tab/split data the egui host consumes via egui_tiles
- `shell/desktop/workbench/graph_tree_facade.rs` — running tally of
  tile reference categories

**Implementation anchors**:

- `shell/desktop/ui/gui.rs:160` — `EguiHost.tiles_tree: Tree<TileKind>`
- `shell/desktop/workbench/tile_post_render.rs:404` — sole `Tree::ui()` call
- `shell/desktop/workbench/tile_behavior.rs:890-1642` —
  `GraphshellTileBehavior` impl (~752 LOC)
- `shell/desktop/workbench/tile_kind.rs:23-38` — `TileKind` enum
  (4 variants, all pure data)
- `shell/desktop/workbench/tile_compositor.rs` — `TileId`-keyed
  compositor state (~2,988 LOC)
- `crates/graph-tree/src/tree.rs:227` — `GraphTree::compute_layout()`
- `crates/graph-tree/src/layout.rs` — `LayoutResult<N>`

---

## 1. Why this is the next slice

The iced host migration plan claims §M1 is done: "`egui_tiles` is
presentation-only." Verified against current code, that's true at the
semantic level — all mutations route through `graph_tree_dual_write` —
but **`egui_tiles::Tree<TileKind>` is still the data structure the
egui host owns and renders from**. The audit found:

- ~60 Rust files reference egui_tiles types (~187 call sites)
- `Tree::ui(&mut Behavior)` is still the rendering entry point
  (`tile_post_render.rs:404`)
- `TileId` (~142 references) is the identity used for pane lookup,
  rect resolution, and compositor keying — not `NodeKey`
- `GraphshellTileBehavior` is a 1,642-LOC trait impl that owes most
  of its existence to egui_tiles' `Behavior<TileKind>` contract

`graph-tree` already owns everything needed to replace this:

- topology, membership, expansion, focus
- Taffy-backed `compute_layout()` returning `pane_rects`,
  `split_boundaries`, `tab_order`, `tree_rows`, `active`
- per-view persistence (already keyed by `GraphViewId`)

The structural retirement is therefore mechanical refactor, not new
authority work. It removes a load-bearing egui dependency that
otherwise blocks `--no-default-features --features iced-host,wry`
from compiling beyond the dep graph (which already excludes the
egui crates per the 2026-04-28 Cargo gate slice).

The iced host (`shell/desktop/ui/iced_host.rs`) does *not* use
egui_tiles today. Once the egui host stops needing it, both hosts
consume `GraphTree::compute_layout()` directly and `egui_tiles`
can leave the workspace.

---

## 2. North star

```
GraphTree<NodeKey>                       ← single semantic owner (already true)
   │
   ├── compute_layout(rect) -> LayoutResult<NodeKey>
   │      │
   │      ├── pane_rects: HashMap<NodeKey, Rect>
   │      ├── split_boundaries: Vec<SplitBoundary<NodeKey>>
   │      ├── tab_order: Vec<TabEntry<NodeKey>>
   │      ├── tree_rows: Vec<OwnedTreeRow<NodeKey>>
   │      └── active: Option<NodeKey>
   │
   └── pane payload: NodeKey -> TileKind (lookup, not tree-embedded)
       │
       ├── Egui host renders panes via direct iter(layout.pane_rects)
       └── Iced host renders panes via direct iter(layout.pane_rects)
```

`Tree<TileKind>` is gone. `TileId` is gone. Both hosts consume the
same `LayoutResult`.

---

## 3. Sequence rules

- Do **not** introduce a new tree data structure — `GraphTree` is
  already authoritative; the change is removing the redundant
  `egui_tiles::Tree<TileKind>` instance, not building its replacement.
- Do **not** delete `tile_behavior.rs` until its contents are inlined
  as standalone functions. Slice 1 extracts; later slices delete.
- Do **not** touch `iced_host.rs` rendering — it already doesn't use
  egui_tiles. The iced host gains no new code from this retirement;
  only egui-host code changes.
- Do **not** combine identity rekey (`TileId → NodeKey`) with behavior
  extraction — those are independent slices and combining them makes
  rollback harder.
- Keep `egui_tiles` as a `Cargo.toml` dep until all Rust code stops
  importing it. The dep removal is the final receipt of done.

---

## 4. Slice plan

Five slices, each shippable on its own. End-of-slice checkpoint
guarantees the build is green.

### S1. Extract `Behavior` methods to standalone functions

**Goal**: every method on `GraphshellTileBehavior` becomes a free
function or method on a non-trait type, callable without the
`egui_tiles::Behavior<TileKind>` trait surface.

Checklist:

- [ ] Extract `pane_ui()` body to `render_pane(ui, tile_id, pane, ctx)`
  in a new `tile_pane_render.rs`
- [ ] Extract `tab_ui()` body to `render_tab(ui, tab_state, ctx)`
- [ ] Extract `tab_title_for_pane()` to `pane_title(pane, ctx)`
- [ ] Extract `on_tab_close()` to `handle_tab_close(tile_id, ctx)`
- [ ] Extract `on_edit()` action handlers to `handle_tile_edit(action, ctx)`
- [ ] Keep `GraphshellTileBehavior::Behavior` impl as a thin wrapper
  that delegates to the new functions
- [ ] Verify visual + interactive parity with the existing render path
  (no `Tree::ui()` call changes yet)

**Done gate**: `tile_behavior.rs` `impl Behavior<TileKind>` block is
≤ 100 LOC (delegation only); all real logic lives in standalone
functions.

### S2. Replace `Tree::ui()` with direct iteration

**Goal**: `tile_post_render.rs` walks `LayoutResult` instead of
delegating to `egui_tiles`.

Checklist:

- [ ] In `render_tile_tree_and_collect_outputs()`, call
  `graph_tree.compute_layout(available_rect)`
- [ ] Iterate `layout.pane_rects` — for each `(node_key, rect)`,
  resolve `TileKind` via runtime lookup, dispatch to `render_pane()`
  from S1
- [ ] Render tab chrome by walking `layout.tab_order`
- [ ] Render split-drag affordances from `layout.split_boundaries`
- [ ] Delete the `tiles_tree.ui(&mut behavior, ui)` call site
- [ ] Keep `tiles_tree` field alive but no longer rendered from —
  becomes a vestigial cache S5 deletes
- [ ] Parity test: same `FrameHostInput` produces same
  `FrameViewModel.active_pane_rects` as before

**Done gate**: `Tree::ui()` is uncalled anywhere. egui_tiles widget
rendering is gone from the hot path.

### S3. Rekey compositor from `TileId` to `NodeKey`

**Goal**: `tile_compositor.rs` and surrounding state stop using
`TileId` as identity.

Checklist:

- [ ] Audit `tile_compositor.rs` for `TileId` references; convert
  all maps and signatures to `NodeKey`
- [ ] Update `OverlayStrokePass` producers (focus / selection /
  hover / semantic overlays) to emit `NodeKey`
- [ ] Update `ViewerSurfaceRegistry` callers — already keys on
  `NodeKey`, but some upstream call sites still pass `TileId`
- [ ] Drop `TileId` type alias and all helpers in
  `shell/desktop/workbench/tile_id.rs`
- [ ] Drop `TileId`-keyed entries in `LAST_SENT_NATIVE_OVERLAY_RECTS`,
  `COMPOSITOR_CONTENT_CALLBACKS`, etc. (most already `NodeKey`-keyed
  per §12.20 of the iced migration plan)

**Done gate**: `grep -r "TileId" shell/` returns only persistence
schema versioning + comments referring to historical name.

### S4. Migrate persistence + collapse `TileKind`

**Goal**: `tiles_tree` field is gone; `TileKind` is a pane-payload
type queryable from runtime state, not embedded in a tree.

Checklist:

- [ ] Replace `Drop` impl serialization of `Tree<TileKind>` with
  `GraphTree<NodeKey>` + `HashMap<NodeKey, TileKind>` payload cache
- [ ] Add a one-shot startup migration that reads old
  `Tree<TileKind>` blobs and emits the new shape (mark in
  `gui.rs:528` startup-import path)
- [ ] Remove `EguiHost.tiles_tree` field at `gui.rs:160`
- [ ] Move `TileKind` from `tile_kind.rs` to a payload module —
  the file becomes a query helper, not an enum-on-tree
- [ ] Verify a fresh-install + restored-session both bring up panes
  correctly

**Done gate**: `tiles_tree` no longer compiled. Old saves still load.

### S5. Delete egui_tiles dep

**Goal**: `egui_tiles = ...` removed from `Cargo.toml` and all
Rust imports gone.

Checklist:

- [ ] Delete `tile_view_ops.rs`, `tile_grouping.rs`,
  `tile_invariants.rs`, `semantic_tabs.rs` (the dual-write /
  parity / invariant glue is unneeded once `tiles_tree` is gone)
- [ ] Delete `graph_tree_dual_write.rs`, `graph_tree_sync.rs` —
  no second authority left to dual-write into or sync from
- [ ] Drop `egui_tiles` from `Cargo.toml` `egui-host` feature and
  from `[dependencies]`
- [ ] Drop `egui_tiles` references in `shell_layout_pass.rs`,
  `tile_render_pass.rs`, etc.
- [ ] `cargo tree -e features | grep egui_tiles` returns nothing
- [ ] Full test suite green

**Done gate**: `egui_tiles` is uncited in the workspace. `cargo
update` no longer pulls it.

---

## 5. Receipts and parity

Each slice produces a parity receipt:

- **S1**: visual diff against pre-slice screenshots; a recorded
  `RecordingPainter` test confirms the same draw-call sequence as
  the previous behavior dispatch
- **S2**: `LayoutResult.pane_rects` matches `tiles_tree`-derived
  rects to within 1px (already exercised by the existing GraphTree
  parity check at `parity.rs`)
- **S3**: compositor frame-path counters (`record_frame_path`)
  show the same path distribution as pre-slice
- **S4**: round-trip of an old `Tree<TileKind>` blob into the new
  format produces equivalent `GraphTree` + payload state
- **S5**: full integration-test suite + soak-test run

The S2 parity receipt is the most load-bearing — it's where we
prove `GraphTree::compute_layout()` is layout-equivalent to the
current `egui_tiles` rendering.

---

## 6. Risks

### 6.1 Tab drag/drop semantics

**Risk**: `egui_tiles` provides drag-to-reorder and drag-to-split
behavior in its widget; replacing it means we own those gestures.

**Mitigation**: `LayoutResult.split_boundaries` already exposes
hit regions for split-handle drag. Tab reorder is a `NavAction`
emitted from a custom hit-tester walking `layout.tab_order`.
~200 LOC of new gesture code in S2; less than what
`tile_behavior.rs:on_edit()` already does.

### 6.2 Persistence migration

**Risk**: existing user saves contain serialized
`Tree<TileKind>`. Loading them in the post-S4 world must work.

**Mitigation**: S4 ships a one-shot loader that reads the old
shape and emits the new one. Tested against snapshots from
several user sessions before merging S4. Rollback path:
re-serialize as the old shape.

### 6.3 Iced parity drift

**Risk**: Iced doesn't use egui_tiles, so nothing in the iced
host changes. But if `LayoutResult`'s shape changes during this
work, iced's layout consumer breaks.

**Mitigation**: don't change `LayoutResult` during the
retirement. If new fields are needed, add them additively. The
iced `IcedGraphCanvasProgram` continues to read `pane_rects`
and `tab_order` only.

### 6.4 Test churn

**Risk**: ~60 files mention egui_tiles types in tests. Each
test may need refactoring.

**Mitigation**: most test references are in fixtures that use
`GraphTree::add_member()` already; the egui_tiles parts are
construction helpers that can collapse to `GraphTree` calls.

### 6.5 Implicit rendering invariants

**Risk**: `Tree::ui()` provides built-in invariants around
container layout, focus restoration, tab activation. Inlining
its loop may surface subtle ordering issues.

**Mitigation**: S2 keeps the existing `Behavior` impl callable
but bypassed; if a regression appears, restoring the old call
is one line. The regression window is the S2 PR, not the
whole retirement.

---

## 7. Decision criteria — when to start

**Start now if**:

- The egui-host Cargo gate (landed 2026-04-28) is the architectural
  signal we wanted, and the iced-only build path needs to actually
  *compile* before iced chrome work can progress confidently.
- No competing work depends on `egui_tiles::Tree<TileKind>` being
  the runtime tree (none identified at scoping time).

**Defer if**:

- Iced host chrome work has a more urgent slice (e.g., toolbar
  parity, content-surface mounting per §M5/M6) that benefits more
  from focus.
- The S2 parity receipt would block on layout differences we don't
  yet understand.

**Don't start if**:

- The iced migration plan's M5/M6 milestones are about to hit
  acceptance — finish those first to validate the runtime API
  shape that `LayoutResult` is consumed by.

---

## 8. Bottom line

This is a mechanical retirement, not a new authority shift. The
hard work — `GraphTree` ownership, parity receipts, dual-write
elimination — already landed in M1. What remains is removing the
cached `egui_tiles::Tree<TileKind>` instance and rerouting the
egui host's render path through `GraphTree::compute_layout()`.

Done condition: `cargo build --no-default-features --features
iced-host,wry` compiles (the S5 receipt). Each slice has its own
done gate above; the build stays green between slices.
