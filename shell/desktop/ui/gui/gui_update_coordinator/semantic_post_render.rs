use super::*;

pub(super) fn finalize_update_frame(
    ctx: &egui::Context,
    _graph_app: &mut GraphBrowserApp,
    _clipboard: &mut Option<Clipboard>,
    toasts: &mut egui_notify::Toasts,
) {
    // Draining pending node-status notices and clipboard-copy requests
    // migrated onto `GraphshellRuntime::ingest_frame_input` through
    // `HostToastPort` + `HostClipboardPort`. The only thing left to do
    // on the host side is drive the actual render of the toast overlay.
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
        focus,
        pending_webview_context_surface_requests,
        graph_search,
        command_authority,
        toasts,
        registry_runtime: _,
        control_panel,
        #[cfg(feature = "diagnostics")]
        diagnostics_state,
        responsive_webviews,
        open_node_tile_after_intents,
        frame_intents,
    } = args;

    // This phase only reads 4 of the bundle's 5 refs; `open` flows
    // through to remain consistent with upstream phase-arg shapes but
    // is not consulted here.
    let GraphSearchAuthorityMut {
        open: _,
        query: graph_search_query,
        filter_mode: graph_search_filter_mode,
        matches: graph_search_matches,
        active_match_index: graph_search_active_match_index,
    } = graph_search;

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

    // M4.1 slice 1c: the focus mutation bundle (`FocusAuthorityMut`) is
    // assembled upstream in `execute_update_frame` and flows through
    // `SemanticAndPostRenderPhaseArgs::focus`. The post-render / tile-
    // render path consumes it directly; no local reassembly needed.
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
            focus,
            command_authority,
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
