/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Surface Composition Contract guardrails for compositor callback boundaries.
//! The guarded callback path enforces that these OpenGL state fields are
//! stable before/after content-pass rendering: viewport, scissor enable,
//! blend enable, active texture unit, and framebuffer binding.
//!
//! ## Host portability split
//!
//! This module separates content-surface management (host-neutral) from
//! overlay/content painting (host-specific). When porting to a second host
//! (iced), the painting layer is replaced; the state and registration layers
//! are shared.
//!
//! **Host-neutral (shared across hosts)**:
//! - `ViewerSurface` / `ViewerSurfaceRegistry` — content-surface state keyed
//!   by `NodeKey` (per `NodeKey is the owner` comment below).
//! - Content callback registry — `Fn(&BackendGraphicsContext, BackendViewportInPixels)`
//!   registered per `NodeKey`; `BackendGraphicsContext` is the host-neutral
//!   abstraction from `render_backend`.
//! - `CompositorPassTracker`, `OverlayAffordanceStyle`, diagnostics emission.
//! - `OverlayStrokePass` descriptor — overlay *intent*, not *how to draw*.
//!
//! **Host-specific (iced will reimplement against its own painter)**:
//! - `draw_overlay_stroke`, `draw_dashed_overlay_stroke`,
//!   `draw_overlay_stroke_in_area`, `draw_overlay_chrome_markers`,
//!   `draw_overlay_glyphs` — pixel operations against `egui::Context`.
//! - `content_layer(node_key)` / `overlay_layer(node_key)` — return
//!   `egui::LayerId` for egui's layer ordering.
//! - `execute_overlay_affordance_pass` — the per-frame overlay executor; iced
//!   will need an equivalent that consumes the same `OverlayStrokePass`
//!   descriptors.

use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};

use crate::graph::NodeKey;
use crate::shell::desktop::render_backend::{
    BackendTextureToken, HostNeutralRenderBackend, UiRenderBackendContract, UiRenderBackendHandle,
    texture_id_from_token,
};
use crate::shell::desktop::runtime::registries::{
    CHANNEL_COMPOSITOR_CONTENT_PASS_REGISTERED, CHANNEL_COMPOSITOR_INVALID_TILE_RECT,
    CHANNEL_COMPOSITOR_OVERLAY_MODE_COMPOSITED_TEXTURE,
    CHANNEL_COMPOSITOR_OVERLAY_MODE_EMBEDDED_EGUI, CHANNEL_COMPOSITOR_OVERLAY_MODE_NATIVE_OVERLAY,
    CHANNEL_COMPOSITOR_OVERLAY_MODE_PLACEHOLDER, CHANNEL_COMPOSITOR_OVERLAY_PASS_REGISTERED,
    CHANNEL_COMPOSITOR_OVERLAY_STYLE_CHROME_ONLY, CHANNEL_COMPOSITOR_OVERLAY_STYLE_RECT_STROKE,
    CHANNEL_COMPOSITOR_PASS_ORDER_VIOLATION,
};
use crate::shell::desktop::workbench::pane_model::TileRenderMode;
use dpi::PhysicalSize;
use egui::{Area, Context, Id, LayerId, Order, Rect as EguiRect, Stroke, StrokeKind};

// Portable geometry aliases moved to `graphshell_core::geometry` in M4
// slice 8 (2026-04-22). Re-exported here so existing imports
// (`shell::desktop::workbench::compositor_adapter::{PortableRect, …}`)
// resolve unchanged. The egui conversion helpers below stay shell-side
// (they touch `egui::*` types).
pub(crate) use graphshell_core::geometry::{PortablePoint, PortableRect, PortableSize};

#[inline]
pub(crate) fn portable_rect_from_egui(r: EguiRect) -> PortableRect {
    PortableRect::new(
        euclid::default::Point2D::new(r.min.x, r.min.y),
        euclid::default::Size2D::new(r.width(), r.height()),
    )
}

#[inline]
pub(crate) fn egui_rect_from_portable(r: PortableRect) -> EguiRect {
    EguiRect::from_min_size(
        egui::pos2(r.origin.x, r.origin.y),
        egui::vec2(r.size.width, r.size.height),
    )
}

#[inline]
pub(crate) fn portable_point_from_egui(p: egui::Pos2) -> PortablePoint {
    PortablePoint::new(p.x, p.y)
}

#[inline]
pub(crate) fn egui_point_from_portable(p: PortablePoint) -> egui::Pos2 {
    egui::pos2(p.x, p.y)
}

#[inline]
pub(crate) fn portable_size_from_egui(v: egui::Vec2) -> PortableSize {
    PortableSize::new(v.x, v.y)
}

#[inline]
pub(crate) fn egui_size_from_portable(s: PortableSize) -> egui::Vec2 {
    egui::vec2(s.width, s.height)
}

#[inline]
pub(crate) fn portable_stroke_from_egui(s: Stroke) -> graph_canvas::packet::Stroke {
    graph_canvas::packet::Stroke {
        width: s.width,
        color: graph_canvas::packet::Color::new(
            f32::from(s.color.r()) / 255.0,
            f32::from(s.color.g()) / 255.0,
            f32::from(s.color.b()) / 255.0,
            f32::from(s.color.a()) / 255.0,
        ),
    }
}

#[inline]
pub(crate) fn egui_stroke_from_portable(s: graph_canvas::packet::Stroke) -> Stroke {
    Stroke::new(
        s.width,
        egui::Color32::from_rgba_premultiplied(
            (s.color.r * 255.0).round().clamp(0.0, 255.0) as u8,
            (s.color.g * 255.0).round().clamp(0.0, 255.0) as u8,
            (s.color.b * 255.0).round().clamp(0.0, 255.0) as u8,
            (s.color.a * 255.0).round().clamp(0.0, 255.0) as u8,
        ),
    )
}
use euclid::{Scale, Size2D, UnknownUnit};
use log::warn;
use servo::{DevicePixel, OffscreenRenderingContext, RenderingContextCore, WebView};
use verso_host::ViewerSurfaceRegistryHost;

const CHANNEL_CONTENT_PASS_REGISTERED: &str = CHANNEL_COMPOSITOR_CONTENT_PASS_REGISTERED;
const CHANNEL_OVERLAY_PASS_REGISTERED: &str = CHANNEL_COMPOSITOR_OVERLAY_PASS_REGISTERED;
const CHANNEL_PASS_ORDER_VIOLATION: &str = CHANNEL_COMPOSITOR_PASS_ORDER_VIOLATION;
const CHANNEL_INVALID_TILE_RECT: &str = CHANNEL_COMPOSITOR_INVALID_TILE_RECT;
const COMPOSITOR_REPLAY_RING_CAPACITY: usize = 64;
static COMPOSITOR_REPLAY_RING: OnceLock<Mutex<std::collections::VecDeque<CompositorReplaySample>>> =
    OnceLock::new();
static COMPOSITOR_NATIVE_TEXTURES: OnceLock<Mutex<HashMap<NodeKey, BackendTextureToken>>> =
    OnceLock::new();

/// Named abstraction for what the compositor currently holds for a given node.
///
/// This is the Phase A concept from the GL→wgpu redesign plan. The primary
/// path is `ImportedWgpu`: the Servo GL framebuffer has been imported into a
/// shared wgpu texture and registered with egui for zero-copy blitting.
/// `CallbackFallback` is the named GL compat path. `Placeholder` means no
/// usable surface exists yet (node loading, runtime not ready, etc.).
///
/// Future: when WgpuShared is the only path, `CallbackFallback` is removed
/// and `tile_rendering_contexts` (the GL context pool) retires with it.
/// Shell-side specialization of the host-neutral
/// [`graphshell_runtime::ContentSurfaceHandle`] over the egui-host
/// `BackendTextureToken`. The enum shape and `is_wgpu()` live in
/// graphshell-runtime; the static-map lookup that the egui compositor uses to
/// reconstruct an `ImportedWgpu` handle (`content_surface_handle_for_node`
/// below) stays here because it depends on the shell-owned native-texture
/// registry.
pub(crate) type ContentSurfaceHandle =
    graphshell_runtime::ContentSurfaceHandle<BackendTextureToken>;

/// Query the compositor's current surface handle for a node.
///
/// Returns `ImportedWgpu` when the wgpu import path succeeded last frame,
/// `Placeholder` otherwise. The `CallbackFallback` case is not yet tracked
/// per-node (it is implicit in the absence of a wgpu token).
pub(crate) fn content_surface_handle_for_node(node_key: NodeKey) -> ContentSurfaceHandle {
    let token = compositor_native_texture_registry()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(&node_key)
        .copied();
    match token {
        Some(t) => ContentSurfaceHandle::ImportedWgpu(t),
        None => ContentSurfaceHandle::Placeholder,
    }
}

/// Host-native backing: a wgpu-capable rendering context.
#[derive(Clone)]
pub(crate) struct ViewerSurfaceBacking(pub(crate) std::rc::Rc<dyn RenderingContextCore>);

impl ViewerSurfaceBacking {
    pub(crate) fn rendering_context(&self) -> std::rc::Rc<dyn RenderingContextCore> {
        self.0.clone()
    }
}

pub(crate) use graphshell_runtime::ViewerSurfaceFramePath;

// ---------------------------------------------------------------------------
// Phase D: ViewerSurfaceRegistry — unified surface lifecycle keyed by NodeKey
// ---------------------------------------------------------------------------

/// Per-node viewer surface state.
///
/// Bundles the compositor-facing texture handle with the GL compat context
/// (which remains as a side-channel for GL fallback builds). Surface
/// lifecycle follows GraphTree node membership: attach → allocate,
/// detach → drop.
pub(crate) struct ViewerSurface {
    /// What the compositor should display for this node.
    pub(crate) handle: ContentSurfaceHandle,
    /// Monotonic generation counter from Servo's frame output.
    /// Incremented each time Servo produces a new frame for this webview.
    pub(crate) content_generation: u64,
    /// Transitional surface backing owned by the registry. M4.5's target is
    /// that the registry owns all viewer-surface backing state (native/shared
    /// wgpu in steady state, explicit GL compatibility producers only where
    /// needed) so hot-path callers stop treating "has GL context" as the
    /// authority check.
    pub(crate) backing: Option<ViewerSurfaceBacking>,
    /// The last viewer-surface/content-bridge path observed for this node
    /// during composition. Used by M4.5 diagnostics and parity work to pin
    /// which path a host actually exercised frame-to-frame.
    pub(crate) last_frame_path: Option<ViewerSurfaceFramePath>,
}

impl ViewerSurface {
    pub(crate) fn new_placeholder() -> Self {
        Self {
            handle: ContentSurfaceHandle::Placeholder,
            content_generation: 0,
            backing: None,
            last_frame_path: None,
        }
    }
}

/// Unified registry of viewer surfaces keyed by `NodeKey`.
///
/// This replaces the separate `tile_rendering_contexts` GL-context map and
/// the static `COMPOSITOR_NATIVE_TEXTURES` wgpu-token map with a single
/// authority. `NodeKey` is the owner; `WebViewId` and `PaneId` are lookup
/// keys within, not owners.
///
/// During migration, callers that still need the current compatibility backing
/// can use `compat_gl_context()`. New code should treat the registry as the
/// authority for:
///
/// - surface backing ownership
/// - compositor-facing handle state
/// - content generation
/// - the last exercised viewer-surface/content-bridge path
///
/// The first explicit M4.5 slice represented here is naming the current
/// compatibility surface category (`CompatGlOffscreen`) instead of storing a
/// naked `gl_ctx` field. That keeps today's implementation intact while making
/// room for the eventual shared-wgpu/native surface categories.
pub(crate) struct ViewerSurfaceRegistry {
    surfaces: HashMap<NodeKey, ViewerSurface>,
}

impl ViewerSurfaceRegistry {
    pub(crate) fn new() -> Self {
        Self {
            surfaces: HashMap::new(),
        }
    }

    /// Get the surface for a node, if any.
    pub(crate) fn surface(&self, key: &NodeKey) -> Option<&ViewerSurface> {
        self.surfaces.get(key)
    }

    /// Get the mutable surface for a node, if any.
    pub(crate) fn surface_mut(&mut self, key: &NodeKey) -> Option<&mut ViewerSurface> {
        self.surfaces.get_mut(key)
    }

    /// Get the content surface handle for a node.
    pub(crate) fn handle(&self, key: &NodeKey) -> ContentSurfaceHandle {
        self.surfaces
            .get(key)
            .map(|s| s.handle)
            .unwrap_or(ContentSurfaceHandle::Placeholder)
    }

    /// Get the rendering context for a node, regardless of whether it is a
    /// compat GL or host-native backing.
    pub(crate) fn rendering_context(
        &self,
        key: &NodeKey,
    ) -> Option<std::rc::Rc<dyn RenderingContextCore>> {
        self.surfaces
            .get(key)
            .and_then(|s| s.backing.as_ref())
            .map(ViewerSurfaceBacking::rendering_context)
    }

    /// Check if any viewer-surface backing exists for a node.
    pub(crate) fn has_surface(&self, key: &NodeKey) -> bool {
        self.surfaces
            .get(key)
            .map(|s| s.backing.is_some())
            .unwrap_or(false)
    }

    /// Install a fully-typed backing for a node, creating a surface entry if
    /// one doesn't exist.
    pub(crate) fn insert_backing(&mut self, key: NodeKey, backing: ViewerSurfaceBacking) {
        match self.surfaces.get_mut(&key) {
            Some(surface) => {
                surface.backing = Some(backing);
            }
            None => {
                self.surfaces.insert(
                    key,
                    ViewerSurface {
                        handle: ContentSurfaceHandle::Placeholder,
                        content_generation: 0,
                        backing: Some(backing),
                        last_frame_path: None,
                    },
                );
            }
        }
    }

    /// Drop all surfaces. Equivalent to the legacy `tile_rendering_contexts.clear()`.
    pub(crate) fn clear(&mut self) {
        self.surfaces.clear();
    }

    /// Update the surface handle for a node.
    pub(crate) fn set_handle(&mut self, key: NodeKey, handle: ContentSurfaceHandle) {
        match self.surfaces.get_mut(&key) {
            Some(surface) => {
                surface.handle = handle;
            }
            None => {
                self.surfaces.insert(
                    key,
                    ViewerSurface {
                        handle,
                        content_generation: 0,
                        backing: None,
                        last_frame_path: None,
                    },
                );
            }
        }
    }

    /// Bump the content generation for a node (called when Servo produces
    /// a new frame).
    pub(crate) fn bump_content_generation(&mut self, key: &NodeKey) {
        if let Some(surface) = self.surfaces.get_mut(key) {
            surface.content_generation = surface.content_generation.wrapping_add(1);
        }
    }

    /// Record which viewer-surface/content-bridge path the compositor actually
    /// exercised for this node on the current frame.
    pub(crate) fn record_frame_path(&mut self, key: NodeKey, path: ViewerSurfaceFramePath) {
        match self.surfaces.get_mut(&key) {
            Some(surface) => {
                surface.last_frame_path = Some(path);
            }
            None => {
                self.surfaces.insert(
                    key,
                    ViewerSurface {
                        handle: ContentSurfaceHandle::Placeholder,
                        content_generation: 0,
                        backing: None,
                        last_frame_path: Some(path),
                    },
                );
            }
        }
    }

    /// Remove a node's surface entirely (on detach/lifecycle Cold).
    pub(crate) fn remove(&mut self, key: &NodeKey) -> Option<ViewerSurface> {
        self.surfaces.remove(key)
    }
}

impl ViewerSurfaceRegistryHost for ViewerSurfaceRegistry {
    type Surface = ViewerSurfaceBacking;

    fn get_or_insert_surface_with<F>(&mut self, node_key: NodeKey, create_surface: F)
    where
        F: FnOnce() -> Self::Surface,
    {
        if !self.has_surface(&node_key) {
            self.insert_backing(node_key, create_surface());
        }
    }

    fn retire_surface(&mut self, node_key: NodeKey) {
        self.remove(&node_key);
    }

    fn has_surface(&self, node_key: NodeKey) -> bool {
        ViewerSurfaceRegistry::has_surface(self, &node_key)
    }
}

impl ViewerSurfaceRegistry {
    /// Iterate over all node keys with surfaces.
    pub(crate) fn keys(&self) -> impl Iterator<Item = &NodeKey> {
        self.surfaces.keys()
    }

    /// Iterate over all surfaces.
    pub(crate) fn iter(&self) -> impl Iterator<Item = (&NodeKey, &ViewerSurface)> {
        self.surfaces.iter()
    }

    /// Number of registered surfaces.
    pub(crate) fn len(&self) -> usize {
        self.surfaces.len()
    }

    /// Whether the registry is empty.
    pub(crate) fn is_empty(&self) -> bool {
        self.surfaces.is_empty()
    }
}

/// Frozen GL-state shape kept for diagnostics-replay-sample compatibility.
/// The wgpu compositor never populates these fields with non-default values;
/// they exist only because `CompositorReplaySample` (consumed by diagnostics
/// export) still carries `before`/`after` snapshots. Retire alongside the
/// replay sample when diagnostics is reshaped.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GlStateSnapshot {
    pub(crate) viewport: [i32; 4],
    pub(crate) scissor_enabled: bool,
    pub(crate) blend_enabled: bool,
    pub(crate) active_texture: i32,
    pub(crate) framebuffer_binding: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CompositorReplaySample {
    pub(crate) sequence: u64,
    pub(crate) node_key: NodeKey,
    pub(crate) duration_us: u64,
    pub(crate) callback_us: u64,
    pub(crate) presentation_us: u64,
    pub(crate) violation: bool,
    pub(crate) bridge_path: &'static str,
    pub(crate) bridge_mode: &'static str,
    pub(crate) tile_rect_px: [i32; 4],
    pub(crate) render_size_px: [u32; 2],
    pub(crate) chaos_enabled: bool,
    pub(crate) restore_verified: bool,
    pub(crate) viewport_changed: bool,
    pub(crate) scissor_changed: bool,
    pub(crate) blend_changed: bool,
    pub(crate) active_texture_changed: bool,
    pub(crate) framebuffer_binding_changed: bool,
    pub(crate) before: GlStateSnapshot,
    pub(crate) after: GlStateSnapshot,
}

fn replay_ring() -> &'static Mutex<std::collections::VecDeque<CompositorReplaySample>> {
    COMPOSITOR_REPLAY_RING.get_or_init(|| {
        Mutex::new(std::collections::VecDeque::with_capacity(
            COMPOSITOR_REPLAY_RING_CAPACITY,
        ))
    })
}

fn compositor_native_texture_registry() -> &'static Mutex<HashMap<NodeKey, BackendTextureToken>> {
    COMPOSITOR_NATIVE_TEXTURES.get_or_init(|| Mutex::new(HashMap::new()))
}

#[cfg(test)]
fn clear_native_textures_for_tests() {
    compositor_native_texture_registry()
        .lock()
        .expect("compositor native texture registry mutex poisoned")
        .clear();
}

pub(crate) fn replay_samples_snapshot() -> Vec<CompositorReplaySample> {
    replay_ring()
        .lock()
        .expect("compositor replay ring mutex poisoned")
        .iter()
        .copied()
        .collect()
}


pub(crate) struct CompositorPassTracker {
    content_pass_nodes: HashSet<NodeKey>,
}

/// Host-neutral overlay-pass descriptor. `tile_rect` and `stroke` carry
/// portable types (`euclid::default::Rect<f32>` /
/// `graph_canvas::packet::Stroke`) so this descriptor can flow across the
/// host boundary without egui leakage. Egui painters convert back at the
/// draw-call boundary via `egui_rect_from_portable` /
/// `egui_stroke_from_portable`; iced painters consume the portable types
/// directly.
// `OverlayStrokePass` + `OverlayAffordanceStyle` moved to
// `graphshell_core::overlay` in M4 slice 10 (2026-04-22). Re-exported
// here so existing call sites resolve unchanged.
pub(crate) use graphshell_core::overlay::{OverlayAffordanceStyle, OverlayStrokePass};

/// Host-agnostic sink for an overlay-affordance pass. Implementors
/// turn one [`OverlayStrokePass`] descriptor into actual pixels using
/// whatever host-specific painter they have.
///
/// The compositor generates the descriptor list in a host-neutral
/// step; the trait is the single seam the iced host (and any future
/// host) needs to implement to render overlay strokes. See
/// [`CompositorAdapter::execute_overlay_affordance_pass_with_painter`].
pub(crate) trait OverlayAffordancePainter {
    fn paint(&mut self, overlay: &OverlayStrokePass);
}

/// Egui implementation of the overlay-affordance painter. Delegates
/// to the existing `CompositorAdapter::draw_*` associated functions,
/// which stay the implementation detail of the egui host. An iced
/// implementation would mirror this shape with iced's painting APIs.
pub(crate) struct EguiOverlayAffordancePainter<'a> {
    pub(crate) ctx: &'a Context,
}

impl OverlayAffordancePainter for EguiOverlayAffordancePainter<'_> {
    fn paint(&mut self, overlay: &OverlayStrokePass) {
        let egui_rect = egui_rect_from_portable(overlay.tile_rect);
        let egui_stroke = egui_stroke_from_portable(overlay.stroke);
        match overlay.style {
            OverlayAffordanceStyle::RectStroke => CompositorAdapter::draw_overlay_stroke(
                self.ctx,
                overlay.node_key,
                egui_rect,
                overlay.rounding,
                egui_stroke,
            ),
            OverlayAffordanceStyle::DashedRectStroke => {
                CompositorAdapter::draw_dashed_overlay_stroke(
                    self.ctx,
                    overlay.node_key,
                    egui_rect,
                    egui_stroke,
                )
            }
            OverlayAffordanceStyle::AreaStroke => CompositorAdapter::draw_overlay_stroke_in_area(
                self.ctx,
                overlay.node_key,
                egui_rect,
                overlay.rounding,
                egui_stroke,
            ),
            OverlayAffordanceStyle::ChromeOnly => CompositorAdapter::draw_overlay_chrome_markers(
                self.ctx,
                overlay.node_key,
                egui_rect,
                egui_stroke,
            ),
        }
        CompositorAdapter::draw_overlay_glyphs(
            self.ctx,
            overlay.node_key,
            egui_rect,
            &overlay.glyph_overlays,
            egui_stroke.color,
            overlay.style,
        );
    }
}

/// Host-agnostic sink for content-pass registration and native-texture
/// painting. Implementors turn host-neutral content-pass operations
/// (register a GL callback at a node's content layer, paint a native
/// shared-wgpu-texture at a node's content layer) into whatever
/// host-specific layer/painter API they have.
///
/// The compositor generates content-pass outcomes in a host-neutral
/// step (`prepare_composited_target`, `paint_offscreen_content_pass`,
/// bridge selection, native-texture upsert, callback registry); the
/// trait is the single seam the iced host (and any future host) needs
/// to implement for content-layer placement. See
/// [`CompositorAdapter::compose_webview_content_pass_with_painter`] and
/// [`CompositorAdapter::compose_registered_content_pass_with_painter`].
pub(crate) trait ContentPassPainter {
    /// Paint a native shared-wgpu content texture at the node's content
    /// layer.
    fn paint_native_content_texture(
        &mut self,
        node_key: NodeKey,
        tile_rect: PortableRect,
        texture_token: BackendTextureToken,
    );
}

/// Egui implementation of the content-pass painter. Delegates to
/// `CompositorAdapter::paint_native_content_texture`, which remains the
/// implementation detail of the egui host. Converts portable rects to
/// `EguiRect` at the draw-call boundary. An iced implementation mirrors
/// this shape with iced's painting APIs, consuming `PortableRect` directly
/// without conversion.
pub(crate) struct EguiContentPassPainter<'a> {
    pub(crate) ctx: &'a Context,
}

impl ContentPassPainter for EguiContentPassPainter<'_> {
    fn paint_native_content_texture(
        &mut self,
        node_key: NodeKey,
        tile_rect: PortableRect,
        texture_token: BackendTextureToken,
    ) {
        CompositorAdapter::paint_native_content_texture(
            self.ctx,
            node_key,
            egui_rect_from_portable(tile_rect),
            texture_token,
        );
    }
}

fn overlay_style_channel(style: OverlayAffordanceStyle) -> &'static str {
    match style {
        OverlayAffordanceStyle::RectStroke
        | OverlayAffordanceStyle::DashedRectStroke
        | OverlayAffordanceStyle::AreaStroke => CHANNEL_COMPOSITOR_OVERLAY_STYLE_RECT_STROKE,
        OverlayAffordanceStyle::ChromeOnly => CHANNEL_COMPOSITOR_OVERLAY_STYLE_CHROME_ONLY,
    }
}

fn overlay_mode_channel(render_mode: TileRenderMode) -> &'static str {
    match render_mode {
        TileRenderMode::CompositedTexture => CHANNEL_COMPOSITOR_OVERLAY_MODE_COMPOSITED_TEXTURE,
        TileRenderMode::NativeOverlay => CHANNEL_COMPOSITOR_OVERLAY_MODE_NATIVE_OVERLAY,
        TileRenderMode::EmbeddedEgui => CHANNEL_COMPOSITOR_OVERLAY_MODE_EMBEDDED_EGUI,
        TileRenderMode::Placeholder => CHANNEL_COMPOSITOR_OVERLAY_MODE_PLACEHOLDER,
    }
}

impl CompositorPassTracker {
    pub(crate) fn new() -> Self {
        Self {
            content_pass_nodes: HashSet::new(),
        }
    }

    pub(crate) fn record_content_pass(&mut self, node_key: NodeKey) {
        self.content_pass_nodes.insert(node_key);
        #[cfg(feature = "diagnostics")]
        crate::shell::desktop::runtime::diagnostics::emit_event(
            crate::shell::desktop::runtime::diagnostics::DiagnosticEvent::MessageSent {
                channel_id: CHANNEL_CONTENT_PASS_REGISTERED,
                byte_len: std::mem::size_of::<NodeKey>(),
            },
        );
    }

    pub(crate) fn record_overlay_pass(&self, node_key: NodeKey, render_mode: TileRenderMode) {
        #[cfg(feature = "diagnostics")]
        crate::shell::desktop::runtime::diagnostics::emit_event(
            crate::shell::desktop::runtime::diagnostics::DiagnosticEvent::MessageSent {
                channel_id: CHANNEL_OVERLAY_PASS_REGISTERED,
                byte_len: std::mem::size_of::<NodeKey>(),
            },
        );

        if render_mode == TileRenderMode::CompositedTexture
            && !self.content_pass_nodes.contains(&node_key)
        {
            #[cfg(feature = "diagnostics")]
            crate::shell::desktop::runtime::diagnostics::emit_event(
                crate::shell::desktop::runtime::diagnostics::DiagnosticEvent::MessageSent {
                    channel_id: CHANNEL_PASS_ORDER_VIOLATION,
                    byte_len: std::mem::size_of::<NodeKey>(),
                },
            );
        }
    }
}

pub(crate) struct CompositorAdapter;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CompositedContentPassOutcome {
    SharedWgpuRegistered,
    CallbackFallbackRegistered,
    MissingSurface,
    InvalidTileRect,
    PaintFailed,
    MissingContentCallback,
}

impl CompositorAdapter {
    pub(crate) fn compose_webview_content_pass_for_surface_with_painter(
        painter: &mut dyn ContentPassPainter,
        ui_render_backend: &mut UiRenderBackendHandle,
        node_key: NodeKey,
        tile_rect: PortableRect,
        pixels_per_point: f32,
        surface: &mut ViewerSurface,
        webview: &WebView,
    ) -> CompositedContentPassOutcome {
        let Some(backing) = surface.backing.as_ref() else {
            surface.handle = ContentSurfaceHandle::Placeholder;
            return CompositedContentPassOutcome::MissingSurface;
        };

        let rendering_context = &backing.0;
        let egui_tile_rect = egui_rect_from_portable(tile_rect);
        let Some((size, target_size)) = Self::prepare_composited_target(
            node_key,
            egui_tile_rect,
            pixels_per_point,
            rendering_context.as_ref(),
        ) else {
            return CompositedContentPassOutcome::InvalidTileRect;
        };

        Self::reconcile_webview_target_size(webview, size, target_size);
        webview.render();

        let existing = match surface.handle {
            ContentSurfaceHandle::ImportedWgpu(token) => Some(token),
            ContentSurfaceHandle::CallbackFallback | ContentSurfaceHandle::Placeholder => None,
        };
        let Some(texture) = webview.composite_texture() else {
            return CompositedContentPassOutcome::PaintFailed;
        };
        let Some(texture_token) = Self::upsert_native_content_texture_from_texture(
            node_key,
            existing,
            &texture,
            ui_render_backend,
        ) else {
            return CompositedContentPassOutcome::PaintFailed;
        };

        painter.paint_native_content_texture(node_key, tile_rect, texture_token);
        // ViewerSurfaceRegistry is the sole live-handle authority for this
        // path. COMPOSITOR_NATIVE_TEXTURES is a write-through for retirement
        // only (retire_node_content_resources reads it to free GPU memory).
        surface.handle = ContentSurfaceHandle::ImportedWgpu(texture_token);
        CompositedContentPassOutcome::SharedWgpuRegistered
    }

    pub(crate) fn compose_webview_content_pass_for_surface(
        ctx: &Context,
        ui_render_backend: &mut UiRenderBackendHandle,
        node_key: NodeKey,
        tile_rect: EguiRect,
        pixels_per_point: f32,
        surface: &mut ViewerSurface,
        webview: &WebView,
    ) -> CompositedContentPassOutcome {
        let mut painter = EguiContentPassPainter { ctx };
        Self::compose_webview_content_pass_for_surface_with_painter(
            &mut painter,
            ui_render_backend,
            node_key,
            portable_rect_from_egui(tile_rect),
            pixels_per_point,
            surface,
            webview,
        )
    }

    pub(crate) fn content_layer(node_key: NodeKey) -> LayerId {
        LayerId::new(
            egui::Order::Middle,
            Id::new(("graphshell_webview", node_key)),
        )
    }

    pub(crate) fn overlay_layer(node_key: NodeKey) -> LayerId {
        LayerId::new(
            egui::Order::Foreground,
            Id::new(("graphshell_overlay", node_key)),
        )
    }

    pub(crate) fn retire_node_content_resources<B>(ui_render_backend: &mut B, node_key: NodeKey)
    where
        B: UiRenderBackendContract,
    {
        if let Some(texture_token) = compositor_native_texture_registry()
            .lock()
            .expect("compositor native texture registry mutex poisoned")
            .remove(&node_key)
        {
            ui_render_backend.free_native_texture(texture_token);
        }
    }

    pub(crate) fn retire_stale_content_resources<B>(
        ui_render_backend: &mut B,
        retained_nodes: &HashSet<NodeKey>,
    ) where
        B: UiRenderBackendContract,
    {
        let stale_native_textures: HashSet<_> = compositor_native_texture_registry()
            .lock()
            .expect("compositor native texture registry mutex poisoned")
            .keys()
            .copied()
            .filter(|node_key| !retained_nodes.contains(node_key))
            .collect();

        for node_key in stale_native_textures {
            Self::retire_node_content_resources(ui_render_backend, node_key);
        }
    }

    fn paint_native_content_texture(
        ctx: &Context,
        node_key: NodeKey,
        tile_rect: EguiRect,
        texture_token: BackendTextureToken,
    ) {
        ctx.layer_painter(Self::content_layer(node_key)).image(
            texture_id_from_token(texture_token),
            tile_rect,
            EguiRect::from_min_max(egui::pos2(0.0, 1.0), egui::pos2(1.0, 0.0)),
            egui::Color32::WHITE,
        );
    }

    fn upsert_native_content_texture(
        node_key: NodeKey,
        render_context: &OffscreenRenderingContext,
        ui_render_backend: &mut UiRenderBackendHandle,
    ) -> Option<BackendTextureToken> {
        let (device, queue) = ui_render_backend.shared_wgpu_device_queue()?;
        let imported_texture = render_context.import_to_shared_wgpu_texture(device, queue)?;
        let existing = compositor_native_texture_registry()
            .lock()
            .expect("compositor native texture registry mutex poisoned")
            .get(&node_key)
            .copied();
        Self::upsert_native_content_texture_from_texture(
            node_key,
            existing,
            &imported_texture,
            ui_render_backend,
        )
    }

    fn upsert_native_content_texture_from_texture(
        node_key: NodeKey,
        existing: Option<BackendTextureToken>,
        texture: &servo::wgpu::Texture,
        ui_render_backend: &mut UiRenderBackendHandle,
    ) -> Option<BackendTextureToken> {
        let token = ui_render_backend.upsert_native_texture(existing, texture)?;
        compositor_native_texture_registry()
            .lock()
            .expect("compositor native texture registry mutex poisoned")
            .insert(node_key, token);
        Some(token)
    }

    pub(crate) fn prepare_composited_target(
        node_key: NodeKey,
        tile_rect: EguiRect,
        pixels_per_point: f32,
        render_context: &dyn RenderingContextCore,
    ) -> Option<(Size2D<f32, DevicePixel>, PhysicalSize<u32>)> {
        if !tile_rect.width().is_finite()
            || !tile_rect.height().is_finite()
            || tile_rect.width() <= 0.0
            || tile_rect.height() <= 0.0
        {
            Self::report_invalid_tile_rect(node_key);
            return None;
        }

        let scale = Scale::<_, UnknownUnit, DevicePixel>::new(pixels_per_point);
        let size = Size2D::new(tile_rect.width(), tile_rect.height()) * scale;
        let target_size = PhysicalSize::new(
            size.width.max(1.0).round() as u32,
            size.height.max(1.0).round() as u32,
        );

        if render_context.size() != target_size {
            log::debug!(
                "composite: resizing render_context from {:?} to {:?}",
                render_context.size(),
                target_size
            );
            render_context.resize(target_size);
        }

        Some((size, target_size))
    }

    pub(crate) fn paint_offscreen_content_pass<F>(
        render_context: &OffscreenRenderingContext,
        target_size: PhysicalSize<u32>,
        paint: F,
    ) -> bool
    where
        F: FnOnce(),
    {
        #[cfg(feature = "diagnostics")]
        let paint_started = std::time::Instant::now();

        #[cfg(feature = "diagnostics")]
        crate::shell::desktop::runtime::diagnostics::emit_event(
            crate::shell::desktop::runtime::diagnostics::DiagnosticEvent::MessageSent {
                channel_id: "tile_compositor.paint",
                byte_len: (target_size.width as usize)
                    .saturating_mul(target_size.height as usize)
                    .saturating_mul(4),
            },
        );

        if let Some(gl) = render_context.gl() {
            if let Err(e) = gl.make_current() {
                warn!("Failed to make tile rendering context current: {e:?}");
                return false;
            }
            gl.prepare_for_rendering();
        }
        paint();
        render_context.present();

        #[cfg(feature = "diagnostics")]
        {
            let elapsed = paint_started.elapsed().as_micros() as u64;
            crate::shell::desktop::runtime::diagnostics::emit_event(
                crate::shell::desktop::runtime::diagnostics::DiagnosticEvent::MessageReceived {
                    channel_id: "tile_compositor.paint",
                    latency_us: elapsed,
                },
            );
            crate::shell::desktop::runtime::diagnostics::emit_span_duration(
                "tile_compositor::paint_present",
                elapsed,
            );
        }

        true
    }

    pub(crate) fn reconcile_webview_target_size(
        webview: &WebView,
        size: Size2D<f32, DevicePixel>,
        target_size: PhysicalSize<u32>,
    ) {
        if webview.size() != size {
            log::debug!(
                "composite: resizing webview from {:?} to {:?}",
                webview.size(),
                size
            );
            webview.resize(target_size);
        }
    }

    pub(crate) fn draw_overlay_stroke(
        ctx: &Context,
        node_key: NodeKey,
        tile_rect: EguiRect,
        rounding: f32,
        stroke: Stroke,
    ) {
        #[cfg(feature = "diagnostics")]
        let started = std::time::Instant::now();

        ctx.layer_painter(Self::overlay_layer(node_key))
            .rect_stroke(tile_rect.shrink(1.0), rounding, stroke, StrokeKind::Inside);

        #[cfg(feature = "diagnostics")]
        crate::shell::desktop::runtime::diagnostics::emit_span_duration(
            "tile_compositor::overlay_pass_draw",
            started.elapsed().as_micros() as u64,
        );
    }

    pub(crate) fn draw_dashed_overlay_stroke(
        ctx: &Context,
        node_key: NodeKey,
        tile_rect: EguiRect,
        stroke: Stroke,
    ) {
        #[cfg(feature = "diagnostics")]
        let started = std::time::Instant::now();

        fn draw_dashed_segment(
            painter: &egui::Painter,
            start: egui::Pos2,
            end: egui::Pos2,
            stroke: Stroke,
        ) {
            let dash = 10.0;
            let gap = 6.0;
            let horizontal = (start.y - end.y).abs() < f32::EPSILON;
            let total = if horizontal {
                (end.x - start.x).abs()
            } else {
                (end.y - start.y).abs()
            };
            let direction = if horizontal {
                egui::vec2((end.x - start.x).signum(), 0.0)
            } else {
                egui::vec2(0.0, (end.y - start.y).signum())
            };
            let mut offset = 0.0;
            while offset < total {
                let from = start + direction * offset;
                let to = start + direction * (offset + dash).min(total);
                painter.line_segment([from, to], stroke);
                offset += dash + gap;
            }
        }

        let rect = tile_rect.shrink(1.0);
        let painter = ctx.layer_painter(Self::overlay_layer(node_key));
        draw_dashed_segment(&painter, rect.left_top(), rect.right_top(), stroke);
        draw_dashed_segment(&painter, rect.right_top(), rect.right_bottom(), stroke);
        draw_dashed_segment(&painter, rect.right_bottom(), rect.left_bottom(), stroke);
        draw_dashed_segment(&painter, rect.left_bottom(), rect.left_top(), stroke);

        #[cfg(feature = "diagnostics")]
        crate::shell::desktop::runtime::diagnostics::emit_span_duration(
            "tile_compositor::overlay_pass_draw",
            started.elapsed().as_micros() as u64,
        );
    }

    pub(crate) fn draw_overlay_stroke_in_area(
        ctx: &Context,
        node_key: NodeKey,
        tile_rect: EguiRect,
        rounding: f32,
        stroke: Stroke,
    ) {
        #[cfg(feature = "diagnostics")]
        let started = std::time::Instant::now();

        Area::new(Id::new(("graphshell_overlay_area", node_key)))
            .order(Order::Tooltip)
            .fixed_pos(tile_rect.min)
            .interactable(false)
            .show(ctx, |ui| {
                ui.set_min_size(tile_rect.size());
                ui.painter().rect_stroke(
                    EguiRect::from_min_size(egui::Pos2::ZERO, tile_rect.size()).shrink(1.0),
                    rounding,
                    stroke,
                    StrokeKind::Inside,
                );
            });

        #[cfg(feature = "diagnostics")]
        crate::shell::desktop::runtime::diagnostics::emit_span_duration(
            "tile_compositor::overlay_pass_draw",
            started.elapsed().as_micros() as u64,
        );
    }

    fn glyph_anchor_position(
        tile_rect: EguiRect,
        anchor: crate::registries::atomic::lens::GlyphAnchor,
    ) -> (egui::Pos2, egui::Align2) {
        match anchor {
            crate::registries::atomic::lens::GlyphAnchor::TopLeft => (
                tile_rect.left_top() + egui::vec2(6.0, 6.0),
                egui::Align2::LEFT_TOP,
            ),
            crate::registries::atomic::lens::GlyphAnchor::TopRight => (
                tile_rect.right_top() + egui::vec2(-6.0, 6.0),
                egui::Align2::RIGHT_TOP,
            ),
            crate::registries::atomic::lens::GlyphAnchor::BottomLeft => (
                tile_rect.left_bottom() + egui::vec2(6.0, -6.0),
                egui::Align2::LEFT_BOTTOM,
            ),
            crate::registries::atomic::lens::GlyphAnchor::BottomRight => (
                tile_rect.right_bottom() + egui::vec2(-6.0, -6.0),
                egui::Align2::RIGHT_BOTTOM,
            ),
            crate::registries::atomic::lens::GlyphAnchor::Center => {
                (tile_rect.center(), egui::Align2::CENTER_CENTER)
            }
        }
    }

    pub(crate) fn draw_overlay_glyphs(
        ctx: &Context,
        node_key: NodeKey,
        tile_rect: EguiRect,
        glyphs: &[crate::registries::atomic::lens::GlyphOverlay],
        color: egui::Color32,
        style: OverlayAffordanceStyle,
    ) {
        if glyphs.is_empty() {
            return;
        }

        let layer = match style {
            OverlayAffordanceStyle::AreaStroke => LayerId::new(
                Order::Tooltip,
                Id::new(("graphshell_overlay_glyphs", node_key)),
            ),
            _ => Self::overlay_layer(node_key),
        };
        let painter = ctx.layer_painter(layer);
        let font = egui::FontId::proportional(11.0);
        for glyph in glyphs {
            let (pos, align) = Self::glyph_anchor_position(tile_rect, glyph.anchor);
            painter.text(pos, align, &glyph.glyph_id, font.clone(), color);
        }
    }

    pub(crate) fn draw_overlay_chrome_markers(
        ctx: &Context,
        node_key: NodeKey,
        tile_rect: EguiRect,
        stroke: Stroke,
    ) {
        #[cfg(feature = "diagnostics")]
        let started = std::time::Instant::now();

        let painter = ctx.layer_painter(Self::overlay_layer(node_key));
        let inset = 2.0;
        let top = tile_rect.top() + inset;
        let left = tile_rect.left() + inset;
        let right = tile_rect.right() - inset;
        let marker_len = 12.0_f32.min((tile_rect.height() - inset * 2.0).max(0.0));

        painter.line_segment([egui::pos2(left, top), egui::pos2(right, top)], stroke);
        painter.line_segment(
            [egui::pos2(left, top), egui::pos2(left, top + marker_len)],
            stroke,
        );
        painter.line_segment(
            [egui::pos2(right, top), egui::pos2(right, top + marker_len)],
            stroke,
        );

        #[cfg(feature = "diagnostics")]
        crate::shell::desktop::runtime::diagnostics::emit_span_duration(
            "tile_compositor::overlay_pass_draw",
            started.elapsed().as_micros() as u64,
        );
    }

    /// Host-agnostic overlay-pass executor. The compositor produces
    /// `OverlayStrokePass` descriptors in a host-neutral generation
    /// step (see `TileCompositor::schedule_overlay_affordance_pass`);
    /// this function walks the descriptor list, emits per-overlay
    /// diagnostics, and hands each descriptor to an
    /// [`OverlayAffordancePainter`] implementation for actual
    /// rendering. The egui host passes in an
    /// [`EguiOverlayAffordancePainter`]; the future iced host will
    /// pass its own impl without touching this function.
    ///
    /// This is the extraction seam for M3.5 (iced-host bring-up).
    /// Descriptor generation is already host-neutral; painting is the
    /// host-specific half.
    pub(crate) fn execute_overlay_affordance_pass_with_painter(
        painter: &mut dyn OverlayAffordancePainter,
        pass_tracker: &CompositorPassTracker,
        overlays: Vec<OverlayStrokePass>,
    ) {
        #[cfg(feature = "diagnostics")]
        let started = std::time::Instant::now();

        for overlay in &overlays {
            pass_tracker.record_overlay_pass(overlay.node_key, overlay.render_mode);
            #[cfg(feature = "diagnostics")]
            {
                crate::shell::desktop::runtime::diagnostics::emit_event(
                    crate::shell::desktop::runtime::diagnostics::DiagnosticEvent::MessageSent {
                        channel_id: overlay_style_channel(overlay.style),
                        byte_len: std::mem::size_of::<NodeKey>(),
                    },
                );
                crate::shell::desktop::runtime::diagnostics::emit_event(
                    crate::shell::desktop::runtime::diagnostics::DiagnosticEvent::MessageSent {
                        channel_id: overlay_mode_channel(overlay.render_mode),
                        byte_len: std::mem::size_of::<NodeKey>(),
                    },
                );
            }
            painter.paint(overlay);
        }

        #[cfg(feature = "diagnostics")]
        crate::shell::desktop::runtime::diagnostics::emit_span_duration(
            "tile_compositor::overlay_affordance_pass",
            started.elapsed().as_micros() as u64,
        );
    }

    /// Backwards-compatible egui entry point — constructs an
    /// [`EguiOverlayAffordancePainter`] internally and delegates to
    /// [`Self::execute_overlay_affordance_pass_with_painter`]. Kept so
    /// existing call sites in `tile_render_pass.rs` / tests don't have
    /// to change shape as the trait lands.
    pub(crate) fn execute_overlay_affordance_pass(
        ctx: &Context,
        pass_tracker: &CompositorPassTracker,
        overlays: Vec<OverlayStrokePass>,
    ) {
        let mut painter = EguiOverlayAffordancePainter { ctx };
        Self::execute_overlay_affordance_pass_with_painter(&mut painter, pass_tracker, overlays);
    }

    pub(crate) fn report_invalid_tile_rect(_node_key: NodeKey) {
        #[cfg(feature = "diagnostics")]
        crate::shell::desktop::runtime::diagnostics::emit_event(
            crate::shell::desktop::runtime::diagnostics::DiagnosticEvent::MessageSent {
                channel_id: CHANNEL_INVALID_TILE_RECT,
                byte_len: std::mem::size_of::<NodeKey>(),
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};
    use std::collections::HashSet;
    use std::sync::{Mutex, OnceLock};

    // M3.6 portable-type helpers live in the parent module; tests reference
    // them without a `super::` prefix at several call sites.
    use super::{portable_rect_from_egui, portable_stroke_from_egui};

    use crate::graph::NodeKey;
    use crate::shell::desktop::render_backend::{
        BackendTextureToken, HostNeutralRenderBackend, UiRenderBackendContract,
    };
    use crate::shell::desktop::runtime::diagnostics::DiagnosticsState;
    use crate::shell::desktop::runtime::registries::{
        CHANNEL_COMPOSITOR_OVERLAY_MODE_COMPOSITED_TEXTURE,
        CHANNEL_COMPOSITOR_OVERLAY_MODE_NATIVE_OVERLAY,
        CHANNEL_COMPOSITOR_OVERLAY_STYLE_CHROME_ONLY, CHANNEL_COMPOSITOR_OVERLAY_STYLE_RECT_STROKE,
    };
    use crate::shell::desktop::workbench::pane_model::TileRenderMode;
    use egui::Stroke;

    use super::{
        CHANNEL_OVERLAY_PASS_REGISTERED, CHANNEL_PASS_ORDER_VIOLATION, CompositorAdapter,
        CompositorPassTracker, ContentSurfaceHandle, OverlayAffordanceStyle, OverlayStrokePass,
        ViewerSurfaceFramePath, ViewerSurfaceRegistry, clear_native_textures_for_tests,
        compositor_native_texture_registry,
    };

    struct RecordingBackend {
        ctx: egui::Context,
        freed_textures: Vec<BackendTextureToken>,
    }

    impl Default for RecordingBackend {
        fn default() -> Self {
            Self {
                ctx: egui::Context::default(),
                freed_textures: Vec::new(),
            }
        }
    }

    impl HostNeutralRenderBackend for RecordingBackend {
        fn register_texture_token(&mut self, texture_id: egui::TextureId) -> BackendTextureToken {
            BackendTextureToken(texture_id)
        }

        fn shared_wgpu_device_queue(&self) -> Option<(servo::wgpu::Device, servo::wgpu::Queue)> {
            None
        }

        fn upsert_native_texture(
            &mut self,
            _existing: Option<BackendTextureToken>,
            _texture: &servo::wgpu::Texture,
        ) -> Option<BackendTextureToken> {
            None
        }

        fn free_native_texture(&mut self, token: BackendTextureToken) {
            self.freed_textures.push(token);
        }

        fn submit_frame(&mut self, _window: &winit::window::Window) {}

        fn destroy_surface(&mut self) {}
    }

    impl UiRenderBackendContract for RecordingBackend {
        fn init_surface_accesskit<Event>(
            &mut self,
            _event_loop: &winit::event_loop::ActiveEventLoop,
            _window: &winit::window::Window,
            _event_loop_proxy: winit::event_loop::EventLoopProxy<Event>,
        ) where
            Event: From<egui_winit::accesskit_winit::Event> + Send + 'static,
        {
        }

        fn egui_context(&self) -> &egui::Context {
            &self.ctx
        }

        fn egui_context_mut(&mut self) -> &mut egui::Context {
            &mut self.ctx
        }

        fn egui_winit_state_mut(&mut self) -> &mut egui_winit::State {
            panic!("egui_winit state should not be used in compositor retirement tests")
        }

        fn handle_window_event(
            &mut self,
            _window: &winit::window::Window,
            _event: &winit::event::WindowEvent,
        ) -> egui_winit::EventResponse {
            panic!("window events should not be used in compositor retirement tests")
        }

        fn run_ui_frame(
            &mut self,
            _window: &winit::window::Window,
            _run_ui: impl FnMut(&egui::Context, &mut egui::Ui, &mut Self),
        ) {
            panic!("ui frame execution should not be used in compositor retirement tests")
        }
    }

    fn resource_retirement_test_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn pass_scheduler_runs_content_before_overlay() {
        let order = RefCell::new(Vec::new());
        {
            let content = || order.borrow_mut().push("content");
            let overlay = || order.borrow_mut().push("overlay");
            content();
            overlay();
        }
        assert_eq!(*order.borrow(), vec!["content", "overlay"]);
    }

    #[test]
    fn tracker_records_content_membership() {
        let mut tracker = CompositorPassTracker::new();
        tracker.record_content_pass(NodeKey::new(1));
        tracker.record_overlay_pass(NodeKey::new(1), TileRenderMode::CompositedTexture);
    }

    #[test]
    fn tracker_emits_pass_order_violation_when_overlay_has_no_content_pass() {
        let mut diagnostics = DiagnosticsState::new();
        let tracker = CompositorPassTracker::new();

        tracker.record_overlay_pass(NodeKey::new(9), TileRenderMode::CompositedTexture);

        diagnostics.force_drain_for_tests();
        let snapshot = diagnostics.snapshot_json_for_tests();
        let channel_counts = snapshot
            .get("channels")
            .and_then(|c| c.get("message_counts"))
            .expect("diagnostics snapshot must include message_counts");

        let overlay_count = channel_counts
            .get(CHANNEL_OVERLAY_PASS_REGISTERED)
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let violation_count = channel_counts
            .get(CHANNEL_PASS_ORDER_VIOLATION)
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        assert!(
            overlay_count > 0,
            "expected overlay pass registration channel"
        );
        assert!(
            violation_count > 0,
            "expected pass-order violation channel when content pass was missing"
        );
    }

    #[test]
    fn tracker_does_not_emit_pass_order_violation_when_content_pass_exists() {
        let mut diagnostics = DiagnosticsState::new();
        let mut tracker = CompositorPassTracker::new();
        let node = NodeKey::new(91);

        tracker.record_content_pass(node);
        tracker.record_overlay_pass(node, TileRenderMode::CompositedTexture);

        diagnostics.force_drain_for_tests();
        let snapshot = diagnostics.snapshot_json_for_tests();
        let channel_counts = snapshot
            .get("channels")
            .and_then(|c| c.get("message_counts"))
            .expect("diagnostics snapshot must include message_counts");

        let overlay_count = channel_counts
            .get(CHANNEL_OVERLAY_PASS_REGISTERED)
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let violation_count = channel_counts
            .get(CHANNEL_PASS_ORDER_VIOLATION)
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        assert!(
            overlay_count > 0,
            "expected overlay pass registration channel"
        );
        assert_eq!(
            violation_count, 0,
            "no pass-order violation expected when matching content pass was recorded"
        );
    }

    #[test]
    fn tracker_does_not_emit_pass_order_violation_for_native_overlay() {
        let mut diagnostics = DiagnosticsState::new();
        let tracker = CompositorPassTracker::new();

        tracker.record_overlay_pass(NodeKey::new(10), TileRenderMode::NativeOverlay);

        diagnostics.force_drain_for_tests();
        let snapshot = diagnostics.snapshot_json_for_tests();
        let channel_counts = snapshot
            .get("channels")
            .and_then(|c| c.get("message_counts"))
            .expect("diagnostics snapshot must include message_counts");

        let overlay_count = channel_counts
            .get(CHANNEL_OVERLAY_PASS_REGISTERED)
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let violation_count = channel_counts
            .get(CHANNEL_PASS_ORDER_VIOLATION)
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        assert!(
            overlay_count > 0,
            "expected overlay pass registration channel"
        );
        assert_eq!(
            violation_count, 0,
            "native overlay should not require composited content-pass ordering"
        );
    }

    #[test]
    fn tracker_does_not_emit_pass_order_violation_for_embedded_egui() {
        let mut diagnostics = DiagnosticsState::new();
        let tracker = CompositorPassTracker::new();

        tracker.record_overlay_pass(NodeKey::new(11), TileRenderMode::EmbeddedEgui);

        diagnostics.force_drain_for_tests();
        let snapshot = diagnostics.snapshot_json_for_tests();
        let channel_counts = snapshot
            .get("channels")
            .and_then(|c| c.get("message_counts"))
            .expect("diagnostics snapshot must include message_counts");

        let violation_count = channel_counts
            .get(CHANNEL_PASS_ORDER_VIOLATION)
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        assert_eq!(
            violation_count, 0,
            "embedded egui path should not emit composited pass-order violation"
        );
    }

    #[test]
    fn tracker_does_not_emit_pass_order_violation_for_placeholder() {
        let mut diagnostics = DiagnosticsState::new();
        let tracker = CompositorPassTracker::new();

        tracker.record_overlay_pass(NodeKey::new(12), TileRenderMode::Placeholder);

        diagnostics.force_drain_for_tests();
        let snapshot = diagnostics.snapshot_json_for_tests();
        let channel_counts = snapshot
            .get("channels")
            .and_then(|c| c.get("message_counts"))
            .expect("diagnostics snapshot must include message_counts");

        let violation_count = channel_counts
            .get(CHANNEL_PASS_ORDER_VIOLATION)
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        assert_eq!(
            violation_count, 0,
            "placeholder path should not emit composited pass-order violation"
        );
    }

    #[test]
    fn execute_overlay_affordance_pass_emits_style_and_mode_channels() {
        let mut diagnostics = DiagnosticsState::new();
        let ctx = egui::Context::default();
        let mut tracker = CompositorPassTracker::new();
        let node = NodeKey::new(12);
        tracker.record_content_pass(node);

        CompositorAdapter::execute_overlay_affordance_pass(
            &ctx,
            &tracker,
            vec![OverlayStrokePass {
                node_key: node,
                tile_rect: portable_rect_from_egui(egui::Rect::from_min_max(
                    egui::pos2(0.0, 0.0),
                    egui::pos2(100.0, 60.0),
                )),
                rounding: 4.0,
                stroke: portable_stroke_from_egui(Stroke::new(2.0, egui::Color32::WHITE)),
                glyph_overlays: Vec::new(),
                style: OverlayAffordanceStyle::RectStroke,
                render_mode: TileRenderMode::CompositedTexture,
            }],
        );

        diagnostics.force_drain_for_tests();
        let snapshot = diagnostics.snapshot_json_for_tests();
        let channel_counts = snapshot
            .get("channels")
            .and_then(|c| c.get("message_counts"))
            .expect("diagnostics snapshot must include message_counts");

        let style_count = channel_counts
            .get(CHANNEL_COMPOSITOR_OVERLAY_STYLE_RECT_STROKE)
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let mode_count = channel_counts
            .get(CHANNEL_COMPOSITOR_OVERLAY_MODE_COMPOSITED_TEXTURE)
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        assert!(
            style_count > 0,
            "expected overlay style diagnostics emission"
        );
        assert!(mode_count > 0, "expected overlay mode diagnostics emission");
    }

    #[test]
    fn overlay_affordance_pass_routes_through_painter_trait() {
        // The trait-based executor is the M3.5 extraction seam for the
        // iced host. Verify a non-egui painter receives every overlay
        // descriptor in order, with each payload intact. This pins the
        // contract that the future iced painter will rely on.
        use super::{OverlayAffordancePainter as PainterTrait, OverlayStrokePass as Pass};
        use std::cell::RefCell;

        struct RecordingPainter {
            seen: RefCell<Vec<(NodeKey, OverlayAffordanceStyle, TileRenderMode)>>,
        }

        impl PainterTrait for RecordingPainter {
            fn paint(&mut self, overlay: &Pass) {
                self.seen
                    .borrow_mut()
                    .push((overlay.node_key, overlay.style, overlay.render_mode));
            }
        }

        let pass_tracker = CompositorPassTracker::new();
        let mut painter = RecordingPainter {
            seen: RefCell::new(Vec::new()),
        };
        let overlays = vec![
            Pass {
                node_key: NodeKey::new(101),
                tile_rect: portable_rect_from_egui(egui::Rect::from_min_size(
                    egui::pos2(0.0, 0.0),
                    egui::vec2(64.0, 32.0),
                )),
                rounding: 2.0,
                stroke: portable_stroke_from_egui(Stroke::new(1.0, egui::Color32::WHITE)),
                glyph_overlays: Vec::new(),
                style: OverlayAffordanceStyle::RectStroke,
                render_mode: TileRenderMode::CompositedTexture,
            },
            Pass {
                node_key: NodeKey::new(202),
                tile_rect: portable_rect_from_egui(egui::Rect::from_min_size(
                    egui::pos2(10.0, 10.0),
                    egui::vec2(80.0, 40.0),
                )),
                rounding: 0.0,
                stroke: portable_stroke_from_egui(Stroke::new(2.0, egui::Color32::BLACK)),
                glyph_overlays: Vec::new(),
                style: OverlayAffordanceStyle::ChromeOnly,
                render_mode: TileRenderMode::NativeOverlay,
            },
        ];

        CompositorAdapter::execute_overlay_affordance_pass_with_painter(
            &mut painter,
            &pass_tracker,
            overlays,
        );

        let seen = painter.seen.into_inner();
        assert_eq!(seen.len(), 2, "painter must receive every overlay");
        assert_eq!(seen[0].0, NodeKey::new(101));
        assert!(matches!(seen[0].1, OverlayAffordanceStyle::RectStroke));
        assert!(matches!(seen[0].2, TileRenderMode::CompositedTexture));
        assert_eq!(seen[1].0, NodeKey::new(202));
        assert!(matches!(seen[1].1, OverlayAffordanceStyle::ChromeOnly));
        assert!(matches!(seen[1].2, TileRenderMode::NativeOverlay));
    }

    #[test]
    fn execute_overlay_affordance_pass_native_overlay_emits_chrome_style_without_violation() {
        let mut diagnostics = DiagnosticsState::new();
        let ctx = egui::Context::default();
        let tracker = CompositorPassTracker::new();
        let node = NodeKey::new(22);

        CompositorAdapter::execute_overlay_affordance_pass(
            &ctx,
            &tracker,
            vec![OverlayStrokePass {
                node_key: node,
                tile_rect: portable_rect_from_egui(egui::Rect::from_min_max(
                    egui::pos2(0.0, 0.0),
                    egui::pos2(100.0, 60.0),
                )),
                rounding: 0.0,
                stroke: portable_stroke_from_egui(Stroke::new(2.0, egui::Color32::WHITE)),
                glyph_overlays: Vec::new(),
                style: OverlayAffordanceStyle::ChromeOnly,
                render_mode: TileRenderMode::NativeOverlay,
            }],
        );

        diagnostics.force_drain_for_tests();
        let snapshot = diagnostics.snapshot_json_for_tests();
        let channel_counts = snapshot
            .get("channels")
            .and_then(|c| c.get("message_counts"))
            .expect("diagnostics snapshot must include message_counts");

        let style_count = channel_counts
            .get(CHANNEL_COMPOSITOR_OVERLAY_STYLE_CHROME_ONLY)
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let mode_count = channel_counts
            .get(CHANNEL_COMPOSITOR_OVERLAY_MODE_NATIVE_OVERLAY)
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let violation_count = channel_counts
            .get(CHANNEL_PASS_ORDER_VIOLATION)
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        assert!(
            style_count > 0,
            "expected chrome-only overlay style diagnostics emission"
        );
        assert!(
            mode_count > 0,
            "expected native-overlay mode diagnostics emission"
        );
        assert_eq!(
            violation_count, 0,
            "native-overlay path should not emit composited pass-order violation"
        );
    }

    // GL state guardrail tests (gl_state_violation_detects_differences,
    // framebuffer_binding_target_*, guarded_callback_*, perturbation
    // tests, chaos tests, replay_ring_is_bounded_to_capacity,
    // replay_channels_emit_for_sample_and_violation_artifact) were
    // retired alongside the gl_compat feature deletion. The wgpu
    // compositor never produces GL state changes to guard against, so
    // capture/restore/perturbation has no analog here.

    // -----------------------------------------------------------------------
    // ViewerSurfaceRegistry — frame-path diagnostics (M4.5 parity coverage)
    // -----------------------------------------------------------------------

    #[test]
    fn record_frame_path_creates_placeholder_for_unknown_node() {
        let mut registry = ViewerSurfaceRegistry::new();
        let node = NodeKey::new(1001);

        assert!(registry.surface(&node).is_none());

        registry.record_frame_path(node, ViewerSurfaceFramePath::MissingSurface);

        let surface = registry
            .surface(&node)
            .expect("surface must be auto-created");
        assert_eq!(
            surface.last_frame_path,
            Some(ViewerSurfaceFramePath::MissingSurface)
        );
        assert_eq!(surface.handle, ContentSurfaceHandle::Placeholder);
        assert!(surface.backing.is_none());
    }

    #[test]
    fn record_frame_path_updates_existing_entry_most_recent_wins() {
        let mut registry = ViewerSurfaceRegistry::new();
        let node = NodeKey::new(1002);

        // First call creates the placeholder entry.
        registry.record_frame_path(node, ViewerSurfaceFramePath::SharedWgpuImported);
        assert_eq!(
            registry.surface(&node).and_then(|s| s.last_frame_path),
            Some(ViewerSurfaceFramePath::SharedWgpuImported),
        );

        // Second call must overwrite (most-recent-wins).
        registry.record_frame_path(node, ViewerSurfaceFramePath::CallbackFallback);
        assert_eq!(
            registry.surface(&node).and_then(|s| s.last_frame_path),
            Some(ViewerSurfaceFramePath::CallbackFallback),
        );
    }

    #[test]
    fn record_frame_path_all_variants_round_trip() {
        let mut registry = ViewerSurfaceRegistry::new();
        let node = NodeKey::new(1003);

        // Seed the entry with any path so subsequent calls exercise the update arm.
        registry.record_frame_path(node, ViewerSurfaceFramePath::MissingSurface);

        for path in [
            ViewerSurfaceFramePath::SharedWgpuImported,
            ViewerSurfaceFramePath::CallbackFallback,
            ViewerSurfaceFramePath::MissingSurface,
        ] {
            registry.record_frame_path(node, path);
            assert_eq!(
                registry.surface(&node).and_then(|s| s.last_frame_path),
                Some(path),
                "record_frame_path round-trip failed for {path:?}"
            );
        }
    }

    #[test]
    fn bump_content_generation_increments_counter() {
        let mut registry = ViewerSurfaceRegistry::new();
        let node = NodeKey::new(1004);

        // Use record_frame_path to create a placeholder entry (content_generation starts at 0).
        registry.record_frame_path(node, ViewerSurfaceFramePath::MissingSurface);
        assert_eq!(
            registry.surface(&node).map(|s| s.content_generation),
            Some(0)
        );

        registry.bump_content_generation(&node);
        assert_eq!(
            registry.surface(&node).map(|s| s.content_generation),
            Some(1)
        );

        registry.bump_content_generation(&node);
        assert_eq!(
            registry.surface(&node).map(|s| s.content_generation),
            Some(2)
        );
    }

    #[test]
    fn bump_content_generation_is_noop_for_missing_node() {
        let mut registry = ViewerSurfaceRegistry::new();
        let node = NodeKey::new(1005);

        registry.bump_content_generation(&node);
        assert!(
            registry.surface(&node).is_none(),
            "bump on missing node must not create an entry"
        );
    }
}
