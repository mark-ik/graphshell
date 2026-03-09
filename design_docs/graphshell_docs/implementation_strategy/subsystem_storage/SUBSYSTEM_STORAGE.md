# Cross-Cutting Subsystem: Persistence & Data Integrity

**Status**: Active / Project Goal
**Subsystem label**: `storage`
**Long form**: Persistence & Data Integrity Subsystem
**Scope**: WAL journal integrity, snapshot consistency, serialization round-trip correctness, single-write-path enforcement, at-rest encryption, and named-graph/workspace-layout data integrity — across all persistence paths
**Subsystem type**: Cross-Cutting Runtime Subsystem (see `TERMINOLOGY.md`)
**Peer subsystems**: `diagnostics` (Diagnostics), `accessibility` (Accessibility), `security` (Security & Access Control), `history` (Traversal & Temporal Integrity)
**Doc role**: Canonical subsystem implementation guide (summarizes guarantees/roadmap and links to detailed persistence specs/code references; avoid duplicate persistence design docs unless needed)
**Sources consolidated**:
- `2026-02-22_registry_layer_plan.md` Phase 6 (three-authority-domain boundary, single-write-path enforcement, `pub(crate)` boundary lock)
- `services/persistence/mod.rs` (GraphStore: fjall WAL + redb snapshots + rkyv serialization + zstd compression + AES-256-GCM encryption)
- `2026-03-08_unified_storage_architecture_plan.md` (storage track split and durability-boundary clarification)
- `archive_docs/` — historical persistence plans (superseded by this document)
**Related**: `SUBSYSTEM_SECURITY.md` §3.4 (cryptographic correctness invariants overlap)

**Policy authority**: This file is the single canonical policy authority for the Storage subsystem.
Supporting storage docs may refine contracts, interfaces, and execution details, but must defer policy authority to this file.
Policy in this file should be distilled from canonical specs and accepted research conclusions.

**Adopted standards** (see [2026-03-04_standards_alignment_report.md](../../research/2026-03-04_standards_alignment_report.md) for full rationale):
- **RFC 3986** — URI syntax for internal scheme parsing (`parser.rs`); all internal address tokens must be RFC 3986-valid.
- **RFC 4122 UUID v4** — node identity (`NodeId`); stable across sessions, no ordering semantics.
- **RFC 4122 UUID v7** — WAL journal entry tokens only; time-ordered sequencing. Must not be used for `NodeId`.
- **XDG Base Directory Specification** (via `directories` crate) — canonical storage path semantics across platforms. Data → `XDG_DATA_HOME`, config → `XDG_CONFIG_HOME`, cache → `XDG_CACHE_HOME`.
- **FIPS 197 / NIST SP 800-38D** — AES-256-GCM at-rest encryption. 256-bit key, 12-byte nonce, 16-byte GCM tag. Nonce never reused.

**Referenced as prior art** (no conformance obligation):
- **OAIS (ISO 14721)** — SIP/AIP/DIP vocabulary informs export/archive design. Not adopted due to disproportionate fixity/format-migration obligations.
- **RFC 6902 JSON Patch** — not adopted for local undo/redo (conflicts with `NodeIndex` instability); see standards report §4.3.

---

## 0A. Subsystem Policies

1. **Single-write-path policy**: Durable graph mutation must flow through canonical write paths; side-channel persistence writes are disallowed.
2. **WAL-first integrity policy**: Journal and snapshot paths must remain consistent and recoverable under interruption/failure.
3. **Roundtrip-safety policy**: Serialization/deserialization and schema evolution must preserve state or degrade explicitly.
4. **Encryption-completeness policy**: Sensitive persistence keyspaces require mandated cryptographic handling with explicit failure behavior.
5. **Recovery-observability policy**: Recovery/snapshot corruption, fallback, and repair paths must be diagnosable and test-backed.

---

## 1. Why This Exists

The persistence layer is the single point where **durable graph state transitions become durable**. Every cold start depends on `GraphStore.recover()`, and every durable graph mutation depends on the canonical journal/snapshot path. A silent corruption in either path is an unrecoverable data loss event.

The dominant failure mode is **silent contract erosion**: a new serialization type is added without a round-trip test, a snapshot path writes unencrypted data, a new keyspace bypasses the WAL journal, a durable mutation path escapes the approved reducer/persistence boundary, or recovery silently skips corrupted entries without surfacing degraded state. None of these produce immediate errors. All produce data loss or integrity failure on the next recovery.

Without subsystem-level treatment, every change to `Graph`, every new `LogEntry` variant, every new persistence keyspace, every new named-snapshot path, and every new persisted workspace/settings payload becomes an unaudited integrity boundary crossing.

---

## 2. Subsystem Model (Four Layers)

| Layer | Persistence Instantiation |
|---|---|
| **Contracts** | WAL integrity, snapshot consistency, serialization round-trip, single-write-path, encryption completeness, archive integrity — §3 |
| **Runtime State** | `GraphStore` (fjall WAL, redb snapshots, AES-256-GCM encryption, zstd compression); `GraphWorkspace` (single-write-path boundary via `pub(crate)`) |
| **Diagnostics** | `persistence.*` channel family — §5 |
| **Validation** | Round-trip tests, boundary contract tests, snapshot/recovery tests, encryption verification — §6 |

---

## 2A. Canonical Storage Tracks

The subsystem is not just "graph WAL + snapshots." It currently spans four related storage tracks:

1. **GraphDurability** — durable graph WAL, latest snapshot, named graph snapshots, graph recovery
2. **WorkspaceLayoutPersistence** — workspace layouts, session autosave rotation, persisted settings payloads currently stored through layout keys
3. **ArchivePersistence** — traversal archive, dissolved archive, export/clear/curation, replay support inputs
4. **PersistenceRecoveryAndHealth** — startup open/recover supervision, timeout fallback, degradation/health observability

The unified storage plan is the canonical staging document for closing the gaps between those tracks and the current guide language.

---

## 3. Required Invariants / Contracts

### 3.1 WAL Journal Integrity

1. **Complete journaling of durable graph mutations** — Every durable graph mutation that enters the approved reducer/app persistence path is journaled to fjall via `log_mutation()`. No durable mutation path bypasses the journal.
2. **Sequence monotonicity** — `log_sequence` is monotonically increasing. No gaps, no reuse, no reset. A gap indicates corruption or truncation.
3. **Serialization fidelity** — `rkyv::to_bytes(entry)` → fjall → `rkyv::from_bytes(stored)` produces a bitwise-identical `LogEntry`. Deserialization failure on any stored entry is a corruption/degradation event, not an invisible contract success.
4. **Keyspace isolation** — The three fjall keyspaces (`mutations`, `traversal_archive`, `dissolved_archive`) are independent. A corruption in one does not affect the others.
5. **Archive append-only** — `archive_append_traversal()` and `archive_dissolved_traversal()` are append-only. Entries are never modified after write.

### 3.2 Snapshot Consistency

1. **Snapshot-journal coherence** — On recovery, the graph state is: latest snapshot + all journal entries after it. The snapshot and journal together must produce a valid graph state identical to what was in memory before shutdown.
2. **Snapshot atomicity** — `take_snapshot()` is an atomic redb write transaction. A crash during snapshot does not corrupt the snapshot DB. The previous snapshot remains valid.
3. **Periodic snapshot guarantee** — `check_periodic_snapshot()` fires at `snapshot_interval` intervals. The interval is configurable but never zero.
4. **Named snapshot isolation** — Named graph snapshots (`save_named_graph_snapshot`) are independent of the automatic snapshot. Saving/loading a named snapshot does not affect the automatic snapshot or the WAL sequence.

### 3.3 Serialization Round-Trip

1. **Graph round-trip** — For any `Graph` value `g`, `deserialize(serialize(g)) == g`. This must hold for all node types, edge types, metadata, and workspace membership.
2. **LogEntry round-trip** — For any `LogEntry` value `e`, `rkyv::from_bytes(rkyv::to_bytes(e)) == e`. This must hold for all `GraphIntent` variants.
3. **Tile layout round-trip** — `load_tile_layout_json(save_tile_layout_json(json)) == json`. JSON fidelity is preserved.
4. **Workspace layout round-trip** — `load_workspace_layout_json(save_workspace_layout_json(name, json)) == json`. Named workspace layouts round-trip with name-key fidelity.
5. **Backward compatibility** — Legacy plaintext payloads (pre-encryption migration) are still readable. `decode_persisted_bytes()` detects the absence of `GSEV0001` magic and falls back to plaintext decode.

### 3.4 Single-Write-Path Enforcement

1. **`pub(crate)` boundary** — Graph topology mutators in `graph/mod.rs` are `pub(crate)`. No external crate can call `graph.add_node()` directly.
2. **Durable-boundary exclusivity** — All durable graph mutations flow through the approved reducer/app persistence boundary. The subsystem does not require every ephemeral or view-local in-memory graph-adjacent mutation to be journaled.
3. **Three-authority domains** — As defined in the registry layer plan Phase 6:
   - Semantic graph: owned by `GraphWorkspace`, mutated only via `apply_reducer_intents()`
   - Spatial layout: owned by `Tree<TileKind>` inside `GraphWorkspace`, driven by intents
   - Runtime instances: owned by `AppServices`, reconciled via `lifecycle_reconcile.rs`
4. **Compiler enforcement** — The `pub(crate)` visibility restriction is a compile-time guarantee. Violation requires explicit `pub` escalation, which is reviewable.

Clarification:

- durable graph truth and graph citizenship are inside the storage contract,
- ephemeral graph-adjacent state such as transient view/layout/form-runtime state is not automatically part of WAL guarantees unless explicitly declared durable.

### 3.5 Encryption Completeness

1. **Default encryption** — All new data written to fjall or redb passes through `encode_persisted_bytes()` which applies zstd compression then AES-256-GCM encryption. No path writes plaintext.
2. **Magic-byte detection** — `GSEV0001` magic prefix distinguishes encrypted from legacy plaintext payloads. Every encrypted payload starts with this prefix + 12-byte nonce + ciphertext.
3. **Key provenance** — The AES-256-GCM key is loaded from the OS keychain (`keyring` crate) or generated and stored there on first use. The key never appears in logs or diagnostic output.
4. **Nonce freshness** — Each `encode_persisted_bytes()` call generates a fresh 12-byte random nonce via `OsRng`. Nonces are never reused (see Security subsystem §3.4).
5. **Legacy migration** — `has_legacy_plaintext_data()` detects unencrypted data on open. `migrate_legacy_plaintext_data()` re-encodes it in place. After migration, no plaintext remains.

### 3.6 Archive Integrity

1. **Traversal archive completeness** — Every dissolved traversal has its state journaled to `traversal_archive_keyspace` before the dissolve mutation is applied.
2. **Dissolved archive completeness** — Every dissolve operation journals what was removed to `dissolved_archive_keyspace`.
3. **Export fidelity** — `export_traversal_archive()` and `export_dissolved_archive()` produce valid String representations of all archive entries. No entry is silently skipped.

---

## 4. Surface Capability Declarations (Folded Approach)

Persistence capability declarations are folded into the relevant registry entries:

### 4.1 Viewer/Surface Persistence Capabilities

Each viewer/surface declares:

```
state_persistence: full | partial | none     // Can this surface's state be saved/restored?
undo_support: full | partial | none          // Does this surface support undo/redo?
export_support: full | partial | none        // Can content be exported?
notes: String
```

### 4.2 Storage Backend Capabilities

`GraphStore` itself declares:

```
journal_backend: fjall (append-only log)
snapshot_backend: redb (ACID transactions)
serialization: rkyv (zero-copy)
compression: zstd (level 3)
encryption: AES-256-GCM (OS keychain key)
```

These are not runtime-configurable but are documented for diagnostics and capability introspection.

#### Why fjall for the WAL

`fjall` was selected as the WAL journal backend for the following reasons:

- **Append-only log semantics**: fjall is a log-structured storage engine with explicit keyspace isolation. It makes the three-domain separation (`mutations`, `traversal_archive`, `dissolved_archive`) a first-class storage concept, not an application-level convention.
- **Failure guarantees**: fjall uses a crash-safe log-structured merge tree (LSM); partial writes are recoverable. This matches the WAL requirement that a sequence gap is detectable corruption, not a silent silent data loss.
- **Pure Rust**: no C FFI, no system library dependency, no WASM concern (fjall is host-only by design — it links against the OS filesystem, which is appropriate for a desktop WAL backend).
- **Upgrade story**: fjall exposes a versioned keyspace API. Schema migration is additive keyspace extension; old keyspaces remain readable during migration windows.
- **WASM-clean boundary**: fjall stays entirely in the host crate (`graphshell-desktop` or equivalent). `graphshell-core` never imports fjall; the WAL log entry types are WASM-clean structs that the host serializes into fjall. This matches the core/host split in `../../technical_architecture/2026-03-08_graphshell_core_extraction_plan.md §2.5`.

---

## 5. Diagnostics Integration

### 5.1 Required Diagnostic Channels

| Channel | Severity | Description |
|---|---|---|
| `persistence.store.opened` | Info | GraphStore successfully opened |
| `persistence.store.open_failed` | Error | GraphStore failed to open |
| `persistence.key.loaded` | Info | Persistence key loaded from keychain |
| `persistence.key.generated` | Info | New persistence key generated (first launch) |
| `persistence.key.unavailable` | Error | Keychain access failed |
| `persistence.journal.entry_written` | Info | Log entry successfully journaled |
| `persistence.journal.write_failed` | Error | Journal write failed |
| `persistence.journal.sequence_gap` | Error | Gap detected in log sequence numbers |
| `persistence.snapshot.taken` | Info | Periodic snapshot completed |
| `persistence.snapshot.failed` | Error | Snapshot write failed |
| `persistence.snapshot.named_saved` | Info | Named graph snapshot saved |
| `persistence.snapshot.named_loaded` | Info | Named graph snapshot loaded |
| `persistence.recovery.started` | Info | Recovery from snapshot+journal started |
| `persistence.recovery.succeeded` | Info | Recovery completed successfully |
| `persistence.recovery.failed` | Error | Recovery failed |
| `persistence.recovery.journal_replay_count` | Info | Number of journal entries replayed |
| `persistence.encryption.legacy_detected` | Warn | Legacy plaintext data detected |
| `persistence.encryption.migration_complete` | Info | Legacy data migrated to encrypted format |
| `persistence.encryption.decrypt_failed` | Error | AES-GCM decryption or tag verification failed |
| `persistence.serialization.roundtrip_failed` | Error | rkyv round-trip mismatch detected |
| `persistence.archive.traversal_appended` | Info | Entry added to traversal archive |
| `persistence.archive.dissolved_appended` | Info | Entry added to dissolved archive |

Current implementation note:

- startup/open/recover coverage already exists in runtime diagnostics under `startup.persistence.*` and `persistence.recover.*`,
- the remaining work is steady-state integrity telemetry and a unified persistence health surface,
- this guide no longer treats the diagnostics story as entirely missing.

### 5.2 Persistence Health Summary (Diagnostic Inspector)

- Store status: `active` / `degraded (key unavailable)` / `failed`
- Journal: entry count, last write timestamp, sequence continuity check
- Snapshot: last snapshot timestamp, snapshot interval, named snapshot count
- Encryption: `encrypted` / `legacy-migration-pending` / `key-unavailable`
- Archive: traversal archive size, dissolved archive size
- Recovery: last recovery status, replay count

### 5.3 Invariant Watchdogs

Required watchdog invariants (start → terminal pairs):
- `persistence.store.open_started` → `opened | open_failed` (5000ms)
- `persistence.recovery.started` → `recovery.succeeded | recovery.failed` (30000ms)
- `persistence.snapshot.started` → `snapshot.taken | snapshot.failed` (10000ms)
- `persistence.encryption.migration_started` → `migration_complete | migration_failed` (60000ms)

---

## 6. Validation Strategy

### 6.1 Test Categories

1. **Round-trip tests (deterministic)** — For every serializable type (`Graph`, `LogEntry`, `GraphSnapshot`, `TileLayout`, `WorkspaceLayout`): serialize → deserialize → assert equality. For every new `GraphIntent` variant: serialize → deserialize → assert equality. These are the **core contract tests**.
2. **WAL integrity tests** — Open store, write N entries, close, reopen, verify sequence continuity and entry fidelity.
3. **Snapshot/recovery tests** — Populate graph, snapshot, add more mutations, recover → assert graph equals expected state.
4. **Encryption tests** — Verify `encode_persisted_bytes` → `decode_persisted_bytes` round-trip. Verify corrupted ciphertext produces error (not silent truncation). Verify legacy plaintext fallback works. Verify nonce uniqueness across multiple calls.
5. **Boundary tests** — Attempt graph mutation from outside `apply_reducer_intents()`: verify compilation failure (`pub(crate)` boundary) or runtime rejection.
6. **Named snapshot tests** — Save, list, load, delete named snapshots. Verify named snapshot isolation (saving named doesn't affect automatic).
7. **Archive tests** — Append to traversal/dissolved archives, verify export, verify clear, verify recent-entries query.

### 6.2 CI Gates

Required checks for PRs touching:
- `services/persistence/` — Full persistence test suite.
- `services/persistence/types.rs` — Round-trip tests for any new/modified type.
- `graph/mod.rs` — Boundary enforcement (no new `pub` escalation without justification).
- Any file adding new `GraphIntent` variants — Must include `LogEntry` round-trip test.

### 6.3 Regression Guard

New serialization types or intent variants that lack round-trip tests are blocked by CI. This prevents the most common persistence regression: a type that serializes correctly today but deserializes incorrectly after a schema change.

---

## 7. Degradation Policy

### 7.1 Required States

- **Full**: Keychain available, encryption active, journal active, snapshots on schedule.
- **Degraded (read-only)**: If journal write fails (disk full, permission denied), app enters read-only mode. Graph can be browsed but not mutated. Explicit diagnostics emitted.
- **Degraded (no encryption)**: If keychain is unavailable but legacy data exists, data is accessible but new writes are blocked until key is available. No silent fallback to plaintext writes.
- **Recovery mode**: On startup, if snapshot is corrupted, attempt journal-only recovery. If journal is also corrupted, start with empty graph and emit critical diagnostic.

Current implementation note:

- startup timeout fallback and open/recover failure handling exist today,
- the explicit degraded-state model and health summary remain partial and are tracked by the unified storage plan.

### 7.2 Required Signals

- Degradation states emit to `persistence.*` channels.
- Diagnostic Inspector reflects persistence status prominently.
- User-visible indicators for: read-only mode, key unavailable, recovery failure.
- No silent data loss. Every unrecoverable corruption event produces an Error-severity diagnostic.

---

## 8. Ownership Boundaries

| Owner | Guarantees |
|---|---|
| **`GraphStore`** | Journal writes, snapshot atomicity, encryption, archive management. The single persistence authority. |
| **`GraphWorkspace`** | Single-write-path boundary. All mutations through `apply_reducer_intents()`. `pub(crate)` enforcement. |
| **`AppServices`** | Holds `GraphStore` handle. No other component has direct persistence access. |
| **Serialization types** (`types.rs`) | `GraphSnapshot`, `LogEntry`, `TileLayout` definitions. Round-trip correctness is their contract. |
| **OS Keychain** | Key storage. Persistence layer trusts but verifies (key format validation on load). |

---

## 9. Implementation Roadmap (Subsystem-Local)

Current priority order:

1. close the storage taxonomy gap (`GraphDurability`, `WorkspaceLayoutPersistence`, `ArchivePersistence`, `PersistenceRecoveryAndHealth`)
2. add missing integrity telemetry and persistence health summary
3. formalize degraded persistence states
4. audit durable mutation boundaries against actual app logging paths and document intentional non-durable exceptions

1. **Wire diagnostic channels** — Add `persistence.*` channel family to `DiagnosticsRegistry`. Emit from all `GraphStore` methods (open, journal, snapshot, recover, encrypt/decrypt).
2. **Add round-trip test coverage** — Verify every serializable type has an explicit round-trip test. Audit all `GraphIntent` variants for `LogEntry` coverage.
3. **Add recovery integrity test** — Populate graph, snapshot, add mutations, corrupt snapshot, verify journal-only recovery produces correct state.
4. **Add encryption edge-case tests** — Corrupted ciphertext → error (not silent), corrupted nonce → error, empty payload → error, legacy plaintext → fallback.
5. **Wire health summary** — Expose journal count, snapshot schedule, encryption status, archive sizes in Diagnostic Inspector.
6. **Add sequence continuity watchdog** — On open, verify `log_sequence` has no gaps. Emit `persistence.journal.sequence_gap` if gaps detected.
7. **Document degradation states** — Wire read-only mode on journal failure. Wire blocked-write on key unavailability.

---

## 10. Current Status & Gaps

Based on the existing `services/persistence/mod.rs` (2340 lines):

**What exists**:
- fjall WAL with three keyspaces (mutations, traversal_archive, dissolved_archive) ✅
- redb snapshot with ACID transactions ✅
- rkyv serialization ✅
- zstd compression ✅
- AES-256-GCM encryption with OS keychain key ✅
- Legacy plaintext migration ✅
- Named graph snapshots ✅
- Workspace layout persistence ✅
- `pub(crate)` boundary on graph mutators ✅

**What's missing**:
- No `persistence.*` diagnostic channels (errors are `log::warn` only)
- No explicit round-trip tests for all `GraphIntent` variants
- No sequence continuity validation on open
- No degradation-mode handling (read-only on failure)
- No persistence health summary in diagnostics pane
- No invariant watchdogs for long-running operations

---

## 11. Dependencies / Blockers

- Some degradation-mode behavior (read-only transitions) requires app-level UX/state wiring beyond `GraphStore`.
- History subsystem replay/preview work depends on persistence archive and WAL guarantees being diagnosable first.
- Security subsystem overlaps on cryptographic correctness and keychain behavior; shared diagnostics naming/severity should stay aligned.

## 12. Linked Docs

- `2026-02-22_registry_layer_plan.md` (Phase 6 single-write-path and authority boundary contracts)
- `services/persistence/mod.rs` (current implementation reference)
- `SUBSYSTEM_SECURITY.md` (crypto/keychain overlap)
- `SUBSYSTEM_HISTORY.md` (archive/replay temporal integrity dependencies)
- `SUBSYSTEM_DIAGNOSTICS.md` (diagnostics infrastructure for persistence health)
- `PLANNING_REGISTER.md` (cross-subsystem sequencing and priorities)

## 13. Done Definition

Persistence is a guaranteed system property when:

- Every graph mutation flows through `apply_reducer_intents()` → `log_mutation()` with no bypass paths.
- Every serializable type has an explicit round-trip test.
- Snapshot + journal recovery produces bit-identical graph state.
- All encryption paths are tested (including corruption detection and legacy fallback).
- `persistence.*` diagnostic channels cover all operations with appropriate severity.
- Sequence continuity is validated on open.
- Degradation modes (read-only, key-unavailable) are wired and tested.
- New intent variants without round-trip tests are blocked by CI.
- The single-write-path boundary (`pub(crate)`) is maintained and reviewed.
