#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::AtomicU64;

    use super::super::{Gui, UpdateFrameStage};
    use base::id::{PIPELINE_NAMESPACE, PainterId, PipelineNamespace, TEST_NAMESPACE};
    use servo::WebViewId;

    use crate::app::GraphBrowserApp;
    use crate::shell::desktop::host::headless_window::HeadlessWindow;
    use crate::shell::desktop::host::window::{EmbedderWindow, GraphSemanticEventKind};
    use crate::prefs::AppPreferences;

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
            .graph
            .get_node(node_key)
            .expect("node should exist")
            .url
            .clone();
        assert_eq!(before, "https://before.example");

        let events = window.take_pending_graph_events();
        assert_eq!(events.len(), 1);
        let intents = super::super::graph_intents_from_semantic_events(events);
        assert_eq!(intents.len(), 1);

        let still_before = app
            .workspace
            .graph
            .get_node(node_key)
            .expect("node should exist")
            .url
            .clone();
        assert_eq!(still_before, "https://before.example");

        app.apply_intents(intents);
        let after = app
            .workspace
            .graph
            .get_node(node_key)
            .expect("node should exist")
            .url
            .clone();
        assert_eq!(after, "https://after.example");
    }
}
