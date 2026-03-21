# Theme Registry Spec

**Doc role:** Canonical registry spec for `theme_registry`.
**Status:** Active / canonical
**Kind:** Atomic registry
**Related docs:**
- [../2026-02-22_registry_layer_plan.md](../2026-02-22_registry_layer_plan.md) (registry ecosystem and ownership model)
- [SYSTEM_REGISTER.md](SYSTEM_REGISTER.md) (register hub and routing boundary)

**Policy authority**: This file is the canonical policy authority for `theme_registry` semantics and boundaries.
Policy in this file should be distilled from canonical specs and accepted research conclusions.

**Adopted standards** (see [2026-03-04_standards_alignment_report.md](../../research/2026-03-04_standards_alignment_report.md) §§3.5, 3.6, 3.7)):
- **OSGi R8** — capability registration, lookup, and fallback floor vocabulary
- **OpenTelemetry Semantic Conventions** — diagnostic channel naming/severity
- **WCAG 2.2 Level AA** — theme token definitions must satisfy minimum contrast ratios (SC 1.4.3, 1.4.6, 1.4.11); fallback theme must meet AA on all surfaces

## Registry Policies

1. **Visual-policy authority**: Theme registry owns canonical visual policy resolution, including semantic token mappings and accessibility-aware variants.
2. **Deterministic-fallback policy**: Theme lookup and fallback behavior must be explicit and stable.
3. **Separation policy**: Theme choices must not implicitly override layout, command, or mutation authorities.
4. **Conformance policy**: Theme providers must honor declared capability and compatibility contracts.

## Purpose and Scope

Provides visual token sets, palettes, and style resolution for UI and graph presentation.

In scope:
- theme registration and lookup
- visual token definitions
- accessibility-aware semantic token mappings
- contrast and monochrome/high-contrast behavior carried by theme definitions
- palette and style capability metadata

Out of scope:
- layout semantics
- camera motion policy
- viewer backend selection

## Canonical Model

This registry is a named capability surface within the Register. It should expose stable lookup and capability contracts, keep failures explicit, and avoid hidden fallback semantics.

Canonical interfaces:
- `get_theme(id)`
- `resolve_token(theme, token)`
- `describe_theme(id) -> ThemeCapability`

## Normative Core

- A theme is not a cosmetic skin. It is the canonical visual policy for semantic rendering.
- Accessibility behavior is part of theme definition, not a layer bolted on after theme resolution.
- Themes express semantic rendering; they do not redefine semantic meaning.
- Theme lookup is explicit and stable across surfaces.
- Fallback themes are canonical and diagnosable.

### Theme Contract

Every theme must satisfy a canonical `ThemeContract`.

Illustrative shape:

```rust
pub struct ThemeContract {
    pub min_contrast_against_canvas: f32,
    pub min_family_luminance_delta: f32,
    pub require_non_color_family_distinction: bool,
    pub require_monochrome_preservation: bool,
}
```

Normative rule:

- the contract defines invariants every theme must satisfy,
- a theme may vary expression, but not violate semantic legibility,
- registry validation must reject themes that fail the contract.

### Semantic Edge Tokens

Edge-family and edge-kind rendering belongs to the theme surface itself.

Illustrative shape:

```rust
pub struct ThemeEdgeTokens {
    pub family_tokens: BTreeMap<EdgeStyleFamily, ThemeEdgeFamilyToken>,
    pub kind_tokens: BTreeMap<EdgeStyleKey, ThemeEdgeKindToken>,
    pub hover: ThemeEdgeEmphasisToken,
    pub selection: ThemeEdgeEmphasisToken,
}
```

Normative rule:

- family identity must remain readable without hue alone,
- sub-kind variation is constrained within family identity,
- monochrome/high-contrast variants are valid theme projections, not separate ad hoc systems.

### Accessibility as Theme Capability

Theme capability metadata must include whether the theme supports:

- monochrome edge rendering,
- high-contrast presentation,
- a declared default accessibility projection for edge styling.

This means “theme” and “accessibility” are not opposing layers. Accessibility is
part of what makes a theme a valid theme.

## Planned Extensions

- per-subsystem theme variants
- semantic color hint integration with KnowledgeRegistry

## Prospective Capabilities

- adaptive themes driven by mode/preset policy
- user-authored theme packages

## Acceptance Criteria

- Registration, lookup, and fallback behavior are covered by registry contract tests.
- At least one harness or scenario path exercises the registry through real app behavior.
- Diagnostics channels emitted by this registry follow the Register diagnostics contract.
- Registry ownership remains explicit and does not collapse into widget-local or ad hoc fallback logic.
