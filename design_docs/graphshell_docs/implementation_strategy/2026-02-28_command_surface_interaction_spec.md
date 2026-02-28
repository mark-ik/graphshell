# Command Surface Interaction Spec

**Date**: 2026-02-28  
**Status**: Canonical interaction contract  
**Priority**: Immediate implementation guidance

**Related**:
- `2026-02-28_ux_contract_register.md`
- `2026-02-27_workbench_frame_tile_interaction_spec.md`
- `2026-02-28_graph_node_edge_interaction_spec.md`
- `2026-02-24_control_ui_ux_plan.md`
- `../research/2026-02-24_interaction_and_semantic_design_schemes.md`
- `../design/KEYBINDINGS.md`

---

## 1. Purpose and Scope

This spec defines the interaction contract for Graphshell's command surfaces.

It explains:

- what command surfaces are for,
- what each command-entry surface means semantically,
- who owns command meaning and availability,
- what state transitions command invocation implies,
- what visual feedback must accompany command use,
- what fallback behavior must happen when command execution is blocked,
- which command surfaces are core, planned, and exploratory.

This spec covers command invocation surfaces, not graph or workbench semantics themselves.

---

## 2. Canonical Surface Model

### 2.1 First-class command surfaces

Graphshell has five first-class command surfaces:

1. **Keyboard Commands**
   - direct keybinding path for known actions
2. **Command Palette**
   - the canonical searchable action list
3. **Radial Menu**
   - the directional command surface, optimized for gamepad and spatial selection
4. **Omnibar-Initiated Commands**
   - command execution from the search/navigation field
5. **Contextual Palette Mode**
   - the command palette filtered to the current target context

### 2.2 Retired and non-canonical surfaces

- The term `Context Menu` is retired as a first-class Graphshell concept.
- Servo's native webview context menu may still exist as an embedder surface, but it is not a Graphshell command authority.

### 2.3 Ownership model

- Graphshell owns command meaning, targeting, availability, and execution.
- `ActionRegistry` is the semantic command authority.
- UI surfaces may render and collect input, but they must not define separate command behavior.

---

## 3. Canonical Interaction Model

### 3.1 Interaction categories

Command-surface interactions fall into five semantic categories:

1. **Discover**
   - reveal available actions
2. **Filter**
   - narrow visible actions by context or search
3. **Target**
   - resolve which object, pane, or graph context the action applies to
4. **Invoke**
   - execute the selected action
5. **Dismiss**
   - close the surface without mutating app state

### 3.2 Canonical guarantees

The command system must make these user expectations reliable:

- the same action means the same thing on every command surface,
- contextual filtering changes visibility, not semantics,
- disabled actions are visible and explained rather than silently hidden,
- command failure is explicit rather than silent,
- input mode affects presentation, not command authority.

---

## 4. Normative Core

### 4.1 Semantic Command Authority

**What this domain is for**

- Keep command meaning unified across all command-entry surfaces.

**Core rule**

- Every command surface must route through the same Graphshell command authority.
- `ActionRegistry::list_actions_for_context(...)` defines what the user can see.
- `ActionRegistry::execute(...)` defines what the user actually runs.

**Who owns it**

- Graphshell command dispatcher and action registry.
- UI surfaces are render and input adapters only.

**State transitions**

- Command invocation may mutate graph state, workbench state, selection state, or settings state according to the action's semantic definition.
- Opening a command surface does not mutate semantic state by itself.

**Visual feedback**

- The active command surface must be visibly distinct.
- Target context and disabled-state explanation must be legible.

**Fallback / degraded behavior**

- If a command cannot execute, Graphshell must provide a blocked-state reason.
- Silent command no-op behavior is forbidden.

### 4.2 Command Palette

**What this domain is for**

- Provide the canonical searchable list of actions.

**Core controls**

- `Ctrl+K` or equivalent opens the global palette.
- Contextual invocation opens the same palette component in a filtered mode.
- Arrow keys move focus.
- Enter confirms.
- Escape dismisses.

**Who owns it**

- Graphshell command system owns the action set and ranking rules.
- The palette UI owns rendering, focus movement within the list, and search text capture.

**State transitions**

- Opening the palette enters a command-browse state.
- Choosing an action dispatches the selected `ActionId` against the current context.
- Dismissing the palette returns focus to the prior region.

**Visual feedback**

- Group actions by `ActionCategory`.
- Disabled actions remain visible and explain why they are unavailable.
- Global palette shows search input; contextual palette shows target-scoped results.

**Fallback / degraded behavior**

- If no actions are available, show an explicit empty state.
- If contextual resolution fails, Graphshell may fall back to global mode, but that fallback must be explicit.

### 4.3 Radial Menu

**What this domain is for**

- Provide a directional command surface optimized for gamepad and low-travel selection.

**Core controls**

- The radial menu is the default command surface in gamepad mode.
- It uses up to 8 sectors with one action per sector.
- D-pad or stick targets a sector.
- Confirm executes.
- Cancel dismisses.

**Who owns it**

- Graphshell command system owns action assignment and paging.
- The radial UI owns sector rendering and directional focus presentation.

**State transitions**

- Opening the radial menu enters a directional-selection state.
- Selecting a sector highlights a candidate action.
- Confirm dispatches that action.

**Visual feedback**

- Active sector highlight must be obvious.
- Labels must remain outside the ring and readable.
- Empty sector positions should not render placeholder arcs.

**Fallback / degraded behavior**

- If more than 8 actions are available, overflow must page predictably.
- If no valid actions exist, the radial menu must not open silently into an empty shell.

### 4.4 Keyboard Commands

**What this domain is for**

- Provide the fastest path for known actions.

**Core controls**

- Keybindings invoke semantic app actions, not widget-local shortcuts with divergent meaning.
- Keyboard commands target the active semantic context.

**Who owns it**

- Graphshell input and command layers own binding-to-action resolution.
- Widgets may capture text input where appropriate, but must not redefine global command semantics.

**State transitions**

- Valid keybinding resolution dispatches a semantic action.
- Conflicting text-entry contexts may defer a command when the focused surface owns text input.

**Visual feedback**

- Global command surfaces should expose shortcut hints.
- Command execution should visibly affect the target surface or emit explicit blocked-state feedback.

**Fallback / degraded behavior**

- If a command is unavailable in the current context, the user must receive an explicit explanation.
- Hidden suppression is forbidden.

### 4.5 Omnibar-Initiated Commands and Contextual Invocation

**What this domain is for**

- Allow command execution from search/navigation and target-scoped entry points.

**Core controls**

- The omnibar may invoke commands as well as navigation and search.
- Contextual invocation from a node, pane, edge, or canvas must use the same action system as the global palette.

**Who owns it**

- Graphshell search and command authorities jointly own query parsing and action dispatch.
- The omnibar UI owns text capture and result presentation.

**State transitions**

- Query mode determines whether the user is navigating, searching, or invoking a command.
- Executing an omnibar command dispatches the same `ActionId` that other command surfaces would invoke.

**Visual feedback**

- Result rows must clearly distinguish navigation targets from commands.
- Contextual invocations must clearly indicate the current target scope.

**Fallback / degraded behavior**

- Ambiguous queries must resolve predictably or ask for clarification through UI, not silent guesswork.
- If no command matches, the user must remain in a recoverable browse state.

### 4.6 Accessibility, Diagnostics, and Surface Boundaries

**What this domain is for**

- Keep command surfaces understandable, inspectable, and usable across input modes.

**Accessibility**

- Every command surface must be dismissible without pointer input.
- Focus return after dismissal must be deterministic.
- Command surfaces must expose actionable labels and disabled-state reasons.

**Diagnostics**

- Failed command dispatch, missing context, and blocked execution must be observable.
- Surface divergence is a correctness bug and should be diagnosable.

**Boundary rule**

- Native embedder menus may exist, but they do not define Graphshell command semantics.

---

## 5. Planned Extensions

- richer command ranking by recency and context relevance,
- configurable radial pages and sector presets,
- action previews before execution for high-impact commands,
- command aliases and user-defined shortcuts.

---

## 6. Prospective Capabilities

- voice-triggered command invocation,
- macro and multi-step command sequences,
- mod-provided command bundles with scoped enablement,
- AI-assisted command suggestions that still route through `ActionRegistry`.

---

## 7. Acceptance Criteria

1. Command meaning is unified across keyboard, palette, radial, and omnibar surfaces.
2. The command palette is the canonical list surface in both global and contextual modes.
3. The radial menu is directional, readable, and context-driven rather than hardcoded.
4. Disabled actions remain visible and explain why they are unavailable.
5. Dismissal and focus return are deterministic.
6. Blocked command execution is explicit and diagnosable.
