/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use egui_tiles::Tree;
use servo::WebViewId;

use crate::app::GraphBrowserApp;
use crate::graph::NodeKey;
use crate::shell::desktop::host::window::{ChromeProjectionSource, EmbedderWindow};
use crate::shell::desktop::runtime::registries;
use crate::shell::desktop::ui::toolbar::toolbar_ui::CommandBarFocusTarget;
use crate::shell::desktop::workbench::pane_model::PaneId;
use crate::shell::desktop::workbench::tile_kind::TileKind;

/// Resolve the currently active node-pane tile, if any.
pub(crate) fn active_node_pane_node(graph_app: &GraphBrowserApp) -> Option<NodeKey> {
    graph_app
        .workspace
        .graph_runtime
        .active_pane_rects
        .first()
        .map(|(_, node_key, _)| *node_key)
}

pub(crate) fn chrome_projection_node(
    graph_app: &GraphBrowserApp,
    window: &EmbedderWindow,
) -> Option<NodeKey> {
    if let Some(node_key) = focused_embedded_content_node(graph_app) {
        return Some(node_key);
    }

    match window.chrome_projection_source() {
        Some(ChromeProjectionSource::Renderer(renderer_id)) => {
            graph_app.get_node_for_webview(renderer_id)
        }
        Some(ChromeProjectionSource::Pane(pane_id)) => {
            registries::phase1_renderer_attachment_for_pane(pane_id)
                .and_then(|attachment| attachment.node_key)
        }
        None => None,
    }
}

fn focused_embedded_content_node(graph_app: &GraphBrowserApp) -> Option<NodeKey> {
    graph_app
        .embedded_content_focus_webview()
        .and_then(|webview_id| graph_app.get_node_for_webview(webview_id))
}

/// Resolve the current command-bar target for omnibar and viewer-facing controls.
///
/// Preference order:
/// 1) focused tile runtime mapping (if available),
/// 2) active tile node fallback.
pub(crate) fn command_bar_focus_target(
    active_toolbar_pane: Option<PaneId>,
    active_runtime_node: Option<NodeKey>,
    focused_node_key: Option<NodeKey>,
    selected_node: Option<NodeKey>,
) -> CommandBarFocusTarget {
    CommandBarFocusTarget::new(
        active_toolbar_pane,
        focused_node_key.or(active_runtime_node).or(selected_node),
    )
}

/// Resolve the explicit target runtime viewer for navigation commands.
pub(crate) fn nav_target_webview_id(
    graph_app: &GraphBrowserApp,
    focused_toolbar_node: Option<NodeKey>,
) -> Option<WebViewId> {
    focused_toolbar_node.and_then(|node_key| graph_app.get_webview_for_node(node_key))
}

#[cfg(test)]
mod tests {
    use super::*;
    use euclid::default::Point2D;
    use std::sync::Arc;
    use std::sync::atomic::AtomicU64;

    use crate::prefs::AppPreferences;
    use crate::shell::desktop::host::headless_window::HeadlessWindow;
    use crate::shell::desktop::host::window::EmbedderWindow;

    /// Create a unique runtime viewer id for testing.
    fn test_webview_id() -> servo::WebViewId {
        thread_local! {
            static NS_INSTALLED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
        }
        NS_INSTALLED.with(|cell| {
            if !cell.get() {
                base::id::PipelineNamespace::install(base::id::PipelineNamespaceId(45));
                cell.set(true);
            }
        });
        servo::WebViewId::new(base::id::PainterId::next())
    }

    #[test]
    fn test_command_bar_focus_target_prefers_focused_node_input() {
        let mut app = GraphBrowserApp::new_for_testing();
        let a = app.add_node_and_sync("https://a.example".into(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("https://b.example".into(), Point2D::new(10.0, 0.0));
        let pane_id = PaneId::new();
        let a_id = test_webview_id();
        let b_id = test_webview_id();
        app.map_webview_to_node(a_id, a);
        app.map_webview_to_node(b_id, b);

        let chosen = command_bar_focus_target(Some(pane_id), Some(a), Some(b), Some(a));
        assert_eq!(chosen.active_pane(), Some(pane_id));
        assert_eq!(chosen.focused_node(), Some(b));
    }

    #[test]
    fn test_command_bar_focus_target_falls_back_to_active_node() {
        let mut app = GraphBrowserApp::new_for_testing();
        let a = app.add_node_and_sync("https://a.example".into(), Point2D::new(0.0, 0.0));
        let a_id = test_webview_id();
        app.map_webview_to_node(a_id, a);

        let chosen = command_bar_focus_target(None, Some(a), None, None);
        assert_eq!(chosen.active_pane(), None);
        assert_eq!(chosen.focused_node(), Some(a));
    }

    #[test]
    fn test_command_bar_focus_target_falls_back_to_selected_node_when_no_live_focus() {
        let mut app = GraphBrowserApp::new_for_testing();
        let selected =
            app.add_node_and_sync("https://selected.example".into(), Point2D::new(0.0, 0.0));
        let other = app.add_node_and_sync("https://other.example".into(), Point2D::new(10.0, 0.0));
        let other_wv = test_webview_id();
        app.map_webview_to_node(other_wv, other);

        let chosen = command_bar_focus_target(None, None, None, Some(selected));
        assert_eq!(chosen.focused_node(), Some(selected));
    }

    #[test]
    fn test_nav_target_webview_id_resolves_mapping() {
        let mut app = GraphBrowserApp::new_for_testing();
        let a = app.add_node_and_sync("https://a.example".into(), Point2D::new(0.0, 0.0));
        let a_id = test_webview_id();
        app.map_webview_to_node(a_id, a);

        assert_eq!(nav_target_webview_id(&app, Some(a)), Some(a_id));
    }

    #[test]
    fn test_nav_target_webview_id_none_without_mapping() {
        let mut app = GraphBrowserApp::new_for_testing();
        let node = app.add_node_and_sync("https://unmapped.example".into(), Point2D::new(0.0, 0.0));
        assert_eq!(nav_target_webview_id(&app, Some(node)), None);
        assert_eq!(nav_target_webview_id(&app, None), None);
    }

    #[test]
    fn test_active_node_pane_node_uses_canonical_pane_helper() {
        let a = NodeKey::new(100);
        let b = NodeKey::new(101);
        let mut app = GraphBrowserApp::new_for_testing();
        // active_pane_rects is the authoritative source; first entry is the active pane.
        app.workspace.graph_runtime.active_pane_rects = vec![
            (PaneId::new(), b, egui::Rect::ZERO),
            (PaneId::new(), a, egui::Rect::ZERO),
        ];

        assert_eq!(active_node_pane_node(&app), Some(b));
    }

    #[test]
    fn test_focused_embedded_content_node_uses_explicit_app_focus() {
        let mut app = GraphBrowserApp::new_for_testing();
        let node = app.add_node_and_sync("https://focus.example".into(), Point2D::new(0.0, 0.0));
        let webview_id = test_webview_id();
        app.map_webview_to_node(webview_id, node);
        app.set_embedded_content_focus_webview(Some(webview_id));

        assert_eq!(focused_embedded_content_node(&app), Some(node));
    }

    #[test]
    fn test_chrome_projection_node_prefers_embedded_focus_over_renderer_projection() {
        let prefs = AppPreferences::default();
        let window = EmbedderWindow::new(HeadlessWindow::new(&prefs), Arc::new(AtomicU64::new(0)));
        let mut app = GraphBrowserApp::new_for_testing();
        let focused_node =
            app.add_node_and_sync("https://focus.example".into(), Point2D::new(0.0, 0.0));
        let projected_node =
            app.add_node_and_sync("https://projected.example".into(), Point2D::new(10.0, 0.0));
        let focused_renderer = test_webview_id();
        let projected_renderer = test_webview_id();
        app.map_webview_to_node(focused_renderer, focused_node);
        app.map_webview_to_node(projected_renderer, projected_node);
        app.set_embedded_content_focus_webview(Some(focused_renderer));
        window.set_chrome_projection_source(Some(ChromeProjectionSource::Renderer(
            projected_renderer,
        )));

        assert_eq!(chrome_projection_node(&app, &window), Some(focused_node));
    }

    #[test]
    fn test_chrome_projection_node_uses_pane_projection_attachment_without_embedded_focus() {
        let prefs = AppPreferences::default();
        let window = EmbedderWindow::new(HeadlessWindow::new(&prefs), Arc::new(AtomicU64::new(0)));
        let mut app = GraphBrowserApp::new_for_testing();
        let pane_id = PaneId::new();
        let projected_node =
            app.add_node_and_sync("https://pane.example".into(), Point2D::new(0.0, 0.0));
        let projected_renderer = test_webview_id();
        registries::phase1_attach_renderer(pane_id, projected_renderer, Some(projected_node))
            .unwrap();
        window.set_chrome_projection_source(Some(ChromeProjectionSource::Pane(pane_id)));

        assert_eq!(chrome_projection_node(&app, &window), Some(projected_node));

        let detached = registries::phase1_detach_renderer(projected_renderer);
        assert_eq!(detached.map(|attachment| attachment.pane_id), Some(pane_id));
    }
}
