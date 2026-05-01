/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Modal overlay widget — a centered card with a scrim that captures
//! click-outside.
//!
//! Used by:
//! - the [Command Palette](
//!   ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_command_palette_spec.md);
//! - the [Node Finder](
//!   ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_node_finder_spec.md);
//! - the expanded swatch hover preview (per
//!   [`iced_composition_skeleton_spec.md` §6.2](
//!   ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_composition_skeleton_spec.md)).
//!
//! Slice 5 scope: materialised via `From<Modal<'_, Message>> for
//! iced::Element<'_, Message>` — composes native iced widgets (stack,
//! mouse_area, container, opaque) so no raw Widget trait impl is needed.
//! Event routing: `mouse_area` on the scrim fires `on_blur`; `opaque`
//! on the card blocks events from passing through to the scrim.

use iced::widget::container;
use iced::widget::mouse_area;
use iced::{Border, Color, Element, Length, Shadow, Vector};

use crate::tokens;

/// Modal overlay around a content `Element`. Renders the content
/// centered above a translucent scrim; clicking the scrim emits the
/// `on_blur` message if configured.
#[allow(dead_code)]
pub struct Modal<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer> {
    pub(crate) content: Element<'a, Message, Theme, Renderer>,
    pub(crate) on_blur: Option<Message>,
    pub(crate) max_width: Option<f32>,
    pub(crate) max_height: Option<f32>,
    /// `[0.0, 1.0]` opacity multiplier applied to the scrim and card.
    /// `1.0` means the modal renders at full opacity (default);
    /// values below `1.0` fade the modal in. Hosts typically drive
    /// this from `graphshell_iced_widgets::animation::ease_out_cubic`
    /// against a "modal opened" instant. Slice 42.
    pub(crate) opacity: f32,
}

impl<'a, Message, Theme, Renderer> Modal<'a, Message, Theme, Renderer>
where
    Message: Clone,
{
    /// Construct a modal wrapping `content`.
    pub fn new(content: impl Into<Element<'a, Message, Theme, Renderer>>) -> Self {
        Self {
            content: content.into(),
            on_blur: None,
            max_width: None,
            max_height: None,
            opacity: 1.0,
        }
    }

    /// Set the message dispatched when the user clicks outside the
    /// modal card (or presses Escape).
    pub fn on_blur(mut self, message: Message) -> Self {
        self.on_blur = Some(message);
        self
    }

    /// Cap the modal card's width in pixels.
    pub fn max_width(mut self, width: f32) -> Self {
        self.max_width = Some(width);
        self
    }

    /// Cap the modal card's height in pixels.
    pub fn max_height(mut self, height: f32) -> Self {
        self.max_height = Some(height);
        self
    }

    /// Set the opacity multiplier (clamped to `[0.0, 1.0]`). The
    /// scrim and card alpha both scale by this factor; `1.0` is
    /// fully opaque (default). Drives modal fade-in animations.
    pub fn opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity.clamp(0.0, 1.0);
        self
    }
}

/// Free-function constructor matching the `gs::modal(content)` shape
/// used in the canonical specs.
pub fn modal<'a, Message, Theme, Renderer>(
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
) -> Modal<'a, Message, Theme, Renderer>
where
    Message: Clone,
{
    Modal::new(content)
}

/// Materialises a [`Modal`] into an iced `Element`.
///
/// Layout (bottom to top via [`iced::widget::stack`]):
///
/// 1. **Scrim**: full-viewport semi-transparent `container` wrapped in
///    a `mouse_area` that fires `on_blur` on left-press. If `on_blur`
///    is `None`, the scrim still renders but does not emit messages.
/// 2. **Card**: the content wrapped in a styled `container` (rounded
///    corners, drop shadow, opaque background), then wrapped in
///    `iced::widget::opaque` so pointer events on the card do not
///    fall through to the scrim's `mouse_area`, then centered via an
///    outer `container`.
///
/// `max_width` / `max_height` constrain the card size. Without them
/// the card shrinks to fit its content.
impl<'a, Message: Clone + 'a> From<Modal<'a, Message>> for Element<'a, Message> {
    fn from(widget: Modal<'a, Message>) -> Self {
        let Modal { content, on_blur, max_width, max_height, opacity } = widget;

        // ── Scrim ──────────────────────────────────────────────────────
        // Full-viewport translucent overlay. The inner space fills all
        // available room so the scrim/mouse_area also fills.
        // Slice 42: scrim alpha multiplied by `opacity` so a modal
        // fade-in shows the underlying chrome through a less-opaque
        // scrim during the animation.
        let scrim_alpha = tokens::MODAL_SCRIM.a * opacity;
        let scrim_inner: Element<'a, Message> = container(
            iced::widget::space().width(Length::Fill).height(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_: &iced::Theme| container::Style {
            background: Some(
                Color {
                    a: scrim_alpha,
                    ..tokens::MODAL_SCRIM
                }
                .into(),
            ),
            ..Default::default()
        })
        .into();

        let scrim: Element<'a, Message> = if let Some(msg) = on_blur {
            mouse_area(scrim_inner).on_press(msg).into()
        } else {
            scrim_inner
        };

        // ── Card ───────────────────────────────────────────────────────
        // Styled panel wrapping the caller's content. `opaque` ensures
        // clicks inside the card do not propagate to the scrim layer.
        // Slice 42: card background + shadow alpha both modulate by
        // `opacity` so the card fades in along with the scrim.
        let mut card_inner = container(content)
            .padding(16)
            .style(move |theme: &iced::Theme| {
                let pal = theme.palette();
                let mut bg = pal.background.base.color;
                bg.a *= opacity;
                container::Style {
                    background: Some(bg.into()),
                    border: Border {
                        radius: tokens::RADIUS_MODAL.into(),
                        ..Default::default()
                    },
                    shadow: Shadow {
                        color: Color::from_rgba(0.0, 0.0, 0.0, 0.4 * opacity),
                        offset: Vector::new(0.0, 4.0),
                        blur_radius: 16.0,
                    },
                    ..Default::default()
                }
            });
        if let Some(w) = max_width {
            card_inner = card_inner.max_width(w);
        }
        if let Some(h) = max_height {
            card_inner = card_inner.max_height(h);
        }

        // Centered outer container: fills all space and centers the
        // (opaque-wrapped) card within it.
        let card: Element<'a, Message> =
            container(iced::widget::opaque(card_inner))
                .center(Length::Fill)
                .into();

        // ── Stack ──────────────────────────────────────────────────────
        iced::widget::stack![scrim, card]
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}
