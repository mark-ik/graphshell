# Radial Palette Geometry and Overflow Spec

**Date**: 2026-03-01  
**Status**: Canonical interaction contract  
**Priority**: Pre-renderer/WGPU required

**Related**:
- `command_surface_interaction_spec.md`
- `../2026-03-01_ux_migration_design_spec.md` (§5.6)
- `../subsystem_ux_semantics/ux_tree_and_probe_spec.md`
- `../subsystem_ux_semantics/ux_scenario_and_harness_spec.md`

---

## 1. Purpose and Scope

This spec defines Radial Palette Mode layout, hit testing, overflow behavior, and readability invariants.

It governs:

- tier-1 and tier-2 ring geometry,
- label placement and overlap prevention,
- overflow strategy and deterministic ordering,
- periphery-rail positioning and hover scaling behavior,
- keyboard/gamepad parity,
- diagnostics and test contracts.

It does not govern:

- action semantics,
- command ranking,
- omnibar or palette behavior outside radial activation.

---

## 2. Geometry Contract

### 2.1 Hub circle and summon behavior

- Right-click contextual summon draws a hub-circle outline at pointer origin.
- Default hub diameter target is approximately three mouse-cursor widths.
- Hub diameter is configurable per profile.

### 2.2 Tier-1 category ring

- Tier 1 represents categories only.
- Exactly 1–8 category buttons are visible per page.
- Buttons sit on a periphery rail and may be repositioned by user along that rail.
- Assignment/order is deterministic after user customization is applied.
- Activation origin is pointer (or focused node center for keyboard invocation).

### 2.3 Tier-2 option ring

- Tier 2 represents options for the currently selected Tier-1 category.
- Tier-2 entries occupy unique angular lanes; no z-stacked entries at same radius/lane.
- Tier-2 radius is configurable per profile and must be greater than hub radius by fixed tokenized spacing.
- Tier-1 category semantics are equivalent to Context Palette Tier-1 strip; Tier-2 option semantics are equivalent to Context Palette Tier-2 list.

### 2.4 Hit targets and hover scaling

- Every visible sector hit region must meet minimum logical size requirements.
- Hit region may exceed visual wedge footprint to preserve accessibility target size.
- Default button size is compact for dense packing.
- Hovered button expands up to half of the hub-circle radius, then returns to compact size on hover exit.
- Hover scaling must not violate lane non-overlap constraints.

---

## 3. Overflow Policy

Overflow resolution order:

1. Context-eligible actions only,
2. Tier-1 ring gets top-priority categories (max 8 per page),
3. Tier-2 ring gets commands for selected category (max 8 per page),
4. Remaining categories/options paginate by deterministic page index.

Determinism invariant:

- Given identical action set + context + profile, ring assignment order is stable.

---

## 4. Readability Contract

### 4.1 Label layout

- Labels are bounded text fields aligned radially away from the center and anchored to each button.
- Labels are hidden when the button is not hovered.
- Hover reveals the label; overflow text is revealed via gentle radial-direction scrolling in-field.
- On Tier-1 category selection, Tier-1 radial labels collapse and selected category label appears in hub.
- Tier-2 labels follow the same bounded radial text-field rule.
- Label bounding boxes must not overlap at final layout.
- On conflict, resolver applies: radial offset → in-field truncation/scroll policy → pagination.

### 4.2 Contrast and state

- Focused/hovered/disabled sectors have distinct visual states.
- Disabled sectors remain readable but non-invokable.

### 4.3 Input parity

- Keyboard and gamepad can invoke any visible sector action without pointer movement.
- Numeric/compass mapping remains stable across openings.
- Radial Palette Mode must be usable both with drag-gesture selection and with hover/click selection.

---

## 5. UxTree and Diagnostics Contract

### 5.1 UxTree

When open, Radial Palette Mode subtree must include:

- root `radial-palette` node,
- Tier-1 category ring node + one `RadialSector` per visible category button,
- Tier-2 option ring node + one `RadialSector` per visible option button,
- action id metadata per sector,
- explicit `enabled` state,
- per-button rail position metadata,
- per-button hover-scale state.

### 5.2 Diagnostics

Required channels:

- `ux:radial_layout` (Info; sector count, ring counts, page)
- `ux:radial_overflow` (Warn; overflow pages used)
- `ux:radial_label_collision` (Warn; pre/post collision counts)
- `ux:radial_mode_fallback` (Warn; radial → context palette fallback reason)

---

## 6. Test Contract

Required scenario coverage:

1. 1–8 category contexts render all Tier-1 categories on one ring page.
2. Tier-1 category selection drives Tier-2 option ring for selected category.
3. >8 categories/options trigger deterministic paging.
4. Tier-1/Tier-2 rings never produce lane/radius overlap under hover scaling.
5. Label collision resolver converges with zero final overlaps.
6. Keyboard/gamepad invocation parity with pointer invocation.
7. Drag-gesture and click/hover flows both complete command dispatch + dismiss.
8. Click-away dismisses without mutation.

---

## 7. Acceptance Criteria

- [ ] Geometry constraints in §2 are implemented.
- [x] Overflow policy in §3 is deterministic and tested.
- [ ] Readability constraints in §4 hold across supported DPI tiers.
- [x] UxTree/diagnostics outputs in §5 are implemented.
- [ ] Scenario tests in §6 are part of CI UX contract gate.
- [ ] Radial hub and per-tier diameters are profile-configurable.

Implementation notes:

- Deterministic tier paging, overflow telemetry, and fallback diagnostics are live in `render/radial_menu.rs`.
- UxTree now projects explicit Tier-1/Tier-2 ring container nodes and a radial overflow/readability summary node in `shell/desktop/workbench/ux_tree.rs`.
- Radial Tier-1 category ordering now reuses shared context+recency+pin policy from `render/action_registry.rs`, matching Context Palette Mode behavior.
- Runtime radial geometry tuning is available in radial mode (`Alt+Up/Down` for Tier-1 ring radius, `Alt+Shift+Up/Down` for Tier-2, `Alt+Ctrl+Up/Down` for hub); values persist via UI state keys.
- Scenario coverage now includes summon gating and pin round-trip checks in `shell/desktop/workbench/tile_post_render.rs` and `render/command_palette.rs`.
- Remaining open items are full readability convergence across DPI tiers and CI scenario-gate expansion from §6.
