# GraphTree / egui_tiles Decoupling Follow-On Plan

**Date**: 2026-04-11
**Status**: Active follow-on strategy
**Scope**: Define the next migration stage after `graph-tree` crate extraction:
move from a parallel mirrored `GraphTree` to a genuinely authoritative
Workbench/Navigator tree model, retire per-frame `egui_tiles` mirroring, and
close the correctness gaps surfaced by the first extraction review.

**Related**:

- `2026-04-10_graph_tree_implementation_plan.md` — original extraction and migration plan
- `../../technical_architecture/graph_tree_spec.md` — canonical `graph-tree` crate design
- `workbench_frame_tile_interaction_spec.md` — current Workbench authority and mutation semantics
- `graphlet_projection_binding_spec.md` — graphlet binding semantics consumed by GraphTree
- `../navigator/NAVIGATOR.md` — Navigator projection semantics to collapse into the shared tree
- `../subsystem_ux_semantics/ux_tree_and_probe_spec.md` — UxTree projection contract
- `../../technical_architecture/ARCHITECTURAL_OVERVIEW.md` — authority orientation

---

## 1. Why This Follow-On Exists

The `graph-tree` extraction has landed as a real workspace crate with:

- framework-agnostic core data structures,
- topology and navigation logic,
- layout computation,
- UxTree emission,
- unit and property tests,
- and initial Graphshell-side adapters and persistence hooks.

That is an important milestone, but it is **not yet the same thing** as
GraphTree becoming the authority.

Current repository reality:

- `GraphTree` exists as a parallel model in the desktop shell,
- `egui_tiles` still appears to be the live arrangement source,
- and the app currently mirrors tile state back into `GraphTree` at startup and
  once per frame.

This means the extraction is real, but the semantic handoff is not complete.

The next stage is therefore not "extract the crate" but:

**stop treating GraphTree as a disposable mirror and make it the semantic owner
of workbench/navigator tree state.**

---

## 2. Current Integration Reality

The current integration shape is:

1. `GraphTree` is restored from persistence if available.
2. The tile tree is then used to rebuild/synchronize the `GraphTree`.
3. That synchronization happens again every frame.

This is visible in:

- `shell/desktop/ui/gui.rs` startup restore + rebuild path
- `shell/desktop/ui/gui.rs` per-frame `rebuild_from_tiles(...)`
- `shell/desktop/workbench/graph_tree_sync.rs`

That is acceptable as a transitional bootstrap layer, but it creates two
problems:

- `GraphTree` cannot yet be trusted as the source of semantic truth,
- and topology-preserving behavior can be flattened or overwritten by the tile
  mirror.

---

## 3. Review Findings To Address

The initial extraction review surfaced four concrete follow-on concerns.

### 3.1 Orphaned traversal attaches

`Traversal` attaches can create members that exist in `members` but are not
reachable from any root if the source parent is missing or placement fails.

Root cause:

- `TreeTopology::attach_child` rejects self-parenting and duplicates, but does
  not validate that the proposed parent is actually present in the topology.
- `GraphTree::apply_attach` inserts the `MemberEntry` even if topology placement
  failed or implicitly targeted a nonexistent parent.

Consequence:

- member exists for persistence/counting/parity,
- but disappears from `visible_rows()`,
- and disappears from layout.

Required fix direction:

- either validate parent existence in `attach_child` / `reparent`,
- or make `apply_attach` fall back to a safe root/anchor placement when the
  requested topology insertion is invalid.

This is a correctness bug, not merely a migration inconvenience.

### 3.2 Topology flattening through tile mirroring

The current `rebuild_from_tiles` path attaches previously unseen tile nodes with
`Provenance::Restored`, which the current `GraphTree` attach logic treats as a
root placement.

This is not a risk — it is the current behavior. The tile tree has no concept
of provenance: it does not know *why* a pane exists (traversal, manual add,
derived graphlet, etc.). Every node gets `Provenance::Restored` and becomes a
root. The entire traversal-derived parent/child topology is destroyed every
frame and rebuilt as a flat list of roots.

Consequence:

- traversal children and graph-derived placements are silently collapsed into
  roots on every frame,
- topology-preserving behavior cannot survive the migration phase under the
  current per-frame rebuild model,
- and any topology information set through `graph_tree_commands` is
  immediately overwritten on the next frame.

The current parity check only validates set-level membership and therefore will
not catch this structural regression.

### 3.3 Persistence keying is too coarse

The crate contract is "one `GraphTree` per graph view," but persistence is
currently written through a single `graph_tree_latest` blob.

Consequence:

- multiple graph views cannot persist independently,
- future parallel trees or workspaces can overwrite each other,
- restored expansion/topology state can belong to the wrong view.

This must be re-keyed by `GraphViewId` before GraphTree can safely become the
authoritative persisted tree model.

### 3.4 Graphlet anchor durability

Linked graphlet binding currently serializes anchor references through
`format!("{:?}", node_key)` — a debug-format string that produces output like
`NodeIndex(42)`. This is a debug hack from the initial binding bridge, not a
stable identifier.

Fix: replace with `NodeKey::index()` as a stable integer. This is a 2-line
change in `graph_tree_binding.rs` (`register_linked_graphlet`) and should be
done in Phase A alongside other correctness hardening.

### 3.5 Memory policy command flow

The `memory_policy` module produces `SetLifecycle(Cold)` actions for
origin-aware lifecycle demotion. During the transition period where
`egui_tiles` is still the live rendering owner, these actions must flow
through both systems: GraphTree processes the NavAction, and the host must
also update the tile tree to reflect the demotion.

This is the same dual-write problem that affects all command paths during
transition (§6 Phase D), but memory policy is a new command source that did
not exist when the tile tree was designed and has no existing tile-side
equivalent. Phase D must account for it explicitly.

### 3.6 Compositor coupling

`tile_compositor.rs` (2,857 lines) keys content callbacks, GL state isolation,
and overlay passes by `TileId`. Phase E (layout from GraphTree) cannot land
without a compositor adapter that translates `GraphTree::compute_layout()`
results into the format the compositor expects. This is not a full compositor
redesign (§9), but it is a prerequisite adapter that Phase E must include.

---

## 4. Immediate Goal

The immediate goal of this follow-on is:

**make `GraphTree` the semantic authority for topology, activation, expansion,
and layout intent, while shrinking `egui_tiles` into a temporary rendering host
or compatibility layer.**

This does **not** require deleting `egui_tiles` on day one.

It does require stopping this current authority shape:

- `egui_tiles` owns live truth
- `GraphTree` is rebuilt from it repeatedly

and moving to:

- `GraphTree` owns semantic truth
- `egui_tiles` reflects or hosts that truth during transition

---

## 5. Migration Principles

### 5.1 Bootstrap once, do not rebuild forever

Reading an old `egui_tiles` layout and converting it into `GraphTree` is a
valid migration step.

Rebuilding `GraphTree` from `egui_tiles` every frame is not a valid long-term
authority model.

### 5.2 Semantic state must flow one way

The semantic direction should become:

`GraphTree` -> projection/adapters -> host rendering/layout

not:

`egui_tiles` -> mirror -> `GraphTree`

except for explicit legacy import or compatibility-only repair paths.

### 5.3 Parity checks must compare structure, not just sets

During the parallel migration phase, parity diagnostics must compare:

- membership
- parent/child relationships
- active member
- expansion state
- visible member order
- visible pane set

Membership-only parity is too weak to protect the migration.

### 5.4 Persistence must match the crate contract

If `GraphTree` is "one per graph view," persistence must be keyed that way
before the tree becomes authoritative.

### 5.5 `egui_tiles` should become an adapter, not a peer

If `egui_tiles` remains temporarily, it should remain only as:

- a host for spatial pane rectangles,
- a compatibility presentation structure,
- or a thin adapter over `GraphTree` layout output.

It should not remain a second semantic owner.

---

## 6. Recommended Phases

### Phase A: Correctness hardening before authority shift

Fix the problems that make authority migration unsafe.

Required work:

1. reject or safely recover invalid traversal-parent placement
2. ensure `reparent` also validates parent existence
3. add tests covering missing-parent attach behavior
4. add tests that verify no member can exist in `members` while being
   unreachable from roots unless explicitly modeled as hidden/off-tree state
5. decide whether linked graphlet anchors need durable canonical identifiers now

Done gate:

- no orphaned invisible members can be created through attach/reparent actions

### Phase B: Stop per-frame full rebuild

Retire the unconditional per-frame `rebuild_from_tiles(...)` path.

This is the highest-risk phase in the migration. Removing per-frame rebuild
means every mutation path that currently touches `tiles_tree` must *also*
dispatch the corresponding NavAction to `GraphTree` — or GraphTree drifts.
The coupling surface is large:

- ~40 functions in `tile_view_ops.rs` (open, close, split, tab, focus, etc.)
- compositor content callback registration
- persistence restore path
- webview lifecycle callbacks (map/unmap/crash)
- frame group operations

**Transition strategy**: dual-write. Every tile mutation site gains a
corresponding `graph_tree_commands` call that keeps GraphTree in sync.
This is the reverse of the current direction (tiles→GraphTree) but
preserves the same consistency guarantee until Phase D flips authority.
The dual-write layer should be a thin adapter — not 40 inline callsites —
so that it can be removed cleanly in Phase D.

Replace the per-frame rebuild with:

- startup import/bootstrap from tile state (one-shot migration)
- dual-write adapter for tile mutations during transition
- optional one-shot recovery or diagnostics repair path, not a frame path

Done gate:

- no frame path rebuilds `GraphTree` from the tiles tree
- dual-write adapter covers all tile mutation paths

### Phase C: Route semantic UI projections from GraphTree only

Make all tree-shaped or tab-shaped projections resolve from `GraphTree`.

Includes:

- Navigator/sidebar member lists
- tree-style tab bars
- active/focused member queries
- focus cycling
- reveal/expand behavior
- graphlet membership decoration

Done gate:

- these surfaces can run correctly from `GraphTree` without consulting
  `egui_tiles` for semantic grouping truth

### Phase D: Make commands hit GraphTree first

Route open/activate/dismiss/reparent/toggle-expand/reveal flows through the
`graph_tree_commands` layer first.

Any mutation of `egui_tiles` during transition should become a consequence of a
`GraphTree` change, not a sibling authority path.

Done gate:

- semantic workbench/nav commands are `GraphTree`-first

### Phase E: Make layout and pane rect derivation GraphTree-owned

Promote `GraphTree::compute_layout()` and the GraphTree layout facade to the
canonical pane-rect source.

This phase requires a compositor adapter. `tile_compositor.rs` (2,857 lines)
currently iterates tiles for content callback dispatch, GL state isolation,
and overlay passes, all keyed by `TileId`. The adapter must translate
GraphTree layout results (`HashMap<NodeKey, Rect>`) into the compositor's
expected input format — either by mapping `NodeKey → TileId` at the boundary,
or by rekeying the compositor from `TileId` to `NodeKey`.

At this stage, `egui_tiles` should no longer be the meaning-bearing layout
structure. It may still be:

- a temporary host widget,
- a rectangle presenter,
- or an internal adapter

but pane placement semantics should come from `GraphTree`.

Done gate:

- pane rects can be derived from `GraphTree` alone
- compositor can dispatch content callbacks using GraphTree-derived rects

### Phase F: Re-key persistence per graph view

Persist one `GraphTree` per `GraphViewId`.

Required outcomes:

- independent restore for multiple graph views
- no cross-view overwrites
- expansion/topology state scoped to the right view
- clear backward-compat migration from the current single-blob storage

Done gate:

- persistence shape matches the crate contract

### Phase G: Shrink or retire `egui_tiles`

Once semantic state, commands, layout intent, and persistence are all
`GraphTree`-owned, the project can decide:

- keep a minimal `egui_tiles` compatibility adapter for desktop only, or
- remove `egui_tiles` entirely

This final step should happen only after the earlier authority shifts are done.

Done gate:

- `egui_tiles` is no longer required as a semantic owner

---

## 7. Concrete Next Task Stack

Recommended next tasks in order:

1. **Topology safety fix**
   - validate parent existence in topology operations or add safe fallback in
     `apply_attach`
   - add unit/property tests for missing-parent traversal attaches

2. **Parity upgrade**
   - extend parity diagnostics beyond membership-set comparison
   - compare topology, active member, expansion state, and visible ordering

3. **Remove per-frame rebuild**
   - demote `rebuild_from_tiles` to startup import + explicit repair tooling

4. **GraphTree-first command routing**
   - ensure open/activate/dismiss/reveal/toggle-expand go through
     `graph_tree_commands`

5. **GraphTree-first projection wiring**
   - navigator/sidebar/tree tabs/focus cycle read from `GraphTree` only

6. **Per-view persistence migration**
   - replace the single `graph_tree_latest` blob with per-view storage keys

7. **Graphlet anchor identifier decision**
   - either formalize stable anchor identifiers now or mark current string
     serialization as intentionally transitional

---

## 8. Acceptance Shape

This follow-on is complete when Graphshell can truthfully say:

- `GraphTree` is not rebuilt from `egui_tiles` every frame,
- no `GraphTree` member can silently disappear from visible topology due to an
  invalid attach,
- traversal-derived parent/child structure survives the migration path,
- parity diagnostics can catch structural drift rather than only membership drift,
- persisted tree state is scoped per graph view,
- and `egui_tiles` is no longer a peer semantic authority beside `GraphTree`.

---

## 9. Non-Goals

This follow-on does **not** require:

- immediate deletion of all `egui_tiles` code,
- redesign of Workbench command semantics,
- redesign of Navigator click grammar,
- or replacement of the compositor pipeline.

It is an authority migration and correctness hardening pass, not a wholesale UI
rewrite.
