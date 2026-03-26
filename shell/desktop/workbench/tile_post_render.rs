/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use egui_tiles::Tree;

use super::tile_behavior::{GraphshellTileBehavior, PendingOpenNode};
use super::tile_compositor;
use super::tile_grouping;
use super::tile_kind::TileKind;
use super::tile_runtime;
use super::ux_tree;
use crate::app::{GraphBrowserApp, GraphIntent, WorkbenchIntent};
use crate::graph::NodeKey;
use crate::shell::desktop::runtime::diagnostics::{DiagnosticEvent, emit_event};
use crate::shell::desktop::runtime::registries::{
    CHANNEL_UX_CONTRACT_WARNING, CHANNEL_UX_LAYOUT_GUTTER_DETECTED,
    CHANNEL_UX_LAYOUT_OVERLAP_DETECTED, CHANNEL_UX_PRESENTATION_BOUNDS_MISSING,
    CHANNEL_UX_TREE_BUILD, CHANNEL_UX_TREE_SNAPSHOT_BUILT,
};
use crate::shell::desktop::ui::persistence_ops;

pub(crate) struct TileRenderOutputs {
    pub(crate) pending_open_nodes: Vec<PendingOpenNode>,
    pub(crate) pending_closed_nodes: Vec<NodeKey>,
    pub(crate) post_render_intents: Vec<GraphIntent>,
}

fn should_summon_radial_palette_on_secondary_click(
    secondary_clicked: bool,
    hovered_graph_node: Option<NodeKey>,
    radial_menu_open: bool,
) -> bool {
    secondary_clicked && hovered_graph_node.is_none() && !radial_menu_open
}

fn active_context_return_target(
    tiles_tree: &Tree<TileKind>,
) -> Option<crate::app::ToolSurfaceReturnTarget> {
    for tile_id in tiles_tree.active_tiles() {
        match tiles_tree.tiles.get(tile_id) {
            Some(egui_tiles::Tile::Pane(TileKind::Graph(view_ref))) => {
                return Some(crate::app::ToolSurfaceReturnTarget::Graph(
                    view_ref.graph_view_id,
                ));
            }
            Some(egui_tiles::Tile::Pane(TileKind::Node(state))) => {
                return Some(crate::app::ToolSurfaceReturnTarget::Node(state.node));
            }
            Some(egui_tiles::Tile::Pane(TileKind::Tool(tool_ref))) => {
                return Some(crate::app::ToolSurfaceReturnTarget::Tool(
                    tool_ref.kind.clone(),
                ));
            }
            _ => {}
        }
    }
    None
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
    #[cfg(feature = "diagnostics")] runtime_focus_inspector: Option<
        crate::shell::desktop::ui::gui_state::RuntimeFocusInspector,
    >,
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
        #[cfg(feature = "diagnostics")]
        runtime_focus_inspector,
    );
    tiles_tree.ui(&mut behavior, ui);

    let pending_open_nodes = behavior.take_pending_open_nodes();
    let pending_closed_nodes = behavior.take_pending_closed_nodes();
    let tab_drag_stopped_nodes = behavior.take_pending_tab_drag_stopped_nodes();
    let mut post_render_intents: Vec<GraphIntent> = behavior
        .take_pending_post_render_intents()
        .into_iter()
        .map(Into::into)
        .collect();

    drop(behavior);

    // Secondary-click outside graph-node context summons the user-selected
    // contextual command surface. Graph-node right-click remains owned by
    // radial/context handling in render::mod.
    if should_summon_radial_palette_on_secondary_click(
        ui.ctx().input(|i| i.pointer.secondary_clicked()),
        graph_app.workspace.graph_runtime.hovered_graph_node,
        graph_app.workspace.chrome_ui.show_radial_menu,
    ) {
        match graph_app.context_command_surface_preference() {
            crate::app::ContextCommandSurfacePreference::RadialPalette => {
                if graph_app
                    .pending_transient_surface_return_target()
                    .is_none()
                {
                    graph_app.set_pending_transient_surface_return_target(
                        active_context_return_target(tiles_tree),
                    );
                }
                if !graph_app.workspace.chrome_ui.show_radial_menu {
                    graph_app.enqueue_workbench_intent(WorkbenchIntent::ToggleRadialMenu);
                }
            }
            crate::app::ContextCommandSurfacePreference::ContextPalette => {
                if graph_app.pending_command_surface_return_target().is_none() {
                    graph_app.set_pending_command_surface_return_target(
                        active_context_return_target(tiles_tree),
                    );
                }
                let pointer = ui.ctx().input(|i| i.pointer.latest_pos());
                graph_app.set_context_palette_anchor(pointer.map(|pos| [pos.x, pos.y]));
                graph_app.open_context_palette();
            }
        }
    }

    // Build a NodeKey → Rect map from the current active tile rects for bounds population.
    let active_rects = tile_compositor::active_node_pane_rects(tiles_tree);
    let node_rect_map: std::collections::HashMap<NodeKey, egui::Rect> = active_rects
        .into_iter()
        .map(|(_, node_key, rect)| (node_key, rect))
        .collect();

    let uxtree_snapshot = ux_tree::build_snapshot_with_rects(
        tiles_tree,
        graph_app,
        uxtree_build_started.elapsed().as_micros() as u64,
        &node_rect_map,
    );
    emit_event(DiagnosticEvent::MessageReceived {
        channel_id: CHANNEL_UX_TREE_BUILD,
        latency_us: uxtree_build_started.elapsed().as_micros() as u64,
    });
    ux_tree::publish_snapshot(&uxtree_snapshot);
    let layout_policy_intents = graph_app.evaluate_workbench_layout_policy(&uxtree_snapshot);
    if !layout_policy_intents.is_empty() {
        graph_app.extend_workbench_intents(layout_policy_intents);
    }
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
    // Emit a diagnostic if any NodePane semantic node has no presentation bounds —
    // indicates a tile that was rendered by the semantic tree but never laid out by the compositor.
    let bounds_missing_count = ux_tree::node_pane_bounds_missing_count(&uxtree_snapshot);
    if bounds_missing_count > 0 {
        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_UX_PRESENTATION_BOUNDS_MISSING,
            byte_len: bounds_missing_count,
        });
    }

    let coverage = ux_tree::run_coverage_analysis(&node_rect_map);
    if coverage.gutter_pair_count > 0 {
        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_UX_LAYOUT_GUTTER_DETECTED,
            byte_len: coverage.gutter_pair_count,
        });
    }
    if coverage.overlap_pair_count > 0 {
        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_UX_LAYOUT_OVERLAP_DETECTED,
            byte_len: coverage.overlap_pair_count,
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
    post_render_intents.extend(
        persistence_ops::frame_layout_sync_intents_for_current_frame(graph_app, tiles_tree),
    );

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
    use std::collections::{HashMap, HashSet};

    use super::{
        active_context_return_target, render_tile_tree_and_collect_outputs,
        should_summon_radial_palette_on_secondary_click,
    };
    use crate::app::{
        GraphBrowserApp, GraphViewId, SurfaceHostId, ToolSurfaceReturnTarget, WorkbenchIntent,
        WorkbenchLayoutConstraint,
    };
    use crate::graph::NodeKey;
    use crate::shell::desktop::runtime::control_panel::ControlPanel;
    #[cfg(feature = "diagnostics")]
    use crate::shell::desktop::runtime::diagnostics::DiagnosticsState;
    use crate::shell::desktop::workbench::pane_model::{
        GraphPaneRef, NodePaneState, ToolPaneRef, ToolPaneState,
    };
    use crate::shell::desktop::workbench::tile_kind::TileKind;
    use crate::shell::desktop::workbench::ux_tree;
    use egui_tiles::{Tiles, Tree};

    #[test]
    fn secondary_click_without_node_summons_palette() {
        assert!(should_summon_radial_palette_on_secondary_click(
            true, None, false
        ));
    }

    #[test]
    fn secondary_click_over_node_does_not_summon_palette() {
        assert!(!should_summon_radial_palette_on_secondary_click(
            true,
            Some(NodeKey::new(1)),
            false
        ));
    }

    #[test]
    fn secondary_click_when_palette_already_open_does_not_toggle() {
        assert!(!should_summon_radial_palette_on_secondary_click(
            true, None, true
        ));
    }

    #[test]
    fn non_secondary_click_never_summons_palette() {
        assert!(!should_summon_radial_palette_on_secondary_click(
            false, None, false
        ));
    }

    #[test]
    fn active_context_return_target_uses_active_node_tile_when_present() {
        let graph_view = GraphViewId::new();
        let node = NodeKey::new(7);
        let mut tiles = Tiles::default();
        let graph = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(graph_view)));
        let node_tile = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(node)));
        let root = tiles.insert_tab_tile(vec![graph, node_tile]);
        let mut tree = Tree::new("context_return_target_node", root, tiles);

        let _ = tree.make_active(
            |_, tile| matches!(tile, egui_tiles::Tile::Pane(TileKind::Node(state)) if state.node == node),
        );

        assert_eq!(
            active_context_return_target(&tree),
            Some(ToolSurfaceReturnTarget::Node(node))
        );
    }

    #[test]
    fn active_context_return_target_supports_active_tool_tile() {
        let mut tiles = Tiles::default();
        let tool = tiles.insert_pane(TileKind::Tool(ToolPaneRef::new(ToolPaneState::Settings)));
        let tree = Tree::new("context_return_target_tool", tool, tiles);

        assert_eq!(
            active_context_return_target(&tree),
            Some(ToolSurfaceReturnTarget::Tool(ToolPaneState::Settings))
        );
    }

    #[test]
    fn render_pass_enqueues_layout_policy_intents_after_uxtree_publish() {
        let mut app = GraphBrowserApp::new_for_testing();
        app.set_workbench_layout_constraint(
            SurfaceHostId::Navigator(crate::app::workbench_layout_policy::NavigatorHostId::Top),
            WorkbenchLayoutConstraint::anchored_split(
                SurfaceHostId::Navigator(crate::app::workbench_layout_policy::NavigatorHostId::Top),
                crate::app::workbench_layout_policy::AnchorEdge::Top,
                0.25,
            ),
        );

        let mut tiles = Tiles::default();
        let graph = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(GraphViewId::default())));
        let mut tree = Tree::new("layout_policy_render_pass", graph, tiles);
        let mut control_panel = ControlPanel::new(None);
        let mut tile_favicon_textures = HashMap::new();
        #[cfg(feature = "diagnostics")]
        let mut diagnostics_state = DiagnosticsState::new();
        let ctx = egui::Context::default();

        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let outputs = render_tile_tree_and_collect_outputs(
                    ui,
                    &mut tree,
                    &mut app,
                    &mut control_panel,
                    &mut tile_favicon_textures,
                    &HashSet::new(),
                    None,
                    false,
                    false,
                    #[cfg(feature = "diagnostics")]
                    &mut diagnostics_state,
                    #[cfg(feature = "diagnostics")]
                    None,
                );
                assert!(outputs.pending_open_nodes.is_empty());
            });
        });

        let drained = app.take_pending_workbench_intents();
        assert!(matches!(
            drained.as_slice(),
            [WorkbenchIntent::ApplyLayoutConstraint {
                surface_host,
                constraint: WorkbenchLayoutConstraint::AnchoredSplit { anchor_edge, .. },
            }] if *surface_host == SurfaceHostId::Navigator(
                crate::app::workbench_layout_policy::NavigatorHostId::Top,
            ) && *anchor_edge == crate::app::workbench_layout_policy::AnchorEdge::Top
        ));

        let snapshot = ux_tree::latest_snapshot().expect("uxtree snapshot should be published");
        assert!(snapshot.semantic_nodes.iter().any(|node| {
            matches!(node.domain, crate::shell::desktop::workbench::ux_tree::UxDomainIdentity::NavigatorProjection { .. })
        }));
    }
}
