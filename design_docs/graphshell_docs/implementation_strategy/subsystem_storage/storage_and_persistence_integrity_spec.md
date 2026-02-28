# Storage and Persistence Integrity Spec

**Date**: 2026-02-28  
**Status**: Canonical subsystem contract  
**Priority**: Immediate implementation guidance

**Related**:
- `SUBSYSTEM_STORAGE.md`
- `../subsystem_security/security_and_access_control_spec.md`
- `../subsystem_history/history_timeline_and_temporal_navigation_spec.md`

---

## 1. Purpose and Scope

This spec defines the canonical contract for the **Storage / Persistence subsystem**.

It governs:

- WAL journal integrity
- snapshot consistency
- serialization round-trip correctness
- single-write-path enforcement
- encryption completeness
- archive data integrity

---

## 2. Canonical Model

Persistence is the durable truth boundary for Graphshell state.

All durable graph mutations must flow through one enforceable write path, and
recovery must reconstruct valid state deterministically.

---

## 3. Normative Core

### 3.1 WAL Integrity

- Graph mutations must be journaled through the canonical journal path.
- Sequence order must be monotonic.
- Serialization failures are corruption events, not silent skips.

### 3.2 Snapshot Consistency

- Recovery state must equal latest valid snapshot plus subsequent journal entries.
- Snapshot writes must be atomic.
- Named snapshots must not corrupt or redefine the automatic snapshot path.

### 3.3 Serialization Round-Trip

- Graph, log entries, and layout payloads must round-trip faithfully.
- Backward-compatibility handling must be explicit.

### 3.4 Single Write Path

- Graph mutation authority must remain constrained to the intended reducer boundary.
- Durability must not depend on hidden side-effect paths.

### 3.5 Encryption Completeness

- New durable writes must use the declared encryption path.
- Key provenance and magic-byte detection must remain explicit.
- Legacy migration must be observable and deterministic.

### 3.6 Archive Integrity

- Archive writes must be complete and append-only.
- Export must preserve fidelity and fail explicitly on error.

---

## 4. Planned Extensions

- stronger automated corruption detection and reporting
- richer persistence health summaries
- clearer surface-level capability declarations for persistence support

---

## 5. Prospective Capabilities

- alternate storage backends with equivalent contract guarantees
- richer migration tooling for schema evolution
- subsystem-level persistence policy presets

---

## 6. Acceptance Criteria

- Core persistence invariants are explicit and tested or diagnosed.
- Recovery, migration, and encryption failures are never silent.
- Single-write-path enforcement remains reviewable and structurally obvious.

