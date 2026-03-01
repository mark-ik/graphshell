# Issue #180 Receipt — Scissor Box Restoration Fix

**Date**: 2026-03-01  
**Issue**: `#180`

## Context

Bridge spike runs showed `failed_by_reason={"scissor":...}` while webviews intermittently appeared as black/hidden tiles, consistent with scissor-state leakage across compositor callbacks.

## Fix Implemented

In `shell/desktop/workbench/compositor_adapter.rs`:

1. Added explicit scissor-box capture/restore helpers:
   - `capture_scissor_box(...)`
   - `restore_scissor_box(...)`
2. In `run_content_callback_with_guardrails(...)`:
   - capture pre-callback scissor box,
   - detect post-callback scissor-box drift,
   - restore scissor box when drift is detected,
   - verify restoration outcome,
   - treat scissor-box drift as violation for replay/diagnostics reporting.
3. Updated violation emission path to include either base GL-state drift or scissor-box drift.

## Validation

- `cargo test -q --lib replay_channels_emit_for_sample_and_violation_artifact`
- `cargo test -q --lib bridge_spike_measurement_payload_contains_contract_fields`
- `cargo check -q`

## Expected Outcome

- Prevent callback-scope scissor-box leakage from clipping subsequent webview composition.
- Reduce or eliminate black-tile symptoms caused by stale scissor state.
- Keep #180 diagnostics attribution consistent with true failure source.