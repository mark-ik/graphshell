# Issue #171 Receipt — Compositor Chaos Mode for GL Isolation Invariants

**Date**: 2026-03-01  
**Issue**: `#171`  
**Domain**: Compositor callback isolation (`lane:embedder-debt`)  
**Owner boundary**: Graphshell compositor adapter and diagnostics authority

## Contract Summary

Add a diagnostics-gated chaos mode that injects bounded GL state perturbations between content and overlay pass boundaries, verifies state restoration guarantees, and records per-probe pass/fail diagnostics.

## Implementation Evidence

### 1) Diagnostics-gated chaos probe mode

- File: `shell/desktop/workbench/compositor_adapter.rs`
- Added runtime gate `GRAPHSHELL_DIAGNOSTICS_COMPOSITOR_CHAOS` (truthy: `1|true|yes|on`).
- Chaos path mutates 1–3 GL state fields deterministically per probe seed across:
  - viewport
  - scissor enable
  - blend enable
  - active texture unit
  - framebuffer binding

### 2) Restore verification in guarded callback path

- File: `shell/desktop/workbench/compositor_adapter.rs`
- Added `run_guarded_callback_with_snapshots_and_perturbation(...)`:
  - captures `before`
  - runs content callback
  - applies optional perturbation
  - captures `after`
  - restores `before` on violation
  - re-captures and verifies restore correctness (`restore_verified`)

### 3) Pass/fail diagnostics channels

- Files:
  - `shell/desktop/runtime/registries/mod.rs`
  - `registries/atomic/diagnostics.rs`
  - `shell/desktop/workbench/compositor_adapter.rs`
- Added channels:
  - `diagnostics.compositor_chaos`
  - `diagnostics.compositor_chaos.pass`
  - `diagnostics.compositor_chaos.fail`
- Severity registration:
  - base and pass: `Info`
  - fail: `Error`

### 4) Focused invariant tests

- File: `shell/desktop/workbench/compositor_adapter.rs`
- Added targeted tests for perturbation detection and restore verification of each invariant:
  - viewport
  - scissor
  - blend
  - active texture
  - framebuffer binding
- Added parser and pass/fail diagnostics helper tests.

## Verification Commands

- `cargo test guarded_callback_perturbation_detects_viewport_invariant -- --nocapture`
- `cargo test guarded_callback_perturbation_detects_scissor_invariant -- --nocapture`
- `cargo test guarded_callback_perturbation_detects_blend_invariant -- --nocapture`
- `cargo test guarded_callback_perturbation_detects_active_texture_invariant -- --nocapture`
- `cargo test guarded_callback_perturbation_detects_framebuffer_binding_invariant -- --nocapture`
- `cargo test chaos_probe_outcome_emits_channels -- --nocapture`
- `cargo check`

## Done-Gate Mapping

- Feature-gated chaos mode added and bounded: ✅
- Restore guarantees verified in guard path: ✅
- Focused invariant tests for required fields: ✅
- Diagnostics pass/fail channels registered and emitted: ✅
