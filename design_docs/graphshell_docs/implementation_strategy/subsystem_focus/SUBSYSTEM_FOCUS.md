# SUBSYSTEM_FOCUS

**Date**: 2026-02-28  
**Status**: Architectural subsystem note  
**Priority**: Immediate architecture clarification

**Related**:
- `focus_and_region_navigation_spec.md`
- `../2026-02-28_ux_contract_register.md`

---

## 1. Purpose

This note defines the **Focus subsystem** as the architectural owner of focus, region activation, and handoff.

It exists to keep one boundary explicit:

- focus ownership is Graphshell-owned,
- framework-local focus state may inform rendering,
- but framework focus must not become semantic authority.

---

## 2. What The Focus Subsystem Owns

- active region
- focus handoff rules
- return-path rules
- modal or blocking focus policy
- cross-surface activation precedence

---

## 3. Cross-Subsystem Policy Layer

`Liquid`, `Gas`, and `Solid` may affect how aggressive or inert transitions feel, but the Focus subsystem still owns deterministic focus authority and handoff rules.

---

## 4. Bridges

- Focus -> Graph: active graph pane ownership
- Focus -> Workbench: active tile/frame ownership
- Focus -> Command: which surface receives command context
- Focus -> Viewer / Control surfaces: return-path and escape behavior

---

## 5. Architectural Rule

If a behavior answers "which surface owns input right now?" it belongs to the **Focus subsystem**.
