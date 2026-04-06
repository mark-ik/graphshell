# UxTree Event Dispatch Spec

**Date**: 2026-03-01  
**Status**: Canonical interaction contract  
**Priority**: Pre-renderer/WGPU required

**Related**:
- `../2026-03-01_ux_migration_design_spec.md` (§3.2, §7)
- `2026-04-05_command_surface_observability_and_at_plan.md`
- `ux_tree_and_probe_spec.md`
- `ux_scenario_and_harness_spec.md`
- `../aspect_input/input_interaction_spec.md`
- `../subsystem_focus/focus_and_region_navigation_spec.md`

---

## 1. Purpose and Scope

This spec defines the canonical dispatch contract for Graphshell UX events routed through UxTree.

It governs:

- event path construction (`composedPath` equivalent),
- capture/target/bubble phase ordering,
- modal isolation and propagation controls,
- authority routing from dispatched actions to Graph vs Workbench mutation paths,
- deterministic testability and diagnostics outputs.

It does not govern:

- concrete keybinding assignments (see input spec),
- visual rendering behavior,
- ActionRegistry command semantics themselves.

---

## 2. Dispatch Model

### 2.1 Event phases

Every dispatchable `UxEvent` follows exactly four steps:

1. **Capture phase**: root → target parent
2. **At target phase**: target node handlers
3. **Bubble phase**: target parent → root (reverse order)
4. **Default action phase**: execute default action if not prevented

### 2.2 Path resolution

`target_path` must be a stable ordered list of `UxNodeId`s from root to target.

Path invariants:

- root appears exactly once at index 0,
- target appears exactly once at the end,
- no duplicate intermediate nodes,
- every node in path exists in current frame snapshot.

If path resolution fails, emit `ux:navigation_violation` and abort dispatch.

---

## 3. Propagation Controls

### 3.1 stopPropagation

Stops further traversal to later nodes/phases but does not cancel already executed handlers.

### 3.2 stopImmediatePropagation

Stops further handlers on the same node and all later nodes/phases.

### 3.3 preventDefault

Blocks the default-action phase while preserving propagation unless separately stopped.

---

## 4. Modal Isolation Contract

When a modal subtree is active (`Dialog`, Radial Palette Mode, command palette):

1. Capture handler at workbench root checks modal state.
2. Non-modal target paths are remapped to modal root.
3. Events outside modal subtree are consumed.
4. `Escape` always resolves to modal `Dismiss` if available.
5. Focus restoration after dismiss returns to previous non-modal focus owner.

Failure to restore focus emits `ux:contract_warning` (Warn). Path resolution failure during modal remapping emits `ux:navigation_violation` (Error) and aborts dispatch.

---

## 5. Authority Routing

After dispatch, resulting actions must route by authority:

- **Graph authority**: graph truth mutations (`GraphIntent` data/state mutations)
- **Workbench authority**: tile/frame/arrangement mutations

Dispatch layer is not allowed to mutate graph or workbench state directly.

Routing invariant:

- every emitted mutation intent includes an authority destination,
- misrouted intent emits `ux:contract_warning` + `registry:action:execute_failed` context.

Command-surface provenance extension:

- dispatches originating from `CommandBar`, command palette, or omnibar submit must record
	their resolution provenance before default action executes,
- provenance payload must include command-surface kind, resolution source,
	resolved target identity or explicit blocked/fallback/no-target reason, and
	any relevant session/request identity for omnibar/provider flows,
- command-surface dispatch must not re-derive target ownership later in widget-local code.

---

## 6. Diagnostics Requirements

Required diagnostic channels for dispatch observability:

- `ux:dispatch_started` (Info)
- `ux:dispatch_phase` (Info; phase + node path)
- `ux:dispatch_consumed` (Info; reason: stopped/immediate/modal)
- `ux:dispatch_default_prevented` (Info)
- `ux:navigation_violation` (Error)

Minimum payload fields:

- event kind,
- command-surface kind when applicable,
- target node id,
- resolution source and resolved-target / fallback / no-target reason when applicable,
- path length,
- phase,
- timestamp/frame index,
- propagation/default flags.

---

## 7. UxHarness Requirements

Core scenarios must assert:

1. Capture-before-target-before-bubble ordering.
2. Modal isolation consumes non-modal actions.
3. `preventDefault` blocks default action only.
4. `Escape` dismisses modal and restores prior focus.
5. Action authority routing maps to expected owner.
6. Command-surface dispatch preserves explicit target-resolution provenance.
7. Omnibar capture exit and command-palette dismiss emit fallback evidence when the stored return target is no longer valid.

---

## 8. Acceptance Criteria

- [ ] Dispatch algorithm implemented with deterministic phase order.
- [ ] Modal isolation contract is enforced for all modal surfaces.
- [ ] Diagnostics channels listed in §6 are emitted in scenario runs.
- [ ] UxScenario tests cover all §7 cases.
- [ ] No direct state mutation occurs inside dispatch layer.
