# COMMAND — Aspect

**Date**: 2026-02-28
**Status**: Architectural aspect note
**Priority**: Immediate architecture clarification

**Related**:

- `command_surface_interaction_spec.md`
- `../2026-02-28_ux_contract_register.md`

**Policy authority**: This file is the canonical policy authority for the Command aspect. Supporting command docs refine contracts/implementation detail and must defer policy authority to this file.
Policy in this file should be distilled from canonical specs and accepted research conclusions.

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
- consistency across keyboard, Search Palette Mode, Context Palette Mode, Radial Palette Mode, and omnibar
- context-aware category ordering so summon target context shapes first-category priority across command surfaces

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

---

## 6. Utility Audit (2026-03-06)

Keep:

- `render/action_registry.rs` shared category-policy helpers (`rank_categories_for_context`, persisted category encode/decode) as the single ranking authority for command surfaces.
- Shared persisted preference keys for category recency/pins to keep Context Palette Mode and Radial Palette Mode aligned.

Consolidated:

- Duplicate category recency/pin persistence helpers have been moved into shared `render/command_profile.rs` and are now consumed by both `render/command_palette.rs` and `render/radial_menu.rs`.

Defer:

- Full profile-backed radial geometry persistence (current implementation persists geometry via egui context state and keyboard tuning controls).
