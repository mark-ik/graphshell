/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::time::{Duration, Instant};

#[cfg(feature = "diagnostics")]
use egui_tiles::TileId;
use egui_tiles::{Container, Tile, Tree};
use servo::{OffscreenRenderingContext, WebViewId, WindowRenderingContext};

use super::tile_behavior::PendingOpenMode;
use super::tile_compositor;
use super::tile_invariants;
use super::tile_kind::TileKind;
use super::tile_post_render;
use super::tile_runtime;
use super::tile_view_ops::{self, TileOpenMode};
use crate::app::{GraphBrowserApp, GraphIntent, GraphViewId, SearchDisplayMode, VisibleNavigationRegionSet};
use crate::graph::NodeKey;
use crate::render::{self, GraphAction};
use crate::shell::desktop::host::running_app_state::RunningAppState;
use crate::shell::desktop::host::window::{
    ChromeProjectionSource, DialogOwner, EmbedderWindow, InputTarget,
};
use crate::shell::desktop::lifecycle::webview_backpressure::{
    self, WebviewCreationBackpressureState,
};
#[cfg(feature = "diagnostics")]
use crate::shell::desktop::runtime::diagnostics::{DiagnosticEvent, emit_event};
use crate::shell::desktop::runtime::registries;
#[cfg(feature = "diagnostics")]
use crate::shell::desktop::runtime::registries::CHANNEL_UX_NAVIGATION_TRANSITION;
#[cfg(feature = "diagnostics")]
use crate::shell::desktop::ui::gui_state::RuntimeFocusInspector;

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
    pub suppress_runtime_side_effects: bool,
    pub focus_ring_node_key: &'a mut Option<NodeKey>,
    pub focus_ring_started_at: &'a mut Option<Instant>,
    pub focus_ring_duration: Duration,
    pub control_panel: &'a mut crate::shell::desktop::runtime::control_panel::ControlPanel,
    #[cfg(feature = "diagnostics")]
    pub diagnostics_state: &'a mut crate::shell::desktop::runtime::diagnostics::DiagnosticsState,
    #[cfg(feature = "diagnostics")]
    pub runtime_focus_inspector: Option<RuntimeFocusInspector>,
}

fn primary_graph_view_id(graph_app: &GraphBrowserApp, tiles_tree: &Tree<TileKind>) -> GraphViewId {
    tile_view_ops::active_graph_view_id(tiles_tree)
        .or(graph_app.workspace.graph_runtime.focused_view)
        .or_else(|| graph_app.workspace.graph_runtime.views.keys().next().copied())
        .unwrap_or_default()
}

pub(crate) fn render_primary_graph_in_ui(
    ui: &mut egui::Ui,
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    graph_search_matches: &HashSet<NodeKey>,
    active_search_match: Option<NodeKey>,
    graph_search_filter_mode: bool,
    search_query_active: bool,
) -> Vec<GraphIntent> {
    let view_id = primary_graph_view_id(graph_app, tiles_tree);
    let actions = render::render_graph_in_ui_collect_actions(
        ui,
        graph_app,
        view_id,
        graph_search_matches,
        active_search_match,
        if graph_search_filter_mode {
            SearchDisplayMode::Filter
        } else {
            SearchDisplayMode::Highlight
        },
        search_query_active,
    );
    let multi_select_modifier = ui.input(|i| i.modifiers.ctrl);
    let mut post_render_intents = Vec::new();
    let mut pending_open_nodes = Vec::new();
    let mut passthrough_actions = Vec::new();

    for action in actions {
        match action {
            GraphAction::FocusNode(key) => {
                post_render_intents.push(GraphIntent::OpenNodeFrameRouted {
                    key,
                    prefer_frame: None,
                });
            }
            GraphAction::FocusNodeSplit(key) => {
                if let Some(primary) = graph_app.focused_selection().primary()
                    && primary != key
                {
                    post_render_intents.push(GraphIntent::CreateUserGroupedEdge {
                        from: primary,
                        to: key,
                        label: None,
                    });
                }
                post_render_intents.push(GraphIntent::SelectNode {
                    key,
                    multi_select: multi_select_modifier,
                });
                pending_open_nodes.push((key, TileOpenMode::SplitHorizontal));
            }
            other => passthrough_actions.push(other),
        }
    }

    post_render_intents.extend(render::intents_from_graph_actions(passthrough_actions));
    for (node_key, mode) in pending_open_nodes {
        tile_view_ops::open_or_focus_node_pane_with_mode(tiles_tree, graph_app, node_key, mode);
    }
    render::sync_graph_positions_from_layout(graph_app);
    render::render_graph_info_in_ui(ui, graph_app, view_id);
    post_render_intents
}

pub(crate) fn run_tile_render_pass_in_ui(
    ui: &mut egui::Ui,
    args: TileRenderPassArgs<'_>,
) -> Vec<GraphIntent> {
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
        suppress_runtime_side_effects,
        focus_ring_node_key,
        focus_ring_started_at,
        focus_ring_duration,
        control_panel,
        #[cfg(feature = "diagnostics")]
        diagnostics_state,
        #[cfg(feature = "diagnostics")]
        runtime_focus_inspector,
    } = args;
    #[cfg(feature = "diagnostics")]
    let render_pass_started = Instant::now();
    #[cfg(feature = "diagnostics")]
    let focused_node_hint_before = *focused_node_hint;

    tile_runtime::refresh_node_pane_render_modes(tiles_tree, graph_app);

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
        #[cfg(feature = "diagnostics")]
        runtime_focus_inspector,
    );
    let mut post_render_intents = outputs.post_render_intents;
    let pending_open_nodes = outputs.pending_open_nodes;
    let pending_closed_nodes = outputs.pending_closed_nodes;

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
            graph_app,
            open.key,
            open_mode_from_pending(open.mode),
        );
    }

    match render_floating_pane_overlays(ctx, graph_app, tiles_tree) {
        Some(FloatingOverlayAction::Promote) => {
            post_render_intents.push(GraphIntent::PromoteEphemeralPane {
                target_tile_context: infer_floating_target_context(tiles_tree),
            });
        }
        Some(FloatingOverlayAction::Dismiss) => {
            tile_view_ops::dismiss_floating_panes(tiles_tree);
        }
        None => {}
    }

    #[cfg(feature = "diagnostics")]
    if let Some(node_key) = diagnostics_state.take_pending_focus_node() {
        log::debug!(
            "tile_render_pass: diagnostics requested pending focus for node {:?}",
            node_key
        );
        tile_view_ops::open_or_focus_node_pane_with_mode(
            tiles_tree,
            graph_app,
            node_key,
            TileOpenMode::Tab,
        );
        post_render_intents.push(GraphIntent::SelectNode {
            key: node_key,
            multi_select: false,
        });
        post_render_intents.push(
            crate::app::RuntimeEvent::PromoteNodeToActive {
                key: node_key,
                cause: crate::app::LifecycleCause::UserSelect,
            }
            .into(),
        );
    }
    if !pending_closed_nodes.is_empty() {
        log::debug!(
            "tile_render_pass: processing {} pending closed nodes",
            pending_closed_nodes.len()
        );
    }
    if !suppress_runtime_side_effects {
        for node_key in pending_closed_nodes {
            if *focused_node_hint == Some(node_key) {
                log::debug!(
                    "tile_render_pass: clearing focused_node_hint for closed node {:?}",
                    node_key
                );
                *focused_node_hint = None;
            }
            log::debug!(
                "tile_render_pass: releasing runtime for closed node {:?}",
                node_key
            );
            tile_runtime::release_node_runtime_for_pane(
                graph_app,
                window,
                tile_rendering_contexts,
                node_key,
                &mut post_render_intents,
            );
        }

        for node_key in tile_post_render::mapped_nodes_without_tiles(graph_app, tiles_tree) {
            if *focused_node_hint == Some(node_key) {
                log::debug!(
                    "tile_render_pass: clearing focused_node_hint for unmapped node {:?}",
                    node_key
                );
                *focused_node_hint = None;
            }
            log::debug!(
                "tile_render_pass: releasing mapped runtime without tile for node {:?}",
                node_key
            );
            tile_runtime::release_node_runtime_for_pane(
                graph_app,
                window,
                tile_rendering_contexts,
                node_key,
                &mut post_render_intents,
            );
        }
    }

    let repaired_active_tile = tile_view_ops::ensure_active_tile(tiles_tree);
    if repaired_active_tile {
        log::debug!("tile_render_pass: repaired empty active tile selection");
    }
    graph_app.prune_workbench_tile_selection(tiles_tree);
    log::debug!(
        "tile_render_pass: active tile count after handoff {}",
        tiles_tree.active_tiles().len()
    );

    tile_runtime::refresh_node_pane_render_modes(tiles_tree, graph_app);

    let active_tile_rects = tile_compositor::active_node_pane_rects(tiles_tree);
    log::debug!(
        "tile_render_pass: {} active tile rects",
        active_tile_rects.len()
    );
    for (_, key, rect) in active_tile_rects.iter() {
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
            if let Some(pid) = parent
                && let Some(egui_tiles::Tile::Container(container)) = tiles_tree.tiles.get(pid)
            {
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

    let active_tiles = tiles_tree.active_tiles();
    log::debug!("tile_render_pass: {} egui active_tiles", active_tiles.len());
    for tile_id in active_tiles.iter().copied() {
        let tile_label = match tiles_tree.tiles.get(tile_id) {
            Some(egui_tiles::Tile::Pane(TileKind::Pane(
                crate::shell::desktop::workbench::pane_model::PaneViewState::Node(_),
            ))) => "Node",
            Some(egui_tiles::Tile::Pane(TileKind::Pane(
                crate::shell::desktop::workbench::pane_model::PaneViewState::Graph(_),
            ))) => "Graph",
            #[cfg(feature = "diagnostics")]
            Some(egui_tiles::Tile::Pane(TileKind::Pane(
                crate::shell::desktop::workbench::pane_model::PaneViewState::Tool(_),
            ))) => "Tool",
            Some(egui_tiles::Tile::Pane(TileKind::Node(_))) => "Node",
            Some(egui_tiles::Tile::Pane(TileKind::Graph(_))) => "Graph",
            #[cfg(feature = "diagnostics")]
            Some(egui_tiles::Tile::Pane(TileKind::Tool(_))) => "Tool",
            Some(egui_tiles::Tile::Container(_)) => "Container",
            None => "Missing",
        };
        log::debug!("tile_render_pass: active tile {:?} label={}", tile_id, tile_label);
    }

    let visible_node_panes = active_tiles
        .iter()
        .filter_map(|tile_id| match tiles_tree.tiles.get(*tile_id) {
            Some(egui_tiles::Tile::Pane(TileKind::Node(state))) => Some(state.pane_id),
            _ => None,
        })
        .collect();
    window.set_visible_node_panes(visible_node_panes);

    if !suppress_runtime_side_effects {
        let mut runtime_viewer_creation_intents = Vec::new();
        let composited_runtime_nodes =
            tile_runtime::all_node_pane_keys_using_composited_runtime(tiles_tree, graph_app);
        for tile_id in active_tiles.iter().copied() {
            let Some(egui_tiles::Tile::Pane(TileKind::Node(state))) = tiles_tree.tiles.get(tile_id)
            else {
                continue;
            };
            if !composited_runtime_nodes.contains(&state.node) {
                continue;
            }
            log::debug!(
                "tile_render_pass: ensuring runtime viewer for active pane {:?} node {:?}",
                state.pane_id,
                state.node
            );
            webview_backpressure::ensure_webview_for_node(
                graph_app,
                window,
                app_state,
                rendering_context,
                window_rendering_context,
                tile_rendering_contexts,
                Some(state.pane_id),
                state.node,
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
            graph_app.apply_reducer_intents(runtime_viewer_creation_intents);
            #[cfg(feature = "diagnostics")]
            diagnostics_state.record_span_duration(
                "app::apply_intents",
                apply_started.elapsed().as_micros() as u64,
            );
            log::debug!("tile_render_pass: applied runtime viewer creation intents");
            for (_, node_key, _) in active_tile_rects.iter().copied() {
                if let Some(wv_id) = graph_app.get_webview_for_node(node_key) {
                    log::debug!(
                        "tile_render_pass: node {:?} NOW mapped to {:?}",
                        node_key,
                        wv_id
                    );
                }
            }
        }
    }

    let focused_node_pane = if graph_surface_focused {
        *focused_node_hint = None;
        None
    } else {
        tile_compositor::activate_focused_node_for_frame(
            window,
            tiles_tree,
            graph_app,
            focused_node_hint,
        );

        if let Some(node_key) = *focused_node_hint
            && graph_app.get_webview_for_node(node_key).is_none()
        {
            ctx.request_repaint();
        }

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
        let focused_node_pane = tile_compositor::focused_node_pane_for_node_panes(
            tiles_tree,
            graph_app,
            *focused_node_hint,
        );
        *focused_node_hint = focused_node_pane.map(|pane| pane.node_key);
        focused_node_pane
    };

    let focused_node_key = focused_node_pane.map(|pane| pane.node_key);
    let focused_pane_id = focused_node_pane.map(|pane| pane.pane_id).or_else(|| {
        tiles_tree.active_tiles().into_iter().find_map(|tile_id| {
            match tiles_tree.tiles.get(tile_id) {
                Some(egui_tiles::Tile::Pane(pane)) => Some(pane.pane_id()),
                _ => None,
            }
        })
    });

    if let Some(focused_node_pane) = focused_node_pane {
        window.set_focused_pane(Some(focused_node_pane.pane_id));
        window.set_dialog_owner(Some(DialogOwner::Pane(focused_node_pane.pane_id)));
        if let Some(attachment) =
            registries::phase1_renderer_attachment_for_pane(focused_node_pane.pane_id)
        {
            window.set_input_target(Some(InputTarget::Renderer(attachment.renderer_id)));
            window.set_chrome_projection_source(Some(ChromeProjectionSource::Renderer(
                attachment.renderer_id,
            )));
        } else {
            window.set_input_target(Some(InputTarget::Pane(focused_node_pane.pane_id)));
            window.set_chrome_projection_source(Some(ChromeProjectionSource::Pane(
                focused_node_pane.pane_id,
            )));
        }
    } else if let Some(focused_pane_id) = focused_pane_id {
        window.set_focused_pane(Some(focused_pane_id));
        window.set_dialog_owner(Some(DialogOwner::Pane(focused_pane_id)));
        window.set_input_target(Some(InputTarget::Pane(focused_pane_id)));
        window.set_chrome_projection_source(Some(ChromeProjectionSource::Pane(focused_pane_id)));
    } else {
        window.set_focused_pane(None);
        window.set_input_target(None);
        window.set_chrome_projection_source(None);
        window.set_dialog_owner(None);
    }

    #[cfg(feature = "diagnostics")]
    emit_navigation_transition_when_focus_hint_changes(
        focused_node_hint_before,
        *focused_node_hint,
    );
    let focus_delta = tile_compositor::FocusDelta::new(focused_node_hint_before, focused_node_key);
    latch_focus_ring_transition(focus_delta, focus_ring_node_key, focus_ring_started_at);

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
        focus_delta,
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
            .map(|(pane_id, node_key, rect)| {
                let render_mode =
                    crate::shell::desktop::workbench::tile_runtime::render_mode_for_pane_in_tree(
                        tiles_tree,
                        *pane_id,
                    );
                let mapped_webview = graph_app.get_webview_for_node(*node_key).is_some();
                let has_context = tile_rendering_contexts.contains_key(node_key);
                let paint_callback_registered = mapped_webview && has_context;
                let render_path_hint =
                    crate::shell::desktop::workbench::tile_runtime::render_path_hint_for_mode(
                        render_mode,
                        mapped_webview,
                        has_context,
                    );
                let estimated_content_bytes = if render_mode
                    == crate::shell::desktop::workbench::pane_model::TileRenderMode::CompositedTexture
                {
                    crate::shell::desktop::workbench::tile_compositor::estimated_composited_tile_content_bytes(
                        *rect,
                        ctx.pixels_per_point(),
                    )
                } else {
                    0
                };
                crate::shell::desktop::runtime::diagnostics::CompositorTileSample {
                    pane_id: pane_id.to_string(),
                    node_key: *node_key,
                    render_mode,
                    estimated_content_bytes,
                    rect: *rect,
                    mapped_webview,
                    has_context,
                    paint_callback_registered,
                    render_path_hint,
                }
            })
            .collect();
        let (content_rect, visible_regions, occluding_host_rects) = graph_app
            .workspace
            .graph_runtime
            .workbench_navigation_geometry
            .as_ref()
            .map(|geometry| {
                (
                    geometry.content_rect,
                    geometry.visible_region_set_or_content(),
                    geometry.occluding_host_rects.clone(),
                )
            })
            .unwrap_or_else(|| {
                let content_rect = ctx.available_rect();
                (
                    content_rect,
                    VisibleNavigationRegionSet::singleton(content_rect),
                    Vec::new(),
                )
            });
        diagnostics_state.push_frame(
            crate::shell::desktop::runtime::diagnostics::CompositorFrameSample {
                sequence: 0,
                active_tile_count: active_tiles_for_diag.len(),
                focused_node_present,
                content_rect,
                visible_regions,
                occluding_host_rects,
                hierarchy: tile_hierarchy_lines(tiles_tree, graph_app),
                tiles,
            },
        );

        if let Some(hovered_node) = diagnostics_state.highlighted_tile_node()
            && let Some((_, _, hovered_rect)) = active_tiles_for_diag
                .iter()
                .copied()
                .find(|(_, node_key, _)| *node_key == hovered_node)
        {
            let render_mode = active_tiles_for_diag
                .iter()
                .find_map(|(pane_id, node_key, _)| {
                    if *node_key == hovered_node {
                        Some(
                            crate::shell::desktop::workbench::tile_runtime::render_mode_for_pane_in_tree(
                                tiles_tree,
                                *pane_id,
                            ),
                        )
                    } else {
                        None
                    }
                })
                .unwrap_or(crate::shell::desktop::workbench::pane_model::TileRenderMode::Placeholder);
            draw_diagnostics_hover_overlay_for_mode(ctx, hovered_node, hovered_rect, render_mode);
        }
    }

    #[cfg(feature = "diagnostics")]
    diagnostics_state.record_span_duration(
        "tile_render_pass::run_tile_render_pass",
        render_pass_started.elapsed().as_micros() as u64,
    );

    post_render_intents
}

fn latch_focus_ring_transition(
    focus_delta: tile_compositor::FocusDelta,
    focus_ring_node_key: &mut Option<NodeKey>,
    focus_ring_started_at: &mut Option<Instant>,
) {
    if !focus_delta.changed_this_frame {
        return;
    }

    *focus_ring_node_key = focus_delta.new_focused_node;
    *focus_ring_started_at = focus_delta.new_focused_node.map(|_| Instant::now());
}

fn open_mode_from_pending(mode: PendingOpenMode) -> TileOpenMode {
    match mode {
        PendingOpenMode::SplitHorizontal => TileOpenMode::SplitHorizontal,
        PendingOpenMode::QuarterPane => TileOpenMode::QuarterPane,
        PendingOpenMode::HalfPane => TileOpenMode::HalfPane,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FloatingOverlayAction {
    Promote,
    Dismiss,
}

const FLOATING_OVERLAY_MIN_WIDTH: f32 = 220.0;
const FLOATING_OVERLAY_MIN_HEIGHT: f32 = 140.0;
const FLOATING_OVERLAY_REGION_MARGIN: f32 = 12.0;

fn infer_floating_target_context(
    tiles_tree: &Tree<TileKind>,
) -> crate::shell::desktop::workbench::pane_model::FloatingPaneTargetTileContext {
    match tiles_tree
        .root()
        .and_then(|root_id| tiles_tree.tiles.get(root_id))
    {
        Some(Tile::Container(Container::Tabs(_))) => {
            crate::shell::desktop::workbench::pane_model::FloatingPaneTargetTileContext::TabGroup
        }
        Some(Tile::Container(Container::Linear(_))) => {
            crate::shell::desktop::workbench::pane_model::FloatingPaneTargetTileContext::Split
        }
        _ => crate::shell::desktop::workbench::pane_model::FloatingPaneTargetTileContext::BareGraph,
    }
}

fn floating_overlay_rect_for_visible_regions(
    visible_regions: &VisibleNavigationRegionSet,
    enlarged_for_viewer_override: bool,
) -> Option<egui::Rect> {
    visible_regions
        .as_slice()
        .iter()
        .copied()
        .filter(|rect| rect.width() > 0.0 && rect.height() > 0.0)
        .map(|region| {
            let padded_region = region.shrink2(egui::vec2(
                FLOATING_OVERLAY_REGION_MARGIN,
                FLOATING_OVERLAY_REGION_MARGIN,
            ));
            let usable_region = if padded_region.width() > 0.0 && padded_region.height() > 0.0 {
                padded_region
            } else {
                region
            };
            let fraction = if enlarged_for_viewer_override { 0.5 } else { 0.38 };
            let width = (usable_region.width() * fraction)
                .clamp(FLOATING_OVERLAY_MIN_WIDTH.min(usable_region.width()), usable_region.width());
            let height = (usable_region.height() * fraction).clamp(
                FLOATING_OVERLAY_MIN_HEIGHT.min(usable_region.height()),
                usable_region.height(),
            );
            let rect = egui::Rect::from_center_size(
                usable_region.center(),
                egui::vec2(width, height),
            );
            let score = rect.width() * rect.height();
            (score, usable_region.width() * usable_region.height(), rect)
        })
        .max_by(|left, right| {
            left.0
                .partial_cmp(&right.0)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| {
                    left.1
                        .partial_cmp(&right.1)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
        })
        .map(|(_, _, rect)| rect)
}

fn render_floating_pane_overlays(
    ctx: &egui::Context,
    graph_app: &GraphBrowserApp,
    tiles_tree: &Tree<TileKind>,
) -> Option<FloatingOverlayAction> {
    let floating_state = tiles_tree.tiles.iter().find_map(|(_, tile)| match tile {
        Tile::Pane(TileKind::Pane(
            crate::shell::desktop::workbench::pane_model::PaneViewState::Node(state),
        )) if state.presentation_mode
            == crate::shell::desktop::workbench::pane_model::PanePresentationMode::Floating =>
        {
            Some(state)
        }
        _ => None,
    })?;

    let visible_regions = graph_app
        .workspace
        .graph_runtime
        .workbench_navigation_geometry
        .as_ref()
        .map(|geometry| geometry.visible_region_set_or_content())
        .unwrap_or_else(|| VisibleNavigationRegionSet::singleton(ctx.available_rect()));
    let rect = floating_overlay_rect_for_visible_regions(
        &visible_regions,
        floating_state.viewer_id_override.is_some(),
    )
    .unwrap_or_else(|| egui::Rect::from_center_size(ctx.available_rect().center(), egui::vec2(280.0, 180.0)));
    let title = graph_app
        .domain_graph()
        .get_node(floating_state.node)
        .map(|node| node.title.clone())
        .unwrap_or_else(|| format!("Node {:?}", floating_state.node));
    let subtitle = graph_app
        .domain_graph()
        .get_node(floating_state.node)
        .map(|node| node.url.clone())
        .unwrap_or_else(|| "Floating pane".to_string());

    let mut action = None;
    egui::Area::new(egui::Id::new((
        "graphshell_floating_overlay",
        floating_state.pane_id,
    )))
    .order(egui::Order::Foreground)
    .fixed_pos(rect.min)
    .show(ctx, |ui| {
        ui.set_min_size(rect.size());
        let frame_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, rect.size());
        let response = ui.allocate_rect(frame_rect, egui::Sense::hover());
        let hovered = response.hovered();
        ui.painter().rect(
            frame_rect,
            12.0,
            egui::Color32::from_rgba_unmultiplied(22, 24, 30, 244),
            egui::Stroke::new(1.0, egui::Color32::from_rgb(90, 104, 126)),
            egui::StrokeKind::Inside,
        );

        let band_rect =
            egui::Rect::from_min_size(frame_rect.min, egui::vec2(frame_rect.width(), 30.0));
        if hovered {
            ui.painter().rect_filled(
                band_rect,
                12.0,
                egui::Color32::from_rgba_unmultiplied(44, 49, 60, 220),
            );
        }

        let content_rect = frame_rect.shrink2(egui::vec2(16.0, 16.0));
        ui.scope_builder(egui::UiBuilder::new().max_rect(content_rect), |ui| {
            ui.add_space(18.0);
            ui.label(egui::RichText::new(title).size(20.0).strong());
            ui.add_space(4.0);
            ui.label(egui::RichText::new(subtitle).color(egui::Color32::from_rgb(170, 176, 190)));
            ui.add_space(12.0);
            ui.label(
                egui::RichText::new("Floating pane skeleton")
                    .color(egui::Color32::from_rgb(215, 220, 230)),
            );
            ui.add_space(6.0);
            ui.small("This overlay remains ephemeral until promoted into the workbench tile tree.");
        });

        if hovered {
            ui.scope_builder(
                egui::UiBuilder::new().max_rect(band_rect.shrink2(egui::vec2(8.0, 4.0))),
                |ui| {
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        if ui.small_button("▣").clicked() {
                            action = Some(FloatingOverlayAction::Promote);
                        }
                        ui.add_space(4.0);
                        ui.small("Promote");
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("✕").clicked() {
                                action = Some(FloatingOverlayAction::Dismiss);
                            }
                        });
                    });
                },
            );
        }
    });

    action
}

fn draw_diagnostics_hover_overlay_for_mode(
    ctx: &egui::Context,
    node_key: NodeKey,
    hovered_rect: egui::Rect,
    render_mode: crate::shell::desktop::workbench::pane_model::TileRenderMode,
) {
    let stroke = egui::Stroke::new(2.0, egui::Color32::from_rgb(255, 80, 80));
    match render_mode {
        crate::shell::desktop::workbench::pane_model::TileRenderMode::CompositedTexture => {
            let overlay_layer = egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::new("graphshell_diag_hover_overlay"),
            );
            ctx.layer_painter(overlay_layer).rect_stroke(
                hovered_rect.shrink(1.0),
                4.0,
                stroke,
                egui::StrokeKind::Inside,
            );
        }
        crate::shell::desktop::workbench::pane_model::TileRenderMode::NativeOverlay => {
            let overlay_layer = egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::new("graphshell_diag_hover_overlay_native"),
            );
            let painter = ctx.layer_painter(overlay_layer);
            let inset = 2.0;
            let top = hovered_rect.top() + inset;
            let left = hovered_rect.left() + inset;
            let right = hovered_rect.right() - inset;
            let marker_len = 12.0_f32.min((hovered_rect.height() - inset * 2.0).max(0.0));
            painter.line_segment([egui::pos2(left, top), egui::pos2(right, top)], stroke);
            painter.line_segment(
                [egui::pos2(left, top), egui::pos2(left, top + marker_len)],
                stroke,
            );
            painter.line_segment(
                [egui::pos2(right, top), egui::pos2(right, top + marker_len)],
                stroke,
            );
        }
        crate::shell::desktop::workbench::pane_model::TileRenderMode::EmbeddedEgui
        | crate::shell::desktop::workbench::pane_model::TileRenderMode::Placeholder => {
            egui::Area::new(egui::Id::new((
                "graphshell_diag_hover_overlay_area",
                node_key,
            )))
            .order(egui::Order::Tooltip)
            .fixed_pos(hovered_rect.min)
            .interactable(false)
            .show(ctx, |ui| {
                ui.set_min_size(hovered_rect.size());
                ui.painter().rect_stroke(
                    egui::Rect::from_min_size(egui::Pos2::ZERO, hovered_rect.size()).shrink(1.0),
                    4.0,
                    stroke,
                    egui::StrokeKind::Inside,
                );
            });
        }
    }
}

#[cfg(feature = "diagnostics")]
fn emit_navigation_transition_when_focus_hint_changes(
    previous_focus_hint: Option<NodeKey>,
    current_focus_hint: Option<NodeKey>,
) {
    if previous_focus_hint != current_focus_hint {
        emit_event(DiagnosticEvent::MessageReceived {
            channel_id: CHANNEL_UX_NAVIGATION_TRANSITION,
            latency_us: 0,
        });
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
            Tile::Pane(TileKind::Pane(
                crate::shell::desktop::workbench::pane_model::PaneViewState::Graph(_),
            )) => ("Graph".to_string(), None),
            Tile::Pane(TileKind::Pane(
                crate::shell::desktop::workbench::pane_model::PaneViewState::Node(state),
            )) => {
                let mapped = graph_app.get_webview_for_node(state.node).is_some();
                (
                    format!("Floating Node {:?} mapped={}", state.node, mapped),
                    Some(state.node),
                )
            }
            #[cfg(feature = "diagnostics")]
            Tile::Pane(TileKind::Pane(
                crate::shell::desktop::workbench::pane_model::PaneViewState::Tool(_),
            )) => ("Tool".to_string(), None),
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
    let ctx = args.ctx;
    let mut post_render_intents = Vec::new();
    egui::CentralPanel::default()
        .frame(egui::Frame::new().fill(egui::Color32::from_rgb(20, 20, 25)))
        .show(ctx, |ui| {
            post_render_intents = run_tile_render_pass_in_ui(ui, args);
        });
    post_render_intents
}

#[cfg(all(test, feature = "diagnostics"))]
mod tests {
    use super::*;

    #[test]
    fn focus_hint_change_emits_ux_navigation_transition_channel() {
        let mut diagnostics = crate::shell::desktop::runtime::diagnostics::DiagnosticsState::new();

        emit_navigation_transition_when_focus_hint_changes(None, Some(NodeKey::new(1)));

        diagnostics.force_drain_for_tests();
        let snapshot = diagnostics.snapshot_json_for_tests().to_string();
        assert!(
            snapshot.contains("ux:navigation_transition"),
            "expected ux:navigation_transition when tile render pass focus hint changes"
        );
    }

    #[test]
    fn floating_overlay_rect_prefers_largest_fitting_visible_region() {
        let rect = floating_overlay_rect_for_visible_regions(
            &VisibleNavigationRegionSet::from_rects(vec![
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(400.0, 40.0)),
                egui::Rect::from_min_max(egui::pos2(0.0, 40.0), egui::pos2(320.0, 260.0)),
                egui::Rect::from_min_max(egui::pos2(0.0, 260.0), egui::pos2(400.0, 300.0)),
            ]),
            false,
        )
        .expect("expected a floating overlay rect");

        assert!(rect.center().x <= 320.0);
        assert!(rect.center().y >= 40.0 && rect.center().y <= 260.0);
        assert!(rect.width() <= 320.0);
        assert!(rect.height() <= 220.0);
    }

    #[test]
    fn floating_overlay_rect_clamps_to_small_visible_region() {
        let rect = floating_overlay_rect_for_visible_regions(
            &VisibleNavigationRegionSet::from_rects(vec![egui::Rect::from_min_max(
                egui::pos2(10.0, 10.0),
                egui::pos2(180.0, 120.0),
            )]),
            true,
        )
        .expect("expected a floating overlay rect");

        assert!(rect.left() >= 10.0);
        assert!(rect.right() <= 180.0);
        assert!(rect.top() >= 10.0);
        assert!(rect.bottom() <= 120.0);
    }
}
