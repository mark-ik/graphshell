/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Hand-rolled iced widgets for the Graphshell host.
//!
//! Per the 2026-04-30 decision (see [`iced_command_palette_spec.md` §1](
//! ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_command_palette_spec.md)),
//! Graphshell owns the small set of iced widgets it actually uses
//! rather than depending on the alpha-stage `iced_aw` crate. Each
//! widget here is an ordinary `iced::widget::Widget<Message, Theme>`
//! impl — no special trait or framework.
//!
//! Conventional import alias at the call site:
//!
//! ```ignore
//! use graphshell_iced_widgets as gs;
//! gs::TileTabs::new()...
//! gs::modal(content)...
//! gs::ContextMenu::new()...
//! ```
//!
//! ## Slice status
//!
//! - **`TileTabs` / `TileTab`** (Slice 4): real widget, materialised via
//!   `From<TileTabs<Message>> for Element<'_, Message>`.
//! - **`Modal`** (Slice 5): real widget, materialised via
//!   `From<Modal<'_, Message>> for Element<'_, Message>` — composes
//!   `stack`, `mouse_area`, and `opaque`.
//! - **`ContextMenu`** (Slice 5): real widget, materialised via
//!   `From<ContextMenu<Message>> for Element<'_, Message>` — composes
//!   `stack`, `pin`, and `opaque`.
//!
//! ## Naming
//!
//! The bar widget is **`TileTabs`** and one entry is **`TileTab`** —
//! never bare `Tabs`/`Tab`. In Graphshell the rendered view of a
//! graph node inside a Pane is the **Tile**; the clickable handle
//! that switches which tile is foregrounded is the tile's tab. Keeping
//! the qualified name avoids the egui_tiles-shaped conflation between
//! "the page" and "the handle that selects it".

pub mod context_menu;
pub mod modal;
pub mod tile_tabs;
pub mod tokens;

pub use context_menu::{ContextMenu, ContextMenuEntry};
pub use modal::{Modal, modal};
pub use tile_tabs::{TileTab, TileTabs};
