# Viewer Registry Spec

**Doc role:** Canonical registry spec for `viewer_registry`.
**Status:** Active / canonical
**Kind:** Atomic registry
**Related docs:**
- [../2026-02-22_registry_layer_plan.md](../2026-02-22_registry_layer_plan.md) (registry ecosystem and ownership model)
- [SYSTEM_REGISTER.md](SYSTEM_REGISTER.md) (register hub and routing boundary)

**Policy authority**: This file is the canonical policy authority for `viewer_registry` semantics and boundaries.
Policy in this file should be distilled from canonical specs and accepted research conclusions.

**Adopted standards** (see [2026-03-04_standards_alignment_report.md](../../research/2026-03-04_standards_alignment_report.md) §§3.5, 3.6, 3.7)):
- **OSGi R8** — capability registration, selection authority, and fallback floor vocabulary
- **OpenTelemetry Semantic Conventions** — diagnostic channel naming/severity
- **WCAG 2.2 Level AA** — each viewer registry entry must declare `AccessibilityCapabilities`; viewer surfaces are subject to all-surfaces WCAG AA conformance

## Registry Policies

1. **Selection-authority policy**: Viewer registry owns content-to-viewer selection, not viewport or tile placement policy.
2. **Fallback-floor policy**: Unsupported content resolves through canonical fallback viewers/core-seed floor.
3. **Diagnosable-selection policy**: Viewer selection/fallback paths must remain explicit and observable.
4. **Capability-declaration policy**: Viewer providers declare capabilities/conformance before selection use.

## Purpose and Scope

Maps MIME types, extensions, and content categories to viewer implementations.

In scope:
- viewer capability registration
- MIME and content-type routing
- core seed viewer floor and fallback ordering

Out of scope:
- viewer pane layout
- document viewport policy
- graph/workbench routing

## Canonical Model

This registry is a named capability surface within the Register. It should expose stable lookup and capability contracts, keep failures explicit, and avoid hidden fallback semantics.

Canonical interfaces:
- `select(content) -> ViewerId`
- `render(ui, content)`
- `describe_viewer(id) -> ViewerCapability`

## Normative Core

- Viewer selection is explicit and diagnosable; unsupported content resolves to canonical fallback viewers.
- Core seed viewers keep the app useful without web backends.
- Viewer selection is independent of pane arrangement and viewport behavior.

## Planned Extensions

- richer capability scoring and content negotiation
- backend-specific conformance declarations

## Prospective Capabilities

- multi-viewer composition for the same payload
- progressive viewer handoff based on capability negotiation

## Acceptance Criteria

- Registration, lookup, and fallback behavior are covered by registry contract tests.
- At least one harness or scenario path exercises the registry through real app behavior.
- Diagnostics channels emitted by this registry follow the Register diagnostics contract.
- Registry ownership remains explicit and does not collapse into widget-local or ad hoc fallback logic.
