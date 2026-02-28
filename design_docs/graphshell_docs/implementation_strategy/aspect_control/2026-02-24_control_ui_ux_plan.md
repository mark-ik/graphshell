# Control UI/UX Plan (2026-02-24)

**Status**: Implementation-Ready
**Relates to**: `2026-02-23_graph_interaction_consistency_plan.md` (ActionRegistry routing,
radial/command palette refactor tasks), `2026-02-22_registry_layer_plan.md` (ActionRegistry,
InputRegistry)

## Naming

There are two first-class control surfaces in Graphshell:

- **Command Palette**: the canonical name for the configurable contextual command list.
  In Mouse/KB mode this is the default surface for right-click on a node (a filtered,
  context-scoped view of the palette). In global mode (`Ctrl+K`) it is a searchable list
  of all registered actions. It is one surface with two scope modes: **contextual** (filtered
  to the target) and **global** (full registry). The distinction is scope, not UI — the same
  component renders both.
- **Radial Menu**: the directional command surface, default in Gamepad mode.

The term "context menu" is retired as a Graphshell concept. The webview right-click popup
from Servo (`dialog.rs`) retains the `ContextMenu` type name as a Servo artefact, but it is
not a first-class Graphshell control surface — it is Servo's native context menu surfaced
through the embedder, and is treated separately from the palette.

---

## Goal

All control surfaces must be:

- **Readable**: labels and icons visible at all zoom levels and window sizes, no occlusion
- **Interactable**: hit targets that work for pointer, keyboard, and gamepad
- **Contextual**: content derived from `ActionRegistry::list_actions_for_context(context)`,
  not hardcoded enums
- **Ergonomic**: spatially arranged to minimise pointer travel; gamepad layouts default to
  directional navigation

This applies to every control UI element in the application. The radial menu (unreadable,
elements stacked) and the command palette (not yet wired to `ActionRegistry`) are the
priority. The principle generalises to all controls.

---

## Current State (as of 2026-02-24)

**Radial menu** (`render/mod.rs::render_radial_command_menu`):

- Hardcoded `RadialCommand` and `RadialDomain` enums (~27 commands across 4 domains). Not
  driven by `ActionRegistry`. Bypasses registry routing.
- Circular rendering mode: concentric ring layout. Commands at computed (angle, radius)
  positions. **Known defect**: labels overlap/stack; elements are unreadable in practice.
- Keyboard/list mode (triggered by right-click on node): horizontal three-group bar. A
  separate rendering path for the same conceptual surface.
- No gamepad navigation. `InputRegistry` bindings for stick/dpad not wired.
- Shortcut binding (`RadialMenuShortcut::F3 | R`) persisted but only two hardcoded options.

**Command palette**: currently split across the circular radial mode (global shortcut) and
the horizontal list mode (right-click on node). The global `Ctrl+K` path is the closest
thing to a command palette today but is not fully wired to `ActionRegistry`. The right-click
path opens the radial menu rather than a proper contextual palette.

**Webview popup** (`dialog.rs`): Servo's `ContextMenu` rendered as a vertical popup for web
content right-clicks. Functional. Not a Graphshell control surface — leave it as-is.

**Gamepad** (`desktop/gamepad.rs`): W3C button/axis mapping via `gilrs`. Events forwarded
to Servo for web content. No graph-layer gamepad interaction — no dpad node focus, no
stick-navigate, no trigger-to-confirm.

---

## Design

### Input Mode

Two modes drive surface defaults. Tracked in `AppPreferences`, updated automatically on
input event type. Users can pin a mode.

```rust
pub enum InputMode {
    MouseKeyboard,
    Gamepad,
}
```

`InputMode` is a **layout hint**, not a gate — both surfaces work in both modes.

| Trigger | Mouse/KB default | Gamepad default |
| --- | --- | --- |
| Right-click / `A` button on focused node | Command palette (contextual) | Radial menu |
| `Ctrl+K` / `Menu` button | Command palette (global) | Radial menu |

---

### Command Palette

**Scope modes**:

- **Contextual** (triggered by right-click on a node, or `A` button on focused node in
  Mouse/KB mode): palette filtered to actions relevant to the target node/context.
  `ActionRegistry::list_actions_for_context(context)` with `target_node = Some(key)`.
- **Global** (`Ctrl+K` / `F2` / `Start` button): full searchable registry.
  `ActionRegistry::list_actions_for_context(context)` with `target_node = None`, plus
  fuzzy search over all registered `ActionId`s. Context-relevant actions ranked first.

**Layout rules** (shared across both scope modes):

- Contextual mode: vertical list, egui popup-style, appears at pointer position.
  Global mode: centred floating panel with a full-width search input at top.
- Actions grouped by `ActionCategory` with `ui.separator()` between groups. Small greyed
  group label above each group.
- Destructive actions (delete, disconnect) last within their group, danger colour from
  `ThemeRegistry`.
- Disabled actions shown greyed with a tooltip explaining why. Not hidden — hidden actions
  make the palette feel inconsistent.
- Maximum visible rows before scroll: 12 (contextual) / 8 (global, below search input).
  Overflow scrolls within the popup (`egui::ScrollArea`).
- Keyboard navigation: arrow keys move focus; Enter confirms; Escape dismisses. Tab wraps.
- Contextual mode position: below-right of pointer. If the popup clips the screen edge,
  flip (right→left, below→above).
- Global mode results: icon + name + shortcut hint (right-aligned).
- Gamepad: stick/dpad navigates; confirm button executes; B-button dismisses.

**Module**: `desktop/command_palette.rs` (extract from `render/mod.rs`).

---

### Radial Menu

**When used**: Default in Gamepad mode. Available in Mouse/KB mode via configurable hotkey.
Optimised for directional navigation — 8 sectors, 4 primary (cardinal) and 4 secondary
(diagonal). No concentric rings: **one action per sector**, confirmed by D-pad/stick
direction + trigger or face button. Overflow onto a second page (LB/RB to cycle).

**Layout rules**:

- Maximum 8 sectors. If `ActionRegistry` returns more than 8 actions, group by
  `ActionCategory` into sectors; overflow to second page.
- Each sector: large icon at the centre of the arc, short label (≤ 12 chars) rendered
  **outside** the ring. No label inside any shape.
- Uniform sector size. No variable-width sectors.
- Active sector: filled arc highlight. Inactive: outline only.
- Escape / B-button: dismiss without executing.
- Empty sector positions are absent — no placeholder arcs for missing actions.

**Pointer interaction** (Mouse/KB mode): hover to highlight; click to confirm. Stick/dpad
always works regardless of mode.

**Rendering location**: pointer position (Mouse/KB) or screen centre (Gamepad mode).

**Module**: `desktop/radial_menu.rs` (extract from `render/mod.rs`).

---

## Action Content: Defaults and Configuration

Both surfaces draw from `ActionRegistry::list_actions_for_context(context)`:

```rust
pub struct ActionContext {
    pub target_node: Option<NodeKey>,    // None = global scope
    pub input_mode: InputMode,
    pub view_id: GraphViewId,
    pub selected_nodes: Vec<NodeKey>,
}
```

**Seed floor defaults**:

| Category | Actions |
| --- | --- |
| Node | Open, Open in Split, Pin/Unpin, Copy URL, Copy Title, Delete, Connect to... |
| Edge | Connect Pair, Connect Both, Remove |
| Graph | Fit to Screen, Toggle Physics, Reheat Physics, Save Snapshot |
| Navigation | Undo, Redo, Restore Session |
| Workspace | Add to Workspace, Move to Active Pane, Open Workspace |

**Gamepad sector defaults** (node context, clockwise from top):

| Sector | Direction | Action |
| --- | --- | --- |
| 0 | Up | Open |
| 1 | Up-Right | Open in Split |
| 2 | Right | Connect to... |
| 3 | Down-Right | Add to Workspace |
| 4 | Down | Delete |
| 5 | Down-Left | Copy URL |
| 6 | Left | Pin/Unpin |
| 7 | Up-Left | Fit to Screen |

Default layout is a seed floor entry in `ActionRegistry`. Remappable via `InputRegistry`.
Mods register additional actions that appear in overflow pages.

---

## Configuration Surface

Stored in `AppPreferences`:

- **Input mode**: Auto / Mouse+KB always / Gamepad always
- **Command palette trigger (contextual)**: Right-click / Long-press / Disabled
- **Radial menu trigger**: Hotkey (configurable) / Right-click / Disabled
- **Radial menu sectors**: remappable per sector via `InputRegistry` action bindings
- **Command palette group order**: drag-to-reorder `ActionCategory` groups
- **Show disabled actions**: Yes (greyed) / No (hidden) — default Yes
- **Gamepad confirm button**: A / Right trigger / configurable

---

## Ergonomics Principle (generalises to all control UI)

> **Every control surface must be operable with the active input mode without requiring
> the other.** A user who has never touched a keyboard must be able to use the full
> application with a gamepad. A user who has never touched a gamepad must be able to use
> it with keyboard and mouse. Touch must work where supported.

Concretely:

- No action is exclusively bound to a mouse gesture without a keyboard/gamepad equivalent
- No action is exclusively bound to a keyboard shortcut without a pointer/gamepad equivalent
- All interactive elements have sufficient hit-target size (minimum 32×32 dp)
- All interactive elements have accessible labels (for screen readers and hover tooltips)
- No label is rendered inside a shape that would occlude it at any zoom level

---

## Implementation Steps

1. **Extract modules**: Move radial menu to `desktop/radial_menu.rs` and command palette
   to `desktop/command_palette.rs`. Leave `render/mod.rs` as the callsite.
2. **Wire `ActionRegistry`**: Replace `RadialCommand` / `RadialDomain` hardcoded enums with
   `ActionRegistry::list_actions_for_context(context)`. Same task as
   `2026-02-23_graph_interaction_consistency_plan.md §Implementation tasks`.
3. **Redesign radial layout**: 8-sector, no concentric rings, labels outside the ring,
   uniform sector size.
4. **Unify command palette**: Merge the circular global mode and the horizontal list mode
   into one `CommandPalette` component with contextual and global scope modes.
5. **Add `InputMode` to `AppPreferences`**: Auto-detect from event type. Feed to dispatch.
6. **Gamepad graph navigation**: Wire dpad/stick to node focus cycling. Wire confirm to
   open radial menu on focused node. Wire B to dismiss.
7. **Command palette keyboard navigation**: Arrow keys, Enter, Escape in both scope modes.
8. **Configuration UI**: Input mode preference, trigger remapping, show-disabled toggle.

Steps 1–4 unblock the readability defect and naming consolidation; do as a single slice.
Steps 5–7 are the gamepad layer. Step 8 is settings UI, last.

---

## Validation

- [ ] Radial menu labels are fully readable at all window sizes (no occlusion)
- [ ] Radial menu is fully navigable with dpad/stick on a connected gamepad
- [ ] Command palette (contextual) appears on right-click and is navigable with arrow keys
- [ ] Command palette (global) is searchable and navigable with arrow keys and gamepad
- [ ] Both surfaces execute actions through `ActionRegistry` (no hardcoded parallel enum)
- [ ] Both surfaces populate from `ActionRegistry::list_actions_for_context`
- [ ] Disabled actions are shown with tooltips explaining the reason (not hidden)
- [ ] Input mode auto-detects on first gamepad event and restores on pointer event
- [ ] All interactive controls in the application meet the 32×32 dp minimum hit-target size
