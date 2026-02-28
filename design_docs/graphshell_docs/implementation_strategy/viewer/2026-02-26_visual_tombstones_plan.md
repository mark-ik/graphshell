# Visual Tombstones Implementation Plan

**Status**: Strategy / Phased Deliverable
**Created**: 2026-02-26
**Author**: Arc (tracking as per RFC #11)
**Research Source**: `design_docs/graphshell_docs/research/2026-02-24_visual_tombstones_research.md`

---

## Executive Summary

Visual tombstones (ghost nodes) preserve graph structure when users delete nodes, preventing topology loss during refactoring, pruning, or history operations. This plan elevates tombstones from research concept to a tracked, phased deliverable with explicit toggle/retention semantics and integration milestones.

**Scope Alignment**: 
- Feature tracks as a **Graph UX Polish** sub-feature, not a critical-path item.
- Initial scope targets Servo/composited viewer tiles (node content preservation is less critical when node display is ephemeral).
- Versioning: Phase 1 (toggle + basic ghost render) → Phase 2 (restoration/GC) → Phase 3 (archive/expiry policy).

---

## Phase 1: Toggle + Ghost Rendering (MVP)

### Scope

Introduce tombstones as a toggleable visual artifact with minimal data model changes.

#### 1.1 Data Model

**New State**: Add `NodeState::Tombstone` variant (lifecycle state, complementing `Active`/`Warm`/`Cold`).

```rust
pub enum NodeState {
    Active,
    Warm,
    Cold,
    Tombstone,  // NEW: Deleted node with preserved structure
}
```

**Tombstone Payload**:
```rust
pub struct TombstoneNodeData {
    pub id: NodeKey,
    pub position: Vec2,  // Preserve spatial anchor
    pub title: Option<String>,  // Optional memo label
    pub edges: Vec<(NodeKey, RelationshipKind)>,  // Preserve topology
    pub deleted_at: DateTime,  // Track deletion timestamp for GC
}
```

**Storage**: Tombstones are persisted in the graph persistence layer alongside active nodes. Queries default to filtering out `Tombstone` state unless explicitly queried (e.g., `graph.nodes_with_state(NodeState::Tombstone)`).

#### 1.2 Rendering

**Visual Design**:
- **Node**: Render as a faint dashed-outline square (40×40 px, 50% opacity, stroke weight 1 px) with a small "×" center mark.
- **Edges**: Connected edges render as dashed/faded lines using a new `EdgeStyle::Ghost` variant (dashed dash pattern, 30% opacity, same color as node outline).
- **Z-order**: Tombstones render *below* active nodes so they don't interfere with navigation.
- **Hover**: Non-interactive for pan/zoom/content navigation. Cursor shows restore/delete options.

**Implementation**:

Add `show_tombstones: bool` field to `GraphViewState` (or appropriate view config struct):

```rust
pub struct GraphViewState {
    // ... existing fields ...
    pub show_tombstones: bool,  // Default: false (hidden by default)
}
```

Update tile render pass to conditionally render tombstone glyphs:

```rust
fn render_node_in_tile(node: &NodeState, config: &GraphViewState) {
    match node {
        NodeState::Tombstone if !config.show_tombstones => {
            // Skip rendering; node is invisible when toggle is off
        }
        NodeState::Tombstone => {
            // Render dashed outline + "×" marker
            render_tombstone_glyph(&node.position, &node.title);
        }
        _ => {
            // Render active node normally
            render_active_node(node);
        }
    }
}
```

#### 1.3 UI Toggle

Add "Show Deleted" toggle to **Graph View Settings Panel** (assumed to exist as part of ongoing Graph UX work):

```
┌─ Graph View Settings ─────────────┐
│ ☐ Show Archived                   │
│ ☑ Show Deleted (ghost nodes)      │  <- NEW
│ ☐ Lock Physics                    │
│ [Advanced]                        │
└───────────────────────────────────┘
```

**Default State**: Off (tombstones hidden until explicitly enabled).

**Interaction**: Toggle is checked/unchecked via a simple boolean control; changes are reflected immediately on the graph canvas and persisted in user preferences (`prefs.rs` or equivalent).

#### 1.4 Acceptance Criteria (Phase 1)

- ✅ `NodeState::Tombstone` state added and persisted without breaking active-node queries.
- ✅ "Show Deleted" toggle exists in Graph View Settings and controls tombstone visibility.
- ✅ Tombstone glyphs (dashed outline, "×" marker) render correctly when toggle is on.
- ✅ Ghost edges render as dashed/faded lines.
- ✅ All existing tests pass; no regressions in active node rendering.
- ✅ Performance baseline: rendering 100+ tombstones does not cause visible frame drops.

---

## Phase 2: Restoration + Explicit Deletion

### Scope

Add user affordances for restoring or permanently deleting tombstone nodes.

#### 2.1 Interaction Design

**Right-Click Context Menu** (on tombstone glyph):

```
┌─ Ghost Node ───────────────────┐
│ [↻ Restore]                    │  Converts Tombstone → Active
│ [🗑 Permanently Delete]         │  Removes node and edges
│ [✎ Add Memo]                   │  Adds/edits title
└────────────────────────────────┘
```

**Keyboard Shortcut** (optional, Phase 2+):
- `U` (Undo deletion) when hovering tombstone (restores the most recent tombstone).

#### 2.2 Restoration Logic

**Restore Action**:
1. Restore `TombstoneNodeData` to `NodeState::Active` with cleared timestamp.
2. Re-fetch or re-queue the node's web content (if content materialization is lazy).
3. Emit diagnostics event: `graph.tombstone_restored(node_id, restore_timestamp)`.

**Edge Restoration**:
- All preserved edges are restored as active relationships.
- If target node is still tombstoned, show a warning: "Linking to deleted node X; restore it first?"

#### 2.3 Acceptance Criteria (Phase 2)

- ✅ Right-click on tombstone shows "Restore" / "Permanently Delete" options.
- ✅ Restore converts `Tombstone` → `Active` and re-materializes content.
- ✅ Permanent Delete removes node + edges from graph and persistence.
- ✅ Diagnostics emit `graph.tombstone_restored` and `graph.tombstone_deleted` events.
- ✅ No regressions in active graph operations.

---

## Phase 3: Retention Policy + Garbage Collection

### Scope

Define automatic cleanup semantics for aged tombstones.

#### 3.1 Retention Policy

**Configuration**:

Add user-configurable retention policy to prefs:

```rust
pub struct TombstoneRetentionPolicy {
    pub enabled: bool,  // Default: true
    pub max_age_days: u32,  // Default: 30 days
    pub manual_clear_on_exit: bool,  // Default: false (don't auto-clear; require explicit action)
}
```

**Semantics**:
- **Retention Period**: Tombstones older than `max_age_days` are candidates for GC.
- **GC Trigger**: On app startup or at configurable intervals (e.g., weekly).
- **Notification**: When GC occurs, a non-blocking toast notification: "Cleaned up X expired ghost nodes (>30 days old)."
- **Opt-Out**: User can set `max_age_days = ∞` to disable automatic GC.

#### 3.2 Explicit Clear

**Manual Clearing**:

Add "Clear All Deleted" button to Graph View Settings:

```
[ Clear All Deleted Nodes ]  <- Deletes all tombstones immediately
```

Confirmation dialog:

```
┌─ Confirm Cleanup ───────────────────────┐
│ Delete 5 ghost nodes?                   │
│ This action cannot be undone.           │
│ [Cancel]  [Delete]                      │
└─────────────────────────────────────────┘
```

#### 3.3 Acceptance Criteria (Phase 3)

- ✅ Retention policy is configurable via prefs (max age, GC enabled/disabled).
- ✅ GC runs on startup; aged tombstones are removed silently.
- ✅ "Clear All Deleted" button offers explicit cleanup with confirmation.
- ✅ Diagnostics emit `graph.tombstones_garbage_collected(count, age_threshold)` events.
- ✅ No performance regression from GC operations on large graphs (1000+ nodes).

---

## Phase-Independent Concerns

### Persistence

**Storage Format**:
- Tombstones are stored in the graph persistence layer alongside active nodes, with `NodeState::Tombstone` discriminant.
- No separate serialization; leverage existing persistence infrastructure.
- Round-trip: load + save on app exit preserves tombstones unless GC removes them.

**Migration**: If upgrading from a version without tombstone support, existing deleted nodes (if tracked) can be materialized as tombstones; otherwise, no migration needed (clean slate).

### Diagnostics

New diagnostics channels (to be registered in `registries/atomic/diagnostics.rs`):

| Channel | Severity | Trigger |
| --- | --- | --- |
| `graph.tombstone_created` | Info | User deletes a node |
| `graph.tombstone_restored` | Info | User restores a tombstone |
| `graph.tombstone_deleted` | Info | User permanently deletes a tombstone |
| `graph.tombstones_garbage_collected` | Info | GC removes aged tombstones |

### Edge Cases

1. **Delete Node with Unsaved Content**: If user deletes a node that has unsaved edits, emit a warning diagnostic before tombstone is created: "Node X has unsaved changes; delete anyway?" User can cancel or confirm.

2. **Restore → Re-fetch Failure**: If restored node's web content fails to load, show placeholder + error diagnostic. Node remains active but content is unavailable (same as any failed load).

3. **Orphaned Edges**: When tombstone points to another tombstone, show a deferred resolution UI: "Links to deleted Y; Y is also deleted. Restore Y first or break link?"

4. **Bulk Delete**: When user selects multiple nodes and deletes, all become tombstones together. Show: "Created 5 ghost nodes."

---

## Integration Points

### Graph View State

Add `show_tombstones: bool` to `GraphViewState` (or tile render config). Tie toggle interaction to state updates:

```rust
impl GraphViewState {
    pub fn toggle_show_tombstones(&mut self) {
        self.show_tombstones = !self.show_tombstones;
        // Emit UI update signal
    }
}
```

### Persistence Layer

Leverage existing `graph.persist()` infrastructure; no new save mechanism needed. Tombstones are nodes with `state: Tombstone`.

### Diagnostics Integration

Register tombstone channels in Phase 1; emit events as per Phases 1–3.

### Badge/Tagging System

Tombstones can carry an implicit `#deleted` tag (or similar) for filtering. If a future "tag by deletion status" feature lands, tombstones provide a natural data source.

### Edge Traversal

If edge traversal (`lane:traversal`) is active, tombstones should appear in traversal history even if their content is gone. Traversal past a tombstone shows: "Node deleted on X; was titled Y."

---

## Non-Goals

- **Anonymous Tombstones**: Tombstones are not used for P2P ghost/conflict nodes (those are a Verse concern, distinct from this UX feature).
- **Undo History**: Tombstones are not an undo/redo mechanism; they are persistent deletions with optional restoration. Full undo would require a separate history/checkpoint system.
- **Tombstone-Only Mode**: Users cannot hide all active nodes and show only tombstones; the toggle is an overlay, not a filter mode.

---

## Timeline

| Phase | Effort (est.) | Blocker | Sequencing |
| --- | --- | --- | --- |
| Phase 1 (MVP) | 2–3 weeks | None | Can start immediately after this plan is accepted |
| Phase 2 (Restoration) | 1–2 weeks | Phase 1 complete | Can run in parallel with other UI polish |
| Phase 3 (GC) | 1 week | Phase 1 complete | Nice-to-have for Phase 1 close-out; can defer to polish cycle |

**Recommended Sequencing**: Phase 1 → Phase 2 (parallel to other Graph UX work) → Phase 3 (polish).

---

## Success Metrics

- **Adoption**: If toggle defaults to off, track what % of users enable it after N months. Target: ≥20% adoption.
- **Performance**: Graph render time with 100+ tombstones visible should not exceed baseline by >10%.
- **Reliability**: GC should not corrupt graph state; round-trip (save + load) should preserve tombstone state perfectly.

---

## References

- **Research**: `design_docs/graphshell_docs/research/2026-02-24_visual_tombstones_research.md`
- **Issue**: #11 (Concept Adoption / Visual Tombstones)
- **Related Lanes**: `lane:graph-ui`, `lane:ux-polish`
