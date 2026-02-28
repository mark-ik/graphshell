/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::time::{Duration, Instant};

use egui_tiles::Tree;
#[cfg(feature = "diagnostics")]
use egui_tiles::{Container, Tile, TileId};
use servo::{OffscreenRenderingContext, WebViewId, WindowRenderingContext};

use super::tile_behavior::PendingOpenMode;
use super::tile_compositor;
use super::tile_invariants;
use super::tile_kind::TileKind;
use super::tile_post_render;
use super::tile_runtime;
use super::tile_view_ops::{self, TileOpenMode};
use crate::app::{GraphBrowserApp, GraphIntent};
use crate::graph::NodeKey;
use crate::shell::desktop::host::running_app_state::RunningAppState;
use crate::shell::desktop::host::window::EmbedderWindow;
use crate::shell::desktop::lifecycle::webview_backpressure::{
    self, WebviewCreationBackpressureState,
};

pub(crate) struct TileRenderPassArgs<'a> {
    pub ctx: &'a egui::Context,
    pub graph_app: &'a mut GraphBrowserApp,
    pub window: &'a EmbedderWindow,
    pub tiles_tree: &'a mut Tree<TileKind>,
    pub tile_rendering_contexts: &'a mut HashMap<NodeKey, Rc<OffscreenRenderingContext>>,
    pub tile_favicon_textures: &'a mut HashMap<NodeKey, (u64, egui::TextureHandle)>,
    pub graph_search_matches: &'a HashSet<NodeKey>,
    pub active_search_match: Option<NodeKey>,
    pub graph_search_filter_mode: bool,
    pub search_query_active: bool,
    pub app_state: &'a Option<Rc<RunningAppState>>,
    pub rendering_context: &'a Rc<OffscreenRenderingContext>,
    pub window_rendering_context: &'a Rc<WindowRenderingContext>,
    pub responsive_webviews: &'a HashSet<WebViewId>,
    pub webview_creation_backpressure: &'a mut HashMap<NodeKey, WebviewCreationBackpressureState>,
    pub focused_node_hint: &'a mut Option<NodeKey>,
    pub graph_surface_focused: bool,
    pub focus_ring_node_key: &'a mut Option<NodeKey>,
    pub focus_ring_started_at: &'a mut Option<Instant>,
    pub focus_ring_duration: Duration,
    pub control_panel: &'a mut crate::shell::desktop::runtime::control_panel::ControlPanel,
    #[cfg(feature = "diagnostics")]
    pub diagnostics_state: &'a mut crate::shell::desktop::runtime::diagnostics::DiagnosticsState,
}

fn open_mode_from_pending(mode: PendingOpenMode) -> TileOpenMode {
    match mode {
        PendingOpenMode::SplitHorizontal => TileOpenMode::SplitHorizontal,
    }
}

#[cfg(feature = "diagnostics")]
fn tile_hierarchy_lines(
    tiles_tree: &Tree<TileKind>,
    graph_app: &GraphBrowserApp,
) -> Vec<crate::shell::desktop::runtime::diagnostics::HierarchySample> {
    fn push_lines(
        tiles_tree: &Tree<TileKind>,
        graph_app: &GraphBrowserApp,
        tile_id: TileId,
        depth: usize,
        active: &HashSet<TileId>,
        out: &mut Vec<crate::shell::desktop::runtime::diagnostics::HierarchySample>,
    ) {
        let Some(tile) = tiles_tree.tiles.get(tile_id) else {
            return;
        };
        let indent = "  ".repeat(depth);
        let marker = if active.contains(&tile_id) { "*" } else { " " };
        let (label, node_key) = match tile {
            Tile::Pane(TileKind::Graph(_)) => ("Graph".to_string(), None),
            Tile::Pane(TileKind::Node(state)) => {
                let mapped = graph_app.get_webview_for_node(state.node).is_some();
                (
                    format!("Node {:?} mapped={}", state.node, mapped),
                    Some(state.node),
                )
            }
            #[cfg(feature = "diagnostics")]
            Tile::Pane(TileKind::Tool(_)) => ("Tool".to_string(), None),
            Tile::Container(Container::Tabs(tabs)) => {
                (format!("Tab Group ({} tabs)", tabs.children.len()), None)
            }
            Tile::Container(Container::Linear(linear)) => {
                use egui_tiles::LinearDir;
                let dir_label = match linear.dir {
                    LinearDir::Horizontal => "Split ↔",
                    LinearDir::Vertical => "Split ↕",
                };
                (
                    format!("{} ({} panes)", dir_label, linear.children.len()),
                    None,
                )
            }
            Tile::Container(other) => (format!("Panel Group ({:?})", other.kind()), None),
        };
        out.push(
            crate::shell::desktop::runtime::diagnostics::HierarchySample {
                line: format!("{}{} {:?} {}", indent, marker, tile_id, label),
                node_key,
            },
        );

        match tile {
            Tile::Container(Container::Tabs(tabs)) => {
                for child in &tabs.children {
                    push_lines(tiles_tree, graph_app, *child, depth + 1, active, out);
                }
            }
            Tile::Container(Container::Linear(linear)) => {
                for child in &linear.children {
                    push_lines(tiles_tree, graph_app, *child, depth + 1, active, out);
                }
            }
            _ => {}
        }
    }

    let mut out = Vec::new();
    let active: HashSet<TileId> = tiles_tree.active_tiles().into_iter().collect();
    if let Some(root) = tiles_tree.root() {
        push_lines(tiles_tree, graph_app, root, 0, &active, &mut out);
    }
    out
}

pub(crate) fn run_tile_render_pass(args: TileRenderPassArgs<'_>) -> Vec<GraphIntent> {
    #[cfg(feature = "diagnostics")]
    let render_pass_started = Instant::now();
    let TileRenderPassArgs {
        ctx,
        graph_app,
        window,
        tiles_tree,
        tile_rendering_contexts,
        tile_favicon_textures,
        graph_search_matches,
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
    } = args;

    tile_runtime::refresh_node_pane_render_modes(tiles_tree, graph_app);

    let mut post_render_intents = Vec::new();
    let mut pending_open_nodes = Vec::new();
    let mut pending_closed_nodes = Vec::new();
    egui::CentralPanel::default()
        .frame(egui::Frame::new().fill(egui::Color32::from_rgb(20, 20, 25)))
        .show(ctx, |ui| {
            let outputs = tile_post_render::render_tile_tree_and_collect_outputs(
                ui,
                tiles_tree,
                graph_app,
                control_panel,
                tile_favicon_textures,
                graph_search_matches,
                active_search_match,
                graph_search_filter_mode,
                search_query_active,
                #[cfg(feature = "diagnostics")]
                diagnostics_state,
            );
            pending_open_nodes.extend(outputs.pending_open_nodes);
            pending_closed_nodes.extend(outputs.pending_closed_nodes);
            post_render_intents.extend(outputs.post_render_intents);
        });

    #[cfg(feature = "diagnostics")]
    diagnostics_state.record_intents(&post_render_intents);

    if !pending_open_nodes.is_empty() {
        for open in pending_open_nodes.iter() {
            log::debug!(
                "tile_render_pass: pending open node {:?} mode {:?}",
                open.key,
                open.mode
            );
        }
    }

    for open in pending_open_nodes {
        tile_view_ops::open_or_focus_node_pane_with_mode(
            tiles_tree,
            open.key,
            open_mode_from_pending(open.mode),
        );
    }

    #[cfg(feature = "diagnostics")]
    if let Some(node_key) = diagnostics_state.take_pending_focus_node() {
        tile_view_ops::open_or_focus_node_pane_with_mode(tiles_tree, node_key, TileOpenMode::Tab);
        post_render_intents.push(GraphIntent::SelectNode {
            key: node_key,
            multi_select: false,
        });
        post_render_intents.push(GraphIntent::PromoteNodeToActive {
            key: node_key,
            cause: crate::app::LifecycleCause::UserSelect,
        });
    }
    for node_key in pending_closed_nodes {
           tile_runtime::release_node_runtime_for_pane(
            graph_app,
            window,
            tile_rendering_contexts,
            node_key,
            &mut post_render_intents,
        );
    }

    for node_key in tile_post_render::mapped_nodes_without_tiles(graph_app, tiles_tree) {
           tile_runtime::release_node_runtime_for_pane(
            graph_app,
            window,
            tile_rendering_contexts,
            node_key,
            &mut post_render_intents,
        );
    }

    let repaired_active_tile = tile_view_ops::ensure_active_tile(tiles_tree);
    if repaired_active_tile {
        log::debug!("tile_render_pass: repaired empty active tile selection");
    }

    tile_runtime::refresh_node_pane_render_modes(tiles_tree, graph_app);

    let active_tile_rects = tile_compositor::active_node_pane_rects(tiles_tree);
    log::debug!(
        "tile_render_pass: {} active tile rects",
        active_tile_rects.len()
    );
    for (key, rect) in active_tile_rects.iter() {
        let mapped = graph_app.get_webview_for_node(*key);
        let has_context = tile_rendering_contexts.contains_key(key);
        log::debug!(
            "tile_render_pass: active tile {:?} rect {:?} mapped_runtime_viewer={:?} has_context={}",
            key,
            rect,
            mapped,
            has_context
        );
    }

    let all_tile_nodes = tile_runtime::all_node_pane_keys(tiles_tree);
    log::debug!("tile_render_pass: {} all tile nodes", all_tile_nodes.len());
    for node_key in all_tile_nodes.iter().copied() {
        log::debug!("tile_render_pass: tile node {:?}", node_key);
        // Debug: find why node might be inactive
        let tile_id = tiles_tree.tiles.iter().find_map(|(id, tile)| {
            if let egui_tiles::Tile::Pane(TileKind::Node(state)) = tile {
                if state.node == node_key {
                    Some(*id)
                } else {
                    None
                }
            } else {
                None
            }
        });
        if let Some(tid) = tile_id {
            let parent = tiles_tree.tiles.parent_of(tid);
            let is_visible = tiles_tree.is_visible(tid);
            log::debug!(
                "tile_render_pass: node {:?} -> tile {:?} parent={:?} visible={}",
                node_key,
                tid,
                parent,
                is_visible
            );
            if let Some(pid) = parent {
                if let Some(egui_tiles::Tile::Container(container)) = tiles_tree.tiles.get(pid) {
                    log::debug!(
                        "tile_render_pass: parent {:?} is {:?}",
                        pid,
                        container.kind()
                    );
                    if let egui_tiles::Container::Tabs(tabs) = container {
                        log::debug!(
                            "tile_render_pass: parent tabs active={:?} children={:?}",
                            tabs.active,
                            tabs.children
                        );
                    }
                }
            }
        }
    }

    let active_tiles = tiles_tree.active_tiles();
    log::debug!("tile_render_pass: {} egui active_tiles", active_tiles.len());
    for tile_id in active_tiles.iter().copied() {
        let tile_label = match tiles_tree.tiles.get(tile_id) {
            Some(egui_tiles::Tile::Pane(TileKind::Node(_))) => "Node",
            Some(egui_tiles::Tile::Pane(TileKind::Graph(_))) => "Graph",
            #[cfg(feature = "diagnostics")]
            Some(egui_tiles::Tile::Pane(TileKind::Tool(_))) => "Tool",
            Some(egui_tiles::Tile::Container(_)) => "Container",
            None => "Missing",
        };
        log::debug!(
            "tile_render_pass: active tile {:?} kind {}",
            tile_id,
            tile_label
        );
    }

    // Ensure runtime viewers exist for active tiles, applying intents immediately
    // so compositing (below) can find mapped runtime viewers via get_webview_for_node.
    let mut runtime_viewer_creation_intents = Vec::new();
    let composited_runtime_nodes =
        tile_runtime::all_node_pane_keys_using_composited_runtime(tiles_tree, graph_app);
    for (node_key, _) in active_tile_rects.iter().copied() {
        if !composited_runtime_nodes.contains(&node_key) {
            continue;
        }
        log::debug!(
            "tile_render_pass: ensuring runtime viewer for active node {:?}",
            node_key
        );
        webview_backpressure::ensure_webview_for_node(
            graph_app,
            window,
            app_state,
            rendering_context,
            window_rendering_context,
            tile_rendering_contexts,
            node_key,
            responsive_webviews,
            webview_creation_backpressure,
            &mut runtime_viewer_creation_intents,
        );
    }
    log::debug!(
        "tile_render_pass: {} runtime viewer creation intents",
        runtime_viewer_creation_intents.len()
    );
    if !runtime_viewer_creation_intents.is_empty() {
        #[cfg(feature = "diagnostics")]
        let apply_started = Instant::now();
        graph_app.apply_intents(runtime_viewer_creation_intents);
        #[cfg(feature = "diagnostics")]
        diagnostics_state.record_span_duration(
            "app::apply_intents",
            apply_started.elapsed().as_micros() as u64,
        );
        log::debug!("tile_render_pass: applied runtime viewer creation intents");
        for (node_key, _) in active_tile_rects.iter().copied() {
            if let Some(wv_id) = graph_app.get_webview_for_node(node_key) {
                log::debug!(
                    "tile_render_pass: node {:?} NOW mapped to {:?}",
                    node_key,
                    wv_id
                );
            }
        }
    }
    let focused_node_key = if graph_surface_focused {
        *focused_node_hint = None;
        None
    } else {
        tile_compositor::activate_focused_node_for_frame(
            window,
            tiles_tree,
            graph_app,
            focused_node_hint,
        );

        let active_tile_violations = tile_invariants::collect_active_tile_mapping_violations(
            tiles_tree,
            graph_app,
            tile_rendering_contexts,
        );
        if !active_tile_violations.is_empty() {
            for violation in &active_tile_violations {
                log::warn!("tile_render_pass: {}", violation);
            }
            #[cfg(feature = "diagnostics")]
            crate::shell::desktop::runtime::diagnostics::emit_event(
                crate::shell::desktop::runtime::diagnostics::DiagnosticEvent::MessageSent {
                    channel_id: "tile_render_pass.active_tile_violation",
                    byte_len: active_tile_violations.len(),
                },
            );
        }
        let focused_node_key = tile_compositor::focused_node_key_for_node_panes(
            tiles_tree,
            graph_app,
            *focused_node_hint,
        );
        *focused_node_hint = focused_node_key;
        focused_node_key
    };
    if *focus_ring_node_key != focused_node_key {
        *focus_ring_node_key = focused_node_key;
        *focus_ring_started_at = focused_node_key.map(|_| Instant::now());
    }

    let focus_ring_alpha = if *focus_ring_node_key == focused_node_key {
        focus_ring_started_at
            .as_ref()
            .map(|started| {
                let elapsed = started.elapsed();
                if elapsed >= focus_ring_duration {
                    0.0
                } else {
                    1.0 - (elapsed.as_secs_f32() / focus_ring_duration.as_secs_f32())
                }
            })
            .unwrap_or(0.0)
    } else {
        0.0
    };

    #[cfg(feature = "diagnostics")]
    let composite_started = Instant::now();
    tile_compositor::composite_active_node_pane_webviews(
        ctx,
        tiles_tree,
        window,
        graph_app,
        tile_rendering_contexts,
        active_tile_rects,
        focused_node_key,
        focus_ring_alpha,
    );
    #[cfg(feature = "diagnostics")]
    diagnostics_state.record_span_duration(
        "tile_compositor::composite_active_node_pane_webviews",
        composite_started.elapsed().as_micros() as u64,
    );

    #[cfg(feature = "diagnostics")]
    {
        let active_tiles_for_diag = tile_compositor::active_node_pane_rects(tiles_tree);
        let focused_node_present = focused_node_key.is_some();
        let tiles = active_tiles_for_diag
            .iter()
            .map(|(node_key, rect)| {
                let mapped_webview = graph_app.get_webview_for_node(*node_key).is_some();
                let has_context = tile_rendering_contexts.contains_key(node_key);
                let paint_callback_registered = mapped_webview && has_context;
                let render_path_hint = if paint_callback_registered {
                    "composited"
                } else if mapped_webview {
                    "missing-context"
                } else {
                    "unmapped-node-viewer"
                };
                crate::shell::desktop::runtime::diagnostics::CompositorTileSample {
                    node_key: *node_key,
                    rect: *rect,
                    mapped_webview,
                    has_context,
                    paint_callback_registered,
                    render_path_hint,
                }
            })
            .collect();
        diagnostics_state.push_frame(
            crate::shell::desktop::runtime::diagnostics::CompositorFrameSample {
                sequence: 0,
                active_tile_count: active_tiles_for_diag.len(),
                focused_node_present,
                viewport_rect: ctx.available_rect(),
                hierarchy: tile_hierarchy_lines(tiles_tree, graph_app),
                tiles,
            },
        );

        if let Some(hovered_node) = diagnostics_state.highlighted_tile_node()
            && let Some((_, hovered_rect)) = active_tiles_for_diag
                .iter()
                .copied()
                .find(|(node_key, _)| *node_key == hovered_node)
        {
            let overlay_layer = egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::new("graphshell_diag_hover_overlay"),
            );
            ctx.layer_painter(overlay_layer).rect_stroke(
                hovered_rect.shrink(1.0),
                4.0,
                egui::Stroke::new(2.0, egui::Color32::from_rgb(255, 80, 80)),
                egui::StrokeKind::Inside,
            );
        }
    }

    #[cfg(feature = "diagnostics")]
    diagnostics_state.record_span_duration(
        "tile_render_pass::run_tile_render_pass",
        render_pass_started.elapsed().as_micros() as u64,
    );

    post_render_intents
}
