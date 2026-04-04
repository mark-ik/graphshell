<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Physics Preferences Surface Plan (2026-04-03)

**Status**: Active follow-on plan
**Scope**: Extracts the user-configuration lane from `2026-02-24_physics_engine_extensibility_plan.md` into an execution plan for the page-backed physics settings surface, scope-aware control ownership, preset portability, and advanced physics overrides.
**Related**:

- `2026-02-24_physics_engine_extensibility_plan.md`
- `2026-04-03_physics_region_plan.md`
- `layout_behaviors_and_physics_spec.md`
- `multi_view_pane_spec.md`
- `../aspect_control/settings_and_control_surfaces_spec.md`
- `../system/register/canvas_registry_spec.md`
- `2026-04-03_semantic_clustering_follow_on_plan.md`
- `2026-04-03_layout_transition_and_history_plan.md`

---

## Context

The umbrella physics note accumulated a real product need inside a brainstorming section: the app
already has a named command route for Physics Settings, existing profile-level toggles, and a
growing set of graph-view-local behaviors, but no single plan says how those controls should be
organized, scoped, staged, or kept consistent with graph/view/registry ownership.

That absence creates two risks:

- the settings page turns into an ad hoc pile of sliders that duplicates registry semantics
- advanced controls such as per-node overrides, region visibility, and preset import/export land
  without a clear scope model or persistence boundary

This plan exists to define the control surface as a Graph-owned semantics consumer hosted through
the canonical settings surface model, not as a second authority for physics behavior.

---

## Non-Goals

- inventing a generic plugin marketplace UI before Graphshell has a real force-registry contract
- replacing `CanvasRegistry`, `PhysicsProfileRegistry`, or `GraphViewId` as the owners of runtime
  state
- turning speculative controls into day-one implementation commitments
- using the settings page to smuggle in unresolved rapier, 3D, or ML contracts

---

## Feature Target 1: Define The Physics Settings Information Architecture

### Target 1 Context

`Settings and Control Surfaces Spec` already says settings are page-backed routes, and the command
surface already points to `verso://settings/physics`. What is missing is the page model for that
route.

### Target 1 Tasks

1. Define `verso://settings/physics` as a page-backed control surface rather than a floating
   physics-only panel.
2. Split the page into clear sections such as profile selection, behavior toggles, graph-view
   controls, region controls, and advanced/export controls.
3. Label every control by scope: user preference, workspace preference, `GraphViewId`, persisted
   preset asset, or per-node metadata.
4. Keep apply timing explicit per section: immediate, staged-until-confirm, or deferred because the
   underlying feature is not yet landed.

### Target 1 Validation Tests

- A user can tell whether a control applies to all views, the current graph view, or persisted
  preset data.
- The same `verso://settings/physics` route can open as an overlay or a workbench-hosted pane
  without changing semantics.
- Unimplemented sections render as explicit deferred states rather than silently disappearing.

---

## Feature Target 2: Land The First-Slice Controls Around Existing Authorities

### Target 2 Context

Current production behavior is still coarse-grained: active preset choice, profile parameters, and
named extension toggles. The settings page should expose that real state first instead of faking a
fully dynamic per-force UI.

### Target 2 Tasks

1. Surface active physics profile resolution and profile metadata without bypassing
   `PhysicsProfileRegistry`.
2. Expose the currently real control set first: profile-backed tuning values, coarse extension
   toggles, lens-physics binding preferences, progressive auto-switch preference, and convergence
   timeout / auto-pause policy.
3. Keep force-specific controls mapped to explicit `PhysicsProfile` or `CanvasRegistry` fields
   until a richer force-registry contract actually exists.
4. Make current vs suggested vs unavailable settings visually distinct so the page does not imply
   that speculative lanes are implemented already.

### Target 2 Validation Tests

- The page can represent the currently landed helper-era physics controls without inventing hidden
  runtime state.
- Lens binding controls align with `layout_behaviors_and_physics_spec.md §§5–6` rather than
  restating different semantics.
- Changing a view-local control affects only the targeted `GraphViewId`.

---

## Feature Target 3: Define Preset Portability And Recommendation Flows

### Target 3 Context

Several brainstormed controls are really profile and preference flows, not render-loop behavior:
import/export, platform defaults, and suggested Z-source pairing.

### Target 3 Tasks

1. Define preset export/import as serialization of `PhysicsProfile` data, not a parallel custom
   settings format.
2. Keep imported presets compatible with registry seed entries so they can later be promoted into
   mod-registered content.
3. Define how platform or device-class optimization preferences interact with explicit user preset
   selection.
4. Treat `suggested_z_source` as a recommendation flow surfaced when a preset is activated in a
   compatible view, not as a hidden automatic override.

### Target 3 Validation Tests

- Exported presets can round-trip without losing named profile semantics.
- Imported presets fail with explicit diagnostics when required fields are missing or unsupported.
- A suggested Z-source can be accepted or ignored without mutating the preset unexpectedly.

---

## Feature Target 4: Define Advanced Override And Region Controls Without Breaking Ownership

### Target 4 Context

The remaining brainstormed controls are advanced because they reach into node metadata or
graph-view-local runtime state. They need explicit ownership boundaries before any UI work starts.

### Target 4 Tasks

1. Treat per-node physics overrides as an advanced lane with an explicit metadata carrier and
   snapshot roundtrip contract, not as anonymous values stored in widget state.
2. Define region visibility, region scope, and region persistence controls against the canonical
   `GraphViewId` and snapshot boundaries rather than reviving the old zone model. Region authority
   should be consumed from `2026-04-03_physics_region_plan.md`.
3. Require all graph-view-local controls to respect `multi_view_pane_spec.md` per-view isolation.
4. If `PhysicsRegion` returns, persist explicit `physics_regions` records rather than relying on a
   legacy `zones` abstraction.

### Target 4 Validation Tests

- Per-node overrides survive snapshot roundtrip only when their storage contract is defined.
- Toggling physics-region visibility never changes whether a region affects simulation.
- A view-local region or override change in pane A does not leak into pane B by default.

---

## Feature Target 5: Triage Cross-Plan Dependencies Instead Of Reopening Them In UI

### Target 5 Context

Some items from the umbrella note are not settings-page design work at all. They are dependency
notes for other lanes and should be recorded as such so the settings plan does not become a catch-all.

### Target 5 Tasks

1. Treat `CanvasRegistry` field authority as already owned by `canvas_registry_spec.md`; this plan
   only consumes those fields.
2. Treat `GraphViewState.dimension` placement as already owned by `multi_view_pane_spec.md`.
3. Keep rapier world ownership out of scope here; any future rapier lane must define whether local
   simulation state owns both positions and physics world state.
4. Keep the semantic-force carrier aligned with `2026-04-03_semantic_clustering_follow_on_plan.md`
   and the Verse local-intelligence research, rather than inventing an ad hoc page-local model.

### Target 5 Validation Tests

- The settings plan does not redefine registry, graph-view, or semantic-clustering ownership.
- Dependency notes point to a concrete canonical home or follow-on plan.
- The page can explicitly mark controls as blocked on another lane instead of hand-waving them into
  existence.

---

## Exit Condition

This plan is complete when Graphshell has a documented `verso://settings/physics` surface with a
clear scope model, first-slice controls aligned to existing authorities, explicit staging for
advanced overrides and preset portability, and a dependency map that keeps unresolved rapier and
semantic-force work out of the settings page until their owning lanes land.
