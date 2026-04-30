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
//! Slice 1 scope: signature-only scaffolding. The `Widget` impl is
//! deferred to the S4 sub-slice that materializes the first consuming
//! surface (Command Palette is the natural first).

use iced::Element;

/// Modal overlay around a content `Element`. Renders the content
/// centered above a translucent scrim; clicking the scrim emits the
/// `on_blur` message if configured.
///
/// Slice 1: builder shape only. The `Widget` impl in S4 will read
/// every field below, so the lints are pre-silenced rather than
/// flagged as dead code today.
#[allow(dead_code)]
pub struct Modal<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer> {
    pub(crate) content: Element<'a, Message, Theme, Renderer>,
    pub(crate) on_blur: Option<Message>,
    pub(crate) max_width: Option<f32>,
    pub(crate) max_height: Option<f32>,
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
        }
    }

    /// Set the message dispatched when the user clicks outside the
    /// modal card (or presses Escape).
    pub fn on_blur(mut self, message: Message) -> Self {
        self.on_blur = Some(message);
        self
    }

    /// Cap the modal card's width.
    pub fn max_width(mut self, width: f32) -> Self {
        self.max_width = Some(width);
        self
    }

    /// Cap the modal card's height.
    pub fn max_height(mut self, height: f32) -> Self {
        self.max_height = Some(height);
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

// TODO(S4): impl iced::widget::Widget<Message, Theme, Renderer> for Modal,
// plus From<Modal<...>> for Element<...>. The Widget impl owns:
//   - layout: full-bounds scrim + centered card with bounded size
//   - draw: scrim color from theme, card background, drop shadow
//   - on_event: pointer events on the scrim → on_blur; events on the
//     card route to inner content
//   - operate: focus trap inside the card; restore on close
// Reference shape: iced::widget::Container + iced::overlay layering.
