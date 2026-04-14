/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use egui_tiles::Tree;
use servo::{OffscreenRenderingContext, WebViewId, WindowRenderingContext};

use crate::app::{GraphBrowserApp, GraphIntent};
use crate::graph::NodeKey;
use crate::input;
use crate::shell::desktop::host::running_app_state::RunningAppState;
use crate::shell::desktop::host::window::EmbedderWindow;
use crate::shell::desktop::lifecycle::webview_backpressure::WebviewCreationBackpressureState;
use crate::shell::desktop::lifecycle::webview_controller;
use crate::shell::desktop::ui::nav_targeting;
use crate::shell::desktop::ui::toolbar_routing;
use crate::shell::desktop::workbench::tile_kind::TileKind;

pub(crate) struct KeyboardPhaseArgs<'a> {
    pub(crate) ctx: &'a egui::Context,
    pub(crate) graph_app: &'a mut GraphBrowserApp,
    pub(crate) graph_surface_focused: bool,
    pub(crate) window: &'a EmbedderWindow,
    pub(crate) tiles_tree: &'a mut Tree<TileKind>,
    pub(crate) tile_rendering_contexts: &'a mut HashMap<NodeKey, Rc<OffscreenRenderingContext>>,
    pub(crate) tile_favicon_textures: &'a mut HashMap<NodeKey, (u64, egui::TextureHandle)>,
    pub(crate) favicon_textures:
        &'a mut HashMap<WebViewId, (egui::TextureHandle, egui::load::SizedTexture)>,
    pub(crate) app_state: &'a Option<Rc<RunningAppState>>,
    pub(crate) rendering_context: &'a Rc<OffscreenRenderingContext>,
    pub(crate) window_rendering_context: &'a Rc<WindowRenderingContext>,
    pub(crate) responsive_webviews: &'a HashSet<WebViewId>,
    pub(crate) webview_creation_backpressure:
        &'a mut HashMap<NodeKey, WebviewCreationBackpressureState>,
    pub(crate) suppress_toggle_view: bool,
}

pub(crate) fn handle_keyboard_phase<F1, F2>(
    args: KeyboardPhaseArgs<'_>,
    frame_intents: &mut Vec<GraphIntent>,
    mut toggle_tile_view: F1,
    mut reset_runtime_webview_state: F2,
) where
    F1: FnMut(
        &mut Tree<TileKind>,
        &mut GraphBrowserApp,
        &EmbedderWindow,
        &Option<Rc<RunningAppState>>,
        &Rc<OffscreenRenderingContext>,
        &Rc<WindowRenderingContext>,
        &mut HashMap<NodeKey, Rc<OffscreenRenderingContext>>,
        &HashSet<WebViewId>,
        &mut HashMap<NodeKey, WebviewCreationBackpressureState>,
        &mut Vec<GraphIntent>,
    ),
    F2: FnMut(
        &mut Tree<TileKind>,
        &mut HashMap<NodeKey, Rc<OffscreenRenderingContext>>,
        &mut HashMap<NodeKey, (u64, egui::TextureHandle)>,
        &mut HashMap<WebViewId, (egui::TextureHandle, egui::load::SizedTexture)>,
    ),
{
    let KeyboardPhaseArgs {
        ctx,
        graph_app,
        graph_surface_focused,
        window,
        tiles_tree,
        tile_rendering_contexts,
        tile_favicon_textures,
        favicon_textures,
        app_state,
        rendering_context,
        window_rendering_context,
        responsive_webviews,
        webview_creation_backpressure,
        suppress_toggle_view,
    } = args;

    let mut keyboard_actions = input::collect_actions(ctx, graph_app);
    let preview_active = graph_app.history_health_summary().preview_mode_active;
    if preview_active {
        keyboard_actions.toggle_view = false;
        keyboard_actions.delete_selected = false;
        keyboard_actions.clear_graph = false;
    }
    if suppress_toggle_view {
        keyboard_actions.toggle_view = false;
    }
    if keyboard_actions.toggle_view {
        toggle_tile_view(
            tiles_tree,
            graph_app,
            window,
            app_state,
            rendering_context,
            window_rendering_context,
            tile_rendering_contexts,
            responsive_webviews,
            webview_creation_backpressure,
            frame_intents,
        );
        keyboard_actions.toggle_view = false;
    }
    if keyboard_actions.delete_selected {
        let nodes_to_close: Vec<_> = graph_app.focused_selection().iter().copied().collect();
        frame_intents.extend(webview_controller::close_webviews_for_nodes(
            graph_app,
            &nodes_to_close,
            window,
        ));
    }
    if keyboard_actions.clear_graph {
        frame_intents.extend(webview_controller::close_all_webviews(graph_app, window));
        reset_runtime_webview_state(
            tiles_tree,
            tile_rendering_contexts,
            tile_favicon_textures,
            favicon_textures,
        );
    }
    if keyboard_actions.open_tag_panel {
        crate::shell::desktop::ui::tag_panel::open_tag_panel_for_current_focus(
            graph_app,
            tiles_tree,
            graph_surface_focused,
            None,
        );
    }
    if keyboard_actions.toggle_semantic_tab_group
        && let Some(focused_pane) = window.focused_pane()
        && let Some(intent) =
            crate::shell::desktop::workbench::semantic_tabs::semantic_tab_toggle_intent_for_pane(
                tiles_tree,
                graph_app,
                focused_pane,
            )
    {
        graph_app.enqueue_workbench_intent(intent);
    }

    let command_bar_focus_target = nav_targeting::command_bar_focus_target(
        window.focused_pane(),
        nav_targeting::active_node_pane_node(tiles_tree),
        nav_targeting::chrome_projection_node(graph_app, window),
        graph_app.focused_selection().primary(),
    );
    if keyboard_actions.toggle_help_panel {
        let _ = toolbar_routing::request_help_panel_toggle(graph_app, command_bar_focus_target);
        keyboard_actions.toggle_help_panel = false;
    }
    if keyboard_actions.toggle_command_palette {
        let _ = toolbar_routing::request_command_palette_toggle(graph_app);
        keyboard_actions.toggle_command_palette = false;
    }
    if keyboard_actions.toggle_radial_menu {
        let _ = toolbar_routing::request_radial_menu_toggle(graph_app, command_bar_focus_target);
        keyboard_actions.toggle_radial_menu = false;
    }
    if keyboard_actions.toggle_workbench_overlay {
        let _ = toolbar_routing::request_workbench_overlay_toggle(graph_app, command_bar_focus_target);
        keyboard_actions.toggle_workbench_overlay = false;
    }
    if keyboard_actions.close_workbench_overlay {
        let _ = toolbar_routing::request_workbench_overlay_close(graph_app);
        keyboard_actions.close_workbench_overlay = false;
    }
    if keyboard_actions.cycle_focus_region {
        let _ = toolbar_routing::request_cycle_focus_region(graph_app, command_bar_focus_target);
        keyboard_actions.cycle_focus_region = false;
    }

    frame_intents.extend(input::intents_from_actions(&keyboard_actions));
    input::dispatch_runtime_requests_from_actions(&keyboard_actions);
    graph_app.extend_workbench_intents(input::workbench_intents_from_actions(&keyboard_actions));
}

