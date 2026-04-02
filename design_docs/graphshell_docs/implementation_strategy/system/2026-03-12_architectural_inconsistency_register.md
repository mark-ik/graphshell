<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Architectural Inconsistency Register (2026-03-12)

**Status**: Active audit register

**Purpose**: Track currently known state-ownership and semantic-model inconsistencies that are not necessarily immediate bugs, but that create architectural drift or misleading terminology.

**Companion docs**:

- `2026-03-12_workspace_decomposition_and_renaming_plan.md`
- `2026-03-06_foundational_reset_graphbrowserapp_field_ownership_map.md`
- `2026-03-08_unified_focus_architecture_plan.md`

---

## 1. Scope

This register covers cases where one or more of the following is true:

1. canonical truth is stored on the wrong owner,
2. one concept has multiple carriers,
3. durable/session/runtime/derived state are mixed in one struct,
4. the current name of a carrier implies the wrong authority or scope.

This is not a bug list. Some entries are intentional bridges. The point is to make the bridges explicit and prevent them from becoming invisible architecture.

---

## 2. Priority Register

| Priority | Inconsistency | Current carrier | Why inconsistent | Target owner |
| --- | --- | --- | --- | --- |
| P1 | Canonical node tags stored as `workspace.semantic_tags` | `GraphWorkspace` | Name implies workspace/session scope; semantics imply node-owned truth | `Node.tags` in `DomainState` |
| P1 | Node owns runtime/session residue alongside durable identity | `Node` | Mixes durable graph identity with webview/session/runtime state | split durable node metadata from node runtime/session state |
| P1 | Pinning has duplicate truth carriers | `Node.is_pinned` + `#pin` tag | One concept stored twice; reducer sync is a band-aid | one canonical source + one derived projection |
| P2 | `GraphViewState` mixes durable view state with runtime cache state | `GraphViewState` | View identity/prefs coexist with `local_simulation` and `egui_state` | split into persisted view state + per-view runtime state |
| P2 | Camera authority is spread across multiple carriers | global `camera`, per-view `camera`, `graph_view_frames` | unclear authoritative camera state | per-view session truth + runtime frame cache |
| P2 | Undo/redo snapshot scope crosses layers | graph bytes + selection + highlighted edge + layout JSON | one transaction bundle spans domain, session, and UI targeting state | explicit layered history model or declared mixed boundary |
| P2 | `#clip` is acting like a node type while modeled as a tag | `#clip` in tag set | semantic type/classification carried as a behavior-tag | explicit clip content facet |
| P3 | Pending orchestration/control-plane state is still spread across ad hoc fields | `pending_*`, app queues, focus queues | command/control authority not fully centralized | explicit runtime authority/control-plane state |
| P3 | Derived indexes read as if they are primary truth | `semantic_index`, `node_workspace_membership`, `graph_view_frames` | caches sit beside canonical state and look authoritative | dedicated derived/runtime cache families |
| P3 | `file_tree_projection_state` naming/ownership mismatch | `GraphWorkspace` | comment frames it as graph-owned projection state; behavior is closer to workbench/tool projection | `WorkbenchSessionState` or dedicated projection state |

---

## 3. Detailed Entries

### 3.1 Canonical node tags stored as `workspace.semantic_tags`

**Current carrier**:

- `workspace.semantic_tags` in `graph_app.rs`

**Why it is inconsistent**:

- tags are semantically node-associated metadata,
- current naming implies workspace/session ownership,
- current storage requires a separate stale-pruning pass because tags are not removed automatically with node lifetime.

**Target owner**:

- `Node.tags` in `DomainState`

**Next action**:

- execute the Phase 1.6 migration added to the node badge and tagging plan.

---

### 3.2 Node owns runtime/session residue alongside durable identity

**Current carrier**:

- `Node` in `model/graph/mod.rs`

**Fields of concern**:

- `history_entries`
- `history_index`
- `thumbnail_*`
- `favicon_*`
- `session_scroll`
- `session_form_draft`
- `lifecycle`

**Why it is inconsistent**:

- some of these are durable enough to justify being on the node,
- others are clearly viewer/session/runtime state,
- together they make `Node` both a durable semantic entity and a live runtime/session envelope.

**Target owner**:

- durable node metadata remains on `Node`
- viewer/session residue moves into a dedicated node runtime/session carrier

**Next action**:

- write a follow-up node-state split plan after the tag migration settles.

---

### 3.3 Pinning has duplicate truth carriers

**Current carrier**:

- `Node.is_pinned`
- `#pin` in the tag set

**Why it is inconsistent**:

- one concept is encoded in two places,
- reducer synchronization is required to stop them drifting,
- this makes it unclear whether pinning is a structural node property or a semantic tag behavior.

**Target owner**:

- one canonical source
- one derived projection

**Likely recommendation**:

- keep canonical physics pin state explicit,
- derive `#pin` tag presentation from it or vice versa, but not both as first-class truth.

---

### 3.4 `GraphViewState` mixes durable view state with runtime cache state

**Current carrier**:

- `GraphViewState`

**Why it is inconsistent**:

- persisted identity and preferences (`id`, `name`, `lens`, `dimension`, per-view camera) coexist with runtime-only caches (`local_simulation`, `egui_state`).

**Target owner**:

- persisted/session view state in `WorkbenchSessionState`
- per-view runtime state in a dedicated runtime carrier

**Next action**:

- split `GraphViewState` into view truth vs. view runtime cache when view/camera cleanup begins.

---

### 3.5 Camera authority is spread across multiple carriers

**Current carrier**:

- global `workspace.camera`
- per-view `GraphViewState.camera`
- `graph_view_frames`

**Why it is inconsistent**:

- three carriers represent overlapping camera state,
- one is a global legacy carrier,
- one is per-view session truth,
- one is render-output/runtime cache.

**Target owner**:

- per-view camera in session/view truth
- `graph_view_frames` as runtime-derived render frame cache
- remove or deprecate global camera carrier

**Next action**:

- camera cleanup should be coupled to the view-state split, not handled ad hoc.

---

### 3.6 Undo/redo snapshot scope crosses layers

**Current carrier**:

- `UndoRedoSnapshot` built in `graph_app.rs`

**Why it is inconsistent**:

- snapshot includes domain graph bytes,
- plus workbench selection,
- plus highlighted edge UI targeting,
- plus workspace layout JSON.

That means one history boundary spans multiple ownership layers without an explicit layered history model.

**Target owner**:

- either an explicit mixed-scope history contract,
- or layered undo families with clear boundaries.

**Next action**:

- document whether this mixed transaction scope is intended architecture or temporary convenience.

---

### 3.7 `#clip` is acting like a node type while modeled as a tag

**Current carrier**:

- `#clip` in the tag set

**Why it is inconsistent**:

- the tag is now expected to drive:
  - distinct border treatment
  - semantic node-type meaning
  - query behavior (`is:clip`)
- that is closer to an explicit content/type facet than a generic organizational tag.

**Target owner**:

- explicit node content facet (recommended),
- or a documented “behavior-tag” class if tags remain the intended carrier

**Next action**:

- prefer a narrow explicit content-facet carrier over a broad node-type hierarchy here.
- recommended direction: `NodeContentFacet::Clip(ClipFacetData)` with `#clip` retained as a derived compatibility projection for badge/query/tag surfaces.
- until that decision is made, clipping docs should treat `#clip` as a bridge carrier and avoid deepening assumptions that tag state is the final clip-type authority.

---

### 3.8 Pending orchestration/control-plane state is still spread across ad hoc fields

**Current carrier**:

- `pending_*` fields
- pending command queues
- focus authority queues

**Why it is inconsistent**:

- some command/control flows are explicit and authority-based,
- others still rely on staged fields and later reconciliation,
- this obscures the real runtime control plane.

**Target owner**:

- explicit `RuntimeAuthorityState`

**Next action**:

- continue migrating ad hoc pending fields into named authority/control-plane families.

---

### 3.9 Derived indexes read as if they are primary truth

**Current carrier**:

- `semantic_index`
- `node_workspace_membership`
- `node_last_active_workspace`
- `graph_view_frames`

**Why it is inconsistent**:

- these are all derived/runtime-ish carriers,
- but they sit beside canonical state in a monolithic container,
- which makes them look more authoritative than they are.

**Target owner**:

- `RuntimeDerivedState`

**Next action**:

- move these into an explicit derived/cache family during workspace decomposition.

---

### 3.10 `file_tree_projection_state` naming/ownership mismatch

**Current carrier**:

- `file_tree_projection_state`

**Why it is inconsistent**:

- comments frame it as graph-owned projection runtime state,
- behavior is much closer to tool/workbench projection state,
- it is not durable graph truth and not part of workbench arrangement semantics either.
- "file_tree" naming is superseded by the **Navigator** model (`2026-03-14_graph_relation_families.md §5`) — should be renamed to `navigator_projection_state` or similar.

**Target owner**:

- `WorkbenchSessionState` or a dedicated projection/tool-state carrier

**Next action**:

- reclassify it explicitly during the session/UI split; rename to `navigator_projection_state` at the same time.

---

## 4. Recommended Order of Architectural Cleanup

1. `workspace.semantic_tags` -> node-owned tags
2. `GraphWorkspace` decomposition into explicit state families
3. `GraphViewState` split
4. camera authority cleanup
5. duplicate carrier cleanup for pinning
6. node-runtime/session split
7. remaining pending/control-plane consolidation

This ordering is intentional:

- first remove the most misleading ownership mismatch,
- then decompose the mixed container,
- then fix nested mixed carriers.

---

## 5. Canonical Reading Rule

Until each inconsistency is migrated:

- treat the current carrier as a **bridge**, not as a canonical precedent
- do not add new features that deepen the inconsistency without an explicit note
- if a new feature depends on a questionable carrier, the plan for that feature should name the inconsistency and state whether it is depending on a bridge or correcting it

That rule is what prevents temporary bridges from silently becoming the architecture.

