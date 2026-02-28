# COMMAND — Aspect

**Date**: 2026-02-28
**Status**: Architectural aspect note
**Priority**: Immediate architecture clarification

**Related**:

- `command_surface_interaction_spec.md`
- `../2026-02-28_ux_contract_register.md`

---

## 1. Purpose

This note defines the **Command aspect** as the architectural owner of semantic action invocation.

It exists to keep one boundary explicit:

- command meaning is Graphshell-owned,
- command surfaces are delivery mechanisms,
- and UI widgets must not define semantic action behavior.

---

## 2. What The Command Aspect Owns

- action registry
- command meaning
- command availability
- command target resolution
- command dispatch and execution routing
- consistency across keyboard, palette, context menu, radial menu, and omnibar

---

## 3. Cross-Domain / Cross-Subsystem Policy Layer

`Liquid`, `Gas`, and `Solid` do not primarily belong to the Command subsystem.

The Command subsystem may expose commands that switch or tune those presets, but it does not define their semantics. It only dispatches intent into the owning subsystems.

---

## 4. Bridges

- Command -> Graph: camera, selection, graph actions
- Command -> Workbench: routing, tile/frame actions
- Command -> Viewer / Tool surfaces: open, close, focus, fallback actions

---

## 5. Architectural Rule

If a behavior answers "what does this action mean?" it belongs to the **Command aspect**.
