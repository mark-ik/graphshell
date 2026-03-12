/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use egui_tiles::Tree;
use servo::WebViewId;

use crate::app::GraphBrowserApp;
use crate::graph::NodeKey;
use crate::shell::desktop::host::window::{ChromeProjectionSource, EmbedderWindow};
use crate::shell::desktop::runtime::registries;
use crate::shell::desktop::workbench::tile_compositor;
use crate::shell::desktop::workbench::tile_kind::TileKind;

/// Resolve the currently active node-pane tile, if any.
pub(crate) fn active_node_pane_node(tiles_tree: &Tree<TileKind>) -> Option<NodeKey> {
    tile_compositor::active_node_pane(tiles_tree).map(|pane| pane.node_key)
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

/// Resolve which node the toolbar/omnibar should target.
///
/// Preference order:
/// 1) focused tile runtime mapping (if available),
/// 2) active tile node fallback.
pub(crate) fn focused_toolbar_node(
    active_runtime_node: Option<NodeKey>,
    focused_node_key: Option<NodeKey>,
    selected_node: Option<NodeKey>,
) -> Option<NodeKey> {
    focused_node_key.or(active_runtime_node).or(selected_node)
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
    fn test_focused_toolbar_node_prefers_focused_node_input() {
        let mut app = GraphBrowserApp::new_for_testing();
        let a = app.add_node_and_sync("https://a.example".into(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("https://b.example".into(), Point2D::new(10.0, 0.0));
        let a_id = test_webview_id();
        let b_id = test_webview_id();
        app.map_webview_to_node(a_id, a);
        app.map_webview_to_node(b_id, b);

        let chosen = focused_toolbar_node(Some(a), Some(b), Some(a));
        assert_eq!(chosen, Some(b));
    }

    #[test]
    fn test_focused_toolbar_node_falls_back_to_active_node() {
        let mut app = GraphBrowserApp::new_for_testing();
        let a = app.add_node_and_sync("https://a.example".into(), Point2D::new(0.0, 0.0));
        let a_id = test_webview_id();
        app.map_webview_to_node(a_id, a);

        let chosen = focused_toolbar_node(Some(a), None, None);
        assert_eq!(chosen, Some(a));
    }

    #[test]
    fn test_focused_toolbar_node_falls_back_to_selected_node_when_no_live_focus() {
        let mut app = GraphBrowserApp::new_for_testing();
        let selected =
            app.add_node_and_sync("https://selected.example".into(), Point2D::new(0.0, 0.0));
        let other = app.add_node_and_sync("https://other.example".into(), Point2D::new(10.0, 0.0));
        let other_wv = test_webview_id();
        app.map_webview_to_node(other_wv, other);

        let chosen = focused_toolbar_node(None, None, Some(selected));
        assert_eq!(chosen, Some(selected));
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
        let mut tiles = egui_tiles::Tiles::default();
        let a_tile = tiles.insert_pane(TileKind::Node(a.into()));
        let b_tile = tiles.insert_pane(TileKind::Node(b.into()));
        let root = tiles.insert_tab_tile(vec![a_tile, b_tile]);
        let mut tree = Tree::new("nav_targeting_active_node", root, tiles);
        let _ = tree.make_active(
            |_, tile| matches!(tile, egui_tiles::Tile::Pane(TileKind::Node(state)) if state.node == b),
        );

        assert_eq!(active_node_pane_node(&tree), Some(b));
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
}
