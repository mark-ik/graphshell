# Semantic Event Pipeline — Interaction Spec

**Date**: 2026-03-12
**Status**: Canonical interaction contract
**Priority**: Implementation-ready (documents existing implementation)

**Related**:

- `ASPECT_INPUT.md`
- `input_interaction_spec.md`
- `../subsystem_diagnostics/` (diagnostic channel policy)
- `shell/desktop/lifecycle/semantic_event_pipeline.rs`

---

## 1. Scope

This spec defines the canonical contracts for:

1. **GraphSemanticEvent → RuntimeEvent translation** — the boundary between the
   Servo embedder and graphshell graph/app logic.
2. **Responsive webview set semantics** — which events qualify a webview as
   "responsive" for a given frame.
3. **Ordering and sequencing invariants** — the `seq` field contract and
   its role in deterministic ingest.
4. **Diagnostics channels** — required channels, severities, and required fields.
5. **Public API contract** — the two exported functions and their behavioral
   contracts.

---

## 2. Boundary Model

`GraphSemanticEvent` is the canonical output of the Servo embedder layer. It
represents a content-level semantic change (URL navigated, history mutated,
title changed, crash, open request) with no graphshell-layer interpretation
applied. It is the clean boundary: the embedder emits it; graphshell consumes it.

`RuntimeEvent` is graphshell's internal event vocabulary. It carries the same
semantic payload but in graphshell types (e.g. `NodeKey` resolved from
`WebViewId`). The pipeline's job is translation and disambiguation — no side
effects, no I/O.

The pipeline is a **pure translation layer**. It holds no mutable state between
calls. Calls are idempotent given the same inputs.

---

## 3. GraphSemanticEvent Contract

```
GraphSemanticEvent {
    seq: u64,           // monotonic sequence number assigned by embedder
    kind: GraphSemanticEventKind,
}

GraphSemanticEventKind =
  | UrlChanged       { webview_id: WebViewId, url: String }
  | HistoryChanged   { webview_id: WebViewId, can_go_back: bool, can_go_forward: bool }
  | PageTitleChanged { webview_id: WebViewId, title: String }
  | WebViewCrashed   { webview_id: WebViewId }
  | HostOpenRequest  { url: String }
```

### 3.1 `seq` Field

- `seq` is a monotonic u64 assigned by the embedder per semantic event.
- The pipeline uses `seq` for ordering guarantees across heterogeneous event
  kinds that may arrive in a batch.
- Consumers downstream of the pipeline must not re-order events from the same
  batch.
- Batches are not required to be contiguous in `seq` space; gaps are allowed
  (events may be filtered at the embedder level before emission).

### 3.2 Ordering Invariant

Within a single pipeline call, emitted `RuntimeEvent` values are ordered
consistent with the input `GraphSemanticEvent` slice order (which reflects
ascending `seq`). Consumers may rely on this for deterministic intent
application.

---

## 4. RuntimeEvent Mapping

| `GraphSemanticEventKind` | `RuntimeEvent` | Notes |
|---|---|---|
| `UrlChanged` | `WebViewUrlChanged { webview_id, url }` | Qualifies webview as responsive |
| `HistoryChanged` | `WebViewHistoryChanged { webview_id, can_go_back, can_go_forward }` | Qualifies webview as responsive |
| `PageTitleChanged` | `WebViewTitleChanged { webview_id, title }` | Qualifies webview as responsive |
| `WebViewCrashed` | `WebViewCrashed { webview_id }` | Does NOT qualify as responsive |
| `HostOpenRequest` | `HostOpenRequest { url }` | No associated webview |

---

## 5. Public API Contract

### 5.1 `runtime_events_from_semantic_events`

```rust
pub fn runtime_events_from_semantic_events(
    events: &[GraphSemanticEvent],
) -> Vec<RuntimeEvent>
```

- Translates a batch of `GraphSemanticEvent` to `Vec<RuntimeEvent>`.
- Does not compute the responsive webview set.
- Use when the caller does not need to track which webviews produced activity.

### 5.2 `runtime_events_and_responsive_from_events`

```rust
pub fn runtime_events_and_responsive_from_events(
    events: &[GraphSemanticEvent],
) -> (Vec<RuntimeEvent>, HashSet<WebViewId>)
```

- Returns the same `Vec<RuntimeEvent>` as the simple form, plus a
  `HashSet<WebViewId>` — the **responsive webview set**.
- The responsive set is the set of `WebViewId`s that produced at least one
  URL, history, or title event in the batch (see §6).
- This is the canonical entry point for the frame loop; prefer this over the
  simple form unless the responsive set is genuinely unneeded.

---

## 6. Responsive Webview Set Semantics

A webview is **responsive** in a given frame if it produced at least one of:

- `UrlChanged`
- `HistoryChanged`
- `PageTitleChanged`

`WebViewCrashed` does **not** qualify a webview as responsive. Crashes are
lifecycle events, not activity events, and must not be conflated with
content-level responsiveness.

`HostOpenRequest` has no associated webview and never contributes to the
responsive set.

**Use cases** for the responsive set:

- Frame loop uses the set to determine which node panes need render mode refresh
  after intent application.
- Webview backpressure logic uses the set to prioritize WebView allocation.

---

## 7. Diagnostics Contract

All diagnostic channels emitted by the pipeline must use the following names
and severities. No channel not listed here may be emitted by this module.

| Channel | Severity | Fields | Intent |
|---|---|---|---|
| `semantic.events_ingest` | `Info` | event count, span timing | Batch received and starting ingest |
| `semantic.intents_emitted` | `Info` | emitted count | Translation complete |
| `semantic.intent.url_changed` | `Info` | `webview_id` | URL event translated |
| `semantic.intent.history_changed` | `Info` | `webview_id` | History event translated |
| `semantic.intent.title_changed` | `Info` | `webview_id` | Title event translated |
| `semantic.intent.webview_crashed` | `Warn` | `webview_id` | Crash event translated |
| `semantic.intent.host_open_request` | `Info` | `url` | Open request translated |

`semantic.intent.webview_crashed` is `Warn` because a crash is an unexpected
content failure. All other per-event channels are `Info`.

Span timing for the full ingest is emitted on `semantic.events_ingest` via the
`semantic_event_pipeline::runtime_events_and_responsive_from_events` span.

### 7.1 Performance Contract

The pipeline must complete within one frame budget. It must not block on I/O.
No file system access, no network calls, no mutex contention with Servo threads
is permitted inside these functions.

---

## 8. Accounting Invariants

These invariants must hold for any valid batch:

1. `emitted_runtime_events.len() == input_semantic_events.len()` — one
   `RuntimeEvent` is produced per `GraphSemanticEvent`. No events are dropped
   or duplicated by the pipeline.
2. The responsive set is a subset of the webview IDs that appear in the emitted
   events: `responsive_set ⊆ { e.webview_id | e in events }`.
3. No webview appears in the responsive set unless it produced a
   URL/history/title event. Crash events must not produce responsive entries.

---

## 9. Test Coverage Requirements

| Criterion | Verification |
|---|---|
| Each `GraphSemanticEventKind` maps to the correct `RuntimeEvent` variant | Unit test: one test per variant |
| `WebViewCrashed` does not add to responsive set | Unit test: crash event → responsive set is empty |
| URL/history/title events add to responsive set | Unit test: each event type → webview in responsive set |
| `HostOpenRequest` never adds to responsive set | Unit test: host open → responsive set is empty |
| Accounting invariant: emitted count equals input count | Proptest: random batch → `|output| == |input|` |
| Accounting invariant: responsive ⊆ webview IDs in events | Proptest: random batch → subset check |
| Order preservation: output order matches input seq order | Unit test: mixed batch → events in input order |
| Snapshot stability: batch output is stable across builds | Insta snapshot test |
