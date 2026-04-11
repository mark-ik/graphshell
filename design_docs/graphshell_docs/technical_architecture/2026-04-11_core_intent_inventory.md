<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# `CoreIntent` Inventory

**Date**: 2026-04-11  
**Status**: Draft architecture inventory  
**Scope**: Classify the current host `GraphIntent` surface into:

- direct `CoreIntent` candidates
- host intents that translate into one or more `CoreIntent` values
- host-only intents that should stay out of the portable reducer
- variants that still need a boundary decision

**Related**:

- [`2026-03-08_graphshell_core_extraction_plan.md`](2026-03-08_graphshell_core_extraction_plan.md)
  — canonical extraction plan
- [`graph_tree_spec.md`](graph_tree_spec.md)
  — sibling portable workbench tree subsystem
- [`graph_canvas_spec.md`](graph_canvas_spec.md)
  — sibling portable graph-view/canvas subsystem

---

## 1. Why This Inventory Exists

The current host `GraphIntent` enum mixes four different things:

- durable graph mutations
- portable graph/workspace session mutations
- host orchestration and bridge intents
- pure UI/runtime/service control intents

`graphshell-core` cannot absorb that enum wholesale without importing host-only
types and responsibilities. The right move is to define `CoreIntent` as the
portable mutation boundary and make the host enum translate into it.

This inventory is the first classification pass over the current enum in
[`app/intents.rs`](C:/Users/mark_/Code/source/repos/graphshell/app/intents.rs).

---

## 2. Bucket Legend

### A. Direct `CoreIntent`

These should survive as first-class portable reducer inputs with little or no
semantic change.

### B. Host → `CoreIntent` Translation

These remain valid host intents, but the mutation-bearing part should be
expressed as one or more `CoreIntent` values. The host computes local context
such as selection, routing, generated IDs, or mapped renderer identity.

### C. Host-Only

These belong to shell/workbench/canvas/runtime/service control and should not
cross into the portable reducer boundary.

### D. Needs Boundary Decision

These are plausible portable intents, but the right home depends on adjacent
design decisions that are not fully settled yet.

---

## 3. Recommended `CoreIntent` Families

For this inventory, the target portable families are:

- `CoreIntent::Graph`
- `CoreIntent::View`
- `CoreIntent::Session`
- `CoreIntent::Sync`

These family names are recommendations, not a frozen API promise.

---

## 4. Inventory

## 4.1 Direct `CoreIntent::Graph` Candidates

- `SetNodePosition`
  Note: portable graph/workspace mutation over `NodeKey` and position.
- `SetNodeUrl`
  Note: should become address-first, likely `SetNodeAddress`.
- `TagNode`
- `UntagNode`
- `AssignClassification`
- `UnassignClassification`
- `AcceptClassification`
- `RejectClassification`
- `SetPrimaryClassification`
- `CreateUserGroupedEdge`
  Note: naming may become more relation-generic in core.
- `RemoveEdge`
- `SetNodePinned`
- `UpdateNodeMimeHint`
- `UpdateNodeViewerOverride`
- `RecordFrameLayoutHint`
- `RemoveFrameLayoutHint`
- `MoveFrameLayoutHint`
- `SetFrameSplitOfferSuppressed`

## 4.2 Direct `CoreIntent::View` Candidates

- `SetViewLensId`
- `SetViewLayoutAlgorithm`
- `SetViewPhysicsProfile`
- `SetViewFilter`
- `ClearViewFilter`
- `SetViewDimension`
- `ToggleSemanticDepthView`
- `SetViewEdgeProjectionOverride`

## 4.3 Direct `CoreIntent::Session` Candidates

- `Undo`
- `Redo`
- `PromoteNodeToActive`
- `DemoteNodeToWarm`
- `DemoteNodeToCold`

## 4.4 Direct `CoreIntent::Sync` Candidates

- `ApplyRemoteDelta`
- `TrustPeer`
- `GrantWorkspaceAccess`
- `ForgetDevice`
- `RevokeWorkspaceAccess`

---

## 5. Host Intents That Should Translate Into `CoreIntent`

## 5.1 Node Creation / Deletion / Bulk Graph Mutation

- `CreateNoteForNode`
  Recommended translation: one or more graph creation intents plus relation
  attachment.
- `CreateNodeNearCenter`
  Recommended translation: host computes ID and position, then emits
  `CoreIntent::Graph(AddNode ...)`.
- `CreateNodeNearCenterAndOpen`
  Recommended translation: graph creation `CoreIntent` plus separate host
  workbench open action.
- `CreateNodeAtUrl`
  Recommended translation: host generates `NodeId`, normalizes address, emits
  core add-node intent.
- `CreateNodeAtUrlAndOpen`
  Recommended translation: add-node core mutation plus host open action.
- `RemoveSelectedNodes`
  Recommended translation: host resolves selection to concrete node keys, emits
  one or more remove/tombstone intents.
- `MarkTombstoneForSelected`
  Recommended translation: host resolves selection, emits lifecycle/removal
  core intents.
- `RestoreGhostNode`
  Recommended translation: portable lifecycle restoration intent.
- `ClearGraph`
  Recommended translation: explicit core graph reset intent or bulk reducer
  operation.

## 5.2 Selection- or Command-Derived Relation Mutations

- `CreateUserGroupedEdgeFromPrimarySelection`
- `GroupNodesBySemanticTags`
- `ExecuteEdgeCommand`
- `TogglePrimaryNodePin`

These depend on host-side selection or command interpretation, but should
translate into durable core graph mutations.

## 5.3 Webview / Browser Events That Carry Durable Node State

- `WebViewUrlChanged`
  Recommended translation: host resolves `RendererId -> NodeKey`, emits a core
  address/history mutation.
- `WebViewHistoryChanged`
  Recommended translation: host resolves node, emits portable node history /
  session-state mutation.
- `WebViewScrollChanged`
  Recommended translation: host resolves node, emits portable node session-state
  mutation.
- `WebViewTitleChanged`
  Recommended translation: host resolves node, emits title mutation if title is
  graph truth or session-state mutation if it is treated as fidelity state.
- `SetNodeFormDraft`
  Recommended translation: portable node session-state mutation.

## 5.4 Routed / Bridge Intents With a Durable Mutation Portion

- `AcceptHostOpenRequest`
  Recommended translation: host computes the open strategy, then emits add-node,
  navigation, or relation core mutations as needed.
- `OpenNodeFrameRouted`
- `OpenNodeWorkspaceRouted`

These are host routing intents, but they often imply durable graph mutations
such as opening a new node, recording traversal, or updating history.

## 5.5 Replay / Sync Intake

- `NostrEventReceived`
  Recommended translation: host parses and validates the event, then emits
  `ApplyRemoteDelta` or publication-specific core intents once the relay event
  format is finalized.

---

## 6. Host-Only Intents

## 6.1 Graph Canvas / Camera / Interaction Runtime

- `TogglePhysics`
- `ToggleGhostNodes`
- `ToggleCameraPositionFitLock`
- `ToggleCameraZoomFitLock`
- `RequestFitToScreen`
- `RequestZoomIn`
- `RequestZoomOut`
- `RequestZoomReset`
- `RequestZoomToSelected`
- `RequestZoomToGraphlet`
- `ReheatPhysics`
- `SetInteracting`
- `SetZoom`
- `SetPhysicsProfile`
- `SetTheme`
- `SetHighlightedEdge`
- `ClearHighlightedEdge`
- `SetSelectionEdgeProjectionOverride`

These belong to graph view / canvas runtime, not the portable mutation kernel.

## 6.2 Shell / Chrome / Panels

- `ToggleHelpPanel`
- `ToggleCommandPalette`
- `ToggleRadialMenu`

## 6.3 Graph View Slot and Workbench Topology Management

- `CreateGraphViewSlot`
- `RenameGraphViewSlot`
- `MoveGraphViewSlot`
- `ArchiveGraphViewSlot`
- `RestoreGraphViewSlot`
- `RouteGraphViewToWorkbench`
- `FocusGraphView`
- `TransferSelectedNodesToGraphView`
- `SetPanePresentationMode`
- `PromoteEphemeralPane`
- `OpenFrameTileGroup`
- `RestorePaneToSemanticTabGroup`
- `CollapseSemanticTabGroupToPaneRest`
- `RepairFrameTabSemantics`

These belong to host workbench orchestration or to sibling portable subsystems
such as `graph-tree`, not to `graphshell-core`.

## 6.4 Selection / Navigator / Specialty View UX

- `SelectNode`
- `UpdateSelection`
- `SelectAll`
- `SetNavigatorProjectionSeedSource`
- `SetNavigatorProjectionMode`
- `SetNavigatorSortMode`
- `SetNavigatorRootFilter`
- `SetNavigatorSelectedRows`
- `SetNavigatorExpandedRows`
- `RebuildNavigatorProjection`
- `SetNavigatorSpecialtyView`

These are user-interface or projection-host concerns rather than durable core
mutations.

## 6.5 Webview / Runtime Bookkeeping

- `TraverseBack`
- `TraverseForward`
- `EnterGraphViewLayoutManager`
- `ExitGraphViewLayoutManager`
- `ToggleGraphViewLayoutManager`
- `MapWebviewToNode`
- `UnmapWebview`
- `WebViewCreated`
- `WebViewCrashed`
- `MarkRuntimeBlocked`
- `ClearRuntimeBlocked`

These depend on host runtime handles, renderer bookkeeping, or runtime-only
state transitions.

## 6.6 History Timeline UX / Runtime Control

- `ClearHistoryTimeline`
- `ClearHistoryDissolved`
- `AutoCurateHistoryTimeline`
- `AutoCurateHistoryDissolved`
- `ExportHistoryTimeline`
- `ExportHistoryDissolved`
- `EnterHistoryTimelinePreview`
- `ExitHistoryTimelinePreview`
- `HistoryTimelinePreviewIsolationViolation`
- `HistoryTimelineReplayStarted`
- `HistoryTimelineReplaySetTotal`
- `HistoryTimelineReplayAdvance`
- `HistoryTimelineReplayReset`
- `HistoryTimelineReplayProgress`
- `HistoryTimelineReplayFinished`
- `HistoryTimelineReturnToPresentFailed`

These are currently host workflow/runtime controls. If a future portable history
engine emerges, they can be revisited.

## 6.7 Host Services / Infrastructure / Diagnostics

- `WorkflowActivated`
- `PersistNostrSubscriptions`
- `Noop`
- `SetMemoryPressureStatus`
- `ModActivated`
- `ModLoadFailed`
- `SyncNow`
- `StartGeminiCapsuleServer`
- `StopGeminiCapsuleServer`
- `ServeNodeAsGemini`
- `UnserveNodeFromGemini`
- `StartGopherCapsuleServer`
- `StopGopherCapsuleServer`
- `ServeNodeAsGopher`
- `UnserveNodeFromGopher`
- `StartFingerServer`
- `StopFingerServer`
- `PublishFingerProfile`
- `UnpublishFingerProfile`

These are host capabilities, not portable graph/workspace mutations.

---

## 7. Needs Boundary Decision

- `SuggestNodeTags`
  Could remain host-only suggestion UI, or become a portable semantic-assist
  payload if suggestions are treated as durable workspace state.
- `DeleteImportRecord`
- `SuppressImportRecordMembership`
- `PromoteImportRecordToUserGroup`
  These likely want to be durable core mutations if import records are part of
  portable graph truth, but that depends on whether import-record state moves
  fully into core during Step 4 or remains host-managed enrichment metadata.
- `SetSelectedFrame`
  Could be portable workspace session state, but may also belong with workbench
  projection state or `graph-tree`.
- `SetWorkbenchEdgeProjection`
  Looks portable at first glance, but "workbench" in the name suggests this may
  belong to a sibling portable workbench/view crate rather than the kernel.

---

## 8. Recommended Migration Order

1. Land the direct `CoreIntent` cases first.
2. Add host translation for the obvious creation/selection/webview-derived
   mutation intents.
3. Leave host-only families in the existing host enum with no pressure to move.
4. Resolve the "Needs Boundary Decision" set before finalizing the Step 4
   done-definition.

---

## 9. Immediate Follow-Ups

- Add a concrete `CoreIntent` type sketch to the code-facing extraction work.
- Build a variant-by-variant translation checklist in the Step 4 execution plan.
- Decide the ownership of import-record state, selected-frame state, and
  workbench edge projection before freezing the reducer boundary.

---

*This inventory is a classification tool, not a frozen API spec. Update it as
the current host enum changes or the `CoreIntent` boundary becomes more precise.*
