<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Node Style Configurability Sweep Plan (2026-04-20)

**Status**: **Archived 2026-04-20** — portable layer + host wiring +
tests landed. Color-picker UI for node-state styles deferred to a
future settings-UI pass (noted in §5 as the single remaining gap).
**Scope**: Continue the retroactive configurability sweep started by
`NavigationPolicy`. Lift the highest-value remaining hardcoded
host-side constants — default node radius and the primary / secondary
selection + search-hit visual triple — into a portable `NodeStyle`
policy with per-view overrides and per-graph defaults.

**Related**:

- [2026-04-20_navigation_policy_plan.md](2026-04-20_navigation_policy_plan.md) — sibling policy (camera / input / inertia feel). Same shape: portable struct, per-view override, per-graph default, app-side resolver, host-side consumer.
- [2026-04-19_step5_spatial_pattern_layouts_plan.md](../graph/2026-04-19_step5_spatial_pattern_layouts_plan.md) — earlier configurability sweep on the layouts side.
- [../../../memory/feedback_configurability_over_opinionated_defaults.md](../../../../../.claude/projects/c--Users-mark--Code/memory/feedback_configurability_over_opinionated_defaults.md) — the user-pinned directive driving this sweep.

---

## 1. Framing

After `NavigationPolicy` landed (2026-04-20), the residual opinionated
defaults on the host side were mostly clustered in
`render/canvas_bridge.rs`:

- `fn default_node_radius() -> f32 { 16.0 }` — baked into every
  `CanvasNode` built from the domain graph.
- Three frozen design decisions inside the `node_overrides` closure in
  `run_graph_canvas_frame`: primary-selected fill/stroke/label-color
  (`(0.3, 0.7, 1.0, 1.0)` fill, white stroke at width `2.5`,
  white label), secondary-selected (`(0.3, 0.6, 0.9, 0.9)` fill,
  `(0.8, 0.9, 1.0, 0.8)` stroke at width `1.5`), search-hit
  (`(0.9, 0.8, 0.2, 1.0)` fill, no stroke).

Per an Explore agent's audit, those were the items most worth lifting:
frequently-changed design decisions that users will reasonably want
to theme. The rest of the audit (label fallback color, label font-size
formula, scene-region corner-radius formula, internal culling
constants) is either already tunable (`OverlayStyle`, `DeriveConfig`,
`LodPolicy`, `ProjectionConfig`) or too-internal to justify lifting
this pass.

## 2. Design

`NodeStyle` lives at `crates/graph-canvas/src/node_style.rs`,
mirroring `NavigationPolicy`'s layering exactly: portable serde
struct, per-view override on `GraphViewState`, per-graph default on
`DomainState`, host-neutral resolver on `GraphBrowserApp`. iced will
read the same resolved style as the current egui host.

Fields:

- `default_radius: f32` — replaces the old
  `default_node_radius()` function. Threaded into `build_scene_input`
  as a new parameter so per-node radius overrides (a future feature)
  can layer on top.
- `primary_selection: NodeStateStyle` — fill, optional stroke,
  optional label color for the focus anchor node.
- `secondary_selection: NodeStateStyle` — for non-primary selected
  nodes.
- `search_hit: NodeStateStyle` — for nodes that match the active
  search query but aren't selected.

`NodeStateStyle` is a small struct `{ fill, stroke, label_color }`
with `stroke` and `label_color` optional so themes can opt out of
either ("fill-only secondary selection", "inherit label color from
the derive pipeline's default").

Exposed constants:

- `DEFAULT_NODE_RADIUS = 16.0` — the one literal that used to live
  inline in `canvas_bridge.rs`, now accessible to both the policy
  defaults and any test that wants to match pre-sweep behavior.

## 3. Host wiring

`render/canvas_bridge.rs::run_graph_canvas_frame` now resolves both
`NodeStyle` and `NavigationPolicy` at the top of the function (one
call each, cheap). The rest of the function changed shape:

- `build_scene_input(..)` grew a `default_node_radius: f32` parameter;
  called with `node_style.default_radius`.
- The `node_overrides` closure inside `derive_scene_with_overlays`
  replaced its three hardcoded `NodeVisualOverride` literals with a
  single match on selection / search state that picks the matching
  `NodeStateStyle` from the resolved policy.
- The private `default_node_radius()` helper was removed; its only
  caller was `build_scene_input`, which now takes the value as a
  parameter.

Three new accessor methods on `GraphBrowserApp`:
`resolve_node_style(view_id)`, `set_graph_view_node_style_override(..)`,
`set_node_style_default(..)` — identical shape to the three
`NavigationPolicy` accessors added on 2026-04-20.

## 4. Iced benefit

iced gets the exact same resolver: call
`app.resolve_node_style(view_id)` from the iced render bridge, feed
the resulting `NodeStyle` into the `node_overrides` closure and the
`build_scene_input` radius argument. No parallel defaults table, no
per-host divergence in selection colors.

## 5. What was NOT included

Deferred to a future pass (from the audit):

- **Label rendering defaults** inside `derive_nodes` — fallback color
  (line 386 in `derive.rs`: `Color(0.9, 0.9, 0.9, 1.0)`), base font
  size formula (`12.0 * depth_scale * zoom`, clamped to min `6.0`),
  label offset below node (`radius + 4.0`). Worth lifting into
  `OverlayStyle` or a new `LabelStyle` when a settings UI needs to
  reach them.
- **Scene-region corner-radius formula** (`6.0 * zoom.max(0.25)`).
  Same story — small polish knob, not worth its own round trip yet.
- **Node-state labels with per-state fonts / bold / icons.** The
  current `NodeStateStyle` is color + stroke only; richer state
  decoration is a bigger design ask.

Explicitly *not* touched:

- `OverlayStyle` — already fully tunable.
- `DeriveConfig` — already exposes `default_node_color`,
  `default_edge_color`, `default_edge_width`, `lod_policy`, `projection`.
- `LodPolicy`, `ProjectionConfig` — both already exposed as configs.

## 6. Receipts

- `cargo test -p graph-canvas --lib node_style::` — 2 pass
  (`defaults_match_pre_sweep_values`, `serde_roundtrip_preserves_all_fields`).
- `cargo test -p graph-canvas --features simulate --lib` — 259 pass
  (was 257 pre-sweep; +2 NodeStyle tests).
- `cargo test -p graphshell --lib render::canvas_bridge` — 22 pass
  (was 19 pre-sweep; +3 resolver / wiring tests:
  `resolve_node_style_falls_back_to_graph_default`,
  `resolve_node_style_prefers_view_override_over_graph_default`,
  `run_graph_canvas_frame_applies_per_view_node_radius_to_scene`).
- `cargo test -p graphshell --lib` — 2155 pass (was 2152 pre-sweep).
- `cargo check -p graphshell --lib` clean.

## 7. Progress

### 2026-04-20

- Plan landed end-to-end following the `NavigationPolicy` cadence:
  `NodeStyle` in graph-canvas with `NodeStateStyle`, per-view +
  per-graph storage, resolver on `GraphBrowserApp`, `canvas_bridge`
  switched from the hardcoded `default_node_radius` + inline selection
  color literals to the resolved policy. Plan links out to the
  audit's deferred items so the next configurability pass can pick
  them up without re-discovery.
