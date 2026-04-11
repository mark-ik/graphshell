/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Synchronization bridge between `egui_tiles::Tree<TileKind>` and `GraphTree<NodeKey>`.
//!
//! During the parallel introduction phase, this module mirrors tile tree
//! mutations into the GraphTree so that both data structures stay in sync.
//! Once the migration is complete and GraphTree becomes the authority,
//! this bridge will be removed.

use egui_tiles::{Tile, Tree};
use graph_tree::{GraphTree, Lifecycle, NavAction, Provenance};

use crate::graph::{NodeKey, NodeLifecycle};

use super::tile_kind::TileKind;

/// Map Graphshell's `NodeLifecycle` to graph-tree's `Lifecycle`.
pub(crate) fn to_graph_tree_lifecycle(lc: NodeLifecycle) -> Lifecycle {
    match lc {
        NodeLifecycle::Active => Lifecycle::Active,
        NodeLifecycle::Warm => Lifecycle::Warm,
        NodeLifecycle::Cold | NodeLifecycle::Tombstone => Lifecycle::Cold,
    }
}

/// Sync a node pane attachment into the GraphTree.
///
/// Called when a new `TileKind::Node` pane is added to the egui_tiles tree.
/// Attaches the node to the GraphTree if not already present.
pub(crate) fn sync_node_attach(
    graph_tree: &mut GraphTree<NodeKey>,
    node_key: NodeKey,
    source: Option<NodeKey>,
) {
    if graph_tree.contains(&node_key) {
        return;
    }

    let provenance = match source {
        Some(src) => Provenance::Traversal {
            source: src,
            edge_kind: None,
        },
        None => Provenance::Anchor,
    };

    graph_tree.apply(NavAction::Attach {
        member: node_key,
        provenance,
    });
}

/// Sync a node pane removal from the GraphTree.
pub(crate) fn sync_node_detach(graph_tree: &mut GraphTree<NodeKey>, node_key: NodeKey) {
    if !graph_tree.contains(&node_key) {
        return;
    }

    graph_tree.apply(NavAction::Detach {
        member: node_key,
        recursive: false,
    });
}

/// Sync lifecycle state for a node.
pub(crate) fn sync_lifecycle(
    graph_tree: &mut GraphTree<NodeKey>,
    node_key: NodeKey,
    lifecycle: NodeLifecycle,
) {
    if !graph_tree.contains(&node_key) {
        return;
    }

    graph_tree.apply(NavAction::SetLifecycle(
        node_key,
        to_graph_tree_lifecycle(lifecycle),
    ));
}

/// Sync active pane focus.
pub(crate) fn sync_active(graph_tree: &mut GraphTree<NodeKey>, node_key: NodeKey) {
    if !graph_tree.contains(&node_key) {
        return;
    }

    graph_tree.apply(NavAction::Activate(node_key));
}

/// Rebuild the GraphTree from the current egui_tiles tree state.
///
/// Used for initial population and for periodic consistency checks.
/// This is a full rebuild — it clears the GraphTree and re-attaches
/// all node panes found in the tile tree.
pub(crate) fn rebuild_from_tiles(
    graph_tree: &mut GraphTree<NodeKey>,
    tiles_tree: &Tree<TileKind>,
    active_node: Option<NodeKey>,
    lifecycle_fn: &dyn Fn(NodeKey) -> NodeLifecycle,
) {
    // Collect all node keys from the tile tree.
    let mut node_keys: Vec<NodeKey> = Vec::new();
    for (_tile_id, tile) in tiles_tree.tiles.iter() {
        if let Tile::Pane(kind) = tile {
            if let Some(state) = kind.node_state() {
                node_keys.push(state.node);
            }
        }
    }

    // Detach any members in graph_tree not in the tile tree.
    let current_members: Vec<NodeKey> = graph_tree
        .members()
        .map(|(k, _)| k.clone())
        .collect();
    for member in &current_members {
        if !node_keys.contains(member) {
            graph_tree.apply(NavAction::Detach {
                member: *member,
                recursive: false,
            });
        }
    }

    // Attach any tile-tree nodes not yet in graph_tree.
    for &node_key in &node_keys {
        if !graph_tree.contains(&node_key) {
            graph_tree.apply(NavAction::Attach {
                member: node_key,
                provenance: Provenance::Restored,
            });
        }

        // Sync lifecycle.
        let lc = lifecycle_fn(node_key);
        graph_tree.apply(NavAction::SetLifecycle(
            node_key,
            to_graph_tree_lifecycle(lc),
        ));
    }

    // Sync active.
    if let Some(active) = active_node {
        if graph_tree.contains(&active) {
            graph_tree.apply(NavAction::Activate(active));
        }
    }
}
