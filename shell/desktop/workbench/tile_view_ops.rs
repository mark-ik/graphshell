/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use egui_tiles::{Container, Tile, Tree};
use servo::{OffscreenRenderingContext, WebViewId, WindowRenderingContext};

use crate::app::{GraphBrowserApp, GraphIntent, GraphViewId};
use crate::graph::NodeKey;
use crate::shell::desktop::host::running_app_state::RunningAppState;
use crate::shell::desktop::host::window::EmbedderWindow;
use crate::shell::desktop::lifecycle::webview_backpressure::{
    self, WebviewCreationBackpressureState,
};
use crate::shell::desktop::workbench::pane_model::NodePaneState;
use crate::shell::desktop::workbench::tile_kind::TileKind;
use crate::shell::desktop::workbench::tile_runtime;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TileOpenMode {
    Tab,
    SplitHorizontal,
}

pub(crate) struct ToggleTileViewArgs<'a> {
    pub(crate) tiles_tree: &'a mut Tree<TileKind>,
    pub(crate) graph_app: &'a mut GraphBrowserApp,
    pub(crate) window: &'a EmbedderWindow,
    pub(crate) app_state: &'a Option<Rc<RunningAppState>>,
    pub(crate) base_rendering_context: &'a Rc<OffscreenRenderingContext>,
    pub(crate) window_rendering_context: &'a Rc<WindowRenderingContext>,
    pub(crate) tile_rendering_contexts: &'a mut HashMap<NodeKey, Rc<OffscreenRenderingContext>>,
    pub(crate) responsive_webviews: &'a HashSet<WebViewId>,
    pub(crate) webview_creation_backpressure:
        &'a mut HashMap<NodeKey, WebviewCreationBackpressureState>,
    pub(crate) lifecycle_intents: &'a mut Vec<GraphIntent>,
}

pub(crate) fn preferred_detail_node(graph_app: &GraphBrowserApp) -> Option<NodeKey> {
    graph_app
        .get_single_selected_node()
        .or_else(|| graph_app.workspace.graph.nodes().next().map(|(key, _)| key))
}

pub(crate) fn active_graph_view_id(tiles_tree: &Tree<TileKind>) -> Option<GraphViewId> {
    let mut last_active_graph = None;
    for tile_id in tiles_tree.active_tiles() {
        if let Some(Tile::Pane(TileKind::Graph(view_id))) = tiles_tree.tiles.get(tile_id) {
            last_active_graph = Some(*view_id);
        }
    }
    last_active_graph
}

pub(crate) fn open_or_focus_graph_pane(tiles_tree: &mut Tree<TileKind>, view_id: GraphViewId) {
    open_or_focus_graph_pane_with_mode(tiles_tree, view_id, TileOpenMode::Tab);
}

pub(crate) fn open_or_focus_graph_pane_with_mode(
    tiles_tree: &mut Tree<TileKind>,
    view_id: GraphViewId,
    mode: TileOpenMode,
) {
    log::debug!(
        "tile_view_ops: open_or_focus_graph_pane_with_mode view {:?} mode {:?}",
        view_id,
        mode
    );

    if tiles_tree.make_active(
        |_, tile| matches!(tile, Tile::Pane(TileKind::Graph(existing)) if *existing == view_id),
    ) {
        log::debug!(
            "tile_view_ops: focused existing graph pane for view {:?}",
            view_id
        );
        return;
    }

    let graph_pane_tile_id = tiles_tree.tiles.insert_pane(TileKind::Graph(view_id));
    let split_leaf_tile_id = tiles_tree.tiles.insert_tab_tile(vec![graph_pane_tile_id]);
    let Some(root_id) = tiles_tree.root() else {
        tiles_tree.root = Some(match mode {
            TileOpenMode::Tab => graph_pane_tile_id,
            TileOpenMode::SplitHorizontal => split_leaf_tile_id,
        });
        return;
    };

    match mode {
        TileOpenMode::Tab => {
            if let Some(Tile::Container(Container::Tabs(tabs))) = tiles_tree.tiles.get_mut(root_id)
            {
                tabs.add_child(graph_pane_tile_id);
                tabs.set_active(graph_pane_tile_id);
                return;
            }

            let tabs_root = tiles_tree
                .tiles
                .insert_tab_tile(vec![root_id, graph_pane_tile_id]);
            tiles_tree.root = Some(tabs_root);
            let _ = tiles_tree.make_active(
                |_, tile| matches!(tile, Tile::Pane(TileKind::Graph(existing)) if *existing == view_id),
            );
        }
        TileOpenMode::SplitHorizontal => {
            let split_lhs_id = if matches!(tiles_tree.tiles.get(root_id), Some(Tile::Pane(_))) {
                let wrapped = tiles_tree.tiles.insert_tab_tile(vec![root_id]);
                tiles_tree.root = Some(wrapped);
                wrapped
            } else {
                root_id
            };

            if let Some(Tile::Container(Container::Linear(linear))) =
                tiles_tree.tiles.get_mut(split_lhs_id)
            {
                linear.add_child(split_leaf_tile_id);
                let _ = tiles_tree.make_active(
                    |_, tile| matches!(tile, Tile::Pane(TileKind::Graph(existing)) if *existing == view_id),
                );
                return;
            }

            let split_root = tiles_tree
                .tiles
                .insert_horizontal_tile(vec![split_lhs_id, split_leaf_tile_id]);
            tiles_tree.root = Some(split_root);
            let _ = tiles_tree.make_active(
                |_, tile| matches!(tile, Tile::Pane(TileKind::Graph(existing)) if *existing == view_id),
            );
        }
    }
}

pub(crate) fn open_or_focus_node_pane(tiles_tree: &mut Tree<TileKind>, node_key: NodeKey) {
    open_or_focus_node_pane_with_mode(tiles_tree, node_key, TileOpenMode::Tab);
}

pub(crate) fn open_or_focus_node_pane_with_mode(
    tiles_tree: &mut Tree<TileKind>,
    node_key: NodeKey,
    mode: TileOpenMode,
) {
    log::debug!(
        "tile_view_ops: open_or_focus_node_pane_with_mode node {:?} mode {:?}",
        node_key,
        mode
    );
    if tiles_tree.make_active(
        |_, tile| matches!(tile, Tile::Pane(TileKind::Node(state)) if state.node == node_key),
    ) {
        log::debug!(
            "tile_view_ops: focused existing node pane for node {:?}",
            node_key
        );
        return;
    }

    let node_pane_tile_id = tiles_tree
        .tiles
        .insert_pane(TileKind::Node(node_key.into()));
    let split_leaf_tile_id = tiles_tree.tiles.insert_tab_tile(vec![node_pane_tile_id]);
    log::debug!(
        "tile_view_ops: inserted node pane {:?} (split leaf {:?}) for node {:?}",
        node_pane_tile_id,
        split_leaf_tile_id,
        node_key
    );
    let Some(root_id) = tiles_tree.root() else {
        tiles_tree.root = Some(match mode {
            TileOpenMode::Tab => node_pane_tile_id,
            TileOpenMode::SplitHorizontal => split_leaf_tile_id,
        });
        log::debug!("tile_view_ops: no root, set root to {:?}", tiles_tree.root);
        return;
    };

    match mode {
        TileOpenMode::Tab => {
            if let Some(Tile::Container(Container::Tabs(tabs))) = tiles_tree.tiles.get_mut(root_id)
            {
                tabs.add_child(node_pane_tile_id);
                tabs.set_active(node_pane_tile_id);
                return;
            }

            let tabs_root = tiles_tree
                .tiles
                .insert_tab_tile(vec![root_id, node_pane_tile_id]);
            tiles_tree.root = Some(tabs_root);
            tiles_tree.make_active(
                |_, tile| matches!(tile, Tile::Pane(TileKind::Node(state)) if state.node == node_key),
            );
        }
        TileOpenMode::SplitHorizontal => {
            // Never split directly against a raw leaf pane: wrap it in tabs first.
            let split_lhs_id = if matches!(
                tiles_tree.tiles.get(root_id),
                Some(Tile::Pane(TileKind::Node(_)))
            ) {
                let wrapped = tiles_tree.tiles.insert_tab_tile(vec![root_id]);
                tiles_tree.root = Some(wrapped);
                wrapped
            } else {
                root_id
            };

            if let Some(Tile::Container(Container::Linear(linear))) =
                tiles_tree.tiles.get_mut(split_lhs_id)
            {
                linear.add_child(split_leaf_tile_id);
                tiles_tree.make_active(
                    |_, tile| matches!(tile, Tile::Pane(TileKind::Node(state)) if state.node == node_key),
                );
                return;
            }
            let split_root = tiles_tree
                .tiles
                .insert_horizontal_tile(vec![split_lhs_id, split_leaf_tile_id]);
            tiles_tree.root = Some(split_root);
            tiles_tree.make_active(
                |_, tile| matches!(tile, Tile::Pane(TileKind::Node(state)) if state.node == node_key),
            );
        }
    }
}

pub(crate) fn detach_node_pane_to_split(tiles_tree: &mut Tree<TileKind>, node_key: NodeKey) {
    let existing_tile_id = tiles_tree
        .tiles
        .iter()
        .find_map(|(tile_id, tile)| match tile {
            Tile::Pane(TileKind::Node(state)) if state.node == node_key => Some(*tile_id),
            _ => None,
        });

    if let Some(tile_id) = existing_tile_id {
        tiles_tree.remove_recursively(tile_id);
    }
    open_or_focus_node_pane_with_mode(tiles_tree, node_key, TileOpenMode::SplitHorizontal);
}

pub(crate) fn toggle_tile_view(args: ToggleTileViewArgs<'_>) {
    if tile_runtime::has_any_node_panes(args.tiles_tree) {
        let node_pane_nodes = tile_runtime::all_node_pane_keys(args.tiles_tree);
        let webview_host_nodes = tile_runtime::all_node_pane_keys_hosting_webview_runtime(
            args.tiles_tree,
            args.graph_app,
        );
        let tile_ids: Vec<_> = args
            .tiles_tree
            .tiles
            .iter()
            .filter_map(|(tile_id, tile)| match tile {
                Tile::Pane(TileKind::Node(_)) => Some(*tile_id),
                _ => None,
            })
            .collect();
        for tile_id in tile_ids {
            args.tiles_tree.remove_recursively(tile_id);
        }
        for node_key in webview_host_nodes.iter().copied() {
            tile_runtime::release_webview_runtime_for_node_pane(
                args.graph_app,
                args.window,
                args.tile_rendering_contexts,
                node_key,
                args.lifecycle_intents,
            );
        }
        for node_key in node_pane_nodes {
            if !webview_host_nodes.contains(&node_key) {
                args.tile_rendering_contexts.remove(&node_key);
            }
        }
    } else if let Some(node_key) = preferred_detail_node(args.graph_app) {
        open_or_focus_node_pane(args.tiles_tree, node_key);
        let opened_node_pane = NodePaneState::for_node(node_key);
        if tile_runtime::node_pane_hosts_webview_runtime(&opened_node_pane, args.graph_app) {
            webview_backpressure::ensure_webview_for_node(
                args.graph_app,
                args.window,
                args.app_state,
                args.base_rendering_context,
                args.window_rendering_context,
                args.tile_rendering_contexts,
                node_key,
                args.responsive_webviews,
                args.webview_creation_backpressure,
                args.lifecycle_intents,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_tiles::Tiles;

    fn count_graph_panes(tiles_tree: &Tree<TileKind>) -> usize {
        tiles_tree
            .tiles
            .iter()
            .filter(|(_, tile)| matches!(tile, Tile::Pane(TileKind::Graph(_))))
            .count()
    }

    fn count_node_panes(tiles_tree: &Tree<TileKind>) -> usize {
        tiles_tree
            .tiles
            .iter()
            .filter(|(_, tile)| matches!(tile, Tile::Pane(TileKind::Node(_))))
            .count()
    }

    fn active_graph_view(tiles_tree: &Tree<TileKind>) -> Option<GraphViewId> {
        active_graph_view_id(tiles_tree)
    }

    #[test]
    fn open_or_focus_graph_pane_focuses_existing_graph_in_mixed_tree() {
        let graph_a = GraphViewId::new();
        let mut tiles = Tiles::default();
        let graph_tile = tiles.insert_pane(TileKind::Graph(graph_a));
        let node_tile = tiles.insert_pane(TileKind::Node(NodeKey::new(0).into()));
        let root = tiles.insert_tab_tile(vec![graph_tile, node_tile]);
        let mut tree = Tree::new("graph_focus_existing", root, tiles);

        assert_eq!(count_graph_panes(&tree), 1);
        assert_eq!(count_node_panes(&tree), 1);

        open_or_focus_graph_pane(&mut tree, graph_a);

        assert_eq!(count_graph_panes(&tree), 1);
        assert_eq!(count_node_panes(&tree), 1);
        assert_eq!(active_graph_view(&tree), Some(graph_a));
    }

    #[test]
    fn open_or_focus_graph_pane_inserts_new_graph_tab_with_requested_id() {
        let graph_a = GraphViewId::new();
        let graph_b = GraphViewId::new();
        let mut tiles = Tiles::default();
        let graph_tile = tiles.insert_pane(TileKind::Graph(graph_a));
        let node_tile = tiles.insert_pane(TileKind::Node(NodeKey::new(1).into()));
        let root = tiles.insert_tab_tile(vec![graph_tile, node_tile]);
        let mut tree = Tree::new("graph_open_new_tab", root, tiles);

        open_or_focus_graph_pane(&mut tree, graph_b);

        assert_eq!(count_graph_panes(&tree), 2);
        assert_eq!(count_node_panes(&tree), 1);
        assert_eq!(active_graph_view(&tree), Some(graph_b));
        assert!(tree
            .tiles
            .iter()
            .any(|(_, tile)| matches!(tile, Tile::Pane(TileKind::Graph(existing)) if *existing == graph_b)));
    }

    #[test]
    fn open_or_focus_graph_pane_split_preserves_ids_and_focuses_new_graph() {
        let graph_a = GraphViewId::new();
        let graph_b = GraphViewId::new();
        let mut tiles = Tiles::default();
        let graph_tile = tiles.insert_pane(TileKind::Graph(graph_a));
        let mut tree = Tree::new("graph_split", graph_tile, tiles);

        open_or_focus_graph_pane_with_mode(&mut tree, graph_b, TileOpenMode::SplitHorizontal);

        assert_eq!(count_graph_panes(&tree), 2);
        assert_eq!(active_graph_view(&tree), Some(graph_b));
        assert!(tree
            .tiles
            .iter()
            .any(|(_, tile)| matches!(tile, Tile::Pane(TileKind::Graph(existing)) if *existing == graph_a)));
        assert!(tree
            .tiles
            .iter()
            .any(|(_, tile)| matches!(tile, Tile::Pane(TileKind::Graph(existing)) if *existing == graph_b)));
    }
}
