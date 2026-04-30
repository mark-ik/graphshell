<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Iced Omnibar Spec

**Date**: 2026-04-29
**Status**: Canonical / Active — second concrete S2 deliverable for the iced jump-ship plan
**Scope**: The iced-side omnibar widget within the `CommandBar` slot. Modes,
widget tree, state shape, Message contract, update routing, IME, focus
interaction with the command palette, completion-mailbox seam, and the
consumption of `CommandBarFocusTarget` and `NavigatorContextProjection`.
This spec adds depth to [`iced_composition_skeleton_spec.md` §7.1](iced_composition_skeleton_spec.md);
the canonical seam definitions live in
[`shell_composition_model_spec.md` §5](shell_composition_model_spec.md).

**Code-sample mode**: This spec uses **illustrative signatures** (per the
session feedback memory). Types like `OmnibarSession`, `CompletionItem`,
`ProviderRequestId` are sketched; full implementation-ready Rust lives in
S3/S4 code, not this spec. Names track the existing landed code
(`HostRequestMailbox`, `CommandBarFocusTarget`) so the reader can find
the implementation anchors.

**Related**:

- [`iced_composition_skeleton_spec.md`](iced_composition_skeleton_spec.md) — Application skeleton (§1.5), CommandBar slot (§7.1), authority routing (§8)
- [`shell_composition_model_spec.md`](shell_composition_model_spec.md) §5 — canonical CommandBar / omnibar seam: `NavigatorContextProjection`, `BreadcrumbPath`, `CommandBarFocusTarget`, host-thread contract
- [`2026-04-03_shell_command_bar_execution_plan.md`](2026-04-03_shell_command_bar_execution_plan.md) — egui-era execution lane (Workstream A); landed seams (HostRequestMailbox, FocusTarget) are reused; egui rendering paths are not
- [`2026-04-28_iced_jump_ship_plan.md`](2026-04-28_iced_jump_ship_plan.md) §4.7 / §4.9 / §4.10 — Navigator buckets, uphill rule, CommandBar coherence guarantee
- [SHELL.md §6](SHELL.md) — Shell ↔ Navigator chrome relationship
- [`../navigator/NAVIGATOR.md` §6](../navigator/NAVIGATOR.md) — Navigator cross-surface verb mapping
- [`../subsystem_ux_semantics/2026-04-05_command_surface_observability_and_at_plan.md`](../subsystem_ux_semantics/2026-04-05_command_surface_observability_and_at_plan.md) — command-surface provenance / AT validation (consumed here)

---

## 1. Intent

The omnibar is a single widget but a multi-authority surface:

- **Shell** owns the input field, mode, dispatch, and focus.
- **Navigator** contributes a read-only breadcrumb / scope-badge /
  graphlet-label projection.
- **Background providers** (search, history, bookmarks, address
  completion) feed asynchronous suggestions through a Shell-supervised
  mailbox.
- **Command authority** receives submitted intents; results surface in
  the Activity Log per the [coherence guarantee for the omnibar](2026-04-28_iced_jump_ship_plan.md).

The egui-era seams for these splits are landed in code
(`CommandBarFocusTarget`, `NavigatorContextProjection`,
`HostRequestMailbox`); the iced-side rendering and update routing are
the gap this spec closes.

---

## 2. Modes

The omnibar has three rendering modes. Mode is Shell-owned per-frame
state; transitions are explicit Messages.

| Mode | Trigger | Visible | Focus |
|---|---|---|---|
| **Display** | Default; not focused | Breadcrumb (left), scope badge, graphlet label (read-only); right-side controls (settings access, sync indicator) | None on the omnibar |
| **Input** | Click into omnibar; `Ctrl+L` or `/` shortcut; explicit Message::OmnibarFocus | Text input field, scope badge (compact), inline completion list below | Omnibar text input |
| **Fullscreen** | Fullscreen content surface active (e.g., presentation mode) | Condensed strip with current node address only | None — input deferred to fullscreen-aware dispatch |

Display ↔ Input transitions are the common case. Fullscreen is a sibling
state used when a Pane requests chrome suppression; it is out of scope
for this spec beyond the table entry.

### 2.1 Mode transition rules

- **Display → Input**: Shell switches mode immediately on focus capture;
  the breadcrumb collapses to a compact scope badge; the text input
  field receives focus via `widget::focus()` `Operation`.
- **Input → Display**: triggered by Escape, click outside, successful
  submission, or focus moving to another surface; Shell restores the
  breadcrumb projection.
- **Input → Display via submit**: the draft text is consumed, the
  resulting `HostIntent` emits, and the omnibar re-enters Display mode
  even before the receiving authority confirms (per the coherence
  guarantee: unconfirmed actions never silently apply, but the omnibar
  does not block waiting for confirmation).

Mode state lives in `OmnibarSession` (§4); transitions never occur from
`view`, only from `update`.

---

## 3. Widget Tree

The omnibar lives inside the `CommandBar` slot's `Container`. Its
internal layout is a single `row!` with conditional children depending
on mode:

```rust
fn omnibar(state: &State) -> Element<'_, Message, GraphshellTheme> {
    // Illustrative signatures — see S3 for typed impl.
    let session = &state.omnibar;

    let breadcrumb_or_input: Element<_, _> = match session.mode {
        OmnibarMode::Display => navigator_breadcrumb(state.navigator_context).into(),
        OmnibarMode::Input   => omnibar_text_input(session).into(),
        OmnibarMode::Fullscreen => fullscreen_strip(state.fullscreen_address.as_ref()).into(),
    };

    container(
        row![
            scope_badge(state.navigator_context.scope_badge.as_deref()),
            breadcrumb_or_input,
            graphlet_label_chip(state.navigator_context.graphlet_label.as_deref()),
            spacer(),                          // pushes right-side controls to the right
            command_palette_trigger_button(),  // dispatches PaletteOpen
            settings_access_button(),          // dispatches OpenSettings
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
| **Completion list** | Shell (UI) / providers (data) | `column!` inside `scrollable` below the input | Input mode only; renders `OmnibarSession::completions` |
| **Command palette trigger** | Shell | `button` | Dispatches `Message::PaletteOpen` |
| **Settings access** | Shell | `button` | Routes to `verso://settings` |
| **Sync status** | Shell | small icon `text` | Read-only indicator; click opens sync diagnostics |
| **Fullscreen strip** | Shell | `text` showing current address | Fullscreen mode only; minimal chrome |

Per the iced jump-ship plan §5, no sub-widget owns domain authority;
each emits Messages that route uphill.

---

## 4. State Shape: `OmnibarSession`

The omnibar's per-frame state is held in `OmnibarSession`, a field of
`GraphshellApp::omnibar` (per [`iced_composition_skeleton_spec.md` §1.5](iced_composition_skeleton_spec.md)):

```rust
// Illustrative signatures — see S3 for typed impl.
pub struct OmnibarSession {
    pub mode: OmnibarMode,
    pub draft: String,                       // current input text
    pub completions: Vec<CompletionItem>,    // current visible completions
    pub completion_focus: Option<usize>,     // keyboard-selected completion, if any
    pub pending_request: Option<ProviderRequestId>,  // outstanding async fetch
    pub last_submission: Option<SubmissionRecord>,   // for retry / Activity Log linking
    pub focus_token: FocusToken,             // returned by Shell focus authority
}

pub enum OmnibarMode {
    Display,
    Input,
    Fullscreen,
}

pub struct CompletionItem {
    pub label: String,
    pub detail: Option<String>,        // secondary text (URL, type, score)
    pub source: ProviderId,            // which provider emitted this
    pub apply_intent: HostIntent,      // intent to dispatch on selection
}
```

State that does **not** live here:

- **Authoritative graph state** — comes from `FrameViewModel`.
- **Navigator-projected breadcrumb / scope badge / graphlet label** —
  comes from `NavigatorContextProjection` (Navigator authority,
  read-only at this seam).
- **Focused-target carrier** — comes from `CommandBarFocusTarget` (per
  [shell_composition_model_spec.md §5.4](shell_composition_model_spec.md));
  the omnibar reads it for routing decisions but does not own it.
- **Provider implementations** — live under `ControlPanel` supervision;
  the omnibar talks to them only through `HostRequestMailbox` mailboxes.

---

## 5. Message Contract

Messages the omnibar emits or consumes (in addition to the global
Message enum from [`iced_composition_skeleton_spec.md` §1.5](iced_composition_skeleton_spec.md)):

```rust
pub enum Message {
    // ... other variants from the global enum ...

    // Mode transitions (Shell-owned)
    OmnibarFocus,                                // Display → Input
    OmnibarBlur,                                 // Input → Display, no submit
    OmnibarFullscreenEnter,
    OmnibarFullscreenExit,

    // Input handling (Shell-owned)
    OmnibarInput(String),                        // text_input on_input
    OmnibarSubmit,                               // text_input on_submit (Enter key)
    OmnibarPaste(String),                        // text_input on_paste
    OmnibarKeyArrowDown,                         // move completion focus
    OmnibarKeyArrowUp,                           // move completion focus
    OmnibarKeyEscape,                            // dismiss completions, then mode
    OmnibarCompletionSelected(usize),            // click or Enter on a completion

    // Navigator seam (read-only consumption)
    BreadcrumbNavigate(BreadcrumbToken),         // click on a breadcrumb token

    // Background provider seam
    OmnibarProviderResult {
        request_id: ProviderRequestId,
        result: ProviderResult,
    },

    // Cross-surface (right-side controls)
    PaletteOpen,                                 // command palette trigger
    OpenSettings,                                // settings access button
    SyncStatusClicked,                           // open sync diagnostics
}
```

Each Message variant corresponds to one user gesture or one async event.
`update` matches on each and either:

1. mutates `OmnibarSession` (widget-local state), and/or
2. dispatches a `HostIntent` upward via `runtime.emit(...)`, and/or
3. spawns a `Task` (e.g., a debounced provider fetch).

No Message variant does all three at once unless the gesture genuinely
implies all three (e.g., `OmnibarSubmit` mutates draft, dispatches
intent, and may spawn a confirmation listener).

---

## 6. Update Routing

The `update` arms for omnibar Messages follow this shape:

```rust
fn update(&mut self, msg: Message) -> Task<Message> {
    match msg {
        Message::OmnibarFocus => {
            self.omnibar.mode = OmnibarMode::Input;
            // Move iced widget focus to the text_input via Operation
            return widget::focus(text_input::Id::new("omnibar"));
        }

        Message::OmnibarInput(draft) => {
            self.omnibar.draft = draft.clone();
            // Debounced provider fetch: cancel prior in-flight, schedule new
            self.omnibar.pending_request = Some(self.runtime.providers().request(draft));
            Task::none()
        }

        Message::OmnibarSubmit => {
            let intent = resolve_submit_intent(
                &self.omnibar.draft,
                self.omnibar.completion_focus
                    .and_then(|i| self.omnibar.completions.get(i)),
                &self.view_model.focus_target,   // CommandBarFocusTarget
            );
            self.runtime.emit(intent);
            self.omnibar.last_submission = Some(SubmissionRecord::from(&intent));
            self.omnibar.draft.clear();
            self.omnibar.mode = OmnibarMode::Display;
            // Restore focus to the prior surface per CommandBarFocusTarget
            return restore_focus(&self.view_model.focus_target);
        }

        Message::OmnibarKeyEscape => {
            if !self.omnibar.completions.is_empty() {
                self.omnibar.completions.clear();
                self.omnibar.completion_focus = None;
            } else {
                self.omnibar.mode = OmnibarMode::Display;
                return restore_focus(&self.view_model.focus_target);
            }
            Task::none()
        }

        Message::OmnibarProviderResult { request_id, result } => {
            // Drop stale results
            if Some(request_id) != self.omnibar.pending_request {
                return Task::none();
            }
            self.omnibar.completions = result.into_completions();
            self.omnibar.completion_focus = None;
            self.omnibar.pending_request = None;
            Task::none()
        }

        // ... other variants ...
    }
}
```

### 6.1 `OmnibarSubmit` resolution rules

The submitted intent is computed from three inputs:

1. **Selected completion** if one is keyboard-focused or just clicked —
   that completion's `apply_intent` is used.
2. **Otherwise**, the draft text is parsed by Shell-side dispatch
   (URL → open node, search query → search intent, command prefix
   → palette action, etc.).
3. The resolved intent is **scoped** by `CommandBarFocusTarget`:
   `FocusedSurface::GraphPrimary` routes mutations to that view's
   `GraphViewId`; `FocusedSurface::WorkbenchTile` routes to that pane;
   `FocusedSurface::NavigatorHost` routes to that host's scope.

Per [shell_composition_model_spec.md §5.4](shell_composition_model_spec.md),
the precedence rule is: keyboard focus owner wins; otherwise the last
pointer-interacted surface wins; otherwise no focused command target is
exposed.

---

## 7. Subscription: Provider Mailbox

Per [shell_composition_model_spec.md §5.3](shell_composition_model_spec.md),
provider results return through a Shell-supervised mailbox
(`HostRequestMailbox<ProviderResult>`). The iced bridge:

```rust
fn subscription(&self) -> Subscription<Message> {
    Subscription::batch([
        // ... other subscriptions from §1.5 ...

        // Drain the omnibar provider mailbox into Messages.
        provider_mailbox_stream(&self.runtime)
            .map(|(request_id, result)| {
                Message::OmnibarProviderResult { request_id, result }
            }),
    ])
}
```

`provider_mailbox_stream` is a thin iced wrapper around the existing
`HostRequestMailbox`; the mailbox itself is supervised by `ControlPanel`
per the existing landed contract. The omnibar's only contact with
providers is through this single Subscription; per the iced jump-ship
plan §9 anti-pattern, the omnibar does not poll the mailbox inside
`view`.

### 7.1 Cancellation and staleness

A provider request is cancelled implicitly when a newer request
supersedes it: each `OmnibarInput` schedules a new
`ProviderRequestId`, and `OmnibarProviderResult` arms drop results whose
`request_id` differs from `pending_request`. The mailbox is also
allowed to cancel in-flight work directly when feasible — this is a
provider-implementation detail, not an iced-side concern.

---

## 8. IME and Accessibility

`text_input` (iced 0.14+) is IME-aware out of the box: composition
events, candidate windows, and dead keys are routed correctly without
additional glue. CJK / Arabic input works end-to-end.

For accessibility:

- **AccessKit role**: `text_input` exposes itself as a `searchbox`
  role via `iced_accessibility`. Completion list items are
  `option` roles within a `listbox` parented to the input.
- **Keyboard navigation**: `Tab` enters the omnibar input; `Esc`
  exits; arrow keys move within the completion list when present
  (`OmnibarKeyArrowDown` / `Up`); `Enter` submits or selects the
  focused completion; `Ctrl+L` / `/` are global shortcuts that emit
  `Message::OmnibarFocus`.
- **Screen reader**: completion list updates announce as ARIA-live
  via `iced_accessibility`'s live-region support.
- **Focus restore**: on submit, blur, or Escape, focus returns to the
  surface named by `CommandBarFocusTarget::focused_surface` per
  [shell_composition_model_spec.md §5.4](shell_composition_model_spec.md).

WCAG 2.2 AA targets per [DOC_POLICY.md adopted standards](../../DOC_POLICY.md):

- SC 2.4.3 (Focus Order): focus moves predictably — omnibar in, Esc out
- SC 2.4.11 (Focus Appearance): visible focus ring on text_input via
  iced::Theme
- SC 2.5.8 (Target Size): text_input and completion rows ≥ 32×32 dp

These targets land at Stage E (per the iced jump-ship plan §12.3) and
gate via the `UxProbeSet` AT-validation contract.

---

## 9. Focus Interaction with the Command Palette

The omnibar and the command palette share `CommandBarFocusTarget` but
are otherwise distinct surfaces. The interaction:

- **`Ctrl+P`** (or palette trigger button click) emits
  `Message::PaletteOpen` and opens the palette `Modal` over the
  current view. The omnibar mode does **not** change; it stays in
  whatever mode it was. Focus moves to the palette.
- **Palette dismiss** (Escape, click outside, action selected) emits
  `Message::PaletteClose` and restores focus to the surface stored in
  `CommandBarFocusTarget` *at palette open time*. If the omnibar held
  focus when the palette opened, focus returns to the omnibar in the
  same mode it had.
- **Palette action that targets the omnibar** (e.g., "Focus omnibar"
  command) dispatches `Message::OmnibarFocus`, putting the omnibar
  into Input mode after the palette closes.
- **Mutual exclusion is not enforced**: the palette is a Modal overlay
  that visually covers the omnibar; the omnibar doesn't need a
  "palette-active" state — its `view` runs as normal beneath the modal,
  but pointer/keyboard input goes to the palette.

Per the coherence guarantees (iced jump-ship plan §4.10), neither
surface mutates graph truth on its own; both emit explicit intents.

---

## 10. CommandBarFocusTarget Consumption

`CommandBarFocusTarget` (per
[shell_composition_model_spec.md §5.4](shell_composition_model_spec.md))
is the omnibar's contract for "what does Submit target?". It carries
`focused_surface` and `focused_node`. The omnibar consumes it at submit
time and at focus-restore time. It does not write to it.

The carrier is computed once per frame by Shell using the
keyboard-focus-then-pointer-focus precedence rule. The omnibar reads
the current snapshot from `view_model.focus_target` and passes it
unmodified into `resolve_submit_intent` and `restore_focus`.

If `focused_surface` is `None` (no focused target), submission still
works — the resolved intent targets the primary graph view as the
default. The omnibar does not silently narrow the target to a
sub-surface; that would violate
[NAVIGATOR.md §I8 command applicability invariant](../navigator/NAVIGATOR.md).

---

## 11. Coherence Guarantee Restated

Per [iced jump-ship plan §4.10](2026-04-28_iced_jump_ship_plan.md):

> **Omnibar**: Typing in the omnibar never mutates graph truth.
> Submission emits an explicit intent (open node, search, navigate).
> The Navigator-projected breadcrumb always reflects current graph
> truth, never an in-progress draft.

This spec preserves the guarantee:

- Draft text lives only in `OmnibarSession::draft`; never written to
  graph state.
- `OmnibarSubmit` emits one `HostIntent` and clears the draft; the
  receiving authority's confirmation surfaces in the Activity Log per
  [iced jump-ship plan §4.7](2026-04-28_iced_jump_ship_plan.md).
- The breadcrumb / scope badge / graphlet label come from
  `NavigatorContextProjection`, which is rebuilt from current graph and
  workbench state each frame; it never reflects the omnibar's
  in-progress draft.

A future change to the omnibar that violates this guarantee — e.g.,
storing recent submissions in a sidecar that aliases graph truth — is a
bug, not a UX preference.

---

## 12. Open Items

Items this spec leaves to subsequent work:

- **Provider catalog**: which providers are wired, in what priority
  order (URL completion, history, bookmarks, search, command palette
  fall-through, semantic suggestions). Provider implementations are
  out of scope for this spec; the seam is `ProviderId` /
  `HostRequestMailbox`.
- **Inline command syntax**: prefix-based command invocation
  (`>action-name args`, `:lens-name`, etc.) is mentioned in
  `command_surface_interaction_spec.md` but not yet specified for the
  iced omnibar.
- **Drag-into-omnibar**: dropping a node, swatch, or external URL
  onto the omnibar emits `Message::OmnibarInput` with the dropped
  payload's address. Spec stub only at this point.
- **Multiple-omnibar policy**: per
  [iced jump-ship plan §11 G4](2026-04-28_iced_jump_ship_plan.md),
  whether the omnibar is per-Pane or global is open. This spec
  assumes one global omnibar in the CommandBar slot; if per-Pane
  omnibars land later, this spec will need a §13 update covering the
  per-Pane focus rule.
- **Visual style / animation**: mode-transition animation curve,
  completion-list slide-in / slide-out, breadcrumb token hover style.
  Stage F polish, not skeleton concern.
- **Confirmation/error toasts on submit**: when the receiving
  authority rejects an intent, the toast surface (per
  [iced jump-ship plan §12.2](2026-04-28_iced_jump_ship_plan.md))
  shows the rejection. Toast routing is shared with other surfaces;
  spec lives in the toast subsystem, not here.

---

## 13. Bottom Line

The iced omnibar is one `text_input` plus a row of read-only Navigator
projections, switching between Display, Input, and Fullscreen modes
under Shell control. State lives in `OmnibarSession`; mutations route
through Messages; provider results return through the existing
`HostRequestMailbox` Subscription. Submission emits a single
`HostIntent` scoped by `CommandBarFocusTarget`; nothing in the omnibar
ever bypasses the uphill rule or aliases graph truth. AccessKit and
IME work via `text_input`'s built-ins (Stage E). The command palette
is a sibling Modal that shares `CommandBarFocusTarget` and restores
focus on dismiss.

This closes one of the remaining S2 sub-deliverables; together with
the composition skeleton (§7.1) and the coherence guarantees (iced
jump-ship plan §4.10) it gives S4 a concrete target to implement
against without re-deriving the seam contracts.
