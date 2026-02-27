# Viewer State Matrix (2026-02-27)

**Status**: Research snapshot / migration aid  
**Scope**: Clarify current viewer state across three layers:
1) declared/selected IDs in `ViewerRegistry`,
2) runtime wiring via mod providers,
3) actual pane rendering behavior today.

## Why this exists

During the migration, viewer IDs can appear implemented when they are only declared. This note tracks the real state so planning and issue scoping can use one source.

## Matrix

| Viewer ID | Registry default mapping | Runtime provider wiring (Verso/mod path) | Render mode mapping | Actually rendered in pane today | Notes |
|---|---|---|---|---|---|
| `viewer:webview` | Yes | Yes | `CompositedTexture` | Yes (composited runtime path) | Canonical default web viewer ID.
| `viewer:servo` | Legacy alias (not default) | N/A | `CompositedTexture` | Yes (via alias compatibility) | Kept for persisted override compatibility.
| `viewer:wry` | Declared in docs/runtime policies | Depends on mod wiring/feature flags | `NativeOverlay` | Partially platform/path-dependent | Not universally active in all runs.
| `viewer:plaintext` | Yes | Core/default | `EmbeddedEgui` | Yes | Implemented in tile behavior text path.
| `viewer:markdown` | Yes | Core/default | `EmbeddedEgui` | Yes | Implemented via markdown embedded render path.
| `viewer:pdf` | Yes | Often remapped by Verso to `viewer:webview` for routing | `EmbeddedEgui` (policy) | Not embedded as dedicated pane renderer in current path | Present in selection/mode mapping; dedicated embed path not active in node pane behavior.
| `viewer:csv` | Yes | Selection/runtime tests present | `EmbeddedEgui` (policy) | Not embedded as dedicated pane renderer in current path | Routing exists; dedicated CSV pane renderer not active.
| `viewer:settings` | Yes (`graphshell://settings`) | Core/default | `EmbeddedEgui` (policy) | Routed to settings intents/surfaces (not generic node embed) | Special internal route.
| `viewer:metadata` | Core-seed/default fallback in seed mode | Core/default | `EmbeddedEgui` (policy) | Limited/indirect path | Used as safe non-web fallback identity in seed contexts.
| `viewer:image` | Documented target | Not yet active in default registry mapping | Intended embedded mode | Not yet | Planned in universal content model work.
| `viewer:directory` | Documented target | Not yet active in default registry mapping | Intended embedded mode | Not yet | Planned in universal content model work.
| `viewer:audio` | Documented target | Not yet active in default registry mapping | Intended embedded mode | Not yet | Planned in universal content model work.

## Current practical interpretation

- Stable today: `viewer:webview`, `viewer:plaintext`, `viewer:markdown`.
- Compatibility maintained: `viewer:servo` alias.
- Declared but not fully pane-embedded in the active path: `viewer:pdf`, `viewer:csv`, and additional documented targets.
- Special route: `viewer:settings` resolves into settings intents rather than a generic content pane implementation.

## Immediate planning value

Use this matrix when defining work items as:

- **Selection complete, render missing**,
- **Render mode mapped, provider wiring partial**,
- **Fully operational**.

This avoids mixing declaration-level readiness with runtime completeness.
