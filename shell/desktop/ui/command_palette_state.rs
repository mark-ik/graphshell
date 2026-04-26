/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Command palette session state.
//!
//! Per the M4 runtime extraction (§3.2 Command routing), palette session
//! state lives on `GraphshellRuntime` and is host-neutral. The types
//! themselves moved to `graphshell_core::shell_state::command_palette`
//! in M4 slice 3 (2026-04-22) so they can be tested without building
//! the egui/servo graphshell graph.
//!
//! This module re-exports them at their original path so existing
//! `CommandPaletteSession` / `SearchPaletteScope` call sites resolve
//! unchanged.

#[allow(unused_imports)]
pub(crate) use graphshell_core::shell_state::command_palette::{
    CommandPaletteSession, SearchPaletteScope,
};
