/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::HashSet;

use egui_tiles::{Container, Tile, TileId, Tiles, Tree};
use uuid::Uuid;

use crate::app::{GraphBrowserApp, WorkbenchIntent};
use crate::graph::NodeKey;
use crate::shell::desktop::ui::persistence_ops;
use crate::shell::desktop::workbench::pane_model::PaneId;
use crate::shell::desktop::workbench::tile_grouping;
use crate::shell::desktop::workbench::tile_kind::TileKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SemanticTabAffordance {
    Restore { group_id: Uuid, member_count: usize },
    Collapse { group_id: Uuid, member_count: usize },
}

pub(crate) fn current_runtime_frame_tab_semantics(
    graph_app: &GraphBrowserApp,
    tiles_tree: &Tree<TileKind>,
) -> Option<crate::app::RuntimeFrameTabSemantics> {
    graph_app
        .current_frame_tab_semantics()
        .cloned()
        .or_else(|| persistence_ops::derive_runtime_frame_tab_semantics_from_tree(tiles_tree))
}

fn tile_id_for_pane(tiles_tree: &Tree<TileKind>, pane_id: PaneId) -> Option<TileId> {
    tiles_tree
        .tiles
        .iter()
        .find_map(|(tile_id, tile)| match tile {
            Tile::Pane(pane) if pane.pane_id() == pane_id => Some(*tile_id),
            _ => None,
        })
}

fn node_key_for_pane(tiles_tree: &Tree<TileKind>, pane_id: PaneId) -> Option<NodeKey> {
    let tile_id = tile_id_for_pane(tiles_tree, pane_id)?;
    match tiles_tree.tiles.get(tile_id) {
        Some(Tile::Pane(tile)) => tile.node_state().map(|state| state.node),
        _ => None,
    }
}

fn node_key_for_pane_in_tiles(tiles: &Tiles<TileKind>, pane_id: PaneId) -> Option<NodeKey> {
    tiles.iter().find_map(|(_, tile)| match tile {
        Tile::Pane(tile) if tile.pane_id() == pane_id => tile.node_state().map(|state| state.node),
        _ => None,
    })
}

fn tile_is_attached(tiles_tree: &Tree<TileKind>, tile_id: TileId) -> bool {
    tiles_tree.root() == Some(tile_id) || tiles_tree.tiles.parent_of(tile_id).is_some()
}

fn tabs_container_for_exact_runtime_members(
    tiles_tree: &Tree<TileKind>,
    ordered_tile_ids: &[TileId],
) -> Option<TileId> {
    let first = *ordered_tile_ids.first()?;
    let parent_id = tiles_tree.tiles.parent_of(first)?;
    let Tile::Container(Container::Tabs(tabs)) = tiles_tree.tiles.get(parent_id)? else {
        return None;
    };

    if ordered_tile_ids
        .iter()
        .all(|tile_id| tiles_tree.tiles.parent_of(*tile_id) == Some(parent_id))
        && tabs.children.len() == ordered_tile_ids.len()
        && tabs
            .children
            .iter()
            .all(|child| ordered_tile_ids.contains(child))
    {
        Some(parent_id)
    } else {
        None
    }
}

pub(crate) fn semantic_tab_node_keys(
    tiles_tree: &Tree<TileKind>,
    graph_app: &GraphBrowserApp,
) -> HashSet<NodeKey> {
    let Some(semantics) = current_runtime_frame_tab_semantics(graph_app, tiles_tree) else {
        return tile_grouping::tab_node_keys_in_tree(tiles_tree);
    };

    let keys: HashSet<_> = semantics
        .tab_groups
        .iter()
        .flat_map(|group| group.pane_ids.iter().copied())
        .filter_map(|pane_id| node_key_for_pane(tiles_tree, pane_id))
        .collect();
    if keys.is_empty() {
        tile_grouping::tab_node_keys_in_tree(tiles_tree)
    } else {
        keys
    }
}

pub(crate) fn semantic_tab_node_order_for_tile(
    tiles_tree: &Tree<TileKind>,
    graph_app: &GraphBrowserApp,
    tile_id: TileId,
) -> Option<Vec<NodeKey>> {
    let pane_id = match tiles_tree.tiles.get(tile_id) {
        Some(Tile::Pane(tile)) => tile.pane_id(),
        _ => return tile_grouping::tab_group_node_order_for_tile(&tiles_tree.tiles, tile_id),
    };

    let fallback = tile_grouping::tab_group_node_order_for_tile(&tiles_tree.tiles, tile_id);
    let Some(semantics) = current_runtime_frame_tab_semantics(graph_app, tiles_tree) else {
        return fallback;
    };
    let Some(group) = semantics
        .tab_groups
        .iter()
        .find(|group| group.pane_ids.contains(&pane_id))
    else {
        return fallback;
    };

    let ordered_nodes: Vec<_> = group
        .pane_ids
        .iter()
        .filter_map(|pane_id| node_key_for_pane(tiles_tree, *pane_id))
        .collect();
    if ordered_nodes.is_empty() {
        return fallback;
    }

    if let Some(tree_order) = fallback {
        let semantic_nodes: HashSet<_> = ordered_nodes.iter().copied().collect();
        if tree_order.len() == ordered_nodes.len()
            && tree_order
                .iter()
                .all(|node_key| semantic_nodes.contains(node_key))
        {
            return Some(tree_order);
        }
    }

    Some(ordered_nodes)
}

pub(crate) fn semantic_tab_node_order_for_tile_in_tiles(
    tiles: &Tiles<TileKind>,
    graph_app: &GraphBrowserApp,
    tile_id: TileId,
) -> Option<Vec<NodeKey>> {
    let pane_id = match tiles.get(tile_id) {
        Some(Tile::Pane(tile)) => tile.pane_id(),
        _ => return tile_grouping::tab_group_node_order_for_tile(tiles, tile_id),
    };

    let fallback = tile_grouping::tab_group_node_order_for_tile(tiles, tile_id);
    let Some(semantics) = graph_app.current_frame_tab_semantics() else {
        return fallback;
    };
    let Some(group) = semantics
        .tab_groups
        .iter()
        .find(|group| group.pane_ids.contains(&pane_id))
    else {
        return fallback;
    };

    let ordered_nodes: Vec<_> = group
        .pane_ids
        .iter()
        .filter_map(|pane_id| node_key_for_pane_in_tiles(tiles, *pane_id))
        .collect();
    if ordered_nodes.is_empty() {
        return fallback;
    }

    if let Some(tree_order) = fallback {
        let semantic_nodes: HashSet<_> = ordered_nodes.iter().copied().collect();
        if tree_order.len() == ordered_nodes.len()
            && tree_order
                .iter()
                .all(|node_key| semantic_nodes.contains(node_key))
        {
            return Some(tree_order);
        }
    }

    Some(ordered_nodes)
}

pub(crate) fn semantic_tab_affordance_for_pane(
    tiles_tree: &Tree<TileKind>,
    graph_app: &GraphBrowserApp,
    pane: PaneId,
) -> Option<SemanticTabAffordance> {
    let semantics = current_runtime_frame_tab_semantics(graph_app, tiles_tree)?;
    let group = semantics
        .tab_groups
        .iter()
        .find(|group| group.pane_ids.len() > 1 && group.pane_ids.contains(&pane))?;
    let ordered_tile_ids: Vec<_> = group
        .pane_ids
        .iter()
        .filter_map(|pane_id| tile_id_for_pane(tiles_tree, *pane_id))
        .collect();
    if ordered_tile_ids.len() <= 1 {
        return None;
    }

    if tabs_container_for_exact_runtime_members(tiles_tree, &ordered_tile_ids).is_some() {
        return Some(SemanticTabAffordance::Collapse {
            group_id: group.group_id,
            member_count: group.pane_ids.len(),
        });
    }

    let pane_tile_id = tile_id_for_pane(tiles_tree, pane)?;
    if !tile_is_attached(tiles_tree, pane_tile_id) {
        return None;
    }

    Some(SemanticTabAffordance::Restore {
        group_id: group.group_id,
        member_count: group.pane_ids.len(),
    })
}

pub(crate) fn semantic_tab_toggle_intent_for_pane(
    tiles_tree: &Tree<TileKind>,
    graph_app: &GraphBrowserApp,
    pane: PaneId,
) -> Option<WorkbenchIntent> {
    match semantic_tab_affordance_for_pane(tiles_tree, graph_app, pane)? {
        SemanticTabAffordance::Restore { group_id, .. } => {
            Some(WorkbenchIntent::RestorePaneToSemanticTabGroup { pane, group_id })
        }
        SemanticTabAffordance::Collapse { group_id, .. } => {
            Some(WorkbenchIntent::CollapseSemanticTabGroupToPaneRest { group_id })
        }
    }
}
