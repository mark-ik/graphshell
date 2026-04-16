use crate::app::{GraphBrowserApp, GraphIntent, ReducerDispatchContext};

pub(crate) fn apply_ui_intents_with_checkpoint(
    app: &mut GraphBrowserApp,
    intents: Vec<GraphIntent>,
) {
    if intents.is_empty() {
        return;
    }
    let layout_before = app
        .last_session_workspace_layout_json()
        .map(str::to_string)
        .or_else(|| app.load_workspace_layout_json(GraphBrowserApp::SESSION_WORKSPACE_LAYOUT_NAME));
    app.apply_reducer_intents_with_context(
        intents,
        ReducerDispatchContext {
            workspace_layout_before: layout_before,
            ..ReducerDispatchContext::default()
        },
    );
}

pub(crate) fn apply_reducer_graph_intents_hardened<I>(app: &mut GraphBrowserApp, intents: I)
where
    I: IntoIterator<Item = GraphIntent>,
{
    app.apply_reducer_intents(intents);
}
