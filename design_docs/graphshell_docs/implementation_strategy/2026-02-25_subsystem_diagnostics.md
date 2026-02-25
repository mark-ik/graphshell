# Cross-Cutting Subsystem: Diagnostics (2026-02-25)

**Status**: Active / Project Goal
**Subsystem label**: `diagnostics`
**Long form**: Diagnostics Subsystem
**Scope**: Runtime observability infrastructure — channel schema, invariant watchdogs, analyzers, test execution pane, and the diagnostic inspector
**Subsystem type**: Cross-Cutting Runtime Subsystem (see `TERMINOLOGY.md`)
**Peer subsystems**: `accessibility` (Accessibility), `security` (Security & Access Control), `storage` (Persistence & Data Integrity), `history` (Traversal & Temporal Integrity)
**Doc role**: Canonical subsystem implementation guide (summarizes guarantees/roadmap and links to detailed research/plans; avoid duplicating subsystem contracts elsewhere)
**Elevates**: `../research/2026-02-24_diagnostics_research.md` (research report → subsystem plan)
**Related**: `2026-02-22_registry_layer_plan.md` (Phase 3 channel contracts), `2026-02-22_test_harness_consolidation_plan.md`

---

## 1. Why This Exists

Diagnostics is the **reference cross-cutting subsystem**. It was the first concern in Graphshell to have declarative contracts (channel schemas, invariant watchdogs), runtime state (event ring, compositor snapshots), and structured validation (harness scenarios, contract tests).

This document formalizes diagnostics as a subsystem with explicit guarantees, not just infrastructure. All other subsystems (accessibility, security, persistence) emit their observability through this subsystem's channels and invariant machinery.

**The key insight from the 2026-02-24 research**: `DiagnosticsRegistry` is actually a *channel schema registry*. Freeing up the namespace reveals three distinct concerns:

1. **ChannelRegistry** (rename of DiagnosticsRegistry) — schema layer
2. **AnalyzerRegistry** (new) — continuous stream processors
3. **TestRegistry** (new, feature-gated) — on-demand isolated test execution

---

## 2. Subsystem Model (Four Layers)

| Layer | Diagnostics Instantiation |
|---|---|
| **Contracts** | Channel schema integrity, invariant watchdog coverage, severity classification, namespace enforcement — §3 |
| **Runtime State** | Event ring (`VecDeque<DiagnosticEvent>`), `DiagnosticGraph` (per-channel counts/latency), compositor snapshots — owned by `DiagnosticsState` |
| **Diagnostics** | Self-referential: `diagnostics.selfcheck.*` channels for startup integrity, orphan channels, violation routing — §5 |
| **Validation** | Contract tests (phase completeness, invariant behavior), harness scenarios, pane smoke tests — §6 |

---

## 3. Required Invariants / Contracts

### 3.1 Channel Schema Integrity

1. **Namespace enforcement** — Mod channels must use their declared namespace prefix. Verse channels use `verse.*`, mod channels use `mod.<mod_id>.*`. Core channels use `registry.*`, `startup.*`, `ui.*`, etc.
2. **No silent auto-registration** — Unknown channels auto-registered at runtime are tracked in an `orphan_channels` set and surfaced via `diagnostics.selfcheck.orphan_channel_registered` (Warn). Auto-registration behavior is preserved for robustness, but orphans are never silent.
3. **Schema version stability** — Once a channel is registered with a schema version, the version must not change without an explicit migration. Conflicting registrations are handled by `ConflictPolicy` (RejectConflict / ReplaceExisting / KeepExisting).
4. **Phase completeness** — All channels declared in phase contracts (Phase 0, 2, 3) must be present in the registry at startup. Missing channels emit `diagnostics.selfcheck.channels_incomplete` (Error).

### 3.2 Invariant Watchdog Contracts

1. **Coverage requirement** — Every `*_started` → `*_{succeeded|failed}` channel pair representing a real operation warrants an invariant watchdog. Current coverage (1 invariant for protocol resolve) is critically thin; watchdogs are required for: mod load, identity sign, persistence open, viewer select, action execute.
2. **Violation context** — `DiagnosticsInvariantViolation` must carry: invariant ID, what triggered the operation, the last channel event observed before expiry, and cumulative violation count for the session.
3. **Violation routing** — All invariant violations returned by `should_emit_and_observe()` must be routed into the event ring. No callsite may silently discard violations.
4. **Pending invariant visibility** — The system must expose which invariants are currently pending (started but not yet terminated), enabling causal debugging.

### 3.3 Severity Classification

1. **All channels have severity** — Every `DiagnosticChannelDescriptor` carries a `ChannelSeverity { Info, Warn, Error }` field (defaulting to `Info`).
2. **Severity semantics**:
   - `Error`: failures, invariant violations, conversion failures
   - `Warn`: drops, missing anchors, stale updates, throttled operations, fallback paths
   - `Info`: normal lifecycle events (received, queued, sent, routed, rebuilt)
3. **Severity is filterable** — The diagnostic pane and any alerting logic can filter/prioritize by severity.

### 3.4 Retention & Configuration

1. **Per-channel retention** — `ChannelConfig.retention_count` is wired to `DiagnosticsState` so the event ring respects per-channel limits from registry config.
2. **Live channel config** — `set_channel_config_global()` and `apply_persisted_channel_configs()` are exposed in the diagnostics pane for runtime toggling/sampling adjustment.
3. **Config roundtrip** — Set → get preserves values for all channel configurations.

---

## 4. Three-Registry Architecture

### 4.1 ChannelRegistry (rename of DiagnosticsRegistry)

The schema layer. Registers channel IDs, ownership, schema versions, sampling config, and invariant contracts. Purely declarative. No behavior.

This is the existing `DiagnosticsRegistry` with explicit recognition that it carries no analysis logic and no runnable behavior.

### 4.2 AnalyzerRegistry (new)

Continuous stream processors. Stateful or stateless functions that consume the live event stream and produce derived signals — health scores, alert conditions, aggregated metrics, pane sections. Run on every drain cycle.

Key properties:
- Mods register analyzers for their own channel namespaces.
- Makes the pane extensible without hardcoding subsystem knowledge into core.
- Ships ungated (production observability infrastructure).

### 4.3 TestRegistry (new, feature-gated)

On-demand isolated execution. Named test cases that run against synthetic fresh state, returning structured pass/fail results.

- Feature-gated: `#[cfg(any(test, feature = "diagnostics_tests"))]`
- Named `TestSuite` structs containing `&'static [TestCase]`
- `TestCase` has `id`, `label`, `fn() -> TestOutcome`
- `TestOutcome`: `Pass`, `Fail { message }`, `Panic { message }`
- Panic catching via `std::panic::catch_unwind`
- Execution on background thread; results streamed to pane via crossbeam channel
- Explicit registration (no proc macros; each module exposes `test_suite() -> TestSuite`)

### 4.4 Classification Rule

| Category | State | Feature Gate | Purpose |
|---|---|---|---|
| **Analyzer** | Live production | None | Continuous observability |
| **Probe** | Live production (read-only) | None | Observational snapshot |
| **Test** | Synthetic isolated | `diagnostics_tests` | Deterministic assertion |

Rule: if you read state, it's a probe. If you create state, it's a test. Probes with side effects don't exist — that's a category error.

**Startup structural verification** (e.g., "do all phase channels exist?") runs as a startup analyzer — no gate, highest-value in production. Emits `startup.selfcheck.*` channels.

---

## 5. Diagnostics Integration (Self-Referential)

### 5.1 Self-Check Channels

| Channel | Severity | Description |
|---|---|---|
| `diagnostics.selfcheck.channels_incomplete` | Error | Phase contract channel missing at startup |
| `diagnostics.selfcheck.orphan_channel_registered` | Warn | Unknown channel auto-registered at runtime |
| `diagnostics.selfcheck.invariant_violation_dropped` | Error | Violation returned by callsite was not routed to event ring |
| `diagnostics.selfcheck.retention_config_mismatch` | Warn | Per-channel retention not respected by state layer |
| `startup.selfcheck.registries_loaded` | Info | All registries initialized successfully |
| `startup.selfcheck.channels_complete` | Info | All phase contract channels present |

### 5.2 Health Summary (Diagnostic Inspector)

- Per-subsystem health indicator (green/yellow/red) derived from failure/fallback channel ratios.
- Invariant graph: a small static DAG showing which invariants are pending, healthy, or violated.
- Active analyzers section: registered analyzers, current output, signal status.
- Violations view: which invariant, when opened, cumulative count, last channel before timeout.

---

## 6. Validation Strategy

### 6.1 Test Categories

1. **Contract tests (deterministic)** — Phase 0/2/3 channel completeness, invariant watchdog behavior (missing terminal → violation), config roundtrip, namespace enforcement, severity assignment.
2. **Integration tests** — Event ring drain cycle produces expected graph, compositor snapshots match expected frame structure, intent → frame correlation.
3. **Pane smoke tests** — Violations view renders violations, health summary computes from channel data, channel config editor persists changes.
4. **Regression probes** — Live-state checks for known bug conditions (e.g., compositor tile count diverges from graph node count). Ship ungated for field debugging.

### 6.2 CI Gates

Required checks for PRs touching:
- `registries/atomic/diagnostics.rs`
- `shell/desktop/runtime/diagnostics.rs`
- `desktop/tests/harness.rs`
- Diagnostics pane rendering code
- Any file registering new channels or invariants

### 6.3 What Runs In-Pane (Gated)

All `DiagnosticsRegistry` unit tests (create fresh local instances, zero global entanglement), channel registration contract tests, invariant watchdog behavior, config roundtrip, namespace enforcement, pure graph state machine tests via `TestHarness`.

**Does not run in-pane**: Tests requiring real webviews/GPU/network, tests conflicting with `GLOBAL_DIAGNOSTICS_REGISTRY`.

---

## 7. Degradation Policy

### 7.1 Diagnostics-Layer Degradation

The diagnostics subsystem itself can degrade:
- If the event ring overflows, oldest events are dropped with a counter increment (`diagnostics.selfcheck.event_ring_overflow`).
- If an analyzer panics, it is isolated and marked failed; other analyzers continue.
- If the pane is unavailable (feature gate), channels still emit; observability is just not visual.

### 7.2 Required Signals

All degradation states are observable through self-check channels. No silent drops.

---

## 8. Surface Capability Declarations

Diagnostics does not require per-surface capability declarations (it is the infrastructure that *consumes* capability declarations from other subsystems). However:

- Each mod-contributed surface should declare its diagnostic channel namespace in its `ModManifest.provides`.
- The diagnostics pane should show channel ownership by surface/mod.

---

## 9. Ownership Boundaries

| Owner | Guarantees |
|---|---|
| **`DiagnosticsRegistry` (ChannelRegistry)** | Channel schema, invariant contracts, namespace enforcement, severity assignment, config persistence |
| **`DiagnosticsState`** | Event ring, drain cycles, `DiagnosticGraph` aggregation, compositor snapshots, JSON export for tests |
| **`AnalyzerRegistry`** (future) | Analyzer lifecycle, continuous processing, derived signal production, pane section registration |
| **`TestRegistry`** (future, gated) | Test case registration, background execution, result streaming, panic isolation |
| **`TestHarness`** | Headless integration driver, thread-local event isolation, structured assertion surface |

---

## 10. Implementation Roadmap (Subsystem-Local)

1. **Fill invariant coverage** — Add watchdogs for mod load, identity sign, persistence open, viewer select, action execute. All have channel pairs already.
2. **Add `ChannelSeverity`** — One field on `DiagnosticChannelDescriptor`; unlocks health summary and pane prioritization.
3. **Surface violations in pane** — Violations are generated; build the dedicated view.
4. **Wire channel config editor** — Infrastructure exists (`set_channel_config_global`, `apply_persisted_channel_configs`); build UI.
5. **Startup structural verification as analyzer** — Runs at init, emits `startup.selfcheck.*` channels, no gate.
6. **AnalyzerRegistry** — Extensible analysis layer. Required for mod-contributed pane sections.
7. **TestRegistry** (`diagnostics_tests` feature) — Developer tool; smoke tests runnable from pane.
8. **Wire `retention_count`** — Connect `DiagnosticsState` event ring to per-channel retention from registry config.
9. **Orphan channel surface** — Track auto-registered unknown channels in a dedicated set.

---

## 11. Current Status & Gaps

**What exists**:
- Diagnostics channel registry/runtime/event ring are implemented and used broadly across the app.
- `ChannelSeverity` support and runtime descriptor helper constructors are now present, improving schema ergonomics and prioritization.
- Invariant watchdog infrastructure and self-check concepts exist in partial form.

**What's missing / open**:
- AnalyzerRegistry and TestRegistry are still planned, not implemented.
- Pane views for violations/health/analyzers/config editing remain incomplete.
- Invariant coverage is still thin relative to the number of `*_started -> *_{succeeded|failed}` workflows.

## 12. Pane-Level Gaps (from Research)

### Missing views
- **Violations** — Dedicated view: which invariant, when opened, cumulative count, last channel before timeout.
- **Health summary** — Per-subsystem green/yellow/red from failure/fallback channel ratios.
- **Invariant graph** — Start→terminal DAG showing pending/healthy/violated.
- **Active analyzers** — Once AnalyzerRegistry exists: registered analyzers, output, signal status.

### Missing controls
- **Channel config editor** — Toggle channels, adjust sample rates live.
- **Filter / search** — By prefix, owner source, state (ever fired vs. silent this session).

### Missing data presentation
- **Rate indicators** — Per-channel events/sec from `message_latency_recent_us`.
- **Intent → frame correlation** — Timeline linking intent batches to compositor state.

---

## 13. Dependencies / Blockers

- AnalyzerRegistry/TestRegistry sequencing depends on keeping diagnostics core runtime stable during other subsystem adoption.
- Pane UX expansion depends on control UI/pane architecture cleanup to avoid embedding more complexity in `render/mod.rs`.
- Cross-subsystem health summaries depend on other subsystems emitting consistent channels + severity semantics.

## 14. Linked Docs

- `../research/2026-02-24_diagnostics_research.md` (primary research basis)
- `2026-02-22_registry_layer_plan.md` (Phase 3 channel contracts and related infrastructure sequencing)
- `2026-02-22_test_harness_consolidation_plan.md` (harness/test execution integration)
- `2026-02-25_planning_register_backlog_and_copilot_guides.md` (cross-subsystem sequencing and backlog)

## 15. Done Definition

The diagnostics subsystem is fully operational when:

- All phase contract channels are present and verified at startup.
- Invariant watchdogs cover every `*_started → *_{succeeded|failed}` pair.
- `ChannelSeverity` is assigned to all channels.
- Violations are surfaced in the diagnostic pane with full context.
- AnalyzerRegistry is implemented and extensible by mods.
- TestRegistry is implemented (gated) with runnable-from-pane tests.
- Self-check channels are active and routing.
- Other subsystem channels (accessibility, security, storage, history) are emitted and visible.
