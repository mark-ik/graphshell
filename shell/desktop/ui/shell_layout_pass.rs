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
            WorkbenchLayerState::WorkbenchActive
                | WorkbenchLayerState::WorkbenchPinned
                | WorkbenchLayerState::WorkbenchOnly
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

#[cfg(test)]
mod tests {
    use super::{ShellLayoutPass, ShellSlotRects};
    use crate::app::workbench_layout_policy::{AnchorEdge, NavigatorHostId};
    use crate::app::{NavigatorHostScope, SurfaceHostId};
    use crate::shell::desktop::ui::toolbar::toolbar_ui::ToolbarUiOutput;
    use crate::shell::desktop::ui::workbench_host::{
        WorkbenchChromeProjection, WorkbenchHostFormFactor, WorkbenchHostLayout,
        WorkbenchLayerState,
    };
    use egui::{Pos2, Rect, Vec2};

    fn workbench_projection(layer_state: WorkbenchLayerState) -> WorkbenchChromeProjection {
        WorkbenchChromeProjection {
            layer_state,
            chrome_policy: layer_state.chrome_policy(),
            host_layout: WorkbenchHostLayout {
                host: SurfaceHostId::Navigator(NavigatorHostId::Left),
                anchor_edge: AnchorEdge::Left,
                form_factor: WorkbenchHostFormFactor::Sidebar,
                configured_scope: NavigatorHostScope::Both,
                resolved_scope: NavigatorHostScope::Both,
                size_fraction: 0.15,
                cross_axis_margin_start_px: 0.0,
                cross_axis_margin_end_px: 0.0,
                resizable: true,
            },
            host_layouts: Vec::new(),
            active_graph_view: None,
            extra_graph_views: Vec::new(),
            active_pane_title: None,
            active_frame_name: None,
            saved_frame_names: Vec::new(),
            navigator_groups: Vec::new(),
            pane_entries: Vec::new(),
            tree_root: None,
            active_graphlet_roster: Vec::new(),
        }
    }

    #[test]
    fn finish_projects_status_bar_into_slot_rects() {
        let ctx = egui::Context::default();
        let status_rect = Rect::from_min_size(Pos2::new(0.0, 576.0), Vec2::new(800.0, 24.0));

        let mut shell_layout = None;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            let toolbar_output = ToolbarUiOutput {
                toggle_tile_view_requested: false,
                open_selected_mode_after_submit: None,
                toolbar_visible: true,
                command_bar_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 40.0))),
                status_bar_rect: Some(status_rect),
            };
            shell_layout = Some(ShellLayoutPass::new(ctx).finish(
                workbench_projection(WorkbenchLayerState::WorkbenchActive),
                toolbar_output,
            ));
        });

        let shell_layout = shell_layout.expect("layout pass should produce an output");
        assert_eq!(shell_layout.slot_rects.status_bar, Some(status_rect));
        assert!(shell_layout.slot_rects.workbench_area.is_some());
    }

    #[test]
    fn finish_leaves_status_bar_empty_when_toolbar_output_has_no_status_rect() {
        let ctx = egui::Context::default();

        let mut slot_rects = None;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            let toolbar_output = ToolbarUiOutput {
                toggle_tile_view_requested: false,
                open_selected_mode_after_submit: None,
                toolbar_visible: false,
                command_bar_rect: None,
                status_bar_rect: None,
            };
            slot_rects = Some(
                ShellLayoutPass::new(ctx)
                    .finish(
                        workbench_projection(WorkbenchLayerState::GraphOnly),
                        toolbar_output,
                    )
                    .slot_rects,
            );
        });

        assert_eq!(
            slot_rects.expect("slot rects should exist"),
            ShellSlotRects {
                command_bar: Rect::from_min_size(Pos2::ZERO, Vec2::ZERO),
                graph_primary: Rect::from_min_size(Pos2::ZERO, Vec2::ZERO),
                workbench_area: None,
                navigator_left: None,
                navigator_right: None,
                navigator_bottom: None,
                status_bar: None,
            }
        );
    }

    #[test]
    fn finish_assigns_workbench_area_for_dedicated_workbench_mode() {
        let ctx = egui::Context::default();

        let mut shell_layout = None;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            let toolbar_output = ToolbarUiOutput {
                toggle_tile_view_requested: false,
                open_selected_mode_after_submit: None,
                toolbar_visible: true,
                command_bar_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 40.0))),
                status_bar_rect: None,
            };
            shell_layout = Some(ShellLayoutPass::new(ctx).finish(
                workbench_projection(WorkbenchLayerState::WorkbenchOnly),
                toolbar_output,
            ));
        });

        let shell_layout = shell_layout.expect("layout pass should produce an output");
        assert!(shell_layout.slot_rects.workbench_area.is_some());
    }

    #[test]
    fn finish_keeps_overlay_workbench_out_of_split_slot_rects() {
        let ctx = egui::Context::default();

        let mut shell_layout = None;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            let toolbar_output = ToolbarUiOutput {
                toggle_tile_view_requested: false,
                open_selected_mode_after_submit: None,
                toolbar_visible: true,
                command_bar_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 40.0))),
                status_bar_rect: None,
            };
            shell_layout = Some(ShellLayoutPass::new(ctx).finish(
                workbench_projection(WorkbenchLayerState::WorkbenchOverlayActive),
                toolbar_output,
            ));
        });

        let shell_layout = shell_layout.expect("layout pass should produce an output");
        assert!(shell_layout.slot_rects.workbench_area.is_none());
    }
}
