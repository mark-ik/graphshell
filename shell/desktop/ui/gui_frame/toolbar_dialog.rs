/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::HashMap;

use egui_tiles::Tree;
use servo::WebViewId;
use winit::window::Window;

use super::super::dialog_panels::{self, DialogPanelsArgs};
use super::super::nav_targeting;
use super::super::navigator_context;
use crate::app::{GraphBrowserApp, GraphIntent};
use crate::graph::NodeKey;
use crate::shell::desktop::host::running_app_state::RunningAppState;
use crate::shell::desktop::host::window::EmbedderWindow;
use crate::shell::desktop::lifecycle::webview_status_sync;
use crate::shell::desktop::runtime::control_panel::ControlPanel;
use crate::shell::desktop::ui::gui_state::{LocalFocusTarget, RuntimeFocusAuthorityState};
use crate::shell::desktop::ui::shell_layout_pass::ShellLayoutPass;
use crate::shell::desktop::ui::toolbar::toolbar_ui::{
    self, OmnibarSearchSession, ToolbarUiInput, ToolbarUiOutput,
};
use crate::shell::desktop::ui::workbench_host::{self, WorkbenchLayerState};
use crate::shell::desktop::workbench::tile_kind::TileKind;

pub(crate) struct ToolbarDialogPhaseArgs<'a> {
    pub(crate) ctx: &'a egui::Context,
    pub(crate) winit_window: &'a Window,
    pub(crate) state: &'a RunningAppState,
    pub(crate) graph_app: &'a mut GraphBrowserApp,
    pub(crate) control_panel: &'a mut ControlPanel,
    pub(crate) window: &'a EmbedderWindow,
    pub(crate) tiles_tree: &'a mut Tree<TileKind>,
    pub(crate) graph_tree: &'a mut graph_tree::GraphTree<NodeKey>,
    pub(crate) graph_surface_focused: bool,
    pub(crate) focus_authority: &'a RuntimeFocusAuthorityState,
    pub(crate) local_widget_focus: &'a mut Option<LocalFocusTarget>,
    pub(crate) location: &'a mut String,
    pub(crate) location_dirty: &'a mut bool,
    pub(crate) location_submitted: &'a mut bool,
    pub(crate) focus_location_field_for_search: bool,
    pub(crate) show_clear_data_confirm: &'a mut bool,
    pub(crate) omnibar_search_session: &'a mut Option<OmnibarSearchSession>,
    pub(crate) toasts: &'a mut egui_notify::Toasts,
    pub(crate) viewer_surfaces: &'a mut crate::shell::desktop::workbench::compositor_adapter::ViewerSurfaceRegistry,
    pub(crate) tile_favicon_textures: &'a mut HashMap<NodeKey, (u64, egui::TextureHandle)>,
    pub(crate) favicon_textures:
        &'a mut HashMap<WebViewId, (egui::TextureHandle, egui::load::SizedTexture)>,
    #[cfg(feature = "diagnostics")]
    pub(crate) diagnostics_state:
        &'a mut crate::shell::desktop::runtime::diagnostics::DiagnosticsState,
}

pub(crate) struct ToolbarDialogPhaseOutput {
    pub(crate) is_graph_view: bool,
    pub(crate) toolbar_output: ToolbarUiOutput,
}

pub(crate) fn handle_toolbar_dialog_phase(
    args: ToolbarDialogPhaseArgs<'_>,
    frame_intents: &mut Vec<GraphIntent>,
) -> ToolbarDialogPhaseOutput {
    let ToolbarDialogPhaseArgs {
        ctx,
        winit_window,
        state,
        graph_app,
        control_panel,
        window,
        tiles_tree,
        graph_tree,
        graph_surface_focused,
        focus_authority,
        local_widget_focus,
        location,
        location_dirty,
        location_submitted,
        focus_location_field_for_search,
        show_clear_data_confirm,
        omnibar_search_session,
        toasts,
        viewer_surfaces,
        tile_favicon_textures,
        favicon_textures,
        #[cfg(feature = "diagnostics")]
        diagnostics_state,
    } = args;

    let active_webview_node = nav_targeting::active_node_pane_node(graph_app);
    let focused_toolbar_node_key = if graph_surface_focused {
        None
    } else {
        nav_targeting::chrome_projection_node(graph_app, window).or(active_webview_node)
    };
    let command_bar_focus_target = nav_targeting::command_bar_focus_target(
        window.focused_pane(),
        active_webview_node,
        focused_toolbar_node_key,
        graph_app.get_single_selected_node(),
    );
    let focused_content_status = webview_status_sync::focused_content_status(
        command_bar_focus_target.focused_node(),
        graph_app,
        window,
    );
    let navigator_ctx = navigator_context::compute_navigator_context(graph_app);
    let shell_layout_pass = ShellLayoutPass::new(ctx);
    let workbench_projection = shell_layout_pass.render_workbench(|| {
        workbench_host::render_workbench_host(
            ctx,
            graph_app,
            tiles_tree,
            graph_tree,
            command_bar_focus_target,
            show_clear_data_confirm,
        )
    });
    let toolbar_output = shell_layout_pass.render_command_bar(
        workbench_projection.layer_state,
        |workbench_layer_state: WorkbenchLayerState| {
            toolbar_ui::render_toolbar_ui(ToolbarUiInput {
                ctx,
                winit_window,
                state,
                graph_app,
                control_panel,
                window,
                tiles_tree,
                navigator_ctx: &navigator_ctx,
                command_bar_focus_target,
                workbench_layer_state,
                focused_content_status: &focused_content_status,
                runtime_focus_state: focus_authority.realized_focus_state.as_ref(),
                local_widget_focus,
                location,
                location_dirty,
                location_submitted,
                focus_location_field_for_search,
                show_clear_data_confirm,
                omnibar_search_session,
                frame_intents,
                #[cfg(feature = "diagnostics")]
                diagnostics_state,
            })
        },
    );
    workbench_host::render_fallback_graph_scope_toolbar_host(
        ctx,
        graph_app,
        tiles_tree,
        graph_tree,
        &workbench_projection,
        show_clear_data_confirm,
    );
    let shell_layout = shell_layout_pass.finish(workbench_projection, toolbar_output);
    let is_graph_view = matches!(
        shell_layout.projection.layer_state,
        workbench_host::WorkbenchLayerState::GraphOnly
            | workbench_host::WorkbenchLayerState::GraphOverlayActive
    );
    if !is_graph_view {
        graph_app.workspace.graph_runtime.hovered_graph_node = None;
    }

    let toolbar_output = shell_layout.toolbar_output;

    dialog_panels::render_dialog_panels(DialogPanelsArgs {
        ctx,
        graph_app,
        window,
        tiles_tree,
        viewer_surfaces,
        tile_favicon_textures,
        favicon_textures,
        frame_intents,
        location_dirty,
        location_submitted,
        show_clear_data_confirm,
        toasts,
    });

    ToolbarDialogPhaseOutput {
        is_graph_view,
        toolbar_output,
    }
}
