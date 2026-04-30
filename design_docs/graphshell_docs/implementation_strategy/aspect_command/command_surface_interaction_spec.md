# Command Surface Interaction Spec

**Date**: 2026-02-28  
**Status**: Canonical interaction contract  
**Priority**: Pre-renderer/WGPU required

**Related**:
- `../2026-02-28_ux_contract_register.md`
- `../shell/2026-04-03_shell_command_bar_execution_plan.md`
- `../workbench/workbench_frame_tile_interaction_spec.md`
- `../canvas/graph_node_edge_interaction_spec.md`
- `../subsystem_ux_semantics/2026-03-04_model_boundary_control_matrix.md`
- `../aspect_control/2026-02-24_control_ui_ux_plan.md`
- `../research/2026-02-24_interaction_and_semantic_design_schemes.md`
- `../../design/KEYBINDINGS.md`

**Adopted standards** (see [standards report](../research/2026-03-04_standards_alignment_report.md) §§3.5, 3.6):

- **WCAG 2.2 Level AA** — SC 2.1.1 (keyboard, all command surfaces), SC 2.4.3 (focus order/return after dismiss), SC 1.4.3 (disabled-state explanation legibility)
- **OpenTelemetry Semantic Conventions** — diagnostics for failed dispatch, missing context, blocked execution

## Model boundary (inherits UX Contract Register §3B)

- `GraphId` = truth boundary.
- `GraphViewId` = scoped view state.
- graph-scoped Navigator hosts = chrome surfaces that may host command entry
   points while remaining above workbench arrangement semantics.
- `Navigator` = graph-backed hierarchical projection over relation families. Legacy alias: "file tree".
- workbench = arrangement boundary.

Command dispatch may target these boundaries, but command surfaces do not redefine their ownership.

## Contract template (inherits UX Contract Register §2A)

Normative command contracts use: intent, trigger, preconditions, semantic result, focus result, visual result, degradation result, owner, verification.

## Terminology lock (inherits UX Contract Register §3C)

- Tile/frame arrangement is not content hierarchy.
- Navigator is not content truth authority.
- Physics presets are not camera modes.

---

## 1. Purpose and Scope

This spec defines the interaction contract for Graphshell's command surfaces.

Command surfaces may be launched from graph-scoped Navigator chrome,
workbench-scoped Navigator context, or direct pointer/keyboard invocation, but
those launch locations do not become semantic owners of the graph/workbench
targets they address.

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

### 2.1 First-class command-entry model (revised 2026-04-29)

Graphshell has three first-class command-entry families:

1. **Keyboard Commands**
   - direct keybinding path for known actions
2. **Command surfaces** — **two**:
   - **Command Palette** (Modal overlay; flat ranked list; fuzzy-search input is the discovery mechanism; Zed/VSCode-shaped)
   - **Context Menu** (right-click on an interactable target; flat list of available actions for that target)
3. **Omnibar-Initiated Commands**
   - command execution from the search/navigation field

Both command surfaces source actions from the same `ActionRegistry`; both
dispatch via the same `ActionId` execution path. They differ only in
trigger and rendering.

**Retired (2026-04-29 simplification):**

- **Search Palette Mode + Context Palette Mode distinction** — collapsed
  into the single Command Palette. Search is the palette's input
  affordance, not a separate mode. Right-click is the Context Menu, not
  a contextual mode of the palette.
- **Two-tier (Tier 1 categories + Tier 2 options) rendering** — replaced
  by flat ranked lists in both surfaces. Categories appear only as
  inline badges on rows for breadth visibility, never as a separate
  selector tier. See §3.3.
- **Cross-mode equivalence rule** (Tier 1 strip = Tier 1 ring) — moot;
  there is no Tier 1.
- **Radial Palette Mode** — deferred indefinitely. Was originally
  gamepad-oriented; if gamepad input lands later as part of the
  input-subsystem rework, a radial surface can be reintroduced as a
  third command-dispatch route with its own design pass. The geometry
  research in [`radial_menu_geometry_and_overflow_spec.md`](radial_menu_geometry_and_overflow_spec.md)
  is preserved as a future reference. See §5 Planned Extensions.

### 2.2 Retired and non-canonical surfaces

- Servo's native webview context menu may still exist as an embedder
  surface, but it is not a Graphshell command authority.
- The earlier retirement of "Context Menu" as a Graphshell concept (in
  favor of "Context Palette") is itself reversed by the 2026-04-29
  simplification: **Context Menu** is the canonical name again, since
  it is no longer a "mode" of the palette but a separate surface with
  its own trigger and rendering.

### 2.2A Naming note (revised 2026-04-29)

To avoid ambiguity in implementation and UI copy:

- **Command Palette** = the Modal flat-list surface invoked by `Ctrl+P`,
  `F2`, or the CommandBar trigger button. Fuzzy-search is its
  discovery mechanism.
- **Context Menu** = the right-click contextual flat-list surface
  scoped to the right-click target.
- **Radial Menu** = deferred (see §5).
- "Context Palette" is **retired as a name** — it is now just the
  Context Menu.
- "Search Palette Mode" / "Context Palette Mode" / "Radial Palette
  Mode" are all retired names; do not reintroduce them in new code or
  docs.

`Interaction Menu` should not be used as a canonical name; it blurs
distinctions that this revision tightens.

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
- input mode affects presentation, not command authority,
- command applicability is determined from the full resolved selection set, not
  by inventing a hidden primary target.

### 3.3 Flat-list command surface contract (revised 2026-04-29)

Both command surfaces (Command Palette + Context Menu) render a **flat
ranked list of available actions**. There is no Tier 1 / Tier 2
selector tier; categories appear only as inline badges on rows for
breadth visibility.

Common contract:

- Action data sourced from `ActionRegistry`.
- Disabled actions render with explicit reasons (per §4.1); never
  hidden silently.
- Verb-target wording per §3.4.
- Selection-set availability gating per §4.1.
- Dispatch via single `ActionId` execution path.
- Pinned actions and recency ordering supported (palette only;
  context menu is small and contextual, no pinning).

Surface-specific differences:

| | **Command Palette** | **Context Menu** |
|---|---|---|
| Trigger | `Ctrl+P` / `F2` / trigger button / programmatic | Right-click on interactable target |
| Rendering | `Modal` overlay with `text_input` filter + scrollable flat list | Anchored `ContextMenu` flat list |
| Scope | global (default) or scoped via context-fallback origin | scoped to right-click target |
| Filter | fuzzy search; empty-query shows pinned + recent + canonical default | no filter; flat available list for the target |
| Mode-switch | "Search commands…" footer entry in Context Menu opens the Palette pre-scoped to the target | none (Palette doesn't switch back) |
| Resize | Modal max-width + max-height; not user-resizable (Stage F polish question) | Not resizable; sized to content |
| Dismiss | Escape, click outside, action selected | Escape, click outside, action selected |

Right-click on any interactable target (graph node, edge, tile,
Pane chrome, Frame border, Navigator row, swatch, base layer) opens
the Context Menu scoped to that target. Right-click never opens the
Command Palette directly; the Context Menu's "Search commands…"
fallback is the keyboard-driven escape into the full action set.

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

**What this domain is for**: - Keep command meaning unified across all command-entry surfaces.

**Core rule**: - Every command surface must route through the same Graphshell command authority.
- `ActionRegistry::list_actions_for_context(...)` defines what the user can see.
- `ActionRegistry::execute(...)` defines what the user actually runs.
- The resolved command target is the current selection set when one exists.
- The selection set may be mixed (`Node`, `Tile`, `Frame`, `Edge`, or other
  interactable graph/workbench objects).
- A command is available only if it validly applies to every object in the
  resolved selection set.
- When invoked, the command applies to all selected objects.
- Silent fallback to a hidden primary target is forbidden.

**Who owns it**: - Graphshell command dispatcher and action registry.
- UI surfaces are render and input adapters only.

**State transitions**: - Command invocation may mutate graph state, workbench state, selection state, or settings state according to the action's semantic definition.
- Opening a command surface does not mutate semantic state by itself.

**Visual feedback**: - The active command surface must be visibly distinct.
- Target context and disabled-state explanation must be legible.

**Fallback / degraded behavior**: - If a command cannot execute, Graphshell must provide a blocked-state reason.
- Silent command no-op behavior is forbidden.

### 4.2 Command Palette (revised 2026-04-29)

**What this domain is for**: - Provide the canonical searchable list of actions in a single Modal surface.

**Core controls**: - `Ctrl+P` (canonical, Zed/VSCode-shaped) opens the palette.
- `F2` (alternate) toggles the same palette.
- The CommandBar trigger button opens the palette.
- Arrow keys move focus within the result list.
- Enter dispatches the focused action.
- Escape dismisses and must not trigger unrelated global mode toggles on the same press.

**Who owns it**: - Graphshell command system owns the action set, availability rules, and ranking.
- The palette UI owns rendering, focus movement within the list, and search text capture.

**State transitions**: - Opening the palette enters a command-browse state with the input focused.
- Empty query shows pinned + recently-used + canonical-default available actions.
- Non-empty query shows fuzzy-match ranked results.
- Choosing an action dispatches the selected `ActionId` against the current selection set.
- Dismissing the palette returns focus to the prior region.

**Visual feedback**: - Render a flat ranked list (no category tier).
- Each row shows: action label (verb-target wording), optional secondary text, optional inline category badge, optional right-aligned keybinding.
- Disabled actions remain visible with reduced opacity and explicit disabled-reason text in the footer when focused.
- Empty result set shows an explicit empty state ("No commands match…").

**Fallback / degraded behavior**: - If no actions are available in the current scope, show an explicit empty state.
- If a Context Menu's "Search commands…" fallback opens the palette, scope is preset to the right-click target.

### 4.3 Context Menu (revised 2026-04-29)

**What this domain is for**: - Provide a right-click-anchored flat list of actions available on the right-click target.

**Core controls**: - Right-click on any interactable target opens the Context Menu scoped to that target.
- Arrow keys move focus.
- Enter dispatches.
- Escape dismisses.

**Who owns it**: - Graphshell command system owns target-scoped action availability.
- The Context Menu UI owns rendering and dismissal.

**State transitions**: - Opening the Context Menu enters a command-browse state focused on the menu.
- Choosing an action dispatches the `ActionId` against the right-click target's selection set.
- "Search commands…" footer entry opens the Command Palette with `PaletteOrigin::ContextFallback`, preserving the right-click target as scope.
- Dismissing the menu returns focus to the prior region.

**Visual feedback**: - Render a flat list of actions; no category tier.
- Each row shows: action label (verb-target wording), optional right-aligned keybinding.
- Disabled actions render with reduced opacity and explicit reason on hover/focus.
- Footer separator + "Search commands…" entry as the keyboard escape into the full action set.

**Fallback / degraded behavior**: - If no actions are available for the right-click target, show only the "Search commands…" footer.
- If a destructive action is selected, route through `ConfirmDialog` per the host spec; do not dispatch silently.

### 4.4 Keyboard Commands

**What this domain is for**: - Provide the fastest path for known actions.

**Core controls**: - Keybindings invoke semantic app actions, not widget-local shortcuts with divergent meaning.
- Keyboard commands target the active semantic context.

**Who owns it**: - Graphshell input and command layers own binding-to-action resolution.
- Widgets may capture text input where appropriate, but must not redefine global command semantics.

**State transitions**: - Valid keybinding resolution dispatches a semantic action.
- Conflicting text-entry contexts may defer a command when the focused surface owns text input.

**Visual feedback**: - Global command surfaces should expose shortcut hints.
- Command execution should visibly affect the target surface or emit explicit blocked-state feedback.

**Fallback / degraded behavior**: - If a command is unavailable in the current context, the user must receive an explicit explanation.
- Hidden suppression is forbidden.

### 4.5 Omnibar (URL entry + breadcrumb display) — revised 2026-04-29

**What this domain is for**: - Provide URL/address entry and Navigator-projected breadcrumb display in a single chrome surface. Per the 2026-04-29 simplification, the omnibar is **not** a command-entry surface; commands go through the Command Palette (§4.2). Graph search by title/tag is **not** an omnibar role; it goes through the **Node Finder** (a separate Modal surface, `Ctrl+P` canonical). The omnibar's submission semantics are URL/address-shaped only.

**Core controls**: - `Ctrl+L` (canonical) focuses the omnibar in Input mode for URL entry.
- Submission (Enter) opens-or-activates a node by canonical address; if the typed text doesn't resolve as an address, omnibar parsing routes the entry to either the Node Finder (for title/tag-shaped queries) or to a default-search behavior per user preference.
- The omnibar exposes URL completions sourced from history-by-URL and bookmark-URLs providers.

**Who owns it**: - The omnibar UI is Shell-owned; URL completions are sourced from history and bookmark providers under `ControlPanel` supervision.
- The Navigator owns the breadcrumb projection (read-only at this seam) per [SHELL.md §6](../shell/SHELL.md).

**State transitions**: - Display mode shows the breadcrumb (current focused-node address path from Navigator).
- Input mode shows a `text_input` with URL-shaped completion list.
- Submission resolves the entered text via address parsing → opens-or-activates the resulting node, or routes the user to the Node Finder if the entry is non-URL-shaped.
- Dismissing input returns the omnibar to Display mode without mutation.

**Canonical parity contract (normative)**: - The omnibar **does not** invoke commands. Command invocation is the Command Palette's responsibility (§4.2). If an action exists for "open node by URL," that action is dispatched through the Palette; the omnibar does not have a parallel command-row path.
- Address resolution is deterministic: a URL/address in the typed text always wins over a fuzzy-match interpretation. Ambiguous text routes to the Node Finder, not silently retargeted.
- The omnibar **does not** have its own `ActionId` semantic surface; it has one submission verb ("open by address").

**Visual feedback**: - Display mode shows the breadcrumb token chain (Navigator-projected).
- Input mode shows the text-input cursor + completion list.
- URL completions render with source badges (history / bookmark).
- Failed address resolution (404, invalid URL, unreachable) surfaces via toast plus an Activity Log entry.

**Focus ownership and identification (normative)**: - Omnibar text field must expose explicit focus ownership state even when caret rendering is unavailable (focus ring, field highlight, focus badge, or equivalent deterministic indicator).
- Focus must be applied to the omnibar only through explicit user selection actions (pointer selection, `Ctrl+L`/platform equivalent, or explicit `OmnibarFocus` command intent).
- On app/frame open, command-surface open, and Context Menu summon paths, focus must not default to omnibar.
- Keyboard command handling must remain owned by the currently focused semantic region unless omnibar focus has been explicitly requested.

**Fallback / degraded behavior**: - Ambiguous queries route to the Node Finder; the user is not silently retargeted.
- If a typed URL does not resolve, the user remains in Input mode with an explicit error state (recoverable, no mutation).

### 4.5A Node Finder — added 2026-04-29

**What this domain is for**: - Provide a fuzzy graph-node search surface for "open this node by title / tag / address / content" intents, separate from the omnibar (which is URL-shaped) and the Command Palette (which is action-shaped).

**Core controls**: - `Ctrl+P` (canonical, Zed/VSCode-shaped) opens a Modal surface with a text input and a fuzzy-ranked list of graph nodes.
- Enter activates the focused node; arrow keys navigate; Escape dismisses.
- Activation routes the selected node to its destination Pane per a user-configurable rule (active Pane / new Pane / replace focused Pane).

**Who owns it**: - The Node Finder UI is Shell-owned; ranking is owned by `graphshell-runtime`'s graph index.
- The result list is a derivation over current graph truth; no node-finder state aliases graph state.

**State transitions**: - Opening enters a graph-search-browse state with the input focused.
- Empty query shows recently-active nodes ranked by recency.
- Non-empty query shows fuzzy-match results across (title, tag, address, content snapshot).
- Activation dispatches a `WorkbenchIntent::OpenNode { node_key, destination }`.
- Dismissal restores focus to the prior region.

**Visual feedback**: - Each result row shows: node title (or address if no title), node-type badge (Web / File / Tool / Internal), match-source badge (Title / Tag / URL / Content), optional content-match snippet.
- Empty result set shows explicit empty state.

**Fallback / degraded behavior**: - If the graph index is unavailable, show explicit "Index unavailable" empty state; do not silently fall back to URL-only matching.
- A "Open as URL…" footer entry opens the omnibar with the typed text pre-filled, when the user wants to interpret their query as a URL after all.

### 4.6 Accessibility, Diagnostics, and Surface Boundaries

**What this domain is for**: - Keep command surfaces understandable, inspectable, and usable across input modes.

**Accessibility**: - Every command surface must be dismissible without pointer input.
- Focus return after dismissal must be deterministic.
- Command surfaces must expose actionable labels and disabled-state reasons.

**Diagnostics**: - Failed command dispatch, missing context, and blocked execution must be observable.
- Surface divergence is a correctness bug and should be diagnosable.

**Boundary rule**: - Native embedder menus may exist, but they do not define Graphshell command semantics.

---

## 5. Planned Extensions

- richer command ranking by recency and context relevance,
- action previews before execution for high-impact commands,
- command aliases and user-defined shortcuts,
- per-domain Command settings pages: pinned-action order, command aliases, keybinding customization — exposed via the **Keybindings** and **General** settings categories in `aspect_control/settings_and_control_surfaces_spec.md §4.2`,
- **Radial Menu reintroduction** — deferred from canonical surfaces in the 2026-04-29 simplification; if gamepad input lands as part of the input-subsystem rework, a third command-dispatch surface (radial geometry per [`radial_menu_geometry_and_overflow_spec.md`](radial_menu_geometry_and_overflow_spec.md)) can be reintroduced with its own design pass. Reintroduction would source the same `ActionRegistry` action set; only the rendering and gesture model differ.

---

## 6. Prospective Capabilities

- voice-triggered command invocation,
- macro and multi-step command sequences,
- mod-provided command bundles with scoped enablement,
- AI-assisted command suggestions that still route through `ActionRegistry`.

---

## 7. Acceptance Criteria (revised 2026-04-29)

1. Command meaning is unified across keyboard, Command Palette, Context Menu, and omnibar surfaces.
2. Command Palette and Context Menu are two separate surfaces sourcing actions from the same `ActionRegistry`; both render flat ranked lists.
3. The Command Palette's fuzzy-search input is the discovery mechanism; there is no separate Search Mode.
4. Right-click on any interactable target opens the Context Menu scoped to that target; the "Search commands…" footer entry is the keyboard escape into the full action set.
5. Disabled actions remain visible and explain why they are unavailable on both surfaces.
6. Dismissal and focus return are deterministic on both surfaces.
7. Blocked command execution is explicit and diagnosable.
8. Destructive actions route through `ConfirmDialog` on both surfaces; dispatch never happens silently.
9. Omnibar command rows execute the same `ActionId` semantics and target-scope resolution as keyboard / Command Palette / Context Menu surfaces.
10. Omnibar and search fields do not capture keyboard commands by default; focus is explicit and visibly identifiable.
11. Radial Menu, two-tier rendering, and Search/Context palette mode distinction do not appear in canonical surfaces (they are retired per §2.1 / §3.3). Reintroduction requires an explicit design pass per §5.
