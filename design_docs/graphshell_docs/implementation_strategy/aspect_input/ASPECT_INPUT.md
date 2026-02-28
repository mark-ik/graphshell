# INPUT — Aspect

**Date**: 2026-02-28
**Status**: Architectural aspect note
**Priority**: Immediate architecture clarification

**Related**:

- `system/register/input_registry_spec.md`
- `../2026-02-28_ux_contract_register.md`
- `../aspect_command/ASPECT_COMMAND.md`
- `../subsystem_focus/SUBSYSTEM_FOCUS.md`

---

## 1. Purpose

This note defines the **Input aspect** as the architectural owner of user input routing from hardware event to semantic intent.

It exists to keep one boundary explicit:

- raw input events are delivery signals,
- the Input aspect owns context-sensitive dispatch,
- and semantic action meaning belongs to the Command aspect, not the input layer.

---

## 2. What The Input Aspect Owns

- input event ingestion (keyboard, pointer, gamepad, touch)
- input context stack (which input mode is active: keyboard, gamepad, modal overlay)
- key/button binding resolution against the active input context
- input remapping configuration (user-defined rebinding)
- chord and sequence recognition
- dispatch of resolved bindings into the Command aspect or Focus subsystem
- consistency of input behavior across all surfaces and input modes

---

## 3. Cross-Domain / Cross-Subsystem Policy Layer

The Input aspect feeds both the **Command aspect** (action dispatch) and the **Focus subsystem** (focus routing).

`Liquid`, `Gas`, and `Solid` may influence input responsiveness expectations (e.g., drag thresholds, gesture sensitivity), but the Input aspect does not define those preset semantics — it consumes them from the owning policy layer.

---

## 4. Bridges

- Input -> Command: resolved binding triggers an action dispatch
- Input -> Focus: navigation keys advance focus within the active region
- Input -> Canvas: pointer events that do not resolve to a bound action fall through to graph-space interaction
- Registry -> Input: `InputRegistry` supplies the active binding configuration and context stack

---

## 5. Architectural Rule

If a behavior answers "how does a hardware or software input event become a routed intent?" it belongs to the **Input aspect**.

