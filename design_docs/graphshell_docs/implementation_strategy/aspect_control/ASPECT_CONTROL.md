# CONTROL — Aspect

**Date**: 2026-02-28
**Status**: Architectural aspect note
**Priority**: Immediate architecture clarification

**Related**:

- `settings_and_control_surfaces_spec.md`
- `../2026-02-28_ux_contract_register.md`

---

## 1. Purpose

This note defines the **Control aspect** as the architectural owner of app-owned tool and control panes.

It exists to keep one boundary explicit:

- tool surfaces are application control surfaces,
- not graph semantic owners,
- and not workbench-layout owners beyond the fact that they are hosted in panes.

---

## 2. What The Control Aspect Owns

- settings surfaces
- history manager surfaces
- diagnostics tool surfaces
- app-owned control pages
- apply / revert / return-path semantics inside those surfaces

---

## 3. Cross-Domain / Cross-Subsystem Policy Layer

`Liquid`, `Gas`, and `Solid` may expose tunable controls in these surfaces, but the subsystem only hosts and edits those settings. The preset semantics still belong to the owning runtime policies.

---

## 4. Bridges

- Workbench -> Control Surfaces: pane hosting and lifecycle
- Command -> Control Surfaces: open / close / navigate actions
- Focus -> Control Surfaces: return path and active tool ownership

---

## 5. Architectural Rule

If a behavior answers "how does the app expose settings or tool controls?" it belongs to the **Control aspect**.
