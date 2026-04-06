use super::*;
use crate::shell::desktop::runtime::diagnostics::{
    DiagnosticEvent, StructuredPayloadField, emit_event, emit_message_received_with_payload,
    emit_message_sent_with_payload, structured_payload_field,
};
use crate::shell::desktop::runtime::registries::{
    CHANNEL_UI_COMMAND_BAR_NAV_ACTION_NO_TARGET, CHANNEL_UI_COMMAND_SURFACE_ROUTE_FALLBACK,
    CHANNEL_UI_COMMAND_SURFACE_ROUTE_NO_TARGET, CHANNEL_UI_COMMAND_SURFACE_ROUTE_RESOLVED,
};

const WEBVIEW_STOP_LOAD_SUPPORTED: bool = false;

fn browser_command_route_fields(
    command: BrowserCommand,
    target: BrowserCommandTarget,
    route_detail: &'static str,
) -> Vec<StructuredPayloadField> {
    vec![
        structured_payload_field("source_surface", "browser_command"),
        structured_payload_field("command_id", command.diagnostic_label()),
        structured_payload_field(
            "target_kind",
            match target {
                BrowserCommandTarget::FocusedInput => "focused_input",
                BrowserCommandTarget::ChromeProjection { .. } => "chrome_projection",
            },
        ),
        structured_payload_field("route_detail", route_detail),
    ]
}

pub(super) enum BrowserCommandRouteOutcome {
    Resolved(WebViewId),
    Fallback(WebViewId),
    NoTarget,
}

pub(super) fn resolve_browser_command_target(
    app: &GraphBrowserApp,
    window: &EmbedderWindow,
    target: BrowserCommandTarget,
) -> BrowserCommandRouteOutcome {
    match target {
        BrowserCommandTarget::FocusedInput => app
            .embedded_content_focus_webview()
            .map(BrowserCommandRouteOutcome::Resolved)
            .unwrap_or(BrowserCommandRouteOutcome::NoTarget),
        BrowserCommandTarget::ChromeProjection { fallback_node } => {
            if let Some(webview_id) = window.explicit_chrome_webview_id() {
                BrowserCommandRouteOutcome::Resolved(webview_id)
            } else if let Some(webview_id) = fallback_node
                .and_then(|node_key| app.get_webview_for_node(node_key))
            {
                BrowserCommandRouteOutcome::Fallback(webview_id)
            } else {
                BrowserCommandRouteOutcome::NoTarget
            }
        }
    }
}

pub(super) fn apply_pending_browser_commands(app: &mut GraphBrowserApp, window: &EmbedderWindow) {
    while let Some((target, command)) = app.take_pending_browser_command() {
        let webview_id = match resolve_browser_command_target(app, window, target) {
            BrowserCommandRouteOutcome::Resolved(webview_id) => {
                emit_message_received_with_payload(
                    CHANNEL_UI_COMMAND_SURFACE_ROUTE_RESOLVED,
                    command.diagnostic_label().len() as u64,
                    browser_command_route_fields(command, target, "resolved_target"),
                );
                webview_id
            }
            BrowserCommandRouteOutcome::Fallback(webview_id) => {
                emit_message_sent_with_payload(
                    CHANNEL_UI_COMMAND_SURFACE_ROUTE_FALLBACK,
                    command.diagnostic_label().len(),
                    browser_command_route_fields(command, target, "fallback_node_webview"),
                );
                webview_id
            }
            BrowserCommandRouteOutcome::NoTarget => {
                emit_message_sent_with_payload(
                    CHANNEL_UI_COMMAND_SURFACE_ROUTE_NO_TARGET,
                    command.diagnostic_label().len(),
                    browser_command_route_fields(command, target, "no_matching_target"),
                );
                emit_event(DiagnosticEvent::MessageSent {
                    channel_id: CHANNEL_UI_COMMAND_BAR_NAV_ACTION_NO_TARGET,
                    byte_len: command.diagnostic_label().len(),
                });
                continue;
            }
        };
        let Some(webview) = window.webview_by_id(webview_id) else {
            emit_message_sent_with_payload(
                CHANNEL_UI_COMMAND_SURFACE_ROUTE_NO_TARGET,
                command.diagnostic_label().len(),
                browser_command_route_fields(command, target, "window_missing_webview"),
            );
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
    use crate::shell::desktop::runtime::registries::CHANNEL_UI_COMMAND_SURFACE_ROUTE_NO_TARGET;
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
                DiagnosticEvent::MessageSentStructured { channel_id, fields, .. }
                    if *channel_id == CHANNEL_UI_COMMAND_SURFACE_ROUTE_NO_TARGET
                        && fields.iter().any(|field| field.name == "route_detail" && field.value == "no_matching_target")
            )),
            "expected generic command-surface no-target diagnostic; got: {emitted:?}"
        );
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
