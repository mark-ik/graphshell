use super::*;
use crate::shell::desktop::workbench::graph_tree_commands;
use crate::shell::desktop::workbench::graph_tree_dual_write as dual_write;
use crate::util::CoordBridge;

pub(super) fn handle_open_tool_pane_intent(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    kind: ToolPaneState,
) {
    let focused_before = active_tool_surface_return_target(tiles_tree);
    if matches!(
        kind,
        ToolPaneState::Settings | ToolPaneState::HistoryManager | ToolPaneState::Diagnostics
    ) {
        maybe_capture_tool_surface_return_target(graph_app, tiles_tree);
    }
    let kind_after = kind.clone();
    open_or_focus_tool_pane_if_available(tiles_tree, kind);

    let focused_after = active_tool_surface_return_target(tiles_tree);
    let transitioned_to_target_tool = matches!(
        focused_after,
        Some(ToolSurfaceReturnTarget::Tool(ref active_kind)) if *active_kind == kind_after
    );

    if transitioned_to_target_tool && focused_before != focused_after {
        emit_event(DiagnosticEvent::MessageReceived {
            channel_id: CHANNEL_UX_NAVIGATION_TRANSITION,
            latency_us: 0,
        });
    }
}

pub(super) fn handle_close_tool_pane_intent(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    kind: ToolPaneState,
    restore_previous_focus: bool,
    focus_handoff: &FocusHandoffPolicy,
) {
    #[cfg(feature = "diagnostics")]
    {
        let focus_before = crate::shell::desktop::ui::gui::workbench_runtime_focus_state(
            graph_app, tiles_tree, None, None, false,
        );
        let closed = tile_view_ops::close_tool_pane(tiles_tree, kind);
        if closed && restore_previous_focus {
            let restored = restore_tool_surface_focus_or_ensure_active_tile(
                graph_app,
                tiles_tree,
                focus_handoff,
            );
            let focus_after = crate::shell::desktop::ui::gui::workbench_runtime_focus_state(
                graph_app, tiles_tree, None, None, false,
            );
            let has_valid_active_target = active_tool_surface_return_target(tiles_tree).is_some();
            if restored || (focus_before != focus_after && has_valid_active_target) {
                emit_event(DiagnosticEvent::MessageReceived {
                    channel_id: CHANNEL_UX_NAVIGATION_TRANSITION,
                    latency_us: 0,
                });
            }
        } else if closed {
            graph_app.set_pending_tool_surface_return_target(None);
            let focus_after = crate::shell::desktop::ui::gui::workbench_runtime_focus_state(
                graph_app, tiles_tree, None, None, false,
            );
            if focus_before != focus_after {
                emit_event(DiagnosticEvent::MessageReceived {
                    channel_id: CHANNEL_UX_NAVIGATION_TRANSITION,
                    latency_us: 0,
                });
            }
        } else if restore_previous_focus {
            emit_event(DiagnosticEvent::MessageSent {
                channel_id: CHANNEL_UX_NAVIGATION_VIOLATION,
                byte_len: 1,
            });
        }
    }
}

pub(super) fn handle_close_pane_intent(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    mut graph_tree: Option<&mut graph_tree::GraphTree<NodeKey>>,
    pane: PaneId,
    restore_previous_focus: bool,
    focus_handoff: &FocusHandoffPolicy,
) {
    let focus_before = crate::shell::desktop::ui::gui::workbench_runtime_focus_state(
        graph_app, tiles_tree, None, None, false,
    );
    // Resolve node key before closing so we can sync GraphTree.
    let node_key = pane_id_for_node_reverse(tiles_tree, pane);
    let closed = if let Some(gt) = graph_tree.as_deref_mut() {
        dual_write::close_pane(tiles_tree, gt, pane, node_key)
    } else {
        tile_view_ops::close_pane(tiles_tree, pane)
    };

    if closed && restore_previous_focus {
        let restored =
            restore_tool_surface_focus_or_ensure_active_tile(graph_app, tiles_tree, focus_handoff);
        let focus_after = crate::shell::desktop::ui::gui::workbench_runtime_focus_state(
            graph_app, tiles_tree, None, None, false,
        );
        let has_valid_active_target = active_tool_surface_return_target(tiles_tree).is_some();
        if restored || (focus_before != focus_after && has_valid_active_target) {
            emit_event(DiagnosticEvent::MessageReceived {
                channel_id: CHANNEL_UX_NAVIGATION_TRANSITION,
                latency_us: 0,
            });
        }
    } else if closed {
        graph_app.set_pending_tool_surface_return_target(None);
        let ensured = if let Some(gt) = graph_tree.as_deref_mut() {
            dual_write::ensure_active_tile(tiles_tree, gt)
        } else {
            tile_view_ops::ensure_active_tile(tiles_tree)
        };
        let focus_after = crate::shell::desktop::ui::gui::workbench_runtime_focus_state(
            graph_app, tiles_tree, None, None, false,
        );
        if ensured || focus_before != focus_after {
            emit_event(DiagnosticEvent::MessageReceived {
                channel_id: CHANNEL_UX_NAVIGATION_TRANSITION,
                latency_us: 0,
            });
        }
    } else if restore_previous_focus {
        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_UX_NAVIGATION_VIOLATION,
            byte_len: 1,
        });
    }
}

/// Reverse-lookup: find the NodeKey for a given PaneId (for pre-close sync).
fn pane_id_for_node_reverse(tiles_tree: &Tree<TileKind>, pane: PaneId) -> Option<NodeKey> {
    tiles_tree.tiles.iter().find_map(|(_, tile)| match tile {
        Tile::Pane(kind) if kind.pane_id() == pane => kind.node_state().map(|s| s.node),
        _ => None,
    })
}

/// Dismiss a tile: close it from the tree and demote its node to `Cold`.
///
/// Unlike `ClosePane`, this does **not** remove any graph edges.  The node
/// stays in its durable graphlet; only the tile presentation is removed and
/// the node lifecycle transitions to `NodeLifecycle::Cold`.  If the pane does
/// not carry a node (e.g. it is a graph or tool pane), the tile is still
/// closed but no lifecycle transition is emitted.
pub(super) fn handle_dismiss_tile_intent(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    mut graph_tree: Option<&mut graph_tree::GraphTree<NodeKey>>,
    pane: PaneId,
) {
    // Resolve the node key before closing the pane so we can still find the
    // tile in the tree.
    let node_key = pane_id_for_node_reverse(tiles_tree, pane);

    let closed = if let Some(gt) = graph_tree.as_deref_mut() {
        dual_write::close_pane(tiles_tree, gt, pane, node_key)
    } else {
        tile_view_ops::close_pane(tiles_tree, pane)
    };

    if closed {
        if let Some(key) = node_key {
            graph_app.demote_node_to_cold_with_cause(key, LifecycleCause::ExplicitClose);
        }
        if let Some(gt) = graph_tree.as_deref_mut() {
            dual_write::ensure_active_tile(tiles_tree, gt);
        } else {
            tile_view_ops::ensure_active_tile(tiles_tree);
        }
    }
}

fn node_has_workbench_presentation(tiles_tree: &Tree<TileKind>, node_key: NodeKey) -> bool {
    tiles_tree.tiles.iter().any(|(_, tile)| {
        matches!(tile, Tile::Pane(TileKind::Node(state)) if state.node == node_key)
            || matches!(
                tile,
                Tile::Pane(TileKind::Pane(
                    crate::shell::desktop::workbench::pane_model::PaneViewState::Node(state),
                )) if state.node == node_key
            )
    })
}

fn focus_node_presentation(tiles_tree: &mut Tree<TileKind>, node_key: NodeKey) -> bool {
    tiles_tree.make_active(|_, tile| {
        matches!(tile, Tile::Pane(TileKind::Node(state)) if state.node == node_key)
            || matches!(
                tile,
                Tile::Pane(TileKind::Pane(
                    crate::shell::desktop::workbench::pane_model::PaneViewState::Node(state),
                )) if state.node == node_key
            )
    })
}

fn active_visible_graph_view_id(tiles_tree: &Tree<TileKind>) -> Option<crate::app::GraphViewId> {
    tiles_tree.active_tiles().into_iter().find_map(|tile_id| {
        let tile = tiles_tree.tiles.get(tile_id)?;
        match tile {
            Tile::Pane(TileKind::Graph(graph_ref)) => Some(graph_ref.graph_view_id),
            Tile::Pane(TileKind::Pane(
                crate::shell::desktop::workbench::pane_model::PaneViewState::Graph(graph_ref),
            )) => Some(graph_ref.graph_view_id),
            _ => None,
        }
    })
}

fn graph_view_id_for_navigation(
    graph_app: &GraphBrowserApp,
    tiles_tree: &Tree<TileKind>,
) -> Option<crate::app::GraphViewId> {
    active_visible_graph_view_id(tiles_tree)
        .or_else(|| {
            tiles_tree.tiles.iter().find_map(|(_, tile)| match tile {
                Tile::Pane(TileKind::Graph(graph_ref)) => Some(graph_ref.graph_view_id),
                Tile::Pane(TileKind::Pane(
                    crate::shell::desktop::workbench::pane_model::PaneViewState::Graph(graph_ref),
                )) => Some(graph_ref.graph_view_id),
                _ => None,
            })
        })
        .or(graph_app.workspace.graph_runtime.focused_view)
        .or_else(|| {
            graph_app
                .workspace
                .graph_runtime
                .views
                .keys()
                .next()
                .copied()
        })
}

fn offscreen_visible_graph_view_for_node(
    graph_app: &GraphBrowserApp,
    tiles_tree: &Tree<TileKind>,
    node_key: NodeKey,
) -> Option<crate::app::GraphViewId> {
    let view_id = active_visible_graph_view_id(tiles_tree)?;
    let canvas_rect = graph_app
        .workspace
        .graph_runtime
        .graph_view_canvas_rects
        .get(&view_id)?;
    let position = graph_app.domain_graph().node_projected_position(node_key)?;
    (!canvas_rect.contains(position.to_pos2())).then_some(view_id)
}

fn pane_id_for_node(tiles_tree: &Tree<TileKind>, node_key: NodeKey) -> Option<PaneId> {
    tiles_tree.tiles.iter().find_map(|(_, tile)| match tile {
        Tile::Pane(TileKind::Node(state)) if state.node == node_key => Some(state.pane_id),
        Tile::Pane(TileKind::Pane(
            crate::shell::desktop::workbench::pane_model::PaneViewState::Node(state),
        )) if state.node == node_key => Some(state.pane_id),
        _ => None,
    })
}

fn set_navigator_row_selection(graph_app: &mut GraphBrowserApp, row_key: Option<String>) {
    if let Some(row_key) = row_key {
        graph_app.set_navigator_selected_rows([row_key]);
    }
}

pub(super) fn handle_select_navigator_node_intent(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    _graph_tree: Option<&mut graph_tree::GraphTree<NodeKey>>,
    node_key: NodeKey,
    row_key: Option<String>,
) {
    set_navigator_row_selection(graph_app, row_key);
    graph_app.apply_reducer_intents([GraphIntent::SelectNode {
        key: node_key,
        multi_select: false,
    }]);
    if let Some(view_id) = offscreen_visible_graph_view_for_node(graph_app, tiles_tree, node_key) {
        graph_app.request_camera_command_for_view(
            Some(view_id),
            crate::app::CameraCommand::FitSelection,
        );
    }
}

pub(super) fn handle_activate_navigator_node_intent(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    mut graph_tree: Option<&mut graph_tree::GraphTree<NodeKey>>,
    node_key: NodeKey,
    row_key: Option<String>,
) {
    set_navigator_row_selection(graph_app, row_key);
    if !graph_app.focused_selection().contains(&node_key) {
        graph_app.apply_reducer_intents([GraphIntent::SelectNode {
            key: node_key,
            multi_select: false,
        }]);
    }

    if node_has_workbench_presentation(tiles_tree, node_key) {
        let activated = focus_node_presentation(tiles_tree, node_key);
        if activated {
            if let Some(gt) = graph_tree.as_deref_mut() {
                graph_tree_commands::activate_node(gt, node_key);
            }
        }
        return;
    }

    let lifecycle = graph_app
        .domain_graph()
        .get_node(node_key)
        .map(|node| node.lifecycle);
    if lifecycle == Some(crate::graph::NodeLifecycle::Cold) {
        if let Some(gt) = graph_tree.as_deref_mut() {
            dual_write::open_or_focus_node(tiles_tree, gt, graph_app, node_key, None);
        } else {
            tile_view_ops::open_node_with_graphlet_routing(tiles_tree, graph_app, node_key);
        }
        return;
    }

    if let Some(view_id) = graph_view_id_for_navigation(graph_app, tiles_tree) {
        tile_view_ops::open_or_focus_graph_pane_with_mode(tiles_tree, view_id, TileOpenMode::Tab);
        graph_app.request_camera_command_for_view(
            Some(view_id),
            crate::app::CameraCommand::FitSelection,
        );
    }
}

pub(super) fn handle_dismiss_navigator_node_intent(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    graph_tree: Option<&mut graph_tree::GraphTree<NodeKey>>,
    node_key: NodeKey,
    row_key: Option<String>,
) {
    set_navigator_row_selection(graph_app, row_key);
    if let Some(pane) = pane_id_for_node(tiles_tree, node_key) {
        handle_dismiss_tile_intent(graph_app, tiles_tree, graph_tree, pane);
        return;
    }

    if graph_app
        .domain_graph()
        .get_node(node_key)
        .is_some_and(|node| node.lifecycle != crate::graph::NodeLifecycle::Cold)
    {
        graph_app.demote_node_to_cold_with_cause(node_key, LifecycleCause::ExplicitClose);
    }
}

pub(super) fn handle_switch_navigator_node_surface_intent(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    mut graph_tree: Option<&mut graph_tree::GraphTree<NodeKey>>,
    node_key: NodeKey,
    row_key: Option<String>,
) {
    set_navigator_row_selection(graph_app, row_key);
    if !graph_app.focused_selection().contains(&node_key) {
        graph_app.apply_reducer_intents([GraphIntent::SelectNode {
            key: node_key,
            multi_select: false,
        }]);
    }

    if matches!(active_visible_graph_view_id(tiles_tree), Some(_)) {
        handle_activate_navigator_node_intent(graph_app, tiles_tree, graph_tree, node_key, None);
        return;
    }

    if let Some(view_id) = graph_view_id_for_navigation(graph_app, tiles_tree) {
        tile_view_ops::open_or_focus_graph_pane_with_mode(tiles_tree, view_id, TileOpenMode::Tab);
        graph_app.request_camera_command_for_view(
            Some(view_id),
            crate::app::CameraCommand::FitSelection,
        );
    } else {
        handle_activate_navigator_node_intent(
            graph_app,
            tiles_tree,
            graph_tree.as_deref_mut(),
            node_key,
            None,
        );
    }
}

/// Open a node in a pane with graphlet-aware routing and graphlet growth.
///
/// Priority order:
/// 1. Node already has a non-floating tile → focus it.
/// 2. Node has a warm durable graphlet peer → join that peer's tab container.
/// 3. Explicit pane target is inside a tab container → join that container and
///    create a [`GraphIntent::CreateUserGroupedEdge`] to make membership durable.
/// 4. Fallback: standard [`tile_view_ops::open_or_focus_node_pane`].
pub(super) fn handle_open_node_in_pane_intent(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    graph_tree: Option<&mut graph_tree::GraphTree<NodeKey>>,
    node: NodeKey,
    pane: PaneId,
) {
    // 1. Node already has a non-floating tile — focus it.
    if tiles_tree.make_active(|_, tile| match tile {
        Tile::Pane(kind) => {
            kind.node_state().is_some_and(|s| s.node == node) && !kind.is_floating()
        }
        _ => false,
    }) {
        tile_runtime::refresh_node_pane_render_modes(tiles_tree, graph_app);
        return;
    }

    // 2. Durable graphlet peer has a warm tile — route into its tab container.
    if let Some(container_id) = tile_view_ops::warm_peer_tab_container(graph_app, tiles_tree, node)
    {
        let node_pane_tile_id = tiles_tree.tiles.insert_pane(TileKind::Node(node.into()));
        if let Some(Tile::Container(Container::Tabs(tabs))) = tiles_tree.tiles.get_mut(container_id)
        {
            tabs.add_child(node_pane_tile_id);
            tabs.set_active(node_pane_tile_id);
        }
        tile_runtime::refresh_node_pane_render_modes(tiles_tree, graph_app);
        return;
    }

    // 3. Explicit pane target is inside a tab container — grow the graphlet.
    if let Some((container_id, anchor_node)) = tab_container_and_anchor_for_pane(tiles_tree, pane) {
        if graph_app.domain_graph().get_node(node).is_some() {
            graph_app.apply_reducer_intents([GraphIntent::CreateUserGroupedEdge {
                from: node,
                to: anchor_node,
                label: None,
            }]);
        }
        let node_pane_tile_id = tiles_tree.tiles.insert_pane(TileKind::Node(node.into()));
        if let Some(Tile::Container(Container::Tabs(tabs))) = tiles_tree.tiles.get_mut(container_id)
        {
            tabs.add_child(node_pane_tile_id);
            tabs.set_active(node_pane_tile_id);
        }
        tile_runtime::refresh_node_pane_render_modes(tiles_tree, graph_app);
        return;
    }

    // 4. Fallback: standard open.
    if let Some(gt) = graph_tree {
        dual_write::open_or_focus_node(tiles_tree, gt, graph_app, node, None);
    } else {
        tile_view_ops::open_or_focus_node_pane(tiles_tree, graph_app, node);
    }
}

/// Return the tab container `TileId` and a representative anchor `NodeKey`
/// for the given `pane`, or `None` if the pane is not inside a `Tabs` container
/// that holds at least one node pane sibling.
fn tab_container_and_anchor_for_pane(
    tiles_tree: &Tree<TileKind>,
    pane: PaneId,
) -> Option<(TileId, NodeKey)> {
    let pane_tile_id = tiles_tree
        .tiles
        .iter()
        .find_map(|(tile_id, tile)| match tile {
            Tile::Pane(kind) if kind.pane_id() == pane => Some(*tile_id),
            _ => None,
        })?;

    let container_id = tiles_tree.tiles.parent_of(pane_tile_id)?;
    if !matches!(
        tiles_tree.tiles.get(container_id),
        Some(Tile::Container(Container::Tabs(_)))
    ) {
        return None;
    }

    // Find a node pane sibling in the same container.
    let anchor = tiles_tree.tiles.iter().find_map(|(tid, tile)| {
        if *tid == pane_tile_id {
            return None;
        }
        if tiles_tree.tiles.parent_of(*tid) != Some(container_id) {
            return None;
        }
        match tile {
            Tile::Pane(TileKind::Node(state)) => Some(state.node),
            _ => None,
        }
    })?;

    Some((container_id, anchor))
}

pub(super) fn handle_set_pane_view_intent(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    pane: PaneId,
    view: PaneViewState,
) {
    match view {
        PaneViewState::Tool(tool_ref) => {
            open_or_focus_tool_pane_if_available(tiles_tree, tool_ref.kind);
        }
        PaneViewState::Node(state) => {
            let exact_pane_updated = if let Some((_, Tile::Pane(TileKind::Node(node_state)))) =
                tiles_tree.tiles.iter_mut().find(|(_, tile)| {
                    matches!(tile, Tile::Pane(TileKind::Node(node_state)) if node_state.pane_id == pane)
                })
            {
                node_state.node = state.node;
                node_state.viewer_id_override = state.viewer_id_override.clone();
                node_state.viewer_switch_reason = state.viewer_switch_reason;
                true
            } else {
                false
            };

            if exact_pane_updated {
                let _ = tiles_tree.make_active(
                    |_, tile| matches!(tile, Tile::Pane(TileKind::Node(candidate)) if candidate.pane_id == pane),
                );
            } else {
                tile_view_ops::open_or_focus_node_pane(tiles_tree, graph_app, state.node);

                if let Some((_, Tile::Pane(TileKind::Node(node_state)))) =
                    tiles_tree.tiles.iter_mut().find(|(_, tile)| {
                        matches!(tile, Tile::Pane(TileKind::Node(node_state)) if node_state.node == state.node)
                    })
                {
                    node_state.viewer_id_override = state.viewer_id_override.clone();
                    node_state.viewer_switch_reason = state.viewer_switch_reason;
                }
            }
            tile_runtime::refresh_node_pane_render_modes(tiles_tree, graph_app);
        }
        PaneViewState::Graph(graph_ref) => {
            tile_view_ops::open_or_focus_graph_pane(tiles_tree, graph_ref.graph_view_id);
        }
    }
}

pub(super) fn handle_swap_viewer_backend_intent(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    pane: PaneId,
    node: NodeKey,
    viewer_id_override: Option<ViewerId>,
) {
    let switch_reason = if viewer_id_override
        .as_ref()
        .is_some_and(|viewer_id| viewer_id.as_str() == "viewer:wry")
        && graph_app.is_crash_blocked(node)
    {
        ViewerSwitchReason::RecoveryPromptAccepted
    } else if viewer_id_override.is_none() {
        ViewerSwitchReason::PolicyPinned
    } else {
        ViewerSwitchReason::UserRequested
    };

    let exact_pane_updated = if let Some((_, Tile::Pane(TileKind::Node(node_state)))) =
        tiles_tree.tiles.iter_mut().find(|(_, tile)| {
            matches!(tile, Tile::Pane(TileKind::Node(node_state)) if node_state.pane_id == pane && node_state.node == node)
        })
    {
        node_state.viewer_id_override = viewer_id_override.clone();
        node_state.viewer_switch_reason = switch_reason;
        true
    } else {
        false
    };

    if exact_pane_updated {
        let _ = tiles_tree.make_active(
            |_, tile| matches!(tile, Tile::Pane(TileKind::Node(candidate)) if candidate.pane_id == pane),
        );
    } else {
        tile_view_ops::open_or_focus_node_pane(tiles_tree, graph_app, node);

        if let Some((_, Tile::Pane(TileKind::Node(node_state)))) =
            tiles_tree.tiles.iter_mut().find(|(_, tile)| {
                matches!(tile, Tile::Pane(TileKind::Node(node_state)) if node_state.node == node)
            })
        {
            node_state.viewer_id_override = viewer_id_override;
            node_state.viewer_switch_reason = switch_reason;
        }
    }

    tile_runtime::refresh_node_pane_render_modes(tiles_tree, graph_app);
}

/// Merge all warm tiles belonging to `seed`'s durable graphlet into a single
/// `Container::Tabs`.
///
/// Called after a `UserGrouped` or `FrameMember` edge is created between two
/// nodes that already have warm tiles in different containers.  The reconciler:
///
/// 1. Computes the full durable graphlet for `seed`.
/// 2. Identifies which members have a warm tile in the tree.
/// 3. If all warm tiles are already in the same container, returns early.
/// 4. Otherwise picks the container of `seed`'s tile as the primary target.
/// 5. For each warm member in a different container: closes the old tile (no
///    lifecycle change) then re-opens the node via graphlet routing so it
///    joins the primary container.
pub(super) fn handle_reconcile_graphlet_tiles_intent(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    _graph_tree: Option<&mut graph_tree::GraphTree<NodeKey>>,
    seed: NodeKey,
) {
    use std::collections::HashSet;

    // 1. Full graphlet including seed under the active projection.
    let view_id = crate::shell::desktop::workbench::tile_view_ops::active_graph_view_id(tiles_tree)
        .or(graph_app.workspace.graph_runtime.focused_view);
    let graphlet = graph_app.graphlet_members_for_nodes_in_view(&[seed], view_id);

    // 2. Warm members — those that currently have a tile in the tree.
    let warm: Vec<(NodeKey, PaneId)> = graphlet
        .iter()
        .filter_map(|&node| {
            tiles_tree.tiles.iter().find_map(|(_, tile)| match tile {
                Tile::Pane(kind) if kind.node_state().is_some_and(|s| s.node == node) => {
                    Some((node, kind.pane_id()))
                }
                _ => None,
            })
        })
        .collect();

    if warm.len() <= 1 {
        return; // nothing to merge
    }

    // 3. Find each tile's parent container.
    let containers: HashSet<_> = warm
        .iter()
        .map(|(node, _)| {
            let tile_id = tiles_tree.tiles.iter().find_map(|(tid, tile)| match tile {
                Tile::Pane(kind) if kind.node_state().is_some_and(|s| s.node == *node) => {
                    Some(*tid)
                }
                _ => None,
            });
            tile_id.and_then(|tid| tiles_tree.tiles.parent_of(tid))
        })
        .collect();

    if containers.len() == 1 {
        return; // all warm tiles already in the same container
    }

    // 4. Primary container: prefer the one holding `seed`; fall back to first warm member.
    let seed_tile_id = tiles_tree.tiles.iter().find_map(|(tid, tile)| match tile {
        Tile::Pane(kind) if kind.node_state().is_some_and(|s| s.node == seed) => Some(*tid),
        _ => None,
    });
    let primary_container = seed_tile_id
        .and_then(|tid| tiles_tree.tiles.parent_of(tid))
        .or_else(|| {
            let first_tile_id = tiles_tree.tiles.iter().find_map(|(tid, tile)| match tile {
                Tile::Pane(kind)
                    if warm
                        .iter()
                        .any(|(n, _)| kind.node_state().is_some_and(|s| s.node == *n)) =>
                {
                    Some(*tid)
                }
                _ => None,
            });
            first_tile_id.and_then(|tid| tiles_tree.tiles.parent_of(tid))
        });

    let Some(primary_container_id) = primary_container else {
        return; // no container context to merge into
    };

    // 5. Nodes whose tiles are NOT in the primary container.
    let nodes_to_merge: Vec<NodeKey> = warm
        .iter()
        .filter(|(node, _)| {
            let tile_id = tiles_tree.tiles.iter().find_map(|(tid, tile)| match tile {
                Tile::Pane(kind) if kind.node_state().is_some_and(|s| s.node == *node) => {
                    Some(*tid)
                }
                _ => None,
            });
            tile_id
                .and_then(|tid| tiles_tree.tiles.parent_of(tid))
                .map(|c| c != primary_container_id)
                .unwrap_or(false)
        })
        .map(|(node, _)| *node)
        .collect();

    for node in nodes_to_merge {
        // Close tile without demoting lifecycle — close_pane is a pure tile-tree op.
        // Extract PaneId before mutably borrowing tiles_tree.
        let pane_id = tiles_tree.tiles.iter().find_map(|(_, tile)| match tile {
            Tile::Pane(kind) if kind.node_state().is_some_and(|s| s.node == node) => {
                Some(kind.pane_id())
            }
            _ => None,
        });
        if let Some(pid) = pane_id {
            tile_view_ops::close_pane(tiles_tree, pid);
        }
        // Re-open: graphlet routing (step 2 of handle_open_node_in_pane_intent) will
        // find `seed` (or any surviving peer) as a warm graphlet peer and route into the
        // primary container.
        tile_view_ops::open_node_with_graphlet_routing(tiles_tree, graph_app, node);
    }
}

pub(super) fn handle_split_pane_intent(
    tiles_tree: &mut Tree<TileKind>,
    source_pane: PaneId,
    direction: SplitDirection,
) {
    let new_view_id = crate::app::GraphViewId::new();
    if !tile_view_ops::split_pane_with_new_graph_view(
        tiles_tree,
        source_pane,
        direction,
        new_view_id,
    ) {
        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_UX_NAVIGATION_VIOLATION,
            byte_len: 1,
        });
    }
}

pub(super) fn handle_restore_pane_to_semantic_tab_group_intent(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    _graph_tree: Option<&mut graph_tree::GraphTree<NodeKey>>,
    pane: PaneId,
    group_id: uuid::Uuid,
) {
    if !tile_view_ops::restore_pane_to_semantic_tab_group(tiles_tree, graph_app, pane, group_id) {
        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_UX_NAVIGATION_VIOLATION,
            byte_len: 1,
        });
    }
}

pub(super) fn handle_collapse_semantic_tab_group_to_pane_rest_intent(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    _graph_tree: Option<&mut graph_tree::GraphTree<NodeKey>>,
    group_id: uuid::Uuid,
) {
    if !tile_view_ops::collapse_semantic_tab_group_to_pane_rest(tiles_tree, graph_app, group_id) {
        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_UX_NAVIGATION_VIOLATION,
            byte_len: 1,
        });
    }
}
