use super::*;

pub(super) fn handle_open_settings_url_intent(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    url: String,
) -> Option<WorkbenchIntent> {
    let Some(route) = GraphBrowserApp::resolve_settings_route(&url) else {
        emit_open_decision(
            UxOpenDecisionPath::SettingsUrl,
            UxOpenDecisionReason::UnresolvedRoute,
        );
        return Some(WorkbenchIntent::OpenSettingsUrl { url });
    };

    let focused_before = active_tool_surface_return_target(tiles_tree);
    if settings_route_targets_overlay(tiles_tree, &route) {
        maybe_capture_transient_surface_return_target(graph_app, tiles_tree);
    } else {
        maybe_capture_tool_surface_return_target(graph_app, tiles_tree);
    }
    open_settings_route_target(graph_app, tiles_tree, route);

    let focused_after = active_tool_surface_return_target(tiles_tree);
    let transitioned_to_settings_surface = matches!(
        focused_after,
        Some(ToolSurfaceReturnTarget::Tool(ToolPaneState::Settings))
            | Some(ToolSurfaceReturnTarget::Tool(ToolPaneState::HistoryManager))
    ) || graph_app.workspace.chrome_ui.show_settings_overlay;
    if transitioned_to_settings_surface
        && (focused_before != focused_after || graph_app.workspace.chrome_ui.show_settings_overlay)
    {
        emit_event(DiagnosticEvent::MessageReceived {
            channel_id: CHANNEL_UX_NAVIGATION_TRANSITION,
            latency_us: 0,
        });
    }

    emit_open_decision(
        UxOpenDecisionPath::SettingsUrl,
        UxOpenDecisionReason::Routed,
    );

    None
}

pub(super) fn handle_open_frame_url_intent(
    graph_app: &mut GraphBrowserApp,
    url: String,
) -> Option<WorkbenchIntent> {
    let Some(frame_name) = GraphBrowserApp::resolve_frame_route(&url) else {
        emit_open_decision(
            UxOpenDecisionPath::FrameUrl,
            UxOpenDecisionReason::UnresolvedRoute,
        );
        return Some(WorkbenchIntent::OpenFrameUrl { url });
    };

    graph_app.request_restore_frame_snapshot_named(frame_name);
    emit_event(DiagnosticEvent::MessageReceived {
        channel_id: CHANNEL_UX_NAVIGATION_TRANSITION,
        latency_us: 0,
    });
    emit_open_decision(UxOpenDecisionPath::FrameUrl, UxOpenDecisionReason::Routed);
    None
}

pub(super) fn handle_open_tool_url_intent(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    url: String,
) -> Option<WorkbenchIntent> {
    let Some(tool_kind) = GraphBrowserApp::resolve_tool_route(&url) else {
        emit_open_decision(
            UxOpenDecisionPath::ToolUrl,
            UxOpenDecisionReason::UnresolvedRoute,
        );
        return Some(WorkbenchIntent::OpenToolUrl { url });
    };

    if matches!(
        tool_kind,
        ToolPaneState::Settings | ToolPaneState::HistoryManager
    ) {
        maybe_capture_tool_surface_return_target(graph_app, tiles_tree);
    }
    open_or_focus_tool_pane_if_available(tiles_tree, tool_kind);
    emit_event(DiagnosticEvent::MessageReceived {
        channel_id: CHANNEL_UX_NAVIGATION_TRANSITION,
        latency_us: 0,
    });
    emit_open_decision(UxOpenDecisionPath::ToolUrl, UxOpenDecisionReason::Routed);
    None
}

pub(super) fn handle_open_view_url_intent(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    url: String,
) -> Option<WorkbenchIntent> {
    let Some(route) = GraphBrowserApp::resolve_view_route(&url) else {
        emit_open_decision(
            UxOpenDecisionPath::ViewUrl,
            UxOpenDecisionReason::UnresolvedRoute,
        );
        return Some(WorkbenchIntent::OpenViewUrl { url });
    };

    match route {
        crate::app::ViewRouteTarget::GraphPane(view_id) => {
            tile_view_ops::open_or_focus_graph_pane(tiles_tree, view_id);
        }
        crate::app::ViewRouteTarget::Graph(graph_id) => {
            let has_snapshot = graph_app
                .list_named_graph_snapshot_names()
                .into_iter()
                .any(|name| name == graph_id);
            if !has_snapshot {
                emit_open_decision(
                    UxOpenDecisionPath::ViewUrl,
                    UxOpenDecisionReason::TargetMissing,
                );
                return Some(WorkbenchIntent::OpenViewUrl { url });
            }
            graph_app.request_restore_graph_snapshot_named(graph_id);
        }
        crate::app::ViewRouteTarget::Note(note_id) => {
            if graph_app.note_record(note_id).is_none() {
                emit_open_decision(
                    UxOpenDecisionPath::ViewUrl,
                    UxOpenDecisionReason::TargetMissing,
                );
                return Some(WorkbenchIntent::OpenViewUrl { url });
            }
            graph_app.request_open_note_by_id(note_id);
        }
        crate::app::ViewRouteTarget::Node(node_id) => {
            let Some(node_key) = graph_app.domain_graph().get_node_key_by_id(node_id) else {
                emit_open_decision(
                    UxOpenDecisionPath::ViewUrl,
                    UxOpenDecisionReason::TargetMissing,
                );
                return Some(WorkbenchIntent::OpenViewUrl { url });
            };
            tile_view_ops::open_or_focus_node_pane(tiles_tree, graph_app, node_key);
        }
    }
    emit_event(DiagnosticEvent::MessageReceived {
        channel_id: CHANNEL_UX_NAVIGATION_TRANSITION,
        latency_us: 0,
    });
    emit_open_decision(UxOpenDecisionPath::ViewUrl, UxOpenDecisionReason::Routed);
    None
}

pub(super) fn handle_open_graph_url_intent(
    graph_app: &mut GraphBrowserApp,
    url: String,
) -> Option<WorkbenchIntent> {
    let Some(graph_id) = GraphBrowserApp::resolve_graph_route(&url) else {
        emit_open_decision(
            UxOpenDecisionPath::GraphUrl,
            UxOpenDecisionReason::UnresolvedRoute,
        );
        return Some(WorkbenchIntent::OpenGraphUrl { url });
    };

    let has_snapshot = graph_app
        .list_named_graph_snapshot_names()
        .into_iter()
        .any(|name| name == graph_id);
    if !has_snapshot {
        emit_open_decision(
            UxOpenDecisionPath::GraphUrl,
            UxOpenDecisionReason::TargetMissing,
        );
        return Some(WorkbenchIntent::OpenGraphUrl { url });
    }

    graph_app.request_restore_graph_snapshot_named(graph_id);
    emit_event(DiagnosticEvent::MessageReceived {
        channel_id: CHANNEL_UX_NAVIGATION_TRANSITION,
        latency_us: 0,
    });
    emit_open_decision(UxOpenDecisionPath::GraphUrl, UxOpenDecisionReason::Routed);
    None
}

pub(super) fn handle_open_note_url_intent(
    graph_app: &mut GraphBrowserApp,
    url: String,
) -> Option<WorkbenchIntent> {
    let Some(note_id) = GraphBrowserApp::resolve_note_route(&url) else {
        emit_open_decision(
            UxOpenDecisionPath::NoteUrl,
            UxOpenDecisionReason::UnresolvedRoute,
        );
        return Some(WorkbenchIntent::OpenNoteUrl { url });
    };

    if graph_app.note_record(note_id).is_none() {
        emit_open_decision(
            UxOpenDecisionPath::NoteUrl,
            UxOpenDecisionReason::TargetMissing,
        );
        return Some(WorkbenchIntent::OpenNoteUrl { url });
    }

    graph_app.request_open_note_by_id(note_id);
    emit_event(DiagnosticEvent::MessageReceived {
        channel_id: CHANNEL_UX_NAVIGATION_TRANSITION,
        latency_us: 0,
    });
    emit_open_decision(UxOpenDecisionPath::NoteUrl, UxOpenDecisionReason::Routed);
    None
}

pub(super) fn handle_open_node_url_intent(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    url: String,
) -> Option<WorkbenchIntent> {
    let Some(node_id) = GraphBrowserApp::resolve_node_route(&url) else {
        emit_open_decision(
            UxOpenDecisionPath::NodeUrl,
            UxOpenDecisionReason::UnresolvedRoute,
        );
        return Some(WorkbenchIntent::OpenNodeUrl { url });
    };

    let Some(node_key) = graph_app.domain_graph().get_node_key_by_id(node_id) else {
        emit_open_decision(
            UxOpenDecisionPath::NodeUrl,
            UxOpenDecisionReason::TargetMissing,
        );
        return Some(WorkbenchIntent::OpenNodeUrl { url });
    };

    tile_view_ops::open_or_focus_node_pane(tiles_tree, graph_app, node_key);
    emit_event(DiagnosticEvent::MessageReceived {
        channel_id: CHANNEL_UX_NAVIGATION_TRANSITION,
        latency_us: 0,
    });
    emit_open_decision(UxOpenDecisionPath::NodeUrl, UxOpenDecisionReason::Routed);
    None
}

pub(super) fn handle_open_clip_url_intent(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    url: String,
) -> Option<WorkbenchIntent> {
    let Some(clip_id) = GraphBrowserApp::resolve_clip_route(&url) else {
        emit_open_decision(
            UxOpenDecisionPath::ClipUrl,
            UxOpenDecisionReason::UnresolvedRoute,
        );
        return Some(WorkbenchIntent::OpenClipUrl { url });
    };

    maybe_capture_tool_surface_return_target(graph_app, tiles_tree);
    graph_app.request_open_clip_by_id(clip_id);
    open_or_focus_tool_pane_if_available(tiles_tree, ToolPaneState::HistoryManager);
    emit_event(DiagnosticEvent::MessageReceived {
        channel_id: CHANNEL_UX_NAVIGATION_TRANSITION,
        latency_us: 0,
    });
    emit_open_decision(UxOpenDecisionPath::ClipUrl, UxOpenDecisionReason::Routed);
    None
}

fn open_settings_route_target(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    route: crate::app::SettingsRouteTarget,
) {
    let kind = graph_app.apply_settings_route_target(route);
    if matches!(kind, ToolPaneState::Settings) && !settings_tool_pane_exists(tiles_tree) {
        graph_app.open_settings_overlay(graph_app.workspace.chrome_ui.settings_tool_page);
    } else {
        open_or_focus_tool_pane_if_available(tiles_tree, kind);
    }
}

fn settings_route_targets_overlay(
    tiles_tree: &Tree<TileKind>,
    route: &crate::app::SettingsRouteTarget,
) -> bool {
    matches!(route, crate::app::SettingsRouteTarget::Settings(_))
        && !settings_tool_pane_exists(tiles_tree)
}

fn settings_tool_pane_exists(tiles_tree: &Tree<TileKind>) -> bool {
    tiles_tree.tiles.iter().any(|(_, tile)| {
        matches!(
            tile,
            Tile::Pane(TileKind::Tool(tool)) if tool.kind == ToolPaneState::Settings
        )
    })
}
