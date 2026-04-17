<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Iced Host Migration Execution Plan (2026-04-14)

**Status**: Active strategy / execution checklist
**Scope**: A robust, future-facing migration path from the current egui host to
an iced host, while minimizing rewrite cost by first making `graph-tree`,
`graph-canvas`, and the compositor/runtime boundaries authoritative.

**Related**:

- `SHELL.md`
- `shell_backlog_pack.md`
- `../workbench/2026-04-11_graph_tree_egui_tiles_decoupling_follow_on_plan.md`
- `../workbench/2026-04-11_egui_tiles_retirement_strategy.md`
- `../graph/2026-04-11_graph_canvas_crate_plan.md`
- `../graph/2026-04-13_graph_canvas_phase0_plan.md`
- `../graph/GRAPH.md`
- `../aspect_render/2026-04-12_rendering_pipeline_status_quo_plan.md`
- `../../research/2026-04-10_ui_framework_alternatives_and_graph_tree_discovery.md`
- `../../technical_architecture/graph_tree_spec.md`
- `../../technical_architecture/graph_canvas_spec.md`

**Implementation anchors**:

- `Cargo.toml`
- `render/mod.rs`
- `render/canvas_bridge.rs`
- `shell/desktop/ui/gui.rs`
- `shell/desktop/workbench/graph_tree_dual_write.rs`
- `shell/desktop/workbench/graph_tree_facade.rs`
- `shell/desktop/workbench/tile_render_pass.rs`
- `shell/desktop/workbench/tile_compositor.rs`

---

## 1. Decision

The recommended path is:

**do not port the current egui shell directly to iced.**

Instead:

1. make `GraphTree` the real workbench authority
2. make `graph-canvas` the live graph surface
3. make the compositor and shell runtime host-neutral
4. add iced as a second host
5. port chrome last

This treats the UI framework as a backend, not as the owner of durable meaning.

That is the most robust path because the two expensive framework-bound systems
in the current repo are exactly the two systems Graphshell is already trying to
escape:

- `egui_tiles` for workbench/layout authority
- `egui_graphs` for graph rendering and interaction

If we port chrome first, we rewrite around both temporary dependencies and then
rewrite again when we retire them. If we move authority first, iced becomes a
host swap instead of a whole-app rewrite.

---

## 2. Current Reality

Current code-verified status:

- the desktop shell still depends directly on `egui`, `egui-winit`,
  `egui-wgpu`, `egui_graphs`, and `egui_tiles`
- `Gui` still stores `egui_tiles::Tree<TileKind>` as the canonical runtime pane tree
- `GraphTree` is already in an advanced dual-write / parallel phase rather
  than an early shadow phase: cycle-safe topology, provenance-aware attach,
  `NavAction` routing, incremental sync, structure-aware parity diagnostics,
  and dual-write mutation adapters are already present
- the remaining `GraphTree` work surface is real but bounded:
  `graph_tree_facade.rs` documents roughly 63 files / ~1646 tile references
  across pane identity, layout rects, mutations, frame threading, persistence,
  and compositor coupling
- the live graph renderer still runs through `egui_graphs`
- `graph-canvas` is much farther along than a mere scaffold: scene derivation,
  camera, projection, hit testing, interaction engine, physics, simulation,
  scripting hooks, and a Vello backend are already implemented in the portable
  crate; the remaining work is making that seam authoritative on the live host path

Implication:

- an iced port is now feasible in principle
- an iced cutover is not yet cheap
- the right next work is authority migration, not shell repainting

---

## 3. North Star

Target architecture:

- `graphshell-core` owns portable graph truth and related domain state
- `graph-tree` owns workbench/navigator tree semantics and layout intent
- `graph-canvas` owns graph-scene derivation, camera, interaction grammar, hit
  testing, and render packets
- the compositor/viewer bridge owns content-surface import and pane/content
  composition with no framework-specific authority
- egui and iced are thin host adapters that:
  - mount surfaces
  - provide viewport rectangles
  - translate raw input events
  - render chrome using host-local widgets

This makes framework choice annoying rather than existential.

---

## 4. Sequence Rules

These rules are part of the plan, not optional style preferences.

- Do not start by porting toolbar, settings, dialogs, or command palette to
  iced while `egui_tiles` and `egui_graphs` still own live authority.
- Do not let `iced` types leak into `graph-tree`, `graph-canvas`, compositor
  boundaries, or future presenter/runtime layers.
- Do not replace egui and GraphTree authority in the same milestone.
- Keep egui alive as the reference host until iced can drive the same runtime
  and produce parity receipts.
- Prefer dual-host overlap over a one-shot framework cutover.

---

## 5. Milestone Plan

### M0. Guardrails and Replay Harness

**Why first**: authority migration and host migration are both risky if we
cannot detect drift. We need a parity harness before we start pulling owners
apart.

Checklist:

- [x] Add a host-neutral event replay format for pointer, keyboard, wheel,
  focus, and command-surface actions
- [x] Add `GraphTree` parity receipts that compare topology, active member,
  expansion state, visible ordering, and visible pane set
  — `graph-tree/src/parity.rs` implements `compare()` with 7 divergence types;
  `graph_tree_sync::parity_check()` runs per-frame in debug builds
- [x] Add graph-canvas packet snapshots for representative graph views
- [x] Add host-level golden tests for command routing, focus transitions, and
  pane activation
- [x] Add a "same state in -> same runtime outputs out" test seam that can be
  shared by egui and iced hosts later

Done gate:

- authority shifts can be verified with structure-aware parity, not just visual
  spot checks

### M1. Make `GraphTree` Authoritative

**Goal**: stop treating `GraphTree` as a shadow and make it the semantic owner
of workbench/navigator tree state.

This milestone starts from an effective "Phase 4b" posture: dual-write is
already real, destructive per-frame rebuild has already been replaced by
incremental sync, and the first true authority step is removing the remaining
frame-by-frame follower sync from `Gui`.

Checklist:

- [x] Replace destructive per-frame `rebuild_from_tiles(...)` usage with
  incremental sync that preserves topology and provenance
- [x] Land topology-safe attach/reparent behavior, including root fallback for
  unreachable provenance sources
- [x] Land structure-aware parity diagnostics
- [x] Land dual-write adapters that pair tile mutations with
  `graph_tree_commands`
- [x] Remove the remaining per-frame `incremental_sync_from_tiles(...)`
  follower path from `Gui` — this is the milestone-defining authority shift
  — removed 2026-04-15; parity check retained as `log::warn!` to catch any
  remaining dual-write gaps
- [x] Keep only startup import from tile state plus explicit repair tooling
  — startup `incremental_sync_from_tiles` at `gui.rs:482` is the sole
  remaining caller; per-frame follower path is gone
- [x] Route open/activate/dismiss/reveal/toggle-expand through
  `graph_tree_commands` first, closing the remaining direct tile-mutation paths
  — 2026-04-15: routed 11 bypass mutation call sites through dual-write
  (`ux_bridge.rs` 8 calls, `tile_render_pass.rs` 2 calls, `gui.rs` 1 call);
  only read-only queries and graph-view-pane opens (not GraphTree members)
  remain as direct `tile_view_ops` calls
- [x] Make navigator/sidebar/tree-tab/focus-cycle reads resolve from
  `GraphTree` or graph truth, not `egui_tiles` — Workbench section sourced
  from GraphTree (Phase C, via `graph_tree_projection.rs`); tree-style and
  flat-tab rendering read from GraphTree via `graph_tree_adapter.rs`.
  Remaining sections (Folders, Domain, Recent, Imported) correctly read from
  `graph_app.domain_graph()` (graph truth) — **not** from `egui_tiles`. These
  are full-graph-domain projections (URL containment, domain grouping, import
  provenance, recency) that must see all nodes, not just workbench members.
  Moving them to GraphTree would lose non-workbench nodes. No tile dependency
  to remove.
- [x] Re-key GraphTree persistence per `GraphViewId`
  — `gui.rs` Drop impl serializes GraphTree keyed by `workbench_view_id`
- [x] Shrink `egui_tiles` to a rendering/presentation adapter rather than a peer
  semantic owner
  — 2026-04-15: all semantic mutations route through dual-write; `on_tab_close`
  Behavior callback now notifies GraphTree via post-render dismiss; tile
  drag-drop is presentation-only (reorder, not add/remove); per-frame follower
  sync removed. Remaining `egui_tiles` role: startup restore (one-time import)
  and presentation rendering. `egui_tiles` is no longer a semantic peer.
- [x] Remove the now-dead `rebuild_from_tiles(...)` helper
  — removed 2026-04-14; `incremental_sync_from_tiles` is the only remaining
  sync path

Done gate:

- no frame path syncs `GraphTree` from tiles
- semantic grouping and focus truth come from `GraphTree`
- `egui_tiles` is presentation-only
- persisted tree state is per view

### M2. Make `graph-canvas` the Live Graph Surface

**Goal**: replace the hot-path `egui_graphs` graph renderer with the portable
graph-canvas seam while staying inside the egui shell.

This milestone is primarily a **host-wiring and authority shift**, not a
greenfield graph-canvas build. The portable crate is already substantially
implemented; what remains is making the egui host consume it as the live graph
surface and retiring the current `egui_graphs` hot path.

Checklist:

- [ ] Move the live graph-view scene derivation path to `graph-canvas`
- [ ] Move the live graph interaction grammar to portable `CanvasInputEvent`
  and `CanvasAction` flows
- [ ] Keep Graphshell-owned camera semantics outside framework metadata/state
- [x] Add an egui host adapter that consumes `graph-canvas` packets and emits
  host-local paint/input glue only
  - landed as `render::render_graph_canvas_in_ui` plus
    `render/canvas_bridge.rs` and `render/canvas_egui_painter.rs`; primary and
    specialty graph hosts can now route through the same profile-gated M2 path
  - 2026-04-16 follow-on: extracted a host-neutral
    `canvas_bridge::run_graph_canvas_frame(...)` seam so the egui adapter now
    mainly does viewport translation, input translation, and packet painting;
    the future iced adapter can consume the same frame runner instead of
    re-owning scene derivation and interaction state
- [ ] Prefer the existing `graph-canvas` Vello backend as the shared rendering
  convergence point where practical, so egui and iced can consume the same
  graph-render backend rather than each owning separate paint logic
- [ ] Preserve current graph overlays and panels around the new canvas seam
- [ ] Remove live dependence on `egui_graphs` from the graph pane hot path

Done gate:

- graph panes render and interact through `graph-canvas`, not `egui_graphs`

### M3. Rekey the Compositor Around Portable Identity

**Goal**: make composition care about `NodeKey`, `PaneId`, content surfaces,
and rects, not framework-owned tree ids.

Checklist:

- [ ] Continue the shift from `TileId` to `NodeKey` / `PaneId` in compositor
  inputs and registries
- [ ] Keep `ViewerSurfaceRegistry` as the intended authority for content
  surfaces
- [ ] Make explicit that pane rect input comes from GraphTree's existing
  taffy-backed `compute_layout()` path; this milestone is about making that
  layout authoritative, not selecting a new pane layout engine
- [ ] Make content callback registration and overlay passes portable across
  host frameworks
- [ ] Keep GL fallback explicit and contained rather than architecturally central

Done gate:

- the compositor does not require egui-owned identity to schedule content
  composition

### M3.5. Runtime Boundary Design Pass

**Goal**: make the M4 extraction line explicit before implementation starts.

This is a design pass, not a "maybe later" note. M4 is where the migration will
be tested, because `Gui` currently mixes:

- durable runtime logic
- host-local rendering glue
- OS/window/event-loop wiring
- framework-owned widget and texture state

Checklist:

- [x] Classify `Gui` fields and responsibilities into:
  - durable runtime logic
  - host adapter state
  - render-backend glue
  - OS/window/event-loop integration
- [x] Write down the boundary for focus authority, command routing, toolbar
  session state, pane targeting, thumbnail/update queues, and compositor-facing
  services
- [x] Define the service-port/effect interface the host-neutral runtime will use
- [x] Define the view-model surface the egui and iced hosts will each consume
- [x] Identify what remains intentionally host-specific even after extraction

**Landed 2026-04-16**: [`2026-04-16_runtime_boundary_design.md`](2026-04-16_runtime_boundary_design.md)
captures the full classification, six `HostPorts` trait surfaces, `FrameViewModel`
shape, `FrameHostInput` shape, and explicit non-goals. M4 can proceed as a
mechanical extraction.

Done gate:

- M4 starts from an explicit runtime/host classification rather than ad hoc
  field-by-field migration

### M4. Extract a Host-Neutral Shell Runtime

**Goal**: split stateful shell/workbench orchestration from the current egui
host implementation.

Checklist:

- [ ] Identify the `Gui` responsibilities that are durable runtime logic versus
  egui host glue
- [ ] Extract presenter/runtime layers for:
  - workbench command routing
  - focus authority and return targets
  - command-palette/session state
  - toolbar/omnibar state
  - pane activation and surface targeting
- [ ] Define host-neutral view-model and effect contracts
- [ ] Make egui consume those contracts instead of owning them implicitly
- [ ] Keep all durable state transitions testable without an egui frame

Done gate:

- egui is no longer the owner of shell/workbench runtime semantics

### M5. Bring Up Iced as a Second Host

**Goal**: prove that iced can host the existing product core without forcing a
second rewrite of graph/workbench/compositor logic.

Checklist:

- [ ] Add an `iced` host behind a feature flag or separate desktop entry point
- [ ] Start with one window, one graph surface, one node pane, and minimal top
  chrome
- [ ] Implement raw input translation from iced into the shared runtime/input
  contracts
- [ ] Implement GraphTree rendering through an iced adapter
- [ ] Implement graph-canvas hosting through an iced adapter, ideally consuming
  the same Vello-backed graph-canvas renderer path proven in M2
- [ ] Mount Servo/viewer content through the same compositor/runtime boundary
- [ ] Add parity runs between egui host and iced host for the same replay inputs

Done gate:

- iced can drive the same runtime/core as egui for a useful subset of the app

### M6. Port Chrome and Reach Host Parity

**Goal**: move the remaining shell surfaces into iced only after the core
surfaces are already portable.

Checklist:

- [ ] Toolbar and omnibar chrome
- [ ] Command palette
- [ ] Settings and control surfaces
- [ ] Dialogs, toasts, and overview surfaces
- [ ] Focus rings, keyboard navigation, and AT parity
- [ ] Performance pass on layout, redraw churn, and texture lifetime
- [ ] Compare egui and iced against the same acceptance checklist

Done gate:

- iced host reaches functional parity for the intended desktop scope

### M7. Cutover and Cleanup

**Goal**: switch default host ownership intentionally, not accidentally.

Checklist:

- [ ] Decide whether egui remains as a debug/reference host
- [ ] Remove now-dead egui-only authority code
- [ ] Remove `egui_graphs` from live dependencies
- [ ] Remove or minimize `egui_tiles` if still only acting as a compatibility
  presenter
- [ ] Archive the host-migration plan with concrete closure receipts

Done gate:

- framework choice is a host concern, not a product-core concern

---

## 6. Immediate Ticket Queue

This is the recommended first task stack, in order.

- [x] Replace destructive per-frame `rebuild_from_tiles(...)` usage with
  incremental sync
- [x] Land topology-safe attach/reparent behavior with root fallback
- [x] Upgrade `GraphTree` parity diagnostics to structure-aware checks
- [x] Remove per-frame `incremental_sync_from_tiles(...)` from `Gui`
- [x] Route remaining semantic workbench commands through
  `graph_tree_commands` (close direct `tile_view_ops` bypasses)
- [x] Move navigator/tree-tab projections to `GraphTree`-only reads
  (Workbench section from GraphTree; Folders/Domain/Recent/Imported
  correctly read from graph truth, not tiles — no tile dependency to remove)
- [x] Finish per-view `GraphTree` persistence migration
- [x] Remove the dead `rebuild_from_tiles(...)` helper
- [ ] Replace live graph packet derivation with `graph-canvas`
- [ ] Replace live graph input resolution with `graph-canvas` actions/events
- [ ] Land the egui graph-canvas host adapter as the live path
- [ ] Rekey compositor adapters from `TileId` toward `NodeKey` / `PaneId`
- [ ] Split `Gui` into runtime/presenter plus egui adapter responsibilities
- [ ] Scaffold iced host entry point with one graph surface
- [ ] Add iced `GraphTree` adapter
- [ ] Add iced `graph-canvas` adapter
- [ ] Add host parity replay tests

---

## 7. Deliberate Sidequests With High Payoff

These are optional only in the sense that we can choose when to do them. They
are explicitly good investments.

### Sidequest A. Portable Shell Runtime Crate or Module Boundary

Why:

- This is the highest-payoff structural cleanup for host portability.
- It turns "port UI" into "implement host adapter."

Payoff:

- smaller iced diff
- smaller future host diff
- better runtime testability

### Sidequest B. Replay and Receipt Infrastructure

Why:

- Host migration without deterministic replay drifts into manual QA theatre.

Payoff:

- confidence during dual-host overlap
- reusable diagnostics for future regressions

### Sidequest C. Advance the Shared Vello Path

Why:

- If both egui and iced hosts can consume the same Vello-backed graph backend,
  we avoid host-specific graph painting logic.
- In practice this may stop being a sidequest and become a critical-path
  convergence step between M2 and M5, depending on how thin the non-Vello host
  adapters remain.

Payoff:

- cleaner host adapters
- better long-term rendering path
- future leverage for non-egui, non-iced hosts

### Sidequest D. Accessibility Above the Framework Layer

Why:

- Rebuilding AT semantics during host migration is pure waste if we can lift
  UxTree / semantic projection above the host boundary first.

Payoff:

- less duplicated host-specific accessibility work
- more trustworthy parity checks

---

## 8. What Not To Do

Avoid these tempting but bad paths:

- Port the toolbar first because it feels visually concrete.
- Add iced widgets directly against current `Gui` state while `Gui` still owns
  egui-specific semantics.
- Keep `GraphTree` and `egui_tiles` as long-term peer authorities.
- Keep `graph-canvas` portable in theory but not on the live path.
- Treat a partial iced demo as evidence that the migration architecture is done.

---

## 9. Acceptance Shape

This migration path is successful when Graphshell can truthfully say:

- `GraphTree` owns workbench/navigator tree semantics
- `graph-canvas` owns graph-surface rendering and interaction semantics
- compositor/viewer composition is portable across hosts
- shell/workbench runtime semantics are testable without an egui frame
- iced and egui can both host the same core for a period of overlap
- framework choice no longer determines product architecture

---

## 10. Progress Log

### 2026-04-14 — Initial execution session

**Dependency upgrades (graph-canvas + middlenet-engine)**:

- Upgraded linebender ecosystem to current versions: vello 0.6→0.8, peniko
  0.5→0.6, parley 0.1→0.8. Zero API changes required — all existing
  `backend_vello.rs` usage (Scene::fill, Scene::stroke, AlphaColor, Fill) was
  stable across versions.
- fontique upgraded transitively from 0.1→0.8, fixing the
  `unresolved import system` WASM breakage in middlenet-engine. skrifa
  consolidated from dual versions (0.19/0.22) to single 0.40.
- All WASM targets verified: `graph-canvas` (plain, physics, simulate, vello)
  and `middlenet-engine` compile for `wasm32-unknown-unknown`.

**M1 cleanup**:

- Removed dead `rebuild_from_tiles(...)` helper from `graph_tree_sync.rs` —
  zero callers remained after the Phase B incremental sync replacement.
- Fixed pre-existing borrow checker error in `graph-tree/src/topology.rs`
  `reorder_children()` — `HashSet<&N>` caused immutable/mutable borrow
  conflict; changed to `HashSet<N>`.
- Fixed missing `#[cfg]` gate and unqualified `HashSet` on
  `traverse_tile_tree()` in `graph_tree_sync.rs`.
- Full workspace now compiles cleanly; 104 graph-tree tests pass (102 unit +
  2 property tests); 138 graph-canvas tests pass.

**Verified done (code-checked)**:

- M1: topology-safe attach with root fallback (`tree.rs:700–704`)
- M1: structure-aware parity diagnostics (`parity.rs`, 7 divergence types,
  13 tests)
- M1: dual-write adapters (`graph_tree_dual_write.rs`, 11 wrapper functions)
- M1: per-view GraphTree persistence (`gui.rs` Drop impl, keyed by
  `workbench_view_id`)
- M1: `rebuild_from_tiles` removed

### 2026-04-15 — M1 authority shift landed

**Dual-write bypass closure**:

- Routed 11 direct `tile_view_ops` mutation call sites through
  `graph_tree_dual_write`: `ux_bridge.rs` (8 calls covering open/focus/close/
  dismiss/tool pane operations), `tile_render_pass.rs` (2 calls for graph
  interaction–driven node opens), `gui.rs` (1 call for clip node creation).
- Only read-only queries (`active_graph_view_id`) and graph-view-pane opens
  (not GraphTree members) remain as direct `tile_view_ops` calls.
- Added `graph_tree` parameter threading through `ux_bridge::handle_runtime_command`,
  `apply_workbench_intent`, `tile_render_pass::render_specialty_graph_in_ui`,
  `tile_render_pass::render_primary_graph_in_ui`, and `TestRegistry`.

**Per-frame follower sync removed**:

- Removed the per-frame `incremental_sync_from_tiles(...)` call from `Gui`.
  GraphTree no longer follows tiles — it is updated only through dual-write
  mutation paths and the one-time startup import.
- Retained the parity check as `log::warn!` in debug builds to catch any
  remaining bypass gaps.
- `incremental_sync_from_tiles` has exactly one remaining caller: the startup
  import at `gui.rs:482`, which reconciles GraphTree with tiles restored from
  persistence.

**Also landed**: `WorkflowSavepoint` — registry-level transaction savepoint
for workflow activation, with early `implemented` check and rollback on failure
(`workflow.rs`, `registries/mod.rs`).

**Next**: move navigator/tree-tab projections to GraphTree-only reads
(M1 remaining item), then begin M2 (graph-canvas as live surface).

---

## 11. Summary

The interesting version of this migration is not "rewrite egui in iced."

It is:

- replace framework-owned graph and tile authority with Graphshell-owned cores
- turn the current framework into a host adapter
- add iced only after the seams are real

That path costs more upfront, but it prevents paying the same migration tax
twice.
