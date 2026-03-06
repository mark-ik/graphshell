/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use egui_tiles::Tree;

use super::tile_behavior::{GraphshellTileBehavior, PendingOpenNode};
use super::tile_grouping;
use super::tile_kind::TileKind;
use super::tile_runtime;
use super::ux_tree;
use crate::app::{GraphBrowserApp, GraphIntent};
use crate::graph::NodeKey;
use crate::shell::desktop::runtime::diagnostics::{DiagnosticEvent, emit_event};
use crate::shell::desktop::runtime::registries::{
    CHANNEL_UX_CONTRACT_WARNING, CHANNEL_UX_TREE_SNAPSHOT_BUILT,
};

pub(crate) struct TileRenderOutputs {
    pub(crate) pending_open_nodes: Vec<PendingOpenNode>,
    pub(crate) pending_closed_nodes: Vec<NodeKey>,
    pub(crate) post_render_intents: Vec<GraphIntent>,
}

fn should_summon_command_palette_on_secondary_click(
    secondary_clicked: bool,
    hovered_graph_node: Option<NodeKey>,
    command_palette_open: bool,
) -> bool {
    secondary_clicked && hovered_graph_node.is_none() && !command_palette_open
}

pub(crate) fn render_tile_tree_and_collect_outputs(
    ui: &mut egui::Ui,
    tiles_tree: &mut Tree<TileKind>,
    graph_app: &mut GraphBrowserApp,
    control_panel: &mut crate::shell::desktop::runtime::control_panel::ControlPanel,
    tile_favicon_textures: &mut HashMap<NodeKey, (u64, egui::TextureHandle)>,
    search_matches: &HashSet<NodeKey>,
    active_search_match: Option<NodeKey>,
    graph_search_filter_mode: bool,
    search_query_active: bool,
    #[cfg(feature = "diagnostics")]
    diagnostics_state: &mut crate::shell::desktop::runtime::diagnostics::DiagnosticsState,
) -> TileRenderOutputs {
    let uxtree_build_started = Instant::now();
    let tab_groups_before = tile_grouping::node_pane_tab_group_memberships(tiles_tree);
    let mut behavior = GraphshellTileBehavior::new(
        graph_app,
        control_panel,
        tile_favicon_textures,
        search_matches,
        active_search_match,
        graph_search_filter_mode,
        search_query_active,
        #[cfg(feature = "diagnostics")]
        diagnostics_state,
    );
    tiles_tree.ui(&mut behavior, ui);

    let pending_open_nodes = behavior.take_pending_open_nodes();
    let pending_closed_nodes = behavior.take_pending_closed_nodes();
    let tab_drag_stopped_nodes = behavior.take_pending_tab_drag_stopped_nodes();
    let mut post_render_intents = behavior.take_pending_graph_intents();

    drop(behavior);

    // Secondary-click outside graph-node context should still summon the command palette.
    // Graph-node right-click remains owned by radial/context handling in render::mod.
    if should_summon_command_palette_on_secondary_click(
        ui.ctx().input(|i| i.pointer.secondary_clicked()),
        graph_app.workspace.hovered_graph_node,
        graph_app.workspace.show_command_palette,
    ) {
        graph_app.toggle_command_palette();
    }

    let uxtree_snapshot = ux_tree::build_snapshot(
        tiles_tree,
        graph_app,
        uxtree_build_started.elapsed().as_micros() as u64,
    );
    ux_tree::publish_snapshot(&uxtree_snapshot);
    emit_event(DiagnosticEvent::MessageSent {
        channel_id: CHANNEL_UX_TREE_SNAPSHOT_BUILT,
        byte_len: uxtree_snapshot.semantic_nodes.len(),
    });
    if let Some(message) = ux_tree::presentation_id_consistency_violation(&uxtree_snapshot) {
        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_UX_CONTRACT_WARNING,
            byte_len: message.len(),
        });
    }

    let tab_groups_after = tile_grouping::node_pane_tab_group_memberships(tiles_tree);
    let tab_group_nodes_after = tile_grouping::tab_group_nodes(tiles_tree);
    post_render_intents.extend(tile_grouping::user_grouped_intents_for_tab_group_moves(
        &tab_groups_before,
        &tab_groups_after,
        &tab_group_nodes_after,
        &tab_drag_stopped_nodes,
    ));

    TileRenderOutputs {
        pending_open_nodes,
        pending_closed_nodes,
        post_render_intents,
    }
}

pub(crate) fn mapped_nodes_without_tiles(
    graph_app: &GraphBrowserApp,
    tiles_tree: &Tree<TileKind>,
) -> Vec<NodeKey> {
    let node_panes_using_composited_runtime =
        tile_runtime::all_node_pane_keys_using_composited_runtime(tiles_tree, graph_app);
    graph_app
        .webview_node_mappings()
        .map(|(_, node_key)| node_key)
        .filter(|node_key| !node_panes_using_composited_runtime.contains(node_key))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::should_summon_command_palette_on_secondary_click;
    use crate::graph::NodeKey;

    #[test]
    fn secondary_click_without_node_summons_palette() {
        assert!(should_summon_command_palette_on_secondary_click(
            true,
            None,
            false
        ));
    }

    #[test]
    fn secondary_click_over_node_does_not_summon_palette() {
        assert!(!should_summon_command_palette_on_secondary_click(
            true,
            Some(NodeKey::new(1)),
            false
        ));
    }

    #[test]
    fn secondary_click_when_palette_already_open_does_not_toggle() {
        assert!(!should_summon_command_palette_on_secondary_click(
            true,
            None,
            true
        ));
    }
}
