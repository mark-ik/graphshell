/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use crate::app::{BrowserCommand, BrowserCommandTarget, GraphBrowserApp, GraphIntent};
use crate::shell::desktop::host::window::EmbedderWindow;
use crate::shell::desktop::lifecycle::webview_controller;
use crate::shell::desktop::runtime::diagnostics::{DiagnosticEvent, emit_event};
use crate::shell::desktop::runtime::registries;
use crate::shell::desktop::runtime::registries::input::binding_id;
use crate::shell::desktop::runtime::registries::{
    CHANNEL_UI_COMMAND_BAR_NAV_ACTION_BLOCKED, CHANNEL_UI_COMMAND_BAR_NAV_ACTION_REQUESTED,
};
use crate::shell::desktop::ui::nav_targeting;
use crate::shell::desktop::ui::toolbar::toolbar_ui::CommandBarFocusTarget;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ToolbarNavAction {
    Back,
    Forward,
    Reload,
    StopLoad,
    ZoomIn,
    ZoomOut,
    ZoomReset,
    Close,
}

pub(crate) enum ToolbarOpenMode {
    Tab,
    SplitHorizontal,
}

pub(crate) struct ToolbarSubmitResult {
    pub(crate) intents: Vec<GraphIntent>,
    pub(crate) mark_clean: bool,
    pub(crate) open_mode: Option<ToolbarOpenMode>,
}

fn nav_action_label(action: ToolbarNavAction) -> &'static str {
    match action {
        ToolbarNavAction::Back => "back",
        ToolbarNavAction::Forward => "forward",
        ToolbarNavAction::Reload => "reload",
        ToolbarNavAction::StopLoad => "stop_load",
        ToolbarNavAction::ZoomIn => "zoom_in",
        ToolbarNavAction::ZoomOut => "zoom_out",
        ToolbarNavAction::ZoomReset => "zoom_reset",
        ToolbarNavAction::Close => "close",
    }
}

fn emit_command_bar_nav_action_requested(action: ToolbarNavAction) {
    emit_event(DiagnosticEvent::MessageSent {
        channel_id: CHANNEL_UI_COMMAND_BAR_NAV_ACTION_REQUESTED,
        byte_len: nav_action_label(action).len(),
    });
}

fn emit_command_bar_nav_action_blocked(action: ToolbarNavAction) {
    emit_event(DiagnosticEvent::MessageSent {
        channel_id: CHANNEL_UI_COMMAND_BAR_NAV_ACTION_BLOCKED,
        byte_len: nav_action_label(action).len(),
    });
}

pub(crate) fn run_nav_action(
    graph_app: &mut GraphBrowserApp,
    _window: &EmbedderWindow,
    command_bar_focus_target: CommandBarFocusTarget,
    action: ToolbarNavAction,
) -> bool {
    emit_command_bar_nav_action_requested(action);
    if let Some(binding_id) = match action {
        ToolbarNavAction::Back => Some(binding_id::toolbar::NAV_BACK),
        ToolbarNavAction::Forward => Some(binding_id::toolbar::NAV_FORWARD),
        ToolbarNavAction::Reload => Some(binding_id::toolbar::NAV_RELOAD),
        ToolbarNavAction::StopLoad
        | ToolbarNavAction::ZoomIn
        | ToolbarNavAction::ZoomOut
        | ToolbarNavAction::ZoomReset
        | ToolbarNavAction::Close => None,
    } {
        if !registries::phase2_resolve_input_binding(binding_id) {
            emit_command_bar_nav_action_blocked(action);
            return false;
        }
    }

    let command = match action {
        ToolbarNavAction::Back => BrowserCommand::Back,
        ToolbarNavAction::Forward => BrowserCommand::Forward,
        ToolbarNavAction::Reload => BrowserCommand::Reload,
        ToolbarNavAction::StopLoad => BrowserCommand::StopLoad,
        ToolbarNavAction::ZoomIn => BrowserCommand::ZoomIn,
        ToolbarNavAction::ZoomOut => BrowserCommand::ZoomOut,
        ToolbarNavAction::ZoomReset => BrowserCommand::ZoomReset,
        ToolbarNavAction::Close => BrowserCommand::Close,
    };
    let target = BrowserCommandTarget::ChromeProjection {
        fallback_node: nav_targeting::chrome_projection_node(graph_app, _window)
            .or(command_bar_focus_target.focused_node()),
    };
    graph_app.request_browser_command(target, command);
    true
}

pub(crate) fn submit_address_bar_intents(
    graph_app: &GraphBrowserApp,
    location: &str,
    is_graph_view: bool,
    command_bar_focus_target: CommandBarFocusTarget,
    split_open_requested: bool,
    window: &EmbedderWindow,
    searchpage: &str,
) -> ToolbarSubmitResult {
    if !registries::phase2_resolve_toolbar_submit_binding() {
        return ToolbarSubmitResult {
            intents: Vec::new(),
            mark_clean: false,
            open_mode: None,
        };
    }

    let submit_result = webview_controller::handle_address_bar_submit_intents(
        graph_app,
        location,
        is_graph_view,
        command_bar_focus_target.focused_node(),
        window,
        searchpage,
    );
    ToolbarSubmitResult {
        intents: submit_result.intents,
        mark_clean: submit_result.outcome.mark_clean,
        open_mode: requested_open_mode(
            submit_result.outcome.open_selected_tile,
            split_open_requested,
        ),
    }
}

fn requested_open_mode(
    open_selected_tile: bool,
    split_open_requested: bool,
) -> Option<ToolbarOpenMode> {
    if !open_selected_tile {
        return None;
    }
    Some(if split_open_requested {
        ToolbarOpenMode::SplitHorizontal
    } else {
        ToolbarOpenMode::Tab
    })
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
    fn test_requested_open_mode_none_when_not_requested() {
        assert!(requested_open_mode(false, false).is_none());
        assert!(requested_open_mode(false, true).is_none());
    }

    #[test]
    fn test_requested_open_mode_tab_when_split_not_requested() {
        assert!(matches!(
            requested_open_mode(true, false),
            Some(ToolbarOpenMode::Tab)
        ));
    }

    #[test]
    fn test_requested_open_mode_split_when_requested() {
        assert!(matches!(
            requested_open_mode(true, true),
            Some(ToolbarOpenMode::SplitHorizontal)
        ));
    }

    #[test]
    fn stop_load_action_enqueues_browser_command_without_binding_lookup() {
        let prefs = AppPreferences::default();
        let window = EmbedderWindow::new(HeadlessWindow::new(&prefs), Arc::new(AtomicU64::new(0)));
        let mut app = GraphBrowserApp::new_for_testing();

        assert!(run_nav_action(
            &mut app,
            &window,
            CommandBarFocusTarget::default(),
            ToolbarNavAction::StopLoad
        ));
        assert_eq!(
            app.take_pending_browser_command(),
            Some((
                BrowserCommandTarget::ChromeProjection {
                    fallback_node: None
                },
                BrowserCommand::StopLoad,
            ))
        );
    }

    #[test]
    fn close_action_enqueues_browser_command_without_binding_lookup() {
        let prefs = AppPreferences::default();
        let window = EmbedderWindow::new(HeadlessWindow::new(&prefs), Arc::new(AtomicU64::new(0)));
        let mut app = GraphBrowserApp::new_for_testing();

        assert!(run_nav_action(
            &mut app,
            &window,
            CommandBarFocusTarget::default(),
            ToolbarNavAction::Close
        ));
        assert_eq!(
            app.take_pending_browser_command(),
            Some((
                BrowserCommandTarget::ChromeProjection {
                    fallback_node: None
                },
                BrowserCommand::Close,
            ))
        );
    }

    #[test]
    fn nav_action_helpers_emit_request_and_blocked_diagnostics() {
        let (diag_tx, diag_rx) = crossbeam_channel::unbounded();
        install_global_sender(diag_tx);

        emit_command_bar_nav_action_requested(ToolbarNavAction::Close);
        emit_command_bar_nav_action_blocked(ToolbarNavAction::Reload);

        let emitted: Vec<DiagnosticEvent> = diag_rx.try_iter().collect();
        assert!(
            emitted.iter().any(|event| matches!(
                event,
                DiagnosticEvent::MessageSent { channel_id, .. }
                    if *channel_id == CHANNEL_UI_COMMAND_BAR_NAV_ACTION_REQUESTED
            )),
            "expected nav-action requested diagnostic; got: {emitted:?}"
        );
        assert!(
            emitted.iter().any(|event| matches!(
                event,
                DiagnosticEvent::MessageSent { channel_id, .. }
                    if *channel_id == CHANNEL_UI_COMMAND_BAR_NAV_ACTION_BLOCKED
            )),
            "expected nav-action blocked diagnostic; got: {emitted:?}"
        );
    }
}
