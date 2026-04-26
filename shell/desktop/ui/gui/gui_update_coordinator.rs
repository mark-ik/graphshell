/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use super::update_frame_phases::{
    ExecuteUpdateFrameArgs, GraphSearchAndKeyboardPhaseArgs, PreFrameAndIntentInitArgs,
    SemanticAndPostRenderPhaseArgs, SemanticLifecyclePhaseArgs,
    ToolbarAndGraphSearchWindowPhaseArgs, UPDATE_FRAME_STAGE_SEQUENCE, UpdateFrameStage,
};
use super::*;

#[path = "gui_update_coordinator/frame_prelude.rs"]
mod frame_prelude;
#[path = "gui_update_coordinator/semantic_post_render.rs"]
mod semantic_post_render;

impl EguiHost {
    pub(super) fn execute_update_frame(args: ExecuteUpdateFrameArgs<'_>) {
        debug_assert!(Self::is_canonical_update_frame_stage_sequence(
            &UPDATE_FRAME_STAGE_SEQUENCE
        ));
        let ExecuteUpdateFrameArgs {
            ctx,
            root_ui,
            ui_render_backend,
            winit_window,
            state,
            window,
            headed_window,
            pending_webview_a11y_updates,
            tiles_tree,
            toolbar_height,
            toasts,
            clipboard,
            favicon_textures,
            tile_favicon_textures,
            thumbnail_channel,
            app_state,
            rendering_context,
            window_rendering_context,
            runtime,
            cached_view_model,
        } = args;
        // Lane B' (2026-04-23): split-borrow runtime fields per call site
        // instead of destructuring up-front. Calls that consume `runtime`
        // as a single ref (PreFrame here; more sub-phases to follow) take
        // it directly; calls that still consume individual fields receive
        // split-borrows from `runtime` after PreFrame returns and the
        // single-ref borrow ends.

        Self::run_update_frame_prelude(ctx, runtime, pending_webview_a11y_updates, tiles_tree);
        // User-gesture notification and idle-watchdog tick migrated onto
        // `GraphshellRuntime::ingest_frame_input`: both consume runtime
        // state (`control_panel`, `registry_runtime`) and the gesture flag
        // now flows through `FrameHostInput::had_input_events`.
        let (pre_frame, mut frame_intents) =
            Self::run_pre_frame_and_initialize_intents(PreFrameAndIntentInitArgs {
                ctx,
                state,
                window,
                favicon_textures,
                thumbnail_channel,
                runtime: &mut *runtime,
            });

        let mut open_node_tile_after_intents: Option<TileOpenMode> = None;

        let mut graph_search_output =
            Self::run_graph_search_and_keyboard_phases(GraphSearchAndKeyboardPhaseArgs {
                ctx,
                toasts,
                window,
                tiles_tree,
                tile_favicon_textures,
                favicon_textures,
                app_state,
                rendering_context,
                window_rendering_context,
                responsive_webviews: &pre_frame.responsive_webviews,
                frame_intents: &mut frame_intents,
                runtime: &mut *runtime,
            });

        Self::run_toolbar_and_graph_search_window_phases(ToolbarAndGraphSearchWindowPhaseArgs {
            ctx,
            root_ui: &mut *root_ui,
            winit_window,
            state,
            window,
            tiles_tree,
            toasts,
            tile_favicon_textures,
            favicon_textures,
            app_state,
            rendering_context,
            window_rendering_context,
            responsive_webviews: &pre_frame.responsive_webviews,
            graph_search_output: &mut graph_search_output,
            frame_intents: &mut frame_intents,
            open_node_tile_after_intents: &mut open_node_tile_after_intents,
            runtime: &mut *runtime,
        });

        // Lane B' (2026-04-23): the runtime destructure that previously
        // sat here is gone — every remaining sub-phase call takes
        // `runtime: &mut GraphshellRuntime` directly. The modal-surface
        // computation that needed `graph_app` / `focus_authority` /
        // `toolbar_state` simultaneously now split-borrows from runtime
        // in scoped blocks; the local-widget-focus clone lives outside
        // any borrow so the mutable borrows it needs are unambiguous.
        let local_widget_focus_for_modal = runtime.focus_authority.local_widget_focus.clone();
        let show_clear_data_confirm = runtime.toolbar_state.show_clear_data_confirm;
        let modal_surface_active = super::focus_state::workspace_runtime_focus_state(
            &mut runtime.graph_app,
            Some(&mut runtime.focus_authority),
            local_widget_focus_for_modal,
            show_clear_data_confirm,
        )
        .overlay_active();

        Self::run_semantic_and_post_render_phases(SemanticAndPostRenderPhaseArgs {
            ctx,
            root_ui: &mut *root_ui,
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
            responsive_webviews: &pre_frame.responsive_webviews,
            open_node_tile_after_intents: &mut open_node_tile_after_intents,
            frame_intents: &mut frame_intents,
            runtime: &mut *runtime,
            cached_view_model,
        });
        Self::finalize_update_frame(ctx, &mut runtime.graph_app, clipboard, toasts);
    }

    pub(super) fn is_canonical_update_frame_stage_sequence(sequence: &[UpdateFrameStage]) -> bool {
        sequence == UPDATE_FRAME_STAGE_SEQUENCE
    }

    #[cfg(test)]
    pub(super) fn update_frame_stage_sequence() -> &'static [UpdateFrameStage] {
        &UPDATE_FRAME_STAGE_SEQUENCE
    }

    fn run_update_frame_prelude(
        ctx: &egui::Context,
        runtime: &mut crate::shell::desktop::ui::gui_state::GraphshellRuntime,
        pending_webview_a11y_updates: &mut HashMap<WebViewId, accesskit::TreeUpdate>,
        tiles_tree: &mut Tree<TileKind>,
    ) {
        frame_prelude::run_update_frame_prelude(
            ctx,
            runtime,
            pending_webview_a11y_updates,
            tiles_tree,
        );
    }

    pub(super) fn configure_frame_toasts(
        toasts: &mut egui_notify::Toasts,
        preference: ToastAnchorPreference,
    ) {
        frame_prelude::configure_frame_toasts(toasts, preference, Self::toast_anchor);
    }

    fn run_pre_frame_and_initialize_intents(
        args: PreFrameAndIntentInitArgs<'_>,
    ) -> (gui_orchestration::PreFramePhaseOutput, Vec<GraphIntent>) {
        frame_prelude::run_pre_frame_and_initialize_intents(args)
    }

    fn run_graph_search_and_keyboard_phases(
        args: GraphSearchAndKeyboardPhaseArgs<'_>,
    ) -> graph_search_flow::GraphSearchFlowOutput {
        let GraphSearchAndKeyboardPhaseArgs {
            ctx,
            toasts,
            window,
            tiles_tree,
            tile_favicon_textures,
            favicon_textures,
            app_state,
            rendering_context,
            window_rendering_context,
            responsive_webviews,
            frame_intents,
            runtime,
        } = args;

        // Lane B' (2026-04-23): destructure runtime internally so the
        // phase consumes the same shape `gui_orchestration::run_graph_search_phase`
        // and `run_keyboard_phase` expect (individual `&mut` refs).
        let graph_app: &mut GraphBrowserApp = &mut runtime.graph_app;
        let graph_search_open = &mut runtime.graph_search_open;
        let graph_search_query = &mut runtime.graph_search_query;
        let graph_search_filter_mode = &mut runtime.graph_search_filter_mode;
        let graph_search_matches = &mut runtime.graph_search_matches;
        let graph_search_active_match_index = &mut runtime.graph_search_active_match_index;
        let focus_authority = &mut runtime.focus_authority;
        let toolbar_state = &mut runtime.toolbar_state;
        let viewer_surfaces = &mut runtime.viewer_surfaces;
        let viewer_surface_host = runtime.viewer_surface_host.as_mut();
        let webview_creation_backpressure = &mut runtime.webview_creation_backpressure;
        let graph_surface_focused = runtime.graph_surface_focused;

        let graph_search_output = gui_orchestration::run_graph_search_phase(
            ctx,
            graph_app,
            toasts,
            graph_search_open,
            &mut focus_authority.local_widget_focus,
            graph_search_query,
            graph_search_filter_mode,
            graph_search_matches,
            graph_search_active_match_index,
            toolbar_state,
            frame_intents,
            pane_queries::tree_has_active_node_pane(graph_app),
        );

        gui_orchestration::run_keyboard_phase(
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
            graph_search_output.suppress_toggle_view,
            frame_intents,
        );

        graph_search_output
    }

    fn run_toolbar_and_graph_search_window_phases(args: ToolbarAndGraphSearchWindowPhaseArgs<'_>) {
        let ToolbarAndGraphSearchWindowPhaseArgs {
            ctx,
            root_ui,
            winit_window,
            state,
            window,
            tiles_tree,
            toasts,
            tile_favicon_textures,
            favicon_textures,
            app_state,
            rendering_context,
            window_rendering_context,
            responsive_webviews,
            graph_search_output,
            frame_intents,
            open_node_tile_after_intents,
            runtime,
        } = args;

        // Lane B' (2026-04-23): split-borrow the runtime fields the
        // sub-phases below consume. Same shape as the previous individual
        // arg destructure; one less hop now that the runtime ref lives on
        // the args struct.
        let graph_app: &mut GraphBrowserApp = &mut runtime.graph_app;
        let control_panel = &mut runtime.control_panel;
        let graph_tree = &mut runtime.graph_tree;
        let graph_surface_focused = runtime.graph_surface_focused;
        let focus_authority = &mut runtime.focus_authority;
        let toolbar_state = &mut runtime.toolbar_state;
        let clear_data_confirm_deadline_secs = &mut runtime.clear_data_confirm_deadline_secs;
        let omnibar_search_session = &mut runtime.omnibar_search_session;
        let omnibar_provider_suggestion_driver = &mut runtime.omnibar_provider_suggestion_driver;
        let command_surface_telemetry = &runtime.command_surface_telemetry;
        let viewer_surfaces = &mut runtime.viewer_surfaces;
        let viewer_surface_host = runtime.viewer_surface_host.as_mut();
        let webview_creation_backpressure = &mut runtime.webview_creation_backpressure;
        let graph_search_open = &mut runtime.graph_search_open;
        let graph_search_query = &mut runtime.graph_search_query;
        let graph_search_filter_mode = &mut runtime.graph_search_filter_mode;
        let graph_search_matches = &mut runtime.graph_search_matches;
        let graph_search_active_match_index = &mut runtime.graph_search_active_match_index;
        #[cfg(feature = "diagnostics")]
        let diagnostics_state = &mut runtime.diagnostics_state;

        let mut local_widget_focus = focus_authority.local_widget_focus.clone();

        let (toolbar_visible, is_graph_view) = gui_orchestration::run_toolbar_phase(
            ctx,
            root_ui,
            winit_window,
            state,
            graph_app,
            control_panel,
            #[cfg(feature = "diagnostics")]
            diagnostics_state,
            window,
            tiles_tree,
            graph_tree,
            graph_surface_focused,
            focus_authority,
            &mut local_widget_focus,
            toolbar_state,
            clear_data_confirm_deadline_secs,
            graph_search_output.focus_location_field_for_search,
            omnibar_search_session,
            omnibar_provider_suggestion_driver,
            command_surface_telemetry,
            toasts,
            viewer_surfaces,
            tile_favicon_textures,
            favicon_textures,
            app_state,
            rendering_context,
            window_rendering_context,
            responsive_webviews,
            webview_creation_backpressure,
            viewer_surface_host,
            frame_intents,
            open_node_tile_after_intents,
        );

        gui_orchestration::run_graph_search_window_phase(
            ctx,
            graph_app,
            toolbar_visible,
            *graph_search_open,
            is_graph_view,
            &mut local_widget_focus,
            graph_search_query,
            graph_search_filter_mode,
            graph_search_matches,
            graph_search_active_match_index,
            graph_search_output,
        );

        focus_authority.local_widget_focus = local_widget_focus;
    }

    fn finalize_update_frame(
        ctx: &egui::Context,
        graph_app: &mut GraphBrowserApp,
        clipboard: &mut Option<Clipboard>,
        toasts: &mut egui_notify::Toasts,
    ) {
        semantic_post_render::finalize_update_frame(ctx, graph_app, clipboard, toasts);
    }

    #[cfg(feature = "diagnostics")]
    fn maybe_toggle_diagnostics_tool_pane(ctx: &egui::Context, tiles_tree: &mut Tree<TileKind>) {
        frame_prelude::maybe_toggle_diagnostics_tool_pane(ctx, tiles_tree);
    }

    #[cfg(not(feature = "diagnostics"))]
    fn maybe_toggle_diagnostics_tool_pane(_ctx: &egui::Context, _tiles_tree: &mut Tree<TileKind>) {}

    fn run_semantic_and_post_render_phases(args: SemanticAndPostRenderPhaseArgs<'_>) {
        semantic_post_render::run_semantic_and_post_render_phases(args);
    }

    fn is_graph_search_query_active(query: &str) -> bool {
        semantic_post_render::is_graph_search_query_active(query)
    }

    fn run_semantic_lifecycle_phase(args: SemanticLifecyclePhaseArgs<'_>) {
        semantic_post_render::run_semantic_lifecycle_phase(args);
    }
}
