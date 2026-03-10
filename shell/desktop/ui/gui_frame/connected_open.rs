/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use super::*;

fn tile_open_mode_from_pending(mode: PendingTileOpenMode) -> tile_view_ops::TileOpenMode {
    match mode {
        PendingTileOpenMode::Tab => tile_view_ops::TileOpenMode::Tab,
        PendingTileOpenMode::SplitHorizontal => tile_view_ops::TileOpenMode::SplitHorizontal,
    }
}

fn find_node_pane_tile_id(tree: &Tree<TileKind>, node_key: NodeKey) -> Option<TileId> {
    tree.tiles.iter().find_map(|(tile_id, tile)| match tile {
        Tile::Pane(TileKind::Node(state)) if state.node == node_key => Some(*tile_id),
        _ => None,
    })
}

fn ensure_node_pane_tile_id(tree: &mut Tree<TileKind>, node_key: NodeKey) -> TileId {
    if let Some(tile_id) = find_node_pane_tile_id(tree, node_key) {
        if let Some(parent_id) = tree.tiles.parent_of(tile_id)
            && matches!(
                tree.tiles.get(parent_id),
                Some(Tile::Container(Container::Tabs(_)))
            )
        {
            return parent_id;
        }
        return tree.tiles.insert_tab_tile(vec![tile_id]);
    }
    let pane_id = tree.tiles.insert_pane(TileKind::Node(node_key.into()));
    tree.tiles.insert_tab_tile(vec![pane_id])
}

fn apply_connected_split_layout(tree: &mut Tree<TileKind>, nodes: &[NodeKey]) {
    if nodes.is_empty() {
        return;
    }
    let split_count = nodes.len().min(MAX_CONNECTED_SPLIT_PANES);
    let split_tile_ids: Vec<TileId> = nodes
        .iter()
        .take(split_count)
        .copied()
        .map(|key| ensure_node_pane_tile_id(tree, key))
        .collect();
    let overflow_tile_ids: Vec<TileId> = nodes
        .iter()
        .skip(split_count)
        .copied()
        .map(|key| ensure_node_pane_tile_id(tree, key))
        .collect();

    let row1 = match split_tile_ids.as_slice() {
        [a] => *a,
        [a, b, ..] => tree.tiles.insert_horizontal_tile(vec![*a, *b]),
        [] => return,
    };

    let grid_root = if split_tile_ids.len() > 2 {
        let row2 = match split_tile_ids.as_slice() {
            [_, _, c] => *c,
            [_, _, c, d, ..] => tree.tiles.insert_horizontal_tile(vec![*c, *d]),
            _ => return,
        };
        tree.tiles.insert_vertical_tile(vec![row1, row2])
    } else {
        row1
    };

    tree.root = if overflow_tile_ids.is_empty() {
        Some(grid_root)
    } else {
        let overflow_tabs = tree.tiles.insert_tab_tile(overflow_tile_ids);
        Some(
            tree.tiles
                .insert_vertical_tile(vec![grid_root, overflow_tabs]),
        )
    };
}

pub(super) fn connected_frame_import_nodes(
    graph_app: &GraphBrowserApp,
    seeds: &[NodeKey],
) -> Vec<NodeKey> {
    graph_app.domain_graph().connected_frame_import_nodes(seeds)
}

fn connected_targets_for_open(
    graph_app: &GraphBrowserApp,
    source: NodeKey,
    scope: PendingConnectedOpenScope,
) -> Vec<NodeKey> {
    let max_depth = match scope {
        PendingConnectedOpenScope::Neighbors => 1,
        PendingConnectedOpenScope::Connected => 2,
    };
    let mut candidates = graph_app
        .domain_graph()
        .connected_candidates_with_depth(source, max_depth);
    let cap = MAX_CONNECTED_OPEN_NODES.saturating_sub(1);

    if candidates.len() > cap {
        candidates.sort_by(|(a, depth_a), (b, depth_b)| {
            graph_app
                .frame_recency_seq_for_node(*b)
                .cmp(&graph_app.frame_recency_seq_for_node(*a))
                .then_with(|| depth_a.cmp(depth_b))
                .then_with(|| a.index().cmp(&b.index()))
        });
        candidates.truncate(cap);
    }

    candidates.sort_by(|(a, depth_a), (b, depth_b)| {
        depth_a
            .cmp(depth_b)
            .then_with(|| {
                graph_app
                    .frame_recency_seq_for_node(*b)
                    .cmp(&graph_app.frame_recency_seq_for_node(*a))
            })
            .then_with(|| a.index().cmp(&b.index()))
    });
    candidates.into_iter().map(|(key, _)| key).collect()
}

pub(super) fn handle_pending_open_connected_from(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
) {
    if let Some((source, open_mode, scope)) = take_valid_pending_open_connected_from(graph_app) {
        execute_pending_open_connected_from(graph_app, tiles_tree, source, open_mode, scope);
    }
}

fn execute_pending_open_connected_from(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    source: NodeKey,
    open_mode: PendingTileOpenMode,
    scope: PendingConnectedOpenScope,
) {
    record_workspace_undo_boundary_from_tiles_tree(
        graph_app,
        tiles_tree,
        UndoBoundaryReason::OpenConnectedNodes,
    );
    let connected = connected_targets_for_open(graph_app, source, scope);
    let ordered = ordered_connected_open_nodes(source, connected);

    apply_connected_open_selection_intents(graph_app, tiles_tree, source, &ordered);
    open_connected_nodes_by_mode(graph_app, tiles_tree, open_mode, &ordered);
}

fn take_valid_pending_open_connected_from(
    graph_app: &mut GraphBrowserApp,
) -> Option<(NodeKey, PendingTileOpenMode, PendingConnectedOpenScope)> {
    if let Some((source, open_mode, scope)) = graph_app.take_pending_open_connected_from()
        && is_valid_connected_open_source(graph_app, source)
    {
        return Some((source, open_mode, scope));
    }

    None
}

fn is_valid_connected_open_source(graph_app: &GraphBrowserApp, source: NodeKey) -> bool {
    graph_app.domain_graph().get_node(source).is_some()
}

fn ordered_connected_open_nodes(source: NodeKey, connected: Vec<NodeKey>) -> Vec<NodeKey> {
    let mut ordered = Vec::with_capacity(connected.len() + 1);
    ordered.push(source);
    ordered.extend(connected);
    ordered
}

fn apply_connected_open_selection_intents(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    source: NodeKey,
    ordered: &[NodeKey],
) {
    let mut intents = build_connected_open_selection_intents(source, ordered);
    apply_intents_if_any(graph_app, tiles_tree, &mut intents);
}

fn build_connected_open_selection_intents(
    source: NodeKey,
    ordered: &[NodeKey],
) -> Vec<GraphIntent> {
    let mut intents = Vec::with_capacity(ordered.len() + 1);
    intents.push(GraphIntent::SelectNode {
        key: source,
        multi_select: false,
    });
    intents
        .push(lifecycle_intents::promote_node_to_active(source, LifecycleCause::UserSelect).into());
    for node in ordered.iter().skip(1) {
        intents.push(
            lifecycle_intents::promote_node_to_active(*node, LifecycleCause::ActiveTileVisible)
                .into(),
        );
    }
    intents
}

fn open_connected_nodes_by_mode(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    open_mode: PendingTileOpenMode,
    ordered: &[NodeKey],
) {
    graph_app.mark_current_frame_synthesized();
    let tile_mode = tile_open_mode_from_pending(open_mode);
    match tile_mode {
        tile_view_ops::TileOpenMode::Tab => {
            open_connected_nodes_as_tabs(graph_app, tiles_tree, ordered);
        }
        tile_view_ops::TileOpenMode::SplitHorizontal => {
            apply_connected_split_layout(tiles_tree, ordered);
        }
    }
}

fn open_connected_nodes_as_tabs(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    ordered: &[NodeKey],
) {
    for node in ordered {
        tile_view_ops::open_or_focus_node_pane_with_mode(
            tiles_tree,
            graph_app,
            *node,
            tile_view_ops::TileOpenMode::Tab,
        );
    }
}
