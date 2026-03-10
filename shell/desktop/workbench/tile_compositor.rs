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
    CHANNEL_COMPOSITOR_CONTENT_CULLED_OFFVIEWPORT, CHANNEL_COMPOSITOR_DEGRADATION_GPU_PRESSURE,
    CHANNEL_COMPOSITOR_DEGRADATION_PLACEHOLDER_MODE,
    CHANNEL_COMPOSITOR_DIFFERENTIAL_CONTENT_COMPOSED,
    CHANNEL_COMPOSITOR_DIFFERENTIAL_FALLBACK_NO_PRIOR_SIGNATURE,
    CHANNEL_COMPOSITOR_DIFFERENTIAL_FALLBACK_SIGNATURE_CHANGED,
    CHANNEL_COMPOSITOR_DIFFERENTIAL_SKIP_RATE_SAMPLE, CHANNEL_COMPOSITOR_FOCUS_ACTIVATION_DEFERRED,
    CHANNEL_COMPOSITOR_OVERLAY_BATCH_SIZE_SAMPLE,
    CHANNEL_COMPOSITOR_OVERLAY_NATIVE_SUPPRESSED_HELP_PANEL,
    CHANNEL_COMPOSITOR_OVERLAY_NATIVE_SUPPRESSED_INTERACTION_MENU,
    CHANNEL_COMPOSITOR_OVERLAY_NATIVE_SUPPRESSED_RADIAL_MENU,
    CHANNEL_COMPOSITOR_RESOURCE_REUSE_CONTEXT_HIT, CHANNEL_COMPOSITOR_RESOURCE_REUSE_CONTEXT_MISS,
};
use crate::shell::desktop::workbench::compositor_adapter::{
    CompositedContentPassOutcome, CompositorAdapter, CompositorPassTracker, OverlayAffordanceStyle,
    OverlayStrokePass,
};
use crate::shell::desktop::workbench::pane_model::{PaneId, TileRenderMode};
use crate::shell::desktop::workbench::{
    interaction_policy::{InteractionUiState, OverlaySuppressionReason},
    tile_kind::TileKind,
};
#[cfg(feature = "wry")]
use crate::{mods::native::verso, mods::native::verso::wry_manager::OverlayRect as WryOverlayRect};

#[derive(Clone, Copy)]
enum ScheduledOverlay {
    Focus,
    Hover,
}

#[derive(Clone, Copy)]
struct ScheduledPanePass {
    pane_id: PaneId,
    node_key: NodeKey,
    tile_rect: egui::Rect,
    render_mode: TileRenderMode,
    overlay: Option<ScheduledOverlay>,
}

#[derive(Clone)]
struct DegradedReceipt {
    tile_rect: egui::Rect,
    message: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FocusedNodePane {
    pub(crate) pane_id: PaneId,
    pub(crate) node_key: NodeKey,
}

#[derive(Default)]
struct CompositedPassCounters {
    evaluated: usize,
    skipped: usize,
    composed: usize,
    composed_estimated_bytes: usize,
}

const DEFAULT_COMPOSITED_CONTENT_BUDGET_BYTES_PER_FRAME: usize = 32 * 1024 * 1024;

pub(crate) fn composited_content_budget_bytes_per_frame() -> usize {
    DEFAULT_COMPOSITED_CONTENT_BUDGET_BYTES_PER_FRAME
}

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

static COMPOSITED_CONTENT_SIGNATURES: OnceLock<
    Mutex<HashMap<NodeKey, CompositedContentSignature>>,
> = OnceLock::new();

fn composited_content_signatures() -> &'static Mutex<HashMap<NodeKey, CompositedContentSignature>> {
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
    composed_content_bytes: usize,
    next_tile_estimated_bytes: usize,
    budget_per_frame_bytes: usize,
) -> bool {
    composed_content_bytes.saturating_add(next_tile_estimated_bytes) > budget_per_frame_bytes
}

fn estimated_tile_content_bytes(tile_rect: egui::Rect, pixels_per_point: f32) -> usize {
    let width_px = (tile_rect.width().max(0.0) * pixels_per_point).ceil() as usize;
    let height_px = (tile_rect.height().max(0.0) * pixels_per_point).ceil() as usize;
    width_px.saturating_mul(height_px).saturating_mul(4)
}

pub(crate) fn estimated_composited_tile_content_bytes(
    tile_rect: egui::Rect,
    pixels_per_point: f32,
) -> usize {
    estimated_tile_content_bytes(tile_rect, pixels_per_point)
}

fn format_gpu_pressure_degraded_receipt(
    estimated_tile_bytes: usize,
    budget_per_frame_bytes: usize,
) -> String {
    let estimated_mib = estimated_tile_bytes as f32 / (1024.0 * 1024.0);
    let budget_mib = budget_per_frame_bytes as f32 / (1024.0 * 1024.0);
    format!(
        "Degraded: deferred {:.1} MiB tile after {:.1} MiB/frame GPU budget.",
        estimated_mib, budget_mib
    )
}

fn sync_native_overlay_for_tile(node_key: NodeKey, tile_rect: egui::Rect, visible: bool) {
    #[cfg(feature = "wry")]
    {
        verso::sync_wry_overlay_for_node(
            node_key,
            WryOverlayRect {
                x: tile_rect.min.x,
                y: tile_rect.min.y,
                width: tile_rect.width(),
                height: tile_rect.height(),
            },
            visible,
        );
    }

    #[cfg(not(feature = "wry"))]
    {
        let _ = (node_key, tile_rect, visible);
    }
}

fn suppression_reason_channel(reason: OverlaySuppressionReason) -> &'static str {
    match reason {
        OverlaySuppressionReason::InteractionMenu => {
            CHANNEL_COMPOSITOR_OVERLAY_NATIVE_SUPPRESSED_INTERACTION_MENU
        }
        OverlaySuppressionReason::HelpPanel => {
            CHANNEL_COMPOSITOR_OVERLAY_NATIVE_SUPPRESSED_HELP_PANEL
        }
        OverlaySuppressionReason::RadialMenu => {
            CHANNEL_COMPOSITOR_OVERLAY_NATIVE_SUPPRESSED_RADIAL_MENU
        }
    }
}

fn run_composited_texture_content_pass(
    ctx: &egui::Context,
    window: &EmbedderWindow,
    graph_app: &GraphBrowserApp,
    tile_rendering_contexts: &mut HashMap<NodeKey, Rc<OffscreenRenderingContext>>,
    pass_tracker: &mut CompositorPassTracker,
    pending_overlay_passes: &mut Vec<OverlayStrokePass>,
    degraded_receipts: &mut Vec<DegradedReceipt>,
    active_composited_nodes: &mut HashSet<NodeKey>,
    counters: &mut CompositedPassCounters,
    node_key: NodeKey,
    tile_rect: egui::Rect,
    focus_ring_alpha: f32,
    overlay: Option<ScheduledOverlay>,
) -> bool {
    let Some(webview_id) = graph_app.get_webview_for_node(node_key) else {
        log::debug!(
            "composite: no runtime viewer mapped for node {:?}",
            node_key
        );
        return false;
    };
    active_composited_nodes.insert(node_key);

    let signature = content_signature_for_tile(webview_id, tile_rect, ctx.pixels_per_point());
    let estimated_tile_bytes = estimated_tile_content_bytes(tile_rect, ctx.pixels_per_point());
    counters.evaluated += 1;
    let differential_decision = differential_content_decision(node_key, signature);

    if should_degrade_for_gpu_pressure(
        counters.composed_estimated_bytes,
        estimated_tile_bytes,
        DEFAULT_COMPOSITED_CONTENT_BUDGET_BYTES_PER_FRAME,
    ) {
        counters.skipped += 1;
        pass_tracker.record_content_pass(node_key);
        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_COMPOSITOR_DEGRADATION_GPU_PRESSURE,
            byte_len: estimated_tile_bytes,
        });
        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_COMPOSITOR_DEGRADATION_PLACEHOLDER_MODE,
            byte_len: 1,
        });
        degraded_receipts.push(DegradedReceipt {
            tile_rect,
            message: format_gpu_pressure_degraded_receipt(
                estimated_tile_bytes,
                DEFAULT_COMPOSITED_CONTENT_BUDGET_BYTES_PER_FRAME,
            ),
        });
        match overlay {
            Some(ScheduledOverlay::Focus) => pending_overlay_passes.push(focus_overlay_for_mode(
                TileRenderMode::Placeholder,
                node_key,
                tile_rect,
                focus_ring_alpha,
            )),
            Some(ScheduledOverlay::Hover) => pending_overlay_passes.push(hover_overlay_for_mode(
                TileRenderMode::Placeholder,
                node_key,
                tile_rect,
            )),
            None => {}
        }
        return false;
    }

    emit_event(DiagnosticEvent::MessageSent {
        channel_id: CHANNEL_COMPOSITOR_DIFFERENTIAL_CONTENT_COMPOSED,
        byte_len: 1,
    });
    if let DifferentialContentDecision::Compose(reason) = differential_decision {
        emit_event(DiagnosticEvent::MessageSent {
            channel_id: differential_fallback_channel(reason),
            byte_len: 1,
        });
    }

    let Some(render_context) = tile_rendering_contexts.get(&node_key).cloned() else {
        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_COMPOSITOR_RESOURCE_REUSE_CONTEXT_MISS,
            byte_len: 1,
        });
        log::debug!("composite: no render_context for node {:?}", node_key);
        return false;
    };
    emit_event(DiagnosticEvent::MessageSent {
        channel_id: CHANNEL_COMPOSITOR_RESOURCE_REUSE_CONTEXT_HIT,
        byte_len: 1,
    });

    let Some(webview) = window.webview_by_id(webview_id) else {
        log::debug!(
            "composite: runtime viewer {:?} not found in window for node {:?}",
            webview_id,
            node_key
        );
        return false;
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
            counters.composed += 1;
            counters.composed_estimated_bytes = counters
                .composed_estimated_bytes
                .saturating_add(estimated_tile_bytes);
            true
        }
        CompositedContentPassOutcome::MissingContentCallback => {
            log::debug!(
                "composite: no adapter content callback available for runtime viewer {:?}",
                webview_id
            );
            true
        }
        CompositedContentPassOutcome::PaintFailed
        | CompositedContentPassOutcome::InvalidTileRect => false,
    }
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
    active_tile_rects: Vec<(PaneId, NodeKey, egui::Rect)>,
    focused_node_key: Option<NodeKey>,
    focus_ring_alpha: f32,
    hovered_node_key: Option<NodeKey>,
) -> Vec<ScheduledPanePass> {
    let mut out = Vec::with_capacity(active_tile_rects.len());
    for (pane_id, node_key, tile_rect) in active_tile_rects {
        let render_mode = render_mode_for_pane(tiles_tree, pane_id);
        let overlay = if focused_node_key == Some(node_key) && focus_ring_alpha > 0.0 {
            Some(ScheduledOverlay::Focus)
        } else if hovered_node_key == Some(node_key) {
            Some(ScheduledOverlay::Hover)
        } else {
            None
        };
        out.push(ScheduledPanePass {
            pane_id,
            node_key,
            tile_rect,
            render_mode,
            overlay,
        });
    }
    out
}

pub(crate) fn active_node_pane_rects(
    tiles_tree: &Tree<TileKind>,
) -> Vec<(PaneId, NodeKey, egui::Rect)> {
    let mut tile_rects = Vec::new();
    for tile_id in tiles_tree.active_tiles() {
        if let Some(Tile::Pane(TileKind::Node(state))) = tiles_tree.tiles.get(tile_id)
            && let Some(rect) = tiles_tree.tiles.rect(tile_id)
        {
            tile_rects.push((state.pane_id, state.node, rect));
        }
    }
    tile_rects
}

pub(crate) fn focused_node_key_for_node_panes(
    tiles_tree: &Tree<TileKind>,
    _graph_app: &GraphBrowserApp,
    focused_hint: Option<NodeKey>,
) -> Option<NodeKey> {
    focused_node_pane_for_node_panes(tiles_tree, _graph_app, focused_hint).map(|pane| pane.node_key)
}

fn hinted_node_pane_for_frame_activation(
    tiles_tree: &Tree<TileKind>,
    focused_hint: Option<NodeKey>,
) -> Option<FocusedNodePane> {
    if let Some(node_key) = focused_hint {
        let hint_present_in_tree = tiles_tree.tiles.iter().find_map(|(_, tile)| match tile {
            Tile::Pane(TileKind::Node(state)) if state.node == node_key => Some(FocusedNodePane {
                pane_id: state.pane_id,
                node_key,
            }),
            _ => None,
        });
        if hint_present_in_tree.is_some() {
            return hint_present_in_tree;
        }
    }

    active_node_pane(tiles_tree)
}

pub(crate) fn focused_node_pane_for_node_panes(
    tiles_tree: &Tree<TileKind>,
    _graph_app: &GraphBrowserApp,
    _focused_hint: Option<NodeKey>,
) -> Option<FocusedNodePane> {
    active_node_pane(tiles_tree)
}

pub(crate) fn node_for_frame_activation(
    tiles_tree: &Tree<TileKind>,
    _graph_app: &GraphBrowserApp,
    focused_hint: Option<NodeKey>,
) -> Option<NodeKey> {
    hinted_node_pane_for_frame_activation(tiles_tree, focused_hint)
        .map(|pane| pane.node_key)
        .or_else(|| {
            active_node_pane_rects(tiles_tree)
                .first()
                .map(|(_, node_key, _)| *node_key)
        })
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
            window.retarget_input_to_webview(wv_id);
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
            window.retarget_input_to_webview(fallback_wv_id);
        }
    }
}

pub(crate) fn composite_active_node_pane_webviews(
    ctx: &egui::Context,
    tiles_tree: &Tree<TileKind>,
    window: &EmbedderWindow,
    graph_app: &GraphBrowserApp,
    tile_rendering_contexts: &mut HashMap<NodeKey, Rc<OffscreenRenderingContext>>,
    active_tile_rects: Vec<(PaneId, NodeKey, egui::Rect)>,
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
    let mut degraded_receipts: Vec<DegradedReceipt> = Vec::new();
    let hover_pos = ctx.input(|i| i.pointer.hover_pos());
    let mut hovered_node_key: Option<NodeKey> = None;
    if let Some(pos) = hover_pos {
        for (_, node_key, tile_rect) in active_tile_rects.iter().copied() {
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
    let interaction_ui = InteractionUiState::new(
        graph_app.workspace.show_command_palette,
        graph_app.workspace.show_help_panel,
        graph_app.workspace.show_radial_menu,
    );
    let mut active_composited_nodes = HashSet::new();
    let mut composited_counters = CompositedPassCounters::default();
    let viewport_rect = ctx.viewport_rect();
    for pass in scheduled_passes {
        let node_key = pass.node_key;
        let tile_rect = pass.tile_rect;
        let render_mode = pass.render_mode;
        let interaction_render_mode = interaction_ui.effective_interaction_render_mode(render_mode);

        if should_cull_tile_content(tile_rect, viewport_rect) {
            if render_mode == TileRenderMode::NativeOverlay {
                sync_native_overlay_for_tile(node_key, tile_rect, false);
            }
            emit_event(DiagnosticEvent::MessageSent {
                channel_id: CHANNEL_COMPOSITOR_CONTENT_CULLED_OFFVIEWPORT,
                byte_len: 1,
            });
            continue;
        }

        if render_mode == TileRenderMode::NativeOverlay {
            // Native overlay backends are synchronized after layout, even though
            // composited texture painting is skipped for this mode.
            let native_overlay_visible = interaction_ui.native_overlay_visible();
            sync_native_overlay_for_tile(node_key, tile_rect, native_overlay_visible);
            if !native_overlay_visible
                && let Some(reason) = interaction_ui.overlay_suppression_reason()
            {
                emit_event(DiagnosticEvent::MessageSent {
                    channel_id: suppression_reason_channel(reason),
                    byte_len: 1,
                });
            }
        }

        if render_mode == TileRenderMode::CompositedTexture
            && !run_composited_texture_content_pass(
                ctx,
                window,
                graph_app,
                tile_rendering_contexts,
                &mut pass_tracker,
                &mut pending_overlay_passes,
                &mut degraded_receipts,
                &mut active_composited_nodes,
                &mut composited_counters,
                node_key,
                tile_rect,
                focus_ring_alpha,
                pass.overlay,
            )
        {
            continue;
        }

        match pass.overlay {
            Some(ScheduledOverlay::Focus) => pending_overlay_passes.push(focus_overlay_for_mode(
                interaction_render_mode,
                node_key,
                tile_rect,
                focus_ring_alpha,
            )),
            Some(ScheduledOverlay::Hover) => pending_overlay_passes.push(hover_overlay_for_mode(
                interaction_render_mode,
                node_key,
                tile_rect,
            )),
            None => {}
        }
    }
    if composited_counters.evaluated > 0 {
        let skip_rate_basis_points =
            (composited_counters.skipped * 10_000) / composited_counters.evaluated;
        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_COMPOSITOR_DIFFERENTIAL_SKIP_RATE_SAMPLE,
            byte_len: skip_rate_basis_points,
        });
    }
    emit_event(DiagnosticEvent::MessageSent {
        channel_id: CHANNEL_COMPOSITOR_OVERLAY_BATCH_SIZE_SAMPLE,
        byte_len: pending_overlay_passes.len(),
    });
    retain_composited_signature_cache(&active_composited_nodes);
    CompositorAdapter::execute_overlay_affordance_pass(ctx, &pass_tracker, pending_overlay_passes);
    render_degraded_receipts(ctx, &degraded_receipts);

    #[cfg(feature = "diagnostics")]
    crate::shell::desktop::runtime::diagnostics::emit_span_duration(
        "tile_compositor::composite_active_node_pane_webviews",
        composite_started.elapsed().as_micros() as u64,
    );
}

fn render_degraded_receipts(ctx: &egui::Context, receipts: &[DegradedReceipt]) {
    if receipts.is_empty() {
        return;
    }

    let layer = egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("graphshell.degraded_receipts"),
    );
    let painter = ctx.layer_painter(layer);
    let font = egui::FontId::proportional(12.0);

    for receipt in receipts {
        let anchor = receipt.tile_rect.left_top() + egui::vec2(8.0, 8.0);
        let box_rect = egui::Rect::from_min_size(anchor, egui::vec2(360.0, 22.0));
        painter.rect_filled(
            box_rect,
            4.0,
            egui::Color32::from_rgba_unmultiplied(45, 30, 20, 225),
        );
        painter.text(
            box_rect.left_center() + egui::vec2(8.0, 0.0),
            egui::Align2::LEFT_CENTER,
            &receipt.message,
            font.clone(),
            egui::Color32::from_rgb(255, 210, 120),
        );
    }
}

pub(crate) fn active_node_pane(tiles_tree: &Tree<TileKind>) -> Option<FocusedNodePane> {
    tiles_tree
        .active_tiles()
        .into_iter()
        .find_map(|tile_id| match tiles_tree.tiles.get(tile_id) {
            Some(Tile::Pane(TileKind::Node(state))) => Some(FocusedNodePane {
                pane_id: state.pane_id,
                node_key: state.node,
            }),
            _ => None,
        })
}

fn render_mode_for_pane(tiles_tree: &Tree<TileKind>, pane_id: PaneId) -> TileRenderMode {
    crate::shell::desktop::workbench::tile_runtime::render_mode_for_pane_in_tree(
        tiles_tree, pane_id,
    )
}

#[derive(Clone, Copy)]
struct OverlayAffordancePolicy {
    style: OverlayAffordanceStyle,
    rounding: f32,
}

fn overlay_affordance_policy_for_render_mode(
    render_mode: TileRenderMode,
) -> OverlayAffordancePolicy {
    match render_mode {
        TileRenderMode::CompositedTexture => OverlayAffordancePolicy {
            style: OverlayAffordanceStyle::RectStroke,
            rounding: 4.0,
        },
        TileRenderMode::NativeOverlay => OverlayAffordancePolicy {
            style: OverlayAffordanceStyle::ChromeOnly,
            rounding: 0.0,
        },
        TileRenderMode::EmbeddedEgui | TileRenderMode::Placeholder => OverlayAffordancePolicy {
            style: OverlayAffordanceStyle::RectStroke,
            rounding: 4.0,
        },
    }
}

fn focus_overlay_for_mode(
    render_mode: TileRenderMode,
    node_key: NodeKey,
    tile_rect: egui::Rect,
    focus_ring_alpha: f32,
) -> OverlayStrokePass {
    let alpha = (focus_ring_alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
    let policy = overlay_affordance_policy_for_render_mode(render_mode);
    let stroke = Stroke::new(
        2.0,
        egui::Color32::from_rgba_unmultiplied(120, 200, 255, alpha),
    );

    OverlayStrokePass {
        node_key,
        tile_rect,
        rounding: policy.rounding,
        stroke,
        style: policy.style,
        render_mode,
    }
}

fn hover_overlay_for_mode(
    render_mode: TileRenderMode,
    node_key: NodeKey,
    tile_rect: egui::Rect,
) -> OverlayStrokePass {
    let policy = overlay_affordance_policy_for_render_mode(render_mode);
    let stroke = Stroke::new(
        1.5,
        egui::Color32::from_rgba_unmultiplied(180, 180, 190, 180),
    );

    OverlayStrokePass {
        node_key,
        tile_rect,
        rounding: policy.rounding,
        stroke,
        style: policy.style,
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

    #[cfg(feature = "wry")]
    use crate::mods::native::verso;
    use crate::shell::desktop::runtime::diagnostics::DiagnosticsState;
    use crate::shell::desktop::runtime::registries::CHANNEL_COMPOSITOR_FOCUS_ACTIVATION_DEFERRED;
    use crate::shell::desktop::workbench::pane_model::GraphPaneRef;

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
        let graph = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(
            crate::app::GraphViewId::default(),
        )));
        let a_tile = tiles.insert_pane(TileKind::Node(a.into()));
        let b_tile = tiles.insert_pane(TileKind::Node(b.into()));
        let root = tiles.insert_tab_tile(vec![graph, a_tile, b_tile]);
        let mut tree = Tree::new("tile_compositor_focus_targets", root, tiles);
        let _ = tree.make_active(
            |_, tile| matches!(tile, Tile::Pane(TileKind::Node(state)) if state.node == a),
        );
        let _ = tree.make_active(
            |_, tile| matches!(tile, Tile::Pane(TileKind::Node(state)) if state.node == b),
        );
        tree
    }

    fn tree_with_node_render_modes(
        a: NodeKey,
        a_mode: TileRenderMode,
        b: NodeKey,
        b_mode: TileRenderMode,
    ) -> Tree<TileKind> {
        let mut tiles = Tiles::default();
        let graph = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(
            crate::app::GraphViewId::default(),
        )));
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
        let _ = tree.make_active(
            |_, tile| matches!(tile, Tile::Pane(TileKind::Node(state)) if state.node == a),
        );
        let _ = tree.make_active(
            |_, tile| matches!(tile, Tile::Pane(TileKind::Node(state)) if state.node == b),
        );
        tree
    }

    fn pane_id_for_node(tree: &Tree<TileKind>, node_key: NodeKey) -> PaneId {
        tree.tiles
            .iter()
            .find_map(|(_, tile)| match tile {
                Tile::Pane(TileKind::Node(state)) if state.node == node_key => Some(state.pane_id),
                _ => None,
            })
            .expect("expected pane id for node")
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
    fn focused_node_pane_returns_stable_pane_identity() {
        let focused = NodeKey::new(30);
        let other = NodeKey::new(31);
        let tree = tree_with_two_active_nodes(focused, other);

        let pane = focused_node_pane_for_node_panes(
            &tree,
            &GraphBrowserApp::new_for_testing(),
            Some(focused),
        )
        .expect("expected focused node pane");

        assert_eq!(pane.node_key, other);
        assert_ne!(pane.pane_id, PaneId::default());
    }

    #[test]
    fn hinted_frame_activation_pane_prefers_present_hint() {
        let focused = NodeKey::new(40);
        let other = NodeKey::new(41);
        let tree = tree_with_two_active_nodes(focused, other);

        let pane = hinted_node_pane_for_frame_activation(&tree, Some(focused))
            .expect("expected hinted node pane");

        assert_eq!(pane.node_key, focused);
        assert_ne!(pane.pane_id, PaneId::default());
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
    fn frame_activation_targets_switch_to_remaining_node_after_pane_close() {
        let mut app = GraphBrowserApp::new_for_testing();
        let a = NodeKey::new(11);
        let b = NodeKey::new(12);
        let a_webview = test_webview_id();
        let b_webview = test_webview_id();
        app.map_webview_to_node(a_webview, a);
        app.map_webview_to_node(b_webview, b);

        let mut tiles = Tiles::default();
        let graph = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(
            crate::app::GraphViewId::default(),
        )));
        let b_tile = tiles.insert_pane(TileKind::Node(b.into()));
        let root = tiles.insert_tab_tile(vec![graph, b_tile]);
        let mut tree = Tree::new("tile_compositor_focus_after_close", root, tiles);
        let _ = tree.make_active(
            |_, tile| matches!(tile, Tile::Pane(TileKind::Node(state)) if state.node == b),
        );

        app.unmap_webview(a_webview);

        let (primary, fallback) = frame_activation_targets(&tree, &app, Some(a));

        assert_eq!(primary, Some(b));
        assert_eq!(fallback, None);
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

    #[cfg(feature = "wry")]
    #[test]
    fn native_overlay_sync_records_visible_true() {
        verso::reset_wry_manager_for_tests();
        let node_key = NodeKey::new(700);
        let rect = egui::Rect::from_min_size(egui::pos2(10.0, 20.0), egui::vec2(300.0, 150.0));

        sync_native_overlay_for_tile(node_key, rect, true);

        let state = verso::last_wry_overlay_sync_for_node_for_tests(node_key)
            .expect("expected overlay sync state");
        assert!(state.visible);
        assert_eq!(state.rect.width, 300.0);
        assert_eq!(state.rect.height, 150.0);
    }

    #[cfg(feature = "wry")]
    #[test]
    fn native_overlay_sync_records_visible_false_for_hidden_tiles() {
        verso::reset_wry_manager_for_tests();
        let node_key = NodeKey::new(701);
        let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(120.0, 80.0));

        sync_native_overlay_for_tile(node_key, rect, false);

        let state = verso::last_wry_overlay_sync_for_node_for_tests(node_key)
            .expect("expected overlay sync state");
        assert!(!state.visible);
    }

    #[test]
    fn suppression_reason_maps_to_expected_channel_ids() {
        assert_eq!(
            suppression_reason_channel(OverlaySuppressionReason::InteractionMenu),
            CHANNEL_COMPOSITOR_OVERLAY_NATIVE_SUPPRESSED_INTERACTION_MENU
        );
        assert_eq!(
            suppression_reason_channel(OverlaySuppressionReason::HelpPanel),
            CHANNEL_COMPOSITOR_OVERLAY_NATIVE_SUPPRESSED_HELP_PANEL
        );
        assert_eq!(
            suppression_reason_channel(OverlaySuppressionReason::RadialMenu),
            CHANNEL_COMPOSITOR_OVERLAY_NATIVE_SUPPRESSED_RADIAL_MENU
        );
    }

    #[test]
    fn suppressed_native_overlay_uses_placeholder_affordance_policy() {
        let ui_state = InteractionUiState::new(true, false, false);
        let effective_mode =
            ui_state.effective_interaction_render_mode(TileRenderMode::NativeOverlay);
        let overlay = focus_overlay_for_mode(
            effective_mode,
            NodeKey::new(702),
            egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(100.0, 100.0)),
            1.0,
        );

        assert_eq!(effective_mode, TileRenderMode::Placeholder);
        assert!(matches!(overlay.style, OverlayAffordanceStyle::RectStroke));
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
        let focused_pane = pane_id_for_node(&tree, focused);
        let other_pane = pane_id_for_node(&tree, other);
        let passes = schedule_active_node_pane_passes(
            &tree,
            vec![
                (
                    focused_pane,
                    focused,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(100.0, 60.0)),
                ),
                (
                    other_pane,
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
        assert!(matches!(
            focused_pass.overlay,
            Some(ScheduledOverlay::Focus)
        ));
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
        let focused_pane = pane_id_for_node(&tree, focused);
        let hovered_pane = pane_id_for_node(&tree, hovered);
        let passes = schedule_active_node_pane_passes(
            &tree,
            vec![
                (
                    focused_pane,
                    focused,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(100.0, 60.0)),
                ),
                (
                    hovered_pane,
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
        assert!(matches!(
            hovered_pass.overlay,
            Some(ScheduledOverlay::Hover)
        ));
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
        let overlay = hover_overlay_for_mode(TileRenderMode::CompositedTexture, node, tile_rect);

        assert!(matches!(overlay.style, OverlayAffordanceStyle::RectStroke));
        assert_eq!(overlay.render_mode, TileRenderMode::CompositedTexture);
    }

    #[test]
    fn focus_overlay_for_placeholder_uses_rect_stroke_style() {
        let node = NodeKey::new(44);
        let tile_rect = egui::Rect::from_min_max(egui::pos2(50.0, 50.0), egui::pos2(150.0, 110.0));
        let overlay = focus_overlay_for_mode(TileRenderMode::Placeholder, node, tile_rect, 1.0);

        assert!(matches!(overlay.style, OverlayAffordanceStyle::RectStroke));
        assert_eq!(overlay.render_mode, TileRenderMode::Placeholder);
    }

    #[test]
    fn hover_overlay_for_embedded_egui_uses_rect_stroke_style() {
        let node = NodeKey::new(45);
        let tile_rect = egui::Rect::from_min_max(egui::pos2(60.0, 60.0), egui::pos2(160.0, 120.0));
        let overlay = hover_overlay_for_mode(TileRenderMode::EmbeddedEgui, node, tile_rect);

        assert!(matches!(overlay.style, OverlayAffordanceStyle::RectStroke));
        assert_eq!(overlay.render_mode, TileRenderMode::EmbeddedEgui);
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
        let focused_pane = pane_id_for_node(&tree, focused);
        let other_pane = pane_id_for_node(&tree, other);

        let passes = schedule_active_node_pane_passes(
            &tree,
            vec![
                (
                    focused_pane,
                    focused,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(100.0, 60.0)),
                ),
                (
                    other_pane,
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
        assert!(matches!(
            focused_pass.overlay,
            Some(ScheduledOverlay::Focus)
        ));
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
        let hovered_pane = pane_id_for_node(&tree, hovered);
        let other_pane = pane_id_for_node(&tree, other);

        let passes = schedule_active_node_pane_passes(
            &tree,
            vec![
                (
                    hovered_pane,
                    hovered,
                    egui::Rect::from_min_max(egui::pos2(120.0, 0.0), egui::pos2(220.0, 60.0)),
                ),
                (
                    other_pane,
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
        assert!(matches!(
            hovered_pass.overlay,
            Some(ScheduledOverlay::Hover)
        ));
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
        let tile_rect = egui::Rect::from_min_max(egui::pos2(40.0, 40.0), egui::pos2(140.0, 100.0));
        let viewport_rect =
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(200.0, 200.0));

        assert!(!should_cull_tile_content(tile_rect, viewport_rect));
    }

    #[test]
    fn gpu_pressure_degradation_triggers_at_budget_boundary() {
        assert!(should_degrade_for_gpu_pressure(
            DEFAULT_COMPOSITED_CONTENT_BUDGET_BYTES_PER_FRAME,
            1,
            DEFAULT_COMPOSITED_CONTENT_BUDGET_BYTES_PER_FRAME,
        ));
        assert!(!should_degrade_for_gpu_pressure(
            DEFAULT_COMPOSITED_CONTENT_BUDGET_BYTES_PER_FRAME.saturating_sub(1024),
            1024,
            DEFAULT_COMPOSITED_CONTENT_BUDGET_BYTES_PER_FRAME,
        ));
    }

    #[test]
    fn estimated_tile_content_bytes_uses_rgba_pixel_footprint() {
        let rect = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(100.0, 50.0));
        assert_eq!(estimated_tile_content_bytes(rect, 2.0), 200 * 100 * 4);
    }
}
