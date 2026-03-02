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
- `../../TERMINOLOGY.md`

---

## 1. Purpose and Scope

This spec defines the canonical cross-tree semantics for **Frame** as a
graph-first organizational object.

It establishes:

1. Frame identity and lifecycle in graph scope.
2. Workbench handles as open views over graph frames.
3. Membership synchronization between graph truth and workbench interactions.
4. Close vs delete semantics (non-destructive close by default).
5. UxTree exposure requirements for frame-aware automation and accessibility.

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

### 3.1 Membership cardinality

- Nodes may be members of zero, one, or many frames.
- Membership is explicit and persisted in graph scope.
- Workbench views may be filtered to one active frame, but that is a view
  choice, not a membership constraint.

### 3.2 Terminology rule

`MagneticZone` is not a runtime implementation authority in current Graphshell.
Use `Frame`, `Frame membership`, and `Frame-affinity region` as canonical terms
for organization semantics.

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

- `uxnode://workbench/workbar/frame[{frame_id}]`
- `uxnode://workbench/tile[graph:{graph_view_id}]/graph-canvas/node[{node_key}]/frame-memberships`

---

## 7. Acceptance Criteria

1. Closing a frame handle does not remove graph frame identity.
2. Frame membership mutations in workbench are reflected in graph state.
3. Node can belong to multiple frames and memberships persist across restart.
4. Deleting frame is explicit and separate from close.
5. UxTree surfaces frame handles and frame-membership actions.
6. Canonical docs in this lane use frame-first terminology.

---

## 8. Implementation Notes

- This is a terminology + semantic authority update, not a `MagneticZone`
  runtime migration.
- Existing canvas docs that mention `MagneticZone` should be updated to
  frame-affinity terminology where they describe organizational behavior.
- If a future dedicated non-frame clustering feature is introduced, it must use
  a distinct canonical term and explicit scope, not `MagneticZone` reuse.
