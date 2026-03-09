# Unified History Architecture Plan

**Date**: 2026-03-08
**Status**: Active — architectural consolidation plan
**Scope**: History subsystem taxonomy, ownership boundaries, storage model, and execution sequencing
**Related**:
- `SUBSYSTEM_HISTORY.md`
- `edge_traversal_spec.md`
- `node_audit_log_spec.md`
- `../../technical_architecture/2026-02-18_universal_node_content_model.md`
- `../../technical_architecture/2026-03-08_graphshell_core_extraction_plan.md`
- `../2026-03-01_ux_migration_design_spec.md`

---

## 1. Problem Statement

The current docs use "history" to refer to several different systems:

1. edge traversal history
2. per-node navigation/address history
3. per-node audit/change history
4. temporal replay / time-travel preview
5. undo/redo checkpoint history

Traversal history is the only one with substantial runtime implementation today.
The others are either partially implied by architecture docs, deferred into
stubs, or implemented as separate mechanisms without a shared top-level model.

This creates four recurring problems:

- subsystem plans overstate what "history" currently covers
- multiple docs imply future mixed timelines without defining them
- core-extraction plans assume node-history structures that the history
  subsystem docs do not yet own
- replay/preview, audit, and undo are discussed as if they are variants of the
  same thing, even though their mutation and storage rules differ

This plan defines the missing top-level architecture.

---

## 2. Canonical History Taxonomy

Graphshell history is split into five distinct but related tracks.

### 2.1 TraversalHistory

**What it records**:
- navigation between nodes
- edge-local repeated traversals
- traversal trigger and direction

**Current status**: Implemented and canonical.

**Canonical truth**:
- edge payload traversal records
- traversal archive / dissolved archive

**Primary surfaces**:
- History Manager timeline
- History Manager dissolved tab
- traversal-aware edge rendering

### 2.2 NodeNavigationHistory

**What it records**:
- address changes within a node's lifetime
- persistent per-node back/forward lineage
- content-state history for a stable node identity

**Current status**: Architecturally intended, not implemented as a canonical
history track.

**Canonical truth target**:
- `NavigateNode`-style WAL records
- per-node address/history entries attached to node identity

**Primary surfaces**:
- per-node history panel
- node pane "history" mode
- future cold-node inspection without activating a renderer

### 2.3 NodeAuditHistory

**What it records**:
- metadata changes (`title`, tags, address, notes, `mime_hint`, etc.)
- node lifecycle events (tombstone, restore, delete)
- workbench-affecting node events when they are semantically relevant

**Current status**: Deferred stub only.

**Canonical truth target**:
- audit-event WAL entries
- node-scoped audit archive/query surface

**Primary surfaces**:
- filtered History Manager views
- per-node audit view
- export / provenance / collaboration follow-ons

### 2.4 TemporalReplay

**What it records**:
- not a new truth source; it is a replay mode over persisted truth

**Current status**: Partially implemented groundwork.

**Canonical truth inputs**:
- snapshots
- WAL
- timeline index

**Primary surfaces**:
- preview mode
- timeline scrubber / replay controls
- return-to-present

### 2.5 UndoRedoHistory

**What it records**:
- reversible editor/workspace checkpoints
- user-facing mutation inversion points

**Current status**: Implemented as a separate checkpoint system.

**Canonical truth**:
- undo/redo snapshot stacks

**Primary surfaces**:
- undo/redo commands
- workspace/layout restoration

**Important rule**: Undo/redo is not the same thing as traversal history or
temporal replay. It is a mutation-reversal system, not a historical event log.

---

## 3. Canonical Relationship Model

These five tracks must be related explicitly rather than merged by accident.

| Track | Is append-only history? | Is reversible? | Is mixed into global timeline by default? |
|---|---|---|---|
| TraversalHistory | Yes | No | Yes |
| NodeNavigationHistory | Yes | No | Not initially |
| NodeAuditHistory | Yes | No | Not initially |
| TemporalReplay | No — replay mechanism | N/A | Operates over other tracks |
| UndoRedoHistory | No — checkpoint stack | Yes | No |

Rules:

1. Traversal history and node navigation history are related but not identical.
   Traversal is inter-node movement; node navigation history is intra-node
   address evolution.
2. Node audit history is not stored in traversal edges and is not represented
   as synthetic traversals.
3. Temporal replay replays persisted truth from the applicable history tracks;
   it must not become a parallel mutable store.
4. Undo/redo stays separate from the append-only history system, though some
   events may be described in audit surfaces for provenance.

---

## 4. Current Implementation Map

### 4.1 Landed

- traversal append path and repeated traversal preservation
- traversal archives + dissolved archives
- History Manager tool pane with Timeline + Dissolved tabs
- clear / export / auto-curation archive operations
- timeline index exposure
- detached replay graph construction
- preview-mode state and several side-effect suppression gates
- undo/redo snapshot stacks

### 4.2 Missing

- canonical node navigation history model (`NavigateNode`, per-node address
  history entries)
- node audit log model and storage
- mixed-history query contract
- canonical temporal-navigation interaction spec
- timeline scrubber / enter-preview / exit-preview user-facing controls
- preview ghost rendering / explicit preview affordances
- History Manager filtering/search contract implementation

### 4.3 Spec / Runtime Drift To Resolve

- some docs still describe history diagnostics/health summary as missing even
  though runtime wiring now exists
- History Manager row click behavior must match the canonical spec
- the missing `history_timeline_and_temporal_navigation_spec.md` leaves Stage F
  without one canonical surface contract

---

## 5. Ownership Boundaries

### 5.1 Graph/Core Ownership

Graph/core should own:

- node identity
- node navigation history entries
- node audit event types
- traversal event types and edge payload truth
- WAL entry schemas for all history tracks except renderer-local ephemeral state

This aligns with `2026-03-08_graphshell_core_extraction_plan.md`.

### 5.2 Host Ownership

The host should own:

- History Manager UI
- preview-mode orchestration and effect suppression
- archive export plumbing
- diagnostics surfacing
- renderer-specific local history bridges before they are normalized into core

### 5.3 Workbench Ownership

Workbench owns:

- pane placement for history surfaces
- preview surface hosting
- return-to-focus behavior when leaving history/preview panes

Workbench does **not** own history truth.

---

## 6. Query and Surface Model

The system needs one explicit query model rather than ad hoc per-pane scans.

### 6.1 Query Classes

1. `GlobalTraversalTimeline`
2. `DissolvedTraversalTimeline`
3. `NodeNavigationTimeline { node }`
4. `NodeAuditTimeline { node }`
5. future `UnifiedTimeline { filters... }`

### 6.2 Initial Surface Strategy

Do not build the final mixed-history timeline first.

Land in this order:

1. keep History Manager Timeline/Dissolved as traversal-only
2. add separate node navigation history surface
3. add separate node audit history surface
4. only then define a mixed timeline query contract

This avoids overloading the existing traversal timeline with incompatible event
types before storage/query semantics are settled.

### 6.3 Mixed Timeline Rule

When a mixed history timeline is eventually introduced, it must:

- use a typed event union, not pretend all entries are traversals
- preserve provenance of event kind
- support filtering by event class
- avoid degrading traversal queries into generic "everything happened" noise

---

## 7. Storage and WAL Plan

### Stage H1 — Normalize History Track Schemas

Define canonical event shapes for:

- `AppendTraversal`
- `NavigateNode`
- `AppendNodeAuditEvent`

Document which stay in core and which host paths merely adapt into them.

### Stage H2 — Keep Traversal Archives Stable

Do not destabilize the existing traversal archive pipeline while adding other
history tracks.

### Stage H3 — Add NodeNavigationHistory Storage

Introduce:

- durable node navigation event schema
- replay/query helpers for a node's address history

### Stage H4 — Add NodeAuditHistory Storage

Introduce:

- audit event schema
- separate archive/query keyspace
- no reuse of traversal archives

### Stage H5 — Unify Replay Inputs

Define which history tracks temporal replay actually consumes in v1.

Recommended v1:

- traversal + graph structural truth only

Deferred:

- node audit overlays in replay
- node navigation-history visual overlays in replay

---

## 8. Temporal Replay Plan Gap Closure

The missing Stage F architectural closure is not just more implementation. It
needs a canonical UX contract.

That spec must define:

- how preview mode is entered
- whether preview begins from timeline row, slider, or both
- what visual affordance marks the app as being in preview
- which commands are blocked vs allowed while preview is active
- what "Return to present" restores
- whether selection/focus in preview is ephemeral or restored

Until that spec exists, Stage F should be considered partially implemented but
not closed.

---

## 9. Recommended Execution Sequence

1. **History doc cleanup**
   Update subsystem docs so they stop implying that all history tracks already
   exist.

2. **Stage F canonical spec**
   Write `history_timeline_and_temporal_navigation_spec.md` from the current
   preview/replay implementation and remaining UX questions.

3. **History Manager parity pass**
   Fix runtime/spec drift in current traversal History Manager behavior
   (row-click target, any stale diagnostics notes, missing channel inventory
   alignment).

4. **NodeNavigationHistory design**
   Turn the universal node content model's `NavigateNode` / address-history
   ideas into an implementation-spec track owned jointly by history + core
   extraction.

5. **NodeAuditHistory design**
   Replace the deferred stub with a real spec once traversal and replay
   contracts are stable.

6. **Mixed timeline decision**
   Only after 4 and 5 are specified should Graphshell decide whether History
   Manager becomes a multi-track timeline or remains a traversal-first surface
   plus node-scoped history panels.

---

## 10. Done Definition

The overarching history architecture is coherent when:

1. TraversalHistory, NodeNavigationHistory, NodeAuditHistory, TemporalReplay,
   and UndoRedoHistory each have explicit scope and storage ownership.
2. No history doc uses "history" ambiguously without naming the track.
3. Stage F has a canonical surface spec, not just implementation notes.
4. Node navigation history and node audit history each have a concrete plan or
   canonical spec rather than being implied by unrelated architecture docs.
5. History Manager surface contracts match runtime behavior.

