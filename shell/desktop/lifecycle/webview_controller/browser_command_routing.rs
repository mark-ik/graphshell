use super::*;

pub(super) fn resolve_browser_command_target(
    app: &GraphBrowserApp,
    window: &EmbedderWindow,
    target: BrowserCommandTarget,
) -> Option<WebViewId> {
    match target {
        BrowserCommandTarget::FocusedInput => app.embedded_content_focus_webview(),
        BrowserCommandTarget::ChromeProjection { fallback_node } => window
            .explicit_chrome_webview_id()
            .or_else(|| fallback_node.and_then(|node_key| app.get_webview_for_node(node_key))),
    }
}

pub(super) fn apply_pending_browser_commands(app: &mut GraphBrowserApp, window: &EmbedderWindow) {
    while let Some((target, command)) = app.take_pending_browser_command() {
        let Some(webview_id) = resolve_browser_command_target(app, window, target) else {
            continue;
        };
        let Some(webview) = window.webview_by_id(webview_id) else {
            continue;
        };
        match command {
            BrowserCommand::Back => {
                let _ = webview.go_back(1);
                window.set_needs_update();
            }
            BrowserCommand::Forward => {
                let _ = webview.go_forward(1);
                window.set_needs_update();
            }
            BrowserCommand::Reload => {
                webview.reload();
                window.set_needs_update();
            }
            BrowserCommand::Close => {
                window.close_webview(webview_id);
            }
        }
    }
}
