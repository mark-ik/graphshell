/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use super::*;

pub(super) fn webview_at_point(
    tiles_tree: &Tree<TileKind>,
    graph_app: &GraphBrowserApp,
    point: Point2D<f32, DeviceIndependentPixel>,
) -> Option<(WebViewId, Point2D<f32, DeviceIndependentPixel>)> {
    let cursor = egui::pos2(point.x, point.y);
    for (tile_id, tile) in tiles_tree.tiles.iter() {
        let tile_id = *tile_id;
        let Tile::Pane(TileKind::Node(state)) = tile else {
            continue;
        };
        if !tiles_tree.is_visible(tile_id) {
            continue;
        }
        let Some(rect) = tiles_tree.tiles.rect(tile_id) else {
            continue;
        };
        if !rect.contains(cursor) {
            continue;
        }
        let Some(webview_id) = graph_app.get_webview_for_node(state.node) else {
            continue;
        };
        let local = egui::Pos2::new(point.x - rect.min.x, point.y - rect.min.y).to_point2d();
        return Some((webview_id, local));
    }
    None
}

pub(super) fn graph_at_point(
    tiles_tree: &Tree<TileKind>,
    point: Point2D<f32, DeviceIndependentPixel>,
) -> bool {
    let cursor = egui::pos2(point.x, point.y);
    tiles_tree.tiles.iter().any(|(tile_id, tile)| {
        let tile_id = *tile_id;
        matches!(tile, Tile::Pane(TileKind::Graph(_)))
            && tiles_tree.is_visible(tile_id)
            && tiles_tree
                .tiles
                .rect(tile_id)
                .is_some_and(|rect| rect.contains(cursor))
    })
}

