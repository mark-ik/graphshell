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

impl Gui {
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
            toasts,
            clipboard,
            favicon_textures,
            tile_rendering_contexts,
            tile_favicon_textures,
            thumbnail_capture_tx,
            thumbnail_capture_rx,
            thumbnail_capture_in_flight,
            webview_creation_backpressure,
            app_state,
            graph_search_open,
            graph_search_query,
            graph_search_filter_mode,
            graph_search_matches,
            graph_search_active_match_index,
            focus_authority,
            focused_node_hint,
            graph_surface_focused,
            focus_ring_node_key,
            focus_ring_started_at,
            focus_ring_duration,
            omnibar_search_session,
            command_palette_toggle_requested,
            pending_webview_context_surface_requests,
            bookmark_import_dialog,
            deferred_open_child_webviews: _,
            rendering_context,
            window_rendering_context,
            registry_runtime,
            control_panel,
            #[cfg(feature = "diagnostics")]
            diagnostics_state,
        } = args;

        Self::run_update_frame_prelude(ctx, graph_app, pending_webview_a11y_updates, tiles_tree);
        // Track user gestures for Tier 1 worker idle suspension (§5 of
        // Runtime Task Budget). Any egui input event counts as a gesture.
        if ctx.input(|i| !i.events.is_empty()) {
            control_panel.notify_user_gesture();
        }
        control_panel.tick_idle_watchdog(registry_runtime);
        let (pre_frame, mut frame_intents) =
            Self::run_pre_frame_and_initialize_intents(PreFrameAndIntentInitArgs {
                ctx,
                graph_app,
                state,
                window,
                favicon_textures,
                thumbnail_capture_tx,
                thumbnail_capture_rx,
                thumbnail_capture_in_flight,
                command_palette_toggle_requested,
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
                graph_search_open,
                graph_search_query,
                graph_search_filter_mode,
                graph_search_matches,
                graph_search_active_match_index,
                focus_authority,
                toolbar_state,
                tile_rendering_contexts,
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
            graph_surface_focused: *graph_surface_focused,
            focus_authority,
            toolbar_state,
            omnibar_search_session,
            toasts,
            tile_rendering_contexts,
            tile_favicon_textures,
            favicon_textures,
            app_state,
            rendering_context,
            window_rendering_context,
            responsive_webviews: &pre_frame.responsive_webviews,
            webview_creation_backpressure,
            graph_search_open,
            graph_search_query,
            graph_search_filter_mode,
            graph_search_matches,
            graph_search_active_match_index,
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
            tile_rendering_contexts,
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
            registry_runtime,
            control_panel,
            #[cfg(feature = "diagnostics")]
            diagnostics_state,
            responsive_webviews: &pre_frame.responsive_webviews,
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

    fn initialize_frame_intents(
        graph_app: &GraphBrowserApp,
        pre_frame_intents: Vec<GraphIntent>,
        control_panel: &mut ControlPanel,
    ) -> Vec<GraphIntent> {
        frame_prelude::initialize_frame_intents(graph_app, pre_frame_intents, control_panel)
    }

    fn update_prefetch_lifecycle_policy(graph_app: &GraphBrowserApp, control_panel: &ControlPanel) {
        frame_prelude::update_prefetch_lifecycle_policy(graph_app, control_panel);
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
            graph_search_open,
            graph_search_query,
            graph_search_filter_mode,
            graph_search_matches,
            graph_search_active_match_index,
            focus_authority,
            toolbar_state,
            tile_rendering_contexts,
            tile_favicon_textures,
            favicon_textures,
            app_state,
            rendering_context,
            window_rendering_context,
            responsive_webviews,
            webview_creation_backpressure,
            frame_intents,
        } = args;

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
            pane_queries::tree_has_active_node_pane(tiles_tree),
        );

        gui_orchestration::run_keyboard_phase(
            ctx,
            graph_app,
            *graph_surface_focused,
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
            omnibar_search_session,
            toasts,
            tile_rendering_contexts,
            tile_favicon_textures,
            favicon_textures,
            app_state,
            rendering_context,
            window_rendering_context,
            responsive_webviews,
            webview_creation_backpressure,
            graph_search_open,
            graph_search_query,
            graph_search_filter_mode,
            graph_search_matches,
            graph_search_active_match_index,
            graph_search_output,
            frame_intents,
            open_node_tile_after_intents,
        } = args;

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
            graph_search_output.focus_location_field_for_search,
            omnibar_search_session,
            toasts,
            tile_rendering_contexts,
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

