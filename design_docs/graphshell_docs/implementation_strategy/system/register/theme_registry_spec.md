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

1. **Token-authority policy**: Theme registry owns visual token/style resolution, not interaction semantics.
2. **Deterministic-fallback policy**: Theme lookup and fallback behavior must be explicit and stable.
3. **Separation policy**: Theme choices must not implicitly override layout, command, or mutation authorities.
4. **Conformance policy**: Theme providers must honor declared capability and compatibility contracts.

## Purpose and Scope

Provides visual token sets, palettes, and style resolution for UI and graph presentation.

In scope:
- theme registration and lookup
- visual token definitions
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

- Themes change appearance, not semantics.
- Theme lookup is explicit and stable across surfaces.
- Fallback themes are canonical and diagnosable.

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
