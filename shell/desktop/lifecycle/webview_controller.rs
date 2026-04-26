/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Webview lifecycle management for the graph browser.
//!
//! Extracts webview create/destroy/sync logic from gui.rs into focused,
//! testable functions. All Servo WebView operations (create, close,
//! sync to graph nodes) live here.

use std::collections::HashSet;

use servo::WebViewId;

use crate::app::{
    BrowserCommand, BrowserCommandTarget, GraphBrowserApp, GraphIntent, LifecycleCause,
    RuntimeEvent, WorkbenchIntent,
};
use crate::graph::NodeKey;
use crate::parser::location_bar_input_to_url;
#[cfg(any(test, not(feature = "diagnostics")))]
use crate::services::search::fuzzy_match_node_keys;
use crate::shell::desktop::host::window::EmbedderWindow;
use crate::shell::desktop::lifecycle::lifecycle_intents;
use crate::shell::desktop::lifecycle::webview_status_sync::{
    renderer_id_from_servo, servo_webview_id_from_renderer,
};
use crate::shell::desktop::runtime::registries;
use crate::util::{GraphAddress, NodeAddress, NoteAddress, VersoAddress};

#[path = "webview_controller/address_bar_routing.rs"]
mod address_bar_routing;
#[path = "webview_controller/browser_command_routing.rs"]
mod browser_command_routing;
#[path = "webview_controller/webview_mapping_reconcile.rs"]
mod webview_mapping_reconcile;

fn reconcile_mappings_and_selection(
    app: &mut GraphBrowserApp,
    seen_webviews: &HashSet<WebViewId>,
    active_webview: Option<WebViewId>,
) -> Vec<GraphIntent> {
    webview_mapping_reconcile::reconcile_mappings_and_selection(app, seen_webviews, active_webview)
}

fn resolve_active_webview_for_sync(
    app: &GraphBrowserApp,
    window_active_webview: Option<WebViewId>,
) -> Option<WebViewId> {
    webview_mapping_reconcile::resolve_active_webview_for_sync(app, window_active_webview)
}

#[cfg(any(test, not(feature = "diagnostics")))]
fn intents_for_graph_view_address_submit(
    app: &GraphBrowserApp,
    input: &str,
) -> (bool, Vec<GraphIntent>, Vec<WorkbenchIntent>) {
    let input = input.trim();
    if input.is_empty() {
        return (false, Vec::new(), Vec::new());
    }

    if let Some(workbench_intent) = route_intent_for_internal_or_domain_url(input) {
        return (false, Vec::new(), vec![workbench_intent]);
    }

    if let Some(selected_node) = app.get_single_selected_node() {
        (
            true,
            vec![
                crate::app::GraphMutation::SetNodeUrl {
                    key: selected_node,
                    new_url: input.to_string(),
                }
                .into(),
            ],
            Vec::new(),
        )
    } else {
        let position = app.suggested_new_node_position(app.focused_selection().primary());
        (
            true,
            vec![
                crate::app::GraphMutation::CreateNodeAtUrl {
                    url: input.to_string(),
                    position,
                }
                .into(),
            ],
            Vec::new(),
        )
    }
}

#[cfg(any(test, not(feature = "diagnostics")))]
fn intents_for_omnibox_node_search(app: &GraphBrowserApp, query: &str) -> (bool, Vec<GraphIntent>) {
    let query = query.trim();
    if query.is_empty() {
        return (false, Vec::new());
    }
    if let Some(key) = fuzzy_match_node_keys(app.domain_graph(), query)
        .first()
        .copied()
    {
        return (
            true,
            vec![GraphIntent::SelectNode {
                key,
                multi_select: false,
            }],
        );
    }
    (false, Vec::new())
}

/// Sync existing webviews to graph mappings.
///
/// This is now structural-reconciliation only (cleanup + active highlight).
/// Structural graph creation and navigation semantics are handled by Servo
/// delegate events routed through GraphIntent reducer paths.
pub(crate) fn sync_to_graph_intents(
    app: &mut GraphBrowserApp,
    window: &EmbedderWindow,
) -> Vec<GraphIntent> {
    // Track which webviews we've seen (to remove stale mappings later).
    let mut seen_webviews = HashSet::new();
    for (wv_id, _) in window.webviews().into_iter() {
        seen_webviews.insert(wv_id);
    }
    let active = resolve_active_webview_for_sync(
        app,
        app.embedded_content_focus_webview()
            .and_then(servo_webview_id_from_renderer),
    );
    reconcile_mappings_and_selection(app, &seen_webviews, active)
}

fn resolve_browser_command_target(
    app: &GraphBrowserApp,
    window: &EmbedderWindow,
    target: BrowserCommandTarget,
) -> Option<WebViewId> {
    match browser_command_routing::resolve_browser_command_target(app, window, target) {
        browser_command_routing::BrowserCommandRouteOutcome::Resolved(webview_id)
        | browser_command_routing::BrowserCommandRouteOutcome::Fallback(webview_id) => {
            Some(webview_id)
        }
        browser_command_routing::BrowserCommandRouteOutcome::NoTarget => None,
    }
}

pub(crate) fn apply_pending_browser_commands(
    app: &mut GraphBrowserApp,
    window: &EmbedderWindow,
    telemetry: &crate::shell::desktop::ui::command_surface_telemetry::CommandSurfaceTelemetry,
) {
    browser_command_routing::apply_pending_browser_commands(app, window, telemetry)
}

pub(crate) struct AddressBarSubmitOutcome {
    pub mark_clean: bool,
    pub open_selected_tile: bool,
}

pub(crate) struct AddressBarIntentOutcome {
    pub outcome: AddressBarSubmitOutcome,
    pub intents: Vec<GraphIntent>,
    pub workbench_intents: Vec<WorkbenchIntent>,
}

fn graph_intents_from_graph_view_submit_result(
    result: crate::shell::desktop::runtime::registries::Phase2GraphViewSubmitResult,
) -> (bool, Vec<GraphIntent>) {
    (
        result.open_selected_tile,
        result.mutations.into_iter().map(Into::into).collect(),
    )
}

fn graph_intents_from_detail_submit_result(
    result: crate::shell::desktop::runtime::registries::Phase2DetailViewSubmitResult,
) -> (bool, Vec<GraphIntent>) {
    let mut intents: Vec<GraphIntent> = result.mutations.into_iter().map(Into::into).collect();
    intents.extend(result.runtime_events.into_iter().map(Into::into));
    (result.open_selected_tile, intents)
}

fn resolve_detail_submit_target(
    app: &GraphBrowserApp,
    focused_node: Option<NodeKey>,
    preferred_webview: Option<WebViewId>,
) -> (Option<NodeKey>, Option<WebViewId>) {
    address_bar_routing::resolve_detail_submit_target(app, focused_node, preferred_webview)
}

fn workbench_route_intent_for_verso_url(normalized_url: &str) -> Option<WorkbenchIntent> {
    address_bar_routing::workbench_route_intent_for_verso_url(normalized_url)
}

fn route_intent_for_internal_or_domain_url(normalized_url: &str) -> Option<WorkbenchIntent> {
    address_bar_routing::route_intent_for_internal_or_domain_url(normalized_url)
}

pub(crate) fn handle_address_bar_submit_intents(
    app: &GraphBrowserApp,
    url: &str,
    is_graph_view: bool,
    focused_node: Option<NodeKey>,
    window: &EmbedderWindow,
    searchpage: &str,
) -> AddressBarIntentOutcome {
    address_bar_routing::handle_address_bar_submit_intents(
        app,
        url,
        is_graph_view,
        focused_node,
        window,
        searchpage,
    )
}

/// Close webviews associated with the given nodes.
///
/// Call before removing nodes from the graph to ensure the actual
/// Servo webviews are properly closed.
pub(crate) fn close_webviews_for_nodes(
    app: &GraphBrowserApp,
    nodes: &[NodeKey],
    window: &EmbedderWindow,
) -> Vec<GraphIntent> {
    let mut intents = Vec::new();
    for &node_key in nodes {
        if let Some(wv_id) = app.get_webview_for_node(node_key) {
            if let Some(servo_webview_id) = servo_webview_id_from_renderer(wv_id) {
                window.close_webview(servo_webview_id);
            }
            intents.push(RuntimeEvent::UnmapWebview { webview_id: wv_id }.into());
        }
        intents.push(
            lifecycle_intents::demote_node_to_cold(node_key, LifecycleCause::ExplicitClose).into(),
        );
    }
    intents
}

/// Close all current webviews and clear their app mappings.
pub(crate) fn close_all_webviews(
    app: &GraphBrowserApp,
    window: &EmbedderWindow,
) -> Vec<GraphIntent> {
    let mut intents = Vec::new();
    let webviews_to_close: Vec<WebViewId> =
        window.webviews().into_iter().map(|(id, _)| id).collect();
    for wv_id in webviews_to_close {
        window.close_webview(wv_id);
        let renderer_id = renderer_id_from_servo(wv_id);
        intents.push(
            RuntimeEvent::UnmapWebview {
                webview_id: renderer_id,
            }
            .into(),
        );
        if let Some(node_key) = app.get_node_for_webview(renderer_id) {
            intents.push(
                lifecycle_intents::demote_node_to_cold(node_key, LifecycleCause::ExplicitClose)
                    .into(),
            );
        }
    }
    intents
}

#[cfg(test)]
mod tests {
    use super::*;
    use euclid::default::Point2D;

    /// Create a unique WebViewId for testing.
    fn test_webview_id() -> servo::WebViewId {
        thread_local! {
            static NS_INSTALLED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
        }
        NS_INSTALLED.with(|cell| {
            if !cell.get() {
                base::id::PipelineNamespace::install(base::id::PipelineNamespaceId(43));
                cell.set(true);
            }
        });
        servo::WebViewId::new(base::id::PainterId::next())
    }

    fn create_node_via_reducer(
        app: &mut GraphBrowserApp,
        url: &str,
        position: Point2D<f32>,
    ) -> NodeKey {
        app.apply_reducer_intents([GraphIntent::CreateNodeAtUrl {
            url: url.to_string(),
            position,
        }]);
        app.workspace
            .domain
            .graph
            .get_node_by_url(url)
            .map(|(key, _)| key)
            .expect("created node should be discoverable by url")
    }

    #[test]
    fn test_new_node_position_for_context_defaults_to_center_when_empty() {
        let app = GraphBrowserApp::new_for_testing();
        let p = app.suggested_new_node_position(None);
        assert!(p.x.is_finite() && p.y.is_finite());
    }

    #[test]
    fn test_new_node_position_for_context_biases_near_anchor() {
        let mut app = GraphBrowserApp::new_for_testing();
        let anchor = app
            .workspace
            .domain
            .graph
            .add_node("https://anchor.com".into(), Point2D::new(100.0, 200.0));
        let p = app.suggested_new_node_position(Some(anchor));
        assert!(p.x >= 100.0 + 140.0 - 50.0 && p.x <= 100.0 + 140.0 + 50.0);
        assert!(p.y >= 200.0 + 80.0 - 50.0 && p.y <= 200.0 + 80.0 + 50.0);
    }

    #[test]
    fn test_new_node_position_for_context_uses_semantic_anchor_from_selection() {
        let mut app = GraphBrowserApp::new_for_testing();
        let math = app
            .workspace
            .domain
            .graph
            .add_node("https://math.example".into(), Point2D::new(320.0, 240.0));
        let numerical = app
            .workspace
            .domain
            .graph
            .add_node("https://numerical.example".into(), Point2D::new(40.0, 40.0));
        let _ = app
            .workspace
            .domain
            .graph
            .insert_node_tag(math, "udc:51".to_string());
        let _ = app
            .workspace
            .domain
            .graph
            .insert_node_tag(numerical, "udc:519.6".to_string());
        let _ = crate::shell::desktop::runtime::registries::phase3_reconcile_semantics(&mut app);
        app.select_node(numerical, false);

        assert_eq!(app.preferred_new_node_anchor(None), Some(math));

        let p = app.suggested_new_node_position(None);
        assert!(p.x >= 320.0 + 140.0 - 50.0 && p.x <= 320.0 + 140.0 + 50.0);
        assert!(p.y >= 240.0 + 80.0 - 50.0 && p.y <= 240.0 + 80.0 + 50.0);
    }

    #[test]
    fn test_reconcile_mappings_removes_stale_webviews() {
        let mut app = GraphBrowserApp::new_for_testing();
        let n1 = app
            .workspace
            .domain
            .graph
            .add_node("https://a.com".into(), Point2D::new(0.0, 0.0));
        let n2 = app
            .workspace
            .domain
            .graph
            .add_node("https://b.com".into(), Point2D::new(1.0, 1.0));
        let w1 = test_webview_id();
        let w2 = test_webview_id();
        app.map_webview_to_node(renderer_id_from_servo(w1), n1);
        app.map_webview_to_node(renderer_id_from_servo(w2), n2);

        let mut seen = HashSet::new();
        seen.insert(w1);
        let intents = reconcile_mappings_and_selection(&mut app, &seen, Some(w1));
        app.apply_reducer_intents(intents);

        assert_eq!(
            app.get_node_for_webview(renderer_id_from_servo(w1)),
            Some(n1)
        );
        assert_eq!(app.get_node_for_webview(renderer_id_from_servo(w2)), None);
        assert_eq!(app.get_single_selected_node(), Some(n1));
    }

    #[test]
    fn test_reconcile_mappings_preserves_existing_selection() {
        let mut app = GraphBrowserApp::new_for_testing();
        let n1 = app
            .workspace
            .domain
            .graph
            .add_node("https://a.com".into(), Point2D::new(0.0, 0.0));
        let n2 = app
            .workspace
            .domain
            .graph
            .add_node("https://b.com".into(), Point2D::new(1.0, 1.0));
        let w1 = test_webview_id();
        app.map_webview_to_node(renderer_id_from_servo(w1), n1);
        app.select_node(n2, false);

        let mut seen = HashSet::new();
        seen.insert(w1);
        let intents = reconcile_mappings_and_selection(&mut app, &seen, Some(w1));
        app.apply_reducer_intents(intents);

        assert_eq!(app.get_single_selected_node(), Some(n2));
    }

    #[test]
    fn test_apply_graph_view_submit_updates_selected_node_url() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("https://old.com".into(), Point2D::new(0.0, 0.0));
        app.select_node(key, false);

        let (open_selected_tile, intents, workbench_intents) =
            intents_for_graph_view_address_submit(&app, "https://new.com");
        assert!(workbench_intents.is_empty());
        app.apply_reducer_intents(intents);

        let node = app.workspace.domain.graph.get_node(key).unwrap();
        assert_eq!(node.url(), "https://new.com");
        assert!(open_selected_tile);
    }

    #[test]
    fn resolve_active_webview_for_sync_prefers_explicit_embedded_focus() {
        let mut app = GraphBrowserApp::new_for_testing();
        let embedded_focus = test_webview_id();
        let stale_window_focus = test_webview_id();
        app.set_embedded_content_focus_webview(Some(renderer_id_from_servo(embedded_focus)));

        assert_eq!(
            resolve_active_webview_for_sync(&app, Some(stale_window_focus)),
            Some(embedded_focus)
        );
    }

    #[test]
    fn test_apply_graph_view_submit_creates_node_when_none_selected() {
        let mut app = GraphBrowserApp::new_for_testing();
        let before = app.workspace.domain.graph.node_count();

        let (open_selected_tile, intents, workbench_intents) =
            intents_for_graph_view_address_submit(&app, "https://created.com");
        assert!(workbench_intents.is_empty());
        app.apply_reducer_intents(intents);

        assert_eq!(app.workspace.domain.graph.node_count(), before + 1);
        let selected = app.get_single_selected_node().unwrap();
        assert_eq!(
            app.workspace.domain.graph.get_node(selected).unwrap().url(),
            "https://created.com"
        );
        assert!(open_selected_tile);
    }

    #[test]
    fn test_apply_graph_view_submit_handle_search_selects_without_navigation() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("https://example.com".into(), Point2D::new(0.0, 0.0));
        if let Some(node) = app.workspace.domain.graph.get_node_mut(key) {
            node.title = "Example Handle".into();
        }
        let original_url = app
            .workspace
            .domain
            .graph
            .get_node(key)
            .unwrap()
            .url()
            .to_string();

        let (open_selected_tile, intents) = intents_for_omnibox_node_search(&app, "example handle");
        app.apply_reducer_intents(intents);

        assert_eq!(app.get_single_selected_node(), Some(key));
        assert_eq!(
            app.workspace.domain.graph.get_node(key).unwrap().url(),
            original_url
        );
        assert!(open_selected_tile);
    }

    #[cfg(feature = "diagnostics")]
    #[test]
    fn protocol_policy_rewrites_unknown_scheme_to_registry_fallback() {
        let parsed = servo::ServoUrl::parse("foo://example.com/path").expect("url should parse");
        let rewritten = crate::shell::desktop::runtime::registries::phase0_decide_navigation_with_control(
            parsed,
            None,
            crate::shell::desktop::runtime::registries::protocol::ProtocolResolveControl::default(),
        )
        .expect("default protocol resolve control should not cancel")
        .normalized_url;
        assert_eq!(rewritten.scheme(), "https");
        assert_eq!(rewritten.host_str(), Some("example.com"));
    }

    #[cfg(feature = "diagnostics")]
    #[test]
    fn protocol_policy_keeps_supported_scheme_unchanged() {
        let parsed = servo::ServoUrl::parse("https://example.com/path").expect("url should parse");
        let rewritten = crate::shell::desktop::runtime::registries::phase0_decide_navigation_with_control(
            parsed.clone(),
            None,
            crate::shell::desktop::runtime::registries::protocol::ProtocolResolveControl::default(),
        )
        .expect("default protocol resolve control should not cancel")
        .normalized_url;
        assert_eq!(rewritten.as_str(), parsed.as_str());
    }

    #[cfg(feature = "diagnostics")]
    #[test]
    fn phase0_decision_for_tests_rewrites_unknown_scheme() {
        let diagnostics = crate::shell::desktop::runtime::diagnostics::DiagnosticsState::new();
        let parsed = servo::ServoUrl::parse("foo://example.com/path").expect("url should parse");

        let rewritten = crate::shell::desktop::runtime::registries::phase0_decide_navigation_for_tests_with_control(
            &diagnostics,
            parsed,
            None,
            crate::shell::desktop::runtime::registries::protocol::ProtocolResolveControl::default(),
        )
        .expect("default protocol resolve control should not cancel")
        .normalized_url;
        assert_eq!(rewritten.scheme(), "https");
    }

    #[test]
    fn workbench_route_intent_is_emitted_for_graphshell_settings_url() {
        let settings_history =
            crate::util::VersoAddress::settings(crate::util::GraphshellSettingsPath::History)
                .to_string();
        let intent = workbench_route_intent_for_verso_url(&settings_history);
        assert!(matches!(
            intent,
            Some(WorkbenchIntent::OpenSettingsUrl { ref url }) if url == &settings_history
        ));

        let none_intent = workbench_route_intent_for_verso_url("https://example.com");
        assert!(none_intent.is_none());
    }

    #[test]
    fn workbench_route_intent_canonicalizes_legacy_graphshell_settings_url() {
        let legacy_url = "graphshell://settings/history";
        let expected_url = "verso://settings/history";
        let intent = workbench_route_intent_for_verso_url(legacy_url);
        assert!(matches!(
            intent,
            Some(WorkbenchIntent::OpenSettingsUrl { ref url }) if url == expected_url
        ));
    }

    #[test]
    fn workbench_route_intent_is_emitted_for_graphshell_frame_url() {
        let frame_url = crate::util::VersoAddress::frame("frame-123").to_string();
        let intent = workbench_route_intent_for_verso_url(&frame_url);
        assert!(matches!(
            intent,
            Some(WorkbenchIntent::OpenFrameUrl { ref url, .. }) if url == &frame_url
        ));
    }

    #[test]
    fn workbench_route_intent_canonicalizes_legacy_graphshell_frame_url() {
        let legacy_url = "graphshell://frame/frame-legacy";
        let expected_url = "verso://frame/frame-legacy";
        let intent = workbench_route_intent_for_verso_url(legacy_url);
        assert!(matches!(
            intent,
            Some(WorkbenchIntent::OpenFrameUrl { ref url, .. }) if url == expected_url
        ));
    }

    #[test]
    fn workbench_route_intent_is_emitted_for_graphshell_tool_url() {
        let tool_url = crate::util::VersoAddress::tool("history", Some(2)).to_string();
        let intent = workbench_route_intent_for_verso_url(&tool_url);
        assert!(matches!(
            intent,
            Some(WorkbenchIntent::OpenToolUrl { ref url }) if url == &tool_url
        ));
    }

    #[test]
    fn workbench_route_intent_canonicalizes_legacy_graphshell_tool_url() {
        let legacy_url = "graphshell://tool/history/2";
        let expected_url = "verso://tool/history/2";
        let intent = workbench_route_intent_for_verso_url(legacy_url);
        assert!(matches!(
            intent,
            Some(WorkbenchIntent::OpenToolUrl { ref url }) if url == expected_url
        ));
    }

    #[test]
    fn workbench_route_intent_is_emitted_for_graphshell_view_url() {
        let view_url =
            crate::util::VersoAddress::view(uuid::Uuid::new_v4().to_string()).to_string();
        let intent = workbench_route_intent_for_verso_url(&view_url);
        assert!(matches!(
            intent,
            Some(WorkbenchIntent::OpenViewUrl { ref url }) if url == &view_url
        ));
    }

    #[test]
    fn workbench_route_intent_canonicalizes_legacy_graphshell_view_node_url() {
        let node_id = uuid::Uuid::new_v4().to_string();
        let legacy_url = format!("graphshell://view/node/{node_id}");
        let expected_url = format!("verso://view/node/{node_id}");
        let intent = workbench_route_intent_for_verso_url(&legacy_url);
        assert!(matches!(
            intent,
            Some(WorkbenchIntent::OpenViewUrl { ref url }) if url == &expected_url
        ));
    }

    #[test]
    fn workbench_route_intent_canonicalizes_legacy_graphshell_view_note_url() {
        let note_id = uuid::Uuid::new_v4().to_string();
        let legacy_url = format!("graphshell://view/note/{note_id}");
        let expected_url = format!("verso://view/note/{note_id}");
        let intent = workbench_route_intent_for_verso_url(&legacy_url);
        assert!(matches!(
            intent,
            Some(WorkbenchIntent::OpenViewUrl { ref url }) if url == &expected_url
        ));
    }

    #[test]
    fn workbench_route_intent_canonicalizes_legacy_graphshell_view_graph_url() {
        let legacy_url = "graphshell://view/graph/graph-main";
        let expected_url = "verso://view/graph/graph-main";
        let intent = workbench_route_intent_for_verso_url(legacy_url);
        assert!(matches!(
            intent,
            Some(WorkbenchIntent::OpenViewUrl { ref url }) if url == expected_url
        ));
    }

    #[test]
    fn workbench_route_intent_canonicalizes_legacy_graphshell_view_uuid_url() {
        let view_id = uuid::Uuid::new_v4();
        let legacy_url = format!("graphshell://view/{view_id}");
        let expected_url = format!("verso://view/{view_id}");
        let intent = workbench_route_intent_for_verso_url(legacy_url.as_str());
        assert!(matches!(
            intent,
            Some(WorkbenchIntent::OpenViewUrl { ref url }) if url == &expected_url
        ));
    }

    #[test]
    fn route_intent_is_emitted_for_graph_domain_url_with_canonicalization() {
        let intent = route_intent_for_internal_or_domain_url("graph://graph-main");
        assert!(matches!(
            intent,
            Some(WorkbenchIntent::OpenGraphUrl { ref url }) if url == "graph://graph-main"
        ));
    }

    #[test]
    fn route_intent_is_emitted_for_note_domain_url_with_canonicalization() {
        let note_id = uuid::Uuid::new_v4().to_string();
        let raw = format!("notes://{note_id}");
        let intent = route_intent_for_internal_or_domain_url(raw.as_str());
        assert!(matches!(
            intent,
            Some(WorkbenchIntent::OpenNoteUrl { ref url }) if url == &raw
        ));
    }

    #[test]
    fn route_intent_is_emitted_for_node_domain_url_with_canonicalization() {
        let node_id = uuid::Uuid::new_v4().to_string();
        let raw = format!("node://{node_id}");
        let intent = route_intent_for_internal_or_domain_url(raw.as_str());
        assert!(matches!(
            intent,
            Some(WorkbenchIntent::OpenNodeUrl { ref url }) if url == &raw
        ));
    }

    #[test]
    fn workbench_route_intent_is_emitted_for_graphshell_clip_url_with_canonicalization() {
        let intent = workbench_route_intent_for_verso_url("graphshell://clip/clip-123");
        assert!(matches!(
            intent,
            Some(WorkbenchIntent::OpenClipUrl { ref url }) if url == "verso://clip/clip-123"
        ));
    }

    #[test]
    fn graph_view_internal_route_submit_does_not_emit_graph_mutation() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("https://old.com".into(), Point2D::new(0.0, 0.0));
        app.select_node(key, false);

        let (open_selected_tile, intents, workbench_intents) =
            intents_for_graph_view_address_submit(&app, "graphshell://frame/demo-frame");

        assert!(!open_selected_tile);
        assert!(intents.is_empty());
        assert_eq!(workbench_intents.len(), 1);
        assert!(matches!(
            workbench_intents.first(),
            Some(WorkbenchIntent::OpenFrameUrl { url, .. }) if url == "verso://frame/demo-frame"
        ));
    }

    #[test]
    fn graph_view_settings_route_submit_does_not_emit_graph_mutation() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("https://old.com".into(), Point2D::new(0.0, 0.0));
        app.select_node(key, false);

        let (open_selected_tile, intents, workbench_intents) =
            intents_for_graph_view_address_submit(&app, "graphshell://settings/history");

        assert!(!open_selected_tile);
        assert!(intents.is_empty());
        assert_eq!(workbench_intents.len(), 1);
        assert!(matches!(
            workbench_intents.first(),
            Some(WorkbenchIntent::OpenSettingsUrl { url }) if url == "verso://settings/history"
        ));
    }

    #[test]
    fn graph_view_tool_route_submit_does_not_emit_graph_mutation() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("https://old.com".into(), Point2D::new(0.0, 0.0));
        app.select_node(key, false);

        let (open_selected_tile, intents, workbench_intents) =
            intents_for_graph_view_address_submit(&app, "graphshell://tool/history/2");

        assert!(!open_selected_tile);
        assert!(intents.is_empty());
        assert_eq!(workbench_intents.len(), 1);
        assert!(matches!(
            workbench_intents.first(),
            Some(WorkbenchIntent::OpenToolUrl { url }) if url == "verso://tool/history/2"
        ));
    }

    #[test]
    fn graph_view_clip_route_submit_does_not_emit_graph_mutation() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("https://old.com".into(), Point2D::new(0.0, 0.0));
        app.select_node(key, false);

        let (open_selected_tile, intents, workbench_intents) =
            intents_for_graph_view_address_submit(&app, "graphshell://clip/clip-123");

        assert!(!open_selected_tile);
        assert!(intents.is_empty());
        assert_eq!(workbench_intents.len(), 1);
        assert!(matches!(
            workbench_intents.first(),
            Some(WorkbenchIntent::OpenClipUrl { url }) if url == "verso://clip/clip-123"
        ));
    }

    #[test]
    fn graph_view_note_domain_submit_does_not_emit_graph_mutation() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("https://old.com".into(), Point2D::new(0.0, 0.0));
        app.select_node(key, false);
        let note_url = format!("notes://{}", uuid::Uuid::new_v4());

        let (open_selected_tile, intents, workbench_intents) =
            intents_for_graph_view_address_submit(&app, &note_url);

        assert!(!open_selected_tile);
        assert!(intents.is_empty());
        assert_eq!(workbench_intents.len(), 1);
        assert!(matches!(
            workbench_intents.first(),
            Some(WorkbenchIntent::OpenNoteUrl { url }) if url == &note_url
        ));
    }

    #[test]
    fn graph_view_node_domain_submit_does_not_emit_graph_mutation() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("https://old.com".into(), Point2D::new(0.0, 0.0));
        app.select_node(key, false);
        let node_url = format!("node://{}", uuid::Uuid::new_v4());

        let (open_selected_tile, intents, workbench_intents) =
            intents_for_graph_view_address_submit(&app, &node_url);

        assert!(!open_selected_tile);
        assert!(intents.is_empty());
        assert_eq!(workbench_intents.len(), 1);
        assert!(matches!(
            workbench_intents.first(),
            Some(WorkbenchIntent::OpenNodeUrl { url }) if url == &node_url
        ));
    }

    #[test]
    fn graph_view_graph_domain_submit_does_not_emit_graph_mutation() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("https://old.com".into(), Point2D::new(0.0, 0.0));
        app.select_node(key, false);
        let graph_url = "graph://graph-main".to_string();

        let (open_selected_tile, intents, workbench_intents) =
            intents_for_graph_view_address_submit(&app, graph_url.as_str());

        assert!(!open_selected_tile);
        assert!(intents.is_empty());
        assert_eq!(workbench_intents.len(), 1);
        assert!(matches!(
            workbench_intents.first(),
            Some(WorkbenchIntent::OpenGraphUrl { url }) if *url == graph_url
        ));
    }

    #[test]
    fn graph_view_node_view_route_submit_does_not_emit_graph_mutation() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("https://old.com".into(), Point2D::new(0.0, 0.0));
        app.select_node(key, false);
        let node_url = format!("verso://view/node/{}", uuid::Uuid::new_v4());

        let (open_selected_tile, intents, workbench_intents) =
            intents_for_graph_view_address_submit(&app, &node_url);

        assert!(!open_selected_tile);
        assert!(intents.is_empty());
        assert_eq!(workbench_intents.len(), 1);
        assert!(matches!(
            workbench_intents.first(),
            Some(WorkbenchIntent::OpenViewUrl { url }) if url == &node_url
        ));
    }

    #[test]
    fn graph_view_note_view_route_submit_does_not_emit_graph_mutation() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("https://old.com".into(), Point2D::new(0.0, 0.0));
        app.select_node(key, false);
        let note_url = format!("verso://view/note/{}", uuid::Uuid::new_v4());

        let (open_selected_tile, intents, workbench_intents) =
            intents_for_graph_view_address_submit(&app, &note_url);

        assert!(!open_selected_tile);
        assert!(intents.is_empty());
        assert_eq!(workbench_intents.len(), 1);
        assert!(matches!(
            workbench_intents.first(),
            Some(WorkbenchIntent::OpenViewUrl { url }) if url == &note_url
        ));
    }

    #[test]
    fn graph_view_graph_view_route_submit_does_not_emit_graph_mutation() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("https://old.com".into(), Point2D::new(0.0, 0.0));
        app.select_node(key, false);
        let graph_url = "verso://view/graph/graph-main".to_string();

        let (open_selected_tile, intents, workbench_intents) =
            intents_for_graph_view_address_submit(&app, graph_url.as_str());

        assert!(!open_selected_tile);
        assert!(intents.is_empty());
        assert_eq!(workbench_intents.len(), 1);
        assert!(matches!(
            workbench_intents.first(),
            Some(WorkbenchIntent::OpenViewUrl { url }) if url == &graph_url
        ));
    }

    #[test]
    fn graph_view_legacy_view_node_submit_does_not_emit_graph_mutation() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("https://old.com".into(), Point2D::new(0.0, 0.0));
        app.select_node(key, false);
        let node_id = uuid::Uuid::new_v4();
        let legacy_url = format!("graphshell://view/node/{node_id}");
        let expected_url = format!("verso://view/node/{node_id}");

        let (open_selected_tile, intents, workbench_intents) =
            intents_for_graph_view_address_submit(&app, legacy_url.as_str());

        assert!(!open_selected_tile);
        assert!(intents.is_empty());
        assert_eq!(workbench_intents.len(), 1);
        assert!(matches!(
            workbench_intents.first(),
            Some(WorkbenchIntent::OpenViewUrl { url }) if url == &expected_url
        ));
    }

    #[test]
    fn graph_view_legacy_view_note_submit_does_not_emit_graph_mutation() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("https://old.com".into(), Point2D::new(0.0, 0.0));
        app.select_node(key, false);
        let note_id = uuid::Uuid::new_v4();
        let legacy_url = format!("graphshell://view/note/{note_id}");
        let expected_url = format!("verso://view/note/{note_id}");

        let (open_selected_tile, intents, workbench_intents) =
            intents_for_graph_view_address_submit(&app, legacy_url.as_str());

        assert!(!open_selected_tile);
        assert!(intents.is_empty());
        assert_eq!(workbench_intents.len(), 1);
        assert!(matches!(
            workbench_intents.first(),
            Some(WorkbenchIntent::OpenViewUrl { url }) if url == &expected_url
        ));
    }

    #[test]
    fn graph_view_legacy_view_graph_submit_does_not_emit_graph_mutation() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("https://old.com".into(), Point2D::new(0.0, 0.0));
        app.select_node(key, false);
        let legacy_url = "graphshell://view/graph/graph-main".to_string();
        let expected_url = "verso://view/graph/graph-main".to_string();

        let (open_selected_tile, intents, workbench_intents) =
            intents_for_graph_view_address_submit(&app, legacy_url.as_str());

        assert!(!open_selected_tile);
        assert!(intents.is_empty());
        assert_eq!(workbench_intents.len(), 1);
        assert!(matches!(
            workbench_intents.first(),
            Some(WorkbenchIntent::OpenViewUrl { url }) if url == &expected_url
        ));
    }

    #[test]
    fn graph_view_legacy_view_uuid_submit_does_not_emit_graph_mutation() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("https://old.com".into(), Point2D::new(0.0, 0.0));
        app.select_node(key, false);
        let view_id = uuid::Uuid::new_v4();
        let legacy_url = format!("graphshell://view/{view_id}");
        let expected_url = format!("verso://view/{view_id}");

        let (open_selected_tile, intents, workbench_intents) =
            intents_for_graph_view_address_submit(&app, legacy_url.as_str());

        assert!(!open_selected_tile);
        assert!(intents.is_empty());
        assert_eq!(workbench_intents.len(), 1);
        assert!(matches!(
            workbench_intents.first(),
            Some(WorkbenchIntent::OpenViewUrl { url }) if url == &expected_url
        ));
    }

    #[test]
    fn resolve_detail_submit_target_prefers_focused_node_mapping() {
        let mut app = GraphBrowserApp::new_for_testing();
        let focused =
            create_node_via_reducer(&mut app, "https://focused.example", Point2D::new(0.0, 0.0));
        let other =
            create_node_via_reducer(&mut app, "https://other.example", Point2D::new(20.0, 0.0));
        let focused_webview = test_webview_id();
        let preferred_webview = test_webview_id();
        app.map_webview_to_node(renderer_id_from_servo(focused_webview), focused);
        app.map_webview_to_node(renderer_id_from_servo(preferred_webview), other);

        let (target_node, target_webview) =
            resolve_detail_submit_target(&app, Some(focused), Some(preferred_webview));
        assert_eq!(target_node, Some(focused));
        assert_eq!(target_webview, Some(focused_webview));
    }

    #[test]
    fn resolve_detail_submit_target_falls_back_to_preferred_webview_mapping() {
        let mut app = GraphBrowserApp::new_for_testing();
        let node = create_node_via_reducer(
            &mut app,
            "https://preferred.example",
            Point2D::new(0.0, 0.0),
        );
        let preferred_webview = test_webview_id();
        app.map_webview_to_node(renderer_id_from_servo(preferred_webview), node);

        let (target_node, target_webview) =
            resolve_detail_submit_target(&app, None, Some(preferred_webview));
        assert_eq!(target_node, Some(node));
        assert_eq!(target_webview, Some(preferred_webview));
    }

    #[test]
    fn resolve_detail_submit_target_uses_preferred_webview_without_mapping() {
        let app = GraphBrowserApp::new_for_testing();
        let preferred_webview = test_webview_id();

        let (target_node, target_webview) =
            resolve_detail_submit_target(&app, None, Some(preferred_webview));
        assert_eq!(target_node, None);
        assert_eq!(target_webview, Some(preferred_webview));
    }

    #[test]
    fn resolve_detail_submit_target_falls_back_when_focused_node_is_stale_after_transition() {
        let mut app = GraphBrowserApp::new_for_testing();
        let stale_focused = create_node_via_reducer(
            &mut app,
            "https://stale-focused.example",
            Point2D::new(0.0, 0.0),
        );
        let remaining = create_node_via_reducer(
            &mut app,
            "https://remaining.example",
            Point2D::new(20.0, 0.0),
        );
        let remaining_webview = test_webview_id();
        app.map_webview_to_node(renderer_id_from_servo(remaining_webview), remaining);

        let (target_node, target_webview) =
            resolve_detail_submit_target(&app, Some(stale_focused), Some(remaining_webview));
        assert_eq!(
            target_node,
            Some(stale_focused),
            "focused node remains authoritative even when its webview mapping is absent"
        );
        assert_eq!(
            target_webview, None,
            "stale focused mapping should not force a stale webview target"
        );

        let (fallback_node, fallback_webview) =
            resolve_detail_submit_target(&app, None, Some(remaining_webview));
        assert_eq!(fallback_node, Some(remaining));
        assert_eq!(fallback_webview, Some(remaining_webview));
    }

    #[test]
    fn resolve_detail_submit_target_prefers_focused_mapping_when_preferred_webview_is_stale() {
        let mut app = GraphBrowserApp::new_for_testing();
        let focused =
            create_node_via_reducer(&mut app, "https://focused.example", Point2D::new(0.0, 0.0));
        let focused_webview = test_webview_id();
        let stale_preferred_webview = test_webview_id();
        app.map_webview_to_node(renderer_id_from_servo(focused_webview), focused);

        let (target_node, target_webview) =
            resolve_detail_submit_target(&app, Some(focused), Some(stale_preferred_webview));
        assert_eq!(target_node, Some(focused));
        assert_eq!(target_webview, Some(focused_webview));
    }
}
