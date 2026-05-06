/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Workspace routing + state — the four files that describe
//! "where the workspace sends things and what state it tracks
//! while doing so."
//!
//! Slice 70 of the Phase 5 `app/` decomposition. The files were
//! flat siblings inside `app/` pre-Slice-70; grouping them under
//! `workspace_routing/` makes the conceptual boundary visible.
//!
//! Members:
//! - [`routing`] — small route-target enums (SettingsRouteTarget,
//!   ToolSurfaceReturnTarget re-export from graphshell_core).
//! - [`workspace_routing`] — graphlet/arrangement partitioning logic
//!   (ArrangementProjectionGroup, ViewGraphletPartition).
//! - [`workspace_commands`] — GraphBrowserApp impls for command
//!   enqueueing/dispatch on the workspace session.
//! - [`workspace_state`] — typed sub-state structs extracted from
//!   GraphWorkspace (ChromeUiState, FrameHintTabRuntime, etc.).
//!
//! The `pub use` re-exports below preserve every existing
//! `crate::app::Foo` resolution.

pub(crate) mod routing;
pub(crate) mod workspace_commands;
pub(crate) mod workspace_routing;
pub(crate) mod workspace_state;

#[allow(unused_imports)]
pub use routing::{SettingsRouteTarget, ToolSurfaceReturnTarget};

#[allow(unused_imports)]
pub use workspace_routing::ViewGraphletPartition;

#[allow(unused_imports)]
pub use workspace_state::{
    ChromeUiState, FrameHintTabRuntime, FrameTileGroupRuntimeState, GraphTooltipTarget,
    GraphViewRuntimeState, NavigatorSpecialtyView, SemanticNavigationNodeRuntime,
    SemanticNavigationRuntimeState, VisibleNavigationRegionSet, WorkbenchNavigationGeometry,
    WorkbenchSessionState,
};
