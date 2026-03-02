# WebView Lifecycle and Crash Recovery Spec

**Date**: 2026-03-01  
**Status**: Canonical interaction contract  
**Priority**: Immediate implementation guidance

**Related**:
- `viewer_presentation_and_fallback_spec.md`
- `universal_content_model_spec.md`
- `wry_integration_spec.md`
- `../aspect_render/frame_assembly_and_compositor_spec.md`
- `../subsystem_ux_semantics/ux_scenario_and_harness_spec.md`

---

## 1. Purpose and Scope

This spec defines runtime lifecycle contracts for webview-backed node panes.

It governs:

- creation/attach/detach/destroy flow,
- crash handling and recovery behavior,
- pane fallback behavior after crash,
- diagnostics and test contracts.

---

## 2. Lifecycle Stages

Canonical stage sequence:

1. `Requested` — open/activate requires webview.
2. `Creating` — runtime allocates webview and binds node.
3. `Attached` — webview mapped to node pane runtime.
4. `Ready` — first stable render/navigation event observed.
5. `Detached` — webview unbound from pane/node.
6. `Destroyed` — runtime resource released.

Crash branch:

- `Attached|Ready -> Crashed` on runtime failure.
- `Crashed -> Recovering` on user/system retry.
- `Recovering -> Creating|Failed` depending on outcome.

---

## 3. Crash Recovery Contract

On crash:

- emit explicit crash diagnostic,
- set pane to explicit degraded/placeholder presentation,
- surface recovery action (`Retry`/`Reload`),
- preserve graph/workbench identity (no silent node deletion).

Retry behavior:

- user or policy-triggered retry enters `Recovering`,
- retry attempts are bounded/rate-limited,
- successful retry returns to `Creating -> Attached -> Ready`,
- exhausted retries keep explicit failed state with reason.

---

## 4. Fallback and UI Contract

When webview is unavailable:

- node pane must remain valid UI surface,
- fallback content explains failure and available actions,
- focus/navigation remain deterministic,
- command surfaces still route through `ActionRegistry`.

---

## 5. Diagnostics Contract

Required channels:

- `viewer:webview_create_requested` (Info)
- `viewer:webview_created` (Info)
- `viewer:webview_attached` (Info)
- `viewer:webview_crashed` (Error)
- `viewer:webview_recover_attempt` (Info)
- `viewer:webview_recover_failed` (Warn/Error)
- `viewer:webview_recovered` (Info)

Minimum payload:

- `node_key`,
- runtime backend,
- stage transition,
- crash/recovery reason,
- attempt count.

---

## 6. Test Contract

Required coverage:

1. create/attach/detach/destroy happy path,
2. crash event drives degraded state + recovery affordance,
3. retry success path,
4. retry exhaustion path,
5. no identity loss on crash/retry.

---

## 7. Acceptance Criteria

- [ ] Stage model in §2 is represented in runtime state/diagnostics.
- [ ] Crash recovery behavior in §3 is implemented.
- [ ] Fallback UI contract in §4 is test-covered.
- [ ] Diagnostics channels in §5 are emitted.
- [ ] Scenario coverage in §6 is CI-gated.
