# `graph_app.rs` Decomposition Plan

**Date**: 2026-03-08  
**Status**: Active; Stages 1-4 landed and wired  
**Primary hotspot**: `graph_app.rs`  
**Related**:
- `../technical_architecture/ARCHITECTURAL_CONCERNS.md`
- `2026-03-06_foundational_reset_implementation_plan.md`
- `2026-03-06_foundational_reset_graphbrowserapp_field_ownership_map.md`

---

## 1. Problem Statement

`graph_app.rs` had grown to 14,714 lines and mixed several incompatible responsibilities:

- reusable state/value types
- selection state and clipboard selection helpers
- bridge-carrier intent types and conversions
- reducer and app orchestration
- persistence/history/workspace/runtime coordination

That concentration makes it too easy for boundary changes to become "just edit the big file" work, which is the opposite of the foundational reset direction.

---

## 2. Target Shape

`graph_app.rs` remains the owning façade for `GraphBrowserApp`, `GraphWorkspace`, reducer logic, and authority-crossing methods, but supporting surfaces move behind dedicated child modules.

Initial intended layout:

```text
graph_app.rs                  owner/orchestrator
app/selection.rs             canonical selection model + undo snapshot carrier
app/intents.rs               bridge-carrier enums + conversion helpers
app/history.rs               history-only support types/helpers
app/workspace_commands.rs    pending app-command queue support
app/workspace_routing.rs     workspace/frame routing, picker, and membership support
app/workbench_commands.rs    pending workbench-intent queue support
app/focus_selection.rs       focused-view selection and transition support
app/persistence.rs           persistence/open/save/autosave helpers
```

The file is not being "split for aesthetics"; each extracted module must represent a real boundary.

Cross-cutting guardrail:

- tracing/diagnostics/perf instrumentation should not regrow as ad hoc inline clusters across unrelated reducer methods; where the signal is durable, prefer thin helper surfaces or child-module ownership so the owner file stays readable as authority logic

---

## 3. Ordered Stages

### Stage 1. Extract stable type surfaces

Move low-risk, high-fanout support surfaces out first:

- `SelectionState`
- `SelectionUpdateMode`
- clipboard selection request types
- undo snapshot carrier
- `WorkbenchIntent`, `AppCommand`, `ViewAction`, `GraphMutation`, `RuntimeEvent`, `GraphIntent`
- conversion helpers between the split intent categories

**Done gate**:

- call sites still import from `crate::app::*`
- no behavior change
- `graph_app.rs` line count reduced materially

**Status**: Landed on 2026-03-08.

Implemented files:

- `app/selection.rs`
- `app/intents.rs`

### Stage 2. Isolate history support

Move history-only enums, summaries, and preview bookkeeping helpers behind a history-focused child module.

Candidate surface:

- `HistoryManagerTab`
- `HistoryCaptureStatus`
- `HistoryTraversalFailureReason`
- `HistoryHealthSummary`

**Done gate**:

- history vocabulary no longer sits beside unrelated app-command and reducer types
- history-specific tests can target the child module directly

**Status**: Landed on 2026-03-08.

Implemented file:

- `app/history.rs`

### Stage 3. Isolate persistence/workspace save paths

Move open/save/autosave helpers and workspace layout persistence glue into a dedicated module owned by `GraphBrowserApp`.

Candidate surface:

- startup persistence open helpers
- session workspace layout save/load helpers
- autosave throttling/retention helpers

**Done gate**:

- persistence sequencing becomes locally auditable
- `graph_app.rs` no longer carries most file I/O orchestration details inline

**Status**: Landed on 2026-03-08.

Implemented file:

- `app/persistence.rs`

### Stage 4. Isolate command-queue helpers

Split pending command queue helpers and frame/workspace routing glue from the reducer body.

Candidate surface:

- `pending_app_commands` drain/enqueue/coalescing helpers
- chooser/prompt request staging helpers
- frame-open routing support
- focused-view/focus-return staging helpers that are queue-adjacent rather than reducer-core logic

**Done gate**:

- app-command queue behavior is inspectable in one module
- manual `pending_*` cleanup work becomes easier to replace with stricter queue semantics

**Status**: Landed on 2026-03-08.

Implemented file:

- `app/workspace_commands.rs`
- `app/workspace_routing.rs`
- `app/workbench_commands.rs`
- `app/focus_selection.rs`

### Stage 5. Reduce remaining owner file to authority logic

After support surfaces are extracted, `graph_app.rs` should primarily hold:

- `GraphBrowserApp`
- `GraphWorkspace`
- reducer-owned mutation orchestration
- explicit authority crossings

Selection-specific correction required in this stage:

- remove or fully demote `workspace.selected_nodes` as a compatibility mirror once remaining runtime call sites are migrated
- keep canonical graph-selection ownership in the scoped selection store rather than in ad hoc runtime mirrors
- ensure undo/redo, focused-view helpers, and persistence snapshot carriers do not reintroduce dual-authority semantics

**Exit target**:

- `graph_app.rs` under ~9k lines without hiding logic in grab-bag modules
- selection ownership is single-authority in runtime code, not canonical-plus-mirror

---

## 4. Non-Goals

- no crate split
- no rename churn at public call sites
- no semantic change to the current bridge-carrier model during Stage 1
- no "misc.rs" dumping ground

---

## 5. 2026-03-08 Implementation Receipt

Landed extraction slices:

- `app/selection.rs` now owns canonical selection state and undo snapshot carrier
- `app/intents.rs` now owns the bridge-carrier intent family and conversions
- `app/history.rs` now owns the history support vocabulary (`HistoryManagerTab`, `HistoryCaptureStatus`, `HistoryTraversalFailureReason`, `HistoryHealthSummary`)
- `app/persistence.rs` now owns startup persistence open, workspace layout save/load, autosave retention/throttling, and persisted UI settings helpers via an `include!`-backed implementation shard
- `app/workspace_commands.rs` now owns the pending app-command queue primitives, coalescing/sanitization helpers, snapshot/clipboard/data-dir command accessors, note/clip-open accessors, and tool-surface return-target staging helpers as an initial Stage 4 slice
- `app/workspace_routing.rs` now owns unsaved-prompt request resolution, workspace/frame membership and recency helpers, frame-open routing, node-context staging, chooser flows, and pending node-open/import routing helpers as a second Stage 4 slice
- `app/workbench_commands.rs` now owns the pending workbench-intent queue helpers (`enqueue`, `extend`, `take`, and test-only count inspection)
- `app/focus_selection.rs` now owns focused-view selection mirroring, focused-selection readers, and focus-transition synchronization helpers
- first Stage 5 cleanup slice landed: live selection mutation now routes through `app/focus_selection.rs`, the UX tree reads focused selection rather than the compatibility mirror, and external test helpers prefer app selection APIs over direct `workspace.selected_nodes` access
- second Stage 5 cleanup slice landed: undo/redo snapshots now capture canonical focused selection rather than the raw compatibility mirror, snapshot restore/reset flows route through focused-selection helpers, and graph-load/reset paths use centralized selection reset semantics
- third Stage 5 cleanup slice landed: internal `graph_app.rs` behavior tests now assert through focused-selection helpers rather than the compatibility mirror, leaving only the intentional stale-mirror regression as a direct test touchpoint
- fourth Stage 5 cleanup slice landed: remaining runtime `graph_app.rs` selection consumers now read through focused-selection helpers, including pin/unpin edge commands and hop-distance primary tracking, so direct compatibility-mirror access is gone from owner-file runtime code
- fifth Stage 5 cleanup slice landed: shell-side omnibar ranking/signifier logic now also reads focused selection through helper APIs, so direct `selected_nodes` field access is confined to `app/focus_selection.rs` and the intentional stale-mirror regression test
- sixth Stage 5 cleanup slice landed: the workspace-level compatibility field was demoted and renamed to `active_selection`, making its cache/projection role explicit while keeping canonical ownership in `selected_nodes_by_view`
- seventh Stage 5 cleanup slice landed: the `active_selection` cache is now private to the app owner/focus-selection seam, so external code can only observe selection through helper APIs rather than by reading the cache field directly
- eighth Stage 5 cleanup slice landed: `active_selection` was removed from `GraphWorkspace` entirely and replaced by a canonical scope-based selection store, with `SelectionScope::Unfocused` covering no-view selection semantics without a separate runtime projection field
- ninth Stage 5 extraction slice landed: `app/history_runtime.rs` now owns history preview gating, replay cursor reconstruction, failure recording, archive export/clear/curation operations, and the full timeline preview/replay reducer cluster, leaving the owner match with a single delegated history-runtime branch
- tenth Stage 5 extraction slice landed: `app/graph_views.rs` now owns graph-view slot/layout persistence, graph-view registration and reconciliation, camera-target resolution, fit-lock handling, and queued camera/zoom command routing, removing another contiguous owner-file subsystem without changing reducer call sites
- eleventh Stage 5 extraction slice landed: `app/runtime_lifecycle.rs` now owns renderer-node mapping, runtime block/crash state, active and warm lifecycle LRU policy, and the webview-created/url/history/scroll/title/crash reducer handlers, removing the remaining lifecycle and renderer-mapping cluster from the owner without changing public app APIs
- twelfth Stage 5 extraction slice landed: `app/graph_mutations.rs` now owns graph add/remove/update helpers, edge mutation logging, traversal append logging, grouped-edge command expansion, node pinning, and destructive graph reset flows, removing the main graph-mutation orchestration cluster from the owner while preserving existing reducer entry points
- thirteenth Stage 5 cleanup slice landed: `app/focus_selection.rs` now also owns hop-distance invalidation tied to primary selection, direct node selection entry, and scoped-selection pruning helpers used by graph-view reconciliation and graph mutation flows, while `app/ux_navigation.rs` now owns tool-surface toggle transitions plus shared UX navigation transition emission so the remaining focus-surface diagnostics are not scattered across the owner
- fourteenth Stage 5 cleanup slice landed: `app/startup_persistence.rs` now owns startup persistence open timeout handling, recovery/open diagnostics emission, and constructor-time graph recovery fallback so the owner constructor keeps the app-assembly path while the persistence-open instrumentation is isolated ahead of servoshell debt-clear work
- fifteenth Stage 5 cleanup slice landed: `app/workspace_routing.rs` now also owns the reducer-side routed node-open acceptance helpers for frame/workspace opens, so frame/workspace resolution and the queued restore/open effects live behind one seam instead of splitting phase-2 host-open semantics between the owner reducer and the routing module
- `graph_app.rs` now declares and re-exports those child modules from the owner façade, removing the prior duplicate/comment-wrapped legacy blocks while keeping the owner/orchestrator boundary intact

Measured result after landing:

- `graph_app.rs`: 14,714 -> 8,526 lines

Remaining highest-value follow-on:

- remaining reducer clusters that still combine orchestration with dense event-specific side effects if another cohesive seam is desired after the runtime lifecycle, graph-mutation, and selection-boundary cleanups
- follow-on cleanup around the scope-based selection naming/documentation if further explicitness would help the servoshell debt-clear focus/input boundary work
