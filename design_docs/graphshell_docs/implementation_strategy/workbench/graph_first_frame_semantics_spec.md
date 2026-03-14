# Graph-First Frame Semantics — Interaction Spec

**Date**: 2026-03-01  
**Status**: Canonical interaction contract  
**Priority**: Immediate terminology + authority alignment

**Related**:

- `workbench_frame_tile_interaction_spec.md`
- `../canvas/graph_node_edge_interaction_spec.md`
- `../canvas/multi_view_pane_spec.md`
- `../aspect_command/command_surface_interaction_spec.md`
- `../aspect_input/input_interaction_spec.md`
- `../subsystem_ux_semantics/ux_tree_and_probe_spec.md`
- `../2026-03-01_ux_migration_design_spec.md`
- `../canvas/2026-03-14_graph_relation_families.md` — ArrangementRelation as the forthcoming graph-edge backing for frame membership (§2.4)
- `../../../TERMINOLOGY.md`

---

## 1. Purpose and Scope

This spec defines the canonical cross-tree semantics for **Frame** as a
graph-first organizational object.

It establishes:

1. Frame identity and lifecycle in graph scope.
2. Frame address semantics under the internal address-as-identity model (`verso://` runtime canonical namespace, legacy `graphshell://` compatibility alias).
3. Workbench handles as open views over graph frames.
4. Membership synchronization between graph truth and workbench interactions.
5. Close vs delete semantics (non-destructive close by default).
6. UxTree exposure requirements for frame-aware automation and accessibility.

This is a semantic authority spec. It does not define rendering geometry.

---

## 2. Canonical Framing

### 2.1 Frame is graph-first

A `Frame` is a graph organizational entity that may exist independently of any
open workbench handle.

- Frame identity lives in graph scope.
- Workbench can open, focus, close, split, and dock frame handles.
- Closing a handle does not remove frame identity.

### 2.2 Handle model

A workbench frame UI element is an **open handle** over a graph frame:

- `OpenFrameHandle(frame_id)` creates/activates a handle.
- `CloseFrameHandle(frame_id)` closes the handle only.
- `DeleteFrame(frame_id)` removes graph frame identity (destructive).

### 2.3 Analogy contract

- Closing a node pane is like closing an open file view.
- Closing a frame handle is like closing an open folder view.
- Neither operation deletes the underlying graph object.

### 2.4 Frame-to-Tilegroup Bridge Contract

Frames intentionally cross the semantic boundary between graph and workbench:

- Graph scope is the authority for frame identity and membership.
- Workbench scope is the authority for tilegroup layout, docking, focus, and handle lifecycle.
- A tilegroup is a workbench handle/projection over a `Frame`, not a second frame identity.

Guardrail wording:

- Preferred: "tilegroup is a handle over a frame."
- Avoid: wording that implies strict identity equivalence between tilegroup lifecycle and frame lifecycle.
- Consequence: close/split/dock remain non-destructive UI operations, while `DeleteFrame` remains an explicit destructive graph operation.

---

## 3. Data Model Contract

```rust
struct GraphFrame {
    frame_id: FrameId,
    label: String,
    color_token: FrameColorToken,
    member_nodes: Vec<NodeKey>,
    created_at: Timestamp,
    updated_at: Timestamp,
}

struct NodeFrameMembership {
    node: NodeKey,
    frames: Vec<FrameId>,
}
```

### 3.0 Frame address identity

Every frame has a canonical internal address:

`verso://frame/<FrameId>`

Legacy note:

- Older docs may still refer to `graphshell://frame/<FrameId>` as the original spec basis.
- Runtime canonical formatting should emit `verso://frame/<FrameId>`.

Address rules:

- The address is issued when the frame identity is created.
- The address is stable for the lifetime of that frame.
- The address resolves to the frame's graph node under the address-as-identity rule in `TERMINOLOGY.md`.
- Closing a workbench handle does not invalidate the address, because handle closure is not graph deletion.
- `DeleteFrame(frame_id)` removes the frame identity and therefore removes the live resolution of `verso://frame/<FrameId>`.

The frame address is the canonical identity bridge between:

- graph storage (`GraphFrame`),
- workbench handles (`OpenFrameHandle` / `CloseFrameHandle`),
- frame-membership visualization on the canvas.

### 3.0a ArrangementRelation backing (forthcoming)

The `member_nodes: Vec<NodeKey>` field on `GraphFrame` is the current in-memory representation. The forthcoming migration replaces this with `ArrangementRelation` / `frame-member` edges in the graph (`EdgeKind::ArrangementRelation { sub_kind: "frame-member" }`). Under that model:

- Frame membership is a set of durable `frame-member` edges between the frame node and its member tile nodes.
- Named frames produce durable `frame-member` edges; unnamed/session frames produce session-only edges.
- The `GraphFrame.member_nodes` field becomes a derived view over these edges, not the authoritative store.
- `AddNodeToFrame` / `RemoveNodeFromFrame` intents assert / retract the corresponding `ArrangementRelation` edge.

Until the migration lands, `GraphFrame.member_nodes` remains authoritative. See `canvas/2026-03-14_graph_relation_families.md §2.4`.

### 3.1 Membership cardinality

- Nodes may be members of zero, one, or many frames.
- Membership is explicit and persisted in graph scope.
- Workbench views may be filtered to one active frame, but that is a view
  choice, not a membership constraint.

### 3.2 Terminology rule

`Frame` is the semantic authority. `MagneticZone` is legacy visualization wording and, where retained, refers only to the canvas presentation of a frame-affinity region.

Canonical wording:

- Use `Frame` for the graph object.
- Use `Frame membership` for node-to-frame organizational links.
- Use `Frame-affinity region` for the visual/layout projection of a frame on the graph canvas.
- If `MagneticZone` appears in older docs, read it as a legacy alias for the visual frame-affinity region, not a separate semantic object.

---

## 4. Interaction Semantics

### 4.1 Membership mutation via workbench operations

Workbench tile operations that imply organization must update graph memberships:

| Workbench operation | Graph effect |
|--------------------|--------------|
| Add node tile to frame | `AddNodeToFrame(frame_id, node_key)` |
| Remove node tile from frame | `RemoveNodeFromFrame(frame_id, node_key)` |
| Create frame from selection | `CreateFrameFromSelection(node_keys...)` |
| Merge frames | `MergeFrames(source, target)` updates member lists |

### 4.2 Close vs delete

| Action | Destructive | Required behavior |
|-------|-------------|-------------------|
| Close frame handle | No | Remove only workbench handle; preserve frame and memberships |
| Close node pane | No | Remove only pane handle; preserve node and memberships |
| Delete frame | Yes | Remove frame identity and membership links; requires explicit confirmation |
| Delete node | Yes | Remove node identity per node lifecycle policy |

### 4.3 Default command policy

- `Ctrl+W` on frame context maps to `CloseFrameHandle` (non-destructive).
- `DeleteFrame` is available only via explicit destructive command path
  (`Command Palette` + confirmation).

---

## 5. Visual and Layout Semantics

### 5.1 Frame-affinity regions

Frames may project a visual region on graph canvas for orientation:

- each frame has a stable color token,
- member nodes may be softly biased toward a frame-affinity centroid,
- affinity is a visual/layout hint only,
- no implied identity duplication.

The frame-affinity region is a visual projection of the same frame identified by `verso://frame/<FrameId>`. It is not a second frame-like object and must not drift into a separate storage identity.

### 5.2 Multiple memberships

If a node belongs to multiple frames:

- frame badges/chips show all memberships,
- active-frame context gets primary highlight,
- secondary memberships remain visible in compact form.

---

## 6. UxTree Contract

UxTree must expose frame semantics for automation/accessibility:

- frame handles in workbench chrome,
- node frame-membership states in node semantic subtree,
- invokable actions:
  - `OpenFrameHandle`
  - `CloseFrameHandle`
  - `AddNodeToFrame`
  - `RemoveNodeFromFrame`
  - `DeleteFrame` (destructive)

### 6.1 Structural expectations

- `uxnode://workbench/sidebar/frame[{frame_id}]` (legacy path: `uxnode://workbench/workbar/frame[{frame_id}]`)
- `uxnode://workbench/tile[graph:{graph_view_id}]/graph-canvas/node[{node_key}]/frame-memberships`

---

## 7. Acceptance Criteria

1. Closing a frame handle does not remove graph frame identity.
2. `verso://frame/<FrameId>` remains stable while the frame exists and stops resolving only after `DeleteFrame` (with legacy `graphshell://frame/<FrameId>` accepted only as a compatibility alias while migration remains active).
3. Frame membership mutations in workbench are reflected in graph state.
4. Node can belong to multiple frames and memberships persist across restart.
5. Deleting frame is explicit and separate from close.
6. UxTree surfaces frame handles and frame-membership actions.
7. Canonical docs in this lane use frame-first terminology.
8. Tilegroup operations (`close`, `split`, `dock`) do not create or delete frame identity unless an explicit frame-destructive command is invoked.

---

## 8. Implementation Notes

- This is a terminology + semantic authority update, not a `MagneticZone`
  runtime migration.
- Existing canvas docs that mention `MagneticZone` should be updated to
  frame-affinity terminology where they describe organizational behavior.
- If a future dedicated non-frame clustering feature is introduced, it must use
  a distinct canonical term and explicit scope, not `MagneticZone` reuse.
