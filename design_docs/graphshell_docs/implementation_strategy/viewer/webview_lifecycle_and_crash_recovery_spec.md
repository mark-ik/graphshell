# WebView Lifecycle and Crash Recovery Spec

**Date**: 2026-03-01  
**Status**: Canonical interaction contract  
**Priority**: Immediate implementation guidance

**Related**:

- `viewer_presentation_and_fallback_spec.md`
- `node_lifecycle_and_runtime_reconcile_spec.md`
- `universal_content_model_spec.md`
- `wry_integration_spec.md`
- `../aspect_render/frame_assembly_and_compositor_spec.md`
- `../subsystem_ux_semantics/ux_scenario_and_harness_spec.md`

---

## 1. Purpose and Scope

This spec defines runtime lifecycle contracts for webview-backed node panes.

It governs:

- creation/attach/detach/destroy flow,
- page-load lifecycle and cancellation behavior,
- crash handling and recovery behavior,
- viewer-local activity state needed by shared chrome,
- pane fallback behavior after crash,
- diagnostics and test contracts.

---

## 2. Lifecycle Stages

Canonical stage sequence:

1. `Requested` — open/activate requires webview.
2. `Creating` — runtime allocates webview and binds node.
3. `Attached` — webview mapped to node pane runtime.
4. `Loading` — page request in progress for the current navigation target.
5. `Ready` — first stable render/navigation event observed.
6. `Detached` — webview unbound from pane/node.
7. `Destroyed` — runtime resource released.

Crash branch:

- `Attached|Ready -> Crashed` on runtime failure.
- `Crashed -> Recovering` on user/system retry.
- `Recovering -> Creating|Failed` depending on outcome.

Page-load branch:

- `Attached|Ready -> Loading` on navigation or reload start.
- `Loading -> Ready` on successful completion.
- `Loading -> Attached|Ready` on explicit stop/cancel.
- `Loading -> Failed` on navigation/load failure.

Viewer activity state projected to shared chrome:

- `find_in_page_available: bool`
- `content_zoom_available: bool`
- `content_zoom_level: Option<f32>`
- `media_state: silent | playing | muted`
- `download_state: idle | active | recent`

---

## 2A. Load Control Contract

While a webview is in `Loading`, Graphshell must expose a visible stop/cancel
load affordance in shared chrome for the focused node viewer.

Required behavior:

- `StopLoad` / `CancelLoad` routes to viewer/runtime authority for the focused
	pane only
- invoking stop/cancel must not destroy the pane or node identity
- cancellation returns the viewer to the last stable attached/ready state when
	possible, or to explicit failure/placeholder state if the navigation target
	cannot yield a stable prior state
- reload and stop must be distinguishable in both UI and diagnostics

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

When page-local capabilities are unavailable:

- find in page must degrade with an explicit blocked/unavailable reason rather
	than silently opening graph search
- content zoom must degrade independently of graph camera zoom
- media controls and downloads state must disappear or show explicit
	unavailable state rather than imply inactivity by silence alone

---

## 5. Diagnostics Contract

Required channels:

- `viewer:webview_create_requested` (Info)
- `viewer:webview_created` (Info)
- `viewer:webview_attached` (Info)
- `viewer:webview_load_started` (Info)
- `viewer:webview_load_stopped` (Info)
- `viewer:webview_load_failed` (Warn)
- `viewer:webview_find_in_page_requested` (Info)
- `viewer:webview_content_zoom_changed` (Info)
- `viewer:webview_media_state_changed` (Info)
- `viewer:webview_download_started` (Info)
- `viewer:webview_crashed` (Error)
- `viewer:webview_recover_attempt` (Info)
- `viewer:webview_recover_failed` (Error)
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
2. load start/complete/stop/fail path,
3. stop/cancel load preserves pane/node identity,
4. find-in-page, content zoom, media state, and downloads state are exposed to
	shared chrome/runtime observers,
5. crash event drives degraded state + recovery affordance,
6. retry success path,
7. retry exhaustion path,
8. no identity loss on crash/retry.

---

## 7. Acceptance Criteria

- [ ] Stage model in §2 is represented in runtime state/diagnostics.
- [ ] Load-control behavior in §2A is implemented.
- [ ] Crash recovery behavior in §3 is implemented.
- [ ] Fallback UI contract in §4 is test-covered.
- [ ] Diagnostics channels in §5 are emitted.
- [ ] Scenario coverage in §6 is CI-gated.
