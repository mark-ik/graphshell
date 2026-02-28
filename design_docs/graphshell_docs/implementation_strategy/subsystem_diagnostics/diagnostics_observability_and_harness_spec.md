# Diagnostics Observability and Harness Spec

**Date**: 2026-02-28  
**Status**: Canonical subsystem contract  
**Priority**: Immediate implementation guidance

**Related**:
- `SUBSYSTEM_DIAGNOSTICS.md`
- `../subsystem_accessibility/accessibility_interaction_and_capability_spec.md`
- `../subsystem_history/history_timeline_and_temporal_navigation_spec.md`

---

## 1. Purpose and Scope

This spec defines the canonical contract for the **Diagnostics subsystem**.

It governs:

- channel schema integrity
- invariant watchdog behavior
- analyzer and harness boundaries
- retention and observability policy
- diagnostics UI readiness as a consumer of runtime signals

---

## 2. Canonical Model

The Diagnostics subsystem has three distinct runtime roles:

1. channel schema registry
2. continuous analyzers
3. on-demand test harness execution

These roles must remain explicit and not collapse into one undifferentiated debug system.

---

## 3. Normative Core

### 3.1 Channel Contracts

- All emitted channels must have a declared schema and severity.
- Unknown channels may be tolerated for robustness, but never silently.
- Phase-required channels must be present at startup.

### 3.2 Invariant Watchdogs

- Started operations that require termination signals must have watchdog coverage.
- Violations must enter the event ring.
- Pending invariants must be visible.

### 3.3 Analyzer vs Harness Separation

- Analyzers are live, continuous observers.
- Harness tests are isolated, synthetic, and explicitly invoked.
- Probes with side effects are forbidden.

### 3.4 Retention and Configuration

- Per-channel retention must be respected.
- Runtime config changes must round-trip correctly.
- Diagnostics state must remain inspectable without mutating semantic subsystem state.

---

## 4. Planned Extensions

- richer diagnostics summaries by subsystem
- in-pane grouped counters and recent-history panels
- stronger invariant coverage across major mutation boundaries

---

## 5. Prospective Capabilities

- mod-contributed analyzers with richer policy hooks
- subsystem health scoring
- automated remediation guidance for common failures

---

## 6. Acceptance Criteria

- Core channels and severities are declared.
- Invariant violations are observable.
- Analyzer and harness boundaries are enforced.
- Diagnostics can summarize subsystem degradation without becoming a mutation authority.

