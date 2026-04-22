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

**Status**: Done gate met (2026-04-21). Live graph panes render and interact
through `graph-canvas`, not `egui_graphs`. Final validation was gated for
several days on disk-space availability (~700 GB target directory against
<1 GB free); confirmed on disk recovery 2026-04-21.

**Goal**: replace the hot-path `egui_graphs` graph renderer with the portable
graph-canvas seam while staying inside the egui shell.

This milestone was primarily a **host-wiring and authority shift**, not a
greenfield graph-canvas build. The portable crate was already substantially
implemented; what remained was making the egui host consume it as the live
graph surface and retiring the `egui_graphs` hot path.

Checklist:

- [x] Move the live graph-view scene derivation path to `graph-canvas`
  — landed; final validation 2026-04-21 on disk recovery
- [x] Move the live graph interaction grammar to portable `CanvasInputEvent`
  and `CanvasAction` flows — landed
- [x] Keep Graphshell-owned camera semantics outside framework metadata/state
  — landed
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
- [x] Prefer the existing `graph-canvas` Vello backend as the shared rendering
  convergence point where practical, so egui and iced can consume the same
  graph-render backend rather than each owning separate paint logic — landed
- [x] Preserve current graph overlays and panels around the new canvas seam
  — landed
- [x] Remove live dependence on `egui_graphs` from the graph pane hot path
  — landed

Done gate:

- graph panes render and interact through `graph-canvas`, not `egui_graphs` ✅

### M3. Rekey the Compositor Around Portable Identity

**Status**: Done gate met (2026-04-20). The compositor hot path already
keys composition on `NodeKey` / `PaneId`; the identity-rekey work the
original checklist anticipated was mostly absorbed into M1 prep. What
remained after M1 is host-specific *presentation* extraction, which
has its own slice: M3.5 below.

**Goal**: make composition care about `NodeKey`, `PaneId`, content surfaces,
and rects, not framework-owned tree ids.

Checklist:

- [x] Continue the shift from `TileId` to `NodeKey` / `PaneId` in compositor
  inputs and registries — done as part of M1; the compositor's static
  registries (`COMPOSITOR_CONTENT_CALLBACKS`,
  `COMPOSITOR_NATIVE_TEXTURES`, `COMPOSITED_CONTENT_SIGNATURES`,
  `LAST_SENT_NATIVE_OVERLAY_RECTS`) all key on `NodeKey`. Frame inputs
  (`active_node_pane_rects_from_graph_tree`) produce
  `(PaneId, NodeKey, egui::Rect)` tuples, not `TileId`.
- [x] Keep `ViewerSurfaceRegistry` as the intended authority for content
  surfaces — `shell/desktop/workbench/compositor_adapter.rs::ViewerSurfaceRegistry`
  owns `HashMap<NodeKey, ViewerSurface>` and is the single consumer seam
  the host passes through.
- [x] Make explicit that pane rect input comes from GraphTree's existing
  taffy-backed `compute_layout()` path — the frame-loop entry point in
  `tile_render_pass.rs` reads rects out of GraphTree + `node_pane_ids:
  HashMap<NodeKey, PaneId>` before handing them to the compositor.
- [x] Keep GL fallback explicit and contained rather than architecturally
  central — GL paths are feature-gated (`#[cfg(feature = "gl_compat")]`),
  limited to `compositor_adapter.rs`, and expressed as a named
  `ContentSurfaceHandle::CallbackFallback` variant distinct from the
  primary wgpu path.
- [~] Make content callback registration and overlay passes portable
  across host frameworks — **overlay-pass painting was extracted on
  2026-04-20** (see `OverlayAffordancePainter` trait +
  `EguiOverlayAffordancePainter` impl in `compositor_adapter.rs`).
  Content-callback registration is still egui-adjacent and is tracked
  under M3.5 below.

Done gate:

- [x] the compositor does not require egui-owned identity to schedule
  content composition — met: identity is `NodeKey` / `PaneId` everywhere
  in the hot path; the remaining egui coupling is in presentation
  (painting, callback registration), not identity.

### M3.5. Host-Neutral Presentation Extraction

**Status**: Done gate met (2026-04-21). Overlay-pass painting
landed 2026-04-20; content-callback executor trait, euclid-typed
descriptors, and iced stub-painter verification all landed 2026-04-21.
Full `cargo check --lib` clean (5m 35s, 91 pre-existing warnings,
no new warnings from this slice).

**Scope clarification (2026-04-21)**: the `HostPaintPort` trait in
`shell/desktop/ui/host_ports.rs` deliberately keeps `egui::Rect` /
`egui::Stroke` / `egui::Color32` in its method signatures as a
documented "cosmetic leak" iced converts at the boundary. That is by
design — slice 2's descriptor conversion targets the `OverlayStrokePass`
struct in `compositor_adapter.rs` and the new narrow painter traits
(`OverlayAffordancePainter`, `ContentPassPainter`), not
`HostPaintPort`'s trait methods. The two layers remain separate by
intent: `HostPaintPort` is the broader host-facade trait (consumed by
`EguiHostPorts` / `IcedHostPorts`); the narrow painter traits are the
compositor's internal extraction seam.

**Goal**: the compositor's presentation layer can be swapped for an
iced equivalent without touching any identity, scheduling, or
descriptor-generation code.

The M3 checklist originally folded "make overlay and content-callback
passes portable" into identity rekeying. On investigation they're
independent: descriptor generation is already host-neutral; painting
and callback invocation are the host-specific seams.

Checklist:

- [x] **Overlay-pass painting** (landed 2026-04-20): `OverlayAffordancePainter`
  trait + `EguiOverlayAffordancePainter` impl. The existing
  `execute_overlay_affordance_pass(ctx, ..)` entry point now wraps a
  new `execute_overlay_affordance_pass_with_painter(painter, ..)`
  that dispatches per-overlay through the trait. iced implements
  `OverlayAffordancePainter` with its own painting APIs and plugs in
  at the single call site. Verified by the
  `overlay_affordance_pass_routes_through_painter_trait` test — a
  non-egui `RecordingPainter` observes every descriptor in order.
- [x] **Content-callback executor** (landed 2026-04-21):
  `ContentPassPainter` trait + `EguiContentPassPainter` impl mirroring
  the overlay-pass extraction pattern. Two methods:
  `register_content_callback_on_layer` (for `ParentRenderCallback`
  bridges) and `paint_native_content_texture` (for `SharedWgpuTexture`
  bridges). The existing `compose_webview_content_pass(ctx, ..)` and
  `compose_registered_content_pass(ctx, ..)` entry points now wrap
  new `_with_painter` variants that route through the trait;
  backwards-compat wrappers construct an `EguiContentPassPainter` and
  delegate. Verified by the `content_pass_routes_through_painter_trait`
  test — a non-egui `RecordingPainter` observes the register-callback
  flow through `compose_registered_content_pass_with_painter`.
- [x] **Euclid-typed descriptors + iced stub-painter verification**
  (landed 2026-04-21, combined slice per plan note). `OverlayStrokePass.tile_rect`
  is now `PortableRect` (= `euclid::default::Rect<f32>`), `.stroke` is
  now `graph_canvas::packet::Stroke`. The narrow painter traits
  (`OverlayAffordancePainter` descriptor field, `ContentPassPainter`
  method signatures) consume portable types directly; `EguiOverlayAffordancePainter`
  and `EguiContentPassPainter` convert via `egui_rect_from_portable` /
  `egui_stroke_from_portable` at the draw-call boundary. Producer
  sites in `tile_compositor.rs` (focus / selection / hover / semantic
  overlay producers) emit portable types via `portable_rect_from_egui` /
  `portable_stroke_from_egui` at struct construction.
  `IcedOverlayAffordancePainter` and `IcedContentPassPainter`
  log-and-count stubs added to `shell/desktop/ui/iced_host_ports.rs` —
  validate the trait boundary from the iced-host side, consume portable
  types directly without egui conversion. Full `cargo check --lib`
  clean. (`HostPaintPort` trait signatures deliberately remain on
  egui types per the scope clarification — that is a separate
  "cosmetic leak" iced converts at its own boundary.)

Done gate:

- no host-specific painting code lives on `CompositorAdapter`'s static
  surface; overlay + content passes invoke traits that the egui and
  iced hosts implement independently.

### M3.6. HostPorts Cosmetic-Leak Cleanup

**Status**: Done gate met (2026-04-21). All three surfaces converted to
portable types; "cosmetic leak" language removed from trait and
viewmodel docstrings; `cargo check --lib` clean (1m 47s, same 91
pre-existing warnings, no new errors or warnings).

**Goal**: remove residual `egui::*` types from the runtime-host boundary
vocabulary before M4 solidifies that boundary as the extraction line. The
M3.5 design identified three surfaces that carry egui types as "cosmetic
leaks iced converts at its own boundary"; M3.6 replaces those with
portable types so the boundary is truly host-neutral and M4's extraction
starts from a clean foundation.

Scope (three surfaces, each already identified in code comments):

- `HostPaintPort` trait methods (host_ports.rs §§HostPaintPort) — replace
  `egui::Rect` / `egui::Stroke` / `egui::Color32` with `PortableRect` /
  `graph_canvas::packet::Stroke` / `graph_canvas::packet::Color`.
- `HostInputPort::pointer_hover_position` (host_ports.rs) and
  `FrameHostInput::pointer_hover` (frame_model.rs) — replace
  `Option<egui::Pos2>` with `Option<euclid::default::Point2D<f32>>`.
- `FrameViewModel::active_pane_rects` and `FrameHostInput::viewport_size`
  (frame_model.rs) — replace `egui::Rect` with `PortableRect` and
  `egui::Vec2` with `euclid::default::Size2D<f32>`.

Impl pattern: egui impls convert via the boundary helpers added in M3.5
(`portable_rect_from_egui`, `egui_rect_from_portable`,
`portable_stroke_from_egui`, `egui_stroke_from_portable`) plus new
point/size helpers. iced impls consume portable types directly (no egui
dependency).

Rationale: M3.5 slice 2 demonstrated that the conversion is tractable —
~15 minutes per surface plus small helper functions. Paying it now, as
a focused prelude slice, keeps M4's diff purely about ownership
extraction (not type renames) and removes the "cosmetic leak" class
from the host-port vocabulary.

Checklist:

- [x] Add `PortablePoint` / `PortableSize` type aliases + conversion
  helpers next to `PortableRect` in `compositor_adapter.rs`.
- [x] `HostPaintPort` trait method signatures: portable types throughout.
  Drop the "cosmetic leak" paragraph from the trait docstring.
- [x] `HostInputPort::pointer_hover_position` return type: portable point.
- [x] `FrameViewModel::active_pane_rects` field type: portable rect.
- [x] `FrameHostInput::pointer_hover` field type: portable point.
- [x] `FrameHostInput::viewport_size` field type: portable size.
- [x] Drop "residual egui leaks" docstring paragraph from frame_model.rs.
- [x] `egui_host_ports.rs`: update the eight affected stub signatures to
  match the new trait surface.
- [x] `iced_host_ports.rs`: update the eight affected stub signatures to
  match; drop "(Note: the port signature still leaks `egui::Pos2` ...)"
  comments.
- [x] `gui.rs` build_frame_host_input site: convert egui source types to
  portable when populating `FrameHostInput`.
- [x] `gui_state.rs` project_view_model: convert egui source types to
  portable when populating `FrameViewModel`.
- [x] `cargo check --lib` clean (1m 47s, no new warnings).

Done gate:

- No `egui::*` types appear in `host_ports.rs` trait method signatures,
  `FrameViewModel` fields, or `FrameHostInput` fields. "Cosmetic leak"
  language is absent from all related docstrings. Build is clean.

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

**Landed 2026-04-16**: [`../../../archive_docs/checkpoint_2026-04-17/graphshell_docs/implementation_strategy/shell/2026-04-16_runtime_boundary_design.md`](../../../archive_docs/checkpoint_2026-04-17/graphshell_docs/implementation_strategy/shell/2026-04-16_runtime_boundary_design.md)
captures the full classification, six `HostPorts` trait surfaces, `FrameViewModel`
shape, `FrameHostInput` shape, and explicit non-goals. M4 can proceed as a
mechanical extraction.

Done gate:

- M4 starts from an explicit runtime/host classification rather than ad hoc
  field-by-field migration

### M4. Extract a Host-Neutral Shell Runtime

**Status (2026-04-22)**: Partially landed. `GraphshellRuntime` owns all
M3.5 Category A fields. M4.1 (focus authority) and M4.4 (thumbnail /
update queues) have landed their first sub-slices through transitional
bundle abstractions (`FocusAuthorityMut`, `ThumbnailChannel` +
`BackendThumbnailPort`); M4.2 (toolbar/omnibar session state) and M4.3
(command routing) have landed in parallel via `ToolbarAuthorityMut`,
`CommandAuthorityMut`, `GraphSearchAuthorityMut`, and the
`OmnibarSearchSession` / `CommandPaletteSession` runtime-owned state.
Full build + test verification is gated on the
`webrender-wgpu` SPIR-V/naga migration. See Progress Log 2026-04-22.

**Goal**: split stateful shell/workbench orchestration from the current egui
host implementation.

Checklist:

- [x] Identify the `Gui` responsibilities that are durable runtime logic versus
  egui host glue — landed via M3.5 runtime boundary design
  (`archive_docs/checkpoint_2026-04-17/.../2026-04-16_runtime_boundary_design.md`)
- [x] Extract presenter/runtime layers for:
  - [x] focus authority and return targets (M4.1 slices 1a–1d) —
    `FocusAuthorityMut` transitional bundle + `FocusRingSettings`
    user-configurable surface
  - [x] toolbar/omnibar session state — `ToolbarAuthorityMut`,
    `OmnibarSearchSession` on runtime
  - [x] command-palette/session state — `CommandAuthorityMut`,
    `CommandPaletteSession` on runtime
  - [x] workbench command routing — `GraphSearchAuthorityMut` +
    runtime-owned search state
  - [x] pane activation and surface targeting — `active_pane_rects`,
    `pane_render_modes`, `pane_viewer_ids` on runtime (landed earlier
    in M1/M3 prep; formalized by M4.1 slice 1c bundle assembly at
    `execute_update_frame`)
  - [x] thumbnail request queue + in-flight tracking (M4.4) —
    `thumbnail_capture_in_flight` on runtime; `ThumbnailChannel` +
    `BackendThumbnailPort` consolidate tx/rx as host-neutral port
- [x] Define host-neutral view-model and effect contracts —
  `FrameViewModel`, `FrameHostInput`, six `Host*Port` traits
- [~] Make egui consume those contracts instead of owning them implicitly
  — dry-run `runtime.tick(input, ports) -> view_model` runs every
  frame; most view-model fields are populated; actual host-side
  consumption of tick output (vs. reading shell state directly) is the
  remaining migration surface
- [~] Keep all durable state transitions testable without an egui frame
  — new tests cover: `FocusRingSpec::alpha_at_with_curve` (4),
  `FocusViewModel` projection incl. bugfix (5), `FocusRingSettings`
  setter / serde (5), `ThumbnailSettings` setter / serde / legacy blobs
  (8), `resize_for_aspect` (3), `encode_thumbnail` PNG/JPEG/WebP round-
  trip (1), `BackendThumbnailPort` dyn dispatch (1),
  `cached_thumbnail_result` dimension-recovery regression (1). All
  pass against graphshell-core; graphshell-lib verification blocked by
  webrender migration.

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
- [x] Replace live graph packet derivation with `graph-canvas` — M2 done 2026-04-21
- [x] Replace live graph input resolution with `graph-canvas` actions/events — M2 done 2026-04-21
- [x] Land the egui graph-canvas host adapter as the live path — M2 done 2026-04-21
- [x] Rekey compositor adapters from `TileId` toward `NodeKey` / `PaneId` — M3 done 2026-04-20
- [x] Content-callback executor trait (M3.5 slice 1) — landed 2026-04-21
- [x] Euclid-typed descriptors + iced stub-painter verification (M3.5 final slice) — landed 2026-04-21
- [~] Split `Gui` into runtime/presenter plus egui adapter responsibilities (M4)
  — substantially landed via transitional bundles (`FocusAuthorityMut`,
  `ToolbarAuthorityMut`, `CommandAuthorityMut`, `GraphSearchAuthorityMut`,
  `ThumbnailChannel` + `BackendThumbnailPort`). Remaining: collapse
  bundle parameters into `&mut GraphshellRuntime` at the phase-args
  level, and move host-side view-model reads to consume `runtime.tick()`
  output rather than reaching into shell state directly.
- [~] Scaffold iced host entry point with one graph surface — partially landed: `shell/desktop/ui/iced_app.rs`, `iced_events.rs`, `iced_graph_canvas.rs`, `iced_host.rs`, `iced_host_ports.rs` (M5 skeleton, all no-op todo stubs), `iced_parity.rs` exist. Full wiring (event translation, focus, texture cache, clipboard, toast, accesskit) remains per iced_host_ports.rs `todo(m5)` markers.
- [ ] Add iced `GraphTree` adapter
- [ ] Add iced `graph-canvas` adapter — note: `iced_graph_canvas.rs` scaffold exists
- [ ] Add host parity replay tests — note: `iced_parity.rs` scaffold exists

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

### 2026-04-20 — M3 done gate met + M3.5 carved

Compositor investigation showed the identity-rekey work the M3
checklist anticipated had already been absorbed into M1 prep.
Registries (`COMPOSITOR_CONTENT_CALLBACKS`,
`COMPOSITOR_NATIVE_TEXTURES`, `COMPOSITED_CONTENT_SIGNATURES`,
`LAST_SENT_NATIVE_OVERLAY_RECTS`), frame inputs
(`active_node_pane_rects_from_graph_tree` — already producing
`(PaneId, NodeKey, egui::Rect)` tuples), and the
`ViewerSurfaceRegistry` content-surface authority all key on
`NodeKey` / `PaneId` today. The compositor does not require
egui-owned identity to schedule composition — the original done gate
— so M3 is marked done.

What was genuinely left after M1 was **host-specific presentation
extraction**, not identity migration. The plan was edited to reflect
this: M3's checklist is now `[x]` marked against the identity items,
and a new M3.5 section was carved for the three presentation slices
(overlay-pass painting, content-callback executor, euclid-typed
overlay descriptors).

**M3.5 overlay-pass painting landed same day** as the pattern-setter
extraction:

- New trait `OverlayAffordancePainter` in
  `shell/desktop/workbench/compositor_adapter.rs` with one method,
  `fn paint(&mut self, overlay: &OverlayStrokePass)`.
- Egui implementation: `EguiOverlayAffordancePainter { ctx: &Context }`
  dispatches to the existing `CompositorAdapter::draw_*` static
  functions (which remain egui's implementation detail).
- New dispatcher
  `CompositorAdapter::execute_overlay_affordance_pass_with_painter(
      painter: &mut dyn OverlayAffordancePainter, pass_tracker,
      overlays)`
  carries the diagnostics + pass-tracker bookkeeping and hands each
  overlay descriptor to the painter.
- Legacy `execute_overlay_affordance_pass(ctx, ...)` kept as a
  thin wrapper that constructs an `EguiOverlayAffordancePainter`
  and delegates — zero call-site churn in `tile_render_pass.rs` or
  tests.
- New test `overlay_affordance_pass_routes_through_painter_trait`
  uses a non-egui `RecordingPainter` to verify every overlay
  descriptor is delivered in order with payload intact. This pins
  the contract the future iced painter will rely on.

Iced bring-up now has a concrete, minimal entry point for overlay
painting: implement `OverlayAffordancePainter` against iced's
drawing APIs and pass it where the egui host currently passes an
`EguiOverlayAffordancePainter`. Everything else — descriptor
generation, diagnostics, pass-tracking — stays the same.

Follow-on (tracked under M3.5 checklist): content-callback executor
trait extraction (~400 LOC, same shape) and euclid-typed overlay
descriptors (scoped with iced bring-up so both impls verify
together). Neither blocks M4; both unblock M5.

**Receipts**:

- `cargo test -p graphshell --lib shell::desktop::workbench::compositor_adapter`
  — 32 pass (was 31 pre-extraction; +1 for the trait-seam test).
- `cargo test -p graphshell --lib` — 2155 pass (was 2154 pre-extraction).
- `cargo test -p graph-canvas --features simulate --lib` — 259 pass
  (unchanged; the portable side didn't move).
- `cargo check -p graphshell --lib` clean.

### 2026-04-22 — M4.1 focus authority + M4.4 thumbnail queues landed

**Verification posture**: the `webrender-wgpu` sibling crate is mid
SPIR-V/naga migration and temporarily uncompilable for the full-web
lane, which blocks `cargo check -p graphshell --lib` and
`cargo test -p graphshell --lib`. All work in this session was
verified against `cargo check -p graphshell-core --lib` (portable
kernel, independent of webrender) and the new tests compile-check in
the same target. Full-lib verification will run when webrender
unblocks.

**M4.1 — focus authority (four sub-slices)**:

- **Slice 1a (view-model cut)**: `FocusRingSpec::alpha_at_with_curve`
  extracts the focus-ring fade math into a host-neutral helper.
  `FocusViewModel` publishes `focus_ring_alpha: f32` so hosts paint
  without re-deriving timing. Shared by `project_view_model()` and
  the render-path alpha compute. 4 unit tests + 3 projection tests.
- **Slice 1b (mutation-path extraction)**: `FocusAuthorityMut<'a>`
  bundle (in `gui_state.rs`) replaces the four per-field
  `&mut Option<NodeKey>` / `&mut Duration` args threaded through
  `TileRenderPassArgs` / `PostRenderPhaseArgs`. Methods
  `hint()` / `set_hint()` / `clear_hint()` / `clear_hint_if_matches()` /
  `latch_ring()` / `ring_alpha()` / `graph_surface_focused()` /
  `reborrow()`. Deletes the standalone `latch_focus_ring_transition`
  free function. Narrow-permissive signatures in
  `ExecuteUpdateFrameArgs` / `GraphSearchAndKeyboardPhaseArgs`
  (`&mut bool` → `bool`, `&mut Duration` → `Duration` where only read).
- **Slice 1c (bundle assembly lifted up)**: `SemanticAndPostRenderPhaseArgs`
  replaces the five individual focus fields with a single
  `focus: FocusAuthorityMut<'a>`. Assembly moved from the
  semantic-post-render destructure up to `execute_update_frame` in
  `gui_update_coordinator.rs`, after phases 1–3 have settled
  `graph_surface_focused`.
- **Slice 1d (configurability + bugfixes + signature tightening)**:
  New `FocusRingSettings { enabled, duration_ms, curve, color_override }`
  on `ChromeUiState` with serde round-trip, setter-side clamping, and
  persistence via `settings.focus_ring_settings`. New
  `FocusRingCurve { Linear, EaseOut, Step }` maps to the shared alpha
  reshape. Color override mutates `presentation.focus_ring` at the
  compositor site; defaults still theme-driven.
  **Bugfix**: `FocusViewModel.focus_ring: Option<FocusRingSpec>` no
  longer lingers `Some` after ring expiry — filtered to `None` when
  `focus_ring_alpha <= 0.0` so hosts gating on `is_some()` don't loop.
  Over-permissive `&mut Duration` / `&mut bool` signatures narrowed.

**M4.4 — thumbnail / update queues**:

- **Field ownership**: `thumbnail_capture_in_flight: HashSet<WebViewId>`
  moved from `EguiHost` to `GraphshellRuntime`. Pure runtime tracking
  per M3.5 §3.5.
- **Customization pass**: `ThumbnailSettings { enabled, width, height,
  filter, format, jpeg_quality, aspect }` on `ChromeUiState`.
  `ThumbnailFilter { Nearest, Triangle, CatmullRom, Gaussian, Lanczos3 }`
  maps onto `image::imageops::FilterType`.
  `ThumbnailFormat { Png, Jpeg, WebP }` — lossless PNG, lossy JPEG
  with quality knob, lossless WebP (image 0.25 built-in encoder).
  `ThumbnailAspect { Fixed, MatchSource, Square }` controls
  preserve-aspect vs. crop-to-fit. All clamped at setter +
  load-time, `#[serde(default)]` for legacy blob compat.
- **tx/rx consolidation**: `ThumbnailChannel` struct consolidates the
  former separate `thumbnail_capture_tx` / `_rx` fields on `EguiHost`.
  Pipeline helpers now take `&dyn BackendThumbnailPort` — a trait
  with `clone_sender()` + `try_recv() -> Option<_>` that
  `ThumbnailChannel` implements. Iced host provides its own channel
  type later; pipeline unchanged.
- **Cached-dimensions bugfix**: `cached_thumbnail_result_for_request`
  previously returned hardcoded default dims (256×192) regardless of
  actual cached bytes, which — combined with downstream
  `set_node_thumbnail` overwriting node-stored dims on any field
  mismatch — actively corrupted node-stored dimensions back to
  defaults on every cache hit when users had configured non-default
  thumbnail sizes. Fix: decode cached bytes once via
  `image::load_from_memory`; fall back to defaults only on decode
  failure.
- **Dual-retain documentation**: the twin `in_flight.retain(…)` calls
  at the top of `request_pending_thumbnail_captures` and
  `load_pending_thumbnail_results` gained comments explaining the
  contract (either entry point safe to call independently).

**M4.1 sibling work (M4.2, M4.3)**:

- M4.2 toolbar/omnibar: `ToolbarAuthorityMut`, `ToolbarEditable`
  split, `OmnibarSearchSession` on runtime, view-model
  `OmnibarViewModel` + `GraphSearchViewModel` projections.
- M4.3 command routing: `CommandAuthorityMut` bundle,
  `CommandPaletteSession` on runtime, `GraphSearchAuthorityMut`
  replacing the five-`&mut` threading. Landed in parallel; see
  current `SemanticAndPostRenderPhaseArgs` shape.

**New cross-subsystem synthesis doc**:

- [`../../technical_architecture/2026-04-22_browser_subsystem_taxonomy_and_mapping.md`](../../technical_architecture/2026-04-22_browser_subsystem_taxonomy_and_mapping.md)
  — taxonomy of canonical browser subsystems (content pipeline,
  networking, process/isolation, storage, navigation/history, chrome,
  input/a11y, devtools, extensions, security, telemetry, distribution,
  sync) mapped to Graphshell's crate/module topology with status tags,
  Graphshell-unique axes (graph truth, workbench, navigator,
  registries, six-track focus, semantic layer, distillery, mods),
  by-design exclusions vs. undecided gaps. Contributor orientation +
  gap-analysis surface.

**Test deltas** (compile-verified against graphshell-core; full-lib
run gated on webrender):

- `FocusRingSpec::alpha_at_with_curve`: +4 (linear / ease-out /
  step / zero-duration / clock-before-start).
- `FocusViewModel` projection: +3 (field reflection / ring
  publication / alpha-zero on stale target / cleared-when-alpha-
  expires bugfix pin / disabled-settings / step-curve).
- `FocusRingSettings`: +2 (setter clamps / serde roundtrip).
- `ThumbnailSettings`: +5 (clamp dimensions & quality / zero-quality
  clamp / format round-trip / aspect round-trip / disabled-settings
  round-trip / legacy-JSON defaults).
- `thumbnail_pipeline`: +5 (`resize_for_aspect` Fixed / MatchSource /
  Square / `encode_thumbnail` all-formats round-trip /
  `BackendThumbnailPort` dyn-dispatch / `cached_thumbnail_result`
  dimension-recovery regression).
- Plus rebased existing tests against `ToolbarEditable` reshape and
  the `FocusAuthorityMut` bundle flow.

### Residue flagged in this session

- **Lossy WebP** — deliberately not implemented. Pure-Rust lossy
  WebP isn't in the ecosystem as of the current `image` 0.25;
  options are FFI-to-libwebp (native dep, build-system cost) or
  vendored C. At thumbnail scale the filesize/quality delta over
  JPEG at matched quality is single-digit percent. Documented in
  the `ThumbnailFormat::WebP` variant rustdoc so future readers know
  the tradeoff and where to grow a fourth encoder arm if it shifts.
- **`ThumbnailCaptureResult.png_bytes` field name** — now holds
  PNG/JPEG/WebP; predates the format knob. Cosmetic; downstream
  decoders use `image::load_from_memory` (magic-byte detection) so
  mixed-format caches coexist cleanly. Rename deferred.
- **`runtime.focus_ring_duration` field** — now vestigial (settings'
  `duration_ms` is the authoritative source in both
  `project_view_model()` and the render path). Production never
  mutates it; only test code writes it. Removing the field plus the
  corresponding slot on `FocusAuthorityMut` is a follow-on cleanup.
- **Phase-args concrete-type residue** — phase-args structs still
  carry `&mut` refs into runtime fields (`focus_authority`,
  `focus_ring_*`, etc.) rather than `&mut GraphshellRuntime`. That
  collapse is the M4 final convergence, pending across sessions.

---

## 11. Summary

The interesting version of this migration is not "rewrite egui in iced."

It is:

- replace framework-owned graph and tile authority with Graphshell-owned cores
- turn the current framework into a host adapter
- add iced only after the seams are real

That path costs more upfront, but it prevents paying the same migration tax
twice.
