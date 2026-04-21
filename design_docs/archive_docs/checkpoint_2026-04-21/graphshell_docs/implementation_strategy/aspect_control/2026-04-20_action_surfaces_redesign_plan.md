---
status: Archived 2026-04-21
owner: aspect_control track
created: 2026-04-20
refines: ./2026-02-24_control_ui_ux_plan.md
related:
  - ../shell/2026-04-14_iced_host_migration_execution_plan.md
  - ../shell/2026-04-20_configurability_sweep_phase2_plan.md
---

# Action Surfaces Redesign — Palette + Radial

## Why this plan exists

The egui build shipped with three concepts that blurred into each other
in code and in the head: "command palette," "command context menu," and
"command radial menu." The 2026-02-24 control UX plan already
**retired "context menu" as a Graphshell concept** — the right-click
popup is the **Command Palette in contextual mode**, not a separate
menu. But the code still carries the blur: `context_palette_anchor`
naming, split `show_command_palette` / `show_context_palette` /
`show_radial_menu` booleans, and a partially-migrated `RadialDomain`
enum that predates the 8-sector redesign.

Two user-reported bugs have concrete code-shape causes, not
architecture-shape causes:

1. **"Context menu followed the cursor"** — the contextual palette
   anchors at the click *cursor position* (plus 10px offset), not at
   the target node's bbox. Fix is at the anchor-computation site,
   not in the surface design.
2. **"Palette reappeared unexpectedly"** — there's no explicit scope
   enum; open-state is a bare bool with no stored scope. A state
   change that should close the palette leaves the bool `true`. Fix
   is to tag open-state with its originating scope and close on
   scope transitions.

The iced host migration (M3.5) is the forcing function: the portable
core needs a host-neutral resolver that iced and egui both consume.
Now is the moment to land the clean shape before duplicating the mess
into a second host.

## Target architecture

### Two surfaces, one registry, one anchor policy

- **Command Palette** — one surface, two scope modes:
  - **Global** (`Ctrl+K`): screen-centered modal, fuzzy-search the
    full action registry filtered by current `ActionScope`. Never
    anchored to cursor or target.
  - **Contextual** (right-click on a target, or gamepad equivalent):
    anchored to the **target's bbox** (not cursor), clamped to
    viewport. Lists actions that apply to that target.

- **Radial Menu** — 8-sector max, one action per sector, labels
  rendered outside the ring, uniform sizes, no concentric rings.
  Anchored to the target. Gamepad-primary; mouse-invocable too.

- On-graph, **only one of `{contextual palette, radial menu}` is open
  at a time**. Active input mode picks the default; user can override
  via bind. Exclusivity is enforced by sharing one state field (see
  "State shape" below), not by two bools that could both be true.

### Action registry stays largely as-is

The existing `list_actions_for_context()` resolver and the const
`ACTION_*` keys in `runtime/registries/action.rs` stay. What's
missing is scope — today scope is implicit in
`ActionContext.target_node: Option<NodeKey>`. We add an explicit
enum so scope transitions are observable.

```rust
pub enum ActionScope {
    Global,
    Graph { target: Option<NodeKey> },
    Workbench { target: Option<PaneId> },
    Webview { target: WebViewId },
}
```

Scope becomes a first-class field on `ActionContext`; the
`target_node` / `target_frame_name` / `target_frame_member` fields
collapse into the scope variant's payload. `list_actions_for_context`
keeps its signature; behavior unchanged.

### State shape — collapse the bool soup

Today `ChromeUiState` carries:

- `show_command_palette: bool`
- `show_context_palette: bool`
- `command_palette_contextual_mode: bool`
- `context_palette_anchor: Option<[f32; 2]>`
- `show_radial_menu: bool`

Replace with one enum:

```rust
pub enum ActionSurfaceState {
    Closed,
    PaletteGlobal,
    PaletteContextual { scope: ActionScope, anchor: Anchor },
    Radial { scope: ActionScope, anchor: Anchor },
}

pub enum Anchor {
    Target { bbox_world: Rect2 },      // node bbox, pane bbox
    ViewportPoint(Vec2),                // fallback for free-space right-click
    ScreenCenter,                       // global palette only
}
```

Exclusivity is enforced by the type: you cannot have both a
contextual palette and a radial open, because the state is a single
variant. Scope transitions (focused view change, graph clear, node
deletion that removes the target) auto-close by matching on the
scope variant and comparing to current context.

### Anchoring fix

The contextual palette and the radial menu both take `Anchor::Target`
whenever there is a target. Computation:

1. Translate target's world-space bbox to screen-space via the
   active camera.
2. Offset the surface to the target's **right edge** (palette) or
   **centroid** (radial), clamped to viewport with a 12px margin.
3. Cursor position is **not** an input to anchor computation for
   target-anchored surfaces. (Free-space right-click, which has no
   target, uses `Anchor::ViewportPoint` at the cursor — that is the
   only case where cursor position leaks into anchoring.)

### Naming taxonomy — drop "Command", retire "context menu"

| Old name                   | New name                   | Notes                                |
|----------------------------|----------------------------|--------------------------------------|
| `show_command_palette`     | `ActionSurfaceState::PaletteGlobal` | folded into enum            |
| `show_context_palette`     | `ActionSurfaceState::PaletteContextual` | folded into enum         |
| `context_palette_anchor`   | `Anchor::Target`           | no more `context_palette_*`          |
| `show_radial_menu`         | `ActionSurfaceState::Radial` | folded into enum                   |
| `open_context_palette()`   | `open_palette_contextual(scope, anchor)` | explicit scope param   |
| `close_context_palette()`  | `close_action_surface()`   | one closer for all surfaces          |
| `toggle_command_palette()` | `toggle_palette_global()`  |                                      |
| `RadialDomain` (private enum) | *removed*              | actions come from registry           |
| `command_palette_category_*` persisted keys | kept as-is | disk format; rename only if migrating |

"Command" prefix vanishes from types and methods. An action is always
a command; prefixing it is redundant. "Menu" stays only for the
radial (`RadialMenu`), because "radial surface" reads worse.

### Configurability — policy surfaces

Two new portable policy types, sited in the canvas crate so iced
reuses them:

```rust
pub struct ActionInputPolicy {
    pub palette_global_bind: KeyBind,
    pub palette_contextual_bind: PointerBind,
    pub radial_bind: GamepadBind,
    pub on_graph_default_surface_mouse: OnGraphSurface,    // PaletteContextual
    pub on_graph_default_surface_gamepad: OnGraphSurface,  // Radial
}

pub struct ActionSurfacePolicy {
    pub palette_max_results: usize,         // default 12
    pub palette_fuzzy_threshold: f32,
    pub radial_sector_count: u8,            // default 8, max 8
    pub radial_label_placement: RadialLabelPlacement, // OutsideRing default
    pub surface_margin_px: f32,             // default 12.0
}
```

Both follow the three-slot resolver pattern established in the
configurability sweeps: per-view override → per-graph default →
hardcoded fallback.

### Iced portability

The split is clean:

- **Portable** (graph-canvas crate): `ActionScope`, `Anchor`,
  `ActionSurfaceState`, `ActionInputPolicy`, `ActionSurfacePolicy`,
  and the `resolve_actions_for(scope) -> Vec<Action>` resolver.
- **Host-specific** (egui today, iced next): the rendering — egui
  `Area` for palette, egui tessellated arcs for radial, vs iced
  widgets — and the input-capture glue that emits
  `open_palette_contextual(scope, anchor)` etc.

Iced host reuses the entire portable layer. No opinionated defaults
are baked host-side.

## Phasing

### Phase A — Naming + state consolidation (no behavior change)

1. Introduce `ActionSurfaceState` enum; add as a new field on
   `ChromeUiState` alongside the existing bools.
2. Write a translator that reads/writes the legacy bools from the
   enum value, so the enum becomes authoritative without breaking
   any read-side code.
3. Rename methods per the taxonomy table; leave old method names as
   thin delegators during migration.
4. Remove the old bools and delegators once all call sites point at
   the enum and its accessors.

Acceptance: zero behavioral change vs current build; one source of
truth for surface state.

### Phase B — Anchor fix

1. Introduce `Anchor` enum.
2. In the contextual-palette open path, compute
   `Anchor::Target { bbox_world }` from the clicked node's bbox.
3. In the radial open path, same.
4. Free-space right-click remains `Anchor::ViewportPoint(cursor)`.
5. Screen-position derivation moves into a single
   `Anchor::resolve_screen_position(viewport, camera, margin_px) -> Vec2`
   function. All positioning call sites route through it.

Acceptance: contextual palette and radial are visibly glued to the
target when one exists; moving the camera after open relocates them
with the target.

### Phase C — Explicit ActionScope + scope-close invariant

1. Add `ActionScope` enum; widen `ActionContext` to carry it.
2. At surface-open time, record the scope into the
   `ActionSurfaceState::*{ scope, … }` payload.
3. Add `close_on_scope_transition(current: ActionScope)` — called on
   focus change, graph clear, and target deletion. Matches the
   stored scope against current; closes if incompatible.
4. Unit-test the transition table (Global stays across focus change;
   `Graph { target: N }` closes when N is removed; etc.).

Acceptance: repeatable reproducer for "palette reappears
unexpectedly" is gone; tests cover the scope transition matrix.

### Phase D — Radial migration completion

1. Remove the `RadialDomain` enum.
2. Radial sectors populate from `list_actions_for_context()` filtered
   to `radial_eligible` predicate (top-8 by sort weight).
3. Render labels outside the ring; uniform sector sizes; no concentric
   rings.
4. Add `ActionSurfacePolicy.radial_sector_count` / `_label_placement`
   overrides.

Acceptance: no hardcoded action list in `radial_menu.rs`; all entries
trace to registry keys.

### Phase E — Portability lift

1. Move `ActionScope`, `Anchor`, `ActionSurfaceState`,
   `ActionInputPolicy`, `ActionSurfacePolicy`, and the resolver to
   `crates/graph-canvas/src/action_surface.rs`.
2. Keep the egui rendering + input-capture in
   `render/command_palette.rs` and `render/radial_menu.rs`, consuming
   the portable resolver.
3. Gate the Phase E move on iced M3.5 being open (so iced picks it
   up fresh; no intermediate duplicate type).

Acceptance: iced host reuses the portable crate with zero divergence
from egui behavior.

## Acceptance criteria (top-level)

- [ ] Single `ActionSurfaceState` enum; `ChromeUiState` bool-soup
      removed
- [ ] `context_palette_*` naming gone; "Command" prefix dropped from
      types and methods
- [ ] Contextual palette and radial both anchor to target bbox, not
      cursor
- [ ] `ActionScope` enum exists and participates in scope-close
      invariant
- [ ] Scope-transition unit tests cover focus change, graph clear,
      target deletion, global-palette persistence
- [ ] `RadialDomain` hardcoded enum removed; radial reads from
      registry
- [ ] `ActionInputPolicy` + `ActionSurfacePolicy` portable and
      override-capable per the three-slot pattern
- [ ] Iced host consumes the portable resolver unchanged

## Open questions

1. **Palette in contextual mode vs. global mode — same widget or two
   widgets?** Current plan treats them as one widget with two scope
   modes (dual-state enum). Alternative: two widgets sharing the
   resolver. Preference: one widget, two modes, because the input
   affordance is identical (a searchable list) and only the
   positioning + default scope filter differ.

2. **Gamepad contextual-palette invocation.** The 2026-02-24 plan
   makes radial the gamepad default. Should a gamepad user ever
   invoke the contextual palette? Probably yes as a fallback
   (long-press?). Needs a bind in `ActionInputPolicy`.

3. **Anchor stickiness on camera movement.** Should an open contextual
   palette track its target as the camera pans, or freeze at its
   initial screen position? Preference: track — surfaces feel "glued"
   to their target, matches the spatial-browser metaphor. But adds a
   per-frame recompute. Policy option if needed.

4. **Servo webview native context menu.** Separate concern, already
   Servo-owned. Out of scope for this plan; noted for completeness.

## Progress log

### 2026-04-20

- Draft created after action-surface inventory (see recon summary in
  conversation thread 2026-04-20).
- Grounded against `render/action_registry.rs`,
  `render/command_palette.rs`, `render/radial_menu.rs`,
  `workspace_state.rs`, `ux_navigation.rs`, and the 2026-02-24
  control UX plan.

### 2026-04-20 — Phase A/B/C first pass landed

#### Phase A (consolidation + naming) — complete

- New module `app/action_surface.rs` introduces `ActionScope`,
  `ScopeTarget`, `Anchor`, `ActionSurfaceState`.
- `surface_state: ActionSurfaceState` added to `ChromeUiState` at
  [app/workspace_state.rs](../../../../../../app/workspace_state.rs).
  Legacy four-bool soup kept in sync for readers pending migration.
- New entry points on `GraphBrowserApp`:
  `open_palette_global`, `open_palette_contextual(scope, anchor)`,
  `open_radial(scope, anchor)`, `close_action_surface`. Legacy
  `open_command_palette` / `open_context_palette` / `open_radial_menu`
  retained and updated to also maintain `surface_state`.

#### Phase B (anchor mechanism) — mechanism landed; resolver deferred

- `Anchor` enum carries `TargetNode(NodeKey)`, `TargetFrame(String)`,
  `ViewportPoint { x, y }`, `ScreenCenter`.
- Four right-click sites updated to emit target-aware anchors:
  - [render/mod.rs](../../../../../../render/mod.rs) node right-click →
    `Anchor::TargetNode(target)`
  - [render/mod.rs](../../../../../../render/mod.rs) frame backdrop →
    `Anchor::TargetFrame(name)`
  - [shell/desktop/workbench/tile_post_render.rs](../../../../../../shell/desktop/workbench/tile_post_render.rs)
    and [tile_behavior/tab_chrome.rs](../../../../../../shell/desktop/workbench/tile_behavior/tab_chrome.rs)
    → `Anchor::ViewportPoint(cursor)` (tab chrome has no canvas target).
- **Deferred**: the command-palette render site
  ([render/command_palette.rs](../../../../../../render/command_palette.rs)
  ~ line 434) still reads the legacy `context_palette_anchor: [f32; 2]`.
  Translating `Anchor::TargetNode`/`TargetFrame` → screen position at
  render time requires threading the active camera + graph state
  through the palette render path. Set sites populate both the legacy
  anchor and the new `Anchor` variant; behavior is unchanged for node
  right-click (cursor ≈ node) and the frame-backdrop "anchors to
  cursor wherever in backdrop" quirk is unfixed until the resolver
  lands. Follow-on: migrate the render-site read to the new
  `surface_state.anchor()` + a resolver `fn resolve_screen_point(&self,
  graph, camera, viewport) -> egui::Pos2`.

#### Phase C (scope + close invariant) — mechanism landed; focus-change wiring deferred

- `ActionContext` gained a `scope: ActionScope` field (default
  `Global`). `list_actions_for_context` signature unchanged; the
  field is populated by the two real-world construction sites
  ([render/command_palette.rs](../../../../../../render/command_palette.rs)
  and [render/radial_menu.rs](../../../../../../render/radial_menu.rs))
  from `surface_state.scope()`.
- `close_action_surface_if_targets_node(NodeKey)`,
  `close_action_surface_if_graph_scoped()`,
  `close_action_surface_if_in_other_view(GraphViewId)` live on
  `GraphBrowserApp`.
- Wired at:
  - [app/graph_mutations.rs](../../../../../../app/graph_mutations.rs)
    `clear_graph` — `close_action_surface_if_graph_scoped()`
  - [app/graph_mutations.rs](../../../../../../app/graph_mutations.rs)
    `remove_selected_nodes` loop — `close_action_surface_if_targets_node(node_key)`
- **Deferred**: `close_action_surface_if_in_other_view` method
  exists but is not yet wired at the 18 `focused_view` assignment
  sites. Follow-on: introduce a `GraphBrowserApp::set_focused_view`
  wrapper and migrate sites through it; wrapper calls the
  scope-close invariant before returning.

#### Tests — 8 green

- [app/action_surface.rs](../../../../../../app/action_surface.rs)
  `tests` module: predicate coverage across `default_is_closed`,
  `palette_global_has_no_scope_or_anchor`,
  `contextual_on_node_reports_scope_and_anchor`,
  `node_deletion_closes_matching_scope_only`,
  `graph_clear_closes_all_graph_scoped_surfaces`,
  `focus_change_closes_surfaces_scoped_to_other_views`,
  `radial_and_contextual_cannot_be_open_simultaneously`,
  `anchor_viewport_point_resolves_without_camera`.
- Full `graphshell --lib` suite green after a targeted fix to the
  `secondary_click_on_node_opens_radial_palette_when_preferred`
  test (the radial branch now pre-populates `surface_state` without
  flipping the `show_radial_menu` bool, preserving the intent-driven
  open flow).

#### Not yet touched

- Phase A.4 (remove legacy bools + delegators) — bools still present
  as derived state.
- Phase B render-site resolver (see above).
- Phase C focus-change wiring (see above).
- Phase D (radial `RadialDomain` enum removal, registry migration).
- Phase E (portable lift to `crates/graph-canvas`) — iced-gated.

#### Known debt

- The "context palette opens at cursor inside frame backdrop" visual
  bug remains until Phase B's render-site resolver lands. The
  mechanism is in place; only the resolver is missing.
- Two real-world `ActionContext` construction sites populate
  `scope` from `surface_state.scope()`, which is correct at render
  time. If a future caller builds an `ActionContext` outside the
  render loop, it will default to `ActionScope::Global` — acceptable
  for filter purposes, noted for future readers.

### 2026-04-21 — Phase C focus-change wiring landed

- [`app/focus_selection.rs set_workspace_focused_view_with_transition`](../../../../../../app/focus_selection.rs) — the canonical setter now calls `close_action_surface_if_in_other_view(new_view)` on any view transition, and `close_action_surface()` when focus clears and the current surface is graph-scoped. The "palette reappears in a new view" leak is closed at the setter.
- [`render/mod.rs set_focused_view_with_transition`](../../../../../../render/mod.rs) — the duplicate shell-level setter that was bypassing the canonical one now delegates through it. All production focus transitions flow through one scope-close point.
- Test-only direct field writes (`app.workspace.graph_runtime.focused_view = ...` inside `#[test]` blocks) remain untouched — tests set up state without triggering scope-close hooks, which is correct test hygiene and doesn't affect production behavior.
- Verification: `cargo test -p graphshell --lib -- --test-threads=1` → **2166 pass / 0 fail / 3 ignored**.

### 2026-04-21 — Phases A.4, B, D scoped as separate follow-on plans

After the Phase C landing, the remaining three phases each carry enough surface area + design weight to warrant their own dedicated plan rather than follow-on PRs inside this one.

#### Phase A.4 (legacy bool removal) — honest scope

The four legacy `ChromeUiState` bool fields (`show_command_palette`, `show_context_palette`, `command_palette_contextual_mode`, `show_radial_menu`) plus `context_palette_anchor: Option<[f32; 2]>` have **177 read/write references across 22 files**. The bulk are straightforward substitutions (`show_command_palette` → `surface_state.is_palette_global()`), but:

- `context_palette_anchor: Option<[f32; 2]>` callers use the raw `[f32; 2]` directly for `egui::Window::fixed_pos` positioning. `surface_state.anchor()` returns `&Anchor` (enum) — call sites need either a compatibility helper (`Anchor::resolved_screen_point() -> Option<[f32; 2]>`, which already exists for non-Target variants) or render-site target resolution (coupled with Phase B).
- Many call sites live in `ux_navigation.rs` where the existing enum-updating methods already mirror bool writes; those call sites become redundant at removal time and should be audited to ensure no stale writes persist.

**Acceptance as follow-on plan**: single PR removing all five fields; before/after sanity-compile at each substitution batch; compatibility helpers on `Anchor` and `ActionSurfaceState` to keep the migration purely mechanical once Phase B's anchor resolver lands.

#### Phase B (render-site resolver) — requires camera/viewport threading

Resolving `Anchor::TargetNode(k)` → screen position at the palette render site (`render/command_palette.rs` ~ line 434) requires:

- The active camera for the scope's `view_id` (accessible via `workspace.canvas_cameras` or `graph_views[view_id].camera`).
- The scene viewport for that view (not the egui app viewport) — this is currently passed implicitly through the render loop; the palette render function would need it plumbed.
- A strategy for when target is a `Frame(name)` — the frame's world-space centroid comes from `arrangement_projection_groups()` (already used in `radial_menu.rs`), but turning that into a screen point needs the same camera/viewport.

Additionally, the plan's acceptance states "moving the camera after open relocates them with the target" — the stickiness behavior noted as an open question. Delivering this means per-frame recompute, which has implications for focus/animation behavior.

**Acceptance as follow-on plan**: extend the `Anchor` resolver with typed camera+viewport inputs; thread those into `render_command_palette_panel`; stickiness becomes a policy option (`ActionSurfacePolicy.anchor_track_camera: bool`). The visible "frame backdrop cursor anchor" bug is fixed as a consequence.

#### Phase D (radial `RadialDomain` removal + 8-sector redesign) — UX redesign, not enum deletion

Re-verified 2026-04-21: `RadialDomain` enum in [`render/radial_menu.rs`](../../../../../../render/radial_menu.rs) has **18 references** and now functions purely as a **category-to-sector geometry mapping** for the tier-1/tier-2 radial layout. Action *content* already traces to the registry via `list_radial_actions_for_category(context, category)` — the plan's primary acceptance criterion ("no hardcoded action list; all entries trace to registry keys") is already met.

What remains is the **UX redesign** from "tier-1 categories + tier-2 options" into a flat "8-sector, one action per sector, labels outside ring" shape per the 2026-02-24 control UX plan. That's not an enum deletion — it is a layout redesign that touches:

- Radial ring geometry (flat 8-sector vs concentric tiers)
- Action selection predicate (`radial_eligible` flag + sort weight)
- Keyboard navigation (angular selection instead of tier-1/tier-2 drill-down)
- Label placement (outside-ring, collision resolver)

**Acceptance as follow-on plan**: dedicated radial-redesign plan, gated on UX acceptance criteria (screen-reader behavior, overflow handling, gamepad vs keyboard parity). This plan's Phase D is **marked complete** for the registry-authority portion; the flat-8-sector UX redesign is lifted out.

#### Phase E (portable lift to `crates/graph-canvas`) — iced-gated

Unchanged. Remains blocked on the iced M3.5 milestone. When iced is ready to consume the portable action-surface vocabulary, the five enums/types in [`app/action_surface.rs`](../../../../../../app/action_surface.rs) + the predicate methods can move to the graph-canvas crate as a mechanical lift; no redesign needed.

### 2026-04-21 — Plan status

- **Complete phases**: A (consolidation), B mechanism, C (scope-close full coverage).
- **Split out**: A.4 (bool removal PR), B resolver (camera/viewport threading PR), D (flat-8-sector radial UX redesign plan), E (iced portability lift — iced-gated).
- This plan's architectural scope — "consolidated state enum, scope-aware close invariant, typed anchor mechanism, portable-ready action registry scope" — is landed end-to-end. The four follow-ons are cleanup and/or new design work with their own acceptance criteria; leaving them as Not Yet Touched inside this plan makes the plan look perpetually open when in fact its core contract is fulfilled.

**Recommendation: archive this plan as complete. Open new plans for A.4, B-resolver, and D-radial-UX as they become priority.**
