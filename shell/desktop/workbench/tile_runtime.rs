/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use egui_tiles::{Tile, Tree};
use servo::{OffscreenRenderingContext, WebViewId};

use crate::app::{GraphBrowserApp, GraphIntent, LifecycleCause};
use crate::graph::{NodeKey, NodeLifecycle};
use crate::shell::desktop::host::window::EmbedderWindow;
use crate::shell::desktop::lifecycle::lifecycle_intents;
use crate::shell::desktop::workbench::pane_model::{NodePaneState, TileRenderMode};
use crate::shell::desktop::workbench::tile_kind::TileKind;

pub(crate) struct TileCoordinator;

impl TileCoordinator {
    fn render_mode_for_viewer_id(viewer_id: &str) -> TileRenderMode {
        match viewer_id {
            // `viewer:servo` remains as a legacy compatibility alias for persisted overrides.
            "viewer:webview" | "viewer:servo" => TileRenderMode::CompositedTexture,
            "viewer:wry" => TileRenderMode::NativeOverlay,
            "viewer:plaintext" | "viewer:markdown" | "viewer:pdf" | "viewer:csv"
            | "viewer:settings" | "viewer:metadata" => TileRenderMode::EmbeddedEgui,
            _ => TileRenderMode::Placeholder,
        }
    }

    pub(crate) fn viewer_id_uses_composited_runtime(viewer_id: &str) -> bool {
        Self::render_mode_for_viewer_id(viewer_id) == TileRenderMode::CompositedTexture
    }

    fn node_pane_effective_viewer_id<'a>(
        state: &'a NodePaneState,
        graph_app: &GraphBrowserApp,
    ) -> Option<&'a str> {
        if let Some(viewer_id_override) = state.viewer_id_override.as_ref() {
            return Some(viewer_id_override.as_str());
        }

        let node = graph_app.workspace.graph.get_node(state.node)?;
        Some(
            crate::registries::atomic::viewer::ViewerRegistry::default()
                .select_for(node.mime_hint.as_deref(), node.address_kind),
        )
    }

    fn resolve_node_pane_render_mode(
        state: &NodePaneState,
        graph_app: &GraphBrowserApp,
    ) -> TileRenderMode {
        Self::node_pane_effective_viewer_id(state, graph_app)
            .map(Self::render_mode_for_viewer_id)
            .unwrap_or(TileRenderMode::Placeholder)
    }

    fn node_pane_uses_composited_runtime_impl(
        state: &NodePaneState,
        graph_app: &GraphBrowserApp,
    ) -> bool {
        Self::resolve_node_pane_render_mode(state, graph_app) == TileRenderMode::CompositedTexture
    }

    fn collect_node_pane_keys_using_composited_runtime(
        tiles_tree: &Tree<TileKind>,
        graph_app: &GraphBrowserApp,
    ) -> HashSet<NodeKey> {
        tiles_tree
            .tiles
            .iter()
            .filter_map(|(_, tile)| match tile {
                Tile::Pane(TileKind::Node(state))
                    if Self::node_pane_uses_composited_runtime_impl(state, graph_app) =>
                {
                    Some(state.node)
                }
                _ => None,
            })
            .collect()
    }

    fn should_preserve_runtime_webview(
        node_exists: bool,
        mapped_webview: Option<WebViewId>,
    ) -> bool {
        node_exists && mapped_webview.is_some()
    }

    pub(crate) fn reset_runtime_webview_state(
        tiles_tree: &mut Tree<TileKind>,
        tile_rendering_contexts: &mut HashMap<NodeKey, Rc<OffscreenRenderingContext>>,
        tile_favicon_textures: &mut HashMap<NodeKey, (u64, egui::TextureHandle)>,
        favicon_textures: &mut HashMap<WebViewId, (egui::TextureHandle, egui::load::SizedTexture)>,
    ) {
        tile_rendering_contexts.clear();
        tile_favicon_textures.clear();
        favicon_textures.clear();
        Self::remove_all_node_panes(tiles_tree);
    }

    pub(crate) fn has_any_node_panes(tiles_tree: &Tree<TileKind>) -> bool {
        tiles_tree
            .tiles
            .iter()
            .any(|(_, tile)| matches!(tile, Tile::Pane(TileKind::Node(_))))
    }

    pub(crate) fn all_node_pane_keys(tiles_tree: &Tree<TileKind>) -> HashSet<NodeKey> {
        tiles_tree
            .tiles
            .iter()
            .filter_map(|(_, tile)| match tile {
                Tile::Pane(TileKind::Node(state)) => Some(state.node),
                _ => None,
            })
            .collect()
    }

    pub(crate) fn all_node_pane_keys_using_composited_runtime(
        tiles_tree: &Tree<TileKind>,
        graph_app: &GraphBrowserApp,
    ) -> HashSet<NodeKey> {
        Self::collect_node_pane_keys_using_composited_runtime(tiles_tree, graph_app)
    }

    pub(crate) fn node_pane_uses_composited_runtime(
        state: &NodePaneState,
        graph_app: &GraphBrowserApp,
    ) -> bool {
        Self::node_pane_uses_composited_runtime_impl(state, graph_app)
    }

    pub(crate) fn refresh_node_pane_render_modes(
        tiles_tree: &mut Tree<TileKind>,
        graph_app: &GraphBrowserApp,
    ) {
        for (_, tile) in tiles_tree.tiles.iter_mut() {
            if let Tile::Pane(TileKind::Node(state)) = tile {
                state.render_mode = Self::resolve_node_pane_render_mode(state, graph_app);
            }
        }
    }

    pub(crate) fn prune_stale_node_pane_keys_only(
        tiles_tree: &mut Tree<TileKind>,
        graph_app: &GraphBrowserApp,
    ) {
        let stale_nodes: Vec<_> = Self::all_node_pane_keys(tiles_tree)
            .into_iter()
            .filter(|node_key| graph_app.workspace.graph.get_node(*node_key).is_none())
            .collect();
        for node_key in stale_nodes {
            Self::remove_node_pane_for_node(tiles_tree, node_key);
        }
    }

    pub(crate) fn remove_all_node_panes(tiles_tree: &mut Tree<TileKind>) {
        let tile_ids: Vec<_> = tiles_tree
            .tiles
            .iter()
            .filter_map(|(tile_id, tile)| match tile {
                Tile::Pane(TileKind::Node(_)) => Some(*tile_id),
                _ => None,
            })
            .collect();
        for tile_id in tile_ids {
            tiles_tree.remove_recursively(tile_id);
        }
    }

    pub(crate) fn remove_node_pane_for_node(tiles_tree: &mut Tree<TileKind>, node_key: NodeKey) {
        let tile_ids: Vec<_> = tiles_tree
            .tiles
            .iter()
            .filter_map(|(tile_id, tile)| match tile {
                Tile::Pane(TileKind::Node(state)) if state.node == node_key => Some(*tile_id),
                _ => None,
            })
            .collect();
        for tile_id in tile_ids {
            tiles_tree.remove_recursively(tile_id);
        }
    }

    pub(crate) fn prune_stale_node_panes(
        tiles_tree: &mut Tree<TileKind>,
        graph_app: &mut GraphBrowserApp,
        window: &EmbedderWindow,
        tile_rendering_contexts: &mut HashMap<NodeKey, Rc<OffscreenRenderingContext>>,
        lifecycle_intents: &mut Vec<GraphIntent>,
    ) {
        let stale_nodes: Vec<_> = Self::all_node_pane_keys(tiles_tree)
            .into_iter()
            .filter(|node_key| graph_app.workspace.graph.get_node(*node_key).is_none())
            .collect();

        for node_key in stale_nodes {
            Self::remove_node_pane_for_node(tiles_tree, node_key);
            Self::release_node_runtime_for_pane(
                graph_app,
                window,
                tile_rendering_contexts,
                node_key,
                lifecycle_intents,
            );
        }
    }

    pub(crate) fn release_node_runtime_for_pane(
        graph_app: &mut GraphBrowserApp,
        window: &EmbedderWindow,
        tile_rendering_contexts: &mut HashMap<NodeKey, Rc<OffscreenRenderingContext>>,
        node_key: NodeKey,
        lifecycle_intents: &mut Vec<GraphIntent>,
    ) {
        let node_exists = graph_app.workspace.graph.get_node(node_key).is_some();
        let mapped_webview = graph_app.get_webview_for_node(node_key);

        if mapped_webview.is_none() {
            tile_rendering_contexts.remove(&node_key);
            return;
        }

        if Self::should_preserve_runtime_webview(node_exists, mapped_webview) {
            let lifecycle = graph_app
                .workspace
                .graph
                .get_node(node_key)
                .map(|node| node.lifecycle)
                .unwrap_or(NodeLifecycle::Cold);
            if lifecycle != NodeLifecycle::Warm {
                lifecycle_intents.push(lifecycle_intents::demote_node_to_warm(
                    node_key,
                    LifecycleCause::WorkspaceRetention,
                ));
            }
            return;
        }

        tile_rendering_contexts.remove(&node_key);

        if let Some(wv_id) = mapped_webview {
            window.close_webview(wv_id);
            lifecycle_intents.push(GraphIntent::UnmapWebview { webview_id: wv_id });
        }
        lifecycle_intents.push(lifecycle_intents::demote_node_to_cold(
            node_key,
            LifecycleCause::NodeRemoval,
        ));
    }
}

pub(crate) fn viewer_id_uses_composited_runtime(viewer_id: &str) -> bool {
    TileCoordinator::viewer_id_uses_composited_runtime(viewer_id)
}

pub(crate) fn reset_runtime_webview_state(
    tiles_tree: &mut Tree<TileKind>,
    tile_rendering_contexts: &mut HashMap<NodeKey, Rc<OffscreenRenderingContext>>,
    tile_favicon_textures: &mut HashMap<NodeKey, (u64, egui::TextureHandle)>,
    favicon_textures: &mut HashMap<WebViewId, (egui::TextureHandle, egui::load::SizedTexture)>,
) {
    TileCoordinator::reset_runtime_webview_state(
        tiles_tree,
        tile_rendering_contexts,
        tile_favicon_textures,
        favicon_textures,
    );
}

pub(crate) fn has_any_node_panes(tiles_tree: &Tree<TileKind>) -> bool {
    TileCoordinator::has_any_node_panes(tiles_tree)
}

pub(crate) fn all_node_pane_keys(tiles_tree: &Tree<TileKind>) -> HashSet<NodeKey> {
    TileCoordinator::all_node_pane_keys(tiles_tree)
}

pub(crate) fn all_node_pane_keys_using_composited_runtime(
    tiles_tree: &Tree<TileKind>,
    graph_app: &GraphBrowserApp,
) -> HashSet<NodeKey> {
    TileCoordinator::all_node_pane_keys_using_composited_runtime(tiles_tree, graph_app)
}

pub(crate) fn node_pane_uses_composited_runtime(
    state: &NodePaneState,
    graph_app: &GraphBrowserApp,
) -> bool {
    TileCoordinator::node_pane_uses_composited_runtime(state, graph_app)
}

pub(crate) fn refresh_node_pane_render_modes(
    tiles_tree: &mut Tree<TileKind>,
    graph_app: &GraphBrowserApp,
) {
    TileCoordinator::refresh_node_pane_render_modes(tiles_tree, graph_app);
}

pub(crate) fn prune_stale_node_pane_keys_only(
    tiles_tree: &mut Tree<TileKind>,
    graph_app: &GraphBrowserApp,
) {
    TileCoordinator::prune_stale_node_pane_keys_only(tiles_tree, graph_app);
}

#[allow(dead_code)]
pub(crate) fn remove_all_node_panes(tiles_tree: &mut Tree<TileKind>) {
    TileCoordinator::remove_all_node_panes(tiles_tree);
}

#[allow(dead_code)]
pub(crate) fn remove_node_pane_for_node(tiles_tree: &mut Tree<TileKind>, node_key: NodeKey) {
    TileCoordinator::remove_node_pane_for_node(tiles_tree, node_key);
}

pub(crate) fn prune_stale_node_panes(
    tiles_tree: &mut Tree<TileKind>,
    graph_app: &mut GraphBrowserApp,
    window: &EmbedderWindow,
    tile_rendering_contexts: &mut HashMap<NodeKey, Rc<OffscreenRenderingContext>>,
    lifecycle_intents: &mut Vec<GraphIntent>,
) {
    TileCoordinator::prune_stale_node_panes(
        tiles_tree,
        graph_app,
        window,
        tile_rendering_contexts,
        lifecycle_intents,
    );
}

pub(crate) fn release_node_runtime_for_pane(
    graph_app: &mut GraphBrowserApp,
    window: &EmbedderWindow,
    tile_rendering_contexts: &mut HashMap<NodeKey, Rc<OffscreenRenderingContext>>,
    node_key: NodeKey,
    lifecycle_intents: &mut Vec<GraphIntent>,
) {
    TileCoordinator::release_node_runtime_for_pane(
        graph_app,
        window,
        tile_rendering_contexts,
        node_key,
        lifecycle_intents,
    );
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::TileCoordinator;
    use crate::app::GraphBrowserApp;
    use crate::shell::desktop::workbench::pane_model::{NodePaneState, TileRenderMode, ViewerId};
    use crate::shell::desktop::workbench::tile_kind::TileKind;
    use base::id::{PIPELINE_NAMESPACE, PainterId, PipelineNamespace, TEST_NAMESPACE};
    use egui_tiles::{Tiles, Tree};
    use euclid::Point2D;

    fn test_webview_id() -> servo::WebViewId {
        PIPELINE_NAMESPACE.with(|tls| {
            if tls.get().is_none() {
                PipelineNamespace::install(TEST_NAMESPACE);
            }
        });
        servo::WebViewId::new(PainterId::next())
    }

    #[test]
    fn preserve_runtime_webview_when_node_exists_and_mapped() {
        let webview_id = test_webview_id();
        assert!(TileCoordinator::should_preserve_runtime_webview(
            true,
            Some(webview_id)
        ));
    }

    #[test]
    fn do_not_preserve_runtime_webview_when_node_missing_or_unmapped() {
        let webview_id = test_webview_id();
        assert!(!TileCoordinator::should_preserve_runtime_webview(
            false,
            Some(webview_id)
        ));
        assert!(!TileCoordinator::should_preserve_runtime_webview(
            true, None
        ));
    }

    fn tree_with_node_pane(state: NodePaneState) -> Tree<TileKind> {
        let mut tiles = Tiles::default();
        let pane = tiles.insert_pane(TileKind::Node(state));
        let root = tiles.insert_tab_tile(vec![pane]);
        Tree::new("tile_runtime_viewer_selection_test", root, tiles)
    }

    #[test]
    fn node_pane_using_composited_runtime_uses_registry_selection_for_http_nodes() {
        let mut app = GraphBrowserApp::new_for_testing();
        let node_key = app.add_node_and_sync("https://example.test".into(), Point2D::new(0.0, 0.0));
        let tree = tree_with_node_pane(NodePaneState::for_node(node_key));

        let hosts = TileCoordinator::all_node_pane_keys_using_composited_runtime(&tree, &app);
        assert!(hosts.contains(&node_key));
    }

    #[test]
    fn node_pane_using_composited_runtime_uses_registry_selection_for_file_nodes() {
        let mut app = GraphBrowserApp::new_for_testing();
        let node_key =
            app.add_node_and_sync("file:///tmp/report.pdf".into(), Point2D::new(0.0, 0.0));
        let tree = tree_with_node_pane(NodePaneState::for_node(node_key));

        let hosts = TileCoordinator::all_node_pane_keys_using_composited_runtime(&tree, &app);
        assert!(!hosts.contains(&node_key));
    }

    #[test]
    fn node_pane_using_composited_runtime_uses_fallback_for_custom_schemes() {
        let mut app = GraphBrowserApp::new_for_testing();
        let node_key =
            app.add_node_and_sync("gemini://example.test".into(), Point2D::new(0.0, 0.0));
        let tree = tree_with_node_pane(NodePaneState::for_node(node_key));

        let hosts = TileCoordinator::all_node_pane_keys_using_composited_runtime(&tree, &app);
        assert!(!hosts.contains(&node_key));
    }

    #[test]
    fn node_pane_using_composited_runtime_preserves_explicit_viewer_override_precedence() {
        let mut app = GraphBrowserApp::new_for_testing();
        let http_node =
            app.add_node_and_sync("https://example.test".into(), Point2D::new(0.0, 0.0));
        let file_node =
            app.add_node_and_sync("file:///tmp/report.pdf".into(), Point2D::new(10.0, 0.0));

        let http_plaintext_tree = tree_with_node_pane(NodePaneState::with_viewer(
            http_node,
            ViewerId::new("viewer:plaintext"),
        ));
        let file_webview_tree = tree_with_node_pane(NodePaneState::with_viewer(
            file_node,
            ViewerId::new("viewer:webview"),
        ));

        let http_hosts =
            TileCoordinator::all_node_pane_keys_using_composited_runtime(&http_plaintext_tree, &app);
        let file_hosts =
            TileCoordinator::all_node_pane_keys_using_composited_runtime(&file_webview_tree, &app);

        assert!(!http_hosts.contains(&http_node));
        assert!(file_hosts.contains(&file_node));
    }

    #[test]
    fn composited_runtime_nodes_are_subset_of_all_node_panes() {
        let mut app = GraphBrowserApp::new_for_testing();
        let webview_node =
            app.add_node_and_sync("https://example.test".into(), Point2D::new(0.0, 0.0));
        let plaintext_node =
            app.add_node_and_sync("file:///tmp/readme.txt".into(), Point2D::new(10.0, 0.0));

        let mut tiles = Tiles::default();
        let a = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(webview_node)));
        let b = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(plaintext_node)));
        let root = tiles.insert_tab_tile(vec![a, b]);
        let tree = Tree::new("tile_runtime_node_vs_host_subset", root, tiles);

        let all_nodes = TileCoordinator::all_node_pane_keys(&tree);
        let host_nodes = TileCoordinator::all_node_pane_keys_using_composited_runtime(&tree, &app);

        assert!(all_nodes.contains(&webview_node));
        assert!(all_nodes.contains(&plaintext_node));
        assert!(host_nodes.contains(&webview_node));
        assert!(!host_nodes.contains(&plaintext_node));
        assert!(host_nodes.is_subset(&all_nodes));
    }

    #[test]
    fn refresh_node_pane_render_modes_sets_mode_for_each_node_pane() {
        let mut app = GraphBrowserApp::new_for_testing();
        let webview_node =
            app.add_node_and_sync("https://example.test".into(), Point2D::new(0.0, 0.0));
        let plaintext_node =
            app.add_node_and_sync("file:///tmp/readme.txt".into(), Point2D::new(10.0, 0.0));

        let mut tiles = Tiles::default();
        let a = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(webview_node)));
        let b = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(plaintext_node)));
        let root = tiles.insert_tab_tile(vec![a, b]);
        let mut tree = Tree::new("tile_runtime_render_mode_refresh", root, tiles);

        TileCoordinator::refresh_node_pane_render_modes(&mut tree, &app);

        let mut mode_for = HashMap::new();
        for (_, tile) in tree.tiles.iter() {
            if let egui_tiles::Tile::Pane(TileKind::Node(state)) = tile {
                mode_for.insert(state.node, state.render_mode);
            }
        }

        assert_eq!(
            mode_for.get(&webview_node).copied(),
            Some(TileRenderMode::CompositedTexture)
        );
        assert_eq!(
            mode_for.get(&plaintext_node).copied(),
            Some(TileRenderMode::EmbeddedEgui)
        );
    }
}
