<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Graph Canvas Overlays and Camera Re-lands Plan (2026-04-19)

**Status**: **Archived 2026-04-20** — every section landed. §2, §2.1
(Miro-style after Mark picked the convention), §3, §4 at the
graph-canvas layer + §4 host wiring (`canvas_bridge` + egui painter)
all shipped. Navigation-policy tuning that grew out of §2 was
lifted into its own plan:
[2026-04-20_navigation_policy_plan.md](2026-04-20_navigation_policy_plan.md) (sibling in this checkpoint).
**Scope**: Re-land the five orphaned-but-doable features identified during
the egui_graphs retirement work. These were pre-existing features that were
silently disabled when the graph-canvas live path landed and `egui_graphs`
was retired. This plan re-lands them on the `CanvasCamera` + `ProjectedScene`
seam so they work uniformly across egui and (future) iced hosts.

**Parent**:
[../../../archive_docs/checkpoint_2026-04-19/graphshell_docs/implementation_strategy/shell/2026-04-18_egui_graphs_retirement_plan.md](../../../archive_docs/checkpoint_2026-04-19/graphshell_docs/implementation_strategy/shell/2026-04-18_egui_graphs_retirement_plan.md) §5.6 (archived 2026-04-19).

**Standards bar**: Firefox-consistent infinite-canvas navigation (plain
wheel pans, `Ctrl/Cmd+wheel` zooms, middle-click pans); accessible via
keyboard where applicable; overlays emitted as packet draw items so all
hosts paint them the same way.

---

## 1. Feature inventory

Five plumbing-only features to re-land, each a thin translation on top of
already-landed infrastructure:

| Feature | Lands on | Approximate size |
|---|---|---|
| Background pan (primary drag) | `CanvasCamera::pan` + input translation | ~30 LOC |
| Plain wheel → pan | `canvas_bridge::collect_canvas_events` | ~20 LOC |
| `Ctrl/Cmd+wheel` → zoom (also trackpad pinch) | `CanvasCamera::zoom` via new event variant | ~40 LOC |
| Middle-click drag → free pan | `canvas_bridge::collect_canvas_events` + engine gesture | ~50 LOC |
| Pan inertia on release | New `CanvasCamera` sidecar state | ~80 LOC |
| Fit / FitSelection / FitGraphlet | `CanvasCamera::zoom` + `pan` target | ~120 LOC |
| Frame affinity backdrops | `ProjectedScene` overlay items | ~100 LOC |
| Scene region backdrops (Arrange/Simulate) | Same packet-overlay path | ~80 LOC |
| Highlighted edge overlay | Same packet-overlay path | ~60 LOC |

Total: ~580 LOC. No new dependencies. All additive on top of the landed
`graph-canvas::{camera, packet, engine}` modules and `canvas_bridge`.

---

## 2. Feature target 1 — Camera input re-lands

### 2.1 Background pan (primary drag)

When the user presses the primary mouse button on an empty area of the
graph (not a node, not a lasso gesture) and drags, pan the camera.

- Add `CanvasInputEvent::PointerDrag { delta: Vector2D<f32>, button: PointerButton, modifiers: Modifiers }`
  or reuse the existing `PointerMoved` + `down` state.
- `InteractionEngine` recognizes drag-on-background and emits
  `CanvasAction::PanCamera(delta)`. (The action already exists;
  `canvas_bridge::apply_pan` already applies it.)
- Host translation: in `canvas_bridge::collect_canvas_events`, emit the
  portable drag events when egui reports a pointer drag and no node is
  hovered.

### 2.2 Plain wheel → pan

Infinite-canvas convention: scrolling the wheel pans the view. See
[feedback_graph_canvas_navigation_defaults.md](../../../../../.claude/projects/c--Users-mark--Code/memory/feedback_graph_canvas_navigation_defaults.md).

- `CanvasInputEvent::Scroll` already carries `delta`, `position`, `modifiers`.
- Engine: when `modifiers.ctrl` is false, translate scroll delta to
  `CanvasAction::PanCamera`. When true, it stays as `ZoomCamera` (below).

### 2.3 `Ctrl/Cmd+wheel` → zoom (and trackpad pinch)

- Most trackpads synthesize `ctrlKey = true` on pinch gestures, so this
  binding covers both physical key+wheel and native pinch.
- The existing `CanvasAction::ZoomCamera { factor, focus }` already exists.
  Engine routes `Scroll` with `modifiers.ctrl` to it.
- Default zoom step: `1.1^(delta / 120.0)` — matches Figma / typical
  infinite-canvas feel.

### 2.4 Middle-click drag → free pan

- New or repurposed `PointerButton::Middle` handling.
- Engine recognizes middle-button-drag and always emits `PanCamera`,
  regardless of what's under the cursor (unlike primary drag which pans
  only on background).

### 2.5 Pan inertia on release

- When primary drag ends or middle-click drag ends with a non-trivial
  velocity, start a per-frame decay.
- New `CanvasCamera` sidecar: `pan_velocity: Vector2D<f32>`.
- Each frame: `pan += pan_velocity * dt; pan_velocity *= damping`. Stop
  when `|pan_velocity| < min`.
- Keep on desktop per memory — reinforces the spatial/physical feel of
  the force-directed graph. `damping` default 0.88.

### 2.6 Validation

- Drag on empty area: camera pans; nodes visually stay in world.
- Scroll with no modifier: camera pans vertically.
- Scroll with `Ctrl`: camera zooms centered on cursor.
- Middle-drag: free-pan regardless of hover target.
- Release after drag: camera continues briefly, decays smoothly.

---

## 3. Feature target 2 — Fit / FitSelection / FitGraphlet

### 3.1 What Fit is

Three commands that compute a bounding box of some node set and position
the camera to frame it:

- **Fit** — all visible nodes.
- **FitSelection** — currently selected nodes.
- **FitGraphlet** — members of the active graphlet projection.

Each is a one-shot camera move.

### 3.2 Implementation

Route the existing `CameraCommand` enum through a new `canvas_bridge` helper:

```rust
pub fn apply_fit_to_bounds(
    camera: &mut CanvasCamera,
    bounds: Rect<f32>,
    viewport: &CanvasViewport,
    padding_ratio: f32, // e.g. 1.08 for 8% margin
) -> Option<()>
```

Compute:

- `fit_zoom = min(viewport.w / bounds.w, viewport.h / bounds.h) / padding_ratio`
- `pan = viewport_center − bounds_center × fit_zoom`
- Clamp `fit_zoom` to the view's `camera.zoom_min / zoom_max`.

The three Fit flavors differ only in which bounds they pass in. Bounds
come from petgraph `node_projected_position` — no dependency on the
retired egui_state.

### 3.3 Validation

- Fit on an empty graph is a no-op.
- Fit with one node centers that node at the current zoom.
- FitSelection with a pin-locked camera respects the lock (no-op or emits
  the existing diagnostic channel).
- FitGraphlet falls back to Fit when no graphlet is active.

---

## 4. Feature target 3 — Overlay packet items

### 4.1 What overlays need

Three overlay kinds that today have zero presence on the live path:

- **Frame affinity backdrops** — one colored rounded rect per frame
  region, behind the nodes, labeled with the frame name.
- **Scene region backdrops** — one shape (rect or circle) per scene
  region, styled by effect kind (attractor, repulsor, dampener, wall).
- **Highlighted edge overlay** — when the user focuses an edge, a thicker
  stroke with optional tooltip endpoint markers.

### 4.2 Where they live

Graph-canvas's `ProjectedScene` already has a `overlays: Vec<SceneDrawItem>`
slot that gets painted between world and hit-proxy layers. Each overlay
kind becomes an `SceneDrawItem` emitted during `derive_scene`.

Concretely, extend the `derive` module:

```rust
// crates/graph-canvas/src/derive.rs
pub struct DeriveConfig {
    // ...existing fields...
    pub frame_regions: Vec<FrameAffinityBackdrop<N>>,
    pub scene_regions: Vec<SceneRegionBackdrop>,
    pub highlighted_edge: Option<HighlightedEdge<N>>,
}
```

Each backdrop carries: shape, fill color, stroke color, optional label.
`derive_scene` projects them through the camera and emits
`SceneDrawItem::{Rect, Circle, Text}` entries into `overlays`.

### 4.3 Host painting

`canvas_egui_painter::paint_projected_scene` already iterates
`scene.overlays` and paints each `SceneDrawItem`. Adding new draw-item
shapes requires the painter to cover them; if `Rect`, `Circle`, and `Text`
are already handled, this is zero host work.

### 4.4 Validation

- Frame backdrops appear behind nodes, not in front. Z-order via overlay
  layer index.
- Backdrops survive camera pan/zoom with no jitter (projected through
  `CanvasCamera`, not layered on top of the egui UI separately).
- iced host inherits backdrops for free when M5 lands, since they're
  emitted as packet items not egui painter calls.

---

## 5. Implementation sequence

1. Camera input re-lands (§2) — lowest risk; all five features share a
   shape. One PR.
2. Fit commands (§3) — one PR; builds on §2's `CanvasCamera` targeting.
3. Overlays (§4) — separate PR; touches `derive_scene` and potentially
   `canvas_egui_painter`. Each overlay kind (frame, scene, edge) can be
   its own commit.

Each step is self-contained and leaves the tree green.

---

## 6. Non-goals

- **Rebuilding the old MetadataFrame lane.** The old camera authority is
  retired; this plan strictly uses `CanvasCamera`.
- **Interactive region editing** — just rendering the regions.
  Interactive drawing/resizing of scene regions is a separate lane covered
  by [../graph/2026-04-03_physics_region_plan.md](../graph/2026-04-03_physics_region_plan.md).
- **Hover tooltips** — covered by the input/accessibility follow-on
  ([2026-04-19_graph_canvas_input_accessibility_followon_plan.md](2026-04-19_graph_canvas_input_accessibility_followon_plan.md)).

---

## 7. Progress

### 2026-04-19

- Plan created after egui_graphs retirement and Step 2–6 layout landings.
  Ready to execute; no design discussions pending.

- **Navigation-defaults slice landed** later the same day. Delivers the
  wheel/zoom/inertia behavior pinned in
  [feedback_graph_canvas_navigation_defaults.md](../../../../../.claude/projects/c--Users-mark--Code/memory/feedback_graph_canvas_navigation_defaults.md).
  Scope covered from this plan: §2.2 (plain wheel → pan), §2.3
  (`Ctrl`/`Cmd`+wheel → zoom, including trackpad pinch via synthesized
  `ctrlKey`), §2.4 (middle-click free pan — was already working in the
  engine's drag state machine), and §2.5 (pan inertia on release).

  Specific changes:
  - [crates/graph-canvas/src/camera.rs](../../../../crates/graph-canvas/src/camera.rs):
    added `CanvasCamera::pan_velocity: Vector2D<f32>` with
    `#[serde(default)]` for backward-compatible deserialize; new
    `tick_inertia(dt, damping_per_second)` method applies velocity, decays
    exponentially, snaps to zero at `PAN_VELOCITY_EPSILON`; exported
    `DEFAULT_PAN_DAMPING_PER_SECOND = 0.003` (≈500ms settle at 60fps).
  - [crates/graph-canvas/src/engine.rs](../../../../crates/graph-canvas/src/engine.rs):
    `Scroll` handler now branches on `modifiers.ctrl` — plain scroll
    emits `PanCamera` scaled by `scroll_pan_pixels_per_unit` (default
    50 px/unit), Ctrl-scroll continues to emit `ZoomCamera`.
    `DragState::Panning` gained a `last_world_delta` field; release
    emits a new `CanvasAction::SetPanInertia(velocity_per_second)` when
    `InteractionConfig::pan_inertia_enabled` (default `true`) and the
    terminal delta is above `PAN_VELOCITY_EPSILON`. Velocity estimate
    is `last_delta * 60.0` — the engine has no real frame clock, so
    assumes 60 Hz tick cadence, which is adequate for feel.
  - [crates/graph-canvas/src/interaction.rs](../../../../crates/graph-canvas/src/interaction.rs):
    new `CanvasAction::SetPanInertia(Vector2D<f32>)` variant.
  - [render/canvas_bridge.rs](../../../../render/canvas_bridge.rs):
    wires `SetPanInertia` into `camera.pan_velocity`; ticks
    `camera.tick_inertia(1/60, DEFAULT_PAN_DAMPING_PER_SECOND)` once
    per frame so idle frames keep decaying. Updated
    `run_graph_canvas_frame_updates_camera_from_scroll_events` to exercise
    both scroll legs (plain scroll → no zoom; Ctrl-scroll → zoom > 1.0).
    Picked up the new `pan_velocity` field in `camera_from_view_frame`
    (starts zero — inertia only accumulates from live drags, never
    persists across view-frame snapshots).

  Seven new tests across camera and engine cover: serde-back-compat for
  cameras without `pan_velocity`, `tick_inertia` applies-and-decays,
  tick snaps to zero under epsilon, plain scroll emits pan (never zoom),
  Ctrl-scroll emits zoom, pan-release seeds inertia when the feature is
  on, pan-release skips inertia when disabled.

- **Still pending in this plan**:

  - **§2.1 Background-pan vs lasso UX.** Today primary-drag on empty
    background starts a lasso (when `lasso_enabled`) — the plan wants
    it to pan. Resolving this needs a modifier convention (e.g.,
    `Shift`+drag for lasso) or a host-level toggle. Punted: the current
    defaults preserve existing lasso behavior and the plan's stated
    "background pan" is covered by middle-click and secondary-drag
    today, both of which already work.
  - **§3 Fit / FitSelection / FitGraphlet — LANDED** (2026-04-19).
    See the dedicated progress entry below.
  - **§4 Overlay backdrops.** `ProjectedScene.overlays` slot and the
    egui painter both exist; `derive_scene` doesn't populate
    `frame_regions`, `scene_regions`, or `highlighted_edge` yet.
    `DeriveConfig` needs the three new input slots and `derive_scene`
    needs to project their shapes through the camera. Not landed.

- **Receipts**: `cargo test -p graph-canvas --lib` camera+engine 24
  passed; `cargo test -p graph-canvas --features simulate --lib` 234
  passed; `cargo test -p graphshell --lib` 2144 passed / 0 failed / 3
  ignored; `cargo check --workspace --exclude servoshell --exclude
  webdriver_server` clean.

### 2026-04-19 (§3 Fit commands)

Second /loop iteration on this plan landed §3's three Fit variants:
`Fit`, `FitSelection`, `FitGraphlet` — the `CameraCommand` enum at
[app/graph_app_types.rs](../../../../app/graph_app_types.rs) is no
longer dormant, and `request_camera_command` emissions from keyboard
handlers (`graph_app.rs:1427/1435`) now actually move the camera.

**Portable layer** — `CanvasCamera::fit_to_bounds` added to
[crates/graph-canvas/src/camera.rs](../../../../crates/graph-canvas/src/camera.rs).
Pure math over a world-space `Rect`, a viewport, a padding ratio, and
zoom bounds. Fits `bounds.center()` to the viewport center, picks a
zoom that matches the tighter axis, clamps to `[zoom_min, zoom_max]`,
clears `pan_velocity` so inertia can't keep coasting past the target,
returns `false` on zero-area viewport so the host can leave the
pending request in place. Zero-area bounds use the `fallback_zoom`
argument and still center. Seven new tests cover:
- centering on bounds.center
- padding ratio reduces zoom
- zoom clamped to max when bounds are tiny
- zero-area bounds use fallback + center
- zero-area viewport → no-op
- inertia cleared on fit

**Host layer** — `apply_fit_camera_command` + `bounds_of_nodes`
helpers added to [render/canvas_bridge.rs](../../../../render/canvas_bridge.rs).
Computes per-variant bounds against the current `CanvasSceneInput`:
- `Fit` → bounds of every node in scene, expanded by radius.
- `FitSelection` → bounds of `app.focused_selection()`; falls back to
  `Fit` when selection is empty.
- `FitGraphlet` → bounds of `view.graphlet_node_mask`; falls back to
  `FitSelection` → `Fit` when mask is missing or empty.
- `SetZoom(factor)` → snaps zoom into the same `[0.1, 10.0]` range as
  drag-zoom; preserves pan; clears `pan_velocity`.

The dispatcher runs once per `run_graph_canvas_frame` after the
inertia tick (so a Fit always overrides residual coast) and only when
`pending_camera_command_target() == view_id`. The pending command is
cleared once consumed — including when the scene is empty, so a
Fit-on-empty-graph doesn't busy-loop. Fit padding ratio is `1.08`
(~4 % margin per edge), matching the retired egui_graphs feel.

**Host-layer tests** — five targeted tests in
`render::canvas_bridge::scene_input_tests`:
- `run_graph_canvas_frame_consumes_pending_fit_over_populated_graph`
  — populated scene + `Fit` clears the command and moves the zoom
  away from identity.
- `run_graph_canvas_frame_consumes_pending_fit_on_empty_graph` —
  empty scene still clears the pending command (no busy-loop).
- `run_graph_canvas_frame_fit_selection_frames_only_selected_nodes` —
  two nodes far apart, only one selected; `FitSelection` pans to the
  selected node's neighborhood, not the midpoint.
- `run_graph_canvas_frame_fit_selection_falls_back_to_fit_when_no_selection`
  — empty selection falls through to `Fit`; zoom moves away from
  identity on a populated graph.
- `run_graph_canvas_frame_set_zoom_clamps_and_clears_pending` — a
  `SetZoom(50.0)` request clamps to the `[0.1, 10.0]` range and
  clears the pending command.

**Receipts**:
- `cargo test -p graph-canvas --lib camera::` — 13/13 pass (was 6/6
  before §3; 7 new `fit_to_bounds_*` tests added).
- `cargo test -p graphshell --lib render::canvas_bridge` — 16/16 pass
  (was 11/11 before §3; 5 new Fit dispatch tests added).
- `cargo test -p graphshell --lib` — 2149/2149 pass (up from 2144, no
  regressions).
- `cargo check -p graphshell --lib` clean.

**Still deferred from this plan**:
- §2.1 background-pan-vs-lasso UX (needs a modifier convention
  discussion before landing).
- §4 overlay backdrops — next /loop iteration on this plan will
  tackle them: add `DeriveConfig` slots for `frame_regions`,
  `scene_regions`, `highlighted_edge`; populate `ProjectedScene.overlays`
  in `derive_scene`; verify egui painter covers the shape set.

### 2026-04-19 (§4 Overlay backdrops)

Third /loop iteration on this plan landed §4. `ProjectedScene.background`
and `ProjectedScene.overlays` layers are now populated with frame-region
discs, scene-region shapes, and a highlighted-edge stroke respectively
— previously both layers were empty on the live graph path, and all
three overlay kinds had zero presence.

Minor departures from the plan text: the richer inputs live alongside
`DeriveConfig` rather than inside it. `DeriveConfig` is non-generic
config (LOD, colors, projection tuning) and the per-frame overlay
inputs need the host's `N` node-id type. Keeping them separate avoids
making `DeriveConfig` generic and keeps the shared config cheap to
reuse across frames.

**New types**, added to
[crates/graph-canvas/src/derive.rs](../../../../crates/graph-canvas/src/derive.rs):

- `OverlayInputs<'a, N>` — borrowed slices for per-frame hints:
  `frame_regions: &'a [FrameRegion<N>]`, `scene_regions: &'a [SceneRegion]`,
  `highlighted_edge: Option<(N, N)>`. `Default` yields empty slices /
  `None` so hosts that don't care about any overlay kind pay nothing.
- `OverlayStyle` — visual tuning (fills, strokes, padding, label font
  size) that hosts can theme without reimplementing the emitter.
  Defaults match the feel of the retired egui_graphs backdrops.

**New entry point**, `derive_scene_with_overlays`. Takes the usual
derive arguments plus `&OverlayInputs<'_, N>` + `&OverlayStyle` and
emits:

- Frame-affinity backdrops → `ProjectedScene.background`
  (enclosing disc = member centroid + max-distance radius +
  `frame_region_padding`, projected to screen via `camera.world_to_screen`
  and scaled by `camera.zoom`). Regions with no members in the current
  scene are dropped — no empty backdrops.
- Scene-region backdrops → `ProjectedScene.background` (`Circle` or
  `RoundedRect`, colored by effect kind, label when present). Regions
  with `visible: false` are skipped. All geometry projected through
  the camera so backdrops pan and zoom with the scene.
- Highlighted edge → `ProjectedScene.overlays` (one `Line` stroke at
  `highlighted_edge_width`, colored by `highlighted_edge_color`). No-op
  when either endpoint is missing from the scene — stale highlights
  from removed nodes are dropped silently.

The existing `derive_scene` becomes a thin wrapper around
`derive_scene_with_overlays` with `OverlayInputs::default()` and
`OverlayStyle::default()`. Every existing call site continues to work
unchanged; the host opts in to overlays by switching to the richer
entry point.

**Layering rationale.** The plan's §4.2 text said overlays emit to
`ProjectedScene.overlays`, but frame and scene backdrops visually
belong *behind* nodes. Emitting them to `background` (which paints
before `world`) matches the visual intent while still flowing through
the same draw-item pipeline the painter already handles. The
highlighted edge stays on `overlays` so it sits atop regular edges
for emphasis.

**Tests** — seven new tests in `derive::tests`:
- `derive_scene_with_overlays_empty_matches_derive_scene` — empty
  overlay inputs produce identical output to the legacy entry point.
- `derive_scene_emits_frame_region_backdrop_in_background_layer` —
  one region with three members → one disc on the background layer.
- `derive_scene_skips_frame_region_with_no_members_in_scene` —
  members referencing missing ids emit nothing.
- `derive_scene_emits_scene_region_backdrop_circle_and_rect` — mixed
  shapes plus a hidden region: one circle + one rect + one label
  (label only on the labeled region), hidden region skipped.
- `derive_scene_emits_highlighted_edge_on_overlay_layer` — Some pair
  → one line on the overlay layer.
- `derive_scene_skips_highlighted_edge_when_endpoints_missing` —
  dangling id → no line, no panic.
- `derive_scene_projects_backdrops_through_camera` — world radius 50
  at zoom 2.0 produces screen radius 100, confirming the projection
  math actually runs in the emitters.

**Still pending for §4 host wiring**:

- `render/canvas_bridge.rs` does not yet compute the overlay inputs
  from the app's `ArrangementRelation(FrameMember)` edges, scene
  regions registry, and hovered-edge state; it still calls the legacy
  `derive_scene`. The pipe is ready — hosts just need to pick up
  `derive_scene_with_overlays` and wire their inputs. Deferred here
  because the app-side region store is being shaped by a concurrent
  plan; the portable derivation layer is the load-bearing gap and
  that's what landed.
- `canvas_egui_painter::paint_projected_scene` already iterates
  `scene.overlays` per the Explore agent's pre-land survey. It does
  not currently iterate `scene.background` — host integration needs
  to extend the painter to paint the background layer before world
  items. Trivial (mirror the overlays loop, pre-world), but
  best-grouped with the host wiring pass above.

**Receipts**:
- `cargo test -p graph-canvas --lib derive::tests` — 21/21 pass (was
  14/14 before §4; 7 new overlay tests).
- `cargo test -p graph-canvas --features simulate --lib` — 247/247
  pass (up from 234 before §4).
- `cargo check --workspace --exclude servoshell --exclude
  webdriver_server` clean.

**Plan scope complete** at the graph-canvas layer. §2 / §3 / §4 all
landed; the only residual work is §2.1 (UX discussion) and the host-
side wiring above (separate concern).

### 2026-04-19 (§2.1 background-pan-vs-lasso UX — Miro-style)

Decision: **Miro/tldraw-style** — plain primary-drag on empty
background pans the camera; `Shift`+primary-drag on empty background
starts a lasso marquee. Matches the infinite-canvas framing pinned in
[feedback_graph_canvas_navigation_defaults.md](../../../../../.claude/projects/c--Users-mark--Code/memory/feedback_graph_canvas_navigation_defaults.md)
("wheel=pan, middle=pan, treat graph canvas as infinite-canvas doc
not webpage") and keeps `Shift` as the universal multi-select modifier
consistent with `Ctrl`-click-toggle on nodes. Right-click stays
unbound at the engine level — reserved for the context-menu lane the
radial-menu plan wants later.

**Implementation**, in
[crates/graph-canvas/src/engine.rs](../../../../crates/graph-canvas/src/engine.rs):

- `DragState::Pending` gained a `modifiers: Modifiers` field, captured
  at `PointerPressed` time. The threshold-crossing handler in
  `handle_pointer_move` reads it when deciding which gesture to start.
- The `(HitTestResult::None, PointerButton::Primary)` arm now splits
  into two sub-arms: with `modifiers.shift && lasso_enabled` it
  launches the lasso (the previous behavior, now gated on Shift);
  without Shift it falls through to the same pan path that middle-
  drag and secondary-drag already use.
- Middle-drag and secondary-drag on background still pan (unchanged).
  Node-drag on a node-hit target still drags the node (unchanged).

**Tests** — one existing engine test replaced with two clearer ones:

- `plain_primary_drag_on_background_pans` (new): press+move without
  Shift emits `PanCamera` and never `LassoBegin`; `state.lasso` stays
  `None`.
- `shift_primary_drag_on_background_lassos` (replaces the former
  `lasso_on_background_drag`): press+move with Shift emits
  `LassoBegin`, then release emits `LassoComplete` — same shape as
  the old test, just gated on the modifier.

**Receipts**:

- `cargo test -p graph-canvas --lib engine::` — 18/18 pass (was 16/16
  pre-§2.1; two lasso-on-background tests replaced by the two
  clearer pairings above).
- `cargo test -p graph-canvas --features simulate --lib` — 248/248
  pass.
- `cargo test -p graphshell --lib` — 2149/2149 pass (no host-side
  regressions; the change is purely inside the engine's gesture
  router).

**Plan closed** — §2 / §2.1 / §3 / §4 all landed at the graph-canvas
layer. Host-side wiring for §4 backdrops (compute overlay inputs in
`render/canvas_bridge.rs`, extend the egui painter to paint
`scene.background`) remains as the single follow-on item and is
tracked inline in the §4 entry.

### 2026-04-19 (§4 host wiring — partial, blocked on upstream)

After stopping, Mark asked whether the host-side wiring was worth
doing given the iced migration; I argued yes for the overlay inputs
(host-neutral, iced will reuse them) and yes-and-cheap for the egui
painter (3-line mirror of the overlays loop). Landed:

- **Painter was already done.** The egui
  [canvas_egui_painter::paint_projected_scene](../../../../render/canvas_egui_painter.rs)
  already iterates `scene.background` before `scene.world` (line 27).
  The earlier §4 entry's claim that the painter needed extending was
  based on a stale Explore-agent snapshot. No edit was required.
- **Host-side overlay input builders** added in
  [render/canvas_bridge.rs](../../../../render/canvas_bridge.rs):
  - `build_portable_frame_regions(app)` calls the existing
    `crate::graph::frame_affinity::derive_frame_affinity_regions` and
    maps each `FrameAffinityRegion` to a portable
    `graph_canvas::layout::extras::FrameRegion<NodeKey>`. Rendered
    frames now track the same membership the physics pass uses.
  - `build_portable_scene_regions(app, view_id)` walks
    `app.graph_view_scene_runtime(view_id)?.regions` and converts each
    app-side `SceneRegionRuntime` (egui types, `uuid::Uuid` ids) into
    a portable `graph_canvas::scene_region::SceneRegion` (euclid types,
    `u64` ids). Shape, effect, label, and visibility round-trip 1:1.
- **ID bridging helper** added on the app-side
  [graph/scene_runtime.rs](../../../../graph/scene_runtime.rs):
  `SceneRegionId::as_u64_low()` returns the lower 64 bits of the
  underlying UUID as a stable `u64`. That's the projection the
  portable `SceneRegionId(pub u64)` needs. Collision risk is
  negligible at the scale of per-view region sets.
- **`run_graph_canvas_frame` switched** from `derive_scene` to
  `derive_scene_with_overlays`. Overlay inputs are built once per
  frame (highlighted edge comes straight from
  `app.workspace.graph_runtime.highlighted_graph_edge: Option<(NodeKey, NodeKey)>`).
  Default `OverlayStyle` is used — hosts can theme later by passing
  their own.

**Upstream blocker** — `cargo build -p graphshell --lib` currently
fails because the path-dep `../webrender-wgpu/webrender` has a
`webrender_build/lib.rs` that calls `shaderc::Compiler::new().ok_or_else(...)`
and `shaderc::CompileOptions::new().ok_or_else(...)`. In the current
shaderc release those constructors return `Option`, not `Result`, so
the method doesn't exist. This is Mark's in-flight SPIR-V shader
pipeline work (see `webrender-wgpu/wr-wgpu-notes/2026-04-18_spirv_shader_pipeline_plan.md`),
unrelated to the overlays work here. `graph-canvas` itself builds
cleanly (`cargo check -p graph-canvas --lib` and `cargo test
-p graph-canvas --features simulate --lib` both green, 248/248), and
`cargo check -p graphshell --lib` emits **zero** errors in any
graphshell-side source — every reported error is in
`webrender_build`. Running the full graphshell test suite has to
wait until the upstream path-dep resolves; the host-wiring changes
are in place and syntactically clean pending that verification.

**Receipts (partial)**:
- `cargo check -p graph-canvas --lib` — clean.
- `cargo test -p graph-canvas --features simulate --lib` — 248/248.
- `cargo check -p graphshell --lib` — zero graphshell-side errors;
  build blocked by webrender_build (pre-existing, unrelated).
- Full `cargo test -p graphshell --lib` — **deferred** until the
  webrender_build/shaderc API mismatch upstream is resolved.
