# Graphlet Projection and Binding Spec

**Date**: 2026-03-25
**Status**: Canonical / Active
**Scope**: Defines how Workbench tile groups bind to canonically defined graphlets, how graphlet projection scopes compose for arrangement workflows, and when the user must be warned before a selector change rewrites linked graphlet structure.

**Related**:

- `../../technical_architecture/graphlet_model.md` — canonical graphlet semantics across domains
- `../canvas/multi_view_pane_spec.md`
- `workbench_frame_tile_interaction_spec.md`
- `workbench_layout_policy_spec.md`
- `../navigator/NAVIGATOR.md`
- `../navigator/navigator_interaction_contract.md`
- `../graph/2026-03-14_graph_relation_families.md` — family-oriented Navigator modes and projection semantics
- `../../TERMINOLOGY.md`
- `../../../archive_docs/checkpoint_2026-03-21/2026-03-20_arrangement_graph_projection_plan.md` — historical background

**Alignment note (2026-03-27)**: newer Navigator planning distinguishes between
graphlet-oriented projection forms and relation-family-oriented section/mode
forms. This spec is only about the graphlet-binding side of that model:

- graphlets remain the canonical object for ego/corridor/component/frontier and
  other bounded local-world derivations,
- relation-family modes (`Workbench`, `Containment`, `Semantic`, `All nodes`)
  remain Navigator-owned projection shapes defined in
  `graph/2026-03-14_graph_relation_families.md`,
- Workbench binding must be able to consume graphlets without redefining them,
  and must not accidentally treat all Navigator family-oriented modes as
  graphlet links.

---

## 1. Purpose

Graphshell already distinguishes graph truth from workbench presentation, but
Workbench still needs a precise answer for how arrangements relate to graphlets:

- graphlet membership depends on which selectors and derivation rules are active,
- Workbench must consume graphlets without redefining them,
- a tile group may either stay linked to a graphlet definition or detach and persist as an arrangement snapshot.

This spec makes the binding model explicit so graph filtering, Navigator projection, and Workbench grouping all speak the same language.

Practical boundary:

- if the user is in a graphlet-oriented workflow, Workbench may bind to that
  graphlet and expose linked/detached behavior;
- if the user is in a family-oriented Navigator mode, Workbench may still open
  nodes or arrangements from that projection, but that does not automatically
  imply a `GraphletBinding::Linked` relationship.

---

## 2. Canonical Graphlet Definition

The canonical graphlet definition lives in `../../technical_architecture/graphlet_model.md`.

For Workbench purposes, the important constraints are:

- graphlets are projection-derived unless explicitly promoted elsewhere,
- graphlets are scope-sensitive and recomputable,
- graphlets are not synonymous with tile groups or frames,
- Workbench may bind to a graphlet, but does not become the owner of graphlet truth.

When this spec discusses graphlets, it is discussing graphlets as consumed by Workbench binding and routing.

Important non-equivalence:

- a relation-family section or Navigator mode is not automatically a graphlet,
- a graphlet may be derived using selectors that mention relation families,
  but the resulting object is still a bounded graphlet with anchors and
  derivation rules, not merely "whatever rows are visible in Navigator."

---

## 3. Edge Projection Model

### 3.1 `EdgeProjectionSpec`

The active graphlet definition is carried by an `EdgeProjectionSpec`.

Suggested shape:

```rust
pub struct EdgeProjectionSpec {
    pub selectors: Vec<RelationSelector>,
    pub source: ProjectionSource,
}

pub enum ProjectionSource {
    GraphDefault { graph_id: GraphId },
    GraphViewOverride { graph_view_id: GraphViewId },
    SelectionOverride {
        graph_view_id: GraphViewId,
        seed_nodes: Vec<NodeKey>,
    },
}
```

The `selectors` field names which edge families/sub-kinds contribute to
connectivity. The `source` field names where that choice came from.

This is a graphlet-projection contract, not a generic contract for every
Navigator projection mode. Family-oriented Navigator modes may reuse similar
selector vocabulary, but `EdgeProjectionSpec` here exists specifically to
define bounded graphlet derivation for binding/routing.

### 3.2 Scope Precedence

Projection resolution follows:

`SelectionOverride -> GraphViewOverride -> GraphDefault`

Meaning:

- a selection-level override affects only the targeted selection workflow
- a graph-view override affects only that `GraphViewId`
- a graph-level default applies when no narrower override exists

### 3.3 Scope Semantics

#### Graph default

- Sets the default edge projection for a `GraphId`.
- Affects graph views that have not declared their own override.
- Does not implicitly overwrite a graph-view or selection override.

#### Graph view override

- Applies only to the target `GraphViewId`.
- May produce graphlets different from sibling graph views over the same graph.
- Must not mutate other graph views' graphlet projections.

#### Selection override

- Applies only to the selected nodes and the workflow launched from them.
- This is the contract behind "multi-select some nodes, turn on History or
  Traversal edges, then enter the workbench with the resulting graphlet warm."
- Must not rewrite unrelated graphlets elsewhere in the graph view.

### 3.4 Multiselection Selector Ranking

When the user has a multiselection and is choosing which relation
families/selectors to project, the selector tool must rank candidates by how
well they explain or connect the current selection.

Minimum ranking rule:

1. rank selectors that produce a component containing **all** selected nodes
   above selectors that only cover a subset
2. among partial matches, rank selectors by **selection coverage count**:
   number of selected nodes that participate in at least one edge matching that
   selector
3. use **largest resulting component size containing any selected node** as the
   next tie-breaker
4. use domain-specific preference only after coverage-based ranking

This keeps the tool honest: when a user selects five nodes, a selector that
meaningfully touches all five should appear above one that only explains two.

Suggested score shape:

```rust
pub struct SelectionProjectionCandidate {
    pub selector: RelationSelector,
    pub selected_node_coverage: usize,
    pub total_selected_nodes: usize,
    pub largest_component_size: usize,
    pub fully_connects_selection: bool,
}
```

### 3.5 Chronological / Spawn-Order Fallback

Some multiselections will have no single relation selector that fully connects
all selected nodes.

Typical examples:

- a hand-picked bundle of recent nodes that share no durable semantic edge
- a loose investigation set where only some members have traversal history
- new nodes that are related primarily by creation order or session context

In those cases, the selector tool should still surface useful options instead of
showing an empty or misleading list.

Required behavior:

- rank partial selectors using the coverage rule in §3.4
- if no selector fully connects the selection, offer **chronological
  organization** as a fallback mode
- chronological fallback may use:
  - navigation/history ordering, when available
  - node spawn/creation ordering, when history does not connect the full set

Important constraint:

- chronological fallback is an **organization mode**, not proof that all nodes
  belong to one edge-defined graphlet under the normal selector model

This should be legible in UI copy. For example:

- "No single relation family connects all 6 selected nodes"
- "Best coverage: History (4/6), Traversal (3/6), User Grouped (2/6)"
- "Open as chronological sequence instead"

---

## 4. Graphlet Projection vs Arrangement Binding

Graphlet projection and workbench arrangement are separate concerns.

- The **graphlet** answers: "which nodes and edges belong to the currently
   resolved meaningful graph subset under the active derivation rules?"
- The **tile group** answers: "which panes are currently arranged together in
  the workbench, and how?"

The relationship between them must therefore be explicit.

### 4.1 `GraphletBinding`

Suggested shape:

```rust
pub enum GraphletBinding {
    DetachedSnapshot,
    Linked {
        projection: EdgeProjectionSpec,
        seed_nodes: Vec<NodeKey>,
        member_nodes: Vec<NodeKey>,
    },
}
```

Meaning:

- `DetachedSnapshot`: the tile group is just an arrangement. It keeps its
  current tiles and layout regardless of future graphlet changes.
- `Linked`: the tile group is explicitly attached to a graphlet definition. Its
  roster is expected to correspond to the graphlet produced by the stored
  projection and seeds.

Non-goal clarification:

- opening content from a Navigator family section does not, by itself, create a
  `Linked` graphlet binding;
- `Linked` is reserved for arrangements that explicitly follow a bounded
  graphlet definition and therefore must participate in selector/binding warning
  logic.

### 4.2 Binding Invariant

If a tile group is `Linked`, the workbench must treat changes to the underlying
graphlet as structurally significant. If recomputation would change the member
set, the system must not silently proceed as though nothing happened.

### 4.3 Arrangement Invariant

Layout geometry is not graphlet truth.

Even when a tile group is linked to a graphlet:

- tab order
- split geometry
- focused tile
- docked/floating arrangement choices

remain workbench arrangement state, not graph truth.

---

## 5. Binding Modes the User Must See

The UI must make linked vs detached state legible.

Minimum model:

- **Linked to graphlet** — selector changes may change membership; the group is
  structurally coupled to graphlet recomputation
- **Detached arrangement** — selector changes do not rewrite the group's roster

This distinction may be shown via badge, chip, context-menu label, or pane
chrome subtitle, but it must be visible somewhere the user can inspect before
changing selectors.

---

## 6. Selector Change Warning Rules

Changing active edge selectors can:

- overwrite a previous projection choice
- reshape a linked graphlet
- split one linked graphlet into several
- merge previously separate linked graphlets
- cause a tile group to cease matching the graphlet it was following

Those cases require explicit warning.

### 6.1 Warn When Overwriting a Prior Projection

Warn before applying a selector change when the target scope already has a
non-empty explicit projection that would be replaced.

Examples:

- replacing a graph-view override with a different selector set
- applying a new selection override to a selection that already has one
- promoting a selection override into a linked workbench group that already
  follows another projection

### 6.2 Warn When a Linked Group Would Change Membership

Warn before applying a selector change when a `Linked` tile group's recomputed
member set would differ from its current linked member set.

Difference includes:

- one or more members would be added
- one or more members would be removed
- the graphlet would split into multiple components
- the graphlet would merge with another linked graphlet

### 6.3 No Structural Warning for Detached Groups

Detached arrangements do not require a graphlet-structure warning when selectors
change, because their roster is no longer governed by graphlet projection.

They may still show an informational notice such as "current arrangement is
detached from graphlet structure," but this is not a blocking confirmation.

### 6.4 Seed Node Deletion

If any node in a `Linked` binding's `seed_nodes` is deleted from graph truth
(tombstoned), the binding can no longer resolve its projection. The system
must not silently leave the binding in a `Linked` state with an unresolvable
projection.

Required behaviour on seed node deletion:

1. Detect that a `Linked` binding's seed set now contains a tombstoned key.
2. Emit a diagnostics event (severity `Warn`) identifying the affected binding
   and the deleted seed keys.
3. Offer the user a two-outcome choice (no Cancel, since deletion has already
   occurred):
   - **Rebase to remaining seeds** — if any non-tombstoned seeds remain,
     recompute the graphlet from the surviving seed set and update
     `member_nodes`. Binding stays `Linked`.
   - **Detach** — convert the binding to `DetachedSnapshot`, preserving the
     current tile group roster as-is.
4. If all seed nodes are tombstoned, the binding cannot stay `Linked`.
   Auto-convert to `DetachedSnapshot` and emit a `Warn` event.

This case is distinct from a selector change: the selector is unchanged but
the projection source is partially or fully invalid.

---

## 7. Required Confirmation Choices

When a selector change would break or overwrite a linked graphlet relationship,
the confirmation surface must offer all three outcomes:

1. **Apply and keep linked**
   - Commit the selector change.
   - Recompute the graphlet.
   - Reconcile the tile group's member roster to the new graphlet.

2. **Apply and detach arrangement**
   - Commit the selector change.
   - Preserve the current tile group roster/layout as-is.
   - Convert the tile group to `DetachedSnapshot`.

3. **Cancel**
   - Do not change selectors.
   - Preserve both the current graphlet projection and arrangement binding.

This choice is the explicit answer to whether arrangement should remain
associated with graphlet structure or not. It must not be an implicit side
effect hidden behind selector toggles.

---

## 8. Workflows

### 8.1 Multi-select -> Enable Traversal/History -> Enter Workbench

1. User selects nodes in a graph view.
2. User enables additional edge families/selectors for that selection.
3. System creates a `SelectionOverride`.
4. Graphlet is computed from the selected nodes under that projection.
5. Workbench opens with the warm members of that resulting graphlet.
6. The resulting tile group starts as `Linked` unless the user explicitly asked
   for a detached arrangement.

This workflow must not alter unrelated graphlets elsewhere in the graph view.

### 8.1A Multi-select With No Fully Connecting Selector

1. User selects several nodes.
2. System ranks candidate selectors by coverage across the current selection.
3. No selector yields a graphlet containing all selected nodes.
4. User may still choose:
   - the highest-coverage selector, producing a partial graphlet rooted in the
     covered subset, or
   - chronological fallback, opening the selection as an ordered workbench
     sequence
5. The resulting workbench surface must indicate whether it is:
   - linked to a true edge-projected graphlet, or
   - an ordered detached arrangement derived from chronology/session order

The system must not silently pretend that chronology created a normal connected
graphlet when it did not.

### 8.2 Changing the Graph Default

1. User changes graph-wide selectors.
2. System evaluates all inheriting graph views and linked groups that depend on
   the graph default.
3. If no linked groups would change membership, apply immediately.
4. If any linked group would change membership, show the confirmation choices in
   §7.

### 8.3 Changing a Graph View Override

1. User changes selectors for one `GraphViewId`.
2. Only that view's graphlets and linked groups are evaluated.
3. Sibling views over the same `GraphId` remain unchanged.

### 8.4 Re-linking a Detached Group

1. User chooses a detached tile group.
2. User explicitly selects "Link to current graphlet" or equivalent.
3. System stores `GraphletBinding::Linked` using the current projection and seed
   context.
4. Future selector changes once again participate in the warning flow.

---

## 9. Navigator Contract Consequences

Navigator graphlet rows must read from the same resolved projection model:

- graph default when no narrower scope exists
- graph-view override when present
- selection override when the Navigator is projecting that workflow context

Navigator rows must not assume that only durable `UserGrouped` or
`FrameMember` edges define graphlet membership.

Compatibility fallback is allowed during migration, but the intended authority
is selector-resolved graphlet projection, not the old durable-only
interpretation.

Companion rule:

- family-oriented Navigator sections/modes defined in
  `graph/2026-03-14_graph_relation_families.md` remain valid Navigator
  projections even when no linked graphlet binding exists;
- the Workbench should only invoke this spec's binding warnings and linked/
  detached semantics when the active workflow is actually graphlet-linked.

---

## 10. Workbench Contract Consequences

Workbench routing must distinguish between:

- opening a node into an existing linked graphlet group
- opening a node into a detached arrangement
- creating a new linked group from a selection override
- opening nodes from family-oriented Navigator projection without establishing a
  graphlet binding

The current durable-only routing helpers are compatibility defaults, not the
final semantic model.

---

## 11. Suggested Runtime Types

```rust
pub struct ResolvedGraphletContext {
    pub projection: EdgeProjectionSpec,
    pub seed_nodes: Vec<NodeKey>,
    pub members: Vec<NodeKey>,
}

pub enum SelectorChangeImpact {
    NoStructuralChange,
    OverwritesExistingProjection,
    ChangesLinkedMembership {
        added: Vec<NodeKey>,
        removed: Vec<NodeKey>,
    },
}

pub enum GraphletRewriteChoice {
    ApplyKeepLinked,
    ApplyDetachArrangement,
    Cancel,
}

pub struct SelectionProjectionCandidate {
    pub selector: RelationSelector,
    pub selected_node_coverage: usize,
    pub total_selected_nodes: usize,
    pub largest_component_size: usize,
    pub fully_connects_selection: bool,
}
```

These are not mandated byte-for-byte, but the reducer/UI contract must carry
equivalent information.

---

## 12. Acceptance Criteria

This model is correctly implemented when:

1. Graphlet computation is explicitly driven by resolved selectors, not a hidden
   durable allowlist.
2. Selector changes can be applied at graph, graph-view, and selection scope.
3. A selection-scoped graphlet workflow can warm/open only the resulting
   graphlet without mutating unrelated graphlets.
4. Tile groups can be inspected as either linked or detached.
5. Selector changes that would rewrite a linked graphlet produce a confirmation
   flow with the three outcomes from §7.
6. Detached tile groups survive selector changes without structural mutation.
7. Navigator and workbench routing consume the same resolved graphlet context.
8. Multiselection selector suggestions are ranked by selection coverage before
   any cosmetic or domain-specific preference ordering.
9. When no selector fully connects the selection, the system can offer
   chronological organization without mislabeling it as a normal connected
   graphlet.
