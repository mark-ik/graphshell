use super::*;

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
