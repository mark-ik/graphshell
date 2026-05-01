//! Chrome surfaces — CommandBar, StatusBar, FrameSwitcher,
//! DropZoneHint, ToastStack. Phase D extraction from view/mod.rs.

use super::*;

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
