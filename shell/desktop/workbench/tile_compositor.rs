/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Tile compositor frame assembly for the Surface Composition Contract.
//! GL callback state isolation is enforced by `CompositorAdapter` guardrails so
//! content callbacks cannot leak viewport/scissor/blend/texture/framebuffer
//! state across composition passes.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::{Hash, Hasher};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
#[cfg(feature = "diagnostics")]
use std::time::Instant;

use egui::{Color32, Stroke, TextureHandle, TextureId};
use egui_tiles::{Tile, TileId, Tree};
use graph_tree::GraphTree;
use image::load_from_memory;
use servo::OffscreenRenderingContext;

use crate::app::{GraphBrowserApp, VisibleNavigationRegionSet};
use crate::graph::{NodeKey, NodeLifecycle};
use crate::registries::atomic::lens::{GlyphOverlay, LensOverlayDescriptor};
use crate::registries::domain::presentation::PresentationProfile;
use crate::shell::desktop::render_backend::UiRenderBackendHandle;
use crate::shell::desktop::host::window::EmbedderWindow;
use crate::shell::desktop::runtime::diagnostics::{DiagnosticEvent, emit_event};
use crate::shell::desktop::runtime::registries::{
    CHANNEL_COMPOSITOR_CONTENT_CULLED_OFFVIEWPORT, CHANNEL_COMPOSITOR_DEGRADATION_GPU_PRESSURE,
    CHANNEL_COMPOSITOR_DEGRADATION_PLACEHOLDER_MODE,
    CHANNEL_COMPOSITOR_DIFFERENTIAL_CONTENT_COMPOSED,
    CHANNEL_COMPOSITOR_DIFFERENTIAL_FALLBACK_NO_PRIOR_SIGNATURE,
    CHANNEL_COMPOSITOR_DIFFERENTIAL_FALLBACK_SIGNATURE_CHANGED,
    CHANNEL_COMPOSITOR_DIFFERENTIAL_SKIP_RATE_SAMPLE, CHANNEL_COMPOSITOR_FOCUS_ACTIVATION_DEFERRED,
    CHANNEL_COMPOSITOR_LENS_OVERLAY_APPLIED, CHANNEL_COMPOSITOR_NATIVE_OVERLAY_RECT_MISMATCH,
    CHANNEL_COMPOSITOR_OVERLAY_BATCH_SIZE_SAMPLE, CHANNEL_COMPOSITOR_OVERLAY_LIFECYCLE_INDICATOR,
    CHANNEL_COMPOSITOR_OVERLAY_NATIVE_SUPPRESSED_HELP_PANEL,
    CHANNEL_COMPOSITOR_OVERLAY_NATIVE_SUPPRESSED_INTERACTION_MENU,
    CHANNEL_COMPOSITOR_OVERLAY_NATIVE_SUPPRESSED_RADIAL_MENU,
    CHANNEL_COMPOSITOR_OVERLAY_NATIVE_SUPPRESSED_TILE_DRAG, CHANNEL_COMPOSITOR_PAINT_NOT_CONFIRMED,
    CHANNEL_COMPOSITOR_RESOURCE_REUSE_CONTEXT_HIT, CHANNEL_COMPOSITOR_RESOURCE_REUSE_CONTEXT_MISS,
    CHANNEL_COMPOSITOR_TILE_ACTIVITY, phase3_resolve_active_presentation_profile,
};
use crate::shell::desktop::workbench::compositor_adapter::{
    CompositedContentPassOutcome, CompositorAdapter, CompositorPassTracker, OverlayAffordanceStyle,
    OverlayStrokePass,
};
use crate::shell::desktop::workbench::pane_model::{PaneId, TileRenderMode};
use crate::shell::desktop::workbench::{
    interaction_policy::{InteractionUiState, OverlaySuppressionReason},
    tile_kind::TileKind,
    tile_view_ops,
};
#[cfg(feature = "wry")]
use crate::{mods::native::verso, mods::native::verso::wry_manager::OverlayRect as WryOverlayRect};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TileSelectionState {
    NotSelected,
    Selected,
    SelectionPrimary,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FocusDelta {
    pub(crate) changed_this_frame: bool,
    pub(crate) new_focused_node: Option<NodeKey>,
    pub(crate) previous_focused_node: Option<NodeKey>,
}

impl FocusDelta {
    pub(crate) fn new(
        previous_focused_node: Option<NodeKey>,
        new_focused_node: Option<NodeKey>,
    ) -> Self {
        Self {
            changed_this_frame: previous_focused_node != new_focused_node,
            new_focused_node,
            previous_focused_node,
        }
    }

    fn touches(self, node_key: NodeKey) -> bool {
        self.new_focused_node == Some(node_key) || self.previous_focused_node == Some(node_key)
    }
}

#[derive(Clone, Debug, PartialEq)]
struct TileSemanticOverlayInput {
    node_key: NodeKey,
    viewer_id: String,
    render_mode: TileRenderMode,
    lifecycle: NodeLifecycle,
    runtime_blocked: bool,
    semantic_generation: u64,
    active_lens_overlay: Option<LensOverlayDescriptor>,
    focus_delta: Option<FocusDelta>,
    selection_state: TileSelectionState,
    has_unread_traversal_activity: bool,
}

#[derive(Clone)]
enum ScheduledOverlay {
    Focus(TileSemanticOverlayInput),
    Selection(TileSemanticOverlayInput),
    Hover(TileSemanticOverlayInput),
    Semantic(TileSemanticOverlayInput),
}

#[derive(Clone)]
struct ScheduledTileSemanticInput {
    pane_id: PaneId,
    tile_rect: egui::Rect,
    semantic: TileSemanticOverlayInput,
}

#[derive(Clone)]
struct ScheduledPanePass {
    pane_id: PaneId,
    tile_rect: egui::Rect,
    semantic: TileSemanticOverlayInput,
    overlays: Vec<ScheduledOverlay>,
}

#[derive(Clone)]
struct PreparedPanePass {
    pass: ScheduledPanePass,
    interaction_render_mode: TileRenderMode,
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

fn active_presentation_profile(app: &GraphBrowserApp) -> PresentationProfile {
    phase3_resolve_active_presentation_profile(app.default_registry_theme_id()).profile
}

// ---------------------------------------------------------------------------
// Phase C: 3-axis invalidation
// ---------------------------------------------------------------------------
//
// The composited content signature is split into three independent axes:
//
// | Axis      | What changes                              | Action                        |
// |-----------|-------------------------------------------|-------------------------------|
// | Content   | Servo produces a new frame                | Re-import wgpu::Texture       |
// | Placement | Tile rect changes (resize, layout)        | Update blit position only     |
// | Semantic  | `semantic_generation` changes              | Re-render overlay/affordance  |
//
// Placement-only changes don't need WebRender re-render — just reposition the blit.
// The monolithic `CompositedContentSignature` is preserved for backwards compat
// but `DifferentialObservation` now exposes per-axis change flags.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CompositedContentSignature {
    webview_id: servo::WebViewId,
    rect_px: [i32; 4],
    semantic_generation: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DifferentialComposeReason {
    NoPriorSignature,
    /// Content axis: webview identity or content generation changed.
    ContentChanged,
    /// Placement axis only: rect moved/resized but content is the same.
    PlacementOnly,
    /// Semantic axis: overlay/affordance generation changed.
    SemanticOnly,
    /// Multiple axes changed.
    SignatureChanged,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DifferentialContentDecision {
    Compose(DifferentialComposeReason),
    SkipUnchanged,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DifferentialDecisionKind {
    Recompose,
    Skip,
    GpuPressureDegraded,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DifferentialObservation {
    decision: DifferentialContentDecision,
    semantic_generation_changed: bool,
    /// Phase C: true when only the rect changed (no content re-import needed).
    placement_only: bool,
    /// Phase C: true when content (webview frame) changed.
    content_changed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CompositorFrameActivitySummary {
    pub(crate) active_tile_keys: Vec<NodeKey>,
    pub(crate) idle_tile_keys: Vec<NodeKey>,
    pub(crate) frame_index: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LifecycleTreatment {
    Active,
    Warm,
    Cold,
    Tombstone,
    RuntimeBlocked,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TileAffordanceAnnotation {
    pub(crate) node_key: NodeKey,
    pub(crate) focus_ring_rendered: bool,
    pub(crate) selection_ring_rendered: bool,
    pub(crate) lifecycle_treatment: LifecycleTreatment,
    pub(crate) lens_glyphs_rendered: Vec<String>,
    /// True when a composited paint callback was registered for this tile's rect
    /// in the current frame. False indicates the tile was laid out but its paint
    /// pass was skipped or failed — a potential rendering gap.
    pub(crate) paint_callback_registered: bool,
}

static COMPOSITED_CONTENT_SIGNATURES: OnceLock<
    Mutex<HashMap<NodeKey, CompositedContentSignature>>,
> = OnceLock::new();
static COMPOSITOR_ACTIVITY_FRAME_SEQUENCE: AtomicU64 = AtomicU64::new(1);
static COMPOSITOR_ACTIVITY_SUMMARIES: OnceLock<Mutex<VecDeque<CompositorFrameActivitySummary>>> =
    OnceLock::new();
static LATEST_AFFORDANCE_ANNOTATIONS: OnceLock<Mutex<Vec<TileAffordanceAnnotation>>> =
    OnceLock::new();
static LAST_SENT_NATIVE_OVERLAY_RECTS: OnceLock<Mutex<HashMap<NodeKey, egui::Rect>>> =
    OnceLock::new();

thread_local! {
    static THUMBNAIL_GHOST_TEXTURES: RefCell<HashMap<NodeKey, (u64, TextureHandle)>> =
        RefCell::new(HashMap::new());
}

fn composited_content_signatures() -> &'static Mutex<HashMap<NodeKey, CompositedContentSignature>> {
    COMPOSITED_CONTENT_SIGNATURES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn compositor_activity_summaries() -> &'static Mutex<VecDeque<CompositorFrameActivitySummary>> {
    COMPOSITOR_ACTIVITY_SUMMARIES.get_or_init(|| Mutex::new(VecDeque::with_capacity(256)))
}

fn latest_affordance_annotations() -> &'static Mutex<Vec<TileAffordanceAnnotation>> {
    LATEST_AFFORDANCE_ANNOTATIONS.get_or_init(|| Mutex::new(Vec::new()))
}

fn last_sent_native_overlay_rects() -> &'static Mutex<HashMap<NodeKey, egui::Rect>> {
    LAST_SENT_NATIVE_OVERLAY_RECTS.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(crate) fn latest_tile_affordance_annotations() -> Vec<TileAffordanceAnnotation> {
    latest_affordance_annotations()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
}

pub(crate) fn compositor_activity_summaries_snapshot() -> Vec<CompositorFrameActivitySummary> {
    compositor_activity_summaries()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .iter()
        .cloned()
        .collect()
}

fn content_signature_for_tile(
    webview_id: servo::WebViewId,
    tile_rect: egui::Rect,
    pixels_per_point: f32,
    semantic_generation: u64,
) -> CompositedContentSignature {
    let min_x = (tile_rect.min.x * pixels_per_point).round() as i32;
    let min_y = (tile_rect.min.y * pixels_per_point).round() as i32;
    let max_x = (tile_rect.max.x * pixels_per_point).round() as i32;
    let max_y = (tile_rect.max.y * pixels_per_point).round() as i32;
    CompositedContentSignature {
        webview_id,
        rect_px: [min_x, min_y, max_x, max_y],
        semantic_generation,
    }
}

fn node_lifecycle_for_tile(graph_app: &GraphBrowserApp, node_key: NodeKey) -> NodeLifecycle {
    graph_app
        .domain_graph()
        .get_node(node_key)
        .map(|node| node.lifecycle)
        .unwrap_or(NodeLifecycle::Cold)
}

fn tile_selection_state_for_tile(
    graph_app: &GraphBrowserApp,
    tile_id: Option<TileId>,
) -> TileSelectionState {
    let Some(tile_id) = tile_id else {
        return TileSelectionState::NotSelected;
    };

    if graph_app.workbench_tile_selection().primary_tile_id == Some(tile_id) {
        TileSelectionState::SelectionPrimary
    } else if graph_app
        .workbench_tile_selection()
        .selected_tile_ids
        .contains(&tile_id)
    {
        TileSelectionState::Selected
    } else {
        TileSelectionState::NotSelected
    }
}

fn tile_id_for_pane(tiles_tree: &Tree<TileKind>, pane_id: PaneId) -> Option<TileId> {
    tiles_tree
        .tiles
        .iter()
        .find_map(|(tile_id, tile)| match tile {
            Tile::Pane(TileKind::Node(state)) if state.pane_id == pane_id => Some(tile_id),
            _ => None,
        })
        .copied()
}

fn hash_render_mode(render_mode: TileRenderMode) -> u8 {
    match render_mode {
        TileRenderMode::CompositedTexture => 0,
        TileRenderMode::NativeOverlay => 1,
        TileRenderMode::EmbeddedEgui => 2,
        TileRenderMode::Placeholder => 3,
    }
}

fn hash_lifecycle(lifecycle: NodeLifecycle) -> u8 {
    match lifecycle {
        NodeLifecycle::Active => 0,
        NodeLifecycle::Warm => 1,
        NodeLifecycle::Cold => 2,
        NodeLifecycle::Tombstone => 3,
    }
}

fn hash_selection_state(selection_state: TileSelectionState) -> u8 {
    match selection_state {
        TileSelectionState::NotSelected => 0,
        TileSelectionState::Selected => 1,
        TileSelectionState::SelectionPrimary => 2,
    }
}

fn semantic_generation_for_tile(
    semantic: TileSemanticOverlayInput,
    active_lens_id: Option<&str>,
) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    (semantic.node_key.index() as u64).hash(&mut hasher);
    semantic.viewer_id.hash(&mut hasher);
    hash_render_mode(semantic.render_mode).hash(&mut hasher);
    hash_lifecycle(semantic.lifecycle).hash(&mut hasher);
    semantic.runtime_blocked.hash(&mut hasher);
    hash_selection_state(semantic.selection_state).hash(&mut hasher);
    semantic.has_unread_traversal_activity.hash(&mut hasher);
    active_lens_id.unwrap_or_default().hash(&mut hasher);
    if let Some(descriptor) = semantic.active_lens_overlay.as_ref() {
        descriptor.suppress_default_affordances.hash(&mut hasher);
        descriptor.opacity_scale.to_bits().hash(&mut hasher);
        descriptor.border_tint.hash(&mut hasher);
        for glyph in &descriptor.glyph_overlays {
            glyph.glyph_id.hash(&mut hasher);
            std::mem::discriminant(&glyph.anchor).hash(&mut hasher);
        }
    }
    if let Some(focus_delta) = semantic.focus_delta {
        focus_delta.changed_this_frame.hash(&mut hasher);
        focus_delta.new_focused_node.hash(&mut hasher);
        focus_delta.previous_focused_node.hash(&mut hasher);
    }
    hasher.finish()
}

fn resolved_lens_preset_for_tile(
    tiles_tree: &Tree<TileKind>,
    graph_app: &GraphBrowserApp,
    node_key: NodeKey,
) -> crate::app::ResolvedLensPreset {
    if let Some(view_id) = tile_view_ops::active_graph_view_id(tiles_tree)
        && let Some(view) = graph_app.workspace.graph_runtime.views.get(&view_id)
        && let Some(lens_id) = view.resolved_lens_id()
    {
        return crate::shell::desktop::runtime::registries::phase2_resolve_lens(lens_id);
    }
    crate::shell::desktop::runtime::registries::phase2_resolve_lens_for_node(graph_app, node_key)
}

fn resolve_tile_semantic_input(
    tiles_tree: &Tree<TileKind>,
    graph_app: &GraphBrowserApp,
    pane_id: PaneId,
    node_key: NodeKey,
    tile_rect: egui::Rect,
    focus_delta: FocusDelta,
) -> ScheduledTileSemanticInput {
    let render_mode = render_mode_for_pane(tiles_tree, pane_id);
    let viewer_id =
        crate::shell::desktop::workbench::tile_runtime::effective_viewer_id_for_pane_in_tree(
            tiles_tree, pane_id, graph_app,
        )
        .unwrap_or_else(|| "viewer:webview".to_string());
    let lifecycle = node_lifecycle_for_tile(graph_app, node_key);
    let runtime_blocked = graph_app.runtime_block_state_for_node(node_key).is_some();
    let has_unread_traversal_activity =
        graph_app.node_has_canonical_tag(node_key, GraphBrowserApp::TAG_UNREAD);
    let selection_state =
        tile_selection_state_for_tile(graph_app, tile_id_for_pane(tiles_tree, pane_id));
    let lens_preset = resolved_lens_preset_for_tile(tiles_tree, graph_app, node_key);
    let mut semantic = TileSemanticOverlayInput {
        node_key,
        viewer_id,
        render_mode,
        lifecycle,
        runtime_blocked,
        semantic_generation: 0,
        active_lens_overlay: lens_preset.overlay_descriptor,
        focus_delta: focus_delta.touches(node_key).then_some(focus_delta),
        selection_state,
        has_unread_traversal_activity,
    };
    semantic.semantic_generation =
        semantic_generation_for_tile(semantic.clone(), Some(lens_preset.lens_id.as_str()));
    ScheduledTileSemanticInput {
        pane_id,
        tile_rect,
        semantic,
    }
}

fn differential_content_decision(
    node_key: NodeKey,
    signature: CompositedContentSignature,
) -> DifferentialObservation {
    let mut cache = composited_content_signatures()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    match cache.insert(node_key, signature) {
        None => DifferentialObservation {
            decision: DifferentialContentDecision::Compose(
                DifferentialComposeReason::NoPriorSignature,
            ),
            semantic_generation_changed: false,
            placement_only: false,
            content_changed: true,
        },
        Some(previous) if previous == signature => DifferentialObservation {
            decision: DifferentialContentDecision::SkipUnchanged,
            semantic_generation_changed: false,
            placement_only: false,
            content_changed: false,
        },
        Some(previous) => {
            // Phase C: decompose the change into independent axes.
            let content_changed = previous.webview_id != signature.webview_id;
            let placement_changed = previous.rect_px != signature.rect_px;
            let semantic_changed =
                previous.semantic_generation != signature.semantic_generation;

            let reason = if content_changed {
                DifferentialComposeReason::ContentChanged
            } else if placement_changed && !semantic_changed {
                DifferentialComposeReason::PlacementOnly
            } else if semantic_changed && !placement_changed {
                DifferentialComposeReason::SemanticOnly
            } else {
                DifferentialComposeReason::SignatureChanged
            };

            DifferentialObservation {
                decision: DifferentialContentDecision::Compose(reason),
                semantic_generation_changed: semantic_changed,
                placement_only: placement_changed && !content_changed && !semantic_changed,
                content_changed,
            }
        }
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
        DifferentialComposeReason::ContentChanged
        | DifferentialComposeReason::PlacementOnly
        | DifferentialComposeReason::SemanticOnly
        | DifferentialComposeReason::SignatureChanged => {
            CHANNEL_COMPOSITOR_DIFFERENTIAL_FALLBACK_SIGNATURE_CHANGED
        }
    }
}

fn should_cull_tile_content(
    tile_rect: egui::Rect,
    viewport_regions: &VisibleNavigationRegionSet,
) -> bool {
    tile_rect.width() <= 0.0
        || tile_rect.height() <= 0.0
        || !viewport_regions.intersects_rect(tile_rect)
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
    // Wave 4: detect rect drift between successive sync calls for the same node.
    {
        const MISMATCH_THRESHOLD: f32 = 1.0;
        let mut last_rects = last_sent_native_overlay_rects()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(prev) = last_rects.get(&node_key) {
            let dx = (tile_rect.min.x - prev.min.x)
                .abs()
                .max((tile_rect.max.x - prev.max.x).abs());
            let dy = (tile_rect.min.y - prev.min.y)
                .abs()
                .max((tile_rect.max.y - prev.max.y).abs());
            if dx > MISMATCH_THRESHOLD || dy > MISMATCH_THRESHOLD {
                emit_event(DiagnosticEvent::MessageSent {
                    channel_id: CHANNEL_COMPOSITOR_NATIVE_OVERLAY_RECT_MISMATCH,
                    byte_len: 1,
                });
            }
        }
        last_rects.insert(node_key, tile_rect);
    }

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
        OverlaySuppressionReason::TileDrag => {
            CHANNEL_COMPOSITOR_OVERLAY_NATIVE_SUPPRESSED_TILE_DRAG
        }
    }
}

fn run_composited_texture_content_pass(
    ctx: &egui::Context,
    ui_render_backend: &mut UiRenderBackendHandle,
    window: &EmbedderWindow,
    graph_app: &GraphBrowserApp,
    presentation: &PresentationProfile,
    tile_rendering_contexts: &mut HashMap<NodeKey, Rc<OffscreenRenderingContext>>,
    pass_tracker: &mut CompositorPassTracker,
    pending_overlay_passes: &mut Vec<OverlayStrokePass>,
    degraded_receipts: &mut Vec<DegradedReceipt>,
    active_composited_nodes: &mut HashSet<NodeKey>,
    counters: &mut CompositedPassCounters,
    semantic: TileSemanticOverlayInput,
    tile_rect: egui::Rect,
    focus_ring_alpha: f32,
    overlays: &[ScheduledOverlay],
    frame_activity: &mut CompositorFrameActivitySummary,
) -> bool {
    let node_key = semantic.node_key;
    if semantic.viewer_id == "viewer:wry" {
        let rendered = render_wry_preview_frame_if_needed(ctx, graph_app, &semantic, tile_rect);
        if rendered {
            pass_tracker.record_content_pass(node_key);
            counters.composed += 1;
            counters.composed_estimated_bytes =
                counters
                    .composed_estimated_bytes
                    .saturating_add(estimated_tile_content_bytes(
                        tile_rect,
                        ctx.pixels_per_point(),
                    ));
            active_composited_nodes.insert(node_key);
            frame_activity.active_tile_keys.push(node_key);
        } else {
            counters.skipped += 1;
        }
        return rendered;
    }

    let Some(webview_id) = graph_app.get_webview_for_node(node_key) else {
        log::debug!(
            "composite: no runtime viewer mapped for node {:?}",
            node_key
        );
        return false;
    };
    active_composited_nodes.insert(node_key);

    let signature = content_signature_for_tile(
        webview_id,
        tile_rect,
        ctx.pixels_per_point(),
        semantic.semantic_generation,
    );
    let estimated_tile_bytes = estimated_tile_content_bytes(tile_rect, ctx.pixels_per_point());
    counters.evaluated += 1;
    let differential_decision = differential_content_decision(node_key, signature);

    if should_degrade_for_gpu_pressure(
        counters.composed_estimated_bytes,
        estimated_tile_bytes,
        DEFAULT_COMPOSITED_CONTENT_BUDGET_BYTES_PER_FRAME,
    ) {
        counters.skipped += 1;
        frame_activity.active_tile_keys.push(node_key);
        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_COMPOSITOR_TILE_ACTIVITY,
            byte_len: std::mem::size_of::<DifferentialDecisionKind>(),
        });
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
        for overlay in overlays.iter().cloned() {
            pending_overlay_passes.push(overlay_pass_for_schedule(
                overlay,
                tile_rect,
                focus_ring_alpha,
                presentation,
                Some(TileRenderMode::Placeholder),
            ));
        }
        return false;
    }

    emit_event(DiagnosticEvent::MessageSent {
        channel_id: CHANNEL_COMPOSITOR_DIFFERENTIAL_CONTENT_COMPOSED,
        byte_len: 1,
    });
    emit_event(DiagnosticEvent::MessageSent {
        channel_id: CHANNEL_COMPOSITOR_TILE_ACTIVITY,
        byte_len: std::mem::size_of::<DifferentialDecisionKind>(),
    });
    match differential_decision.decision {
        DifferentialContentDecision::Compose(_) => frame_activity.active_tile_keys.push(node_key),
        DifferentialContentDecision::SkipUnchanged => frame_activity.idle_tile_keys.push(node_key),
    }
    if let DifferentialContentDecision::Compose(reason) = differential_decision.decision {
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
        ui_render_backend,
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
    active_tile_inputs: Vec<ScheduledTileSemanticInput>,
    focused_node_key: Option<NodeKey>,
    focus_ring_alpha: f32,
    hovered_node_key: Option<NodeKey>,
) -> Vec<ScheduledPanePass> {
    let mut out = Vec::with_capacity(active_tile_inputs.len());
    for input in active_tile_inputs {
        let semantic = input.semantic;
        let mut overlays = Vec::new();
        if semantic.runtime_blocked
            || semantic.lifecycle != NodeLifecycle::Active
            || semantic.active_lens_overlay.is_some()
        {
            overlays.push(ScheduledOverlay::Semantic(semantic.clone()));
        }
        if semantic.selection_state != TileSelectionState::NotSelected {
            overlays.push(ScheduledOverlay::Selection(semantic.clone()));
        }
        if focused_node_key == Some(semantic.node_key) && focus_ring_alpha > 0.0 {
            overlays.push(ScheduledOverlay::Focus(semantic.clone()));
        } else if overlays.is_empty() && hovered_node_key == Some(semantic.node_key) {
            overlays.push(ScheduledOverlay::Hover(semantic.clone()));
        }
        out.push(ScheduledPanePass {
            pane_id: input.pane_id,
            tile_rect: input.tile_rect,
            semantic,
            overlays,
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

/// Full layout output from GraphTree for the compositor.
pub(crate) struct GraphTreeLayoutOutput {
    pub pane_rects: Vec<(PaneId, NodeKey, egui::Rect)>,
    pub split_boundaries: Vec<graph_tree::SplitBoundary<NodeKey>>,
    /// Tree rows for sidebar rendering (always populated).
    pub tree_rows: Vec<graph_tree::OwnedTreeRow<NodeKey>>,
    /// Tab ordering for flat tab bar rendering.
    pub tab_order: Vec<graph_tree::TabEntry<NodeKey>>,
    /// Currently active member.
    pub active: Option<NodeKey>,
    /// Raw graph-tree pane rects (before PaneId lookup), for pane chrome rendering.
    pub raw_pane_rects: std::collections::HashMap<NodeKey, graph_tree::Rect>,
}

/// Phase G: GraphTree-keyed compositor input with GraphTree layout authority.
///
/// GraphTree is both the membership authority (which nodes are visible) and the
/// layout authority (pane rects via taffy-backed `compute_layout()`).
/// PaneId lookup still comes from egui_tiles during migration — once PaneId is
/// replaced by a GraphTree-native identifier, the tiles_tree parameter can be removed.
pub(crate) fn active_node_pane_rects_from_graph_tree(
    graph_tree: &GraphTree<NodeKey>,
    tiles_tree: &Tree<TileKind>,
    available: egui::Rect,
) -> GraphTreeLayoutOutput {
    let gt_rect = graph_tree::Rect::new(
        available.left(), available.top(), available.width(), available.height(),
    );
    let layout = graph_tree.compute_layout(gt_rect);

    let pane_rects = layout.pane_rects.iter().filter_map(|(node_key, rect)| {
        // PaneId lookup from tiles_tree during migration.
        let pane_id = tiles_tree.tiles.iter().find_map(|(_, tile)| {
            if let Tile::Pane(TileKind::Node(state)) = tile {
                if state.node == *node_key {
                    return Some(state.pane_id);
                }
            }
            None
        })?;
        let egui_rect = egui::Rect::from_min_size(
            egui::pos2(rect.x, rect.y),
            egui::vec2(rect.w, rect.h),
        );
        Some((pane_id, *node_key, egui_rect))
    }).collect();

    GraphTreeLayoutOutput {
        pane_rects,
        split_boundaries: layout.split_boundaries,
        tree_rows: layout.tree_rows,
        tab_order: layout.tab_order,
        active: layout.active,
        raw_pane_rects: layout.pane_rects,
    }
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
    graph_app: &mut GraphBrowserApp,
    focused_node_hint: &mut Option<NodeKey>,
) {
    let (primary, fallback) = frame_activation_targets(tiles_tree, graph_app, *focused_node_hint);
    if let Some(node_key) = primary {
        *focused_node_hint = Some(node_key);
        if let Some(wv_id) = graph_app.get_webview_for_node(node_key) {
            graph_app.set_embedded_content_focus_webview(Some(wv_id));
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
            graph_app.set_embedded_content_focus_webview(Some(fallback_wv_id));
            window.retarget_input_to_webview(fallback_wv_id);
        }
    }
}

fn retained_node_keys_for_active_tile_rects(
    active_tile_rects: &[(PaneId, NodeKey, egui::Rect)],
) -> HashSet<NodeKey> {
    active_tile_rects
        .iter()
        .map(|(_, node_key, _)| *node_key)
        .collect()
}

pub(crate) fn composite_active_node_pane_webviews(
    ctx: &egui::Context,
    ui_render_backend: &mut UiRenderBackendHandle,
    tiles_tree: &Tree<TileKind>,
    window: &EmbedderWindow,
    graph_app: &GraphBrowserApp,
    tile_rendering_contexts: &mut HashMap<NodeKey, Rc<OffscreenRenderingContext>>,
    active_tile_rects: &[(PaneId, NodeKey, egui::Rect)],
    focused_node_key: Option<NodeKey>,
    focus_delta: FocusDelta,
    focus_ring_alpha: f32,
) {
    #[cfg(feature = "diagnostics")]
    let composite_started = Instant::now();
    log::debug!(
        "composite_active_node_pane_runtime_viewers: {} tiles",
        active_tile_rects.len()
    );
    let retained_node_keys = retained_node_keys_for_active_tile_rects(active_tile_rects);
    CompositorAdapter::retire_stale_content_resources(ui_render_backend, &retained_node_keys);
    let presentation = active_presentation_profile(graph_app);
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
    let semantic_inputs: Vec<_> = active_tile_rects
        .iter()
        .copied()
        .map(|(pane_id, node_key, tile_rect)| {
            resolve_tile_semantic_input(
                tiles_tree,
                graph_app,
                pane_id,
                node_key,
                tile_rect,
                focus_delta,
            )
        })
        .collect();
    let scheduled_passes = schedule_active_node_pane_passes(
        semantic_inputs,
        focused_node_key,
        focus_ring_alpha,
        hovered_node_key,
    );
    let tile_drag_active = ctx.dragged_id().is_some();
    let interaction_ui = InteractionUiState::new(
        graph_app.workspace.chrome_ui.show_command_palette,
        graph_app.workspace.chrome_ui.show_help_panel,
        graph_app.workspace.chrome_ui.show_radial_menu,
    )
    .with_tile_drag_active(tile_drag_active);
    let mut active_composited_nodes = HashSet::new();
    let mut composited_counters = CompositedPassCounters::default();
    let viewport_regions = graph_app
        .workspace
        .graph_runtime
        .workbench_navigation_geometry
        .as_ref()
        .map(|geometry| geometry.visible_region_set_or_content())
        .unwrap_or_else(|| VisibleNavigationRegionSet::singleton(ctx.viewport_rect()));
    let frame_index = COMPOSITOR_ACTIVITY_FRAME_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let mut frame_activity = CompositorFrameActivitySummary {
        active_tile_keys: Vec::new(),
        idle_tile_keys: Vec::new(),
        frame_index,
    };
    let mut affordance_annotations = Vec::with_capacity(scheduled_passes.len());
    let mut prepared_passes = Vec::with_capacity(scheduled_passes.len());

    #[cfg(feature = "diagnostics")]
    let pass1_started = Instant::now();

    // Pass 1: prepare/sync stage (viewport culling, native overlay sync, thumbnail ghost).
    for pass in scheduled_passes {
        let semantic = pass.semantic.clone();
        let node_key = semantic.node_key;
        let tile_rect = pass.tile_rect;
        let render_mode = semantic.render_mode;
        let interaction_render_mode = interaction_ui.effective_interaction_render_mode(render_mode);

        if should_cull_tile_content(tile_rect, &viewport_regions) {
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

        let ghost_rendered = render_thumbnail_ghost_if_needed(ctx, graph_app, &semantic, tile_rect);
        if ghost_rendered {
            frame_activity.active_tile_keys.push(node_key);
        }

        prepared_passes.push(PreparedPanePass {
            pass,
            interaction_render_mode,
        });
    }

    #[cfg(feature = "diagnostics")]
    crate::shell::desktop::runtime::diagnostics::emit_span_duration(
        "tile_compositor::pass1_prepare",
        pass1_started.elapsed().as_micros() as u64,
    );

    #[cfg(feature = "diagnostics")]
    let pass2_started = Instant::now();

    // Pass 2: content stage (composited content callback registration and activity accounting).
    for prepared in prepared_passes {
        let pass = prepared.pass;
        let semantic = pass.semantic;
        let tile_rect = pass.tile_rect;
        let render_mode = semantic.render_mode;

        let paint_callback_registered = if render_mode == TileRenderMode::CompositedTexture
            && semantic.lifecycle != NodeLifecycle::Cold
            && semantic.lifecycle != NodeLifecycle::Tombstone
        {
            run_composited_texture_content_pass(
                ctx,
                ui_render_backend,
                window,
                graph_app,
                &presentation,
                tile_rendering_contexts,
                &mut pass_tracker,
                &mut pending_overlay_passes,
                &mut degraded_receipts,
                &mut active_composited_nodes,
                &mut composited_counters,
                semantic.clone(),
                tile_rect,
                focus_ring_alpha,
                &pass.overlays,
                &mut frame_activity,
            )
        } else {
            // NativeOverlay tiles and Cold/Tombstone tiles are handled outside
            // the composited paint path — not a paint failure.
            true
        };

        affordance_annotations.push(tile_affordance_annotation(
            &semantic,
            &pass.overlays,
            paint_callback_registered,
        ));

        if paint_callback_registered {
            for overlay in pass.overlays {
                pending_overlay_passes.push(overlay_pass_for_schedule(
                    overlay,
                    tile_rect,
                    focus_ring_alpha,
                    &presentation,
                    Some(prepared.interaction_render_mode),
                ));
            }
        }
    }

    #[cfg(feature = "diagnostics")]
    crate::shell::desktop::runtime::diagnostics::emit_span_duration(
        "tile_compositor::pass2_content",
        pass2_started.elapsed().as_micros() as u64,
    );

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
    if !frame_activity.active_tile_keys.is_empty() || !frame_activity.idle_tile_keys.is_empty() {
        let mut summaries = compositor_activity_summaries()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        summaries.push_back(frame_activity);
        while summaries.len() > 256 {
            summaries.pop_front();
        }
    }
    let paint_not_confirmed_count = affordance_annotations
        .iter()
        .filter(|a| !a.paint_callback_registered)
        .count();
    if paint_not_confirmed_count > 0 {
        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_COMPOSITOR_PAINT_NOT_CONFIRMED,
            byte_len: paint_not_confirmed_count,
        });
    }
    *latest_affordance_annotations()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = affordance_annotations;

    #[cfg(feature = "diagnostics")]
    let pass3_started = Instant::now();

    // Pass 3: overlay affordance stage (focus/selection/hover rings over post-content surface).
    CompositorAdapter::execute_overlay_affordance_pass(ctx, &pass_tracker, pending_overlay_passes);

    #[cfg(feature = "diagnostics")]
    crate::shell::desktop::runtime::diagnostics::emit_span_duration(
        "tile_compositor::pass3_overlay",
        pass3_started.elapsed().as_micros() as u64,
    );

    render_degraded_receipts(ctx, &degraded_receipts, &presentation);

    #[cfg(feature = "diagnostics")]
    crate::shell::desktop::runtime::diagnostics::emit_span_duration(
        "tile_compositor::composite_active_node_pane_webviews",
        composite_started.elapsed().as_micros() as u64,
    );
}

fn render_degraded_receipts(
    ctx: &egui::Context,
    receipts: &[DegradedReceipt],
    presentation: &PresentationProfile,
) {
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
            presentation.degraded_receipt_background.to_color32(),
        );
        painter.text(
            box_rect.left_center() + egui::vec2(8.0, 0.0),
            egui::Align2::LEFT_CENTER,
            &receipt.message,
            font.clone(),
            presentation.degraded_receipt_text.to_color32(),
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
    tiles_tree
        .tiles
        .iter()
        .find_map(|(_, tile)| match tile {
            Tile::Pane(kind @ TileKind::Node(state)) if state.pane_id == pane_id => {
                kind.node_render_mode()
            }
            _ => None,
        })
        .unwrap_or(TileRenderMode::Placeholder)
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
            style: OverlayAffordanceStyle::EguiAreaStroke,
            rounding: 4.0,
        },
    }
}

fn lifecycle_stroke_alpha(lifecycle: NodeLifecycle) -> f32 {
    match lifecycle {
        NodeLifecycle::Active => 1.0,
        NodeLifecycle::Warm => 0.7,
        NodeLifecycle::Cold => 0.4,
        NodeLifecycle::Tombstone => 0.25,
    }
}

fn lifecycle_base_color(
    presentation: &PresentationProfile,
    lifecycle: NodeLifecycle,
) -> egui::Color32 {
    match lifecycle {
        NodeLifecycle::Active => presentation.lifecycle_active.to_color32(),
        NodeLifecycle::Warm => presentation.lifecycle_warm.to_color32(),
        NodeLifecycle::Cold => presentation.lifecycle_cold.to_color32(),
        NodeLifecycle::Tombstone => presentation.lifecycle_tombstone.to_color32(),
    }
}

fn overlay_color_for_input(
    semantic: &TileSemanticOverlayInput,
    presentation: &PresentationProfile,
    fallback_color: egui::Color32,
) -> egui::Color32 {
    if semantic.runtime_blocked {
        presentation.crash_blocked.to_color32()
    } else if semantic.selection_state == TileSelectionState::SelectionPrimary {
        presentation.selection_primary.to_color32()
    } else if semantic.selection_state == TileSelectionState::Selected {
        presentation.selection_primary.to_color32()
    } else if semantic.lifecycle == NodeLifecycle::Active {
        fallback_color
    } else {
        lifecycle_base_color(presentation, semantic.lifecycle)
    }
}

fn overlay_stroke_width(kind: &ScheduledOverlay, semantic: &TileSemanticOverlayInput) -> f32 {
    match kind {
        ScheduledOverlay::Focus(_) => 2.0,
        ScheduledOverlay::Selection(_) => match semantic.selection_state {
            TileSelectionState::SelectionPrimary => 2.5,
            TileSelectionState::Selected => 1.75,
            TileSelectionState::NotSelected => 1.5,
        },
        ScheduledOverlay::Hover(_) => 1.5,
        ScheduledOverlay::Semantic(_) if semantic.runtime_blocked => 2.0,
        ScheduledOverlay::Semantic(_) => match semantic.lifecycle {
            NodeLifecycle::Active => 1.5,
            NodeLifecycle::Warm => 1.5,
            NodeLifecycle::Cold => 1.25,
            NodeLifecycle::Tombstone => 1.0,
        },
    }
}

fn apply_lens_overlay_tint(
    base_color: Color32,
    lens_overlay: Option<&LensOverlayDescriptor>,
    semantic: &TileSemanticOverlayInput,
) -> Color32 {
    if semantic.runtime_blocked {
        return base_color;
    }
    lens_overlay
        .and_then(|descriptor| descriptor.border_tint)
        .map(|tint| {
            Color32::from_rgba_unmultiplied(
                (((base_color.r() as u16) + (tint.r() as u16)) / 2) as u8,
                (((base_color.g() as u16) + (tint.g() as u16)) / 2) as u8,
                (((base_color.b() as u16) + (tint.b() as u16)) / 2) as u8,
                base_color.a(),
            )
        })
        .unwrap_or(base_color)
}

fn overlay_glyphs_for_input(semantic: &TileSemanticOverlayInput) -> Vec<GlyphOverlay> {
    semantic
        .active_lens_overlay
        .as_ref()
        .map(|descriptor| descriptor.glyph_overlays.clone())
        .unwrap_or_default()
}

fn lifecycle_treatment_for_input(semantic: &TileSemanticOverlayInput) -> LifecycleTreatment {
    if semantic.runtime_blocked {
        LifecycleTreatment::RuntimeBlocked
    } else {
        match semantic.lifecycle {
            NodeLifecycle::Active => LifecycleTreatment::Active,
            NodeLifecycle::Warm => LifecycleTreatment::Warm,
            NodeLifecycle::Cold => LifecycleTreatment::Cold,
            NodeLifecycle::Tombstone => LifecycleTreatment::Tombstone,
        }
    }
}

fn tile_affordance_annotation(
    semantic: &TileSemanticOverlayInput,
    overlays: &[ScheduledOverlay],
    paint_callback_registered: bool,
) -> TileAffordanceAnnotation {
    TileAffordanceAnnotation {
        node_key: semantic.node_key,
        focus_ring_rendered: overlays
            .iter()
            .any(|overlay| matches!(overlay, ScheduledOverlay::Focus(_))),
        selection_ring_rendered: overlays
            .iter()
            .any(|overlay| matches!(overlay, ScheduledOverlay::Selection(_))),
        lifecycle_treatment: lifecycle_treatment_for_input(semantic),
        lens_glyphs_rendered: overlay_glyphs_for_input(semantic)
            .into_iter()
            .map(|glyph| glyph.glyph_id)
            .collect(),
        paint_callback_registered,
    }
}

fn thumbnail_ghost_hash(width: u32, height: u32, bytes: &[u8]) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    width.hash(&mut hasher);
    height.hash(&mut hasher);
    bytes.hash(&mut hasher);
    hasher.finish()
}

#[cfg(feature = "wry")]
fn wry_preview_texture_id(ctx: &egui::Context, node_key: NodeKey) -> Option<TextureId> {
    let png_bytes = verso::wry_frame_png_bytes_for_node(node_key)?;
    let image = load_from_memory(&png_bytes).ok()?.to_rgba8();
    let size = [image.width() as usize, image.height() as usize];
    let rgba = image.into_raw();
    let hash = thumbnail_ghost_hash(size[0] as u32, size[1] as u32, &png_bytes);
    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, &rgba);

    THUMBNAIL_GHOST_TEXTURES.with(|cache| {
        let mut cache = cache.borrow_mut();
        let handle = if let Some((cached_hash, handle)) = cache.get(&node_key) {
            if *cached_hash == hash {
                handle.clone()
            } else {
                let handle = ctx.load_texture(
                    format!("tile-wry-preview-{node_key:?}-{hash}"),
                    color_image,
                    Default::default(),
                );
                cache.insert(node_key, (hash, handle.clone()));
                handle
            }
        } else {
            let handle = ctx.load_texture(
                format!("tile-wry-preview-{node_key:?}-{hash}"),
                color_image,
                Default::default(),
            );
            cache.insert(node_key, (hash, handle.clone()));
            handle
        };
        Some(handle.id())
    })
}

fn webview_preview_refresh_interval_for_lifecycle(
    graph_app: &GraphBrowserApp,
    lifecycle: NodeLifecycle,
) -> Option<std::time::Duration> {
    match lifecycle {
        NodeLifecycle::Active => Some(std::time::Duration::from_secs(
            graph_app.webview_preview_active_refresh_secs(),
        )),
        NodeLifecycle::Warm => Some(std::time::Duration::from_secs(
            graph_app.webview_preview_warm_refresh_secs(),
        )),
        NodeLifecycle::Cold | NodeLifecycle::Tombstone => None,
    }
}

#[cfg(not(feature = "wry"))]
fn wry_preview_texture_id(_ctx: &egui::Context, _node_key: NodeKey) -> Option<TextureId> {
    None
}

#[cfg(feature = "wry")]
fn render_wry_preview_frame_if_needed(
    ctx: &egui::Context,
    graph_app: &GraphBrowserApp,
    semantic: &TileSemanticOverlayInput,
    tile_rect: egui::Rect,
) -> bool {
    if semantic.viewer_id != "viewer:wry"
        || semantic.render_mode != TileRenderMode::CompositedTexture
    {
        return false;
    }

    let texture_id = if let Some(texture_id) = wry_preview_texture_id(ctx, semantic.node_key) {
        if let Some(refresh_interval) =
            webview_preview_refresh_interval_for_lifecycle(graph_app, semantic.lifecycle)
        {
            let _ = verso::refresh_wry_frame_for_node_if_stale(semantic.node_key, refresh_interval);
        }
        Some(texture_id)
    } else {
        if webview_preview_refresh_interval_for_lifecycle(graph_app, semantic.lifecycle).is_some() {
            verso::refresh_wry_frame_for_node(semantic.node_key);
        }
        wry_preview_texture_id(ctx, semantic.node_key)
    };
    let Some(texture_id) = texture_id else {
        return false;
    };

    ctx.layer_painter(CompositorAdapter::content_layer(semantic.node_key))
        .image(
            texture_id,
            tile_rect,
            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
            Color32::WHITE,
        );
    true
}

#[cfg(not(feature = "wry"))]
fn render_wry_preview_frame_if_needed(
    _ctx: &egui::Context,
    _graph_app: &GraphBrowserApp,
    _semantic: &TileSemanticOverlayInput,
    _tile_rect: egui::Rect,
) -> bool {
    false
}

#[cfg(feature = "wry")]
fn frozen_webview_snapshot_texture_id(
    ctx: &egui::Context,
    semantic: &TileSemanticOverlayInput,
) -> Option<TextureId> {
    if semantic.viewer_id == "viewer:wry" {
        wry_preview_texture_id(ctx, semantic.node_key)
    } else {
        None
    }
}

#[cfg(not(feature = "wry"))]
fn frozen_webview_snapshot_texture_id(
    _ctx: &egui::Context,
    _semantic: &TileSemanticOverlayInput,
) -> Option<TextureId> {
    None
}

fn thumbnail_ghost_texture_id(
    ctx: &egui::Context,
    graph_app: &GraphBrowserApp,
    node_key: NodeKey,
) -> Option<TextureId> {
    let node = graph_app.domain_graph().get_node(node_key)?;
    let thumbnail_png = node.thumbnail_png.as_ref()?;
    if node.thumbnail_width == 0 || node.thumbnail_height == 0 {
        return None;
    }

    let image = load_from_memory(thumbnail_png).ok()?.to_rgba8();
    let size = [image.width() as usize, image.height() as usize];
    let rgba = image.into_raw();
    let hash = thumbnail_ghost_hash(node.thumbnail_width, node.thumbnail_height, thumbnail_png);
    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, &rgba);

    THUMBNAIL_GHOST_TEXTURES.with(|cache| {
        let mut cache = cache.borrow_mut();
        let handle = if let Some((cached_hash, handle)) = cache.get(&node_key) {
            if *cached_hash == hash {
                handle.clone()
            } else {
                let handle = ctx.load_texture(
                    format!("tile-thumbnail-ghost-{node_key:?}-{hash}"),
                    color_image,
                    Default::default(),
                );
                cache.insert(node_key, (hash, handle.clone()));
                handle
            }
        } else {
            let handle = ctx.load_texture(
                format!("tile-thumbnail-ghost-{node_key:?}-{hash}"),
                color_image,
                Default::default(),
            );
            cache.insert(node_key, (hash, handle.clone()));
            handle
        };
        Some(handle.id())
    })
}

fn render_thumbnail_ghost_if_needed(
    ctx: &egui::Context,
    graph_app: &GraphBrowserApp,
    semantic: &TileSemanticOverlayInput,
    tile_rect: egui::Rect,
) -> bool {
    if !matches!(
        semantic.lifecycle,
        NodeLifecycle::Cold | NodeLifecycle::Tombstone
    ) {
        return false;
    }
    if !matches!(
        semantic.render_mode,
        TileRenderMode::CompositedTexture | TileRenderMode::Placeholder
    ) {
        return false;
    }
    let texture_id = frozen_webview_snapshot_texture_id(ctx, semantic)
        .or_else(|| thumbnail_ghost_texture_id(ctx, graph_app, semantic.node_key));
    let Some(texture_id) = texture_id else {
        return false;
    };

    let tint = match semantic.lifecycle {
        NodeLifecycle::Cold => Color32::from_white_alpha(64),
        NodeLifecycle::Tombstone => Color32::from_white_alpha(40),
        _ => Color32::WHITE,
    };
    ctx.layer_painter(CompositorAdapter::content_layer(semantic.node_key))
        .image(
            texture_id,
            tile_rect.shrink2(egui::vec2(3.0, 3.0)),
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            tint,
        );
    true
}

fn overlay_pass_for_schedule(
    overlay: ScheduledOverlay,
    tile_rect: egui::Rect,
    focus_ring_alpha: f32,
    presentation: &PresentationProfile,
    render_mode_override: Option<TileRenderMode>,
) -> OverlayStrokePass {
    match overlay {
        ScheduledOverlay::Focus(mut semantic) => {
            if let Some(render_mode) = render_mode_override {
                semantic.render_mode = render_mode;
            }
            focus_overlay_for_mode(semantic, tile_rect, focus_ring_alpha, presentation)
        }
        ScheduledOverlay::Selection(mut semantic) => {
            if let Some(render_mode) = render_mode_override {
                semantic.render_mode = render_mode;
            }
            selection_overlay_for_mode(semantic, tile_rect, presentation)
        }
        ScheduledOverlay::Hover(mut semantic) => {
            if let Some(render_mode) = render_mode_override {
                semantic.render_mode = render_mode;
            }
            hover_overlay_for_mode(semantic, tile_rect, presentation)
        }
        ScheduledOverlay::Semantic(mut semantic) => {
            if let Some(render_mode) = render_mode_override {
                semantic.render_mode = render_mode;
            }
            semantic_overlay_for_mode(semantic, tile_rect, presentation)
        }
    }
}

fn focus_overlay_for_mode(
    semantic: TileSemanticOverlayInput,
    tile_rect: egui::Rect,
    focus_ring_alpha: f32,
    presentation: &PresentationProfile,
) -> OverlayStrokePass {
    let render_mode = semantic.render_mode;
    let alpha = (focus_ring_alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
    let policy = overlay_affordance_policy_for_render_mode(render_mode);
    let base_color = presentation.focus_ring.with_alpha(alpha);
    let overlay_color = apply_lens_overlay_tint(
        overlay_color_for_input(&semantic, presentation, base_color),
        semantic.active_lens_overlay.as_ref(),
        &semantic,
    );
    let overlay_alpha = ((alpha as f32) * lifecycle_stroke_alpha(semantic.lifecycle))
        .round()
        .clamp(0.0, 255.0) as u8;
    let stroke = Stroke::new(
        overlay_stroke_width(&ScheduledOverlay::Focus(semantic.clone()), &semantic),
        overlay_color.gamma_multiply((overlay_alpha as f32 / 255.0).clamp(0.0, 1.0)),
    );

    OverlayStrokePass {
        node_key: semantic.node_key,
        tile_rect,
        rounding: policy.rounding,
        stroke,
        glyph_overlays: Vec::new(),
        style: policy.style,
        render_mode,
    }
}

fn selection_overlay_for_mode(
    semantic: TileSemanticOverlayInput,
    tile_rect: egui::Rect,
    presentation: &PresentationProfile,
) -> OverlayStrokePass {
    let policy = overlay_affordance_policy_for_render_mode(semantic.render_mode);
    let opacity = match semantic.selection_state {
        TileSelectionState::SelectionPrimary => 1.0,
        TileSelectionState::Selected => 0.72,
        TileSelectionState::NotSelected => 0.0,
    };
    let stroke = Stroke::new(
        overlay_stroke_width(&ScheduledOverlay::Selection(semantic.clone()), &semantic),
        presentation
            .selection_primary
            .to_color32()
            .gamma_multiply(opacity),
    );

    OverlayStrokePass {
        node_key: semantic.node_key,
        tile_rect: tile_rect.shrink(3.0),
        rounding: policy.rounding,
        stroke,
        glyph_overlays: Vec::new(),
        style: policy.style,
        render_mode: semantic.render_mode,
    }
}

fn hover_overlay_for_mode(
    semantic: TileSemanticOverlayInput,
    tile_rect: egui::Rect,
    presentation: &PresentationProfile,
) -> OverlayStrokePass {
    let render_mode = semantic.render_mode;
    let policy = overlay_affordance_policy_for_render_mode(render_mode);
    let base_color = presentation.hover_ring.to_color32();
    let overlay_color = apply_lens_overlay_tint(
        overlay_color_for_input(&semantic, presentation, base_color),
        semantic.active_lens_overlay.as_ref(),
        &semantic,
    );
    let stroke = Stroke::new(
        overlay_stroke_width(&ScheduledOverlay::Hover(semantic.clone()), &semantic),
        overlay_color.gamma_multiply(lifecycle_stroke_alpha(semantic.lifecycle)),
    );

    OverlayStrokePass {
        node_key: semantic.node_key,
        tile_rect,
        rounding: policy.rounding,
        stroke,
        glyph_overlays: Vec::new(),
        style: policy.style,
        render_mode,
    }
}

fn semantic_overlay_for_mode(
    semantic: TileSemanticOverlayInput,
    tile_rect: egui::Rect,
    presentation: &PresentationProfile,
) -> OverlayStrokePass {
    let policy = overlay_affordance_policy_for_render_mode(semantic.render_mode);
    let default_base_color = lifecycle_base_color(presentation, semantic.lifecycle);
    let suppress_default = semantic
        .active_lens_overlay
        .as_ref()
        .map(|descriptor| descriptor.suppress_default_affordances)
        .unwrap_or(false)
        && !semantic.runtime_blocked;
    let base_color = if suppress_default {
        Color32::TRANSPARENT
    } else {
        default_base_color
    };
    let overlay_color = apply_lens_overlay_tint(
        overlay_color_for_input(&semantic, presentation, base_color),
        semantic.active_lens_overlay.as_ref(),
        &semantic,
    );
    let lens_opacity_scale = semantic
        .active_lens_overlay
        .as_ref()
        .map(|descriptor| descriptor.opacity_scale)
        .unwrap_or(1.0)
        .clamp(0.0, 2.0);
    let opacity = if semantic.selection_state == TileSelectionState::SelectionPrimary {
        lifecycle_stroke_alpha(semantic.lifecycle).max(0.85)
    } else if semantic.selection_state == TileSelectionState::Selected {
        lifecycle_stroke_alpha(semantic.lifecycle).max(0.6)
    } else {
        lifecycle_stroke_alpha(semantic.lifecycle)
    } * lens_opacity_scale;
    let stroke = Stroke::new(
        overlay_stroke_width(&ScheduledOverlay::Semantic(semantic.clone()), &semantic),
        overlay_color.gamma_multiply(opacity),
    );
    let glyph_overlays = overlay_glyphs_for_input(&semantic);
    if semantic.lifecycle != NodeLifecycle::Active || semantic.runtime_blocked {
        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_COMPOSITOR_OVERLAY_LIFECYCLE_INDICATOR,
            byte_len: 1,
        });
    }
    if !glyph_overlays.is_empty() || semantic.active_lens_overlay.is_some() {
        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_COMPOSITOR_LENS_OVERLAY_APPLIED,
            byte_len: glyph_overlays.len().max(1),
        });
    }
    let style = if semantic.lifecycle == NodeLifecycle::Tombstone
        && semantic.render_mode != TileRenderMode::NativeOverlay
    {
        OverlayAffordanceStyle::DashedRectStroke
    } else {
        policy.style
    };

    OverlayStrokePass {
        node_key: semantic.node_key,
        tile_rect,
        rounding: policy.rounding,
        stroke,
        glyph_overlays,
        style,
        render_mode: semantic.render_mode,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base::id::{PIPELINE_NAMESPACE, PainterId, PipelineNamespace, TEST_NAMESPACE};
    use egui_tiles::Tiles;
    use euclid::default::Point2D;
    use std::panic::AssertUnwindSafe;
    use std::sync::Arc;
    use std::sync::atomic::AtomicU64;

    #[cfg(feature = "wry")]
    use crate::mods::native::verso;
    use crate::shell::desktop::runtime::diagnostics::DiagnosticsState;
    use crate::shell::desktop::runtime::registries::CHANNEL_COMPOSITOR_FOCUS_ACTIVATION_DEFERRED;
    use crate::shell::desktop::workbench::pane_model::GraphPaneRef;

    fn test_presentation_profile() -> PresentationProfile {
        crate::registries::domain::presentation::PresentationDomainRegistry::default()
            .resolve_profile("physics:default", "theme:default")
            .profile
    }

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

    fn test_semantic_input(
        node_key: NodeKey,
        render_mode: TileRenderMode,
    ) -> TileSemanticOverlayInput {
        TileSemanticOverlayInput {
            node_key,
            viewer_id: "viewer:webview".to_string(),
            render_mode,
            lifecycle: NodeLifecycle::Active,
            runtime_blocked: false,
            semantic_generation: 0,
            active_lens_overlay: None,
            focus_delta: None,
            selection_state: TileSelectionState::NotSelected,
            has_unread_traversal_activity: false,
        }
    }

    fn scheduled_tile_input(
        pane_id: PaneId,
        node_key: NodeKey,
        tile_rect: egui::Rect,
        render_mode: TileRenderMode,
    ) -> ScheduledTileSemanticInput {
        ScheduledTileSemanticInput {
            pane_id,
            tile_rect,
            semantic: test_semantic_input(node_key, render_mode),
        }
    }

    #[test]
    fn retained_node_keys_for_active_tile_rects_deduplicates_nodes() {
        let a = NodeKey::new(200);
        let b = NodeKey::new(201);
        let tree = tree_with_two_active_nodes(a, b);
        let a_pane = pane_id_for_node(&tree, a);
        let b_pane = pane_id_for_node(&tree, b);
        let active_tile_rects = vec![
            (
                a_pane,
                a,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(10.0, 10.0)),
            ),
            (
                PaneId::default(),
                a,
                egui::Rect::from_min_max(egui::pos2(10.0, 0.0), egui::pos2(20.0, 10.0)),
            ),
            (
                b_pane,
                b,
                egui::Rect::from_min_max(egui::pos2(20.0, 0.0), egui::pos2(30.0, 10.0)),
            ),
        ];

        let retained = retained_node_keys_for_active_tile_rects(&active_tile_rects);

        assert_eq!(retained, HashSet::from([a, b]));
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
            activate_focused_node_for_frame(&window, &tree, &mut app, &mut focused_hint)
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
    fn activate_focused_node_for_frame_updates_embedded_focus_authority() {
        let mut app = GraphBrowserApp::new_for_testing();
        let node = NodeKey::new(12);
        let webview_id = test_webview_id();
        app.map_webview_to_node(webview_id, node);

        let prefs = crate::prefs::AppPreferences::default();
        let window = crate::shell::desktop::host::window::EmbedderWindow::new(
            crate::shell::desktop::host::headless_window::HeadlessWindow::new(&prefs),
            Arc::new(AtomicU64::new(0)),
        );
        let tree = tree_with_two_active_nodes(node, NodeKey::new(13));
        let mut focused_hint = Some(node);

        let _ = std::panic::catch_unwind(AssertUnwindSafe(|| {
            activate_focused_node_for_frame(&window, &tree, &mut app, &mut focused_hint)
        }));

        assert_eq!(app.embedded_content_focus_webview(), Some(webview_id));
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
        assert_eq!(
            suppression_reason_channel(OverlaySuppressionReason::TileDrag),
            CHANNEL_COMPOSITOR_OVERLAY_NATIVE_SUPPRESSED_TILE_DRAG
        );
    }

    #[test]
    fn suppressed_native_overlay_uses_placeholder_affordance_policy() {
        let ui_state = InteractionUiState::new(true, false, false);
        let effective_mode =
            ui_state.effective_interaction_render_mode(TileRenderMode::NativeOverlay);
        let overlay = focus_overlay_for_mode(
            test_semantic_input(NodeKey::new(702), effective_mode),
            egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(100.0, 100.0)),
            1.0,
            &test_presentation_profile(),
        );

        assert_eq!(effective_mode, TileRenderMode::Placeholder);
        assert!(matches!(
            overlay.style,
            OverlayAffordanceStyle::EguiAreaStroke
        ));
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
            vec![
                scheduled_tile_input(
                    focused_pane,
                    focused,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(100.0, 60.0)),
                    TileRenderMode::CompositedTexture,
                ),
                scheduled_tile_input(
                    other_pane,
                    other,
                    egui::Rect::from_min_max(egui::pos2(120.0, 0.0), egui::pos2(220.0, 60.0)),
                    TileRenderMode::NativeOverlay,
                ),
            ],
            Some(focused),
            1.0,
            None,
        );

        let focused_pass = passes
            .iter()
            .find(|pass| pass.semantic.node_key == focused)
            .expect("focused node pass should be scheduled");
        assert_eq!(
            focused_pass.semantic.render_mode,
            TileRenderMode::CompositedTexture
        );
        assert!(matches!(
            focused_pass.overlays.last(),
            Some(ScheduledOverlay::Focus(_))
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
            vec![
                scheduled_tile_input(
                    focused_pane,
                    focused,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(100.0, 60.0)),
                    TileRenderMode::CompositedTexture,
                ),
                scheduled_tile_input(
                    hovered_pane,
                    hovered,
                    egui::Rect::from_min_max(egui::pos2(120.0, 0.0), egui::pos2(220.0, 60.0)),
                    TileRenderMode::NativeOverlay,
                ),
            ],
            Some(focused),
            1.0,
            Some(hovered),
        );

        let hovered_pass = passes
            .iter()
            .find(|pass| pass.semantic.node_key == hovered)
            .expect("hovered node pass should be scheduled");
        assert_eq!(
            hovered_pass.semantic.render_mode,
            TileRenderMode::NativeOverlay
        );
        assert!(matches!(
            hovered_pass.overlays.last(),
            Some(ScheduledOverlay::Hover(_))
        ));
    }

    #[test]
    fn focus_overlay_for_native_overlay_uses_chrome_only_style() {
        let node = NodeKey::new(40);
        let tile_rect = egui::Rect::from_min_max(egui::pos2(10.0, 10.0), egui::pos2(110.0, 70.0));
        let overlay = focus_overlay_for_mode(
            test_semantic_input(node, TileRenderMode::NativeOverlay),
            tile_rect,
            1.0,
            &test_presentation_profile(),
        );

        assert!(matches!(overlay.style, OverlayAffordanceStyle::ChromeOnly));
        assert_eq!(overlay.render_mode, TileRenderMode::NativeOverlay);
    }

    #[test]
    fn focus_overlay_for_composited_texture_uses_rect_stroke_style() {
        let node = NodeKey::new(41);
        let tile_rect = egui::Rect::from_min_max(egui::pos2(20.0, 20.0), egui::pos2(120.0, 80.0));
        let overlay = focus_overlay_for_mode(
            test_semantic_input(node, TileRenderMode::CompositedTexture),
            tile_rect,
            1.0,
            &test_presentation_profile(),
        );

        assert!(matches!(overlay.style, OverlayAffordanceStyle::RectStroke));
        assert_eq!(overlay.render_mode, TileRenderMode::CompositedTexture);
    }

    #[test]
    fn hover_overlay_for_native_overlay_uses_chrome_only_style() {
        let node = NodeKey::new(42);
        let tile_rect = egui::Rect::from_min_max(egui::pos2(30.0, 30.0), egui::pos2(130.0, 90.0));
        let overlay = hover_overlay_for_mode(
            test_semantic_input(node, TileRenderMode::NativeOverlay),
            tile_rect,
            &test_presentation_profile(),
        );

        assert!(matches!(overlay.style, OverlayAffordanceStyle::ChromeOnly));
        assert_eq!(overlay.render_mode, TileRenderMode::NativeOverlay);
    }

    #[test]
    fn hover_overlay_for_composited_texture_uses_rect_stroke_style() {
        let node = NodeKey::new(43);
        let tile_rect = egui::Rect::from_min_max(egui::pos2(40.0, 40.0), egui::pos2(140.0, 100.0));
        let overlay = hover_overlay_for_mode(
            test_semantic_input(node, TileRenderMode::CompositedTexture),
            tile_rect,
            &test_presentation_profile(),
        );

        assert!(matches!(overlay.style, OverlayAffordanceStyle::RectStroke));
        assert_eq!(overlay.render_mode, TileRenderMode::CompositedTexture);
    }

    #[test]
    fn focus_overlay_for_placeholder_uses_area_style() {
        let node = NodeKey::new(44);
        let tile_rect = egui::Rect::from_min_max(egui::pos2(50.0, 50.0), egui::pos2(150.0, 110.0));
        let overlay = focus_overlay_for_mode(
            test_semantic_input(node, TileRenderMode::Placeholder),
            tile_rect,
            1.0,
            &test_presentation_profile(),
        );

        assert!(matches!(
            overlay.style,
            OverlayAffordanceStyle::EguiAreaStroke
        ));
        assert_eq!(overlay.render_mode, TileRenderMode::Placeholder);
    }

    #[test]
    fn hover_overlay_for_embedded_egui_uses_area_style() {
        let node = NodeKey::new(45);
        let tile_rect = egui::Rect::from_min_max(egui::pos2(60.0, 60.0), egui::pos2(160.0, 120.0));
        let overlay = hover_overlay_for_mode(
            test_semantic_input(node, TileRenderMode::EmbeddedEgui),
            tile_rect,
            &test_presentation_profile(),
        );

        assert!(matches!(
            overlay.style,
            OverlayAffordanceStyle::EguiAreaStroke
        ));
        assert_eq!(overlay.render_mode, TileRenderMode::EmbeddedEgui);
    }

    #[test]
    fn selection_overlay_for_placeholder_uses_area_style() {
        let node = NodeKey::new(46);
        let tile_rect = egui::Rect::from_min_max(egui::pos2(70.0, 70.0), egui::pos2(170.0, 130.0));
        let mut semantic = test_semantic_input(node, TileRenderMode::Placeholder);
        semantic.selection_state = TileSelectionState::SelectionPrimary;
        let overlay = selection_overlay_for_mode(semantic, tile_rect, &test_presentation_profile());

        assert!(matches!(
            overlay.style,
            OverlayAffordanceStyle::EguiAreaStroke
        ));
        assert_eq!(overlay.render_mode, TileRenderMode::Placeholder);
    }

    #[test]
    fn differential_content_decision_skips_when_signature_is_unchanged() {
        clear_composited_signature_cache_for_tests();
        let node = NodeKey::new(50);
        let signature = content_signature_for_tile(
            test_webview_id(),
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(100.0, 60.0)),
            1.0,
            1,
        );

        assert!(matches!(
            differential_content_decision(node, signature),
            DifferentialObservation {
                decision: DifferentialContentDecision::Compose(
                    DifferentialComposeReason::NoPriorSignature
                ),
                ..
            }
        ));
        assert!(matches!(
            differential_content_decision(node, signature),
            DifferentialObservation {
                decision: DifferentialContentDecision::SkipUnchanged,
                ..
            }
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
            1,
        );
        let changed = content_signature_for_tile(
            webview,
            egui::Rect::from_min_max(egui::pos2(10.0, 0.0), egui::pos2(110.0, 60.0)),
            1.0,
            1,
        );

        let _ = differential_content_decision(node, original);
        // Phase C: rect-only change is now decomposed as PlacementOnly.
        assert!(matches!(
            differential_content_decision(node, changed),
            DifferentialObservation {
                decision: DifferentialContentDecision::Compose(
                    DifferentialComposeReason::PlacementOnly
                ),
                placement_only: true,
                content_changed: false,
                ..
            }
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
            1,
        );

        let _ = differential_content_decision(focused, signature);
        assert!(matches!(
            differential_content_decision(focused, signature),
            DifferentialObservation {
                decision: DifferentialContentDecision::SkipUnchanged,
                ..
            }
        ));
        let focused_pane = pane_id_for_node(&tree, focused);
        let other_pane = pane_id_for_node(&tree, other);

        let passes = schedule_active_node_pane_passes(
            vec![
                scheduled_tile_input(
                    focused_pane,
                    focused,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(100.0, 60.0)),
                    TileRenderMode::CompositedTexture,
                ),
                scheduled_tile_input(
                    other_pane,
                    other,
                    egui::Rect::from_min_max(egui::pos2(120.0, 0.0), egui::pos2(220.0, 60.0)),
                    TileRenderMode::CompositedTexture,
                ),
            ],
            Some(focused),
            1.0,
            None,
        );

        let focused_pass = passes
            .iter()
            .find(|pass| pass.semantic.node_key == focused)
            .expect("focused node pass should be scheduled");
        assert!(matches!(
            focused_pass.overlays.last(),
            Some(ScheduledOverlay::Focus(_))
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
            1,
        );

        let _ = differential_content_decision(hovered, signature);
        assert!(matches!(
            differential_content_decision(hovered, signature),
            DifferentialObservation {
                decision: DifferentialContentDecision::SkipUnchanged,
                ..
            }
        ));
        let hovered_pane = pane_id_for_node(&tree, hovered);
        let other_pane = pane_id_for_node(&tree, other);

        let passes = schedule_active_node_pane_passes(
            vec![
                scheduled_tile_input(
                    hovered_pane,
                    hovered,
                    egui::Rect::from_min_max(egui::pos2(120.0, 0.0), egui::pos2(220.0, 60.0)),
                    TileRenderMode::CompositedTexture,
                ),
                scheduled_tile_input(
                    other_pane,
                    other,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(100.0, 60.0)),
                    TileRenderMode::CompositedTexture,
                ),
            ],
            Some(other),
            1.0,
            Some(hovered),
        );

        let hovered_pass = passes
            .iter()
            .find(|pass| pass.semantic.node_key == hovered)
            .expect("hovered node pass should be scheduled");
        assert!(matches!(
            hovered_pass.overlays.last(),
            Some(ScheduledOverlay::Hover(_))
        ));
    }

    #[test]
    fn content_signature_changes_when_semantic_generation_changes() {
        let webview = test_webview_id();
        let rect = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(100.0, 60.0));
        let original = content_signature_for_tile(webview, rect, 1.0, 1);
        let changed = content_signature_for_tile(webview, rect, 1.0, 2);

        assert_ne!(original, changed);
    }

    #[test]
    fn resolve_tile_semantic_input_marks_unread_traversal_activity_from_semantic_tags() {
        let mut app = GraphBrowserApp::new_for_testing();
        let unread = app.add_node_and_sync(
            "https://example.com/unread".to_string(),
            Point2D::new(0.0, 0.0),
        );
        let other = app.add_node_and_sync(
            "https://example.com/other".to_string(),
            Point2D::new(40.0, 0.0),
        );
        let tree = tree_with_two_active_nodes(unread, other);
        let pane_id = pane_id_for_node(&tree, unread);
        let tile_rect = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(100.0, 60.0));

        let baseline = resolve_tile_semantic_input(
            &tree,
            &app,
            pane_id,
            unread,
            tile_rect,
            FocusDelta::new(None, None),
        );

        let _ = app
            .workspace
            .domain
            .graph
            .insert_node_tag(unread, GraphBrowserApp::TAG_UNREAD.to_string());

        let flagged = resolve_tile_semantic_input(
            &tree,
            &app,
            pane_id,
            unread,
            tile_rect,
            FocusDelta::new(None, None),
        );

        assert!(!baseline.semantic.has_unread_traversal_activity);
        assert!(flagged.semantic.has_unread_traversal_activity);
        assert_ne!(
            baseline.semantic.semantic_generation,
            flagged.semantic.semantic_generation
        );
    }

    #[test]
    fn schedule_passes_add_semantic_overlay_for_cold_tiles() {
        let node = NodeKey::new(56);
        let pane_id = PaneId::new();
        let mut semantic = test_semantic_input(node, TileRenderMode::CompositedTexture);
        semantic.lifecycle = NodeLifecycle::Cold;
        semantic.semantic_generation = 7;

        let passes = schedule_active_node_pane_passes(
            vec![ScheduledTileSemanticInput {
                pane_id,
                tile_rect: egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(100.0, 60.0)),
                semantic,
            }],
            None,
            1.0,
            None,
        );

        assert!(matches!(
            passes[0].overlays.first(),
            Some(ScheduledOverlay::Semantic(_))
        ));
    }

    #[test]
    fn webview_preview_refresh_interval_uses_app_preferences_and_freezes_cold_nodes() {
        let mut app = GraphBrowserApp::new_for_testing();
        app.set_webview_preview_active_refresh_secs(7);
        app.set_webview_preview_warm_refresh_secs(45);

        assert_eq!(
            webview_preview_refresh_interval_for_lifecycle(&app, NodeLifecycle::Active),
            Some(std::time::Duration::from_secs(7))
        );
        assert_eq!(
            webview_preview_refresh_interval_for_lifecycle(&app, NodeLifecycle::Warm),
            Some(std::time::Duration::from_secs(45))
        );
        assert_eq!(
            webview_preview_refresh_interval_for_lifecycle(&app, NodeLifecycle::Cold),
            None
        );
        assert_eq!(
            webview_preview_refresh_interval_for_lifecycle(&app, NodeLifecycle::Tombstone),
            None
        );
    }

    #[test]
    fn semantic_overlay_for_runtime_blocked_uses_warning_color() {
        let node = NodeKey::new(57);
        let mut semantic = test_semantic_input(node, TileRenderMode::CompositedTexture);
        semantic.runtime_blocked = true;
        let presentation = test_presentation_profile();

        let overlay = semantic_overlay_for_mode(
            semantic,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(120.0, 80.0)),
            &presentation,
        );

        assert_eq!(
            overlay.stroke.color,
            presentation.crash_blocked.to_color32()
        );
    }

    #[test]
    fn schedule_passes_include_selection_and_focus_for_primary_selection() {
        let node = NodeKey::new(58);
        let pane_id = PaneId::new();
        let mut semantic = test_semantic_input(node, TileRenderMode::CompositedTexture);
        semantic.selection_state = TileSelectionState::SelectionPrimary;

        let passes = schedule_active_node_pane_passes(
            vec![ScheduledTileSemanticInput {
                pane_id,
                tile_rect: egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(100.0, 60.0)),
                semantic,
            }],
            Some(node),
            1.0,
            None,
        );

        assert_eq!(passes[0].overlays.len(), 2);
        assert!(matches!(
            passes[0].overlays[0],
            ScheduledOverlay::Selection(_)
        ));
        assert!(matches!(passes[0].overlays[1], ScheduledOverlay::Focus(_)));
    }

    #[test]
    fn semantic_overlay_applies_lens_glyphs_and_tint() {
        let mut semantic = test_semantic_input(NodeKey::new(59), TileRenderMode::CompositedTexture);
        semantic.active_lens_overlay = Some(LensOverlayDescriptor {
            border_tint: Some(Color32::from_rgb(20, 220, 180)),
            glyph_overlays: vec![GlyphOverlay {
                glyph_id: "semantic".to_string(),
                anchor: crate::registries::atomic::lens::GlyphAnchor::TopRight,
            }],
            opacity_scale: 1.0,
            suppress_default_affordances: false,
        });

        let overlay = semantic_overlay_for_mode(
            semantic,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(120.0, 80.0)),
            &test_presentation_profile(),
        );

        assert_eq!(overlay.glyph_overlays.len(), 1);
    }

    #[test]
    fn semantic_overlay_uses_dashed_style_for_tombstone_tiles() {
        let mut semantic = test_semantic_input(NodeKey::new(60), TileRenderMode::CompositedTexture);
        semantic.lifecycle = NodeLifecycle::Tombstone;

        let overlay = semantic_overlay_for_mode(
            semantic,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(120.0, 80.0)),
            &test_presentation_profile(),
        );

        assert!(matches!(
            overlay.style,
            OverlayAffordanceStyle::DashedRectStroke
        ));
    }

    #[test]
    fn focus_delta_marks_changed_nodes_only() {
        let delta = FocusDelta::new(Some(NodeKey::new(70)), Some(NodeKey::new(71)));

        assert!(delta.changed_this_frame);
        assert!(delta.touches(NodeKey::new(70)));
        assert!(delta.touches(NodeKey::new(71)));
        assert!(!delta.touches(NodeKey::new(72)));
    }

    #[test]
    fn affordance_annotation_reports_focus_selection_and_lens_glyphs() {
        let mut semantic = test_semantic_input(NodeKey::new(61), TileRenderMode::CompositedTexture);
        semantic.selection_state = TileSelectionState::SelectionPrimary;
        semantic.runtime_blocked = true;
        semantic.active_lens_overlay = Some(LensOverlayDescriptor {
            border_tint: None,
            glyph_overlays: vec![GlyphOverlay {
                glyph_id: "semantic".to_string(),
                anchor: crate::registries::atomic::lens::GlyphAnchor::TopRight,
            }],
            opacity_scale: 1.0,
            suppress_default_affordances: false,
        });

        let annotation = tile_affordance_annotation(
            &semantic,
            &[
                ScheduledOverlay::Selection(semantic.clone()),
                ScheduledOverlay::Focus(semantic.clone()),
            ],
            true,
        );

        assert!(annotation.focus_ring_rendered);
        assert!(annotation.selection_ring_rendered);
        assert_eq!(
            annotation.lifecycle_treatment,
            LifecycleTreatment::RuntimeBlocked
        );
        assert_eq!(
            annotation.lens_glyphs_rendered,
            vec!["semantic".to_string()]
        );
    }

    #[test]
    fn should_cull_tile_content_when_disjoint_from_viewport() {
        let tile_rect =
            egui::Rect::from_min_max(egui::pos2(300.0, 300.0), egui::pos2(360.0, 360.0));
        let viewport_rect =
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(200.0, 200.0));

        assert!(should_cull_tile_content(
            tile_rect,
            &VisibleNavigationRegionSet::singleton(viewport_rect),
        ));
    }

    #[test]
    fn should_not_cull_tile_content_when_visible_in_viewport() {
        let tile_rect = egui::Rect::from_min_max(egui::pos2(40.0, 40.0), egui::pos2(140.0, 100.0));
        let viewport_rect =
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(200.0, 200.0));

        assert!(!should_cull_tile_content(
            tile_rect,
            &VisibleNavigationRegionSet::singleton(viewport_rect),
        ));
    }

    #[test]
    fn should_not_cull_tile_content_when_visible_in_any_navigation_region_rect() {
        let tile_rect = egui::Rect::from_min_max(egui::pos2(340.0, 8.0), egui::pos2(380.0, 32.0));
        let viewport_regions = VisibleNavigationRegionSet::from_rects(vec![
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(400.0, 40.0)),
            egui::Rect::from_min_max(egui::pos2(0.0, 40.0), egui::pos2(320.0, 260.0)),
            egui::Rect::from_min_max(egui::pos2(0.0, 260.0), egui::pos2(400.0, 300.0)),
        ]);

        assert!(!should_cull_tile_content(tile_rect, &viewport_regions));
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
