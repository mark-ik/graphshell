#[cfg(test)]
mod tests {
    use super::super::{EguiHost, UpdateFrameStage};
    use std::sync::Arc;
    use std::sync::atomic::AtomicU64;

    use crate::app::GraphBrowserApp;
    use crate::prefs::AppPreferences;
    use crate::shell::desktop::host::headless_window::HeadlessWindow;
    use crate::shell::desktop::host::window::{EmbedderWindow, WebViewLifecycleEventKind};
    #[cfg(feature = "diagnostics")]
    use crate::shell::desktop::runtime::registries::CHANNEL_UX_EMBEDDED_FOCUS_RECLAIM;
    use crate::shell::desktop::ui::gui_state::{GraphshellRuntime, RuntimeFocusAuthorityState};

    fn test_renderer_id() -> crate::app::RendererId {
        crate::app::renderer_id::test_renderer_id()
    }

    #[test]
    fn gui_test_module_compiles() {
        assert!(true);
    }

    #[test]
    fn update_frame_stage_sequence_is_canonical() {
        let sequence = EguiHost::update_frame_stage_sequence();
        assert!(EguiHost::is_canonical_update_frame_stage_sequence(sequence));
    }

    #[test]
    fn update_frame_stage_sequence_has_expected_order() {
        let sequence = EguiHost::update_frame_stage_sequence();
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
        let renderer_id = test_renderer_id();
        app.map_webview_to_node(renderer_id, node_key);

        window.enqueue_test_graph_event_kind(WebViewLifecycleEventKind::UrlChanged {
            webview_id: renderer_id,
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
        let mut runtime_state = GraphshellRuntime::for_testing();
        let node_key = runtime_state.graph_app.add_node_and_sync(
            "https://focused.example".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );
        let renderer_id = test_renderer_id();
        runtime_state
            .graph_app
            .map_webview_to_node(renderer_id, node_key);
        runtime_state
            .graph_app
            .set_embedded_content_focus_webview(Some(renderer_id));

        super::super::clear_embedded_content_focus(&mut runtime_state);

        assert!(
            runtime_state
                .graph_app
                .embedded_content_focus_webview()
                .is_none()
        );
    }

    #[cfg(feature = "diagnostics")]
    #[test]
    fn clearing_embedded_content_focus_emits_reclaim_diagnostic() {
        let mut diagnostics = crate::shell::desktop::runtime::diagnostics::DiagnosticsState::new();
        let mut runtime_state = GraphshellRuntime::for_testing();
        let node_key = runtime_state.graph_app.add_node_and_sync(
            "https://focused.example".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );
        let renderer_id = test_renderer_id();
        runtime_state
            .graph_app
            .map_webview_to_node(renderer_id, node_key);
        runtime_state
            .graph_app
            .set_embedded_content_focus_webview(Some(renderer_id));

        super::super::clear_embedded_content_focus(&mut runtime_state);

        diagnostics.force_drain_for_tests();
        let snapshot = diagnostics.snapshot_json_for_tests().to_string();
        assert!(
            snapshot.contains(CHANNEL_UX_EMBEDDED_FOCUS_RECLAIM),
            "expected embedded focus reclaim diagnostics when host-side focus is reclaimed"
        );
    }

    // ------------------------------------------------------------------
    // FocusViewModel projection (M4.1 slice 1a)
    //
    // These pin the runtime-side projection of focus state into the
    // host-neutral view model. Hosts — egui today, iced later — paint
    // focus chrome off `FrameViewModel.focus` without reaching into
    // `GraphshellRuntime` fields or re-deriving ring-alpha timing math.
    // ------------------------------------------------------------------

    #[test]
    fn focus_view_model_reflects_runtime_focus_fields() {
        use crate::graph::NodeKey;
        use crate::shell::desktop::workbench::pane_model::PaneId;

        let mut runtime = GraphshellRuntime::for_testing();
        let focused = NodeKey::new(42);

        // §12.6 reconciliation: `focused_node` now comes from
        // `active_pane_rects.first()` (same source as `focused_node_key()`),
        // not `focused_node_hint`. Setting the pane rect is what drives it.
        runtime.graph_surface_focused = false;
        runtime.graph_app.workspace.graph_runtime.active_pane_rects =
            vec![(PaneId::new(), focused, egui::Rect::ZERO)];

        let view_model = runtime.project_view_model();
        assert_eq!(view_model.focus.focused_node, Some(focused));
        assert!(!view_model.focus.graph_surface_focused);
        assert_eq!(view_model.active_pane, Some(focused));
    }

    #[test]
    fn focus_view_model_focused_node_is_none_when_graph_surface_focused() {
        use crate::graph::NodeKey;
        use crate::shell::desktop::workbench::pane_model::PaneId;

        let mut runtime = GraphshellRuntime::for_testing();
        let focused = NodeKey::new(42);

        // Even with a pane rect present, `focused_node` should be None
        // when the graph surface has focus — matches `focused_node_key()`.
        runtime.graph_surface_focused = true;
        runtime.graph_app.workspace.graph_runtime.active_pane_rects =
            vec![(PaneId::new(), focused, egui::Rect::ZERO)];

        let view_model = runtime.project_view_model();
        assert_eq!(view_model.focus.focused_node, None);
        assert!(view_model.focus.graph_surface_focused);
    }

    #[test]
    fn focus_view_model_publishes_focus_ring_when_latched() {
        use crate::graph::NodeKey;
        use crate::shell::desktop::ui::portable_time::portable_now;
        use crate::shell::desktop::workbench::pane_model::PaneId;
        use std::time::Duration;

        let mut runtime = GraphshellRuntime::for_testing();
        let focused = NodeKey::new(7);

        // Simulate a just-latched focus transition: the ring animation
        // is targeted at `focused` and the active pane is on the same
        // node, so the host should see a positive alpha.
        runtime.focused_node_hint = Some(focused);
        runtime.focus_ring_node_key = Some(focused);
        runtime.focus_ring_started_at = Some(portable_now());
        runtime.graph_app.workspace.graph_runtime.active_pane_rects =
            vec![(PaneId::new(), focused, egui::Rect::ZERO)];

        let view_model = runtime.project_view_model();
        let spec = view_model
            .focus
            .focus_ring
            .expect("focus_ring populated when ring node latched");
        assert_eq!(spec.node_key, focused);
        assert!(
            view_model.focus.focus_ring_alpha > 0.0,
            "focus_ring_alpha should be > 0 immediately after the transition"
        );
        assert!(
            view_model.focus.focus_ring_alpha <= 1.0,
            "focus_ring_alpha should not exceed 1.0 (got {})",
            view_model.focus.focus_ring_alpha
        );
    }

    #[test]
    fn focus_view_model_alpha_is_zero_when_ring_targets_different_node() {
        use crate::graph::NodeKey;
        use crate::shell::desktop::ui::portable_time::portable_now;
        use crate::shell::desktop::workbench::pane_model::PaneId;
        use std::time::Duration;

        let mut runtime = GraphshellRuntime::for_testing();
        let focused = NodeKey::new(3);
        let stale_ring_target = NodeKey::new(99);

        runtime.focused_node_hint = Some(focused);
        runtime.focus_ring_node_key = Some(stale_ring_target);
        runtime.focus_ring_started_at = Some(portable_now());
        runtime.graph_app.workspace.graph_runtime.active_pane_rects =
            vec![(PaneId::new(), focused, egui::Rect::ZERO)];

        let view_model = runtime.project_view_model();
        assert_eq!(
            view_model.focus.focus_ring_alpha, 0.0,
            "alpha should be 0 when ring target differs from focused pane"
        );
    }

    // ------------------------------------------------------------------
    // FocusRingSettings integration (M4.1 slice 1d)
    //
    // These pin the new user-configurable surface on
    // `chrome_ui.focus_ring_settings`: the `enabled` kill switch, curve
    // selection flowing through to projection, and the bugfix where
    // `focus_ring: Option<FocusRingSpec>` no longer lingers `Some` after
    // the animation has expired.
    // ------------------------------------------------------------------

    #[test]
    fn focus_view_model_ring_cleared_when_alpha_expires() {
        use crate::app::FocusRingSettings;
        use crate::graph::NodeKey;
        use crate::shell::desktop::workbench::pane_model::PaneId;
        use graphshell_core::time::PortableInstant;

        let mut runtime = GraphshellRuntime::for_testing();
        let focused = NodeKey::new(11);

        runtime.focused_node_hint = Some(focused);
        runtime.focus_ring_node_key = Some(focused);
        // Started at origin; pair with an instant-off duration so the
        // animation is deterministically elapsed regardless of how long
        // the test process has been alive.
        runtime.focus_ring_started_at = Some(PortableInstant::ORIGIN);
        runtime.graph_app.workspace.chrome_ui.focus_ring_settings = FocusRingSettings {
            duration_ms: 0,
            ..FocusRingSettings::default()
        };
        runtime.graph_app.workspace.graph_runtime.active_pane_rects =
            vec![(PaneId::new(), focused, egui::Rect::ZERO)];

        let view_model = runtime.project_view_model();
        assert_eq!(view_model.focus.focus_ring_alpha, 0.0);
        assert!(
            view_model.focus.focus_ring.is_none(),
            "focus_ring Option must clear once the animation has expired \
             (hosts gating on is_some() would otherwise loop forever)"
        );
    }

    #[test]
    fn focus_view_model_honors_disabled_settings() {
        use crate::app::FocusRingSettings;
        use crate::graph::NodeKey;
        use crate::shell::desktop::ui::portable_time::portable_now;
        use crate::shell::desktop::workbench::pane_model::PaneId;
        use std::time::Duration;

        let mut runtime = GraphshellRuntime::for_testing();
        let focused = NodeKey::new(17);

        runtime.focused_node_hint = Some(focused);
        runtime.focus_ring_node_key = Some(focused);
        runtime.focus_ring_started_at = Some(portable_now());
        runtime.graph_app.workspace.graph_runtime.active_pane_rects =
            vec![(PaneId::new(), focused, egui::Rect::ZERO)];
        runtime.graph_app.workspace.chrome_ui.focus_ring_settings = FocusRingSettings {
            enabled: false,
            ..FocusRingSettings::default()
        };

        let view_model = runtime.project_view_model();
        assert_eq!(
            view_model.focus.focus_ring_alpha, 0.0,
            "disabled ring must project alpha = 0 regardless of timing"
        );
        assert!(
            view_model.focus.focus_ring.is_none(),
            "disabled ring must drop the FocusRingSpec so hosts don't paint"
        );
    }

    #[test]
    fn focus_view_model_applies_step_curve() {
        use crate::app::{FocusRingCurve, FocusRingSettings};
        use crate::graph::NodeKey;
        use crate::shell::desktop::ui::portable_time::portable_now;
        use crate::shell::desktop::workbench::pane_model::PaneId;
        use graphshell_core::time::PortableInstant;
        use std::time::Duration;

        let mut runtime = GraphshellRuntime::for_testing();
        let focused = NodeKey::new(23);

        runtime.focused_node_hint = Some(focused);
        runtime.focus_ring_node_key = Some(focused);
        // 250ms into a 1000ms animation: Linear would give ~0.75,
        // Step must give exactly 1.0.
        runtime.focus_ring_started_at =
            Some(PortableInstant(portable_now().millis().saturating_sub(250)));
        runtime.graph_app.workspace.graph_runtime.active_pane_rects =
            vec![(PaneId::new(), focused, egui::Rect::ZERO)];
        runtime.graph_app.workspace.chrome_ui.focus_ring_settings = FocusRingSettings {
            curve: FocusRingCurve::Step,
            ..FocusRingSettings::default()
        };

        let view_model = runtime.project_view_model();
        assert!(
            (view_model.focus.focus_ring_alpha - 1.0).abs() < 1e-3,
            "step curve should hold full alpha mid-animation, got {}",
            view_model.focus.focus_ring_alpha
        );
    }

    #[test]
    fn focus_ring_settings_setter_clamps_duration() {
        use crate::app::GraphBrowserApp;
        use crate::app::{FocusRingCurve, FocusRingSettings};

        let mut app = GraphBrowserApp::new_for_testing();
        app.set_focus_ring_settings(FocusRingSettings {
            enabled: true,
            duration_ms: 999_999, // out of range
            curve: FocusRingCurve::Linear,
            color_override: Some([10, 20, 30]),
        });

        let stored = app.focus_ring_settings();
        assert_eq!(
            stored.duration_ms,
            FocusRingSettings::MAX_DURATION_MS,
            "setter must clamp duration_ms to MAX_DURATION_MS"
        );
        assert_eq!(stored.color_override, Some([10, 20, 30]));
    }

    #[test]
    fn focus_ring_settings_serde_roundtrip_with_defaults() {
        // Old workspaces without this JSON blob must still
        // deserialize cleanly into default settings (via #[serde(default)]).
        use crate::app::FocusRingSettings;

        let json = "{}";
        let parsed: FocusRingSettings =
            serde_json::from_str(json).expect("defaults must cover empty JSON");
        assert_eq!(parsed, FocusRingSettings::default());
    }
}
