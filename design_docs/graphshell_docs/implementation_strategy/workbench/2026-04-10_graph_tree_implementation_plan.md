# GraphTree Implementation Plan

**Date**: 2026-04-10
**Status**: Active — Phases 0–6 complete, Phase 7 remaining (strategy planned)
**Scope**: Phased migration from egui_tiles to GraphTree, including Navigator
projection collapse, arrangement-edge consumption, and UxTree integration.

**Related**:

- `../../technical_architecture/graph_tree_spec.md` — GraphTree crate API design
- `2026-04-11_graph_tree_egui_tiles_decoupling_follow_on_plan.md` — post-extraction authority migration, review findings, and `egui_tiles` decoupling sequence
- `../../research/2026-04-10_ui_framework_alternatives_and_graph_tree_discovery.md` — research backing
- `../../technical_architecture/graphlet_model.md` — graphlet semantics (consumed, not replaced)
- `../navigator/NAVIGATOR.md` — Navigator domain spec (projection collapses into GraphTree)
- `../navigator/navigator_interaction_contract.md` — click grammar (carried forward unchanged)
- `../navigator/navigator_backlog_pack.md` — Navigator items absorbed/unblocked
- `graphlet_projection_binding_spec.md` — binding semantics (consumed)
- `workbench_frame_tile_interaction_spec.md` — mutation routing (preserved)
- `../../research/2026-02-27_egui_stack_assessment.md` — "when to replace" criteria
- `../PLANNING_REGISTER.md` — lane registration
- `WORKBENCH.md` — Workbench domain authority (preserved)
- `../shell/shell_composition_model_spec.md` — WorkbenchArea slot (preserved)
- `../subsystem_ux_semantics/ux_tree_and_probe_spec.md` — UxTree projection contract

---

## 1. Why Now

The egui stack assessment (Feb 2026) said: replace egui_tiles when "adapter code
becomes larger than the value the widget provides" or "core UX keeps being
distorted to fit widget assumptions."

2026-04-11 note:

- the `graph-tree` crate extraction has now landed,
- but the desktop shell still mirrors `egui_tiles` back into `GraphTree`
  during the migration phase,
- so the next stage is no longer "extract the crate" but "complete the
  authority migration and decouple semantic truth from `egui_tiles`."

Treat the 2026-04-11 follow-on plan as the active execution note for that
post-extraction stage.

Current state:

- **~25.5k lines** of workbench code in `shell/desktop/workbench/`
- **53 files** reference `egui_tiles` across the codebase
- `tile_view_ops.rs` alone is **2,443 lines** of adapter/mutation code
- `tile_behavior.rs` is **1,681 lines** of `Behavior` trait overrides
- `GraphshellTileBehavior`, `GraphletBinding`, `TileCoordinator`, `FrameLayout`
  persistence, focus routing — all layered on top of a data structure that
  speaks rectangles, not graphlets
- Navigator maintains a **parallel projection** over the same membership truth,
  requiring synchronization that GraphTree eliminates
- The arrangement-graph-projection plan (shipped 2026-03-21) already moved
  membership truth to graph edges — but the tile tree still uses egui_tiles as
  the realization layer, creating a semantic translation gap

The adapter code now exceeds the value. The UX is distorted (Navigator vs.
Workbench scope split exists because egui_tiles can't express what the graph
already knows). The criteria are met.

---

## 2. What Gets Rolled In

### Features and specs that GraphTree absorbs or unblocks

| Item | Source | How GraphTree addresses it |
|------|--------|--------------------------|
| Navigator section collapse | NAVIGATOR.md §8 | Sections become `ProjectionLens` variants over one tree |
| Navigator/Workbench scope split | chrome_scope_split_plan | Eliminated — one tree, pick your lens |
| Navigator arrangement projection | NV15 (backlog) | `visible_rows()` with Arrangement lens replaces bespoke projection |
| Navigator expansion state | NV05 (backlog) | `expanded: HashSet<N>` is a first-class field, session-persisted |
| Navigator click grammar | NV06 / interaction contract | Carried forward unchanged; `NavAction` maps 1:1 |
| Navigator reveal rule | NV11 (backlog) | `Reveal(N)` in `NavAction` with lens-aware ancestor expansion |
| Navigator recents | NV14 (backlog) | `Recency` projection lens over the same tree |
| Navigator/Workbench sync | NV23 / WB25 (backlog) | Eliminated — no sync needed when there's one tree |
| Graphlet binding | graphlet_projection_binding_spec | Per-graphlet `GraphletBinding` inside the tree |
| EdgeProjectionSpec consumption | graphlet_projection_binding_spec §3 | `GraphletRef` reads selectors from graph truth |
| Frame graph representation | frame_graph_representation_spec | Stable `GraphletId` + tile-center coords for canvas minimap |
| Containment/domain grouping | graph_relation_families | Containment lens derives hierarchy from domain/url-path edges |
| Tile tree invariants | egui_stack_assessment §5.3 | `apply_nav()` enforces invariants by construction |
| Minimum pane sizes | ux_integration_research G-DO-2 | `LayoutOverride` per member, enforced by taffy |
| Node vs. tile identity confusion | ux_integration_research G-IA-1 | `Lifecycle` makes the distinction explicit (Cold = in graph, not in pane) |
| UxTree projection | ux_tree_and_probe_spec | `emit_ux_tree()` produces `UxNodeDescriptor` tree directly |
| Compositor bridge | frame_assembly_and_compositor_spec | `compute_layout()` provides pane rects; compositor queries tree, not egui_tiles |
| WorkbenchIntent routing | SYSTEM_REGISTER §9.2 | `NavAction` → `apply_nav()` → `TreeIntent` → host converts to `WorkbenchIntent` |
| Arrangement-edge extensibility | graph_relation_families | String-keyed edge families, not closed enums |
| Extension/PWA portability | portable_web_core_host_envelopes | Framework-agnostic tree serializes as JSON; renders as DOM in extension hosts |

### Navigator backlog items absorbed

| ID | Item | Status after GraphTree |
|----|------|----------------------|
| NV04 | Section Mapping Audit | Replaced by `ProjectionLens` variants |
| NV05 | Expansion State Contract | Built into `GraphTree.expanded` |
| NV10 | Projection Refresh Triggers | Tree rebuilds from graph deltas via `Attach`/`Detach`/`SetLifecycle` |
| NV14 | Recents Contract | `Recency` lens |
| NV15 | Arrangement Projection | `Arrangement` lens + `visible_rows()` |
| NV23 | Workbench-Navigator Sync | Eliminated — shared tree |

Items NOT absorbed (remain Navigator-owned):

| ID | Item | Why separate |
|----|------|-------------|
| NV16 | Search/Filter Model | Navigator search is a query over graph truth, not a tree operation |
| NV20 | Accessibility/Keyboard | Framework-specific; handled by `GraphTreeRenderer` adapter |
| NV21 | Diagnostics Pack | Diagnostics channels are subsystem-owned, not tree-owned |

---

## 3. Code Impact Map

### Files that change substantially (GraphTree replaces core logic)

| File | Lines | Current role | Migration |
|------|-------|-------------|-----------|
| `tile_view_ops.rs` | 2,443 | ~40 tree mutation functions | Replaced by `apply_nav()` with typed `NavAction` |
| `tile_behavior.rs` | 1,681 | `Behavior` trait overrides for egui_tiles | Replaced by `GraphTreeRenderer` impl for egui |
| `tile_kind.rs` | 135 | `TileKind` enum (Pane/Graph/Node/Tool) | Replaced by `MemberEntry<NodeKey>` with lifecycle + provenance |
| `tile_grouping.rs` | 166 | Frame/graphlet group tracking | Replaced by `GraphletRef` inside tree |
| `semantic_tabs.rs` | 248 | Tab ordering/chrome | Replaced by `TabEntry` from `compute_layout()` |
| `tile_invariants.rs` | 149 | Tile tree invariant checks | Built into `apply_nav()` postconditions |
| `persistence_ops.rs` | 3,586 | `FrameLayout` serialization | Replaced by `GraphTree` serde (phased migration for backward compat) |
| `navigator_context.rs` | 142 | Navigator scope/context | Replaced by `ProjectionLens` |

### Files that adapt (thin bridge to new API)

| File | Lines | Current role | Migration |
|------|-------|-------------|-----------|
| `tile_compositor.rs` | 2,857 | Per-tile render scheduling | Reads `compute_layout().pane_rects` instead of iterating egui_tiles |
| `tile_render_pass.rs` | 1,236 | Per-tile render dispatch | Dispatches from `LayoutResult` pane rects |
| `tile_post_render.rs` | 1,205 | Post-render overlays | Uses tree for overlay positioning |
| `tile_runtime.rs` | 1,015 | Runtime tile state | Thins to lifecycle bridge (`SetLifecycle` calls) |
| `compositor_adapter.rs` | 2,461 | GL/wgpu compositor adapter | Unchanged — reads rects from tree instead of egui_tiles |
| `pane_model.rs` | 703 | Pane state model | Merges into `MemberEntry` or thins to viewer-specific state |
| `interaction_policy.rs` | 121 | Interaction policy | Unchanged — policy sits above tree |
| `ux_tree.rs` | 3,408 | UxTree projection | Calls `emit_ux_tree()` instead of walking egui_tiles |
| `ux_bridge.rs` | 1,129 | UxTree egui bridge | Adapts `UxNodeDescriptor` to egui AccessKit |
| `ux_probes.rs` | 1,343 | UxTree invariant probes | Probes read from tree's structural API |
| `workspace_state.rs` | 690 | Workspace persistence | Serializes `GraphTree` instead of `Tree<TileKind>` |
| `workbench_commands.rs` | 761 | Workbench command dispatch | Routes commands to `NavAction` instead of direct tree mutation |

### Files in the frame pipeline (parameter threading)

The tile tree is currently threaded through the frame pipeline as a parameter:
`gui_frame.rs` → `gui_orchestration.rs` → `tile_render_pass.rs` → `workbench_host.rs`.
GraphTree must slot into this same pipeline, maintaining the same ownership
and borrowing discipline.

| File | Lines | Current role | Migration |
|------|-------|-------------|-----------|
| `gui_frame.rs` | ~600 | Passes `tiles_tree` to render/composition | Thread `GraphTree` instead |
| `gui_orchestration.rs` | ~1,200 | Frame phase orchestration | Thread `GraphTree` through phases |
| `gui_frame/frame_persistence.rs` | ~500 | Snapshot save/restore/prune | Use `GraphTree` serialization |
| `gui/workbench_intent_interceptor.rs` | ~400 | Intercepts intents → tile mutations | Route through `NavAction` |
| `gui/intent_translation.rs` | ~300 | Intent → tile_view_ops calls | Route through `NavAction` |

### Intent dispatch and lifecycle coordination

| File | Lines | Current role | Migration |
|------|-------|-------------|-----------|
| `app/intent_phases.rs` | ~1,500 | Phase handlers dispatching to tile_view_ops | Dispatch to `apply_nav()` |
| `lifecycle/lifecycle_reconcile.rs` | ~800 | Node pane lifecycle coordination | Bridge via `SetLifecycle` NavAction |
| `runtime/registries/workbench_surface.rs` | ~1,500 | Workbench surface registry; queries tiles_tree | Query `GraphTree` API |
| `runtime/registries/workbench_surface/focus_routing.rs` | ~300 | FocusCycleRegion dispatch | Use `CycleFocus` / `CycleFocusRegion` NavAction |
| `tile_behavior/pending_intents.rs` | ~200 | Pending intent queue | Route through `NavAction` batching |
| `app/startup_persistence.rs` | ~300 | Load persisted layout on startup | Use `GraphTree` deserialization |

### Tile behavior sub-modules (content rendering — low impact)

These render pane content and are mostly independent of tree structure:

| File | Lines | Migration |
|------|-------|-----------|
| `tile_behavior/node_pane_ui.rs` | ~1,000 | No — content rendering is tree-agnostic |
| `tile_behavior/tool_pane_ui.rs` | ~400 | No — content rendering is tree-agnostic |
| `tile_behavior/tab_chrome.rs` | ~200 | Adapt — tab title/icon rendering uses new `TabEntry` |

### Files with scattered references (need import/type updates)

~30 additional files with `egui_tiles::` imports or `Tree<TileKind>` type
references that update to `GraphTree<NodeKey>`. Mechanical changes.

---

## 4. Phased Implementation

### Phase 0: Crate scaffold + core types (1 week)

**Goal**: `graph-tree` crate compiles with all types, no logic.

- Create `graph-tree/` workspace member with zero framework deps
- Implement all core types from `graph_tree_spec.md` §6.2–6.6:
  `GraphTree<N>`, `MemberEntry`, `Lifecycle`, `Provenance`, `TreeTopology`,
  `GraphletRef`, `GraphletBinding`, `ProjectionLens`, `LayoutMode`
- Implement `Rect`, `MemberId` trait, `ViewId` trait
- Serde derives on all types
- Unit tests for type construction and serialization round-trip

**Files created**: `graph-tree/Cargo.toml`, `graph-tree/src/{lib,tree,topology,member,graphlet,lens,layout,nav,query,ux,serde_compat}.rs`

**Done gate**: `cargo test -p graph-tree` passes; types serialize/deserialize.

### Phase 1: Tree operations + navigation (2 weeks)

**Goal**: `apply_nav()` handles all `NavAction` variants with invariant enforcement.

- Implement `TreeTopology` mutation methods (attach/detach/reparent/reorder)
- Implement provenance-based placement rules (traversal→child, manual→sibling,
  derived→sibling, anchor→root)
- Implement `apply_nav()` for all `NavAction` variants
- Implement `visible_walk()` with expansion state
- Implement tree query functions (ancestors, descendants, siblings, depth)
- Property tests: tree invariants hold after any sequence of NavActions
- Implement `derive_topology()` for petgraph feature

**Done gate**: Property tests pass; all NavAction variants covered; tree never
enters invalid state (orphan nodes, cycles, missing parents).

### Phase 2: Layout computation (1 week)

**Goal**: `compute_layout()` produces correct rects for all layout modes.

- Implement taffy tree builder from GraphTree topology
- Implement `TreeStyleTabs` layout (tree rows + single active pane rect)
- Implement `FlatTabs` layout (tab order + single active pane rect)
- Implement `SplitPanes` layout (taffy-computed rects per active member)
- Implement `LayoutOverride` application (min/max, flex)
- Unit tests: layout produces non-overlapping rects; tree rows respect expansion

**Done gate**: Layout tests pass for all three modes; taffy integration works.

### Phase 3: UxTree + accessibility emission (1 week)

**Goal**: `emit_ux_tree()` produces correct `UxNodeDescriptor` tree.

- Implement `UxNodeDescriptor` builder from GraphTree state
- Map `LayoutMode` to appropriate UxRole hierarchy (TreeView/TabList/SplitContainer)
- Verify against `ux_tree_and_probe_spec.md` completeness contract
- Unit tests: every visible member produces at least one UxNode

**Done gate**: UxNode tree is structurally complete for all layout modes.

### Phase 4: egui adapter + shell integration (2–3 weeks)

**Goal**: GraphTree renders inside Graphshell's WorkbenchArea, replacing egui_tiles.

This is the big integration phase. It proceeds in sub-steps:

#### 4a. GraphTreeRenderer for egui (1 week)

- Implement `GraphTreeRenderer` trait for egui
- `render_tree_tabs()`: tree-style tab sidebar with lifecycle badges, expansion,
  provenance indicators
- `render_flat_tabs()`: traditional tab bar (fallback mode)
- `render_pane_chrome()`: split pane borders and resize handles
- Wire into `shell_layout_pass.rs` WorkbenchArea slot

#### 4b. Compositor bridge (3–4 days)

- Replace egui_tiles iteration in `tile_compositor.rs` with `compute_layout().pane_rects`
- Replace egui_tiles iteration in `tile_render_pass.rs`
- Verify compositor adapter still receives correct rects and render modes

#### 4c. Command routing (3–4 days)

- Replace direct `tile_view_ops.rs` calls with `NavAction` dispatch
- Wire `workbench_commands.rs` to emit `NavAction` → `apply_nav()` → `TreeIntent`
- Convert `TreeIntent` to `WorkbenchIntent` at the host boundary
- Verify all existing workbench keyboard shortcuts still work

#### 4d. Navigator projection (3–4 days)

- Replace `navigator_context.rs` scope management with `ProjectionLens`
- Wire Navigator sidebar to read `visible_rows()` from GraphTree
- Implement lens switching UI (Traversal / Arrangement / Containment / Semantic / Recency / All)
- Verify click grammar works identically to current Navigator

**Done gate**: Graphshell renders with GraphTree; all existing tile operations
work; Navigator sidebar shows correct projection; compositor renders content
in correct rects.

### Phase 5: Persistence migration (1 week)

**Goal**: Frame layout save/restore works with GraphTree serialization.

- Implement `GraphTree` → JSON serialization (already covered by serde derives)
- Write migration function: `FrameLayout` (old) → `GraphTree<NodeKey>` (new)
- Backward-compatible loading: detect old format, migrate on load
- Forward persistence: save as GraphTree JSON
- Test: load old layout, verify correct tree structure, save, reload

**Done gate**: Existing saved layouts load correctly; new saves use GraphTree format.

### Phase 6: Graphlet binding + reconciliation (2 weeks)

**Goal**: Linked graphlet bindings auto-update tree membership from graph truth.

- Implement `EdgeProjectionSpec` consumption from graph truth
- Implement `Linked` binding: graph membership changes → tree roster updates
- Implement `Forked` binding detection (manual override of linked membership)
- Implement roster delta emission for reconciliation UI
- Implement containment lens derivation from `ContainmentRelation` edges
- Test: add/remove node from graphlet → tree updates automatically
- Test: manual override → binding forks with reason

**Done gate**: Linked graphlets stay synchronized; forks are detectable and
carry reason metadata.

### Phase 7: Cleanup + retirement (1 week)

**Goal**: Remove egui_tiles dependency entirely.

- Remove `egui_tiles` from `Cargo.toml`
- Delete `tile_view_ops.rs` (replaced by `apply_nav()`)
- Delete `tile_kind.rs` (replaced by `MemberEntry`)
- Delete `tile_grouping.rs` (replaced by `GraphletRef`)
- Delete `tile_invariants.rs` (replaced by `apply_nav()` postconditions)
- Delete `semantic_tabs.rs` (replaced by `TabEntry`)
- Thin `tile_behavior.rs` to `GraphTreeRenderer` adapter
- Update all 53 files with `egui_tiles` imports
- Final test pass: `cargo test` green, `cargo clippy` clean

**Done gate**: Zero `egui_tiles` references remain; all tests pass.

---

## 5. Lane Registration

GraphTree should be registered in PLANNING_REGISTER §1D as:

```
lane:graph-tree — GraphTree crate: framework-agnostic graphlet-native tile tree
  replacing egui_tiles, collapsing Navigator/Workbench projection gap
```

**Section**: C (UX / Interaction / Graph Capability) or B (Core Platform)

**Dependencies**:
- Arrangement-graph-projection plan (shipped — provides the arrangement-edge model)
- Relation families (design — provides containment/traversal/semantic edge vocabulary)
- Shell composition model (implemented — provides WorkbenchArea slot)

**Dependents (unblocked by GraphTree)**:
- NV15 Navigator Arrangement Projection
- NV23 Workbench-Navigator Contract Sync
- Constellation projection plan (can consume richer tree)
- Extension/PWA tile tree (framework-agnostic tree serializes to JSON)

---

## 6. Risk Mitigation

| Risk | Mitigation |
|------|-----------|
| Scope creep into full reconciliation UI | Phase 6 implements binding mechanics only; reconciliation UI (fork/rebase/unlink chooser) is Phase 7+ |
| Compositor regression | Phase 4b runs compositor tests against both old and new pane rects before switching |
| Persistence migration breaks saved layouts | Phase 5 detects old format and migrates; never overwrites until verified |
| Performance regression (taffy vs proportional) | Benchmark layout computation in Phase 2; taffy is fast but verify |
| Navigator behavior regression | Phase 4d tests click grammar against existing interaction contract acceptance criteria |
| 53-file import churn | Phase 7 is mechanical; can be done as a single well-tested PR |

---

## 7. Verification

### Per-phase gates (listed above)

### End-to-end verification

- All existing tile operations work (split, close, tab switch, drag-drop, focus cycle)
- Navigator sidebar shows correct tree with lifecycle badges
- Lens switching shows different projections of same membership
- Graph canvas frame minimap renders correct frame bounding boxes
- UxTree probes pass with GraphTree-sourced structure
- Saved layouts round-trip through new persistence format
- Linked graphlet membership updates when graph changes
- Cold members visible in Navigator with correct badges
- `cargo test` green, `cargo clippy` clean, zero `egui_tiles` references
