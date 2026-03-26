/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use super::*;
use crate::shell::desktop::ui::dialog::DialogCommand;
use crate::shell::desktop::ui::gui_state::PendingWebviewContextSurfaceRequest;
#[cfg(feature = "diagnostics")]
use crate::shell::desktop::ui::gui_state::RuntimeFocusInspector;
use crate::shell::desktop::ui::workbench_host::{WorkbenchChromeProjection, WorkbenchLayerState};

fn open_preferred_context_command_surface_for_webview_target(
    ctx: &egui::Context,
    graph_app: &mut GraphBrowserApp,
    webview_id: WebViewId,
    anchor: [f32; 2],
) -> bool {
    let Some(node_key) = graph_app.get_node_for_webview(webview_id) else {
        return false;
    };

    match graph_app.context_command_surface_preference() {
        crate::app::ContextCommandSurfacePreference::RadialPalette => {
            graph_app.set_pending_node_context_target(Some(node_key));
            if graph_app
                .pending_transient_surface_return_target()
                .is_none()
            {
                graph_app.set_pending_transient_surface_return_target(Some(
                    crate::app::ToolSurfaceReturnTarget::Node(node_key),
                ));
            }
            ctx.data_mut(|d| {
                d.insert_persisted(
                    egui::Id::new("radial_menu_center"),
                    egui::pos2(anchor[0], anchor[1]),
                );
            });
            graph_app.open_radial_menu();
        }
        crate::app::ContextCommandSurfacePreference::ContextPalette => {
            graph_app.set_pending_node_context_target(Some(node_key));
            if graph_app.pending_command_surface_return_target().is_none() {
                graph_app.set_pending_command_surface_return_target(Some(
                    crate::app::ToolSurfaceReturnTarget::Node(node_key),
                ));
            }
            graph_app.set_context_palette_anchor(Some(anchor));
            graph_app.open_context_palette();
        }
    }
    true
}

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
    pub(crate) pending_webview_context_surface_requests:
        &'a mut Vec<PendingWebviewContextSurfaceRequest>,
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
        pending_webview_context_surface_requests,
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

    let preview_mode_active = history_preview_mode_active(graph_app);

    *toolbar_height = Length::new(ctx.available_rect().min.y);
    if !preview_mode_active {
        graph_app.check_periodic_snapshot();
    }

    for PendingWebviewContextSurfaceRequest { webview_id, anchor } in
        pending_webview_context_surface_requests.drain(..)
    {
        let _ = open_preferred_context_command_surface_for_webview_target(
            ctx, graph_app, webview_id, anchor,
        );
    }

    let focused_dialog_webview = if graph_surface_focused {
        None
    } else {
        window.explicit_dialog_webview_id()
    };
    let dialog_commands = std::cell::RefCell::new(Vec::new());
    headed_window.for_each_active_dialog(
        window,
        focused_dialog_webview,
        *toolbar_height,
        |dialog| {
            let result = dialog.update(ctx);
            if let Some(command) = result.command {
                dialog_commands.borrow_mut().push(command);
            }
            result.keep_open
        },
    );
    for command in dialog_commands.into_inner() {
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

    let mut post_render_intents = Vec::new();
    let search_matches: HashSet<NodeKey> = graph_search_matches.iter().copied().collect();
    let active_search_match =
        active_graph_search_match(graph_search_matches, graph_search_active_match_index);
    let layer_state =
        WorkbenchChromeProjection::from_tree(graph_app, tiles_tree, window.focused_pane())
            .layer_state;

    if matches!(
        layer_state,
        WorkbenchLayerState::WorkbenchActive | WorkbenchLayerState::WorkbenchPinned
    ) {
        egui::SidePanel::right("workbench_area")
            .frame(egui::Frame::new().fill(egui::Color32::from_rgb(20, 20, 25)))
            .show(ctx, |ui| {
                post_render_intents.extend(tile_render_pass::run_tile_render_pass_in_ui(
                    ui,
                    TileRenderPassArgs {
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
                    },
                ));
            });
    }

    egui::CentralPanel::default()
        .frame(egui::Frame::new().fill(egui::Color32::from_rgb(20, 20, 25)))
        .show(ctx, |ui| {
            post_render_intents.extend(tile_render_pass::render_primary_graph_in_ui(
                ui,
                graph_app,
                tiles_tree,
                &search_matches,
                active_search_match,
                graph_search_filter_mode,
                search_query_active,
            ));
        });

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
    if let Some(state) = graph_app
        .workspace
        .graph_runtime
        .clip_inspector_state
        .as_ref()
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

#[cfg(test)]
mod tests {
    use super::open_preferred_context_command_surface_for_webview_target;
    use crate::app::{ContextCommandSurfacePreference, GraphBrowserApp, ToolSurfaceReturnTarget};
    use servo::WebViewId;

    fn test_webview_id() -> WebViewId {
        thread_local! {
            static NS_INSTALLED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
        }
        NS_INSTALLED.with(|cell| {
            if !cell.get() {
                base::id::PipelineNamespace::install(base::id::PipelineNamespaceId(91));
                cell.set(true);
            }
        });
        WebViewId::new(base::id::PainterId::next())
    }

    #[test]
    fn mapped_webview_target_opens_context_palette_when_preferred() {
        let mut app = GraphBrowserApp::new_for_testing();
        let node = app.add_node_and_sync(
            "https://context.example".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );
        let webview_id = test_webview_id();
        app.map_webview_to_node(webview_id, node);

        app.set_context_command_surface_preference(ContextCommandSurfacePreference::ContextPalette);
        let ctx = egui::Context::default();

        assert!(open_preferred_context_command_surface_for_webview_target(
            &ctx,
            &mut app,
            webview_id,
            [12.0, 24.0]
        ));
        assert_eq!(app.pending_node_context_target(), Some(node));
        assert_eq!(
            app.pending_command_surface_return_target(),
            Some(ToolSurfaceReturnTarget::Node(node))
        );
        assert_eq!(
            app.workspace.chrome_ui.context_palette_anchor,
            Some([12.0, 24.0])
        );
        assert!(app.workspace.chrome_ui.show_context_palette);
        assert!(!app.workspace.chrome_ui.show_command_palette);
        assert!(!app.workspace.chrome_ui.show_radial_menu);
    }

    #[test]
    fn mapped_webview_target_opens_radial_menu_when_preferred() {
        let mut app = GraphBrowserApp::new_for_testing();
        let node = app.add_node_and_sync(
            "https://context.example".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );
        let webview_id = test_webview_id();
        app.map_webview_to_node(webview_id, node);
        app.set_context_command_surface_preference(ContextCommandSurfacePreference::RadialPalette);
        let ctx = egui::Context::default();

        assert!(open_preferred_context_command_surface_for_webview_target(
            &ctx,
            &mut app,
            webview_id,
            [20.0, 36.0]
        ));
        assert_eq!(app.pending_node_context_target(), Some(node));
        assert_eq!(
            app.pending_transient_surface_return_target(),
            Some(ToolSurfaceReturnTarget::Node(node))
        );
        assert!(app.workspace.chrome_ui.show_radial_menu);
        assert!(!app.workspace.chrome_ui.show_context_palette);
        let center =
            ctx.data_mut(|d| d.get_persisted::<egui::Pos2>(egui::Id::new("radial_menu_center")));
        assert_eq!(center, Some(egui::pos2(20.0, 36.0)));
    }

    #[test]
    fn webview_context_command_surface_ignores_unmapped_webview() {
        let mut app = GraphBrowserApp::new_for_testing();
        let webview_id = test_webview_id();
        let ctx = egui::Context::default();

        assert!(!open_preferred_context_command_surface_for_webview_target(
            &ctx,
            &mut app,
            webview_id,
            [4.0, 8.0]
        ));
        assert_eq!(app.pending_node_context_target(), None);
        assert_eq!(app.pending_command_surface_return_target(), None);
        assert_eq!(app.pending_transient_surface_return_target(), None);
        assert!(!app.workspace.chrome_ui.show_context_palette);
        assert!(!app.workspace.chrome_ui.show_radial_menu);
    }
}
