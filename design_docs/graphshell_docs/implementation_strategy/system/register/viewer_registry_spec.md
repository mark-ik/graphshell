# Viewer Registry Spec

**Doc role:** Canonical registry spec for `viewer_registry`.
**Status:** Active / canonical
**Kind:** Atomic registry
**Related docs:**
- [../2026-02-22_registry_layer_plan.md](../2026-02-22_registry_layer_plan.md) (registry ecosystem and ownership model)
- [SYSTEM_REGISTER.md](SYSTEM_REGISTER.md) (register hub and routing boundary)

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
