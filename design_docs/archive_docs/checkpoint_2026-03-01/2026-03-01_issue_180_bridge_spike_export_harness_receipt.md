# Issue #180 Receipt — Bridge Spike Export Harness

**Date**: 2026-03-01  
**Issue**: `#180`  
**Domain**: Runtime-viewer bridge precondition for `egui_glow -> egui_wgpu`

## Purpose

Provide a minimal executable harness artifact for #180 by exporting bridge-spike measurements into a dedicated JSON file from the diagnostics pane.

## Scope Implemented

1. Added a dedicated export action in diagnostics UI:
   - `Save Bridge Spike JSON`
2. Added exporter method:
   - `export_bridge_spike_json()`
   - output filename: `bridge-spike-<unix-secs>.json`
3. Added bridge-spike measurement payload builder with contract fields:
   - `bridge_path_used`
   - `sample_count`
   - `failed_frame_count`
   - `avg_callback_us`
   - `avg_presentation_us`
   - latest sample (`bridge_path`, `tile_rect_px`, `render_size_px`, callback/presentation/duration)
4. Included per-sample records to preserve measured evidence history in each export.
5. Added focused unit test proving payload contains required measurement contract fields.

## Validation

- `cargo test -q --lib bridge_spike_measurement_payload_contains_contract_fields`
- `cargo test -q --lib replay_export_feedback_includes_path_and_counts`
- `cargo test -q --lib replay_ring_is_bounded_to_capacity`
- `cargo check -q`

## Notes

- This slice remains additive and backend-neutral.
- It does not perform the `egui_glow -> egui_wgpu` swap; it enables #180 evidence capture needed before unblocking #183.