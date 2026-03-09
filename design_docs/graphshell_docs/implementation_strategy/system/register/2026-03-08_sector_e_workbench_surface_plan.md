<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Sector E — Workbench Surface Registry Development Plan

**Doc role:** Implementation plan for the workbench surface registry sector
**Status:** Active / planning
**Date:** 2026-03-08
**Parent:** [2026-03-08_registry_development_plan.md](2026-03-08_registry_development_plan.md)
**Registries covered:** `WorkbenchSurfaceRegistry`, `WorkflowRegistry`
**Specs:** [workbench_surface_registry_spec.md](workbench_surface_registry_spec.md), [workflow_registry_spec.md](workflow_registry_spec.md)
**Also depends on:** `SYSTEM_REGISTER.md` (workbench mutation authority), Sector A (`ViewerSurfaceRegistry`), Sector D (`LayoutDomainRegistry`)

---

## Purpose

The workbench surface is the tile-tree arrangement that hosts pane content. SYSTEM_REGISTER
documents it as one of the two mutation authorities: the **Workbench Authority** owns tile-tree
shape. `WorkbenchSurfaceRegistry` is the registry surface of that authority — it resolves
layout policy and interaction policy for pane hosts, and provides the canonical extension
point for per-surface customisation.

`WorkflowRegistry` is the session-mode compositor that combines `LensProfile` × `WorkbenchProfile`
into a named workflow (e.g. "Research mode", "Reading mode"). It depends on `WorkbenchSurfaceRegistry`
being real.

Neither registry exists yet.

---

## Current State

| Registry | Struct | Key gaps |
|---|---|---|
| `WorkbenchSurfaceRegistry` | ❌ | No struct; tile layout policy is inline in `tile_behavior.rs` and `ux_tree.rs` |
| `WorkflowRegistry` | ❌ | No struct; no session mode composition |

Workbench layout decisions currently live as:
- Magic constants in `tile_behavior.rs` (split ratios, pane size limits).
- Hardcoded tab/pane logic in `workbench/ux_tree.rs`.
- Panel visibility booleans (legacy, partially removed).

---

## Phase E1 — WorkbenchSurfaceRegistry: Tile-tree layout policy authority

**Unlocks:** Two-authority model enforcement; tile layout configurable by mods; workbench parity
with canvas customisation.

### E1.1 — Define `WorkbenchLayoutPolicy` and `WorkbenchInteractionPolicy`

The `workbench_surface_registry_spec.md` separates layout policy (how tiles are arranged) from
interaction policy (how users operate the workbench surface):

```rust
pub struct WorkbenchLayoutPolicy {
    pub default_split_direction: SplitDirection,  // Horizontal | Vertical
    pub min_pane_size: Vec2,
    pub tab_strip_visible: bool,
    pub tab_strip_position: TabStripPosition,    // Top | Bottom
    pub resize_handles_visible: bool,
    pub initial_layout: InitialLayout,           // Single | TwoPane | Grid
}

pub struct WorkbenchInteractionPolicy {
    pub drag_to_split: bool,
    pub double_click_to_expand: bool,
    pub keyboard_focus_cycle: FocusCycle,        // Tabs | Panes | Both
    pub close_empty_panes: bool,
}

pub struct WorkbenchSurfaceProfile {
    pub id: WorkbenchSurfaceProfileId,
    pub display_name: String,
    pub layout: WorkbenchLayoutPolicy,
    pub interaction: WorkbenchInteractionPolicy,
}

pub struct WorkbenchSurfaceRegistry {
    profiles: HashMap<WorkbenchSurfaceProfileId, WorkbenchSurfaceProfile>,
    active: WorkbenchSurfaceProfileId,
}
```

Built-in profiles:
- `WORKBENCH_PROFILE_DEFAULT` — current behaviour reconstructed as a named profile.
- `WORKBENCH_PROFILE_FOCUS` — single-pane, no split, tabs visible.
- `WORKBENCH_PROFILE_COMPARE` — two-pane horizontal, equal split, both titles visible.

**Done gates:**
- [ ] `WorkbenchSurfaceRegistry` struct in `shell/desktop/runtime/registries/workbench_surface.rs`.
- [ ] `DEFAULT`, `FOCUS`, `COMPARE` profiles registered.
- [ ] `resolve_layout_policy()`, `resolve_interaction_policy()`, `describe_surface()` implemented.
- [ ] Added to `RegistryRuntime`.
- [ ] `DIAG_WORKBENCH_SURFACE` channel (Info severity).
- [ ] Unit tests: profile lookup, fallback to default.

### E1.2 — Migrate `tile_behavior.rs` constants into the registry

All magic constants in `tile_behavior.rs` and `ux_tree.rs` for split ratios, pane minimums, and
tab strip configuration are replaced with calls to the active `WorkbenchSurfaceProfile`.

The `tile-tree-authority` policy from the spec: tile-tree shape mutations are initiated by
workbench intent, resolved by this registry, and never intercepted by graph-reducer paths.

**Done gates:**
- [ ] No magic layout constants in `tile_behavior.rs` or `ux_tree.rs`.
- [ ] `tile_behavior.rs` calls `registries.workbench_surface.resolve_layout_policy()`.
- [ ] Regression test: default profile reproduces current visual layout behaviour.

### E1.3 — Focus handoff policy

The `focus-handoff` policy: focus transfer between graph canvas and pane content is explicit and
policy-driven, not implicit.

```rust
pub struct FocusHandoffPolicy {
    pub canvas_to_pane_trigger: FocusTrigger,   // Click | KeyboardNav | Auto
    pub pane_to_canvas_trigger: FocusTrigger,
    pub focus_ring: FocusRingSpec,
}
```

Wire into the existing focus-activation path in `tile_behavior.rs`.

**Done gates:**
- [ ] `FocusHandoffPolicy` field on `WorkbenchSurfaceProfile`.
- [ ] Focus activation path reads from `WorkbenchSurfaceRegistry`.
- [ ] Stabilization bug: "focus activation" (PLANNING_REGISTER §1A bug register) addressed here.

### E1.4 — Locking constraint

The spec's `locking-constraint` policy: layout mutations can be locked to prevent accidental
splits/closes (e.g. in kiosk or presentation modes).

```rust
pub enum WorkbenchLock {
    None,
    PreventSplit,
    PreventClose,
    FullLock,
}
```

`WorkbenchSurfaceRegistry::can_mutate(lock: WorkbenchLock, intent: &WorkbenchIntent) -> bool`
guards all tile-tree mutations.

**Done gates:**
- [ ] `WorkbenchLock` enum defined and checked in workbench intent handlers.
- [ ] `FullLock` prevents all tile-tree mutations; useful for presentation mode.

---

## Phase E2 — WorkflowRegistry: Session mode composition

**Unlocks:** Named "modes" the user can switch between; Lens × Workbench profile binding;
PLANNING_REGISTER §1D prospective `history-stage-f` and `presence-collaboration` modes.

The `workflow_registry_spec.md`'s `composed-workflow` policy: a workflow is a `LensProfile ×
WorkbenchProfile` composition, not a monolithic session state object.

### E2.1 — Define `WorkflowDescriptor` and `WorkflowRegistry`

```rust
pub struct WorkflowDescriptor {
    pub id: WorkflowId,
    pub display_name: String,
    pub lens_profile: LensProfileRef,                   // from LensRegistry (Sector A)
    pub workbench_profile: WorkbenchSurfaceProfileId,   // from WorkbenchSurfaceRegistry
    pub canvas_profile: CanvasProfileId,                // from CanvasRegistry (Sector D)
    pub physics_profile: PhysicsProfileId,              // from PhysicsProfileRegistry (Sector D)
}

pub struct WorkflowRegistry {
    workflows: HashMap<WorkflowId, WorkflowDescriptor>,
    active: Option<WorkflowId>,
}
```

Built-in workflows:
- `workflow:default` — standard browsing (current default behaviour).
- `workflow:research` — two-pane compare + semantic lens overlay + gas physics.
- `workflow:reading` — single-pane focus + identity lens + solid physics (stable graph).

**Done gates:**
- [ ] `WorkflowRegistry` struct in `shell/desktop/runtime/registries/workflow.rs`.
- [ ] `DEFAULT`, `RESEARCH`, `READING` built-in workflows.
- [ ] `activate_workflow(id)` resolves and applies all constituent profiles atomically.
- [ ] Added to `RegistryRuntime`.
- [ ] `DIAG_WORKFLOW` channel (Info severity) emits on activation.

### E2.2 — `activate_workflow()` — atomic profile activation

Activating a workflow applies all constituent profiles in the correct order:

1. `CanvasRegistry::set_active_profile(descriptor.canvas_profile)`.
2. `PhysicsProfileRegistry::set_active_profile(descriptor.physics_profile)`.
3. `WorkbenchSurfaceRegistry::set_active_profile(descriptor.workbench_profile)`.
4. `LensRegistry::set_active_lens(descriptor.lens_profile)`.
5. Emit `GraphIntent::WorkflowActivated { workflow_id }` through the reducer for WAL logging.
6. Emit `SignalKind::Lifecycle(WorkflowChanged)` via `SignalRoutingLayer`.

The `deterministic-activation` policy: workflow activation must be transactional — either all
profiles update or none do (rollback on partial failure).

**Done gates:**
- [ ] `activate_workflow()` applies all profiles in sequence.
- [ ] Partial failure rolls back to previous profiles.
- [ ] `GraphIntent::WorkflowActivated` is WAL-logged.
- [ ] Scenario test: switch workflow → graph, workbench, and lens all update.

### E2.3 — Workflow persistence

Active workflow ID persists to workspace state so it is restored on restart.

**Done gates:**
- [ ] Active workflow ID serialised into workspace save.
- [ ] On restore, `WorkflowRegistry::activate_workflow()` is called with the saved ID.
- [ ] Unknown workflow ID on restore falls back to `workflow:default` with a `Warn` diagnostic.

### E2.4 — Prospective: history mode and presence mode (deferred)

Per PLANNING_REGISTER §1D:
- `workflow:history` (Stage F) — temporal lens + graph frozen layout + timeline pane.
- `workflow:presence` (Collaborative) — Coop-follow lens + shared viewport + presence overlays.

These are registered as stub workflows (display_name set, no implementation) to hold the
namespace while the feature lanes develop.

**Done gates (deferred):**
- [ ] `workflow:history` and `workflow:presence` stub descriptors registered.
- [ ] `activate_workflow()` for stubs returns `ActionOutcome::Failure` with "not yet implemented".

---

## Acceptance Criteria (Sector E complete)

- [ ] `WorkbenchSurfaceRegistry` resolves all tile layout and interaction policy; no magic
  constants in `tile_behavior.rs` or `ux_tree.rs`.
- [ ] The two-authority model is enforced: workbench intent → `WorkbenchSurfaceRegistry` →
  tile-tree mutation; graph reducer never touches tile-tree directly.
- [ ] Focus handoff policy is explicit and configurable.
- [ ] `WorkflowRegistry` activates named workflows; `DEFAULT`, `RESEARCH`, `READING` work.
- [ ] Workflow activation is atomic; partial failures roll back.
- [ ] Active workflow persists and restores across app restart.
- [ ] `DIAG_WORKBENCH_SURFACE` and `DIAG_WORKFLOW` emit with correct severity.

---

## Related Documents

- [workbench_surface_registry_spec.md](workbench_surface_registry_spec.md)
- [workflow_registry_spec.md](workflow_registry_spec.md)
- [SYSTEM_REGISTER.md](SYSTEM_REGISTER.md) — workbench mutation authority
- [2026-03-08_sector_a_content_pipeline_plan.md](2026-03-08_sector_a_content_pipeline_plan.md) — ViewerSurfaceRegistry (for LayoutDomain)
- [2026-03-08_sector_d_canvas_surface_plan.md](2026-03-08_sector_d_canvas_surface_plan.md) — CanvasRegistry + LayoutDomainRegistry
- [2026-03-08_registry_development_plan.md](2026-03-08_registry_development_plan.md) — master index
