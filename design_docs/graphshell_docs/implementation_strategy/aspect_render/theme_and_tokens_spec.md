<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Theme and Tokens Spec

**Date**: 2026-04-30
**Status**: Canonical / Active
**Scope**: Token-based theming for Graphshell. Defines the canonical token
catalog (color, typography, density, motion, elevation), token resolution
through the [settings spine](../aspect_control/settings_and_permissions_spine_spec.md)
(default → persona → graph → view/tile → pane), the iced realization as
`GraphshellTheme` (an `iced::Theme` extension), the `gs::*` widget
consumption pattern, and the libcosmic-compatibility path.

**Related**:

- [`ASPECT_RENDER.md`](ASPECT_RENDER.md) — Render aspect authority
- [`../aspect_control/settings_and_permissions_spine_spec.md`](../aspect_control/settings_and_permissions_spine_spec.md) — five-scope spine that theme tokens resolve through
- [`../shell/iced_composition_skeleton_spec.md` §1.5.2](../shell/iced_composition_skeleton_spec.md) — `GraphshellTheme` as `Application::Theme`; cosmic-time animation hook
- [`../../TERMINOLOGY.md`](../../TERMINOLOGY.md) — `Theme` term canonical definition
- libcosmic theme system (external reference) — token taxonomy precedent

---

## 1. Intent

Every visual property in Graphshell — color, font, spacing, motion timing,
elevation, focus ring style — resolves through a **token**. Tokens have
canonical names; concrete values come from a resolved theme; widgets read
tokens, not hardcoded values.

This buys:

- **Atomic theme switches** at any scope (persona dark/light flip, per-graph
  sepia, per-view high-contrast — all with one settings write).
- **Coherent visual language** across the iced host, the canvas Program,
  and the embedded viewer chrome.
- **libcosmic compatibility path**: token names align with cosmic-theme
  where possible, so a future libcosmic distribution shipping Graphshell
  can subclass tokens without per-widget patching.

---

## 2. Token Categories

Tokens are grouped by visual concern. Each category has a small canonical
catalog; new tokens are added through this spec, not invented per-widget.

### 2.1 Color tokens

```rust
pub struct ColorTokens {
    // Surface (chrome, panes, modals)
    pub surface_base: Color,        // app background
    pub surface_raised: Color,      // panes, modal containers
    pub surface_overlay: Color,     // modal overlays, popovers
    pub surface_inset: Color,       // input fields, depressed surfaces

    // Text
    pub text_primary: Color,
    pub text_secondary: Color,      // secondary copy, disabled-state labels
    pub text_inverse: Color,        // on-accent backgrounds
    pub text_link: Color,

    // Accents
    pub accent: Color,              // primary brand / highlight
    pub accent_hover: Color,
    pub accent_active: Color,
    pub accent_subtle: Color,       // for badge backgrounds

    // Borders + dividers
    pub border_subtle: Color,
    pub border_strong: Color,
    pub divider: Color,

    // Semantic states
    pub success: Color,
    pub warning: Color,
    pub error: Color,
    pub info: Color,

    // Focus + selection
    pub focus_ring: Color,
    pub selection_bg: Color,
    pub selection_fg: Color,

    // Graph-domain colors (canvas)
    pub canvas_bg: Color,
    pub node_default: Color,
    pub node_active: Color,           // currently active in a tile pane
    pub node_inactive: Color,         // graphlet member, not active
    pub node_ghost: Color,            // tombstoned, dashed
    pub edge_default: Color,
    pub edge_traversal: Color,
    pub edge_user_grouped: Color,
    pub edge_containment: Color,
    pub edge_arrangement: Color,
}
```

Per-scope overrides (e.g., a graph-scope `accent = sepia_orange`) write
to single token keys; resolution per the settings spine returns the
narrowest defined value.

### 2.2 Typography tokens

```rust
pub struct TypographyTokens {
    pub font_family_ui: FontFamily,        // system UI font stack
    pub font_family_content: FontFamily,   // for tile content; reader-mode aware
    pub font_family_mono: FontFamily,      // for code, tool panes

    pub font_size_xs: f32,
    pub font_size_sm: f32,
    pub font_size_base: f32,
    pub font_size_lg: f32,
    pub font_size_xl: f32,

    pub line_height_tight: f32,
    pub line_height_base: f32,
    pub line_height_relaxed: f32,

    pub font_weight_regular: FontWeight,
    pub font_weight_medium: FontWeight,
    pub font_weight_bold: FontWeight,
}
```

### 2.3 Density tokens

Per-Pane density is tile-or-pane scope (per the settings spine §3.1);
density tokens scale spacing and target sizes.

```rust
pub struct DensityTokens {
    pub spacing_xxs: f32,                  // 2 dp at default density
    pub spacing_xs: f32,
    pub spacing_sm: f32,
    pub spacing_base: f32,                 // 8 dp
    pub spacing_lg: f32,
    pub spacing_xl: f32,
    pub spacing_xxl: f32,

    pub target_size_min: f32,              // ≥ 32 dp per WCAG 2.2 SC 2.5.8
    pub target_size_compact: f32,          // ≥ 32 dp; compact density
    pub target_size_relaxed: f32,          // ≥ 44 dp; touch-friendly

    pub border_radius_sm: f32,
    pub border_radius_base: f32,
    pub border_radius_lg: f32,
    pub border_radius_pill: f32,           // for chip-style elements
}
```

### 2.4 Motion tokens

Animation timing + easing. Consumed by `cosmic-time` per the
[composition skeleton §1.5.2](../shell/iced_composition_skeleton_spec.md).

```rust
pub struct MotionTokens {
    pub duration_instant: Duration,        // 0 ms — for reduced-motion mode
    pub duration_xs: Duration,             // 80 ms — micro feedback
    pub duration_sm: Duration,             // 150 ms — hover transitions
    pub duration_base: Duration,           // 220 ms — modal / palette
    pub duration_lg: Duration,             // 320 ms — pane reflow

    pub easing_linear: EasingCurve,
    pub easing_standard: EasingCurve,      // ease-in-out
    pub easing_decelerate: EasingCurve,    // entering elements
    pub easing_accelerate: EasingCurve,    // exiting elements

    /// When OS or persona-scope reduced-motion is active, all
    /// durations collapse to `duration_instant` and easings to linear.
    pub reduced_motion_active: bool,
}
```

### 2.5 Elevation tokens

Shadow / lift / outline depth for layered surfaces.

```rust
pub struct ElevationTokens {
    pub elevation_0: ElevationStyle,       // flush with parent
    pub elevation_1: ElevationStyle,       // tile pane, pane chrome
    pub elevation_2: ElevationStyle,       // dropdown, popover
    pub elevation_3: ElevationStyle,       // modal (palette, finder)
    pub elevation_4: ElevationStyle,       // tooltips, transient hovers
}

pub struct ElevationStyle {
    pub shadow: Option<ShadowConfig>,
    pub outline_width: f32,
    pub outline_color_token: ColorTokenRef,  // references border_subtle, etc.
}
```

---

## 3. `GraphshellTheme` — iced realization

`GraphshellTheme` is `Application::Theme` for the iced host (per
[composition skeleton §1.5](../shell/iced_composition_skeleton_spec.md)).
It carries the resolved tokens for the current scope path:

```rust
#[derive(Clone, Debug)]
pub struct GraphshellTheme {
    pub colors: ColorTokens,
    pub typography: TypographyTokens,
    pub density: DensityTokens,
    pub motion: MotionTokens,
    pub elevation: ElevationTokens,
}

impl iced::theme::Base for GraphshellTheme {
    fn base(&self) -> iced::theme::Style { /* map tokens to iced base */ }
    fn palette(&self) -> iced::theme::Palette { /* map ColorTokens to iced::Palette */ }
}
```

The Theme value in `Application::theme()` is read from the view-model;
view-model rebuilds the resolved Theme each tick when settings change.
Per the [no-poll anti-pattern](../shell/iced_composition_skeleton_spec.md),
widgets do not poll for theme changes — they consume the active Theme
through iced's standard theme propagation.

### 3.1 Widget consumption pattern

Hand-rolled `gs::*` widgets read tokens through the active Theme:

```rust
impl iced::widget::Widget<Message, GraphshellTheme, Renderer> for gs::TileTabs {
    fn draw(&self, ..., theme: &GraphshellTheme, ...) {
        let bg = theme.colors.surface_raised;
        let active = theme.colors.accent;
        let radius = theme.density.border_radius_base;
        // ... use tokens, not hardcoded values ...
    }
}
```

Built-in iced widgets (`text_input`, `button`, `scrollable`, `pane_grid`,
`canvas::Canvas`, `shader`) consume the Theme through the
`iced::theme::Base` impl on `GraphshellTheme`. Style customization for
built-ins uses the existing iced `Style` API; the Theme just provides
the inputs.

---

## 4. Theme Resolution Through the Settings Spine

Per [settings spine §3](../aspect_control/settings_and_permissions_spine_spec.md),
theme tokens have a canonical scope of **persona** but support overrides
at every narrower scope.

Resolution example:

```text
default:    accent = #4F8AFF
persona:    accent = #FF7B3D       (user picked sepia accent)
graph:      —                      (no graph-scope override)
view:       accent = #2BB673       (research view: green accent)
pane:       —

Effective accent for that pane: #2BB673  (view-scope wins)
```

The view-model rebuilds `GraphshellTheme` per-tick by walking the active
scope path for each surface and resolving the relevant tokens.

### 4.1 Theme presets

Personas can pick from named presets (`Light`, `Dark`, `HighContrast`,
`Sepia`, `Cosmic`); a preset is a bundle of token overrides at persona
scope. Presets ship with Graphshell as default-scope catalogs and
populate the persona's settings tree on selection.

Custom themes are user-authored token overrides (no preset binding);
persisted at persona scope.

### 4.2 System-following modes

Three system-following toggles affect resolution:

- **`follow_system_dark_mode`**: when `true`, the persona's effective
  Light/Dark variant follows the OS color-scheme preference; the user's
  explicit dark-or-light choice is suppressed.
- **`follow_system_reduced_motion`**: when `true`, motion tokens
  collapse to instant/linear regardless of persona choice.
- **`follow_system_high_contrast`**: when `true`, color tokens map to
  the `HighContrast` preset.

Each toggle is a persona-scope setting; OS state is read through the
host platform's `iced::system` integration and folded into the
view-model per tick.

---

## 5. Animation Hook

Per [composition skeleton §1.5.2](../shell/iced_composition_skeleton_spec.md),
animations use [`cosmic-time`](https://crates.io/crates/cosmic-time) (or
its iced 0.14+ successor). Motion tokens (§2.4) are the inputs:

```rust
let pulse = cosmic_time::Timeline::new()
    .push(cosmic_time::Keyframe::new(theme.motion.duration_sm)
        .ease(theme.motion.easing_standard))
    .build();
```

Reduced-motion mode (`reduced_motion_active = true`) is checked once per
animation start; when active, the timeline collapses to instant.

The Tick Subscription (per [composition skeleton §1.5](../shell/iced_composition_skeleton_spec.md))
drives per-frame interpolation. Animation state stays widget-local where
possible.

---

## 6. libcosmic Compatibility

Token names align with cosmic-theme where possible. The intent: a
COSMIC-DE distribution of Graphshell can subclass tokens to inherit
COSMIC system theme without per-widget patching.

Mapping (canonical):

| Graphshell token | cosmic-theme equivalent |
|---|---|
| `surface_base` | `bg_color` |
| `surface_raised` | `bg_component_color` |
| `surface_overlay` | `bg_overlay_color` |
| `text_primary` | `text_color` |
| `text_secondary` | `text_color_dim` (approx.) |
| `accent` | `accent_color` |
| `border_subtle` | `divider_color` |
| `focus_ring` | `accent_color_focus` |

Where Graphshell tokens have no cosmic equivalent (e.g., the graph-domain
canvas tokens), they live as Graphshell extensions; cosmic-theme is read
for the chrome subset only.

The libcosmic-compat layer is a separate adapter crate (planned, not in
first-bring-up) that translates a cosmic theme into a `GraphshellTheme`
instance.

---

## 7. Accessibility

Theme tokens encode accessibility-relevant defaults:

- **Contrast ratios** for `text_primary` on `surface_base` and
  `accent` on `surface_base` meet WCAG 2.2 AA (4.5:1 body, 3:1 large)
  in the default Light and Dark presets. The HighContrast preset
  meets AAA (7:1).
- **Focus ring** (`focus_ring` color + 2 dp outline) meets WCAG 2.2 SC
  2.4.11 contrast and visibility.
- **Target size** tokens (`target_size_min` ≥ 32 dp) meet SC 2.5.8.
- **Reduced motion** mode collapses all animations.

These are token-level defaults; widget impls must use the tokens
correctly. AT validation (per the
[command-surface observability plan](../subsystem_ux_semantics/2026-04-05_command_surface_observability_and_at_plan.md))
catches widget-level violations.

---

## 8. Theming Strategy by Surface

| Surface | Token consumption |
|---|---|
| App-level chrome (CommandBar, StatusBar) | persona-scope tokens; theme transitions are atomic |
| Frame split tree (`pane_grid`) | persona-scope; `surface_raised` for pane bodies, `border_subtle` for split lines |
| Tile pane / canvas pane | `pane`-scope override possible; canvas pane uses canvas-domain tokens (`canvas_bg`, `node_*`, `edge_*`) |
| Modals (Palette, Node Finder, ConfirmDialog) | persona-scope; `surface_overlay` + `elevation_3` |
| Context Menu | persona-scope; `surface_raised` + `elevation_2` |
| Tile content (viewer body) | tile-scope reader mode toggles `font_family_content` and density |
| Settings panes | persona-scope (or shows the scope being edited) |
| Tool panes (Diagnostics, Devtools, Downloads) | persona-scope; `font_family_mono` for content where applicable |
| Toasts | persona-scope; `elevation_4` |

Per-surface specs reference this spec for token resolution; they don't
re-spec tokens per surface.

---

## 9. Open Items

- **Token catalog appendix**: this spec defines categories; the
  canonical default values for each token (Light / Dark / HighContrast
  / Sepia / Cosmic presets) live in a separate appendix or in code.
- **Custom theme authoring UX**: how users author and share custom
  themes (TOML export, drag-import, persona settings). Tracked under
  Settings panes.
- **libcosmic adapter crate**: separate work item, post-bring-up.
- **Theme transition animation**: when switching presets, do tokens
  cross-fade or instant-swap? Currently instant; cross-fade is a
  Stage F polish.
- **Per-graph theme inheritance UX**: how do users see what's
  inherited vs overridden in a graph-scope settings pane? Tracked
  under Settings panes UX.
- **Print theming**: print/export views use a separate theme variant
  (high-contrast, no canvas tokens needed). Tracked alongside the
  print pipeline (graphshell-net or its own spec).

---

## 10. Bottom Line

One token catalog (color / typography / density / motion / elevation)
resolves through the five-scope settings spine. `GraphshellTheme` is
the iced realization, consumed by hand-rolled `gs::*` widgets directly
and by built-in iced widgets through `iced::theme::Base`. Token names
align with libcosmic where possible; a future libcosmic-compat adapter
can subclass tokens without per-widget patching. Animation runs
through cosmic-time keyed on motion tokens. WCAG 2.2 AA contrast +
focus + target-size requirements are encoded in the default presets.

This is the visual-language layer beneath every iced surface; surface
specs reference this for tokens and don't re-spec their own.
