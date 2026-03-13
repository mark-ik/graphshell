use super::*;

pub(super) fn reconcile_mappings_and_selection(
    app: &mut GraphBrowserApp,
    seen_webviews: &HashSet<WebViewId>,
    active_webview: Option<WebViewId>,
) -> Vec<GraphIntent> {
    let mut intents = Vec::new();
    if let Some(active_wv_id) = active_webview
        && let Some(active_node_key) = app.get_node_for_webview(active_wv_id)
    {
        intents.push(GraphIntent::SelectNode {
            key: active_node_key,
            multi_select: false,
        });
    }

    let old_webviews: Vec<WebViewId> = app
        .webview_node_mappings()
        .filter(|(wv_id, _)| !seen_webviews.contains(wv_id))
        .map(|(wv_id, _)| wv_id)
        .collect();

    for wv_id in old_webviews {
        intents.push(RuntimeEvent::UnmapWebview { webview_id: wv_id }.into());
    }
    intents
}

pub(super) fn resolve_active_webview_for_sync(
    app: &GraphBrowserApp,
    window_active_webview: Option<WebViewId>,
) -> Option<WebViewId> {
    app.embedded_content_focus_webview()
        .or(window_active_webview)
}
