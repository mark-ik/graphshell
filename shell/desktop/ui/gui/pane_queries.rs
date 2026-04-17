/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use super::*;

pub(super) fn tree_has_active_node_pane(graph_app: &GraphBrowserApp) -> bool {
    graph_app
        .workspace
        .graph_runtime
        .active_pane_rects
        .first()
        .is_some()
}

pub(super) fn tree_has_any_node_panes(tiles_tree: &Tree<TileKind>) -> bool {
    tile_runtime::has_any_node_panes(tiles_tree)
}

pub(super) fn reconcile_workspace_graph_views_from_tiles(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &Tree<TileKind>,
) {
    let live_graph_views = graph_view_ids_from_tiles(tiles_tree);
    graph_app.reconcile_workspace_graph_views(
        &live_graph_views,
        tile_view_ops::active_graph_view_id(tiles_tree),
    );
}

fn active_node_pane_node(tiles_tree: &Tree<TileKind>) -> Option<NodeKey> {
    tiles_tree
        .active_tiles()
        .into_iter()
        .find_map(|tile_id| active_node_key_for_tile_id(tiles_tree, tile_id))
}

fn active_node_key_for_tile_id(tiles_tree: &Tree<TileKind>, tile_id: TileId) -> Option<NodeKey> {
    let tile = tiles_tree.tiles.get(tile_id);
    node_key_from_node_pane_tile(tile)
}

fn node_key_from_node_pane_tile(tile: Option<&Tile<TileKind>>) -> Option<NodeKey> {
    match tile {
        Some(Tile::Pane(TileKind::Node(state))) => Some(state.node),
        _ => None,
    }
}

fn graph_view_ids_from_tiles(tiles_tree: &Tree<TileKind>) -> HashSet<GraphViewId> {
    tiles_tree
        .tiles
        .iter()
        .filter_map(|(_, tile)| match tile {
            Tile::Pane(TileKind::Graph(view_ref)) => Some(view_ref.graph_view_id),
            _ => None,
        })
        .collect()
}
