<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Command and Keybinding Accounting Plan

**Date**: 2026-03-10
**Status**: Planning — post-Sector-H/G execution
**Scope**: Gap analysis of command/keybinding coverage; usability path after structural work
**Priority**: After Sectors H and G (signal infrastructure, mod/agent/theme runtime)

**Related**:
- `KEYBINDINGS.md`
- `command_semantics_matrix.md` (D1)
- `../implementation_strategy/aspect_command/command_surface_interaction_spec.md`
- `../implementation_strategy/aspect_command/ASPECT_COMMAND.md`
- `../implementation_strategy/canvas/graph_node_edge_interaction_spec.md`
- `../implementation_strategy/system/register/2026-03-08_sector_e_workbench_surface_plan.md`

---

## 1. Context

The structural work on registries, authority boundaries, and signal infrastructure
now provides real backing for a more complete command surface. This plan captures
where the current command/keybinding accounting is incomplete and what needs to
be done to close the usability gap. It is a planning document only — no
implementation until Sectors H and G are complete.

---

## 2. Current State: What Exists

### 2.1 Command Semantics Matrix (D1)

`command_semantics_matrix.md` is the canonical authority. It documents ~30 action
IDs with surface mappings, preconditions, side effects, and keyboard shortcuts.

**Keyboard shortcuts assigned in the matrix (9 actions):**

| Action ID | Shortcut |
|---|---|
| `NodeNew` | `N` |
| `NodePinToggle` | `L` |
| `NodePinSelected` | `I` |
| `NodeUnpinSelected` | `U` |
| `NodeDelete` | `Delete` |
| `EdgeConnectPair` | `G` |
| `EdgeConnectBoth` | `Shift+G` |
| `EdgeRemoveUser` | `Alt+G` |
| `GraphTogglePhysics` | `T` |
| `GraphPhysicsConfig` | `P` |
| `GraphCommandPalette` | `F2` |
| `PersistUndo` | `Ctrl+Z` |
| `PersistRedo` | `Ctrl+Y` |

**Actions in the matrix with no keyboard shortcut (17 actions):**

`NodeNewAsTab`, `NodeChooseFrame`, `NodeAddToFrame`, `NodeAddConnectedToFrame`,
`NodeOpenFrame`, `NodeOpenNeighbors`, `NodeOpenConnected`, `NodeOpenSplit`,
`NodeDetachToSplit`, `NodeMoveToActivePane`, `NodeCopyUrl`, `NodeCopyTitle`,
`GraphFit`, `PersistSaveSnapshot`, `PersistRestoreSession`, `PersistSaveGraph`,
`PersistRestoreLatestGraph`, `PersistOpenHub`.

### 2.2 KEYBINDINGS.md

`KEYBINDINGS.md` documents keybindings that are **not routed through ActionRegistry**
and therefore absent from the matrix. These are currently handled by direct
`GraphIntent` / keyboard dispatch:

| Keybinding | Behavior | Matrix coverage |
|---|---|---|
| `Home` / `Esc` | Toggle Graph/Detail view | Not in matrix |
| `R` | Reheat physics | Not in matrix |
| `+` / `-` / `0` | Zoom in/out/reset | Not in matrix |
| `C` | Toggle position-fit lock | Not in matrix |
| `Z` | Toggle zoom-fit lock | Not in matrix |
| `F9` | Open Camera Controls in Settings > Physics | Not in matrix |
| `W/A/S/D` / `Arrow Keys` | Pan graph camera | Not in matrix |
| `Ctrl+Click` | Multi-select | Not in matrix |
| `Right+Drag` | Lasso select (replace) | Not in matrix |
| `Right+Shift+Drag` | Lasso add to selection | Not in matrix |
| `Right+Ctrl+Drag` | Lasso add to selection | Not in matrix |
| `Right+Alt+Drag` | Lasso toggle selection | Not in matrix |
| `Ctrl+F` | Graph search | Not in matrix |
| `Ctrl+Z (hold)` | Undo preview indicator | Not in matrix |
| `Back` / `Forward` | Traversal navigation | Not in matrix |
| `F1` / `?` | Toggle keyboard shortcut help | Not in matrix |
| `F3` | Toggle radial palette mode | Not in matrix |

### 2.3 ActionRegistry — Registered Action IDs

The runtime `ActionRegistry` (action.rs) currently registers:

- `omnibox:node_search`
- `graph:view_submit`
- `detail:view_submit`
- `graph:node_open`
- `graph:node_close`
- `graph:edge_create`
- `graph:set_physics_profile`
- `graph:navigate_back`
- `graph:navigate_forward`
- `graph:select_node`
- `graph:deselect_all`
- `workbench:split_horizontal`
- `workbench:split_vertical`
- `workbench:close_pane`
- `workbench:command_palette_open`
- `workbench:settings_open`
- `verse:pair_device`
- `verse:sync_now`
- `verse:share_workspace`
- `verse:forget_device`

That is **20 registered action IDs** in the runtime, vs **30 action IDs** in the D1
matrix. The matrix documents semantics for the 30; the runtime only backs 20.

---

## 3. Gap Analysis

### 3.1 Matrix-to-Registry Delta

Actions documented in D1 that have **no matching registered action ID**:

| Matrix Action ID | Description | Missing registry ID |
|---|---|---|
| `NodeNew` | Create node near center | `graph:node_new` |
| `NodeNewAsTab` | Create node as new tab | `graph:node_new_as_tab` |
| `NodePinToggle` | Toggle pin on primary node | `graph:node_pin_toggle` |
| `NodePinSelected` | Pin selected node(s) | `graph:node_pin_selected` |
| `NodeUnpinSelected` | Unpin selected node(s) | `graph:node_unpin_selected` |
| `NodeDelete` | Delete selected node(s) | `graph:node_delete` |
| `NodeChooseFrame` | Choose frame | `graph:node_choose_frame` |
| `NodeAddToFrame` | Add to frame | `graph:node_add_to_frame` |
| `NodeAddConnectedToFrame` | Add connected to frame | `graph:node_add_connected_to_frame` |
| `NodeOpenFrame` | Open via frame route | `graph:node_open_frame` |
| `NodeOpenNeighbors` | Open with neighbors | `graph:node_open_neighbors` |
| `NodeOpenConnected` | Open with connected | `graph:node_open_connected` |
| `NodeOpenSplit` | Open node in split | `graph:node_open_split` |
| `NodeDetachToSplit` | Detach focused to split | `workbench:detach_to_split` |
| `NodeMoveToActivePane` | Move node to active pane | `workbench:move_node_to_pane` |
| `NodeCopyUrl` | Copy node URL | `graph:node_copy_url` |
| `NodeCopyTitle` | Copy node title | `graph:node_copy_title` |
| `EdgeConnectPair` | Connect source → target | `graph:edge_connect_pair` |
| `EdgeConnectBoth` | Connect both directions | `graph:edge_connect_both` |
| `EdgeRemoveUser` | Remove user edge | `graph:edge_remove_user` |
| `GraphFit` | Fit graph to screen | `graph:fit` |
| `GraphTogglePhysics` | Toggle physics simulation | `graph:toggle_physics` |
| `PersistUndo` | Undo | `workbench:undo` |
| `PersistRedo` | Redo | `workbench:redo` |
| `PersistSaveSnapshot` | Save frame snapshot | `workbench:save_snapshot` |
| `PersistRestoreSession` | Restore session frame | `workbench:restore_session` |
| `PersistSaveGraph` | Save graph snapshot | `workbench:save_graph` |
| `PersistRestoreLatestGraph` | Restore latest graph | `workbench:restore_graph` |
| `PersistOpenHub` | Open persistence hub | `workbench:open_persistence_hub` |

The matrix describes these as Palette/Radial-exposed actions but they are wired
today through direct graph intent dispatch, not through `ActionRegistry`. The matrix
is the semantic spec — the registry is what needs to catch up.

### 3.2 KEYBINDINGS.md Actions Not in Matrix

These keybindings have no semantic representation in D1 at all. Many are
lower-level (camera/input primitives) and may not belong in the matrix; some should.

**Should be in the matrix** (command-class actions, palette-worthy):

| Keybinding | Action concept | Proposed action ID |
|---|---|---|
| `Home` / `Esc` | Toggle graph/detail view | `graph:toggle_detail_view` |
| `Ctrl+F` | Graph search | `graph:search_open` |
| `F1` / `?` | Toggle keyboard shortcut help | `workbench:help_open` |
| `F3` | Toggle radial palette mode | `workbench:radial_palette_toggle` |
| `R` | Reheat physics | `graph:reheat_physics` |
| `C` | Toggle position-fit lock | `graph:toggle_position_fit_lock` |
| `Z` | Toggle zoom-fit lock | `graph:toggle_zoom_fit_lock` |
| `+` / `-` / `0` | Zoom in/out/reset | `graph:zoom_in`, `graph:zoom_out`, `graph:zoom_reset` |
| `Ctrl+Shift+Delete` | Clear graph | `graph:clear` |

**Input primitives — not action-class** (should stay in KEYBINDINGS.md only):

- `W/A/S/D` / `Arrow Keys` — camera pan (configurable input mode, not a semantic action)
- `Ctrl+Click` — multi-select modifier
- `Right+Drag` variants — lasso select modifiers
- `Ctrl+Z (hold)` — undo preview gesture (hold-key behavior, not a discrete command)

### 3.3 Absent Command Classes

Neither the matrix nor the registry documents any commands for:

1. **Workflow activation** — `WorkflowRegistry` is now real with `workflow:default`,
   `workflow:research`, `workflow:reading`. There are no action IDs to activate them.
   Proposed: `workbench:activate_workflow` (parameterized by workflow ID).

2. **Lens/profile switching** — `LensRegistry` and `CanvasRegistry` are either
   landed or in-progress. No user-facing command exists for switching active lens
   or canvas profile.

3. **Traversal navigation** — `graph:navigate_back` / `graph:navigate_forward` are
   in the registry, but they are not in the D1 matrix. The matrix should include them
   along with `Back`/`Forward` keybinding entries in KEYBINDINGS.md.

4. **History Manager** — opening the History Manager tool pane has no action ID.
   Proposed: `workbench:open_history_manager`.

5. **Graph camera control** — `F9` opens Camera Controls in Settings > Physics,
   but this isn't in the matrix and is only partially described in KEYBINDINGS.md.

### 3.4 Context-per-Entity Palette Gap

The command surface spec (§3.3) requires that summon target context shapes the
first-category priority in the palette. But the matrix's `Surfaces` column does
not distinguish which actions are available per entity context (node summon vs
edge summon vs canvas summon vs pane summon). This means:

- No formal specification exists for "what appears when you right-click a node" vs
  "what appears when you right-click on empty canvas."
- Edge context is entirely absent from the action model: right-clicking an edge
  has no documented palette category, no action IDs, and no disabled-state spec.

This is the §4.3 gap noted in the previous analysis: `graph_node_edge_interaction_spec.md`
§4.3 covers hover/click/double-click for traversal inspection only. It has no
normative edge-management command map, and no context-palette category for edges.

---

## 4. Recommended Work: Post-Sector-H/G

These are sequenced by dependency and impact, not by calendar.

### Phase C1 — Matrix completion (registry alignment)

**Scope**: Bring D1 matrix and ActionRegistry into alignment.

1. **C1.1 — Add matrix-documented actions to registry**

   Register the ~29 missing action IDs from §3.1. Execution handlers can remain
   thin adapters to the existing direct-dispatch paths while the registry expands.
   The goal is that `ActionRegistry::list_actions_for_context(...)` returns a
   correct, complete action set — not that every handler is rewritten.

   Implementation note (2026-03-10): this step required broadening
   `ActionRegistry` dispatch beyond reducer intents and workbench intents.
   A meaningful subset of matrix actions (`node_copy_*`, frame/session restore,
   graph restore/save, connected-open flows) are canonically backed by queued
   `AppCommand`s. Treat that as part of the intended semantic command surface,
   not as an exception to be hidden behind direct UI calls.

   Status (2026-03-10): landed. The matrix-documented action IDs from §3.1 are
   now registered as thin adapters over existing reducer/workbench/app-command
   paths. Remaining work is matrix/keybinding specification completion in C1.2+
   rather than additional registry-coverage catch-up for that table.

   Blocking note: `workbench:split_*` / `workbench:close_pane` are gated by stable
   `PaneId` on all pane variants (PLANNING_REGISTER Structural Groundwork Guardrail).

2. **C1.2 — Add KEYBINDINGS.md "command-class" actions to matrix**

   Add action IDs for the 9 items in §3.2's "should be in the matrix" list.
   Each needs a matrix row with surface coverage, preconditions, and shortcut.

   Status (2026-03-10): landed. `command_semantics_matrix.md` now includes the
   command-class keybinding rows for detail-view toggle, graph search, help,
   radial palette, physics reheat, fit-lock toggles, zoom commands, and clear graph.

3. **C1.3 — Add traversal navigation actions to matrix**

   `graph:navigate_back` / `graph:navigate_forward` are already registered.
   Add them to the matrix and add `Back`/`Forward` to their shortcut column.
   KEYBINDINGS.md §History Scope Semantics already documents this behavior.

   Status (2026-03-10): landed. The matrix and `KEYBINDINGS.md` now both account
   for `Back` / `Forward` as semantic traversal commands.

### Phase C2 — Context-per-entity palette specification

**Scope**: Define what each summon context shows.

1. **C2.1 — Add context-category map to matrix**

   Extend the D1 matrix with a `Contexts` column or a companion table that
   maps each action ID to the entity contexts in which it appears:
   `(node, edge, canvas_background, pane_header, tool_pane)`.

   This is required before the palette can be context-aware per the spec.

2. **C2.2 — Edge context actions**

   Define the action set for edge context. At minimum:
   - `EdgeRemoveUser` (already in matrix, needs edge-context flag)
   - `EdgeConnectPair` / `EdgeConnectBoth` (same)
   - future: label editing surface when label carrier is wired

   Add a brief edge-management command section to
   `graph_node_edge_interaction_spec.md §4.3`.

### Phase C3 — Workflow and session mode commands

**Scope**: Expose WorkflowRegistry through the command surface.

1. **C3.1 — `workbench:activate_workflow`**

   Parameterized action that calls `WorkflowRegistry::activate_workflow(id)`.
   Built-in workflows (`workflow:default`, `workflow:research`, `workflow:reading`)
   appear in the palette as distinct command rows (one per workflow ID).

   Blocked by: Sectors H and G (signal/mod runtime dependencies for full
   workflow activation side-effects).

2. **C3.2 — History Manager open command**

   `workbench:open_history_manager` action that opens the history tool pane.
   Simple adapter to existing tool-pane routing.

### Phase C4 — Keybindings settings surface

**Scope**: Make keybindings discoverable and configurable.

Current state: keybindings are documented in KEYBINDINGS.md and hardcoded in the
input dispatch layer. The `F1` / `?` help shortcut documents them statically.
The command surface spec §5 ("Planned Extensions") includes user-defined shortcuts
and the Settings spec (`aspect_control/settings_and_control_surfaces_spec.md §4.2`)
includes a **Keybindings** settings category.

Work needed:
- `workbench:help_open` action (already has `F1` keybinding, needs registry entry)
- Settings > Keybindings page reads from and writes to the rebinding store
- `ActionRegistry` must expose shortcut hints for display in the help overlay
  and in palette action rows (shortcut badge column)

This phase is deliberately last — it depends on the registry having a complete
action set (C1) and the settings surface being stable.

---

## 5. Matrix update needed now (pre-Sector-H/G)

These can be done immediately without implementation work:

1. **Add `graph:navigate_back` / `graph:navigate_forward` rows** to D1 matrix
   (they're registered, just not documented in the matrix).
2. **Add `Contexts` column** to the matrix — even if empty/partial, it establishes
   the slot and makes the gap legible.
3. **Add edge context note** to `graph_node_edge_interaction_spec.md §4.3`.

These are doc-only and close a visible spec/reality drift. They should happen
before the next registry or command work, not after.

---

## 6. Default Keybinding Design

### 6.1 Comparable application conventions

Graphshell occupies two lineages simultaneously: **browser** and **spatial graph editor**.
These lineages disagree on a few conventions but agree on more than is obvious.
The defaults below are designed to match the dominant convention in each category,
and to avoid the most common muscle-memory conflicts.

Key reference applications:

- **Browser** (Chrome/Firefox/Vivaldi): `Ctrl+T/W/L`, `Alt+Left/Right`, `Ctrl+F`, `Ctrl+K`, `Ctrl+,`
- **Graph/canvas tools** (Figma, Miro, draw.io): `F` for fit, `Delete`/`Backspace` for delete, drag-to-connect
- **Node editors** (Blender Node, Houdini): `Tab` for mode toggle, `G` for grab (conflict), `F` for fit
- **Command launchers** (VS Code, Notion, Linear, Slack): `Ctrl+K` for palette — industry standard
- **Spatial tools** generally: `Space` or `F` for fit, `WASD` or arrows for pan

### 6.2 Action-by-action analysis

**`F2` → `Ctrl+K` for command palette** (change recommended)

`F2` = rename in VS Code, Windows Explorer, Excel — users pressing `F2` on a node
expect rename, not a command palette. `Ctrl+K` is the dominant industry default for
command launchers. `F2` should be freed for a future node-rename action.
Retain `F2` as a configurable alias only.

**`Home` / `Esc` → `Home` for view toggle** (change recommended)

`Esc` is universally "cancel / dismiss current overlay." Using it to toggle a
persistent mode creates a layered semantic collision: users who press `Esc` to dismiss
a palette will also inadvertently toggle the view. `Esc` should be reserved for
dismiss-only.

`Tab` for mode-switching is established in Blender and yEd, but is incompatible
with accessibility requirements: `Tab` is the primary focus-traversal key for
keyboard-only and assistive-technology users, and must not be repurposed for a
persistent mode-switch (see §6.5). `Home` alone is the recommended default — it
has a clear spatial metaphor ("return to home view"), is not a focus-navigation key,
and has no conflicts in graph canvas context.

**`F3` → reassign radial palette** (change recommended)

`F3` = find-next in Firefox, Chrome, VS Code, Word, and most text editors.
Users trained on any browser will press `F3` to cycle search results and get
a radial menu instead. Proposed: `Ctrl+Space` for radial palette (launcher-adjacent
convention, fast single-hand chord). `F3` should alias graph search or be configurable.

**`N` for new node** — keep. No browser-level bare-`N` conflict. Mnemonic.

**`Delete` / add `Backspace`** — add `Backspace` as second default.
Both keys feel correct in graph tools (Figma, draw.io, yEd all accept either).

**`G` / `Shift+G` / `Alt+G` for edge ops** — keep as primary default.
Blender uses `G` for grab/move — a conflict for 3D users, but Graphshell is closer
to a browser+graph tool than a 3D editor, so this is acceptable. The chord family
(`Shift+G`, `Alt+G`) is internally consistent. Most likely remapping candidate for
power users; should be clearly surfaced in Settings > Keybindings.

**`P` for physics settings → `Shift+T`** (change recommended)

`P` freed for pin operations (see below). Physics-related actions cluster on `T`
(toggle physics). `Shift+T` for the physics settings panel is a natural extension.

**`L` / `I` / `U` for pin → `Shift+P` / `P` / `Alt+P`** (change recommended)

`P` = pin is strongly mnemonic. `I` for pin and `U` for unpin have no mnemonic
justification. `L` is arbitrary. The full cluster: `P` = pin selected, `Alt+P` =
unpin selected, `Shift+P` = pin-toggle on primary. Requires freeing `P` from physics.

**`T` for physics toggle** — keep. Fast, no conflicts, acceptable for graph tools.

**`R` for reheat** — keep. Fast, no conflicts.

**`C` for position-fit lock** — keep with caution.
`Ctrl+C` = copy is deep muscle memory; bare `C` without modifier is safe, but
accidental fast-key presses (`C` instead of `Ctrl+C`) are a real risk. Document as
a high-remapping-candidate in the settings UI. No immediately better alternative
without losing discoverability.

**`Z` for zoom-fit lock** — keep with caution.
Same concern as `C` above: `Ctrl+Z` = undo, and pressing `Z` alone while
trying `Ctrl+Z` is easy to do. Same recommendation: document as remapping candidate.

**`+` / `-` / `0` for zoom** — keep. Universal standard.

**`Ctrl+F` for search** — keep. Universal standard.

**`F` for fit-to-screen** (add — currently unassigned)

`F` for "fit/frame to screen" is standard in Figma, Blender, Inkscape, draw.io.
Currently `GraphFit` has no keyboard shortcut. This should be the default.

**`Ctrl+Z` / `Ctrl+Y` — keep. Add `Ctrl+Shift+Z` as redo alias.**

`Ctrl+Shift+Z` for redo is the macOS and Linux standard. Both should be defaults.

**Workbench shortcuts** (add — currently palette-only)

These follow VS Code conventions which are the dominant multi-pane editor standard:

- `Ctrl+\` — split horizontal (VS Code, JetBrains)
- `Ctrl+Shift+\` — split vertical
- `Ctrl+W` — close pane (browser + VS Code standard)
- `Ctrl+,` — open settings (VS Code, Chrome, many Electron apps)

**`Alt+Left` / `Alt+Right` for traversal back/forward** (add as aliases)

`Back`/`Forward` mouse buttons are already the default. `Alt+Left`/`Alt+Right` are
the keyboard equivalents in every browser and are expected to work.

### 6.3 Proposed default table

Full revised defaults. Entries marked `(add)` have no current shortcut.
Entries marked `(change)` differ from the current default.

| Action | Proposed shortcut | Status vs current |
|---|---|---|
| Toggle Graph/Detail view | `Home` | change — remove `Esc`; drop `Tab` (see §6.5) |
| New node | `N` | keep |
| Delete selected | `Delete`, `Backspace` | add `Backspace` |
| Clear graph | `Ctrl+Shift+Delete` | keep |
| Command palette | `Ctrl+K` | change from `F2` |
| Radial palette | `Ctrl+Space` | change from `F3` |
| Node rename (future) | `F2` | reserved — no action yet |
| Graph search | `Ctrl+F` | keep |
| Help overlay | `F1`, `?` | keep |
| Pin selected | `P` | change from `I` |
| Unpin selected | `Alt+P` | change from `U` |
| Pin toggle | `Shift+P` | change from `L` |
| Connect pair (→) | `G` | keep |
| Connect both (↔) | `Shift+G` | keep |
| Remove user edge | `Alt+G` | keep |
| Toggle physics | `T` | keep |
| Reheat physics | `R` | keep |
| Physics settings | `Shift+T` | change from `P` |
| Camera controls | `F9` | keep |
| Zoom in / out / reset | `+` / `-` / `0` | keep |
| Fit to screen | `F` | add |
| Position-fit lock | `C` | keep (caution) |
| Zoom-fit lock | `Z` | keep (caution) |
| Pan camera | `WASD` / `Arrow Keys` | keep |
| Undo | `Ctrl+Z` | keep |
| Redo | `Ctrl+Y`, `Ctrl+Shift+Z` | add `Ctrl+Shift+Z` alias |
| Traversal back | `Back`, `Alt+Left` | add `Alt+Left` |
| Traversal forward | `Forward`, `Alt+Right` | add `Alt+Right` |
| Settings open | `Ctrl+,` | add |
| Split horizontal | `Ctrl+\` | add |
| Split vertical | `Ctrl+Shift+\` | add |
| Close pane | `Ctrl+W` | add |
| Activate workflow (future) | palette only initially | — |
| Open History Manager | palette only initially | — |

### 6.4 Accessibility constraints

#### WCAG 2.1.4 Character Key Shortcuts (Level A — hard requirement)

SC 2.1.4 applies to every single-character shortcut (a letter, digit, or symbol
key fired without Ctrl/Alt/Meta). The rule: single-character shortcuts must be
either remappable, disableable, or scoped to a component's focus state.

The current implementation partially satisfies the "focus-scoped" prong —
single-character shortcuts are suppressed when a text-input field captures
keyboard input (WCAG 2.1.4 "Untested" in the accessibility baseline checklist,
with partial evidence from input-layer regression coverage). However:

- AT (screen reader, switch access) focus states are not the same as text-input
  capture. A user navigating with a screen reader to a graph node may not have
  the graph canvas claiming keyboard ownership in the same way a mouse user does.
  A single keypress on `N` or `Delete` could fire unexpectedly.
- The proposed additions (`P`, `F`) increase the single-character surface.

**Compliance requirement**: all single-character shortcuts below must be
remappable or disableable. This makes C4 (Settings > Keybindings) a WCAG
compliance dependency, not an optional enhancement.

Single-character shortcuts requiring C4 compliance coverage:

`N`, `T`, `R`, `P`, `G`, `F`, `C`, `Z`, `?`, `Delete`, `Backspace`

Chord-based shortcuts (`Ctrl+*`, `Alt+*`, `Shift+*`) are exempt from SC 2.1.4
by definition and have no compliance risk.

#### `Tab` — reserved for focus navigation

`Tab` must not be assigned as a default shortcut for any graph command.
It is the primary focus traversal key for keyboard-only and AT users across
all platforms. Using bare `Tab` for a mode-switch (as §6.2 initially suggested)
would break keyboard navigation for any user who relies on sequential focus cycling.
This constraint applies to any single-key assignment of `Tab` in graph canvas context.

#### `Ctrl+Space` — IME conflict risk

`Ctrl+Space` is the default input-method (IME) switch chord on Windows for CJK
input modes (Simplified Chinese, Japanese, Korean). Assigning it as the radial
palette default will intercept the IME switch for users of those input methods.
This is a localization risk, not a current blocker (Graphshell does not yet target
CJK locales). Document as a known conflict and plan to offer an alternate default
before any CJK localization effort.

#### Focus guard completeness

The existing suppression of single-character shortcuts in text-entry contexts
must extend to AT-driven focus states. Before C4 ships, the focus guard behavior
should be verified against NVDA and Narrator (per the screen reader test matrix
in the accessibility baseline checklist §3) to confirm that single-char shortcuts
do not fire when AT navigates to a node without explicit graph-canvas pointer focus.

### 6.5 Configurability model

Three tiers, aligned with what comparable apps provide:

#### Tier 1 — Action-level rebinding (Settings > Keybindings)

Every `ActionId` in the matrix gets a rebindable shortcut. The settings page:

- Shows action name, current binding, and default binding side by side
- Flags conflicts (two actions with the same chord in the same context)
- Offers per-action "reset to default" and a global "reset all to default"
- Groups actions by scope class (Node, Edge, Graph, Workbench, Navigation, Command Surface)
- Displays shortcut hints that feed back to the help overlay and palette action rows

This is the C4 phase work. The `InputRegistry` / `KeybindingRegistry` must back it.

#### Tier 2 — Input mode selection (Settings > Input, already exists conceptually)

- WASD vs Arrow Keys for camera pan (already documented as configurable)
- Mouse button assignment for lasso select (right-click vs middle-click)
- Right-drag vs middle-drag for canvas pan

#### Tier 3 — Workflow preset keybinding profiles (future)

`WorkflowRegistry` profiles could bundle a keybinding preset alongside the
workbench/canvas/physics profiles. For example, a `workflow:research` could default
to a more keyboard-centric binding scheme. This is explicitly deferred to after C4 is
stable — it should not be designed as a workaround for missing action-level rebinding.

#### What must not be configurable

- `Ctrl+C`/`Ctrl+V`/`Ctrl+X` — OS copy/paste conventions; these are reserved
- `Esc` = dismiss (inside any open surface) — a UX primitive
- Left-click = select (primary pointer semantics)

---

## 7. Summary: What's Missing

| Gap | Where | Phase |
|---|---|---|
| ~29 matrix action IDs not registered in ActionRegistry | action.rs | C1.1 |
| 9 KEYBINDINGS.md command-class actions not in matrix | command_semantics_matrix.md | C1.2 |
| navigate_back/forward in registry but not matrix | command_semantics_matrix.md | C1.3 (now) |
| No context-per-entity column in matrix | command_semantics_matrix.md | C2.1 |
| Edge command context undefined in spec | graph_node_edge_interaction_spec.md | C2.2 |
| No workflow activation command | ActionRegistry | C3.1 |
| No history manager open command | ActionRegistry | C3.2 |
| No keybindings settings surface | Settings UX | C4 |
| Label entry UI for edge label (label-drop seam) | action.rs / reducer | B3.3 blocker |
| Shortcut changes needed vs current defaults | KEYBINDINGS.md | §6.3 (pre-C1) |
