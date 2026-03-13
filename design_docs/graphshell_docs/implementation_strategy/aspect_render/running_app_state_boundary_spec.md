# RunningAppState Boundary Spec

**Date:** 2026-03-12  
**Status:** Canonical ownership contract  
**Priority:** Stage 4b / Embedder decomposition unblocker

**Related docs:**

- [`2026-02-20_embedder_decomposition_plan.md`](./2026-02-20_embedder_decomposition_plan.md)
- [`../viewer/node_lifecycle_and_runtime_reconcile_spec.md`](../viewer/node_lifecycle_and_runtime_reconcile_spec.md)
- [`../viewer/webview_lifecycle_and_crash_recovery_spec.md`](../viewer/webview_lifecycle_and_crash_recovery_spec.md)
- [`../subsystem_focus/focus_and_region_navigation_spec.md`](../subsystem_focus/focus_and_region_navigation_spec.md)
- [`../../technical_architecture/2026-03-12_specification_coverage_register.md`](../../technical_architecture/2026-03-12_specification_coverage_register.md)

---

## 1. Purpose and Scope

This spec defines what `running_app_state.rs` currently owns, what it must not own, and the contract it maintains with `EmbedderCore`, windows, the graph/runtime layer, and external host subsystems.

It governs:

- current ownership boundaries,
- allowed mutation style,
- event and callback flow,
- embedder/runtime coordination rules,
- Stage 4b decomposition constraints.

It does not govern:

- graph reducer semantics,
- focus state machine semantics,
- workbench tile mutation semantics,
- detailed webview lifecycle policy beyond the boundary between host runtime and graph/runtime consumers.

---

## 2. Canonical Role

`RunningAppState` is the graphshell-side host runtime coordinator that sits between `EmbedderCore` and higher-level application/runtime logic.

Current role:

- own graphshell-specific host/runtime state that must live near the embedder loop,
- expose narrowly-scoped services to windows and delegates,
- collect external/host-originated semantic events,
- enqueue or forward those events to graph/runtime consumers without directly becoming a reducer.

Normative rule:

- `RunningAppState` is not the graph application model,
- it is not the reducer,
- it is not a general-purpose global state bag,
- it is a host/runtime boundary object.

---

## 3. Current Ownership

The current file owns or coordinates these families of state:

### 3.1 Embedder/runtime ownership

- `EmbedderCore`
- focused window tracking through embedder-facing APIs
- window lifecycle accessors and event-loop coordination

### 3.2 Host-facing service ownership

- pending create requests and tokens for deferred webview creation
- webdriver channels and response bookkeeping
- gamepad provider access and pending gamepad UI commands
- app preferences needed by host/runtime coordination

### 3.3 Event-queue ownership

- pending graph semantic events emitted from windows / delegates / embedder callbacks
- pending webdriver responses and pending host-UI commands

Normative rule:

- if a concern requires direct coordination with Servo/Winit/WebDriver/Gamepad/window creation, `RunningAppState` is an acceptable carrier,
- if a concern is canonical graph/workbench/domain truth, it is not.

---

## 4. Explicit Non-Ownership

`RunningAppState` must not directly own or mutate the canonical graph application model.

The boundary is already partially enforced by test.

Forbidden direct ownership/mutation:

- `GraphBrowserApp`
- `GraphWorkspace`
- reducer application (`apply_intents`)
- direct emission of `GraphIntent` mutations through application-owned reducer calls
- workbench tile-tree mutation
- semantic focus authority ownership

Current enforcement evidence:

- `servo_callbacks_only_enqueue_events` asserts that host-side callback paths do not directly reference `GraphBrowserApp`, `GraphWorkspace`, `GraphIntent`, or `apply_intents`.

Normative rule:

- callbacks and host/runtime paths may enqueue semantic events or host commands,
- they must not bypass the graph/runtime authority path and mutate app state directly.

---

## 5. Event and Callback Contract

### 5.1 Semantic boundary

`GraphSemanticEvent` is the canonical boundary type between embedder/window callbacks and higher-level graph/runtime processing.

Contract:

- windows and delegates emit semantic events,
- `RunningAppState` stores/drains them,
- the GUI/runtime pipeline later translates them into graph/runtime actions.

Normative rule:

- callback code should express meaning in semantic-event form,
- not as direct graph mutations.

### 5.2 Callback discipline

Host callbacks may:

- enqueue semantic events,
- store pending create requests,
- acknowledge webdriver requests,
- record host-side runtime signals,
- enqueue gamepad UI commands.

Host callbacks must not:

- mutate graph/domain/workbench state directly,
- interpret semantic events and immediately apply reducer logic,
- silently perform cross-layer state repair outside the event pipeline.

---

## 6. Pending Create Request Contract

`RunningAppState` is the current owner of deferred webview creation requests.

Current model:

- host/delegate path stores a `PendingCreateRequest`,
- reconcile/lifecycle path later consumes it by token,
- actual webview creation happens through the host/embedder surface.

Normative rule:

- pending create requests are host-runtime coordination state, not graph-domain state,
- the token/request table must remain narrow and bounded to create-time orchestration,
- graph/runtime layers may request creation indirectly but must not own the host builder state itself.

---

## 7. WebDriver Boundary

`RunningAppState` currently owns WebDriver coordination because it is inherently host/embedder-facing.

It owns:

- webdriver receiver,
- sender tables,
- pending response bookkeeping,
- screenshot/script/load-url handling entry points,
- interrupt coordination.

Normative rule:

- protocol-side webdriver coordination belongs in host/runtime territory,
- graph/runtime layers may observe its semantic effects, but they should not own transport channels directly.

Non-goal:

- this spec does not define WebDriver protocol semantics; it defines only local ownership boundaries.

---

## 8. Gamepad Boundary

`RunningAppState` currently owns gamepad-provider access and dispatch because it bridges platform input and webview/runtime targeting.

It may:

- receive provider events,
- resolve a content webview target through host/runtime targeting rules,
- dispatch content/gamepad events to the correct runtime target,
- enqueue UI commands for later consumption.

It must not:

- directly mutate graph selection or focus state as a shortcut for gamepad handling,
- bypass the established focus/embedded-content targeting rules.

---

## 9. EmbedderCore Relationship

The Stage 4b decomposition target is:

- `EmbedderCore` owns pure embedder runtime responsibilities,
- `RunningAppState` owns graphshell-side coordination and bridge state.

Current practical split:

- `EmbedderCore` already owns Servo and window/embedder core runtime,
- `RunningAppState` wraps it and adds graphshell-specific host/runtime state.

Normative rule for future extraction:

- if a field or method exists only to satisfy embedder/runtime mechanics independent of graphshell policy, it should move toward `EmbedderCore`,
- if a field or method exists to bridge host runtime into graphshell application semantics, it may remain on `RunningAppState`.

Stage 4b target:

- `RunningAppState` becomes thinner, not more globally authoritative.

---

## 10. Decomposition Rules for Stage 4b

When continuing the split:

1. do not change semantic callback ordering as part of ownership refactors,
2. preserve `GraphSemanticEvent` as the callback-to-runtime boundary,
3. do not let `RunningAppState` reacquire reducer/application mutation privileges,
4. extract pure embedder mechanics before extracting cross-layer coordination,
5. keep host/runtime service tables explicit rather than burying them in mixed utility modules.

Recommended extraction ordering:

1. isolate additional pure window/embedder utilities,
2. isolate webdriver transport helpers if they can move without breaking the host boundary,
3. isolate gamepad bridge helpers if they remain host/runtime-only,
4. keep semantic-event emission/drain behavior stable through each step.

---

## 11. Diagnostics and Test Contract

Required guarantees:

- callback paths remain enqueue-only with respect to graph/runtime state,
- pending create request flow remains explicit and testable,
- semantic events remain drainable in deterministic order,
- host/runtime subsystems (webdriver, gamepad, window create) remain observable without requiring direct graph mutation.

Required coverage:

1. callback paths do not reference reducer/application mutation APIs directly,
2. pending create requests can be stored and consumed exactly once,
3. semantic-event drain works without direct graph coupling,
4. host reclaim / content targeting behavior remains deterministic,
5. embedder decomposition refactors preserve callback ordering.

---

## 12. Acceptance Criteria

- [ ] `RunningAppState` is documented as a host/runtime coordinator, not as canonical application state.
- [ ] direct graph/reducer mutation from host callbacks remains forbidden and test-covered.
- [ ] `GraphSemanticEvent` remains the canonical callback boundary.
- [ ] pending create requests, webdriver coordination, and gamepad bridging remain explicitly owned.
- [ ] Stage 4b extraction work can classify fields/methods against this boundary without rediscovering ownership from source each time.
