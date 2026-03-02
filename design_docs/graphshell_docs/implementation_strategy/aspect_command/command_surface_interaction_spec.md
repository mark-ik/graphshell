# Command Surface Interaction Spec

**Date**: 2026-02-28  
**Status**: Canonical interaction contract  
**Priority**: Pre-renderer/WGPU required

**Related**:
- `../2026-02-28_ux_contract_register.md`
- `../workbench/workbench_frame_tile_interaction_spec.md`
- `../canvas/graph_node_edge_interaction_spec.md`
- `../2026-02-24_control_ui_ux_plan.md`
- `../research/2026-02-24_interaction_and_semantic_design_schemes.md`
- `../../design/KEYBINDINGS.md`

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

### 2.1 First-class command-entry model

Graphshell has three first-class command-entry families:

1. **Keyboard Commands**
   - direct keybinding path for known actions
2. **Command Palette (canonical shell)**
   - one command surface with multiple presentation modes:
     - **Search Palette Mode** (search-first list with scope dropdown)
     - **Context Palette Mode** (right-click contextual, list-first)
     - **Radial Palette Mode** (right-click contextual, radial-first)
3. **Omnibar-Initiated Commands**
   - command execution from the search/navigation field

Context Palette Mode and Radial Palette Mode are not separate semantic systems; they are mode presentations over the same palette authority.

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

### 3.3 Two-tier command palette contract

All contextual palette modes (Context Palette + Radial Palette) use the same two-tier structure:

1. **Tier 1 — Category selector**
   - category-level action grouping by UX-relevant context
   - user-editable ordering
   - pinned categories supported
2. **Tier 2 — Command options for selected category**
   - actions in the selected category
   - user-editable ordering

Cross-mode equivalence rule:

- Context Palette Tier 1 horizontal category strip is semantically equivalent to Radial Palette Tier 1 ring.
- Context Palette Tier 2 vertical command list is semantically equivalent to Radial Palette Tier 2 option ring.
- Changing category pin/order in one mode updates the same underlying palette profile used by the other mode.

Invocation and dismissal contract:

- Right-click summons contextual command palette shell.
- The shell may open directly in Search Palette Mode, Context Palette Mode, or Radial Palette Mode per user preference/profile.
- When Search Palette Mode is opened from right-click contextual invocation, it must show a search bar with a scope dropdown (for example: current target, active pane, active graph, or workbench).
- Clicking outside current palette context dismisses the shell without command mutation.
- Palette surfaces are resizable in situ.

### 3.4 Verb-target wording policy (`#299`)

Command labels must follow explicit `Verb + Target (+ Destination/Scope when needed)` grammar.

Canonical wording rules:

1. Use explicit target nouns for destructive verbs:
   - `Delete Selected Node(s)` (graph content mutation)
   - avoid targetless `Delete` for command labels.
2. Reserve `Close` for UI presentation containers (tile/frame/window/dialog), not graph content deletion.
3. For destination-dependent verbs (`Open`, `Move`), include destination semantics:
   - `Open Node in Split`
   - `Open via Frame Route`
   - `Move Node to Active Pane`
4. Disabled action explanations must state unmet precondition and satisfy guidance.

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

### 4.2 Command Palette (Search + Context Palette Mode)

**What this domain is for**

- Provide the canonical searchable list of actions.

**Core controls**

- `Ctrl+K` or equivalent opens Search Palette Mode.
- Contextual invocation opens the same palette component in Context Palette Mode.
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
- Search Palette Mode shows search input plus scope dropdown; Context Palette Mode shows target-scoped results.
- Context Palette Mode Tier 1 is a horizontally scrollable category strip.
- Tier 1 categories can be pinned and reordered by user customization.
- Context Palette Mode Tier 2 is a vertically scrollable command list for the selected Tier 1 category.

**Fallback / degraded behavior**

- If no actions are available, show an explicit empty state.
- If contextual resolution fails, Graphshell may fall back to Search Palette Mode, but that fallback must be explicit.

### 4.3 Radial Palette Mode

**What this domain is for**

- Provide the radial presentation mode of the canonical command palette for low-travel contextual selection.

**Core controls**

- Radial Palette Mode is summonable from right-click contextual invocation.
- It supports gesture and non-gesture operation:
   - non-gesture: click/hover/select,
   - gesture: directional drag/flick.
- Tier 1 (category ring) and Tier 2 (command ring for selected category) follow §3.3.
- D-pad or stick targets a sector.
- Confirm executes.
- Cancel or click-away dismisses.

**Who owns it**

- Graphshell command system owns category/command assignment, ordering, pinning, and paging.
- The radial UI owns ring rendering, radial label behavior, hover growth, and directional focus presentation.

**State transitions**

- Opening Radial Palette Mode draws a hub-circle outline at pointer origin.
- Tier 1 category buttons appear on the periphery rail.
- Selecting a Tier 1 category activates Tier 2 option ring for that category.
- Confirming a Tier 2 option dispatches action and dismisses the palette shell.

**Visual feedback**

- Active sector highlight must be obvious.
- Tier 1/Tier 2 circular buttons sit on periphery rails and are user-repositionable along each rail.
- Default button size is compact; hovered buttons expand up to half the hub-circle radius for clickability.
- Non-hovered buttons return to compact size.
- Label behavior:
   - each label is a bounded text field radiating away from center and aligned to its button,
   - labels are hidden while not hovered,
   - on hover, labels appear and reveal overflow via gentle radial-direction scrolling,
   - on Tier 1 selection, Tier 1 radial labels collapse and selected category title is shown in the hub.
- Tier 2 option labels follow the same bounded radial text-field rule.
- Empty sector positions should not render placeholder arcs.

**Fallback / degraded behavior**

- If more than 8 actions are available, overflow must page predictably.
- If no valid actions exist, Radial Palette Mode must not open silently into an empty shell.
- If radial layout cannot satisfy non-overlap constraints at current diameters, degrade to Context Palette Mode with explicit notice.

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
- Contextual invocation from a node, pane, edge, or canvas must use the same action system as Search Palette Mode.

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

1. Command meaning is unified across keyboard, command palette modes, and omnibar surfaces.
2. Context Palette and Radial Palette are mode presentations over one canonical command palette authority.
3. Tier 1 category semantics and Tier 2 option semantics are equivalent across Context Palette and Radial Palette modes.
4. The radial palette is directional, readable, and context-driven rather than hardcoded.
5. Contextual palette shell is resizable in situ and dismisses on click-away context change.
6. Disabled actions remain visible and explain why they are unavailable.
7. Dismissal and focus return are deterministic.
8. Blocked command execution is explicit and diagnosable.


