# Issue #180 Receipt — Bridge Diagnostics Wishlist (Implemented)

**Date**: 2026-03-01  
**Issue**: `#180`

## Purpose

Add deeper bridge-spike diagnostics so failed runs explain *why* they failed and whether chaos/restore conditions were involved.

## Added Diagnostics

Per replay sample (`CompositorReplaySample`):

- `chaos_enabled`
- `restore_verified`
- `viewport_changed`
- `scissor_changed`
- `blend_changed`
- `active_texture_changed`
- `framebuffer_binding_changed`

Bridge spike export aggregate (`measurement_contract`):

- `chaos_enabled_sample_count`
- `restore_verification_fail_count`
- `failed_by_reason` map:
  - `viewport`
  - `scissor`
  - `blend`
  - `active_texture`
  - `framebuffer_binding`

Per-sample JSON entries now include `failure_flags` and run-condition markers.

## Validation

- `cargo test -q --lib bridge_spike_measurement_payload_contains_contract_fields`
- `cargo test -q --lib replay_export_feedback_includes_path_and_counts`
- `cargo test -q --lib replay_ring_is_bounded_to_capacity`
- `cargo check -q`

## Outcome

Future bridge-spike captures can distinguish:

- whether failures correlate to specific GL invariants,
- whether chaos was active,
- and whether state restoration verification failed.