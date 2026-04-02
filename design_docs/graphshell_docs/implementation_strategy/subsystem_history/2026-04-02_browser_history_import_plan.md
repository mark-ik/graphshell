# Browser History Import Plan

**Date**: 2026-04-02  
**Status**: Planned / currentized split from `2026-02-11_bookmarks_history_import_plan.md`  
**Scope**: Import external browser-history datasets as imported knowledge capture without treating them as Graphshell traversal truth.

**Related**:

- [SUBSYSTEM_HISTORY.md](SUBSYSTEM_HISTORY.md)
- [history_timeline_and_temporal_navigation_spec.md](history_timeline_and_temporal_navigation_spec.md)
- [2026-03-18_mixed_timeline_contract.md](2026-03-18_mixed_timeline_contract.md)
- [../graph/2026-03-11_graph_enrichment_plan.md](../graph/2026-03-11_graph_enrichment_plan.md)
- [../system/register/action_registry_spec.md](../system/register/action_registry_spec.md)

---

## Goal

Allow a user to seed Graphshell from an external browser-history export or SQLite database by importing visited URLs as imported graph content.

The import should create useful nodes and import provenance while respecting the current History subsystem boundary: imported browser history is **not** the same thing as Graphshell's live traversal/archive truth.

## Hard Boundary

The current History subsystem owns traversal truth, archive fidelity, preview/replay isolation, and recent-history projections.

Therefore MVP browser-history import must **not**:

- append synthetic traversal records,
- create fake `History` edges representing imported visits,
- populate replay/archive state,
- or masquerade as live `WebViewHistoryChanged` state.

Imported browser history is external imported data, not native temporal truth.

## Non-Goals

- Do not reconstruct a complete visit graph or referrer tree in MVP.
- Do not inject imported visits into the History Manager timeline/dissolved tabs.
- Do not promise replay/preview over imported browser-history datasets.

## Current-System Fit

This plan must align with the existing imported-data model:

- node import provenance,
- import records and membership normalization,
- imported relation sub-kinds,
- ActionRegistry-backed invocation,
- imported-data visibility in Navigator/search/provenance UI.

The earlier combined plan's optional `AddTraversal` idea is no longer correct for MVP under the active History subsystem policy.

## Canonical MVP Model

### 1. Invocation Surface

- Provide `import.history_from_file` through a native import mod or equivalent registry-owned host action.
- Start with explicit command-surface invocation and file selection.

### 2. Accepted Inputs

- SQLite browser history database copies/exports where schema is sufficiently understood.
- Read-only handling only.

### 3. Security and IO Policy

- Open SQLite databases read-only.
- Prefer importing from user-selected copies when the source DB is browser-locked.
- Never write into the source profile database.
- Treat source files as untrusted input and avoid file-derived SQL construction.

### 4. Imported History Node Model

For each imported history row selected for MVP import:

- normalize the URL,
- create or merge the corresponding node,
- attach history-import provenance,
- update import-record membership for the import run/source,
- do **not** apply `#starred` by default.

### 5. Imported History Semantics

If imported-history structural relationships are represented in the graph, they must use imported-data semantics such as `ImportedSubKind::HistoryImport`, not the live `History` traversal family.

If the current graph model is insufficient to represent visit-level metadata cleanly, MVP should stop at node import + provenance rather than inventing a fake edge/traversal model.

## MVP Scoping Rule

Because current graph carriers already support provenance and import records, but not a canonical imported-visit event store, MVP browser-history import should be intentionally narrow:

- seed nodes from recent/high-value history rows,
- preserve source labeling and membership,
- leave rich visit chronology, ranking, and replay for a follow-on carrier design.

If the product needs visit-count, recency, or referrer ranking in UI, first define a dedicated imported-history metadata carrier rather than overloading traversal truth.

## Implementation Slices

### Slice A: Read-Only Source Reader

- Support a known browser-history SQLite schema first.
- Extract a bounded import set such as:
  - most recent N rows,
  - or rows within a configurable time window,
  - or both.

This prevents graph explosion during the first MVP.

### Slice B: Graph Merge and Provenance

- Route execution through explicit action -> intent -> reducer boundaries.
- For each accepted row:
  - create or merge the URL node,
  - refine title if useful,
  - attach import provenance,
  - update import-record membership.

### Slice C: Imported-History Projection

- Ensure imported history is visible through imported-data/provenance surfaces.
- Do not route imported browser-history rows into live History Manager timeline state.
- A future dedicated imported-history view may be added later if the product needs it.

### Slice D: Follow-On Metadata Design

If richer imported-history behavior is required, add a dedicated follow-on spec for:

- imported visit counters,
- last-visited timestamps,
- referrer/source attribution,
- ranking/scoring inputs,
- optional imported-history-only timeline views.

That work should land only after the metadata carrier is explicit.

## Validation

### Manual

1. Import a copied browser-history SQLite file and verify recent URLs are added or merged into the graph.
2. Re-import overlapping history data and verify node duplication does not occur.
3. Verify imported items show up as imported/provenance-backed data, not as live timeline traversal rows.
4. Verify no synthetic back/forward history edges or replay artifacts appear after import.

### Automated

- Source-reader tests for supported SQLite schema parsing.
- Merge/provenance tests for repeated imports.
- Import-record persistence tests.
- Regression tests asserting history import does **not** create traversal append events, `History` edges, or preview/archive state mutations.

## Done Gate

This plan is complete when:

- external browser-history data can be imported through an explicit command surface,
- imported URLs merge into the graph without duplication,
- provenance/import-record membership are preserved,
- the resulting data is visible as imported content,
- and the live History subsystem remains authoritative for traversal truth.
