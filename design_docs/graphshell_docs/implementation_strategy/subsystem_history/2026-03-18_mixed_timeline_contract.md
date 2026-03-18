# Mixed Timeline Contract

**Date**: 2026-03-18
**Status**: Active spec — typed union, filter API, query shape, and surface
behavior for the multi-track history timeline
**Scope**: Defines the concrete contract that the unified history architecture
plan (§6.3) deferred until per-track schemas and surfaces were stable
**Prerequisites satisfied**:
- Per-track WAL schemas landed: `AppendTraversal`, `NavigateNode`,
  `AppendNodeAuditEvent` (all in `services/persistence/types.rs`)
- Per-track query helpers landed: `node_navigation_history()`,
  `node_audit_history()`, `timeline_index_entries()`
- Per-track UI surfaces landed: History Manager Timeline/Dissolved tabs,
  node history panel, node audit panel
**Related**:
- `2026-03-08_unified_history_architecture_plan.md` (parent architecture)
- `../../../archive_docs/checkpoint_2026-03-18/node_navigation_history_spec.md` (archived: implemented 2026-03-18)
- `node_audit_log_spec.md`
- `edge_traversal_spec.md`
- `history_timeline_and_temporal_navigation_spec.md`

---

## 1. Purpose

The per-track history surfaces work well when the user already knows which
track they want. But there is no way to ask "what happened at 14:32?" across
all tracks, or to see a node's full lifecycle (created → navigated → renamed →
tagged → traversed-to) in one chronological stream.

This spec defines the typed event union, canonical filter API, query shape, and
surface behavior needed to answer cross-track queries without degrading
single-track queries into noise.

---

## 2. Typed Event Union — `HistoryTimelineEvent`

A single enum that wraps per-track entries while preserving provenance.

```rust
/// A typed union of all history-track events that can appear in a mixed
/// timeline. Each variant carries the original per-track payload plus a
/// shared temporal envelope.
#[derive(Debug, Clone)]
pub enum HistoryEventKind {
    /// Inter-node traversal (TraversalHistory track).
    Traversal {
        from_node_id: String,
        to_node_id: String,
        trigger: PersistedNavigationTrigger,
    },
    /// Intra-node address evolution (NodeNavigationHistory track).
    NodeNavigation {
        node_id: String,
        from_url: String,
        to_url: String,
        trigger: PersistedNavigationTrigger,
    },
    /// Node metadata/lifecycle audit (NodeAuditHistory track).
    NodeAudit {
        node_id: String,
        event: NodeAuditEventKind,
    },
    /// Graph structural event (node added or removed).
    GraphStructure {
        node_id: String,
        is_addition: bool,
    },
}

/// The shared temporal envelope wrapping every mixed-timeline row.
#[derive(Debug, Clone)]
pub struct HistoryTimelineEvent {
    /// Wall-clock time of the event (ms since UNIX epoch).
    pub timestamp_ms: u64,
    /// WAL log position for stable ordering of same-ms events.
    pub log_position: u64,
    /// The typed event payload.
    pub kind: HistoryEventKind,
}
```

### 2.1 Design Rules

1. **No synthetic traversals.** Audit and navigation events are never coerced
   into fake `Traversal` variants. Each event renders with its own icon and
   description template.
2. **Provenance preserved.** `HistoryEventKind` discriminant is the track
   provenance. Query consumers can match on variant to filter or group by
   track.
3. **Shared envelope.** `timestamp_ms` + `log_position` provides total order.
   `log_position` breaks ties when two events share the same millisecond.
4. **Exhaustive at read time.** The projection function (§4) must handle every
   `LogEntry` variant that carries a `timestamp_ms`. Variants without
   timestamps (`AddEdge`, `RemoveEdge`, `ClearGraph`, `UpdateNodeTitle`,
   `PinNode`, `UpdateNodeUrl`, `TagNode`, `UntagNode`, `UpdateNodeMimeHint`,
   `UpdateNodeAddressKind`) are excluded — they are WAL snapshot entries, not
   timestamped history events. Their effects are captured by the corresponding
   `AppendNodeAuditEvent` entry when one exists.
5. **Additive extension.** Future tracks (e.g., UndoRedo checkpoints,
   workbench-structure events) add new `HistoryEventKind` variants. Existing
   variants are append-only; fields are never removed.

---

## 3. Canonical Filter API — `HistoryTimelineFilter`

```rust
/// Filter predicate for mixed-timeline queries. All fields are optional;
/// `None` means "no constraint on this axis." Multiple constraints are
/// AND-combined.
#[derive(Debug, Clone, Default)]
pub struct HistoryTimelineFilter {
    /// Include only these track kinds. `None` or empty = all tracks.
    pub tracks: Option<Vec<HistoryTrackKind>>,
    /// Include only events touching this node (as source, target, or subject).
    pub node_id: Option<String>,
    /// Include only events at or after this timestamp.
    pub after_ms: Option<u64>,
    /// Include only events at or before this timestamp.
    pub before_ms: Option<u64>,
    /// Full-text substring match against the event's display-text projection
    /// (URL, title, tag name, etc.). Case-insensitive.
    pub text_contains: Option<String>,
}

/// Track-kind discriminant for filter predicates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HistoryTrackKind {
    Traversal,
    NodeNavigation,
    NodeAudit,
    GraphStructure,
}
```

### 3.1 Filter Semantics

| Filter field | Match rule |
|---|---|
| `tracks` | Event's `HistoryEventKind` discriminant is in the set |
| `node_id` | Event references the node as `from_node_id`, `to_node_id`, or `node_id` |
| `after_ms` | `event.timestamp_ms >= after_ms` |
| `before_ms` | `event.timestamp_ms <= before_ms` |
| `text_contains` | Case-insensitive substring match on the event's rendered summary text |

Multiple non-`None` fields are AND-combined: an event must match every
specified constraint to appear in results.

### 3.2 Default Filter (History Manager "All" tab)

When the History Manager switches to the mixed-timeline tab with no user
filter active, the implicit filter is:

```rust
HistoryTimelineFilter {
    tracks: None,         // all tracks
    node_id: None,        // all nodes
    after_ms: None,       // no start bound
    before_ms: None,      // no end bound
    text_contains: None,  // no text search
}
```

This returns all timestamped events in reverse chronological order, subject to
the existing `GRAPHSHELL_HISTORY_MANAGER_LIMIT` row cap.

---

## 4. Query Shape — `mixed_timeline_entries()`

A single query function on `GraphStore` that projects `LogEntry` records into
the typed union and applies the filter.

```rust
impl GraphStore {
    /// Retrieve a filtered, sorted mixed-timeline view over all history tracks.
    ///
    /// Returns newest-first. `limit` caps the result count after filtering.
    pub fn mixed_timeline_entries(
        &self,
        filter: &HistoryTimelineFilter,
        limit: usize,
    ) -> Vec<HistoryTimelineEvent> { ... }
}
```

### 4.1 Projection Rules

The function scans the WAL `log_keyspace` (same iteration as existing
`timeline_index_entries`) and maps each timestamped `LogEntry` variant
to the corresponding `HistoryEventKind`:

| WAL variant | → `HistoryEventKind` |
|---|---|
| `AppendTraversal { from_node_id, to_node_id, timestamp_ms, trigger }` | `Traversal { from_node_id, to_node_id, trigger }` |
| `NavigateNode { node_id, from_url, to_url, trigger, timestamp_ms }` | `NodeNavigation { node_id, from_url, to_url, trigger }` |
| `AppendNodeAuditEvent { node_id, event, timestamp_ms }` | `NodeAudit { node_id, event }` |
| `AddNode { node_id, timestamp_ms, .. }` | `GraphStructure { node_id, is_addition: true }` |
| `RemoveNode { node_id, timestamp_ms }` | `GraphStructure { node_id, is_addition: false }` |
| all other variants | skipped (no `timestamp_ms`) |

### 4.2 Sort Contract

Results are sorted by `(timestamp_ms DESC, log_position DESC)` — newest first,
tiebreaking by WAL position. This matches the existing `timeline_index_entries`
sort order.

### 4.3 Performance Notes

- The WAL scan is O(N) in log entries. For v1 this is acceptable because the
  existing `timeline_index_entries` is also a full scan.
- If the scan becomes a bottleneck, a secondary index keyed on
  `(timestamp_ms, log_position)` should be introduced in a later stage (H6
  index pass). The query API stays the same.
- The `text_contains` filter requires projecting the display summary before
  matching. This is the most expensive filter; it runs after all other
  predicates as a final pass.

---

## 5. Surface Behavior — History Manager "All" Tab

### 5.1 Tab Addition

Add a third tab to `HistoryManagerTab`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HistoryManagerTab {
    #[default]
    Timeline,
    Dissolved,
    /// Mixed multi-track timeline (all history tracks, filtered).
    All,
}
```

The "All" tab becomes the new default once the mixed timeline ships. During
development, `Timeline` remains the default until the mixed surface passes its
acceptance tests.

### 5.2 Row Rendering

Each row in the "All" tab renders a `HistoryTimelineEvent` with:

| Element | Source | Example |
|---|---|---|
| **Time label** | `timestamp_ms` → relative format | "3m ago" |
| **Track badge** | `HistoryEventKind` discriminant | `[T]`, `[N]`, `[A]`, `[+]`/`[-]` |
| **Icon** | variant-specific (reuses per-track icon logic) | 🔗, ⬅, ✏, 🏷, 📌 |
| **Summary text** | variant-specific projection | "example.com → docs.example.com" |
| **Node context** | `node_id` resolved to title if available | "(My Page)" |

Track badge characters:
- `[T]` — Traversal (matches Timeline tab color)
- `[N]` — NodeNavigation
- `[A]` — NodeAudit
- `[+]` — Node added
- `[-]` — Node removed

### 5.3 Filter Chips

Above the row list, a horizontal chip bar allows toggling track kinds:

```
[All] [Traversal] [Navigation] [Audit] [Structure]  🔍 ___________
```

- Clicking a track chip sets `filter.tracks` to that single kind.
- "All" clears the track filter (default).
- The search field populates `filter.text_contains`.
- Chips are non-exclusive in v1 — selecting multiple chips combines them
  as an OR within `filter.tracks` (i.e., the `tracks` vec holds multiple
  kinds). The outer AND with other filter fields is preserved.

### 5.4 Node-Scoped Filter Activation

When a node is selected in the graph and the user opens the "All" tab, the
surface pre-populates `filter.node_id` with the selected node's ID and shows
a dismissable "Filtered to: {node_title}" chip. Dismissing the chip clears
`node_id` back to `None`.

This replaces the need to navigate to the per-node history/audit panels for a
cross-track node view, while keeping those panels available for focused
single-track inspection.

### 5.5 Click-to-Navigate

Row click behavior matches the existing per-track convention:

| Event kind | Click action |
|---|---|
| `Traversal` | Focus the target node (`to_node_id`) |
| `NodeNavigation` | Set node URL to `to_url` (same as node history panel) |
| `NodeAudit` | Focus the subject node (`node_id`) |
| `GraphStructure` | Focus the created/removed node (no-op if tombstoned) |

### 5.6 Preview Mode Interaction

The "All" tab respects the same preview-mode constraints as the Timeline tab:

- In preview mode, the "Viewing history" banner and replay controls appear.
- Row clicks are suppressed (no live graph mutations in preview).
- The preview timeline uses `mixed_timeline_entries` with `before_ms` set to
  the preview cursor timestamp to show events up to the preview point.

---

## 6. Temporal Replay Integration

### 6.1 v1 Scope

The Stage F temporal replay mechanism (`replay_to_timestamp`) continues to
consume only `AppendTraversal`, `AddNode`, `RemoveNode`, `NavigateNode`, and
`AppendNodeAuditEvent` for graph reconstruction — the same set it uses today.

The mixed timeline surface is a read-only view; it does not change the replay
input set.

### 6.2 Future v2

When replay supports visual overlays (ghost badges for audit events, address
transitions highlighted on nodes), the mixed timeline "All" tab becomes the
natural scrubber surface. The `HistoryTimelineFilter.before_ms` field already
supports this: set it to the scrubber cursor position to show a time-bounded
mixed view.

---

## 7. Implementation Stages

### Stage M1 — Types and Projection

1. Add `HistoryEventKind`, `HistoryTimelineEvent`, `HistoryTrackKind`, and
   `HistoryTimelineFilter` to `services/persistence/types.rs`.
2. Implement `mixed_timeline_entries()` on `GraphStore`.
3. Tests: verify projection covers all timestamped WAL variants, sort order
   matches, each filter field works in isolation and combined.

### Stage M2 — Tab and Unfiltered Rendering

1. Add `HistoryManagerTab::All` variant.
2. Wire `mixed_timeline_entries` into the History Manager render path.
3. Render rows with track badges, icons, and summary text.
4. No filtering UI yet — "All" tab always shows the full unfiltered stream.

### Stage M3 — Filter UI

1. Add track-kind chip bar.
2. Add text search field.
3. Add node-scoped auto-filter on selection.
4. Wire filter state into `mixed_timeline_entries` calls.

### Stage M4 — Preview Integration

1. Suppress row clicks in preview mode.
2. Pass `before_ms` from preview cursor into the "All" tab query.
3. Show the same "Viewing history" banner and replay controls.

### Stage M5 — Default Tab Promotion

1. Switch `HistoryManagerTab` default from `Timeline` to `All`.
2. Acceptance test: opening History Manager with no prior tab state shows the
   "All" tab.

---

## 8. Acceptance Criteria

1. `HistoryTimelineEvent` is a concrete Rust type with exhaustive
   `HistoryEventKind` variants covering all five timestamped WAL entry types.
2. `mixed_timeline_entries()` returns correct, chronologically sorted results
   for an empty filter, a single-track filter, a node-scoped filter, a
   time-range filter, and a text-search filter.
3. The "All" tab renders mixed-track rows with distinguishable track badges
   and correct per-variant icons/descriptions.
4. Track chip filtering toggles work (single-track, multi-track, all).
5. Node-scoped auto-filter activates on selection and dismisses cleanly.
6. Click-to-navigate dispatches the correct intent per event kind.
7. Preview mode suppresses clicks and bounds the query to the preview cursor.
8. No existing per-track surface (Timeline tab, Dissolved tab, node history
   panel, node audit panel) is degraded or removed by the mixed timeline
   addition.

---

## 9. Non-Goals

- **Undo/redo in mixed timeline.** UndoRedoHistory is a checkpoint stack, not
  an append-only event stream. It stays separate per the architecture plan §3.
- **AWAL/lineage entries.** Agent and engram provenance events are not history
  events. The architecture plan §3.1–3.2 keeps them separate; boundary
  crossings appear as `NodeAudit` entries only.
- **Secondary WAL index.** The O(N) scan is acceptable for v1. An index is a
  performance optimization, not a contract change.
- **Mixed dissolved timeline.** The Dissolved archive is traversal-specific
  and has no multi-track equivalent in this spec.
