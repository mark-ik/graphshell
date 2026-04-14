use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn run_graph_search_phase(
    ctx: &egui::Context,
    graph_app: &mut GraphBrowserApp,
    toasts: &mut egui_notify::Toasts,
    graph_search_open: &mut bool,
    local_widget_focus: &mut Option<LocalFocusTarget>,
    graph_search_query: &mut String,
    graph_search_filter_mode: &mut bool,
    graph_search_matches: &mut Vec<NodeKey>,
    graph_search_active_match_index: &mut Option<usize>,
    toolbar_state: &mut ToolbarState,
    frame_intents: &mut Vec<GraphIntent>,
    has_active_node_pane: bool,
) -> graph_search_flow::GraphSearchFlowOutput {
    let graph_search_available = !has_active_node_pane;

    if let Some(request) = graph_app.take_pending_graph_search_request() {
        if request.record_history {
            maybe_push_graph_search_history(graph_app, &request);
        }
        *graph_search_query = request.query;
        *graph_search_filter_mode = request.filter_mode;
        crate::shell::desktop::ui::gui::apply_graph_search_local_focus_state(
            graph_search_open,
            local_widget_focus,
            !graph_search_query.trim().is_empty(),
        );
        graph_app.workspace.graph_runtime.active_graph_search_origin = request.origin;
        graph_app
            .workspace
            .graph_runtime
            .active_graph_search_neighborhood_anchor = request.neighborhood_anchor;
        graph_app
            .workspace
            .graph_runtime
            .active_graph_search_neighborhood_depth = request.neighborhood_depth;
        if let Some(message) = request.toast_message {
            toasts.success(message);
        }
        graph_app.workspace.graph_runtime.egui_state_dirty = true;
    }

    graph_app.workspace.graph_runtime.active_graph_search_query =
        graph_search_query.trim().to_string();
    graph_app.workspace.graph_runtime.search_display_mode = if *graph_search_filter_mode {
        SearchDisplayMode::Filter
    } else {
        SearchDisplayMode::Highlight
    };

    let output = graph_search_flow::handle_graph_search_flow(
        GraphSearchFlowArgs {
            ctx,
            graph_app,
            graph_search_open,
            local_widget_focus,
            graph_search_query,
            graph_search_filter_mode,
            graph_search_matches,
            graph_search_active_match_index,
            location: &mut toolbar_state.location,
            location_dirty: &mut toolbar_state.location_dirty,
            frame_intents,
            graph_search_available,
        },
        |graph_app, query, matches, active_index| {
            refresh_graph_search_matches(graph_app, query, matches, active_index);
        },
        |matches, active_index, delta| {
            step_graph_search_active_match(matches, active_index, delta);
        },
        |matches, active_index| active_graph_search_match(matches, active_index),
    );

    if !*graph_search_open && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        graph_app.enqueue_workbench_intent(WorkbenchIntent::ClearTileSelection);
    }
    graph_app
        .workspace
        .graph_runtime
        .active_graph_search_match_count = graph_search_matches.len();
    output
}

#[allow(clippy::too_many_arguments)]
pub(super) fn run_graph_search_window_phase(
    ctx: &egui::Context,
    graph_app: &mut GraphBrowserApp,
    toolbar_visible: bool,
    graph_search_open: bool,
    is_graph_view: bool,
    local_widget_focus: &mut Option<LocalFocusTarget>,
    graph_search_query: &mut String,
    graph_search_filter_mode: &mut bool,
    graph_search_matches: &mut Vec<NodeKey>,
    graph_search_active_match_index: &mut Option<usize>,
    graph_search_output: &mut graph_search_flow::GraphSearchFlowOutput,
) {
    if should_render_graph_search_window(toolbar_visible, graph_search_open, is_graph_view) {
        graph_search_ui::render_graph_search_window(
            GraphSearchUiArgs {
                ctx,
                graph_app,
                graph_search_query,
                graph_search_filter_mode,
                graph_search_matches,
                graph_search_active_match_index,
                local_widget_focus,
                focus_graph_search_field: &mut graph_search_output.focus_graph_search_field,
            },
            |graph_app, query, matches, active_index| {
                refresh_graph_search_matches(graph_app, query, matches, active_index);
            },
        );
    }
}

fn should_render_graph_search_window(
    toolbar_visible: bool,
    graph_search_open: bool,
    is_graph_view: bool,
) -> bool {
    !toolbar_visible && graph_search_open && is_graph_view
}

