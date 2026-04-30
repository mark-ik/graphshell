<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Iced Command Palette Spec

**Date**: 2026-04-29 (revised)
**Status**: Canonical / Active — third concrete S2 deliverable for the iced jump-ship plan
**Scope**: The two iced-side command-dispatch surfaces — **Command Palette**
(Modal overlay with fuzzy-search filter over a contextual command list,
Zed/VSCode-shaped) and **Context Menu** (right-click on any interactable
target, flat list of available commands). Both source actions from the same
`ActionRegistry` and route dispatch through `HostIntent::Action` with the
uphill rule. The previous draft of this spec mandated a two-tier
category/option contract across three modes (Search / Context / Radial);
that model is **retired** per the 2026-04-29 simplification — see §1.1.

**Code-sample mode**: **Illustrative signatures**. Concrete S3/S4 code lives
in the implementation, not this spec.

**Related**:

- [`../aspect_command/command_surface_interaction_spec.md`](../aspect_command/command_surface_interaction_spec.md) — canonical command-surface contract (revised 2026-04-29 to drop two-tier + Radial)
- [`../aspect_command/ASPECT_COMMAND.md`](../aspect_command/ASPECT_COMMAND.md) — Command aspect authority
- [`iced_composition_skeleton_spec.md`](iced_composition_skeleton_spec.md) — Application skeleton (§1.5), CommandBar slot (§7.2), context palette (§7.3), uphill rule routing (§8)
- [`iced_omnibar_spec.md`](iced_omnibar_spec.md) — sibling spec; shares focus-dance contract
- [`2026-04-28_iced_jump_ship_plan.md` §4.10](2026-04-28_iced_jump_ship_plan.md) — coherence guarantees for command palette and context menu
- [SHELL.md §6](SHELL.md) — Shell ↔ Navigator chrome relationship
- [`../subsystem_ux_semantics/2026-04-05_command_surface_observability_and_at_plan.md`](../subsystem_ux_semantics/2026-04-05_command_surface_observability_and_at_plan.md) — provenance / AT validation

---

## 1. Intent and Boundary

Two surfaces handle command discovery and dispatch:

| Surface | Trigger | Shape | Source |
|---|---|---|---|
| **Command Palette** | `Ctrl+Shift+P` (canonical, Zed/VSCode-shaped), `F2` (alternate), CommandBar trigger button, programmatic | `Modal` overlay with `text_input` filter + flat ranked list of currently-available commands | `ActionRegistry::rank_for_query(query, view_model)` |
| **Context Menu** | Right-click on an interactable target | `iced_aw::ContextMenu` with a flat list of commands available on that target | `ActionRegistry::available_for(target, view_model)` |

Both surfaces:

- read action data exclusively from `ActionRegistry` (atomic registry per
  TERMINOLOGY.md §Registry Architecture);
- dispatch via a single `HostIntent::Action` per selection;
- gate by selection-set availability per
  [command_surface_interaction_spec.md §4.1](../aspect_command/command_surface_interaction_spec.md);
- show disabled actions with explicit reasons;
- gate destructive actions through `ConfirmDialog` (per §5);
- route uphill per the iced jump-ship plan §4.9.

### 1.1 What this spec retires (2026-04-29 simplification)

The previous draft mandated a two-tier category/option model across three
modes (Search Palette / Context Palette / Radial Palette) with cross-mode
equivalence rules from
[command_surface_interaction_spec.md §3.3](../aspect_command/command_surface_interaction_spec.md).
That model is retired:

- **Two-tier rendering** (Tier 1 horizontal category strip + Tier 2
  vertical option list) → **flat ranked list** in both Command Palette
  and Context Menu. The search filter is the discovery mechanism;
  category browsing is dropped.
- **Search Palette Mode + Context Palette Mode distinction** → folded
  into a single Command Palette; "search" is the palette's input
  affordance, not a separate mode.
- **Radial Palette Mode** → retired indefinitely. Was originally gamepad-
  oriented per [iced jump-ship plan §11 G2](2026-04-28_iced_jump_ship_plan.md);
  if gamepad input lands later as part of the input-subsystem rework, a
  radial surface can be reintroduced as a third command-dispatch route
  with its own design pass. The geometry research in
  [`../aspect_command/radial_menu_geometry_and_overflow_spec.md`](../aspect_command/radial_menu_geometry_and_overflow_spec.md)
  is preserved for that future work.
- **Cross-mode equivalence rule** (Tier 1 strip = Tier 1 ring) → moot;
  there is no Tier 1.

This is a real change to the canonical aspect_command spec; the canonical
spec was updated in the same 2026-04-29 commit. See its §3 and §4 for the
revised canonical interaction model.

---

## 2. Command Palette

### 2.1 Invocation

Trigger sources, all converging on `Message::PaletteOpen { origin }`:

- **`Ctrl+Shift+P`** — global keyboard shortcut, captured by the iced
  application's keyboard subscription. Canonical (Zed/VSCode-shaped).
  (Note: `Ctrl+P` is reserved for the
  [Node Finder](iced_node_finder_spec.md) per the 2026-04-29 omnibar-
  split simplification, matching Zed's separation of file finder vs
  command palette.)
- **`F2`** — alternate shortcut for parity with prior canonical
  binding (see [command_surface_interaction_spec.md §4.2](../aspect_command/command_surface_interaction_spec.md)).
- **CommandBar trigger button click** — emits `Message::PaletteOpen`
  with `PaletteOrigin::TriggerButton`.
- **Context Menu → Search fallback** — a "Search commands…" footer
  entry in any Context Menu opens the palette pre-scoped to that
  target. Origin is `PaletteOrigin::ContextFallback`.
- **Programmatic** — actions that open the palette as part of their
  effect (rare).

Origin is recorded for diagnostics provenance per
[`subsystem_ux_semantics/2026-04-05_command_surface_observability_and_at_plan.md`](../subsystem_ux_semantics/2026-04-05_command_surface_observability_and_at_plan.md).

### 2.2 Widget Tree

```rust
fn command_palette_overlay(state: &State) -> Option<Element<'_, Message, GraphshellTheme>> {
    state.command_palette.is_open.then(|| {
        modal(
            container(
                column![
                    text_input(
                        &state.command_palette.query,
                        Message::PaletteQuery,
                    )
                    .on_submit(Message::PaletteSubmitFocused)
                    .id(text_input::Id::new("command_palette_input"))
                    .placeholder("Type a command or search…"),

                    horizontal_rule(),

                    palette_results_list(
                        &state.command_palette.ranked_actions,
                        state.command_palette.focused_index,
                    ),

                    palette_footer(state),  // disabled-reason explanation, hint text
                ]
                .spacing(8)
                .padding(12)
            )
            .style(palette_container_style)
            .max_width(640)
            .max_height(480)
        )
        .on_blur(Message::PaletteCloseAndRestoreFocus)
        .into()
    })
}

fn palette_results_list<'a>(
    ranked: &'a [RankedAction],
    focused: Option<usize>,
) -> Element<'a, Message, GraphshellTheme> {
    scrollable(
        column(ranked.iter().enumerate().map(|(i, action)| {
            action_row(action, focused == Some(i))
                .on_press(Message::PaletteActionSelected(action.id))
        }))
        .spacing(2)
    )
    .into()
}
```

### 2.3 State Shape

```rust
// Illustrative.
pub struct CommandPaletteState {
    pub is_open: bool,
    pub origin: PaletteOrigin,
    pub query: String,                       // empty = "all available, default order"
    pub scope: PaletteScope,                 // CurrentTarget | ActivePane | ActiveGraph | Workbench
    pub ranked_actions: Vec<RankedAction>,
    pub focused_index: Option<usize>,        // keyboard-focused row
    pub focus_token: Option<widget::Id>,  // saved iced widget focus id at open time
    pub pending_confirmation: Option<PendingConfirmation>,  // see §5
    pub current_request: Option<RankRequestId>,
}

pub struct RankedAction {
    pub id: ActionId,
    pub label: String,                       // verb-target wording, canonical
    pub description: Option<String>,         // secondary text
    pub category_badge: Option<String>,      // inline category indicator (small)
    pub keybinding: Option<String>,          // right-aligned shortcut display
    pub is_available: bool,
    pub disabled_reason: Option<String>,
}

pub enum PaletteOrigin {
    KeyboardShortcut,
    TriggerButton,
    ContextFallback { target: ContextualTarget },
    ProgrammaticByAction(ActionId),
}
```

### 2.4 Filtering and Ranking

Empty query: `ranked_actions` shows all actions available in the current
context, ordered by:

1. Pinned actions (user-customizable, persisted in `WorkbenchProfile`)
2. Recently used actions (per `ActionRegistry`'s recency tracking)
3. Default canonical order from `ActionRegistry`

Non-empty query: `ranked_actions` is the result of fuzzy-match scoring
against `(label, description, category)` tokens. Ranking algorithm is
runtime-side (not iced-side); the palette consumes
`ActionRegistryViewModel::rank_for_query(query, scope, view_model)`
asynchronously.

```rust
pub trait ActionRegistryViewModel {
    /// Default available action list (no query). Pinned-first, recency,
    /// then canonical order. Sync — fast enough to compute per frame.
    fn available_for_scope(&self, scope: PaletteScope) -> Vec<RankedAction>;

    /// Fuzzy-match ranked list for a query. Async — may compute on a
    /// background task for large action sets.
    fn rank_for_query(
        &self,
        query: String,
        scope: PaletteScope,
    ) -> impl Future<Output = (RankRequestId, Vec<RankedAction>)>;

    /// Selection-set availability gate.
    fn is_available(&self, action_id: ActionId, target: &SelectionSet) -> bool;
    fn disabled_reason(&self, action_id: ActionId, target: &SelectionSet) -> Option<String>;
}
```

### 2.5 Action Rows

Each row in the palette shows:

- **Action label** (verb-target wording per
  [command_surface_interaction_spec.md §3.4](../aspect_command/command_surface_interaction_spec.md)) —
  the canonical text from `ActionRegistry`, never reformatted.
- **Optional secondary text** — short description, ≤ 80 chars.
- **Optional category badge** — small inline chip showing source
  category for breadth visibility (e.g., "Graph", "Workbench",
  "View"). Category is informational only; rows are never grouped by
  it.
- **Right-aligned keybinding** — current shortcut for the action, if
  any.
- **Disabled state** — disabled rows render with reduced opacity and
  no `on_press` binding; `disabled_reason` shows in the footer when
  the disabled row is focused.

### 2.6 Message Contract

```rust
pub enum Message {
    // Open / close
    PaletteOpen { origin: PaletteOrigin },
    PaletteClose,
    PaletteCloseAndRestoreFocus,

    // Input
    PaletteQuery(String),                    // text_input on_input

    // Navigation
    PaletteFocusNext,                        // ArrowDown / Tab
    PaletteFocusPrev,                        // ArrowUp / Shift+Tab
    PaletteFocusedRowChanged(usize),         // mouse hover updates focus

    // Submit
    PaletteSubmitFocused,                    // Enter on focused row
    PaletteActionSelected(ActionId),         // click on a row

    // Async
    PaletteRankResultsReady {
        request_id: RankRequestId,
        results: Vec<RankedAction>,
    },

    // Confirmation (destructive actions, see §5)
    PaletteConfirmDispatch,
    PaletteConfirmCancel,
}
```

### 2.7 Update Routing

Sketches of the load-bearing arms:

```rust
fn update(&mut self, msg: Message) -> Task<Message> {
    match msg {
        Message::PaletteOpen { origin } => {
            // Per the 2026-04-29 omnibar-split simplification, the palette
            // stores its own focus_token (no shared CommandBarFocusTarget).
            self.command_palette = CommandPaletteState::open_for(
                origin,
                self.current_focused_widget_id(),
                self.runtime.actions().available_for_scope(PaletteScope::default()),
            );
            return widget::focus(text_input::Id::new("command_palette_input"));
        }

        Message::PaletteQuery(query) => {
            self.command_palette.query = query.clone();
            if query.is_empty() {
                // Restore default available list
                self.command_palette.ranked_actions =
                    self.runtime.actions().available_for_scope(self.command_palette.scope);
                self.command_palette.focused_index = None;
                Task::none()
            } else {
                // Spawn fuzzy rank; result returns via PaletteRankResultsReady
                let req = self.runtime.actions().next_rank_request_id();
                self.command_palette.current_request = Some(req);
                Task::perform(
                    self.runtime.actions().rank_for_query(query, self.command_palette.scope),
                    move |(rid, results)| Message::PaletteRankResultsReady {
                        request_id: rid,
                        results,
                    },
                )
            }
        }

        Message::PaletteRankResultsReady { request_id, results } => {
            // Drop stale results
            if Some(request_id) != self.command_palette.current_request {
                return Task::none();
            }
            self.command_palette.ranked_actions = results;
            self.command_palette.focused_index = (!results.is_empty()).then_some(0);
            Task::none()
        }

        Message::PaletteActionSelected(action_id) => {
            let target = self.view_model.current_selection_set();
            if !self.runtime.actions().is_available(action_id, &target) {
                self.command_palette.show_disabled_explanation_for(action_id);
                return Task::none();
            }
            // Destructive actions go through ConfirmDialog (§5)
            if self.runtime.actions().requires_confirmation_dialog(action_id, &target) {
                self.command_palette.pending_confirmation = Some(
                    PendingConfirmation::for_action(action_id, target)
                );
                return Task::none();
            }
            // Otherwise dispatch immediately
            self.runtime.emit(HostIntent::Action(ActionInvocation {
                action_id,
                target,
                origin: ActionOrigin::CommandPalette(self.command_palette.origin),
            }));
            return Task::done(Message::PaletteCloseAndRestoreFocus);
        }

        Message::PaletteCloseAndRestoreFocus => {
            let restore_target = self.command_palette.focus_token.clone();
            self.command_palette.close();
            return restore_focus_to(restore_target);
        }

        // ... other arms ...
    }
}
```

### 2.8 Focus Dance with the Omnibar

Per [`iced_omnibar_spec.md` §9](iced_omnibar_spec.md):

- The palette opens as a `Modal` overlay; pointer/keyboard input goes
  to the palette, the omnibar's `view` continues running beneath but
  is not focused.
- `command_palette.focus_token` is recorded at `PaletteOpen` time.
- On dismiss (Escape, click outside, action selected), focus returns
  to `focus_token` via `widget::focus()` `Operation`.

The omnibar and the palette never simultaneously hold input focus.

### 2.9 ActionRegistry Consumption

Every palette-rendered list is a derivation from
`ActionRegistryViewModel` against the current frame's view-model. The
palette never owns action data. No palette state aliases action truth.

The `ActionRegistry` itself is canonical (atomic registry, see
TERMINOLOGY.md §Registry Architecture). The palette is a renderer
over its projected views.

---

## 3. Context Menu

### 3.1 Invocation

Right-click on any interactable target opens a context menu scoped to
that target. Per the
[composition skeleton spec §7.3](iced_composition_skeleton_spec.md), the
target catalog is:

- Tile (in tile pane tab)
- Canvas node / canvas edge
- Frame border / Split handle
- Navigator row (Tree Spine / Activity Log)
- Swatch (Navigator Swatches bucket or expanded preview)
- Empty FrameSplitTree (canvas base layer)

### 3.2 Widget Tree

```rust
fn target_with_context_menu<'a>(
    target_widget: Element<'a, Message, GraphshellTheme>,
    target_id: ContextualTarget,
    available: &'a [RankedAction],
) -> Element<'a, Message, GraphshellTheme> {
    iced_aw::ContextMenu::new(target_widget, move || {
        column(
            available.iter().map(|action| {
                context_menu_item(action)
                    .on_press(Message::ContextMenuActionSelected {
                        target: target_id.clone(),
                        action_id: action.id,
                    })
            })
            .chain(std::iter::once(
                context_menu_separator()
            ))
            .chain(std::iter::once(
                context_menu_search_fallback()
                    .on_press(Message::PaletteOpen {
                        origin: PaletteOrigin::ContextFallback { target: target_id.clone() }
                    })
            ))
        ).into()
    })
    .into()
}
```

The context menu is a **flat list** of available actions, plus a
"Search commands…" footer entry that opens the Command Palette
pre-scoped to the target. No category tier; the context already implies
the relevant category.

### 3.3 Action Source

`ActionRegistry::available_for(target, view_model)` returns the flat
list of actions that validly apply to the right-click target. Standard
selection-set availability rules
(per [command_surface_interaction_spec.md §4.1](../aspect_command/command_surface_interaction_spec.md))
apply.

### 3.4 Dispatch

Same uphill route as the Command Palette:

```rust
Message::ContextMenuActionSelected { target, action_id } => {
    let selection = SelectionSet::from(target);
    if self.runtime.actions().requires_confirmation_dialog(action_id, &selection) {
        self.pending_confirmation = Some(PendingConfirmation::for_action(action_id, selection));
        return Task::none();
    }
    self.runtime.emit(HostIntent::Action(ActionInvocation {
        action_id,
        target: selection,
        origin: ActionOrigin::ContextMenu(target.kind()),
    }));
    Task::none()
}
```

The context menu dismisses on action selection, click outside, or
Escape — `iced_aw::ContextMenu` handles dismissal automatically.

### 3.5 Mode Switch (Context → Command Palette)

The "Search commands…" footer entry in any context menu emits
`Message::PaletteOpen { origin: ContextFallback { target } }`. The
palette opens with `scope = PaletteScope::CurrentTarget` and the same
`SelectionSet` derived from the right-click target. This gives the
user keyboard-driven escape into the full command set when the
context-menu list isn't enough.

Reverse switch (Palette → Context Menu) is not supported — once the
palette is open, the user dismisses it explicitly to return to a
context-menu flow.

---

## 4. Verb-Target Wording (canonical pass-through)

Per [command_surface_interaction_spec.md §3.4](../aspect_command/command_surface_interaction_spec.md):

> Command labels must follow explicit `Verb + Target (+ Destination/Scope when needed)` grammar.

iced rendering passes the canonical label through verbatim. Both the
palette and the context menu render `RankedAction.label` unchanged.

If a future iced styling pass adds inline icons or color cues, those
sit alongside the canonical label, not as substitutes.

---

## 5. Destructive Action Confirmation

Destructive actions (Tombstone, Remove edge, Discard frame snapshot,
etc.) carry a confirmation step. Iced realization:

- Action's `requires_confirmation_dialog` flag (from `ActionRegistry`)
  gates a `ConfirmDialog` modal that intercepts the dispatch path.
- The dialog shows: action name, target description (which nodes /
  edges / frames), and "Confirm" / "Cancel" buttons.
- Confirm dispatches the `HostIntent` and closes both palette/menu and
  dialog; Cancel closes the dialog and restores palette/menu focus.
- Keyboard: Enter confirms, Escape cancels.

```rust
fn confirm_dialog(state: &State) -> Option<Element<Message>> {
    state.command_palette.pending_confirmation.as_ref().map(|p| {
        modal(
            container(column![
                text(p.action_name.clone()).size(18),
                text(p.target_description.clone()),
                vertical_space(),
                row![
                    button("Cancel").on_press(Message::PaletteConfirmCancel),
                    horizontal_space(),
                    button("Confirm").on_press(Message::PaletteConfirmDispatch),
                ]
            ])
        ).into()
    })
}
```

The same `pending_confirmation` field handles both palette-initiated
and context-menu-initiated destructive actions; only one
`ConfirmDialog` is ever active at a time.

Single-step destructive actions (e.g., explicit Tombstone-with-acknowledgment)
skip the confirmation dialog if `ActionRegistry::requires_confirmation_dialog`
returns false; `is_destructive` is a description, not the gate.

---

## 6. Coherence Guarantee Restated

Per [iced jump-ship plan §4.10](2026-04-28_iced_jump_ship_plan.md):

> **Command palette**: Selecting an action emits a single `HostIntent`;
> the action only takes effect once the receiving authority confirms via
> `IntentApplied`. Confirmation appears in the Activity Log; unconfirmed
> actions never silently apply.
>
> **Context palette**: Right-click never mutates graph truth on its own.
> Each action in the menu emits an explicit intent; destructive actions
> carry confirmation; non-destructive actions take effect immediately
> and appear in the Activity Log.

This spec preserves both:

- `PaletteActionSelected` and `ContextMenuActionSelected` arms always
  emit one `HostIntent::Action(...)`; neither surface mutates graph /
  workbench / shell state directly.
- After dispatch, the palette/menu closes — neither shows "success";
  the Activity Log is the canonical confirmation surface.
- Failed actions surface via toast (per the iced jump-ship plan §12.2)
  and a row in the Activity Log; surfaces do not silently swallow
  failures.
- Disabled actions never dispatch (§2.5); the disabled-reason text is
  shown instead.
- Destructive actions route through `ConfirmDialog` (§5).

---

## 7. Accessibility

Per [command_surface_interaction_spec.md §4.6](../aspect_command/command_surface_interaction_spec.md):

- **Keyboard equivalents** for both surfaces (Tab / Arrow keys
  navigate; Enter dispatches; Escape dismisses).
- **AccessKit roles**:
  - Command Palette Modal → `dialog`
  - Palette `text_input` → `searchbox`
  - Palette result list → `listbox`, rows → `option`
  - Context Menu → `menu`, items → `menuitem`
- **Live region** on the palette result list announces selection
  changes during keyboard navigation.
- **Focus appearance** meets WCAG 2.2 AA SC 2.4.11 via `iced::Theme`.
- **Target size** for action rows ≥ 32×32 dp (SC 2.5.8).
- **Disabled-reason** text reaches AT users via a `description`
  attribute on the disabled row (not just a tooltip).

These targets land at Stage E (per the iced jump-ship plan §12.3) and
gate via the `UxProbeSet` AT-validation contract.

---

## 8. Provenance and Diagnostics

Both surfaces participate in the command-surface provenance contract
per [command_surface_observability_and_at_plan.md](../subsystem_ux_semantics/2026-04-05_command_surface_observability_and_at_plan.md):

- `Message::PaletteOpen` → `command-surface.palette` `opened` event
  with origin / scope / focused-target snapshot.
- `Message::PaletteActionSelected` (or `ContextMenuActionSelected`)
  → `command-surface.palette` (or `.context-menu`) `dispatched` event
  with action ID, target selection, origin, and resolution result
  (`resolved` / `blocked` / `fallback` / `no-target`).
- Disabled-action attempts emit a `blocked` receipt with the
  `disabled_reason`.
- Palette dismiss emits a `dismissed` receipt; if the focus restore
  failed (stale return target), an explicit `fallback` receipt
  records what happened.

The trace shape is owned by `subsystem_ux_semantics`; the palette and
context menu use the existing landed trace path.

---

## 9. Retired Surfaces (memory only)

The following models from prior drafts are retired and should not be
re-added without an explicit design pass:

- **Search Palette Mode + Context Palette Mode distinction**: collapsed
  into a single Command Palette. The "search input" *is* the palette's
  affordance; there is no Context Palette Mode separately invoked.
- **Two-tier (Tier 1 categories + Tier 2 options)**: replaced by flat
  ranked list. Categories appear only as inline badges on action rows
  for breadth visibility; rows are not grouped by category.
- **Cross-mode equivalence rule** (Tier 1 strip = Tier 1 ring):
  retired alongside Tier 1.
- **Radial Palette Mode**: deferred indefinitely. Was originally
  gamepad-oriented per [iced jump-ship plan §11 G2](2026-04-28_iced_jump_ship_plan.md).
  If gamepad input lands later, radial can be reintroduced as a third
  surface; the geometry research in
  [`../aspect_command/radial_menu_geometry_and_overflow_spec.md`](../aspect_command/radial_menu_geometry_and_overflow_spec.md)
  is preserved for that future pass.

These were specified in the canonical
[command_surface_interaction_spec.md](../aspect_command/command_surface_interaction_spec.md)
prior to 2026-04-29; the canonical spec was revised in the same commit
that landed this version of the iced spec.

---

## 10. Open Items

- **Pinned actions UI**: pinning surface (probably right-click on an
  action row in the palette → "Pin to top"). Not specified here.
- **Inline command syntax** (e.g., `>action-name args`): potential
  affordance for power users in the palette. Tracked in
  `iced_omnibar_spec.md` §12.
- **Action ranking algorithm**: out of scope; lives in `ActionRegistry`
  via runtime view-model.
- **Action history / recently-used persistence**: runtime / settings
  concern, not iced rendering. Read from `WorkbenchProfile`.
- **Keyboard shortcut customization**: routed through settings; the
  palette displays the current keybinding next to each action via
  `ActionRegistry`'s shortcut metadata.
- **Visual polish**: row styling, category-badge colors, modal
  enter/exit animation. Stage F polish, not skeleton concern.

---

## 11. Bottom Line

The iced command surfaces are **two**: a Command Palette
(`Modal` + `text_input` + flat ranked list, Zed/VSCode-shaped) and a
Context Menu (`iced_aw::ContextMenu` with a flat list). Both source
actions from `ActionRegistry`; both dispatch one `HostIntent::Action`
per selection through the uphill rule. Search-as-filter is the palette's
discovery mechanism, not a separate mode. Two-tier rendering, Search /
Context mode distinction, and Radial Palette Mode are retired per the
2026-04-29 simplification. AccessKit roles and keyboard equivalents
land at Stage E. Destructive actions go through `ConfirmDialog`.

The canonical UX (action source, verb-target wording, accessibility
targets, acceptance criteria) lives in
[command_surface_interaction_spec.md](../aspect_command/command_surface_interaction_spec.md)
(also revised 2026-04-29); this spec is the iced renderer.

This closes the third concrete S2 sub-deliverable in its simplified
form. Together with the composition skeleton, omnibar, browser
amenities, and coherence guarantees, the iced command-surface story
is anchored for S4 implementation.
