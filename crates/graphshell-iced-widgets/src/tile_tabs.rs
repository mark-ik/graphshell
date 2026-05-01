/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Tile-tab bar — a horizontal row of clickable handles, one per
//! Active tile in a tile pane's graphlet.
//!
//! Per [`iced_composition_skeleton_spec.md` §4.1](
//! ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_composition_skeleton_spec.md):
//! a [`TileTabs`] widget renders inside each tile [`Pane`]; clicking a
//! [`TileTab`] activates that tile in the Pane. Closing a tab via the
//! close `×` deactivates the tile (Active → Inactive presentation
//! state) per TERMINOLOGY.md and the iced jump-ship plan §4.4.
//!
//! Naming note: a *tab* is a tile's tab — a handle, not the rendered
//! view that the handle selects. The view is the tile. Hence
//! `TileTab` / `TileTabs`, never bare `Tab` / `Tabs`.
//!
//! Implementation strategy: materialised via `From<TileTabs<Message>>
//! for iced::Element<'_, Message>` — each tab becomes a composited
//! `button` so iced's built-in event handling and text measurement are
//! reused. The inner close `×` button captures click events before the
//! outer tab button sees them, giving correct select-vs-close dispatch
//! without any manual hit-testing.

use iced::widget::{button, mouse_area, row, text};
use iced::{Alignment, Border, Color, Element, Length};

/// One entry in a [`TileTabs`] bar — a clickable handle that switches
/// which tile is foregrounded in the parent Pane.
#[derive(Debug, Clone)]
pub struct TileTab {
    /// Display label (typically the tile's title, possibly truncated).
    pub label: String,
    /// Whether this tab can be closed via a built-in `×`. Closing a
    /// tab emits the `on_close` message configured on the parent
    /// [`TileTabs`]; runtime semantics flip the tile from Active to
    /// Inactive presentation state without touching graph truth.
    pub closable: bool,
}

impl TileTab {
    /// Construct a tile tab with the given label. Closable by default;
    /// callers can opt out via [`TileTab::not_closable`] for system /
    /// pinned tiles that should not surface a close affordance.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            closable: true,
        }
    }

    /// Mark this tab as non-closable (no `×` rendered).
    pub fn not_closable(mut self) -> Self {
        self.closable = false;
        self
    }
}

/// A horizontal row of [`TileTab`]s, with optional select / close
/// callbacks. One [`TileTabs`] per tile [`Pane`].
///
/// Materialised into an [`iced::Element`] via the `From` impl; each
/// tab composes to a `button` so iced handles text measurement and
/// pointer interaction automatically.
pub struct TileTabs<Message> {
    pub(crate) tabs: Vec<TileTab>,
    pub(crate) selected: Option<usize>,
    pub(crate) on_select: Option<Box<dyn Fn(usize) -> Message>>,
    pub(crate) on_close: Option<Box<dyn Fn(usize) -> Message>>,
    pub(crate) on_right_click: Option<Box<dyn Fn(usize) -> Message>>,
}

impl<Message> TileTabs<Message> {
    /// Construct an empty tab bar.
    pub fn new() -> Self {
        Self {
            tabs: Vec::new(),
            selected: None,
            on_select: None,
            on_close: None,
            on_right_click: None,
        }
    }

    /// Append one [`TileTab`].
    pub fn push(mut self, tab: TileTab) -> Self {
        self.tabs.push(tab);
        self
    }

    /// Append every tab from an iterator.
    pub fn push_iter(mut self, iter: impl IntoIterator<Item = TileTab>) -> Self {
        self.tabs.extend(iter);
        self
    }

    /// Mark which tab is currently foregrounded (rendered with the
    /// selected style). `None` means no tile is active in the pane —
    /// rare; only happens transiently between activations.
    pub fn selected(mut self, index: Option<usize>) -> Self {
        self.selected = index;
        self
    }

    /// Register a click handler. Receives the index of the clicked
    /// tab; the host translates into `Message::ActivateTab { ... }`.
    pub fn on_select(mut self, f: impl Fn(usize) -> Message + 'static) -> Self {
        self.on_select = Some(Box::new(f));
        self
    }

    /// Register a close handler. Receives the index of the closed
    /// tab; the host translates into `Message::CloseTile { ... }`.
    pub fn on_close(mut self, f: impl Fn(usize) -> Message + 'static) -> Self {
        self.on_close = Some(Box::new(f));
        self
    }

    /// Register a right-click handler. Receives the index of the
    /// right-clicked tab; the host typically translates into a
    /// `ContextMenuOpen` message keyed on that tab's node identity.
    /// Wraps each tab's outer button in a `mouse_area` so iced's
    /// right-press capture fires before the button consumes the event.
    pub fn on_right_click(mut self, f: impl Fn(usize) -> Message + 'static) -> Self {
        self.on_right_click = Some(Box::new(f));
        self
    }
}

impl<Message> Default for TileTabs<Message> {
    fn default() -> Self {
        Self::new()
    }
}

/// Materialises a [`TileTabs`] into an iced `Element`.
///
/// Each tab becomes a `button` (outer) optionally containing a nested
/// close `button` (inner). Iced propagates pointer events depth-first
/// and stops at the first widget that captures — clicking `×` fires
/// `on_close(i)` only; clicking the label area fires `on_select(i)`.
impl<'a, Message: Clone + 'a> From<TileTabs<Message>> for Element<'a, Message> {
    fn from(widget: TileTabs<Message>) -> Self {
        let TileTabs {
            tabs,
            selected,
            on_select,
            on_close,
            on_right_click,
        } = widget;

        if tabs.is_empty() {
            // Empty tab bar: zero-height spacer so layout is stable.
            return iced::widget::Space::new().width(Length::Fill).into();
        }

        let cells: Vec<Element<'a, Message>> = tabs
            .into_iter()
            .enumerate()
            .map(|(i, tab)| {
                build_tab_cell(
                    tab.label,
                    tab.closable,
                    selected == Some(i),
                    on_select.as_ref().map(|f| f(i)),
                    on_close.as_ref().map(|f| f(i)),
                    on_right_click.as_ref().map(|f| f(i)),
                )
            })
            .collect();

        row(cells)
            .spacing(1)
            .height(Length::Shrink)
            .into()
    }
}

/// Builds a single tab cell element.
///
/// `select_msg` drives the outer button; `close_msg` drives the inner
/// `×` button. When both are present, iced's event-capture propagation
/// ensures close fires without also triggering select.
fn build_tab_cell<'a, Message: Clone + 'a>(
    label: String,
    closable: bool,
    is_selected: bool,
    select_msg: Option<Message>,
    close_msg: Option<Message>,
    right_click_msg: Option<Message>,
) -> Element<'a, Message> {
    let label_el: Element<'a, Message> = text(label).size(13).into();

    let content: Element<'a, Message> = if closable {
        let close_btn = button(text("×").size(11))
            .on_press_maybe(close_msg)
            .padding([1, 4])
            .style(|_theme: &iced::Theme, status| button::Style {
                background: match status {
                    button::Status::Hovered | button::Status::Pressed => {
                        Some(Color::from_rgba(1.0, 1.0, 1.0, 0.15).into())
                    }
                    _ => None,
                },
                border: Border { radius: 2.0.into(), ..Default::default() },
                ..Default::default()
            });

        row![label_el, close_btn]
            .spacing(4)
            .align_y(Alignment::Center)
            .into()
    } else {
        label_el
    };

    let tab_button = button(content)
        .on_press_maybe(select_msg)
        .padding([4, 8])
        .style(move |theme: &iced::Theme, status| {
            let pal = theme.palette();
            let hovered = matches!(
                status,
                button::Status::Hovered | button::Status::Pressed
            );
            button::Style {
                background: Some(if is_selected {
                    pal.primary.strong.color.into()
                } else if hovered {
                    Color::from_rgba(1.0, 1.0, 1.0, 0.08).into()
                } else {
                    Color::TRANSPARENT.into()
                }),
                text_color: if is_selected {
                    pal.primary.strong.text
                } else {
                    pal.background.base.text
                },
                border: Border { radius: 4.0.into(), ..Default::default() },
                ..Default::default()
            }
        });

    // Wrap the tab button in a mouse_area when a right-click handler
    // is configured, so iced's right-press capture fires the host's
    // ContextMenuOpen-shaped message before the underlying button
    // sees the event.
    if let Some(msg) = right_click_msg {
        mouse_area(tab_button).on_right_press(msg).into()
    } else {
        tab_button.into()
    }
}
