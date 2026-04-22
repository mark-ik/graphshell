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
            ui_render_backend,
            winit_window,
            state,
            window,
            headed_window,
            graph_app,
            pending_webview_a11y_updates,
            tiles_tree,
            graph_tree,
            toolbar_height,
            toolbar_state,
            clear_data_confirm_deadline_secs,
            toasts,
            clipboard,
            favicon_textures,
            viewer_surfaces,
            tile_favicon_textures,
            thumbnail_channel,
            thumbnail_capture_in_flight,
            webview_creation_backpressure,
            app_state,
            mut graph_search,
            mut command_authority,
            focus_authority,
            focused_node_hint,
            graph_surface_focused,
            focus_ring_node_key,
            focus_ring_started_at,
            focus_ring_duration,
            omnibar_search_session,
            omnibar_provider_suggestion_driver,
            command_surface_telemetry,
            pending_webview_context_surface_requests,
            bookmark_import_dialog,
            rendering_context,
            window_rendering_context,
            registry_runtime,
            control_panel,
            #[cfg(feature = "diagnostics")]
            diagnostics_state,
        } = args;

        Self::run_update_frame_prelude(ctx, graph_app, pending_webview_a11y_updates, tiles_tree);
        // User-gesture notification and idle-watchdog tick migrated onto
        // `GraphshellRuntime::ingest_frame_input`: both consume runtime
        // state (`control_panel`, `registry_runtime`) and the gesture flag
        // now flows through `FrameHostInput::had_input_events`.
        let (pre_frame, mut frame_intents) =
            Self::run_pre_frame_and_initialize_intents(PreFrameAndIntentInitArgs {
                ctx,
                graph_app,
                state,
                window,
                favicon_textures,
                thumbnail_channel,
                thumbnail_capture_in_flight,
                command_authority: command_authority.reborrow(),
                control_panel,
            });

        let mut open_node_tile_after_intents: Option<TileOpenMode> = None;

        let mut graph_search_output =
            Self::run_graph_search_and_keyboard_phases(GraphSearchAndKeyboardPhaseArgs {
                ctx,
                graph_app,
                toasts,
                window,
                tiles_tree,
                graph_surface_focused,
                graph_search: graph_search.reborrow(),
                focus_authority,
                toolbar_state,
                viewer_surfaces,
                tile_favicon_textures,
                favicon_textures,
                app_state,
                rendering_context,
                window_rendering_context,
                responsive_webviews: &pre_frame.responsive_webviews,
                webview_creation_backpressure,
                frame_intents: &mut frame_intents,
            });

        Self::run_toolbar_and_graph_search_window_phases(ToolbarAndGraphSearchWindowPhaseArgs {
            ctx,
            winit_window,
            state,
            graph_app,
            control_panel,
            #[cfg(feature = "diagnostics")]
            diagnostics_state,
            window,
            tiles_tree,
            graph_tree,
            focused_node_hint: *focused_node_hint,
            graph_surface_focused,
            focus_authority,
            toolbar_state,
            clear_data_confirm_deadline_secs,
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
            responsive_webviews: &pre_frame.responsive_webviews,
            webview_creation_backpressure,
            graph_search: graph_search.reborrow(),
            graph_search_output: &mut graph_search_output,
            frame_intents: &mut frame_intents,
            open_node_tile_after_intents: &mut open_node_tile_after_intents,
        });

        let modal_surface_active = super::focus_state::workspace_runtime_focus_state(
            graph_app,
            Some(focus_authority),
            focus_authority.local_widget_focus.clone(),
            toolbar_state.show_clear_data_confirm,
        )
        .overlay_active();

        // M4.1 slice 1c: assemble the host-facing focus mutation bundle
        // after phases 1–3 have settled `graph_surface_focused`. The
        // downstream render/post-render path takes ownership of this
        // handle and calls named methods (`clear_hint`, `set_hint`,
        // `latch_ring`, …) instead of touching individual runtime
        // fields. Upstream phases still consume individual refs because
        // each phase only touches a subset.
        let focus = crate::shell::desktop::ui::gui_state::FocusAuthorityMut {
            focused_node_hint,
            graph_surface_focused,
            focus_ring_node_key,
            focus_ring_started_at,
            focus_ring_duration,
        };
        Self::run_semantic_and_post_render_phases(SemanticAndPostRenderPhaseArgs {
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
            graph_search: graph_search.reborrow(),
            command_authority: command_authority.reborrow(),
            toasts,
            registry_runtime,
            control_panel,
            #[cfg(feature = "diagnostics")]
            diagnostics_state,
            responsive_webviews: &pre_frame.responsive_webviews,
            command_surface_telemetry,
            open_node_tile_after_intents: &mut open_node_tile_after_intents,
            frame_intents: &mut frame_intents,
        });
        Self::finalize_update_frame(ctx, graph_app, clipboard, toasts);
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
        graph_app: &mut GraphBrowserApp,
        pending_webview_a11y_updates: &mut HashMap<WebViewId, accesskit::TreeUpdate>,
        tiles_tree: &mut Tree<TileKind>,
    ) {
        frame_prelude::run_update_frame_prelude(
            ctx,
            graph_app,
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
            graph_app,
            toasts,
            window,
            tiles_tree,
            graph_surface_focused,
            graph_search,
            focus_authority,
            toolbar_state,
            viewer_surfaces,
            tile_favicon_textures,
            favicon_textures,
            app_state,
            rendering_context,
            window_rendering_context,
            responsive_webviews,
            webview_creation_backpressure,
            frame_intents,
        } = args;

        // Destructure the bundle into the raw refs that
        // `gui_orchestration::run_graph_search_phase` still consumes.
        // The bundle is the host-facing shape; the five refs are the
        // widget-orchestration shape.
        let GraphSearchAuthorityMut {
            open: graph_search_open,
            query: graph_search_query,
            filter_mode: graph_search_filter_mode,
            matches: graph_search_matches,
            active_match_index: graph_search_active_match_index,
        } = graph_search;

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
            graph_search_output.suppress_toggle_view,
            frame_intents,
        );

        graph_search_output
    }

    fn run_toolbar_and_graph_search_window_phases(args: ToolbarAndGraphSearchWindowPhaseArgs<'_>) {
        let ToolbarAndGraphSearchWindowPhaseArgs {
            ctx,
            winit_window,
            state,
            graph_app,
            control_panel,
            #[cfg(feature = "diagnostics")]
            diagnostics_state,
            window,
            tiles_tree,
            graph_tree,
            focused_node_hint: _,
            graph_surface_focused,
            focus_authority,
            toolbar_state,
            clear_data_confirm_deadline_secs,
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
            graph_search,
            graph_search_output,
            frame_intents,
            open_node_tile_after_intents,
        } = args;

        let GraphSearchAuthorityMut {
            open: graph_search_open,
            query: graph_search_query,
            filter_mode: graph_search_filter_mode,
            matches: graph_search_matches,
            active_match_index: graph_search_active_match_index,
        } = graph_search;

        let mut local_widget_focus = focus_authority.local_widget_focus.clone();

        let (toolbar_visible, is_graph_view) = gui_orchestration::run_toolbar_phase(
            ctx,
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
