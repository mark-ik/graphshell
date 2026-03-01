# Issue #180 Baseline Receipt — GL to WGPU Bridge Spike Readiness

**Date**: 2026-03-01  
**Issue**: `#180`  
**Domain**: Runtime-viewer bridge precondition for `egui_glow -> egui_wgpu`

## Purpose

Capture the current baseline before implementing the GL->`wgpu` bridge spike. This establishes measurable starting facts and narrows the first executable spike surface.

## Baseline Findings

1. `egui_wgpu` is not yet present in dependencies.
   - Evidence: `Cargo.toml` currently includes `egui_glow` and no `egui_wgpu` entry.
2. Composited content callback path is GL-bound through Servo offscreen contexts.
   - Evidence hotspot: `shell/desktop/workbench/compositor_adapter.rs`
   - Key seam: `render_context.render_to_parent_callback()` in `register_content_pass_from_render_context(...)`.
3. Runtime viewer composition currently depends on `OffscreenRenderingContext` across workbench/UI lifecycle modules.
   - Hotspots include:
     - `shell/desktop/workbench/tile_compositor.rs`
     - `shell/desktop/workbench/tile_render_pass.rs`
     - `shell/desktop/workbench/tile_runtime.rs`
     - `shell/desktop/ui/gui.rs`
     - `shell/desktop/ui/gui_frame.rs`

## Immediate Spike Surface (Narrow)

Define the first spike as one composited content path only:

- Keep existing `egui_glow` mainline untouched.
- Add an isolated `wgpu`-spike module behind feature gate and/or test harness entry.
- Target one node-pane content surface and measure:
  - copy count
  - callback latency
  - frame-time delta
  - resize behavior
  - repeated tile-rect update behavior

## Measurement Contract (to satisfy #180 done gate)

For each test run, capture:

- bridge path used (copy/interop strategy)
- tile rect + render size
- end-to-end callback time (us)
- presentation time (us)
- dropped/failed frame count

## Next Execution Step

Implement a minimal bridge-spike harness that can run without replacing the active renderer backend, then record measured results in a follow-up receipt.

## Relationship to #183

`#183` remains blocked until this spike provides measured bridge viability evidence.
