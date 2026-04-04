use super::*;
use crate::shell::desktop::runtime::diagnostics::{DiagnosticEvent, emit_event};
use crate::shell::desktop::runtime::registries::CHANNEL_UI_COMMAND_BAR_NAV_ACTION_NO_TARGET;

const WEBVIEW_STOP_LOAD_SUPPORTED: bool = false;

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
            emit_event(DiagnosticEvent::MessageSent {
                channel_id: CHANNEL_UI_COMMAND_BAR_NAV_ACTION_NO_TARGET,
                byte_len: command.diagnostic_label().len(),
            });
            continue;
        };
        let Some(webview) = window.webview_by_id(webview_id) else {
            emit_event(DiagnosticEvent::MessageSent {
                channel_id: CHANNEL_UI_COMMAND_BAR_NAV_ACTION_NO_TARGET,
                byte_len: command.diagnostic_label().len(),
            });
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
            BrowserCommand::StopLoad => {
                if WEBVIEW_STOP_LOAD_SUPPORTED {
                    window.set_needs_update();
                }
            }
            BrowserCommand::ZoomIn => {
                webview.set_page_zoom(webview.page_zoom() + 0.1);
                window.set_needs_update();
            }
            BrowserCommand::ZoomOut => {
                webview.set_page_zoom(webview.page_zoom() - 0.1);
                window.set_needs_update();
            }
            BrowserCommand::ZoomReset => {
                webview.set_page_zoom(1.0);
                window.set_needs_update();
            }
            BrowserCommand::Close => {
                window.close_webview(webview_id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prefs::AppPreferences;
    use crate::shell::desktop::host::headless_window::HeadlessWindow;
    use crate::shell::desktop::host::window::EmbedderWindow;
    use crate::shell::desktop::runtime::diagnostics::{DiagnosticEvent, install_global_sender};
    use std::sync::Arc;
    use std::sync::atomic::AtomicU64;

    #[test]
    fn pending_browser_command_without_target_emits_no_target_diagnostic() {
        let prefs = AppPreferences::default();
        let window = EmbedderWindow::new(HeadlessWindow::new(&prefs), Arc::new(AtomicU64::new(0)));
        let mut app = GraphBrowserApp::new_for_testing();
        let (diag_tx, diag_rx) = crossbeam_channel::unbounded();
        install_global_sender(diag_tx);

        app.request_browser_command(
            BrowserCommandTarget::ChromeProjection { fallback_node: None },
            BrowserCommand::Close,
        );

        apply_pending_browser_commands(&mut app, &window);

        let emitted: Vec<DiagnosticEvent> = diag_rx.try_iter().collect();
        assert!(
            emitted.iter().any(|event| matches!(
                event,
                DiagnosticEvent::MessageSent { channel_id, .. }
                    if *channel_id == CHANNEL_UI_COMMAND_BAR_NAV_ACTION_NO_TARGET
            )),
            "expected no-target diagnostic; got: {emitted:?}"
        );
    }
}
