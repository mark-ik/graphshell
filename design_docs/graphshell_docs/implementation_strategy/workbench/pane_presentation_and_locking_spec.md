# Pane Presentation and Locking Spec

**Date**: 2026-03-01
**Status**: Canonical interaction contract
**Priority**: Immediate implementation guidance

**Related**:

- `workbench_frame_tile_interaction_spec.md` — workbench arrangement contract
- `pane_chrome_and_promotion_spec.md` — **canonical authority for `PanePresentationMode` enum, graduated chrome model, Tile Viewer Chrome Strip, tab-selector overlay, transition triggers, tab reorder, and chrome rendering rules** (§§2–7); this spec is authoritative for `PaneLock` only
- `2026-03-03_pane_opening_mode_and_simplification_suppressed_plan.md` — `PaneOpeningMode`, graph-citizenship boundary, `SimplificationSuppressed`
- `../canvas/multi_view_pane_spec.md` — per-view graph pane isolation and hoist/unhoist operations
- `../subsystem_focus/focus_and_region_navigation_spec.md` — focus return paths and arbitration rules

---

## 1. Purpose and Scope

This spec defines the canonical `PaneLock` state model and the cross-cutting constraints that apply to every presentation or lock state change.

**Division of authority**:

- `pane_chrome_and_promotion_spec.md` owns: `PanePresentationMode` enum (`Tiled`/`Docked`/`Floating`/`Fullscreen`), graduated chrome model, Tile Viewer Chrome Strip, compatibility mode (Wry), tab-selector overlay rendering, presentation-mode transition triggers and effects, tab reorder semantics, and docked-pane close/restore behavior.
- This spec owns: `PaneLock` states and their behavioral rules, the invariant that any presentation or lock change must not orphan focus or mutate graph state, and the diagnostics contract covering both.
- `2026-03-03_pane_opening_mode_and_simplification_suppressed_plan.md` owns: whether a pane has graph citizenship (`QuarterPane`/`HalfPane`/`FullPane`/`Tile`), promotion semantics, and `SimplificationSuppressed`.

These three documents form a joint authority for pane lifecycle. No one document is sufficient alone; they should be read together.

---

## 2. Presentation Model Summary

> Full `PanePresentationMode` contract is in `pane_chrome_and_promotion_spec.md §§2–6`.

Canonical modes (summary only):

- `Tiled` — full chrome; participates in all tile-tree mobility.
- `Docked` — reduced chrome; position-locked from user interaction.
- `Fullscreen` — content-only; all chrome hidden (future; not in current scope).

**Key invariants (normative here)**:

1. `PanePresentationMode` changes must not mutate graph state — no node creation/deletion, no address write, no traversal append.
2. `PanePresentationMode` changes must not move the pane to a new position in the tile tree; only chrome rendering and mobility affordances change.
3. `PanePresentationMode` changes must not orphan focus; focus handoff follows `focus_and_region_navigation_spec.md §4.7`.

---

## 3. PaneLock Contract

`PaneLock` is a per-pane policy bit governing user-initiated structural operations. It is separate from `PanePresentationMode`.

```text
PaneLock =
  | Unlocked         -- all move/resize/reorder/close operations available
  | PositionLocked   -- content interaction allowed; user-initiated placement mutation blocked
  | FullyLocked      -- user-initiated placement, reorder, and close all blocked
```

### 3.1 `Unlocked`

No constraints beyond normal workbench rules.

### 3.2 `PositionLocked`

- User cannot drag the pane to a new position or reorder it in a Tab Group.
- User can still resize it (if the frame permits), focus it, and close it.
- Content within the pane is fully interactive.
- `Docked` panes are implicitly `PositionLocked` from the user's perspective; their `PaneLock` field is nonetheless separate from their `PanePresentationMode` and may be set independently.

### 3.3 `FullyLocked`

- All user-initiated structural operations are blocked: drag, reorder, close.
- `FullyLocked` is reserved for **system-owned panes** (e.g., a required diagnostics panel during a critical operation). It is not user-assignable through normal settings.
- Focus is still granted to `FullyLocked` panes.
- The system may release `FullyLocked` programmatically via explicit intent.

### 3.4 Lock state rules

- Lock state is explicit and queryable from runtime and UI.
- Lock state changes are routed through explicit `GraphIntent` variants; no direct field mutation from UI callsites.
- Forbidden operations on locked panes produce **explicit** visual or diagnostic feedback — silent failure is forbidden.
- Lock state is workbench-owned state; it does not affect graph content or node identity.

---

## 4. Focus and Close Handoff Contract

These invariants apply regardless of the current `PanePresentationMode` or `PaneLock` state:

1. **No orphaned focus**: toggling presentation mode or lock state must not leave the UI in a state with no valid focus owner.
2. **Deterministic close successor**: when a pane is closed, focus routes to the next pane via the canonical return-path algorithm (`focus_and_region_navigation_spec.md §4.7.3`). The successor must be determined before the close takes effect.
3. **Recoverable focus during hide/dock**: when a pane transitions to `Docked` or otherwise reduces its chrome, the prior focus owner must either retain focus or hand off to a deterministic successor. A later transition back to `Tiled` may restore focus if the pane was the prior owner, but must not do so silently if another pane claimed focus in the interim.
4. **Close under `FullyLocked`**: the system (not the user) may close a `FullyLocked` pane via intent; the same focus handoff rules apply.

---

## 5. Diagnostics Contract

All channels must follow the `namespace:name` pattern and the severity rules from `design_docs/graphshell_docs/research/2026-03-04_standards_alignment_report.md §3.6`.

| Channel | Severity | When emitted | Required payload fields |
| ------- | -------- | ------------ | ----------------------- |
| `workbench:pane_presentation_changed` | `Info` | `PanePresentationMode` changes | `tile_id`, `old_mode`, `new_mode` |
| `workbench:pane_lock_changed` | `Info` | `PaneLock` state changes | `tile_id`, `old_lock`, `new_lock`, `changed_by` (user or system) |
| `workbench:pane_lock_blocked_operation` | `Warn` | User attempts a blocked operation on a locked pane | `tile_id`, `lock_state`, `attempted_operation` |
| `workbench:pane_focus_handoff` | `Info` | Focus transfers as a result of close, hide, or presentation change | `source_tile_id`, `successor_tile_id`, `handoff_reason` |
| `workbench:pane_focus_orphan` | `Error` | A close or transition leaves no valid focus owner | `source_tile_id`, `attempted_successor`, `reason` |

`workbench:pane_focus_orphan` is `Error` severity because focus-orphan states are bugs in the focus handoff algorithm, not expected user-facing events.

---

## 6. Test Contract

Required test coverage:

1. `Tiled ↔ Docked` transition preserves pane `TileId` and tree position — no movement.
2. `Tiled ↔ Docked` transition does not create/delete graph nodes, write addresses, or append traversal history.
3. `PositionLocked` pane: drag attempt has no effect; close still works.
4. `FullyLocked` pane: drag, reorder, and user-close all produce explicit blocked feedback; system-close via intent succeeds.
5. Close handoff is deterministic: after close, focus lands on the expected successor per `focus_and_region_navigation_spec.md §4.7.3` — not on an arbitrary pane.
6. Presentation toggle while focused: focus is retained or handed off to a deterministic successor; `workbench:pane_focus_orphan` is never emitted.
7. Lock state change emits `workbench:pane_lock_changed`; blocked operation emits `workbench:pane_lock_blocked_operation`.

---

## 7. Acceptance Criteria

| Criterion | Verification |
| --------- | ------------ |
| `PanePresentationMode` change does not mutate graph state | Test (§6.2): switch `Docked ↔ Tiled` → no graph node, address, or traversal side-effects |
| `PanePresentationMode` change does not move pane in tile tree | Test (§6.1): switch → `TileId` at same tree position |
| `PositionLocked` blocks drag/reorder but allows close | Test (§6.3): drag attempt → no-op with explicit feedback; close button still works |
| `FullyLocked` blocks close/reorder; system-intent close succeeds | Test (§6.4): user close attempt → blocked with explicit feedback; `GraphIntent` close → succeeds |
| Close focus handoff is deterministic | Test (§6.5): close tile → focus on expected successor per return-path spec |
| Focus not orphaned by presentation/lock toggle | Test (§6.6): toggle mode while focused → `workbench:pane_focus_orphan` never emitted |
| Diagnostics emitted for presentation change | Test (§6.7): mode change → `workbench:pane_presentation_changed` in signal log |
| Diagnostics emitted for lock change | Test (§6.7): lock change → `workbench:pane_lock_changed` in signal log |
| Diagnostics emitted for blocked operation | Test (§6.7): blocked drag on locked pane → `workbench:pane_lock_blocked_operation` in signal log |
