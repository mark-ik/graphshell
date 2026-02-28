/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{Receiver, Sender};

use crate::app::{GraphBrowserApp, GraphIntent, SearchDisplayMode};
use crate::graph::NodeKey;
use crate::services::search::fuzzy_match_node_keys;
use crate::shell::desktop::host::window::EmbedderWindow;
use crate::shell::desktop::host::running_app_state::RunningAppState;
use crate::shell::desktop::ui::graph_search_flow::{self, GraphSearchFlowArgs};
use crate::shell::desktop::ui::gui_frame::ToolbarDialogPhaseArgs;
use crate::shell::desktop::ui::gui_state::ToolbarState;
use crate::shell::desktop::ui::gui_frame::{self, PreFrameIngestArgs};
use crate::shell::desktop::ui::thumbnail_pipeline::ThumbnailCaptureResult;
use crate::shell::desktop::ui::toolbar::toolbar_ui::OmnibarSearchSession;
use crate::shell::desktop::ui::toolbar_routing::ToolbarOpenMode;
use crate::shell::desktop::workbench::tile_kind::TileKind;
use crate::shell::desktop::workbench::tile_view_ops::{TileOpenMode, ToggleTileViewArgs};
use crate::shell::desktop::lifecycle::webview_backpressure::WebviewCreationBackpressureState;
#[cfg(feature = "diagnostics")]
use crate::shell::desktop::runtime::diagnostics;
use egui_tiles::Tree;
use servo::{OffscreenRenderingContext, WindowRenderingContext};
use std::rc::Rc;
use winit::window::Window;
use servo::WebViewId;

pub(crate) struct PreFramePhaseOutput {
    pub(crate) frame_intents: Vec<GraphIntent>,
    pub(crate) pending_open_child_webviews: Vec<WebViewId>,
    pub(crate) responsive_webviews: HashSet<WebViewId>,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn run_pre_frame_phase(
    ctx: &egui::Context,
    graph_app: &mut GraphBrowserApp,
    state: &RunningAppState,
    window: &EmbedderWindow,
    favicon_textures: &mut HashMap<WebViewId, (egui::TextureHandle, egui::load::SizedTexture)>,
    thumbnail_capture_tx: &Sender<ThumbnailCaptureResult>,
    thumbnail_capture_rx: &Receiver<ThumbnailCaptureResult>,
    thumbnail_capture_in_flight: &mut HashSet<WebViewId>,
    command_palette_toggle_requested: &mut bool,
) -> PreFramePhaseOutput {
    let mut frame_intents = Vec::new();
    if *command_palette_toggle_requested {
        *command_palette_toggle_requested = false;
        frame_intents.push(GraphIntent::ToggleCommandPalette);
    }

    let pre_frame = gui_frame::ingest_pre_frame(
        PreFrameIngestArgs {
            ctx,
            graph_app,
            app_state: state,
            window,
            favicon_textures,
            thumbnail_capture_tx,
            thumbnail_capture_rx,
            thumbnail_capture_in_flight,
        },
        &mut frame_intents,
    );
    PreFramePhaseOutput {
        frame_intents,
        pending_open_child_webviews: pre_frame.pending_open_child_webviews,
        responsive_webviews: pre_frame.responsive_webviews,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn run_graph_search_phase(
    ctx: &egui::Context,
    graph_app: &mut GraphBrowserApp,
    graph_search_open: &mut bool,
    graph_search_query: &mut String,
    graph_search_filter_mode: &mut bool,
    graph_search_matches: &mut Vec<NodeKey>,
    graph_search_active_match_index: &mut Option<usize>,
    toolbar_state: &mut ToolbarState,
    frame_intents: &mut Vec<GraphIntent>,
    has_active_node_pane: bool,
) -> graph_search_flow::GraphSearchFlowOutput {
    let graph_search_available = !has_active_node_pane;
    graph_app.workspace.search_display_mode = if *graph_search_filter_mode {
        SearchDisplayMode::Filter
    } else {
        SearchDisplayMode::Highlight
    };
    graph_search_flow::handle_graph_search_flow(
        GraphSearchFlowArgs {
            ctx,
            graph_app,
            graph_search_open,
            graph_search_query,
            graph_search_filter_mode,
            graph_search_matches,
            graph_search_active_match_index,
            location: &mut toolbar_state.location,
            location_dirty: &mut toolbar_state.location_dirty,
            frame_intents,
            graph_search_available,
        },
        |graph_app, query, matches, active_index| {
            refresh_graph_search_matches(graph_app, query, matches, active_index);
        },
        |matches, active_index, delta| {
            step_graph_search_active_match(matches, active_index, delta);
        },
        |matches, active_index| active_graph_search_match(matches, active_index),
    )
}

pub(crate) fn active_graph_search_match(
    matches: &[NodeKey],
    active_index: Option<usize>,
) -> Option<NodeKey> {
    let idx = active_index?;
    matches.get(idx).copied()
}

pub(crate) fn refresh_graph_search_matches(
    graph_app: &GraphBrowserApp,
    query: &str,
    matches: &mut Vec<NodeKey>,
    active_index: &mut Option<usize>,
) {
    if query.trim().is_empty() {
        matches.clear();
        *active_index = None;
        return;
    }

    *matches = fuzzy_match_node_keys(&graph_app.workspace.graph, query);
    if matches.is_empty() {
        *active_index = None;
    } else if active_index.is_none_or(|idx| idx >= matches.len()) {
        *active_index = Some(0);
    }
}

fn step_graph_search_active_match(
    matches: &[NodeKey],
    active_index: &mut Option<usize>,
    step: isize,
) {
    if matches.is_empty() {
        *active_index = None;
        return;
    }

    let current = active_index.unwrap_or(0) as isize;
    let len = matches.len() as isize;
    let next = (current + step).rem_euclid(len) as usize;
    *active_index = Some(next);
}

pub(crate) fn open_mode_from_toolbar(mode: ToolbarOpenMode) -> TileOpenMode {
    match mode {
        ToolbarOpenMode::Tab => TileOpenMode::Tab,
        ToolbarOpenMode::SplitHorizontal => TileOpenMode::SplitHorizontal,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn run_toolbar_phase(
    ctx: &egui::Context,
    winit_window: &Window,
    state: &RunningAppState,
    graph_app: &mut GraphBrowserApp,
    #[cfg(feature = "diagnostics")] diagnostics_state: &mut diagnostics::DiagnosticsState,
    window: &EmbedderWindow,
    tiles_tree: &mut Tree<TileKind>,
    focused_node_hint: Option<NodeKey>,
    graph_surface_focused: bool,
    toolbar_state: &mut ToolbarState,
    focus_location_field_for_search: bool,
    omnibar_search_session: &mut Option<OmnibarSearchSession>,
    toasts: &mut egui_notify::Toasts,
    tile_rendering_contexts: &mut HashMap<NodeKey, Rc<OffscreenRenderingContext>>,
    tile_favicon_textures: &mut HashMap<NodeKey, (u64, egui::TextureHandle)>,
    favicon_textures: &mut HashMap<WebViewId, (egui::TextureHandle, egui::load::SizedTexture)>,
    app_state: &Option<Rc<RunningAppState>>,
    rendering_context: &Rc<OffscreenRenderingContext>,
    window_rendering_context: &Rc<WindowRenderingContext>,
    responsive_webviews: &HashSet<WebViewId>,
    webview_creation_backpressure: &mut HashMap<NodeKey, WebviewCreationBackpressureState>,
    frame_intents: &mut Vec<GraphIntent>,
    open_node_tile_after_intents: &mut Option<TileOpenMode>,
) -> (bool, bool) {
    let toolbar_dialog_phase = gui_frame::handle_toolbar_dialog_phase(
        ToolbarDialogPhaseArgs {
            ctx,
            winit_window,
            state,
            graph_app,
            window,
            tiles_tree,
            focused_node_hint,
            graph_surface_focused,
            can_go_back: toolbar_state.can_go_back,
            can_go_forward: toolbar_state.can_go_forward,
            location: &mut toolbar_state.location,
            location_dirty: &mut toolbar_state.location_dirty,
            location_submitted: &mut toolbar_state.location_submitted,
            focus_location_field_for_search,
            show_clear_data_confirm: &mut toolbar_state.show_clear_data_confirm,
            omnibar_search_session,
            toasts,
            tile_rendering_contexts,
            tile_favicon_textures,
            favicon_textures,
            #[cfg(feature = "diagnostics")]
            diagnostics_state,
        },
        frame_intents,
    );
    let toolbar_output = toolbar_dialog_phase.toolbar_output;
    let is_graph_view = toolbar_dialog_phase.is_graph_view;
    if toolbar_output.toggle_tile_view_requested {
        crate::shell::desktop::workbench::tile_view_ops::toggle_tile_view(ToggleTileViewArgs {
            tiles_tree,
            graph_app,
            window,
            app_state,
            base_rendering_context: rendering_context,
            window_rendering_context,
            tile_rendering_contexts,
            responsive_webviews,
            webview_creation_backpressure,
            lifecycle_intents: frame_intents,
        });
    }
    if let Some(open_mode) = toolbar_output.open_selected_mode_after_submit {
        *open_node_tile_after_intents = Some(open_mode_from_toolbar(open_mode));
    }

    (toolbar_output.toolbar_visible, is_graph_view)
}
