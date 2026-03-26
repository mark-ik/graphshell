# UX Integration Research: File Tree + Tile Tree + UX Tree

**Date**: 2026-03-02
**Status**: Research / Active
**Category**: Research (Category 1 per DOC_POLICY §5)
**Purpose**: Investigate what UX best-practice research is needed before integrating the file tree (Graph data model) and tile tree (egui_tiles workbench layout) with the UX tree (UxTree semantic projection), and what inconsistencies currently exist relative to industry standards.

**Related**:
- `../implementation_strategy/subsystem_ux_semantics/SUBSYSTEM_UX_SEMANTICS.md` — UxTree architecture and subsystem policy authority
- `../implementation_strategy/subsystem_ux_semantics/2026-02-28_ux_contract_register.md` — UX contract register and domain model
- `../implementation_strategy/subsystem_focus/SUBSYSTEM_FOCUS.md` — Focus subsystem policy
- `../implementation_strategy/subsystem_focus/focus_and_region_navigation_spec.md` — Focus and region navigation contract
- `../implementation_strategy/subsystem_accessibility/SUBSYSTEM_ACCESSIBILITY.md` — Accessibility subsystem policy
- `../implementation_strategy/workbench/workbench_frame_tile_interaction_spec.md` — Tile tree interaction contract
- `../implementation_strategy/graph/graph_node_edge_interaction_spec.md` — Graph surface interaction contract
- `../implementation_strategy/aspect_command/command_surface_interaction_spec.md` — Command surface interaction contract
- `2026-02-24_interaction_and_semantic_design_schemes.md` — Lens/physics/semantic design research
- `../../TERMINOLOGY.md` — Canonical terminology

---

## 1. Research Objective

Graphshell has three tree structures that must interoperate coherently:

| Tree | What it models | Canonical owner | Current state |
|------|---------------|-----------------|---------------|
| **File tree** (Graph data model) | Content identity: Nodes, Edges, traversal history, tags, lifecycle | Graph reducer (`apply_intents`) | Implemented; `petgraph`-backed `Graph` with `NodeKey`/`EdgePayload` |
| **Tile tree** (Workbench layout) | Spatial arrangement: Panes, Containers, Splits, Tab Groups, Frames | `egui_tiles::Tree<TileKind>` via `Gui::tiles_tree` | Implemented; `egui_tiles` with `TileKind::{Graph, Node, Tool}` |
| **UX tree** (Semantic projection) | Machine-readable UI semantics: roles, states, actions, IDs, invariants | `UxTreeBuilder` (proposed) | Design-phase; no code yet |

The UX tree is designed to be a read-only per-frame projection of the other two trees into a stable semantic model. Before building it, we need to answer:

1. What information architecture gaps exist between the file tree and tile tree that will surface as UX inconsistencies?
2. What interaction semantics must be unified across all command surfaces before the UX tree can faithfully project them?
3. What predictability/state rules are needed so the UX tree's invariants (S/N/M series) can be satisfied?
4. What accessibility baseline must the integrated system meet?
5. What density/overflow behaviors must be standardized so the UX tree's structural checks are meaningful?

---

## 2. Best-Practice Framework

### 2.1 Framework composition

The research framework is a practical bundle of five complementary lenses:

| Lens | Source | What it covers | Why Graphshell needs it |
|------|--------|---------------|------------------------|
| **Usability heuristics** | Nielsen's 10 Heuristics (1994) | Visibility, feedback, consistency, error prevention, recognition, flexibility, aesthetic minimalism, error recovery, help/documentation | General UX quality floor across all surfaces |
| **WCAG 2.2 AA** | W3C Web Content Accessibility Guidelines | Perceivable, operable, understandable, robust — with specific success criteria | Minimum accessibility conformance target; maps to `SUBSYSTEM_ACCESSIBILITY.md` contracts |
| **Interaction consistency rules** | Graphshell UX Contract Register (§2–3) | One action grammar, canonical guarantees, ownership model | Prevents divergent semantics across command surfaces |
| **Information architecture** | Rosenfeld & Morville; card sorting principles | Object model clarity, scope boundaries, labeling, navigation systems | Grounds the file-tree→tile-tree→UX-tree projection in a coherent IA |
| **Semantic command language** | Graphshell `ActionRegistry` + command surface interaction spec | Verb + Target + Scope + Outcome grammar; preconditions; side effects; undoability | Ensures every command is expressible, discoverable, and testable through one authority |

### 2.2 How the framework applies

Each research area below (§3–§9) is evaluated against all five lenses. Findings are tagged with the lens that motivates them:

- `[NH]` — Nielsen Heuristic (number)
- `[WCAG]` — WCAG 2.2 criterion (number)
- `[IC]` — Interaction Consistency (Graphshell UX contract register rule)
- `[IA]` — Information Architecture
- `[SCL]` — Semantic Command Language

---

## 3. Information Architecture: Object Model Clarity

### 3.1 Research question

What is a file tree node vs. a tile tree node, where do actions live, and what are the scope boundaries?

### 3.2 Current state

The project has clear architectural separation:

- **Graph node** (`NodeKey`) = content identity, persisted, has lifecycle states (`Active`/`Warm`/`Cold`/`Tombstone`), carries metadata (URL, title, tags, `AddressKind`, `mime_hint`).
- **Tile** (`TileId`) = layout position, transient within a Frame, has a `TileKind` payload that references content.
- The mapping is **one-to-many**: a single `NodeKey` can appear in multiple tiles across frames/panes. A tile references a node; a node does not know about its tiles.

### 3.3 Identified gaps

| Gap | Lens | Description | Risk if unresolved |
|-----|------|-------------|-------------------|
| **G-IA-1**: Node vs. Tile identity confusion in user model | `[IA]` `[NH1]` | Users see "tabs" and "panes" (tile-tree concepts) but act on "pages" and "files" (graph-node concepts). Closing a tab ≠ deleting a node, but the affordance feels identical to browser tab-close. | Users accidentally treat tile-close as node-delete; ghost node confusion. |
| **G-IA-2**: Action scope ambiguity | `[IA]` `[IC]` | Some actions operate on the tile (close, split, move), others on the node (delete, tag, open-in-new-frame), and others on the graph (add edge, lasso). The scope is implicit in the action name, not explicit. | Users invoke tile-scoped actions expecting node-scoped effects, or vice versa. |
| **G-IA-3**: Multi-instance node confusion | `[IA]` `[NH6]` | A node can be open in multiple tiles (same or different frames). Editing in one tile may or may not affect the other tiles depending on what "editing" means (content is shared; camera/scroll is per-tile). | Users expect independent copies and are surprised by shared state, or expect synced state and are surprised by independent viewport. |
| **G-IA-4**: Graph scope vs. workbench scope vs. frame scope | `[IA]` | Terminology defines Workbench Scope, Graph Scope, and Frame, but the user has no visual cue for which scope an action affects. | Users change settings or selections expecting frame-local effect but getting workbench-wide effect. |
| **G-IA-5**: Ghost Node lifecycle legibility | `[IA]` `[NH1]` | Ghost Nodes (Tombstone lifecycle) are hidden by default ("Show Deleted" off). Users who delete a node and then look for it may not realize it still exists in the graph as a ghost. The terminology gap between "close tile" and "delete node" compounds this. | Users lose content they intended to keep, or fail to realize deletion is soft. |

### 3.4 Research needed

1. **Object-action mapping audit**: Enumerate every current action and classify it as node-scoped, tile-scoped, graph-scoped, or frame-scoped. Produce the Command Semantics Matrix (Deliverable 1).
2. **Labeling study**: Review action labels, tooltip text, and menu group headers for scope clarity. Do labels like "Close" and "Delete" sufficiently distinguish tile ↔ node operations?
3. **Multi-instance policy**: Define canonical rules for what state is shared vs. per-tile when a node appears in multiple panes. (Camera = per-tile. Content = shared. Selection highlight = per-view. Scroll position = per-tile.)

---

## 4. Interaction Semantics: One Action Grammar

### 4.1 Research question

Can every user action be expressed as `Verb + Target + Scope + Outcome` consistently across menus, command palette, radial, and shortcuts?

### 4.2 Current state

The command surface interaction spec (§3.2) guarantees: "the same action means the same thing on every command surface." The `ActionRegistry` is the semantic command authority. The current implementation has:

- Keyboard shortcuts: direct keybinding → `GraphIntent` variant.
- Command palette (Edge Commands window): hardcoded action list in `render/command_palette.rs`.
- Radial menu: hardcoded 8-sector layout in `render/radial_menu.rs`.
- Toolbar settings menu: inline egui button grid in `toolbar_settings_menu.rs`.
- Context menu: Servo-delegated right-click menu in `shell/desktop/ui/dialog.rs`.

### 4.3 Identified gaps

| Gap | Lens | Description | Risk if unresolved |
|-----|------|-------------|-------------------|
| **G-IS-1**: Hardcoded action lists | `[IC]` `[SCL]` | Command palette, radial menu, and toolbar settings menu each maintain independent action lists rather than querying `ActionRegistry`. | Adding a new action requires touching 3+ files; surfaces may diverge on available actions. |
| **G-IS-2**: No explicit Verb+Target+Scope grammar | `[SCL]` | Actions are named by verb alone ("Open", "Close", "Delete") without explicit target/scope disambiguation. | "Close" on a node in the command palette may mean close-tile or close-node depending on context, with no grammar to resolve it. |
| **G-IS-3**: Disabled action visibility | `[IC]` `[NH6]` | The contract says "disabled actions visible and explained rather than silently hidden." Current implementation hides unavailable actions entirely in some surfaces (radial menu), shows them greyed without explanation in others (command palette). | Users cannot discover why an action is unavailable; violates progressive disclosure principle. |
| **G-IS-4**: Context resolution rules undocumented | `[IC]` `[SCL]` | When a command is invoked via keyboard shortcut, the target is resolved by "focused context." But the focus subsystem's semantic focus owner is not yet wired to the action dispatch. | Keystroke lands on wrong target; user has no mental model for which pane "owns" the shortcut. |
| **G-IS-5**: Omnibar command dispatch not integrated | `[IC]` | The omnibar is primarily a URL bar. Command dispatch from the omnibar (e.g., typing `:close` or `/search`) is planned but not implemented. | One of three first-class command-entry families is non-functional for commands. |

### 4.4 Research needed

1. **Action grammar specification**: Define the canonical `Verb + Target + Scope + Outcome` template and apply it to every existing action. Example: `Close + Tile + ActiveFrame + TileRemovedFromTree` vs. `Delete + Node + WorkbenchGraph + NodeTombstoned`.
2. **ActionRegistry audit**: Compare the list of actions reachable from each surface (keyboard, palette, radial, toolbar, omnibar) and identify divergences.
3. **Disabled action policy**: Define when to show disabled-with-explanation vs. hide-entirely. Recommendation: always show in palette mode (with reason tooltip); hide in radial mode (limited sectors, no room for explanation); show greyed in toolbar (compact but visible).
4. **Context resolution spec**: Define how "focused context" is resolved for each surface at dispatch time, referencing the Focus subsystem's semantic focus owner.

---

## 5. Predictability and State: Focus, Selection, Active Pane

### 5.1 Research question

What focus rules, selection rules, and active-pane rules are needed, and how are conflicts resolved when multiple trees are visible?

### 5.2 Current state

The Focus subsystem (`SUBSYSTEM_FOCUS.md`) defines five ownership categories: Region Focus, Local Focus, Capture, Return Path, and Cross-Surface Rules. The focus and region navigation spec defines 8 primary regions and canonical guarantees ("always one semantic focus owner," "deterministic close/open handoff"). The UX contract register (§6.7) lists focus as a core domain.

### 5.3 Identified gaps

| Gap | Lens | Description | Risk if unresolved |
|-----|------|-------------|-------------------|
| **G-PS-1**: Selection truth source unclear in multi-pane | `[NH1]` `[IC]` | Graph selection (which nodes are selected) is currently per-`GraphBrowserApp` (global). But with multiple Graph View panes, selection should be per-`GraphViewId` (per-view). The code has begun migrating but the selection truth source is ambiguous. | Selecting nodes in one graph pane visually selects them in all graph panes; "Focus Selection" (`Z`) zooms the wrong view. |
| **G-PS-2**: Active pane vs. focused region | `[IA]` `[IC]` | "Active pane" (the tile receiving user input) and "focused region" (the Focus subsystem's semantic owner) are conceptually adjacent but architecturally separate. The mapping is implicit. | Framework focus and semantic focus diverge; keystrokes go to wrong surface. |
| **G-PS-3**: Focus return after modal dismiss | `[NH5]` `[IC]` | The focus spec requires "closing a surface returns focus to the next valid context." But the concrete return-path calculation (which pane gets focus when a dialog closes) is not implemented. | Dialog closes and focus lands on an arbitrary pane, or nowhere visible. |
| **G-PS-4**: Conflict on simultaneous tree visibility | `[IA]` | When a Graph View pane and a Node Pane are both visible in a split, pointer hover in the graph pane and keyboard focus in the node pane create a dual-authority scenario. | WASD panning vs. text entry conflict when both panes are "active." |
| **G-PS-5**: Frame switch state preservation | `[NH7]` | Switching frames should preserve per-frame selection, camera, and focus state. Frame Snapshot persists layout and manifest, but runtime focus/selection state is not part of the snapshot. | Switching away from a frame and back loses the user's working context; feels unpredictable. |

### 5.4 Research needed

1. **Selection scope model**: Define whether graph selection is global (shared across all Graph View panes in a Workbench), per-Frame, or per-GraphViewId. Canonical/Divergent view semantics may require per-view selection.
2. **Focus-to-active-pane mapping**: Document the exact mapping between Focus subsystem regions and `egui_tiles` active-tile state.
3. **Return-path algorithm**: Specify the concrete focus return path: on modal close, on pane close, on frame switch. Priority order: (1) previously focused sibling, (2) parent container's other child, (3) first visible graph pane, (4) omnibar.
4. **Input mode arbitration**: When pointer hover and keyboard focus target different panes, define which wins for each input type. Proposal: keyboard events always go to semantic focus owner; pointer events go to hovered surface; WASD is keyboard therefore follows semantic focus.

---

## 6. Discoverability: Progressive Disclosure and Messaging

### 6.1 Research question

How do users discover what they can do, and what do they see when they can't do something?

### 6.2 Current state

Graphshell has:
- A "Keyboard Shortcuts" help panel (`H` key).
- An "Edge Commands" floating palette.
- A radial menu (prototype).
- Toast notifications (`egui_notify`).
- No onboarding, no empty-state messaging, no inline hints, no "why unavailable" tooltips on disabled actions.

### 6.3 Identified gaps

| Gap | Lens | Description | Risk if unresolved |
|-----|------|-------------|-------------------|
| **G-D-1**: Empty graph state | `[NH1]` `[NH10]` | Opening Graphshell with no graph shows an empty canvas with no guidance. Users don't know how to start. | First-run abandonment; users don't realize they can type a URL in the omnibar. |
| **G-D-2**: Empty frame state | `[NH1]` | Creating a new frame shows an empty tile area. No hint about how to add content. | Users don't know they can open nodes into the new frame. |
| **G-D-3**: Disabled action explanation | `[NH9]` `[WCAG 3.3.3]` | When an action is disabled (e.g., "Focus Selection" with <2 nodes selected), the user sees a greyed button or nothing at all. No explanation of what precondition is unmet. | Users assume the feature is broken rather than understanding the precondition. |
| **G-D-4**: Command alias discoverability | `[NH6]` `[NH7]` | Users who know the keyboard shortcut may not know the palette command name, and vice versa. No cross-reference between surfaces. | Efficiency users bypass the palette; discovery users never learn shortcuts; both miss capabilities. |
| **G-D-5**: Progressive disclosure for complex actions | `[NH2]` `[NH8]` | Advanced capabilities (Lenses, Divergent views, MagneticZones, Ghost Nodes) have no progressive disclosure path. Users must already know these features exist to use them. | Power features are invisible; Graphshell feels simpler than it is. |
| **G-D-6**: No "did you mean" or suggestion affordance | `[NH7]` | When a user action fails or produces no result (e.g., typing an invalid URL), there is no suggestion or next-step guidance. | Dead-end experiences that leave the user stranded. |

### 6.4 Research needed

1. **Empty state inventory**: Enumerate every surface that can be empty (graph, frame, pane, search results, history, diagnostics) and define what each should show.
2. **Inline hint policy**: Define where persistent vs. dismissible hints appear. Candidates: omnibar placeholder text ("Enter a URL or type / for commands"), empty graph canvas ("Drag a URL here or press Ctrl+L"), empty frame ("Split or drag a tab to add content").
3. **Disabled action tooltip spec**: For each action that can be disabled, define the "why unavailable" message. Format: "[Action] requires [precondition]. [How to satisfy it]."
4. **Shortcut cross-reference**: In command palette items, show the keyboard shortcut. In keyboard shortcut help, show the palette command name. In radial sectors, show the shortcut key.

---

## 7. Feedback and Recovery: Optimistic UI, Undo, Error Copy

### 7.1 Research question

How does the system communicate action results, and how does the user recover from mistakes?

### 7.2 Current state

- **Optimistic UI**: Not implemented. All actions are synchronous through the intent reducer.
- **Undo**: Not implemented. `GraphIntent` variants are applied destructively; no undo stack exists.
- **Error feedback**: Toast notifications exist (`egui_notify`) but are used sparingly. Viewer load failures show a placeholder. No structured error copy (no error codes, no "copy error to clipboard" affordance).
- **Loading states**: Node viewer panes show a brief loading indicator during WebView creation, but backpressure cooldowns are invisible to the user.
- **Confirmation policy**: No destructive action requires confirmation. Close-tile, delete-node, and clear-graph all execute immediately.

### 7.3 Identified gaps

| Gap | Lens | Description | Risk if unresolved |
|-----|------|-------------|-------------------|
| **G-FR-1**: No undo for any action | `[NH3]` `[NH5]` | Deleting a node, closing a tile, clearing a selection, or removing an edge cannot be undone. Ghost Nodes partially mitigate node deletion, but other actions are gone. | Users fear making changes; prevents exploratory use. |
| **G-FR-2**: No confirmation for destructive actions | `[NH3]` `[NH5]` | "Delete Node" executes immediately on click. No "Are you sure?" for irreversible or high-impact operations. | Accidental data loss; users don't learn that Ghost Nodes are recoverable. |
| **G-FR-3**: Backpressure invisible | `[NH1]` | When WebView creation is throttled (backpressure cooldown), the user sees no indication. The node simply doesn't open, or opens with a long delay. | Users click repeatedly, queuing more WebView creations, worsening the backpressure. |
| **G-FR-4**: No structured error reporting | `[NH9]` | When a viewer fails to load, the placeholder shows generic text. No error code, no "Report Issue" affordance, no copy-to-clipboard. | Users cannot communicate errors effectively; debugging is harder. |
| **G-FR-5**: Toast notification coverage | `[NH1]` | Currently used only for a few events. Many state changes (frame switch, settings applied, export complete) produce no feedback. | Users are unsure whether their action succeeded. |
| **G-FR-6**: Confirmation policy undefined | `[IC]` | No documented rule for which actions require confirmation. Without a policy, some future destructive actions will get confirms and others won't, creating inconsistency. | Inconsistent confirm/no-confirm across similar operations. |

### 7.4 Research needed

1. **Undo architecture research**: Investigate undo strategies compatible with the intent reducer pattern. Options: (a) inverse-intent stack (each `GraphIntent` has a computed inverse), (b) snapshot-based undo (checkpoint before each action), (c) event-sourced rollback (replay all intents except the last N). Evaluate memory/performance trade-offs.
2. **Confirmation policy definition**: Classify actions by destructiveness: `safe` (no confirm), `soft-destructive` (recoverable, no confirm but toast with "Undo" link), `hard-destructive` (irreversible, confirm dialog). Map every current action to a tier.
3. **Loading/blocked state inventory**: Enumerate every state where the user is waiting (WebView creation, network fetch, physics settle, export) and define what visual feedback each should produce.
4. **Error taxonomy**: Define error categories (network, viewer, permission, internal) with structured fields (code, message, recovery suggestion, copy-to-clipboard affordance).

---

## 8. Accessibility: Keyboard Parity and Standards Conformance

### 8.1 Research question

Does the integrated file-tree + tile-tree system meet WCAG 2.2 AA, and can every operation be performed without a pointer?

### 8.2 Current state

The Accessibility subsystem (`SUBSYSTEM_ACCESSIBILITY.md`) defines contracts for tree integrity, focus/navigation, action routing, and degradation. The focus spec defines F6 region cycling, escape hatches, and capture rules. The UX semantics subsystem maps `UxRole` → `AccessKit Role`.

Current implementation:
- `egui` provides basic AccessKit integration (widget labels, roles, focus).
- Graph canvas accessibility is not implemented (nodes are paint commands, not AccessKit nodes).
- Keyboard navigation exists for some actions (WASD pan, arrow keys, shortcuts) but is not comprehensive.
- No screen reader testing has been performed.

### 8.3 Identified gaps mapped to WCAG 2.2 AA

| Gap | WCAG Criterion | Description | Current state |
|-----|---------------|-------------|---------------|
| **G-A-1**: Graph nodes not keyboard-focusable | 2.1.1 Keyboard | Graph nodes are rendered as paint commands. No Tab/arrow navigation between nodes. | Missing |
| **G-A-2**: No visible focus indicator on graph nodes | 2.4.7 Focus Visible | Even when a node is "selected," the selection ring is a graph-level concept, not a focus indicator for keyboard navigation. | Missing |
| **G-A-3**: Graph nodes lack accessible names | 4.1.2 Name, Role, Value | Nodes are not exposed to AccessKit. Screen readers cannot perceive node labels or roles. | Missing |
| **G-A-4**: No focus order in graph canvas | 2.4.3 Focus Order | The canvas has no navigable focus order for its child elements (nodes, edges). | Missing |
| **G-A-5**: Radial menu not keyboard-operable | 2.1.1 Keyboard | The radial menu is pointer-driven (spatial sectors). No keyboard navigation between sectors. | Missing |
| **G-A-6**: No skip links or landmarks | 2.4.1 Bypass Blocks | No skip link from toolbar to content. No WAI-ARIA landmark mapping for graph, pane, toolbar regions. | Missing |
| **G-A-7**: Contrast ratios unverified | 1.4.3 Contrast (Minimum) | No contrast audit has been performed on graph node labels, edge labels, or UI chrome. | Unverified |
| **G-A-8**: Motion/animation no reduced-motion support | 2.3.3 Animation from Interactions | Physics simulation runs continuously. No `prefers-reduced-motion` check. No option to disable/reduce animation. | Missing |
| **G-A-9**: Touch target sizes unverified | 2.5.8 Target Size (Minimum) | WCAG 2.2 requires 24×24 CSS px minimum. Graph node hit targets and small toolbar buttons may violate this. | Unverified |
| **G-A-10**: No error identification for form inputs | 3.3.1 Error Identification | Omnibar input errors (invalid URL) produce no accessible error message. | Missing |
| **G-A-11**: Keyboard trap in web content | 2.1.2 No Keyboard Trap | When a Node Pane has a WebView with web content focused, users may be unable to return focus to host UI without using mouse. | Partially addressed (Escape path planned) |

### 8.4 Research needed

1. **Keyboard navigation model for graph canvas**: Research spatial keyboard navigation strategies for force-directed graphs. Options: (a) Tab cycles through nodes in spatial order (nearest-neighbor from current), (b) arrow keys move focus in cardinal direction to nearest node, (c) search-to-focus (type node label to jump). Evaluate approaches used by Mermaid, D3, Graphviz interactive viewers.
2. **AccessKit integration depth**: Determine what subset of graph structure to expose to AccessKit. Full node list could be thousands of items; need a LOD-based approach (only expose nodes at current zoom level ≥ Compact).
3. **Reduced-motion implementation**: Map `prefers-reduced-motion` to physics behavior. Options: freeze simulation after initial layout, reduce spring constants to near-zero, disable animation transitions entirely.
4. **Contrast audit procedure**: Define tool and method for programmatic contrast checking of egui theme colors against WCAG thresholds.
5. **Screen reader testing plan**: Identify test matrix (NVDA + Firefox pattern for reference, then NVDA/Narrator on Windows, VoiceOver on macOS when supported).

---

## 9. Density and Overflow: Scrolling, Truncation, Resize

### 9.1 Research question

What scrolling defaults, max heights, truncation rules, and resize behaviors should be standard across all surfaces?

### 9.2 Current state

Recent work (2026-03-02) added scroll defaults and resize behavior to floating menus:
- "Keyboard Shortcuts" panel: `ScrollArea::vertical` + `resizable(true)`.
- "Edge Commands" palette: `ScrollArea::vertical` + `resizable(true)`.
- "Choose Frame" picker: `ScrollArea::vertical` + `resizable(true)`.
- Settings tool pane (docked): `ScrollArea::vertical`.
- Toolbar settings menu popup: `ScrollArea::vertical` with `max_height`.
- Context menus: `ScrollArea::vertical` with `max_height`.

### 9.3 Identified gaps

| Gap | Lens | Description | Risk if unresolved |
|-----|------|-------------|-------------------|
| **G-DO-1**: No truncation/ellipsis policy | `[NH8]` | Node labels, tab titles, and pane headers have no standard truncation rule. Some truncate mid-word, some don't truncate at all, some overflow. | Visual inconsistency; long titles break layouts. |
| **G-DO-2**: Minimum pane size undefined | `[NH2]` | Users can resize split panes to near-zero width/height. No minimum usable size enforced. | Panes become too small to interact with; controls are clipped and inaccessible. |
| **G-DO-3**: Floating surface position persistence | `[NH7]` | Floating windows (help, command palette) reopen at default position every time, not where the user last placed them. | Users must reposition windows repeatedly. |
| **G-DO-4**: Overflow behavior for graph info overlay | `[NH8]` | The graph info overlay (node count, edge count, camera info) is a fixed-position text block. With many info lines, it could overflow the viewport. | Text overlaps interactive elements or extends off-screen. |
| **G-DO-5**: Max-height heuristic varies | `[IC]` | Different surfaces compute max-height differently. Settings menu uses `content_rect().height() - 120.0`; context menu uses a similar but separate calculation. No shared constant or policy. | Inconsistent overflow thresholds across surfaces. |
| **G-DO-6**: Scroll position not preserved | `[NH7]` | Scrolling position in the settings pane, command palette, or help panel is reset when the surface is closed and reopened. | Users lose their place in long content. |
| **G-DO-7**: No horizontal overflow policy | `[NH8]` | Only vertical scroll has been standardized. Wide content (long URLs in nav bars, wide diagnostic tables) has no horizontal overflow rule. | Content clips silently or breaks layout. |

### 9.4 Research needed

1. **Truncation policy**: Define standard truncation rules. Proposal: tab titles truncate with ellipsis at container width minus 24px (close button). Node labels truncate at LOD-appropriate character limits. Tooltips show full text on hover.
2. **Minimum pane size**: Define per-`TileKind` minimums. Proposal: `Graph` pane minimum 200×150px. `Node` pane minimum 300×200px (to fit nav bar + content). `Tool` pane minimum 200×150px.
3. **Overflow constant unification**: Extract max-height calculations into a shared function: `fn surface_max_height(ctx: &egui::Context, margin: f32) -> f32`.
4. **Floating position persistence**: Add `Window::default_pos` from saved position to floating windows. Store in session state (not serialized to disk).

---

## 10. Deliverable Set

The research areas above inform five concrete deliverables that, together, make the integrated file-tree + tile-tree + UX-tree system feel coherent and professional.

### Deliverable 1: Command Semantics Matrix

**Purpose**: Single table of all commands by surface, scope, preconditions, side effects, and undoability.

**Columns**:

| Column | Description |
|--------|-------------|
| Action ID | `ActionRegistry` identifier |
| Verb | Human-readable action verb |
| Target | Node / Tile / Frame / Graph / App |
| Scope | per-View / per-Frame / per-Workbench / Global |
| Preconditions | Required state before execution |
| Side effects | State changes produced |
| Undoable | Yes (inverse intent) / Soft (ghost/recoverable) / No |
| Surfaces | Which surfaces expose this action (Keyboard / Palette / Radial / Toolbar / Omnibar) |
| Disabled behavior | Hidden / Greyed+reason / N/A |
| Shortcut | Keybinding(s) |

**Source data**: §4 research (interaction semantics audit).

**Format**: Markdown table in `design_docs/graphshell_docs/design/command_semantics_matrix.md`.

**Acceptance**: Every implemented `GraphIntent` variant has a row. Every command surface (keyboard, palette, radial, toolbar, omnibar) is represented. No action appears on one surface with different semantics than another.

### Deliverable 2: Interaction Contract

**Purpose**: Focus/selection/navigation rules across trees.

**Sections**:

1. **Selection scope model** — per-view vs. per-frame vs. global selection, with rationale.
2. **Focus ownership map** — which architectural component owns focus at each state, referencing Focus subsystem regions.
3. **Focus handoff rules** — concrete algorithm for spawn, close, frame-switch, modal-dismiss.
4. **Input arbitration** — pointer vs. keyboard targeting when they disagree.
5. **Active-pane indicator contract** — what visual affordance distinguishes the focused pane.

**Source data**: §5 research (predictability/state gaps).

**Format**: Extends `focus_and_region_navigation_spec.md` with concrete algorithms, or new section in the workbench interaction spec.

**Acceptance**: UxTree invariants N1–N4 are satisfiable by the documented rules. Focus return path is computable from the documented algorithm with no ambiguity.

### Deliverable 3: Surface Behavior Spec

**Purpose**: Scroll, resize, overflow, empty/loading/error states across all surfaces.

**Sections**:

1. **Scroll defaults** — per-surface-class scroll policy (vertical always, horizontal per-content-type).
2. **Max-height policy** — shared computation, per-surface margin constants.
3. **Resize behavior** — which surfaces are resizable, minimum sizes, position persistence.
4. **Truncation rules** — character/pixel limits, ellipsis placement, tooltip fallback.
5. **Empty states** — per-surface empty-state content (text + optional action button).
6. **Loading states** — per-surface loading indicator (spinner, skeleton, progress bar).
7. **Error states** — per-surface error display (inline message, structured error card, recovery action).
8. **Floating surface lifecycle** — position persistence, open/close animation (or none per reduced-motion), z-order rules.

**Source data**: §9 research (density/overflow gaps) + §6 research (discoverability/empty states) + §7 research (feedback/loading/error states).

**Format**: `design_docs/graphshell_docs/design/surface_behavior_spec.md`.

**Acceptance**: Every surface class (graph pane, node pane, tool pane, floating window, popup menu, toast, dialog) has documented behavior for scroll, overflow, empty, loading, and error states.

### Deliverable 4: Accessibility Baseline Checklist

**Purpose**: WCAG 2.2 AA-mapped pass/fail criteria per surface.

**Structure**: One row per WCAG success criterion (Level A + AA), with columns:

| Column | Description |
|--------|-------------|
| Criterion | WCAG number + name |
| Level | A or AA |
| Graph Pane | Pass / Fail / N/A / Untested |
| Node Pane | Pass / Fail / N/A / Untested |
| Tool Pane | Pass / Fail / N/A / Untested |
| Floating Windows | Pass / Fail / N/A / Untested |
| Dialogs | Pass / Fail / N/A / Untested |
| Omnibar | Pass / Fail / N/A / Untested |
| Workbar | Pass / Fail / N/A / Untested |
| Notes | Implementation detail or gap description |

**Source data**: §8 research (accessibility gaps).

**Format**: `design_docs/graphshell_docs/design/accessibility_baseline_checklist.md`.

**Acceptance**: All Level A criteria assessed. All Level AA criteria at minimum tagged as `Untested` with gap description. No criterion left blank.

### Deliverable 5: UX Telemetry Plan

**Purpose**: Define what to measure so UX quality can be tracked quantitatively over time.

**Metrics**:

| Metric | What it measures | Collection method |
|--------|-----------------|-------------------|
| Task success rate | % of user intents that reach expected outcome | UxScenario pass rate in CI |
| Command abandonment rate | % of command palette opens that dismiss without invocation | Diagnostics counter: `command.palette.opened` vs. `command.palette.invoked` |
| Undo-after-action rate | % of actions followed by undo within 5 seconds | Diagnostics: `action.invoked` → `action.undone` within time window |
| Focus confusion events | Times focus return path produces an unexpected region | UxProbe N3 violations: `ux:navigation_violation` count |
| Backpressure visibility | Times user acted during invisible cooldown | Diagnostics: `viewer.backpressure.user_action_during_cooldown` |
| Accessibility invariant pass rate | % of S-series invariants passing per CI run | UxProbe pass/fail ratio |
| First-action latency | Time from app open to first user action | Timestamp delta: `app.ready` → first `action.invoked` |
| Focus cycle completeness | F6 visits all expected regions | UxProbe N3/N4 pass rate |

**Implementation approach**: All metrics collected through existing diagnostics channel infrastructure (`DiagnosticsRegistry`). No external analytics. Local-only by default; optional Verse-published aggregate metrics are a Tier 2 concern.

**Source data**: §3–§9 research gaps, particularly feedback/recovery (§7) and focus/predictability (§5).

**Format**: Section in `design_docs/graphshell_docs/design/ux_telemetry_plan.md`.

**Acceptance**: Each metric has a defined collection method, a diagnostics channel (existing or new), and a target threshold (even if the initial target is "establish baseline").

---

## 11. Current Inconsistency Summary

This section consolidates the gaps from §3–§9 into an actionable priority matrix.

### 11.1 Critical (blocks UxTree invariant satisfaction)

| ID | Gap | Blocking invariant |
|----|-----|--------------------|
| G-IA-2 | Action scope ambiguity | S1 (label clarity) |
| G-IS-1 | Hardcoded action lists diverge across surfaces | UX contract guarantee: "same action, same meaning" |
| G-PS-1 | Selection truth source unclear in multi-pane | M1 (pane/node state consistency) |
| G-PS-2 | Active pane vs. focused region divergence | S3 (exactly one focused node), N1–N4 (focus traversal) |
| G-A-1 | Graph nodes not keyboard-focusable | N4 (tab traversal completeness) |
| G-A-3 | Graph nodes lack accessible names | S1 (every interactive control labeled) |

### 11.2 High (degrades UX quality but doesn't block invariants)

| ID | Gap | Quality impact |
|----|-----|---------------|
| G-IA-1 | Node vs. Tile identity confusion | Users mistake close-tile for delete-node |
| G-IS-3 | Disabled action no explanation | Users assume features are broken |
| G-PS-3 | Focus return after modal dismiss | Focus lands arbitrarily after dialog close |
| G-D-1 | Empty graph state no guidance | First-run abandonment |
| G-FR-1 | No undo for any action | Users fear making changes |
| G-FR-2 | No confirmation for destructive actions | Accidental data loss |
| G-A-8 | No reduced-motion support | Motion-sensitive users cannot use the app |

### 11.3 Medium (quality polish)

| ID | Gap | Quality impact |
|----|-----|---------------|
| G-IA-3 | Multi-instance node confusion | Shared vs. independent viewport surprise |
| G-IS-4 | Context resolution undocumented | Keystroke targeting ambiguity |
| G-D-4 | Command alias no cross-reference | Efficiency/discovery ceiling |
| G-DO-1 | No truncation/ellipsis policy | Visual inconsistency |
| G-DO-2 | Minimum pane size undefined | Unusable micro-panes |
| G-DO-5 | Max-height heuristic varies | Inconsistent overflow |
| G-A-7 | Contrast ratios unverified | Potential WCAG failures |
| G-A-9 | Touch target sizes unverified | Potential WCAG failures |

### 11.4 Low (deferred/long-horizon)

| ID | Gap | Quality impact |
|----|-----|---------------|
| G-IA-4 | Scope visual cues | Advanced users; scope is rarely ambiguous in single-frame use |
| G-IS-5 | Omnibar command dispatch | Planned feature, not a consistency bug |
| G-D-5 | Progressive disclosure for power features | Important but not a regression |
| G-DO-3 | Floating position persistence | Convenience; not a UX contract issue |
| G-DO-6 | Scroll position preservation | Convenience |

---

## 12. Research Execution Order

The research areas have dependencies. Recommended execution order:

1. **Information architecture audit** (§3) — Produces the object-action mapping that feeds everything else.
2. **Interaction semantics audit** (§4) — Produces the Command Semantics Matrix (Deliverable 1) and identifies the `ActionRegistry` gap.
3. **Predictability/state research** (§5) — Produces the Interaction Contract (Deliverable 2); depends on IA clarity from step 1.
4. **Density/overflow standardization** (§9) — Produces the Surface Behavior Spec (Deliverable 3); can run in parallel with steps 2–3.
5. **Discoverability research** (§6) + **Feedback/recovery research** (§7) — Folds into Deliverables 1 and 3; depends on steps 1–4 for inventory.
6. **Accessibility audit** (§8) — Produces the Accessibility Baseline Checklist (Deliverable 4); depends on steps 1–3 for surface inventory and focus model.
7. **Telemetry planning** — Produces the UX Telemetry Plan (Deliverable 5); depends on all other deliverables to identify what to measure.

---

## 13. Relationship to Existing Docs

This research document feeds the deliverable set but does not replace existing canonical specs or subsystem authority docs.

| Existing doc | This doc's relationship |
|--------------|------------------------|
| `SUBSYSTEM_UX_SEMANTICS.md` | This doc identifies gaps the UxTree must model; UxTree builder design should reference gap findings |
| `SUBSYSTEM_ACCESSIBILITY.md` | Accessibility gaps (§8) feed the subsystem's contract coverage; this doc does not redefine policy |
| `SUBSYSTEM_FOCUS.md` | Focus gaps (§5) identify missing implementation detail; policy authority stays with the subsystem doc |
| UX Contract Register | This doc's gap analysis feeds new contract slices; contract template and workflow stay in the register |
| Interaction specs (6 canonical) | This doc audits consistency across specs; specs remain the canonical behavior contracts |
| `TERMINOLOGY.md` | All terms in this doc use canonical terminology; no new terms are introduced |
