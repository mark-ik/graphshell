# VIEWER — Layout Domain Feature Area

**Date**: 2026-02-28
**Status**: Architectural domain feature note
**Priority**: Immediate architecture clarification

**Related**:

- `viewer_presentation_and_fallback_spec.md`
- `../2026-02-28_ux_contract_register.md`

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

`Liquid`, `Gas`, and `Solid` may influence presentation feel, motion, and framing defaults, but Viewer still owns visual fallback and visible degradation behavior.

---

## 4. Bridges

- Graph -> Viewer: node or graph content to render
- Workbench -> Viewer: pane host and destination rect
- Focus -> Viewer: active vs inactive presentation state

---

## 5. Architectural Rule

If a behavior answers "how is this content visibly presented right now?" it belongs to the **Viewer**.
