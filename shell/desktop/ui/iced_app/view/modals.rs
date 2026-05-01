//! Modal-overlay render functions extracted from `view/mod.rs` —
//! Slice 44 / Phase D continuation. Covers the `gs::Modal`-backed
//! surfaces (Command Palette / Node Finder / Confirm Dialog /
//! Node Create / Frame Rename) plus `gs::ContextMenu` (right-click
//! overlay), and the helpers they share (`modal_fade_opacity`,
//! palette + finder row builders).

use super::*;

pub(crate) fn render_command_palette(app: &IcedApp) -> Element<'_, Message> {
    let input = text_input("Type a command or search…", &app.command_palette.query)
        .id(iced::widget::Id::new(PALETTE_INPUT_ID))
        .on_input(Message::PaletteQuery)
        .on_submit(Message::PaletteSubmitFocused)
        .size(14)
        .padding(6)
        .width(Length::Fill);

    let divider = rule::horizontal(1.0);

    let visible = visible_palette_actions(&app.command_palette);
    let focused = app.command_palette.focused_index;

    let results: Element<'_, Message> = if visible.is_empty() {
        let empty_label = if app.command_palette.query.is_empty() {
            "— No actions available"
        } else {
            "— No matching actions"
        };
        container(text(empty_label).size(12))
            .padding(12)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    } else {
        let rows: Vec<Element<'_, Message>> = visible
            .iter()
            .enumerate()
            .map(|(i, action)| render_palette_row(i, action, focused == Some(i)))
            .collect();
        scrollable(column(rows).spacing(2).padding(4))
            .height(Length::Fill)
            .into()
    };

    let footer = text("Esc to dismiss · ↑/↓ to navigate · Enter to run").size(11);

    let body = iced::widget::column![
        text("Command Palette").size(13),
        input,
        divider,
        results,
        footer,
    ]
    .spacing(8)
    .padding(12)
    .width(Length::Fill)
    .height(Length::Fill);

    // Slice 42 / 47: fade-in opacity computed from the host's
    // modal_opened_at. Duration 150ms with ease_out_cubic feels
    // snappy without a flash. The same clock + curve drives every
    // gs::Modal-backed surface — palette, finder, node-create,
    // frame-rename, and confirm-dialog all read this value.
    Modal::new(body)
        .on_blur(Message::PaletteCloseAndRestoreFocus)
        .max_width(640.0)
        .max_height(480.0)
        .opacity(modal_fade_opacity(app))
        .into()
}

/// Opacity factor for the active gs::Modal-backed surface. Reads
/// `IcedApp::modal_opened_at` and applies an ease-out-cubic curve
/// over a 150ms duration. Returns `1.0` if no timestamp is set or
/// the modal has been open longer than the duration. Mutually
/// exclusive overlays share the same clock — opening one modal
/// resets `modal_opened_at`, so the new surface fades in from the
/// scrim cleanly without ghost frames from the previous one.
pub(crate) fn modal_fade_opacity(app: &IcedApp) -> f32 {
    const FADE_MS: u64 = 150;
    let Some(opened_at) = app.modal_opened_at else {
        return 1.0;
    };
    let anim = animation::Animation {
        started_at: opened_at,
        duration: std::time::Duration::from_millis(FADE_MS),
    };
    let t = anim.progress(std::time::Instant::now());
    animation::ease_out_cubic(t)
}

/// One row in the Command Palette ranked-action list.
///
/// Layout: label (filling, bold-ish via size) on the left, optional
/// keybinding right-aligned. Description (if present) appears beneath
/// the label at smaller size. Disabled rows pass `None` to
/// `on_press_maybe`; focused rows get a brighter background.
pub(crate) fn render_palette_row<'a>(
    idx: usize,
    action: &'a RankedAction,
    is_focused: bool,
) -> Element<'a, Message> {
    // Header line: label + optional keybinding.
    let label_el: Element<'a, Message> = text(action.label.as_str()).size(13).width(Length::Fill).into();
    let header: Element<'a, Message> = if let Some(kb) = action.keybinding.as_deref() {
        iced::widget::row![label_el, text(kb).size(11)]
            .spacing(8)
            .align_y(iced::Alignment::Center)
            .into()
    } else {
        label_el
    };

    // Optional description line.
    let body: Element<'a, Message> = match action.description.as_deref() {
        Some(desc) if !desc.is_empty() => iced::widget::column![header, text(desc).size(11)]
            .spacing(2)
            .into(),
        _ => header,
    };

    let msg: Option<Message> = if action.is_available {
        Some(Message::PaletteActionSelected(idx))
    } else {
        None
    };

    let is_disabled = !action.is_available;

    button(body)
        .on_press_maybe(msg)
        .padding([6, 10])
        .width(Length::Fill)
        .style(move |theme: &iced::Theme, status| {
            let pal = theme.palette();
            let hovered = matches!(
                status,
                iced::widget::button::Status::Hovered
                    | iced::widget::button::Status::Pressed
            );
            let bg = if is_focused {
                Some(pal.primary.weak.color.into())
            } else if hovered && !is_disabled {
                Some(tokens::HOVER_OVERLAY_SUBTLE.into())
            } else {
                None
            };
            let text_color = if is_disabled {
                iced::Color {
                    a: pal.background.base.text.a * 0.4,
                    ..pal.background.base.text
                }
            } else if is_focused {
                pal.primary.weak.text
            } else {
                pal.background.base.text
            };
            iced::widget::button::Style {
                background: bg,
                text_color,
                border: iced::Border {
                    radius: 3.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        })
        .into()
}

/// Render the Node Finder modal. Per
/// [`iced_node_finder_spec.md` §3](
/// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_node_finder_spec.md).
///
/// Slice 7 renders real result rows from the (placeholder) result list
/// with click handlers and focused-state highlighting. Arrow-key nav
/// routes through `NodeFinderFocusUp/Down`; Enter fires the focused row
/// via `NodeFinderSubmitFocused`.
pub(crate) fn render_node_finder(app: &IcedApp) -> Element<'_, Message> {
    let input = text_input(
        "Search nodes by title, tag, URL, or content…",
        &app.node_finder.query,
    )
    .id(iced::widget::Id::new(NODE_FINDER_INPUT_ID))
    .on_input(Message::NodeFinderQuery)
    .on_submit(Message::NodeFinderSubmitFocused)
    .size(14)
    .padding(6)
    .width(Length::Fill);

    let divider = rule::horizontal(1.0);

    let visible = visible_finder_results(&app.node_finder);
    let focused = app.node_finder.focused_index;

    let results: Element<'_, Message> = if visible.is_empty() {
        let empty_label = if app.node_finder.query.is_empty() {
            "— No recently-active nodes yet"
        } else {
            "— No matching nodes"
        };
        container(text(empty_label).size(12))
            .padding(12)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    } else {
        let rows: Vec<Element<'_, Message>> = visible
            .iter()
            .enumerate()
            .map(|(i, result)| render_finder_row(i, result, focused == Some(i)))
            .collect();
        scrollable(column(rows).spacing(2).padding(4))
            .height(Length::Fill)
            .into()
    };

    let footer = text("Esc to dismiss · ↑/↓ to navigate · Enter to open").size(11);

    let body = iced::widget::column![
        text("Node Finder").size(13),
        input,
        divider,
        results,
        footer,
    ]
    .spacing(8)
    .padding(12)
    .width(Length::Fill)
    .height(Length::Fill);

    Modal::new(body)
        .on_blur(Message::NodeFinderCloseAndRestoreFocus)
        .max_width(640.0)
        .max_height(480.0)
        .opacity(modal_fade_opacity(app))
        .into()
}

/// One row in the Node Finder result list.
///
/// Layout: title (filling) + node-type chip on the right; address
/// (smaller, dimmer) beneath the title. The match-source badge from
/// the spec is folded into the type chip until styling tokens land.
pub(crate) fn render_finder_row<'a>(
    idx: usize,
    result: &'a NodeFinderResult,
    is_focused: bool,
) -> Element<'a, Message> {
    let title_el: Element<'a, Message> = text(result.title.as_str()).size(13).width(Length::Fill).into();
    let header: Element<'a, Message> = iced::widget::row![
        title_el,
        text(result.node_type.as_str()).size(10),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center)
    .into();

    let body = iced::widget::column![header, text(result.address.as_str()).size(11)]
        .spacing(2);

    button(body)
        .on_press(Message::NodeFinderResultSelected(idx))
        .padding([6, 10])
        .width(Length::Fill)
        .style(move |theme: &iced::Theme, status| {
            let pal = theme.palette();
            let hovered = matches!(
                status,
                iced::widget::button::Status::Hovered
                    | iced::widget::button::Status::Pressed
            );
            let bg = if is_focused {
                Some(pal.primary.weak.color.into())
            } else if hovered {
                Some(tokens::HOVER_OVERLAY_SUBTLE.into())
            } else {
                None
            };
            let text_color = if is_focused {
                pal.primary.weak.text
            } else {
                pal.background.base.text
            };
            iced::widget::button::Style {
                background: bg,
                text_color,
                border: iced::Border {
                    radius: 3.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        })
        .into()
}

/// Render the FrameRename label-input modal — Slice 34. Visible
/// whenever `FrameRenameState::is_open` is true. Click-outside
/// (`Modal::on_blur`) and Escape both fire `FrameRenameCancel`;
/// Enter on the text input fires `FrameRenameSubmit`.
pub(crate) fn render_frame_rename_modal(app: &IcedApp) -> Element<'_, Message> {
    let title = text("Rename frame").size(15);
    let body = text(format!(
        "Renaming \"{}\". Empty submissions are no-ops.",
        app.frame_label
    ))
    .size(13);
    let input = text_input("Frame label", &app.frame_rename.label_draft)
        .id(iced::widget::Id::new(FRAME_RENAME_INPUT_ID))
        .on_input(Message::FrameRenameInput)
        .on_submit(Message::FrameRenameSubmit)
        .size(14)
        .padding(6)
        .width(Length::Fill);

    let cancel = button(text("Cancel").size(13))
        .on_press(Message::FrameRenameCancel)
        .padding([6, 14]);
    let apply = button(text("Apply").size(13))
        .on_press(Message::FrameRenameSubmit)
        .padding([6, 14]);

    let buttons = iced::widget::row![
        iced::widget::Space::new().width(Length::Fill),
        cancel,
        apply,
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);

    let inner = iced::widget::column![title, body, input, buttons]
        .spacing(12)
        .padding(4)
        .width(Length::Fill);

    Modal::new(inner)
        .on_blur(Message::FrameRenameCancel)
        .max_width(420.0)
        .opacity(modal_fade_opacity(app))
        .into()
}

/// Render the NodeCreate URL-input modal — Slice 32. Visible
/// whenever `NodeCreateState::is_open` is true. Click-outside
/// (`Modal::on_blur`) and Escape both fire `NodeCreateCancel`;
/// Enter on the text input fires `NodeCreateSubmit`.
pub(crate) fn render_node_create_modal(app: &IcedApp) -> Element<'_, Message> {
    let title = text("Create node").size(15);
    let body = text("Enter a URL or address to open as a new node.").size(13);
    let input = text_input("https://…", &app.node_create.url_draft)
        .id(iced::widget::Id::new(NODE_CREATE_INPUT_ID))
        .on_input(Message::NodeCreateInput)
        .on_submit(Message::NodeCreateSubmit)
        .size(14)
        .padding(6)
        .width(Length::Fill);

    let cancel = button(text("Cancel").size(13))
        .on_press(Message::NodeCreateCancel)
        .padding([6, 14]);
    let create = button(text("Create").size(13))
        .on_press(Message::NodeCreateSubmit)
        .padding([6, 14]);

    let buttons = iced::widget::row![
        iced::widget::Space::new().width(Length::Fill),
        cancel,
        create,
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);

    let inner = iced::widget::column![title, body, input, buttons]
        .spacing(12)
        .padding(4)
        .width(Length::Fill);

    Modal::new(inner)
        .on_blur(Message::NodeCreateCancel)
        .max_width(480.0)
        .opacity(modal_fade_opacity(app))
        .into()
}

/// Render the confirmation modal that gates destructive intents.
/// Shown when `ConfirmDialogState::is_open` is `true`; click-outside
/// (`Modal::on_blur`) and Escape both fire `ConfirmDialogCancel`. Per
/// [`iced_command_palette_spec.md` §5](
/// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_command_palette_spec.md).
pub(crate) fn render_confirm_dialog(app: &IcedApp) -> Element<'_, Message> {
    let title = text("Confirm destructive action").size(15);
    let body = text(format!(
        "{} cannot be undone. Continue?",
        app.confirm_dialog.action_label
    ))
    .size(13);

    let cancel = button(text("Cancel").size(13))
        .on_press(Message::ConfirmDialogCancel)
        .padding([6, 14]);
    let confirm = button(text("Confirm").size(13))
        .on_press(Message::ConfirmDialogConfirm)
        .padding([6, 14])
        .style(|theme: &iced::Theme, status| {
            let pal = theme.palette();
            let hovered = matches!(
                status,
                iced::widget::button::Status::Hovered
                    | iced::widget::button::Status::Pressed
            );
            iced::widget::button::Style {
                background: Some(if hovered {
                    pal.danger.strong.color.into()
                } else {
                    pal.danger.base.color.into()
                }),
                text_color: pal.danger.strong.text,
                border: iced::Border {
                    radius: 3.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        });

    let buttons = iced::widget::row![
        iced::widget::Space::new().width(Length::Fill),
        cancel,
        confirm,
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);

    let inner = iced::widget::column![title, body, buttons]
        .spacing(12)
        .padding(4)
        .width(Length::Fill);

    Modal::new(inner)
        .on_blur(Message::ConfirmDialogCancel)
        .max_width(420.0)
        .opacity(modal_fade_opacity(app))
        .into()
}

/// Render the right-click context menu using `gs::ContextMenu`. The
/// widget itself does the positioning (via `pin` at the recorded
/// anchor) and the overlay layering (full-viewport dismiss area
/// behind an opaque menu panel). The host-side `ContextMenuItem`
/// pairs the display entry with an optional dispatch payload; only
/// the entry half is handed to the widget.
pub(crate) fn render_context_menu(app: &IcedApp) -> Element<'_, Message> {
    let mut menu = ContextMenu::new(app.context_menu.anchor)
        .on_select(Message::ContextMenuEntrySelected)
        .on_dismiss(Message::ContextMenuDismiss);
    for item in &app.context_menu.items {
        menu = menu.push(item.entry.clone());
    }
    menu.into()
}
