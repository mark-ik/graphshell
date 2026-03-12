# VIEWER — Layout Domain Feature Area

**Date**: 2026-02-28
**Status**: Architectural domain feature note
**Priority**: Immediate architecture clarification

**Related**:

- `viewer_presentation_and_fallback_spec.md` — canonical viewer selection, render-mode, and fallback contract
- `wry_integration_spec.md` — Wry webview integration contract
- `webview_lifecycle_and_crash_recovery_spec.md` — webview lifecycle, warm/cold/crashed states, recovery
- `node_lifecycle_and_runtime_reconcile_spec.md` — node → pane lifecycle reconciliation
- `node_viewport_preview_spec.md` — thumbnail/preview surface contract
- `visual_tombstones_spec.md` — tombstone display for dead/removed nodes
- `clipping_and_dom_extraction_spec.md` — DOM-level clipping and content extraction
- `universal_content_model_spec.md` — content model underlying viewer rendering
- `2026-02-26_composited_viewer_pass_contract.md` — composited viewer render pass contract
- `../2026-02-28_ux_contract_register.md`

**Adopted standards** (see [2026-03-04_standards_alignment_report.md](../../research/2026-03-04_standards_alignment_report.md) §§3.5, 3.6, 3.7)):
- **WCAG 2.2 Level AA** — viewer surfaces must expose accessible content structure; degraded/fallback states must remain operable and perceivable per SC 1.3.1, 4.1.2
- **OSGi R8** — viewer capability declaration and selection follow OSGi capability vocabulary
- **OpenTelemetry Semantic Conventions** — viewer fallback and degraded-state events follow OTel naming/severity

---

## 1. Purpose

This note defines the **Viewer** as the architectural owner of how content is presented once a destination exists.

It exists to keep one boundary explicit:

- Graph and Workbench decide what should be shown and where,
- Viewer decides how that content is rendered and what fallback state is visible.

---

## 2. What The Viewer Domain Feature Area Owns

- viewer selection
- placeholder and fallback presentation
- degraded-state presentation
- loading / blocked viewer surfaces
- overlay and presentation clarity rules

---

## 3. Cross-Domain / Cross-Subsystem Policy Layer

Physics/motion presets (`Liquid`, `Gas`, `Solid` — see `../canvas/layout_behaviors_and_physics_spec.md`) may influence presentation feel and motion emphasis, but Viewer still owns visual fallback and visible degradation behavior. These presets do not own graph camera policy or camera-lock semantics.

---

## 4. Bridges

- Graph -> Viewer: node or graph content to render
- Workbench -> Viewer: pane host and destination rect
- Focus -> Viewer: active vs inactive presentation state

---

## 5. Architectural Rule

If a behavior answers "how is this content visibly presented right now?" it belongs to the **Viewer**.
