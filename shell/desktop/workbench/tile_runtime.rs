/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use egui_tiles::{Tile, Tree};
use servo::{OffscreenRenderingContext, WebViewId};

use crate::app::{GraphBrowserApp, GraphIntent, LifecycleCause};
use crate::shell::desktop::lifecycle::lifecycle_intents;
use crate::shell::desktop::workbench::pane_model::{NodePaneState, ViewerId};
use crate::shell::desktop::workbench::tile_kind::TileKind;
use crate::graph::{NodeKey, NodeLifecycle};
use crate::shell::desktop::host::window::EmbedderWindow;

pub(crate) struct TileCoordinator;

impl TileCoordinator {
    fn node_pane_hosts_webview_runtime(state: &NodePaneState) -> bool {
        state
            .viewer_id_override
            .as_ref()
            .map(ViewerId::as_str)
            .map_or(true, |viewer_id| viewer_id == "viewer:webview")
    }

    fn collect_webview_host_node_pane_keys(tiles_tree: &Tree<TileKind>) -> HashSet<NodeKey> {
        tiles_tree
            .tiles
            .iter()
            .filter_map(|(_, tile)| match tile {
                Tile::Pane(TileKind::Node(state))
                    if Self::node_pane_hosts_webview_runtime(state) =>
                {
                    Some(state.node)
                }
                _ => None,
            })
            .collect()
    }

    fn should_preserve_runtime_webview(node_exists: bool, mapped_webview: Option<WebViewId>) -> bool {
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

    pub(crate) fn all_webview_host_node_pane_keys(tiles_tree: &Tree<TileKind>) -> HashSet<NodeKey> {
        Self::collect_webview_host_node_pane_keys(tiles_tree)
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
            Self::release_webview_runtime_for_node_pane(
                graph_app,
                window,
                tile_rendering_contexts,
                node_key,
                lifecycle_intents,
            );
        }
    }

    pub(crate) fn release_webview_runtime_for_node_pane(
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
                .workspace.graph
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

pub(crate) fn all_webview_host_node_pane_keys(tiles_tree: &Tree<TileKind>) -> HashSet<NodeKey> {
    TileCoordinator::all_webview_host_node_pane_keys(tiles_tree)
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

pub(crate) fn release_webview_runtime_for_node_pane(
    graph_app: &mut GraphBrowserApp,
    window: &EmbedderWindow,
    tile_rendering_contexts: &mut HashMap<NodeKey, Rc<OffscreenRenderingContext>>,
    node_key: NodeKey,
    lifecycle_intents: &mut Vec<GraphIntent>,
) {
    TileCoordinator::release_webview_runtime_for_node_pane(
        graph_app,
        window,
        tile_rendering_contexts,
        node_key,
        lifecycle_intents,
    );
}

pub(crate) fn close_webview_for_node(
    graph_app: &mut GraphBrowserApp,
    window: &EmbedderWindow,
    tile_rendering_contexts: &mut HashMap<NodeKey, Rc<OffscreenRenderingContext>>,
    node_key: NodeKey,
    lifecycle_intents: &mut Vec<GraphIntent>,
) {
    release_webview_runtime_for_node_pane(
        graph_app,
        window,
        tile_rendering_contexts,
        node_key,
        lifecycle_intents,
    );
}

#[cfg(test)]
mod tests {
    use super::TileCoordinator;
    use base::id::{PIPELINE_NAMESPACE, PainterId, PipelineNamespace, TEST_NAMESPACE};
    use crate::shell::desktop::workbench::pane_model::{NodePaneState, ViewerId};

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
        assert!(TileCoordinator::should_preserve_runtime_webview(true, Some(webview_id)));
    }

    #[test]
    fn do_not_preserve_runtime_webview_when_node_missing_or_unmapped() {
        let webview_id = test_webview_id();
        assert!(!TileCoordinator::should_preserve_runtime_webview(false, Some(webview_id)));
        assert!(!TileCoordinator::should_preserve_runtime_webview(true, None));
    }

    #[test]
    fn node_pane_webview_runtime_hosting_respects_viewer_override() {
        let default_state = NodePaneState::for_node(petgraph::stable_graph::NodeIndex::new(0));
        let webview_state = NodePaneState::with_viewer(
            petgraph::stable_graph::NodeIndex::new(1),
            ViewerId::new("viewer:webview"),
        );
        let plaintext_state = NodePaneState::with_viewer(
            petgraph::stable_graph::NodeIndex::new(2),
            ViewerId::new("viewer:plaintext"),
        );

        assert!(TileCoordinator::node_pane_hosts_webview_runtime(&default_state));
        assert!(TileCoordinator::node_pane_hosts_webview_runtime(&webview_state));
        assert!(!TileCoordinator::node_pane_hosts_webview_runtime(&plaintext_state));
    }
}
