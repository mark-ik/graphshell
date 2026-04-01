/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::{HashMap, HashSet};

use egui_tiles::{Container, Tile, TileId, Tiles, Tree};

use crate::app::GraphIntent;
use crate::graph::NodeKey;
use crate::shell::desktop::workbench::tile_kind::TileKind;

fn nearest_tabs_container_for_tile(tiles: &Tiles<TileKind>, mut tile_id: TileId) -> Option<TileId> {
    while let Some(parent_id) = tiles.parent_of(tile_id) {
        if matches!(
            tiles.get(parent_id),
            Some(Tile::Container(Container::Tabs(_)))
        ) {
            return Some(parent_id);
        }
        tile_id = parent_id;
    }
    None
}

fn node_key_for_tile(tile: &TileKind) -> Option<NodeKey> {
    tile.node_state().map(|state| state.node)
}

pub(crate) fn node_pane_tab_group_memberships(
    tiles_tree: &Tree<TileKind>,
) -> HashMap<NodeKey, TileId> {
    let mut memberships = HashMap::new();
    for (tile_id, tile) in tiles_tree.tiles.iter() {
        let Tile::Pane(tile_kind) = tile else {
            continue;
        };
        let Some(node_key) = node_key_for_tile(tile_kind) else {
            continue;
        };
        if let Some(group_id) = nearest_tabs_container_for_tile(&tiles_tree.tiles, *tile_id) {
            memberships.insert(node_key, group_id);
        }
    }
    memberships
}

pub(crate) fn webview_tab_group_memberships(
    tiles_tree: &Tree<TileKind>,
) -> HashMap<NodeKey, TileId> {
    node_pane_tab_group_memberships(tiles_tree)
}

pub(crate) fn tab_group_nodes(tiles_tree: &Tree<TileKind>) -> HashMap<TileId, Vec<NodeKey>> {
    let mut groups: HashMap<TileId, Vec<NodeKey>> = HashMap::new();
    for (tile_id, tile) in tiles_tree.tiles.iter() {
        let Tile::Pane(tile_kind) = tile else {
            continue;
        };
        let Some(node_key) = node_key_for_tile(tile_kind) else {
            continue;
        };
        if let Some(group_id) = nearest_tabs_container_for_tile(&tiles_tree.tiles, *tile_id) {
            groups.entry(group_id).or_default().push(node_key);
        }
    }
    groups
}

pub(crate) fn tab_group_node_order_for_tile(
    tiles: &Tiles<TileKind>,
    tile_id: TileId,
) -> Option<Vec<NodeKey>> {
    let group_id = nearest_tabs_container_for_tile(tiles, tile_id)?;
    let Tile::Container(Container::Tabs(tabs)) = tiles.get(group_id)? else {
        return None;
    };

    let ordered_nodes: Vec<_> = tabs
        .children
        .iter()
        .filter_map(|child_id| match tiles.get(*child_id) {
            Some(Tile::Pane(tile_kind)) => node_key_for_tile(tile_kind),
            _ => None,
        })
        .collect();
    (!ordered_nodes.is_empty()).then_some(ordered_nodes)
}

pub(crate) fn tab_node_keys_in_tree(tiles_tree: &Tree<TileKind>) -> HashSet<NodeKey> {
    node_pane_tab_group_memberships(tiles_tree)
        .into_keys()
        .collect()
}

pub(crate) fn user_grouped_intents_for_tab_group_moves(
    tab_groups_before: &HashMap<NodeKey, TileId>,
    tab_groups_after: &HashMap<NodeKey, TileId>,
    tab_group_nodes_after: &HashMap<TileId, Vec<NodeKey>>,
    moved_nodes: &std::collections::HashSet<NodeKey>,
) -> Vec<GraphIntent> {
    let mut intents = Vec::new();
    for (node_key, before_group) in tab_groups_before {
        if !moved_nodes.contains(node_key) {
            continue;
        }
        let Some(after_group) = tab_groups_after.get(node_key) else {
            continue;
        };
        if before_group == after_group {
            continue;
        }
        let Some(peers) = tab_group_nodes_after.get(after_group) else {
            continue;
        };
        if let Some(anchor) = peers.iter().copied().find(|peer| *peer != *node_key) {
            intents.push(GraphIntent::CreateUserGroupedEdge {
                from: *node_key,
                to: anchor,
                label: None,
            });
        }
    }
    intents
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::desktop::workbench::pane_model::{NodePaneState, PaneViewState};

    #[test]
    fn memberships_include_pane_wrapped_node_tiles() {
        let mut tiles = Tiles::default();
        let legacy_node = tiles.insert_pane(TileKind::Node(NodeKey::new(1).into()));
        let pane_wrapped = tiles.insert_pane(TileKind::Pane(PaneViewState::Node(
            NodePaneState::for_node(NodeKey::new(2)),
        )));
        let root = tiles.insert_tab_tile(vec![legacy_node, pane_wrapped]);
        let tree = Tree::new("tab_grouping_membership", root, tiles);

        let memberships = node_pane_tab_group_memberships(&tree);

        assert_eq!(memberships.len(), 2);
        assert_eq!(memberships.get(&NodeKey::new(1)), Some(&root));
        assert_eq!(memberships.get(&NodeKey::new(2)), Some(&root));
    }

    #[test]
    fn tab_group_order_for_tile_includes_pane_wrapped_node_tiles() {
        let mut tiles = Tiles::default();
        let first = tiles.insert_pane(TileKind::Pane(PaneViewState::Node(
            NodePaneState::for_node(NodeKey::new(10)),
        )));
        let second = tiles.insert_pane(TileKind::Node(NodeKey::new(11).into()));
        let _root = tiles.insert_tab_tile(vec![first, second]);

        let ordered = tab_group_node_order_for_tile(&tiles, second).expect("ordered nodes");

        assert_eq!(ordered, vec![NodeKey::new(10), NodeKey::new(11)]);
    }
}
