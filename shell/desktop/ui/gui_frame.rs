/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::Rc;
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};

use egui_tiles::{Container, Tile, TileId, Tiles, Tree};
use euclid::Length;
use log::{debug, warn};
use servo::{DeviceIndependentPixel, OffscreenRenderingContext, WebViewId, WindowRenderingContext};
use winit::window::Window;

use super::dialog_panels::{self, DialogPanelsArgs};
use super::nav_targeting;
use crate::app::{
    GraphBrowserApp, GraphIntent, GraphViewId, LifecycleCause, PendingConnectedOpenScope,
    PendingNodeOpenRequest, PendingTileOpenMode, UnsavedFramePromptAction,
    UnsavedFramePromptRequest,
};
use crate::graph::NodeKey;
use crate::input;
use crate::render;
use crate::shell::desktop::host::headed_window::HeadedWindow;
use crate::shell::desktop::host::running_app_state::RunningAppState;
use crate::shell::desktop::host::window::EmbedderWindow;
use crate::shell::desktop::lifecycle::lifecycle_intents;
use crate::shell::desktop::lifecycle::lifecycle_reconcile::{self, RuntimeReconcileArgs};
use crate::shell::desktop::lifecycle::semantic_event_pipeline;
use crate::shell::desktop::lifecycle::webview_backpressure::WebviewCreationBackpressureState;
use crate::shell::desktop::lifecycle::webview_controller;
use crate::shell::desktop::runtime::diagnostics;
use crate::shell::desktop::ui::persistence_ops;
use crate::shell::desktop::ui::thumbnail_pipeline;
use crate::shell::desktop::ui::thumbnail_pipeline::ThumbnailCaptureResult;
use crate::shell::desktop::ui::toolbar::toolbar_ui::{
    self, OmnibarSearchSession, ToolbarUiInput, ToolbarUiOutput,
};
use crate::shell::desktop::workbench::pane_model::{NodePaneState, ToolPaneState};
use crate::shell::desktop::workbench::tile_compositor;
use crate::shell::desktop::workbench::tile_invariants;
use crate::shell::desktop::workbench::tile_kind::TileKind;
use crate::shell::desktop::workbench::tile_render_pass::{self, TileRenderPassArgs};
use crate::shell::desktop::workbench::tile_runtime;
use crate::shell::desktop::workbench::tile_view_ops;

fn tile_open_mode_from_pending(
    mode: crate::app::PendingTileOpenMode,
) -> tile_view_ops::TileOpenMode {
    match mode {
        crate::app::PendingTileOpenMode::Tab => tile_view_ops::TileOpenMode::Tab,
        crate::app::PendingTileOpenMode::SplitHorizontal => {
            tile_view_ops::TileOpenMode::SplitHorizontal
        }
    }
}

const MAX_CONNECTED_SPLIT_PANES: usize = 4;
const MAX_CONNECTED_OPEN_NODES: usize = 12;

fn find_node_pane_tile_id(tree: &Tree<TileKind>, node_key: NodeKey) -> Option<TileId> {
    tree.tiles.iter().find_map(|(tile_id, tile)| match tile {
        Tile::Pane(TileKind::Node(state)) if state.node == node_key => Some(*tile_id),
        _ => None,
    })
}

fn ensure_node_pane_tile_id(tree: &mut Tree<TileKind>, node_key: NodeKey) -> TileId {
    if let Some(tile_id) = find_node_pane_tile_id(tree, node_key) {
        if let Some(parent_id) = tree.tiles.parent_of(tile_id)
            && matches!(
                tree.tiles.get(parent_id),
                Some(Tile::Container(Container::Tabs(_)))
            )
        {
            return parent_id;
        }
        return tree.tiles.insert_tab_tile(vec![tile_id]);
    }
    let pane_id = tree.tiles.insert_pane(TileKind::Node(node_key.into()));
    tree.tiles.insert_tab_tile(vec![pane_id])
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
                graph_app.capture_undo_checkpoint(Some(current_layout_json));
            }
            if restored_tree.root().is_some() {
                debug!(
                    "frame restore: restored '{}' with {} resolved nodes",
                    name,
                    restored_nodes.len()
                );
                if let Some(request) = routed_open_request.take()
                    && graph_app.workspace.graph.get_node(request.key).is_some()
                {
                    debug!(
                        "gui_frame: opening routed node {:?} in restored frame",
                        request.key
                    );
                    tile_view_ops::open_or_focus_node_pane_with_mode(
                        &mut restored_tree,
                        request.key,
                        pending_tile_mode_to_tile_mode(request.mode),
                    );
                    graph_app.apply_intents([lifecycle_intents::promote_node_to_active(
                        request.key,
                        LifecycleCause::Restore,
                    )]);
                }
                graph_app.note_frame_activated(name, restored_nodes);
                if let Err(e) =
                    persistence_ops::mark_named_frame_bundle_activated(graph_app, name)
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
        .filter(|key| graph_app.workspace.graph.get_node(*key).is_some())
        .collect();
    if live_nodes.is_empty() {
        warn!("Cannot add empty/missing node set to frame snapshot '{name}'");
        return;
    }

    let mut workspace_tree = match persistence_ops::load_named_frame_bundle(graph_app, name) {
        Ok(bundle) => {
            match persistence_ops::restore_runtime_tree_from_frame_bundle(graph_app, &bundle) {
                Ok((tree, _)) => tree,
                Err(e) => {
                    warn!("Failed to restore named frame snapshot '{name}' for add-tab operation: {e}");
                    frame_tree_with_single_node(live_nodes[0])
                }
            }
        }
        Err(_) => frame_tree_with_single_node(live_nodes[0]),
    };
    if workspace_tree.root().is_none() {
        workspace_tree = frame_tree_with_single_node(live_nodes[0]);
    }
    for node_key in live_nodes {
        tile_view_ops::open_or_focus_node_pane_with_mode(
            &mut workspace_tree,
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

fn connected_frame_import_nodes(
    graph_app: &GraphBrowserApp,
    seeds: &[NodeKey],
) -> Vec<NodeKey> {
    let mut out = HashSet::new();
    for seed in seeds {
        if graph_app.workspace.graph.get_node(*seed).is_none() {
            continue;
        }
        out.insert(*seed);
        out.extend(graph_app.workspace.graph.out_neighbors(*seed));
        out.extend(graph_app.workspace.graph.in_neighbors(*seed));
    }
    let mut nodes: Vec<NodeKey> = out
        .into_iter()
        .filter(|key| graph_app.workspace.graph.get_node(*key).is_some())
        .collect();
    nodes.sort_by_key(|key| key.index());
    nodes
}

fn apply_connected_split_layout(tree: &mut Tree<TileKind>, nodes: &[NodeKey]) {
    if nodes.is_empty() {
        return;
    }
    let split_count = nodes.len().min(MAX_CONNECTED_SPLIT_PANES);
    let split_tile_ids: Vec<TileId> = nodes
        .iter()
        .take(split_count)
        .copied()
        .map(|key| ensure_node_pane_tile_id(tree, key))
        .collect();
    let overflow_tile_ids: Vec<TileId> = nodes
        .iter()
        .skip(split_count)
        .copied()
        .map(|key| ensure_node_pane_tile_id(tree, key))
        .collect();

    let row1 = match split_tile_ids.as_slice() {
        [a] => *a,
        [a, b, ..] => tree.tiles.insert_horizontal_tile(vec![*a, *b]),
        [] => return,
    };

    let grid_root = if split_tile_ids.len() > 2 {
        let row2 = match split_tile_ids.as_slice() {
            [_, _, c] => *c,
            [_, _, c, d, ..] => tree.tiles.insert_horizontal_tile(vec![*c, *d]),
            _ => return,
        };
        tree.tiles.insert_vertical_tile(vec![row1, row2])
    } else {
        row1
    };

    tree.root = if overflow_tile_ids.is_empty() {
        Some(grid_root)
    } else {
        let overflow_tabs = tree.tiles.insert_tab_tile(overflow_tile_ids);
        Some(
            tree.tiles
                .insert_vertical_tile(vec![grid_root, overflow_tabs]),
        )
    };
}

fn undirected_neighbors_sorted(graph_app: &GraphBrowserApp, node_key: NodeKey) -> Vec<NodeKey> {
    let mut neighbors: Vec<NodeKey> = graph_app
        .workspace
        .graph
        .out_neighbors(node_key)
        .chain(graph_app.workspace.graph.in_neighbors(node_key))
        .filter(|key| *key != node_key && graph_app.workspace.graph.get_node(*key).is_some())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    neighbors.sort_by_key(|key| key.index());
    neighbors
}

fn connected_candidates_with_depth(
    graph_app: &GraphBrowserApp,
    source: NodeKey,
    scope: PendingConnectedOpenScope,
) -> Vec<(NodeKey, u8)> {
    match scope {
        PendingConnectedOpenScope::Neighbors => undirected_neighbors_sorted(graph_app, source)
            .into_iter()
            .map(|key| (key, 1))
            .collect(),
        PendingConnectedOpenScope::Connected => {
            let mut out = Vec::new();
            let mut visited = HashSet::from([source]);
            let mut queue = VecDeque::from([(source, 0_u8)]);

            while let Some((current, depth)) = queue.pop_front() {
                if depth >= 2 {
                    continue;
                }
                for neighbor in undirected_neighbors_sorted(graph_app, current) {
                    if !visited.insert(neighbor) {
                        continue;
                    }
                    let next_depth = depth + 1;
                    out.push((neighbor, next_depth));
                    if next_depth < 2 {
                        queue.push_back((neighbor, next_depth));
                    }
                }
            }

            out
        }
    }
}

fn connected_targets_for_open(
    graph_app: &GraphBrowserApp,
    source: NodeKey,
    scope: PendingConnectedOpenScope,
) -> Vec<NodeKey> {
    let mut candidates = connected_candidates_with_depth(graph_app, source, scope);
    let cap = MAX_CONNECTED_OPEN_NODES.saturating_sub(1);

    if candidates.len() > cap {
        candidates.sort_by(|(a, depth_a), (b, depth_b)| {
            graph_app
                .frame_recency_seq_for_node(*b)
                .cmp(&graph_app.frame_recency_seq_for_node(*a))
                .then_with(|| depth_a.cmp(depth_b))
                .then_with(|| a.index().cmp(&b.index()))
        });
        candidates.truncate(cap);
    }

    candidates.sort_by(|(a, depth_a), (b, depth_b)| {
        depth_a
            .cmp(depth_b)
            .then_with(|| {
                graph_app
                    .frame_recency_seq_for_node(*b)
                    .cmp(&graph_app.frame_recency_seq_for_node(*a))
            })
            .then_with(|| a.index().cmp(&b.index()))
    });
    candidates.into_iter().map(|(key, _)| key).collect()
}

pub(crate) struct PreFrameIngestArgs<'a> {
    pub(crate) ctx: &'a egui::Context,
    pub(crate) graph_app: &'a GraphBrowserApp,
    pub(crate) app_state: &'a RunningAppState,
    pub(crate) window: &'a EmbedderWindow,
    pub(crate) favicon_textures:
        &'a mut HashMap<WebViewId, (egui::TextureHandle, egui::load::SizedTexture)>,
    pub(crate) thumbnail_capture_tx: &'a Sender<ThumbnailCaptureResult>,
    pub(crate) thumbnail_capture_rx: &'a Receiver<ThumbnailCaptureResult>,
    pub(crate) thumbnail_capture_in_flight: &'a mut HashSet<WebViewId>,
}

pub(crate) struct PreFrameIngestOutput {
    pub(crate) pending_open_child_webviews: Vec<WebViewId>,
    pub(crate) responsive_webviews: HashSet<WebViewId>,
}

pub(crate) fn ingest_pre_frame(
    args: PreFrameIngestArgs<'_>,
    frame_intents: &mut Vec<GraphIntent>,
) -> PreFrameIngestOutput {
    let PreFrameIngestArgs {
        ctx,
        graph_app,
        app_state,
        window,
        favicon_textures,
        thumbnail_capture_tx,
        thumbnail_capture_rx,
        thumbnail_capture_in_flight,
    } = args;

    frame_intents.extend(thumbnail_pipeline::load_pending_thumbnail_results(
        graph_app,
        window,
        thumbnail_capture_rx,
        thumbnail_capture_in_flight,
    ));
    let (semantic_intents, pending_open_child_webviews, responsive_webviews) =
        semantic_event_pipeline::graph_intents_and_responsive_from_events(
            app_state.take_pending_graph_events(),
        );
    frame_intents.extend(semantic_intents);
    frame_intents.extend(thumbnail_pipeline::load_pending_favicons(
        ctx,
        window,
        graph_app,
        favicon_textures,
    ));
    thumbnail_pipeline::request_pending_thumbnail_captures(
        graph_app,
        window,
        thumbnail_capture_tx,
        thumbnail_capture_in_flight,
    );

    PreFrameIngestOutput {
        pending_open_child_webviews,
        responsive_webviews,
    }
}

pub(crate) fn apply_intents_if_any(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &Tree<TileKind>,
    intents: &mut Vec<GraphIntent>,
) {
    if intents.is_empty() {
        return;
    }

    let mut undo_count = 0usize;
    let mut redo_count = 0usize;
    let mut apply_list = Vec::new();
    for intent in std::mem::take(intents) {
        match intent {
            GraphIntent::Undo => undo_count += 1,
            GraphIntent::Redo => redo_count += 1,
            other => apply_list.push(other),
        }
    }

    let layout_json = serde_json::to_string(tiles_tree).ok();
    if !apply_list.is_empty() {
        #[cfg(feature = "diagnostics")]
        let apply_count = apply_list.len();
        if apply_list.iter().any(is_user_undoable_intent) {
            graph_app.capture_undo_checkpoint(layout_json.clone());
        }
        #[cfg(feature = "diagnostics")]
        let apply_started = Instant::now();
        #[cfg(feature = "diagnostics")]
        diagnostics::emit_event(diagnostics::DiagnosticEvent::MessageSent {
            channel_id: "graph_intents.apply",
            byte_len: apply_count,
        });
        graph_app.apply_intents(apply_list);
        #[cfg(feature = "diagnostics")]
        {
            let elapsed = apply_started.elapsed().as_micros() as u64;
            diagnostics::emit_event(diagnostics::DiagnosticEvent::MessageReceived {
                channel_id: "graph_intents.apply",
                latency_us: elapsed,
            });
            diagnostics::emit_span_duration("gui_frame::apply_intents_if_any", elapsed);
        }
    }

    for _ in 0..undo_count {
        let _ = graph_app.perform_undo(layout_json.clone());
    }
    for _ in 0..redo_count {
        let _ = graph_app.perform_redo(layout_json.clone());
    }

    #[cfg(debug_assertions)]
    debug_assert!(
        intents.is_empty(),
        "intent buffer must be drained by apply_intents_if_any"
    );
}

fn is_user_undoable_intent(intent: &GraphIntent) -> bool {
    matches!(
        intent,
        GraphIntent::CreateNodeNearCenter
            | GraphIntent::CreateNodeNearCenterAndOpen { .. }
            | GraphIntent::CreateNodeAtUrl { .. }
            | GraphIntent::CreateNodeAtUrlAndOpen { .. }
            | GraphIntent::RemoveSelectedNodes
            | GraphIntent::ClearGraph
            | GraphIntent::SetNodePosition { .. }
            | GraphIntent::SetNodeUrl { .. }
            | GraphIntent::CreateUserGroupedEdge { .. }
            | GraphIntent::RemoveEdge { .. }
            | GraphIntent::ExecuteEdgeCommand { .. }
            | GraphIntent::SetNodePinned { .. }
            | GraphIntent::TogglePrimaryNodePin
            | GraphIntent::PromoteNodeToActive { .. }
            | GraphIntent::DemoteNodeToWarm { .. }
            | GraphIntent::DemoteNodeToCold { .. }
    )
}

pub(crate) fn open_pending_child_webviews_for_tiles<F>(
    graph_app: &GraphBrowserApp,
    pending_open_child_webviews: Vec<WebViewId>,
    mut open_for_node: F,
) where
    F: FnMut(NodeKey),
{
    for child_webview_id in pending_open_child_webviews {
        if let Some(node_key) = graph_app.get_node_for_webview(child_webview_id) {
            open_for_node(node_key);
        }
    }
}

pub(crate) struct KeyboardPhaseArgs<'a> {
    pub(crate) ctx: &'a egui::Context,
    pub(crate) graph_app: &'a mut GraphBrowserApp,
    pub(crate) window: &'a EmbedderWindow,
    pub(crate) tiles_tree: &'a mut Tree<TileKind>,
    pub(crate) tile_rendering_contexts: &'a mut HashMap<NodeKey, Rc<OffscreenRenderingContext>>,
    pub(crate) tile_favicon_textures: &'a mut HashMap<NodeKey, (u64, egui::TextureHandle)>,
    pub(crate) favicon_textures:
        &'a mut HashMap<WebViewId, (egui::TextureHandle, egui::load::SizedTexture)>,
    pub(crate) app_state: &'a Option<Rc<RunningAppState>>,
    pub(crate) rendering_context: &'a Rc<OffscreenRenderingContext>,
    pub(crate) window_rendering_context: &'a Rc<WindowRenderingContext>,
    pub(crate) responsive_webviews: &'a HashSet<WebViewId>,
    pub(crate) webview_creation_backpressure:
        &'a mut HashMap<NodeKey, WebviewCreationBackpressureState>,
    pub(crate) suppress_toggle_view: bool,
}

pub(crate) fn handle_keyboard_phase<F1, F2>(
    args: KeyboardPhaseArgs<'_>,
    frame_intents: &mut Vec<GraphIntent>,
    mut toggle_tile_view: F1,
    mut reset_runtime_webview_state: F2,
) where
    F1: FnMut(
        &mut Tree<TileKind>,
        &mut GraphBrowserApp,
        &EmbedderWindow,
        &Option<Rc<RunningAppState>>,
        &Rc<OffscreenRenderingContext>,
        &Rc<WindowRenderingContext>,
        &mut HashMap<NodeKey, Rc<OffscreenRenderingContext>>,
        &HashSet<WebViewId>,
        &mut HashMap<NodeKey, WebviewCreationBackpressureState>,
        &mut Vec<GraphIntent>,
    ),
    F2: FnMut(
        &mut Tree<TileKind>,
        &mut HashMap<NodeKey, Rc<OffscreenRenderingContext>>,
        &mut HashMap<NodeKey, (u64, egui::TextureHandle)>,
        &mut HashMap<WebViewId, (egui::TextureHandle, egui::load::SizedTexture)>,
    ),
{
    let KeyboardPhaseArgs {
        ctx,
        graph_app,
        window,
        tiles_tree,
        tile_rendering_contexts,
        tile_favicon_textures,
        favicon_textures,
        app_state,
        rendering_context,
        window_rendering_context,
        responsive_webviews,
        webview_creation_backpressure,
        suppress_toggle_view,
    } = args;

    let mut keyboard_actions = input::collect_actions(ctx, graph_app);
    if suppress_toggle_view {
        keyboard_actions.toggle_view = false;
    }
    if keyboard_actions.toggle_view {
        toggle_tile_view(
            tiles_tree,
            graph_app,
            window,
            app_state,
            rendering_context,
            window_rendering_context,
            tile_rendering_contexts,
            responsive_webviews,
            webview_creation_backpressure,
            frame_intents,
        );
        keyboard_actions.toggle_view = false;
    }
    if keyboard_actions.delete_selected {
        let nodes_to_close: Vec<_> = graph_app.workspace.selected_nodes.iter().copied().collect();
        frame_intents.extend(webview_controller::close_webviews_for_nodes(
            graph_app,
            &nodes_to_close,
            window,
        ));
    }
    if keyboard_actions.clear_graph {
        frame_intents.extend(webview_controller::close_all_webviews(graph_app, window));
        reset_runtime_webview_state(
            tiles_tree,
            tile_rendering_contexts,
            tile_favicon_textures,
            favicon_textures,
        );
    }
    frame_intents.extend(input::intents_from_actions(&keyboard_actions));
}

pub(crate) fn active_node_pane_node(tiles_tree: &Tree<TileKind>) -> Option<NodeKey> {
    tiles_tree
        .active_tiles()
        .into_iter()
        .find_map(|tile_id| match tiles_tree.tiles.get(tile_id) {
            Some(egui_tiles::Tile::Pane(TileKind::Node(state))) => Some(state.node),
            _ => None,
        })
}

pub(crate) struct ToolbarDialogPhaseArgs<'a> {
    pub(crate) ctx: &'a egui::Context,
    pub(crate) winit_window: &'a Window,
    pub(crate) state: &'a RunningAppState,
    pub(crate) graph_app: &'a mut GraphBrowserApp,
    pub(crate) window: &'a EmbedderWindow,
    pub(crate) tiles_tree: &'a mut Tree<TileKind>,
    pub(crate) focused_node_hint: Option<NodeKey>,
    pub(crate) graph_surface_focused: bool,
    pub(crate) can_go_back: bool,
    pub(crate) can_go_forward: bool,
    pub(crate) location: &'a mut String,
    pub(crate) location_dirty: &'a mut bool,
    pub(crate) location_submitted: &'a mut bool,
    pub(crate) focus_location_field_for_search: bool,
    pub(crate) show_clear_data_confirm: &'a mut bool,
    pub(crate) omnibar_search_session: &'a mut Option<OmnibarSearchSession>,
    pub(crate) toasts: &'a mut egui_notify::Toasts,
    pub(crate) tile_rendering_contexts: &'a mut HashMap<NodeKey, Rc<OffscreenRenderingContext>>,
    pub(crate) tile_favicon_textures: &'a mut HashMap<NodeKey, (u64, egui::TextureHandle)>,
    pub(crate) favicon_textures:
        &'a mut HashMap<WebViewId, (egui::TextureHandle, egui::load::SizedTexture)>,
    #[cfg(feature = "diagnostics")]
    pub(crate) diagnostics_state:
        &'a mut crate::shell::desktop::runtime::diagnostics::DiagnosticsState,
}

pub(crate) struct ToolbarDialogPhaseOutput {
    pub(crate) is_graph_view: bool,
    pub(crate) toolbar_output: ToolbarUiOutput,
}

pub(crate) fn handle_toolbar_dialog_phase(
    args: ToolbarDialogPhaseArgs<'_>,
    frame_intents: &mut Vec<GraphIntent>,
) -> ToolbarDialogPhaseOutput {
    let ToolbarDialogPhaseArgs {
        ctx,
        winit_window,
        state,
        graph_app,
        window,
        tiles_tree,
        focused_node_hint,
        graph_surface_focused,
        can_go_back,
        can_go_forward,
        location,
        location_dirty,
        location_submitted,
        focus_location_field_for_search,
        show_clear_data_confirm,
        omnibar_search_session,
        toasts,
        tile_rendering_contexts,
        tile_favicon_textures,
        favicon_textures,
        #[cfg(feature = "diagnostics")]
        diagnostics_state,
    } = args;

    let active_webview_node = active_node_pane_node(tiles_tree);
    let focused_toolbar_node_key = if graph_surface_focused {
        None
    } else {
        tile_compositor::focused_node_key_for_node_panes(tiles_tree, graph_app, focused_node_hint)
    };
    let focused_toolbar_node = nav_targeting::focused_toolbar_node(
        active_webview_node,
        focused_toolbar_node_key,
        graph_app.get_single_selected_node(),
    );
    let has_node_panes = tile_runtime::has_any_node_panes(tiles_tree);
    let is_graph_view = !has_node_panes;
    if !is_graph_view {
        graph_app.workspace.hovered_graph_node = None;
    }

    let toolbar_output = toolbar_ui::render_toolbar_ui(ToolbarUiInput {
        ctx,
        winit_window,
        state,
        graph_app,
        window,
        tiles_tree,
        focused_toolbar_node,
        has_node_panes,
        can_go_back,
        can_go_forward,
        location,
        location_dirty,
        location_submitted,
        focus_location_field_for_search,
        show_clear_data_confirm,
        omnibar_search_session,
        frame_intents,
        #[cfg(feature = "diagnostics")]
        diagnostics_state,
    });

    dialog_panels::render_dialog_panels(DialogPanelsArgs {
        ctx,
        graph_app,
        window,
        tiles_tree,
        tile_rendering_contexts,
        tile_favicon_textures,
        favicon_textures,
        frame_intents,
        location_dirty,
        location_submitted,
        show_clear_data_confirm,
        toasts,
    });

    ToolbarDialogPhaseOutput {
        is_graph_view,
        toolbar_output,
    }
}

pub(crate) struct LifecycleReconcilePhaseArgs<'a> {
    pub(crate) graph_app: &'a mut GraphBrowserApp,
    pub(crate) tiles_tree: &'a mut Tree<TileKind>,
    pub(crate) window: &'a EmbedderWindow,
    pub(crate) app_state: &'a Option<Rc<RunningAppState>>,
    pub(crate) rendering_context: &'a Rc<OffscreenRenderingContext>,
    pub(crate) window_rendering_context: &'a Rc<WindowRenderingContext>,
    pub(crate) tile_rendering_contexts: &'a mut HashMap<NodeKey, Rc<OffscreenRenderingContext>>,
    pub(crate) tile_favicon_textures: &'a mut HashMap<NodeKey, (u64, egui::TextureHandle)>,
    pub(crate) favicon_textures:
        &'a mut HashMap<WebViewId, (egui::TextureHandle, egui::load::SizedTexture)>,
    pub(crate) responsive_webviews: &'a HashSet<WebViewId>,
    pub(crate) webview_creation_backpressure:
        &'a mut HashMap<NodeKey, WebviewCreationBackpressureState>,
}

// After lifecycle intents are applied, ensure runtime viewers exist for Active nodes without tiles.
// This handles prewarm nodes (selected but not opened in tiles).
// Visible tile nodes are handled separately in tile_render_pass.
fn ensure_webviews_for_active_prewarm_nodes(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    window: &EmbedderWindow,
    app_state: &Option<Rc<RunningAppState>>,
    rendering_context: &Rc<OffscreenRenderingContext>,
    window_rendering_context: &Rc<WindowRenderingContext>,
    tile_rendering_contexts: &mut HashMap<NodeKey, Rc<OffscreenRenderingContext>>,
    responsive_webviews: &HashSet<WebViewId>,
    webview_creation_backpressure: &mut HashMap<NodeKey, WebviewCreationBackpressureState>,
) {
    use crate::graph::NodeLifecycle;
    use crate::shell::desktop::workbench::tile_compositor;

    // Find nodes that are Active but don't have visible tiles (prewarm candidates).
    let tile_nodes: std::collections::HashSet<NodeKey> =
        tile_compositor::active_node_pane_rects(tiles_tree)
            .into_iter()
            .map(|(node_key, _)| node_key)
            .collect();

    // Local buffer for runtime viewer creation intents.
    let mut prewarm_intents = Vec::new();

    // Check if primary selected node is Active and not in a tile.
    if let Some(selected_key) = graph_app.get_single_selected_node() {
        if !tile_nodes.contains(&selected_key) {
            if let Some(node) = graph_app.workspace.graph.get_node(selected_key) {
                if node.lifecycle == NodeLifecycle::Active {
                    let default_node_pane = NodePaneState::for_node(selected_key);
                    if tile_runtime::node_pane_uses_composited_runtime(
                        &default_node_pane,
                        graph_app,
                    )
                    {
                        crate::shell::desktop::lifecycle::webview_backpressure::ensure_webview_for_node(
                            graph_app,
                            window,
                            app_state,
                            rendering_context,
                            window_rendering_context,
                            tile_rendering_contexts,
                            selected_key,
                            responsive_webviews,
                            webview_creation_backpressure,
                            &mut prewarm_intents,
                        );
                    }
                }
            }
        }
    }

    // Apply prewarm intents immediately (shouldn't include user-undoable intents).
    if !prewarm_intents.is_empty() {
        graph_app.apply_intents(prewarm_intents);
    }
}

pub(crate) fn run_lifecycle_reconcile_and_apply(
    args: LifecycleReconcilePhaseArgs<'_>,
    frame_intents: &mut Vec<GraphIntent>,
) {
    let LifecycleReconcilePhaseArgs {
        graph_app,
        tiles_tree,
        window,
        app_state,
        rendering_context,
        window_rendering_context,
        tile_rendering_contexts,
        tile_favicon_textures,
        favicon_textures,
        responsive_webviews,
        webview_creation_backpressure,
    } = args;

    let reconcile_start_index = frame_intents.len();

    lifecycle_reconcile::reconcile_runtime(RuntimeReconcileArgs {
        graph_app,
        tiles_tree,
        window,
        tile_rendering_contexts,
        tile_favicon_textures,
        favicon_textures,
        responsive_webviews,
        webview_creation_backpressure,
        frame_intents,
    });

    #[cfg(debug_assertions)]
    {
        for intent in &frame_intents[reconcile_start_index..] {
            debug_assert!(
                !matches!(intent, GraphIntent::Undo | GraphIntent::Redo),
                "reconcile must not emit undo/redo intents"
            );
        }
    }

    apply_intents_if_any(graph_app, tiles_tree, frame_intents);

    // After intents are applied, ensure runtime viewers for Active nodes without tiles (prewarm).
    // Visible tile nodes are handled later in tile_render_pass.
    ensure_webviews_for_active_prewarm_nodes(
        graph_app,
        tiles_tree,
        window,
        app_state,
        rendering_context,
        window_rendering_context,
        tile_rendering_contexts,
        responsive_webviews,
        webview_creation_backpressure,
    );

    #[cfg(debug_assertions)]
    debug_assert!(
        frame_intents.is_empty(),
        "frame intents must be empty after reconcile-and-apply phase"
    );
}

pub(crate) struct PostRenderPhaseArgs<'a> {
    pub(crate) ctx: &'a egui::Context,
    pub(crate) graph_app: &'a mut GraphBrowserApp,
    pub(crate) window: &'a EmbedderWindow,
    pub(crate) headed_window: &'a HeadedWindow,
    pub(crate) tiles_tree: &'a mut Tree<TileKind>,
    pub(crate) tile_rendering_contexts: &'a mut HashMap<NodeKey, Rc<OffscreenRenderingContext>>,
    pub(crate) tile_favicon_textures: &'a mut HashMap<NodeKey, (u64, egui::TextureHandle)>,
    pub(crate) favicon_textures:
        &'a mut HashMap<WebViewId, (egui::TextureHandle, egui::load::SizedTexture)>,
    pub(crate) toolbar_height: &'a mut Length<f32, DeviceIndependentPixel>,
    pub(crate) graph_search_matches: &'a [NodeKey],
    pub(crate) graph_search_active_match_index: Option<usize>,
    pub(crate) graph_search_filter_mode: bool,
    pub(crate) search_query_active: bool,
    pub(crate) app_state: &'a Option<Rc<RunningAppState>>,
    pub(crate) rendering_context: &'a Rc<OffscreenRenderingContext>,
    pub(crate) window_rendering_context: &'a Rc<WindowRenderingContext>,
    pub(crate) responsive_webviews: &'a HashSet<WebViewId>,
    pub(crate) webview_creation_backpressure:
        &'a mut HashMap<NodeKey, WebviewCreationBackpressureState>,
    pub(crate) focused_node_hint: &'a mut Option<NodeKey>,
    pub(crate) graph_surface_focused: bool,
    pub(crate) focus_ring_node_key: &'a mut Option<NodeKey>,
    pub(crate) focus_ring_started_at: &'a mut Option<Instant>,
    pub(crate) focus_ring_duration: Duration,
    pub(crate) toasts: &'a mut egui_notify::Toasts,
    pub(crate) control_panel: &'a mut crate::shell::desktop::runtime::control_panel::ControlPanel,
    #[cfg(feature = "diagnostics")]
    pub(crate) diagnostics_state: &'a mut diagnostics::DiagnosticsState,
}

pub(crate) fn run_post_render_phase<FActive>(
    args: PostRenderPhaseArgs<'_>,
    active_graph_search_match: FActive,
) where
    FActive: Fn(&[NodeKey], Option<usize>) -> Option<NodeKey>,
{
    let PostRenderPhaseArgs {
        ctx,
        graph_app,
        window,
        headed_window,
        tiles_tree,
        tile_rendering_contexts,
        tile_favicon_textures,
        favicon_textures,
        toolbar_height,
        graph_search_matches,
        graph_search_active_match_index,
        graph_search_filter_mode,
        search_query_active,
        app_state,
        rendering_context,
        window_rendering_context,
        responsive_webviews,
        webview_creation_backpressure,
        focused_node_hint,
        graph_surface_focused,
        focus_ring_node_key,
        focus_ring_started_at,
        focus_ring_duration,
        toasts,
        control_panel,
        #[cfg(feature = "diagnostics")]
        diagnostics_state,
    } = args;

    #[cfg(debug_assertions)]
    {
        for violation in tile_invariants::collect_tile_invariant_violations(
            tiles_tree,
            graph_app,
            tile_rendering_contexts,
        ) {
            warn!("{violation}");
        }
    }

    let has_node_panes = tile_runtime::has_any_node_panes(tiles_tree);
    let is_graph_view = !has_node_panes;

    *toolbar_height = Length::new(ctx.available_rect().min.y);
    graph_app.check_periodic_snapshot();

    let focused_dialog_webview = if graph_surface_focused {
        None
    } else {
        tile_compositor::focused_node_key_for_node_panes(tiles_tree, graph_app, *focused_node_hint)
            .and_then(|node_key| graph_app.get_webview_for_node(node_key))
    };
    headed_window.for_each_active_dialog(
        window,
        focused_dialog_webview,
        *toolbar_height,
        |dialog| dialog.update(ctx),
    );

    let mut post_render_intents = Vec::new();
    if is_graph_view || has_node_panes {
        let search_matches: HashSet<NodeKey> = graph_search_matches.iter().copied().collect();
        let active_search_match =
            active_graph_search_match(graph_search_matches, graph_search_active_match_index);
        post_render_intents.extend(tile_render_pass::run_tile_render_pass(TileRenderPassArgs {
            ctx,
            graph_app,
            window,
            tiles_tree,
            tile_rendering_contexts,
            tile_favicon_textures,
            graph_search_matches: &search_matches,
            active_search_match,
            graph_search_filter_mode,
            search_query_active,
            app_state,
            rendering_context,
            window_rendering_context,
            responsive_webviews,
            webview_creation_backpressure,
            focused_node_hint,
            graph_surface_focused,
            focus_ring_node_key,
            focus_ring_started_at,
            focus_ring_duration,
            control_panel,
            #[cfg(feature = "diagnostics")]
            diagnostics_state,
        }));
    }
    apply_intents_if_any(graph_app, tiles_tree, &mut post_render_intents);

    render::render_help_panel(ctx, graph_app);
    let focused_pane_node = focused_dialog_webview
        .and_then(|webview_id| graph_app.get_node_for_webview(webview_id))
        .or_else(|| active_node_pane_node(tiles_tree));
    render::render_command_palette_panel(
        ctx,
        graph_app,
        graph_app.workspace.hovered_graph_node,
        focused_pane_node,
    );
    render::render_radial_command_menu(
        ctx,
        graph_app,
        graph_app.workspace.hovered_graph_node,
        focused_pane_node,
    );
    if let Some(target_dir) = graph_app.take_pending_switch_data_dir() {
        match persistence_ops::switch_persistence_store(
            graph_app,
            window,
            tiles_tree,
            tile_rendering_contexts,
            tile_favicon_textures,
            favicon_textures,
            &mut post_render_intents,
            target_dir.clone(),
        ) {
            Ok(()) => toasts.success(format!(
                "Switched graph data directory to {}",
                target_dir.display()
            )),
            Err(e) => toasts.error(format!("Failed to switch data directory: {e}")),
        };
    }
    let open_settings_tool_pane = render::render_choose_frame_picker(ctx, graph_app)
        || render::render_unsaved_frame_prompt(ctx, graph_app);
    if open_settings_tool_pane {
        tile_view_ops::open_or_focus_tool_pane(tiles_tree, ToolPaneState::Settings);
    }

    if let Some((request, action)) = graph_app.take_unsaved_workspace_prompt_resolution() {
        match (request, action) {
            (
                UnsavedFramePromptRequest::FrameSwitch { name, focus_node },
                UnsavedFramePromptAction::ProceedWithoutSaving,
            ) => {
                let open_request = focus_node.map(|key| PendingNodeOpenRequest {
                    key,
                    mode: PendingTileOpenMode::Tab,
                });
                restore_named_frame_snapshot(graph_app, tiles_tree, &name, open_request);
            }
            (
                UnsavedFramePromptRequest::FrameSwitch { .. },
                UnsavedFramePromptAction::Cancel,
            ) => {}
        }
    }

    if graph_app.take_pending_save_frame_snapshot() {
        match serde_json::to_string(tiles_tree) {
            Ok(layout_json) => graph_app.save_tile_layout_json(&layout_json),
            Err(e) => warn!("Failed to serialize tile layout for frame snapshot: {e}"),
        }
    }

    if let Some(name) = graph_app.take_pending_save_frame_snapshot_named() {
        match persistence_ops::save_named_frame_bundle(graph_app, &name, tiles_tree) {
            Ok(()) => {
                let _ =
                    persistence_ops::refresh_frame_membership_cache_from_manifests(graph_app);
            }
            Err(e) => warn!("Failed to serialize tile layout for frame snapshot '{name}': {e}"),
        }
    }

    if graph_app.take_pending_prune_empty_frames() {
        let deleted = persistence_ops::prune_empty_named_workspaces(graph_app);
        warn!("Pruned {deleted} empty named frame snapshots");
    }

    if let Some(keep) = graph_app.take_pending_keep_latest_named_frames() {
        let deleted = persistence_ops::keep_latest_named_workspaces(graph_app, keep);
        warn!("Removed {deleted} named frame snapshots beyond latest {keep}");
    }

    if let Some(name) = graph_app.take_pending_restore_frame_snapshot_named() {
        let open_request = graph_app.take_pending_frame_restore_open_request();
        if graph_app.should_prompt_unsaved_workspace_save() {
            if graph_app.consume_unsaved_workspace_prompt_warning() {
                warn!("Current frame has unsaved graph changes before switching to '{name}'");
            }
            graph_app.request_unsaved_workspace_prompt(
                UnsavedFramePromptRequest::FrameSwitch {
                    name,
                    focus_node: open_request.map(|request| request.key),
                },
            );
        } else {
            restore_named_frame_snapshot(graph_app, tiles_tree, &name, open_request);
        }
    }

    if let Some((node_key, frame_name)) = graph_app.take_pending_add_node_to_frame() {
        add_nodes_to_named_frame_snapshot(graph_app, &frame_name, &[node_key]);
    }

    if let Some((seed_nodes, frame_name)) = graph_app.take_pending_add_connected_to_frame()
    {
        let nodes = connected_frame_import_nodes(graph_app, &seed_nodes);
        add_nodes_to_named_frame_snapshot(graph_app, &frame_name, &nodes);
    }

    if let Some((nodes, frame_name)) = graph_app.take_pending_add_exact_to_frame() {
        add_nodes_to_named_frame_snapshot(graph_app, &frame_name, &nodes);
    }

    if let Some(name) = graph_app.take_pending_save_graph_snapshot_named()
        && let Err(e) = graph_app.save_named_graph_snapshot(&name)
    {
        warn!("Failed to save named graph snapshot '{name}': {e}");
    }

    if let Some(name) = graph_app.take_pending_delete_graph_snapshot_named()
        && let Err(e) = graph_app.delete_named_graph_snapshot(&name)
    {
        warn!("Failed to delete named graph snapshot '{name}': {e}");
    }

    if let Some(name) = graph_app.take_pending_restore_graph_snapshot_named() {
        if let Ok(layout_json) = serde_json::to_string(tiles_tree) {
            graph_app.capture_undo_checkpoint(Some(layout_json));
        }
        let close_intents = webview_controller::close_all_webviews(graph_app, window);
        if !close_intents.is_empty() {
            graph_app.apply_intents(close_intents);
        }
        match graph_app.load_named_graph_snapshot(&name) {
            Ok(()) => {
                tile_rendering_contexts.clear();
                tile_favicon_textures.clear();
                webview_creation_backpressure.clear();
                *focused_node_hint = None;
                let mut tiles = Tiles::default();
                let graph_tile_id = tiles.insert_pane(TileKind::Graph(GraphViewId::default()));
                *tiles_tree = Tree::new("graphshell_tiles", graph_tile_id, tiles);
            }
            Err(e) => warn!("Failed to load named graph snapshot '{name}': {e}"),
        }
    }

    if graph_app.take_pending_restore_graph_snapshot_latest() {
        if let Ok(layout_json) = serde_json::to_string(tiles_tree) {
            graph_app.capture_undo_checkpoint(Some(layout_json));
        }
        let close_intents = webview_controller::close_all_webviews(graph_app, window);
        if !close_intents.is_empty() {
            graph_app.apply_intents(close_intents);
        }
        match graph_app.load_latest_graph_snapshot() {
            Ok(()) => {
                tile_rendering_contexts.clear();
                tile_favicon_textures.clear();
                webview_creation_backpressure.clear();
                *focused_node_hint = None;
                let mut tiles = Tiles::default();
                let graph_tile_id = tiles.insert_pane(TileKind::Graph(GraphViewId::default()));
                *tiles_tree = Tree::new("graphshell_tiles", graph_tile_id, tiles);
            }
            Err(e) => warn!("Failed to load autosaved latest graph snapshot: {e}"),
        }
    }

    if let Some(node_key) = graph_app.take_pending_detach_node_to_split() {
        if let Ok(layout_json) = serde_json::to_string(tiles_tree) {
            graph_app.capture_undo_checkpoint(Some(layout_json));
        }
        tile_view_ops::detach_node_pane_to_split(tiles_tree, node_key);
    }

    if let Some((source, open_mode, scope)) = graph_app.take_pending_open_connected_from()
        && graph_app.workspace.graph.get_node(source).is_some()
    {
        if let Ok(layout_json) = serde_json::to_string(tiles_tree) {
            graph_app.capture_undo_checkpoint(Some(layout_json));
        }
        let connected = connected_targets_for_open(graph_app, source, scope);

        let mut intents = Vec::with_capacity(connected.len() + 2);
        intents.push(GraphIntent::SelectNode {
            key: source,
            multi_select: false,
        });
        intents.push(lifecycle_intents::promote_node_to_active(
            source,
            LifecycleCause::UserSelect,
        ));
        for node in &connected {
            intents.push(lifecycle_intents::promote_node_to_active(
                *node,
                LifecycleCause::ActiveTileVisible,
            ));
        }
        graph_app.apply_intents(intents);

        let mut ordered = Vec::with_capacity(connected.len() + 1);
        ordered.push(source);
        ordered.extend(connected);

        graph_app.mark_current_frame_synthesized();
        let tile_mode = tile_open_mode_from_pending(open_mode);
        match tile_mode {
            tile_view_ops::TileOpenMode::Tab => {
                for node in ordered {
                    tile_view_ops::open_or_focus_node_pane_with_mode(
                        tiles_tree,
                        node,
                        tile_view_ops::TileOpenMode::Tab,
                    );
                }
            }
            tile_view_ops::TileOpenMode::SplitHorizontal => {
                // One-shot connected-open uses a compact 2-up / 2x2 split policy (max 4),
                // with overflow grouped into tabs below the split area.
                apply_connected_split_layout(tiles_tree, &ordered);
            }
        }
    }

    if let Some(layout_json) = graph_app.take_pending_history_frame_layout_json() {
        match serde_json::from_str::<Tree<TileKind>>(&layout_json) {
            Ok(mut restored_tree) => {
                tile_runtime::prune_stale_node_pane_keys_only(&mut restored_tree, graph_app);
                if restored_tree.root().is_some() {
                    *tiles_tree = restored_tree;
                    graph_app.mark_session_frame_layout_json(&layout_json);
                }
            }
            Err(e) => warn!("Failed to deserialize undo/redo frame snapshot: {e}"),
        }
    }

    let prompt_pending = graph_app.unsaved_workspace_prompt_request().is_some();
    // Session autosave should not block on unsaved-workspace prompts. Prompting
    // is reserved for explicit frame-switch actions.
    if !prompt_pending {
        match serde_json::to_string(tiles_tree) {
            Ok(layout_json) => match persistence_ops::serialize_named_frame_bundle(
                graph_app,
                GraphBrowserApp::SESSION_WORKSPACE_LAYOUT_NAME,
                tiles_tree,
            ) {
                Ok(bundle_json) => graph_app
                    .save_session_workspace_layout_blob_if_changed(&bundle_json, &layout_json),
                Err(e) => warn!("Failed to serialize session frame bundle: {e}"),
            },
            Err(e) => warn!("Failed to serialize session frame layout: {e}"),
        }
    }
}
