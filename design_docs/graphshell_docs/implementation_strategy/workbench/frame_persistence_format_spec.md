# Frame Persistence Format Spec

**Date:** 2026-03-12  
**Status:** Canonical storage and restore contract  
**Priority:** Tier 3 / important stability contract

**Related docs:**

- [`workbench_frame_tile_interaction_spec.md`](./workbench_frame_tile_interaction_spec.md)
- [`pane_chrome_and_promotion_spec.md`](./pane_chrome_and_promotion_spec.md)
- [`../../archive_docs/checkpoint_2026-04-02/graphshell_docs/implementation_strategy/workbench/2026-02-22_workbench_tab_semantics_overlay_and_promotion_plan.md`](../../archive_docs/checkpoint_2026-04-02/graphshell_docs/implementation_strategy/workbench/2026-02-22_workbench_tab_semantics_overlay_and_promotion_plan.md) — archived rollout record
- [`../subsystem_storage/storage_and_persistence_integrity_spec.md`](../subsystem_storage/storage_and_persistence_integrity_spec.md)
- [`../system/2026-03-12_workspace_decomposition_and_renaming_plan.md`](../system/2026-03-12_workspace_decomposition_and_renaming_plan.md)
- [`../../technical_architecture/2026-03-12_specification_coverage_register.md`](../../technical_architecture/2026-03-12_specification_coverage_register.md)

---

## 1. Purpose and Scope

This spec defines the persisted format and restore contract for named workbench frame snapshots.

It governs:

- the persisted bundle shape,
- pane identity rules inside a persisted frame,
- manifest/layout consistency requirements,
- stale-node and backward-compat recovery behavior,
- save, load, validate, repair, restore, and deletion semantics,
- the relationship between persisted frame bundles and graph-backed frame representation.

It does not govern:

- live tile-tree mutation semantics,
- tab-group creation/grouping semantics,
- command-surface flows that trigger save or restore,
- domain-graph persistence outside the frame bundle projection.

---

## 2. Canonical Role

A named frame snapshot is a durable serialization of a workbench arrangement.

Normative rule:

- the persisted frame bundle is a durable storage format for workbench arrangement,
- it is not a second live workbench authority,
- runtime tile trees are restored from it, not mirrored continuously to it.

Related rule:

- frame bundles and graph-backed frame entities are distinct but coordinated artifacts,
- the bundle is the concrete layout/manifest format,
- the graph representation is the semantic persistence projection.

---

## 3. Persisted Bundle Shape

The canonical persisted object is `PersistedFrame` / `PersistedWorkspace`.

Current persisted top-level fields:

- `version`
- `name`
- `layout`
- `manifest`
- `frame_tab_semantics` (optional)
- `metadata`
- `workbench_profile`

Current optional-field rule:

- `FrameTabSemantics` is now an optional additive field in the persisted bundle shape
- bundle load/save must continue to work with or without `frame_tab_semantics` present
- semantic tab overlay remains frame state only; it is not graph WAL data

### 3.1 Layout

`FrameLayout` contains:

- `tree: Tree<PersistedPaneTile>`

`PersistedPaneTile` currently allows:

- `Graph` (legacy persisted graph-pane form kept for compatibility)
- `Pane(PersistedPaneId)`
- `LegacyDiagnostic` (deserialize-only backward compatibility)

Normative rule:

- layout stores structure and pane references,
- it does not store full pane payloads inline.

### 3.2 Manifest

`FrameManifest` contains:

- `panes: BTreeMap<PaneId, PaneContent>`
- `member_node_uuids: BTreeSet<Uuid>`

`PaneContent` currently allows:

- `Graph`
- `NodePane { node_uuid }`
- `Tool { kind }`

Normative rule:

- the manifest is the canonical content map for pane ids referenced by layout,
- every `Pane(id)` referenced in layout must resolve through manifest.

### 3.3 Metadata

`FrameMetadata` contains:

- `created_at_ms`
- `updated_at_ms`
- `last_activated_at_ms`

Normative rule:

- metadata is durable bookkeeping,
- it does not affect structural validity of the frame bundle.

### 3.4 Optional field: semantic tab overlay

`FrameTabSemantics` is an additive persisted frame-bundle field used to preserve semantic tab
membership independently from structural tile-tree normalization.

Normative rule:

- `FrameTabSemantics` is frame state only
- it is additive to the persisted bundle and must remain backward compatible
- it does not belong in graph WAL / domain-graph persistence lanes
- consumers must tolerate the field being absent and fall back to deriving tab meaning from
  current persisted layout and manifest when needed

---

## 4. Identity and Scope Rules

### 4.1 Persisted pane id

`PersistedPaneId` is a local serialized-pane identifier.

Normative rule:

- it is scoped only to a single persisted frame bundle,
- it is distinct from runtime `PaneId`,
- it must not be treated as a stable cross-bundle or live-runtime pane identity.

### 4.2 Node identity

Node pane membership is persisted by node UUID, not runtime node key.

Normative rule:

- restore resolves UUIDs back into live node keys,
- stale node UUIDs may be dropped during restore/repair if they can no longer resolve,
- UUID is the durable node carrier for this format.

### 4.3 Graph pane and tool pane identity

- `Graph` panes are persisted structurally as graph-pane placeholders and restore into a default graph-view payload.
- tool panes persist by `ToolPaneState` kind, not by transient runtime handle identity.

---

## 5. Validation Contract

`validate_frame_bundle(...)` is the canonical structural validator.

It must enforce:

1. every layout-referenced `PaneId` exists in the manifest,
2. derived node membership from manifest equals declared `member_node_uuids`.

Current canonical validation failures:

- `MissingManifestPane`
- `MembershipMismatch`

Normative rule:

- missing manifest pane is a hard structural error,
- membership mismatch is repairable if the rest of the bundle is structurally sound.

---

## 6. Repair Contract

Current repairable condition:

- manifest membership mismatch.

Current repair action:

- recompute `member_node_uuids` from manifest pane content.

Normative rule:

- repair may normalize derived bookkeeping,
- repair must not silently invent missing panes or fabricate missing node bindings.

Future repair extensions must remain explicit and bounded.

---

## 7. Save Contract

### 7.1 Serialization source

The source of truth for save is the live runtime `Tree<TileKind>`.

Save sequence:

1. convert runtime tree into persisted layout tree,
2. derive manifest entries from runtime pane content,
3. derive membership from manifest,
4. carry forward compatible metadata when overwriting an existing named frame,
5. write serialized JSON bundle,
6. update graph-backed frame representation.

Normative rule:

- saving a frame snapshot must serialize from live workbench state,
- not from a stale cached layout mirror.

### 7.2 Save compatibility behavior

When overwriting an existing named frame:

- `created_at_ms` is preserved if valid,
- `updated_at_ms` is refreshed,
- `last_activated_at_ms` is preserved unless separately updated by activation flow.

---

## 8. Load and Restore Contract

### 8.1 Load

Loading a named frame bundle must:

1. retrieve serialized JSON by name,
2. deserialize bundle,
3. validate,
4. repair membership mismatch if needed,
5. fail on structural errors that cannot be repaired safely.

### 8.2 Restore

Restore must:

1. convert the persisted layout/manifest back into a runtime `Tree<TileKind>`,
2. resolve node UUIDs into current live `NodeKey` values,
3. drop or skip stale members that cannot be resolved,
4. produce the restored runtime tree plus the set of restored nodes,
5. preserve runtime validity even when some persisted members are stale.

Normative rule:

- restore is best-effort with respect to stale node resolution,
- but it must never produce an invalid runtime tree.

### 8.3 Empty-restore fallback

If restore yields an empty tree:

- the system may fall back to opening the routed request in the current frame,
- it must not silently claim success on an empty restored layout.

### 8.4 Failed-restore fallback

If restore fails:

- a warning/error path is emitted,
- routed open requests may fall back to current-frame open behavior,
- the failure must remain diagnosable.

---

## 9. Backward Compatibility Contract

Current explicit backward-compat path:

- `LegacyDiagnostic` persisted pane tiles are accepted during deserialize and mapped into the generic tool-pane path.

Normative rule:

- backward compatibility may preserve old persisted variants through deserialize-only aliases,
- current writers must not continue emitting obsolete variants once the new canonical form exists.

---

## 10. Deletion and Retention Contract

Named frame bundles are independently manageable persisted artifacts.

Supported lifecycle operations include:

- save/update named frame bundle,
- delete named frame bundle,
- prune empty named frame bundles,
- keep latest N named frame bundles by activation recency.

Normative rule:

- deletion removes the named persisted bundle,
- corresponding graph-backed frame projection should be removed or synchronized accordingly,
- pruning/retention behavior must operate on named persisted bundles, not on live runtime frames.

---

## 11. Graph Representation Relationship

**Authority split** (updated 2026-03-21 per
`2026-03-20_arrangement_graph_projection_plan.md`):

- **Graph edges carry membership truth.** `ArrangementRelation(FrameMember)` and
  `UserGrouped` edges are the durable, authoritative record of which nodes belong
  to which named frame. They persist in the graph store (redb WAL) independently
  of any frame bundle, survive restarts without a separate bootstrap step, and are
  the canonical input to graphlet computation.
- **FrameSnapshot captures workspace-restore state.** The frame bundle records
  which nodes were warm/active at save time plus presentation state (active-tab
  identity, split geometry). Its role is workspace restore — re-opening the saved
  tiles and arrangement shape when a frame is loaded — not carrying membership
  truth. If the bundle is absent or stale, graph edges reconstruct durable
  membership without it.
- **Neither should silently diverge.** On save, graph membership and bundle
  membership must be mutually consistent. On load, graph edges take precedence for
  membership; bundle layout takes precedence for presentation shape.

Normative rule (revised):

- the FrameSnapshot bundle is the workspace-restore format for named frames,
- the graph-backed `FrameMember` edges are the authoritative membership record,
- save must synchronize graph representation,
- delete must remove graph representation,
- activation should update activation metadata consistently.

---

## 12. Diagnostics and Test Contract

Required coverage:

1. save/load/restore roundtrip of a mixed frame,
2. missing-manifest-pane validation failure,
3. membership-mismatch repair,
4. stale-node restore behavior,
5. graph representation sync on save,
6. graph representation deletion on bundle delete,
7. empty restore fallback behavior,
8. backward-compat deserialize of legacy diagnostic panes.

Required diagnostics:

- save failure,
- restore failure,
- stale-node pruning/repair,
- graph-sync failure where applicable.

---

## 13. Acceptance Criteria

- [ ] persisted frame bundle shape is treated as a canonical storage contract.
- [ ] layout and manifest roles remain distinct and explicit.
- [ ] validation and repair rules remain bounded and test-covered.
- [ ] restore never yields an invalid runtime tree.
- [ ] stale members are handled explicitly rather than by silent corruption.
- [ ] graph-backed frame projection remains synchronized with bundle lifecycle operations.
