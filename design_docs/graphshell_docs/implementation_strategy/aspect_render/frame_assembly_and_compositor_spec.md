# Frame Assembly and Compositor — Interaction Spec

**Date**: 2026-02-28
**Status**: Canonical interaction contract
**Priority**: Active (Stages 1–4 complete; Stage 4b implemented)

**Related**:

- `ASPECT_RENDER.md`
- `../../../archive_docs/checkpoint_2026-03-22/graphshell_docs/implementation_strategy/aspect_render/2026-02-20_embedder_decomposition_plan.md`
- `../PLANNING_REGISTER.md` §0, §0.10
- `../workbench/workbench_layout_policy_spec.md` §3.2, §3.4
- `../viewer/viewer_presentation_and_fallback_spec.md`
- `../../TERMINOLOGY.md` — `CompositorAdapter`, `TileRenderMode`, `Composition Pass`, `Surface Composition Contract`

**Adopted standards** (see [2026-03-04_standards_alignment_report.md](../../research/2026-03-04_standards_alignment_report.md) §§3.6, 3.7)):
- **OpenTelemetry Semantic Conventions** — diagnostics channels (GL state violation, chaos mode, frame timing) follow OTel naming and severity conventions
- **OSGi R8** — `TileRenderMode` resolution and `CompositorAdapter` capability dispatch follow OSGi capability vocabulary

---

## 1. Scope

This spec defines the canonical contracts for:

1. **Composition pass ordering** — the three-pass frame model and ownership rules.
2. **TileRenderMode** — the render pipeline classification for each tile.
3. **CompositorAdapter** — lifecycle, GL state isolation, callback ordering.
4. **Embedder decomposition seams** — `EmbedderCore` vs `RunningAppState` boundary.
5. **Frame loop coordination** — begin/layout/paint ownership.

This spec covers the **current render architecture** and what is being actively built. It does not specify the deferred `egui_glow → egui_wgpu` migration (see `2026-02-27_egui_wgpu_custom_canvas_migration_strategy.md`).

Historical note: the earlier composited-viewer contract note has been archived. Any still-relevant future-work ideas from its Appendix A are tracked in `../PLANNING_REGISTER.md` §0.10 rather than in a separate active render-contract doc.

---

## 2. Composition Pass Model Contract

Every node viewer pane tile frame is composed in three ordered passes:

| Pass | Name | Owner | Content |
|------|------|-------|---------|
| 1 | UI Chrome Pass | Render aspect | Tab bar, badge overlay, focus/hover rings, tile chrome |
| 2 | Content Pass | Viewer (via CompositorAdapter) | Web content, native content, or fallback surface |
| 3 | Overlay Affordance Pass | Render aspect | Selection rings, diagnostic affordances, pointer hit targets |

**Invariant**: Pass ordering is Graphshell-owned sequencing and must not rely on incidental egui layer behavior. The three-pass order is always Chrome → Content → Overlay. No pass may mutate the output of a preceding pass.

**Invariant**: The `CompositedTexture` `TileRenderMode` renders Overlay Affordance Pass content over the composited texture in the pipeline; the `NativeOverlay` mode renders affordances in tile chrome/gutter regions because native content owns its own window region.

---

## 3. TileRenderMode Contract

`TileRenderMode` is the runtime-authoritative render pipeline classification for a node viewer pane tile. It is resolved from `ViewerRegistry` at viewer attachment time.

```text
TileRenderMode =
  | CompositedTexture   -- Servo GL texture composited into egui frame
  | NativeOverlay       -- native window overlay (e.g. Wry); owns its own region
  | EmbeddedEgui        -- egui-native content (native viewers, metadata card)
  | Placeholder         -- no viewer attached; fallback surface
```

**Invariant**: Every `NodePaneState` must have a `TileRenderMode` set at viewer attachment time. A tile must never enter the compositor dispatch without a resolved `TileRenderMode`.

**Resolution path**: `ViewerRegistry::resolve_mode(viewer_id) -> TileRenderMode`. The resolved value is stored on `NodePaneState` and updated only when the viewer changes.

### 3.1 Per-Mode Compositor Behavior

| Mode | Content Pass behavior | Overlay Pass behavior |
|------|----------------------|----------------------|
| `CompositedTexture` | Invoke `CompositorAdapter` GL callback; render Servo texture into tile rect | Render affordances over texture in tile rect |
| `NativeOverlay` | No GL callback; native content owns its region | Render affordances in chrome/gutter only (not over native content) |
| `EmbeddedEgui` | Render via normal egui widget tree | Render affordances as egui overlays |
| `Placeholder` | Render fallback surface (loading indicator, error state, empty state) | Render affordances over fallback |

### 3.2 Navigation Geometry Contract Note

Workbench layout policy may derive **visible navigation geometry** that differs
from the logical navigation-region remainder when overlay-form Navigator hosts
occlude part of the workbench surface.

Current runtime rule:

1. Runtime consumers that make visibility or placement decisions should honor the
   full visible navigation rect set rather than collapsing immediately to one
   fallback rect.
2. This includes viewport culling, diagnostics geometry summaries, and floating
   overlay placement.
3. Graph/input consumers follow the same rule via the runtime-carried typed
   visible-region contract.
4. The remaining follow-on work is promotion of visible navigation geometry into
   a first-class pane/render contract so the canonical render model itself no
   longer speaks in single-rect terms where a derived visible region set is the
   real authority.

See `workbench_layout_policy_spec.md` §3.4 for the authoritative definition of
logical navigation region versus visible navigation geometry.

---

## 4. CompositorAdapter Contract

`CompositorAdapter` wraps backend-specific content callbacks. It owns:

- Callback ordering within the Content Pass.
- GL state isolation (save/restore GL state around callbacks; the callback must not leak GL state into the egui render path).
- Clipping and viewport contracts (the callback renders only within the tile rect).
- The post-content overlay hook (called after content callback, before the Overlay Affordance Pass).

### 4.1 GL State Isolation Invariant

Every `CompositorAdapter` callback must:
1. Save GL state before invoking the content callback.
2. Restore GL state after the callback returns, regardless of whether the callback succeeded or panicked.
3. The egui render path must observe consistent GL state before and after a compositor callback.

**GL state diagnostics**: Violations (leaked GL state) must be observable via the diagnostics channel schema. A `GL_STATE_MISMATCH` channel event is emitted when state restoration fails.

### 4.2 Callback Registration

Content callbacks are registered at viewer attachment time. A callback is a function of type:

```
fn render_content(tile_rect: Rect, clip_rect: Rect, gl_state: &mut GlStateGuard)
```

Callbacks are unregistered at viewer detachment time. A tile with no registered callback falls back to `Placeholder` rendering.

---

## 5. Embedder Decomposition Seam Contract

The historical `RunningAppState` monolith conflated embedder and app-layer responsibilities. The decomposition boundary is:

| `EmbedderCore` (embedder responsibility) | `RunningAppState` (app-layer responsibility) |
|------------------------------------------|---------------------------------------------|
| Servo instance | `EmbedderCore` (owned) |
| Windows map (`HashMap<EmbedderWindowId, EmbedderWindow>`) | `AppPreferences` |
| Event loop waker | `Gui` handle |
| `WebViewDelegate` + `ServoDelegate` trait impls | Intent queues |
| WebDriver channels | Gamepad provider (app-level routing) |

**EmbedderCore invariant**: `EmbedderCore` must not hold references to graphshell app state (preferences, intent queues, graph data). It communicates via `GraphSemanticEvent` emissions only.

**`GraphSemanticEvent` is the clean boundary**: All semantic information crossing from the embedder layer into the graphshell graph/app layer must pass through `GraphSemanticEvent`. No direct embedder→graph calls.

### 5.1 Decomposition Stage Summary

| Stage | Status | Description |
|-------|--------|-------------|
| 1 | Complete | Semantic bridge extraction (`semantic_event_pipeline.rs`) |
| 2 | Complete | Toolbar decomposition (7 focused submodules) |
| 3 | Complete | `CompositorAdapter` extraction (wraps rendering paths `EmbedderCore` exposes) |
| 4a | ✅ Complete | `shell/desktop/ui/gui.rs` frame orchestration isolated from workbench layout driving |
| 4b | ✅ Complete | `EmbedderCore`/`RunningAppState` boundary closure plus host-runtime service extraction (`WebDriverRuntime`, `GamepadRuntime`, `EmbedderWindow` internal service splits) |

**Historical Stage 4 sequencing note**: Compositor pass-order correctness and GL-state diagnostics hardening landed before the Stage 4b decomposition follow-through. See `viewer/2026-02-26_composited_viewer_pass_contract.md` Appendix A for the sequencing rationale.

---

## 6. Frame Loop Coordination Contract

The egui frame loop coordinates three phases:

1. **Begin frame** — `ctx.begin_frame(input)`: owned by the Render aspect entry point.
2. **Layout pass** — widget tree construction and layout computation: owned by Workbench (tile tree traversal) + Viewer (per-tile content).
3. **Paint** — `ctx.end_frame()` + GPU surface present: owned by the Render aspect.

**Invariant**: Workbench must not call `ctx.begin_frame` or `ctx.end_frame`. It participates in the layout pass only. Frame start/end are Render aspect responsibilities.

**Invariant**: The compositor dispatch (CompositorAdapter callbacks) runs within the layout pass, after the tile rect is known and before `end_frame`. It does not run between begin/end frame boundaries.

---

## 7. Acceptance Criteria

| Criterion | Verification |
|-----------|-------------|
| Pass ordering is always Chrome → Content → Overlay | Test: instrument pass entry points → verify ordering across 100 frames |
| GL state is identical before and after a compositor callback | Test: capture GL state before/after callback → no differences |
| GL state mismatch emits diagnostics channel event | Test: inject a leaking callback → `GL_STATE_MISMATCH` event in diagnostics |
| Every `NodePaneState` has a resolved `TileRenderMode` | Test: attach viewer → `tile_render_mode` field is non-null |
| `NativeOverlay` affordances render in chrome/gutter only | Test: `NativeOverlay` mode → no affordance draw calls inside native content rect |
| `EmbedderCore` emits no direct graph mutations | Architecture invariant: no `graph_app.*` calls from `EmbedderCore` module |
| `GraphSemanticEvent` is the only crossing point | Architecture invariant: all embedder→app communication passes through `GraphSemanticEvent` |
| Compositor callback is unregistered on viewer detach | Test: detach viewer → callback list is empty; tile falls back to `Placeholder` |
