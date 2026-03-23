# Webview Backpressure Spec

**Date:** 2026-03-12  
**Status:** Canonical runtime policy contract  
**Priority:** Immediate lifecycle/runtime traceability

**Related docs:**

- [`node_lifecycle_and_runtime_reconcile_spec.md`](./node_lifecycle_and_runtime_reconcile_spec.md)
- [`webview_lifecycle_and_crash_recovery_spec.md`](./webview_lifecycle_and_crash_recovery_spec.md)
- [`archived embedder decomposition plan`](../../../archive_docs/checkpoint_2026-03-22/graphshell_docs/implementation_strategy/aspect_render/2026-02-20_embedder_decomposition_plan.md)
- [`../../technical_architecture/2026-03-12_specification_coverage_register.md`](../../technical_architecture/2026-03-12_specification_coverage_register.md)

---

## 1. Purpose and Scope

This spec defines the runtime admission, retry, timeout, and cooldown policy implemented by `webview_backpressure.rs`.

It governs:

- when webview creation is attempted,
- how provisional creation is confirmed,
- how retries and cooldown are applied,
- when a node becomes `RuntimeBlocked`,
- how blocked state is cleared,
- diagnostics and test obligations.

It does not govern:

- general lifecycle state semantics beyond the backpressure policy,
- crash recovery for already-attached webviews,
- viewer fallback rendering policy except where blocked state must be surfaced.

---

## 2. Canonical Role

Webview backpressure is the admission-control policy for activating webview-backed nodes under unstable or overloaded runtime conditions.

Current product meaning:

- Servo webview creation is not directly fallible in the embedder API,
- Graphshell therefore infers success or failure from downstream evidence,
- repeated unconfirmed create attempts are bounded and eventually converted into explicit blocked state.

Normative rule:

- failed or unstable activation must become explicit runtime state,
- not an invisible retry loop.

---

## 3. Current State Model

Per-node tracked state:

- `retry_count`
- `pending` creation probe
- `cooldown_until`
- `cooldown_step`

Current meaning:

- `pending` means a create attempt has been issued and awaits confirmation,
- `retry_count` counts creation attempts since the last confirmed success or cooldown reset,
- `cooldown_until` prevents immediate reattempt after exhaustion,
- `cooldown_step` drives exponential cooldown backoff.

Normative rule:

- this state is runtime policy state, not canonical node/domain truth,
- it must remain per-node and bounded.

---

## 4. Create Attempt Contract

### 4.1 Preconditions

`ensure_webview_for_node(...)` may attempt creation only when:

- the node exists,
- the node lifecycle is `Active`,
- runtime has `viewer:webview` capability,
- there is no confirmed existing live/responsive mapped webview for that node,
- the node is not currently in cooldown,
- there is no pending unresolved probe for that node.

### 4.2 Existing mapping behavior

If the app still believes a node is mapped to a webview but the window no longer contains that webview:

- an explicit `UnmapWebview` runtime event is emitted,
- backpressure state remains authoritative for the next attempt.

### 4.3 Attempt start behavior

When a create attempt starts:

- a webview is created or finalized from a pending create request,
- node-to-webview mapping is emitted,
- the node is promoted toward active runtime use,
- a creation probe is armed,
- `retry_count` increments.

Normative rule:

- a create attempt is real only after a concrete host/webview creation path is taken and a probe is armed,
- merely desiring activation does not count as a retry.

---

## 5. Confirmation Contract

Because creation is not synchronously fallible, confirmation is inferred.

Current confirmation signals:

- the webview emits a responsive semantic signal, or
- the window contains the webview and it remains live past the confirmation window.

Current timing constants:

- confirmation window: 2 seconds
- timeout window: 8 seconds

Outcome classification:

- `Confirmed`
- `Pending`
- `TimedOut`

Normative rule:

- a webview create attempt is considered successful only after one of the confirmation conditions is satisfied,
- not merely because a create call returned.

---

## 6. Timeout, Retry, and Cooldown Policy

### 6.1 Timeout behavior

If a pending probe reaches timeout without confirmation:

- the webview is closed if still present,
- the mapping is explicitly unmapped,
- the pending probe is cleared,
- the node remains eligible for retry unless retry exhaustion is reached.

### 6.2 Retry bound

Current maximum retries before cooldown:

- `WEBVIEW_CREATION_MAX_RETRIES = 3`

Normative rule:

- retry loops must be bounded,
- repeated unstable creation must not churn indefinitely in the hot path.

### 6.3 Cooldown behavior

After retry exhaustion:

- exponential cooldown is armed,
- `RuntimeBlocked(CreateRetryExhausted)` is emitted with `retry_at`,
- `retry_count` resets,
- the node remains blocked until cooldown expires and a subsequent clear path runs.

Current cooldown behavior:

- exponential backoff,
- minimum 1 second,
- maximum 30 seconds,
- bounded step growth.

### 6.4 Cooldown expiry

When cooldown has elapsed and reconcile revisits the node:

- cooldown is cleared,
- blocked state is explicitly cleared,
- retry budget is reset,
- normal create-attempt logic may resume.

Normative rule:

- cooldown expiry clears the blocked state explicitly; it is not merely implied by time passing.

---

## 7. RuntimeBlocked Contract

This module is a canonical source of `RuntimeBlocked(CreateRetryExhausted)` for webview-backed activation failure.

Current meaning:

- `RuntimeBlocked` is not a generic rendering hint here,
- it specifically means repeated creation attempts failed to reach confirmed live state.

Required behavior:

- blocked state must be visible to lifecycle/viewer/compositor consumers,
- blocked state should carry a retry time when cooldown is active,
- success confirmation or cooldown-expiry clear path must emit `ClearRuntimeBlocked`.

Normative rule:

- if a tile or node is shown as `RuntimeBlocked` because of webview create exhaustion, this module must be traceable as the source of that state.

---

## 8. Lifecycle Relationship

Backpressure does not replace lifecycle; it constrains activation.

Relationship to lifecycle specs:

- lifecycle still governs whether a node is eligible for activation at all,
- backpressure governs whether activation is presently admitted,
- `RuntimeBlocked` preserves explicit failure state without silently changing canonical node identity.

Current code behavior:

- only `Active` nodes are considered for ensure/retry,
- non-`Active` nodes drop backpressure state.

Normative rule:

- backpressure state is subordinate to lifecycle eligibility,
- not an alternate lifecycle state machine.

---

## 9. Diagnostics Contract

Current diagnostic signals include:

- create attempt
- confirmation
- timeout
- cooldown
- span durations for ensure/reconcile

Required diagnostics semantics:

- create attempts are observable,
- confirmation latency is observable,
- timeout is observable,
- cooldown entry is observable,
- blocked/clear runtime events remain the user-visible lifecycle boundary.

Normative rule:

- diagnostics must make it possible to explain why a node became `RuntimeBlocked(CreateRetryExhausted)`.

---

## 10. Test Contract

Required coverage:

1. probe confirms on responsive signal,
2. probe confirms on stable live webview after confirmation window,
3. probe times out without confirmation,
4. cooldown delay is bounded and monotonic by step,
5. cooldown arm updates deadline and step,
6. timed-out probe closes/unmaps stale webview,
7. retry exhaustion emits blocked state with retry time,
8. cooldown expiry clears blocked state and resets retry budget,
9. non-`Active` nodes do not retain backpressure state,
10. source URL restoration for cold restore is deterministic.

---

## 11. Acceptance Criteria

- [ ] create confirmation remains explicit and inferential, not assumed from API success.
- [ ] retry loops remain bounded.
- [ ] cooldown policy remains explicit, exponential, and test-covered.
- [ ] `RuntimeBlocked(CreateRetryExhausted)` remains traceable to this policy.
- [ ] blocked-state clear paths are explicit and test-covered.
- [ ] lifecycle and compositor consumers can rely on this spec as the source for webview-create exhaustion semantics.
