# Diagnostics System Research Report

**Document Type**: Research report — architecture, gaps, and forward design
**Date**: 2026-02-24
**Scope**: `registries/atomic/diagnostics.rs`, `shell/desktop/runtime/diagnostics.rs`, `desktop/tests/harness.rs`, diagnostics pane

> **Terminology note (2026-02-26)**: The names `TestHarness` and `TestRegistry` have been swapped
> since this document was written. In current canonical terminology:
>
> - **`TestRegistry`** = the `cargo test` fixture struct (`desktop/tests/harness.rs`) — app factory + assertion surface
> - **`TestHarness`** = the planned in-pane runner — named test suites, background execution, panic isolation
>
> Wherever this document says "TestHarness" for the `cargo test` fixture, read `TestRegistry`.
> Wherever it says "TestRegistry" for the in-pane runner concept, read `TestHarness`.
> See `SUBSYSTEM_DIAGNOSTICS.md §4` and `TERMINOLOGY.md` for canonical definitions.

---

## 1. Current System Inventory

### 1.1 DiagnosticsRegistry (`registries/atomic/diagnostics.rs`)

Manages channel schema and invariant contracts:

- **Channel registration** — Core/Mod/Verse/Agent/Runtime sources, namespace enforcement, schema versioning, conflict policies (`RejectConflict`, `ReplaceExisting`, `KeepExisting`)
- **Per-channel config** — `enabled`, `sample_rate` (0.0–1.0), `retention_count`
- **Invariant watchdogs** — start channel → terminal channels within timeout_ms; `sweep_invariants()` collects expired tokens as `DiagnosticsInvariantViolation`
- **Global accessors** — `should_emit_and_observe()`, `list_channel_configs_snapshot()`, `set_channel_config_global()`, `apply_persisted_channel_configs()`

49 channels registered across three phase contracts (Phase 0: protocol/viewer, Phase 2: action/input/lens/layout/theme/physics, Phase 3: identity/diagnostics/mod/startup/persistence/verse/ui).

### 1.2 DiagnosticsState (`shell/desktop/runtime/diagnostics.rs`)

Runtime event accumulation and pane state:

- **Event ring** — `VecDeque<DiagnosticEvent>` drained at 100ms intervals; `force_drain_for_tests()` for deterministic flushing
- **Event types** — `Span`, `MessageSent`, `MessageReceived`, `CompositorFrame`, `IntentBatch`
- **DiagnosticGraph** — per-channel message counts, bytes sent, latency (cumulative + recent VecDeque), span enter/exit counts
- **Compositor snapshots** — frame sequence, active tile count, tile rects, webview mapping, paint callback state, hierarchy
- **JSON export** — `snapshot_json_for_tests()` for assertion-driven scenario testing

### 1.3 TestHarness (`desktop/tests/harness.rs`)

Headless integration driver:

- Wraps `GraphBrowserApp::new_for_testing()` + `DiagnosticsState`
- `step_with_tile_sample()` / `step_with_frame_sample()` — deterministic compositor frame injection
- `snapshot()` → `serde_json::Value` — structured assertion surface
- `channel_count()`, `all_channels()`, `tile_for_node()` — snapshot query helpers
- Uses thread-local `TEST_DIAGNOSTICS_TX` rather than `GLOBAL_DIAGNOSTICS_TX` — already isolated from production event stream

Scenarios cover: routing, persistence, layout, grouping, tags, undo/redo, registries, diagnostics startup.

---

## 2. Conceptual Model: Three Registries

The most important clarification arising from this analysis: **`DiagnosticsRegistry` is misnamed**. It is a channel/invariant schema registry — it answers "what events are valid, who owns them, what contracts must hold between them." It carries no analysis logic and no runnable behavior.

Freeing up the "diagnostic" namespace reveals three genuinely distinct concerns that warrant separate registries:

### ChannelRegistry *(rename of DiagnosticsRegistry)*
The schema layer. Registers channel IDs, ownership, schema versions, sampling config, and invariant contracts. Purely declarative. No behavior.

### AnalyzerRegistry *(new)*
Continuous stream processors. Registers stateful or stateless functions that consume the live event stream and produce derived signals — health scores, alert conditions, aggregated metrics, pane sections. These run in the background on every drain cycle. Mods register analyzers for their own channel namespaces, making the pane extensible without hardcoding subsystem knowledge into core.

### TestRegistry *(new, feature-gated)*
On-demand isolated execution. Registers named test cases that run against synthetic fresh state and return structured pass/fail results. A developer tool, not part of the production observability surface. Feature-gated: `#[cfg(any(test, feature = "diagnostics_tests"))]`.

---

## 3. Probe vs. Analyzer vs. Test

Within the TestRegistry concept, there are two meaningfully different things that share a "run on demand" UX:

| | Probe | Test |
|---|---|---|
| **State** | Reads live production state | Creates synthetic isolated state |
| **Purpose** | Observational — "what is the system doing?" | Assertive — "does the system behave correctly?" |
| **Feature gate** | No | Yes |
| **Output** | Observations / structured snapshot | Pass / Fail / Panic |
| **Side effects** | None (read-only by definition) | Contained within isolated harness |

**The classification rule:** if you're reading state, it's a probe. If you're creating state, it's a test.

Both probes and analyzers ship ungated. Tests ship gated. In the pane, probes and tests share a "Run" invocation UX; analyzers have a separate "Active Analyzers" section showing continuous output.

### Feature gating summary

| Category | Gate | Rationale |
|---|---|---|
| Analyzers | None | Production observability infrastructure |
| Probes | None | Read-only live state; valuable precisely when things break in production |
| Tests | `diagnostics_tests` | Isolated harness machinery (`TestHarness`, `new_for_testing`) not needed in production |

### Exceptions and edge cases

**Startup structural verification** — checks like "do all phase0/2/3 channels exist in the registry at boot?" look like tests but read live global state. These belong as **startup analyzers** (run once at init, emit results as channel events, e.g., `startup.selfcheck.channels_incomplete`). No gate. Coverage is highest value precisely in production.

**Regression probes** — if a specific bug has occurred (e.g., compositor tile count diverges from graph node count), a live-state check for that condition is a probe. The isolated deterministic reproduction lives in the test registry for CI; the live-state variant ships ungated for field debugging.

**Mod contract verification** — when a mod loads, verifying its registered channels conform to its declared contract (namespace, schema version, capabilities) is a probe triggered at load time. No gate.

**Probes with side effects** don't exist — that's a category error. If something has side effects, it's an action or a test, not a probe. The presence of side effects is a signal of miscategorization, not a reason to gate.

---

## 4. System-Level Gaps

### 4.1 Invariant coverage is critically thin

`register_default_invariants()` registers exactly **one** invariant: `invariant.registry.protocol.resolve_completes` (500ms). The system has multi-step channel pairs for mod load, identity sign, persistence open, verse init, action execute, and viewer select — none have watchdogs. Every `*_started` → `*_{succeeded|failed}` pair representing a real operation warrants an invariant.

### 4.2 Violations are contextless

`DiagnosticsInvariantViolation` carries only `invariant_id`, `start_channel`, and `deadline_unix_ms`. Missing:
- What triggered the operation (the input: URL, action ID, mod ID)
- The last channel event observed before expiry
- Cumulative violation count for this invariant in the session

When an invariant fires, you know *that* it happened, not *why*.

### 4.3 `retention_count` is stranded

`ChannelConfig.retention_count` exists and is persisted, but `DiagnosticsRegistry` doesn't maintain per-channel event queues — the ring lives in `DiagnosticsState`. These layers don't communicate. Wire them up (state layer respects per-channel retention from registry config) or remove the field until there's a plan to use it.

### 4.4 Auto-registration is silently permissive

`should_emit_channel()` auto-registers unknown channels as `Runtime / "Auto-registered runtime channel"` with no signal that uncontracted code paths are emitting. A dedicated `orphan_channels` counter or set in `DiagnosticsRegistry` would surface this without changing behavior.

### 4.5 Violation return path needs auditing

`should_emit_and_observe()` returns `Vec<DiagnosticsInvariantViolation>`. If any callsite discards these rather than routing them into the event ring, violations are silently lost. Audit all callsites.

### 4.6 No severity tier

All channels are peers. `registry.protocol.resolve_failed` is semantically more important than `registry.layout.fallback_used`. A `ChannelSeverity { Info, Warn, Error }` field on `DiagnosticChannelDescriptor` (defaulting to `Info`) would let the pane and any alerting logic distinguish signal from noise without restructuring anything.

---

## 5. Pane-Level Gaps

### Missing tabs / views

**Violations** — `DiagnosticsInvariantViolation` events are generated but not prominently surfaced. A dedicated violations view showing: which invariant, when it opened, cumulative count this session, last observed channel before timeout.

**Health summary** — per-subsystem (protocol, registry, identity, persistence, verse) derived from failure/fallback channel ratios using the existing `message_counts`. A green/yellow/red indicator per subsystem, computed purely from channel data already in `DiagnosticGraph`.

**Invariant graph** — the registry already knows start→terminal relationships. A small static DAG showing which invariants are currently pending, healthy, or violated would make the watchdog system legible rather than just present.

**Active analyzers** — once AnalyzerRegistry exists, the pane needs a section showing registered analyzers, their current output, and whether they're producing signal.

### Missing controls

**Channel config editor** — `set_channel_config_global()` and `apply_persisted_channel_configs()` exist but there is presumably no UI to toggle channels or adjust sample rates live. Being able to silence a noisy channel or raise sampling on a quiet one at runtime is high-value during debugging.

**Filter / search** — 50+ channels need filtering by prefix (`registry.*`, `verse.*`, `mod.*`), by owner source, and by state (ever fired vs. silent this session).

### Missing data presentation

**Rate indicators** — `message_latency_recent_us` has a VecDeque that could feed a per-channel "events/sec" rolling average or sparkline. Cumulative counts alone hide whether a channel fired 100 times at startup or is firing 10 times/sec right now.

**Intent → frame correlation** — `intents` VecDeque and compositor frames are both recorded but independently. A timeline linking "intent batch at frame N" to "compositor state at frame N+K" would make graph interaction bugs causally traceable.

---

## 6. TestRegistry Architecture

When built, the TestRegistry should:

- Hold named `TestSuite` structs, each containing a `&'static [TestCase]`
- Each `TestCase` has `id`, `label`, and a bare `fn() -> TestOutcome` (not a closure — no captures, stateless)
- `TestOutcome` variants: `Pass`, `Fail { message: String }`, `Panic { message: String }`
- Panic catching via `std::panic::catch_unwind`
- Execution on a background thread; results streamed back to the pane via crossbeam channel
- Registration is explicit: each module exposes a `test_suite() -> TestSuite` function, feature-gated; a top-level `TestRegistry::all()` collects them

No proc macros or `inventory` crate needed. Explicit registration fits the codebase's existing patterns and the `inventory` crate is already in use for native mod registration — avoid overlap in meaning.

### What runs safely in-pane (gated)

- All `DiagnosticsRegistry` unit tests — create fresh local instances, zero global state entanglement
- Channel registration contract tests — phase0/2/3 completeness
- Invariant watchdog behavior — missing terminal → violation
- Config roundtrip — set → get preserves values
- Namespace enforcement — mod/verse channel prefix rules
- Pure graph state machine tests via `TestHarness` — grouping, tagging, undo/redo

### What doesn't run in-pane

- Tests requiring real webviews, GPU context, iroh network, or production file paths
- Tests that would conflict with `GLOBAL_DIAGNOSTICS_REGISTRY` (OnceLock) — pure local-instance tests avoid this; tests calling `should_emit_and_observe()` globally would share the production registry and should not run in-pane

---

## 7. Priority Order

1. **Fill invariant coverage** — add watchdogs for mod load, identity sign, persistence open, viewer select, action execute; all have the channel pairs already
2. **Add `ChannelSeverity` to `DiagnosticChannelDescriptor`** — one field, unlocks health summary and pane prioritization
3. **Surface violations in the pane** — violations are generated; build the view
4. **Wire channel config editor in the pane** — infrastructure exists, no UI
5. **Startup structural verification as analyzer** — runs at init, emits `startup.selfcheck.*` channels, no gate, replaces the need for equivalent test coverage in production
6. **AnalyzerRegistry** — extensible analysis layer; required for mod-contributed pane sections
7. **TestRegistry** (`diagnostics_tests` feature) — developer tool; smoke tests runnable from pane

---

## Related Documents

- [IMPLEMENTATION_ROADMAP.md](../implementation_strategy/IMPLEMENTATION_ROADMAP.md) — Feature Target 10 (Diagnostic/Engine Inspector Mode, complete)
- [2026-02-22_registry_layer_plan.md](../implementation_strategy/2026-02-22_registry_layer_plan.md) — Registry architecture and phase plan
- [2026-02-22_test_harness_consolidation_plan.md](../implementation_strategy/2026-02-22_test_harness_consolidation_plan.md) — Test harness architecture
