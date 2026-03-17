/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use super::*;
use crate::shell::desktop::ui::dialog::DialogCommand;
#[cfg(feature = "diagnostics")]
use crate::shell::desktop::ui::gui_state::RuntimeFocusInspector;

pub(crate) struct PostRenderPhaseArgs<'a> {
    pub(crate) ctx: &'a egui::Context,
    pub(crate) graph_app: &'a mut GraphBrowserApp,
    pub(crate) window: &'a EmbedderWindow,
    pub(crate) headed_window: &'a HeadedWindow,
    pub(crate) tiles_tree: &'a mut Tree<TileKind>,
    pub(crate) tile_rendering_contexts: &'a mut HashMap<NodeKey, Rc<OffscreenRenderingContext>>,
    pub(crate) tile_favicon_textures: &'a mut HashMap<NodeKey, (u64, egui::TextureHandle)>,
    pub(crate) favicon_textures:
        &'a mut HashMap<WebViewId, (egui::TextureHandle, egui::load::SizedTexture)>,
    pub(crate) toolbar_height: &'a mut Length<f32, DeviceIndependentPixel>,
    pub(crate) graph_search_matches: &'a [NodeKey],
    pub(crate) graph_search_active_match_index: Option<usize>,
    pub(crate) graph_search_filter_mode: bool,
    pub(crate) search_query_active: bool,
    pub(crate) app_state: &'a Option<Rc<RunningAppState>>,
    pub(crate) rendering_context: &'a Rc<OffscreenRenderingContext>,
    pub(crate) window_rendering_context: &'a Rc<WindowRenderingContext>,
    pub(crate) responsive_webviews: &'a HashSet<WebViewId>,
    pub(crate) webview_creation_backpressure:
        &'a mut HashMap<NodeKey, WebviewCreationBackpressureState>,
    pub(crate) focused_node_hint: &'a mut Option<NodeKey>,
    pub(crate) graph_surface_focused: bool,
    pub(crate) focus_ring_node_key: &'a mut Option<NodeKey>,
    pub(crate) focus_ring_started_at: &'a mut Option<Instant>,
    pub(crate) focus_ring_duration: Duration,
    pub(crate) toasts: &'a mut egui_notify::Toasts,
    pub(crate) control_panel: &'a mut crate::shell::desktop::runtime::control_panel::ControlPanel,
    #[cfg(feature = "diagnostics")]
    pub(crate) diagnostics_state: &'a mut diagnostics::DiagnosticsState,
    #[cfg(feature = "diagnostics")]
    pub(crate) runtime_focus_inspector: Option<RuntimeFocusInspector>,
}

pub(crate) fn run_post_render_phase<FActive>(
    args: PostRenderPhaseArgs<'_>,
    active_graph_search_match: FActive,
) where
    FActive: Fn(&[NodeKey], Option<usize>) -> Option<NodeKey>,
{
    let PostRenderPhaseArgs {
        ctx,
        graph_app,
        window,
        headed_window,
        tiles_tree,
        tile_rendering_contexts,
        tile_favicon_textures,
        favicon_textures,
        toolbar_height,
        graph_search_matches,
        graph_search_active_match_index,
        graph_search_filter_mode,
        search_query_active,
        app_state,
        rendering_context,
        window_rendering_context,
        responsive_webviews,
        webview_creation_backpressure,
        focused_node_hint,
        graph_surface_focused,
        focus_ring_node_key,
        focus_ring_started_at,
        focus_ring_duration,
        toasts,
        control_panel,
        #[cfg(feature = "diagnostics")]
        diagnostics_state,
        #[cfg(feature = "diagnostics")]
        runtime_focus_inspector,
    } = args;

    #[cfg(debug_assertions)]
    {
        for violation in tile_invariants::collect_tile_invariant_violations(
            tiles_tree,
            graph_app,
            tile_rendering_contexts,
        ) {
            warn!("{violation}");
        }
    }

    let has_node_panes = tile_runtime::has_any_node_panes(tiles_tree);
    let is_graph_view = !has_node_panes;
    let preview_mode_active = history_preview_mode_active(graph_app);

    *toolbar_height = Length::new(ctx.available_rect().min.y);
    if !preview_mode_active {
        graph_app.check_periodic_snapshot();
    }

    let focused_dialog_webview = if graph_surface_focused {
        None
    } else {
        window.explicit_dialog_webview_id()
    };
    headed_window.for_each_active_dialog(
        window,
        focused_dialog_webview,
        *toolbar_height,
        |dialog| {
            let result = dialog.update(ctx);
            if let Some(command) = result.command {
                match command {
                    DialogCommand::ClipElement {
                        webview_id,
                        element_rect,
                    } => headed_window.request_clip_element(window, webview_id, element_rect),
                    DialogCommand::InspectPageElements { webview_id } => {
                        headed_window.request_page_inspector_candidates(window, webview_id)
                    }
                }
            }
            result.keep_open
        },
    );

    let mut post_render_intents = Vec::new();
    if is_graph_view || has_node_panes {
        let search_matches: HashSet<NodeKey> = graph_search_matches.iter().copied().collect();
        let active_search_match =
            active_graph_search_match(graph_search_matches, graph_search_active_match_index);
        post_render_intents.extend(tile_render_pass::run_tile_render_pass(TileRenderPassArgs {
            ctx,
            graph_app,
            window,
            tiles_tree,
            tile_rendering_contexts,
            tile_favicon_textures,
            graph_search_matches: &search_matches,
            active_search_match,
            graph_search_filter_mode,
            search_query_active,
            app_state,
            rendering_context,
            window_rendering_context,
            responsive_webviews,
            webview_creation_backpressure,
            focused_node_hint,
            graph_surface_focused,
            suppress_runtime_side_effects: preview_mode_active,
            focus_ring_node_key,
            focus_ring_started_at,
            focus_ring_duration,
            control_panel,
            #[cfg(feature = "diagnostics")]
            diagnostics_state,
            #[cfg(feature = "diagnostics")]
            runtime_focus_inspector,
        }));
    }
    apply_intents_if_any(graph_app, tiles_tree, &mut post_render_intents);

    render::render_help_panel(ctx, graph_app);
    render::render_settings_overlay_panel(ctx, graph_app, Some(control_panel));
    render::render_clip_inspector_panel(ctx, graph_app);
    if let Some(webview_id) = graph_app
        .workspace
        .graph_runtime
        .pending_clip_inspector_highlight_clear
        .take()
    {
        headed_window.sync_clip_inspector_highlight(window, webview_id, None);
    }
    if let Some(state) = graph_app.workspace.graph_runtime.clip_inspector_state.as_ref()
        && state.highlight_dirty
    {
        headed_window.sync_clip_inspector_highlight(
            window,
            state.webview_id,
            state
                .pointer_stack
                .get(state.pointer_stack_index)
                .and_then(|capture| capture.dom_path.as_deref()),
        );
        graph_app.clear_clip_inspector_highlight_dirty();
    }
    let active_node_pane =
        crate::shell::desktop::workbench::tile_compositor::active_node_pane(tiles_tree);
    let focused_pane_node = nav_targeting::chrome_projection_node(graph_app, window)
        .or_else(|| {
            focused_dialog_webview.and_then(|webview_id| graph_app.get_node_for_webview(webview_id))
        })
        .or_else(|| active_node_pane.map(|pane| pane.node_key));
    render::render_command_palette_panel(
        ctx,
        graph_app,
        graph_app.workspace.graph_runtime.hovered_graph_node,
        focused_pane_node,
        active_node_pane.map(|pane| pane.pane_id),
    );
    render::render_radial_command_menu(
        ctx,
        graph_app,
        graph_app.workspace.graph_runtime.hovered_graph_node,
        focused_pane_node,
        active_node_pane.map(|pane| pane.pane_id),
    );
    if !preview_mode_active && let Some(target_dir) = graph_app.take_pending_switch_data_dir() {
        match persistence_ops::switch_persistence_store(
            graph_app,
            window,
            tiles_tree,
            tile_rendering_contexts,
            tile_favicon_textures,
            favicon_textures,
            &mut post_render_intents,
            target_dir.clone(),
        ) {
            Ok(()) => toasts.success(format!(
                "Switched graph data directory to {}",
                target_dir.display()
            )),
            Err(e) => toasts.error(format!("Failed to switch data directory: {e}")),
        };
    }
    apply_intents_if_any(graph_app, tiles_tree, &mut post_render_intents);

    let open_settings_tool_pane = render::render_choose_frame_picker(ctx, graph_app)
        || render::render_unsaved_frame_prompt(ctx, graph_app);
    if open_settings_tool_pane {
        tile_view_ops::open_or_focus_tool_pane(tiles_tree, ToolPaneState::Settings);
    }

    if !preview_mode_active {
        pending_actions::run_post_render_pending_actions(
            graph_app,
            window,
            tiles_tree,
            tile_rendering_contexts,
            tile_favicon_textures,
            webview_creation_backpressure,
            focused_node_hint,
        );
    }

    while let Some((key, url)) = graph_app.take_pending_protocol_probe() {
        control_panel.handle_protocol_probe_request(key, url);
    }
}
