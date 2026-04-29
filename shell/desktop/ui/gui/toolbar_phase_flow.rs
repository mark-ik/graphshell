/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Toolbar + keyboard phase — delegates to `gui_frame` sub-phases and
//! post-processes their outputs (toggle-tile-view intents, post-submit
//! open-mode, toolbar visibility).
//!
//! Split out of `gui_orchestration.rs` as part of M6 §4.1. Owns:
//!
//! - [`run_keyboard_phase`] — keyboard phase driver with the
//!   host-specific toggle-tile-view + webview-state-reset closures
//!   embedded.
//! - [`run_toolbar_phase`] — toolbar-dialog phase driver that returns
//!   `(toolbar_visible, is_graph_view)`.
//! - Private helpers for toggle-tile-view and post-submit open mode.
//! - `open_mode_from_toolbar` — shared conversion helper.

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use egui_tiles::Tree;
use servo::WebViewId;
use servo::{OffscreenRenderingContext, WindowRenderingContext};
use winit::window::Window;

use crate::app::{GraphBrowserApp, GraphIntent};
use crate::graph::NodeKey;
use crate::shell::desktop::host::running_app_state::RunningAppState;
use crate::shell::desktop::host::window::EmbedderWindow;
use crate::shell::desktop::runtime::control_panel::ControlPanel;
#[cfg(feature = "diagnostics")]
use crate::shell::desktop::runtime::diagnostics;
use crate::shell::desktop::ui::gui_frame::{self, ToolbarDialogPhaseArgs};
use crate::shell::desktop::ui::gui_state::{
    LocalFocusTarget, RuntimeFocusAuthorityState, ToolbarState,
};
use crate::shell::desktop::ui::omnibar_state::OmnibarSearchSession;
use crate::shell::desktop::ui::toolbar_routing::ToolbarOpenMode;
use crate::shell::desktop::workbench::tile_kind::TileKind;
use crate::shell::desktop::workbench::tile_view_ops::{TileOpenMode, ToggleTileViewArgs};
use graphshell_runtime::WebviewCreationBackpressureState;

pub(crate) fn open_mode_from_toolbar(mode: ToolbarOpenMode) -> TileOpenMode {
    match mode {
        ToolbarOpenMode::Tab => TileOpenMode::Tab,
        ToolbarOpenMode::SplitHorizontal => TileOpenMode::SplitHorizontal,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn run_keyboard_phase(
    ctx: &egui::Context,
    graph_app: &mut GraphBrowserApp,
    graph_surface_focused: bool,
    window: &EmbedderWindow,
    tiles_tree: &mut Tree<TileKind>,
    viewer_surfaces: &mut crate::shell::desktop::workbench::compositor_adapter::ViewerSurfaceRegistry,
    tile_favicon_textures: &mut HashMap<NodeKey, (u64, egui::TextureHandle)>,
    favicon_textures: &mut HashMap<WebViewId, (egui::TextureHandle, egui::load::SizedTexture)>,
    app_state: &Option<Rc<RunningAppState>>,
    rendering_context: &Rc<OffscreenRenderingContext>,
    window_rendering_context: &Rc<WindowRenderingContext>,
    responsive_webviews: &HashSet<WebViewId>,
    webview_creation_backpressure: &mut HashMap<NodeKey, WebviewCreationBackpressureState>,
    viewer_surface_host: &mut dyn graphshell_core::viewer_host::ViewerSurfaceHost<
        crate::shell::desktop::workbench::compositor_adapter::ViewerSurfaceRegistry,
    >,
    suppress_toggle_view: bool,
    frame_intents: &mut Vec<GraphIntent>,
) {
    gui_frame::handle_keyboard_phase(
        gui_frame::KeyboardPhaseArgs {
            ctx,
            graph_app,
            graph_surface_focused,
            window,
            tiles_tree,
            viewer_surfaces,
            tile_favicon_textures,
            favicon_textures,
            app_state,
            rendering_context,
            window_rendering_context,
            responsive_webviews,
            webview_creation_backpressure,
            viewer_surface_host,
            suppress_toggle_view,
        },
        frame_intents,
        |tiles_tree,
         graph_app,
         window,
         app_state,
         rendering_context,
         window_rendering_context,
         viewer_surfaces,
         viewer_surface_host,
         responsive_webviews,
         webview_creation_backpressure,
         frame_intents| {
            crate::shell::desktop::workbench::tile_view_ops::toggle_tile_view(ToggleTileViewArgs {
                tiles_tree,
                graph_app,
                window,
                app_state,
                base_rendering_context: rendering_context,
                window_rendering_context,
                viewer_surfaces,
                viewer_surface_host,
                responsive_webviews,
                webview_creation_backpressure,
                lifecycle_intents: frame_intents,
            });
        },
        |tiles_tree,
         viewer_surfaces,
         viewer_surface_host,
         tile_favicon_textures,
         favicon_textures| {
            crate::shell::desktop::workbench::tile_runtime::reset_runtime_webview_state(
                tiles_tree,
                viewer_surfaces,
                viewer_surface_host,
                tile_favicon_textures,
                favicon_textures,
            );
        },
    );
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn run_toolbar_phase(
    ctx: &egui::Context,
    root_ui: &mut egui::Ui,
    winit_window: &Window,
    state: &RunningAppState,
    graph_app: &mut GraphBrowserApp,
    control_panel: &mut ControlPanel,
    #[cfg(feature = "diagnostics")] diagnostics_state: &mut diagnostics::DiagnosticsState,
    window: &EmbedderWindow,
    tiles_tree: &mut Tree<TileKind>,
    graph_tree: &mut graph_tree::GraphTree<NodeKey>,
    graph_surface_focused: bool,
    focus_authority: &RuntimeFocusAuthorityState,
    local_widget_focus: &mut Option<LocalFocusTarget>,
    toolbar_state: &mut ToolbarState,
    clear_data_confirm_deadline_secs: &mut Option<f64>,
    focus_location_field_for_search: bool,
    omnibar_search_session: &mut Option<OmnibarSearchSession>,
    omnibar_provider_suggestion_driver: &mut Option<
        crate::shell::desktop::ui::toolbar::toolbar_provider_driver::ProviderSuggestionDriver,
    >,
    command_surface_telemetry:
        &crate::shell::desktop::ui::command_surface_telemetry::CommandSurfaceTelemetry,
    toasts: &mut egui_notify::Toasts,
    viewer_surfaces: &mut crate::shell::desktop::workbench::compositor_adapter::ViewerSurfaceRegistry,
    tile_favicon_textures: &mut HashMap<NodeKey, (u64, egui::TextureHandle)>,
    favicon_textures: &mut HashMap<WebViewId, (egui::TextureHandle, egui::load::SizedTexture)>,
    app_state: &Option<Rc<RunningAppState>>,
    rendering_context: &Rc<OffscreenRenderingContext>,
    window_rendering_context: &Rc<WindowRenderingContext>,
    responsive_webviews: &HashSet<WebViewId>,
    webview_creation_backpressure: &mut HashMap<NodeKey, WebviewCreationBackpressureState>,
    viewer_surface_host: &mut dyn graphshell_core::viewer_host::ViewerSurfaceHost<
        crate::shell::desktop::workbench::compositor_adapter::ViewerSurfaceRegistry,
    >,
    frame_intents: &mut Vec<GraphIntent>,
    open_node_tile_after_intents: &mut Option<TileOpenMode>,
) -> (bool, bool) {
    let toolbar_dialog_phase = gui_frame::handle_toolbar_dialog_phase(
        ToolbarDialogPhaseArgs {
            ctx,
            root_ui,
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
            editable: &mut toolbar_state.editable,
            focus_location_field_for_search,
            show_clear_data_confirm: &mut toolbar_state.show_clear_data_confirm,
            clear_data_confirm_deadline_secs,
            omnibar_search_session,
            omnibar_provider_suggestion_driver,
            command_surface_telemetry,
            toasts,
            viewer_surfaces,
            viewer_surface_host,
            tile_favicon_textures,
            favicon_textures,
            #[cfg(feature = "diagnostics")]
            diagnostics_state,
        },
        frame_intents,
    );
    let toolbar_output = toolbar_dialog_phase.toolbar_output;
    let is_graph_view = toolbar_dialog_phase.is_graph_view;
    handle_toolbar_toggle_tile_view_request(
        toolbar_output.toggle_tile_view_requested,
        tiles_tree,
        graph_app,
        window,
        app_state,
        rendering_context,
        window_rendering_context,
        viewer_surfaces,
        viewer_surface_host,
        responsive_webviews,
        webview_creation_backpressure,
        frame_intents,
    );
    handle_toolbar_open_selected_mode_after_submit(
        toolbar_output.open_selected_mode_after_submit,
        open_node_tile_after_intents,
    );

    (toolbar_output.toolbar_visible, is_graph_view)
}

#[allow(clippy::too_many_arguments)]
fn handle_toolbar_toggle_tile_view_request(
    toggle_tile_view_requested: bool,
    tiles_tree: &mut Tree<TileKind>,
    graph_app: &mut GraphBrowserApp,
    window: &EmbedderWindow,
    app_state: &Option<Rc<RunningAppState>>,
    rendering_context: &Rc<OffscreenRenderingContext>,
    window_rendering_context: &Rc<WindowRenderingContext>,
    viewer_surfaces: &mut crate::shell::desktop::workbench::compositor_adapter::ViewerSurfaceRegistry,
    viewer_surface_host: &mut dyn graphshell_core::viewer_host::ViewerSurfaceHost<
        crate::shell::desktop::workbench::compositor_adapter::ViewerSurfaceRegistry,
    >,
    responsive_webviews: &HashSet<WebViewId>,
    webview_creation_backpressure: &mut HashMap<NodeKey, WebviewCreationBackpressureState>,
    frame_intents: &mut Vec<GraphIntent>,
) {
    if toggle_tile_view_requested && !graph_app.history_health_summary().preview_mode_active {
        crate::shell::desktop::workbench::tile_view_ops::toggle_tile_view(ToggleTileViewArgs {
            tiles_tree,
            graph_app,
            window,
            app_state,
            base_rendering_context: rendering_context,
            window_rendering_context,
            viewer_surfaces,
            viewer_surface_host,
            responsive_webviews,
            webview_creation_backpressure,
            lifecycle_intents: frame_intents,
        });
    }
}

fn handle_toolbar_open_selected_mode_after_submit(
    open_selected_mode_after_submit: Option<ToolbarOpenMode>,
    open_node_tile_after_intents: &mut Option<TileOpenMode>,
) {
    if let Some(open_mode) = open_selected_mode_after_submit {
        *open_node_tile_after_intents = Some(open_mode_from_toolbar(open_mode));
    }
}
