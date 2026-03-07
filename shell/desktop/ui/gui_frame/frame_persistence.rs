/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use super::*;

pub(super) fn handle_pending_frame_snapshot_actions(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
) {
    handle_pending_frame_prompt_and_restore(graph_app, tiles_tree);
    handle_pending_frame_save_prune_and_import(graph_app, tiles_tree);
}

fn handle_pending_frame_prompt_and_restore(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
) {
    handle_unsaved_workspace_prompt_resolution(graph_app, tiles_tree);
    handle_pending_named_frame_snapshot_restore_request(graph_app, tiles_tree);
}

fn handle_unsaved_workspace_prompt_resolution(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
) {
    if let Some((request, action)) = graph_app.take_unsaved_workspace_prompt_resolution() {
        match (request, action) {
            (
                UnsavedFramePromptRequest::FrameSwitch { name, focus_node },
                UnsavedFramePromptAction::ProceedWithoutSaving,
            ) => {
                let open_request = pending_open_request_from_focus_node(focus_node);
                restore_named_frame_snapshot(graph_app, tiles_tree, &name, open_request);
            }
            (UnsavedFramePromptRequest::FrameSwitch { .. }, UnsavedFramePromptAction::Cancel) => {}
        }
    }
}

fn pending_open_request_from_focus_node(
    focus_node: Option<NodeKey>,
) -> Option<PendingNodeOpenRequest> {
    focus_node.map(|key| PendingNodeOpenRequest {
        key,
        mode: PendingTileOpenMode::Tab,
    })
}

fn handle_pending_named_frame_snapshot_restore_request(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
) {
    if let Some(name) = graph_app.take_pending_restore_frame_snapshot_named() {
        let open_request = graph_app.take_pending_frame_restore_open_request();
        if graph_app.should_prompt_unsaved_workspace_save() {
            warn_unsaved_changes_before_frame_switch(graph_app, &name);
            request_unsaved_workspace_frame_switch_prompt(graph_app, name, open_request);
        } else {
            restore_named_frame_snapshot(graph_app, tiles_tree, &name, open_request);
        }
    }
}

fn warn_unsaved_changes_before_frame_switch(graph_app: &mut GraphBrowserApp, name: &str) {
    if graph_app.consume_unsaved_workspace_prompt_warning() {
        warn!("Current frame has unsaved graph changes before switching to '{name}'");
    }
}

fn request_unsaved_workspace_frame_switch_prompt(
    graph_app: &mut GraphBrowserApp,
    name: String,
    open_request: Option<PendingNodeOpenRequest>,
) {
    graph_app.request_unsaved_workspace_prompt(UnsavedFramePromptRequest::FrameSwitch {
        name,
        focus_node: open_request.map(|request| request.key),
    });
}

fn handle_pending_frame_save_prune_and_import(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
) {
    handle_pending_frame_save_and_prune(graph_app, tiles_tree);
    handle_pending_frame_import_actions(graph_app);
}

fn handle_pending_frame_save_and_prune(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &Tree<TileKind>,
) {
    handle_pending_frame_save_layout_actions(graph_app, tiles_tree);
    handle_pending_frame_prune_retention_actions(graph_app);
}

fn handle_pending_frame_save_layout_actions(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &Tree<TileKind>,
) {
    handle_pending_save_frame_snapshot(graph_app, tiles_tree);
    handle_pending_save_frame_snapshot_named(graph_app, tiles_tree);
}

fn handle_pending_save_frame_snapshot(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &Tree<TileKind>,
) {
    if graph_app.take_pending_save_frame_snapshot()
        && let Some(layout_json) =
            workspace_layout::serialize_tiles_tree_layout_json(tiles_tree, "frame snapshot")
    {
        graph_app.save_tile_layout_json(&layout_json);
    }
}

fn handle_pending_save_frame_snapshot_named(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &Tree<TileKind>,
) {
    if let Some(name) = graph_app.take_pending_save_frame_snapshot_named() {
        match persistence_ops::save_named_frame_bundle(graph_app, &name, tiles_tree) {
            Ok(()) => refresh_frame_membership_cache(graph_app),
            Err(e) => warn!("Failed to serialize tile layout for frame snapshot '{name}': {e}"),
        }
    }
}

fn refresh_frame_membership_cache(graph_app: &mut GraphBrowserApp) {
    let _ = persistence_ops::refresh_frame_membership_cache_from_manifests(graph_app);
}

fn handle_pending_frame_prune_retention_actions(graph_app: &mut GraphBrowserApp) {
    handle_pending_prune_empty_frames(graph_app);
    handle_pending_keep_latest_named_frames(graph_app);
}

fn handle_pending_prune_empty_frames(graph_app: &mut GraphBrowserApp) {
    if graph_app.take_pending_prune_empty_frames() {
        let deleted = persistence_ops::prune_empty_named_workspaces(graph_app);
        warn!("Pruned {deleted} empty named frame snapshots");
    }
}

fn handle_pending_keep_latest_named_frames(graph_app: &mut GraphBrowserApp) {
    if let Some(keep) = graph_app.take_pending_keep_latest_named_frames() {
        let deleted = persistence_ops::keep_latest_named_workspaces(graph_app, keep);
        warn!("Removed {deleted} named frame snapshots beyond latest {keep}");
    }
}

fn handle_pending_frame_import_actions(graph_app: &mut GraphBrowserApp) {
    handle_pending_add_node_to_frame(graph_app);
    handle_pending_add_connected_to_frame(graph_app);
    handle_pending_add_exact_to_frame(graph_app);
}

fn handle_pending_add_node_to_frame(graph_app: &mut GraphBrowserApp) {
    if let Some((node_key, frame_name)) = graph_app.take_pending_add_node_to_frame() {
        add_nodes_to_named_frame_snapshot(graph_app, &frame_name, &[node_key]);
    }
}

fn handle_pending_add_connected_to_frame(graph_app: &mut GraphBrowserApp) {
    if let Some((seed_nodes, frame_name)) = graph_app.take_pending_add_connected_to_frame() {
        add_connected_nodes_to_named_frame_snapshot(graph_app, &frame_name, &seed_nodes);
    }
}

fn add_connected_nodes_to_named_frame_snapshot(
    graph_app: &mut GraphBrowserApp,
    frame_name: &str,
    seed_nodes: &[NodeKey],
) {
    let nodes = connected_open::connected_frame_import_nodes(graph_app, seed_nodes);
    add_nodes_to_named_frame_snapshot(graph_app, frame_name, &nodes);
}

fn handle_pending_add_exact_to_frame(graph_app: &mut GraphBrowserApp) {
    if let Some((nodes, frame_name)) = graph_app.take_pending_add_exact_to_frame() {
        add_nodes_to_named_frame_snapshot(graph_app, &frame_name, &nodes);
    }
}

fn restore_named_frame_snapshot(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    name: &str,
    mut routed_open_request: Option<PendingNodeOpenRequest>,
) {
    debug!("gui_frame: attempting to restore frame snapshot '{}'", name);
    match persistence_ops::load_named_frame_bundle(graph_app, name).and_then(|bundle| {
        persistence_ops::restore_runtime_tree_from_frame_bundle(graph_app, &bundle)
    }) {
        Ok((mut restored_tree, restored_nodes)) => {
            if let Ok(current_layout_json) = serde_json::to_string(tiles_tree) {
                graph_app.record_workspace_undo_boundary(
                    Some(current_layout_json),
                    UndoBoundaryReason::RestoreFrameSnapshot,
                );
            }
            if restored_tree.root().is_some() {
                debug!(
                    "frame restore: restored '{}' with {} resolved nodes",
                    name,
                    restored_nodes.len()
                );
                if let Some(request) = routed_open_request.take()
                    && graph_app.domain_graph().get_node(request.key).is_some()
                {
                    debug!(
                        "gui_frame: opening routed node {:?} in restored frame",
                        request.key
                    );
                    tile_view_ops::open_or_focus_node_pane_with_mode(
                        &mut restored_tree,
                        graph_app,
                        request.key,
                        pending_tile_mode_to_tile_mode(request.mode),
                    );
                    let mut restore_intents = vec![
                        lifecycle_intents::promote_node_to_active(
                            request.key,
                            LifecycleCause::Restore,
                        )
                        .into(),
                    ];
                    apply_intents_if_any(graph_app, &restored_tree, &mut restore_intents);
                }
                graph_app.note_frame_activated(name, restored_nodes);
                if let Err(e) = persistence_ops::mark_named_frame_bundle_activated(graph_app, name)
                {
                    warn!("Failed to mark frame bundle '{name}' activated: {e}");
                }
                if let Ok(runtime_layout_json) = serde_json::to_string(&restored_tree) {
                    graph_app.mark_session_frame_layout_json(&runtime_layout_json);
                }
                *tiles_tree = restored_tree;
            } else if let Some(request) = routed_open_request.take() {
                warn!(
                    "Frame snapshot '{name}' is empty after restore resolution; falling back to current frame open"
                );
                graph_app.select_node(request.key, false);
                graph_app.request_open_node_tile_mode(request.key, request.mode);
            }
        }
        Err(e) => {
            warn!("Failed to restore frame snapshot '{name}': {e}");
            if let Some(request) = routed_open_request.take() {
                graph_app.select_node(request.key, false);
                graph_app.request_open_node_tile_mode(request.key, request.mode);
            }
        }
    }
}

fn pending_tile_mode_to_tile_mode(mode: PendingTileOpenMode) -> tile_view_ops::TileOpenMode {
    match mode {
        PendingTileOpenMode::Tab => tile_view_ops::TileOpenMode::Tab,
        PendingTileOpenMode::SplitHorizontal => tile_view_ops::TileOpenMode::SplitHorizontal,
    }
}

fn frame_tree_with_single_node(node_key: NodeKey) -> Tree<TileKind> {
    let mut tiles = Tiles::default();
    let pane_id = tiles.insert_pane(TileKind::Node(node_key.into()));
    let root = tiles.insert_tab_tile(vec![pane_id]);
    Tree::new("graphshell_workspace_layout", root, tiles)
}

fn add_nodes_to_named_frame_snapshot(
    graph_app: &mut GraphBrowserApp,
    name: &str,
    node_keys: &[NodeKey],
) {
    if GraphBrowserApp::is_reserved_workspace_layout_name(name) {
        warn!("Cannot add nodes to reserved frame snapshot '{name}'");
        return;
    }
    let live_nodes: Vec<NodeKey> = node_keys
        .iter()
        .copied()
        .filter(|key| graph_app.domain_graph().get_node(*key).is_some())
        .collect();
    if live_nodes.is_empty() {
        warn!("Cannot add empty/missing node set to frame snapshot '{name}'");
        return;
    }

    let mut workspace_tree = match persistence_ops::load_named_frame_bundle(graph_app, name) {
        Ok(bundle) => match persistence_ops::restore_runtime_tree_from_frame_bundle(graph_app, &bundle) {
            Ok((tree, _)) => tree,
            Err(e) => {
                warn!(
                    "Failed to restore named frame snapshot '{name}' for add-tab operation: {e}"
                );
                frame_tree_with_single_node(live_nodes[0])
            }
        },
        Err(_) => frame_tree_with_single_node(live_nodes[0]),
    };
    if workspace_tree.root().is_none() {
        workspace_tree = frame_tree_with_single_node(live_nodes[0]);
    }
    for node_key in live_nodes {
        tile_view_ops::open_or_focus_node_pane_with_mode(
            &mut workspace_tree,
            graph_app,
            node_key,
            tile_view_ops::TileOpenMode::Tab,
        );
    }
    match persistence_ops::save_named_frame_bundle(graph_app, name, &workspace_tree) {
        Ok(()) => {
            let _ = persistence_ops::refresh_frame_membership_cache_from_manifests(graph_app);
        }
        Err(e) => warn!("Failed to save frame snapshot '{name}' after add-tab operation: {e}"),
    }
}

fn connected_frame_import_nodes(graph_app: &GraphBrowserApp, seeds: &[NodeKey]) -> Vec<NodeKey> {
    let mut out = HashSet::new();
    for seed in seeds {
        if graph_app.domain_graph().get_node(*seed).is_none() {
            continue;
        }
        out.insert(*seed);
        out.extend(graph_app.domain_graph().neighbors_undirected(*seed));
    }
    let mut nodes: Vec<NodeKey> = out
        .into_iter()
        .filter(|key| graph_app.domain_graph().get_node(*key).is_some())
        .collect();
    nodes.sort_by_key(|key| key.index());
    nodes
}
