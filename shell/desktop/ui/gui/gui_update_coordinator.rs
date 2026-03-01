/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use super::*;

impl Gui {
    pub(super) fn execute_update_frame(args: ExecuteUpdateFrameArgs<'_>) {
        debug_assert!(
            Self::is_canonical_update_frame_stage_sequence(&UPDATE_FRAME_STAGE_SEQUENCE)
        );
        let ExecuteUpdateFrameArgs {
            ctx,
            winit_window,
            state,
            window,
            headed_window,
            graph_app,
            pending_webview_a11y_updates,
            tiles_tree,
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
            focused_node_hint,
            graph_surface_focused,
            focus_ring_node_key,
            focus_ring_started_at,
            focus_ring_duration,
            omnibar_search_session,
            command_palette_toggle_requested,
            deferred_open_child_webviews,
            rendering_context,
            window_rendering_context,
            registry_runtime,
            control_panel,
            #[cfg(feature = "diagnostics")]
            diagnostics_state,
        } = args;

        Self::run_update_frame_prelude(ctx, graph_app, pending_webview_a11y_updates, tiles_tree);
        let (pre_frame, mut frame_intents) = Self::run_pre_frame_and_initialize_intents(
            PreFrameAndIntentInitArgs {
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
            },
        );

        let mut open_node_tile_after_intents: Option<TileOpenMode> = None;

        let mut graph_search_output = Self::run_graph_search_and_keyboard_phases(
            GraphSearchAndKeyboardPhaseArgs {
                ctx,
                graph_app,
                window,
                tiles_tree,
                graph_search_open,
                graph_search_query,
                graph_search_filter_mode,
                graph_search_matches,
                graph_search_active_match_index,
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
            },
        );

        Self::run_toolbar_and_graph_search_window_phases(ToolbarAndGraphSearchWindowPhaseArgs {
            ctx,
            winit_window,
            state,
            graph_app,
            #[cfg(feature = "diagnostics")]
            diagnostics_state,
            window,
            tiles_tree,
            focused_node_hint: *focused_node_hint,
            graph_surface_focused: *graph_surface_focused,
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

        Self::run_semantic_and_post_render_phases(SemanticAndPostRenderPhaseArgs {
            ctx,
            graph_app,
            window,
            headed_window,
            tiles_tree,
            toolbar_height,
            tile_rendering_contexts,
            tile_favicon_textures,
            favicon_textures,
            app_state,
            rendering_context,
            window_rendering_context,
            webview_creation_backpressure,
            focused_node_hint,
            graph_surface_focused,
            focus_ring_node_key,
            focus_ring_started_at,
            focus_ring_duration,
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
            pending_open_child_webviews: {
                let mut pending = std::mem::take(deferred_open_child_webviews);
                pending.extend(pre_frame.pending_open_child_webviews);
                pending
            },
            deferred_open_child_webviews,
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
        graph_app.tick_frame();

        Self::inject_webview_a11y_updates(ctx, pending_webview_a11y_updates);
        Self::maybe_toggle_diagnostics_tool_pane(ctx, tiles_tree);
    }

    pub(super) fn configure_frame_toasts(
        toasts: &mut egui_notify::Toasts,
        preference: ToastAnchorPreference,
    ) {
        *toasts = std::mem::take(toasts)
            .with_anchor(Self::toast_anchor(preference))
            .with_margin(egui::vec2(12.0, 12.0));
    }

    fn initialize_frame_intents(
        pre_frame_intents: Vec<GraphIntent>,
        control_panel: &mut ControlPanel,
    ) -> Vec<GraphIntent> {
        let mut frame_intents = pre_frame_intents;
        frame_intents.extend(control_panel.drain_pending());
        frame_intents
    }

    fn run_pre_frame_and_initialize_intents(
        args: PreFrameAndIntentInitArgs<'_>,
    ) -> (gui_orchestration::PreFramePhaseOutput, Vec<GraphIntent>) {
        let PreFrameAndIntentInitArgs {
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
        } = args;

        let pre_frame = gui_orchestration::run_pre_frame_phase(
            ctx,
            graph_app,
            state,
            window,
            favicon_textures,
            thumbnail_capture_tx,
            thumbnail_capture_rx,
            thumbnail_capture_in_flight,
            command_palette_toggle_requested,
        );
        let frame_intents =
            Self::initialize_frame_intents(pre_frame.frame_intents.clone(), control_panel);

        (pre_frame, frame_intents)
    }

    fn run_graph_search_and_keyboard_phases(
        args: GraphSearchAndKeyboardPhaseArgs<'_>,
    ) -> graph_search_flow::GraphSearchFlowOutput {
        let GraphSearchAndKeyboardPhaseArgs {
            ctx,
            graph_app,
            window,
            tiles_tree,
            graph_search_open,
            graph_search_query,
            graph_search_filter_mode,
            graph_search_matches,
            graph_search_active_match_index,
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
            graph_search_open,
            graph_search_query,
            graph_search_filter_mode,
            graph_search_matches,
            graph_search_active_match_index,
            toolbar_state,
            frame_intents,
            Self::tree_has_active_node_pane(tiles_tree),
        );

        gui_orchestration::run_keyboard_phase(
            ctx,
            graph_app,
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

    fn run_toolbar_and_graph_search_window_phases(
        args: ToolbarAndGraphSearchWindowPhaseArgs<'_>,
    ) {
        let ToolbarAndGraphSearchWindowPhaseArgs {
            ctx,
            winit_window,
            state,
            graph_app,
            #[cfg(feature = "diagnostics")]
            diagnostics_state,
            window,
            tiles_tree,
            focused_node_hint,
            graph_surface_focused,
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

        let (toolbar_visible, is_graph_view) = gui_orchestration::run_toolbar_phase(
            ctx,
            winit_window,
            state,
            graph_app,
            #[cfg(feature = "diagnostics")]
            diagnostics_state,
            window,
            tiles_tree,
            focused_node_hint,
            graph_surface_focused,
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
            graph_search_query,
            graph_search_filter_mode,
            graph_search_matches,
            graph_search_active_match_index,
            graph_search_output,
        );
    }

    fn finalize_update_frame(
        ctx: &egui::Context,
        graph_app: &mut GraphBrowserApp,
        clipboard: &mut Option<Clipboard>,
        toasts: &mut egui_notify::Toasts,
    ) {
        gui_orchestration::handle_pending_clipboard_copy_requests(graph_app, clipboard, toasts);
        toasts.show(ctx);
    }

    #[cfg(feature = "diagnostics")]
    fn maybe_toggle_diagnostics_tool_pane(ctx: &egui::Context, tiles_tree: &mut Tree<TileKind>) {
        let toggle_diagnostics = ctx.input(|i| {
            i.key_pressed(egui::Key::F12)
                || (i.modifiers.ctrl && i.modifiers.shift && i.key_pressed(egui::Key::D))
        });
        if toggle_diagnostics {
            Self::open_or_focus_diagnostics_tool_pane(tiles_tree);
        }
    }

    #[cfg(not(feature = "diagnostics"))]
    fn maybe_toggle_diagnostics_tool_pane(
        _ctx: &egui::Context,
        _tiles_tree: &mut Tree<TileKind>,
    ) {
    }

    fn run_semantic_and_post_render_phases(args: SemanticAndPostRenderPhaseArgs<'_>) {
        let SemanticAndPostRenderPhaseArgs {
            ctx,
            graph_app,
            window,
            headed_window,
            tiles_tree,
            toolbar_height,
            tile_rendering_contexts,
            tile_favicon_textures,
            favicon_textures,
            app_state,
            rendering_context,
            window_rendering_context,
            webview_creation_backpressure,
            focused_node_hint,
            graph_surface_focused,
            focus_ring_node_key,
            focus_ring_started_at,
            focus_ring_duration,
            graph_search_query,
            graph_search_matches,
            graph_search_active_match_index,
            graph_search_filter_mode,
            toasts,
            registry_runtime,
            control_panel,
            #[cfg(feature = "diagnostics")]
            diagnostics_state,
            responsive_webviews,
            pending_open_child_webviews,
            deferred_open_child_webviews,
            open_node_tile_after_intents,
            frame_intents,
        } = args;

        Self::run_semantic_lifecycle_phase(SemanticLifecyclePhaseArgs {
            graph_app,
            tiles_tree,
            window,
            app_state,
            rendering_context,
            window_rendering_context,
            tile_rendering_contexts,
            tile_favicon_textures,
            favicon_textures,
            responsive_webviews,
            pending_open_child_webviews,
            deferred_open_child_webviews,
            webview_creation_backpressure,
            open_node_tile_after_intents,
            frame_intents,
        });

        knowledge::reconcile_semantics(graph_app, &registry_runtime.knowledge);
        let search_query_active = Self::is_graph_search_query_active(graph_search_query);

        gui_frame::run_post_render_phase(
            gui_frame::PostRenderPhaseArgs {
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
                toasts,
                control_panel,
                #[cfg(feature = "diagnostics")]
                diagnostics_state,
            },
            |matches, active_index| {
                gui_orchestration::active_graph_search_match(matches, active_index)
            },
        );
    }

    fn is_graph_search_query_active(query: &str) -> bool {
        !query.trim().is_empty()
    }

    fn run_semantic_lifecycle_phase(args: SemanticLifecyclePhaseArgs<'_>) {
        let SemanticLifecyclePhaseArgs {
            graph_app,
            tiles_tree,
            window,
            app_state,
            rendering_context,
            window_rendering_context,
            tile_rendering_contexts,
            tile_favicon_textures,
            favicon_textures,
            responsive_webviews,
            pending_open_child_webviews,
            deferred_open_child_webviews,
            webview_creation_backpressure,
            open_node_tile_after_intents,
            frame_intents,
        } = args;

        *deferred_open_child_webviews = gui_orchestration::run_semantic_lifecycle_phase(
            graph_app,
            tiles_tree,
            window,
            app_state,
            rendering_context,
            window_rendering_context,
            tile_rendering_contexts,
            tile_favicon_textures,
            favicon_textures,
            responsive_webviews,
            pending_open_child_webviews,
            webview_creation_backpressure,
            open_node_tile_after_intents,
            frame_intents,
        );
    }
}
