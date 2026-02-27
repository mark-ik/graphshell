/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::HashMap;
use std::rc::Rc;
#[cfg(feature = "diagnostics")]
use std::time::Instant;

use egui::Stroke;
use egui_tiles::{Tile, Tree};
use servo::OffscreenRenderingContext;

use crate::app::GraphBrowserApp;
use crate::graph::NodeKey;
use crate::shell::desktop::host::window::EmbedderWindow;
use crate::shell::desktop::workbench::compositor_adapter::{
    CompositedContentPassOutcome, CompositorAdapter, CompositorPassTracker, OverlayAffordanceStyle,
    OverlayStrokePass,
};
use crate::shell::desktop::workbench::pane_model::TileRenderMode;
use crate::shell::desktop::workbench::tile_kind::TileKind;

#[derive(Clone, Copy)]
enum ScheduledOverlay {
    Focus,
    Hover,
}

#[derive(Clone, Copy)]
struct ScheduledPanePass {
    node_key: NodeKey,
    tile_rect: egui::Rect,
    render_mode: TileRenderMode,
    overlay: Option<ScheduledOverlay>,
}

fn schedule_active_node_pane_passes(
    tiles_tree: &Tree<TileKind>,
    active_tile_rects: Vec<(NodeKey, egui::Rect)>,
    focused_node_key: Option<NodeKey>,
    focus_ring_alpha: f32,
    hovered_node_key: Option<NodeKey>,
) -> Vec<ScheduledPanePass> {
    let mut out = Vec::with_capacity(active_tile_rects.len());
    for (node_key, tile_rect) in active_tile_rects {
        let render_mode = render_mode_for_node_pane(tiles_tree, node_key);
        let overlay = if focused_node_key == Some(node_key) && focus_ring_alpha > 0.0 {
            Some(ScheduledOverlay::Focus)
        } else if hovered_node_key == Some(node_key) {
            Some(ScheduledOverlay::Hover)
        } else {
            None
        };
        out.push(ScheduledPanePass {
            node_key,
            tile_rect,
            render_mode,
            overlay,
        });
    }
    out
}

pub(crate) fn active_node_pane_rects(tiles_tree: &Tree<TileKind>) -> Vec<(NodeKey, egui::Rect)> {
    let mut tile_rects = Vec::new();
    for tile_id in tiles_tree.active_tiles() {
        if let Some(Tile::Pane(TileKind::Node(state))) = tiles_tree.tiles.get(tile_id)
            && let Some(rect) = tiles_tree.tiles.rect(tile_id)
        {
            tile_rects.push((state.node, rect));
        }
    }
    tile_rects
}

pub(crate) fn focused_node_key_for_node_panes(
    tiles_tree: &Tree<TileKind>,
    _graph_app: &GraphBrowserApp,
    focused_hint: Option<NodeKey>,
) -> Option<NodeKey> {
    if let Some(node_key) = focused_hint {
        let hint_present_in_tree = tiles_tree.tiles.iter().any(|(_, tile)| {
            matches!(tile, Tile::Pane(TileKind::Node(state)) if state.node == node_key)
        });
        if hint_present_in_tree {
            return Some(node_key);
        }
    }

    active_node_pane_key(tiles_tree)
}

pub(crate) fn node_for_frame_activation(
    tiles_tree: &Tree<TileKind>,
    graph_app: &GraphBrowserApp,
    focused_hint: Option<NodeKey>,
) -> Option<NodeKey> {
    focused_node_key_for_node_panes(tiles_tree, graph_app, focused_hint)
        .or_else(|| active_node_pane_rects(tiles_tree).first().map(|(node_key, _)| *node_key))
}

pub(crate) fn activate_focused_node_for_frame(
    window: &EmbedderWindow,
    tiles_tree: &Tree<TileKind>,
    graph_app: &GraphBrowserApp,
    focused_node_hint: &mut Option<NodeKey>,
) {
    if let Some(node_key) = node_for_frame_activation(tiles_tree, graph_app, *focused_node_hint) {
        *focused_node_hint = Some(node_key);
        if let Some(wv_id) = graph_app.get_webview_for_node(node_key) {
            window.activate_webview(wv_id);
        }
    }
}

pub(crate) fn composite_active_node_pane_webviews(
    ctx: &egui::Context,
    tiles_tree: &Tree<TileKind>,
    window: &EmbedderWindow,
    graph_app: &GraphBrowserApp,
    tile_rendering_contexts: &mut HashMap<NodeKey, Rc<OffscreenRenderingContext>>,
    active_tile_rects: Vec<(NodeKey, egui::Rect)>,
    focused_node_key: Option<NodeKey>,
    focus_ring_alpha: f32,
) {
    #[cfg(feature = "diagnostics")]
    let composite_started = Instant::now();
    log::debug!(
        "composite_active_node_pane_runtime_viewers: {} tiles",
        active_tile_rects.len()
    );
    let mut pass_tracker = CompositorPassTracker::new();
    let mut pending_overlay_passes: Vec<OverlayStrokePass> = Vec::new();
    let hover_pos = ctx.input(|i| i.pointer.hover_pos());
    let mut hovered_node_key: Option<NodeKey> = None;
    if let Some(pos) = hover_pos {
        for (node_key, tile_rect) in active_tile_rects.iter().copied() {
            if !tile_rect.contains(pos) {
                continue;
            }
            hovered_node_key = Some(node_key);
            break;
        }
    }
    let scheduled_passes = schedule_active_node_pane_passes(
        tiles_tree,
        active_tile_rects,
        focused_node_key,
        focus_ring_alpha,
        hovered_node_key,
    );
    for pass in scheduled_passes {
        let node_key = pass.node_key;
        let tile_rect = pass.tile_rect;
        let render_mode = pass.render_mode;
        let node_webview_id = graph_app.get_webview_for_node(node_key);

        if render_mode == TileRenderMode::CompositedTexture {
            let Some(render_context) = tile_rendering_contexts.get(&node_key).cloned() else {
                log::debug!("composite: no render_context for node {:?}", node_key);
                continue;
            };

            let Some(webview_id) = node_webview_id else {
                log::debug!("composite: no runtime viewer mapped for node {:?}", node_key);
                continue;
            };
            let Some(webview) = window.webview_by_id(webview_id) else {
                log::debug!(
                    "composite: runtime viewer {:?} not found in window for node {:?}",
                    webview_id,
                    node_key
                );
                continue;
            };
            log::debug!(
                "composite: painting runtime viewer {:?} for node {:?} at rect {:?}",
                webview_id,
                node_key,
                tile_rect
            );
            match CompositorAdapter::compose_webview_content_pass(
                ctx,
                node_key,
                tile_rect,
                ctx.pixels_per_point(),
                &render_context,
                &webview,
            ) {
                CompositedContentPassOutcome::Registered => {
                    log::debug!(
                        "composite: registered content pass callback for runtime viewer {:?}",
                        webview_id
                    );
                    pass_tracker.record_content_pass(node_key);
                }
                CompositedContentPassOutcome::MissingContentCallback => {
                    log::debug!(
                        "composite: no adapter content callback available for runtime viewer {:?}",
                        webview_id
                    );
                }
                CompositedContentPassOutcome::PaintFailed
                | CompositedContentPassOutcome::InvalidTileRect => {
                    continue;
                }
            }
        }

        match pass.overlay {
            Some(ScheduledOverlay::Focus) => pending_overlay_passes.push(focus_overlay_for_mode(
                render_mode,
                node_key,
                tile_rect,
                focus_ring_alpha,
            )),
            Some(ScheduledOverlay::Hover) => {
                pending_overlay_passes.push(hover_overlay_for_mode(render_mode, node_key, tile_rect))
            }
            None => {}
        }
    }
    CompositorAdapter::execute_overlay_affordance_pass(ctx, &pass_tracker, pending_overlay_passes);

    #[cfg(feature = "diagnostics")]
    crate::shell::desktop::runtime::diagnostics::emit_span_duration(
        "tile_compositor::composite_active_node_pane_webviews",
        composite_started.elapsed().as_micros() as u64,
    );
}

fn active_node_pane_key(tiles_tree: &Tree<TileKind>) -> Option<NodeKey> {
    tiles_tree
        .active_tiles()
        .into_iter()
        .find_map(|tile_id| match tiles_tree.tiles.get(tile_id) {
            Some(Tile::Pane(TileKind::Node(state))) => Some(state.node),
            _ => None,
        })
}

fn render_mode_for_node_pane(tiles_tree: &Tree<TileKind>, node_key: NodeKey) -> TileRenderMode {
    tiles_tree
        .tiles
        .iter()
        .find_map(|(_, tile)| match tile {
            Tile::Pane(TileKind::Node(state)) if state.node == node_key => Some(state.render_mode),
            _ => None,
        })
        .unwrap_or(TileRenderMode::Placeholder)
}

fn focus_overlay_for_mode(
    render_mode: TileRenderMode,
    node_key: NodeKey,
    tile_rect: egui::Rect,
    focus_ring_alpha: f32,
) -> OverlayStrokePass {
    let alpha = (focus_ring_alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
    let (rect, rounding, stroke, style) = match render_mode {
        TileRenderMode::CompositedTexture => (
            tile_rect,
            4.0,
            Stroke::new(
                2.0,
                egui::Color32::from_rgba_unmultiplied(120, 200, 255, alpha),
            ),
            OverlayAffordanceStyle::RectStroke,
        ),
        TileRenderMode::NativeOverlay => (
            tile_rect,
            0.0,
            Stroke::new(
                2.0,
                egui::Color32::from_rgba_unmultiplied(120, 200, 255, alpha),
            ),
            OverlayAffordanceStyle::ChromeOnly,
        ),
        TileRenderMode::EmbeddedEgui | TileRenderMode::Placeholder => (
            tile_rect,
            4.0,
            Stroke::new(
                2.0,
                egui::Color32::from_rgba_unmultiplied(120, 200, 255, alpha),
            ),
            OverlayAffordanceStyle::RectStroke,
        ),
    };

    OverlayStrokePass {
        node_key,
        tile_rect: rect,
        rounding,
        stroke,
        style,
        render_mode,
    }
}

fn hover_overlay_for_mode(
    render_mode: TileRenderMode,
    node_key: NodeKey,
    tile_rect: egui::Rect,
) -> OverlayStrokePass {
    let (rect, rounding, stroke, style) = match render_mode {
        TileRenderMode::CompositedTexture => (
            tile_rect,
            4.0,
            Stroke::new(1.5, egui::Color32::from_rgba_unmultiplied(180, 180, 190, 180)),
            OverlayAffordanceStyle::RectStroke,
        ),
        TileRenderMode::NativeOverlay => (
            tile_rect,
            0.0,
            Stroke::new(1.5, egui::Color32::from_rgba_unmultiplied(180, 180, 190, 180)),
            OverlayAffordanceStyle::ChromeOnly,
        ),
        TileRenderMode::EmbeddedEgui | TileRenderMode::Placeholder => (
            tile_rect,
            4.0,
            Stroke::new(1.5, egui::Color32::from_rgba_unmultiplied(180, 180, 190, 180)),
            OverlayAffordanceStyle::RectStroke,
        ),
    };

    OverlayStrokePass {
        node_key,
        tile_rect: rect,
        rounding,
        stroke,
        style,
        render_mode,
    }
}
