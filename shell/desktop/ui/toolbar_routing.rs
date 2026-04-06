/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use crate::app::{
    BrowserCommand, BrowserCommandTarget, GraphBrowserApp, GraphIntent, WorkbenchIntent,
};
use crate::graph::NodeKey;
use crate::shell::desktop::host::window::EmbedderWindow;
use crate::shell::desktop::lifecycle::webview_controller;
use crate::shell::desktop::runtime::diagnostics::{
    DiagnosticEvent, emit_event, emit_message_received_with_payload,
    emit_message_sent_with_payload, structured_payload_field,
};
use crate::shell::desktop::runtime::registries;
use crate::shell::desktop::runtime::registries::input::binding_id;
use crate::shell::desktop::runtime::registries::{
    CHANNEL_UI_COMMAND_BAR_COMMAND_PALETTE_REQUESTED,
    CHANNEL_UI_COMMAND_SURFACE_ROUTE_BLOCKED,
    CHANNEL_UI_COMMAND_SURFACE_ROUTE_RESOLVED,
    CHANNEL_UI_COMMAND_BAR_WORKBENCH_COMMAND_BLOCKED_BY_FOCUS,
    CHANNEL_UI_COMMAND_BAR_WORKBENCH_COMMAND_EXECUTED,
    CHANNEL_UI_COMMAND_BAR_WORKBENCH_COMMAND_REQUESTED,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ShellWorkbenchCommand {
    OpenCommandPalette,
    CloseCommandPalette,
    ToggleCommandPalette,
    CloseHelpPanel,
    ToggleHelpPanel,
    CloseRadialMenu,
    ToggleRadialMenu,
    CycleFocusRegion,
}

pub(crate) struct ToolbarSubmitResult {
    pub(crate) intents: Vec<GraphIntent>,
    pub(crate) mark_clean: bool,
    pub(crate) open_mode: Option<ToolbarOpenMode>,
    pub(crate) workbench_intents: Vec<WorkbenchIntent>,
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

fn emit_command_bar_command_palette_requested() {
    emit_event(DiagnosticEvent::MessageSent {
        channel_id: CHANNEL_UI_COMMAND_BAR_COMMAND_PALETTE_REQUESTED,
        byte_len: "command_palette".len(),
    });
}

fn emit_command_surface_route_resolved(
    command_id: &'static str,
    target_kind: &'static str,
    route_detail: &'static str,
    label_len: usize,
) {
    emit_message_received_with_payload(
        CHANNEL_UI_COMMAND_SURFACE_ROUTE_RESOLVED,
        label_len.max(1) as u64,
        vec![
            structured_payload_field("source_surface", "command_bar"),
            structured_payload_field("command_id", command_id),
            structured_payload_field("target_kind", target_kind),
            structured_payload_field("route_detail", route_detail),
        ],
    );
}

fn emit_command_surface_route_blocked(
    command_id: &'static str,
    target_kind: &'static str,
    route_detail: &'static str,
    label_len: usize,
) {
    emit_message_sent_with_payload(
        CHANNEL_UI_COMMAND_SURFACE_ROUTE_BLOCKED,
        label_len.max(1),
        vec![
            structured_payload_field("source_surface", "command_bar"),
            structured_payload_field("command_id", command_id),
            structured_payload_field("target_kind", target_kind),
            structured_payload_field("route_detail", route_detail),
        ],
    );
}

fn shell_workbench_command_label(command: ShellWorkbenchCommand) -> &'static str {
    match command {
        ShellWorkbenchCommand::OpenCommandPalette => "command_palette_open",
        ShellWorkbenchCommand::CloseCommandPalette => "command_palette_close",
        ShellWorkbenchCommand::ToggleCommandPalette => "command_palette",
        ShellWorkbenchCommand::CloseHelpPanel => "help_panel_close",
        ShellWorkbenchCommand::ToggleHelpPanel => "help_panel",
        ShellWorkbenchCommand::CloseRadialMenu => "radial_menu_close",
        ShellWorkbenchCommand::ToggleRadialMenu => "radial_menu",
        ShellWorkbenchCommand::CycleFocusRegion => "cycle_focus_region",
    }
}

fn emit_workbench_command_requested(command: ShellWorkbenchCommand) {
    emit_event(DiagnosticEvent::MessageSent {
        channel_id: CHANNEL_UI_COMMAND_BAR_WORKBENCH_COMMAND_REQUESTED,
        byte_len: shell_workbench_command_label(command).len(),
    });
}

fn emit_workbench_command_executed(command: ShellWorkbenchCommand) {
    emit_event(DiagnosticEvent::MessageReceived {
        channel_id: CHANNEL_UI_COMMAND_BAR_WORKBENCH_COMMAND_EXECUTED,
        latency_us: shell_workbench_command_label(command).len() as u64,
    });
}

fn emit_workbench_command_blocked_by_focus(command: ShellWorkbenchCommand) {
    emit_event(DiagnosticEvent::MessageSent {
        channel_id: CHANNEL_UI_COMMAND_BAR_WORKBENCH_COMMAND_BLOCKED_BY_FOCUS,
        byte_len: shell_workbench_command_label(command).len(),
    });
}

fn workbench_intent_for_command(command: ShellWorkbenchCommand) -> WorkbenchIntent {
    match command {
        ShellWorkbenchCommand::OpenCommandPalette => WorkbenchIntent::OpenCommandPalette,
        ShellWorkbenchCommand::CloseCommandPalette => WorkbenchIntent::CloseCommandPalette,
        ShellWorkbenchCommand::ToggleCommandPalette => WorkbenchIntent::ToggleCommandPalette,
        ShellWorkbenchCommand::CloseHelpPanel => WorkbenchIntent::CloseHelpPanel,
        ShellWorkbenchCommand::ToggleHelpPanel => WorkbenchIntent::ToggleHelpPanel,
        ShellWorkbenchCommand::CloseRadialMenu => WorkbenchIntent::CloseRadialMenu,
        ShellWorkbenchCommand::ToggleRadialMenu => WorkbenchIntent::ToggleRadialMenu,
        ShellWorkbenchCommand::CycleFocusRegion => WorkbenchIntent::CycleFocusRegion,
    }
}

fn command_requires_focused_pane(command: ShellWorkbenchCommand) -> bool {
    matches!(command, ShellWorkbenchCommand::CycleFocusRegion)
}

pub(crate) fn request_workbench_command(
    graph_app: &mut GraphBrowserApp,
    command: ShellWorkbenchCommand,
    command_bar_focus_target: CommandBarFocusTarget,
) -> bool {
    if matches!(
        command,
        ShellWorkbenchCommand::OpenCommandPalette
            | ShellWorkbenchCommand::CloseCommandPalette
            | ShellWorkbenchCommand::ToggleCommandPalette
    ) {
        emit_command_bar_command_palette_requested();
    }
    emit_workbench_command_requested(command);

    if command_requires_focused_pane(command) && command_bar_focus_target.active_pane().is_none() {
        emit_command_surface_route_blocked(
            shell_workbench_command_label(command),
            "focused_pane",
            "focused_pane_required",
            shell_workbench_command_label(command).len(),
        );
        emit_workbench_command_blocked_by_focus(command);
        return false;
    }

    graph_app.enqueue_workbench_intent(workbench_intent_for_command(command));
    emit_command_surface_route_resolved(
        shell_workbench_command_label(command),
        if command_requires_focused_pane(command) {
            "focused_pane"
        } else {
            "workbench_intent"
        },
        "intent_enqueued",
        shell_workbench_command_label(command).len(),
    );
    emit_workbench_command_executed(command);
    true
}

pub(crate) fn request_command_palette_toggle(graph_app: &mut GraphBrowserApp) {
    let _ = request_workbench_command(
        graph_app,
        ShellWorkbenchCommand::ToggleCommandPalette,
        CommandBarFocusTarget::default(),
    );
}

pub(crate) fn request_command_palette_open(graph_app: &mut GraphBrowserApp) {
    let _ = request_workbench_command(
        graph_app,
        ShellWorkbenchCommand::OpenCommandPalette,
        CommandBarFocusTarget::default(),
    );
}

pub(crate) fn request_command_palette_close(graph_app: &mut GraphBrowserApp) {
    let _ = request_workbench_command(
        graph_app,
        ShellWorkbenchCommand::CloseCommandPalette,
        CommandBarFocusTarget::default(),
    );
}

pub(crate) fn request_help_panel_toggle(
    graph_app: &mut GraphBrowserApp,
    command_bar_focus_target: CommandBarFocusTarget,
) -> bool {
    request_workbench_command(
        graph_app,
        ShellWorkbenchCommand::ToggleHelpPanel,
        command_bar_focus_target,
    )
}

pub(crate) fn request_help_panel_close(graph_app: &mut GraphBrowserApp) -> bool {
    request_workbench_command(
        graph_app,
        ShellWorkbenchCommand::CloseHelpPanel,
        CommandBarFocusTarget::default(),
    )
}

pub(crate) fn request_radial_menu_toggle(
    graph_app: &mut GraphBrowserApp,
    command_bar_focus_target: CommandBarFocusTarget,
) -> bool {
    request_workbench_command(
        graph_app,
        ShellWorkbenchCommand::ToggleRadialMenu,
        command_bar_focus_target,
    )
}

pub(crate) fn request_radial_menu_close(
    graph_app: &mut GraphBrowserApp,
    command_bar_focus_target: CommandBarFocusTarget,
) -> bool {
    request_workbench_command(
        graph_app,
        ShellWorkbenchCommand::CloseRadialMenu,
        command_bar_focus_target,
    )
}

pub(crate) fn request_cycle_focus_region(
    graph_app: &mut GraphBrowserApp,
    command_bar_focus_target: CommandBarFocusTarget,
) -> bool {
    request_workbench_command(
        graph_app,
        ShellWorkbenchCommand::CycleFocusRegion,
        command_bar_focus_target,
    )
}

pub(crate) fn run_nav_action_for_fallback_node(
    graph_app: &mut GraphBrowserApp,
    fallback_node: Option<NodeKey>,
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
            emit_command_surface_route_blocked(
                nav_action_label(action),
                "input_binding",
                "binding_unresolved",
                nav_action_label(action).len(),
            );
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
    let target = BrowserCommandTarget::ChromeProjection { fallback_node };
    graph_app.request_browser_command(target, command);
    true
}

pub(crate) fn run_nav_action(
    graph_app: &mut GraphBrowserApp,
    window: &EmbedderWindow,
    command_bar_focus_target: CommandBarFocusTarget,
    action: ToolbarNavAction,
) -> bool {
    run_nav_action_for_fallback_node(
        graph_app,
        nav_targeting::chrome_projection_node(graph_app, window)
            .or(command_bar_focus_target.focused_node()),
        action,
    )
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
            workbench_intents: Vec::new(),
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
        workbench_intents: submit_result.workbench_intents,
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
    use crate::shell::desktop::host::window::{ChromeProjectionSource, EmbedderWindow};
    use crate::shell::desktop::runtime::diagnostics::{DiagnosticEvent, install_global_sender};
    use crate::shell::desktop::runtime::registries::{
        CHANNEL_UI_COMMAND_BAR_WORKBENCH_COMMAND_BLOCKED_BY_FOCUS,
        CHANNEL_UI_COMMAND_BAR_WORKBENCH_COMMAND_EXECUTED,
        CHANNEL_UI_COMMAND_BAR_WORKBENCH_COMMAND_REQUESTED,
    };
    use std::sync::Arc;
    use std::sync::atomic::AtomicU64;

    fn test_webview_id() -> servo::WebViewId {
        thread_local! {
            static NS_INSTALLED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
        }
        NS_INSTALLED.with(|cell| {
            if !cell.get() {
                base::id::PipelineNamespace::install(base::id::PipelineNamespaceId(46));
                cell.set(true);
            }
        });
        servo::WebViewId::new(base::id::PainterId::next())
    }

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
    fn nav_action_prefers_chrome_projection_fallback_over_focus_target_node() {
        let prefs = AppPreferences::default();
        let window = EmbedderWindow::new(HeadlessWindow::new(&prefs), Arc::new(AtomicU64::new(0)));
        let mut app = GraphBrowserApp::new_for_testing();
        let focused_node =
            app.add_node_and_sync("https://focused.example".into(), euclid::point2(0.0, 0.0));
        let projected_node =
            app.add_node_and_sync("https://projected.example".into(), euclid::point2(10.0, 0.0));
        let projected_renderer = test_webview_id();
        app.map_webview_to_node(projected_renderer, projected_node);
        window.set_chrome_projection_source(Some(ChromeProjectionSource::Renderer(
            projected_renderer,
        )));

        assert!(run_nav_action(
            &mut app,
            &window,
            CommandBarFocusTarget::new(None, Some(focused_node)),
            ToolbarNavAction::StopLoad,
        ));
        assert_eq!(
            app.take_pending_browser_command(),
            Some((
                BrowserCommandTarget::ChromeProjection {
                    fallback_node: Some(projected_node)
                },
                BrowserCommand::StopLoad,
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

    #[test]
    fn cycle_focus_region_blocks_without_active_pane_and_emits_focus_diagnostic() {
        let (diag_tx, diag_rx) = crossbeam_channel::unbounded();
        install_global_sender(diag_tx);
        let mut app = GraphBrowserApp::new_for_testing();

        assert!(!request_cycle_focus_region(
            &mut app,
            CommandBarFocusTarget::new(None, None)
        ));
        assert!(app.take_pending_workbench_intents().is_empty());

        let emitted: Vec<DiagnosticEvent> = diag_rx.try_iter().collect();
        assert!(
            emitted.iter().any(|event| matches!(
                event,
                DiagnosticEvent::MessageSent { channel_id, .. }
                    if *channel_id == CHANNEL_UI_COMMAND_BAR_WORKBENCH_COMMAND_REQUESTED
            )),
            "expected workbench-command requested diagnostic; got: {emitted:?}"
        );
        assert!(
            emitted.iter().any(|event| matches!(
                event,
                DiagnosticEvent::MessageSent { channel_id, .. }
                    if *channel_id == CHANNEL_UI_COMMAND_BAR_WORKBENCH_COMMAND_BLOCKED_BY_FOCUS
            )),
            "expected focus-block diagnostic; got: {emitted:?}"
        );
        assert!(
            emitted.iter().any(|event| matches!(
                event,
                DiagnosticEvent::MessageSentStructured { channel_id, fields, .. }
                    if *channel_id == CHANNEL_UI_COMMAND_SURFACE_ROUTE_BLOCKED
                        && fields.iter().any(|field| field.name == "route_detail" && field.value == "focused_pane_required")
            )),
            "expected structured route-blocked diagnostic; got: {emitted:?}"
        );
    }

    #[test]
    fn help_panel_toggle_enqueues_and_emits_executed_diagnostic() {
        let (diag_tx, diag_rx) = crossbeam_channel::unbounded();
        install_global_sender(diag_tx);
        let mut app = GraphBrowserApp::new_for_testing();

        assert!(request_help_panel_toggle(
            &mut app,
            CommandBarFocusTarget::new(None, None)
        ));
        assert!(matches!(
            app.take_pending_workbench_intents().as_slice(),
            [WorkbenchIntent::ToggleHelpPanel]
        ));

        let emitted: Vec<DiagnosticEvent> = diag_rx.try_iter().collect();
        assert!(
            emitted.iter().any(|event| matches!(
                event,
                DiagnosticEvent::MessageReceived { channel_id, .. }
                    if *channel_id == CHANNEL_UI_COMMAND_BAR_WORKBENCH_COMMAND_EXECUTED
            )),
            "expected executed diagnostic; got: {emitted:?}"
        );
        assert!(
            emitted.iter().any(|event| matches!(
                event,
                DiagnosticEvent::MessageReceivedStructured { channel_id, fields, .. }
                    if *channel_id == CHANNEL_UI_COMMAND_SURFACE_ROUTE_RESOLVED
                        && fields.iter().any(|field| field.name == "command_id" && field.value == "help_panel")
            )),
            "expected structured route-resolved diagnostic; got: {emitted:?}"
        );
    }

    #[test]
    fn command_palette_open_enqueues_and_emits_diagnostics() {
        let (diag_tx, diag_rx) = crossbeam_channel::unbounded();
        install_global_sender(diag_tx);
        let mut app = GraphBrowserApp::new_for_testing();

        request_command_palette_open(&mut app);

        assert!(matches!(
            app.take_pending_workbench_intents().as_slice(),
            [WorkbenchIntent::OpenCommandPalette]
        ));

        let emitted: Vec<DiagnosticEvent> = diag_rx.try_iter().collect();
        assert!(
            emitted.iter().any(|event| matches!(
                event,
                DiagnosticEvent::MessageSent { channel_id, .. }
                    if *channel_id == CHANNEL_UI_COMMAND_BAR_COMMAND_PALETTE_REQUESTED
            )),
            "expected command palette requested diagnostic; got: {emitted:?}"
        );
        assert!(
            emitted.iter().any(|event| matches!(
                event,
                DiagnosticEvent::MessageReceived { channel_id, .. }
                    if *channel_id == CHANNEL_UI_COMMAND_BAR_WORKBENCH_COMMAND_EXECUTED
            )),
            "expected executed diagnostic; got: {emitted:?}"
        );
    }

    #[test]
    fn command_palette_close_enqueues_and_emits_diagnostics() {
        let (diag_tx, diag_rx) = crossbeam_channel::unbounded();
        install_global_sender(diag_tx);
        let mut app = GraphBrowserApp::new_for_testing();

        request_command_palette_close(&mut app);

        assert!(matches!(
            app.take_pending_workbench_intents().as_slice(),
            [WorkbenchIntent::CloseCommandPalette]
        ));

        let emitted: Vec<DiagnosticEvent> = diag_rx.try_iter().collect();
        assert!(
            emitted.iter().any(|event| matches!(
                event,
                DiagnosticEvent::MessageSent { channel_id, .. }
                    if *channel_id == CHANNEL_UI_COMMAND_BAR_COMMAND_PALETTE_REQUESTED
            )),
            "expected command palette requested diagnostic; got: {emitted:?}"
        );
        assert!(
            emitted.iter().any(|event| matches!(
                event,
                DiagnosticEvent::MessageReceived { channel_id, .. }
                    if *channel_id == CHANNEL_UI_COMMAND_BAR_WORKBENCH_COMMAND_EXECUTED
            )),
            "expected executed diagnostic; got: {emitted:?}"
        );
    }
}
