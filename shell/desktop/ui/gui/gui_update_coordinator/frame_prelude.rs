use super::*;

pub(super) fn run_update_frame_prelude(
    ctx: &egui::Context,
    graph_app: &mut GraphBrowserApp,
    pending_webview_a11y_updates: &mut HashMap<WebViewId, accesskit::TreeUpdate>,
    tiles_tree: &mut Tree<TileKind>,
) {
    // `graph_app.tick_frame()` migrated onto `GraphshellRuntime::tick` in
    // M4.5b Step 4 and now runs on the runtime's per-tick path.
    pane_queries::reconcile_workspace_graph_views_from_tiles(graph_app, tiles_tree);

    accessibility::inject_uxtree_a11y_updates(ctx, graph_app);
    accessibility::inject_webview_a11y_updates(ctx, pending_webview_a11y_updates);
    maybe_toggle_diagnostics_tool_pane(ctx, tiles_tree);
}

pub(super) fn configure_frame_toasts(
    toasts: &mut egui_notify::Toasts,
    preference: ToastAnchorPreference,
    toast_anchor: fn(ToastAnchorPreference) -> egui_notify::Anchor,
) {
    *toasts = std::mem::take(toasts)
        .with_anchor(toast_anchor(preference))
        .with_margin(egui::vec2(12.0, 12.0));
}

pub(super) fn initialize_frame_intents(
    _graph_app: &GraphBrowserApp,
    pre_frame_intents: Vec<GraphIntent>,
    control_panel: &mut ControlPanel,
) -> Vec<GraphIntent> {
    // `update_prefetch_lifecycle_policy` migrated onto `GraphshellRuntime::tick`
    // in M4.5b Step 5.
    let mut frame_intents = pre_frame_intents;
    frame_intents.extend(control_panel.drain_pending());
    frame_intents
}

pub(super) fn run_pre_frame_and_initialize_intents(
    args: PreFrameAndIntentInitArgs<'_>,
) -> (gui_orchestration::PreFramePhaseOutput, Vec<GraphIntent>) {
    let PreFrameAndIntentInitArgs {
        ctx,
        state,
        window,
        favicon_textures,
        thumbnail_channel,
        runtime,
    } = args;

    let pre_frame = gui_orchestration::run_pre_frame_phase(
        ctx,
        &mut runtime.graph_app,
        state,
        window,
        favicon_textures,
        thumbnail_channel,
        &mut runtime.thumbnail_capture_in_flight,
        &mut runtime.command_palette_toggle_requested,
    );
    let frame_intents = initialize_frame_intents(
        &mut runtime.graph_app,
        pre_frame.frame_intents.clone(),
        &mut runtime.control_panel,
    );

    (pre_frame, frame_intents)
}

#[cfg(feature = "diagnostics")]
pub(super) fn maybe_toggle_diagnostics_tool_pane(
    ctx: &egui::Context,
    tiles_tree: &mut Tree<TileKind>,
) {
    let toggle_diagnostics = ctx.input(|i| {
        i.key_pressed(egui::Key::F12)
            || (i.modifiers.ctrl && i.modifiers.shift && i.key_pressed(egui::Key::D))
    });
    if toggle_diagnostics {
        tile_view_ops::open_or_focus_tool_pane(
            tiles_tree,
            crate::shell::desktop::workbench::pane_model::ToolPaneState::Diagnostics,
        );
    }
}

#[cfg(not(feature = "diagnostics"))]
pub(super) fn maybe_toggle_diagnostics_tool_pane(
    _ctx: &egui::Context,
    _tiles_tree: &mut Tree<TileKind>,
) {
}
