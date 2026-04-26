use super::*;

pub(super) fn reconcile_mappings_and_selection(
    app: &mut GraphBrowserApp,
    seen_webviews: &HashSet<WebViewId>,
    active_webview: Option<WebViewId>,
) -> Vec<GraphIntent> {
    let mut intents = Vec::new();
    if let Some(active_wv_id) = active_webview
        && let Some(active_node_key) = app.get_node_for_webview(
            crate::shell::desktop::lifecycle::webview_status_sync::renderer_id_from_servo(
                active_wv_id,
            ),
        )
        && app.focused_selection().primary().is_none()
    {
        intents.push(GraphIntent::SelectNode {
            key: active_node_key,
            multi_select: false,
        });
    }

    let old_webviews: Vec<_> = app
        .webview_node_mappings()
        .filter_map(|(renderer_id, _)| {
            match crate::shell::desktop::lifecycle::webview_status_sync::servo_webview_id_from_renderer(renderer_id) {
                Some(wv_id) if seen_webviews.contains(&wv_id) => None,
                _ => Some(renderer_id),
            }
        })
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
        .and_then(
            crate::shell::desktop::lifecycle::webview_status_sync::servo_webview_id_from_renderer,
        )
        .or(window_active_webview)
}
