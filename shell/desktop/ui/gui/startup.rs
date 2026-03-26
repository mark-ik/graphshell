/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use super::*;
use crate::app::SearchDisplayMode;

fn normalize_graph_only_workbench_tree(tiles_tree: &mut Tree<TileKind>) {
    // Graph canvas is hosted in CentralPanel (ShellPrimary), not the tile tree.
    // Strip any TileKind::Graph panes so they never render twice.
    let graph_tile_ids: Vec<_> = tiles_tree
        .tiles
        .iter()
        .filter_map(|(id, tile)| {
            if matches!(tile, egui_tiles::Tile::Pane(TileKind::Graph(_))) {
                Some(*id)
            } else {
                None
            }
        })
        .collect();
    for tile_id in graph_tile_ids {
        tiles_tree.remove_recursively(tile_id);
    }
}

pub(super) fn restore_startup_session_frame_if_available(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
) -> bool {
    if let Some(layout_json) =
        graph_app.load_workspace_layout_json(GraphBrowserApp::SESSION_WORKSPACE_LAYOUT_NAME)
        && let Ok(mut restored_tree) = serde_json::from_str::<Tree<TileKind>>(&layout_json)
    {
        tile_runtime::prune_stale_node_pane_keys_only(&mut restored_tree, graph_app);
        normalize_graph_only_workbench_tree(&mut restored_tree);
        if restored_tree.root().is_some() {
            graph_app.mark_session_frame_layout_json(&layout_json);
            log::debug!("gui: restored startup session frame from session layout json");
            *tiles_tree = restored_tree;
            return true;
        }
    }

    if let Ok(bundle) = persistence_ops::load_named_frame_bundle(
        graph_app,
        GraphBrowserApp::SESSION_WORKSPACE_LAYOUT_NAME,
    ) && let Ok((mut restored_tree, _)) = {
        persistence_ops::apply_workbench_profile_from_bundle(graph_app, &bundle);
        persistence_ops::restore_runtime_tree_from_frame_bundle(graph_app, &bundle)
    } {
        normalize_graph_only_workbench_tree(&mut restored_tree);
        if restored_tree.root().is_some() {
            if let Ok(runtime_layout_json) = serde_json::to_string(&restored_tree) {
                graph_app.mark_session_frame_layout_json(&runtime_layout_json);
            }
            log::debug!("gui: restored startup session frame from legacy session bundle");
            *tiles_tree = restored_tree;
            return true;
        }
    }

    if let Some(layout_json) = graph_app.load_tile_layout_json()
        && let Ok(mut restored_tree) = serde_json::from_str::<Tree<TileKind>>(&layout_json)
    {
        tile_runtime::prune_stale_node_pane_keys_only(&mut restored_tree, graph_app);
        normalize_graph_only_workbench_tree(&mut restored_tree);
        if restored_tree.root().is_some() {
            graph_app.mark_session_frame_layout_json(&layout_json);
            log::debug!("gui: restored startup session frame from compatibility layout json");
            *tiles_tree = restored_tree;
            return true;
        }
    }

    false
}

pub(super) fn initialize_startup_graph_and_tiles(
    graph_data_dir: Option<PathBuf>,
    initial_url: &Url,
    graph_snapshot_interval_secs: Option<u64>,
) -> (GraphBrowserApp, Tree<TileKind>, bool) {
    let initial_data_dir =
        graph_data_dir.unwrap_or_else(crate::services::persistence::GraphStore::default_data_dir);
    let mut graph_app = GraphBrowserApp::new_from_dir(initial_data_dir);
    if let Some(snapshot_secs) = graph_snapshot_interval_secs
        && let Err(e) = graph_app.set_snapshot_interval_secs(snapshot_secs)
    {
        warn!("Failed to apply snapshot interval from startup preferences: {e}");
    }

    let mut tiles = Tiles::default();
    let bootstrap_graph_tile_id = tiles.insert_pane(TileKind::Graph(
        crate::shell::desktop::workbench::pane_model::GraphPaneRef::new(GraphViewId::default()),
    ));
    let mut tiles_tree = Tree::new("graphshell_tiles", bootstrap_graph_tile_id, tiles);
    normalize_graph_only_workbench_tree(&mut tiles_tree);
    let _ = restore_startup_session_frame_if_available(&mut graph_app, &mut tiles_tree);

    // Only create initial node if graph wasn't recovered from persistence.
    if !graph_app.has_recovered_graph() {
        use euclid::default::Point2D;
        graph_app.apply_reducer_intents([GraphIntent::CreateNodeAtUrl {
            url: initial_url.to_string(),
            position: Point2D::new(400.0, 300.0),
        }]);
    }

    let membership_index = persistence_ops::build_membership_index_from_frame_manifests(&graph_app);
    graph_app.init_membership_index(membership_index);
    let (workspace_recency, workspace_activation_seq) =
        persistence_ops::build_frame_activation_recency_from_frame_manifests(&graph_app);
    graph_app.init_frame_activation_recency(workspace_recency, workspace_activation_seq);

    let initial_search_filter_mode = matches!(
        graph_app.workspace.graph_runtime.search_display_mode,
        SearchDisplayMode::Filter
    );

    (graph_app, tiles_tree, initial_search_filter_mode)
}
