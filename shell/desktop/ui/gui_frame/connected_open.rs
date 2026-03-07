/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use super::*;

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

fn build_connected_open_selection_intents(source: NodeKey, ordered: &[NodeKey]) -> Vec<GraphIntent> {
    let mut intents = Vec::with_capacity(ordered.len() + 1);
    intents.push(GraphIntent::SelectNode {
        key: source,
        multi_select: false,
    });
    intents.push(lifecycle_intents::promote_node_to_active(source, LifecycleCause::UserSelect).into());
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
