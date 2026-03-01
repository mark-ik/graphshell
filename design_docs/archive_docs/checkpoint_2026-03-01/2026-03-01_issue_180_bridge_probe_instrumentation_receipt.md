# Issue #180 Receipt — Compositor Bridge-Probe Instrumentation

**Date**: 2026-03-01  
**Issue**: `#180`  
**Domain**: Runtime-viewer bridge precondition for `egui_glow -> egui_wgpu`

## Purpose

Land additive diagnostics instrumentation that captures bridge-readiness metrics without changing the active renderer backend.

## Scope Implemented

1. Added new diagnostics channels:
   - `diagnostics.compositor_bridge_probe`
   - `diagnostics.compositor_bridge_probe.failed_frame`
   - `diagnostics.compositor_bridge_probe.callback_us_sample`
   - `diagnostics.compositor_bridge_probe.presentation_us_sample`
2. Emitted bridge probe and sample events from guarded compositor callback path in `shell/desktop/workbench/compositor_adapter.rs`.
3. Extended diagnostics replay summary and compositor diagnostics pane with:
   - `bridge_probe_count`
   - `bridge_failed_frame_count`
   - `avg_bridge_callback_us`
   - `avg_bridge_presentation_us`
4. Registered channels in diagnostics registry with explicit severities.
5. Added focused tests for bridge probe channel aggregation and snapshot summary values.

## Validation

- `cargo check -q`
- `cargo test -q --lib replay_channels_emit_for_sample_and_violation_artifact`
- `cargo test -q --lib snapshot_json_includes_compositor_bridge_probe_summary_metrics`

## Notes

- This slice is backend-neutral and does not replace `egui_glow`.
- `#183` remains blocked pending completion of #180 bridge viability evidence collection.