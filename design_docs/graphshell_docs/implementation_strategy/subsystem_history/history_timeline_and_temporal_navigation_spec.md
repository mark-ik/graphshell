# History Timeline and Temporal Navigation Spec

**Date**: 2026-03-18 (revised; originally 2026-02-28)
**Status**: Canonical surface contract — Stage F
**Priority**: Implementation guidance for History Manager timeline UX

**Related**:
- `SUBSYSTEM_HISTORY.md`
- `2026-03-08_unified_history_architecture_plan.md`
- `edge_traversal_spec.md`
- `../subsystem_storage/storage_and_persistence_integrity_spec.md`

---

## 1. Purpose and Scope

This spec defines the canonical **Stage F surface contract** for temporal
navigation in the History subsystem. It governs:

- how preview mode is entered and exited
- which commands are blocked while preview is active
- what visual affordances mark preview as active
- scrubber/replay cursor semantics
- what "Return to present" restores
- selection and focus behavior across the preview boundary
- isolation invariants the runtime must enforce

This spec is authoritative for the History Manager timeline UX and for the
`apply_history_preview_cursor` / `EnterHistoryTimelinePreview` /
`ExitHistoryTimelinePreview` intent paths in `history_runtime.rs`.

It does not cover traversal capture correctness (see `edge_traversal_spec.md`)
or archive integrity (see `SUBSYSTEM_HISTORY.md §3.2`).

---

## 2. Canonical Model

### 2.1 Two Modes: Live and Preview

The app is always in one of two modes:

| Mode | State | Graph truth source |
|---|---|---|
| **Live** | `history_preview_mode_active = false` | `workspace.domain.graph` (authoritative) |
| **Preview** | `history_preview_mode_active = true` | `history_preview_graph` (detached replica) |

Preview is a read-only lens over a historical graph snapshot. It does **not**
represent a branched edit; it is the past made visible without being writable.

### 2.2 Preview State Fields

```
history_preview_mode_active: bool
history_preview_live_graph_snapshot: Option<Graph>  // snapshot taken at EnterPreview
history_preview_graph: Option<Graph>                // current-step replica
history_replay_cursor: Option<usize>               // 0 = present baseline, N = step N
history_replay_total_steps: Option<usize>          // cardinality of timeline index entries
history_replay_in_progress: bool
history_last_preview_isolation_violation: bool
history_last_return_to_present_result: Option<String>
```

---

## 3. Entering Preview

### 3.1 Entry Path

Preview is entered via `GraphIntent::EnterHistoryTimelinePreview`. There is one
authorised entry path:

1. User selects a timeline row in the History Manager **or** activates the
   timeline scrubber.
2. The UI emits `EnterHistoryTimelinePreview`.
3. The runtime snapshots `workspace.domain.graph` into
   `history_preview_live_graph_snapshot` and sets
   `history_preview_graph = snapshot.clone()` (cursor = 0, the present).

### 3.2 Entry Postconditions

After `EnterHistoryTimelinePreview`:

- `history_preview_mode_active = true`
- `history_preview_live_graph_snapshot = Some(live_graph_at_entry)`
- `history_preview_graph = Some(live_graph_at_entry)` (cursor 0 = present)
- `history_replay_cursor = None` (scrubber not yet positioned)
- `history_last_preview_isolation_violation = false`
- `CHANNEL_HISTORY_TIMELINE_PREVIEW_ENTERED` diagnostic emitted

### 3.3 Entry from timeline row vs. scrubber

Both entry paths emit `EnterHistoryTimelinePreview`. The distinction is that
a row-click then immediately emits `HistoryTimelineReplaySetTotal` +
`HistoryTimelineReplayAdvance` to position the cursor at the selected entry.
The scrubber maintains its own position and emits advance/reset as the user
drags. Both result in the same runtime state shape.

---

## 4. Scrubber and Replay Cursor

### 4.1 Cursor Semantics

The replay cursor is a step index over the `timeline_index_entries` vector,
sorted **oldest-first** (ascending timestamp / log_position):

- cursor = 0 → present (baseline snapshot, before any historical step)
- cursor = 1 → first historical event
- cursor = N → Nth event
- cursor = total_steps → last event (oldest visible)

The graph displayed at cursor N is the result of `replay_to_timestamp` over the
N-th entry in the chronological timeline index.

### 4.2 Timeline Index Coverage

The timeline index now includes:

- `AppendTraversal` entries (navigation events)
- `AddNode` structural entries
- `RemoveNode` structural entries

This means scrubbing shows structural graph history (node creation/removal) as
well as traversal events. The scrubber position reflects the full WAL event
history, not just navigation.

### 4.3 Intent Sequence for Scrubbing

```
EnterHistoryTimelinePreview
  → HistoryTimelineReplaySetTotal { total_steps: N }
  → HistoryTimelineReplayAdvance { steps: K }   // position at step K
  → HistoryTimelineReplayAdvance { steps: 1 }   // drag one step forward
  → HistoryTimelineReplayReset                  // return scrubber to present
  → ExitHistoryTimelinePreview                  // leave preview
```

Advance is clamped at `total_steps`; advance beyond `total_steps` is silently
bounded (not an error).

### 4.4 Replay Reset

`HistoryTimelineReplayReset`:

- sets `history_replay_in_progress = false`
- resets cursor to 0 (or `None` if `total_steps` is unset)
- restores `history_preview_graph = history_preview_live_graph_snapshot.clone()`

Reset is not exit. The app remains in preview mode at the present-baseline
position. This allows the scrubber to return to "now" without exiting preview.

---

## 5. Blocked Commands During Preview

### 5.1 Blocking Rule

While `history_preview_mode_active = true`, any intent that falls into the
following categories is blocked and records an isolation violation instead:

1. **Graph mutations** — any intent classified by `intent.as_graph_mutation()`,
   including `AddNode`, `RemoveNode`, `AddEdge`, `RemoveEdge`, URL updates, etc.
2. **Runtime events** — any intent classified by `intent.as_runtime_event()`,
   including webview lifecycle events (navigate, load, close).
3. **View write actions** — the following `ViewAction` variants:
   - `SetNodePosition`
   - `SetNodeFormDraft`
   - `SetNodeThumbnail`
   - `SetNodeFavicon`

### 5.2 Allowed During Preview

Everything not in §5.1 is allowed, including:

- read-only navigation (camera pan/zoom, selection changes)
- UI overlay toggles (settings, command palette, context menu)
- history manager UI operations (scrubber advance, row selection)
- diagnostic queries
- workbench layout changes that do not write to graph truth

### 5.3 Isolation Violation Recording

When a blocked intent is received:

1. `history_last_preview_isolation_violation = true`
2. `CHANNEL_HISTORY_TIMELINE_PREVIEW_ISOLATION_VIOLATION` emitted
3. The intent is not applied — no graph state change occurs

The violation is **not** a fatal error; preview remains active. The UI should
surface the violation state (e.g., a notice in the preview overlay).

---

## 6. Visual Affordance for Preview Mode

The following visual affordances mark the app as being in preview:

### 6.1 Required

- **Preview banner** — a persistent preview-status banner labeling the current
  graph state as "Viewing history" and showing the timestamp/step of the
  preview position. It may live in the History Manager pane or a detached
  overlay, but it must remain visible while preview is active without requiring
  row-by-row inspection of the timeline list.
- **Scrubber timeline** — a horizontal scrubber bar in the History Manager pane
  (or detached overlay) showing position within the timeline index.
- **Return to present button** — a labelled action target that emits
  `ExitHistoryTimelinePreview`. Must be reachable from both the banner and
  the scrubber area.

### 6.2 Optional (not gated on Stage F landing)

- **Dimmed live-graph affordance** — non-preview UI chrome dims or is marked
  as locked during preview.
- **Timestamp annotation on hovered node** — shows the WAL timestamp of the
  most recent event for the hovered node at the current cursor position.
- **Isolation-violation toast** — transient notification when a blocked intent
  is attempted during preview.

---

## 7. Exiting Preview (Return to Present)

### 7.1 Exit Path

Preview is exited via `GraphIntent::ExitHistoryTimelinePreview`.

Runtime behavior:

1. `history_preview_live_graph_snapshot` is taken and restored to
   `workspace.domain.graph`.
2. `history_preview_mode_active = false`
3. `history_replay_in_progress = false`
4. `history_preview_graph = None`
5. `history_last_return_to_present_result = Some("restored")`
6. `CHANNEL_HISTORY_TIMELINE_PREVIEW_EXITED` diagnostic emitted

If `history_preview_live_graph_snapshot` is `None` at exit (unexpected), exit
still clears preview mode — but `last_return_to_present_result` is NOT set to
"restored", and `CHANNEL_HISTORY_TIMELINE_RETURN_TO_PRESENT_FAILED` should be
emitted.

### 7.2 What "Return to Present" Restores

| State | Restored? |
|---|---|
| `workspace.domain.graph` | Yes — from snapshot |
| Renderer/webview live instances | Not affected (preview never touched them) |
| Camera position | Not restored (camera changes during preview are kept) |
| Node selection | Not restored (selection changes during preview are kept) |
| Workbench layout | Not affected |
| WAL | Not affected (no WAL writes occurred during preview) |

Camera and selection are intentionally not restored: if a user navigated to a
different graph location while in preview, returning to present should leave
them at that location, not snap them back to where they were before preview.

### 7.3 Return-to-Present Failure

`GraphIntent::HistoryTimelineReturnToPresentFailed { detail: String }` records
a failure path. This intent exists for caller-side orchestration failures
(e.g., an external process attempted to exit preview and failed). It does not
itself clear preview mode — the caller should emit `ExitHistoryTimelinePreview`
for the normal exit path and only emit `ReturnToPresentFailed` if the restore
itself could not be completed.

---

## 8. Isolation Invariants

These invariants must hold at all times. Tests in `§9` cover them.

1. **No WAL writes in preview** — `log_mutation` must not be called while
   `history_preview_mode_active = true`. The block-intent gate in
   `apply_reducer_intent_internal` prevents graph-mutation intents from
   reaching the WAL path.
2. **No webview lifecycle mutations in preview** — `as_runtime_event()` intents
   are blocked (§5.1). The reconciler in `reconcile_webview_lifecycle` should
   additionally check `history_preview_mode_active` before acting on lifecycle
   deltas.
3. **No live graph mutations in preview** — `workspace.domain.graph` must
   equal `history_preview_live_graph_snapshot` at all times during preview.
   Any mutation path that bypasses the intent gate (direct field write, etc.)
   is a violation.
4. **Snapshot validity** — `history_preview_live_graph_snapshot` must be
   populated at all times when `history_preview_mode_active = true`. If it is
   `None` during preview, that is an invariant violation.
5. **Clean return** — After `ExitHistoryTimelinePreview`, all preview fields
   must be cleared. No `history_preview_graph` or snapshot leak into live state.

---

## 9. Acceptance Criteria

### Stage F Baseline (must pass before merging)

- [ ] `EnterHistoryTimelinePreview` snapshots live graph, sets preview active
- [ ] `ExitHistoryTimelinePreview` restores from snapshot, clears all preview fields
- [ ] Blocked intents (graph mutations, runtime events, view writes) do not
      mutate graph state during preview and record an isolation violation
- [ ] `HistoryTimelineReplayAdvance` moves cursor forward; graph reflects step
- [ ] `HistoryTimelineReplayReset` returns cursor to 0 without exiting preview
- [ ] Preview mode does not write WAL entries
- [ ] All required diagnostic channels emit on their respective events
- [ ] Isolation violation does not crash or corrupt state

### Tests

Covered by existing tests in `graph_app.rs`:

- `history_health_summary_tracks_preview_and_return_to_present_failure`
- `history_preview_blocks_graph_mutations_and_records_isolation_violation`
- replay advance / set-total / reset sequence tests

### Runtime Parity

`history_health_summary()` must expose:

- `preview_mode_active: bool`
- `last_preview_isolation_violation: bool`
- `replay_cursor: Option<usize>`
- `replay_total_steps: Option<usize>`
- `replay_in_progress: bool`
- `last_return_to_present_result: Option<String>`

These drive the subsystem diagnostics pane (§5.2 of `SUBSYSTEM_HISTORY.md`).

---

## 10. Out of Scope (Stage F)

The following are explicitly deferred:

- Mixed-history timeline showing traversal + node-navigation + audit events
  on the same scrubber (see `2026-03-08_unified_history_architecture_plan.md §6`)
- Node audit history surfaces
- NodeNavigationHistory timeline
- Branched or writable historical graph states ("edit from history")
- Replay of webview/renderer state alongside graph state
- Node-level timestamp annotations in the canvas during preview
