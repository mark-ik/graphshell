# Pane Presentation and Locking Spec

**Date**: 2026-03-01  
**Status**: Canonical interaction contract  
**Priority**: Immediate implementation guidance

**Related**:
- `workbench_frame_tile_interaction_spec.md`
- `pane_chrome_and_promotion_spec.md`
- `../canvas/multi_view_pane_spec.md`
- `../subsystem_focus/focus_and_region_navigation_spec.md`

---

## 1. Purpose and Scope

This spec defines canonical pane presentation and lock behavior for workbench surfaces.

It governs:

- tiled vs docked pane presentation semantics,
- `PaneLock` state model and behavior,
- interaction constraints for locked panes,
- focus and close-hand-off behavior constraints.

---

## 2. Presentation Model

Presentation states:

- `TiledPane`: pane participates in tile-tree layout.
- `DockedPane`: pane is docked/chrome-managed per workbench policy.

Transition contract:

- transitions must preserve pane identity,
- transitions must preserve content binding (node/tool association),
- transitions must not mutate graph semantics.

---

## 3. PaneLock Contract

`PaneLock` states:

- `Unlocked`: full move/resize/reorder operations allowed.
- `PositionLocked`: content interaction allowed; placement mutation blocked.
- `FullyLocked`: placement and structural mutation blocked except explicit unlock.

Behavior invariants:

- lock state is explicit and queryable in runtime/UI semantics,
- forbidden operations on locked panes produce explicit feedback,
- lock state changes are routed through explicit intents.

---

## 4. Focus and Close Contract

Focus invariants:

- pane presentation or lock toggles must not orphan focus,
- close operation must hand off focus to deterministic successor,
- hidden/docked transitions must retain recoverable focus path.

---

## 5. Diagnostics Contract

Required channels:

- `workbench:pane_presentation_changed` (Info)
- `workbench:pane_lock_changed` (Info)
- `workbench:pane_lock_blocked_operation` (Warn)
- `workbench:pane_focus_handoff` (Info)

Minimum payload:

- `tile_id`/pane identity,
- old/new presentation,
- old/new lock state,
- attempted operation (when blocked),
- focus predecessor/successor.

---

## 6. Test Contract

Required coverage:

1. tiled↔docked transition preserves pane identity/binding,
2. `PositionLocked` blocks structural movement operations,
3. `FullyLocked` blocks close/reorder unless explicitly allowed by policy,
4. close successor focus handoff remains deterministic across lock states.

---

## 7. Acceptance Criteria

- [ ] Presentation and lock models in §§2-3 are implemented.
- [ ] Focus/close invariants in §4 hold in scenario tests.
- [ ] Diagnostics in §5 are emitted.
- [ ] Tests in §6 are CI-gated.
