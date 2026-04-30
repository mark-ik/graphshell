/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Context menu — flat list of available actions, anchored to a
//! pointer position, opened on right-click.
//!
//! Per [`iced_command_palette_spec.md` §7.3](
//! ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_command_palette_spec.md)
//! and [`iced_composition_skeleton_spec.md` §7.3](
//! ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_composition_skeleton_spec.md):
//! triggered on mouse-right anywhere a target is identifiable;
//! actions sourced from `ActionRegistry::available_for(target, ...)`;
//! flat list, no submenus or nested categories (the egui-era two-tier
//! model is retired per the 2026-04-29 simplification).
//!
//! Slice 5 scope: materialised via `From<ContextMenu<Message>> for
//! iced::Element<'_, Message>`. Positioned using `iced::widget::pin`
//! inside a `stack` so the menu appears at the recorded anchor point.
//! `iced::widget::opaque` blocks click-through from menu to dismiss
//! area. Disabled entries render dimmed and accept no clicks.

use iced::widget::{button, column, container, mouse_area, row, text};
use iced::{Alignment, Border, Color, Element, Length, Point, Shadow, Vector};

/// One entry in a context menu. Disabled entries render with a
/// dimmed style and surface the `disabled_reason` on hover (per
/// [`iced_command_palette_spec.md` §3.4](
/// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_command_palette_spec.md)).
#[derive(Debug, Clone)]
pub struct ContextMenuEntry {
    pub label: String,
    pub shortcut_hint: Option<String>,
    pub destructive: bool,
    pub disabled_reason: Option<String>,
}

impl ContextMenuEntry {
    /// Construct an enabled, non-destructive entry.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            shortcut_hint: None,
            destructive: false,
            disabled_reason: None,
        }
    }

    /// Mark this entry destructive — render with a warning style and
    /// route through the host's `ConfirmDialog` gate before dispatch.
    pub fn destructive(mut self) -> Self {
        self.destructive = true;
        self
    }

    /// Attach a keyboard-shortcut hint (rendered right-aligned).
    pub fn shortcut_hint(mut self, hint: impl Into<String>) -> Self {
        self.shortcut_hint = Some(hint.into());
        self
    }

    /// Mark this entry disabled. The provided reason is surfaced as a
    /// tooltip / hover description so the user understands why the
    /// action cannot fire (selection-set mismatch, missing capability,
    /// etc.).
    pub fn disabled(mut self, reason: impl Into<String>) -> Self {
        self.disabled_reason = Some(reason.into());
        self
    }
}

/// A flat-list context menu anchored at a pointer position.
///
/// Materialised into an [`iced::Element`] via the `From` impl. Each
/// entry becomes a `button`; disabled entries pass `None` to
/// `on_press_maybe` so they render inert without special handling.
pub struct ContextMenu<Message> {
    pub(crate) entries: Vec<ContextMenuEntry>,
    pub(crate) anchor: Point,
    pub(crate) on_select: Option<Box<dyn Fn(usize) -> Message>>,
    pub(crate) on_dismiss: Option<Message>,
}

impl<Message> ContextMenu<Message> {
    /// Construct an empty menu anchored at `anchor` (typically the
    /// pointer position when the right-click fired).
    pub fn new(anchor: Point) -> Self {
        Self {
            entries: Vec::new(),
            anchor,
            on_select: None,
            on_dismiss: None,
        }
    }

    /// Append one entry.
    pub fn push(mut self, entry: ContextMenuEntry) -> Self {
        self.entries.push(entry);
        self
    }

    /// Append every entry from an iterator.
    pub fn push_iter(mut self, iter: impl IntoIterator<Item = ContextMenuEntry>) -> Self {
        self.entries.extend(iter);
        self
    }

    /// Register the click handler. Receives the entry index; the host
    /// translates into the appropriate `HostIntent`.
    pub fn on_select(mut self, f: impl Fn(usize) -> Message + 'static) -> Self {
        self.on_select = Some(Box::new(f));
        self
    }

    /// Register the dismiss handler (Escape or click outside).
    pub fn on_dismiss(mut self, message: Message) -> Self {
        self.on_dismiss = Some(message);
        self
    }
}

/// Materialises a [`ContextMenu`] into an iced `Element`.
///
/// Layout (bottom to top via [`iced::widget::stack`]):
///
/// 1. **Dismiss area**: full-viewport invisible layer wrapped in a
///    `mouse_area` that fires `on_dismiss` on left-press. Covers the
///    whole window so any click outside the menu panel dismisses it.
/// 2. **Menu panel**: a `column` of `button` rows wrapped in an
///    `opaque` container (blocks click-through to the dismiss layer)
///    then positioned at `anchor` via [`iced::widget::pin`].
///
/// Disabled entries receive `on_press_maybe(None)` — iced renders them
/// inert. Destructive entries receive a red text style. Shortcut hints
/// appear right-aligned in each row.
///
/// The menu panel has a fixed width of 200 px; screen-edge clamping is
/// deferred to a future slice that adds a `float` translation closure.
impl<'a, Message: Clone + 'a> From<ContextMenu<Message>> for Element<'a, Message> {
    fn from(widget: ContextMenu<Message>) -> Self {
        let ContextMenu { entries, anchor, on_select, on_dismiss } = widget;

        if entries.is_empty() {
            return iced::widget::space().width(Length::Fill).height(Length::Fill).into();
        }

        // ── Dismiss area ───────────────────────────────────────────────
        // Full-size invisible layer; fires on_dismiss on any press that
        // is not captured by the (opaque) menu panel above it.
        let dismiss_fill: Element<'a, Message> =
            container(iced::widget::space().width(Length::Fill).height(Length::Fill))
                .width(Length::Fill)
                .height(Length::Fill)
                .into();

        let dismiss: Element<'a, Message> = if let Some(msg) = on_dismiss {
            mouse_area(dismiss_fill).on_press(msg).into()
        } else {
            dismiss_fill
        };

        // ── Menu panel entries ─────────────────────────────────────────
        let entry_rows: Vec<Element<'a, Message>> = entries
            .into_iter()
            .enumerate()
            .map(|(i, entry)| build_entry(i, entry, on_select.as_ref()))
            .collect();

        let panel: Element<'a, Message> = container(
            column(entry_rows).spacing(0).width(Length::Shrink),
        )
        .width(200)
        .padding([4, 0])
        .style(|theme: &iced::Theme| {
            let pal = theme.palette();
            container::Style {
                background: Some(pal.background.base.color.into()),
                border: Border {
                    radius: 4.0.into(),
                    ..Default::default()
                },
                shadow: Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.3),
                    offset: Vector::new(0.0, 4.0),
                    blur_radius: 8.0,
                },
                ..Default::default()
            }
        })
        .into();

        // `opaque` prevents clicks inside the panel from also firing the
        // dismiss area's mouse_area below it.
        let pinned: Element<'a, Message> =
            iced::widget::pin(iced::widget::opaque(panel))
                .x(anchor.x)
                .y(anchor.y)
                .into();

        // ── Stack ──────────────────────────────────────────────────────
        iced::widget::stack![dismiss, pinned]
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}

/// Builds one context-menu entry button.
fn build_entry<'a, Message: Clone + 'a>(
    idx: usize,
    entry: ContextMenuEntry,
    on_select: Option<&Box<dyn Fn(usize) -> Message>>,
) -> Element<'a, Message> {
    let is_disabled = entry.disabled_reason.is_some();
    let is_destructive = entry.destructive;

    // Row content: label (fills) + optional right-aligned shortcut hint.
    let label_el: Element<'a, Message> = text(entry.label).size(13).width(Length::Fill).into();
    let content: Element<'a, Message> = if let Some(hint) = entry.shortcut_hint {
        row![label_el, text(hint).size(11)]
            .spacing(8)
            .align_y(Alignment::Center)
            .into()
    } else {
        label_el
    };

    // Message is None for disabled entries — iced renders them inert.
    let msg: Option<Message> = if is_disabled {
        None
    } else {
        on_select.map(|f| f(idx))
    };

    button(content)
        .on_press_maybe(msg)
        .padding([4, 12])
        .width(Length::Fill)
        .style(move |theme: &iced::Theme, status| {
            let pal = theme.palette();
            let hovered =
                matches!(status, button::Status::Hovered | button::Status::Pressed);

            let text_color = if is_disabled {
                // Dimmed: 40% opacity approximated via a grey blend.
                Color {
                    a: pal.background.base.text.a * 0.4,
                    ..pal.background.base.text
                }
            } else if is_destructive {
                // Red-tinted text for destructive actions.
                Color::from_rgb(0.8, 0.2, 0.2)
            } else {
                pal.background.base.text
            };

            button::Style {
                background: if hovered && !is_disabled {
                    Some(Color::from_rgba(1.0, 1.0, 1.0, 0.08).into())
                } else {
                    None
                },
                text_color,
                border: Border::default(),
                ..Default::default()
            }
        })
        .into()
}
