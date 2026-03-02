# 2026-03-01 WebRender Readiness Gate + Feature Guardrails

## Purpose

Define a high-confidence gate for any future Glow → wgpu switch work rooted in WebRender evolution, while preserving current milestone velocity (UI/UX + workflow proof) on Glow.

This document is a **readiness contract**, not a runtime-switch authorization.

## Policy Decision

1. **Glow remains active runtime composition policy** for the current milestone.
2. **WebRender/wgpu work starts now as readiness work** (spikes, contracts, diagnostics parity, dependency wiring).
3. **Renderer switch is blocked** until all readiness gates in this document are closed with linked evidence.
4. **Feature work continues**, but must follow the guardrails below to prevent switch-cost inflation.

## Readiness Gates (must be closed before switch)

### G1 — Dependency control and reproducibility

- A local patch path is demonstrated and documented for WebRender/Servo integration.
- Build inputs are pinned (fork branch/rev or Cargo patch) and reproducible in CI or scripted local setup.
- Rollback path back to Glow baseline is documented and tested.

### G2 — Backend contract parity

- `render_backend` remains the sole ownership boundary for content-bridge mode selection.
- No direct new `render_to_parent` callback wiring is introduced outside backend-owned seams.
- Equivalent bridge-path/mode diagnostics are emitted for experimental WebRender/wgpu paths.

### G3 — Pass-contract safety

- No regressions in pass-order invariants (content pass before overlay affordance pass).
- No regressions in callback-state isolation guardrails and replay diagnostics.
- Non-interop capability fallback behavior is proven and observable.

### G4 — Platform confidence

- Target platform matrix for current milestone has explicit pass/fail state.
- Any platform-specific interop assumptions are documented with fallback behavior.
- Known gaps are tracked with issue links and tagged as switch blockers or non-blockers.

### G5 — Regression envelope

- Focused tests for bridge mode selection and fallback behavior are green.
- Replay snapshot schema remains stable (or change is documented with migration notes).
- No open stabilization bugs tied to render-path migration semantics.

## Feature Guardrails (effective immediately)

All new feature slices touching rendering/composition must satisfy:

1. **No new renderer-specific coupling in UI/workflow code**
   - Feature code in workbench/UI modules must consume backend contracts, not backend internals.

2. **Bridge metadata preservation**
   - If a slice touches content-pass wiring, it must preserve bridge-path and bridge-mode observability.

3. **Fallback-safe behavior**
   - New behavior must define what happens when preferred rendering capability is unavailable.

4. **Receipt-linked evidence**
   - Every migration-adjacent feature slice posts tracker evidence proving guardrail compliance.

## Execution Shape

### Track A — Milestone delivery (Glow active)

- Continue UI/UX and workflow-proof feature work on Glow.
- Require guardrail compliance for all render-adjacent slices.

### Track B — WebRender readiness

- Run bounded spikes for local patching and integration seams.
- Keep switch logic policy-gated and disabled by default.
- Build evidence required by G1–G5.

## Switch Authorization Rule

Runtime switch from Glow baseline to WebRender/wgpu path is permitted only when:

- G1–G5 are all closed,
- closure evidence is linked in tracker,
- and a dedicated switch receipt confirms rollback validation.

Until then, Glow remains the production composition path.

## Tracker linkage

- Backend migration tracker: `#183`.
- Related lanes: `#88`, `#99`, `#92`, `#90`.
