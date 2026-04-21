---
status: Active
owner: iced-migration track
created: 2026-04-20
supersedes: —
related:
  - ../2026-04-14_iced_host_migration_execution_plan.md
  - ../2026-04-20_navigation_policy_plan.md (archived)
  - ../2026-04-20_node_style_configurability_sweep_plan.md (archived)
---

# Configurability Sweep — Phase 2

Follow-on to the navigation-policy and node-style sweeps. Same pattern
(portable struct + per-view override + per-graph default + host-neutral
resolver), applied to two new surfaces: physics release behavior, and
shell chrome theming.

## Why this round

The navigation-policy sweep proved the three-slot resolver pattern:
`resolve_X(view_id)` returns view override ? per-graph default ?
preset fallback. Carrying that through iced M3.5 means the iced host
can reuse the same resolver calls without re-deriving from egui state.

Two surfaces carried hardcoded constants that would have forced the
iced host to replicate opinionated defaults:

1. `SimulateMotionProfile` — release-impulse tuning for drag→fling
   was inlined at `graph/scene_runtime.rs:236` with an app-local
   duplicate struct.
2. Shell chrome colors — status dots, workbench panel background,
   and the high-contrast selection-highlight trio were `Color32::from_rgb`
   literals scattered across four files.

Both are user-visible defaults that a hosting embedder (iced, future
wgpu-native) will want to override without patching the shell.

## §1 SimulateMotionProfile — landed

### Receipts

- `crates/graph-canvas/src/scene_physics.rs` — `SimulateMotionProfile`
  gained `Serialize + Deserialize + Default`. `Default` derives from
  `SimulateBehaviorPreset::default()` so serialized forms round-trip.
- `app/graph_views.rs` — `GraphViewState` gained
  `simulate_motion_override: Option<SimulateMotionProfile>` with
  `#[serde(default)]`. Added `resolve_simulate_motion_profile(view_id)`,
  `set_graph_view_simulate_motion_override`,
  `set_simulate_motion_default` on `GraphBrowserApp`.
- `domain/mod.rs` — `DomainState.simulate_motion_default: Option<…>`
  added; both constructor sites updated.
- `graph/scene_runtime.rs:236` — call site swapped from inlined
  struct to `app.resolve_simulate_motion_profile(view_id)`.
- `render/canvas_bridge.rs` — three tests:
  - `resolve_simulate_motion_profile_falls_back_to_preset`
  - `resolve_simulate_motion_profile_prefers_view_override`
  - `resolve_simulate_motion_profile_falls_back_to_per_graph_default`

### Known debt

- `app::graph_views::SimulateBehaviorPreset` and
  `graph_canvas::scene_physics::SimulateBehaviorPreset` are still
  duplicated. `resolve_simulate_motion_profile` does a manual match
  to bridge them. Dedup deferred — touches more surface than this
  sweep warranted.

## §2 Theme palette — landed (cheap pass)

Six new tokens added to `ThemeTokenSet` in
`shell/desktop/runtime/registries/theme.rs`, wired across all four
theme variants (default / light / dark / high_contrast):

- `status_warning`, `status_neutral` — paired with existing
  `status_success` for the three-state sync-status dot.
- `workbench_panel_background` — the `(20, 20, 25)` chrome-fill that
  recurred at four frame call sites.
- `selection_highlight_background` /
  `selection_highlight_text` / `selection_highlight_stroke` —
  the high-contrast-theme trio previously inlined in egui `Visuals`.

### Call sites swapped

- `shell/desktop/ui/workbench_host.rs` — `graph_scope_sync_status`
  now resolves `status_neutral` / `status_warning` / `status_success`
  from theme tokens.
- `shell/desktop/ui/gui_frame/post_render_phase.rs` — both
  `SidePanel::right("workbench_area")` and `CentralPanel::default()`
  frames now pull `workbench_panel_background` from the active theme.
- `shell/desktop/workbench/tile_render_pass.rs:1391` — `run_tile_render_pass`
  `CentralPanel` frame swapped.
- `shell/desktop/ui/toolbar/toolbar_ui.rs` —
  `render_fullscreen_origin_strip` derives its translucent fill from
  `workbench_panel_background` + alpha 220, instead of the literal.
- `shell/desktop/ui/gui.rs` — `apply_runtime_theme_visuals` high-contrast
  branch now reads the three selection-highlight tokens instead of
  inlining `from_rgb(255, 230, 0)` / `WHITE` / `BLACK`.

All four theme variants resolve to values that preserve visual
parity with the prior hardcoded defaults (default theme uses the
same rgb triples that were inlined).

### Deferred (follow-on sweeps)

- Minimap palette — `shell/desktop/ui/overview_plane.rs` has ~12
  `Color32::WHITE` / literal-rgb sites clustered around the overview
  rendering. They share one visual semantic (minimap foreground)
  and belong in a dedicated `minimap_*` token pass.
- Divider stroke tokens — 8 sites in `gui_frame/*` and the workspace
  swatch use `Stroke::new` with ad-hoc grays. Group into
  `divider_stroke_primary` / `_subtle` in a follow-on.
- `NodeStyle` color-picker UI — the node-style sweep landed the
  policy surface but didn't wire an egui `color_edit_button_rgba`
  control for end users. Follow-on UX-layer task.

## §3 Iced M3.5 alignment

Both lifts feed directly into the M3.5 carve-out in
`../2026-04-14_iced_host_migration_execution_plan.md`. When the iced
host renders the workbench frame or simulates release impulse, it
calls the same resolver methods on `GraphBrowserApp` — no bespoke
defaults to re-derive. That closes the two outstanding items noted
in the M3 completion log.

## Acceptance criteria

- [x] `SimulateMotionProfile` portable with `Default + Serialize +
      Deserialize`
- [x] `resolve_simulate_motion_profile` on `GraphBrowserApp` with
      view-override + per-graph-default + preset-fallback semantics
- [x] Three unit tests covering each resolver path
- [x] Six new theme tokens wired in all four theme variants
- [x] Five shell call sites swapped off `Color32::from_rgb` literals
- [x] `cargo check -p graphshell` clean (warnings unchanged)
- [x] `cargo test -p graph-canvas --lib` green (224 pass)
- [x] Canvas-bridge resolver suite green (25 pass)
- [x] Broader `graphshell --lib` default-features suite (2158 pass /
      3 ignored)

## Progress log

### 2026-04-20

- SimulateMotionProfile portability + resolver landed with tests.
- Theme palette tokens added; five call sites swapped.
- Deferred sweeps logged in §2.
- Iced M3.5 alignment noted in §3.
