# Input Interaction Spec

**Date**: 2026-03-01
**Status**: Canonical interaction contract
**Priority**: Immediate implementation guidance

**Related**:
- `ASPECT_INPUT.md`
- `../2026-02-28_ux_contract_register.md`
- `../subsystem_focus/focus_and_region_navigation_spec.md`
- `../aspect_command/command_surface_interaction_spec.md`
- `../system/register/input_registry_spec.md`
- `../../design/KEYBINDINGS.md`
- `../../TERMINOLOGY.md` — `ActionRegistry`, `InputRegistry`

---

## 1. Purpose and Scope

This spec defines the canonical contract for how hardware input events become routed intents
in Graphshell.

It covers:

- what the Input aspect owns and what it does not own,
- the input context stack and how context switches,
- how raw events are resolved to bound actions,
- chord and sequence recognition rules,
- remapping and profile contract,
- cross-surface routing priority,
- fallback and degraded-mode behavior,
- diagnostics obligations,
- acceptance criteria.

This spec covers input routing, not semantic action meaning. Action meaning belongs to
`ActionRegistry` and the Command aspect. Focus advancement belongs to the Focus subsystem.
Canvas-space pointer interaction (hit testing, drag, lasso) belongs to the Canvas spec.

---

## 2. Ownership Boundaries

### 2.1 What the Input aspect owns

- hardware event ingestion: keyboard, pointer, gamepad, touch
- input context stack: which surface and mode owns input at any moment
- keybinding/button-binding resolution within the active context
- chord recognition and sequence detection
- user-defined remapping configuration
- dispatch of resolved bindings to the Command aspect or Focus subsystem
- unresolved pointer event passthrough to the canvas or active viewer

### 2.2 What the Input aspect does not own

- action semantics — owned by `ActionRegistry`
- focus routing — owned by the Focus subsystem
- canvas-space pointer semantics (hit testing, drag, lasso) — owned by Canvas
- command availability and targeting — owned by `ActionRegistry` + Command aspect
- viewer-internal input handling — owned by the viewer (Servo, Wry, EmbeddedEgui viewers
  may consume events before the Input aspect sees them)

---

## 3. Input Context Stack

### 3.1 Context definition

An **input context** is a named scope that determines:

- which bindings are active,
- which raw events are intercepted before passthrough,
- whether global bindings remain available or are suppressed.

### 3.2 Canonical input contexts

| Context | Active when | Global bindings |
|---------|-------------|----------------|
| `Normal` | default application state | all active |
| `TextEntry` | omnibar, search field, inline editor, any text input is active | most suppressed; Escape exits |
| `Modal` | a blocking dialog, confirmation surface, or overlay that requires resolution | all suppressed except modal-specific |
| `GamepadNav` | gamepad input mode is active (radial menu / D-pad navigation) | gamepad-specific active; keyboard still resolves |
| `CommandPalette` | command palette or radial menu is open | palette-specific active; Escape exits |
| `GraphDrag` | pointer button held for node drag or lasso in canvas | drag/lasso-specific; most keyboard suppressed |
| `EmbeddedContent` | keyboard focus is inside a Servo webview or embedded viewer | webview-local events consumed by viewer; host escape path must remain |

### 3.3 Stack rules

- The input context stack has exactly one active context at a time.
- Opening a surface pushes a new context; closing restores the prior context.
- Context stack transitions are deterministic — no implicit context changes from hover or
  pointer position alone.
- A context pushed without a matching pop is a correctness bug.

### 3.4 Context invariants

**Invariant**: `EmbeddedContent` context must always expose at least one host-reachable
escape binding (e.g. `Escape` or a configurable host-focus-reclaim binding) that the
embedded viewer cannot suppress.

**Invariant**: `TextEntry` and `Modal` suppress global bindings exactly — they do not
suppress each other's entry/exit keys. A modal opened while in `TextEntry` must produce a
clean context push.

---

## 4. Binding Resolution

### 4.1 Resolution pipeline

For each incoming hardware event, the Input aspect executes in order:

1. Check whether the active viewer consumes the event first (Servo/Wry/EmbeddedEgui handle
   their own keyboard/pointer events before host routing). If consumed, stop.
2. Check the active input context's binding table for an exact match.
3. Check whether the event extends an in-progress chord (§4.2).
4. Check whether the event resolves a pending sequence (§4.3).
5. If a binding is found, dispatch to the Command aspect with the resolved `ActionId`.
6. If no binding is found in the active context, check context-independent global bindings
   (e.g. `F6` for region cycling, which must remain accessible from all contexts except
   `Modal`).
7. If still unresolved, pass the raw event through to the active canvas or viewer for
   canvas-space interpretation.

### 4.2 Chord recognition

A **chord** is a binding that requires two or more keys held simultaneously.

Rules:

- Chords are always specified as an ordered modifier + trigger set (e.g. `Ctrl+Shift+K`).
- Modifier keys (`Ctrl`, `Shift`, `Alt`, `Meta`) are recognized as chord participants, not
  as standalone triggers unless explicitly bound alone.
- Partial chord state (modifiers held but trigger not yet pressed) must not fire any action.
- Chord resolution is immediate when the trigger key is pressed; no timeout.

### 4.3 Sequence recognition

A **sequence** is a binding that requires two or more distinct key presses in order, each
pressed and released before the next.

Rules:

- Sequences have a configurable timeout; the default is 1000 ms between presses.
- If the timeout expires mid-sequence, the partial sequence is abandoned and each key is
  re-evaluated as a standalone event from that point.
- Sequences only fire on completion; partial sequences do not dispatch partial actions.
- Sequences and chords do not interact: a chord mid-sequence resets the sequence.

### 4.4 Resolution priority

When multiple bindings could match the same event:

1. Context-specific bindings take priority over global bindings.
2. More specific bindings (longer chord or sequence) take priority over less specific ones.
3. If two bindings of equal specificity conflict, the one registered later wins, and a
   `CHANNEL_INPUT_BINDING_CONFLICT` diagnostic is emitted at `Warn` severity.

---

## 5. Remapping Contract

### 5.1 Scope

User-defined remapping allows binding tables to be modified at runtime without recompilation.

Rules:

- Remapping operates on named `InputProfile` objects registered in `InputRegistry`.
- The active profile is selected globally and applies to the `Normal` context by default.
- Per-context profile overrides are supported (e.g. gamepad-specific profile activates with
  `GamepadNav` context).

### 5.2 Remapping invariants

**Invariant**: Remapping may not suppress the host-focus-reclaim escape path from
`EmbeddedContent` context.

**Invariant**: Remapping may not remove bindings that are marked `system-reserved` in the
binding table. System-reserved bindings are a fixed set declared by Graphshell core
(e.g. `Escape` exits `Modal`; `F6` cycles regions).

**Invariant**: A remapping that produces a conflict between two active bindings must be
detected at profile activation time and reported as a diagnostic, not silently resolved at
event time.

### 5.3 Profile lifecycle

- Profiles are registered in `InputRegistry` at startup (native mods) or at runtime (WASM
  mods).
- Profile changes take effect immediately; no restart is required.
- The active profile name is persisted in app preferences.

---

## 6. Cross-Surface Routing Priority

When multiple surfaces are active (e.g. graph pane + viewer pane + command palette), the
Input aspect routes using the following priority order:

| Priority | Surface | Condition |
|----------|---------|-----------|
| 1 | Modal / blocking surface | `Modal` context active |
| 2 | Command palette / radial menu | `CommandPalette` context active |
| 3 | Omnibar / text entry | `TextEntry` context active |
| 4 | Active viewer (Servo/Wry) | `EmbeddedContent` context active; viewer-local event handling runs first |
| 5 | Active canvas pane | `Normal` or `GraphDrag` context; canvas-space pointer passthrough |
| 6 | Workbench chrome | fallback for unresolved events with no canvas or viewer active |

**Invariant**: A lower-priority surface must never intercept an event that a higher-priority
surface has claimed. Event consumption is exclusive per event.

---

## 7. Gamepad Input Contract

### 7.1 Input mode detection

Graphshell detects active input mode based on the most recently active input device:

- gamepad button/axis activity → activates `GamepadNav` context
- keyboard/pointer activity → deactivates `GamepadNav` context if it was active

Mode transitions are immediate. The user can switch freely between keyboard/pointer and
gamepad mid-session.

### 7.2 Gamepad binding model

Gamepad bindings use the same `InputProfile` mechanism as keyboard bindings.

- D-pad directions map to navigation actions (focus advance, radial menu sector selection).
- Analog sticks map to scroll / camera pan in canvas or viewer panes.
- Shoulder buttons and face buttons map to `ActionId` entries.
- Trigger axes may be mapped to actions or treated as analog; no implicit analog → discrete
  conversion unless the binding specifies a threshold.

### 7.3 Radial menu interaction

In `GamepadNav` context, the radial menu is the primary command surface.

- D-pad or left stick selects a radial sector.
- Confirm button (A / Cross) invokes the selected action.
- Cancel button (B / Circle) closes the radial menu without action.
- The radial menu opening/closing is a context push/pop (`CommandPalette` context).

---

## 8. Diagnostics Obligations

| Channel | Severity | Condition |
|---------|----------|-----------|
| `CHANNEL_INPUT_BINDING_CONFLICT` | `Warn` | Two bindings of equal specificity conflict on the same event |
| `CHANNEL_INPUT_CONTEXT_PUSH` | `Info` | Context stack push |
| `CHANNEL_INPUT_CONTEXT_POP` | `Info` | Context stack pop |
| `CHANNEL_INPUT_CONTEXT_LEAK` | `Error` | Context pushed without matching pop (stack leak detected) |
| `CHANNEL_INPUT_ESCAPE_SUPPRESSED` | `Error` | Host-focus-reclaim escape is suppressed in `EmbeddedContent` context |
| `CHANNEL_INPUT_SEQUENCE_TIMEOUT` | `Info` | In-progress sequence timed out and was abandoned |
| `CHANNEL_INPUT_DISPATCH_US_SAMPLE` | `Info` | Sampled event-to-dispatch latency in microseconds |
| `CHANNEL_INPUT_REMAP_CONFLICT` | `Warn` | Profile activation produced a binding conflict |

All channels follow the Graphshell diagnostics channel schema (severity field required;
`Info` for normal observability, `Warn` for recoverable policy violations, `Error` for
correctness invariant breaks).

---

## 9. Planned Extensions

- per-context profile override selection in settings surface,
- chord timeout configurability,
- sequence timeout configurability per-profile,
- richer gamepad axis mapping (dead zones, curves, per-game profiles),
- input macro recording (sequence → action batch, gated on `ActionRegistry` batch support),
- input replay for diagnostics (record event stream, replay for regression testing).

---

## 10. Prospective Capabilities

- voice input context and speech-to-action pipeline,
- eye-tracking integration as a pointer supplement,
- adaptive binding suggestions based on usage patterns (AgentRegistry),
- hardware-specific input packs distributed as WASM mods.

---

## 11. Acceptance Criteria

1. Exactly one input context is active at all times; context stack is never empty.
2. Chords and sequences do not fire partial actions.
3. `EmbeddedContent` context always preserves a host-reachable escape path.
4. `Modal` context suppresses global bindings completely except its own exit path.
5. Binding conflicts are detected and reported as diagnostics, not silently resolved.
6. Remapping cannot suppress system-reserved bindings.
7. Gamepad and keyboard/pointer contexts switch immediately on device activity.
8. All input routing is diagnosable through the defined channel set.
9. Input dispatch latency is sampled and available in diagnostics.
