# Cross-Cutting Subsystem: History

**Status**: Active / Project Goal
**Subsystem label**: `history`
**Long form**: Traversal & Temporal Integrity Subsystem
**Scope**: Traversal capture correctness, timeline/history integrity, replay/preview isolation, archive fidelity, and temporal restoration semantics ("return to present")
**Subsystem type**: Cross-Cutting Runtime Subsystem (see `TERMINOLOGY.md`)
**Peer subsystems**: `diagnostics` (Diagnostics), `accessibility` (Accessibility), `security` (Security & Access Control), `storage` (Persistence & Data Integrity)
**Doc role**: Canonical subsystem implementation guide (summarizes guarantees/roadmap and links to detailed history/traversal plans as they evolve)
**Sources consolidated**:
- `2026-02-20_edge_traversal_impl_plan.md` (Stages A-F, especially Stage E/F)
- `2026-02-20_edge_traversal_model_research.md` (temporal model assumptions)
- `2026-03-08_unified_history_architecture_plan.md` (top-level history taxonomy and sequencing)
- `PLANNING_REGISTER.md` (temporal navigation adoption + Stage F append notes)
**Related**: `SUBSYSTEM_STORAGE.md` (archive/WAL correctness), `SUBSYSTEM_DIAGNOSTICS.md` (timeline observability), `../../../verse_docs/implementation_strategy/lineage_dag_spec.md`, `../../../verse_docs/implementation_strategy/2026-03-09_agent_wal_and_distillery_architecture_plan.md`
`../canvas/2026-03-14_graph_relation_families.md` (Traversal family + Navigator "Recent" projection)

**Policy authority**: This file is the single canonical policy authority for the History subsystem.
Supporting history docs may refine contracts, interfaces, and execution details, but must defer policy authority to this file.
Policy in this file should be distilled from canonical specs and accepted research conclusions.

**Adopted standards** (see [2026-03-04_standards_alignment_report.md](../../research/2026-03-04_standards_alignment_report.md) §3.6):
- **OpenTelemetry Semantic Conventions** — `history.*` and `traversal.*` diagnostic channel naming and severity

---

## 0A. Subsystem Policies

1. **Temporal-integrity policy**: Traversal capture ordering and archive transfer correctness are mandatory invariants.
2. **Replay-isolation policy**: Preview/replay paths must not mutate live graph truth or write unintended WAL entries.
3. **Restoration policy**: Returning from temporal preview to present state must be deterministic and lossless.
4. **Archive-fidelity policy**: Dissolved/archived traversal state must preserve identity and reconstructable timeline semantics.
5. **Temporal-observability policy**: Timeline append/replay/restore failures must surface via explicit diagnostics and tests.
6. **Separate-authorities policy**: History may share traversal semantics with `AWAL` and lineage DAGs, but it remains the authority only for graph temporal truth.
7. **Shared-projection policy**: History owns traversal truth; Navigator, settings,
   diagnostics, and workbench chrome may project or summarize that truth, but
   must not define an independent recent-history structure.

---

## 1. Why This Exists

History in Graphshell is not just a pane. It is a temporal truth system spanning:

- traversal capture (`Traversal` records on edges),
- archive storage (active/dissolved traversal keyspaces),
- history presentation (History Manager timeline + dissolved views),
- and future temporal replay/preview (Stage F).

The dominant failure mode is **silent temporal integrity erosion**: traversal append order becomes incorrect, preview mode mutates live state, replay writes to WAL, dissolved/archive transfers lose entries, or "return to present" leaks preview state into the live graph.

Without subsystem-level treatment, every traversal/UI/persistence change becomes an unaudited temporal correctness boundary crossing.

History also now sits next to two adjacent append-only traversal-capable systems:

- `AWAL`, which owns agent temporal truth
- lineage DAGs, which own provenance truth for engrams and FLora checkpoints

The subsystem must make that family resemblance clear without collapsing the
three systems into one storage authority.

The subsystem also consumes the compositor's `compositor:tile_activity` signal
as a read-only runtime hint. That signal is not traversal truth and does not
write archive state on its own; it is used to annotate History Manager rows
with recent "node is currently alive/being-interacted-with" evidence without
introducing a second polling path into the viewer runtime.

History is also the canonical source for traversal-family projection into the
Navigator's `Recent` section. That sidebar section is a read-only projection
over history truth, not a second recents store.

---

## 2. Subsystem Model (Four Layers)

| Layer | History Instantiation |
|---|---|
| **Contracts** | Traversal capture correctness, replay isolation, temporal restoration, archive completeness — §3 |
| **Runtime State** | History Manager state, traversal archives, preview-mode state (future), replay cursor/index (future) |
| **Diagnostics** | `history.*`, `traversal.*`, and replay-preview channels surfaced via diagnostics subsystem — §5 |
| **Validation** | Traversal correctness tests, archive integrity tests, replay isolation tests, timeline/preview scenarios — §6 |

---

## 3. Required Invariants / Contracts

### 3.1 Traversal Capture Integrity

1. **Correct prior/current ordering** — URL and edge traversal append paths record prior/current endpoints in the correct order. No inversion on `WebViewUrlChanged` or equivalent lifecycle events.
2. **No silent traversal loss** — A traversal event is either appended or rejected with explicit diagnostics; it is never silently dropped.
3. **Timestamp monotonicity (per source)** — Traversal timestamps are monotonic per emitting source/session, or explicit skew handling is applied and logged.
4. **Edge association correctness** — Traversals are attached to the correct edge(s) derived from the navigation event.

### 3.2 Archive Integrity

1. **Dissolution completeness** — Traversals removed from active history are written to dissolved/traversal archives before removal from active state.
2. **Archive append-only semantics** — Archived traversal records are append-only; edits are represented as new records, not mutation in place.
3. **Export fidelity** — History exports include all qualifying entries; no silent skip on malformed entries (error diagnostics instead).

### 3.3 Temporal Replay / Preview Isolation (Stage F)

1. **No WAL writes in preview** — Timeline preview/replay never appends to the live WAL.
2. **No webview lifecycle mutations in preview** — Preview replay cannot create/destroy/navigate live webviews.
3. **No live graph mutations in preview** — Replay operates on detached preview state only.
4. **No persistence side effects in preview** — Snapshot writes, archival passes, and persistence mutations are suppressed.
5. **Clean return to present** — Exiting preview restores live state exactly (no preview leakage).

### 3.4 Temporal Restoration Semantics

1. **Focus/selection restoration** — Exiting Room/Map preview/history navigation restores focus/selection per the documented policy.
2. **Timeline cursor validity** — Cursor/index references remain valid across archive compaction or emit explicit invalidation with fallback behavior.

### 3.5 Shared Traversal Semantics

History traversal is structurally similar to traversal in `AWAL` and lineage
DAG systems:

- all can use cursor-based walks
- all can use cutoff/filter/replay-like policies
- all can reconstruct a selected state from append-only records

But the truth source differs:

- History = graph/content state over time
- `AWAL` = agent observation/action state over time
- lineage DAG = provenance/derivation state over ancestry

The shared primitive is traversal semantics, not one unified history store.

### 3.6 Boundary Event Requirement

When node-derived activity is distilled or promoted into intelligence
artifacts, the operation should produce linked records across systems:

- a history-side audit event
- one or more `AWAL` entries
- one or more lineage-DAG references

History owns only the history-side audit/provenance event, not the downstream
agent or lineage truth.

### 3.7 Compositor Activity Signal Contract

History may consume `compositor:tile_activity` as a supplemental runtime signal.

Rules:

1. The compositor remains the authority for tile-activity emission.
2. History remains the authority for traversal/archive truth.
3. History reads frame-level aggregated summaries, not per-tile per-frame
   streams, from the bounded ring buffer.
4. Activity signals may annotate History Manager rows and future temporal UI,
   but they must not silently create, delete, or reorder traversal records.
5. The ring buffer for this signal is bounded (256 frames) and treated as a
   best-effort recent-activity hint, not durable history.

---

## 4. Surface Capability Declarations (Folded Approach)

History capability declarations are folded into relevant registry/surface entries:

### 4.1 Viewer/Surface History Capabilities

Each viewer/surface may declare:

```
traversal_capture: full | partial | none
timeline_navigation: full | partial | none
preview_mode: full | partial | none
archive_export: full | partial | none
notes: String
```

Examples:
- Graph canvas: `timeline_navigation` + future `preview_mode`
- Web viewers: `traversal_capture`
- History Manager tool pane: `timeline_navigation`, `archive_export`, recent
   compositor activity annotation
- Navigator / Workbench Sidebar: read-only traversal projection (`Recent`
  section) sourced from history truth rather than a local recents cache

---

## 5. Diagnostics Integration

### 5.1 Required Diagnostic Channels

| Channel | Severity | Description |
|---|---|---|
| `history.traversal.recorded` | Info | Traversal appended successfully |
| `history.traversal.record_failed` | Error | Traversal append failed |
| `history.traversal.ordering_corrected` | Warn | Ordering anomaly detected and corrected |
| `history.archive.dissolved_appended` | Info | Dissolved traversal archived |
| `history.archive.export_failed` | Error | Archive export failed |
| `history.timeline.preview_entered` | Info | Timeline preview mode entered |
| `history.timeline.preview_exited` | Info | Timeline preview mode exited |
| `history.timeline.preview_isolation_violation` | Error | Preview attempted a forbidden live side effect |
| `history.timeline.replay_started` | Info | Replay operation started |
| `history.timeline.replay_succeeded` | Info | Replay operation succeeded |
| `history.timeline.replay_failed` | Error | Replay operation failed |
| `history.timeline.return_to_present_failed` | Error | Failed to restore live state after preview |
| `compositor:tile_activity` | Info | Recent active/idle tile summaries consumed by History Manager as a supplemental runtime hint |

### 5.2 History Health Summary (Subsystem/Diagnostics Pane)

- Traversal capture status (`active` / `degraded`)
- Recent traversal append failure count
- Archive sizes (active/dissolved)
- Preview mode status (`off` / `active`)
- Replay isolation status (last violation / none)
- Last return-to-present result

Additional projection rule:

- The same history-owned aggregates that power subsystem health may also drive
  Navigator `Recent` section counts/badges. UI surfaces should reuse those
  aggregates rather than recomputing their own recency models from raw viewer
  state.

---

## 6. Validation Strategy

### 6.1 Test Categories

1. **Traversal correctness tests**
- Prior/current URL ordering
- Edge association correctness
- traversal append on navigation lifecycle events

2. **Archive integrity tests**
- dissolved traversal archival before removal
- export completeness
- clear/list/recent query correctness

3. **Replay/preview isolation tests (Stage F)**
- preview mode does not write WAL
- preview mode does not mutate live graph
- preview mode suppresses webview lifecycle actions
- close preview restores live state

4. **History UI integration tests**
- timeline selection updates preview cursor correctly
- dissolved tab operations preserve archive invariants

### 6.2 CI Gates

Required checks for PRs touching:
- traversal/history model code
- history manager UI code
- replay/preview code paths
- persistence archive code (`traversal_archive`, `dissolved_archive`)
- webview lifecycle events that append traversal records

---

## 7. Degradation Policy

### 7.1 Required States

- **Full**: traversal capture and archive operations healthy; preview/replay (if enabled) isolated and working.
- **Degraded (capture-only)**: timeline preview/replay unavailable, but traversal capture and archive remain functional.
- **Degraded (history-readonly)**: history browsing works, but archive mutation/export or preview is unavailable.
- **Unavailable**: history subsystem cannot initialize; explicit diagnostics and UI status required.

### 7.2 Required Signals

- All degradation states emit history diagnostics channels.
- Subsystem/diagnostics panes surface degraded history status.
- No silent disabling of replay/preview or traversal capture.

---

## 8. Ownership Boundaries

| Owner | Guarantees |
|---|---|
| **History Manager** | Timeline/dissolved UI semantics, cursor behavior, user-visible history operations |
| **Traversal model / append paths** | Correct traversal capture ordering and edge association |
| **Persistence layer** (`GraphStore` archives) | Archive durability, append-only behavior, export fidelity |
| **Preview/replay controller** (future) | Detached replay state, isolation enforcement, clean return-to-present |
| **Diagnostics subsystem** | History channel visibility, invariant violations, health summaries |

History does **not** own `AWAL`, distillation policy, or lineage DAG truth.

---

## 9. Current Status & Gaps

**What exists**:
- Traversal capture and archive-related behaviors exist in the codebase and are referenced by history manager work and persistence archives.
- History subsystem contracts (capture/archive/replay isolation) are explicitly defined in this guide.
- Unified architecture taxonomy defined in `2026-03-08_unified_history_architecture_plan.md`.
- Canonical Stage F surface contract defined in `history_timeline_and_temporal_navigation_spec.md`
  (revised 2026-03-18): enter/exit preview, scrubber cursor, blocked commands, affordances, return-to-present.
- WAL timeline index now covers traversal, structure, node navigation, and node audit
  timestamped entries (`AppendTraversal`, `AddNode`, `RemoveNode`, `NavigateNode`,
  `AppendNodeAuditEvent`) — scrubber replay and mixed timeline queries reflect full
  timestamped history coverage.
- History Manager UI: "Enter Preview" button (live mode, when entries exist), "Viewing history"
  banner + "Return to Present" button (preview mode); row clicks navigate to traversal destination.
- Node navigation history is implemented end-to-end:
  `NavigateNode` WAL variant, `node_navigation_history` query, and node-pane history surface.
- Node audit history is implemented end-to-end:
  `AppendNodeAuditEvent` WAL variant, `node_audit_history` query, and node-pane audit surface.
- Mixed timeline surface is implemented in History Manager as tab `All`, backed by
  `HistoryTimelineEvent`/`HistoryTimelineFilter` and `mixed_timeline_entries`.

**What's missing / open**:
- Canonical history-side boundary-event schema for distillation/promotion links
  into `AWAL` and lineage systems.
- Preview mode affordances beyond the History Manager pane (e.g., canvas-level
  preview banner visible outside the tool pane).
- Cross-track diagnostics and focused tests for mixed timeline filtering/query correctness
  (`history.*` channels + integration tests over `mixed_timeline_entries`).

## 10. Dependencies / Blockers

- Replay/preview work is blocked on stable Stage E history manager behavior and predictable archive/WAL shapes.
- Isolation enforcement depends on clear effect-suppression gates in GUI/runtime lifecycle paths.
- Some correctness guarantees overlap with `storage`; sequencing should land archive diagnostics and persistence integrity hooks first.

## 11. Linked Docs

- `2026-02-20_edge_traversal_impl_plan.md` (Stage F temporal navigation/replay planning)
- `PLANNING_REGISTER.md` (cross-subsystem sequencing and preserved Stage F backlog notes)
- `SUBSYSTEM_STORAGE.md` (archive durability and WAL integrity dependencies)
- `SUBSYSTEM_DIAGNOSTICS.md` (history diagnostics channels/health summary infrastructure)

## 12. Implementation Roadmap (Subsystem-Local)

1. **Traversal correctness audit + tests** — especially URL prior/current ordering and edge association.
2. **History doc taxonomy cleanup** — align subsystem docs with
   `2026-03-08_unified_history_architecture_plan.md`. ✅ Done 2026-03-18.
3. **Stage F canonical spec** — write
   `history_timeline_and_temporal_navigation_spec.md` from the current runtime
   preview/replay shape. ✅ Done 2026-03-18.
4. **WAL timeline index coverage** — index timestamped structural and history tracks
  alongside traversal (`AddNode`, `RemoveNode`, `NavigateNode`, `AppendNodeAuditEvent`).
  ✅ Done 2026-03-18.
5. **History Manager parity** — "Enter Preview" / "Return to Present" UI,
   row click to traversal destination. ✅ Done 2026-03-18.
6. **Node navigation history** — `NavigateNode` WAL variant, per-node query,
  node history panel surface. Spec: `archive_docs/checkpoint_2026-03-18/node_navigation_history_spec.md` (archived).
  ✅ Spec and implementation done 2026-03-18.
7. **Node audit history** — concrete spec and implementation for timestamped audit events.
  ✅ Done 2026-03-18 (`node_audit_log_spec.md`, WAL/query emit paths).
8. **Mixed timeline contract + surface** — typed union, filter API, query shape,
  History Manager `All` tab.
  ✅ Done 2026-03-18 (`2026-03-18_mixed_timeline_contract.md`).
9. **Archive integrity tests** — dissolved/traversal archive completeness + export checks.
10. **Boundary-event schema** — define the history-side audit event that links
   node activity to distillation/promotion events in `AWAL` and lineage DAGs.

---

## 13. Done Definition

History is a guaranteed system property when:

- Traversal capture correctness is test-covered and diagnosable
- Archive integrity is enforced and observable
- Replay/preview isolation contracts are implemented and validated (when Stage F lands)
- Return-to-present restoration is deterministic and tested
- History degradation states are explicit, observable, and user-visible
- The history subsystem's relationship to `AWAL` and lineage traversal is
  explicit without blurring ownership
