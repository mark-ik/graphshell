use super::*;
use crate::app::WorkbenchIntent;

fn channel_count(snapshot: &serde_json::Value, channel_id: &'static str) -> u64 {
    snapshot["channels"]["message_counts"][channel_id]
        .as_u64()
        .unwrap_or(0)
}

#[test]
fn graph_surface_focus_state_emits_ux_navigation_transition_on_change() {
    let mut runtime_state = GraphshellRuntime::for_testing();
    runtime_state.focused_node_hint = Some(NodeKey::new(7));
    let graph_view = GraphViewId::new();
    let mut diagnostics = crate::shell::desktop::runtime::diagnostics::DiagnosticsState::new();

    apply_graph_surface_focus_state(&mut runtime_state, Some(graph_view));

    diagnostics.force_drain_for_tests();
    let snapshot = diagnostics.snapshot_json_for_tests();
    assert!(
        channel_count(
            &snapshot,
            crate::shell::desktop::runtime::registries::CHANNEL_UX_NAVIGATION_TRANSITION
        ) > 0,
        "expected ux:navigation_transition when graph surface focus changes"
    );
}

#[test]
fn node_focus_state_emits_ux_navigation_transition_on_change() {
    let mut runtime_state = GraphshellRuntime::for_testing();
    runtime_state.graph_surface_focused = true;
    let mut diagnostics = crate::shell::desktop::runtime::diagnostics::DiagnosticsState::new();

    apply_node_focus_state(&mut runtime_state, Some(NodeKey::new(42)));

    diagnostics.force_drain_for_tests();
    let snapshot = diagnostics.snapshot_json_for_tests();
    assert!(
        channel_count(
            &snapshot,
            crate::shell::desktop::runtime::registries::CHANNEL_UX_NAVIGATION_TRANSITION
        ) > 0,
        "expected ux:navigation_transition when node focus changes"
    );
}

#[test]
fn node_focus_state_noop_does_not_emit_ux_navigation_transition() {
    let focused_node = NodeKey::new(42);
    let mut runtime_state = GraphshellRuntime::for_testing();
    runtime_state.focused_node_hint = Some(focused_node);
    let mut diagnostics = crate::shell::desktop::runtime::diagnostics::DiagnosticsState::new();

    apply_node_focus_state(&mut runtime_state, Some(focused_node));

    diagnostics.force_drain_for_tests();
    let snapshot = diagnostics.snapshot_json_for_tests();
    assert!(
        channel_count(
            &snapshot,
            crate::shell::desktop::runtime::registries::CHANNEL_UX_NAVIGATION_TRANSITION
        ) == 0,
        "did not expect ux:navigation_transition when node focus is unchanged"
    );
}

#[test]
fn graph_surface_focus_state_noop_does_not_emit_ux_navigation_transition() {
    let graph_view = GraphViewId::new();
    let mut runtime_state = GraphshellRuntime::for_testing();
    runtime_state.graph_surface_focused = true;
    runtime_state.graph_app.workspace.graph_runtime.focused_view = Some(graph_view);
    let mut diagnostics = crate::shell::desktop::runtime::diagnostics::DiagnosticsState::new();

    apply_graph_surface_focus_state(&mut runtime_state, Some(graph_view));

    diagnostics.force_drain_for_tests();
    let snapshot = diagnostics.snapshot_json_for_tests();
    assert!(
        channel_count(
            &snapshot,
            crate::shell::desktop::runtime::registries::CHANNEL_UX_NAVIGATION_TRANSITION
        ) == 0,
        "did not expect ux:navigation_transition when graph surface focus is unchanged"
    );
}

#[test]
fn hosted_settings_route_request_emits_open_decision_and_opens_tool_pane() {
    let mut diagnostics = crate::shell::desktop::runtime::diagnostics::DiagnosticsState::new();
    let mut app = GraphBrowserApp::new_for_testing();
    let primary_graph_view = GraphViewId::new();
    let secondary_graph_view = GraphViewId::new();
    app.ensure_graph_view_registered(primary_graph_view);
    app.ensure_graph_view_registered(secondary_graph_view);
    let mut tiles = egui_tiles::Tiles::default();
    let primary_graph = tiles.insert_pane(TileKind::Graph(
        crate::shell::desktop::workbench::pane_model::GraphPaneRef::new(primary_graph_view),
    ));
    let secondary_graph = tiles.insert_pane(TileKind::Graph(
        crate::shell::desktop::workbench::pane_model::GraphPaneRef::new(secondary_graph_view),
    ));
    let root = tiles.insert_tab_tile(vec![primary_graph, secondary_graph]);
    let mut tree = egui_tiles::Tree::new("hosted_settings_route_request", root, tiles);

    apply_requested_settings_route_update(
        &mut app,
        &mut tree,
        crate::util::VersoAddress::settings(crate::util::GraphshellSettingsPath::General)
            .to_string(),
    );

    assert!(!app.workspace.chrome_ui.show_settings_overlay);
    assert!(tree.tiles.iter().any(|(_, tile)| matches!(
        tile,
        egui_tiles::Tile::Pane(TileKind::Tool(tool))
            if tool.kind == crate::shell::desktop::workbench::pane_model::ToolPaneState::Settings
    )));
    assert!(app.take_pending_workbench_intents().is_empty());

    diagnostics.force_drain_for_tests();
    let snapshot = diagnostics.snapshot_json_for_tests().to_string();
    assert!(snapshot.contains("ux:open_decision_path"));
    assert!(snapshot.contains("ux:open_decision_reason"));
}

#[test]
fn unresolved_settings_route_request_falls_back_to_open_settings_intent() {
    let mut diagnostics = crate::shell::desktop::runtime::diagnostics::DiagnosticsState::new();
    let mut app = GraphBrowserApp::new_for_testing();
    let graph_view = GraphViewId::new();
    app.ensure_graph_view_registered(graph_view);
    let mut tiles = egui_tiles::Tiles::default();
    let root = tiles.insert_pane(TileKind::Graph(
        crate::shell::desktop::workbench::pane_model::GraphPaneRef::new(graph_view),
    ));
    let mut tree = egui_tiles::Tree::new("unresolved_settings_route_request", root, tiles);
    let unresolved_url = "verso://settings/not-a-real-route".to_string();

    apply_requested_settings_route_update(&mut app, &mut tree, unresolved_url.clone());

    let pending = app.take_pending_workbench_intents();
    assert!(matches!(
        pending.as_slice(),
        [WorkbenchIntent::OpenSettingsUrl { url }] if url == &unresolved_url
    ));

    diagnostics.force_drain_for_tests();
    let snapshot = diagnostics.snapshot_json_for_tests().to_string();
    assert!(snapshot.contains("ux:open_decision_path"));
    assert!(snapshot.contains("ux:open_decision_reason"));
}
