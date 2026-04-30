<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Iced Command Palette Spec

**Date**: 2026-04-29
**Status**: Canonical / Active — third concrete S2 deliverable for the iced jump-ship plan
**Scope**: The iced-side rendering and event routing for the canonical
command palette modes defined in
[`../aspect_command/command_surface_interaction_spec.md`](../aspect_command/command_surface_interaction_spec.md).
Covers Search Palette Mode and Context Palette Mode in full; Radial Palette
Mode is stubbed (deferred to the input-rework lane). Reuses the canonical
two-tier palette contract, selection-set availability rules, verb-target
wording, and `ActionRegistry` source — does not redefine.

**Code-sample mode**: **Illustrative signatures** (per the
[spec-code-samples feedback memory](../../../../.claude/projects/c--Users-mark--Code/memory/feedback_spec_code_samples_illustrative_vs_implementation_ready.md)).
Concrete S3/S4 code lives in the implementation, not this spec.

**Related**:

- [`../aspect_command/command_surface_interaction_spec.md`](../aspect_command/command_surface_interaction_spec.md) — **canonical** palette UX, two-tier contract, three modes, ActionRegistry source, verb-target wording, accessibility, acceptance criteria
- [`../aspect_command/ASPECT_COMMAND.md`](../aspect_command/ASPECT_COMMAND.md) — Command aspect authority
- [`../aspect_command/radial_menu_geometry_and_overflow_spec.md`](../aspect_command/radial_menu_geometry_and_overflow_spec.md) — radial geometry policy
- [`iced_composition_skeleton_spec.md`](iced_composition_skeleton_spec.md) — Application skeleton (§1.5), CommandBar slot (§7.2), context palette (§7.3), uphill rule routing (§8)
- [`iced_omnibar_spec.md`](iced_omnibar_spec.md) — sibling spec; shares `CommandBarFocusTarget` and focus-dance contract
- [`shell_composition_model_spec.md` §5](shell_composition_model_spec.md) — `CommandBarFocusTarget`, host-thread contract
- [`2026-04-28_iced_jump_ship_plan.md` §4.10](2026-04-28_iced_jump_ship_plan.md) — coherence guarantees for command palette and context palette
- [`../subsystem_ux_semantics/2026-04-05_command_surface_observability_and_at_plan.md`](../subsystem_ux_semantics/2026-04-05_command_surface_observability_and_at_plan.md) — provenance / AT validation contract

---

## 1. Intent and Boundary

The canonical palette UX is fully specified in
[command_surface_interaction_spec.md](../aspect_command/command_surface_interaction_spec.md).
This spec covers the iced-side rendering and Message routing only:

- which iced widgets render each canonical mode
- the Message contract for invocation, navigation, dispatch, dismiss
- focus dance with the omnibar and the prior-focused surface
- how `ActionRegistry` projects into iced view-model state per frame
- where two-tier vs single-list rendering diverges in the iced widget tree
- coherence guarantee restated for the iced implementation

This spec does **not** redefine: action meaning, action availability rules,
two-tier semantics, category ordering policy, verb-target wording, or
acceptance criteria. Those live in the canonical aspect_command spec.

A change to canonical palette UX lives in the aspect_command spec; this
spec re-renders the change. A change to iced widget choice or Message
shape lives here.

---

## 2. Three Modes — iced Realization

Per [command_surface_interaction_spec.md §3.1](../aspect_command/command_surface_interaction_spec.md):

| Mode | Canonical UX | iced realization | Status |
|---|---|---|---|
| **Search Palette Mode** | Searchable list + scope dropdown | `Modal` overlay with `text_input` + scrollable two-tier rendering | Specified here |
| **Context Palette Mode** | Right-click contextual shell with two-tier category/option | `iced_aw::ContextMenu` triggered on right-click, two-tier renderer inside | Specified here |
| **Radial Palette Mode** | Positional radial-menu invocation (gamepad / touch / long-press) | Custom `canvas::Program` overlay rendering radial geometry | **Deferred** (see §10) |

Search and Context Palette Modes are the iced bring-up targets; both are
common-case mouse/keyboard surfaces. Radial Palette Mode is deferred
until the input subsystem rework lands per the iced jump-ship plan §11 G2
and the existing radial_menu_geometry_and_overflow_spec.md.

---

## 3. Search Palette Mode

### 3.1 Invocation

Trigger sources:

- **`Ctrl+K`** (canonical) — global keyboard shortcut, captured by
  the iced application's keyboard subscription
- **`F2`** (canonical alternate) — same shortcut
- **CommandBar trigger button** click — emits `Message::PaletteOpen`
- **Context Palette → Search fallback** — when contextual resolution
  fails or user explicitly switches modes; emits `Message::PaletteOpen`
  with `PaletteOrigin::ContextFallback`
- **Programmatic** — actions or commands that open the palette as
  part of their effect (rare; emits the same `Message::PaletteOpen`)

All paths converge on `Message::PaletteOpen { origin }`. Origin is
recorded for diagnostics provenance per
[`subsystem_ux_semantics/2026-04-05_command_surface_observability_and_at_plan.md`](../subsystem_ux_semantics/2026-04-05_command_surface_observability_and_at_plan.md).

### 3.2 Widget Tree

```rust
fn search_palette_overlay(state: &State) -> Option<Element<'_, Message, GraphshellTheme>> {
    state.command_palette.is_open.then(|| {
        modal(
            container(
                column![
                    palette_header(state),                   // "Commands" + scope dropdown
                    text_input(
                        &state.command_palette.query,
                        Message::PaletteQuery,
                    )
                    .on_submit(Message::PaletteSubmitFocused)
                    .id(text_input::Id::new("command_palette_input")),

                    horizontal_rule(),

                    two_tier_action_view(
                        &state.command_palette.tier1,
                        &state.command_palette.tier2,
                        state.command_palette.tier_focus,
                    ),

                    palette_footer(state),                   // disabled-action explanation, hint text
                ]
                .spacing(8)
                .padding(12)
            )
            .style(palette_container_style)
            .max_width(560)
        )
        .on_blur(Message::PaletteClose)
        .into()
    })
}
```

### 3.3 Two-Tier Rendering

Per [command_surface_interaction_spec.md §3.3](../aspect_command/command_surface_interaction_spec.md),
all contextual palette modes (and Search Palette Mode in its
contextual-results state) use two tiers.

- **Tier 1** — horizontally scrollable category strip with pinned
  categories and user-editable order. Iced realization:
  `scrollable` containing a `row!` of category chip buttons.
- **Tier 2** — vertically scrollable command list for the selected
  Tier 1 category. Iced realization: `scrollable` containing a
  `column!` of action rows.

```rust
fn two_tier_action_view<'a>(
    tier1: &'a [PaletteCategory],
    tier2: &'a [PaletteAction],
    tier_focus: TierFocus,
) -> Element<'a, Message, GraphshellTheme> {
    column![
        // Tier 1
        scrollable(
            row(tier1.iter().enumerate().map(|(i, cat)| {
                category_chip(cat, tier_focus.is_tier1(i))
                    .on_press(Message::PaletteCategorySelected(cat.id))
            }))
            .spacing(4)
        )
        .direction(scrollable::Direction::Horizontal(...)),

        // Tier 2
        scrollable(
            column(tier2.iter().enumerate().map(|(i, action)| {
                action_row(action, tier_focus.is_tier2(i))
                    .on_press(Message::PaletteActionSelected(action.id))
            }))
            .spacing(2)
        ),
    ]
    .into()
}
```

### 3.4 Single-List Rendering (search-result mode)

When a non-empty query is active in Search Palette Mode, the two-tier
view collapses to a flat ranked list:

- Tier 1 strip hides (or shows category chips as filter affordances
  with a "any" sentinel selected by default).
- Tier 2 becomes the ranked search-result list, showing matched
  actions across categories.
- Action rows show the source category as a small badge so the user
  can see breadth/grouping.

Mode switch is determined by `state.command_palette.query.is_empty()`.
The transition is render-only; underlying state shape doesn't change.

### 3.5 State Shape

```rust
pub struct CommandPaletteState {
    pub is_open: bool,
    pub mode: PaletteMode,                 // Search | Context (Radial out of scope)
    pub origin: PaletteOrigin,             // diagnostics provenance
    pub query: String,                     // empty when in two-tier; non-empty when in single-list
    pub scope: PaletteScope,               // CurrentTarget | ActivePane | ActiveGraph | Workbench
    pub tier1: Vec<PaletteCategory>,       // current categories
    pub tier2: Vec<PaletteAction>,         // current actions for selected category, or ranked results
    pub tier_focus: TierFocus,             // keyboard focus position
    pub selected_category: Option<CategoryId>,
    pub focus_at_open: Option<FocusedSurface>,  // for Esc/dismiss restore
}

pub enum PaletteMode {
    Search,
    Context { target: ContextualTarget },  // see §4
    // Radial — deferred
}

pub enum PaletteOrigin {
    KeyboardShortcut,
    TriggerButton,
    ContextFallback,
    ProgrammaticByAction(ActionId),
    RightClickOn(ContextualTarget),
}

pub enum TierFocus {
    Tier1(usize),
    Tier2(usize),
    Input,
}
```

### 3.6 Message Contract

```rust
pub enum Message {
    // ... global from §1.5 of composition skeleton spec ...

    // Open / close
    PaletteOpen { origin: PaletteOrigin },
    PaletteClose,
    PaletteCloseAndRestoreFocus,

    // Input
    PaletteQuery(String),                  // text_input on_input
    PaletteScopeChanged(PaletteScope),

    // Navigation
    PaletteCategorySelected(CategoryId),   // Tier 1 click
    PaletteActionSelected(ActionId),       // Tier 2 click / row press
    PaletteKeyArrowDown,
    PaletteKeyArrowUp,
    PaletteKeyArrowRight,                  // Tier 1 → Tier 2 (or category next)
    PaletteKeyArrowLeft,                   // Tier 2 → Tier 1 (or category prev)
    PaletteKeyTab,                         // input → Tier 1 → Tier 2 cycle
    PaletteKeyEscape,                      // dismiss; restore focus

    // Submit
    PaletteSubmitFocused,                  // Enter on focused row
    PaletteSubmitDispatched { action_id: ActionId, target: SelectionSet },

    // Lifecycle / async
    PaletteCategoryActionsLoaded { category: CategoryId, actions: Vec<PaletteAction> },
    PaletteSearchResultsReady { generation: u64, results: Vec<PaletteAction> },
}
```

### 3.7 Update Routing

Sketches of the load-bearing arms:

```rust
fn update(&mut self, msg: Message) -> Task<Message> {
    match msg {
        Message::PaletteOpen { origin } => {
            self.command_palette = CommandPaletteState::open_for(
                origin,
                /* focus_at_open */ self.view_model.focus_target.focused_surface.clone(),
                /* canonical defaults: */
                PaletteMode::Search,
                self.runtime.actions().tier1_for_scope(PaletteScope::default()),
            );
            return widget::focus(text_input::Id::new("command_palette_input"));
        }

        Message::PaletteQuery(query) => {
            self.command_palette.query = query.clone();
            if query.is_empty() {
                // Restore two-tier view from the selected category
                self.command_palette.tier2 = self.runtime.actions()
                    .tier2_for(self.command_palette.selected_category, &self.view_model);
                Task::none()
            } else {
                // Spawn ranked search; result returns via PaletteSearchResultsReady
                self.command_palette.tier_focus = TierFocus::Tier2(0);
                let gen = self.command_palette.next_generation();
                Task::perform(
                    self.runtime.actions().rank_for_query(query, &self.view_model),
                    move |results| Message::PaletteSearchResultsReady { generation: gen, results }
                )
            }
        }

        Message::PaletteActionSelected(action_id) => {
            // Resolve the selection-set per command_surface_interaction_spec.md §4.1
            let target = self.view_model.current_selection_set();
            // Availability gate: action must apply to every object in the set
            if !self.runtime.actions().is_available(action_id, &target) {
                // Show disabled-state explanation; do not dispatch
                self.command_palette.show_disabled_explanation_for(action_id);
                return Task::none();
            }
            // Dispatch via the canonical ActionRegistry::execute path
            self.runtime.emit(HostIntent::Action(ActionInvocation {
                action_id,
                target,
                origin: ActionOrigin::CommandPalette(self.command_palette.origin),
            }));
            // Close palette and restore focus per §3.8
            return Task::done(Message::PaletteCloseAndRestoreFocus);
        }

        Message::PaletteKeyEscape => {
            return Task::done(Message::PaletteCloseAndRestoreFocus);
        }

        Message::PaletteCloseAndRestoreFocus => {
            let restore_target = self.command_palette.focus_at_open.clone();
            self.command_palette.close();
            return restore_focus_to(restore_target);
        }

        // ... other arms ...
    }
}
```

### 3.8 Focus Dance with the Omnibar

Per [`iced_omnibar_spec.md` §9](iced_omnibar_spec.md):

- The palette opens as a Modal overlay; pointer/keyboard input goes
  to the palette, the omnibar's `view` continues running beneath but
  is not focused.
- `command_palette.focus_at_open` is recorded at `PaletteOpen` time.
- On dismiss (Escape, click outside, action selected), focus returns
  to `focus_at_open` via `widget::focus()` `Operation`.
- If `focus_at_open` was the omnibar in Input mode, the omnibar
  stays in Input mode under the palette and re-receives focus on
  dismiss.
- A palette action that targets the omnibar (e.g., "Focus omnibar")
  emits `Message::OmnibarFocus` after `PaletteCloseAndRestoreFocus`,
  overriding the restore.

The omnibar and the palette never simultaneously hold input focus.

### 3.9 ActionRegistry Consumption

The palette never owns action data — it's always projected from the
canonical `ActionRegistry` (atomic registry, see TERMINOLOGY.md §Registry
Architecture).

Per-frame projection contract:

```rust
// What the runtime exposes via FrameViewModel
pub trait ActionRegistryViewModel {
    fn tier1_for_scope(&self, scope: PaletteScope) -> Vec<PaletteCategory>;
    fn tier2_for(
        &self,
        category: Option<CategoryId>,
        view_model: &FrameViewModel,
    ) -> Vec<PaletteAction>;
    fn rank_for_query(
        &self,
        query: String,
        view_model: &FrameViewModel,
    ) -> impl Future<Output = Vec<PaletteAction>>;
    fn is_available(&self, action_id: ActionId, target: &SelectionSet) -> bool;
    fn disabled_reason(&self, action_id: ActionId, target: &SelectionSet) -> Option<DisabledReason>;
}
```

Every palette-rendered list is a derivation from this trait against the
current frame's view-model. No palette state aliases action truth.

### 3.10 Disabled Actions

Per [command_surface_interaction_spec.md §4.1](../aspect_command/command_surface_interaction_spec.md):

> Silent fallback to a hidden primary target is forbidden.
> Silent command no-op behavior is forbidden.

Iced realization:

- Disabled actions render with reduced opacity and no `on_press`
  binding, so click does nothing.
- Hovering or focusing a disabled action shows the
  `DisabledReason` (e.g., "Selection contains an Edge; edges cannot
  be opened in a Pane") in the palette footer or as a tooltip.
- Submitting a disabled action via Enter is intercepted in
  `update`; it shows the explanation but does not dispatch.

This guarantees the user sees why every visible action is unavailable;
they never have to guess.

---

## 4. Context Palette Mode

### 4.1 Invocation

Right-click on any interactable target opens Context Palette Mode scoped
to that target. Per the
[composition skeleton spec §7.3](iced_composition_skeleton_spec.md), the
target catalog is:

- Tile (in tile pane tab)
- Canvas node / canvas edge
- Frame border / Split handle
- Navigator row (Tree Spine / Activity Log)
- Swatch (Navigator Swatches bucket or expanded preview)
- Empty FrameSplitTree (canvas base layer)

Iced realization: `iced_aw::ContextMenu` mounted on the target widget,
triggered by right-click event. The `ContextMenu` builder takes a
closure that returns the menu element; we pass the same two-tier renderer
as Search Palette Mode.

```rust
fn tile_pane_with_context_menu<'a>(...) -> Element<'a, Message, GraphshellTheme> {
    iced_aw::ContextMenu::new(
        tile_pane_body(...),
        || context_palette_menu(target_id),
    )
    .into()
}

fn context_palette_menu(target: ContextualTarget) -> Element<'_, Message, GraphshellTheme> {
    // Reuse the two-tier renderer; tier1/tier2 are populated by
    // ActionRegistry::tier1_for_target(target).
    two_tier_action_view(...)
}
```

### 4.2 Mode-Specific Behavior

Differences from Search Palette Mode:

- **No search input**. The context palette starts directly in the
  two-tier view scoped to the target.
- **Origin** is `RightClickOn(ContextualTarget)`.
- **Anchored to the right-click position** (per
  `iced_aw::ContextMenu` placement).
- **Dismiss-on-blur**: clicking outside the menu dismisses it.
- **Search fallback**: a "Search…" entry at the bottom of Tier 1
  switches to Search Palette Mode with the same target as scope, per
  [command_surface_interaction_spec.md §3.3](../aspect_command/command_surface_interaction_spec.md).

The Message contract is the same as Search Palette Mode (Open / Close /
KeyArrows / ActionSelected / SubmitFocused). Only `PaletteQuery` and the
single-list rendering path don't apply to Context Palette Mode (until
the user switches to Search via the fallback).

### 4.3 Mode Switch (Context → Search)

When the user picks "Search…" from a context palette:

1. `Message::PaletteOpen { origin: ContextFallback }` fires.
2. The Context Palette Mode dismisses (via `iced_aw::ContextMenu`'s
   blur handling).
3. Search Palette Mode opens centered (Modal), scope preset to the
   right-click target.
4. Focus moves to the search input.
5. The contextual target's available actions appear as the initial
   ranked list.

Reverse switch (Search → Context) is not supported in this spec —
once Search Palette Mode is open, the user dismisses it explicitly to
return to a contextual flow.

---

## 5. Verb-Target Wording (canonical pass-through)

Per [command_surface_interaction_spec.md §3.4](../aspect_command/command_surface_interaction_spec.md):

> Command labels must follow explicit `Verb + Target (+ Destination/Scope when needed)` grammar.

iced rendering passes the canonical label through verbatim. The palette
never re-formats action labels, never abbreviates `Delete Selected
Node(s)` to `Delete`, never substitutes `Close` for graph-content
deletion. Label content is `ActionRegistry`'s responsibility; the
palette is a renderer.

If a future iced styling pass adds inline icons or color cues, those
sit alongside the canonical label, not as substitutes.

---

## 6. Coherence Guarantee Restated

Per [iced jump-ship plan §4.10](2026-04-28_iced_jump_ship_plan.md):

> **Command palette**: Selecting an action emits a single `HostIntent`;
> the action only takes effect once the receiving authority confirms via
> `IntentApplied`. Confirmation appears in the Activity Log; unconfirmed
> actions never silently apply.

This spec preserves the guarantee:

- `PaletteActionSelected` arms always emit one `HostIntent::Action(...)`
  via `runtime.emit`; the palette never mutates graph / workbench /
  shell state directly.
- The palette closes after dispatch but does not "show success"; the
  Activity Log is the canonical confirmation surface.
- A failed action surfaces via toast (per the iced jump-ship plan §12.2
  toast path) and a row in the Activity Log; the palette does not
  silently swallow failures.
- Disabled actions never dispatch (§3.10); the disabled-reason text is
  shown instead.

> **Context palette**: Right-click never mutates graph truth on its own.
> Each action in the menu emits an explicit intent; destructive actions
> (Tombstone, Remove edge) carry confirmation; non-destructive actions
> take effect immediately and appear in the Activity Log.

The Context Palette Mode shares the Search Palette Mode's dispatch
path, so the same guarantee applies. Destructive action confirmation
is implemented via a `ConfirmDialog` widget overlaid before dispatch
(per §7).

---

## 7. Destructive Action Confirmation

Destructive actions (Tombstone, Remove edge, Discard frame snapshot,
etc.) carry a confirmation step. Iced realization:

- The action's `is_destructive` flag (from `ActionRegistry`) gates a
  `ConfirmDialog` modal that intercepts `PaletteActionSelected`.
- The dialog shows: action name, target description (which nodes /
  edges / frames), and "Confirm" / "Cancel" buttons.
- Confirm dispatches the `HostIntent` and closes both palette and
  dialog; Cancel closes the dialog and restores palette focus.
- Keyboard: Enter confirms, Escape cancels.

```rust
fn confirm_dialog(state: &State) -> Option<Element<Message>> {
    state.command_palette.pending_confirmation.as_ref().map(|p| {
        modal(
            container(column![
                text(p.action_name.clone()),
                text(p.target_description.clone()),
                row![
                    button("Cancel").on_press(Message::PaletteConfirmCancel),
                    button("Confirm").on_press(Message::PaletteConfirmDispatch),
                ]
            ])
        ).into()
    })
}
```

Single-step destructive actions (e.g., Tombstone with explicit
acknowledgment) skip the confirmation dialog if the action's
canonical definition marks them as such; `is_destructive` is
necessary but `requires_confirmation_dialog` is the gate.

---

## 8. Accessibility

Per [command_surface_interaction_spec.md §4.6](../aspect_command/command_surface_interaction_spec.md):

- **Keyboard equivalents** for every palette mode (Tab cycles input →
  Tier1 → Tier2; arrow keys navigate within tier; Enter confirms;
  Escape dismisses).
- **AccessKit roles**: Modal palette is a `dialog`; text_input is a
  `searchbox`; Tier 1 chips are a `tablist` of `tab` roles; Tier 2 is a
  `listbox` of `option` roles. Context palette is a `menu` of
  `menuitem` roles.
- **Live region** on the Tier 2 list announces selection changes
  during keyboard navigation.
- **Focus appearance** meets WCAG 2.2 AA SC 2.4.11 via iced::Theme.
- **Target size** for action rows ≥ 32×32 dp (SC 2.5.8).
- **Disabled-reason** text reaches AT users via a `description`
  attribute on the disabled row (not just a tooltip).

These targets land at Stage E (per the iced jump-ship plan §12.3) and
gate via the `UxProbeSet` AT-validation contract per
[`subsystem_ux_semantics/2026-04-05_command_surface_observability_and_at_plan.md`](../subsystem_ux_semantics/2026-04-05_command_surface_observability_and_at_plan.md).

---

## 9. Provenance and Diagnostics

The palette participates in the command-surface provenance contract
per [command_surface_observability_and_at_plan.md](../subsystem_ux_semantics/2026-04-05_command_surface_observability_and_at_plan.md):

- Each `Message::PaletteOpen` records origin, scope, and timing.
- Each `Message::PaletteActionSelected` (and the resulting
  `HostIntent::Action` dispatch) emits a `command-surface.palette`
  trace event with the action ID, target selection, origin, and
  resolution result (resolved / blocked / fallback / no-target).
- Disabled-action attempts emit a `blocked` receipt with the
  `DisabledReason`.
- Palette dismiss emits a `dismissed` receipt; if the focus restore
  failed (stale return target), an explicit `fallback` receipt
  records what happened.

Provenance is the iced surface's obligation; the trace shape is owned
by `subsystem_ux_semantics`. The palette uses the existing landed
trace path; it does not invent a parallel diagnostic channel.

---

## 10. Radial Palette Mode (Deferred Stub)

Per the iced jump-ship plan §11 G2 and the existing
[`radial_menu_geometry_and_overflow_spec.md`](../aspect_command/radial_menu_geometry_and_overflow_spec.md),
Radial Palette Mode is deferred until the input subsystem rework lands.
The deferred shape:

- iced realization: custom `canvas::Program` overlay rendering radial
  geometry. Hit testing handled by the Program.
- Same two-tier contract as Context Palette Mode (per
  [command_surface_interaction_spec.md §3.3](../aspect_command/command_surface_interaction_spec.md)
  cross-mode equivalence rule): Tier 1 ring = category strip, Tier 2
  ring = option list.
- Invocation: gamepad button hold, touch long-press, mouse press-and-hold.
- Dispatch path: same `Message::PaletteActionSelected` as the other
  modes.

Until the input rework, Radial Palette Mode is not buildable; the
command-action set still works through Search and Context Palette
Modes for keyboard / mouse users.

---

## 11. Open Items

- **Inline command syntax** (e.g., `>action-name args`): not yet
  specified for either Search Palette Mode or the omnibar. Tracked in
  iced_omnibar_spec.md §12.
- **Action ranking algorithm**: out of scope for this spec; lives in
  the canonical aspect_command material. The iced palette consumes
  whatever ranking the runtime exposes.
- **Palette state persistence across sessions** (recently-used
  actions, pinned categories): runtime / settings concern, not iced
  rendering. The palette reads pinned/order state from `WorkbenchProfile`
  via the runtime view-model.
- **Keyboard shortcut customization**: routed through settings; the
  palette displays the current keybinding next to each action via
  `ActionRegistry`'s shortcut metadata.
- **Tier 1 horizontal scroll vs wrap**: design polish question.
  Spec uses scroll per
  [command_surface_interaction_spec.md §4.2](../aspect_command/command_surface_interaction_spec.md);
  if a wider screen makes wrap preferable, the palette can switch
  rendering mode based on available width.
- **Style parity with omnibar dropdown completions**: visual unification
  question for Stage F polish; not skeleton concern.

---

## 12. Bottom Line

The iced command palette is one `Modal` overlay (Search Palette Mode)
plus `iced_aw::ContextMenu` instances (Context Palette Mode), both
rendering the canonical two-tier action view sourced from
`ActionRegistry`. State lives in `CommandPaletteState`; mutations route
through Messages; action dispatch emits `HostIntent::Action` once and
closes the palette without waiting for confirmation, which arrives via
Activity Log. Disabled actions render with explicit reasons. Destructive
actions carry `ConfirmDialog` modal gates. AccessKit roles and keyboard
equivalents land at Stage E. Radial Palette Mode is deferred. The
canonical UX (two-tier model, three modes, ActionRegistry source,
verb-target wording, acceptance criteria) lives in
[command_surface_interaction_spec.md](../aspect_command/command_surface_interaction_spec.md);
this spec is the iced renderer.

This closes the third concrete S2 sub-deliverable. Together with the
composition skeleton, omnibar, and coherence guarantees, the iced
command-surface story is fully anchored for S4 implementation.
