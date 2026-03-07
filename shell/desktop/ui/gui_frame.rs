/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::{HashMap, HashSet};
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
use super::undo_boundary::record_workspace_undo_boundary_from_tiles_tree;
use crate::app::{
    GraphBrowserApp, GraphIntent, GraphViewId, LifecycleCause, PendingConnectedOpenScope,
    PendingNodeOpenRequest, PendingTileOpenMode, ReducerDispatchContext, UndoBoundaryReason,
    UnsavedFramePromptAction, UnsavedFramePromptRequest,
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
use crate::shell::desktop::runtime::registries::{
    CHANNEL_SEMANTIC_CREATE_NEW_WEBVIEW_UNMAPPED, CHANNEL_UX_NAVIGATION_TRANSITION,
};
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

#[path = "gui_frame/pending_actions.rs"]
mod pending_actions;
#[path = "gui_frame/connected_open.rs"]
mod connected_open;
#[path = "gui_frame/graph_snapshot.rs"]
mod graph_snapshot;
#[path = "gui_frame/frame_persistence.rs"]
mod frame_persistence;
#[path = "gui_frame/workspace_layout.rs"]
mod workspace_layout;

// Ownership map (Stage 4b gui_frame responsibility split):
// - `gui_frame.rs` remains the frame-phase facade and host for shared frame helpers.
// - `gui_frame/pending_actions.rs` owns post-render pending-action pipeline coordination.
// - Feature/domain helpers (frame snapshot, graph snapshot, workspace-layout handlers)
//   remain in this module and are invoked by the pending-actions coordinator.

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
        .domain_graph()
        .neighbors_undirected(node_key)
        .filter(|key| *key != node_key && graph_app.domain_graph().get_node(*key).is_some())
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
            let depth1 = undirected_neighbors_sorted(graph_app, source);
            for neighbor in depth1 {
                if visited.insert(neighbor) {
                    out.push((neighbor, 1));
                }
            }

            let depth1_nodes: Vec<NodeKey> = out
                .iter()
                .filter_map(|(node, depth)| (*depth == 1).then_some(*node))
                .collect();
            for depth1_node in depth1_nodes {
                for neighbor in undirected_neighbors_sorted(graph_app, depth1_node) {
                    if visited.insert(neighbor) {
                        out.push((neighbor, 2));
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
    let (semantic_events, pending_open_child_webviews, responsive_webviews) =
        semantic_event_pipeline::runtime_events_and_responsive_from_events(
            app_state.take_pending_graph_events(),
        );
    frame_intents.extend(semantic_events.into_iter().map(Into::into));
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
        #[cfg(feature = "diagnostics")]
        let apply_started = Instant::now();
        #[cfg(feature = "diagnostics")]
        diagnostics::emit_event(diagnostics::DiagnosticEvent::MessageSent {
            channel_id: "graph_intents.apply",
            byte_len: apply_count,
        });
        graph_app.apply_reducer_intents_with_context(
            apply_list,
            ReducerDispatchContext {
                workspace_layout_before: layout_json.clone(),
                ..ReducerDispatchContext::default()
            },
        );
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

    if let Some(layout_json) = &layout_json {
        graph_app.mark_session_frame_layout_json(layout_json);
    }
    for _ in 0..undo_count {
        graph_app.apply_reducer_intents([GraphIntent::Undo]);
    }
    for _ in 0..redo_count {
        graph_app.apply_reducer_intents([GraphIntent::Redo]);
    }

    #[cfg(debug_assertions)]
    debug_assert!(
        intents.is_empty(),
        "intent buffer must be drained by apply_intents_if_any"
    );
}

pub(crate) fn open_pending_child_webviews_for_tiles<F>(
    graph_app: &GraphBrowserApp,
    pending_open_child_webviews: Vec<WebViewId>,
    mut open_for_node: F,
) -> Vec<WebViewId>
where
    F: FnMut(NodeKey),
{
    let mut deferred_webviews = Vec::new();
    for child_webview_id in pending_open_child_webviews {
        if let Some(node_key) = graph_app.get_node_for_webview(child_webview_id) {
            open_for_node(node_key);
        } else {
            deferred_webviews.push(child_webview_id);
            warn!(
                "semantic child-webview {:?} had no node mapping; skipping pane-open",
                child_webview_id
            );
            #[cfg(feature = "diagnostics")]
            diagnostics::emit_event(diagnostics::DiagnosticEvent::MessageSent {
                channel_id: CHANNEL_SEMANTIC_CREATE_NEW_WEBVIEW_UNMAPPED,
                byte_len: 1,
            });
        }
    }
    deferred_webviews
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
    let preview_active = history_preview_mode_active(graph_app);
    if preview_active {
        keyboard_actions.toggle_view = false;
        keyboard_actions.delete_selected = false;
        keyboard_actions.clear_graph = false;
    }
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
        let nodes_to_close: Vec<_> = graph_app.focused_selection().iter().copied().collect();
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
    graph_app.extend_workbench_intents(input::workbench_intents_from_actions(&keyboard_actions));
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
            if let Some(node) = graph_app.domain_graph().get_node(selected_key) {
                if node.lifecycle == NodeLifecycle::Active {
                    let default_node_pane = NodePaneState::for_node(selected_key);
                    if tile_runtime::node_pane_uses_composited_runtime(
                        &default_node_pane,
                        graph_app,
                    ) {
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
        apply_intents_if_any(graph_app, tiles_tree, &mut prewarm_intents);
    }
}

fn history_preview_mode_active(graph_app: &GraphBrowserApp) -> bool {
    graph_app.history_health_summary().preview_mode_active
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

    if history_preview_mode_active(graph_app) {
        frame_intents.clear();
        return;
    }

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
    let preview_mode_active = history_preview_mode_active(graph_app);

    *toolbar_height = Length::new(ctx.available_rect().min.y);
    if !preview_mode_active {
        graph_app.check_periodic_snapshot();
    }

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
            suppress_runtime_side_effects: preview_mode_active,
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
    if !preview_mode_active && let Some(target_dir) = graph_app.take_pending_switch_data_dir() {
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
    apply_intents_if_any(graph_app, tiles_tree, &mut post_render_intents);

    let open_settings_tool_pane = render::render_choose_frame_picker(ctx, graph_app)
        || render::render_unsaved_frame_prompt(ctx, graph_app);
    if open_settings_tool_pane {
        tile_view_ops::open_or_focus_tool_pane(tiles_tree, ToolPaneState::Settings);
    }

    if !preview_mode_active {
        pending_actions::run_post_render_pending_actions(
            graph_app,
            window,
            tiles_tree,
            tile_rendering_contexts,
            tile_favicon_textures,
            webview_creation_backpressure,
            focused_node_hint,
        );
    }
}

fn handle_pending_frame_snapshot_actions(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
) {
    frame_persistence::handle_pending_frame_snapshot_actions(graph_app, tiles_tree);
}

fn handle_pending_graph_snapshot_actions(
    graph_app: &mut GraphBrowserApp,
    window: &EmbedderWindow,
    tiles_tree: &mut Tree<TileKind>,
    tile_rendering_contexts: &mut HashMap<NodeKey, Rc<OffscreenRenderingContext>>,
    tile_favicon_textures: &mut HashMap<NodeKey, (u64, egui::TextureHandle)>,
    webview_creation_backpressure: &mut HashMap<NodeKey, WebviewCreationBackpressureState>,
    focused_node_hint: &mut Option<NodeKey>,
) {
    graph_snapshot::handle_pending_graph_snapshot_actions(
        graph_app,
        window,
        tiles_tree,
        tile_rendering_contexts,
        tile_favicon_textures,
        webview_creation_backpressure,
        focused_node_hint,
    );
}

fn serialize_tiles_tree_layout_json(tiles_tree: &Tree<TileKind>, context: &str) -> Option<String> {
    match serde_json::to_string(tiles_tree) {
        Ok(layout_json) => Some(layout_json),
        Err(e) => {
            warn!("Failed to serialize tile layout for {context}: {e}");
            None
        }
    }
}

fn reset_graph_workspace_after_snapshot_restore(
    tiles_tree: &mut Tree<TileKind>,
    tile_rendering_contexts: &mut HashMap<NodeKey, Rc<OffscreenRenderingContext>>,
    tile_favicon_textures: &mut HashMap<NodeKey, (u64, egui::TextureHandle)>,
    webview_creation_backpressure: &mut HashMap<NodeKey, WebviewCreationBackpressureState>,
    focused_node_hint: &mut Option<NodeKey>,
) {
    let previous_focus_hint = *focused_node_hint;
    tile_rendering_contexts.clear();
    tile_favicon_textures.clear();
    webview_creation_backpressure.clear();
    *focused_node_hint = None;
    if previous_focus_hint != *focused_node_hint {
        diagnostics::emit_event(diagnostics::DiagnosticEvent::MessageReceived {
            channel_id: CHANNEL_UX_NAVIGATION_TRANSITION,
            latency_us: 0,
        });
    }
    let mut tiles = Tiles::default();
    let graph_tile_id = tiles.insert_pane(TileKind::Graph(GraphViewId::default()));
    *tiles_tree = Tree::new("graphshell_tiles", graph_tile_id, tiles);
}

#[cfg(all(test, feature = "diagnostics"))]
mod tests {
    use super::*;
    use crate::app::GraphIntent;

    #[test]
    fn snapshot_restore_focus_reset_emits_ux_navigation_transition_channel() {
        let mut tiles = Tiles::default();
        let root = tiles.insert_pane(TileKind::Graph(GraphViewId::default()));
        let mut tree = Tree::new("graphshell_tiles", root, tiles);
        let mut tile_rendering_contexts: HashMap<NodeKey, Rc<OffscreenRenderingContext>> =
            HashMap::new();
        let mut tile_favicon_textures: HashMap<NodeKey, (u64, egui::TextureHandle)> =
            HashMap::new();
        let mut webview_creation_backpressure: HashMap<NodeKey, WebviewCreationBackpressureState> =
            HashMap::new();
        let mut focused_node_hint = Some(NodeKey::new(9));
        let mut diagnostics = diagnostics::DiagnosticsState::new();

        reset_graph_workspace_after_snapshot_restore(
            &mut tree,
            &mut tile_rendering_contexts,
            &mut tile_favicon_textures,
            &mut webview_creation_backpressure,
            &mut focused_node_hint,
        );

        diagnostics.force_drain_for_tests();
        let snapshot = diagnostics.snapshot_json_for_tests().to_string();
        assert!(
            snapshot.contains("ux:navigation_transition"),
            "expected ux:navigation_transition when snapshot restore clears focus hint"
        );
    }

    #[test]
    fn history_preview_mode_active_tracks_preview_flag() {
        let mut app = GraphBrowserApp::new_for_testing();
        assert!(!history_preview_mode_active(&app));

        app.apply_reducer_intents([GraphIntent::EnterHistoryTimelinePreview]);
        assert!(history_preview_mode_active(&app));

        app.apply_reducer_intents([GraphIntent::ExitHistoryTimelinePreview]);
        assert!(!history_preview_mode_active(&app));
    }
}

#[cfg(test)]
mod connected_open_tests {
    use super::*;
    use crate::app::PendingConnectedOpenScope;
    use euclid::Point2D;

    #[test]
    fn connected_scope_depth_two_dedupes_shared_second_hop() {
        let mut app = GraphBrowserApp::new_for_testing();
        let source = app.add_node_and_sync("https://source.example".into(), Point2D::zero());
        let left = app.add_node_and_sync("https://left.example".into(), Point2D::new(10.0, 0.0));
        let right =
            app.add_node_and_sync("https://right.example".into(), Point2D::new(20.0, 0.0));
        let shared = app.add_node_and_sync(
            "https://shared.example".into(),
            Point2D::new(30.0, 0.0),
        );

        let _ = app.add_edge_and_sync(source, left, crate::model::graph::EdgeType::Hyperlink);
        let _ = app.add_edge_and_sync(source, right, crate::model::graph::EdgeType::Hyperlink);
        let _ = app.add_edge_and_sync(left, shared, crate::model::graph::EdgeType::Hyperlink);
        let _ = app.add_edge_and_sync(right, shared, crate::model::graph::EdgeType::Hyperlink);

        let candidates =
            connected_candidates_with_depth(&app, source, PendingConnectedOpenScope::Connected);

        assert!(candidates.contains(&(left, 1)));
        assert!(candidates.contains(&(right, 1)));
        assert!(candidates.contains(&(shared, 2)));
        assert_eq!(
            candidates
                .iter()
                .filter(|(key, depth)| *key == shared && *depth == 2)
                .count(),
            1,
            "shared second-hop candidate should be emitted once"
        );
    }

    #[test]
    fn neighbors_scope_reports_only_depth_one_neighbors() {
        let mut app = GraphBrowserApp::new_for_testing();
        let source = app.add_node_and_sync("https://source.example".into(), Point2D::zero());
        let neighbor = app.add_node_and_sync(
            "https://neighbor.example".into(),
            Point2D::new(10.0, 0.0),
        );
        let depth_two = app.add_node_and_sync(
            "https://depth-two.example".into(),
            Point2D::new(20.0, 0.0),
        );

        let _ = app.add_edge_and_sync(source, neighbor, crate::model::graph::EdgeType::Hyperlink);
        let _ = app.add_edge_and_sync(neighbor, depth_two, crate::model::graph::EdgeType::Hyperlink);

        let candidates =
            connected_candidates_with_depth(&app, source, PendingConnectedOpenScope::Neighbors);

        assert_eq!(candidates, vec![(neighbor, 1)]);
    }
}
