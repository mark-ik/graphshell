# Unified Storage Architecture Plan

**Date**: 2026-03-08  
**Status**: Active consolidation plan  
**Purpose**: Reconcile the canonical Storage subsystem docs with the actual runtime persistence model and define the next architectural closure steps.

**Related**:
- `SUBSYSTEM_STORAGE.md`
- `storage_and_persistence_integrity_spec.md`
- `2026-03-11_graphstore_vs_client_storage_manager_note.md`
- `../subsystem_history/2026-03-08_unified_history_architecture_plan.md`
- `../subsystem_security/2026-03-08_unified_security_architecture_plan.md`
- `../system/2026-03-03_graphshell_address_scheme_implementation_plan.md`

---

## 1. Why This Plan Exists

The storage subsystem is more implemented than several peer subsystem guides imply:

- `GraphStore` already provides WAL, snapshots, encryption, migration, named graph snapshots, workspace layout persistence, and archive storage.
- startup open/recover diagnostics already exist.
- replay/archive persistence is already shared with the history subsystem.

The remaining problem is architectural clarity:

- the docs still flatten storage into one monolithic persistence story,
- they overstate "single write path" as if every in-memory graph mutation were durable,
- and they understate the runtime split between graph durability, workspace persistence, archive persistence, and recovery/degradation behavior.

This plan reorganizes the subsystem around the storage tracks that actually exist.

It is also explicitly scoped to the landed `GraphStore` side of the subsystem.
It does not treat future WHATWG-style browser-origin storage coordination as the
same problem. If Graphshell later adds a Servo-compatible `ClientStorageManager`,
that should be modeled as a parallel storage authority for site data rather than
as a re-description of Graphshell app durability.

---

## 2. Canonical Storage Tracks

### 2.1 GraphDurability

Durable graph state for graph-backed nodes and edges.

Owns:

- WAL journal entries for durable graph mutations
- automatic latest snapshot
- named graph snapshots
- recovery as `snapshot + replay`
- address-as-identity durable enrollment semantics

Primary implementation:

- `services/persistence/mod.rs`
- `services/persistence/types.rs`
- reducer/app mutation logging paths

### 2.2 WorkspaceLayoutPersistence

Durable workspace and UI layout payloads that are not the same thing as graph WAL state.

Owns:

- latest workspace layout payload
- named workspace layouts
- session autosave rotation/history
- persisted settings payloads currently stored through workspace-layout keys

Primary implementation:

- `app/persistence.rs`
- `services/persistence/mod.rs`
- `shell/desktop/ui/persistence_ops.rs`

### 2.3 ArchivePersistence

Append-only durable archives used by history and replay consumers.

Owns:

- traversal archive
- dissolved archive
- archive export/clear/curation
- timeline replay index support

Primary implementation:

- `services/persistence/mod.rs`
- history UI/runtime consumers

### 2.4 PersistenceRecoveryAndHealth

Startup/open/recover supervision, degradation handling, and integrity observability.

Owns:

- startup store-open timeout behavior
- recovery success/failure reporting
- corruption/decrypt/migration observability
- persistence health summary and degraded-mode state

Primary implementation today:

- `app/startup_persistence.rs`
- diagnostics registry/runtime

Missing closure:

- health summary
- explicit degradation modes
- complete integrity telemetry

---

## 3. Canonical Terminology Correction

### 3.1 "Single Write Path" means single durable write path

The subsystem must stop using "single write path" to mean "every graph mutation in memory."

Canonical meaning:

- all **durable graph-citizenship** mutations and other durable graph state transitions must flow through the approved reducer/persistence boundary,
- ephemeral/view/runtime mutations do not become durable merely because they touch graph-adjacent in-memory state,
- recovery only promises correctness for durable accepted state.

Examples of non-durable in-memory mutation classes that do not, by themselves, violate the storage contract:

- temporary selection/focus state
- purely spatial/view-local adjustments not treated as durable graph truth
- form-draft/runtime capture fields
- transient runtime metadata not declared durable

### 3.2 Durable mutation vs graph mutation

The storage subsystem must explicitly distinguish:

- `durable mutation`
- `ephemeral graph-adjacent mutation`
- `workspace/layout persistence write`
- `archive append`

These are related but not identical.

---

## 4. Landed vs Missing

### 4.1 Landed

- fjall WAL keyspaces for mutations and archives
- redb snapshot storage
- `rkyv` serialization and broad round-trip coverage
- zstd compression
- AES-256-GCM at-rest encryption with keychain-backed key material
- legacy plaintext migration
- named graph snapshots
- workspace layout persistence and autosave/history rotation
- traversal archive / dissolved archive append-export-clear-curation flows
- startup open timeout handling
- startup/recover diagnostics for open success/failure/timeout and recover success/failure

### 4.2 Partial

- single durable write-path enforcement is mostly present, but the docs are broader than the implementation contract
- archive integrity is present, but still split across storage/history docs
- recovery observability exists, but only for startup/open/recover, not the full integrity taxonomy

### 4.3 Missing

- continuity validation and sequence-gap detection on open/recovery
- persistence health summary
- explicit degraded persistence states
- full `persistence.*` integrity telemetry for journal writes, snapshots, decrypt failures, migration, and replay anomalies
- a canonical storage taxonomy in the subsystem guide

---

## 5. Architectural Corrections

### 5.1 Reframe the subsystem guide around the four tracks

`SUBSYSTEM_STORAGE.md` should no longer imply that storage is only graph WAL + snapshots.

It should explicitly model:

- `GraphDurability`
- `WorkspaceLayoutPersistence`
- `ArchivePersistence`
- `PersistenceRecoveryAndHealth`

### 5.2 Narrow the durability invariant language

Replace broad wording like:

- "Every graph mutation is journaled"
- "No code path outside `apply_reducer_intents()` modifies graph state"

with narrower language:

- every **durable graph mutation** is journaled through the canonical path,
- recovery reconstructs durable accepted state,
- ephemeral graph-adjacent state is outside WAL guarantees unless explicitly declared durable.

### 5.3 Make degradation first-class

The subsystem should define explicit runtime states:

- `Full`
- `DegradedReadOnly`
- `DegradedKeyUnavailable`
- `RecoveryFallback`
- `Unavailable`

These states should drive diagnostics and any user-visible persistence warnings.

### 5.4 Normalize storage/history boundary

Archive persistence is storage-owned infrastructure serving history-owned semantics.

That means:

- archive keyspace integrity belongs to Storage,
- interpretation of archive entries as timeline/history product state belongs to History,
- replay correctness across that boundary needs linked validation but separate ownership.

---

## 6. Sequenced Plan

### Phase 1 — Taxonomy Closure

Update `SUBSYSTEM_STORAGE.md` to:

- reflect landed startup/recover diagnostics,
- replace the old monolithic framing with the four-track model,
- narrow "single write path" to the durable boundary,
- record the remaining missing integrity/health work accurately.

Done gate:

- subsystem guide no longer claims storage is less implemented than it really is,
- subsystem guide no longer overclaims "all graph mutations" when the real contract is narrower.

### Phase 2 — Integrity Telemetry Closure

Add the missing diagnostics and health model:

- journal write success/failure
- snapshot success/failure
- migration detected/completed/failed
- decrypt failure / replay corruption surfacing
- persistence health snapshot in diagnostics inspector

Done gate:

- `persistence.*` diagnostics reflect both startup supervision and steady-state integrity behavior.

### Phase 3 — Recovery/Degradation Closure

Implement explicit persistence degradation states and recovery fallback reporting.

Done gate:

- open/recover/key failures map to explicit subsystem states rather than only warnings or `None` returns.

### Phase 4 — Durable Boundary Audit

Audit durable graph mutations against the reducer/app logging path and document intentional non-durable exceptions.

Done gate:

- the code and docs agree on which mutation classes are durable,
- the remaining direct in-memory mutation paths are either justified as non-durable or moved behind durable intent/logging flows.

---

## 7. Relation To Other Plans

- This plan depends on the unified History plan for clean archive/timeline ownership language, but it does not wait on history UI work.
- This plan overlaps with the unified Security plan on key provenance and crypto failure handling, but Storage remains the owner of persistence crypto execution.
- This plan should stay aligned with the servoshell debt-clear work because ephemeral pane open behavior must not accidentally become durable graph enrollment.

---

## 8. Acceptance Criteria

- The storage subsystem guide reflects the runtime as it exists today.
- The subsystem distinguishes durable graph truth from other persisted payload classes.
- Missing work is framed as observability/degradation/integrity closure, not as absence of a persistence subsystem.
- Archive persistence ownership is explicit and non-duplicative with History.
