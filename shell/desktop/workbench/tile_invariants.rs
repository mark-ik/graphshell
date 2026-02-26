/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::HashMap;
use std::rc::Rc;

use egui_tiles::Tree;
use servo::OffscreenRenderingContext;

use crate::app::GraphBrowserApp;
use crate::shell::desktop::workbench::tile_kind::TileKind;
use crate::shell::desktop::workbench::tile_runtime;
use crate::graph::NodeKey;

pub(crate) fn collect_tile_invariant_violations(
    tiles_tree: &Tree<TileKind>,
    graph_app: &GraphBrowserApp,
    tile_rendering_contexts: &HashMap<NodeKey, Rc<OffscreenRenderingContext>>,
) -> Vec<String> {
    let mut violations = Vec::new();
    for node_key in tile_runtime::all_node_pane_keys_hosting_webview_runtime(tiles_tree, graph_app) {
        if graph_app.workspace.graph.get_node(node_key).is_none() {
            violations.push(format!(
                "tile/webview-host desync: tile has stale node key {}",
                node_key.index()
            ));
            continue;
        }
        if graph_app.get_webview_for_node(node_key).is_none() {
            violations.push(format!(
                "tile/webview-host desync: node {} is missing webview mapping",
                node_key.index()
            ));
        }
        if !tile_rendering_contexts.contains_key(&node_key) {
            violations.push(format!(
                "tile/context desync: node {} is missing rendering context",
                node_key.index()
            ));
        }
    }
    violations
}

pub(crate) fn collect_active_tile_mapping_violations(
    tiles_tree: &Tree<TileKind>,
    graph_app: &GraphBrowserApp,
    tile_rendering_contexts: &HashMap<NodeKey, Rc<OffscreenRenderingContext>>,
) -> Vec<String> {
    let mut violations = Vec::new();
    let node_panes_hosting_webview_runtime =
        tile_runtime::all_node_pane_keys_hosting_webview_runtime(tiles_tree, graph_app);
    for tile_id in tiles_tree.active_tiles() {
        let Some(egui_tiles::Tile::Pane(TileKind::Node(state))) = tiles_tree.tiles.get(tile_id)
        else {
            continue;
        };
        let node_key = state.node;
        if !node_panes_hosting_webview_runtime.contains(&node_key) {
            continue;
        }
        if graph_app.workspace.graph.get_node(node_key).is_none() {
            violations.push(format!(
                "active tile desync: node {} no longer exists in graph",
                node_key.index()
            ));
            continue;
        }
        if graph_app.get_webview_for_node(node_key).is_none() {
            violations.push(format!(
                "active tile desync: node {} is missing webview mapping",
                node_key.index()
            ));
        }
        if !tile_rendering_contexts.contains_key(&node_key) {
            violations.push(format!(
                "active tile desync: node {} is missing rendering context",
                node_key.index()
            ));
        }
    }
    violations
}

#[cfg(test)]
mod tests {
    use super::*;

    use egui_tiles::Tiles;
    use euclid::Point2D;

    fn tree_with_active_node_pane_hosting_webview_runtime(node_key: NodeKey) -> Tree<TileKind> {
        let mut tiles = Tiles::default();
        let graph = tiles.insert_pane(TileKind::Graph(crate::app::GraphViewId::default()));
        let node_pane = tiles.insert_pane(TileKind::Node(node_key.into()));
        let root = tiles.insert_tab_tile(vec![graph, node_pane]);
        let mut tree = Tree::new("tile_invariants_test", root, tiles);
        let _ = tree.make_active(|tile_id, _| tile_id == node_pane);
        tree
    }

    #[test]
    fn active_tile_mapping_violations_detect_missing_mapping_and_context() {
        let mut app = GraphBrowserApp::new_for_testing();
        let node_key = app.add_node_and_sync("https://example.test".into(), Point2D::new(0.0, 0.0));
        if let Some(webview_id) = app.get_webview_for_node(node_key) {
            let _ = app.unmap_webview(webview_id);
        }
        let tree = tree_with_active_node_pane_hosting_webview_runtime(node_key);
        let contexts: HashMap<NodeKey, Rc<OffscreenRenderingContext>> = HashMap::new();

        let violations = collect_active_tile_mapping_violations(&tree, &app, &contexts);

        assert!(
            violations
                .iter()
                .any(|v| v.contains("missing webview mapping"))
        );
        assert!(
            violations
                .iter()
                .any(|v| v.contains("missing rendering context"))
        );
    }

    #[test]
    fn active_tile_mapping_violations_ignore_non_active_node_panes_hosting_webview_runtime() {
        let mut app = GraphBrowserApp::new_for_testing();
        let node_key = app.add_node_and_sync("https://example.test".into(), Point2D::new(0.0, 0.0));

        let mut tiles = Tiles::default();
        let graph = tiles.insert_pane(TileKind::Graph(crate::app::GraphViewId::default()));
        let node_pane = tiles.insert_pane(TileKind::Node(node_key.into()));
        let root = tiles.insert_tab_tile(vec![graph, node_pane]);
        let mut tree = Tree::new("tile_invariants_non_active", root, tiles);
        let _ = tree.make_active(|tile_id, _| tile_id == graph);

        let contexts: HashMap<NodeKey, Rc<OffscreenRenderingContext>> = HashMap::new();
        let violations = collect_active_tile_mapping_violations(&tree, &app, &contexts);

        assert!(violations.is_empty());
    }
}
