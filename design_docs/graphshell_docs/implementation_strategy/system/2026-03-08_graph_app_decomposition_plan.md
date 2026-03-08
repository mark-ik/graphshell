# `graph_app.rs` Decomposition Plan

**Date**: 2026-03-08  
**Status**: Active; Stages 1-3 landed  
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

### Stage 5. Reduce remaining owner file to authority logic

After support surfaces are extracted, `graph_app.rs` should primarily hold:

- `GraphBrowserApp`
- `GraphWorkspace`
- reducer-owned mutation orchestration
- explicit authority crossings

Selection-specific correction required in this stage:

- remove or fully demote `workspace.selected_nodes` as a compatibility mirror once remaining runtime call sites are migrated
- keep `workspace.selected_nodes_by_view` as the sole authoritative graph-selection owner
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
- `graph_app.rs` re-exports those surfaces and remains the owner/orchestrator

Measured result after landing:

- `graph_app.rs`: 14,714 -> 11,269 lines

Remaining highest-value follow-on:

- command-queue helper extraction
- selection compatibility-mirror removal (`selected_nodes` -> per-view-only authority)
- cross-cutting instrumentation helper extraction where reducer methods have started to accumulate tracing/diagnostics clusters
