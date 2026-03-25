/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use super::*;

pub(super) fn ensure_tiles_tree_root(tiles_tree: &mut Tree<TileKind>) {
    let first_tile_id = tiles_tree.tiles.iter().next().map(|(tile_id, _)| *tile_id);
    if tiles_tree.root().is_none() && let Some(tile_id) = first_tile_id {
        set_tiles_tree_root(tiles_tree, tile_id);
    }
}

fn set_tiles_tree_root(tiles_tree: &mut Tree<TileKind>, root_tile_id: TileId) {
    tiles_tree.root = Some(root_tile_id);
}
