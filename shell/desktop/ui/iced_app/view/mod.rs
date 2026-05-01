//! View-layer free functions extracted from `iced_app/mod.rs` —
//! Phase A of the post-Slice-39 decomposition. All `render_*`
//! helpers, hotkey detection, host-route mapping, and event
//! formatting live here; mod.rs keeps the orchestration (state
//! types, IcedApp, Message, update, view, run_application).

use super::*;

mod modals;
pub(crate) use modals::*;
mod panes;
mod navigator;
pub(crate) use navigator::*;
pub(crate) use panes::*;


// ---------------------------------------------------------------------------
// Navigator host rendering — Slice 4 (structural layout)
// ---------------------------------------------------------------------------

/// Which edge of the workbench a Navigator host is anchored to.
///
/// Left / Right → sidebar form factor (vertical column, fixed width).
/// Top / Bottom → toolbar form factor (horizontal row, fixed height).
/// Per [`iced_composition_skeleton_spec.md` §2](
/// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_composition_skeleton_spec.md).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum NavigatorAnchor {
    Left,
    Right,
    Top,
    Bottom,
}

/// Render one Navigator host slot with stub Presentation Buckets.
///
/// Per spec §6: each host renders the three canonical buckets — Tree
/// Spine, Swatches, Activity Log — in a layout appropriate for its
/// form factor. This slice renders structural stubs; real bucket content
/// (lazy+scrollable trees, canvas swatch grid, event stream) lands once
/// the Navigator domain layer is wired (S5).
pub(crate) fn render_navigator_host(app: &IcedApp, anchor: NavigatorAnchor) -> Element<'_, Message> {
    // Tree Spine bucket — Slice 20 reads from the runtime's GraphTree
    // and renders one row per member. Each row is a button that
    // dispatches `Message::TreeSpineNodeClicked(node_key)` → the
    // runtime promotes the node to focused via HostIntent::OpenNode.
    let tree_spine: Element<'_, Message> = render_tree_spine_bucket(app);

    // Swatches bucket — Slice 33 renders one compact canvas card per
    // built-in projection recipe. Per
    // [`iced_composition_skeleton_spec.md` §6.2](
    // ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_composition_skeleton_spec.md):
    // virtualized grid of `canvas::Program` instances at the
    // `RenderProfile::Swatch` profile.
    let swatches: Element<'_, Message> = render_swatches_bucket(app);

    // Activity Log bucket — Slice 27 reads from the host's bounded
    // RecordingObserver and renders one row per UxEvent in
    // most-recent-first order. Per
    // [`iced_composition_skeleton_spec.md` §6.3](
    // ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_composition_skeleton_spec.md):
    // event-stream view of recent runtime activity.
    let activity_log: Element<'_, Message> = render_activity_log_bucket(app);

    match anchor {
        NavigatorAnchor::Left | NavigatorAnchor::Right => {
            // Sidebar form factor: vertical column, fixed width.
            container(
                iced::widget::column![tree_spine, swatches, activity_log]
                    .spacing(4)
                    .height(Length::Fill),
            )
            .width(Length::Fixed(180.0))
            .height(Length::Fill)
            .padding(6)
            .into()
        }
        NavigatorAnchor::Top | NavigatorAnchor::Bottom => {
            // Toolbar form factor: horizontal row, fixed height.
            container(
                iced::widget::row![tree_spine, swatches, activity_log]
                    .spacing(4)
                    .width(Length::Fill),
            )
            .width(Length::Fill)
            .height(Length::Fixed(120.0))
            .padding(6)
            .into()
        }
    }
}

/// Render the CommandBar slot omnibar. Per
/// [`iced_omnibar_spec.md` §3](
/// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_omnibar_spec.md).
///
/// Slice 2: structural layout with placeholder sub-widgets. Real
/// Navigator projections (scope badge content, graphlet chip, settings
/// button routing, sync status) land in S4 when those surfaces exist.
pub(crate) fn render_command_bar(app: &IcedApp) -> Element<'_, Message> {
    let scope_badge = text("–").size(12);

    let center: Element<'_, Message> = match app.omnibar.mode {
        OmnibarMode::Display => {
            let location = app
                .last_view_model
                .as_ref()
                .map(|vm| vm.toolbar.location.as_str())
                .unwrap_or("—");
            text(location).size(14).width(Length::Fill).into()
        }
        OmnibarMode::Input => text_input("Enter URL or search…", &app.omnibar.draft)
            .id(iced::widget::Id::new(OMNIBAR_INPUT_ID))
            .on_input(Message::OmnibarInput)
            .on_submit(Message::OmnibarSubmit)
            .size(14)
            .padding(4)
            .width(Length::Fill)
            .into(),
    };

    let settings_stub = text("⚙").size(14);
    let sync_stub = text("◉").size(12);

    iced::widget::row![scope_badge, center, settings_stub, sync_stub,]
        .spacing(8)
        .align_y(iced::Alignment::Center)
        .into()
}

/// Render the Command Palette modal. Per
/// [`iced_command_palette_spec.md` §2.2](
/// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_command_palette_spec.md).
///
/// Slice 7 renders real result rows from the (placeholder) action list,
/// with focused-state highlighting and click handlers per row. Disabled
/// rows render dimmed and accept no clicks (`on_press_maybe(None)`).
/// Arrow-key navigation routes through `PaletteFocusUp/Down`; Enter
/// fires the focused row via `PaletteSubmitFocused`.

/// Is this iced event the "focus the omnibar" hotkey?
/// Ctrl+L (Cmd+L on macOS via `Modifiers::command()`). Consumed at
/// the app level — never reaches the runtime's `HostEvent` translation.
pub(crate) fn is_omnibar_focus_hotkey(event: &iced::Event) -> bool {
    match event {
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: iced::keyboard::Key::Character(c),
            modifiers,
            ..
        }) => c.as_ref().eq_ignore_ascii_case("l") && modifiers.command(),
        _ => false,
    }
}

/// Is this iced event the "open Command Palette" hotkey?
/// Ctrl+Shift+P (Zed/VSCode-shaped). Per
/// [`iced_command_palette_spec.md` §2.1](
/// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_command_palette_spec.md).
pub(crate) fn is_command_palette_hotkey(event: &iced::Event) -> bool {
    match event {
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: iced::keyboard::Key::Character(c),
            modifiers,
            ..
        }) => {
            c.as_ref().eq_ignore_ascii_case("p")
                && modifiers.command()
                && modifiers.shift()
        }
        _ => false,
    }
}

/// Is this iced event the "open Node Finder" hotkey?
/// Ctrl+P **without** Shift (Zed/VSCode-shaped). Per
/// [`iced_node_finder_spec.md` §2](
/// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_node_finder_spec.md).
pub(crate) fn is_node_finder_hotkey(event: &iced::Event) -> bool {
    match event {
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: iced::keyboard::Key::Character(c),
            modifiers,
            ..
        }) => {
            c.as_ref().eq_ignore_ascii_case("p")
                && modifiers.command()
                && !modifiers.shift()
        }
        _ => false,
    }
}

/// Is this iced event an Escape keypress?
pub(crate) fn is_escape_key(event: &iced::Event) -> bool {
    matches!(
        event,
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
            ..
        })
    )
}

/// Is this iced event an ArrowDown keypress?
pub(crate) fn is_arrow_down_key(event: &iced::Event) -> bool {
    matches!(
        event,
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowDown),
            ..
        })
    )
}

/// Is this iced event an ArrowUp keypress?
pub(crate) fn is_arrow_up_key(event: &iced::Event) -> bool {
    matches!(
        event,
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowUp),
            ..
        })
    )
}

/// Does `s` look like a URL or bare hostname?
///
/// Per [`iced_omnibar_spec.md` §6.1](
/// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_omnibar_spec.md):
/// explicit scheme (`://`) → URL; no spaces + contains `.` → bare
/// host. Everything else → non-URL-shaped → route to Node Finder.
pub(crate) fn is_url_shaped(s: &str) -> bool {
    let s = s.trim();
    if s.is_empty() {
        return false;
    }
    if s.contains("://") {
        return true;
    }
    !s.contains(' ') && s.contains('.')
}

/// Slice 28 host-side intercept for `ActionId`s whose effect is
/// opening or toggling an iced-owned overlay or rearranging
/// host-side composition state. Returns `Some(Message)` when the
/// host should handle the action directly; `None` lets the caller
/// fall through to `HostIntent::Action` runtime dispatch.
///
/// Slice 31 extends this with Frame switcher routing — Frame
/// composition lives in `IcedApp` (`frame`, `inactive_frames`),
/// not the runtime, so Frame* actions intercept here.
pub(crate) fn host_routed_action(
    action_id: graphshell_core::actions::ActionId,
) -> Option<Message> {
    use graphshell_core::actions::ActionId;
    match action_id {
        ActionId::GraphCommandPalette => Some(Message::PaletteOpen {
            origin: PaletteOrigin::Programmatic,
        }),
        // GraphRadialMenu was retired (see iced_command_palette_spec.md
        // §7.4). Re-introducing it is part of the input-subsystem
        // rework, not a host-route today.

        // Slice 31: Frame composition lives host-side.
        ActionId::FrameOpen => Some(Message::NewFrame),
        ActionId::FrameDelete => Some(Message::CloseCurrentFrame),
        // FrameSelect cycles to the next frame. The caller can pre-
        // compute the target index, but the simplest dispatch is a
        // sentinel: SwitchFrame(0) (the most-recently-backgrounded
        // frame). A future picker modal can route via SwitchFrame(idx)
        // for explicit selection.
        ActionId::FrameSelect => Some(Message::SwitchFrame(0)),
        // Slice 34: rename modal owns the active Frame's label.
        ActionId::FrameRename => Some(Message::FrameRenameOpen),

        // Slice 32: NodeCreate modal lives host-side; both NodeNew
        // and NodeNewAsTab open the same URL-input modal. The
        // pane-vs-tab distinction is downstream (the tab semantics
        // would route through workbench-routing once the pane
        // policy lands).
        ActionId::NodeNew | ActionId::NodeNewAsTab => Some(Message::NodeCreateOpen),
        _ => None,
    }
}

/// Emit a UX event onto the runtime's observer registry. Centralized
/// so every emission site has identical borrow shape — `&self.host.runtime`
/// is enough; emit() takes `&self`. Per
/// [`ux_observability`](
/// ../../../crates/graphshell-core/src/ux_observability.rs).
pub(crate) fn emit_ux_event(app: &IcedApp, event: graphshell_core::ux_observability::UxEvent) {
    app.host.runtime.ux_observers.emit(event);
}

/// Render the drop-zone hint banner — Slice 36 / 38. Visible only
/// while a pane drag is in progress
/// (`FrameState::drag_in_progress == true`, between Picked and
/// Dropped/Canceled). Pane_grid handles the drop logic; this banner
/// is a visible cue describing the drop targets.
///
/// Slice 38 modulates the banner's background alpha by a sine pulse
/// computed from the host's `startup_instant`, so the banner
/// breathes (period 1200ms) while the drag is active. The base
/// color is the theme's primary-weak; alpha ramps `[0.45, 0.95]`.
pub(crate) fn render_drop_zone_hint(pulse: f32) -> Element<'static, Message> {
    let hint = text(
        "Dragging — drop on a pane edge to split, on the center to swap panes",
    )
    .size(11);
    // Map pulse [0,1] → alpha [0.45, 0.95] so the banner stays
    // visible at trough but is more opaque at crest.
    let alpha = 0.45 + 0.50 * pulse;
    container(hint)
        .padding([3, 8])
        .width(Length::Fill)
        .height(Length::Fixed(22.0))
        .center_y(Length::Fill)
        .style(move |theme: &iced::Theme| {
            let pal = theme.palette();
            let bg = iced::Color {
                a: alpha,
                ..pal.primary.weak.color
            };
            container::Style {
                background: Some(bg.into()),
                text_color: Some(pal.primary.weak.text),
                ..Default::default()
            }
        })
        .into()
}

/// Render the Frame switcher bar — Slice 31. Visible only when
/// there's more than one Frame open. Each Frame is a small button
/// labeled by `frame_label`; the active Frame is highlighted; a
/// trailing "+" button creates a new Frame.
pub(crate) fn render_frame_switcher(app: &IcedApp) -> Element<'_, Message> {
    let mut row = iced::widget::row![
        text(format!("{} (active)", app.frame_label)).size(11),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);

    for (idx, frame) in app.inactive_frames.iter().enumerate() {
        let label = frame.label.clone();
        row = row.push(
            button(text(label).size(11))
                .on_press(Message::SwitchFrame(idx))
                .padding([2, 8])
                .style(|theme: &iced::Theme, status| {
                    let pal = theme.palette();
                    let hovered = matches!(
                        status,
                        iced::widget::button::Status::Hovered
                            | iced::widget::button::Status::Pressed
                    );
                    iced::widget::button::Style {
                        background: if hovered {
                            Some(tokens::HOVER_OVERLAY_STRONG.into())
                        } else {
                            None
                        },
                        text_color: pal.background.base.text,
                        border: iced::Border {
                            radius: 3.0.into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    }
                }),
        );
    }

    row = row.push(iced::widget::Space::new().width(Length::Fill));
    row = row.push(
        button(text("+").size(11))
            .on_press(Message::NewFrame)
            .padding([2, 8]),
    );
    if !app.inactive_frames.is_empty() {
        row = row.push(
            button(text("×").size(11))
                .on_press(Message::CloseCurrentFrame)
                .padding([2, 8]),
        );
    }

    container(row)
        .padding([3, 8])
        .width(Length::Fill)
        .height(Length::Fixed(22.0))
        .style(|theme: &iced::Theme| {
            let pal = theme.palette();
            container::Style {
                background: Some(
                    tokens::chrome_band(
                        pal.background.base.text,
                        tokens::CHROME_BAND_MEDIUM,
                    )
                    .into(),
                ),
                ..Default::default()
            }
        })
        .into()
}

/// Render the Tree Spine bucket — Navigator's left-rail "structural
/// list" of nodes in the workbench's GraphTree. Per
/// [`iced_composition_skeleton_spec.md` §6.1](
/// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_composition_skeleton_spec.md).
///
/// Slice 20 wiring: read from `runtime.graph_tree.members()` and emit
/// one button per member with the resolved title (from the domain
/// graph). Click → `Message::TreeSpineNodeClicked(node_key)` → push
/// `HostIntent::OpenNode { node_key }`. Lifecycle / Active+Inactive
/// toggles, indentation by topology depth, and `lazy` virtualization
/// are subsequent slices once their domain hooks are wired.

/// Render the StatusBar slot. Per
/// [`iced_composition_skeleton_spec.md` §2](
/// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_composition_skeleton_spec.md):
/// ambient system status, process indicators, background task count.
///
/// Slice 19 wires four indicators sourced from runtime state:
/// - **status dot** — green "ready" pulse (will animate on activity
///   in a later slice with `cosmic-time`)
/// - **actions** — `runtime.dispatched_action_count` (cumulative
///   `HostIntent::Action` dispatches since runtime start)
/// - **pending** — `host.pending_host_intents.len()` (queued intents
///   awaiting the next tick drain)
/// - **focused** — `runtime.focused_node_hint` (rendered as the
///   underlying NodeKey index, or "—" when no node is focused)
pub(crate) fn render_status_bar(app: &IcedApp) -> Element<'_, Message> {
    let dispatched = app.host.runtime.dispatched_action_count;
    let opened = app.host.runtime.opened_node_count;
    let pending = app.host.pending_host_intents.len();
    let focused_label = app
        .host
        .runtime
        .focused_node_hint
        .map(|k| format!("n{}", k.index()))
        .unwrap_or_else(|| "—".to_string());

    let dot = text("●").size(11).style(|theme: &iced::Theme| {
        let pal = theme.palette();
        iced::widget::text::Style {
            color: Some(pal.success.base.color),
        }
    });
    let ready = text("ready").size(11);
    let actions = text(format!("actions: {dispatched}")).size(11);
    // Slice 41: surface opened_node_count alongside actions —
    // previously only `dispatched_action_count` was visible.
    let opens = text(format!("opens: {opened}")).size(11);
    let pending_text = text(format!("pending: {pending}")).size(11);
    let focused = text(format!("focused: {focused_label}")).size(11);

    container(
        iced::widget::row![
            dot,
            ready,
            iced::widget::Space::new().width(Length::Fixed(8.0)),
            actions,
            iced::widget::Space::new().width(Length::Fixed(8.0)),
            opens,
            iced::widget::Space::new().width(Length::Fixed(8.0)),
            pending_text,
            iced::widget::Space::new().width(Length::Fixed(8.0)),
            focused,
            iced::widget::Space::new().width(Length::Fill),
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center),
    )
    .padding([3, 8])
    .width(Length::Fill)
    .height(Length::Fixed(20.0))
    .style(|theme: &iced::Theme| {
        let pal = theme.palette();
        container::Style {
            background: Some(
                tokens::chrome_band(pal.background.base.text, tokens::CHROME_BAND_BASE)
                    .into(),
            ),
            ..Default::default()
        }
    })
    .into()
}

/// Render the host's toast queue as a stack of severity-prefixed rows.
pub(crate) fn render_toast_stack(
    toasts: &[graphshell_runtime::ToastSpec],
) -> iced::widget::Column<'_, Message> {
    if toasts.is_empty() {
        return iced::widget::column![];
    }
    let mut col = iced::widget::column![].spacing(4);
    for toast in toasts {
        let severity_tag = match toast.severity {
            ToastSeverity::Info => "ℹ",
            ToastSeverity::Success => "✓",
            ToastSeverity::Warning => "⚠",
            ToastSeverity::Error => "✗",
        };
        col = col.push(text(format!("{severity_tag} {}", toast.message)).size(12));
    }
    col
}
