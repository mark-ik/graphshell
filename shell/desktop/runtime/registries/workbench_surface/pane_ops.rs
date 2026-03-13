use super::*;

pub(super) fn handle_open_tool_pane_intent(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    kind: ToolPaneState,
) {
    let focused_before = active_tool_surface_return_target(tiles_tree);
    if matches!(
        kind,
        ToolPaneState::Settings | ToolPaneState::HistoryManager
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
    pane: PaneId,
    restore_previous_focus: bool,
    focus_handoff: &FocusHandoffPolicy,
) {
    let focus_before = crate::shell::desktop::ui::gui::workbench_runtime_focus_state(
        graph_app, tiles_tree, None, None, false,
    );
    let closed = tile_view_ops::close_pane(tiles_tree, pane);

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
        let ensured = tile_view_ops::ensure_active_tile(tiles_tree);
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

pub(super) fn handle_open_node_in_pane_intent(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    node: NodeKey,
    pane: PaneId,
) {
    let _ = pane;
    tile_view_ops::open_or_focus_node_pane(tiles_tree, graph_app, node);
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
    let exact_pane_updated = if let Some((_, Tile::Pane(TileKind::Node(node_state)))) =
        tiles_tree.tiles.iter_mut().find(|(_, tile)| {
            matches!(tile, Tile::Pane(TileKind::Node(node_state)) if node_state.pane_id == pane && node_state.node == node)
        })
    {
        node_state.viewer_id_override = viewer_id_override.clone();
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
        }
    }

    tile_runtime::refresh_node_pane_render_modes(tiles_tree, graph_app);
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
