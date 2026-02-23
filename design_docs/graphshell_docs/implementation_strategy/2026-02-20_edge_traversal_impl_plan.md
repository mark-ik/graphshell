<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Edge Traversal Model — Implementation Plan (2026-02-20)

**Status**: Refactored (2026-02-22) into architecture-first staged delivery plan
**Related plans**:
- `2026-02-22_workbench_workspace_manifest_persistence_plan.md` (archived; manifest migration record)
- `2026-02-22_workbench_tab_semantics_overlay_and_promotion_plan.md` (active workbench tab semantics follow-on)

---

## Purpose

This document translates the traversal-model research into an implementation sequence that fits
Graphshell's current architecture:

- reducer/policy in `app.rs`
- rendering in `render/*` and `graph/*`
- persistence/WAL in `persistence/*`
- frame/effect orchestration in `desktop/*`

The goal is to avoid a "data model first, UI later" rewrite that cuts across these seams.

---

## Current Reality (2026-02-22)

Traversal semantics are not yet implemented.

Current behavior still reflects the pre-traversal model:

- history traversal edges are deduplicated (repeat navigations discarded)
- edge semantics are not yet split into structure vs temporal traversal records
- traversal events are not first-class WAL entries
- no History Manager UI
- no timeline scrubber / preview mode

This refactor keeps the original scope, but reorganizes it to better exploit the existing
architecture and reduce migration risk.

---

## Progress Snapshot (2026-02-22)

Implementation status:

- **Stages 0-D are complete**
- Active stage: **Stage D complete, ready for Stage E**
- Completed stages: **Stage 0 (Design Lock), Stage A (Data Model), Stage B (WAL Integration), Stage C (Rendering), Stage D (History Panel PoC)**

### Stage D completion summary (2026-02-22):

Delivered:
1. "History Panel" toolbar entry added to settings menu
2. Non-modal "Navigation History" panel showing last 50 traversals
3. Traversals sorted by timestamp descending with relative time display
4. Row click focuses and zooms to source node
5. Traversal trigger indicators (Back/Forward/Unknown)

Implementation notes:
- Panel queries all edges via `graph.inner.edge_references()` and collects traversals
- Displays intuitive relative time labels ("just now", "5m ago", "2h ago", etc.)
- Click handler uses `SelectNode` + `RequestZoomToSelected` intents
- Panel state managed via `show_traversal_history_panel` boolean flag
- Rendering integrated into `gui_frame.rs` after help panel

All Stage D validation criteria met:
- ✅ Navigate A → B and verify traversal appears in panel (compile-time validated)
- ✅ End-to-end data flow proven: `push_traversal` → WAL → replayable state → UI listing

Next recommended work: Stage E (History Manager with tiered storage and dissolution)

### Stage E kickoff summary (2026-02-23):

Delivered (storage foundation slice):
1. Added dedicated persistence keyspaces: `traversal_archive` and `dissolved_archive`.
2. Added `GraphStore` archive APIs for traversal entries:
  - `archive_append_traversal(...)`
  - `archive_dissolved_traversal(...)`
3. Added archive-keyspace smoke coverage for both paths in `persistence/mod.rs` tests.

Notes:
- This is an infrastructure kickoff only; transfer ordering, archival passes, and History Manager UI tabs remain in Stage E remaining scope.

Update this section when Stage E work begins.

---

## Scope and Non-Goals

### In Scope

1. Replace edge semantic model (`EdgeType` -> traversal-capable payload)
2. Record repeated traversals with trigger + timestamp
3. Persist traversal events in WAL
4. Render traversal-aware edge visuals
5. Ship a minimal History panel (PoC)
6. Follow-on History Manager and temporal navigation

### Out of Scope (This Doc)

1. Full command-palette redesign
2. Unrelated graph UX polish items that do not depend on traversal semantics
3. Multi-window architecture changes
4. Webview/embedder decomposition changes

---

## Architectural Boundaries (How to Build This Without Fighting the Codebase)

### `app.rs` (authoritative traversal policy and mutation)

Owns:

- traversal append decision rules (inter-node vs intra-node, self-loop skip, unknown-node skip)
- URL capture ordering correctness in `WebViewUrlChanged`
- edge payload mutation (`push_traversal`)
- reducer-side intent handling and trigger classification

Rules:

- traversal append logic should live in one function (`push_traversal`)
- UI/render code must not mutate traversal state directly
- replay paths should reuse the same append semantics when possible

### `graph/mod.rs` (data model + graph primitives)

Owns:

- `Traversal`, `NavigationTrigger`, `EdgePayload`
- `StableGraph<Node, EdgePayload, ...>`
- edge mutation/query helpers (e.g. `get_edge_mut`)

Rules:

- `EdgePayload` encodes both structure and temporal data
- display-only computations (dominant direction, stroke width) should not be stored here

### `persistence/*` (WAL and storage semantics)

Owns:

- `LogEntry::{AppendTraversal, AssertEdge, RetractEdge}` additions
- replay semantics for traversal events
- later: cold archive/dissolved archive keyspaces and export helpers

Rules:

- in-memory mutation and WAL append order must be explicit and tested
- crash/recovery semantics for hot/cold archival must be treated as persistence work, not UI work

### `render/*` + `graph/egui_adapter.rs` (presentation only)

Owns:

- traversal-aware edge display deduplication
- edge tooltip/inspection stub
- History panel UI presentation (PoC)
- later: timeline controls and preview affordances

Rules:

- render layer derives visuals from `EdgePayload`; it does not define traversal truth
- keep rendering calculations deterministic and local (e.g. width mapping, direction thresholds)

### `desktop/gui_frame.rs` (orchestration / effect sequencing)

Owns:

- wiring UI requests to app intents
- later: timeline preview mode switching and frame-level effect suppression

Rules:

- preview mode must not accidentally drive live persistence/webview effects

---

## Integration Constraints (Cross-Cutting Guardrails)

These constraints are intended to preserve a single traversal truth path and prevent preview/UI
features from creating alternate mutation paths or side effects while the stages land.

### 1. Intent / reducer boundary discipline

- `render/*` and UI surfaces request actions and render traversal-derived state; they must not
  mutate traversal truth directly.
- `app.rs` remains the single authority for traversal append semantics and traversal-related policy.
- `desktop/*` orchestrates effects and preview-mode suppression, but does not redefine traversal truth.

### 2. Traversal truth source-of-truth discipline

- `EdgePayload` + WAL replay define traversal truth.
- History/Timeline features are consumers of traversal truth, not alternate stores.
- Rendered aggregates (stroke width, dominant direction, counts) remain derived/transient.

### 3. Ordering and durability discipline

- In-memory traversal mutation and WAL append order must be explicit and test-covered.
- Replay must deterministically reconstruct traversal state.
- Archive/dissolution crash-order guarantees remain persistence responsibilities, not UI responsibilities.

### 4. Preview-mode effect isolation

- Preview mode must not emit live WAL writes, webview lifecycle transitions, or live app mutations.
- Preview state should be isolated from live traversal append paths and effect dispatch.

### 5. Incremental migration rule

- Each stage should preserve one coherent traversal append/replay path.
- Avoid temporary duplicate traversal write paths unless one is clearly transitional and test-covered.

---

## Design Contracts (Must Hold Across Phases)

1. Repeated navigations between the same two known nodes append traversal records; they do not get deduplicated away.
2. Traversal records are temporal events, distinct from user-asserted structural edges.
3. Traversal append correctness depends on URL capture ordering (`prior_url` before URL mutation).
4. Self-loops and intra-node URL changes do not create traversal records.
5. Traversal replay from WAL reproduces in-memory traversal state deterministically.
6. Rendered edge directionality is derived from traversal aggregates, not stored as edge truth.
7. UI history/timeline features consume traversal data; they do not become alternate sources of truth.

---

## Delivery Strategy (Architecture-First Stages)

This is the same functional scope as the original plan, but sequenced around stable integration
boundaries.

### Stage 0 (Recommended): Traversal Model Decision Lock (Design Prep)

Goal:

- lock core traversal/event semantics before Stage A/B type and WAL migrations to reduce churn

Primary modules (design targets):

- `graph/mod.rs`
- `app.rs`
- `persistence/*` (`LogEntry` / replay semantics)

Deliverables:

1. Finalized `Traversal` record shape (v1 fields)
2. Finalized `NavigationTrigger` scope for v1
3. Finalized `EdgePayload` split (structural assertions vs traversal events)
4. WAL event semantics and replay rules (`AppendTraversal`, `AssertEdge`, `RetractEdge`)
5. Explicit URL-capture ordering contract for `WebViewUrlChanged`

Tasks:

- [x] Document final `Traversal` / `EdgePayload` v1 field set
- [x] Document v1 trigger taxonomy and deferred variants
- [x] Document append/replay ordering rules (in-memory vs WAL)
- [x] Confirm repeat traversal append policy (no dedup overwrite)
- [x] Confirm Stage A/B validation list matches the locked model

Stage 0 acceptance:

- Stage A/B coding can begin without unresolved semantic ambiguity in payload/WAL shape

Stage 0 decisions (locked for Stage A/B unless implementation blocker is found):

1. `Traversal` (v1 minimum fields)
- `timestamp_ms: u64` (Unix epoch milliseconds)
- `trigger: NavigationTrigger`

Rationale:
- enough to support repeat traversal recording, ordering, replay, and first rendering/UI aggregates
- URL/source/destination are encoded by the edge endpoints, so they do not belong in `Traversal`

2. `NavigationTrigger` (v1 scope)
- `Unknown`
- `Back`
- `Forward`

Deferred variants (explicitly not required for Stage A/B):
- `LinkClick`
- `AddressBar`
- `ScriptRedirect`
- `Reload`
- `SessionRestore`

Rationale:
- current app signal path can confidently infer back/forward-like transitions from history index
  movement, but richer trigger provenance is not yet available across all navigation paths

3. `EdgePayload` split (v1)
- structural assertions and traversal events are stored together on the edge payload
- v1 structural assertions are boolean flags:
  - `hyperlink_asserted: bool`
  - `user_grouped_asserted: bool`
- temporal traversal events stored as:
  - `traversals: Vec<Traversal>`

Rationale:
- avoids forcing traversal truth into the old `EdgeType::History` bucket
- preserves user assertions independently of traversal events
- supports edges that are both user-asserted and traversal-active

4. Repeat traversal policy (v1)
- repeated traversals append `Traversal` records to the existing edge payload; they are not deduplicated
- self-loops and unknown destination/source mappings are skipped
- intra-node URL changes (same node logical destination) do not append traversal records

5. Append + WAL ordering rule (Stage A/B target contract)
- Stage A (pre-WAL integration): in-memory append only
- Stage B+ (with WAL): mutation helper defines explicit order and tests assert it
- replay must reconstruct the same traversal sequence deterministically

6. `WebViewUrlChanged` / history ordering contract
- capture prior history state (`old_entries`, `old_index`) before mutating node history state
- traversal append decision is derived from old->new history transition
- URL/title/session state updates must not erase the prior traversal context before append decision

7. Compatibility/migration note for Stage A
- Stage A may keep compatibility helper methods for old edge-style callers while migrating to
  `EdgePayload`, but traversal truth must flow through `push_traversal(...)` only

### Stage A: Core Traversal Data Model + Reducer Integration (PoC Foundation)

Goal:

- establish traversal-capable edge payloads and append semantics without UI/storage complexity

Primary modules:

- `graph/mod.rs`
- `app.rs`

Deliverables:

1. `EdgeType` replaced by `EdgePayload`
2. `Traversal` + `NavigationTrigger` introduced
3. `push_traversal(...)` added in `app.rs`
4. `WebViewUrlChanged` path uses correct URL capture ordering
5. existing history-edge dedup path removed/replaced

Notes:

- Keep scope tight: no History Manager UI yet, no cold archive yet
- Focus on type migration and reducer correctness first

Core PoC tasks:

- [ ] Add `Traversal`, `NavigationTrigger`, `EdgePayload` to `graph/mod.rs`
- [ ] Migrate `StableGraph<Node, EdgeType, Directed>` to `StableGraph<Node, EdgePayload, Directed>`
- [ ] Update all `add_edge` callsites to construct `EdgePayload`
- [ ] Add `Graph::get_edge_mut(...) -> Option<&mut EdgePayload>`
- [ ] Add `GraphBrowserApp::push_traversal(...)`
- [ ] Replace `maybe_add_history_traversal_edge` call path with `push_traversal`
- [ ] Enforce URL capture ordering in `WebViewUrlChanged`

Stage A validation:

- `test_push_traversal_appends_to_existing_edge`
- `test_push_traversal_creates_edge_if_absent`
- `test_push_traversal_skips_self_loop`
- `test_push_traversal_skips_unknown_destination`
- `test_push_traversal_url_capture_ordering`
- updated graph tests for `EdgePayload`

### Stage B: WAL Integration for Traversal Truth

Goal:

- make traversal events durable and replayable before building UI around them

Primary modules:

- `persistence/mod.rs` (and WAL replay path)
- `app.rs`

Deliverables:

1. `LogEntry::AppendTraversal`
2. `LogEntry::AssertEdge`
3. `LogEntry::RetractEdge`
4. replay support that reconstructs traversal payload state

Why this stage comes before UI:

- History/Timeline UI without durable traversal events would be a throwaway integration

Tasks:

- [ ] Add `AppendTraversal`, `AssertEdge`, `RetractEdge` variants to `LogEntry`
- [ ] Write `AppendTraversal` from `push_traversal`
- [ ] Replay `AppendTraversal` in WAL replay path
- [ ] Write/replay `AssertEdge` and `RetractEdge` for user-asserted edge semantics
- [ ] Update persistence tests for expanded `LogEntry` enum

Stage B validation:

- `test_append_traversal_log_entry_replays_correctly`
- `test_assert_edge_log_entry_creates_user_asserted_edge`
- `test_retract_edge_clears_user_asserted`

### Stage C: Traversal-Aware Rendering + Inspection Stub (First User-Visible Value)

Goal:

- expose traversal semantics in graph view using existing render/adapter architecture

Primary modules:

- `graph/egui_adapter.rs`
- `render/mod.rs`

Deliverables:

1. logical-edge display dedup (A<->B pair rendered once)
2. traversal-count weighted strokes
3. dominant-direction arrows from traversal ratios
4. edge hover tooltip with counts/timestamp summary

Tasks:

- [ ] Add logical-edge pair dedup in graph adapter edge iteration
- [ ] Compute directional traversal aggregates (`ab`, `ba`)
- [ ] Map aggregate traversal count to stroke width (deterministic function)
- [ ] Render dominant direction using threshold policy (default >60%)
- [ ] Add edge hover tooltip/inspection stub in `render/mod.rs`

Design note:

- Keep threshold/width mapping local and documented; avoid embedding policy into `EdgePayload`

Stage C validation:

- `test_display_dedup_skips_reverse_pair`
- `test_dominant_direction_above_threshold`
- `test_dominant_direction_below_threshold`
- `test_traversal_count_drives_stroke_width`
- headed hover tooltip sanity check

### Stage D: History Panel PoC (UI Consumer, No Cold Tier Yet)

Goal:

- prove end-to-end data flow (`push_traversal` -> WAL -> replayable state -> UI listing)

Primary modules:

- `render/mod.rs` and/or toolbar UI modules
- `app.rs` (read-only query helpers if needed)

Deliverables:

1. "History" panel/window listing recent traversals from hot in-memory edge payloads
2. row click focuses/pans to source node

Tasks:

- [x] Add History entry to toolbar/menu
- [x] Add non-modal "Traversal History" panel (last N traversals, e.g. 50)
- [x] Sort by timestamp descending
- [x] Row click emits focus intent

Stage D validation:

- [x] headed: navigate A -> B and verify traversal appears in panel

**Status: COMPLETE (2026-02-22)**

### Stage E: History Manager (Tiered Storage + Dissolution + Full UI)

Goal:

- durable long-term traversal storage and lifecycle management

Primary modules:

- `persistence/*`
- `app.rs` (remove-node ordering hooks)
- UI surface for history manager

Sub-areas:

1. Hot/cold tiered storage (`EdgePayload.traversals` + `traversal_archive`)
2. Dissolution transfer for removed edges/nodes (`dissolved_archive`)
3. Full History Manager UI (timeline, dissolved, delete, export, curation)

Key ordering contracts:

- archive writes fsync before hot-tier removal/count mutation
- dissolution transfer fsync before petgraph removal

Representative tasks:

- [x] Add `traversal_archive` keyspace and archival pass (kickoff storage API in place)
- [ ] Add recovery scan for archived traversal counts (snapshot-absent path)
- [ ] Implement dissolution transfer on edge/node removal
- [ ] Add History Manager UI tabs (Timeline, Dissolved)
- [ ] Add delete, auto-curation, export

Representative validation:

- archival transfer/count reconciliation tests
- dissolution ordering tests
- headed UI workflows for Timeline/Dissolved tabs

Minimum shippable boundary (Stage E):

- tiered persistence + recovery correctness may ship before the full History Manager UI is complete
- UI sub-surfaces (Timeline/Dissolved/delete/export/curation) may be staged as long as storage and
  recovery invariants are complete and test-covered

### Stage F: Temporal Navigation (Preview Mode)

Goal:

- time-scrub graph state via WAL replay without mutating live runtime state

Primary modules:

- `persistence/*` (timeline index + replay helper)
- `app.rs` / top-level state (preview state)
- `render/*` (timeline UI + ghost visuals)
- `desktop/gui_frame.rs` (effect suppression / preview orchestration)

Deliverables:

1. timeline index (`timestamp -> WAL position`)
2. `replay_to_timestamp(...)`
3. preview-only graph state
4. timeline slider + "Return to present"
5. ghost rendering in preview mode

Critical architectural rule:

- preview mode operates on a copy and must not trigger WAL writes, webview lifecycle actions, or
  live app mutations

Tasks:

- [ ] Build timeline index from WAL
- [ ] Implement replay-to-timestamp from nearest snapshot
- [ ] Add preview state container
- [ ] Gate persistence/webview side effects while preview is active
- [ ] Add timeline UI controls and ghost rendering

Stage F validation:

- `test_replay_to_timestamp_produces_subset_of_full_graph`
- `test_preview_mode_does_not_write_wal`
- `test_close_timeline_preview_restores_live_state`
- headed slider/return-to-present flow

Minimum shippable boundary (Stage F):

- preview-state isolation + side-effect suppression correctness ships before timeline UX polish
- scrubber/ghost UI polish is secondary to proving non-live mode cannot mutate persistence/runtime

---

## Feature Coupling (Absorb Traversal-Dependent UX Work Here)

These should remain coupled to this traversal plan instead of separate UX polish tasks:

1. neighborhood focus/filter behavior that depends on traversal topology or time
2. edge filtering semantics once `EdgePayload` exists
3. faceted search dimensions using traversal metadata (count, recency, trigger)
4. relevance weighting that incorporates traversal frequency/recency

Reason:

- these features depend on traversal truth and should be designed against the same data model and
  persistence semantics, not retrofitted later

---

## Implementation Leverage (Use Existing Features/Patterns)

Use the current architecture to reduce risk:

- reducer-first mutations in `app.rs` (mirrors lifecycle/routing work)
- WAL replay infrastructure already exists; extend `LogEntry` instead of adding parallel logs
- graph adapter layer (`graph/egui_adapter.rs`) is the correct place for traversal visual derivation
- render modules can host PoC panels/tooltips without changing graph data ownership
- `desktop/gui_frame.rs` is the right place to enforce preview-mode effect suppression

Anti-patterns to avoid:

- storing display-derived metrics (dominant direction, width) in `EdgePayload`
- building History UI on ad hoc scans that ignore WAL/persistence semantics
- timeline preview mutating live `GraphBrowserApp` or emitting side-effecting intents

---

## Risks and Mitigations

### Risk: Broad `EdgeType` -> `EdgePayload` migration churn

Mitigation:

- isolate Stage A to type migration + reducer logic only
- land rendering and WAL changes in separate stages
- update `Progress Snapshot` when stage scope changes materially

### Risk: URL capture ordering bugs create incorrect traversals

Mitigation:

- explicit unit test for prior-url capture ordering
- code comments at `WebViewUrlChanged` call site

### Risk: Timeline preview accidentally writes persistence or triggers webviews

Mitigation:

- explicit preview-mode gating in orchestrator/effect paths
- test asserting no WAL writes during preview

### Risk: Hot/cold archival corruption under crash ordering

Mitigation:

- fsync ordering contract + fault-injection integration test before enabling by default

---

## Research Cross-References

- `2026-02-20_edge_traversal_model_research.md`:
  - edge payload and trigger model
  - inter/intra traversal decision rule
  - persistence additions (`AppendTraversal`, `AssertEdge`, `RetractEdge`)
  - display dedup and dominant-direction thresholding
  - hot/cold storage and dissolution transfer

---

## Progress

### 2026-02-20 — Session 1 (Original Draft)

- Plan created from traversal model research.
- Phase-based scope defined (PoC, History Manager, Temporal Navigation).
- Implementation not started.

### 2026-02-22 — Refactor

- Reworked into architecture-first staged delivery plan aligned with current module boundaries.
- Made reducer/render/persistence/desktop ownership explicit.
- Preserved original feature scope while improving sequencing for lower migration risk.
