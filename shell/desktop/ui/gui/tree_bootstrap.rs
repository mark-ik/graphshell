/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use super::*;

pub(super) fn ensure_tiles_tree_root(tiles_tree: &mut Tree<TileKind>) {
    if tiles_tree.root().is_none() {
        let graph_tile_id = insert_default_graph_tile(tiles_tree);
        set_tiles_tree_root(tiles_tree, graph_tile_id);
    }
}

fn insert_default_graph_tile(tiles_tree: &mut Tree<TileKind>) -> TileId {
    tiles_tree
        .tiles
        .insert_pane(TileKind::Graph(GraphViewId::default()))
}

fn set_tiles_tree_root(tiles_tree: &mut Tree<TileKind>, root_tile_id: TileId) {
    tiles_tree.root = Some(root_tile_id);
}