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
- `PLANNING_REGISTER.md` (temporal navigation adoption + Stage F append notes)
**Related**: `SUBSYSTEM_STORAGE.md` (archive/WAL correctness), `SUBSYSTEM_DIAGNOSTICS.md` (timeline observability)

---

## 1. Why This Exists

History in Graphshell is not just a pane. It is a temporal truth system spanning:

- traversal capture (`Traversal` records on edges),
- archive storage (active/dissolved traversal keyspaces),
- history presentation (History Manager timeline + dissolved views),
- and future temporal replay/preview (Stage F).

The dominant failure mode is **silent temporal integrity erosion**: traversal append order becomes incorrect, preview mode mutates live state, replay writes to WAL, dissolved/archive transfers lose entries, or "return to present" leaks preview state into the live graph.

Without subsystem-level treatment, every traversal/UI/persistence change becomes an unaudited temporal correctness boundary crossing.

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
- History Manager tool pane: `timeline_navigation`, `archive_export`

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

### 5.2 History Health Summary (Subsystem/Diagnostics Pane)

- Traversal capture status (`active` / `degraded`)
- Recent traversal append failure count
- Archive sizes (active/dissolved)
- Preview mode status (`off` / `active`)
- Replay isolation status (last violation / none)
- Last return-to-present result

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

---

## 9. Current Status & Gaps

**What exists**:
- Traversal capture and archive-related behaviors exist in the codebase and are referenced by history manager work and persistence archives.
- Stage F temporal navigation/replay concept and isolation constraints are documented in planning materials.
- History subsystem contracts (capture/archive/replay isolation) are now explicitly defined in this guide.

**What's missing / open**:
- Dedicated `history.*` diagnostics channels and subsystem health summary.
- Replay/preview controller and isolation enforcement implementation.
- CI coverage for traversal correctness and preview non-side-effect guarantees.

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
2. **History diagnostic channels** — emit `history.*` channels for traversal/archive operations.
3. **History health summary** — expose capture/archive status in diagnostics and history subsystem pane.
4. **Archive integrity tests** — dissolved/traversal archive completeness + export checks.
5. **Stage F replay scaffold** — preview state model + isolation gates in `gui_frame.rs`.
6. **Replay isolation tests** — lock in no-WAL/no-live-mutation guarantees before UI polish.

---

## 13. Done Definition

History is a guaranteed system property when:

- Traversal capture correctness is test-covered and diagnosable
- Archive integrity is enforced and observable
- Replay/preview isolation contracts are implemented and validated (when Stage F lands)
- Return-to-present restoration is deterministic and tested
- History degradation states are explicit, observable, and user-visible
