# Storage and Persistence Integrity Spec

**Date**: 2026-02-28  
**Status**: Canonical subsystem contract  
**Priority**: Immediate implementation guidance

**Related**:
- `SUBSYSTEM_STORAGE.md`
- `../workbench/2026-03-03_pane_opening_mode_and_simplification_suppressed_plan.md`
- `../system/2026-03-03_graphshell_address_scheme_implementation_plan.md`
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
- address-as-identity write semantics
- non-durable ephemeral pane-open behavior
- encryption completeness
- archive data integrity

---

## 2. Canonical Model

Persistence is the durable truth boundary for Graphshell state.

All durable graph mutations must flow through one enforceable write path, and
recovery must reconstruct valid state deterministically.

Address-as-identity corollary:

- durable graph citizenship changes are durable writes,
- ephemeral pane opens are not durable writes by themselves,
- address issuance and address persistence must not be conflated.

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

### 3.4A Address-as-identity write boundary

The canonical write path is also the canonical graph-citizenship boundary.

Rules:

- A pane becomes graph-backed only when its canonical address is written through the approved reducer/persistence path and resolves to a live node.
- Merely opening a pane, rendering a pane, or changing pane chrome must not create durable graph state.
- Internal `verso://` surfaces that are graph-owned at creation time still use the same canonical durable write path for enrollment; they are not exempt from single-write-path review.
- Delete/tombstone operations are the inverse durable boundary: they remove or tombstone the live resolution of the address through the same reviewable mutation path.

### 3.4B Ephemeral pane-open non-write behavior

Opening content in an ephemeral pane mode (`QuarterPane`, `HalfPane`, `FullPane`) is explicitly non-durable unless and until the pane is promoted into graph-backed state.

That means:

- no WAL entry is required solely because an ephemeral pane opened,
- no node record is created solely because an ephemeral pane opened,
- no snapshot delta is required solely because an ephemeral pane opened,
- closing an ephemeral pane produces no graph persistence mutation unless another explicit durable action occurred.

If an ephemeral pane later becomes graph-backed:

- the promotion/enrollment event is the first durable write point,
- address issuance may occur earlier in transient memory, but persistence authority begins only at canonical graph enrollment.

### 3.5 Encryption Completeness

- New durable writes must use the declared encryption path.
- Key provenance and magic-byte detection must remain explicit.
- Legacy migration must be observable and deterministic.

### 3.6 Archive Integrity

- Archive writes must be complete and append-only.
- Export must preserve fidelity and fail explicitly on error.

### 3.7 Restore and identity stability

- Persistence restore must reconstruct the same canonical address for an existing graph-backed internal surface identity.
- Restore must not mint a fresh canonical internal address for a previously persisted frame, tool instance, graph view, settings page, or clip node. Runtime canonical formatting is `verso://...`; legacy `graphshell://...` remains compatibility-only.
- Recovery logic must be able to distinguish "ephemeral pane re-opened" from "persisted graph-backed tile restored" so non-durable pane state is not mistaken for missing graph data.

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
- Ephemeral pane open/close behavior is explicitly non-durable until graph enrollment occurs.
- Address-as-identity write and restore semantics are explicit for internal `verso://` surfaces (with `graphshell://` treated only as a legacy alias during migration).

