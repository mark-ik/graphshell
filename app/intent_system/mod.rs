/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Intent vocabulary + selection state — the four files that
//! together describe "what the host wants the runtime to do" and
//! "what's currently selected for it to act on."
//!
//! Slice 69 of the Phase 5 `app/` decomposition. The files were
//! flat siblings inside `app/` pre-Slice-69; grouping them under
//! `intent_system/` makes the conceptual boundary visible and is
//! the prerequisite for eventually extracting them to a
//! `graphshell-intent-system` crate (blocked today by the
//! `app↔graph` cycle through `crate::graph::physics::GraphBrowserApp`).
//!
//! Members:
//! - [`intents`] — the intent enum/struct vocabulary
//!   (AppCommand, GraphIntent, GraphMutation, RuntimeEvent,
//!   ViewAction, NavigatorProjectionIntent, NodeStatusNoticeRequest,
//!   plus the BrowserCommand/EdgeCommand wedge re-exports from
//!   graphshell-core::intents per Slice 57).
//! - [`intent_phases`] — multi-phase intent application machinery
//!   (validation, sanctioned-write enforcement, side-effect ordering).
//! - [`selection`] — SelectionState + SelectionScope + the undo
//!   snapshot type.
//! - [`focus_selection`] — focused-node tracking that selection-aware
//!   intents read from.
//!
//! The `pub use` re-exports below keep all 946 in-tree
//! `crate::app::Foo` call sites resolving unchanged.

pub(crate) mod focus_selection;
pub(crate) mod intent_phases;
pub(crate) mod intents;
pub(crate) mod selection;

pub(crate) use selection::{SelectionScope, UndoRedoSnapshot};
pub use selection::{SelectionState, SelectionUpdateMode};

pub use intents::{
    AppCommand, BrowserCommand, BrowserCommandTarget, GraphIntent, GraphMutation,
    NodeStatusNoticeRequest, RuntimeEvent, RuntimeUserStylesheetSpec, ViewAction,
};
