/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Command-surface telemetry sink.
//!
//! M4 slice 10 (2026-04-22) moved the sink and all its data shapes to
//! [`graphshell_core::shell_state::command_surface_telemetry`] once
//! `PaneId` and `ToolSurfaceReturnTarget` became portable. The
//! `Mutex`-wrapped interior was already wasm-compatible
//! (empirically verified on `wasm32-unknown-unknown`), so no
//! Mutex-related refactor was required. Re-exports here preserve the
//! shell-side import paths.

pub(crate) use graphshell_core::shell_state::command_surface_telemetry::{
    CommandBarSemanticMetadata, CommandRouteEventSequenceMetadata,
    CommandSurfaceEventSequenceMetadata, CommandSurfaceSemanticSnapshot, CommandSurfaceTelemetry,
    OmnibarMailboxEventSequenceMetadata, OmnibarSemanticMetadata, PaletteSurfaceSemanticMetadata,
};
