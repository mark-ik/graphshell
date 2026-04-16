use super::*;
use crate::shell::desktop::runtime::diagnostics::{
    DiagnosticEvent, emit_event, emit_message_received_with_payload,
    emit_message_sent_with_payload, structured_payload_field,
};

fn return_target_kind(target: Option<ToolSurfaceReturnTarget>) -> &'static str {
    match target {
        Some(ToolSurfaceReturnTarget::Graph(_)) => "graph",
        Some(ToolSurfaceReturnTarget::Node(_)) => "node",
        Some(ToolSurfaceReturnTarget::Tool(_)) => "tool",
        None => "active_surface",
    }
}

fn emit_focus_restore_route_resolved(
    target: Option<ToolSurfaceReturnTarget>,
    route_detail: &'static str,
) {
    emit_message_received_with_payload(
        CHANNEL_UI_COMMAND_SURFACE_ROUTE_RESOLVED,
        1,
        vec![
            structured_payload_field("source_surface", "workbench_focus_restore"),
            structured_payload_field("command_id", "surface_return"),
            structured_payload_field("target_kind", return_target_kind(target)),
            structured_payload_field("route_detail", route_detail),
        ],
    );
}

fn emit_focus_restore_route_fallback(
    target: Option<ToolSurfaceReturnTarget>,
    route_detail: &'static str,
) {
    emit_message_sent_with_payload(
        CHANNEL_UI_COMMAND_SURFACE_ROUTE_FALLBACK,
        1,
        vec![
            structured_payload_field("source_surface", "workbench_focus_restore"),
            structured_payload_field("command_id", "surface_return"),
            structured_payload_field("target_kind", return_target_kind(target)),
            structured_payload_field("route_detail", route_detail),
        ],
    );
}

pub(super) fn handle_cycle_focus_region_intent(
    graph_app: &GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    _graph_tree: Option<&mut graph_tree::GraphTree<NodeKey>>,
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
    let mut resolved = false;
    let mut used_fallback = false;
    let mut fallback_detail = None;

    if let Some(target) = target.clone() {
        if focus_tool_surface_return_target(tiles_tree, target.clone()) {
            resolved = true;
            emit_focus_restore_route_resolved(Some(target), "explicit_target_restored");
        } else {
            let fallback_target_available = active_tool_surface_return_target(tiles_tree).is_some();
            if allow_ensure_active_tile {
                let ensured_active_tile = tile_view_ops::ensure_active_tile(tiles_tree);
                resolved = ensured_active_tile || fallback_target_available;
                used_fallback = resolved;
                fallback_detail = if ensured_active_tile {
                    Some("ensure_active_tile")
                } else if fallback_target_available {
                    Some("preserve_active_surface")
                } else {
                    None
                };
            } else {
                resolved = fallback_target_available;
                used_fallback = fallback_target_available;
                fallback_detail = fallback_target_available.then_some("preserve_active_surface");
            }
        }
    } else if allow_ensure_active_tile {
        resolved = tile_view_ops::ensure_active_tile(tiles_tree)
            || active_tool_surface_return_target(tiles_tree).is_some();
        used_fallback = resolved;
        fallback_detail = resolved.then_some("ensure_active_tile");
    }

    if used_fallback {
        emit_focus_restore_route_fallback(target, fallback_detail.unwrap_or("fallback_resolved"));
    }

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
