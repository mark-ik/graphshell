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

/// Build a `NodeKey → PaneId` mapping from the current tile tree.
///
/// This replaces per-query tile tree scans for PaneId lookup. Call at startup
/// and after dual-write mutations that add or remove node panes.
pub(crate) fn build_node_pane_id_map(
    tiles_tree: &Tree<TileKind>,
) -> std::collections::HashMap<NodeKey, super::pane_model::PaneId> {
    use egui_tiles::Tile;
    let mut map = std::collections::HashMap::new();
    for (_tile_id, tile) in tiles_tree.tiles.iter() {
        if let Tile::Pane(kind) = tile {
            if let Some(state) = kind.node_state() {
                map.insert(state.node, state.pane_id);
            }
        }
    }
    map
}

pub(crate) fn build_pane_render_mode_map(
    tiles_tree: &Tree<TileKind>,
) -> std::collections::HashMap<super::pane_model::PaneId, super::pane_model::TileRenderMode> {
    use egui_tiles::Tile;
    let mut map = std::collections::HashMap::new();
    for (_tile_id, tile) in tiles_tree.tiles.iter() {
        if let Tile::Pane(kind) = tile {
            if let Some(mode) = kind.node_render_mode() {
                map.insert(kind.pane_id(), mode);
            }
        }
    }
    map
}

pub(crate) fn build_pane_viewer_id_map(
    tiles_tree: &Tree<TileKind>,
) -> std::collections::HashMap<super::pane_model::PaneId, String> {
    use egui_tiles::Tile;
    let mut map = std::collections::HashMap::new();
    for (_tile_id, tile) in tiles_tree.tiles.iter() {
        if let Tile::Pane(TileKind::Node(state)) = tile {
            if let Some(viewer_id) = state.resolved_viewer_id.as_ref() {
                map.insert(state.pane_id, viewer_id.clone());
            }
        }
    }
    map
}

/// Produce `(PaneId, NodeKey, egui::Rect)` tuples from GraphTree layout, matching
/// the format of `tile_compositor::active_node_pane_rects()`.
///
/// During the migration, PaneId is still looked up from the tile tree since
/// the GraphTree doesn't carry pane identity. Once GraphTree becomes the
/// layout authority, PaneId will be stored as member metadata.
pub(crate) fn active_node_pane_rects_from_graph_tree(
    graph_tree: &GraphTree<NodeKey>,
    tiles_tree: &Tree<TileKind>,
    available: graph_tree::Rect,
) -> Vec<(super::pane_model::PaneId, NodeKey, egui::Rect)> {
    let layout = graph_tree.compute_layout(available);
    let mut result = Vec::new();

    for (member, rect) in &layout.pane_rects {
        // Look up PaneId from the tile tree (migration bridge).
        let pane_id = tiles_tree.tiles.iter().find_map(|(_, tile)| {
            if let Tile::Pane(kind) = tile {
                if let Some(state) = kind.node_state() {
                    if state.node == *member {
                        return Some(state.pane_id);
                    }
                }
            }
            None
        });

        if let Some(pane_id) = pane_id {
            let egui_rect =
                egui::Rect::from_min_size(egui::pos2(rect.x, rect.y), egui::vec2(rect.w, rect.h));
            result.push((pane_id, *member, egui_rect));
        }
    }

    result
}

/// Incremental sync: reconcile GraphTree membership with the tile tree
/// without destroying topology.
///
/// - Attaches newly appeared tile nodes using `provenance_fn` to infer the
///   correct traversal parent/child structure from the domain graph
/// - Detaches members that vanished from the tile tree
/// - Syncs lifecycle state for members present in both
/// - Syncs active member
///
/// Existing parent/child relationships, provenance, and expansion state
/// are preserved.
///
/// `provenance_fn` is called only for nodes NOT yet in GraphTree. It should
/// query the domain graph to determine the correct `Provenance` — typically
/// `Traversal { source }` if a traversal edge exists, or `Anchor` otherwise.
pub(crate) fn incremental_sync_from_tiles(
    graph_tree: &mut GraphTree<NodeKey>,
    tiles_tree: &Tree<TileKind>,
    active_node: Option<NodeKey>,
    lifecycle_fn: &dyn Fn(NodeKey) -> NodeLifecycle,
    provenance_fn: &dyn Fn(NodeKey) -> Provenance<NodeKey>,
) {
    // Collect all node keys currently in the tile tree.
    let mut tile_nodes: Vec<NodeKey> = Vec::new();
    for (_tile_id, tile) in tiles_tree.tiles.iter() {
        if let Tile::Pane(kind) = tile {
            if let Some(state) = kind.node_state() {
                tile_nodes.push(state.node);
            }
        }
    }

    let tile_set: std::collections::HashSet<NodeKey> = tile_nodes.iter().copied().collect();

    // 1. Detach members that disappeared from the tile tree.
    //    Only detach non-Cold members — Cold members are expected to exist
    //    only in GraphTree (they have no tile representation).
    let current_members: Vec<(NodeKey, graph_tree::Lifecycle)> = graph_tree
        .members()
        .map(|(k, e)| (*k, e.lifecycle))
        .collect();
    for (member, lifecycle) in &current_members {
        if !tile_set.contains(member) && *lifecycle != graph_tree::Lifecycle::Cold {
            graph_tree.apply(NavAction::Detach {
                member: *member,
                recursive: false,
            });
        }
    }

    // 2. Attach tile nodes not yet in GraphTree.
    //    Use provenance_fn to infer the correct attachment provenance from the
    //    domain graph (traversal edge → Traversal parent/child; no edge → Anchor).
    for &node_key in &tile_nodes {
        if !graph_tree.contains(&node_key) {
            graph_tree.apply(NavAction::Attach {
                member: node_key,
                provenance: provenance_fn(node_key),
            });
        }
    }

    // 3. Sync lifecycle for members present in both.
    for &node_key in &tile_nodes {
        if graph_tree.contains(&node_key) {
            let lc = lifecycle_fn(node_key);
            graph_tree.apply(NavAction::SetLifecycle(
                node_key,
                to_graph_tree_lifecycle(lc),
            ));
        }
    }

    // 4. Sync active member.
    if let Some(active) = active_node {
        if graph_tree.contains(&active) {
            graph_tree.apply(NavAction::Activate(active));
        }
    }
}

/// Parity check: verify the GraphTree contains the same node panes as the tile tree.
///
/// Returns a list of discrepancies (empty if in sync). Intended for diagnostics
/// builds and debug assertions.
///
/// Checks membership, topology (when available), active member, visibility,
/// expansion state, and visible ordering. Cold members in GraphTree that are
/// absent from the tile tree are expected and not flagged.
///
/// `active_node` is the currently focused node key from the host; passed into
/// the external snapshot so the active-member comparison is meaningful.
#[cfg(any(feature = "diagnostics", debug_assertions))]
pub(crate) fn parity_check(
    graph_tree: &GraphTree<NodeKey>,
    tiles_tree: &Tree<TileKind>,
    active_node: Option<NodeKey>,
) -> Vec<ParityDiscrepancy> {
    let snapshot = build_external_snapshot(tiles_tree, active_node);
    let report = graph_tree::parity::compare(graph_tree, &snapshot);

    // Convert structural parity report to legacy discrepancy list for
    // backward compat with the existing debug_assert call site.
    let mut discrepancies = Vec::new();
    for divergence in &report.divergences {
        match divergence {
            graph_tree::parity::ParityDivergence::MissingFromExternal(nk) => {
                discrepancies.push(ParityDiscrepancy::MissingInTileTree(*nk));
            }
            graph_tree::parity::ParityDivergence::MissingFromGraphTree(nk) => {
                discrepancies.push(ParityDiscrepancy::MissingInGraphTree(*nk));
            }
            graph_tree::parity::ParityDivergence::TopologyMismatch { member, .. } => {
                discrepancies.push(ParityDiscrepancy::TopologyMismatch(*member));
            }
            graph_tree::parity::ParityDivergence::ActiveMismatch { .. } => {
                discrepancies.push(ParityDiscrepancy::ActiveMismatch);
            }
            graph_tree::parity::ParityDivergence::VisibilityMismatch { member, .. } => {
                discrepancies.push(ParityDiscrepancy::VisibilityMismatch(*member));
            }
            graph_tree::parity::ParityDivergence::ExpansionMismatch { member, .. } => {
                discrepancies.push(ParityDiscrepancy::ExpansionMismatch(*member));
            }
            graph_tree::parity::ParityDivergence::VisibleOrderMismatch { .. } => {
                discrepancies.push(ParityDiscrepancy::VisibleOrderMismatch);
            }
        }
    }

    discrepancies
}

/// Build an `ExternalTreeSnapshot` from the tile tree for structural parity comparison.
///
/// `active_node` is the currently focused NodeKey from the host; included so the
/// active-member comparison is meaningful. Pass `None` if focus is unknown.
#[cfg(any(feature = "diagnostics", debug_assertions))]
fn traverse_tile_tree(
    tiles: &egui_tiles::Tiles<TileKind>,
    tile_id: egui_tiles::TileId,
    visible_set: &mut std::collections::HashSet<NodeKey>,
    expanded_set: &mut std::collections::HashSet<NodeKey>,
    visible_order: &mut Vec<NodeKey>,
    is_visible: bool,
) {
    // If a parent is not visible, its children are typically not visible on screen,
    // though they exist. We consider a pane 'visible' if it actually draws.
    let tile = match tiles.get(tile_id) {
        Some(t) => t,
        None => return,
    };

    match tile {
        Tile::Pane(kind) => {
            if let Some(state) = kind.node_state() {
                if is_visible {
                    visible_set.insert(state.node);
                    visible_order.push(state.node);
                }
            }
        }
        Tile::Container(container) => match container {
            egui_tiles::Container::Tabs(tabs) => {
                // In Tabs, only the active tab is visible content-wise.
                // The others only show headers.
                for &child in &tabs.children {
                    let child_is_visible = is_visible && Some(child) == tabs.active;
                    traverse_tile_tree(
                        tiles,
                        child,
                        visible_set,
                        expanded_set,
                        visible_order,
                        child_is_visible,
                    );
                }
            }
            egui_tiles::Container::Linear(linear) => {
                for &child in &linear.children {
                    traverse_tile_tree(
                        tiles,
                        child,
                        visible_set,
                        expanded_set,
                        visible_order,
                        is_visible,
                    );
                }
            }
            egui_tiles::Container::Grid(grid) => {
                // In a grid, children are ordered visually by their position.
                // For simplicity, we just walk the children iterator.
                for &child in grid.children() {
                    traverse_tile_tree(
                        tiles,
                        child,
                        visible_set,
                        expanded_set,
                        visible_order,
                        is_visible,
                    );
                }
            }
        },
    }
}

#[cfg(any(feature = "diagnostics", debug_assertions))]
fn build_external_snapshot(
    tiles_tree: &Tree<TileKind>,
    active_node: Option<NodeKey>,
) -> graph_tree::parity::ExternalTreeSnapshot<NodeKey> {
    use std::collections::{HashMap, HashSet};

    let mut members = HashSet::new();
    let mut visible = HashSet::new();
    let mut visible_order = Vec::new();
    let mut expanded = HashSet::new();
    let children: HashMap<NodeKey, Vec<NodeKey>> = HashMap::new();

    // 1. Members can be gathered just by iterating all panes.
    for (_tile_id, tile) in tiles_tree.tiles.iter() {
        if let Tile::Pane(kind) = tile {
            if let Some(state) = kind.node_state() {
                members.insert(state.node);
            }
        }
    }

    // 2. Visible and visible_order require traversal from root.
    if let Some(root_id) = tiles_tree.root() {
        traverse_tile_tree(
            &tiles_tree.tiles,
            root_id,
            &mut visible,
            &mut expanded,
            &mut visible_order,
            true,
        );
    }

    // The tile tree doesn't have node-level parent/child relationships
    // (it has container/pane structure, not semantic parent/child), so
    // we leave `children` empty. Topology mismatches between GraphTree's
    // rich parent/child structure and tiles' flat pane list are expected
    // during the transition phase.
    //
    // TODO(Phase D): When GraphTree becomes authority, topology comparison
    // becomes meaningful and this should be populated.

    graph_tree::parity::ExternalTreeSnapshot {
        members,
        children,
        active: active_node,
        visible,
        expanded,
        visible_order,
    }
}

#[cfg(any(feature = "diagnostics", debug_assertions))]
#[derive(Debug, Clone)]
pub(crate) enum ParityDiscrepancy {
    /// Node exists in tile tree but not in GraphTree.
    MissingInGraphTree(NodeKey),
    /// Node exists in GraphTree but not in tile tree (and is not Cold).
    MissingInTileTree(NodeKey),
    /// Parent/child topology differs between GraphTree and tile tree.
    TopologyMismatch(NodeKey),
    /// Active member disagrees.
    ActiveMismatch,
    /// Visibility differs (visible in one but not the other).
    VisibilityMismatch(NodeKey),
    /// Expansion state differs.
    ExpansionMismatch(NodeKey),
    /// Visible member ordering differs.
    VisibleOrderMismatch,
}
