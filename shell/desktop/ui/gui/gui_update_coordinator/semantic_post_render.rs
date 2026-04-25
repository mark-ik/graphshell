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
        root_ui,
        ui_render_backend,
        window,
        headed_window,
        tiles_tree,
        modal_surface_active,
        toolbar_height,
        tile_favicon_textures,
        favicon_textures,
        app_state,
        rendering_context,
        window_rendering_context,
        toasts,
        responsive_webviews,
        open_node_tile_after_intents,
        frame_intents,
        runtime,
        cached_view_model,
    } = args;

    // Lane B' (2026-04-23): SemanticLifecycle takes runtime directly;
    // no split-borrows in scope during its call. Split-borrows are
    // constructed AFTER SemanticLifecycle returns, scoped to the
    // PostRender + intermediate-computation block.
    run_semantic_lifecycle_phase(SemanticLifecyclePhaseArgs {
        tiles_tree,
        modal_surface_active,
        window,
        app_state,
        rendering_context,
        window_rendering_context,
        tile_favicon_textures,
        favicon_textures,
        responsive_webviews,
        open_node_tile_after_intents,
        frame_intents,
        runtime: &mut *runtime,
    });

    // Lane B' (2026-04-23): PostRender now takes runtime directly. The
    // intermediate `phase3_reconcile_semantics` and `runtime_focus_inspector`
    // computations split-borrow from runtime in scoped blocks before the
    // PostRender call so the borrows don't conflict with the single
    // `&mut *runtime` reborrow PostRender consumes.
    let search_query_active = is_graph_search_query_active(&runtime.graph_search_query);
    let graph_search_active_match_index = runtime.graph_search_active_match_index;
    let graph_search_filter_mode = runtime.graph_search_filter_mode;
    crate::shell::desktop::runtime::registries::phase3_reconcile_semantics(&mut runtime.graph_app);
    #[cfg(feature = "diagnostics")]
    let runtime_focus_inspector = {
        let inspector = crate::shell::desktop::ui::gui::runtime_focus_inspector(
            &mut runtime.graph_app,
            &mut runtime.focus_authority,
            None,
            false,
        );
        Some(inspector)
    };

    // Snapshot graph_search_matches so we can release the borrow before
    // PostRender takes runtime.
    let graph_search_matches_snapshot: Vec<NodeKey> = runtime.graph_search_matches.clone();

    gui_frame::run_post_render_phase(
        gui_frame::PostRenderPhaseArgs {
            ctx,
            root_ui,
            ui_render_backend,
            window,
            headed_window,
            tiles_tree,
            tile_favicon_textures,
            favicon_textures,
            toolbar_height,
            graph_search_matches: &graph_search_matches_snapshot,
            graph_search_active_match_index,
            graph_search_filter_mode,
            search_query_active,
            app_state,
            rendering_context,
            window_rendering_context,
            responsive_webviews,
            toasts,
            #[cfg(feature = "diagnostics")]
            runtime_focus_inspector,
            runtime: &mut *runtime,
            cached_view_model,
        },
        |matches, active_index| gui_orchestration::active_graph_search_match(matches, active_index),
    );
}

pub(super) fn is_graph_search_query_active(query: &str) -> bool {
    !query.trim().is_empty()
}

pub(super) fn run_semantic_lifecycle_phase(args: SemanticLifecyclePhaseArgs<'_>) {
    let SemanticLifecyclePhaseArgs {
        tiles_tree,
        modal_surface_active,
        window,
        app_state,
        rendering_context,
        window_rendering_context,
        tile_favicon_textures,
        favicon_textures,
        responsive_webviews,
        open_node_tile_after_intents,
        frame_intents,
        runtime,
    } = args;

    gui_orchestration::run_semantic_lifecycle_phase(
        &mut runtime.graph_app,
        tiles_tree,
        &mut runtime.graph_tree,
        modal_surface_active,
        &mut runtime.focus_authority,
        window,
        app_state,
        rendering_context,
        window_rendering_context,
        &mut runtime.viewer_surfaces,
        runtime.viewer_surface_host.as_mut(),
        tile_favicon_textures,
        favicon_textures,
        responsive_webviews,
        &mut runtime.webview_creation_backpressure,
        &runtime.command_surface_telemetry,
        open_node_tile_after_intents,
        frame_intents,
    );
}
