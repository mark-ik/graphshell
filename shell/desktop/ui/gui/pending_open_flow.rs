/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Pending-open flows — node/note/clip requests queued on `graph_app`
//! get drained into workbench intents + tile-tree mutations here.
//!
//! Split out of `gui_orchestration.rs` as part of M6 §4.1. Owns:
//!
//! - [`handle_pending_open_node_after_intents`] — primary entry point;
//!   takes a queued open request + selection, synthesizes the intent
//!   chain (`SelectNode`, optional `CreateUserGroupedEdge`,
//!   `promote_node_to_active`), and performs the tile mutation.
//! - [`handle_pending_open_note_after_intents`] — note → linked node +
//!   history manager surface.
//! - [`handle_pending_open_clip_after_intents`] — clip → node + history
//!   manager surface.
//! - Private helpers for undo checkpointing, anchor capture, in-workspace
//!   detection, grouped-edge emission, and pending-mode conversion.

use egui_tiles::Tree;

use crate::app::{GraphBrowserApp, GraphIntent, LifecycleCause, PendingTileOpenMode, UndoBoundaryReason};
use crate::graph::NodeKey;
use crate::shell::desktop::lifecycle::lifecycle_intents;
use crate::shell::desktop::runtime::diagnostics::{DiagnosticEvent, emit_event};
use crate::shell::desktop::runtime::registries::CHANNEL_UX_NAVIGATION_TRANSITION;
use crate::shell::desktop::ui::nav_targeting;
use crate::shell::desktop::workbench::pane_model::ToolPaneState;
use crate::shell::desktop::workbench::tile_kind::TileKind;
use crate::shell::desktop::workbench::tile_view_ops::TileOpenMode;

pub(crate) fn handle_pending_open_node_after_intents(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    open_node_tile_after_intents: &mut Option<TileOpenMode>,
    frame_intents: &mut Vec<GraphIntent>,
) {
    let queued_open_mode = open_node_tile_after_intents.take();
    let pending_open_request = take_pending_open_node_request_selection(graph_app);

    log::debug!(
        "gui: pending open node phase queued_mode={:?} pending_request={:?} selected={:?}",
        queued_open_mode,
        pending_open_request,
        graph_app.get_single_selected_node()
    );

    let open_candidate = pending_open_request
        .map(|(node_key, mode)| (Some(node_key), mode))
        .or_else(|| queued_open_mode.map(|mode| (None, mode)));

    if let Some((request_node_key, open_mode)) = open_candidate
        && let Some(node_key) = request_node_key.or_else(|| graph_app.get_single_selected_node())
    {
        if request_node_key.is_some() {
            frame_intents.push(GraphIntent::SelectNode {
                key: node_key,
                multi_select: false,
            });
        }
        execute_pending_open_node_after_intents(
            graph_app,
            tiles_tree,
            frame_intents,
            node_key,
            open_mode,
        );

        log::debug!(
            "gui: executed pending open node {:?} mode {:?}; active_tiles={}",
            node_key,
            open_mode,
            tiles_tree.active_tiles().len()
        );
    } else if open_candidate.is_some() {
        log::debug!("gui: pending open node skipped because no valid selected node is available");
    }
}

pub(crate) fn handle_pending_open_note_after_intents(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
) {
    let Some(note_id) = graph_app.take_pending_open_note_request() else {
        return;
    };

    let linked_node = graph_app
        .note_record(note_id)
        .and_then(|note| note.linked_node);
    if let Some(node_key) = linked_node
        && graph_app.domain_graph().get_node(node_key).is_some()
    {
        crate::shell::desktop::workbench::tile_view_ops::open_or_focus_node_pane(
            tiles_tree, graph_app, node_key,
        );
    }

    crate::shell::desktop::workbench::tile_view_ops::open_or_focus_tool_pane(
        tiles_tree,
        ToolPaneState::HistoryManager,
    );
    emit_event(DiagnosticEvent::MessageReceived {
        channel_id: CHANNEL_UX_NAVIGATION_TRANSITION,
        latency_us: 0,
    });
}

pub(crate) fn handle_pending_open_clip_after_intents(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
) {
    let Some(clip_id) = graph_app.take_pending_open_clip_request() else {
        return;
    };

    if let Some(node_key) = graph_app.find_clip_node_by_id(&clip_id) {
        crate::shell::desktop::workbench::tile_view_ops::open_or_focus_node_pane(
            tiles_tree, graph_app, node_key,
        );
    }

    crate::shell::desktop::workbench::tile_view_ops::open_or_focus_tool_pane(
        tiles_tree,
        ToolPaneState::HistoryManager,
    );
    emit_event(DiagnosticEvent::MessageReceived {
        channel_id: CHANNEL_UX_NAVIGATION_TRANSITION,
        latency_us: 0,
    });
}

fn open_mode_from_pending(mode: PendingTileOpenMode) -> TileOpenMode {
    match mode {
        PendingTileOpenMode::Tab => TileOpenMode::Tab,
        PendingTileOpenMode::SplitHorizontal => TileOpenMode::SplitHorizontal,
        PendingTileOpenMode::QuarterPane => TileOpenMode::QuarterPane,
        PendingTileOpenMode::HalfPane => TileOpenMode::HalfPane,
    }
}

fn take_pending_open_node_request_selection(
    graph_app: &mut GraphBrowserApp,
) -> Option<(NodeKey, TileOpenMode)> {
    if let Some(open_request) = graph_app.take_pending_open_node_request() {
        log::debug!(
            "gui: handle_pending_open_node_after_intents taking request for {:?}",
            open_request.key
        );
        let open_mode = open_mode_from_pending(open_request.mode);
        return Some((open_request.key, open_mode));
    }

    None
}

fn execute_pending_open_node_after_intents(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    frame_intents: &mut Vec<GraphIntent>,
    node_key: NodeKey,
    open_mode: TileOpenMode,
) {
    capture_open_node_undo_checkpoint(graph_app, tiles_tree);
    let anchor_before_open = anchor_before_tab_open(graph_app, open_mode);
    let node_already_in_workspace = is_node_already_in_workspace(tiles_tree, node_key);
    log::debug!(
        "gui: calling open_or_focus_node_pane_with_mode for {:?} mode {:?}",
        node_key,
        open_mode
    );
    crate::shell::desktop::workbench::tile_view_ops::open_or_focus_node_pane_with_mode(
        tiles_tree, graph_app, node_key, open_mode,
    );
    maybe_push_grouped_edge_after_tab_open(
        frame_intents,
        open_mode,
        node_already_in_workspace,
        anchor_before_open,
        node_key,
    );
    frame_intents.push(
        lifecycle_intents::promote_node_to_active(node_key, LifecycleCause::UserSelect).into(),
    );
}

fn capture_open_node_undo_checkpoint(graph_app: &mut GraphBrowserApp, tiles_tree: &Tree<TileKind>) {
    if let Ok(layout_json) = serde_json::to_string(tiles_tree) {
        graph_app
            .record_workspace_undo_boundary(Some(layout_json), UndoBoundaryReason::OpenNodePane);
    }
}

fn anchor_before_tab_open(graph_app: &GraphBrowserApp, open_mode: TileOpenMode) -> Option<NodeKey> {
    if open_mode == TileOpenMode::Tab {
        nav_targeting::active_node_pane_node(graph_app)
    } else {
        None
    }
}

fn is_node_already_in_workspace(tiles_tree: &Tree<TileKind>, node_key: NodeKey) -> bool {
    tiles_tree.tiles.iter().any(|(_, tile)| {
        matches!(
            tile,
            egui_tiles::Tile::Pane(TileKind::Node(state)) if state.node == node_key
        )
    })
}

fn maybe_push_grouped_edge_after_tab_open(
    frame_intents: &mut Vec<GraphIntent>,
    open_mode: TileOpenMode,
    node_already_in_workspace: bool,
    anchor_before_open: Option<NodeKey>,
    node_key: NodeKey,
) {
    if open_mode == TileOpenMode::Tab
        && !node_already_in_workspace
        && let Some(anchor) = anchor_before_open
        && anchor != node_key
    {
        frame_intents.push(GraphIntent::CreateUserGroupedEdge {
            from: anchor,
            to: node_key,
            label: None,
        });
    }
}
