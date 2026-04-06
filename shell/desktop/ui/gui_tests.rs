#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::AtomicU64;

    use super::super::{Gui, UpdateFrameStage};
    use base::id::{PIPELINE_NAMESPACE, PainterId, PipelineNamespace, TEST_NAMESPACE};
    use servo::WebViewId;

    use crate::app::GraphBrowserApp;
    use crate::prefs::AppPreferences;
    use crate::shell::desktop::host::headless_window::HeadlessWindow;
    use crate::shell::desktop::host::window::{EmbedderWindow, GraphSemanticEventKind};
    #[cfg(feature = "diagnostics")]
    use crate::shell::desktop::runtime::registries::CHANNEL_UX_EMBEDDED_FOCUS_RECLAIM;
    use crate::shell::desktop::ui::gui_state::{GuiRuntimeState, RuntimeFocusAuthorityState};

    fn test_webview_id() -> WebViewId {
        PIPELINE_NAMESPACE.with(|tls| {
            if tls.get().is_none() {
                PipelineNamespace::install(TEST_NAMESPACE);
            }
        });
        WebViewId::new(PainterId::next())
    }

    #[test]
    fn gui_test_module_compiles() {
        assert!(true);
    }

    #[test]
    fn update_frame_stage_sequence_is_canonical() {
        let sequence = Gui::update_frame_stage_sequence();
        assert!(Gui::is_canonical_update_frame_stage_sequence(sequence));
    }

    #[test]
    fn update_frame_stage_sequence_has_expected_order() {
        let sequence = Gui::update_frame_stage_sequence();
        assert_eq!(sequence.len(), 6);
        assert_eq!(sequence[0], UpdateFrameStage::Prelude);
        assert_eq!(sequence[1], UpdateFrameStage::PreFrameInit);
        assert_eq!(sequence[2], UpdateFrameStage::GraphSearchAndKeyboard);
        assert_eq!(sequence[3], UpdateFrameStage::ToolbarAndGraphSearchWindow);
        assert_eq!(sequence[4], UpdateFrameStage::SemanticAndPostRender);
        assert_eq!(sequence[5], UpdateFrameStage::Finalize);
    }

    #[test]
    fn servo_callback_events_are_enqueue_only_until_reducer_applies_intents() {
        let prefs = AppPreferences::default();
        let shared_seq = Arc::new(AtomicU64::new(0));
        let window = EmbedderWindow::new(HeadlessWindow::new(&prefs), shared_seq);

        let mut app = GraphBrowserApp::new_for_testing();
        let node_key = app.add_node_and_sync(
            "https://before.example".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );
        let webview_id = test_webview_id();
        app.map_webview_to_node(webview_id, node_key);

        window.enqueue_test_graph_event_kind(GraphSemanticEventKind::UrlChanged {
            webview_id,
            new_url: "https://after.example".to_string(),
        });

        let before = app
            .workspace
            .domain
            .graph
            .get_node(node_key)
            .expect("node should exist")
            .url()
            .to_owned();
        assert_eq!(before, "https://before.example");

        let events = window.take_pending_graph_events();
        assert_eq!(events.len(), 1);
        let intents = super::super::graph_intents_from_semantic_events(events);
        assert_eq!(intents.len(), 1);

        let still_before = app
            .workspace
            .domain
            .graph
            .get_node(node_key)
            .expect("node should exist")
            .url()
            .to_owned();
        assert_eq!(still_before, "https://before.example");

        app.apply_reducer_intents(intents);
        let after = app
            .workspace
            .domain
            .graph
            .get_node(node_key)
            .expect("node should exist")
            .url()
            .to_owned();
        assert_eq!(after, "https://after.example");
    }

    #[test]
    fn host_reclaim_clears_embedded_content_focus_authority() {
        let mut app = GraphBrowserApp::new_for_testing();
        let mut runtime_state = GuiRuntimeState {
            graph_search_open: false,
            graph_search_query: String::new(),
            graph_search_filter_mode: false,
            graph_search_matches: Vec::new(),
            graph_search_active_match_index: None,
            focused_node_hint: None,
            graph_surface_focused: false,
            focus_ring_node_key: None,
            focus_ring_started_at: None,
            focus_ring_duration: std::time::Duration::from_millis(500),
            omnibar_search_session: None,
            focus_authority: RuntimeFocusAuthorityState::default(),
            toolbar_drafts: std::collections::HashMap::new(),
            command_palette_toggle_requested: false,
            pending_webview_context_surface_requests: Vec::new(),
            deferred_open_child_webviews: Vec::new(),
        };
        let node_key = app.add_node_and_sync(
            "https://focused.example".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );
        let webview_id = test_webview_id();
        app.map_webview_to_node(webview_id, node_key);
        app.set_embedded_content_focus_webview(Some(webview_id));

        super::super::clear_embedded_content_focus(&mut runtime_state, &mut app);

        assert!(app.embedded_content_focus_webview().is_none());
    }

    #[cfg(feature = "diagnostics")]
    #[test]
    fn clearing_embedded_content_focus_emits_reclaim_diagnostic() {
        let mut diagnostics = crate::shell::desktop::runtime::diagnostics::DiagnosticsState::new();
        let mut app = GraphBrowserApp::new_for_testing();
        let mut runtime_state = GuiRuntimeState {
            graph_search_open: false,
            graph_search_query: String::new(),
            graph_search_filter_mode: false,
            graph_search_matches: Vec::new(),
            graph_search_active_match_index: None,
            focused_node_hint: None,
            graph_surface_focused: false,
            focus_ring_node_key: None,
            focus_ring_started_at: None,
            focus_ring_duration: std::time::Duration::from_millis(500),
            omnibar_search_session: None,
            focus_authority: RuntimeFocusAuthorityState::default(),
            toolbar_drafts: std::collections::HashMap::new(),
            command_palette_toggle_requested: false,
            pending_webview_context_surface_requests: Vec::new(),
            deferred_open_child_webviews: Vec::new(),
        };
        let node_key = app.add_node_and_sync(
            "https://focused.example".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );
        let webview_id = test_webview_id();
        app.map_webview_to_node(webview_id, node_key);
        app.set_embedded_content_focus_webview(Some(webview_id));

        super::super::clear_embedded_content_focus(&mut runtime_state, &mut app);

        diagnostics.force_drain_for_tests();
        let snapshot = diagnostics.snapshot_json_for_tests().to_string();
        assert!(
            snapshot.contains(CHANNEL_UX_EMBEDDED_FOCUS_RECLAIM),
            "expected embedded focus reclaim diagnostics when host-side focus is reclaimed"
        );
    }
}
