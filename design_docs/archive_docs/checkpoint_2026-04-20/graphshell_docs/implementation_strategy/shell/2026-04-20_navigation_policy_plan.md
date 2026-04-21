<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Navigation Policy Plan (2026-04-20)

**Status**: **Archived 2026-04-20** — landed end-to-end including
the settings UI and zombie-prefs removal follow-on.
**Scope**: Lift every host-side hardcoded navigation constant (zoom
bounds, fit padding, pan-inertia damping, scroll rate, drag threshold,
lasso-gating modifier) into a first-class user-tunable
`NavigationPolicy` with per-view overrides and per-graph defaults, so
Graphshell's canvas feel is configurable rather than opinionated. The
policy lives in `graph-canvas` so the future iced host reads the same
values as the current egui host without a parallel constants table.

**Related**:

- [2026-04-19_graph_canvas_overlays_and_camera_relands_plan.md](2026-04-19_graph_canvas_overlays_and_camera_relands_plan.md) — where the hardcoded constants landed that this plan is lifting.
- [../graph/2026-04-19_step5_spatial_pattern_layouts_plan.md](../graph/2026-04-19_step5_spatial_pattern_layouts_plan.md) — precedent for retroactive configurability (Batches A–D exposed every discretionary layout choice as a knob).
- [../../../memory/feedback_configurability_over_opinionated_defaults.md](../../../../../.claude/projects/c--Users-mark--Code/memory/feedback_configurability_over_opinionated_defaults.md) — the user-pinned directive driving this pass.
- [2026-04-14_iced_host_migration_execution_plan.md](2026-04-14_iced_host_migration_execution_plan.md) — iced bring-up that benefits directly.

---

## 1. Framing

The earlier overlays+camera re-lands plan landed navigation-defaults
(wheel=pan, Ctrl+wheel=zoom, middle-click=pan, pan inertia) and Fit
commands, but left the tuning knobs as hardcoded constants in
`render/canvas_bridge.rs`:

- `FIT_ZOOM_MIN = 0.1`, `FIT_ZOOM_MAX = 10.0` — duplicated between
  `apply_fit_camera_command` and `apply_zoom`.
- `FIT_FALLBACK_ZOOM = 1.0`, `FIT_PADDING_RATIO = 1.08`.
- `DEFAULT_PAN_DAMPING_PER_SECOND = 0.003` (imported from graph-canvas
  but used as a constant; no user tuning).
- `scroll_pan_pixels_per_unit = 50.0`, `scroll_zoom_factor = 0.1`,
  `drag_threshold_px = 6.0` — defaults baked into
  `InteractionConfig::default()` with no host-level override path.
- Lasso modifier hardcoded to `Shift` in the engine.

Per the configurability directive (the layouts lesson), every
discretionary choice should be a user knob. This plan is the
retroactive sweep for the navigation surface.

User direction (2026-04-20): "per view navigation policy with per
graph policy defaults, as long as iced benefits." → all policy types
live in `graph-canvas` (portable), per-view override + per-graph
default sit on app-side durable state, and the resolver is a
`GraphBrowserApp` method that both hosts call.

## 2. Design

### 2.1 `NavigationPolicy` type

Lives at `crates/graph-canvas/src/navigation.rs`. Flat serde struct of
primitives plus one `LassoModifier` enum. Every field has an exposed
constant for the baseline value, so callers that want to compare to
"the default" don't pattern-match against magic numbers.

Fields:

- `zoom_min: f32`, `zoom_max: f32` — clamp applied by both
  `apply_zoom` and `fit_to_bounds`. Deduplicated compared to the
  pre-policy code, which carried two separate clamps.
- `fit_padding_ratio: f32` — ratio ≥ 1.0 around fit bounds.
- `fit_fallback_zoom: f32` — zoom used when the fit bounds collapse
  to a point.
- `pan_damping_per_second: f32` — inertia decay.
- `scroll_pan_pixels_per_unit: f32`, `scroll_zoom_factor: f32` —
  scroll tuning.
- `drag_threshold_px: f32` — click-vs-drag disambiguation.
- `pan_inertia_enabled`, `lasso_enabled`, `node_drag_enabled` —
  feature toggles.
- `lasso_modifier: LassoModifier` — enum `{ Shift, Ctrl, Alt, None }`.
  `None` is the Figma/Sketch convention: primary-drag always lassoes.

`NavigationPolicy::to_interaction_config()` projects the input-relevant
subset into an `InteractionConfig`, so the engine keeps a narrow
config surface but stays in sync with any policy change.

### 2.2 Per-view override, per-graph default

- `GraphViewState.navigation_policy_override: Option<NavigationPolicy>`
  (new). `#[serde(default)]` for backwards-compatible deserialize of
  older view snapshots; serde Deserialize helper, Clone impl, Debug
  impl, `new_with_id` constructor all carry the field.
- `DomainState.navigation_policy_default: NavigationPolicy` (new).
  Non-optional — every `DomainState` has a baseline. Both
  `GraphBrowserApp::new_from_dir` and `new_for_testing` initialize it
  to `NavigationPolicy::default()`.

### 2.3 Resolver

Single entry point on `GraphBrowserApp`:

```rust
pub fn resolve_navigation_policy(&self, view_id: GraphViewId) -> NavigationPolicy
```

Precedence: view override → graph default → `NavigationPolicy::default()`
baseline (only if the view is missing entirely; otherwise the graph
default backs every view). Plus two setters —
`set_graph_view_navigation_policy_override(...)` and
`set_navigation_policy_default(...)` — so the settings surface (a
future plan) writes without reaching into private fields.

### 2.4 Host wiring

`render/canvas_bridge.rs::run_graph_canvas_frame` resolves the policy
once per frame, before the engine is refreshed. From there the policy
threads into:

- The engine's `InteractionConfig` via
  `navigation_policy.to_interaction_config()`, refreshed every frame
  so user tuning takes effect immediately without engine rebuild.
- `camera.tick_inertia(1.0 / 60.0, navigation_policy.pan_damping_per_second)`.
- `apply_zoom(..., &navigation_policy)` — the clamp now lives in
  `NavigationPolicy::clamp_zoom` and is shared with fit.
- `apply_fit_camera_command(..., &navigation_policy)` — zoom bounds,
  padding, fallback zoom all flow from the policy.

The hardcoded `FIT_*` and `DEFAULT_PAN_DAMPING_PER_SECOND` constants
in `canvas_bridge.rs` are removed; the section header was kept with a
pointer comment back to this plan.

### 2.5 Engine-side modifier routing

`InteractionConfig` gained a `lasso_modifier: LassoModifier` field
(defaulting to `Shift`) and a helper
`press_should_lasso(modifiers: Modifiers) -> bool` that the engine
uses instead of the old inline `modifiers.shift && lasso_enabled`
check. This keeps modifier-routing logic in one place and makes it
obvious how `None` (always-lasso) works.

## 3. Iced benefit

Iced will pick up the exact same NavigationPolicy via the
host-neutral resolver. No parallel constants table, no per-host
divergence in zoom bounds or scroll-pan rate. When the future iced
compositor calls `run_graph_canvas_frame` (or its iced equivalent),
the same `resolve_navigation_policy(view_id)` call returns the
resolved policy and threads into the iced-side `InteractionEngine`
and painter identically.

## 4. What was NOT included

Intentionally out of scope for this pass:

- **Settings UI / preferences page** to tune the policy interactively.
  The durable state and accessors are in place; the presentation
  layer is a separate plan (probably the graph-canvas input/
  accessibility follow-on).
- **Migration of per-view persisted policy**. There is no persisted
  policy to migrate yet; new views serialize with `None` override,
  old snapshots deserialize with the default via `#[serde(default)]`.
- **Per-edge / per-node overrides**. Navigation feel is per-view,
  not per-element.
- **Non-navigation policies** (physics profile, layout policy, etc.)
  already have their own per-view/per-graph surfaces and are unchanged.

## 5. Receipts

- `cargo test -p graph-canvas --lib navigation:: engine::` — 25 pass
  (4 new NavigationPolicy tests, 3 new engine tests covering Ctrl,
  None, and Shift-when-policy-is-Ctrl variants).
- `cargo test -p graph-canvas --features simulate --lib` — 255/255
  pass (was 248 before this plan; +7 navigation tests).
- `cargo test -p graphshell --lib` — 2152/2152 pass (was 2149 before
  this plan; +3 canvas_bridge resolver tests:
  `resolve_navigation_policy_falls_back_to_graph_default`,
  `resolve_navigation_policy_prefers_view_override_over_graph_default`,
  `run_graph_canvas_frame_honors_per_view_zoom_clamp`).
- `cargo check -p graphshell --lib` clean.

## 6. Progress

### 2026-04-20

- Plan landed end-to-end in one session following the /loop cadence
  that shipped §4 host wiring. `NavigationPolicy` in graph-canvas,
  per-view + per-graph storage, resolver on `GraphBrowserApp`,
  `canvas_bridge` switched from hardcoded constants to resolved
  policy, engine modifier-routing consolidated. All receipts above
  green.
- Follow-on plan referenced in the overlays+camera re-lands doc is
  settled: §2.1 was parent; this plan closes the configurability
  gap that followed the initial Miro-style landing.

### 2026-04-20 (zombie-prefs removal + settings UI)

While scoping the settings UI for NavigationPolicy I found a parallel
config system that needed reconciling: `workspace.chrome_ui.camera_pan_inertia_enabled`
and `camera_pan_inertia_damping` existed as workspace-global fields,
persisted to disk and displayed in the Physics settings page, but
**never consumed** by the actual inertia tick — `canvas_bridge`'s
`tick_inertia` call pulled from `NavigationPolicy::pan_damping_per_second`
directly and the old fields were orphaned.

Picked option C from the three reconciliation options (rip out the
zombies; no shim needed because they added no unique value):

- **Deleted fields** from [app/workspace_state.rs](../../../../app/workspace_state.rs):
  `camera_pan_inertia_enabled: bool`, `camera_pan_inertia_damping: f32`.
- **Deleted accessors** from [app/settings_persistence.rs](../../../../app/settings_persistence.rs):
  `camera_pan_inertia_enabled`, `set_camera_pan_inertia_enabled`,
  `camera_pan_inertia_damping`, `set_camera_pan_inertia_damping`, and
  the `save_camera_pan_inertia_*` helpers.
- **Deleted persistence keys**: `SETTINGS_CAMERA_PAN_INERTIA_ENABLED_NAME`
  and `SETTINGS_CAMERA_PAN_INERTIA_DAMPING_NAME` from `graph_app.rs`,
  plus the `is_reserved_workspace_layout_name` entries and the load-
  path blocks in `settings_persistence.rs`. Legacy workspace JSON
  carrying the old keys is silently ignored on load.
- **Deleted defaults**: `DEFAULT_CAMERA_PAN_INERTIA_ENABLED` and
  `DEFAULT_CAMERA_PAN_INERTIA_DAMPING` constants in `graph_app.rs`,
  plus the two `GraphBrowserApp` constructor sites that initialized
  the zombie fields.
- **Deleted test** `test_camera_pan_inertia_settings_persist_across_restart`
  in `graph_app_tests.rs` — it tested persistence of the zombie
  fields only; pan-inertia persistence now lives on `NavigationPolicy`
  inside `GraphViewState` and `DomainState`.

**New settings UI** in [render/panels.rs](../../../../render/panels.rs):
`render_navigation_policy_settings_in_ui` replaces the old zombie
section inside `render_camera_controls_settings_in_ui`. Exposes every
NavigationPolicy knob the portable struct holds:

- Zoom range (min / max sliders, clamped so min ≤ max).
- Fit padding ratio slider (1.0×..1.50×).
- Scroll pan rate slider (5..200 px/unit).
- Drag threshold slider (1..24 px).
- Pan inertia checkbox + logarithmic damping slider (0.0001..0.05
  per second), hidden when inertia is off.
- Lasso modifier radio: Shift / Ctrl / Alt / None (with a tooltip on
  None noting the Figma convention flip).
- Two reset buttons: "Reset view override" (drops the per-view
  override; inherits graph default) and "Reset per-graph default"
  (returns the graph default to the portable baseline).

The UI resolves the policy once at the top, lets the user edit a
local mutable copy, then writes through `set_graph_view_navigation_policy_override`
when a view is focused or `set_navigation_policy_default` when not —
mirroring how other per-view-vs-per-graph surfaces work elsewhere
(physics profile, layout policy).

**Receipts**:
- `cargo check -p graphshell --lib` clean.
- `cargo test -p graphshell --lib` — 2154 pass (was 2155; -1 from
  the deleted zombie persistence test, no regressions elsewhere).
- `cargo test -p graph-canvas --features simulate --lib` — 259 pass
  (unchanged; the portable layer didn't change).

The settings UI for NodeStyle (color pickers for primary / secondary
/ search-hit states plus the default_radius slider) is a separate
pass — that one needs an egui color-picker integration and its own
settings section. Tracked as a follow-on.
