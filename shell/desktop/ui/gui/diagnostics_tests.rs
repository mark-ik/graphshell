use super::*;
use crate::app::WorkbenchIntent;

#[test]
fn graph_surface_focus_state_emits_ux_navigation_transition_on_change() {
    let mut runtime_state = GuiRuntimeState {
        graph_search_open: false,
        graph_search_query: String::new(),
        graph_search_filter_mode: false,
        graph_search_matches: Vec::new(),
        graph_search_active_match_index: None,
        focused_node_hint: Some(NodeKey::new(7)),
        graph_surface_focused: false,
        focus_ring_node_key: None,
        focus_ring_started_at: None,
        focus_ring_duration: Duration::from_millis(500),
        omnibar_search_session: None,
        focus_authority: crate::shell::desktop::ui::gui_state::RuntimeFocusAuthorityState::default(
        ),
        toolbar_drafts: std::collections::HashMap::new(),
        command_palette_toggle_requested: false,
        pending_webview_context_surface_requests: Vec::new(),
        deferred_open_child_webviews: Vec::new(),
    };
    let mut app = GraphBrowserApp::new_for_testing();
    let graph_view = GraphViewId::new();
    let mut diagnostics = crate::shell::desktop::runtime::diagnostics::DiagnosticsState::new();

    apply_graph_surface_focus_state(&mut runtime_state, &mut app, Some(graph_view));

    diagnostics.force_drain_for_tests();
    let snapshot = diagnostics.snapshot_json_for_tests().to_string();
    assert!(
        snapshot.contains("ux:navigation_transition"),
        "expected ux:navigation_transition when graph surface focus changes"
    );
}

#[test]
fn node_focus_state_emits_ux_navigation_transition_on_change() {
    let mut runtime_state = GuiRuntimeState {
        graph_search_open: false,
        graph_search_query: String::new(),
        graph_search_filter_mode: false,
        graph_search_matches: Vec::new(),
        graph_search_active_match_index: None,
        focused_node_hint: None,
        graph_surface_focused: true,
        focus_ring_node_key: None,
        focus_ring_started_at: None,
        focus_ring_duration: Duration::from_millis(500),
        omnibar_search_session: None,
        focus_authority: crate::shell::desktop::ui::gui_state::RuntimeFocusAuthorityState::default(
        ),
        toolbar_drafts: std::collections::HashMap::new(),
        command_palette_toggle_requested: false,
        pending_webview_context_surface_requests: Vec::new(),
        deferred_open_child_webviews: Vec::new(),
    };
    let mut diagnostics = crate::shell::desktop::runtime::diagnostics::DiagnosticsState::new();

    apply_node_focus_state(&mut runtime_state, Some(NodeKey::new(42)));

    diagnostics.force_drain_for_tests();
    let snapshot = diagnostics.snapshot_json_for_tests().to_string();
    assert!(
        snapshot.contains("ux:navigation_transition"),
        "expected ux:navigation_transition when node focus changes"
    );
}

#[test]
fn node_focus_state_noop_does_not_emit_ux_navigation_transition() {
    let focused_node = NodeKey::new(42);
    let mut runtime_state = GuiRuntimeState {
        graph_search_open: false,
        graph_search_query: String::new(),
        graph_search_filter_mode: false,
        graph_search_matches: Vec::new(),
        graph_search_active_match_index: None,
        focused_node_hint: Some(focused_node),
        graph_surface_focused: false,
        focus_ring_node_key: None,
        focus_ring_started_at: None,
        focus_ring_duration: Duration::from_millis(500),
        omnibar_search_session: None,
        focus_authority: crate::shell::desktop::ui::gui_state::RuntimeFocusAuthorityState::default(
        ),
        toolbar_drafts: std::collections::HashMap::new(),
        command_palette_toggle_requested: false,
        pending_webview_context_surface_requests: Vec::new(),
        deferred_open_child_webviews: Vec::new(),
    };
    let mut diagnostics = crate::shell::desktop::runtime::diagnostics::DiagnosticsState::new();

    apply_node_focus_state(&mut runtime_state, Some(focused_node));

    diagnostics.force_drain_for_tests();
    let snapshot = diagnostics.snapshot_json_for_tests().to_string();
    assert!(
        !snapshot.contains("ux:navigation_transition"),
        "did not expect ux:navigation_transition when node focus is unchanged"
    );
}

#[test]
fn graph_surface_focus_state_noop_does_not_emit_ux_navigation_transition() {
    let graph_view = GraphViewId::new();
    let mut runtime_state = GuiRuntimeState {
        graph_search_open: false,
        graph_search_query: String::new(),
        graph_search_filter_mode: false,
        graph_search_matches: Vec::new(),
        graph_search_active_match_index: None,
        focused_node_hint: None,
        graph_surface_focused: true,
        focus_ring_node_key: None,
        focus_ring_started_at: None,
        focus_ring_duration: Duration::from_millis(500),
        omnibar_search_session: None,
        focus_authority: crate::shell::desktop::ui::gui_state::RuntimeFocusAuthorityState::default(
        ),
        toolbar_drafts: std::collections::HashMap::new(),
        command_palette_toggle_requested: false,
        pending_webview_context_surface_requests: Vec::new(),
        deferred_open_child_webviews: Vec::new(),
    };
    let mut app = GraphBrowserApp::new_for_testing();
    app.workspace.graph_runtime.focused_view = Some(graph_view);
    let mut diagnostics = crate::shell::desktop::runtime::diagnostics::DiagnosticsState::new();

    apply_graph_surface_focus_state(&mut runtime_state, &mut app, Some(graph_view));

    diagnostics.force_drain_for_tests();
    let snapshot = diagnostics.snapshot_json_for_tests().to_string();
    assert!(
        !snapshot.contains("ux:navigation_transition"),
        "did not expect ux:navigation_transition when graph surface focus is unchanged"
    );
}

#[test]
fn hosted_settings_route_request_emits_open_decision_and_opens_tool_pane() {
    let mut diagnostics = crate::shell::desktop::runtime::diagnostics::DiagnosticsState::new();
    let mut app = GraphBrowserApp::new_for_testing();
    let graph_view = GraphViewId::new();
    app.ensure_graph_view_registered(graph_view);
    let mut tiles = egui_tiles::Tiles::default();
    let root = tiles.insert_pane(
        TileKind::Graph(crate::shell::desktop::workbench::pane_model::GraphPaneRef::new(
            graph_view,
        )),
    );
    let mut tree = egui_tiles::Tree::new("hosted_settings_route_request", root, tiles);

    apply_requested_settings_route_update(
        &mut app,
        &mut tree,
        crate::util::VersoAddress::settings(crate::util::GraphshellSettingsPath::General)
            .to_string(),
        false,
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
    let root = tiles.insert_pane(
        TileKind::Graph(crate::shell::desktop::workbench::pane_model::GraphPaneRef::new(
            graph_view,
        )),
    );
    let mut tree = egui_tiles::Tree::new("unresolved_settings_route_request", root, tiles);
    let unresolved_url = "verso://settings/not-a-real-route".to_string();

    apply_requested_settings_route_update(&mut app, &mut tree, unresolved_url.clone(), false);

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
