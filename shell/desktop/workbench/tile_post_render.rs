/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::{BTreeSet, HashMap, HashSet};
use std::time::Instant;

use egui_tiles::{TileId, Tree};

use super::tile_behavior::{GraphshellTileBehavior, PendingOpenNode};
use super::tile_compositor;
use super::tile_grouping;
use super::tile_kind::TileKind;
use super::tile_runtime;
use super::ux_tree;
use crate::app::{GraphBrowserApp, GraphIntent};
use crate::graph::NodeKey;
use crate::shell::desktop::runtime::diagnostics::{DiagnosticEvent, emit_event};
use crate::shell::desktop::runtime::registries::{
    CHANNEL_UX_CONTRACT_WARNING, CHANNEL_UX_LAYOUT_GUTTER_DETECTED,
    CHANNEL_UX_LAYOUT_OVERLAP_DETECTED, CHANNEL_UX_PRESENTATION_BOUNDS_MISSING,
    CHANNEL_UX_TREE_BUILD, CHANNEL_UX_TREE_SNAPSHOT_BUILT,
};
use crate::shell::desktop::ui::toolbar::toolbar_ui::CommandBarFocusTarget;
use crate::shell::desktop::ui::persistence_ops;
use crate::shell::desktop::ui::toolbar_routing;
use crate::shell::desktop::workbench::pane_model::PaneId;

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

fn active_frame_group_anchor(
    graph_app: &GraphBrowserApp,
    tiles_tree: &Tree<TileKind>,
) -> Option<NodeKey> {
    let mut cursor = tiles_tree.root();
    while let Some(tile_id) = cursor {
        if let Some(state) = graph_app
            .workspace
            .graph_runtime
            .frame_tile_groups
            .get(&tile_id)
        {
            return Some(state.frame_anchor);
        }

        cursor = match tiles_tree.tiles.get(tile_id) {
            Some(egui_tiles::Tile::Container(egui_tiles::Container::Tabs(tabs))) => tabs.active,
            _ => None,
        };
    }

    for active_tile_id in tiles_tree.active_tiles().into_iter().rev() {
        let mut cursor: Option<TileId> = Some(active_tile_id);
        while let Some(tile_id) = cursor {
            if let Some(state) = graph_app
                .workspace
                .graph_runtime
                .frame_tile_groups
                .get(&tile_id)
            {
                return Some(state.frame_anchor);
            }
            cursor = tiles_tree.tiles.parent_of(tile_id);
        }
    }
    None
}

fn frame_name_for_anchor(graph_app: &GraphBrowserApp, frame_anchor: NodeKey) -> Option<String> {
    let frame_url = graph_app
        .domain_graph()
        .get_node(frame_anchor)?
        .url()
        .to_string();
    GraphBrowserApp::resolve_frame_route(&frame_url)
}

fn sync_current_frame_from_active_tile_group(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &Tree<TileKind>,
) {
    let Some(frame_anchor) = active_frame_group_anchor(graph_app, tiles_tree) else {
        return;
    };
    let Some(frame_name) = frame_name_for_anchor(graph_app, frame_anchor) else {
        return;
    };
    if graph_app.current_frame_name() == Some(frame_name.as_str()) {
        return;
    }

    let member_keys =
        persistence_ops::ordered_live_frame_member_keys_for_anchor(graph_app, frame_anchor);
    if member_keys.is_empty() {
        return;
    }

    graph_app.note_frame_activated(&frame_name, member_keys);
}

fn related_tab_nodes_for_changed_groups(
    tab_groups_before: &HashMap<NodeKey, TileId>,
    tab_group_nodes_before: &HashMap<TileId, Vec<NodeKey>>,
    tab_groups_after: &HashMap<NodeKey, TileId>,
    tab_group_nodes_after: &HashMap<TileId, Vec<NodeKey>>,
    moved_nodes: &HashSet<NodeKey>,
) -> HashSet<NodeKey> {
    let mut related = moved_nodes.clone();
    for node_key in moved_nodes {
        if let Some(group_id) = tab_groups_before.get(node_key)
            && let Some(peers) = tab_group_nodes_before.get(group_id)
        {
            related.extend(peers.iter().copied());
        }
        if let Some(group_id) = tab_groups_after.get(node_key)
            && let Some(peers) = tab_group_nodes_after.get(group_id)
        {
            related.extend(peers.iter().copied());
        }
    }
    related
}

fn touched_pane_ids_for_node_keys(
    tiles_tree: &Tree<TileKind>,
    node_keys: &HashSet<NodeKey>,
) -> HashSet<PaneId> {
    tiles_tree
        .tiles
        .iter()
        .filter_map(|(_, tile)| match tile {
            egui_tiles::Tile::Pane(tile_kind)
                if tile_kind
                    .node_state()
                    .is_some_and(|state| node_keys.contains(&state.node)) =>
            {
                Some(tile_kind.pane_id())
            }
            _ => None,
        })
        .collect()
}

fn existing_pane_ids_in_tree(tiles_tree: &Tree<TileKind>) -> HashSet<PaneId> {
    tiles_tree
        .tiles
        .iter()
        .filter_map(|(_, tile)| match tile {
            egui_tiles::Tile::Pane(tile_kind) => Some(tile_kind.pane_id()),
            _ => None,
        })
        .collect()
}

fn changed_tab_nodes(
    tab_groups_before: &HashMap<NodeKey, TileId>,
    tab_groups_after: &HashMap<NodeKey, TileId>,
) -> HashSet<NodeKey> {
    tab_groups_before
        .keys()
        .chain(tab_groups_after.keys())
        .copied()
        .filter(|node_key| tab_groups_before.get(node_key) != tab_groups_after.get(node_key))
        .collect()
}

fn runtime_group_membership_key(pane_ids: &[PaneId]) -> BTreeSet<String> {
    pane_ids.iter().map(ToString::to_string).collect()
}

fn repaired_runtime_group_for_existing_panes(
    group: crate::app::RuntimeTabGroupMetadata,
    existing_pane_ids: &HashSet<PaneId>,
) -> Option<crate::app::RuntimeTabGroupMetadata> {
    let pane_ids: Vec<_> = group
        .pane_ids
        .into_iter()
        .filter(|pane_id| existing_pane_ids.contains(pane_id))
        .collect();
    if pane_ids.is_empty() {
        return None;
    }

    let active_pane_id = group
        .active_pane_id
        .filter(|pane_id| pane_ids.contains(pane_id))
        .or_else(|| pane_ids.first().copied());

    Some(crate::app::RuntimeTabGroupMetadata {
        group_id: group.group_id,
        pane_ids,
        active_pane_id,
    })
}

fn reconcile_runtime_semantics_for_touched_panes(
    current: Option<crate::app::RuntimeFrameTabSemantics>,
    derived: Option<crate::app::RuntimeFrameTabSemantics>,
    touched_pane_ids: &HashSet<PaneId>,
    existing_pane_ids: &HashSet<PaneId>,
) -> Option<crate::app::RuntimeFrameTabSemantics> {
    if touched_pane_ids.is_empty() {
        return current.or(derived);
    }

    match (current, derived) {
        (None, derived) => derived,
        (Some(current), None) => {
            let tab_groups: Vec<_> = current
                .tab_groups
                .into_iter()
                .filter_map(|group| {
                    if group
                        .pane_ids
                        .iter()
                        .any(|pane_id| touched_pane_ids.contains(pane_id))
                    {
                        repaired_runtime_group_for_existing_panes(group, existing_pane_ids)
                    } else {
                        Some(group)
                    }
                })
                .collect();
            if tab_groups.is_empty() {
                None
            } else {
                Some(crate::app::RuntimeFrameTabSemantics {
                    version: current.version,
                    tab_groups,
                })
            }
        }
        (Some(current), Some(derived)) => {
            let mut derived_by_membership: HashMap<
                BTreeSet<String>,
                crate::app::RuntimeTabGroupMetadata,
            > = derived
                .tab_groups
                .into_iter()
                .filter(|group| {
                    group
                        .pane_ids
                        .iter()
                        .any(|pane_id| touched_pane_ids.contains(pane_id))
                })
                .map(|group| (runtime_group_membership_key(&group.pane_ids), group))
                .collect();
            let mut tab_groups = Vec::new();

            for group in current.tab_groups {
                if !group
                    .pane_ids
                    .iter()
                    .any(|pane_id| touched_pane_ids.contains(pane_id))
                {
                    tab_groups.push(group);
                    continue;
                }

                let key = runtime_group_membership_key(&group.pane_ids);
                if let Some(derived_group) = derived_by_membership.remove(&key) {
                    tab_groups.push(crate::app::RuntimeTabGroupMetadata {
                        group_id: group.group_id,
                        pane_ids: derived_group.pane_ids,
                        active_pane_id: derived_group.active_pane_id,
                    });
                } else if let Some(repaired_group) =
                    repaired_runtime_group_for_existing_panes(group, existing_pane_ids)
                {
                    let repaired_key = runtime_group_membership_key(&repaired_group.pane_ids);
                    if let Some(derived_group) = derived_by_membership.remove(&repaired_key) {
                        tab_groups.push(crate::app::RuntimeTabGroupMetadata {
                            group_id: repaired_group.group_id,
                            pane_ids: derived_group.pane_ids,
                            active_pane_id: derived_group
                                .active_pane_id
                                .or(repaired_group.active_pane_id),
                        });
                    } else {
                        tab_groups.push(repaired_group);
                    }
                }
            }

            tab_groups.extend(derived_by_membership.into_values());
            if tab_groups.is_empty() {
                None
            } else {
                Some(crate::app::RuntimeFrameTabSemantics {
                    version: derived.version,
                    tab_groups,
                })
            }
        }
    }
}

fn sync_runtime_semantics_after_tab_drop(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &Tree<TileKind>,
    tab_groups_before: &HashMap<NodeKey, TileId>,
    tab_group_nodes_before: &HashMap<TileId, Vec<NodeKey>>,
    tab_groups_after: &HashMap<NodeKey, TileId>,
    tab_group_nodes_after: &HashMap<TileId, Vec<NodeKey>>,
    changed_nodes: &HashSet<NodeKey>,
) {
    let touched_nodes = related_tab_nodes_for_changed_groups(
        tab_groups_before,
        tab_group_nodes_before,
        tab_groups_after,
        tab_group_nodes_after,
        changed_nodes,
    );
    let touched_pane_ids = touched_pane_ids_for_node_keys(tiles_tree, &touched_nodes);
    if touched_pane_ids.is_empty() {
        return;
    }

    let current = graph_app.current_frame_tab_semantics().cloned();
    let derived = persistence_ops::derive_runtime_frame_tab_semantics_from_tree(tiles_tree);
    let existing_pane_ids = existing_pane_ids_in_tree(tiles_tree);
    graph_app.set_current_frame_tab_semantics(reconcile_runtime_semantics_for_touched_panes(
        current,
        derived,
        &touched_pane_ids,
        &existing_pane_ids,
    ));
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
    let tab_group_nodes_before = tile_grouping::tab_group_nodes(tiles_tree);
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
    let pending_tile_drop_edit = behavior.take_pending_tile_drop_edit();
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
                    let _ = toolbar_routing::request_radial_menu_toggle(
                        graph_app,
                        CommandBarFocusTarget::new(
                            None,
                            active_frame_group_anchor(graph_app, tiles_tree),
                        ),
                    );
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
    let changed_tab_nodes = changed_tab_nodes(&tab_groups_before, &tab_groups_after);
    post_render_intents.extend(tile_grouping::user_grouped_intents_for_tab_group_moves(
        &tab_groups_before,
        &tab_groups_after,
        &tab_group_nodes_after,
        &tab_drag_stopped_nodes,
    ));
    let changed_semantic_nodes = changed_tab_nodes
        .union(&tab_drag_stopped_nodes)
        .copied()
        .collect::<HashSet<_>>();
    if pending_tile_drop_edit || !changed_semantic_nodes.is_empty() {
        sync_runtime_semantics_after_tab_drop(
            graph_app,
            tiles_tree,
            &tab_groups_before,
            &tab_group_nodes_before,
            &tab_groups_after,
            &tab_group_nodes_after,
            &changed_semantic_nodes,
        );
    }
    persistence_ops::refresh_frame_tile_group_runtime(graph_app, tiles_tree);
    sync_current_frame_from_active_tile_group(graph_app, tiles_tree);
    if pending_tile_drop_edit {
        post_render_intents.extend(
            persistence_ops::frame_layout_sync_intents_for_registered_frame_groups(
                graph_app, tiles_tree,
            ),
        );
    }

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
        active_context_return_target, reconcile_runtime_semantics_for_touched_panes,
        render_tile_tree_and_collect_outputs, should_summon_radial_palette_on_secondary_click,
        sync_current_frame_from_active_tile_group, sync_runtime_semantics_after_tab_drop,
    };
    use crate::app::{
        GraphBrowserApp, GraphIntent, GraphViewId, RuntimeFrameTabSemantics,
        RuntimeTabGroupMetadata, SurfaceHostId, ToolSurfaceReturnTarget, WorkbenchIntent,
        WorkbenchLayoutConstraint,
    };
    use crate::graph::NodeKey;
    use crate::shell::desktop::runtime::control_panel::ControlPanel;
    #[cfg(feature = "diagnostics")]
    use crate::shell::desktop::runtime::diagnostics::DiagnosticsState;
    use crate::shell::desktop::workbench::pane_model::{
        GraphPaneRef, NodePaneState, PaneId, ToolPaneRef, ToolPaneState,
    };
    use crate::shell::desktop::workbench::tile_grouping;
    use crate::shell::desktop::workbench::tile_kind::TileKind;
    use crate::shell::desktop::workbench::ux_tree;
    use egui_tiles::{Container, Tile, Tiles, Tree};
    use tempfile::TempDir;

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

    #[test]
    fn reconcile_runtime_semantics_for_touched_panes_drops_only_edited_groups() {
        let a = PaneId::new();
        let b = PaneId::new();
        let c = PaneId::new();
        let d = PaneId::new();
        let untouched_group_id = uuid::Uuid::new_v4();
        let current = RuntimeFrameTabSemantics {
            version: 1,
            tab_groups: vec![
                RuntimeTabGroupMetadata {
                    group_id: uuid::Uuid::new_v4(),
                    pane_ids: vec![a, b],
                    active_pane_id: Some(a),
                },
                RuntimeTabGroupMetadata {
                    group_id: untouched_group_id,
                    pane_ids: vec![c, d],
                    active_pane_id: Some(c),
                },
            ],
        };
        let touched = HashSet::from([a, b]);

        let existing = HashSet::from([c, d]);

        let reconciled =
            reconcile_runtime_semantics_for_touched_panes(Some(current), None, &touched, &existing)
                .expect("untouched group should remain");

        assert_eq!(reconciled.tab_groups.len(), 1);
        assert_eq!(reconciled.tab_groups[0].group_id, untouched_group_id);
        assert_eq!(reconciled.tab_groups[0].pane_ids, vec![c, d]);
    }

    #[test]
    fn reconcile_runtime_semantics_for_touched_panes_preserves_exact_group_id() {
        let a = PaneId::new();
        let b = PaneId::new();
        let preserved_group_id = uuid::Uuid::new_v4();
        let current = RuntimeFrameTabSemantics {
            version: 1,
            tab_groups: vec![RuntimeTabGroupMetadata {
                group_id: preserved_group_id,
                pane_ids: vec![a, b],
                active_pane_id: Some(a),
            }],
        };
        let derived = RuntimeFrameTabSemantics {
            version: 1,
            tab_groups: vec![RuntimeTabGroupMetadata {
                group_id: uuid::Uuid::new_v4(),
                pane_ids: vec![b, a],
                active_pane_id: Some(b),
            }],
        };
        let touched = HashSet::from([a, b]);

        let existing = HashSet::from([a, b]);

        let reconciled = reconcile_runtime_semantics_for_touched_panes(
            Some(current),
            Some(derived),
            &touched,
            &existing,
        )
        .expect("reconciled semantics");

        assert_eq!(reconciled.tab_groups.len(), 1);
        assert_eq!(reconciled.tab_groups[0].group_id, preserved_group_id);
        assert_eq!(reconciled.tab_groups[0].pane_ids, vec![b, a]);
        assert_eq!(reconciled.tab_groups[0].active_pane_id, Some(b));
    }

    #[test]
    fn reconcile_runtime_semantics_for_touched_panes_preserves_surviving_single_pane() {
        let a = PaneId::new();
        let b = PaneId::new();
        let preserved_group_id = uuid::Uuid::new_v4();
        let current = RuntimeFrameTabSemantics {
            version: 1,
            tab_groups: vec![RuntimeTabGroupMetadata {
                group_id: preserved_group_id,
                pane_ids: vec![a, b],
                active_pane_id: Some(a),
            }],
        };
        let touched = HashSet::from([a]);
        let existing = HashSet::from([a]);

        let reconciled =
            reconcile_runtime_semantics_for_touched_panes(Some(current), None, &touched, &existing)
                .expect("surviving pane-rest group should remain");

        assert_eq!(reconciled.tab_groups.len(), 1);
        assert_eq!(reconciled.tab_groups[0].group_id, preserved_group_id);
        assert_eq!(reconciled.tab_groups[0].pane_ids, vec![a]);
        assert_eq!(reconciled.tab_groups[0].active_pane_id, Some(a));
    }

    #[test]
    fn sync_runtime_semantics_preserves_single_surviving_pane_after_group_collapse() {
        let mut app = GraphBrowserApp::new_for_testing();
        let a = app.add_node_and_sync(
            "https://simplify-preserve-a.example".into(),
            euclid::default::Point2D::new(0.0, 0.0),
        );

        let mut tiles = Tiles::default();
        let a_tile = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(a)));
        let root = tiles.insert_tab_tile(vec![a_tile]);
        let tree = Tree::new("simplify_preserves_semantics", root, tiles);

        let pane_a = match tree.tiles.get(a_tile) {
            Some(Tile::Pane(tile)) => tile.pane_id(),
            other => panic!("expected pane tile for a, got {other:?}"),
        };
        let removed_pane = PaneId::new();
        app.set_current_frame_tab_semantics(Some(RuntimeFrameTabSemantics {
            version: 1,
            tab_groups: vec![RuntimeTabGroupMetadata {
                group_id: uuid::Uuid::new_v4(),
                pane_ids: vec![pane_a, removed_pane],
                active_pane_id: Some(pane_a),
            }],
        }));

        let before = HashMap::from([(a, root)]);
        let after = HashMap::new();
        let before_nodes = HashMap::from([(root, vec![a])]);
        let after_nodes = HashMap::new();
        let changed_nodes = HashSet::from([a]);

        sync_runtime_semantics_after_tab_drop(
            &mut app,
            &tree,
            &before,
            &before_nodes,
            &after,
            &after_nodes,
            &changed_nodes,
        );

        let reconciled = app
            .current_frame_tab_semantics()
            .expect("semantic overlay should remain after structural collapse");
        assert_eq!(reconciled.tab_groups.len(), 1);
        assert_eq!(reconciled.tab_groups[0].pane_ids, vec![pane_a]);
        assert_eq!(reconciled.tab_groups[0].active_pane_id, Some(pane_a));
    }

    #[test]
    fn save_restore_roundtrip_preserves_pane_rest_semantics_after_simplify_reapply() {
        let dir = TempDir::new().unwrap();
        let mut app = GraphBrowserApp::new_from_dir(dir.path().to_path_buf());
        let a = app.add_node_and_sync(
            "https://simplify-roundtrip-a.example".into(),
            euclid::default::Point2D::new(0.0, 0.0),
        );
        let b = app.add_node_and_sync(
            "https://simplify-roundtrip-b.example".into(),
            euclid::default::Point2D::new(1.0, 0.0),
        );

        let mut tiles = Tiles::default();
        let a_tile = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(a)));
        let b_tile = tiles.insert_pane(TileKind::Node(NodePaneState::for_node(b)));
        let root = tiles.insert_tab_tile(vec![a_tile, b_tile]);
        let mut tree = Tree::new("simplify_roundtrip_tree", root, tiles);

        let semantics =
            crate::shell::desktop::ui::persistence_ops::derive_runtime_frame_tab_semantics_from_tree(
                &tree,
            )
            .expect("initial runtime semantics");
        app.set_current_frame_tab_semantics(Some(semantics));
        crate::shell::desktop::ui::persistence_ops::save_named_frame_bundle(
            &mut app,
            "workspace-simplify-roundtrip-before",
            &tree,
        )
        .expect("save initial frame");

        let before_groups = tile_grouping::node_pane_tab_group_memberships(&tree);
        let before_group_nodes = tile_grouping::tab_group_nodes(&tree);
        let _ = tree.remove_recursively(b_tile);
        tree.simplify(&egui_tiles::SimplificationOptions::default());
        let after_groups = tile_grouping::node_pane_tab_group_memberships(&tree);
        let after_group_nodes = tile_grouping::tab_group_nodes(&tree);
        let changed_nodes = HashSet::from([a, b]);

        sync_runtime_semantics_after_tab_drop(
            &mut app,
            &tree,
            &before_groups,
            &before_group_nodes,
            &after_groups,
            &after_group_nodes,
            &changed_nodes,
        );

        crate::shell::desktop::ui::persistence_ops::save_named_frame_bundle(
            &mut app,
            "workspace-simplify-roundtrip-after",
            &tree,
        )
        .expect("save simplified frame");

        let bundle = crate::shell::desktop::ui::persistence_ops::load_named_frame_bundle(
            &app,
            "workspace-simplify-roundtrip-after",
        )
        .expect("load simplified frame");
        assert_eq!(
            crate::shell::desktop::ui::persistence_ops::saved_tab_node_keys_for_frame_bundle(
                &app, &bundle
            ),
            HashSet::from([a])
        );
        let tab_groups =
            crate::shell::desktop::ui::persistence_ops::semantic_tab_groups_for_frame(&bundle);
        assert_eq!(tab_groups.len(), 1);
        assert_eq!(tab_groups[0].pane_ids.len(), 1);

        let (restored_tree, _) =
            crate::shell::desktop::ui::persistence_ops::restore_runtime_tree_from_frame_bundle(
                &app, &bundle,
            )
            .expect("restore simplified frame");
        let restored_semantics =
            crate::shell::desktop::ui::persistence_ops::runtime_frame_tab_semantics_from_restored_bundle(
                &app,
                &bundle,
                &restored_tree,
            )
            .expect("restored pane-rest semantics");
        assert_eq!(restored_semantics.tab_groups.len(), 1);
        assert_eq!(restored_semantics.tab_groups[0].pane_ids.len(), 1);
    }

    #[test]
    fn sync_current_frame_from_active_tile_group_tracks_active_frame_group() {
        let mut app = GraphBrowserApp::new_for_testing();
        let a = app.add_node_and_sync(
            "https://frame-a-left.example".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );
        let b = app.add_node_and_sync(
            "https://frame-a-right.example".to_string(),
            euclid::default::Point2D::new(1.0, 0.0),
        );
        let c = app.add_node_and_sync(
            "https://frame-b-left.example".to_string(),
            euclid::default::Point2D::new(2.0, 0.0),
        );
        let d = app.add_node_and_sync(
            "https://frame-b-right.example".to_string(),
            euclid::default::Point2D::new(3.0, 0.0),
        );

        let mut frame_a_tiles = Tiles::default();
        let frame_a_left = frame_a_tiles.insert_pane(TileKind::Node(a.into()));
        let frame_a_right = frame_a_tiles.insert_pane(TileKind::Node(b.into()));
        let frame_a_root = frame_a_tiles.insert_tab_tile(vec![frame_a_left, frame_a_right]);
        let frame_a_tree = Tree::new("frame_a_seed", frame_a_root, frame_a_tiles);
        let frame_a_anchor =
            app.sync_named_workbench_frame_graph_representation("workspace-alpha", &frame_a_tree);

        let mut frame_b_tiles = Tiles::default();
        let frame_b_left = frame_b_tiles.insert_pane(TileKind::Node(c.into()));
        let frame_b_right = frame_b_tiles.insert_pane(TileKind::Node(d.into()));
        let frame_b_root = frame_b_tiles.insert_tab_tile(vec![frame_b_left, frame_b_right]);
        let frame_b_tree = Tree::new("frame_b_seed", frame_b_root, frame_b_tiles);
        let frame_b_anchor =
            app.sync_named_workbench_frame_graph_representation("workspace-beta", &frame_b_tree);

        app.apply_reducer_intents([GraphIntent::RecordFrameLayoutHint {
            frame: frame_b_anchor,
            hint: crate::graph::FrameLayoutHint::SplitHalf {
                first: app
                    .domain_graph()
                    .get_node(c)
                    .expect("frame b first")
                    .id
                    .to_string(),
                second: app
                    .domain_graph()
                    .get_node(d)
                    .expect("frame b second")
                    .id
                    .to_string(),
                orientation: crate::graph::SplitOrientation::Vertical,
            },
        }]);

        let mut tiles = Tiles::default();
        let (frame_a_tabs, _, _) =
            crate::shell::desktop::ui::persistence_ops::materialize_frame_tile_group_tabs(
                &app,
                frame_a_anchor,
                &mut tiles,
            )
            .expect("frame a tabs");
        let frame_a_group = tiles.insert_tab_tile(frame_a_tabs);

        let (frame_b_tabs, _, _) =
            crate::shell::desktop::ui::persistence_ops::materialize_frame_tile_group_tabs(
                &app,
                frame_b_anchor,
                &mut tiles,
            )
            .expect("frame b tabs");
        let frame_b_group = tiles.insert_tab_tile(frame_b_tabs);

        let root = tiles.insert_tab_tile(vec![frame_a_group, frame_b_group]);
        let mut tree = Tree::new("active_frame_group_sync", root, tiles);
        if let Some(Tile::Container(Container::Tabs(tabs))) = tree.tiles.get_mut(root) {
            tabs.set_active(frame_b_group);
        }

        crate::shell::desktop::ui::persistence_ops::register_frame_tile_group_runtime(
            &mut app,
            &tree,
            frame_a_group,
            frame_a_anchor,
        );
        crate::shell::desktop::ui::persistence_ops::register_frame_tile_group_runtime(
            &mut app,
            &tree,
            frame_b_group,
            frame_b_anchor,
        );

        sync_current_frame_from_active_tile_group(&mut app, &tree);

        assert_eq!(app.current_frame_name(), Some("workspace-beta"));
    }
}
