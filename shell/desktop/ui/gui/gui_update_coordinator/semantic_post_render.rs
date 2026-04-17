use super::*;

pub(super) fn finalize_update_frame(
    ctx: &egui::Context,
    graph_app: &mut GraphBrowserApp,
    clipboard: &mut Option<Clipboard>,
    toasts: &mut egui_notify::Toasts,
) {
    gui_orchestration::handle_pending_node_status_notices(graph_app, toasts);
    gui_orchestration::handle_pending_clipboard_copy_requests(graph_app, clipboard, toasts);
    toasts.show(ctx);
}

pub(super) fn run_semantic_and_post_render_phases(args: SemanticAndPostRenderPhaseArgs<'_>) {
    let SemanticAndPostRenderPhaseArgs {
        ctx,
        ui_render_backend,
        graph_app,
        bookmark_import_dialog,
        window,
        headed_window,
        tiles_tree,
        graph_tree,
        modal_surface_active,
        toolbar_height,
        viewer_surfaces,
        tile_favicon_textures,
        favicon_textures,
        app_state,
        rendering_context,
        window_rendering_context,
        webview_creation_backpressure,
        focus_authority,
        focused_node_hint,
        graph_surface_focused,
        focus_ring_node_key,
        focus_ring_started_at,
        focus_ring_duration,
        pending_webview_context_surface_requests,
        graph_search_query,
        graph_search_matches,
        graph_search_active_match_index,
        graph_search_filter_mode,
        toasts,
        registry_runtime: _,
        control_panel,
        #[cfg(feature = "diagnostics")]
        diagnostics_state,
        responsive_webviews,
        open_node_tile_after_intents,
        frame_intents,
    } = args;

    run_semantic_lifecycle_phase(SemanticLifecyclePhaseArgs {
        graph_app,
        tiles_tree,
        graph_tree,
        modal_surface_active,
        focus_authority,
        window,
        app_state,
        rendering_context,
        window_rendering_context,
        viewer_surfaces,
        tile_favicon_textures,
        favicon_textures,
        responsive_webviews,
        webview_creation_backpressure,
        open_node_tile_after_intents,
        frame_intents,
    });

    crate::shell::desktop::runtime::registries::phase3_reconcile_semantics(graph_app);
    let search_query_active = is_graph_search_query_active(graph_search_query);
    #[cfg(feature = "diagnostics")]
    let runtime_focus_inspector = Some(crate::shell::desktop::ui::gui::runtime_focus_inspector(
        graph_app,
        focus_authority,
        None,
        false,
    ));

    gui_frame::run_post_render_phase(
        gui_frame::PostRenderPhaseArgs {
            ctx,
            ui_render_backend,
            graph_app,
            bookmark_import_dialog,
            window,
            headed_window,
            tiles_tree,
            graph_tree,
            viewer_surfaces,
            tile_favicon_textures,
            favicon_textures,
            toolbar_height,
            graph_search_matches,
            graph_search_active_match_index: *graph_search_active_match_index,
            graph_search_filter_mode: *graph_search_filter_mode,
            search_query_active,
            app_state,
            rendering_context,
            window_rendering_context,
            responsive_webviews,
            webview_creation_backpressure,
            focused_node_hint,
            graph_surface_focused: *graph_surface_focused,
            focus_ring_node_key,
            focus_ring_started_at,
            focus_ring_duration: *focus_ring_duration,
            pending_webview_context_surface_requests,
            toasts,
            control_panel,
            #[cfg(feature = "diagnostics")]
            diagnostics_state,
            #[cfg(feature = "diagnostics")]
            runtime_focus_inspector,
        },
        |matches, active_index| gui_orchestration::active_graph_search_match(matches, active_index),
    );
}

pub(super) fn is_graph_search_query_active(query: &str) -> bool {
    !query.trim().is_empty()
}

pub(super) fn run_semantic_lifecycle_phase(args: SemanticLifecyclePhaseArgs<'_>) {
    let SemanticLifecyclePhaseArgs {
        graph_app,
        tiles_tree,
        graph_tree,
        modal_surface_active,
        focus_authority,
        window,
        app_state,
        rendering_context,
        window_rendering_context,
        viewer_surfaces,
        tile_favicon_textures,
        favicon_textures,
        responsive_webviews,
        webview_creation_backpressure,
        open_node_tile_after_intents,
        frame_intents,
    } = args;

    gui_orchestration::run_semantic_lifecycle_phase(
        graph_app,
        tiles_tree,
        graph_tree,
        modal_surface_active,
        focus_authority,
        window,
        app_state,
        rendering_context,
        window_rendering_context,
        viewer_surfaces,
        tile_favicon_textures,
        favicon_textures,
        responsive_webviews,
        webview_creation_backpressure,
        open_node_tile_after_intents,
        frame_intents,
    );
}
