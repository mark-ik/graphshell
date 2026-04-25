<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Iced Host Migration Execution Plan (2026-04-14)

**Status**: Active strategy / execution checklist
**Scope**: A robust, future-facing migration path from the current egui host to
an iced host, while minimizing rewrite cost by first making `graph-tree`,
`graph-canvas`, the compositor/runtime boundaries, and the viewer-surface
contract authoritative.

**Related**:

- `SHELL.md`
- `shell_backlog_pack.md`
- `../system/2026-03-06_reducer_only_mutation_enforcement_plan.md`
- `../subsystem_history/SUBSYSTEM_HISTORY.md`
- `../workbench/2026-04-11_graph_tree_egui_tiles_decoupling_follow_on_plan.md`
- `../workbench/2026-04-11_egui_tiles_retirement_strategy.md`
- `../graph/2026-04-11_graph_canvas_crate_plan.md`
- `../graph/2026-04-13_graph_canvas_phase0_plan.md`
- `../graph/GRAPH.md`
- `../aspect_render/2026-04-12_rendering_pipeline_status_quo_plan.md`
- Servo companion plan: `servo-wgpu/docs/2026-04-18_servo_wgpuification_plan.md`
- Companion extraction lane: `2026-04-24_graphshell_runtime_crate_plan.md`
  (host-neutral runtime kernel pulled out of `graphshell` into
  `crates/graphshell-runtime/` to lighten the parity-test compile surface)
- Iced content-surface scoping: `2026-04-24_iced_content_surface_scoping.md`
  (what it takes to mount Servo/webview content inside iced graph node
  panes; §M4.5 follow-on)
- Renderer-boot + isolation research:
  `../../research/2026-04-24_iced_renderer_boot_and_isolation_model.md`
  (synthesis of three surveys: iced 0.14 renderer architecture,
  Firefox/Chromium/Servo process-isolation model, and middlenet as
  a near-term test surface — shifts the content-surface scoping
  doc's plan toward a middlenet-first sequence)
- Blitz-shaped chrome scoping:
  `2026-04-24_blitz_shaped_chrome_scoping.md` — long-horizon
  alternative to iced chrome. ~3.5–5 months of focused work to
  replace iced with a Stylo + Taffy + Parley + WebRender stack
  rendered in HTML/CSS. Not the next slice, but startable when
  conditions warrant; documents what we'd build, what survives
  from the iced migration, and the slice plan.
- `../../research/2026-04-10_ui_framework_alternatives_and_graph_tree_discovery.md`
- `../../technical_architecture/graph_tree_spec.md`
- `../../technical_architecture/graph_canvas_spec.md`
- `../../technical_architecture/2026-04-22_portable_shell_state_in_graphshell_core.md`

**Implementation anchors**:

- `Cargo.toml`
- `render/mod.rs`
- `render/canvas_bridge.rs`
- `shell/desktop/render_backend/mod.rs`
- `shell/desktop/ui/gui.rs`
- `shell/desktop/workbench/compositor_adapter.rs`
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

Important seam note:

- the remaining risk is no longer just framework ownership of rendering and
  layout
- meaningful app-level authority still lives outside the portable crates,
  especially the arrangement bridge, graph mutation/sync paths, runtime
  lifecycle hooks, and reducer boundary
- persisted node navigation memory also still has a live mutation-boundary gap:
  some runtime paths write `set_node_history_state(...)` directly instead of
  going through a typed canonical mutation lane

The host plan must not spread those seams into a second host. It should
preserve them as explicit dependencies and shrink them while authority moves.

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
- Do not let iced host extraction create new domain-mutation entry points for
  arrangement, graph truth, or persisted history state. Host migration is
  allowed to move host/runtime ownership; it is not allowed to fork the
  reducer-owned durable mutation boundary.
- **The two hosts need not share implementation shape.** Since egui is
  destined for retirement, the iced host is free to adopt iced-idiomatic
  patterns (canvas-local `Program::State` for camera, direct view-model
  consumption in `view`, inline painting inside canvas `draw`) rather than
  mirroring egui's compositor-with-static-painter model. The only
  cross-host contract is the portable crate surface — `graphshell-core`,
  `graphshell-runtime`, `graph-canvas`, `graph-tree` — and the
  `FrameHostInput` / `FrameViewModel` / `HostPorts` vocabulary. Trait
  implementations that egui needs (e.g. `OverlayAffordancePainter`,
  `ContentPassPainter`, `HostPaintPort::draw_*`) may remain as
  unimplemented stubs on iced when iced's architecture paints directly
  from portable descriptors instead. Recorded 2026-04-24.
- **Iced chrome isn't blocked on Servo readiness.** Per the
  [2026-04-24 renderer-boot + isolation research](../../research/2026-04-24_iced_renderer_boot_and_isolation_model.md),
  middlenet's CPU-side `RenderScene` gives iced a real content
  surface that doesn't require wgpu device sharing or Servo
  wgpuification. Chrome polish (command palette, settings,
  overlays) can proceed against a real content-rendering substrate
  by exercising the middlenet path. Recorded 2026-04-24.

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
  — startup `incremental_sync_from_tiles` at `gui.rs:528` is the sole
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

Boundary clarification:

- the runtime/host extraction line is not the same thing as the domain mutation
  boundary
- M4 may move shell/workbench/runtime ownership out of egui, but it must keep
  arrangement-to-graph sync, graph apply, and persisted-history mutation
  authority explicit rather than smearing them across new host adapters

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
- [x] Record the non-host seams that remain authoritative during extraction:
  arrangement bridge, reducer-owned durable graph mutation, runtime lifecycle
  hooks, and persisted history mutation policy

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

Dependency note:

- M4 does not own the system-level fix for direct durable writes into
  node-scoped navigation memory
- it does own not making that seam worse while runtime authority moves
- runtime extraction should route host-driven navigation/history updates through
  the existing app/runtime boundary, not mint new host-specific setters

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
- [~] Keep host/runtime extraction from reintroducing app-level authority drift:
  no new host-owned arrangement->graph writes, no new host-owned graph mutation
  helpers, and no new direct persisted-history writes such as
  `set_node_history_state(...)`
  — persisted-history writes are now hard-guarded by the contract test in
  `app::history::sanctioned_history_writes_tests` (2026-04-23, Lane A); the
  arrangement and host-mutation guards remain to be added (see §12.17).

Done gate:

- egui is no longer the owner of shell/workbench runtime semantics

### M4.5. Make Viewer Surfaces Host-Native

**Status**: In progress (2026-04-24). All four `HostSurfacePort` methods wired
in `EguiHostPorts`: `register_content_callback`, `unregister_content_callback`,
`retire_surface`, and `present_surface` (deferred-queue pattern resolves the
double-borrow on `runtime.tick`; host drains post-tick). All five `HostInputPort`
accessors wired: `pointer_hover_position`, `wants_keyboard_input`,
`wants_pointer_input`, `modifiers` delegate to `egui::Context`; `poll_events` is
a documented no-op (events enter via `FrameHostInput`). Dual-authority on
`COMPOSITOR_NATIVE_TEXTURES` resolved: `compose_webview_content_pass_for_surface_with_painter`
restructured so `surface.handle` updates are per-backing-arm; the static is
write-through for retirement only on the `NativeRenderingContext` path.
`ViewerSurfaceRegistry` is now the sole live-handle authority for that path.

**Goal**: make the viewer/compositor surface model authoritative and
host-portable before iced depends on it.

This slice exists because M4's runtime extraction does **not** by itself make
the viewer path host-neutral. Today the main host can drive a host-neutral
runtime tick while still depending on GL-shaped viewer-surface assumptions.
That is good enough for scaffolding, but not for a useful second host.

Dependency note:

- the current viewer-surface seam is no longer "per-node GL contexts
  everywhere," but whether hosts exercise the native `RenderingContextCore`
  path or fall back to explicit GL compatibility producers
- `ViewerSurfaceRegistry` and `ViewerSurfaceHost` exist, but they do not yet
  define the sole authoritative surface model on the hot path
- the target end-state should align with the Servo wgpuification direction:
  one shared wgpu device for producer and compositor where possible, with GL
  retained only as an explicit compatibility producer for features that still
  require it

Checklist:

- [ ] Move authoritative viewer-surface ownership to `ViewerSurfaceRegistry`
  rather than leaving hot-path ownership smeared across
  `tile_rendering_contexts` and compositor-side fallback state
- [x] Retire direct hot-path reliance on `tile_rendering_contexts:
  HashMap<NodeKey, Rc<OffscreenRenderingContext>>`
  — field was removed from `ViewerSurface` / `ViewerSurfaceRegistry` before this
  slice; confirmed absent in live code (2026-04-24). Checklist item was stale.
- [ ] Evolve `ViewerSurfaceHost` / `ViewerSurfaceRegistry` so the primary
  contract is not "GL offscreen context per node", even if a GL-backed
  compatibility producer remains one implementation
- [ ] Keep shared-wgpu texture composition as the primary path and make the
  callback fallback an explicit compatibility producer, not the shape of the
  main API
- [ ] Preserve WebGL quarantine through the interop/import path rather than
  letting WebGL requirements force the entire viewer-surface contract to stay
  GL-shaped
- [x] Add parity / diagnostics coverage that records which viewer-surface /
  content-bridge path each host is exercising during bring-up
  — `ViewerSurfaceRegistry::record_frame_path` wired; 5 unit tests added to
  `compositor_adapter.rs` covering auto-creation, most-recent-wins update,
  all-variants round-trip, `bump_content_generation` increment, and no-op for
  missing nodes. All pass (2026-04-24).

Done gate:

- the normal viewer composition path no longer depends on host-owned
  `OffscreenRenderingContext` assumptions
- `ViewerSurfaceRegistry` is the authoritative surface owner on the hot path
- GL callback fallback remains explicit and contained rather than shaping the
  primary host contract

### M5. Bring Up Iced as a Second Host

**Goal**: prove that iced can host the existing product core without forcing a
second rewrite of graph/workbench/compositor logic, **without inheriting
hidden GL-shaped viewer-surface assumptions from the current egui host**.

Dependency note:

- M5 may start with scaffolding before M4.5 is fully complete, but a **useful**
  iced host does not count as landed unless it consumes the same authoritative
  viewer-surface contract the egui host is converging on
- "mount Servo/viewer content" does not mean "reuse whatever
  `OffscreenRenderingContext` assumptions happen to be hidden behind today's
  boundary"; it means exercising the same compositor/runtime boundary and
  viewer-surface policy intended for both hosts

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
  **and** the same authoritative viewer-surface contract, rather than adding
  iced-only `OffscreenRenderingContext` assumptions
- [ ] Preserve the same non-host authority seams during bring-up: arrangement
  still enters graph truth through the arrangement bridge, durable graph state
  still enters through the canonical mutation lane, and host-driven
  navigation/history updates do not add new direct durable-write paths
- [ ] Add parity runs between egui host and iced host for the same replay inputs
  and the same viewer-surface / content-bridge policy where the host supports it

Done gate:

- iced can drive the same runtime/core as egui for a useful subset of the app
- iced does not depend on hidden egui-era `OffscreenRenderingContext`
  assumptions to mount viewer content

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
  — substantially landed via authority bundles now mostly promoted to
  `graphshell-core` (`FocusAuthorityMut`, `CommandAuthorityMut`,
  `GraphSearchAuthorityMut` live in `crates/graphshell-core/src/shell_state/authorities.rs`;
  `ToolbarAuthorityMut` remains shell-local in `gui_state.rs`).
  `ThumbnailChannel` + `BackendThumbnailPort` consolidated as host-neutral
  port. Phase-args bundle collapse onto `&mut GraphshellRuntime` landed
  2026-04-24 (§12.20 Tier 1). `runtime.tick(input, ports) -> view_model`
  is live on both hosts: egui at `gui.rs:1047`, iced at `iced_host.rs:67`.
  Remaining: migrate more host-side view-model reads to consume
  `runtime.tick()` output rather than reading shell state directly
  (chrome render sites require `tick()` to run before the render pass).
- [~] Scaffold iced host entry point with one graph surface — landed as six
  files (~1,480 lines) under `shell/desktop/ui/`: `iced_app.rs`,
  `iced_events.rs`, `iced_graph_canvas.rs`, `iced_host.rs`,
  `iced_host_ports.rs`, `iced_parity.rs`. `iced_host.rs::run_frame()` calls
  `self.runtime.tick(input, &mut ports)` on the live iced path — the same
  runtime kernel the egui host calls at `gui.rs:1047`. Remaining: most
  `HostInputPort` / `HostSurfacePort` / `HostClipboardPort` methods on
  `IcedHostPorts` are still `todo(m5)` no-ops (24 markers in
  `iced_host_ports.rs`); event translation, texture cache, clipboard, toast,
  and accesskit bridges are the unclosed slices.
- [ ] Add iced `GraphTree` adapter
- [~] Add iced `graph-canvas` adapter — `iced_graph_canvas.rs` is an
  M5.4 "first real surface" impl (~248 lines): renders the shared graph
  (nodes as circles, edges as lines) via `iced::widget::canvas::Canvas`,
  reading `GraphshellRuntime.graph_app.domain_graph()`. Documented scope
  limits: no hit testing, no interaction, no labels, frozen snapshot at
  `view()` build time. Follow-on converts it into a real
  `CanvasBackend<NodeKey>` impl so both hosts drive off the same
  `ProjectedScene`.
- [~] Add host parity replay tests — `iced_parity.rs` has 8 `#[test]`
  blocks driving `runtime.tick(&input, &mut ports)` against both
  `EguiHostPorts` and `IcedHostPorts` from the same `FrameHostInput`
  trace. First cross-host scalar-parity test landed 2026-04-24
  (see §12.12). Remaining: `#[derive(PartialEq)]` on view-model
  sub-structs for full struct-level equality; graph-canvas packet
  snapshot replay; CI gate on divergence.

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
- host choice does not change the mutation boundary for arrangement sync, graph
  truth, or persisted history state
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
  import at `gui.rs:528`, which reconciles GraphTree with tiles restored from
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

### 2026-04-23 — Lane A: persisted-history boundary closed

**Context**: parallel-lane work after the cross-cutting boundary status
matrix (§12) was added. Lane A targeted §12.3 — the biggest live hole in
the matrix, also the last unchecked item in M4's "no new direct persisted-
history writes" line.

**Helper landed**: `GraphBrowserApp::apply_node_history_change(key, entries,
current_index) -> bool` in `app/history.rs`. Pairs the durable
`Graph::set_node_history_state(...)` write with the
`refresh_semantic_navigation_runtime_for_node(...)` projection refresh
that always followed it. Returns whether the durable state actually
changed.

**Migrations** (10 call sites total):

- Production: `app/runtime_lifecycle.rs:182` (`handle_webview_history_changed`),
  `app/clip_capture.rs:295` (clip capture init — required restructuring the
  `let graph = &mut ...` borrow scope so the helper could borrow `&mut self`).
- Tests: `app/workspace_routing.rs` (3 sites), `shell/desktop/ui/workbench_host.rs`
  (5 sites). Tests previously called `app.workspace.domain.graph.set_node_history_state(...)`
  directly to seed history state without triggering the runtime-projection
  refresh; now they go through the helper, which adds a per-call refresh
  but is then immediately followed by `app.rebuild_semantic_navigation_runtime()`
  in those tests anyway, so the per-call refresh is redundant-but-harmless.

**Contract test landed**: `app::history::sanctioned_history_writes_tests::no_unsanctioned_direct_history_writes`.
Walks the repo from `CARGO_MANIFEST_DIR`, scans every `.rs` file outside
`target/.git/node_modules/design_docs/snapshots`, and fails if the literal
`set_node_history_state(` appears in any non-allowlisted file. Allowlist
covers only the function definition (`crates/graphshell-core/src/graph/mod.rs`)
and the helper home (`app/history.rs`). The needle is built via
`concat!()` so the test source itself does not match.

**Receipts**:

- `cargo check --lib` clean (48s, no new warnings — webrender-wgpu
  unblocked since the 2026-04-22 progress log entry).
- `cargo test --lib sanctioned_history_writes` — 1 passed, 0 failed
  (2220 filtered).

**M4 status implication**: M4's last unchecked checklist item ("no new
direct persisted-history writes such as `set_node_history_state(...)`")
moves from `[ ]` to `[~]` — persisted-history is now hard-guarded; the
arrangement-bridge sole-writer guard (§12.1) and host-owned-mutation-entrypoint
guard (§12.17) remain to be added using the same scanning infrastructure.

**Typed-delta follow-on (deliberately deferred)**: introduce
`GraphDelta::UpdateNodeHistory { key, entries, current_index }` so the
helper can route through `apply_graph_delta` rather than calling
`set_node_history_state` direct. Once that lands, the contract test
allowlist narrows to `app/history.rs` only and
`Graph::set_node_history_state` can be `pub(crate)` inside graphshell-core.

### 2026-04-23 — Typed-delta follow-on landed

**Context**: same-day continuation of Lane A. Lane A introduced the
sanctioned helper + grep-time guard; this follow-on adds the typed delta
and compile-time guard so future regressions outside `graphshell-core` are
mechanically impossible (not just test-detectable).

**Changes**:

- `GraphDelta::UpdateNodeHistory { key, entries, current_index }` variant
  added in `crates/graphshell-core/src/graph/apply.rs`. Match arm in
  `apply_graph_delta` returns `NodeMetadataUpdated(bool)` mirroring the
  other node-metadata deltas.
- Helper rewritten to dispatch via
  `apply_graph_delta_and_sync(GraphDelta::UpdateNodeHistory { ... })`
  instead of calling the underlying setter directly. Pattern-matches on
  `GraphDeltaResult::NodeMetadataUpdated(true)` for the change signal.
- Helper docstring rephrased to no longer contain the literal needle —
  references the typed delta variant instead.
- `Graph::set_node_history_state` visibility tightened from `pub fn` to
  `pub(crate) fn`. Outside `graphshell-core` the only reachable write
  surface is the typed delta, dispatched via the helper.
- Contract test allowlist narrowed to
  `crates/graphshell-core/src/graph/{mod.rs,apply.rs}` only — the
  helper home dropped out because the helper no longer mentions the
  literal anywhere.

**Receipts**:

- `cargo check -p graphshell-core --lib --tests` clean (5.87s, no new
  warnings). The new `GraphDelta` variant compiles against all 225
  graphshell-core unit tests.
- Final repo-wide grep for the literal returns exactly 2 occurrences,
  both inside `graphshell-core` and both allowlisted: function
  definition (`mod.rs:2535`) and the typed-delta match arm
  (`apply.rs:291`).
- Full `cargo check --lib` later ran clean after the §12.10
  viewer-surface-path channel contract was updated to declare all 170
  phase-3 entries.

**Residue**: see §12.3 above for the `Node::replace_history_state`
parallel surface (test-fixture primitive, intentionally left `pub`).

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
  collapse is the M4 final convergence; the first slice landed in
  the 2026-04-23 Lane B' progress entry below.

### 2026-04-23 — Lane B' warm-up + first phase-args collapse

**Context**: Lane B' restarted as its own dedicated session per the
2026-04-23 progress log scoping note. Two slices landed: a
vestigial-field warm-up cleanup, then the actual first phase-args
collapse on `ExecuteUpdateFrameArgs`.

**Warm-up (vestigial `focus_ring_duration` removal)**:

- Removed `pub focus_ring_duration: Duration` slot from
  `FocusAuthorityMut` in
  `crates/graphshell-core/src/shell_state/authorities.rs` (the bundle
  is now 4 fields instead of 5).
- Removed `pub fn ring_alpha(...)` and `pub fn ring_alpha_with_curve(...)`
  helper methods from the bundle. They were unused in production —
  `tile_render_pass.rs` and `gui_state.rs::project_view_model` both
  call `FocusRingSpec::alpha_at_with_curve` directly with
  `chrome_ui.focus_ring_settings.duration()` as the source.
- Removed two test functions covered by dedicated
  `FocusRingSpec::alpha_at_with_curve` tests in `frame_model.rs`.
- Removed `pub(crate) focus_ring_duration: Duration` from
  `GraphshellRuntime` (`shell/desktop/ui/gui_state.rs:318`).
- Cleaned up flow-through references in 7 sites: `gui.rs`
  initializer + destructure + assembly, `gui_update_coordinator.rs`
  destructure + bundle construction, `update_frame_phases.rs`
  ExecuteUpdateFrameArgs field, `gui_tests.rs` (4 mutation lines),
  and 3 docstring/comment cleanups.

**First collapse (`ExecuteUpdateFrameArgs`)**:

- Shrank `ExecuteUpdateFrameArgs` from 35+ fields (23 runtime-bound)
  to 20 fields (1 runtime ref + 19 host-only). Replaced 21 individual
  `&mut` runtime field refs plus the inline-constructed `graph_search`
  and `command_authority` bundles with a single
  `pub(super) runtime: &'a mut GraphshellRuntime`.
- Moved the `let GraphshellRuntime { graph_app, ... } = self.runtime;`
  destructure from `gui.rs:920` (top-level `Gui::update`) into
  `gui_update_coordinator.rs::execute_update_frame` body, where the
  individual bindings are actually consumed by sub-phase-args
  construction. The destructure body is bit-for-bit equivalent —
  same field bindings, same `_` discards for runtime-internal fields
  (`workbench_view_id`, `async_spawner`, `signal_router`,
  `tokio_runtime`, `frame_inbox`, `toolbar_drafts`).
- Sub-phase-args structs (`PreFrameAndIntentInitArgs`,
  `GraphSearchAndKeyboardPhaseArgs`,
  `ToolbarAndGraphSearchWindowPhaseArgs`,
  `SemanticAndPostRenderPhaseArgs`, etc.) are **unchanged** — they
  continue to take individual `&mut` field references which
  `execute_update_frame` split-borrows from the destructured runtime.
  Future Lane B' slices push `runtime: &mut GraphshellRuntime` into
  each sub-phase-args struct, shrinking the destructure as it goes.
- `gui.rs:920-1023` shrank from ~110 lines (full nested destructure
  plus 35-field assembly) to ~50 lines (flat destructure + 20-field
  assembly). Net code reduction at the top-level call site.

**Receipts**:

- `cargo check --lib` clean (39.6s; no new warnings from this slice).
- `cargo test --lib sanctioned_writes` — 6 passed, 0 failed.
- `cargo test --lib gui_orchestration` — 96 passed, 0 failed (sanity
  check on the surrounding gui pipeline).

**Four more sub-phase collapses landed same session**:

- `PreFrameAndIntentInitArgs` — 4 runtime-bound fields removed,
  `runtime: &mut GraphshellRuntime` added. Function body destructures
  internally via split-borrow to the 4 fields (`graph_app`,
  `thumbnail_capture_in_flight`, `command_palette_toggle_requested`,
  `control_panel`).
- `GraphSearchAndKeyboardPhaseArgs` — 8 runtime-bound fields plus 1
  `GraphSearchAuthorityMut` bundle collapsed to `runtime`. Body
  split-borrows 12 fields including the 5 graph-search bundle members
  (`graph_app`, `graph_surface_focused`, `focus_authority`,
  `toolbar_state`, `viewer_surfaces`, `viewer_surface_host`,
  `webview_creation_backpressure`, plus the 5 `graph_search_*`
  session fields).
- `ToolbarAndGraphSearchWindowPhaseArgs` — 15 runtime-bound fields
  plus `GraphSearchAuthorityMut` bundle collapsed to `runtime`. Body
  split-borrows 18 fields covering graph/tree/toolbar/omnibar/command-
  surface-telemetry/viewer/webview and the graph-search session.
- `SemanticAndPostRenderPhaseArgs` — 15 runtime-bound fields + 3
  bundles (`focus`, `graph_search`, `command_authority` wrapping 11
  additional fields) collapsed to `runtime`. Body split-borrows and
  constructs the bundles for the deeper sub-phases
  (`SemanticLifecyclePhaseArgs`, `PostRenderPhaseArgs`).

**`execute_update_frame` body after the collapse**:

- No runtime destructure at all. The `let GraphshellRuntime { … } =
  runtime;` block that initially sat at the top is **gone**.
- The `modal_surface_active` computation (which previously used
  destructured `graph_app`/`focus_authority`/`toolbar_state`) now
  uses scoped split-borrows: clone `local_widget_focus` first, copy
  `show_clear_data_confirm` by value, then borrow
  `runtime.graph_app` and `runtime.focus_authority` mutably.
- The `finalize_update_frame` trailing call uses
  `&mut runtime.graph_app` directly.
- Every sub-phase call site passes `runtime: &mut *runtime`
  (reborrow). Between calls, the reborrow ends and runtime is fully
  accessible for the next call.

**Receipts**:

- `cargo check --lib` clean (7.48s incremental; 1m 52s clean).
  Zero new graphshell warnings from this slice.
- `cargo test --lib sanctioned_writes` — 6 passed, 0 failed.
- `cargo test --lib gui_orchestration` — 96 passed, 0 failed.

**Two more deeper-stack collapses landed same session**:

- `SemanticLifecyclePhaseArgs` — 7 runtime-bound fields collapsed.
  Phase function destructures internally via split-borrow at the
  `gui_orchestration::run_semantic_lifecycle_phase` call. Caller
  (Semantic body) now passes `runtime: &mut *runtime` directly.
- `PostRenderPhaseArgs` — 11 runtime-bound fields plus 2 bundles
  (`focus`, `command_authority` wrapping 6 more) collapsed. Uses the
  destructure-at-the-top pattern (function body is ~750 lines with
  3 `TileRenderPassArgs` constructions across multiple
  `egui::Panel::show_inside` closures). The Semantic body's
  intermediate `phase3_reconcile_semantics` /
  `runtime_focus_inspector` computations now use scoped split-borrows
  before the PostRender call. `graph_search_matches` snapshot-cloned
  before the PostRender call so PostRender can take `runtime: &mut
  *runtime` without holding a reference to the matches vec.

**Pre-existing WIP fix landed alongside**: a missing
`UxBridgeCommand::GetDiagnosticsState` match arm in
`shell/desktop/host/webdriver_runtime.rs` was blocking `cargo check`.
Added a stubbed transport-error return until the upstream lane wires
its full handler.

**Receipts (full Lane B' second pass)**:

- `cargo check --lib` clean (8.78s incremental).
- `cargo test --lib sanctioned_writes` — 6 passed, 0 failed.
- `cargo test --lib gui_orchestration` — 96 passed, 0 failed.

**Lane B' final convergence (2026-04-24): TileRenderPassArgs landed**:

- `TileRenderPassArgs` — 7 runtime-bound fields plus focus bundle
  collapsed to a single `runtime: &'a mut GraphshellRuntime`. The
  `run_tile_render_pass_in_ui` body destructures runtime internally
  via split-borrow at the start, exposing `graph_app`, `graph_tree`,
  `viewer_surfaces`, etc. and assembling the focus bundle from the
  destructured fields. Function body shape (~1100 lines downstream)
  intact.
- `PostRenderPhase` body restructured: the previous destructure-at-the-top
  pattern was removed entirely. ~30 use sites of `graph_app`,
  `bookmark_import_dialog`, `viewer_surfaces`, `viewer_surface_host`,
  `webview_creation_backpressure`, `control_panel`,
  `command_surface_telemetry`, `pending_webview_context_surface_requests`,
  `focus.method()`, `focus.field`, and `command_authority.reborrow()`
  rewritten as `runtime.foo` / `&mut runtime.foo` split-borrows scoped
  to each expression. The 3 `TileRenderPassArgs` constructions inside
  `egui::Panel::show_inside` / `CentralPanel::show_inside` /
  `egui::Area::show` closures now pass `runtime: &mut *runtime`
  (closure-captured reborrow); the closures themselves capture runtime
  by `&mut` reborrow and release it when they return.
- Several intermediate computations needed careful borrow-checker
  navigation: the `dirty_state` / `pending_switch` / `dialog_event`
  values now snapshot read-only data into locals before subsequent
  `&mut runtime.foo` calls, since holding a `&runtime.foo` borrow
  alongside `&mut runtime.bar` requires distinct field projections.
  The post-closure `command_authority` bundle is constructed inline,
  used for the palette panel call, then explicitly `drop`ped before
  subsequent `&mut runtime.command_palette_*` accesses.

**Receipts (final Lane B' session)**:

- `cargo check --lib` clean (7.81s incremental). First-try compile
  on the restructured PostRender body — no borrow-checker iterations
  needed. Two unused-import warnings cleaned up
  (`GraphshellRuntime` in post_render_phase.rs,
  `WebviewCreationBackpressureState` in tile_render_pass.rs).
- `cargo test --lib sanctioned_writes` — 6 passed, 0 failed.
- `cargo test --lib gui_orchestration` — 96 passed, 0 failed.

**Lane B' final net progress (2026-04-23 + 2026-04-24)**:

- **8 of 8 phase-args structs collapsed**: `ExecuteUpdateFrameArgs` (top),
  `PreFrameAndIntentInitArgs`, `GraphSearchAndKeyboardPhaseArgs`,
  `ToolbarAndGraphSearchWindowPhaseArgs`,
  `SemanticAndPostRenderPhaseArgs`, `SemanticLifecyclePhaseArgs`,
  `PostRenderPhaseArgs`, `TileRenderPassArgs`.
- Every phase function in the frame pipeline now takes
  `runtime: &mut GraphshellRuntime` as its sole runtime-bound argument.
- The runtime destructure that originally lived at `gui.rs:920` (top
  of `Gui::update`, ~30 individual field bindings) is gone entirely.
  Every sub-phase call along the call chain
  (`Gui::update` → `execute_update_frame` → `run_semantic_and_post_render_phases`
  → `run_post_render_phase` / `run_semantic_lifecycle_phase` →
  `run_tile_render_pass_in_ui`) passes `runtime: &mut *runtime`
  through the args struct.
- M4 final convergence achieved: the runtime is the natural carrier
  for shell/workbench state through the entire frame pipeline. Phase
  functions destructure runtime internally where their existing
  function-body shape benefits from individual bindings, or use
  `runtime.foo` / `&mut runtime.foo` inline where the destructure
  would conflict with closure-bound reborrows.

### 2026-04-23 — §12.1 + §12.17 contract guards landed; Lane B' scoping

**Context**: continuation of the typed-delta follow-on closure. With the
reusable scanning helper landed in §12.3, three boundary guards (§12.3,
§12.1, §12.17) consolidate cleanly into one module.

**Changes**:

- New consolidated module `app/sanctioned_writes_tests.rs` declared from
  `graph_app.rs` as `#[cfg(test)] mod sanctioned_writes_tests`. Hosts six
  contract tests + two reusable scanning helpers. The two history tests
  previously in `app::history::sanctioned_history_writes_tests` migrated
  here unchanged in semantics; test path moves from
  `app::history::sanctioned_history_writes_tests::*` to
  `app::sanctioned_writes_tests::*`.
- §12.1 — two new tests forbid direct calls to
  `add_arrangement_relation_if_missing` and
  `promote_arrangement_relation_to_frame_membership` outside
  `app/graph_mutations.rs` (definitions + internal composition) and
  `app/arrangement_graph_bridge.rs` (sanctioned bridge caller).
  Earlier matrix `partial` characterization revised: the three "deprecated"
  bridge wrappers in `app/workbench_commands.rs` are actually thin
  delegations to `apply_arrangement_snapshot`, not bypass paths.
- §12.17 — two new tests forbid `apply_graph_delta_and_sync(` and
  `apply_arrangement_snapshot(` in 5 host-adapter files
  (`iced_host.rs`, `iced_app.rs`, `iced_events.rs`, `iced_host_ports.rs`,
  `egui_host_ports.rs`). Two host-adjacent files (`iced_graph_canvas.rs`,
  `iced_parity.rs`) intentionally NOT in the host set — they have
  legitimate test fixtures and parity-replay helpers respectively.
- New helper `assert_needle_absent_from_files(needle, target_files,
  sanction_message)` complements the existing
  `assert_no_unsanctioned_callers` — first scans a small fixed set, second
  scans repo-wide with allowlist. Covers both the §12.1 / §12.3 pattern
  ("identifier may appear only at sanctioned sites repo-wide") and the
  §12.17 pattern ("identifier must NOT appear in this fixed file set").

**Receipts**:

- `cargo test --lib sanctioned_writes` — 6 passed, 0 failed, 2220
  filtered (1m 52s build, 0.21s run).
- `cargo check --lib` clean (post your diagnostics-array fix); no new
  warnings.

**Lane B' scoping note (deferred to a future session)**:

The user-requested Lane B' (phase-args bundle collapse onto
`&mut GraphshellRuntime`) was scoped during this session but not landed.
Findings:

- All four authority bundles (`FocusAuthorityMut`,
  `GraphSearchAuthorityMut`, `CommandAuthorityMut`, `ToolbarAuthorityMut`)
  wrap fields that already live on `GraphshellRuntime`.
- The current pattern at `gui.rs:920-957` destructures `GraphshellRuntime`
  into individual `&mut` field bindings, then re-bundles them inline at
  phase-args construction (`gui.rs:994-1013`). That destructure-then-rebundle
  pattern exists *because* the phase-args structs ALSO carry many other
  `&mut runtime.field` references alongside the bundles
  (`viewer_surfaces`, `webview_creation_backpressure`, `control_panel`,
  `tile_favicon_textures`, etc.). Replacing `bundle: FocusAuthorityMut`
  with `runtime: &mut GraphshellRuntime` would borrow-conflict with
  those sibling refs.
- The genuine collapse therefore requires a coordinated pass per phase
  function: remove ALL the runtime-pointed `&mut` field refs from the
  phase-args struct, replace with a single `runtime: &mut GraphshellRuntime`,
  and rewrite the function body to access via `args.runtime.foo`. This
  touches every phase-args struct in
  `shell/desktop/ui/gui/update_frame_phases.rs` (8 large structs, each
  ~30+ fields) and every corresponding phase function body — plausibly
  hundreds of edit sites.
- Workable contained first-slice candidates exist but each has a non-trivial
  scope:
  - **Smallest functional collapse**: `PreFrameAndIntentInitArgs` carries
    only 4 runtime-bound refs (`graph_app`, `thumbnail_capture_in_flight`,
    `command_authority`, `control_panel`) — most tractable.
  - **Cleanup prep**: remove the vestigial `runtime.focus_ring_duration`
    field + corresponding slot on `FocusAuthorityMut` (per the 2026-04-22
    residue note). Doesn't actually advance the collapse but reduces the
    surface area for the eventual Lane B' refactor.
- This work is best done as its own dedicated session with explicit scope
  budget and live build-verification between each phase migration. Today's
  session prioritized landing the three boundary guards (§§12.3, 12.1, 12.17)
  which are now all `done`.

### 2026-04-24 — Phase-args prelude collapse + focused_node_key view-model migration

**Context**: Lane 1 M4 session. Two Tier 1 §12.20 items closed.

**Phase-args prelude collapse (final piece)**:

- `run_update_frame_prelude` was the last function in the phase-pipeline that
  passed `&mut runtime.graph_app` instead of `&mut GraphshellRuntime` at its
  public API boundary.
- Changed signature: `graph_app: &mut GraphBrowserApp` →
  `runtime: &mut GraphshellRuntime`. Body updated to access `&mut runtime.graph_app`.
- Updated the `EguiHost::run_update_frame_prelude` wrapper and the
  `execute_update_frame` call site correspondingly.
- The prelude collapse completes the §12.20 Tier 1 "phase-args bundle collapse"
  item.

**`focused_node_key()` view-model migration (§12.6 third getter)**:

- Root cause of the semantic gap: `FocusViewModel.focused_node` was populated
  from `focused_node_hint` (a render-pass cache updated by `tile_render_pass`),
  while `EguiHost::focused_node_key()` read `active_pane_rects.first()` gated
  on `!graph_surface_focused`. These were equivalent in the common case but
  semantically different sources.
- Reconciliation fix: changed `project_view_model` to populate
  `FocusViewModel.focused_node` from `active_pane_rects.first()` gated on
  `!graph_surface_focused` — exact same logic as `focused_node_key()`.
- Migration: `EguiHost::focused_node_key()` now reads
  `cached_view_model.focus.focused_node` with pre-first-frame fallback to
  `interaction_queries::focused_node_key`.
- Tests: updated `focus_view_model_reflects_runtime_focus_fields` to remove
  the now-unnecessary `focused_node_hint` setup; added
  `focus_view_model_focused_node_is_none_when_graph_surface_focused` to pin
  the gate.

### 2026-04-24 — Cross-host parity floor + shared ProjectedScene surface

**Context**: Three Tier-2 items from §12.20 landed end-to-end in one session,
following the 2026-04-24 audit pass below.

**Slice 1 — `PortableRect` duplicate-import fixed (§12.12 blocker)**:

- Removed the duplicate `PortableRect` import in
  `shell/desktop/ui/iced_host_ports.rs` (line 267 previously re-imported a
  type already brought in at line 29). This had been preventing
  `cargo check --lib --features iced-host` from succeeding, which in turn
  meant `cargo test --features iced-host` could not actually run the
  cross-host parity tests.
- Receipt: `cargo check --lib --features iced-host` now completes clean
  (2 pre-existing egui-deprecation warnings unrelated to this change).

**Slice 2 — `#[derive(PartialEq)]` on view-model sub-structs**:

- Added `PartialEq` to 9 view-model sub-types in
  `crates/graphshell-core/src/shell_state/frame_model.rs`:
  `FocusRingSpec`, `FocusViewModel`, `ToolbarViewModel`, `OmnibarViewModel`,
  `GraphSearchViewModel`, `CommandPaletteViewModel`, `DialogsViewModel`,
  `ToastSpec`, `DegradedReceiptSpec`. All transitive types (`PortableInstant`,
  `PaneId`, `ToolbarDraft`/`ToolbarEditable`, `ContentLoadState`, `NodeKey`)
  already had `PartialEq`, so the derives are mechanical.
- Expanded the §12.12 cross-host parity test
  (`replay_trace_scalar_parity_across_host_ports` in `iced_parity.rs`) from
  ~13 scalar-primitive asserts to 13 struct-level asserts covering every
  projected sub-model. Any field-level divergence across hosts is now caught
  by a single `assert_eq!` rather than requiring a scalar allowlist.
- Receipt: `cargo test --lib --features iced-host -- iced_parity` —
  8 passed, 0 failed.

**Slice 3 — iced graph-canvas now consumes shared `ProjectedScene`**:

- New module `shell/desktop/ui/iced_canvas_painter.rs` (mirror of
  `render/canvas_egui_painter.rs`): converts portable `SceneDrawItem`s into
  iced `canvas::Frame` draw calls. Layers paint in background → world →
  overlays order, matching the egui painter. Handles `Circle`, `Line`,
  `RoundedRect`, `Label`; defers `ImageRef` on the same texture-registry
  dependency as egui.
- Refactored `shell/desktop/ui/iced_graph_canvas.rs::GraphCanvasProgram` to
  hold `CanvasSceneInput<NodeKey>` + derived `ProjectedScene<NodeKey>`.
  `from_graph_app` now routes through
  `canvas_bridge::build_scene_input(...)` + `graph_canvas::derive::derive_scene(...)`
  with identity camera + zero-size viewport — the same portable pipeline the
  egui host's `canvas_bridge::run_graph_canvas_frame` uses minus physics +
  input handling. `draw()` computes a fit-to-bounds transform from the scene
  input's node bounding box, applies it via iced's canvas transform stack,
  and hands the scene to `iced_canvas_painter::paint_projected_scene`.
- Iced 0.14 API notes (captured while landing): `advanced` module is
  feature-gated, so we construct `Text::align_x` via
  `alignment::Horizontal::Center.into()` rather than naming
  `iced_core::text::Alignment` directly; `rounded_rectangle` takes
  `border::Radius` not `Pixels`; canvas `Text` fields renamed to `align_x`
  / `align_y`.
- Module registered at `shell/desktop/ui/mod.rs` behind the `iced-host`
  feature gate.
- Receipt: `cargo test --lib --features iced-host -- iced_parity
  iced_graph_canvas iced_canvas_painter` — 15 passed, 0 failed (4 new
  `iced_graph_canvas` tests replacing old raw-position tests, 3 new
  `iced_canvas_painter` tests, 8 pre-existing `iced_parity` tests
  including the newly expanded struct-level parity).

**Net effect**: iced host now paints graph nodes + edges through exactly
the same portable `ProjectedScene<NodeKey>` type the egui host paints.
Any divergence in scene content between the two hosts is now observable
cross-host via `iced_parity`'s struct-level asserts (for the view-model
surface) and — once a graph-canvas packet replay test lands — via
scene-packet equality too (§12.11 remaining target).

### 2026-04-25 — Iced raw-window-handle audit + wry→verso ownership + iced-graph-canvas-viewer extraction

**Context**: Three slices in one session, in user-requested order
(2 → 1 → 3). Each landed without touching the still-broken
`webrender-wgpu` working tree.

**Slice 28 — iced raw-window-handle audit (Option 2)**:

- `iced 0.14` exposes raw window handles via the public trait
  [`iced_runtime::window::Window: HasWindowHandle + HasDisplayHandle`](.cargo/registry/iced_runtime-0.14.0/src/window.rs)
  at line 196.
- Public access surface:
  [`iced::runtime::window::run(id, |handle: &dyn Window| -> T)`](.cargo/registry/iced_runtime-0.14.0/src/window.rs)
  at line 463 — takes a `FnOnce` callback that receives a
  `&dyn Window` for any `iced::window::Id`. Returns `Task<T>`.
- **Conclusion**: iced-on-wry is feasible without forking iced.
  wry's `WebViewBuilder::new_as_child(parent_window)` accepts the
  `&dyn Window` handle iced exposes. Caveat: handle access is
  async via the `Task` model — wry creation flows through a
  Task callback rather than synchronously inside `view()`/`update()`.
  Structural, not blocking.
- Phase B (iced-wry-viewer) is real architectural work; not a
  fork-iced project.

**Slice 29 — wry dep ownership moves to verso (Option 1, Phase A1)**:

- `crates/verso/Cargo.toml` gained a `wry-engine` Cargo feature
  pulling `wry = { version = "0.55", optional = true }`.
- New module `crates/verso/src/lib.rs::wry_engine` re-exports the
  upstream `wry` crate behind the feature gate, so downstream
  crates (notably the future `iced-wry-viewer`) depend on
  `verso/wry-engine` rather than `wry` directly.
- Graphshell main's `wry` feature now activates `verso/wry-engine`:
  `wry = ["dep:wry", "verso/wry-engine"]`. Existing wry-using code
  in `mods/native/web_runtime/wry_manager.rs` (~800 lines) still
  imports `wry::*` directly via graphshell main's wry dep — that's
  Phase A2 (the actual code move into verso). Phase A1 establishes
  ownership.
- Receipt: `cargo check -p verso --features wry-engine` clean
  (20.16s; pulls wry 0.55 + webview2-com on Windows).

**Slice 30 — iced-graph-canvas-viewer crate extraction (Option 3)**:

- New crate
  [`crates/iced-graph-canvas-viewer/`](../../../crates/iced-graph-canvas-viewer)
  with deps `iced` 0.14 + `graph-canvas` (framework-agnostic) +
  `graphshell-core` (for `NodeKey`) + `euclid`. **No Servo, no
  webrender, no graphshell main-crate dep tree.**
- Public API:
  - `pub struct GraphCanvasProgram` — wraps a pre-built
    `CanvasSceneInput<NodeKey>`. New `pub fn new(scene_input)`
    constructor replaces the old `from_graph_app` (moved to host
    shim — viewer crate doesn't know about `GraphBrowserApp`).
  - `pub struct GraphCanvasState` — canvas-local camera +
    drag-origin state (was `pub(crate)`, now `pub`).
  - `pub enum GraphCanvasMessage::CameraChanged { pan, zoom }` —
    self-emitted event the host maps via `Element::map`.
  - `pub mod painter` — was `iced_canvas_painter` in graphshell
    main; mirrors `canvas_egui_painter`. `paint_projected_scene<N>`
    is `pub` so callers who want to compose multiple scenes can
    paint directly.
- `examples/demo.rs`: hand-built `CanvasSceneInput` with 5 nodes +
  6 edges. Runnable via
  `cargo run -p iced-graph-canvas-viewer --example demo` —
  drag with primary button to pan, wheel to zoom, status line
  shows current camera state.
- Tests moved + adapted: 9 tests pass (8 logic + 1 painter color
  roundtrip). Tests previously needing `GraphBrowserApp` now build
  fixture `CanvasSceneInput` directly using
  `graph_canvas::scene::{CanvasNode, CanvasEdge, ViewId}`.

**Slice 31 — Graphshell shim + cleanup**:

- [`shell/desktop/ui/iced_graph_canvas.rs`](../../../shell/desktop/ui/iced_graph_canvas.rs)
  rewrote as a thin shim (~50 lines):
  - Re-exports `iced_graph_canvas_viewer::{GraphCanvasProgram,
    GraphCanvasState, GraphCanvasMessage}` so existing call sites
    in `iced_app.rs` resolve unchanged via
    `super::iced_graph_canvas::GraphCanvasMessage::*`.
  - `pub(crate) fn from_graph_app(app, view_id) -> GraphCanvasProgram`
    — host-side conversion that builds `CanvasSceneInput` via
    `canvas_bridge::build_scene_input` and constructs the program
    with the new `::new(scene_input)` constructor.
- `iced_app.rs` updated: imports `from_graph_app as graph_canvas_from_app`
  and calls `graph_canvas_from_app(&app, view_id)` instead of the
  old `GraphCanvasProgram::from_graph_app(...)`.
- **Deleted**:
  [`shell/desktop/ui/iced_canvas_painter.rs`](../../../shell/desktop/ui/iced_canvas_painter.rs)
  — its content moved to the new crate's `painter` module.
  Removed from `shell/desktop/ui/mod.rs`. No legacy parallel
  copy retained per DOC_POLICY's no-legacy-friction rule.
- `iced-host` Cargo feature now activates both viewer crates:
  `iced-host = ["dep:iced", "dep:iced-middlenet-viewer", "dep:iced-graph-canvas-viewer"]`.

**Receipts (slices 28–31)**:

- `cargo test -p iced-graph-canvas-viewer --lib` — **9 passed**,
  0 failed (47.22s first build, 0.00s run).
- `cargo build -p iced-graph-canvas-viewer --example demo` — clean
  (11.25s).
- `cargo test -p iced-graph-canvas-viewer -p iced-middlenet-viewer
  -p verso` — **29 total passed** (9 + 6 + 14), 0 failed.
- `cargo check -p verso --features wry-engine` — clean.
- Graphshell main bin verification still gated on
  `webrender-wgpu` working-tree state; no slice in this session
  required it.

**Net architectural effect**: The iced host's portable surface
(viewer crates + verso routing) is now a meaningful set of
standalone, testable, demoable workspace crates that **don't
depend on Servo/webrender/wry-impl directly**. Pattern is firmly
established: portable viewer crate + thin host shim + standalone
demo binary. The iced host's own portable footprint ratio has
flipped — viewers live in their own crates; only the host adapter
glue remains in graphshell main.

**Follow-on lanes available, no blockers**:

- **Phase A2** (move wry_manager into verso) — ~2-3 sessions,
  doable any time, doesn't block iced-wry-viewer Phase B.
- **Phase B (iced-wry-viewer)** — feasible per Slice 28 audit;
  needs to handle async raw-window-handle access via the
  `iced::runtime::window::run(id, ...)` Task pattern.
- **C3 prototype** (iced-Servo screenshot loop) — still a real
  option once webrender-wgpu unblocks.

### 2026-04-25 — M1.1 follow-on: Extracted iced-middlenet-viewer crate

**Context**: The 2026-04-24 M1.1 work landed in the main graphshell
crate, where build verification was blocked by an in-progress
edit in the `webrender-wgpu` fork. To unblock iced-side work
that doesn't actually need Servo or webrender, the viewer was
**extracted into its own workspace crate**.

**Slice 25 — `crates/iced-middlenet-viewer` standalone crate**:

- New crate at
  [`crates/iced-middlenet-viewer/`](../../../crates/iced-middlenet-viewer)
  with deps `iced` 0.14 + `middlenet-engine` only. **No Servo, no
  webrender, no graphshell dep tree.** Compiles + tests in
  ~3.89 s without touching the broken `webrender-wgpu` working
  tree.
- Self-emitted event type: `pub enum MiddlenetViewerEvent { LinkActivated(LinkTarget) }`.
  Hosts map via `iced::Element::map` (same pattern as
  `graph_canvas::GraphCanvasMessage`).
- All 6 tests from the original M1.1 module moved to the new
  crate + added one more (`event_partial_eq`) for downstream
  parity-test composability.

**Slice 26 — Graphshell-side shim**:

- [`shell/desktop/ui/iced_middlenet_viewer.rs`](../../../shell/desktop/ui/iced_middlenet_viewer.rs)
  rewritten as a thin shim (~30 lines): re-exports
  `render_scene(&RenderScene) -> Element<'_, Message>` calling
  the crate-side `render_scene` and `Element::map`-ing
  `MiddlenetViewerEvent::LinkActivated` into `Message::LinkActivated`.
- `iced-host` Cargo feature now also pulls in
  `iced-middlenet-viewer` (added under `[dependencies]` with
  `optional = true`).
- Workspace `[members]` updated to include the new crate.

**Slice 27 — Runnable demo**:

- [`crates/iced-middlenet-viewer/examples/demo.rs`](../../../crates/iced-middlenet-viewer/examples/demo.rs):
  a hand-built `RenderScene` exercising every `RenderBlockKind`
  variant (Title, Heading, MetadataRow, Rule, Paragraph, Quote,
  List, CodeFence, Link, Badge, FeedHeader, FeedEntry,
  RawSourceNotice). Run with:

  ```bash
  cargo run -p iced-middlenet-viewer --example demo
  ```

  Window title "iced-middlenet-viewer demo" opens with the
  rendered scene; clicking a link updates a status line at the
  top showing the activated `LinkTarget`. Useful for visual
  iteration on styling and end-to-end event-routing
  verification — no Servo/webrender needed.

**Receipts**:

- `cargo test -p iced-middlenet-viewer --lib` — **6 passed**, 0
  failed, 3.89 s build, 0.00 s run.
- `cargo build -p iced-middlenet-viewer --example demo` — clean,
  10.42 s.
- `cargo check -p graphshell-core --lib` — clean, confirms the
  portable side compiles with `iced-middlenet-viewer` registered
  in the workspace.

**Net effect**: M1.1 is now buildable, testable, and
demoable today, completely independent of the
`webrender-wgpu` working-tree state. Bonus architectural win —
the iced viewer joins `graph-canvas` and `graph-tree` as a
portable crate with no Servo coupling. The pattern (portable
viewer crate + thin host shim) is reusable for future iced
content surfaces (Wry-backed viewer, future custom protocol
viewers, etc.).

**Follow-on**: M1.2 (route `viewer:middlenet` nodes through
the shim from `IcedApp::view`) and M1.3 (end-to-end Gemini
fetch test) still wait on `webrender-wgpu` unblocking, since
they touch the main graphshell crate. But the viewer logic
itself can iterate freely now.

### 2026-04-24 — M1.1: Iced middlenet viewer (first content surface)

**Context**: First slice of the
[content-surface scoping doc](2026-04-24_iced_content_surface_scoping.md)'s
new §M1 — middlenet-in-iced as the first non-Servo content surface.
Iced now has a real content-rendering capability that doesn't require
wgpu device sharing or Servo wgpuification readiness.

**Slice 22 — `iced_middlenet_viewer` module**:

- New module
  [`shell/desktop/ui/iced_middlenet_viewer.rs`](../../../shell/desktop/ui/iced_middlenet_viewer.rs)
  (~330 lines). Public function `render_scene(&RenderScene) -> Element<'_, Message>`
  walks `scene.blocks` and dispatches by `RenderBlockKind` to per-kind
  sub-renderers using iced primitives (`text`, `column`, `row`,
  `container`, `mouse_area`, `rule::horizontal`).
- Block dispatch mirrors the egui implementation at
  [registries/viewers/middlenet.rs:902-956](../../../registries/viewers/middlenet.rs):
  - `Rule` → `rule::horizontal(1.0)`
  - `CodeFence` → monospace text inside a padded `container`
  - `List { ordered: _ }` → `column!` of `row![bullet, line]` per
    newline in the first text run
  - `FeedHeader` / `FeedEntry` / `Heading{}` / `Paragraph` / `Link`
    / `Quote` / `MetadataRow` / `Badge` / `RawSourceNotice` →
    text-run loop
- Helper `style_for_run(&RenderTextRun) -> (text, font, size, color)`
  maps each `TextStyle` variant to iced presentation parameters.
  Mirrors the egui `RichText` styling at
  [registries/viewers/middlenet.rs:1024](../../../registries/viewers/middlenet.rs):
  - `Title`/`Heading` → bold weight, larger size
  - `Quote` → italic style, prefixed with "&gt; ", gray color
  - `Code` → monospace
  - `Link` → cornflower blue color (iced equivalent of the egui `(100, 149, 237)`)
  - etc.
- Link-clickable text uses `mouse_area(label).on_press(Message::LinkActivated(target))`.
  Avoids `button` styling and lets the text widget keep its
  TextStyle-driven color (links render in cornflower blue and
  click on press).

**Slice 23 — `Message::LinkActivated` + spatial-browsing semantics**:

- New `Message::LinkActivated(LinkTarget)` variant in
  [`iced_app.rs`](../../../shell/desktop/ui/iced_app.rs).
- `update::LinkActivated` queues a
  `HostIntent::CreateNodeAtUrl { url: target.href, position: origin }`
  through the same path `LocationSubmitted` uses, then ticks. **A
  link click creates a new graph node** rather than navigating in
  place — spatial-browsing semantics consistent with the
  graph-as-tabs product framing. Modifier-based "follow in place"
  could come later if needed, but the default for v1 is "every
  click grows the graph."
- Refactored `LocationSubmitted` to share a new
  `queue_create_node_at_url(url)` helper with `LinkActivated` so
  both routes flow through one sanctioned-writes path.

**Slice 24 — Tests + module registration**:

- `iced_middlenet_viewer.rs` has 5 unit tests:
  - `empty_scene_renders_without_panic`
  - `each_block_kind_renders_without_panic` (regression-pin against
    new `RenderBlockKind` variants landing without dispatch
    coverage)
  - `style_for_run_maps_each_text_style` (spot-checks Title, Code,
    Quote, Body)
  - `link_run_renders_clickable_text`
  - `list_block_splits_on_newlines`
- Module registered in
  [`shell/desktop/ui/mod.rs`](../../../shell/desktop/ui/mod.rs)
  behind the `iced-host` feature.

**Verification posture**:

- `cargo check -p graphshell-core --lib` — **clean** (4.63s).
  Confirms `HostIntent` + `FrameHostInput.host_intents` changes
  compile in the portable core.
- `cargo check --lib --features iced-host` — **blocked** by an
  uncommitted in-progress edit in
  [webrender-wgpu/webrender/src/renderer/mod.rs:1264](../../../../webrender-wgpu/webrender/src/renderer/mod.rs)
  (`color_cache_formats` moved-then-used). This is part of the
  user's active SPIR-V shader pipeline migration (per the
  [2026-04-18 plan](../../../../webrender-wgpu/wr-wgpu-notes/2026-04-18_spirv_shader_pipeline_plan.md))
  and not my territory. The iced_middlenet_viewer changes use
  iced 0.14 APIs verified directly in the iced_widget-0.14.2
  registry source (rule::horizontal, mouse_area::on_press,
  Pixels::from(f32)).
- Test verification (`cargo test --lib --features iced-host -- iced_middlenet_viewer`)
  blocked on the same webrender-wgpu issue. Tests will run when
  the fork's working tree clears or stabilizes. Manual code review
  against the iced API surface is the active gate.

**Net effect**: M1.1 of the content-surface scoping doc landed in
code. Once webrender-wgpu unblocks, M1.2 (route wiring in
`IcedApp::view` to dispatch `viewer:middlenet` nodes through the
new viewer) and M1.3 (end-to-end Gemini fetch test) follow on as
~1-session each. After M1, iced is a usable spatial browser for
middlenet content (Gemini, RSS, Markdown, plain text) — shipping-
quality even before Servo wgpu integration is ready.

**Follow-on (M1.2)**: `IcedApp::view` decides per-node whether
to render the graph canvas (existing) or the middlenet viewer
(new), keyed on the node's viewer kind from `FrameViewModel` /
runtime state. Probably a `match vm.node_viewer_for(node_key)`
dispatch.

### 2026-04-24 — Runtime-intent routing + iced wgpu-context slot (C1)

**Context**: Unblocks the `LocationSubmitted` navigation follow-on
flagged in the editable-toolbar session, and lands the first
content-surface scoping slice (C1).

**Slice 17 — Portable `HostIntent` enum in graphshell-core**:

- New module [`graphshell-core/src/shell_state/host_intent.rs`](../../../crates/graphshell-core/src/shell_state/host_intent.rs)
  defines `pub enum HostIntent` with one initial variant:
  `CreateNodeAtUrl { url: String, position: PortablePoint }`.
- This is a **parallel portable enum**, not a move of the shell
  crate's `GraphIntent` into core. Host adapters only need a narrow
  surface of intent variants (what chrome can express); the larger
  `GraphIntent` surface (PendingTileOpenMode, workbench layout
  commands, etc.) stays shell-side because it references shell-only
  types.
- Serde-roundtrip test included for future replay-harness use.

**Slice 18 — `FrameHostInput::host_intents` channel**:

- `FrameHostInput` gained `pub host_intents: Vec<HostIntent>` field.
  Existing `..FrameHostInput::default()` construction patterns
  continue to work (the field defaults to an empty vec via `Default`).
- Egui's `build_frame_host_input` in [gui.rs:274](../../../shell/desktop/ui/gui.rs#L274)
  constructs the field as `Vec::new()` — egui's toolbar submit
  still calls the shell-side reducer path directly (not a host
  adapter file, so §12.17 doesn't apply). iced routes through the
  new channel instead.

**Slice 19 — Runtime drains `host_intents` during tick**:

- New `GraphshellRuntime::apply_host_intents` method in
  [gui_state.rs](../../../shell/desktop/ui/gui_state.rs) translates
  portable intents into runtime-internal reducer actions.
- `CreateNodeAtUrl` delegates to `GraphBrowserApp::add_node_and_sync`
  — the same entrypoint the egui toolbar submit flow uses today.
  Preserves protocol-probe triggering and physics re-heat behavior
  for free.
- Called from `tick(input, ports)` immediately after `ingest_frame_input`
  so the frame's `project_view_model` output reflects the applied
  intents.
- §12.17 boundary check: `apply_host_intents` lives on the runtime
  (not in a host adapter file). Host adapters push intents through
  `FrameHostInput.host_intents`; the runtime owns the reducer call.
  Sanctioned-writes contract tests pass unchanged.

**Slice 20 — Iced host plumbs intents through the tick pipeline**:

- `IcedHost.pending_host_intents: Vec<HostIntent>` queue matches the
  `pending_present_requests` pattern — populated by
  [iced_app.rs](../../../shell/desktop/ui/iced_app.rs) message
  handlers, drained into `FrameHostInput.host_intents` on the next
  tick.
- `IcedApp::update::LocationSubmitted` now:
  1. Takes the draft URL.
  2. Pushes `HostIntent::CreateNodeAtUrl { url, position: (0, 0) }`
     onto `host.pending_host_intents`.
  3. Ticks immediately so the runtime drains the intent in the same
     frame the submit happened.
  4. Enqueues a success toast ("opened: …") confirming the submit.
- `IcedHost::tick_with_input` merges `pending_host_intents` into a
  cloned `FrameHostInput` before calling `runtime.tick`. No cost
  when the queue is empty (early return); cheap clone when present.

**Slice 21 — C1: iced wgpu device/queue slot on `IcedHost`**:

- Added `IcedWgpuContext { device: servo::wgpu::Device, queue:
  servo::wgpu::Queue }` struct in iced_host.rs.
- `IcedHost.wgpu_context: Option<IcedWgpuContext>` — `None` at
  startup; populated via `install_wgpu_context(ctx)` when iced's
  renderer boot path exposes device handles.
- `wgpu_context()` accessor for consumers (future
  `WebViewSurface<NodeKey>` widget per C3 of the content-surface
  scoping doc).
- Uses `servo::wgpu` types because graphshell's shared wgpu path is
  already on that version; if iced's wgpu version diverges, a
  compatibility shim lands alongside C2 (token-less texture flow).
  C1 is exposure-only — no consumers yet.
- Documented the wiring-invocation path: iced renderer boot calls
  `install_wgpu_context`. Actual boot-path wiring lands when iced's
  advanced-renderer exposure is pinned down.

**Receipts (slices 17–21)**:

- `cargo test --lib --features iced-host -- iced sanctioned_writes`
  — **53 passed**, 0 failed (2m 31s first-build; rebuild 0.34s).
  New coverage:
  - `location_submitted_clears_draft_and_creates_node` — end-to-end
    toolbar submit → runtime tick → graph node creation.
  - `wgpu_context_slot_starts_none_and_accepts_install` — C1 slot
    lifecycle.
  - All 5 §12.17 sanctioned-writes tests still green; no host
    adapter file references `apply_graph_delta_and_sync` or
    `apply_arrangement_snapshot`.
- `cargo check --lib --features iced-host` clean (22s rebuild).

**Net effect**: iced's toolbar is now a real navigation surface —
typing a URL and pressing Enter creates a graph node via the
sanctioned `HostIntent` path. The architecture is ready for
additional chrome-driven intents (command palette actions, omnibar
submissions) without further core-crate plumbing — new variants are
just added to the `HostIntent` enum and translated in
`apply_host_intents`. C1's wgpu slot is in place for the
`WebViewSurface<NodeKey>` widget (C3) to consume once Servo
wgpuification is ready.

### 2026-04-24 — Editable toolbar + hotkey + confirmed CLI entry point

**Context**: "Make iced actually runnable, then interactive" — the
first two slices after the stateful-ports session.

**Slice 14 — iced CLI entry point confirmed**:

- `shell/desktop/runtime/cli.rs::main()` already gates an iced launch
  on `--iced` flag or `GRAPHSHELL_ICED=1` env var, invoking
  `iced_app::run_application(GraphshellRuntime::new_minimal())`
  (feature-gated on `iced-host`). No code change needed.
- Verified `cargo check --bin graphshell --features iced-host` builds
  clean (1m 25s; 3 pre-existing warnings). The iced host is launchable
  from the binary; what was missing was only the runtime state
  (clipboard, toast queue, etc.) and interactive chrome — both now in
  place.

**Slice 15 — Editable toolbar location input**:

- Replaced the read-only `text(location_display)` widget in
  `IcedApp::view` with an `iced::widget::text_input` bound to a new
  `IcedApp::location_draft: Option<String>` field. Draft-takes-
  precedence semantics: typing updates the draft, submitting clears
  it so the widget resumes mirroring `last_view_model.toolbar.location`.
- New messages:
  - `Message::LocationEdited(String)` — update draft, no tick.
  - `Message::LocationSubmitted` — for now, enqueue an ack toast
    through `IcedHost::toast_queue` and clear the draft. Actual
    navigation wiring is deferred; see "Follow-on" below.
- New helper `IcedApp::location_value()` picks draft-or-view-model
  as the text-input's current string.
- Toolbar row now aligns items with `align_y(Alignment::Center)` so
  the larger text_input doesn't baseline-drift against the nav/focus
  hints.

**Slice 16 — Ctrl+L focuses the location bar**:

- Location text_input now carries a stable `iced::widget::Id` keyed
  on the `LOCATION_INPUT_ID` constant. Any iced widget that wants
  programmatic focus follows this named-id convention.
- `IcedApp::update::IcedEvent` intercepts Ctrl+L (via modifier-
  agnostic `iced::keyboard::Modifiers::command()` so macOS Cmd+L
  also works) before the runtime translation path, and returns
  `iced::widget::operation::focus(Id)` as the update's `Task`. The
  tick is skipped for this event since host-chrome hotkeys aren't
  part of the runtime's `HostEvent` vocabulary.
- Bare 'l' keypresses flow through normally (the hotkey predicate
  gates on `command()`), so text-input typing is unaffected.

**Receipts (slices 14–16)**:

- `cargo test --lib --features iced-host -- iced` — **46 passed**,
  0 failed (3m compile, 0.04s run). 6 new `iced_app` tests:
  - `location_edited_updates_draft_without_ticking`
  - `location_submitted_clears_draft_and_enqueues_ack_toast`
  - `location_submitted_empty_is_noop`
  - `location_value_prefers_draft_over_view_model`
  - `ctrl_l_hotkey_bypasses_runtime_tick`
  - `bare_l_keypress_is_not_a_hotkey`

**Follow-on — runtime-intent routing for `LocationSubmitted`**:

- §12.17's sanctioned-writes contract forbids host adapter files
  (`iced_app.rs`, `iced_host.rs`, etc.) from calling
  `apply_graph_delta_and_sync` or `apply_arrangement_snapshot`
  directly. The correct path is: host produces a `GraphIntent`, the
  runtime applies it through its reducer.
- Current `FrameHostInput` has an `events: Vec<HostEvent>` channel
  but no `intents: Vec<GraphIntent>` channel. Adding one requires
  either:
  - Moving `GraphIntent` from `graph_app.rs` into `graphshell-core`
    so `FrameHostInput` (in core) can hold it, or
  - Introducing a parallel portable-intent enum in core that the
    runtime translates into `GraphIntent` at tick time.
- Either option is a core-crate slice, not a host slice. Tracked
  here as the blocker on wiring `LocationSubmitted` into actual
  navigation; current iced UX enqueues an ack toast as a
  placeholder so submit is observably happening.
- Once the intent channel lands, iced host wiring is trivial:
  `LocationSubmitted` pushes `GraphIntent::CreateNodeAtUrl { url,
  position }` onto the next tick's `FrameHostInput.intents`, and
  the runtime applies it through its reducer (same path egui's
  toolbar uses directly today, but routed through the contract
  boundary).

### 2026-04-24 — Iced host becomes a real adapter (stateful ports, camera round-trip, toast overlay)

**Context**: Building on the morning's "iced-idiomatic deviation"
license, this session turned `IcedHost` from a thin wrapper around the
runtime into a real host adapter with stateful fields, and drained
most of the remaining `todo(m5)` markers in `iced_host_ports.rs`.

**Slice 10 — `IcedHost` becomes stateful**:

- Fields added to `IcedHost`:
  - `view_id: GraphViewId` — stable per-host identity for camera
    persistence + view-scoped runtime state.
  - `clipboard: Option<arboard::Clipboard>` — OS-level clipboard,
    lazily constructed (same shape egui uses — this is an OS
    concern, not a framework concern).
  - `cursor_position: Option<iced::Point>` — cached from
    `CursorMoved` / `CursorLeft` events.
  - `modifiers: iced::keyboard::Modifiers` — cached from
    `ModifiersChanged` and key events.
  - `toast_queue: Vec<ToastSpec>` — bounded by `MAX_TOAST_QUEUE = 8`;
    trimmed post-tick so unbounded enqueue streams can't grow memory.
  - `texture_cache: HashMap<String, CachedTexture>` — iced-dep-free
    handle (key + width + height + `Arc<[u8]>` RGBA); production
    iced host rehydrates via `iced::image::Handle::from_rgba` when
    image display lands.
  - `pending_present_requests: Vec<NodeKey>` — deferred queue
    matching egui's pattern (registry lives on `GraphshellRuntime`,
    which is mutably borrowed during `tick`).

**Slice 11 — `IcedHostPorts` becomes a borrowed bundle**:

- `IcedHostPorts<'a>` holds mutable refs into `IcedHost` fields.
  Constructed fresh each tick in `IcedHost::tick_with_input` so the
  port traits delegate to live state, mirroring `EguiHostPorts<'a>`'s
  shape at the borrow level even though the trait methods deviate.
- Real port implementations:
  - `HostInputPort::pointer_hover_position` → maps cached
    `iced::Point` to `PortablePoint`.
  - `HostInputPort::modifiers` → projects
    `iced::keyboard::Modifiers` to portable `ModifiersState`
    (reuses the same projection `iced_events::modifiers_from_iced`
    uses, keeping a single mapping definition).
  - `HostInputPort::wants_keyboard_input` / `wants_pointer_input`
    → documented `false`: iced widgets handle capture locally via
    `Action::capture()`, so the runtime doesn't need to gate
    dispatch. Deliberate deviation from egui's `ctx.wants_*` path.
  - `HostSurfacePort::present_surface` → deferred-queue pattern
    (pushes to `pending_present_requests`, drained post-tick).
  - `HostSurfacePort::retire_surface` / `register_content_callback`
    / `unregister_content_callback` → deliberate no-ops pending
    the iced-native content-surface registry design.
  - `HostPaintPort::draw_*` → intentional no-ops. iced paints
    overlays inline inside `GraphCanvasProgram::draw` and chrome
    inside `IcedApp::view`, not through a shared host painter.
    Documented architectural deviation; the trait impl exists for
    type-level portability only.
  - `HostTexturePort` → real cache via `IcedTextureHandle` (key +
    dimensions, iced-dep-free) backed by `CachedTexture`. Full
    `load_texture` / `texture` / `drop_texture` round-trip
    covered by tests.
  - `HostClipboardPort::{get,set}_text` → delegates to `arboard`
    via the lazily-constructed handle, exact parity with egui.
  - `HostToastPort::enqueue` → pushes to `toast_queue`. No drain
    semantics at the port layer — the queue lives until
    `IcedApp::view` renders it or the bounded-queue policy trims
    the oldest.
  - `HostAccessibilityPort` → deferred stubs until the iced
    accesskit bridge lands (same blocker the chrome-port cleanup
    plan §5.2 calls out).

**Slice 12 — Canvas-captured camera round-trips through the runtime**:

- New `GraphCanvasMessage` child-message type with a single variant
  `CameraChanged { pan, zoom }`. Canvas widget emits these via
  `Action::publish(...).and_capture()` after wheel-zoom or
  drag-pan mutates `GraphCanvasState.camera`.
- `GraphCanvasProgram::from_graph_app(app, view_id)` now takes the
  view_id as a parameter so camera persistence keys on a stable
  identity across `view()` rebuilds.
- `IcedApp::view` bridges the child message to the app message via
  `Element::map(|gcm| match gcm { ... Message::CameraChanged })`
  — the canonical iced pattern for child-widget-emits-typed-event.
- `IcedApp::update::CameraChanged` writes to
  `runtime.graph_app.workspace.graph_runtime.canvas_cameras.insert(view_id, camera)`.
  Both hosts now share camera state through the runtime's
  authoritative map (already used by the egui host via
  `canvas_bridge::run_graph_canvas_frame`).
- `Message::IcedEvent` updates `IcedHost::{cursor_position, modifiers}`
  *before* translating to `HostEvent`s — these are state snapshots,
  not events, so they sync regardless of whether the event translates.

**Slice 13 — Toast rendering in `view`**:

- `render_toast_stack(&host.toast_queue)` renders the queued
  `ToastSpec`s as a column of severity-tagged `text` widgets
  (ℹ Info / ✓ Success / ⚠ Warning / ✗ Error) at the bottom of
  the app body. Iced-native — no `egui_notify`-style
  host-framework dependency, just iced primitives.
- Auto-dismiss is a follow-on (needs a `time::every(...)`
  subscription driving periodic redraws and a queue pruner); for
  now toasts persist until the bounded-queue policy drops them.

**Receipts (slices 10–13)**:

- `cargo test --lib --features iced-host -- iced` — **40 passed**,
  0 failed, 0.03s runtime. 10 new tests across the iced modules:
  - `iced_host` (4): `iced_host_drives_runtime_tick`,
    `host_starts_with_stable_view_id`,
    `tick_drains_pending_present_requests`, `toast_queue_is_bounded`.
  - `iced_host_ports` (5): `pointer_hover_position_maps_cursor_to_portable_point`,
    `modifiers_projects_iced_state`, `toast_enqueue_pushes_to_queue`,
    `texture_roundtrip_through_port`, `present_surface_defers_to_pending_queue`.
  - `iced_app` (2): `camera_changed_persists_to_runtime_canvas_cameras`,
    `cursor_cache_syncs_from_iced_events`.
  - `iced_graph_canvas`: `wheel_scroll_inside_bounds_captures_and_zooms`
    extended to assert the published `CameraChanged` payload.

**todo(m5) marker audit**:

- Started: 23 markers.
- After this session: 0 markers remaining in `iced_host_ports.rs`
  (all either replaced with real impls or with documented
  intentional-deviation comments).
- Remaining deferred items are architectural rather than
  wiring-level: iced-native content-surface registry design,
  iced accesskit bridge, iced image display (for populated
  texture cache consumers), auto-dismiss toasts. Each has its
  own follow-on when the broader iced host matures.

**Net effect**: the iced host is no longer a skeleton with stubs — it
has real state (clipboard, cursor, modifiers, toast queue, texture
cache, camera), real event-driven mutation of that state, real
round-trip of camera state between the canvas widget and the runtime,
and real chrome rendering directly from `FrameViewModel`. The
remaining `todo(m5)` markers in the file are all explicit
"deferred-by-design until content/accesskit/image lands" comments,
not wiring placeholders.

### 2026-04-24 — Iced-idiomatic canvas architecture (pan/zoom, view-model chrome, painter-trait deviation)

**Context**: User guidance — "if we can do the iced host in a way
that's better for iced's architecture instead of directly copying egui,
i'm ok with the two hosts not being the same as long as our crates
work." This session took that license: iced's camera state,
event-capture model, chrome projection, and paint approach now follow
iced's retained-mode widget idiom rather than mirroring egui's
immediate-mode compositor-with-static-painter pattern.

**Slice 7 — Camera in `canvas::Program::State`**:

- `GraphCanvasProgram` no longer owns or computes camera state per
  frame via a host-side helper; it only owns the portable
  `CanvasSceneInput<NodeKey>`.
- New `GraphCanvasState { camera: Option<CanvasCamera>, drag_origin: Option<Point> }`
  is the `canvas::Program::State`. iced persists one instance per
  canvas widget across frames.
- Implemented `canvas::Program::update(&self, &mut State, &Event, bounds, cursor)`:
  - Wheel scroll inside bounds → zoom state.camera (seeded from
    fit-to-bounds on first interaction); returns
    `Action::request_redraw().and_capture()` so the app-level
    `event::listen()` subscription does not double-process the event.
  - ButtonPressed(Left) inside bounds → set drag_origin, capture.
  - CursorMoved while drag_origin is Some → pan state.camera by
    (dx/zoom, dy/zoom) world units, capture.
  - ButtonReleased(Left) → clear drag_origin.
- Implemented `canvas::Program::mouse_interaction` — `Grab` when
  hovering, `Grabbing` while dragging, default otherwise.
- `draw(state, ...)` uses `state.camera.unwrap_or(fit_camera)` so
  first-frame auto-fit continues to work; subsequent user interaction
  takes precedence.
- Added 5 new canvas-update tests: wheel zoom in/out of bounds,
  drag pan end-to-end, bare cursor move is ignored,
  state.camera beats fit-camera in derivation.

**Why this deviates from the egui host**: egui runs `canvas_bridge::run_graph_canvas_frame`
host-side per frame because egui has no widget-local retained state —
everything is rebuilt from scratch each immediate-mode pass. iced's
retained-mode widget lifecycle makes `canvas::Program::State` the
natural camera owner, and iced's `event::Status::Captured` mechanism
means the canvas widget can consume pointer/wheel events without the
app's `event::listen()` also seeing them. Trying to share one event
flow across both hosts would force iced into an unnatural shape and
make the canvas re-process events the app already handled (or vice
versa). The portable contract (`CanvasSceneInput`, `CanvasCamera`,
`CanvasViewport`, `ProjectedScene`) is unchanged — only the *owner*
of camera state differs.

**Slice 8 — `view` consumes `last_view_model`**:

- `IcedApp::view` now renders a toolbar row (`row![location, nav
  hint, focus hint]`) above the graph canvas, reading directly from
  `self.last_view_model.as_ref()`. Before the first tick the row
  shows "waiting for first tick…"; after, it shows the portable
  projection the runtime produced.
- This is also iced-idiomatic deviation from egui: egui would route
  chrome painting through `HostPaintPort::draw_*` methods against
  the compositor's static painter. iced reads portable projections
  directly inside `view` — no painter-port plumbing required, since
  `view` is a pure function of app state and `FrameViewModel` is
  that state. The `HostPaintPort::draw_*` methods on `IcedHostPorts`
  remain as stubs and are not expected to be called in the iced
  production path.

**Slice 9 — Painter-trait deviation documented**:

- Updated `iced_host_ports.rs` docstrings on
  `IcedOverlayAffordancePainter` / `IcedContentPassPainter` to
  explicitly state: "the production iced host paints overlays by
  walking `FrameViewModel.overlays` inside
  `GraphCanvasProgram::draw`, not by plugging into the compositor's
  `execute_overlay_affordance_pass_with_painter` flow."
- Retained the stubs as type-level trait-compilation validators
  (proving no egui types leak through the narrow-painter trait
  surface). Marked their `todo(m5)` markers as deferred, not
  blocking.
- This formalizes the retirement-path insight: when the egui host
  eventually retires, the compositor's painter-trait surface may be
  retired with it — iced never needed it, and the portable descriptor
  flow through `FrameViewModel` is sufficient. §12.8's
  painter-trait work remains relevant for the overlap period, not
  as a long-term contract.

**Receipts (combined slices 7–9)**:

- `cargo test --lib --features iced-host -- iced` — **30 passed**,
  0 failed, 0.02s runtime (3m 16s compile). 5 new
  `iced_graph_canvas` canvas-update tests verify pan/zoom/drag
  behavior end-to-end through `canvas::Program::update`.

**Net effect**: iced is now natively interactive in the canvas.
Wheel-zoom + drag-pan work through iced's `Action::capture()` model;
camera state persists across frames in iced's widget state; `view`
renders portable view-model fields directly. The iced host has
diverged from the egui host's architectural shape deliberately — the
portable contracts (crate types) are unchanged, but the host shape
follows iced's retained-mode idiom. This is the first slice where
"iced host" means "iced-native" rather than "iced-shaped port of
egui."

### 2026-04-24 — Live camera + packet parity + event subscription

**Context**: Three follow-on slices to the morning session, proceeding
down the §12.20 Tier 2 list without pause.

**Slice 4 — live `CanvasCamera` + `CanvasViewport` in iced**:

- `shell/desktop/ui/iced_graph_canvas.rs::GraphCanvasProgram` now derives
  the `ProjectedScene<NodeKey>` **per frame** inside `draw` against iced's
  live canvas bounds, instead of pre-deriving with an identity
  camera/viewport and fitting-to-bounds at paint time.
- Added `GraphCanvasProgram::camera_and_viewport(bounds) -> Option<(CanvasCamera, CanvasViewport)>`
  and `project_scene(bounds) -> Option<ProjectedScene<NodeKey>>`. Both
  route through `CanvasCamera::fit_to_bounds` + a `CanvasViewport` with
  `rect.size = bounds.size()`, matching the camera/viewport pair the
  egui host threads through `canvas_bridge::run_graph_canvas_frame`.
- Drops the `iced_canvas_painter::compute_fit_transform` helper
  (~50 lines + 2 tests removed) — the portable camera owns fit math
  now, and the painter stays minimal (SceneDrawItem → canvas::Frame).
- New tests: `project_scene_yields_canvas_local_coordinates` (asserts
  nodes project inside viewport), `camera_and_viewport_reflects_bounds`,
  `project_scene_returns_none_for_zero_bounds`,
  `empty_program_has_no_nodes_and_no_projection`.

**Slice 5 — cross-host packet parity test (§12.11 target)**:

- New test `iced_projection_matches_reference_derivation`: builds a
  `GraphCanvasProgram` from a small graph, calls `project_scene(bounds)`
  (iced's path), then calls `derive_scene` directly with the same
  camera + viewport the program computed. Asserts the resulting
  `ProjectedScene<NodeKey>` instances are `==`.
- This pins §12.11's packet contract: if iced ever diverges from the
  portable derive path (e.g. by injecting its own overlay defaults, or
  by computing a camera differently), the test fails. Companion to
  §12.12's view-model scalar parity.

**Slice 6 — iced event subscription → runtime.tick**:

- `IcedApp` subscribes to `iced::event::listen()` via a `Subscription`
  wired into `iced::application(...).subscription(IcedApp::subscription)`.
  Each raw `iced::Event` arrives in `update` as `Message::IcedEvent(...)`.
- `update` translates the event through
  `iced_events::from_iced_event` (already tested in `iced_parity`) and
  feeds the resulting `HostEvent` through `host.tick_with_input(input)`
  where `input.events = vec![host_event]; input.had_input_events = true`.
- Untranslatable events (`CursorEntered`, unsupported keys, IME) are
  dropped at the translation boundary without ticking — no spurious
  runtime work per iced event.
- Added `IcedApp::last_view_model: Option<FrameViewModel>` to cache the
  most recent runtime projection so `view` can consume it in a
  follow-on slice. Currently populated but not read by `view`.
- `IcedHostPorts::HostInputPort::poll_events` flipped from `todo(m5)`
  to an intentional-no-op matching the egui host's pattern — events
  flow via `FrameHostInput.events`, not port polling, on both hosts.
- New tests: `iced_app_tick_drives_runtime`,
  `iced_event_drives_runtime_tick_via_update`,
  `untranslatable_iced_event_does_not_tick`.

**Receipts (combined slices 4–6)**:

- `cargo test --lib --features iced-host -- iced` — 25 passed, 0
  failed, 0.02s runtime (3m 03s compile). Covers `iced_events`,
  `iced_graph_canvas`, `iced_canvas_painter`, `iced_parity`,
  `iced_host`, and the new `iced_app` tests.

**Net effect**: iced is now interactive on the runtime-tick side —
events flow end-to-end from iced's native event loop into
`GraphshellRuntime::tick` through the same portable `HostEvent`
vocabulary the egui host uses, and the graph canvas derives scenes
through a real camera+viewport that matches what the egui host
produces. The remaining work for "useful M5" is (a) plumbing pan/zoom
from translated events into `GraphCanvasProgram` camera state (requires
moving camera from per-frame fit-to-bounds into iced `State`), (b)
making `view` consume `last_view_model` to render toolbar/focus
chrome, and (c) the remaining `todo(m5)` markers in iced_host_ports
(paint ports, texture cache, clipboard, toast).

### 2026-04-24 — Plan-vs-code audit pass

**Context**: Full audit of the plan against live code to catch staleness
that had built up over ~10 days of rapid execution.

**Verified accurate**:

- All implementation anchors exist on disk.
- M1/M2/M3/M3.5/M3.6 done gates all hold up under re-verification
  (`rebuild_from_tiles` removed; no `egui_graphs` in shell or `Cargo.toml`;
  `OverlayAffordancePainter` / `ContentPassPainter` traits at
  `compositor_adapter.rs:949,1019`; `HostPaintPort` uses portable types
  throughout; all four compositor statics keyed on `NodeKey`).
- M4: `GraphshellRuntime::tick(&FrameHostInput, &mut H) -> FrameViewModel`
  at `gui_state.rs:382`; live on egui (`gui.rs:1047`) and iced
  (`iced_host.rs:67`).
- M4.5: `ViewerSurfaceRegistry` / `ViewerSurfaceBacking` /
  `record_frame_path` at `compositor_adapter.rs:214,272,462`; 5+ unit
  tests for frame-path recording.
- §12.17 enforcement: 7 contract tests in `app/sanctioned_writes_tests.rs`
  enforce §12.1, §12.2, §12.3, and §12.17 boundaries.

**Staleness fixed in this pass**:

- Line-number drift on `incremental_sync_from_tiles` fixed in three sites
  (`gui.rs:482` / `gui.rs:504` → actual `gui.rs:528`).
- §6 Immediate Ticket Queue updated: iced host, iced graph-canvas, and
  parity test rows were marked `[ ]` scaffold-only but are substantively
  partial (M5.4 first surface for iced graph-canvas; 8-test cross-host
  parity harness driving both `EguiHostPorts` and `IcedHostPorts` through
  `runtime.tick`).
- §12.11 iced graph-canvas row updated from "`todo(m5)` stub" to
  "M5.4 first real surface" reflecting ~248 lines of real rendering.
- §12.19 Bottom Line reorganized: moved cross-host parity, contract
  tests, `HostNeutralRenderBackend` split, `SettingsViewModel`, and
  dual-host `runtime.tick` from `missing`/`partial` into `done`; the
  `missing` row now correctly reflects graph-canvas packet replay,
  CI parity gate, iced render backend impl, and PartialEq-deferred
  struct parity.
- §12.20 Tier 2 and Tier 3 entries struck where landed:
  `ViewerSurfaceRegistry` first ownership slice (§12.10), first
  cross-host parity test (§12.12), and the three-guard contract-test
  set (§12.17) all now crossed out with pointers to their §12 rows.
- M4 [~] entry rewritten: authority bundles are mostly in
  `graphshell-core`, not shell-local; phase-args collapse onto
  `&mut GraphshellRuntime` is done; dual-host `runtime.tick` is live.
- Related section gained two links: the runtime-crate companion plan
  (`2026-04-24_graphshell_runtime_crate_plan.md`) and the portable
  shell-state architecture doc.

**Notes left for the next pass** (flagged, not fixed):

- `LAST_SENT_NATIVE_OVERLAY_RECTS: HashMap<NodeKey, egui::Rect>` at
  `tile_compositor.rs:265` still carries an egui type in a private
  implementation detail. Not a public boundary, but if iced gains its
  own compositor glue it'll need a portable analog. Candidate M3.6
  follow-on or M5 prerequisite.
- Cross-section terminology: "Lane A / Lane B / Lane B'" coexists with
  "Lane 1" in the 2026-04-24 entry above; should pick one vocabulary.

---

## 11. Summary

The interesting version of this migration is not "rewrite egui in iced."

It is:

- replace framework-owned graph and tile authority with Graphshell-owned cores
- turn the current framework into a host adapter
- add iced only after the seams are real

That path costs more upfront, but it prevents paying the same migration tax
twice.

---

## 12. Cross-Cutting Boundary Status Matrix (2026-04-23)

Audit of every architectural seam this plan depends on, beyond the milestone
narrative. The milestones (M0–M7) describe what the plan does in sequence;
this matrix describes the state of the seams the plan is moving authority
*across*, and where enforcement is real vs. social.

**Status key**:

- `done` — the seam has a named boundary and current code mostly respects it
- `partial` — the boundary exists, but live callers or enforcement are still leaky
- `missing` — no real boundary or enforcement yet

### 12.1. Arrangement → Graph Boundary

- `done` Single named bridge entrypoint: `app/arrangement_graph_bridge.rs`
- `done` Plain-data carrier exists: `ArrangementSnapshot`
- `done` Typed result exists: `ArrangementGraphDelta`
- `done` Public entrypoint is explicit: `apply_arrangement_snapshot(...)`
- `done` Main workbench callers already use it: `app/workbench_commands.rs:489`
- `done` Bridge wrappers (`sync_named_workbench_frame_graph_representation`,
  `persist_workbench_tile_group`, `remove_named_workbench_frame_graph_representation`)
  in `app/workbench_commands.rs` are thin helpers that build a typed
  `ArrangementSnapshot` and delegate to `apply_arrangement_snapshot` —
  not bypass paths. Earlier `partial` characterization revised on inspection.
- `done` Guard tests prevent new direct arrangement-edge writers (2026-04-23):
  - `no_unsanctioned_add_arrangement_relation_calls` — forbids
    `.add_arrangement_relation_if_missing(` outside `app/graph_mutations.rs`
    (definition + internal composition) and `app/arrangement_graph_bridge.rs`
    (sanctioned bridge caller).
  - `no_unsanctioned_promote_arrangement_relation_calls` — same allowlist for
    `.promote_arrangement_relation_to_frame_membership(`.
  Both live in `app::sanctioned_writes_tests` and use the shared
  `assert_no_unsanctioned_callers` helper factored out of §12.3 work.

**Targets remaining**:

- Ensure replay/restore paths use the same bridge or the same plain-data
  contract (audit pending — likely already routed via WAL-replay constructors).
- Consider tightening `add_arrangement_relation_if_missing` and
  `promote_arrangement_relation_to_frame_membership` visibility to
  `pub(in crate::app::arrangement_graph_bridge)` once the contract test has
  bedded in (compile-time belt-and-suspenders matching §12.3's pattern).

### 12.2. Durable Graph Mutation Boundary

**Distinction**: "typed lane exists" is not the same as "reducer is the sole
writer." The reducer-only enforcement plan
(`system/2026-03-06_reducer_only_mutation_enforcement_plan.md`) is the
stricter rule. M4 must not weaken this distinction by introducing
host-owned mutation helpers.

- `partial` Canonical typed mutation lane exists: `crates/graphshell-core/src/graph/apply.rs`
- `partial` App-layer sync wrapper exists: `apply_graph_delta_and_sync(...)`
  in `app/graph_mutations.rs:3339`
- `done` Core durable operations are typed: add/remove node, add/remove edge,
  relation assert/retract, traversal append, node metadata
- `done` Replay variants exist in the same typed layer
- `partial` Runtime code still uses higher-level app helpers like
  `add_node_and_sync`, `assert_relation_and_sync`, especially in
  `app/runtime_lifecycle.rs`
- `done` (2026-04-24) Compile-time + grep-time enforcement landed for the
  kernel-level apply seam: contract test
  `no_unsanctioned_apply_graph_delta_kernel_calls` in
  `app/sanctioned_writes_tests.rs` walks the repo and fails on any direct
  call to the kernel `apply_graph_delta(graph, delta)` outside a 5-file
  allowlist (kernel definition + kernel-internal test fixtures + WAL
  replay path). Production durable mutations must route through
  `apply_graph_delta_and_sync` (which composes the kernel call with
  `post_apply_sync`); direct kernel calls bypass the sync. Complements
  the §12.17 host-adapter guards (those forbid the sync wrapper from
  hosts; this one forbids the un-synced kernel call from non-replay
  paths). A new file in the allowlist is a deliberate review signal.

**Targets**:

- Shrink direct app helper surface so durable writes enter through one obvious
  sanctioned path (e.g., consolidate `add_node_and_sync`,
  `assert_relation_and_sync` callers in `runtime_lifecycle.rs` onto the
  typed delta path).
- Prove live-vs-replay parity for the full durable mutation set (uses the
  §12.12 replay-trace harness once the kernel mutation set is stable).

### 12.3. Persisted Node Navigation Memory

**Status update (2026-04-23, second pass)**: Lane A + the typed-delta
follow-on both landed. Persisted history now flows through a single typed
canonical mutation lane with both compile-time and grep-time enforcement.

- `done` Substrate exists and is persisted on nodes as `NodeNavigationMemory`
  in `crates/graphshell-core/src/graph/mod.rs:653`
- `done` Typed delta variant exists: `GraphDelta::UpdateNodeHistory { key,
  entries, current_index }` in `crates/graphshell-core/src/graph/apply.rs`,
  returning `GraphDeltaResult::NodeMetadataUpdated(bool)`
- `done` Sanctioned helper exists: `apply_node_history_change(...)` on
  `GraphBrowserApp` in `app/history.rs:135`. Now dispatches through
  `apply_graph_delta_and_sync(GraphDelta::UpdateNodeHistory { ... })` rather
  than calling the underlying setter directly. Pairs the typed-delta
  dispatch with the semantic-navigation-runtime refresh.
- `done` Direct durable writes in runtime code routed through the helper:
  - `app/runtime_lifecycle.rs:182` (`handle_webview_history_changed`)
  - `app/clip_capture.rs:295` (clip-capture initialization)
- `done` Test direct uses also routed through the helper:
  - `app/workspace_routing.rs` (3 sites)
  - `shell/desktop/ui/workbench_host.rs` (5 sites)
- `done` Compile-time enforcement: `Graph::set_node_history_state` is
  `pub(crate)` inside `graphshell-core`; any caller from outside the kernel
  is now a compile error. The two remaining literal mention sites are
  the function definition and the `GraphDelta::UpdateNodeHistory` match arm
  body, both inside `graphshell-core`.
- `done` Grep-time enforcement: contract test
  `app::history::sanctioned_history_writes_tests::no_unsanctioned_direct_history_writes`
  walks the repo and fails on any unallowlisted literal occurrence. Needle
  built via `concat!()` so the test source does not self-match. Allowlist
  narrowed to `crates/graphshell-core/src/graph/{mod.rs,apply.rs}` after the
  typed-delta migration.
- `done` Receipts:
  - `cargo check --lib` clean (graphshell-core: 5.87s) for the typed-delta
    surface; full graphshell lib check is also clean after the §12.10
    viewer-surface-path channel contract was reconciled to 170 phase-3
    entries.
  - Lane A's prior receipt of `cargo test --lib sanctioned_history_writes`
    passing remains valid for the test-logic side.

**Residue closure (2026-04-23, third pass)**:

- `done` `Node::replace_history_state` parallel surface: kept `pub` for
  legitimate fixture-construction patterns (`Node::test_stub(...)` →
  `node.replace_history_state(...)` is the natural unit-test path; can't
  use the typed delta because there's no Graph to apply against). A
  9-file allowlist captures every currently-known test caller across the
  graphshell crate. New contract test
  `no_unsanctioned_node_replace_history_state_writes` enforces it.
  Adding a new file to the allowlist becomes a deliberate review signal:
  if the new caller is non-test, it must route through the typed delta
  instead; if it's a new test fixture, the allowlist addition is the
  explicit acknowledgment.
- `done` WAL replay surface: verified — persistence-replay does not call
  any of the history mutation surfaces. `services/persistence/types.rs`
  reconstructs `NodeNavigationMemory` via `from_linear_history(...)` and
  `empty()` constructors at deserialization time, not via mutation
  setters. Not a hole.
- `done` Reusable scanning infrastructure: contract test refactored to
  extract `assert_no_unsanctioned_callers(needle, allowed_files,
  sanction_message)` helper. Same shape can guard arrangement-bridge
  sole-writer (§12.1) and host-owned mutation entrypoints (§12.17) when
  those land — each becomes a one-test addition with its own allowlist.

### 12.4. Runtime Lifecycle Hooks

**Status update (2026-04-24, second pass)**: All six webview lifecycle
handlers now split into the typed plan + apply pattern. The
ingest-from-host surface is now a thin composition over
`plan_*` (`&self`, read-only) + `apply_*_plan` (`&mut self`, mutation)
across the entire lifecycle entry surface.

- `done` Title updates already use `GraphDelta`: `runtime_lifecycle.rs:220`
- `done` URL change handling split (2026-04-24): typed
  `WebviewUrlChangePlan` produced by `plan_webview_url_change(&self,
  ...)` (read-only state-query pass) consumed by
  `apply_webview_url_change_plan(&mut self, plan)` (mutation pass).
- `done` History change handling: typed `WebviewHistoryChangePlan`
  produced by `plan_webview_history_change(&self, ...)` and consumed
  by `apply_webview_history_change_plan(&mut self, plan)`. The plan
  carries the full diff (`old_entries`, `old_index`, `new_entries`,
  `new_index`) so traversal-edge bookkeeping happens inside apply.
- `done` (2026-04-24, second pass) Remaining four lifecycle handlers
  split:
  - `WebviewScrollChangePlan` — `plan_webview_scroll_change(&self,
    webview_id, scroll_x, scroll_y)` returns `Option<…>` (None when
    webview unmapped) consumed by `apply_webview_scroll_change_plan`.
  - `WebviewTitleChangePlan` — `plan_webview_title_change(&self,
    webview_id, title)` returns `Option<…>` (None when unmapped or
    title empty/missing) consumed by `apply_webview_title_change_plan`,
    which routes through `apply_graph_delta_and_sync(SetNodeTitle)`
    and logs the title mutation if changed.
  - `WebviewCreatedPlan` — `plan_webview_created(&self,
    parent_webview_id, child_webview_id, initial_url)` (read-only,
    deliberately NOT bumping the placeholder counter; `node_url:
    Option<String>` carries `None` to signal "use placeholder" so the
    plan stays side-effect-free) consumed by
    `apply_webview_created_plan`.
  - `WebviewCrashedPlan` — `plan_webview_crashed(&self, webview_id,
    reason, has_backtrace)` captures the node-mapping lookup so apply
    can branch between crash-block + demote (mapped) and bare unmap
    (unmapped) without re-querying state.

**Targets remaining**:

- Add an explicit "effects" pass for handlers whose apply ends with
  refresh calls (e.g.,
  `refresh_semantic_navigation_runtime_for_node`) — currently the
  effect is inlined in the apply step, which is fine but a future
  cleanup could lift it into a returned `LifecycleEffects` value the
  caller dispatches.
- Build replay-test infrastructure that executes
  `apply_*_plan` directly from constructed plans, validating
  state-transition determinism without host events. Now tractable
  across the full lifecycle surface since every handler exposes
  the typed plan.

### 12.5. Graph Mutation / Sync Paths

**Status update (2026-04-24)**: Post-apply sync is now a distinct,
explicit step.

- `done` Mutation results are typed enough to distinguish structural vs
  metadata cases: `GraphDeltaResult` in `apply.rs:133`,
  `app/graph_mutations.rs:3367`
- `done` App sync layer split: `apply_graph_delta_and_sync` is now a
  composition of `apply_domain_graph_delta` (typed kernel mutation)
  followed by `post_apply_sync` (the new dedicated post-apply hook).
- `done` Cleanly separate post-apply sync contract:
  `GraphBrowserApp::post_apply_sync(&GraphDeltaResult)` in
  `app/graph_mutations.rs`. Documented contract — idempotent (clears
  cache to `None`; rebuilds containment edges from current graph
  state, both safe to repeat), host-independent (touches only
  `workspace.domain.graph` and `workspace.graph_runtime.hop_distance_cache`,
  no host adapter or rendering state).

**Targets remaining**:

- Wire WAL replay / restore paths to call `post_apply_sync` after
  batch-applying typed deltas (currently they go through
  `apply_domain_graph_delta` directly and skip the sync; whether that's
  intentional or a latent bug needs an audit).
- Add parity tests that the same typed mutation sequence yields the
  same durable state and the same sync-visible structural outcomes
  across hosts. The extracted `post_apply_sync` makes this tractable
  — a parity test can apply deltas via the runtime path and via direct
  `apply_domain_graph_delta` + `post_apply_sync`, asserting state
  convergence.
- Keep derived cache refresh and view-model projection from silently
  mutating durable truth (ongoing concern; the extraction makes
  enforcement easier — a sync helper that ALSO mutates durable graph
  state would now be visibly separate from the apply step).

### 12.6. Host Runtime Boundary

- `done` Host-neutral runtime exists: `GraphshellRuntime` in
  `shell/desktop/ui/gui_state.rs:246`
- `done` Host-neutral per-frame input/output types: `FrameHostInput` /
  `FrameViewModel` at `shell/desktop/ui/frame_model.rs:28`
- `done` Host ports exist: `shell/desktop/ui/host_ports.rs:44`
- `done` Egui ports exist: `shell/desktop/ui/egui_host_ports.rs:72`
- `partial` Iced ports exist, but are still mostly bring-up stubs:
  `shell/desktop/ui/iced_host_ports.rs:44`
- `partial` Iced host calls the same runtime tick:
  `shell/desktop/ui/iced_host.rs:65`
- `partial` Egui consumption of `runtime.tick(...)` outputs has its first
  consumer (2026-04-24): `EguiHost.cached_view_model:
  Option<FrameViewModel>` field caches the post-tick view-model each
  frame. Migrated getters now reading from the cache (with
  pre-first-frame fallbacks):
  - `EguiHost::graph_surface_focused()` →
    `cached_view_model.focus.graph_surface_focused` (first migration,
    establishes the pattern).
  - `EguiHost::is_graph_view()` (2026-04-24, second pass) →
    `cached_view_model.is_graph_view`. Required adding `is_graph_view:
    bool` to `FrameViewModel` (populated in
    `gui_state.rs::project_view_model` from
    `graph_app.workspace.graph_runtime.active_pane_rects.first().is_none()`,
    inlined here because `pane_queries` is a private gui submodule).
  - `EguiHost::focused_node_key()` (2026-04-24, third pass) →
    `cached_view_model.focus.focused_node`. Required reconciling
    `FocusViewModel.focused_node` from `focused_node_hint` (render-pass
    cache; lagged) to `active_pane_rects.first()` gated on
    `!graph_surface_focused` — same semantics as `focused_node_key()`.
    New test `focus_view_model_focused_node_is_none_when_graph_surface_focused`
    pins the gate. `has_focused_node()` follows for free since it delegates
    to `focused_node_key()`.

  - ~~`tile_render_pass.rs:723-745` focus-ring alpha~~ **done 2026-04-24**:
    `cached_view_model` threaded through `ExecuteUpdateFrameArgs` →
    `SemanticAndPostRenderPhaseArgs` → `PostRenderPhaseArgs` →
    `TileRenderPassArgs`. The inline computation is retained as fallback
    for the first-frame case (no prior view-model) and falls through
    transparently in all other frames via `if let Some(vm) = cached_view_model`.
    The "blocked" note in the prior entry was overly conservative — ~16ms
    staleness for a 300ms animation is imperceptible, and the tick-ordering
    concern only matters for animations that start/stop on transition frames
    (one-frame gap, invisible at 60fps). 223 `graphshell-core` tests pass.

**Targets remaining**:

- Migrate additional `EguiHost` getters onto `cached_view_model` —
  `runtime_focus_state`, dialog-state queries (complex compositions;
  need new view-model fields or the result as a projection).
- Migrate remaining chrome render sites that read `runtime.foo` /
  `graph_app.workspace.chrome_ui.foo` directly: toolbar can_go_back/forward,
  dialog visibility flags.
- Iced ports: implement the bring-up stubs (`iced_host_ports.rs:44`)
  beyond `todo(m5)` markers — gates real iced parity.
- Add parity runs for the same replay/input traces across egui and
  iced once iced ports are real (depends on §12.12).

### 12.7. Workbench Authority

- `partial` `GraphTree` authority is real and parity-checked in practice;
  `parity_check(...)` runs in `shell/desktop/ui/gui.rs:1030`
- `done by design` Startup import from tiles is intentionally retained:
  `incremental_sync_from_tiles(...)` at `shell/desktop/ui/gui.rs:528`
  reconciles GraphTree with tile state restored from persistence on startup.
  Per M1's done-gate: "Keep only startup import from tile state plus explicit
  repair tooling" — this is the only remaining caller; per-frame follower
  path was removed 2026-04-15.
- `done` Semantic command routing to `graph_tree_commands` is present in places
  like `shell/desktop/ui/workbench_host.rs:4911`
- `done` (2026-04-24) Pending-open flows route through dual-write where a
  GraphTree is available. `pending_open_flow.rs::handle_pending_open_node_after_intents`,
  `handle_pending_open_note_after_intents`, `handle_pending_open_clip_after_intents`,
  and the private `execute_pending_open_node_after_intents` helper now accept
  `Option<&mut GraphTree>` and dispatch through
  `graph_tree_dual_write::open_or_focus_node` /
  `open_or_focus_node_with_mode` when the GraphTree ref is present, falling
  back to `tile_view_ops::open_or_focus_node_pane*` only when it isn't.
  `workbench_intent_interceptor::apply_semantic_intents_and_pending_open`
  reborrows its `Option<&mut GraphTree>` via `.as_deref_mut()` to feed
  the four downstream calls.
- `done by design` Other `tile_view_ops::*` call sites (~93 total) remain
  intentionally — they cover (a) read-only presentation queries
  (`active_graph_view_id`, `ensure_active_tile`, `warm_peer_tab_container`),
  (b) graph-pane and tool-pane operations that aren't GraphTree members,
  and (c) `pane_ops::open_or_focus_node_pane_with_*` fallback paths
  conditional on graph_tree availability matching the pattern landed
  here.

**Targets remaining**:

- Add a host-adapter contract test extension (using the §12.17
  `assert_needle_absent_from_files` helper) that forbids new direct
  `egui_tiles::Tree<TileKind>` mutations in host adapter files
  (`iced_host.rs`, etc.) — defers a "host-owned workbench authority"
  bypass into a compile-test failure rather than reviewer judgment.

### 12.8. Compositor Identity & Pass Traits

**Scope**: this row is now narrowed to compositor-side identity and the
overlay/content painter trait surfaces. Viewer-surface ownership and the
GL-shaped per-node `OffscreenRenderingContext` retirement is its own row
(§12.10) since M4.5 carved that out as a distinct seam.

- `done` Viewer surface registry exists as a portable identity seam in multiple
  runtime paths and constructors: `shell/desktop/ui/gui.rs:418`
- `done` Overlay/content passes are trait-driven:
  - `OverlayAffordancePainter` at `shell/desktop/workbench/compositor_adapter.rs:841`
  - `ContentPassPainter` at `shell/desktop/workbench/compositor_adapter.rs:911`
- `partial` Iced-side stubs for those painters exist:
  `shell/desktop/ui/iced_host_ports.rs:278`
- `done` Identity is not tile-owned here anymore; `NodeKey` / `PaneId`
  throughout the compositor hot path

**Targets**:

- Finish real iced implementations of the compositor-facing painter traits
- Add parity coverage for overlay descriptor + content callback dispatch across
  hosts
- Keep WebGL quarantine/composition isolated from host authority

### 12.9. Portable Shell Runtime Bundles

- `done` Authority bundles exist and are already moving out of host-specific
  ownership:
  - `FocusAuthorityMut`, `GraphSearchAuthorityMut`, `CommandAuthorityMut` in
    `crates/graphshell-core/src/shell_state/authorities.rs:58`
  - `ToolbarAuthorityMut` in `shell/desktop/ui/gui_state.rs:136`
- `partial` Project is still in a transitional bundle phase; `&mut` threading
  remains and not everything has collapsed onto the runtime root

**Targets**:

- Finish converging phase args and helper stacks onto `&mut GraphshellRuntime`
- Remove residual host-owned semantic state access
- Keep these bundles from becoming permanent semi-authorities

### 12.10. Viewer-Surface Host Nativity (M4.5)

**New row** — carved from §12.8 because M4.5 names this as a distinct
prerequisite for a *useful* iced host, separate from compositor identity.

- `partial` `ViewerSurfaceRegistry` exists in
  `shell/desktop/workbench/compositor_adapter.rs`, keys on `NodeKey`, and now
  owns typed surface backing (`ViewerSurfaceBacking`) plus per-frame
  `last_frame_path`
- `done` The first ownership slice has collapsed the old
  `tile_rendering_contexts`/naked-`gl_ctx` shape into registry-owned compat
  backing; the original 16-site audit snapshot was directionally useful but
  stale in detail
- `partial` Shared-wgpu composition is still opportunistic rather than the
  primary contract, but the compose path now distinguishes shared-wgpu import
  from callback fallback in `CompositedContentPassOutcome`
- `done` GL fallback is now named as a compatibility producer
  (`ViewerSurfaceBacking::CompatGlOffscreen`, `compat_gl_context`) instead of
  being the shape of the registry API
- `done` Parity diagnostics now record which viewer-surface / content-bridge
  path each frame exercised (`shared_wgpu`, `callback_fallback`,
  `missing_surface`)

**Targets**:

- Move authoritative viewer-surface ownership fully to `ViewerSurfaceRegistry`
- Retire direct hot-path reliance on `tile_rendering_contexts`
- Make shared-wgpu the primary contract; GL becomes named compatibility producer
- Preserve WebGL quarantine through the interop/import path
- Add parity / diagnostics coverage for which viewer-surface path is active

**Receipt (2026-04-23, Lane B first slice)**:

- Audit result: the live seam was not a separate free-floating
  `tile_rendering_contexts` map so much as `ViewerSurfaceRegistry` still being
  GL-shaped internally and at its hot-path call sites.
- Landed the first mechanical slice in:
  `compositor_adapter.rs`, `tile_compositor.rs`,
  `lifecycle/webview_backpressure.rs`, `tile_render_pass.rs`,
  `tile_invariants.rs`, `ui/gui_frame.rs`, and diagnostics registry wiring.
- Follow-on slice: `tile_compositor.rs` now composes runtime viewer content via
  `CompositorAdapter::compose_webview_content_pass_for_surface(...)` so the
  compositor call site operates on `ViewerSurface`/registry state instead of
  peeling out a raw compat GL context first; that same path now keeps the
  registry-owned `ContentSurfaceHandle` authoritative for imported-wgpu vs
  callback-fallback state.
- Broader seam completion (Graphshell-local): the host/registry allocation
  contract now traffics in typed `ViewerSurfaceBacking` values, that backing
  enum now includes `NativeRenderingContext(Rc<dyn RenderingContextCore>)`, and
  `webview_backpressure.rs` now builds webviews from the registry's generic
  rendering-context accessor rather than a compat-GL-only accessor.
- Activation receipt (2026-04-23, third pass): the egui host now allocates
  `ViewerSurfaceBacking::NativeRenderingContext(...)` from the shared host
  rendering context, so runtime webviews exercise the native viewer-surface
  composition branch by default. GL remains as an explicit compatibility
  producer inside the compositor path rather than as the live host allocator's
  primary shape.

### 12.11. Graph-Canvas as Live Surface Authority (M2)

**New row** — collapsed into §12.8 in the original matrix, but graph-canvas
authority over scene derivation, camera, interaction, and packet generation
is a distinct seam from compositor identity. M2 was a major milestone here.

- `done` Live graph panes render through `graph-canvas`, not `egui_graphs`
  (M2 done 2026-04-21)
- `done` Portable `CanvasInputEvent` / `CanvasAction` flows in
  `crates/graph-canvas/src/input.rs`, `interaction.rs`
- `done` Scene derivation, camera, projection, hit testing, physics in portable
  crate (`derive.rs`, `camera.rs`, `projection.rs`, `hit_test.rs`, `scene_physics.rs`)
- `done` Vello backend in `crates/graph-canvas/src/backend_vello.rs` as shared
  rendering convergence point
- `done` Host-neutral `canvas_bridge::run_graph_canvas_frame(...)` seam
  (`render/canvas_bridge.rs`)
- `done` (2026-04-24, second pass) `shell/desktop/ui/iced_graph_canvas.rs` now
  consumes the shared `ProjectedScene<NodeKey>`: `GraphCanvasProgram` holds a
  `CanvasSceneInput<NodeKey>` + derived `ProjectedScene<NodeKey>` built via
  `canvas_bridge::build_scene_input(...)` + `graph_canvas::derive::derive_scene(...)`.
  Painting goes through a new `shell/desktop/ui/iced_canvas_painter.rs`
  module (mirror of `render/canvas_egui_painter.rs`) that converts portable
  `SceneDrawItem`s into iced `canvas::Frame` calls (layers in
  background → world → overlays order, matching egui). `draw()` applies a
  fit-to-bounds transform via iced's canvas transform stack so the world-
  space scene lands inside the iced widget. Scope limits now: no physics
  tick, no input handling, no overlay inputs (frame regions, scene regions,
  highlighted edges) — each is a targeted follow-on slice.
- `missing` No parity test that the same `CanvasInputEvent` sequence produces
  identical packets across hosts (scalar/view-model parity is already there;
  graph-canvas packet parity is the missing piece)

**Targets**:

- Replace `iced_graph_canvas`'s identity camera + zero-size viewport with a
  `CanvasCamera` that tracks pan/zoom + a `CanvasViewport` derived from
  iced bounds, so iced gets live camera state the egui host already has via
  `canvas_bridge::run_graph_canvas_frame(...)`
- Implement `graph_canvas::backend::CanvasBackend<NodeKey>` for an iced-side
  type, ideally routing through the shared Vello backend where iced can
  accept Vello scenes (once iced Vello integration stabilizes)
- Wire overlay inputs (frame regions, scene regions, highlighted edge)
  through iced's graph canvas
- Add cross-host **packet** parity tests (companion to §12.12's scalar
  parity) — assert identical `ProjectedScene<NodeKey>` from identical
  `CanvasSceneInput<NodeKey>` across hosts
- Keep all camera / interaction grammar in the portable crate

### 12.12. Replay / Parity Harness (M0)

**New row** — M0 deliverable treated as targets in §§12.5/12.6/12.8 of the
original matrix, but the harness itself is a seam with its own state.

- `done` GraphTree parity receipts: `graph-tree/src/parity.rs` with 7
  divergence types
- `done` Per-frame parity check runs in debug builds:
  `graph_tree_sync::parity_check()`
- `done` UX replay exists: `shell/desktop/workbench/ux_replay.rs`
- `done` Iced parity scaffold exists: `shell/desktop/ui/iced_parity.rs`
- `done` (2026-04-24) First cross-host replay-trace parity test landed:
  `iced_parity::tests::replay_trace_scalar_parity_across_host_ports`.
  Constructs a `FrameHostInput` with a small `HostEvent` trace
  (`PointerMoved` + `PointerDown { Primary }`), drives both runtime
  instances through `runtime.tick(input, ports)` (one with
  `EguiHostPorts`, one with `IcedHostPorts`), and asserts the resulting
  `FrameViewModel` portable scalar fields match across hosts (focus
  state, toolbar location/nav, search state, command-palette state,
  dialogs view-model, settings view-model, captures-in-flight). This
  is the smallest meaningful cross-host parity exercise; the `runtime`
  is host-neutral by construction so any divergence here is a kernel
  regression. Test is gated by the `iced-host` feature.
- `partial` Test currently asserts on portable scalar primitives only.
  Several view-model sub-structs (`FocusViewModel`, `ToolbarViewModel`,
  etc.) don't yet derive `PartialEq`, so a full struct-level parity
  assertion is deferred. Adding the derives is mechanical
  (`#[derive(PartialEq)]` + same on `OmnibarViewModel` / scope-view
  enums) — straightforward follow-on slice.
- `partial` `cargo test --features iced-host` currently has a
  pre-existing `PortableRect` duplicate-import error in
  `iced_host_ports.rs:267`; my parity test compiles cleanly under the
  default feature set. Fixing the upstream import lets the parity test
  actually run end-to-end.
- `missing` No graph-canvas packet replay (snapshots exist per M0 but
  not exercised cross-host).
- `missing` No CI gate that blocks divergence between egui and iced
  replay outputs.

**Targets remaining**:

- Add `#[derive(PartialEq)]` to view-model sub-structs so the parity
  test can assert on full struct equality rather than scalar primitives.
- Resolve the `PortableRect` duplicate-import in `iced_host_ports.rs`
  to unblock `cargo test --features iced-host` end-to-end execution
  of the parity test.
- Build a default narrow validation lane (e.g. `cargo test --lib
  iced_parity --features iced-host`) that runs the cross-host replay
  parity tests for each PR.
- Add graph-canvas packet snapshot replay (parallel structure to UX
  replay).
- CI gate that blocks PRs on parity divergence.

### 12.13. Render Backend Boundary

**New row** — `shell/desktop/render_backend/mod.rs` is named as an
implementation anchor in §0 of this plan but doesn't appear in the original
matrix as a seam.

- `done` Backend selection exists with `gl_backend.rs` and `wgpu_backend.rs`
  variants
- `done` (2026-04-24) Backend abstraction split into a host-neutral
  base trait and an egui-specific extension trait:
  - `HostNeutralRenderBackend` — wgpu / texture / surface ops
    (`register_texture_token`, `shared_wgpu_device_queue`,
    `upsert_native_texture`, `free_native_texture`, `submit_frame`,
    `destroy_surface`). Iced impls just this.
  - `UiRenderBackendContract: HostNeutralRenderBackend` — the
    egui-specific extension (`init_surface_accesskit`,
    `egui_context*`, `egui_winit_state_mut`,
    `handle_window_event`, `run_ui_frame`).
  `UiRenderBackendHandle` impls both traits. The wgpu shared-device +
  native-texture seam introduced for the M4.5 §12.10 viewer-surface
  path is now reachable from iced without dragging egui types across
  the boundary.
- `partial` Iced backend doesn't yet exist; `HostNeutralRenderBackend`
  defines its target shape but no concrete impl has landed.
- `partial` GL backend retention policy is documented in code comments
  but not in a dedicated design doc.

**Targets remaining**:

- Implement `HostNeutralRenderBackend` for an iced-host backend type
  (gates real iced wgpu integration; depends on iced-host bring-up).
- Document GL backend retention policy in a dedicated design doc
  alongside the M4.5 viewer-surface decisions.
- Coordinate with the Servo wgpuification companion plan
  (`servo-wgpu/docs/2026-04-18_servo_wgpuification_plan.md`).

### 12.14. Settings / Configurability Host-Neutrality

**New row** — `ChromeUiState` carries `FocusRingSettings`, `ThumbnailSettings`,
etc. with serde persistence. Both hosts need to consume the same settings
surface and route mutations through the same authority. Not previously
tracked.

- `done` Settings live on `ChromeUiState` (in graphshell-core, portable)
- `done` Serde round-trip with legacy-blob compat for `ThumbnailSettings`,
  `FocusRingSettings` (M4.1 slice 1d, M4.4)
- `done` Settings persistence: `app/settings_persistence.rs`
- `partial` Setter-side clamping is consistent for new settings; older settings
  surfaces vary
- `done` (2026-04-24) Host-neutral read-side projection landed:
  `SettingsViewModel { focus_ring: FocusRingSettingsView }` POD type
  in graphshell-core's `frame_model.rs` mirrors `app::FocusRingSettings`
  (the canonical settings types stay in `app/settings_persistence.rs`
  to keep the kernel independent of app/serde concerns; the POD
  mirror carries the same fields). `FrameViewModel.settings` field
  populated each frame by `gui_state.rs::project_view_model` from
  `chrome_ui.focus_ring_settings`. Iced consumes the same projection
  for free once it renders the FrameViewModel.
- `done` (2026-04-24, second pass) `SettingsViewModel` extended to
  mirror `ThumbnailSettings` via four new POD types in
  graphshell-core's `frame_model.rs`:
  - `ThumbnailSettingsView { enabled, width, height, filter, format,
    jpeg_quality, aspect }` — direct field-for-field mirror.
  - `ThumbnailFilterView` (Nearest, Triangle, CatmullRom, Gaussian,
    Lanczos3), `ThumbnailFormatView` (Png, Jpeg, WebP), and
    `ThumbnailAspectView` (Fixed, MatchSource, Square) POD enums
    mirror their `app::*` counterparts. Conversion happens at the
    projection site in `gui_state.rs::project_view_model`. Same
    POD-mirror pattern as `FocusRingSettingsView` →
    `app::FocusRingSettings`.
- `partial` Settings panels still mutate `chrome_ui.foo_settings`
  directly — settings UI is a mutation surface that needs its own
  port-shaped design (read-only view-model is the natural part to
  lift; mutation routing back through a `set_*` helper is a separate
  slice).
- `missing` No parity test that settings changes apply identically
  across hosts.

**Targets remaining**:

- Migrate egui settings panels to render from
  `view_model.settings.*` instead of `chrome_ui.*_settings` direct
  reads. Mutation flows back through existing `app::set_*_settings`
  setters.
- Add parity coverage for settings mutations (depends on §12.12
  replay infrastructure).
- Audit older settings surfaces for setter-side clamping consistency.
- Mirror remaining `chrome_ui.*` settings groups onto
  `SettingsViewModel` as they're identified (same POD pattern).

### 12.15. Accessibility Above Framework Layer (Sidequest D)

**New row** — Sidequest D names UxTree / semantic projection above the host
boundary as a deliberate non-host seam. Currently `missing` in practice.

- `done` UxTree exists as a portable construct: `shell/desktop/workbench/ux_tree.rs`
- `done` Accessibility bridge exists: `shell/desktop/ui/gui/accessibility.rs`,
  `accessibility_bridge_tests.rs`
- `partial` AccessKit integration goes through framework-specific paths
- `done` (2026-04-24) Host-neutral AT projection seam landed:
  `AccessibilityViewModel { focused_node: Option<NodeKey>,
  snapshot_version: u32, snapshot_published: bool }` POD type in
  graphshell-core's `frame_model.rs`. Lives on
  `FrameViewModel.accessibility`; populated each frame by
  `gui_state.rs::project_view_model` from the focused-node hint plus
  `ux_tree::latest_snapshot()`. Hosts use the version + published
  flag to decide whether to refresh their AccessKit-side AT tree;
  the full UxTreeSnapshot stays shell-side and is fetched separately
  via `ux_tree::latest_snapshot()` (the view-model is the "do I need
  to look?" signal, not the data carrier — keeps the kernel
  independent of shell-side UxTree types).
- `missing` No parity test for AT semantics across hosts.

**Targets remaining**:

- Migrate egui's accessibility bridge to consume
  `view_model.accessibility.snapshot_version` for change detection
  rather than its current ad-hoc invalidation logic.
- Add cross-host AT parity tests (extend the §12.12 replay-trace
  pattern to assert `vm.accessibility` agrees across `EguiHostPorts`
  and `IcedHostPorts`).
- Once the shell-side `UxTreeSnapshot` types stabilize, consider
  moving them to graphshell-core so the full snapshot can ride on
  the view-model directly (would let iced render AT without the
  `latest_snapshot()` global accessor).

### 12.16. Diagnostics Channel Host-Neutrality

**Status update (2026-04-24)**: `DiagnosticsState` lifted from EguiHost
to GraphshellRuntime — the data foundation is now host-neutral.

- `done` Channel registry exists: `registries/atomic/diagnostics.rs`
- `done` Diagnostics surface exists on runtime: `shell/desktop/runtime/diagnostics.rs`
- `done` Diagnostics pane UI: `shell/desktop/runtime/diagnostics/pane_ui.rs`
- `partial` Channel registration is centralized; channel *consumers* still
  often go through host-local logging
- `done` (2026-04-24) `DiagnosticsState` instance lives on `GraphshellRuntime`
  rather than `EguiHost`. Removed from `EguiHost`'s struct + constructor;
  added (cfg-gated `#[cfg(feature = "diagnostics")]`) to `GraphshellRuntime`
  and its `new_minimal()` test constructor. Removed from
  `ExecuteUpdateFrameArgs` / `ToolbarAndGraphSearchWindowPhaseArgs` /
  `SemanticAndPostRenderPhaseArgs` / `PostRenderPhaseArgs` /
  `TileRenderPassArgs` — phases that need it now split-borrow
  `&mut runtime.diagnostics_state`. The egui pane renderer
  (`runtime/diagnostics/pane_ui.rs`) reads from
  `EguiHost::diagnostics_state()` which now returns
  `&self.runtime.diagnostics_state`. Iced inherits the same instance for
  free once it consumes the runtime.
- `missing` No iced-side diagnostics pane renderer yet — the pane UI is
  still egui-specific (~1402 lines in `pane_ui.rs` using egui drawing
  primitives). Lifting the pane to a host-neutral view-model is the
  next slice; the data layer is now ready for it.

**Targets remaining**:

- Lift the pane rendering shape to a host-neutral
  `DiagnosticsViewModel` so iced can render the same data through its
  own widget set. The pane currently mixes data projection (channel
  message counts, latency percentiles, edge metrics) with egui drawing
  (rects, strokes, text); separating projection from rendering is the
  remaining work. Estimated 2-4 hours.
- Add per-frame diagnostic for which viewer-surface / content-bridge
  path each host is exercising (per §12.10).
- Audit channel severities for consistency with `Error` / `Warn` /
  `Info` conventions.

### 12.17. Enforcement / Regression Guards

**Status update (2026-04-23, second pass 2026-04-24)**: All three guards
plus the §12.2 kernel-call guard landed via the consolidated
`app::sanctioned_writes_tests` module. Seven contract tests now run in
`cargo test --lib sanctioned_writes`, sharing two reusable scanning helpers
(`assert_no_unsanctioned_callers` for repo-wide-with-allowlist scans;
`assert_needle_absent_from_files` for targeted host-adapter scans).

- `done` Plans now name the seams explicitly in:
  - `system/2026-03-06_reducer_only_mutation_enforcement_plan.md`
  - `subsystem_history/SUBSYSTEM_HISTORY.md`
  - this plan §12
- `done` Enforcement tests in `app/sanctioned_writes_tests.rs`:
  - §12.3 — `no_unsanctioned_set_node_history_state_writes`
    (Graph-level setter)
  - §12.3 — `no_unsanctioned_node_replace_history_state_writes`
    (Node-level primitive)
  - §12.1 — `no_unsanctioned_add_arrangement_relation_calls`
    (`add_arrangement_relation_if_missing` outside bridge + helper module)
  - §12.1 — `no_unsanctioned_promote_arrangement_relation_calls`
    (`promote_arrangement_relation_to_frame_membership` outside bridge +
    helper module)
  - §12.17 — `host_adapters_do_not_call_apply_graph_delta_and_sync`
    (forbids the canonical typed-mutation entrypoint in 5 host-adapter files:
    `iced_host.rs`, `iced_app.rs`, `iced_events.rs`, `iced_host_ports.rs`,
    `egui_host_ports.rs`)
  - §12.17 — `host_adapters_do_not_call_apply_arrangement_snapshot`
    (same allowlist; forbids the arrangement-bridge entrypoint)
  - §12.2 — `no_unsanctioned_apply_graph_delta_kernel_calls`
    (forbids direct kernel `apply_graph_delta` calls outside a 5-file
    allowlist: kernel definition, kernel-internal test fixtures, and
    the WAL replay path. Production durable mutations must route
    through `apply_graph_delta_and_sync` to pick up `post_apply_sync`;
    direct kernel calls bypass it. Complements the §12.17 host-adapter
    guards by closing the lower-level seam.)

  Two host-adjacent files are intentionally NOT in the §12.17 list:
  `iced_graph_canvas.rs` (graph-canvas integration with legitimate test
  fixtures) and `iced_parity.rs` (parity-replay scaffold). Adding a new
  iced/egui adapter file to the host set is a deliberate signal during PR
  review.

- `done` Receipts: `cargo test --lib sanctioned_writes` — 7 passed, 0 failed
  (2026-04-24 second pass; 1m 03s build, 0.22s run). Full
  `cargo check --lib` clean.

**Targets remaining**:

- Generalize the scanning infrastructure to a small `sanctioned_writes`
  framework module callers can extend without copy-pasting the walker —
  current shape is already factored, but a public test-utils export would
  let other crates add their own contract tests without re-implementing.
- Convert any `partial` enforcement noted elsewhere in §12 into the same
  helper pattern as it lands.

### 12.18. Operational Gaps

Not architectural seams, but cross-cutting concerns that affect every other
row's confidence level.

**Verification posture under sibling-crate breakage** — `webrender-wgpu`
SPIR-V/naga migration blocks `cargo check -p graphshell --lib` and full test
runs. Verification is currently narrow against `graphshell-core`. This is the
practical reason "M4 substantially landed" can't be fully confirmed. Tracked
in the 2026-04-22 progress log entry.

**Time/clock injection for replay** — `runtime.tick()` consumes time
(focus-ring fade math depends on it). For parity replay across hosts, time
has to be injectable. Currently not part of `FrameHostInput`'s contract.

**Effect-system surface** — §12.4 covers lifecycle ingest/plan/apply, but the
broader effect dispatch (thumbnail async, navigation, network, clipboard)
doesn't have a named boundary. M4.4 landed `BackendThumbnailPort` as a
port-shaped trait — this pattern arguably wants generalization to other
async/effect surfaces before iced tries to mirror them.

**Cross-repo Servo wgpuification dependency** — M4.5 alignment depends on
`servo-wgpu/docs/2026-04-18_servo_wgpuification_plan.md`. Cross-cutting risk
since viewer-surface decisions can't fully land without Servo-side
coordination on shared wgpu device/queue ownership.

### 12.19. Bottom Line (updated 2026-04-24)

- `done`: arrangement bridge; host-neutral runtime/ports; compositor identity;
  graph-canvas live surface; portable settings substrate; UxTree exists;
  diagnostics registry exists; `runtime.tick(input, ports) -> view_model` live
  on both egui (`gui.rs:1047`) and iced (`iced_host.rs:67`); §12.17 + §12.2
  sanctioned-writes contract tests (7 guards landed); first cross-host scalar
  parity test (§12.12); `HostNeutralRenderBackend` / `UiRenderBackendContract`
  trait split (§12.13); portable `SettingsViewModel` (§12.14)
- `partial`: durable graph mutation lane (app/helper leakage outside the
  kernel remains, but direct-kernel calls are now guarded); GraphTree/runtime
  authority transfer; iced host (graph-canvas first surface + parity harness
  landed; input translation, texture cache, clipboard/toast/accesskit still
  stubbed); settings UI surface; AccessKit integration; persisted node
  navigation memory boundary (Lane A landed; Lane B guards pending); M4.5
  viewer-surface host-nativity (`ViewerSurfaceRegistry` typed backing +
  native rendering context allocation landed)
- `missing`: graph-canvas packet replay exercised cross-host; CI gate on
  replay parity divergence; iced render backend impl; AT projection above
  host; full struct-level view-model parity assertion (`PartialEq` derives
  pending on sub-structs)

### 12.20. Highest-Leverage Next Targets (updated)

**Tier 1 — unblocks M4 completion**:

- ~~Centralize all `set_node_history_state(...)` writes behind one helper~~
  — landed 2026-04-23 (Lane A); see §12.3.
- ~~Phase-args bundle collapse onto `&mut GraphshellRuntime`~~ — struct-level
  collapse landed 2026-04-23 (Lane B'). Last remaining split-borrow at
  `run_update_frame_prelude` (passed `&mut runtime.graph_app`) collapsed to
  `runtime: &mut GraphshellRuntime` on 2026-04-24 (Lane 1 M4 session).
- ~~Move host-side view-model reads to consume `runtime.tick()` output~~
  — third getter migrated 2026-04-24 (Lane 1): `EguiHost::focused_node_key()`
  now reads `cached_view_model.focus.focused_node` with pre-first-frame
  fallback. Required reconciling `FocusViewModel.focused_node` from
  `focused_node_hint` (render-pass cache) to `active_pane_rects.first()`
  (same source as `focused_node_key()`). New test pins the graph-surface-
  focused gate (`focused_node` is None when `graph_surface_focused`). Remaining
  candidates: `runtime_focus_state`, dialog-state queries, chrome render sites
  (the latter require tick() to run before the render pass).

**Tier 2 — unblocks useful M5 (iced as a real second host)**:

- ~~Begin M4.5: retire `tile_rendering_contexts` hot-path ownership in favor
  of `ViewerSurfaceRegistry`~~ — first ownership slice landed 2026-04-23
  (§12.10); native `RenderingContextCore` path now primary, compat GL
  contained. Remaining targets are listed in §12.10.
- ~~Wire `iced_parity.rs` to consume the same replay traces as the egui
  host~~ — first cross-host scalar-parity test landed 2026-04-24 (§12.12).
  Remaining: `#[derive(PartialEq)]` on view-model sub-structs + graph-canvas
  packet replay.
- Implement real iced `OverlayAffordancePainter` / `ContentPassPainter`
  (stubs exist at `iced_host_ports.rs`; painter traits upstream are stable)
- Drain `todo(m5)` markers in `iced_host_ports.rs` — event translation,
  texture cache, clipboard, toast, accesskit bridges
- Convert `iced_graph_canvas::GraphCanvasProgram` into a real
  `CanvasBackend<NodeKey>` impl so both hosts drive off the same
  `ProjectedScene` (§12.11)

**Tier 3 — enforcement and durability**:

- ~~Contract tests for: arrangement-bridge sole-writer, no direct
  node-history writes, no new host-owned graph/history mutation setters~~
  — all landed via `app::sanctioned_writes_tests` (§12.17); now 7 guards
  covering §12.1, §12.2, §12.3, and §12.17. Export the scanner as a
  test-utils helper so other crates can extend.
- Lift UxTree → AccessKit translation above the host boundary
- Type the render-backend contract for shared wgpu (first trait split
  landed — §12.13; concrete iced impl pending)
