# Rendering Pipeline Status-Quo Plan

**Date**: 2026-04-12
**Status**: Active execution anchor
**Scope**: Reality-based rendering-pipeline status and execution order, verified against the current codebase rather than aspirational plan text.

**Related**:

- `ASPECT_RENDER.md` - Render aspect policy authority
- `gl_to_wgpu_plan.md` - narrower compositor migration strategy; useful history, not the best current-state summary
- `render_backend_contract_spec.md` - backend-boundary contract
- `2026-03-12_compositor_expansion_plan.md` - semantic composition opportunities while the current compositor remains in service
- `../graph/GRAPH.md` - Graph domain note at the canvas surface
- `../graph/2026-04-11_graph_canvas_crate_plan.md` - future `graph-canvas` extraction plan
- `../../technical_architecture/graph_canvas_spec.md` - future `graph-canvas` API design

---

## 1. Purpose

This document records the rendering pipeline as it actually exists on April 12, 2026.

It is intended to replace stale "phase complete" assumptions with code-verified status and
to keep one execution order explicit:

1. finish compositor migration on the real hot path
2. optimize the existing graph renderer in place
3. extract `graph-canvas`
4. only then pursue backend replacement work

This doc is more concrete than `ASPECT_RENDER.md`, more current than the older compositor
phase plans, and less aspirational than the `graph-canvas` / Vello scene plans.

---

## 2. Current Pipeline

The live rendering stack is:

- `egui-wgpu` owns shell and chrome composition
- Servo web content renders through offscreen contexts and is composited per node pane
- shared-wgpu texture import exists and is attempted opportunistically for composited web content
- GL callback fallback and GL guardrail machinery are still active supported paths
- graph rendering still goes through `egui_graphs` plus retained `EguiGraphState`
- graph thumbnails still use PNG bytes and egui texture upload
- graph physics is still the CPU Barnes-Hut path
- `TwoPointFive`, `Isometric`, and `SceneMode` are real view/runtime state, not evidence of a new projected-scene world-render path

In practice this means Graphshell is already on `egui-wgpu` for the shell, but the
compositor is still mid-migration and the graph renderer is still the existing widget path.

---

## 3. Verified Status By Lane

Use the status labels in this document consistently:

- `landed`
- `partial`
- `scaffolded`
- `follow-on`

### 3.1 Web Compositor

- `ContentSurfaceHandle`: `partial`
  - The type exists and names the intended compositor-facing surface states.
  - The live compositor still relies on `tile_rendering_contexts: HashMap<NodeKey, Rc<OffscreenRenderingContext>>` on the hot path.
- Bridge redesign: `partial`
  - `BackendContentBridge` includes both shared-wgpu and callback variants.
  - The actual compose path still reaches shared-texture import directly through compositor code rather than a fully authoritative bridge abstraction.
- Invalidation split: `partial`
  - `tile_compositor` distinguishes content, placement, and semantic changes in its differential observation.
  - Real content generation is not yet driven by `ViewerSurfaceRegistry`; the signature still relies on `webview_id`, rect, and semantic generation.
- `ViewerSurfaceRegistry`: `scaffolded`
  - The registry exists and is stored on `Gui`.
  - The compositor/runtime hot path still routes through the older GL-context map rather than this registry as sole authority.
- NodeKey-centric adapter seam: `landed`
  - The compositor modules that matter are NodeKey-centric and no longer shape their main interfaces around `TileId`.
  - This does not mean the entire workbench has switched to `graph-tree` authority yet.
- GL guardrail retirement: `follow-on`
  - GL state isolation and chaos/restore diagnostics are still active.
  - `gl_compat` remains part of the default feature set.

### 3.2 Graph Renderer

- Current renderer: `landed`
  - The graph still renders through `egui_graphs` and `GraphView::<...>::new(&mut state.graph)`.
- Retained state and viewport culling: `landed`
  - `EguiGraphState` remains active retained graph state.
  - viewport culling and filtered visible-node submission are real current behavior.
- Thumbnail path: `landed`
  - Runtime screenshots are resized and encoded as PNG.
  - Thumbnails are later decoded and uploaded as egui textures.
- Physics: `landed`
  - The active path is CPU Barnes-Hut and single-threaded.
- Projected-scene / Vello path: `follow-on`
  - There is no active `ProjectedScene` packet seam in code.
  - There is no live Vello backend in the repo.

### 3.3 Portable Foundations

- `graphshell-core`: `landed`
  - The portable graph data model crate exists and is already in the workspace.
- `graph-tree`: `landed`
  - The portable tree/layout crate exists and computes layout.
  - It is not yet the sole render authority for the workbench runtime.
- `graph-canvas`: `follow-on`
  - The crate does not exist yet.
  - The current extraction target is still represented by docs and code seams, not by a landed crate.

---

## 4. Active Execution Priorities

This section is the current execution order.

### Priority 1: Finish the compositor migration

- move authoritative surface ownership to `ViewerSurfaceRegistry`
- retire direct hot-path reliance on `tile_rendering_contexts`
- wire real content generation into invalidation
- keep GL fallback explicit and contained rather than letting it continue to shape the primary architecture

### Priority 2: Improve the current graph renderer

- reduce thumbnail churn
- reduce unnecessary `egui_state` rebuilds
- tighten submission and LOD policy on the existing path
- profile physics before changing the architecture

### Priority 3: Extract `graph-canvas`

- derive portable packet, input, camera, and projection contracts from current code
- preserve current egui host behavior while extracting the seam
- treat `graphshell-core` as the portable graph-truth dependency and `graph-tree` as the sibling pane/layout subsystem

### Priority 4: Backend replacement follow-on

- Vello
- projected-scene rendering
- Rapier `Simulate`
- scripting
- zero-copy thumbnail ingestion

These are not prerequisites for current compositor stabilization or for the first
`graph-canvas` extraction slices.

---

## 5. Non-Goals

This document does not claim:

- that GL fallback is retired
- that `ViewerSurfaceRegistry` is already authoritative
- that `graph-canvas` already exists
- that Vello is active
- that Rapier or Wasmtime-style scene scripting is active
- that `TwoPointFive` or `Isometric` already have a new renderer

It is a status-quo document, not a future-state ratification.

---

## 6. Verification Notes

This plan was checked against the live rendering hotspots rather than relying on plan text alone.

The main evidence came from the current compositor, backend, graph-render, and thumbnail paths:

- `shell/desktop/workbench/compositor_adapter.rs`
- `shell/desktop/workbench/tile_compositor.rs`
- `shell/desktop/render_backend/*`
- `shell/desktop/ui/gui.rs`
- `render/mod.rs`
- `model/graph/egui_adapter.rs`
- `graph/layouts/barnes_hut_force_directed.rs`
- `shell/desktop/ui/thumbnail_pipeline.rs`

Those files show that several migration abstractions are present, but not yet authoritative,
while the graph path remains the active `egui_graphs` renderer with performance and extraction
work still ahead of it.

---

## 7. Relationship To Other Plans

- `ASPECT_RENDER.md` remains the policy authority for Render.
- `gl_to_wgpu_plan.md` remains useful as a narrower compositor migration/history document.
- `2026-04-11_graph_canvas_crate_plan.md` remains the future extraction plan for `graph-canvas`.
- This document supersedes both as the best answer to "what is true in the renderer right now?"

