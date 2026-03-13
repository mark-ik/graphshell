use super::*;

pub(super) fn handle_cycle_focus_region_intent(
    graph_app: &GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    focus_cycle: crate::registries::domain::layout::workbench_surface::FocusCycle,
) -> bool {
    let before =
        crate::shell::desktop::ui::gui::workspace_runtime_focus_state(graph_app, None, None, false);
    let cycled = match focus_cycle {
        crate::registries::domain::layout::workbench_surface::FocusCycle::Panes => {
            cycle_semantic_workbench_region(graph_app, tiles_tree)
        }
        _ => tile_view_ops::cycle_focus_region_with_policy(tiles_tree, focus_cycle),
    };
    let after = crate::shell::desktop::ui::gui::workbench_runtime_focus_state(
        graph_app, tiles_tree, None, None, false,
    );
    cycled && before != after
}

pub(super) fn active_tool_surface_return_target(
    tiles_tree: &Tree<TileKind>,
) -> Option<ToolSurfaceReturnTarget> {
    for tile_id in tiles_tree.active_tiles() {
        match tiles_tree.tiles.get(tile_id) {
            Some(Tile::Pane(TileKind::Graph(view_ref))) => {
                return Some(ToolSurfaceReturnTarget::Graph(view_ref.graph_view_id));
            }
            Some(Tile::Pane(TileKind::Node(state))) => {
                return Some(ToolSurfaceReturnTarget::Node(state.node));
            }
            #[cfg(feature = "diagnostics")]
            Some(Tile::Pane(TileKind::Tool(tool_ref))) => {
                return Some(ToolSurfaceReturnTarget::Tool(tool_ref.kind.clone()));
            }
            _ => {}
        }
    }
    None
}

pub(super) fn focus_tool_surface_return_target(
    tiles_tree: &mut Tree<TileKind>,
    target: ToolSurfaceReturnTarget,
) -> bool {
    match target {
        ToolSurfaceReturnTarget::Graph(view_id) => tiles_tree.make_active(
            |_, tile| {
                matches!(tile, Tile::Pane(TileKind::Graph(existing)) if existing.graph_view_id == view_id)
            },
        ),
        ToolSurfaceReturnTarget::Node(node_key) => tiles_tree.make_active(
            |_, tile| matches!(tile, Tile::Pane(TileKind::Node(state)) if state.node == node_key),
        ),
        ToolSurfaceReturnTarget::Tool(kind) => {
            #[cfg(feature = "diagnostics")]
            {
                tiles_tree.make_active(|_, tile| {
                    matches!(tile, Tile::Pane(TileKind::Tool(existing)) if existing.kind == kind)
                })
            }
            #[cfg(not(feature = "diagnostics"))]
            {
                let _ = kind;
                false
            }
        }
    }
}

pub(super) fn restore_focus_target_or_ensure_active_tile(
    graph_app: &GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    target: Option<ToolSurfaceReturnTarget>,
    allow_ensure_active_tile: bool,
) -> bool {
    let focus_before = crate::shell::desktop::ui::gui::workbench_runtime_focus_state(
        graph_app, tiles_tree, None, None, false,
    );
    let resolved = if let Some(target) = target {
        let restored = focus_tool_surface_return_target(tiles_tree, target);
        if restored {
            true
        } else if allow_ensure_active_tile {
            tile_view_ops::ensure_active_tile(tiles_tree)
        } else {
            false
        }
    } else if allow_ensure_active_tile {
        tile_view_ops::ensure_active_tile(tiles_tree)
    } else {
        false
    };

    if !resolved && active_tool_surface_return_target(tiles_tree).is_none() {
        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_UX_NAVIGATION_VIOLATION,
            byte_len: 1,
        });
    }

    let focus_after = crate::shell::desktop::ui::gui::workbench_runtime_focus_state(
        graph_app, tiles_tree, None, None, false,
    );
    resolved && focus_before != focus_after
}
