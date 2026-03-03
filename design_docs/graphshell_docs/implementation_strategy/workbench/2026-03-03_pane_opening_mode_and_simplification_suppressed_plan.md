# Pane Opening Mode and Simplification Suppressed Plan

**Date**: 2026-03-03
**Status**: Active / Canonical planning slice
**Purpose**: Define the runtime contract for pane opening mode, graph-citizenship boundaries, and `SimplificationSuppressed` behavior so workbench structure and graph identity stop drifting.

**Canonical references**:

- `../../../TERMINOLOGY.md`
- `pane_chrome_and_promotion_spec.md`
- `2026-02-22_workbench_tab_semantics_overlay_and_promotion_plan.md`
- `graph_first_frame_semantics_spec.md`
- `../2026-03-03_spec_conflict_resolution_register.md`

---

## 1. Why This Plan Exists

Graphshell currently risks conflating three different concepts:

1. Opening a pane.
2. Giving that pane graph citizenship.
3. Changing the pane's visible chrome.

This plan separates them. Pane Opening Mode owns the first two. Pane Presentation Mode owns the third.

---

## 2. Canonical Opening Modes

```rust
PaneOpeningMode =
  | QuarterPane
  | HalfPane
  | FullPane
  | Tile
```

Semantics:

- `QuarterPane`, `HalfPane`, and `FullPane` are ephemeral opening modes.
- `Tile` is the graph-backed opening mode.
- Opening-mode choice is made before pane presentation chrome is considered.
- `Tiled` / `Docked` remain presentation choices and do not change opening mode by themselves.

---

## 3. Graph-Citizenship Contract

### 3.1 Ephemeral modes

When a pane opens in `QuarterPane`, `HalfPane`, or `FullPane`:

- a visible pane may be created,
- the pane may receive focus,
- the pane may participate in local workbench layout,
- no graph node is created solely because the pane opened,
- no address is written solely because the pane opened.

### 3.2 Tile mode

When a pane opens in `Tile`:

- the pane must resolve to a stable address,
- that address must be written through the canonical graph write path,
- a graph node is created or reused according to address-as-identity rules,
- the pane becomes graph-backed and eligible for frame membership, traversal linkage, and graph-owned reopening.

### 3.3 Transition into graph citizenship

A pane may start ephemeral and later transition to `Tile`.

That transition:

- is the canonical **Promotion** event,
- changes graph citizenship,
- may create a node,
- may trigger deferred traversal edge assertion if history capture is appropriate,
- must not be confused with a `Docked <-> Tiled` chrome change.

---

## 4. Structural Constraints

Opening mode constrains what structural transforms are legal while a pane remains ephemeral.

Rules:

1. Ephemeral panes may exist inside the workbench tree, but they are not semantic graph tiles.
2. Structural normalization (`simplify()`) may not silently convert an ephemeral pane into a graph-backed tile.
3. Structural normalization may collapse ephemeral-only containers only when the resulting pane remains ephemeral and address-free.
4. A graph-backed `Tile` may be structurally rewrapped or unwrapped only if graph citizenship remains unchanged.

---

## 5. `SimplificationSuppressed` Contract

`SimplificationSuppressed` is a per-pane or per-container policy bit that blocks structural collapse when simplification would erase required reopening semantics.

Use it when:

- a pane rest state must remain recoverable without losing semantic-tab metadata,
- an ephemeral pane is in a transient flow where automatic collapse would change user-visible reopening behavior,
- a reducer-driven transition is mid-flight and the tree must remain stable until confirmation.

Rules:

1. `SimplificationSuppressed` blocks automatic structural normalization only; it does not block explicit user close.
2. It is temporary policy, not graph identity.
3. Clearing it re-allows normal simplify behavior.
4. It must not be used as a hidden substitute for graph citizenship.

---

## 6. Dismissal Semantics

Closing behavior depends on opening mode:

- Closing an ephemeral pane removes only the open handle.
- Closing a `Tile` handle removes only the handle by default; graph node deletion remains a separate destructive action.
- Dismissing a transient ephemeral pane must not leave behind orphan graph records.
- Promoting an ephemeral pane to `Tile` before dismissal moves it onto the graph-backed close/delete path.

---

## 7. Integration Points

The following boundaries must consume this plan:

- Opening reducers: decide `PaneOpeningMode` before creating pane runtime state.
- Workbench layout apply layer: preserve opening mode across split/reorder/restore operations.
- Address issuance: run only when entering `Tile`.
- Traversal reducer: treat promotion-to-`Tile` as the only opening-mode transition that can emit `NavigationTrigger::PanePromotion`.
- Pane chrome rendering: read presentation mode only; never infer graph citizenship from visible tab chrome.

---

## 8. Validation Gates

This plan is considered landed only when all of the following are true:

1. Opening an ephemeral pane does not create a graph node.
2. Entering `Tile` always routes through the canonical address write path.
3. `Docked <-> Tiled` changes do not alter opening mode.
4. `SimplificationSuppressed` prevents unwanted structural collapse in at least one restore/simplify scenario test.
5. Close/dismiss behavior is distinct for ephemeral vs graph-backed panes and is test-covered.

Suggested scenario checks:

- Open content in `HalfPane` -> confirm no graph node exists.
- Promote the same pane to `Tile` -> confirm address write and node creation/reuse.
- Toggle `Docked <-> Tiled` after promotion -> confirm no new graph mutation.
- Run simplify on a suppressed pane-rest container -> confirm structure is preserved until suppression clears.

---

## 9. Immediate Implementation Slices

1. Add `PaneOpeningMode` to pane runtime state where opening decisions are stored today.
2. Introduce `SimplificationSuppressed` in workbench structural policy state.
3. Route promotion-to-`Tile` through one explicit reducer path.
4. Add diagnostics fields for `opening_mode`, `is_graph_backed`, and `simplification_suppressed`.
5. Update affected specs to reference this plan as the authority for opening semantics.
