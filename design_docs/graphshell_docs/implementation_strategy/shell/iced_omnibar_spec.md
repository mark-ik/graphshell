<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Iced Omnibar Spec

**Date**: 2026-04-29 (revised)
**Status**: Canonical / Active — S2 sub-deliverable for the iced jump-ship plan
**Scope**: The iced-side omnibar widget within the `CommandBar` slot. **URL
entry + Navigator breadcrumb display only.** Per the 2026-04-29 simplification,
the omnibar is no longer a multi-role chrome — graph search by title/tag goes
through the [Node Finder](iced_node_finder_spec.md), command invocation goes
through the [Command Palette](iced_command_palette_spec.md), and
`CommandBarFocusTarget` is retired
([shell_composition_model_spec.md §5.4](shell_composition_model_spec.md)).

**Code-sample mode**: **Illustrative signatures**. Concrete S3/S4 code lives
in the implementation, not this spec.

**Related**:

- [`iced_composition_skeleton_spec.md`](iced_composition_skeleton_spec.md) — Application skeleton, CommandBar slot
- [`iced_node_finder_spec.md`](iced_node_finder_spec.md) — sibling surface for graph-node fuzzy search (`Ctrl+P`)
- [`iced_command_palette_spec.md`](iced_command_palette_spec.md) — sibling surface for command invocation (`Ctrl+Shift+P`)
- [`shell_composition_model_spec.md` §5](shell_composition_model_spec.md) — canonical omnibar seam (`NavigatorContextProjection`, `BreadcrumbPath`); §5.4 CommandBarFocusTarget retired
- [`../aspect_command/command_surface_interaction_spec.md` §4.5](../aspect_command/command_surface_interaction_spec.md) — canonical Omnibar contract (revised 2026-04-29 same-day; commands explicitly out of scope)
- [`2026-04-03_shell_command_bar_execution_plan.md`](2026-04-03_shell_command_bar_execution_plan.md) — egui-era plan; landed seams reused; egui rendering paths frozen
- [`2026-04-28_iced_jump_ship_plan.md` §4.10](2026-04-28_iced_jump_ship_plan.md) — coherence guarantee for the omnibar
- [`../navigator/NAVIGATOR.md` §6](../navigator/NAVIGATOR.md) — Navigator cross-surface verb mapping

---

## 1. Intent

The omnibar is the URL/address entry surface and the canonical home for the
Navigator's read-only breadcrumb projection. It does one thing: open or
activate a node by its canonical address.

What the omnibar **does**:

- accepts URL/address input (typed or pasted);
- shows URL completions from history-by-URL and bookmark-URL providers;
- displays the Navigator-projected breadcrumb in Display mode;
- routes ambiguous (non-URL-shaped) input to the Node Finder rather than
  silently retargeting.

What the omnibar **does not** do (per 2026-04-29 simplification):

- ❌ search graph nodes by title / tag / content → that's the
  [Node Finder](iced_node_finder_spec.md).
- ❌ invoke commands → that's the
  [Command Palette](iced_command_palette_spec.md).
- ❌ resolve a focused-target carrier → `CommandBarFocusTarget` retired
  ([shell_composition_model_spec.md §5.4](shell_composition_model_spec.md)).
- ❌ host inline command syntax (`>action-name args`, `:lens-name`, etc.).
- ❌ host fullscreen-strip behavior beyond the basic Display mode (which
  already covers the "show current address" case).

This is a deliberate narrowing. The egui-era omnibar conflated four roles
(URL, search, command, breadcrumb) into one widget; the simplification
gives each role its own focused surface.

---

## 2. Modes

The omnibar has two rendering modes (down from three in the prior draft —
Fullscreen mode is dropped; if a future fullscreen content surface needs an
address chip, that's its own chrome, not an omnibar mode).

| Mode | Trigger | Visible | Focus |
|---|---|---|---|
| **Display** | Default; not focused | Breadcrumb (Navigator-projected, read-only); right-side controls (settings access, sync indicator) | None on the omnibar |
| **Input** | `Ctrl+L` (canonical), click into omnibar, explicit `Message::OmnibarFocus` | Text input field, URL-shaped completion list inline below | Omnibar text input |

Mode transitions are explicit Messages dispatched from `update`; they
never occur from `view`.

### 2.1 Mode transition rules

- **Display → Input**: focus capture via `Ctrl+L` or pointer click;
  breadcrumb collapses; `text_input` receives focus via `widget::focus()`
  `Operation`.
- **Input → Display via Escape, blur, or click outside**: text input
  loses focus; breadcrumb restored; no mutation.
- **Input → Display via successful submit**: typed text consumed; an
  `OmnibarIntent::OpenAddress` (or, for non-URL-shaped text,
  `OmnibarIntent::RouteToNodeFinder`) is dispatched; omnibar returns to
  Display mode; focus restored to whichever widget held focus before
  `OmnibarFocus`.

---

## 3. Widget Tree

Inside the `CommandBar` slot's `Container`:

```rust
fn omnibar(state: &State) -> Element<'_, Message, GraphshellTheme> {
    let session = &state.omnibar;
    let breadcrumb_or_input: Element<_, _> = match session.mode {
        OmnibarMode::Display => navigator_breadcrumb(state.navigator_context).into(),
        OmnibarMode::Input   => omnibar_text_input(session).into(),
    };

    container(
        row![
            scope_badge(state.navigator_context.scope_badge.as_deref()),
            breadcrumb_or_input,
            graphlet_label_chip(state.navigator_context.graphlet_label.as_deref()),
            spacer(),
            // Right-side controls (no command palette / node finder triggers
            // here; those are global keybindings + Tree Spine entries).
            settings_access_button(),
            sync_status_indicator(state.sync_status),
        ]
        .spacing(8)
        .align_y(Vertical::Center)
    )
    .padding([4, 8])
    .style(omnibar_container_style)
    .into()
}
```

### 3.1 Sub-widget responsibilities

| Sub-widget | Authority | iced primitive | Notes |
|---|---|---|---|
| **Scope badge** | Navigator (content) / Shell (rendering) | `text` styled chip | Always shown; one short word/phrase identifying current graph scope |
| **Breadcrumb** | Navigator (content) / Shell (rendering) | `row!` of clickable `text` tokens with separator chars | Display mode only; clicking a token emits `Message::BreadcrumbNavigate` |
| **Graphlet label chip** | Navigator (content) / Shell (rendering) | `text` styled chip | Shown when a named/pinned graphlet is active |
| **Text input** | Shell | `text_input` (iced 0.14, IME-aware) | Input mode only; on_input + on_submit + on_paste handled |
| **URL completion list** | Shell (UI) / providers (data) | `column!` inside `scrollable` below the input | Input mode only; renders `OmnibarSession::completions` (URL-shaped only) |
| **Settings access** | Shell | `button` | Routes to `verso://settings` |
| **Sync status** | Shell | small icon `text` | Read-only indicator; click opens sync diagnostics |

The omnibar **does not** render command-palette or node-finder trigger
buttons — those are global keybindings (`Ctrl+Shift+P`, `Ctrl+P`) that
work regardless of focus. Adding palette/finder buttons here would
re-couple the surfaces, which is exactly what the simplification
removes.

---

## 4. State Shape: `OmnibarSession`

```rust
pub struct OmnibarSession {
    pub mode: OmnibarMode,
    pub draft: String,                       // current input text
    pub completions: Vec<UrlCompletionItem>,  // URL-shaped suggestions only
    pub completion_focus: Option<usize>,
    pub pending_request: Option<ProviderRequestId>,
    pub last_submission: Option<SubmissionRecord>,
    pub focus_token: Option<widget::Id>,     // saved iced focus id at open time
}

pub enum OmnibarMode {
    Display,
    Input,
}

pub struct UrlCompletionItem {
    pub address: String,                     // canonical address
    pub label: Option<String>,               // page title if known
    pub source: UrlCompletionSource,         // History | Bookmark
}
```

State that does **not** live here:

- **Authoritative graph state** — comes from `FrameViewModel`.
- **Navigator-projected breadcrumb / scope badge / graphlet label** —
  comes from `NavigatorContextProjection` (Navigator authority,
  read-only at this seam).
- **Focused-target carrier** — retired (see §1, §
  [shell_composition_model_spec.md §5.4](shell_composition_model_spec.md)).
- **Provider implementations** — under `ControlPanel` supervision; the
  omnibar talks to them only through `HostRequestMailbox` mailboxes.

---

## 5. Message Contract

```rust
pub enum Message {
    // Mode transitions
    OmnibarFocus,                            // Display → Input
    OmnibarBlur,                             // Input → Display, no submit

    // Input handling
    OmnibarInput(String),
    OmnibarSubmit,                           // Enter
    OmnibarPaste(String),
    OmnibarKeyArrowDown,                     // move completion focus
    OmnibarKeyArrowUp,
    OmnibarKeyEscape,                        // dismiss; restore focus
    OmnibarCompletionSelected(usize),

    // Navigator seam (read-only)
    BreadcrumbNavigate(BreadcrumbToken),

    // Provider seam
    OmnibarProviderResult {
        request_id: ProviderRequestId,
        result: UrlProviderResult,
    },

    // Submission outcome (Shell-decided)
    OmnibarRouteToNodeFinder(String),        // non-URL-shaped query → node finder
}
```

Notably absent (per simplification): no command-result variants, no
selection-target variants, no completion-source-selector variants.

---

## 6. Update Routing

```rust
fn update(&mut self, msg: Message) -> Task<Message> {
    match msg {
        Message::OmnibarFocus => {
            self.omnibar.mode = OmnibarMode::Input;
            self.omnibar.focus_token = self.current_focused_widget_id();
            return widget::focus(text_input::Id::new("omnibar"));
        }

        Message::OmnibarInput(draft) => {
            self.omnibar.draft = draft.clone();
            // URL-shaped suggestions only — providers are history-by-URL
            // and bookmark-URLs.
            self.omnibar.pending_request = Some(
                self.runtime.url_providers().request(draft)
            );
            Task::none()
        }

        Message::OmnibarSubmit => {
            let resolution = resolve_submit(&self.omnibar.draft, self.omnibar.completion_focus
                .and_then(|i| self.omnibar.completions.get(i)));
            match resolution {
                SubmitResolution::OpenAddress(intent) => {
                    self.runtime.emit(intent);
                    self.omnibar.last_submission = Some(SubmissionRecord::Address(...));
                    self.omnibar.draft.clear();
                    self.omnibar.mode = OmnibarMode::Display;
                    return restore_focus(self.omnibar.focus_token.take());
                }
                SubmitResolution::RouteToNodeFinder(query) => {
                    // Non-URL-shaped query: pass to Node Finder rather than
                    // silently retargeting inside the omnibar.
                    return Task::done(Message::OmnibarRouteToNodeFinder(query));
                }
            }
        }

        Message::OmnibarRouteToNodeFinder(query) => {
            self.omnibar.draft.clear();
            self.omnibar.mode = OmnibarMode::Display;
            self.node_finder.open_with_query(query);
            return widget::focus(text_input::Id::new("node_finder_input"));
        }

        Message::OmnibarKeyEscape => {
            if !self.omnibar.completions.is_empty() {
                self.omnibar.completions.clear();
                self.omnibar.completion_focus = None;
            } else {
                self.omnibar.mode = OmnibarMode::Display;
                return restore_focus(self.omnibar.focus_token.take());
            }
            Task::none()
        }

        // ... other arms ...
    }
}
```

### 6.1 Submit resolution

`resolve_submit` parses the draft text:

1. If a completion is keyboard-focused, that completion's `address` is
   used (always URL-shaped from URL providers).
2. Otherwise, the draft is parsed:
   - URL-shaped (matches `^[a-z][a-z0-9+.-]*://...` or `verso://...` or a
     bare host with `.` and TLD-shape) → `SubmitResolution::OpenAddress`.
   - Otherwise → `SubmitResolution::RouteToNodeFinder` with the draft as
     the initial query.

There is no "default web search" behavior baked into the omnibar
itself; default-web-search is a node finder fallback if the user
configures it as such.

### 6.2 Routing the resulting Pane

Once `OpenAddress` is dispatched, the receiving authority
(`graphshell-runtime` graph reducer) handles the open. The destination
Pane is determined by a user-configurable rule:

- **Active Pane** (default): activate the resulting node in the
  currently focused tile Pane, or the most-recent active Pane if focus
  isn't on a tile Pane.
- **New Pane**: create a new tile Pane in a Split adjacent to the
  current focus.
- **Replace focused Pane**: replace the focused Pane's `GraphletId` /
  active tile with the new node.

The rule is a `WorkbenchProfile` setting; the omnibar does not encode
it — it just emits the open intent and lets the runtime + workbench
resolve placement. This is what `CommandBarFocusTarget` was solving
in a more complex way; the simplification pushes the resolution to
the runtime (which already has full focus + workbench state).

---

## 7. Subscription: URL Provider Mailbox

```rust
fn subscription(&self) -> Subscription<Message> {
    Subscription::batch([
        // ... other subscriptions ...
        url_provider_mailbox_stream(&self.runtime)
            .map(|(request_id, result)| {
                Message::OmnibarProviderResult { request_id, result }
            }),
    ])
}
```

`url_provider_mailbox_stream` is a thin iced wrapper around the existing
`HostRequestMailbox<UrlProviderResult>`; the mailbox is supervised by
`ControlPanel` per the existing landed contract. URL providers feed only
URL-shaped suggestions; other ranking (title/tag/content) is the Node
Finder's responsibility.

Cancellation by request-id supersession: each `OmnibarInput` schedules
a new `ProviderRequestId`; `OmnibarProviderResult` arms drop results
whose `request_id` differs from `pending_request`.

---

## 8. IME and Accessibility

`text_input` (iced 0.14+) is IME-aware out of the box: composition
events, candidate windows, and dead keys are routed correctly. CJK /
Arabic input works end-to-end.

For accessibility:

- **AccessKit role**: `text_input` exposes itself as a `combobox` with an
  `editable` attribute (since URL completions are present); completion
  list items are `option` roles.
- **Keyboard navigation**: `Ctrl+L` enters Input mode; arrow keys
  navigate the completion list; Enter submits; Escape exits.
- **Focus restore**: on submit, blur, or Escape, focus returns to the
  widget id stored in `focus_token` at `OmnibarFocus` time. There is no
  Shell-level `CommandBarFocusTarget` — each surface stores its own.
- **WCAG 2.2 AA targets** per [DOC_POLICY.md](../../DOC_POLICY.md):
  SC 2.4.3 (Focus Order), SC 2.4.11 (Focus Appearance), SC 2.5.8
  (Target Size).

These targets land at Stage E (per the iced jump-ship plan §12.3) and
gate via the `UxProbeSet` AT-validation contract.

---

## 9. Focus Interaction with Other Command Surfaces

The omnibar, the [Command Palette](iced_command_palette_spec.md), and
the [Node Finder](iced_node_finder_spec.md) are three distinct
keyboard-focus targets. Per the simplification:

- Each surface stores its own `focus_token` at open time.
- Each surface restores focus on dismiss to its own stored token.
- Surfaces do not share a focus carrier; there is no
  `CommandBarFocusTarget`.
- A surface's "search" / "command" / "URL" intent does not silently
  retarget another surface; ambiguous input routes explicitly (e.g.,
  the omnibar's `RouteToNodeFinder`).

The omnibar can request opening the Node Finder via
`Message::OmnibarRouteToNodeFinder` (per §6.1); that dispatches a
distinct surface activation, with focus moving cleanly between
surfaces.

---

## 10. Coherence Guarantee Restated

Per [iced jump-ship plan §4.10](2026-04-28_iced_jump_ship_plan.md):

> **Omnibar**: Typing in the omnibar never mutates graph truth.
> Submission emits an explicit intent (open node, search, navigate).
> The Navigator-projected breadcrumb always reflects current graph
> truth, never an in-progress draft.

This spec preserves and tightens the guarantee:

- Draft text lives only in `OmnibarSession::draft`; never written to
  graph state.
- `OmnibarSubmit` emits at most one `HostIntent` (`OpenAddress`) and
  clears the draft. Non-URL-shaped input does not emit a graph intent
  from the omnibar; it explicitly routes to the Node Finder.
- The breadcrumb / scope badge / graphlet label come from
  `NavigatorContextProjection`, rebuilt from current truth each frame.
- "Command" submissions are out of scope — commands go through the
  Command Palette with its own selection-set + `ActionRegistry` flow.

A future change that re-adds command invocation to the omnibar
violates this guarantee and the canonical
[command_surface_interaction_spec.md §4.5](../aspect_command/command_surface_interaction_spec.md);
do not re-add.

---

## 11. Open Items

- **URL provider catalog**: which providers feed completions, in what
  priority order. Out of scope here; lives in the URL-provider service
  spec.
- **Default-web-search-as-fallback**: when a non-URL-shaped query
  routes to the Node Finder, an optional Node Finder footer entry
  "Search the web for X" can dispatch to a configured web search
  engine. Configuration lives in Settings; the omnibar itself doesn't
  expose this.
- **Drag-into-omnibar**: dropping a node, swatch, or external URL onto
  the omnibar emits `Message::OmnibarInput` with the dropped payload's
  address. Spec stub only.
- **Visual style / animation**: mode-transition animation curve,
  completion-list slide-in / slide-out, breadcrumb token hover style.
  Stage F polish, not skeleton concern.

---

## 12. Bottom Line

The iced omnibar is one `text_input` plus a row of read-only Navigator
projections, switching between Display and Input modes under Shell
control. Submission emits one `HostIntent::OpenAddress` for URL-shaped
input or routes non-URL input to the Node Finder. State is widget-local;
URL completions return through `HostRequestMailbox`; nothing in the
omnibar mutates graph truth or hosts commands. The egui-era multi-role
omnibar with `CommandBarFocusTarget` and prefix-syntax command entry is
retired; commands go through the [Command Palette](iced_command_palette_spec.md),
graph search through the [Node Finder](iced_node_finder_spec.md).

This closes the omnibar S2 sub-deliverable in its simplified form.
