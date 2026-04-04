/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use crate::graph::GraphletKind;
use crate::shell::desktop::ui::toolbar::toolbar_ui::ToolbarUiOutput;
use crate::shell::desktop::ui::workbench_host::{WorkbenchChromeProjection, WorkbenchLayerState};
use egui::{Pos2, Rect, Vec2};

/// The hosting context for a graph canvas render unit.
///
/// A `GraphViewId` + layout/camera state is agnostic to its host. This enum
/// captures the three valid hosting contexts as defined in
/// `shell_composition_model_spec.md §4`.
///
/// Phase 4 note: `NavigatorSpecialty` requires `GraphletKind` to exist (now
/// implemented in `graph/graphlet.rs`) and Navigator specialty host
/// infrastructure. The variant is defined here to unblock downstream typing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum GraphCanvasHostCtx {
    /// Mounted directly by Shell in `GraphPrimary` (CentralPanel).
    /// No surrounding chrome. Lifecycle owned by Shell.
    ShellPrimary,

    /// Hosted as `TileKind::Graph(GraphViewId)` inside the Workbench tile tree.
    /// Surrounded by a tab strip. Lifecycle owned by Workbench.
    WorkbenchTile { tile_id: egui_tiles::TileId },

    /// Hosted inside a Navigator specialty host for a scoped graphlet view
    /// (ego, corridor, component, atlas, etc.).
    /// Surrounded by Navigator chrome. Lifecycle owned by Navigator.
    ///
    /// The `graphlet_kind` determines the edge family mask and layout algorithm
    /// Navigator configures for this specialty view.
    NavigatorSpecialty { graphlet_kind: GraphletKind },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ShellSlot {
    CommandBar,
    NavigatorLeft,
    NavigatorRight,
    WorkbenchArea,
    GraphPrimary,
    NavigatorBottom,
    StatusBar,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ShellSlotRects {
    pub(crate) command_bar: Rect,
    pub(crate) graph_primary: Rect,
    pub(crate) workbench_area: Option<Rect>,
    pub(crate) navigator_left: Option<Rect>,
    pub(crate) navigator_right: Option<Rect>,
    pub(crate) navigator_bottom: Option<Rect>,
    pub(crate) status_bar: Option<Rect>,
}

impl Default for ShellSlotRects {
    fn default() -> Self {
        let empty = Rect::from_min_size(Pos2::ZERO, Vec2::ZERO);
        Self {
            command_bar: empty,
            graph_primary: empty,
            workbench_area: None,
            navigator_left: None,
            navigator_right: None,
            navigator_bottom: None,
            status_bar: None,
        }
    }
}

pub(crate) struct ShellLayoutRenderOutput {
    pub(crate) projection: WorkbenchChromeProjection,
    pub(crate) toolbar_output: ToolbarUiOutput,
    pub(crate) slot_rects: ShellSlotRects,
}

pub(crate) struct ShellLayoutPass<'a> {
    ctx: &'a egui::Context,
}

impl<'a> ShellLayoutPass<'a> {
    pub(crate) fn new(ctx: &'a egui::Context) -> Self {
        Self { ctx }
    }

    pub(crate) fn render_workbench<F>(&self, render_workbench: F) -> WorkbenchChromeProjection
    where
        F: FnOnce() -> WorkbenchChromeProjection,
    {
        render_workbench()
    }

    pub(crate) fn render_command_bar<F>(
        &self,
        layer_state: WorkbenchLayerState,
        render_command_bar: F,
    ) -> ToolbarUiOutput
    where
        F: FnOnce(WorkbenchLayerState) -> ToolbarUiOutput,
    {
        render_command_bar(layer_state)
    }

    pub(crate) fn finish(
        &self,
        projection: WorkbenchChromeProjection,
        toolbar_output: ToolbarUiOutput,
    ) -> ShellLayoutRenderOutput {
        let mut slot_rects = ShellSlotRects {
            command_bar: toolbar_output
                .command_bar_rect
                .unwrap_or_else(|| Rect::from_min_size(Pos2::ZERO, Vec2::ZERO)),
            status_bar: toolbar_output.status_bar_rect,
            ..ShellSlotRects::default()
        };
        if matches!(
            projection.layer_state,
            WorkbenchLayerState::WorkbenchActive | WorkbenchLayerState::WorkbenchPinned
        ) {
            slot_rects.workbench_area = Some(self.ctx.available_rect());
        }

        ShellLayoutRenderOutput {
            projection,
            toolbar_output,
            slot_rects,
        }
    }
}
