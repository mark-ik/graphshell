<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Sector E — Workbench Surface Registry Development Plan

**Doc role:** Implementation plan for the workbench surface registry sector
**Status:** Active / implemented with follow-on notes
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

**Implementation note (2026-03-10, completion update):**
- `WorkbenchSurfaceRegistry` now exists in
  `shell/desktop/runtime/registries/workbench_surface.rs` and is the concrete
  workbench mutation authority object for split/close/open/focus policy.
- `gui_orchestration.rs` is now a thin adapter into that registry rather than
  the de facto authority body.
- `WorkflowRegistry` now exists in
  `shell/desktop/runtime/registries/workflow.rs` with built-in
  `workflow:default`, `workflow:research`, `workflow:reading`, plus stub
  `workflow:history` and `workflow:presence`.
- Workflow activation is real over the state that exists today:
  workbench surface active profile, persisted canvas profile id, and persisted
  lens/physics/theme defaults.
- Full transactional rollback across stateful canvas/physics authorities is
  still blocked on Sector D because those registries are not yet runtime-owned
  active profile authorities in code.

**Implementation note (2026-03-10):**
- B3.4 groundwork landed before Sector E: reducer ingress now has an explicit
  warning/classification seam for graph-carrier intents that are really
  workbench-authority bridges.
- This should be treated as a deliberate intermediate architecture state:
  the authority contract is now explicit, but the concrete authority object is
  still the queue + `gui_orchestration.rs` dispatch path rather than a real
  `WorkbenchSurfaceRegistry`.
- Sector E should absorb that seam rather than bypass it. The eventual registry
  implementation should replace ad hoc dispatch internals without changing the
  boundary contract:
  `bridge intent -> workbench authority -> tile-tree mutation`.

---

## Current State

| Registry | Struct | Key gaps |
|---|---|---|
| `WorkbenchSurfaceRegistry` | ✅ | Remaining cleanup is policy migration/compression, not authority existence |
| `WorkflowRegistry` | ✅ | Remaining gap is fully transactional activation across future Sector D runtime authorities |

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
- [x] `WorkbenchSurfaceRegistry` struct in `shell/desktop/runtime/registries/workbench_surface.rs`.
- [x] `DEFAULT`, `FOCUS`, `COMPARE` profiles registered.
- [x] `resolve_layout_policy()`, `resolve_interaction_policy()`, `describe_surface()` implemented.
- [x] Added to `RegistryRuntime`.
- [x] `DIAG_WORKBENCH_SURFACE` channel (Info severity).
- [x] Unit tests: profile lookup, fallback to default.

### E1.2 — Migrate `tile_behavior.rs` constants into the registry

All magic constants in `tile_behavior.rs` and `ux_tree.rs` for split ratios, pane minimums, and
tab strip configuration are replaced with calls to the active `WorkbenchSurfaceProfile`.

The `tile-tree-authority` policy from the spec: tile-tree shape mutations are initiated by
workbench intent, resolved by this registry, and never intercepted by graph-reducer paths.

Interim boundary rule until this phase lands:
- Graph-reducer ingress may still receive graph-carrier bridge intents that are
  destined for workbench authority. Those paths must warn and enqueue/delegate;
  they must not be treated as permission for reducer-owned tile-tree mutation.

**Done gates:**
- [ ] No magic layout constants in `tile_behavior.rs` or `ux_tree.rs`.
- [x] `tile_behavior.rs` calls `registries.workbench_surface.resolve_layout_policy()`.
- [ ] Regression test: default profile reproduces current visual layout behaviour.
- [x] Existing reducer-side bridge classification/warn logic is deleted or
  reduced to a thin adapter because `WorkbenchSurfaceRegistry` has become the
  real authority object rather than a future placeholder.

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
- [x] `FocusHandoffPolicy` field on `WorkbenchSurfaceProfile`.
- [x] Focus activation path reads from `WorkbenchSurfaceRegistry`.
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
- [x] `WorkbenchLock` enum defined and checked in workbench intent handlers.
- [x] `FullLock` prevents all tile-tree mutations; useful for presentation mode.

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
- [x] `WorkflowRegistry` struct in `shell/desktop/runtime/registries/workflow.rs`.
- [x] `DEFAULT`, `RESEARCH`, `READING` built-in workflows.
- [x] `activate_workflow(id)` resolves and applies all constituent profiles over the currently implemented runtime/persistence carriers.
- [x] Added to `RegistryRuntime`.
- [x] `DIAG_WORKFLOW` channel (Info severity) emits on activation.

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

**Implementation note (2026-03-10):**
- The current code can apply workflow activation deterministically because the
  active workbench profile and persisted defaults are infallible writes in the
  current carrier model.
- The original stronger rollback requirement still depends on Sector D making
  canvas/physics active profiles runtime-stateful authorities rather than
  persisted defaults.

**Done gates:**
- [x] `activate_workflow()` applies all profiles in sequence over the current runtime/persistence carriers.
- [ ] Partial failure rolls back to previous profiles.
- [ ] `GraphIntent::WorkflowActivated` is WAL-logged.
- [x] Scenario-level runtime tests cover workflow activation updating workbench profile and registry defaults.

### E2.3 — Workflow persistence

Active workflow ID persists to workspace state so it is restored on restart.

**Done gates:**
- [x] Active workflow ID serialised into workspace save.
- [x] On restore, `WorkflowRegistry::activate_workflow()` is called with the saved ID.
- [ ] Unknown workflow ID on restore falls back to `workflow:default` with a `Warn` diagnostic.

### E2.4 — Prospective: history mode and presence mode (deferred)

Per PLANNING_REGISTER §1D:
- `workflow:history` (Stage F) — temporal lens + graph frozen layout + timeline pane.
- `workflow:presence` (Collaborative) — Coop-follow lens + shared viewport + presence overlays.

These are registered as stub workflows (display_name set, no implementation) to hold the
namespace while the feature lanes develop.

**Done gates (deferred):**
- [x] `workflow:history` and `workflow:presence` stub descriptors registered.
- [x] `activate_workflow()` for stubs returns `ActionOutcome::Failure` with "not yet implemented".

---

## Acceptance Criteria (Sector E implemented state)

- [x] `WorkbenchSurfaceRegistry` is the real workbench authority object for tile-tree mutation.
- [x] The two-authority model is enforced as `workbench intent -> WorkbenchSurfaceRegistry ->
  tile-tree mutation`; reducer bridge paths are thin adapters/warnings rather than alternate authorities.
- [x] Focus handoff policy is explicit and configurable.
- [x] `WorkflowRegistry` activates named workflows; `DEFAULT`, `RESEARCH`, `READING` work.
- [x] Active workflow persists and restores across app restart.
- [x] `DIAG_WORKBENCH_SURFACE` and `DIAG_WORKFLOW` emit with correct severity.

Follow-on acceptance still pending outside Sector E proper:
- [ ] Full rollback semantics across stateful canvas/physics authorities (Sector D dependency).
- [ ] Full WAL-carried workflow activation logging.
- [ ] Final cleanup of any remaining policy/magic constants outside the active workbench registry path.

---

## Related Documents

- [workbench_surface_registry_spec.md](workbench_surface_registry_spec.md)
- [workflow_registry_spec.md](workflow_registry_spec.md)
- [SYSTEM_REGISTER.md](SYSTEM_REGISTER.md) — workbench mutation authority
- [2026-03-08_sector_a_content_pipeline_plan.md](2026-03-08_sector_a_content_pipeline_plan.md) — ViewerSurfaceRegistry (for LayoutDomain)
- [2026-03-08_sector_d_canvas_surface_plan.md](2026-03-08_sector_d_canvas_surface_plan.md) — CanvasRegistry + LayoutDomainRegistry
- [2026-03-08_registry_development_plan.md](2026-03-08_registry_development_plan.md) — master index
