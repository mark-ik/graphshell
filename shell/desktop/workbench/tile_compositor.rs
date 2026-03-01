/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::{Mutex, OnceLock};
#[cfg(feature = "diagnostics")]
use std::time::Instant;

use egui::Stroke;
use egui_tiles::{Tile, Tree};
use servo::OffscreenRenderingContext;

use crate::app::GraphBrowserApp;
use crate::graph::NodeKey;
use crate::shell::desktop::host::window::EmbedderWindow;
use crate::shell::desktop::runtime::diagnostics::{DiagnosticEvent, emit_event};
use crate::shell::desktop::runtime::registries::{
    CHANNEL_COMPOSITOR_CONTENT_CULLED_OFFVIEWPORT,
    CHANNEL_COMPOSITOR_DEGRADATION_GPU_PRESSURE,
    CHANNEL_COMPOSITOR_DEGRADATION_PLACEHOLDER_MODE,
    CHANNEL_COMPOSITOR_DIFFERENTIAL_CONTENT_COMPOSED,
    CHANNEL_COMPOSITOR_DIFFERENTIAL_CONTENT_SKIPPED,
    CHANNEL_COMPOSITOR_DIFFERENTIAL_FALLBACK_NO_PRIOR_SIGNATURE,
    CHANNEL_COMPOSITOR_DIFFERENTIAL_FALLBACK_SIGNATURE_CHANGED,
    CHANNEL_COMPOSITOR_DIFFERENTIAL_SKIP_RATE_SAMPLE, CHANNEL_COMPOSITOR_FOCUS_ACTIVATION_DEFERRED,
};
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

const DEFAULT_COMPOSITED_CONTENT_BUDGET_PER_FRAME: usize = 6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CompositedContentSignature {
    webview_id: servo::WebViewId,
    rect_px: [i32; 4],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DifferentialComposeReason {
    NoPriorSignature,
    SignatureChanged,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DifferentialContentDecision {
    Compose(DifferentialComposeReason),
    SkipUnchanged,
}

static COMPOSITED_CONTENT_SIGNATURES: OnceLock<Mutex<HashMap<NodeKey, CompositedContentSignature>>> =
    OnceLock::new();

fn composited_content_signatures(
) -> &'static Mutex<HashMap<NodeKey, CompositedContentSignature>> {
    COMPOSITED_CONTENT_SIGNATURES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn content_signature_for_tile(
    webview_id: servo::WebViewId,
    tile_rect: egui::Rect,
    pixels_per_point: f32,
) -> CompositedContentSignature {
    let min_x = (tile_rect.min.x * pixels_per_point).round() as i32;
    let min_y = (tile_rect.min.y * pixels_per_point).round() as i32;
    let max_x = (tile_rect.max.x * pixels_per_point).round() as i32;
    let max_y = (tile_rect.max.y * pixels_per_point).round() as i32;
    CompositedContentSignature {
        webview_id,
        rect_px: [min_x, min_y, max_x, max_y],
    }
}

fn differential_content_decision(
    node_key: NodeKey,
    signature: CompositedContentSignature,
) -> DifferentialContentDecision {
    let mut cache = composited_content_signatures()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    match cache.insert(node_key, signature) {
        None => DifferentialContentDecision::Compose(DifferentialComposeReason::NoPriorSignature),
        Some(previous) if previous != signature => {
            DifferentialContentDecision::Compose(DifferentialComposeReason::SignatureChanged)
        }
        Some(_) => DifferentialContentDecision::SkipUnchanged,
    }
}

fn retain_composited_signature_cache(active_nodes: &HashSet<NodeKey>) {
    let mut cache = composited_content_signatures()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    cache.retain(|node_key, _| active_nodes.contains(node_key));
}

fn differential_fallback_channel(reason: DifferentialComposeReason) -> &'static str {
    match reason {
        DifferentialComposeReason::NoPriorSignature => {
            CHANNEL_COMPOSITOR_DIFFERENTIAL_FALLBACK_NO_PRIOR_SIGNATURE
        }
        DifferentialComposeReason::SignatureChanged => {
            CHANNEL_COMPOSITOR_DIFFERENTIAL_FALLBACK_SIGNATURE_CHANGED
        }
    }
}

fn should_cull_tile_content(tile_rect: egui::Rect, viewport_rect: egui::Rect) -> bool {
    tile_rect.width() <= 0.0 || tile_rect.height() <= 0.0 || !tile_rect.intersects(viewport_rect)
}

fn should_degrade_for_gpu_pressure(
    composed_content_passes: usize,
    budget_per_frame: usize,
) -> bool {
    composed_content_passes >= budget_per_frame
}

#[cfg(test)]
fn clear_composited_signature_cache_for_tests() {
    composited_content_signatures()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clear();
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

fn mapped_active_node_for_activation_fallback(
    tiles_tree: &Tree<TileKind>,
    graph_app: &GraphBrowserApp,
    excluded: Option<NodeKey>,
) -> Option<NodeKey> {
    tiles_tree
        .active_tiles()
        .into_iter()
        .filter_map(|tile_id| match tiles_tree.tiles.get(tile_id) {
            Some(Tile::Pane(TileKind::Node(state))) => Some(state.node),
            _ => None,
        })
        .find(|node_key| {
            Some(*node_key) != excluded && graph_app.get_webview_for_node(*node_key).is_some()
        })
}

fn frame_activation_targets(
    tiles_tree: &Tree<TileKind>,
    graph_app: &GraphBrowserApp,
    focused_hint: Option<NodeKey>,
) -> (Option<NodeKey>, Option<NodeKey>) {
    let primary = node_for_frame_activation(tiles_tree, graph_app, focused_hint);
    let fallback = primary.and_then(|node_key| {
        if graph_app.get_webview_for_node(node_key).is_some() {
            None
        } else {
            mapped_active_node_for_activation_fallback(tiles_tree, graph_app, Some(node_key))
        }
    });
    (primary, fallback)
}

pub(crate) fn activate_focused_node_for_frame(
    window: &EmbedderWindow,
    tiles_tree: &Tree<TileKind>,
    graph_app: &GraphBrowserApp,
    focused_node_hint: &mut Option<NodeKey>,
) {
    let (primary, fallback) = frame_activation_targets(tiles_tree, graph_app, *focused_node_hint);
    if let Some(node_key) = primary {
        *focused_node_hint = Some(node_key);
        if let Some(wv_id) = graph_app.get_webview_for_node(node_key) {
            window.activate_webview(wv_id);
        } else if let Some(fallback_node) = fallback
            && let Some(fallback_wv_id) = graph_app.get_webview_for_node(fallback_node)
        {
            log::debug!(
                "tile_compositor: deferring activation for unmapped focus node {:?}; using mapped fallback {:?}",
                node_key,
                fallback_node
            );
            emit_event(DiagnosticEvent::MessageSent {
                channel_id: CHANNEL_COMPOSITOR_FOCUS_ACTIVATION_DEFERRED,
                byte_len: 1,
            });
            window.activate_webview(fallback_wv_id);
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
    let mut active_composited_nodes = HashSet::new();
    let mut evaluated_composited_passes = 0usize;
    let mut skipped_composited_passes = 0usize;
    let mut composed_composited_passes = 0usize;
    let viewport_rect = ctx.viewport_rect();
    for pass in scheduled_passes {
        let node_key = pass.node_key;
        let tile_rect = pass.tile_rect;
        let render_mode = pass.render_mode;
        let node_webview_id = graph_app.get_webview_for_node(node_key);

        if should_cull_tile_content(tile_rect, viewport_rect) {
            emit_event(DiagnosticEvent::MessageSent {
                channel_id: CHANNEL_COMPOSITOR_CONTENT_CULLED_OFFVIEWPORT,
                byte_len: 1,
            });
            continue;
        }

        if render_mode == TileRenderMode::CompositedTexture {
            let Some(webview_id) = node_webview_id else {
                log::debug!("composite: no runtime viewer mapped for node {:?}", node_key);
                continue;
            };
            active_composited_nodes.insert(node_key);

            let signature = content_signature_for_tile(webview_id, tile_rect, ctx.pixels_per_point());
            evaluated_composited_passes += 1;
            match differential_content_decision(node_key, signature) {
                DifferentialContentDecision::SkipUnchanged => {
                    skipped_composited_passes += 1;
                    pass_tracker.record_content_pass(node_key);
                    emit_event(DiagnosticEvent::MessageSent {
                        channel_id: CHANNEL_COMPOSITOR_DIFFERENTIAL_CONTENT_SKIPPED,
                        byte_len: 1,
                    });
                }
                DifferentialContentDecision::Compose(reason) => {
                    if should_degrade_for_gpu_pressure(
                        composed_composited_passes,
                        DEFAULT_COMPOSITED_CONTENT_BUDGET_PER_FRAME,
                    ) {
                        skipped_composited_passes += 1;
                        pass_tracker.record_content_pass(node_key);
                        emit_event(DiagnosticEvent::MessageSent {
                            channel_id: CHANNEL_COMPOSITOR_DEGRADATION_GPU_PRESSURE,
                            byte_len: DEFAULT_COMPOSITED_CONTENT_BUDGET_PER_FRAME,
                        });
                        emit_event(DiagnosticEvent::MessageSent {
                            channel_id: CHANNEL_COMPOSITOR_DEGRADATION_PLACEHOLDER_MODE,
                            byte_len: 1,
                        });
                        match pass.overlay {
                            Some(ScheduledOverlay::Focus) => {
                                pending_overlay_passes.push(focus_overlay_for_mode(
                                    TileRenderMode::Placeholder,
                                    node_key,
                                    tile_rect,
                                    focus_ring_alpha,
                                ))
                            }
                            Some(ScheduledOverlay::Hover) => {
                                pending_overlay_passes.push(hover_overlay_for_mode(
                                    TileRenderMode::Placeholder,
                                    node_key,
                                    tile_rect,
                                ))
                            }
                            None => {}
                        }
                        continue;
                    }

                    emit_event(DiagnosticEvent::MessageSent {
                        channel_id: CHANNEL_COMPOSITOR_DIFFERENTIAL_CONTENT_COMPOSED,
                        byte_len: 1,
                    });
                    emit_event(DiagnosticEvent::MessageSent {
                        channel_id: differential_fallback_channel(reason),
                        byte_len: 1,
                    });

                    let Some(render_context) = tile_rendering_contexts.get(&node_key).cloned() else {
                        log::debug!("composite: no render_context for node {:?}", node_key);
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
                            composed_composited_passes += 1;
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
                pending_overlay_passes
                    .push(hover_overlay_for_mode(render_mode, node_key, tile_rect))
            }
            None => {}
        }
    }
    if evaluated_composited_passes > 0 {
        let skip_rate_basis_points = (skipped_composited_passes * 10_000) / evaluated_composited_passes;
        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_COMPOSITOR_DIFFERENTIAL_SKIP_RATE_SAMPLE,
            byte_len: skip_rate_basis_points,
        });
    }
    retain_composited_signature_cache(&active_composited_nodes);
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

#[cfg(test)]
mod tests {
    use super::*;
    use base::id::{PIPELINE_NAMESPACE, PainterId, PipelineNamespace, TEST_NAMESPACE};
    use egui_tiles::Tiles;
    use std::panic::AssertUnwindSafe;
    use std::sync::Arc;
    use std::sync::atomic::AtomicU64;

    use crate::shell::desktop::runtime::diagnostics::DiagnosticsState;
    use crate::shell::desktop::runtime::registries::CHANNEL_COMPOSITOR_FOCUS_ACTIVATION_DEFERRED;

    fn test_webview_id() -> servo::WebViewId {
        PIPELINE_NAMESPACE.with(|tls| {
            if tls.get().is_none() {
                PipelineNamespace::install(TEST_NAMESPACE);
            }
        });
        servo::WebViewId::new(PainterId::next())
    }

    fn tree_with_two_active_nodes(a: NodeKey, b: NodeKey) -> Tree<TileKind> {
        let mut tiles = Tiles::default();
        let graph = tiles.insert_pane(TileKind::Graph(crate::app::GraphViewId::default()));
        let a_tile = tiles.insert_pane(TileKind::Node(a.into()));
        let b_tile = tiles.insert_pane(TileKind::Node(b.into()));
        let root = tiles.insert_tab_tile(vec![graph, a_tile, b_tile]);
        let mut tree = Tree::new("tile_compositor_focus_targets", root, tiles);
        let _ = tree.make_active(|_, tile| {
            matches!(tile, Tile::Pane(TileKind::Node(state)) if state.node == a)
        });
        let _ = tree.make_active(|_, tile| {
            matches!(tile, Tile::Pane(TileKind::Node(state)) if state.node == b)
        });
        tree
    }

    fn tree_with_node_render_modes(
        a: NodeKey,
        a_mode: TileRenderMode,
        b: NodeKey,
        b_mode: TileRenderMode,
    ) -> Tree<TileKind> {
        let mut tiles = Tiles::default();
        let graph = tiles.insert_pane(TileKind::Graph(crate::app::GraphViewId::default()));
        let a_tile = tiles.insert_pane(TileKind::Node(a.into()));
        let b_tile = tiles.insert_pane(TileKind::Node(b.into()));

        if let Some(Tile::Pane(TileKind::Node(state))) = tiles.get_mut(a_tile) {
            state.render_mode = a_mode;
        }
        if let Some(Tile::Pane(TileKind::Node(state))) = tiles.get_mut(b_tile) {
            state.render_mode = b_mode;
        }

        let root = tiles.insert_tab_tile(vec![graph, a_tile, b_tile]);
        let mut tree = Tree::new("tile_compositor_render_mode_schedule", root, tiles);
        let _ = tree.make_active(|_, tile| {
            matches!(tile, Tile::Pane(TileKind::Node(state)) if state.node == a)
        });
        let _ = tree.make_active(|_, tile| {
            matches!(tile, Tile::Pane(TileKind::Node(state)) if state.node == b)
        });
        tree
    }

    #[test]
    fn frame_activation_targets_prefers_primary_when_mapped() {
        let mut app = GraphBrowserApp::new_for_testing();
        let a = NodeKey::new(1);
        let b = NodeKey::new(2);
        let tree = tree_with_two_active_nodes(a, b);
        app.map_webview_to_node(test_webview_id(), a);

        let (primary, fallback) = frame_activation_targets(&tree, &app, Some(a));

        assert_eq!(primary, Some(a));
        assert_eq!(fallback, None);
    }

    #[test]
    fn frame_activation_targets_retains_unmapped_primary_and_uses_mapped_fallback() {
        let mut app = GraphBrowserApp::new_for_testing();
        let a = NodeKey::new(3);
        let b = NodeKey::new(4);
        let tree = tree_with_two_active_nodes(a, b);
        app.map_webview_to_node(test_webview_id(), b);

        let (primary, fallback) = frame_activation_targets(&tree, &app, Some(a));

        assert_eq!(primary, Some(a));
        assert_eq!(fallback, Some(b));
    }

    #[test]
    fn deferred_focus_activation_emits_diagnostics_channel() {
        let mut app = GraphBrowserApp::new_for_testing();
        let primary = NodeKey::new(7);
        let fallback = NodeKey::new(8);
        let tree = tree_with_two_active_nodes(primary, fallback);
        app.map_webview_to_node(test_webview_id(), fallback);

        let mut diagnostics = DiagnosticsState::new();
        let prefs = crate::prefs::AppPreferences::default();
        let window = crate::shell::desktop::host::window::EmbedderWindow::new(
            crate::shell::desktop::host::headless_window::HeadlessWindow::new(&prefs),
            Arc::new(AtomicU64::new(0)),
        );

        let mut focused_hint = Some(primary);
        let _ = std::panic::catch_unwind(AssertUnwindSafe(|| {
            activate_focused_node_for_frame(&window, &tree, &app, &mut focused_hint)
        }));

        diagnostics.force_drain_for_tests();
        let snapshot = diagnostics.snapshot_json_for_tests();
        let channel_count = snapshot
            .get("channels")
            .and_then(|c| c.get("message_counts"))
            .and_then(|m| m.get(CHANNEL_COMPOSITOR_FOCUS_ACTIVATION_DEFERRED))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        assert!(
            channel_count > 0,
            "expected deferred focus activation channel to be emitted"
        );
        assert_eq!(focused_hint, Some(primary));
    }

    #[test]
    fn schedule_passes_mark_focused_composited_tile_for_focus_overlay() {
        let focused = NodeKey::new(30);
        let other = NodeKey::new(31);
        let tree = tree_with_node_render_modes(
            focused,
            TileRenderMode::CompositedTexture,
            other,
            TileRenderMode::NativeOverlay,
        );
        let passes = schedule_active_node_pane_passes(
            &tree,
            vec![
                (
                    focused,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(100.0, 60.0)),
                ),
                (
                    other,
                    egui::Rect::from_min_max(egui::pos2(120.0, 0.0), egui::pos2(220.0, 60.0)),
                ),
            ],
            Some(focused),
            1.0,
            None,
        );

        let focused_pass = passes
            .iter()
            .find(|pass| pass.node_key == focused)
            .expect("focused node pass should be scheduled");
        assert_eq!(focused_pass.render_mode, TileRenderMode::CompositedTexture);
        assert!(matches!(focused_pass.overlay, Some(ScheduledOverlay::Focus)));
    }

    #[test]
    fn schedule_passes_mark_hovered_native_overlay_tile_for_hover_overlay() {
        let focused = NodeKey::new(32);
        let hovered = NodeKey::new(33);
        let tree = tree_with_node_render_modes(
            focused,
            TileRenderMode::CompositedTexture,
            hovered,
            TileRenderMode::NativeOverlay,
        );
        let passes = schedule_active_node_pane_passes(
            &tree,
            vec![
                (
                    focused,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(100.0, 60.0)),
                ),
                (
                    hovered,
                    egui::Rect::from_min_max(egui::pos2(120.0, 0.0), egui::pos2(220.0, 60.0)),
                ),
            ],
            Some(focused),
            1.0,
            Some(hovered),
        );

        let hovered_pass = passes
            .iter()
            .find(|pass| pass.node_key == hovered)
            .expect("hovered node pass should be scheduled");
        assert_eq!(hovered_pass.render_mode, TileRenderMode::NativeOverlay);
        assert!(matches!(hovered_pass.overlay, Some(ScheduledOverlay::Hover)));
    }

    #[test]
    fn focus_overlay_for_native_overlay_uses_chrome_only_style() {
        let node = NodeKey::new(40);
        let tile_rect = egui::Rect::from_min_max(egui::pos2(10.0, 10.0), egui::pos2(110.0, 70.0));
        let overlay = focus_overlay_for_mode(TileRenderMode::NativeOverlay, node, tile_rect, 1.0);

        assert!(matches!(overlay.style, OverlayAffordanceStyle::ChromeOnly));
        assert_eq!(overlay.render_mode, TileRenderMode::NativeOverlay);
    }

    #[test]
    fn focus_overlay_for_composited_texture_uses_rect_stroke_style() {
        let node = NodeKey::new(41);
        let tile_rect = egui::Rect::from_min_max(egui::pos2(20.0, 20.0), egui::pos2(120.0, 80.0));
        let overlay =
            focus_overlay_for_mode(TileRenderMode::CompositedTexture, node, tile_rect, 1.0);

        assert!(matches!(overlay.style, OverlayAffordanceStyle::RectStroke));
        assert_eq!(overlay.render_mode, TileRenderMode::CompositedTexture);
    }

    #[test]
    fn hover_overlay_for_native_overlay_uses_chrome_only_style() {
        let node = NodeKey::new(42);
        let tile_rect = egui::Rect::from_min_max(egui::pos2(30.0, 30.0), egui::pos2(130.0, 90.0));
        let overlay = hover_overlay_for_mode(TileRenderMode::NativeOverlay, node, tile_rect);

        assert!(matches!(overlay.style, OverlayAffordanceStyle::ChromeOnly));
        assert_eq!(overlay.render_mode, TileRenderMode::NativeOverlay);
    }

    #[test]
    fn hover_overlay_for_composited_texture_uses_rect_stroke_style() {
        let node = NodeKey::new(43);
        let tile_rect = egui::Rect::from_min_max(egui::pos2(40.0, 40.0), egui::pos2(140.0, 100.0));
        let overlay =
            hover_overlay_for_mode(TileRenderMode::CompositedTexture, node, tile_rect);

        assert!(matches!(overlay.style, OverlayAffordanceStyle::RectStroke));
        assert_eq!(overlay.render_mode, TileRenderMode::CompositedTexture);
    }

    #[test]
    fn differential_content_decision_skips_when_signature_is_unchanged() {
        clear_composited_signature_cache_for_tests();
        let node = NodeKey::new(50);
        let signature = content_signature_for_tile(
            test_webview_id(),
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(100.0, 60.0)),
            1.0,
        );

        assert!(matches!(
            differential_content_decision(node, signature),
            DifferentialContentDecision::Compose(DifferentialComposeReason::NoPriorSignature)
        ));
        assert!(matches!(
            differential_content_decision(node, signature),
            DifferentialContentDecision::SkipUnchanged
        ));
    }

    #[test]
    fn differential_content_decision_recomposes_when_signature_changes() {
        clear_composited_signature_cache_for_tests();
        let node = NodeKey::new(51);
        let webview = test_webview_id();
        let original = content_signature_for_tile(
            webview,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(100.0, 60.0)),
            1.0,
        );
        let changed = content_signature_for_tile(
            webview,
            egui::Rect::from_min_max(egui::pos2(10.0, 0.0), egui::pos2(110.0, 60.0)),
            1.0,
        );

        let _ = differential_content_decision(node, original);
        assert!(matches!(
            differential_content_decision(node, changed),
            DifferentialContentDecision::Compose(DifferentialComposeReason::SignatureChanged)
        ));
    }

    #[test]
    fn focus_overlay_scheduling_is_preserved_when_content_signature_is_clean() {
        clear_composited_signature_cache_for_tests();
        let focused = NodeKey::new(52);
        let other = NodeKey::new(53);
        let tree = tree_with_node_render_modes(
            focused,
            TileRenderMode::CompositedTexture,
            other,
            TileRenderMode::CompositedTexture,
        );
        let signature = content_signature_for_tile(
            test_webview_id(),
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(100.0, 60.0)),
            1.0,
        );

        let _ = differential_content_decision(focused, signature);
        assert!(matches!(
            differential_content_decision(focused, signature),
            DifferentialContentDecision::SkipUnchanged
        ));

        let passes = schedule_active_node_pane_passes(
            &tree,
            vec![
                (
                    focused,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(100.0, 60.0)),
                ),
                (
                    other,
                    egui::Rect::from_min_max(egui::pos2(120.0, 0.0), egui::pos2(220.0, 60.0)),
                ),
            ],
            Some(focused),
            1.0,
            None,
        );

        let focused_pass = passes
            .iter()
            .find(|pass| pass.node_key == focused)
            .expect("focused node pass should be scheduled");
        assert!(matches!(focused_pass.overlay, Some(ScheduledOverlay::Focus)));
    }

    #[test]
    fn hover_overlay_scheduling_is_preserved_when_content_signature_is_clean() {
        clear_composited_signature_cache_for_tests();
        let hovered = NodeKey::new(54);
        let other = NodeKey::new(55);
        let tree = tree_with_node_render_modes(
            hovered,
            TileRenderMode::CompositedTexture,
            other,
            TileRenderMode::CompositedTexture,
        );
        let signature = content_signature_for_tile(
            test_webview_id(),
            egui::Rect::from_min_max(egui::pos2(120.0, 0.0), egui::pos2(220.0, 60.0)),
            1.0,
        );

        let _ = differential_content_decision(hovered, signature);
        assert!(matches!(
            differential_content_decision(hovered, signature),
            DifferentialContentDecision::SkipUnchanged
        ));

        let passes = schedule_active_node_pane_passes(
            &tree,
            vec![
                (
                    hovered,
                    egui::Rect::from_min_max(egui::pos2(120.0, 0.0), egui::pos2(220.0, 60.0)),
                ),
                (
                    other,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(100.0, 60.0)),
                ),
            ],
            Some(other),
            1.0,
            Some(hovered),
        );

        let hovered_pass = passes
            .iter()
            .find(|pass| pass.node_key == hovered)
            .expect("hovered node pass should be scheduled");
        assert!(matches!(hovered_pass.overlay, Some(ScheduledOverlay::Hover)));
    }

    #[test]
    fn should_cull_tile_content_when_disjoint_from_viewport() {
        let tile_rect =
            egui::Rect::from_min_max(egui::pos2(300.0, 300.0), egui::pos2(360.0, 360.0));
        let viewport_rect =
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(200.0, 200.0));

        assert!(should_cull_tile_content(tile_rect, viewport_rect));
    }

    #[test]
    fn should_not_cull_tile_content_when_visible_in_viewport() {
        let tile_rect =
            egui::Rect::from_min_max(egui::pos2(40.0, 40.0), egui::pos2(140.0, 100.0));
        let viewport_rect =
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(200.0, 200.0));

        assert!(!should_cull_tile_content(tile_rect, viewport_rect));
    }

    #[test]
    fn gpu_pressure_degradation_triggers_at_budget_boundary() {
        assert!(!should_degrade_for_gpu_pressure(
            DEFAULT_COMPOSITED_CONTENT_BUDGET_PER_FRAME - 1,
            DEFAULT_COMPOSITED_CONTENT_BUDGET_PER_FRAME,
        ));
        assert!(should_degrade_for_gpu_pressure(
            DEFAULT_COMPOSITED_CONTENT_BUDGET_PER_FRAME,
            DEFAULT_COMPOSITED_CONTENT_BUDGET_PER_FRAME,
        ));
    }
}
