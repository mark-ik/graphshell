use super::super::harness::TestRegistry;
use crate::app::{GraphBrowserApp, GraphIntent};
use crate::shell::desktop::runtime::registries;
use crate::shell::desktop::runtime::registries::protocol::ProtocolResolveControl;
use euclid::default::Point2D;
use servo::ServoUrl;

#[test]
fn phase0_registry_normalization_emits_protocol_and_viewer_success_channels() {
    let mut harness = TestRegistry::new();
    let parsed = ServoUrl::parse("https://example.com/readme.md").expect("url should parse");

    let decision = registries::phase0_decide_navigation_for_tests(
        &harness.diagnostics,
        parsed,
        None,
    );
    let rewritten = decision.normalized_url;
    assert_eq!(rewritten.scheme(), "https");

    let snapshot = harness.snapshot();
    assert!(
        TestRegistry::channel_count(&snapshot, "registry.protocol.resolve_started") > 0,
        "protocol resolve start channel should be emitted"
    );
    assert!(
        TestRegistry::channel_count(&snapshot, "registry.protocol.resolve_succeeded") > 0,
        "protocol resolve success channel should be emitted"
    );
    assert!(
        TestRegistry::channel_count(&snapshot, "registry.viewer.select_started") > 0,
        "viewer select start channel should be emitted"
    );
    assert!(
        TestRegistry::channel_count(&snapshot, "registry.viewer.select_succeeded") > 0,
        "viewer select success channel should be emitted"
    );
}

#[test]
fn phase0_registry_normalization_emits_fallback_channels_for_unknown_scheme_and_extension() {
    let mut harness = TestRegistry::new();
    let parsed = ServoUrl::parse("foo://example.com/archive.unknown").expect("url should parse");

    let decision = registries::phase0_decide_navigation_for_tests(
        &harness.diagnostics,
        parsed,
        None,
    );
    let rewritten = decision.normalized_url;
    assert_eq!(rewritten.scheme(), "https");

    let snapshot = harness.snapshot();
    assert!(
        TestRegistry::channel_count(&snapshot, "registry.protocol.resolve_failed") > 0,
        "unknown scheme should emit protocol resolve failed channel"
    );
    assert!(
        TestRegistry::channel_count(&snapshot, "registry.protocol.fallback_used") > 0,
        "unknown scheme should emit protocol fallback channel"
    );
    assert!(
        TestRegistry::channel_count(&snapshot, "registry.viewer.fallback_used") > 0,
        "unknown extension should emit viewer fallback channel"
    );
}

#[test]
fn phase0_registry_graph_view_normalization_rewrites_unknown_scheme_and_emits_fallback_channels() {
    let mut harness = TestRegistry::new();
    let parsed = ServoUrl::parse("foo://example.com/path").expect("url should parse");

    let rewritten = crate::shell::desktop::runtime::registries::phase0_decide_navigation_for_tests(
        &harness.diagnostics,
        parsed,
        None,
    )
    .normalized_url;

    assert_eq!(rewritten.scheme(), "https");

    let snapshot = harness.snapshot();
    assert!(
        TestRegistry::channel_count(&snapshot, "registry.protocol.resolve_failed") > 0,
        "unknown scheme should emit protocol resolve failed channel"
    );
    assert!(
        TestRegistry::channel_count(&snapshot, "registry.protocol.fallback_used") > 0,
        "unknown scheme should emit protocol fallback channel"
    );
}

#[test]
fn phase0_registry_decision_uses_mime_hint_and_emits_viewer_success_channels() {
    let mut harness = TestRegistry::new();
    let parsed = ServoUrl::parse("https://example.com/file.bin").expect("url should parse");

    let decision = registries::phase0_decide_navigation_for_tests(
        &harness.diagnostics,
        parsed,
        Some("text/csv"),
    );

    assert_eq!(decision.viewer.viewer_id, "viewer:csv");
    assert_eq!(decision.viewer.matched_by, "mime");

    let snapshot = harness.snapshot();
    assert!(
        TestRegistry::channel_count(&snapshot, "registry.viewer.select_started") > 0,
        "viewer select start channel should be emitted"
    );
    assert!(
        TestRegistry::channel_count(&snapshot, "registry.viewer.select_succeeded") > 0,
        "viewer select success channel should be emitted"
    );
}

#[test]
fn phase0_registry_decision_uses_protocol_inferred_mime_hint_when_available() {
    let mut harness = TestRegistry::new();
    let parsed = ServoUrl::parse("data:text/csv,foo,bar").expect("url should parse");

    let decision = registries::phase0_decide_navigation_for_tests(&harness.diagnostics, parsed, None);

    assert_eq!(decision.protocol.inferred_mime_hint.as_deref(), Some("text/csv"));
    assert_eq!(decision.viewer.viewer_id, "viewer:csv");
    assert_eq!(decision.viewer.matched_by, "mime");

    let snapshot = harness.snapshot();
    assert!(
        TestRegistry::channel_count(&snapshot, "registry.protocol.resolve_started") > 0,
        "protocol resolve start channel should be emitted"
    );
    assert!(
        TestRegistry::channel_count(&snapshot, "registry.viewer.select_started") > 0,
        "viewer select start channel should be emitted"
    );
    assert!(
        TestRegistry::channel_count(&snapshot, "registry.viewer.select_succeeded") > 0,
        "viewer select success channel should be emitted"
    );
}

#[test]
fn phase0_registry_decision_selects_settings_viewer_for_graphshell_settings_url() {
    let mut harness = TestRegistry::new();
    let parsed = ServoUrl::parse("graphshell://settings/history").expect("url should parse");

    let decision = registries::phase0_decide_navigation_for_tests(&harness.diagnostics, parsed, None);

    assert_eq!(decision.normalized_url.scheme(), "graphshell");
    assert_eq!(decision.viewer.viewer_id, "viewer:settings");
    assert_eq!(decision.viewer.matched_by, "internal");

    let snapshot = harness.snapshot();
    assert!(
        TestRegistry::channel_count(&snapshot, "registry.protocol.resolve_succeeded") > 0,
        "graphshell scheme should emit protocol resolve success channel"
    );
    assert!(
        TestRegistry::channel_count(&snapshot, "registry.viewer.select_succeeded") > 0,
        "settings viewer selection should emit viewer select success channel"
    );
}

#[test]
fn phase0_registry_cancellation_short_circuits_before_viewer_selection() {
    let mut harness = TestRegistry::new();
    let parsed = ServoUrl::parse("https://example.com/readme.md").expect("url should parse");

    let decision = registries::phase0_decide_navigation_for_tests_with_control(
        &harness.diagnostics,
        parsed,
        None,
        ProtocolResolveControl::cancelled(),
    );

    assert!(decision.is_none());

    let snapshot = harness.snapshot();
    assert!(
        TestRegistry::channel_count(&snapshot, "registry.protocol.resolve_started") > 0,
        "protocol resolve start channel should be emitted"
    );
    assert!(
        TestRegistry::channel_count(&snapshot, "registry.protocol.resolve_failed") > 0,
        "cancelled protocol resolution should emit protocol resolve failed channel"
    );
    assert_eq!(
        TestRegistry::channel_count(&snapshot, "registry.viewer.select_started"),
        0,
        "viewer selection should not start when protocol resolution is cancelled"
    );
}

#[test]
fn phase2_action_registry_omnibox_search_emits_action_channels() {
    let mut harness = TestRegistry::new();
    let mut app = GraphBrowserApp::new_for_testing();
    let key = app
        .workspace
        .graph
        .add_node("https://example.com".into(), Point2D::new(0.0, 0.0));
    if let Some(node) = app.workspace.graph.get_node_mut(key) {
        node.title = "Example Handle".into();
    }

    let intents = registries::phase2_execute_omnibox_node_search_action_for_tests(
        &harness.diagnostics,
        &app,
        "example handle",
    );
    assert!(matches!(
        intents.first(),
        Some(GraphIntent::SelectNode { key: selected, .. }) if *selected == key
    ));

    let snapshot = harness.snapshot();
    assert!(
        TestRegistry::channel_count(&snapshot, "registry.action.execute_started") > 0,
        "action execute start channel should be emitted"
    );
    assert!(
        TestRegistry::channel_count(&snapshot, "registry.action.execute_succeeded") > 0,
        "action execute success channel should be emitted"
    );
    assert_eq!(
        TestRegistry::channel_count(&snapshot, "registry.action.execute_failed"),
        0,
        "action execute failed channel should not be emitted on successful search"
    );
}

#[test]
fn phase2_action_registry_graph_submit_emits_action_channels() {
    let mut harness = TestRegistry::new();
    let mut app = GraphBrowserApp::new_for_testing();
    let key = app
        .workspace
        .graph
        .add_node("https://start.com".into(), Point2D::new(0.0, 0.0));
    app.workspace.selected_nodes.select(key, false);

    let (open_selected_tile, intents) = registries::phase2_execute_graph_view_submit_action_for_tests(
        &harness.diagnostics,
        &app,
        "https://next.com",
    );
    assert!(open_selected_tile);
    assert!(matches!(
        intents.first(),
        Some(GraphIntent::SetNodeUrl { key: selected, new_url })
            if *selected == key && new_url == "https://next.com"
    ));

    let snapshot = harness.snapshot();
    assert!(
        TestRegistry::channel_count(&snapshot, "registry.action.execute_started") > 0,
        "action execute start channel should be emitted"
    );
    assert!(
        TestRegistry::channel_count(&snapshot, "registry.action.execute_succeeded") > 0,
        "action execute success channel should be emitted"
    );
    assert_eq!(
        TestRegistry::channel_count(&snapshot, "registry.action.execute_failed"),
        0,
        "action execute failed channel should not be emitted on successful graph submit"
    );
}

#[test]
fn phase2_action_registry_detail_submit_emits_action_channels() {
    let mut harness = TestRegistry::new();
    let mut app = GraphBrowserApp::new_for_testing();
    let key = app
        .workspace
        .graph
        .add_node("https://start.com".into(), Point2D::new(0.0, 0.0));

    let (open_selected_tile, intents) = registries::phase2_execute_detail_view_submit_action_for_tests(
        &harness.diagnostics,
        &app,
        "https://detail-next.com",
        Some(key),
    );
    assert!(!open_selected_tile);
    assert!(matches!(
        intents.first(),
        Some(GraphIntent::SetNodeUrl { key: selected, new_url })
            if *selected == key && new_url == "https://detail-next.com"
    ));

    let snapshot = harness.snapshot();
    assert!(
        TestRegistry::channel_count(&snapshot, "registry.action.execute_started") > 0,
        "action execute start channel should be emitted"
    );
    assert!(
        TestRegistry::channel_count(&snapshot, "registry.action.execute_succeeded") > 0,
        "action execute success channel should be emitted"
    );
    assert_eq!(
        TestRegistry::channel_count(&snapshot, "registry.action.execute_failed"),
        0,
        "action execute failed channel should not be emitted on successful detail submit"
    );
}

#[test]
fn phase2_input_registry_toolbar_submit_binding_emits_resolved_channel() {
    let mut harness = TestRegistry::new();

    let resolved = registries::phase2_resolve_toolbar_submit_binding_for_tests(&harness.diagnostics);
    assert!(resolved);

    let snapshot = harness.snapshot();
    assert!(
        TestRegistry::channel_count(&snapshot, "registry.input.binding_resolved") > 0,
        "input binding resolved channel should be emitted"
    );
    assert_eq!(
        TestRegistry::channel_count(&snapshot, "registry.input.binding_missing"),
        0,
        "input binding missing channel should not be emitted"
    );
    assert_eq!(
        TestRegistry::channel_count(&snapshot, "registry.input.binding_conflict"),
        0,
        "input binding conflict channel should not be emitted"
    );
}

#[test]
fn phase2_input_registry_toolbar_nav_binding_emits_resolved_channel() {
    let mut harness = TestRegistry::new();

    let resolved = registries::phase2_resolve_input_binding_for_tests(
        &harness.diagnostics,
        registries::input::INPUT_BINDING_TOOLBAR_NAV_RELOAD,
    );
    assert!(resolved);

    let snapshot = harness.snapshot();
    assert!(
        TestRegistry::channel_count(&snapshot, "registry.input.binding_resolved") > 0,
        "input binding resolved channel should be emitted"
    );
    assert_eq!(
        TestRegistry::channel_count(&snapshot, "registry.input.binding_missing"),
        0,
        "input binding missing channel should not be emitted"
    );
}

#[test]
fn phase2_lens_registry_default_id_emits_resolve_succeeded_channel() {
    let mut harness = TestRegistry::new();

    let lens = registries::phase2_resolve_lens_for_tests(
        &harness.diagnostics,
        registries::lens::LENS_ID_DEFAULT,
    );
    assert_eq!(lens.name, "Default");

    let snapshot = harness.snapshot();
    assert!(
        TestRegistry::channel_count(&snapshot, "registry.lens.resolve_succeeded") > 0,
        "lens resolve succeeded channel should be emitted"
    );
    assert!(
        TestRegistry::channel_count(&snapshot, "registry.layout.lookup_succeeded") > 0,
        "layout lookup succeeded channel should be emitted"
    );
    assert!(
        TestRegistry::channel_count(&snapshot, "registry.physics.lookup_succeeded") > 0,
        "physics lookup succeeded channel should be emitted"
    );
    assert!(
        TestRegistry::channel_count(&snapshot, "registry.theme.lookup_succeeded") > 0,
        "theme lookup succeeded channel should be emitted"
    );
    assert_eq!(
        TestRegistry::channel_count(&snapshot, "registry.lens.resolve_failed"),
        0,
        "lens resolve failed channel should not be emitted for default id"
    );
}

#[test]
fn phase2_lens_registry_unknown_id_emits_failed_and_fallback_channels() {
    let mut harness = TestRegistry::new();

    let lens = registries::phase2_resolve_lens_for_tests(&harness.diagnostics, "lens:unknown");
    assert_eq!(lens.name, "Default");

    let snapshot = harness.snapshot();
    assert!(
        TestRegistry::channel_count(&snapshot, "registry.lens.resolve_failed") > 0,
        "lens resolve failed channel should be emitted for unknown id"
    );
    assert!(
        TestRegistry::channel_count(&snapshot, "registry.lens.fallback_used") > 0,
        "lens fallback channel should be emitted for unknown id"
    );
    assert!(
        TestRegistry::channel_count(&snapshot, "registry.layout.lookup_succeeded") > 0,
        "layout lookup should still resolve through fallback-composed lens"
    );
    assert!(
        TestRegistry::channel_count(&snapshot, "registry.physics.lookup_succeeded") > 0,
        "physics lookup should still resolve through fallback-composed lens"
    );
    assert!(
        TestRegistry::channel_count(&snapshot, "registry.theme.lookup_succeeded") > 0,
        "theme lookup should still resolve through fallback-composed lens"
    );
}

#[test]
fn phase2_lens_component_id_resolution_emits_component_fallback_channels() {
    let mut harness = TestRegistry::new();
    let mut lens = crate::app::LensConfig::default();
    lens.physics_id = Some("physics:unknown".to_string());
    lens.layout_id = Some("layout:unknown".to_string());
    lens.theme_id = Some("theme:unknown".to_string());

    let normalized = registries::phase2_resolve_lens_components_for_tests(&harness.diagnostics, &lens);
    assert_eq!(
        normalized.physics_id.as_deref(),
        Some(registries::physics::PHYSICS_ID_DEFAULT)
    );
    assert_eq!(
        normalized.layout_id.as_deref(),
        Some(crate::registries::atomic::layout::LAYOUT_ID_DEFAULT)
    );
    assert_eq!(
        normalized.theme_id.as_deref(),
        Some(crate::registries::atomic::theme::THEME_ID_DEFAULT)
    );

    let snapshot = harness.snapshot();
    assert!(
        TestRegistry::channel_count(&snapshot, "registry.physics.lookup_failed") > 0,
        "physics lookup failed channel should be emitted for unknown id"
    );
    assert!(
        TestRegistry::channel_count(&snapshot, "registry.physics.fallback_used") > 0,
        "physics fallback channel should be emitted for unknown id"
    );
    assert!(
        TestRegistry::channel_count(&snapshot, "registry.layout.lookup_failed") > 0,
        "layout lookup failed channel should be emitted for unknown id"
    );
    assert!(
        TestRegistry::channel_count(&snapshot, "registry.layout.fallback_used") > 0,
        "layout fallback channel should be emitted for unknown id"
    );
    assert!(
        TestRegistry::channel_count(&snapshot, "registry.theme.lookup_failed") > 0,
        "theme lookup failed channel should be emitted for unknown id"
    );
    assert!(
        TestRegistry::channel_count(&snapshot, "registry.theme.fallback_used") > 0,
        "theme fallback channel should be emitted for unknown id"
    );
}

#[test]
fn phase3_identity_registry_sign_success_emits_identity_channels() {
    let mut harness = TestRegistry::new();

    let signature = registries::phase3_sign_identity_payload_for_tests(
        &harness.diagnostics,
        "identity:default",
        b"payload",
    );
    assert!(signature.as_deref().is_some_and(|sig| sig.starts_with("sig:")));

    let snapshot = harness.snapshot();
    assert!(
        TestRegistry::channel_count(&snapshot, "registry.identity.sign_started") > 0,
        "identity sign started channel should be emitted"
    );
    assert!(
        TestRegistry::channel_count(&snapshot, "registry.identity.sign_succeeded") > 0,
        "identity sign succeeded channel should be emitted"
    );
    assert_eq!(
        TestRegistry::channel_count(&snapshot, "registry.identity.sign_failed"),
        0,
        "identity sign failed channel should not be emitted on success"
    );
}

#[test]
fn phase3_identity_registry_key_unavailable_emits_failure_channels() {
    let mut harness = TestRegistry::new();

    let signature = registries::phase3_sign_identity_payload_for_tests(
        &harness.diagnostics,
        "identity:locked",
        b"payload",
    );
    assert_eq!(signature, None);

    let snapshot = harness.snapshot();
    assert!(
        TestRegistry::channel_count(&snapshot, "registry.identity.sign_started") > 0,
        "identity sign started channel should be emitted"
    );
    assert!(
        TestRegistry::channel_count(&snapshot, "registry.identity.sign_failed") > 0,
        "identity sign failed channel should be emitted"
    );
    assert!(
        TestRegistry::channel_count(&snapshot, "registry.identity.key_unavailable") > 0,
        "identity key unavailable channel should be emitted"
    );
}

#[test]
fn diagnostics_channel_config_update_emits_config_changed_channel() {
    let mut harness = TestRegistry::new();
    let config = crate::registries::atomic::diagnostics::ChannelConfig {
        enabled: true,
        sample_rate: 0.5,
        retention_count: 256,
    };

    crate::shell::desktop::runtime::diagnostics::apply_channel_config_update_with_diagnostics(
        &harness.diagnostics,
        &mut harness.app,
        registries::CHANNEL_VIEWER_SELECT_STARTED,
        config,
    );

    let snapshot = harness.snapshot();
    assert!(
        TestRegistry::channel_count(&snapshot, registries::CHANNEL_DIAGNOSTICS_CONFIG_CHANGED) > 0,
        "diagnostics config update should emit config changed channel"
    );
}
