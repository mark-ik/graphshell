/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Semantic lifecycle phase — runs after workbench-authority intents
//! have been intercepted. Applies remaining intents through the
//! reducer and reconciles webview lifecycle state.
//!
//! Split out of `gui_orchestration.rs` as part of M6 §4.1. Owns the
//! top-level phase entry + its two internal helpers. The shared
//! `workbench_intent_interceptor` submodule and
//! `gui_frame::run_lifecycle_reconcile_and_apply` do the actual work;
//! this module threads arguments between them.

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use egui_tiles::Tree;
use servo::WebViewId;
use servo::{OffscreenRenderingContext, WindowRenderingContext};

use crate::app::{GraphBrowserApp, GraphIntent};
use crate::graph::NodeKey;
use crate::shell::desktop::host::running_app_state::RunningAppState;
use crate::shell::desktop::host::window::EmbedderWindow;
use crate::shell::desktop::lifecycle::webview_backpressure::WebviewCreationBackpressureState;
use crate::shell::desktop::ui::gui_frame::{self};
use crate::shell::desktop::ui::gui_state::RuntimeFocusAuthorityState;
use crate::shell::desktop::workbench::tile_kind::TileKind;
use crate::shell::desktop::workbench::tile_view_ops::TileOpenMode;

use super::workbench_intent_interceptor;

#[allow(clippy::too_many_arguments)]
pub(crate) fn run_semantic_lifecycle_phase(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    graph_tree: &mut graph_tree::GraphTree<NodeKey>,
    modal_surface_active: bool,
    focus_authority: &mut RuntimeFocusAuthorityState,
    window: &EmbedderWindow,
    app_state: &Option<Rc<RunningAppState>>,
    rendering_context: &Rc<OffscreenRenderingContext>,
    window_rendering_context: &Rc<WindowRenderingContext>,
    viewer_surfaces: &mut crate::shell::desktop::workbench::compositor_adapter::ViewerSurfaceRegistry,
    tile_favicon_textures: &mut HashMap<NodeKey, (u64, egui::TextureHandle)>,
    favicon_textures: &mut HashMap<WebViewId, (egui::TextureHandle, egui::load::SizedTexture)>,
    responsive_webviews: &HashSet<WebViewId>,
    webview_creation_backpressure: &mut HashMap<NodeKey, WebviewCreationBackpressureState>,
    open_node_tile_after_intents: &mut Option<TileOpenMode>,
    frame_intents: &mut Vec<GraphIntent>,
) {
    apply_semantic_intents_and_pending_open(
        graph_app,
        tiles_tree,
        Some(graph_tree),
        modal_surface_active,
        focus_authority,
        open_node_tile_after_intents,
        frame_intents,
    );

    reconcile_semantic_lifecycle_phase(
        graph_app,
        tiles_tree,
        window,
        app_state,
        rendering_context,
        window_rendering_context,
        viewer_surfaces,
        tile_favicon_textures,
        favicon_textures,
        responsive_webviews,
        webview_creation_backpressure,
        frame_intents,
    );
}

fn apply_semantic_intents_and_pending_open(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    graph_tree: Option<&mut graph_tree::GraphTree<NodeKey>>,
    modal_surface_active: bool,
    focus_authority: &mut RuntimeFocusAuthorityState,
    open_node_tile_after_intents: &mut Option<TileOpenMode>,
    frame_intents: &mut Vec<GraphIntent>,
) {
    workbench_intent_interceptor::apply_semantic_intents_and_pending_open(
        graph_app,
        tiles_tree,
        graph_tree,
        modal_surface_active,
        focus_authority,
        open_node_tile_after_intents,
        frame_intents,
    );
}

#[allow(clippy::too_many_arguments)]
fn reconcile_semantic_lifecycle_phase(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    window: &EmbedderWindow,
    app_state: &Option<Rc<RunningAppState>>,
    rendering_context: &Rc<OffscreenRenderingContext>,
    window_rendering_context: &Rc<WindowRenderingContext>,
    viewer_surfaces: &mut crate::shell::desktop::workbench::compositor_adapter::ViewerSurfaceRegistry,
    tile_favicon_textures: &mut HashMap<NodeKey, (u64, egui::TextureHandle)>,
    favicon_textures: &mut HashMap<WebViewId, (egui::TextureHandle, egui::load::SizedTexture)>,
    responsive_webviews: &HashSet<WebViewId>,
    webview_creation_backpressure: &mut HashMap<NodeKey, WebviewCreationBackpressureState>,
    frame_intents: &mut Vec<GraphIntent>,
) {
    gui_frame::run_lifecycle_reconcile_and_apply(
        gui_frame::LifecycleReconcilePhaseArgs {
            graph_app,
            tiles_tree,
            window,
            app_state,
            rendering_context,
            window_rendering_context,
            viewer_surfaces,
            tile_favicon_textures,
            favicon_textures,
            responsive_webviews,
            webview_creation_backpressure,
        },
        frame_intents,
    );
}
