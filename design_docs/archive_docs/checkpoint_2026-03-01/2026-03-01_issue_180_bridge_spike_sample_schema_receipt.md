# Issue #180 Receipt — Bridge Spike Sample Schema

**Date**: 2026-03-01  
**Issue**: `#180`  
**Domain**: Runtime-viewer bridge precondition for `egui_glow -> egui_wgpu`

## Purpose

Capture the next bridge-spike harness slice by recording concrete path/geometry/timing fields in compositor replay samples for measurement export.

## Scope Implemented

1. Extended `CompositorReplaySample` with bridge-spike measurement fields:
   - `bridge_path`
   - `tile_rect_px`
   - `render_size_px`
   - `callback_us`
   - `presentation_us`
2. Added `BridgeProbeContext` and wired it from `register_render_to_parent_content_pass(...)` into `run_content_callback_with_guardrails(...)`.
3. Bound current bridge-path label to `gl.render_to_parent_callback`.
4. Included new sample fields in diagnostics JSON replay export (`compositor_replay_samples`).
5. Updated replay-sample fixture coverage in diagnostics/compositor tests.

## Validation

- `cargo test -q --lib replay_export_feedback_includes_path_and_counts`
- `cargo test -q --lib replay_ring_is_bounded_to_capacity`
- `cargo test -q --lib snapshot_json_includes_compositor_replay_samples_section`
- `cargo check -q`

## Notes

- This remains additive and backend-neutral; no `egui_glow` swap occurs in this slice.
- These fields are intended to support the measurement contract in follow-up #180 evidence receipts.