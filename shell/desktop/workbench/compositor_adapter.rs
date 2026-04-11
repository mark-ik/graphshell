/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Surface Composition Contract guardrails for compositor callback boundaries.
//! The guarded callback path enforces that these OpenGL state fields are
//! stable before/after content-pass rendering: viewport, scissor enable,
//! blend enable, active texture unit, and framebuffer binding.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use crate::graph::NodeKey;
use crate::shell::desktop::render_backend::{
    BackendContentBridge, BackendCustomPass, BackendFramebufferHandle, BackendGraphicsContext,
    BackendParentRenderCallback, BackendParentRenderRegionInPixels, BackendTextureToken,
    BackendViewportInPixels,
    backend_active_texture, backend_bind_framebuffer, backend_chaos_alternate_texture_unit,
    backend_chaos_framebuffer_handle, backend_content_bridge_mode_label,
    backend_content_bridge_path, backend_framebuffer_binding, backend_framebuffer_from_binding,
    backend_is_blend_enabled, backend_is_scissor_enabled, backend_primary_texture_unit,
    backend_scissor_box, backend_set_active_texture, backend_set_blend_enabled,
    backend_set_scissor_box, backend_set_scissor_enabled, backend_set_viewport, backend_viewport,
    custom_pass_from_backend_viewport, register_custom_paint_callback,
    select_content_bridge_from_render_context, texture_id_from_token, UiRenderBackendContract,
    UiRenderBackendHandle,
};
use crate::shell::desktop::runtime::registries::{
    CHANNEL_COMPOSITOR_CONTENT_PASS_REGISTERED, CHANNEL_COMPOSITOR_GL_STATE_VIOLATION,
    CHANNEL_COMPOSITOR_INVALID_TILE_RECT, CHANNEL_COMPOSITOR_OVERLAY_MODE_COMPOSITED_TEXTURE,
    CHANNEL_COMPOSITOR_OVERLAY_MODE_EMBEDDED_EGUI, CHANNEL_COMPOSITOR_OVERLAY_MODE_NATIVE_OVERLAY,
    CHANNEL_COMPOSITOR_OVERLAY_MODE_PLACEHOLDER, CHANNEL_COMPOSITOR_OVERLAY_PASS_REGISTERED,
    CHANNEL_COMPOSITOR_OVERLAY_STYLE_CHROME_ONLY, CHANNEL_COMPOSITOR_OVERLAY_STYLE_RECT_STROKE,
    CHANNEL_COMPOSITOR_PASS_ORDER_VIOLATION, CHANNEL_COMPOSITOR_REPLAY_ARTIFACT_RECORDED,
    CHANNEL_COMPOSITOR_REPLAY_SAMPLE_RECORDED,
    CHANNEL_DIAGNOSTICS_COMPOSITOR_BRIDGE_CALLBACK_US_SAMPLE,
    CHANNEL_DIAGNOSTICS_COMPOSITOR_BRIDGE_PRESENTATION_US_SAMPLE,
    CHANNEL_DIAGNOSTICS_COMPOSITOR_BRIDGE_PROBE,
    CHANNEL_DIAGNOSTICS_COMPOSITOR_BRIDGE_PROBE_FAILED_FRAME, CHANNEL_DIAGNOSTICS_COMPOSITOR_CHAOS,
    CHANNEL_DIAGNOSTICS_COMPOSITOR_CHAOS_FAIL, CHANNEL_DIAGNOSTICS_COMPOSITOR_CHAOS_PASS,
};
use crate::shell::desktop::workbench::pane_model::TileRenderMode;
use dpi::PhysicalSize;
use egui::{Area, Context, Id, LayerId, Order, Rect as EguiRect, Stroke, StrokeKind};
use euclid::{Scale, Size2D, UnknownUnit};
use log::warn;
use servo::{DevicePixel, OffscreenRenderingContext, RenderingContext, WebView};

const CHANNEL_CONTENT_PASS_REGISTERED: &str = CHANNEL_COMPOSITOR_CONTENT_PASS_REGISTERED;
const CHANNEL_OVERLAY_PASS_REGISTERED: &str = CHANNEL_COMPOSITOR_OVERLAY_PASS_REGISTERED;
const CHANNEL_PASS_ORDER_VIOLATION: &str = CHANNEL_COMPOSITOR_PASS_ORDER_VIOLATION;
const CHANNEL_INVALID_TILE_RECT: &str = CHANNEL_COMPOSITOR_INVALID_TILE_RECT;
const COMPOSITOR_CHAOS_ENV_VAR: &str = "GRAPHSHELL_DIAGNOSTICS_COMPOSITOR_CHAOS";
const COMPOSITOR_REPLAY_RING_CAPACITY: usize = 64;
static COMPOSITOR_REPLAY_SEQUENCE: AtomicU64 = AtomicU64::new(1);
static COMPOSITOR_REPLAY_RING: OnceLock<Mutex<std::collections::VecDeque<CompositorReplaySample>>> =
    OnceLock::new();
static COMPOSITOR_CONTENT_CALLBACKS: OnceLock<Mutex<HashMap<NodeKey, RegisteredContentCallback>>> =
    OnceLock::new();
static COMPOSITOR_NATIVE_TEXTURES: OnceLock<Mutex<HashMap<NodeKey, BackendTextureToken>>> =
    OnceLock::new();
#[cfg(feature = "diagnostics")]
static COMPOSITOR_CHAOS_ENABLED: OnceLock<bool> = OnceLock::new();

type CompositorContentCallback =
    std::sync::Arc<dyn Fn(&BackendGraphicsContext, BackendViewportInPixels) + Send + Sync>;

#[derive(Clone)]
struct RegisteredContentCallback {
    bridge_path: &'static str,
    bridge_mode: &'static str,
    callback: CompositorContentCallback,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct BridgeProbeContext {
    pub(crate) bridge_path: &'static str,
    pub(crate) bridge_mode: &'static str,
    pub(crate) tile_rect_px: [i32; 4],
    pub(crate) render_size_px: [u32; 2],
}

fn replay_ring() -> &'static Mutex<std::collections::VecDeque<CompositorReplaySample>> {
    COMPOSITOR_REPLAY_RING.get_or_init(|| {
        Mutex::new(std::collections::VecDeque::with_capacity(
            COMPOSITOR_REPLAY_RING_CAPACITY,
        ))
    })
}

fn content_callback_registry() -> &'static Mutex<HashMap<NodeKey, RegisteredContentCallback>> {
    COMPOSITOR_CONTENT_CALLBACKS.get_or_init(|| Mutex::new(HashMap::new()))
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

fn push_replay_sample(sample: CompositorReplaySample) {
    let mut ring = replay_ring()
        .lock()
        .expect("compositor replay ring mutex poisoned");
    if ring.len() >= COMPOSITOR_REPLAY_RING_CAPACITY {
        ring.pop_front();
    }
    ring.push_back(sample);
}

pub(crate) fn replay_samples_snapshot() -> Vec<CompositorReplaySample> {
    replay_ring()
        .lock()
        .expect("compositor replay ring mutex poisoned")
        .iter()
        .copied()
        .collect()
}

#[cfg(test)]
fn clear_replay_samples_for_tests() {
    replay_ring()
        .lock()
        .expect("compositor replay ring mutex poisoned")
        .clear();
    COMPOSITOR_REPLAY_SEQUENCE.store(1, Ordering::Relaxed);
}

#[cfg(test)]
fn clear_content_callbacks_for_tests() {
    content_callback_registry()
        .lock()
        .expect("compositor content callback registry mutex poisoned")
        .clear();
}

#[cfg(test)]
fn record_registered_content_bridge_receipt_for_tests(
    node_key: NodeKey,
) -> Option<CompositorReplaySample> {
    let registered = content_callback_registry()
        .lock()
        .expect("compositor content callback registry mutex poisoned")
        .get(&node_key)
        .cloned()?;

    let state = GlStateSnapshot {
        viewport: [0, 0, 0, 0],
        scissor_enabled: false,
        blend_enabled: false,
        active_texture: 0,
        framebuffer_binding: 0,
    };
    let sample = CompositorReplaySample {
        sequence: COMPOSITOR_REPLAY_SEQUENCE.fetch_add(1, Ordering::Relaxed),
        node_key,
        duration_us: 0,
        callback_us: 0,
        presentation_us: 0,
        violation: false,
        bridge_path: registered.bridge_path,
        bridge_mode: registered.bridge_mode,
        tile_rect_px: [0, 0, 0, 0],
        render_size_px: [0, 0],
        chaos_enabled: false,
        restore_verified: true,
        viewport_changed: false,
        scissor_changed: false,
        blend_changed: false,
        active_texture_changed: false,
        framebuffer_binding_changed: false,
        before: state,
        after: state,
    };
    push_replay_sample(sample);
    Some(sample)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GlStateSnapshot {
    pub(crate) viewport: [i32; 4],
    pub(crate) scissor_enabled: bool,
    pub(crate) blend_enabled: bool,
    pub(crate) active_texture: i32,
    pub(crate) framebuffer_binding: i32,
}

fn gl_state_violated(before: GlStateSnapshot, after: GlStateSnapshot) -> bool {
    before != after
}

fn gl_state_change_flags(
    before: GlStateSnapshot,
    after: GlStateSnapshot,
) -> (bool, bool, bool, bool, bool) {
    (
        before.viewport != after.viewport,
        before.scissor_enabled != after.scissor_enabled,
        before.blend_enabled != after.blend_enabled,
        before.active_texture != after.active_texture,
        before.framebuffer_binding != after.framebuffer_binding,
    )
}

fn chaos_mode_enabled_from_raw(raw: Option<&str>) -> bool {
    raw.map(str::trim)
        .map(str::to_ascii_lowercase)
        .map(|value| matches!(value.as_str(), "1" | "true" | "yes" | "on"))
        .unwrap_or(false)
}

fn compositor_chaos_mode_enabled() -> bool {
    #[cfg(feature = "diagnostics")]
    {
        *COMPOSITOR_CHAOS_ENABLED.get_or_init(|| {
            let raw = std::env::var(COMPOSITOR_CHAOS_ENV_VAR).ok();
            chaos_mode_enabled_from_raw(raw.as_deref())
        })
    }

    #[cfg(not(feature = "diagnostics"))]
    {
        false
    }
}

fn chaos_probe_passed(chaos_enabled: bool, violated: bool, restore_verified: bool) -> bool {
    if !chaos_enabled {
        return true;
    }
    violated && restore_verified
}

fn emit_chaos_probe_outcome(chaos_enabled: bool, passed: bool) {
    if !chaos_enabled {
        return;
    }

    #[cfg(feature = "diagnostics")]
    {
        crate::shell::desktop::runtime::diagnostics::emit_event(
            crate::shell::desktop::runtime::diagnostics::DiagnosticEvent::MessageSent {
                channel_id: CHANNEL_DIAGNOSTICS_COMPOSITOR_CHAOS,
                byte_len: std::mem::size_of::<NodeKey>(),
            },
        );

        let channel_id = if passed {
            CHANNEL_DIAGNOSTICS_COMPOSITOR_CHAOS_PASS
        } else {
            CHANNEL_DIAGNOSTICS_COMPOSITOR_CHAOS_FAIL
        };

        crate::shell::desktop::runtime::diagnostics::emit_event(
            crate::shell::desktop::runtime::diagnostics::DiagnosticEvent::MessageSent {
                channel_id,
                byte_len: std::mem::size_of::<NodeKey>(),
            },
        );
    }
}

fn capture_gl_state(gl: &BackendGraphicsContext) -> GlStateSnapshot {
    GlStateSnapshot {
        viewport: backend_viewport(gl),
        scissor_enabled: backend_is_scissor_enabled(gl),
        blend_enabled: backend_is_blend_enabled(gl),
        active_texture: backend_active_texture(gl),
        framebuffer_binding: backend_framebuffer_binding(gl),
    }
}

fn capture_scissor_box(gl: &BackendGraphicsContext) -> [i32; 4] {
    backend_scissor_box(gl)
}

fn restore_scissor_box(gl: &BackendGraphicsContext, scissor_box: [i32; 4]) {
    backend_set_scissor_box(gl, scissor_box);
}

fn run_render_with_scissor_isolation<F>(gl: &BackendGraphicsContext, render: F)
where
    F: FnOnce(),
{
    let incoming_scissor_enabled = backend_is_scissor_enabled(gl);
    let incoming_scissor_box = capture_scissor_box(gl);

    if incoming_scissor_enabled {
        backend_set_scissor_enabled(gl, false);
    }

    render();

    if incoming_scissor_enabled {
        backend_set_scissor_enabled(gl, true);
        restore_scissor_box(gl, incoming_scissor_box);
    }
}

fn restore_gl_state(gl: &BackendGraphicsContext, snapshot: GlStateSnapshot) {
    backend_set_viewport(gl, snapshot.viewport);
    backend_set_scissor_enabled(gl, snapshot.scissor_enabled);
    backend_set_blend_enabled(gl, snapshot.blend_enabled);
    backend_set_active_texture(gl, snapshot.active_texture as u32);
    backend_bind_framebuffer(gl, framebuffer_binding_target(snapshot.framebuffer_binding));
}

fn inject_chaos_gl_perturbation(gl: &BackendGraphicsContext, seed: u64) {
    let mutation_count = (seed % 3 + 1) as usize;
    let start = (seed as usize) % 5;
    for offset in 0..mutation_count {
        let selector = (start + offset) % 5;
        match selector {
            0 => {
                backend_set_viewport(gl, [13, 17, 7, 5]);
            }
            1 => {
                if backend_is_scissor_enabled(gl) {
                    backend_set_scissor_enabled(gl, false);
                } else {
                    backend_set_scissor_enabled(gl, true);
                }
            }
            2 => {
                if backend_is_blend_enabled(gl) {
                    backend_set_blend_enabled(gl, false);
                } else {
                    backend_set_blend_enabled(gl, true);
                }
            }
            3 => {
                let active = backend_active_texture(gl);
                let bumped = if active == backend_primary_texture_unit() as i32 {
                    backend_chaos_alternate_texture_unit()
                } else {
                    backend_primary_texture_unit()
                };
                backend_set_active_texture(gl, bumped);
            }
            _ => {
                let bound = backend_framebuffer_binding(gl);
                if bound == 0 {
                    backend_bind_framebuffer(gl, Some(backend_chaos_framebuffer_handle()));
                } else {
                    backend_bind_framebuffer(gl, None);
                }
            }
        }
    }
}

fn framebuffer_binding_target(binding: i32) -> Option<BackendFramebufferHandle> {
    backend_framebuffer_from_binding(binding)
}

fn run_guarded_callback<Capture, Render, Restore>(
    capture: Capture,
    render: Render,
    restore: Restore,
) -> bool
where
    Capture: FnMut() -> GlStateSnapshot,
    Render: FnOnce(),
    Restore: FnMut(GlStateSnapshot),
{
    let (violated, _before, _after) = run_guarded_callback_with_snapshots(capture, render, restore);
    violated
}

fn run_guarded_callback_with_snapshots_and_perturbation<Capture, Render, Perturb, Restore>(
    mut capture: Capture,
    render: Render,
    mut perturb: Perturb,
    mut restore: Restore,
) -> (bool, GlStateSnapshot, GlStateSnapshot, bool)
where
    Capture: FnMut() -> GlStateSnapshot,
    Render: FnOnce(),
    Perturb: FnMut(),
    Restore: FnMut(GlStateSnapshot),
{
    let before = capture();
    render();
    perturb();
    let after = capture();
    if gl_state_violated(before, after) {
        restore(before);
        let restored = capture();
        return (true, before, after, restored == before);
    }
    (false, before, after, true)
}

fn run_guarded_callback_with_snapshots<Capture, Render, Restore>(
    mut capture: Capture,
    render: Render,
    mut restore: Restore,
) -> (bool, GlStateSnapshot, GlStateSnapshot)
where
    Capture: FnMut() -> GlStateSnapshot,
    Render: FnOnce(),
    Restore: FnMut(GlStateSnapshot),
{
    let (violated, before, after, _restore_verified) =
        run_guarded_callback_with_snapshots_and_perturbation(
            &mut capture,
            render,
            || {},
            &mut restore,
        );
    (violated, before, after)
}

pub(crate) struct CompositorPassTracker {
    content_pass_nodes: HashSet<NodeKey>,
}

#[derive(Clone)]
pub(crate) struct OverlayStrokePass {
    pub(crate) node_key: NodeKey,
    pub(crate) tile_rect: EguiRect,
    pub(crate) rounding: f32,
    pub(crate) stroke: Stroke,
    pub(crate) glyph_overlays: Vec<crate::registries::atomic::lens::GlyphOverlay>,
    pub(crate) style: OverlayAffordanceStyle,
    pub(crate) render_mode: TileRenderMode,
}

#[derive(Clone, Copy)]
pub(crate) enum OverlayAffordanceStyle {
    RectStroke,
    DashedRectStroke,
    EguiAreaStroke,
    ChromeOnly,
}

fn overlay_style_channel(style: OverlayAffordanceStyle) -> &'static str {
    match style {
        OverlayAffordanceStyle::RectStroke
        | OverlayAffordanceStyle::DashedRectStroke
        | OverlayAffordanceStyle::EguiAreaStroke => CHANNEL_COMPOSITOR_OVERLAY_STYLE_RECT_STROKE,
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
    Registered,
    InvalidTileRect,
    PaintFailed,
    MissingContentCallback,
}

impl CompositorAdapter {
    /// Compose a webview into an offscreen target and register the guarded
    /// content pass callback against the parent painter.
    ///
    /// This keeps callback wiring (`render_to_parent`) and guardrails localized
    /// to the adapter boundary rather than call sites.
    pub(crate) fn compose_webview_content_pass(
        ctx: &Context,
        ui_render_backend: &mut UiRenderBackendHandle,
        node_key: NodeKey,
        tile_rect: EguiRect,
        pixels_per_point: f32,
        render_context: &OffscreenRenderingContext,
        webview: &WebView,
    ) -> CompositedContentPassOutcome {
        let Some((size, target_size)) =
            Self::prepare_composited_target(node_key, tile_rect, pixels_per_point, render_context)
        else {
            return CompositedContentPassOutcome::InvalidTileRect;
        };

        Self::reconcile_webview_target_size(webview, size, target_size);

        if !Self::paint_offscreen_content_pass(render_context, target_size, || {
            webview.paint();
        }) {
            return CompositedContentPassOutcome::PaintFailed;
        }

        if let Some(texture_token) =
            Self::upsert_native_content_texture(node_key, render_context, ui_render_backend)
        {
            Self::unregister_content_callback(node_key);
            Self::paint_native_content_texture(ctx, node_key, tile_rect, texture_token);
            return CompositedContentPassOutcome::Registered;
        }

        if !Self::register_content_callback_from_render_context(node_key, render_context) {
            return CompositedContentPassOutcome::MissingContentCallback;
        }

        Self::compose_registered_content_pass(ctx, node_key, tile_rect)
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

    pub(crate) fn register_content_pass(
        ctx: &Context,
        node_key: NodeKey,
        tile_rect: EguiRect,
        callback: BackendCustomPass,
    ) {
        let layer = Self::content_layer(node_key);
        register_custom_paint_callback(ctx, layer, tile_rect, callback);
    }

    pub(crate) fn register_content_callback(
        node_key: NodeKey,
        bridge_path: &'static str,
        bridge_mode: &'static str,
        callback: CompositorContentCallback,
    ) {
        content_callback_registry()
            .lock()
            .expect("compositor content callback registry mutex poisoned")
            .insert(
                node_key,
                RegisteredContentCallback {
                    bridge_path,
                    bridge_mode,
                    callback,
                },
            );
    }

    pub(crate) fn unregister_content_callback(node_key: NodeKey) -> bool {
        content_callback_registry()
            .lock()
            .expect("compositor content callback registry mutex poisoned")
            .remove(&node_key)
            .is_some()
    }

    pub(crate) fn retire_node_content_resources<B>(
        ui_render_backend: &mut B,
        node_key: NodeKey,
    ) where
        B: UiRenderBackendContract,
    {
        Self::unregister_content_callback(node_key);

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
        let stale_callbacks: HashSet<_> = content_callback_registry()
            .lock()
            .expect("compositor content callback registry mutex poisoned")
            .keys()
            .copied()
            .filter(|node_key| !retained_nodes.contains(node_key))
            .collect();
        let stale_native_textures: HashSet<_> = compositor_native_texture_registry()
            .lock()
            .expect("compositor native texture registry mutex poisoned")
            .keys()
            .copied()
            .filter(|node_key| !retained_nodes.contains(node_key))
            .collect();

        for node_key in stale_callbacks.union(&stale_native_textures).copied() {
            Self::retire_node_content_resources(ui_render_backend, node_key);
        }
    }

    pub(crate) fn compose_registered_content_pass(
        ctx: &Context,
        node_key: NodeKey,
        tile_rect: EguiRect,
    ) -> CompositedContentPassOutcome {
        let Some(callback) = Self::registered_content_pass_callback(node_key) else {
            return CompositedContentPassOutcome::MissingContentCallback;
        };
        Self::register_content_pass(ctx, node_key, tile_rect, callback);
        CompositedContentPassOutcome::Registered
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
        let token = ui_render_backend.upsert_native_texture(existing, &imported_texture)?;
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
        render_context: &OffscreenRenderingContext,
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

        if let Err(e) = render_context.make_current() {
            warn!("Failed to make tile rendering context current: {e:?}");
            return false;
        }

        render_context.prepare_for_rendering();
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

    fn content_callback_from_parent_render(
        node_key: NodeKey,
        bridge_path: &'static str,
        bridge_mode: &'static str,
        render_to_parent: BackendParentRenderCallback,
    ) -> CompositorContentCallback {
        let _ = (node_key, bridge_path, bridge_mode);
        std::sync::Arc::new(move |gl, clip: BackendViewportInPixels| {
            let rect_in_parent = BackendParentRenderRegionInPixels {
                left_px: clip.left_px,
                from_bottom_px: clip.from_bottom_px,
                width_px: clip.width_px,
                height_px: clip.height_px,
            };
            render_to_parent(gl, rect_in_parent);
        })
    }

    fn register_content_callback_from_render_context(
        node_key: NodeKey,
        render_context: &OffscreenRenderingContext,
    ) -> bool {
        let Some(bridge) = select_content_bridge_from_render_context(render_context) else {
            return false;
        };

        let bridge_path = backend_content_bridge_path(bridge.mode);
        let bridge_mode = backend_content_bridge_mode_label(bridge.mode);

        let BackendContentBridge::ParentRenderCallback(callback) = bridge.bridge;

        Self::register_content_callback(
            node_key,
            bridge_path,
            bridge_mode,
            Self::content_callback_from_parent_render(node_key, bridge_path, bridge_mode, callback),
        );
        true
    }

    fn registered_content_pass_callback(node_key: NodeKey) -> Option<BackendCustomPass> {
        let registered = content_callback_registry()
            .lock()
            .expect("compositor content callback registry mutex poisoned")
            .get(&node_key)
            .cloned()?;

        Some(custom_pass_from_backend_viewport(
            move |gl, clip: BackendViewportInPixels| {
                #[cfg(feature = "diagnostics")]
                let started = std::time::Instant::now();

                let probe_context = BridgeProbeContext {
                    bridge_path: registered.bridge_path,
                    bridge_mode: registered.bridge_mode,
                    tile_rect_px: [
                        clip.left_px,
                        clip.from_bottom_px,
                        clip.width_px,
                        clip.height_px,
                    ],
                    render_size_px: [clip.width_px as u32, clip.height_px as u32],
                };

                CompositorAdapter::run_content_callback_with_guardrails(
                    node_key,
                    gl,
                    probe_context,
                    || (registered.callback)(gl, clip),
                );

                #[cfg(feature = "diagnostics")]
                crate::shell::desktop::runtime::diagnostics::emit_span_duration(
                    "tile_compositor::content_pass_callback",
                    started.elapsed().as_micros() as u64,
                );
            },
        ))
    }

    pub(crate) fn run_content_callback_with_guardrails<F>(
        _node_key: NodeKey,
        gl: &BackendGraphicsContext,
        probe_context: BridgeProbeContext,
        render: F,
    ) where
        F: FnOnce(),
    {
        let started = std::time::Instant::now();
        let chaos_enabled = compositor_chaos_mode_enabled();
        let chaos_seed = COMPOSITOR_REPLAY_SEQUENCE.load(Ordering::Relaxed);
        let scissor_box_before = capture_scissor_box(gl);

        let (violated, before, after, restore_verified) =
            run_guarded_callback_with_snapshots_and_perturbation(
                || capture_gl_state(gl),
                || run_render_with_scissor_isolation(gl, render),
                || {
                    if chaos_enabled {
                        inject_chaos_gl_perturbation(gl, chaos_seed);
                    }
                },
                |snapshot| restore_gl_state(gl, snapshot),
            );
        let scissor_box_after = capture_scissor_box(gl);
        let scissor_box_changed = scissor_box_before != scissor_box_after;
        let mut restore_verified = restore_verified;
        if scissor_box_changed {
            restore_scissor_box(gl, scissor_box_before);
            restore_verified = restore_verified && capture_scissor_box(gl) == scissor_box_before;
        }
        let violation_detected = violated || scissor_box_changed;
        let chaos_passed = chaos_probe_passed(chaos_enabled, violation_detected, restore_verified);
        emit_chaos_probe_outcome(chaos_enabled, chaos_passed);
        let (
            viewport_changed,
            scissor_changed,
            blend_changed,
            active_texture_changed,
            framebuffer_binding_changed,
        ) = gl_state_change_flags(before, after);
        let scissor_changed = scissor_changed || scissor_box_changed;

        let elapsed = started.elapsed().as_micros() as u64;
        let sequence = COMPOSITOR_REPLAY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        push_replay_sample(CompositorReplaySample {
            sequence,
            node_key: _node_key,
            duration_us: elapsed,
            callback_us: elapsed,
            presentation_us: elapsed,
            violation: violation_detected,
            bridge_path: probe_context.bridge_path,
            bridge_mode: probe_context.bridge_mode,
            tile_rect_px: probe_context.tile_rect_px,
            render_size_px: probe_context.render_size_px,
            chaos_enabled,
            restore_verified,
            viewport_changed,
            scissor_changed,
            blend_changed,
            active_texture_changed,
            framebuffer_binding_changed,
            before,
            after,
        });

        #[cfg(feature = "diagnostics")]
        crate::shell::desktop::runtime::diagnostics::emit_event(
            crate::shell::desktop::runtime::diagnostics::DiagnosticEvent::MessageSent {
                channel_id: CHANNEL_COMPOSITOR_REPLAY_SAMPLE_RECORDED,
                byte_len: std::mem::size_of::<CompositorReplaySample>(),
            },
        );

        #[cfg(feature = "diagnostics")]
        {
            crate::shell::desktop::runtime::diagnostics::emit_event(
                crate::shell::desktop::runtime::diagnostics::DiagnosticEvent::MessageSent {
                    channel_id: CHANNEL_DIAGNOSTICS_COMPOSITOR_BRIDGE_PROBE,
                    byte_len: std::mem::size_of::<NodeKey>(),
                },
            );

            crate::shell::desktop::runtime::diagnostics::emit_event(
                crate::shell::desktop::runtime::diagnostics::DiagnosticEvent::MessageSent {
                    channel_id: CHANNEL_DIAGNOSTICS_COMPOSITOR_BRIDGE_CALLBACK_US_SAMPLE,
                    byte_len: elapsed as usize,
                },
            );

            crate::shell::desktop::runtime::diagnostics::emit_event(
                crate::shell::desktop::runtime::diagnostics::DiagnosticEvent::MessageSent {
                    channel_id: CHANNEL_DIAGNOSTICS_COMPOSITOR_BRIDGE_PRESENTATION_US_SAMPLE,
                    byte_len: elapsed as usize,
                },
            );
        }

        if violation_detected {
            #[cfg(feature = "diagnostics")]
            crate::shell::desktop::runtime::diagnostics::emit_event(
                crate::shell::desktop::runtime::diagnostics::DiagnosticEvent::MessageSent {
                    channel_id: CHANNEL_COMPOSITOR_GL_STATE_VIOLATION,
                    byte_len: std::mem::size_of::<NodeKey>()
                        + std::mem::size_of::<GlStateSnapshot>(),
                },
            );

            #[cfg(feature = "diagnostics")]
            crate::shell::desktop::runtime::diagnostics::emit_event(
                crate::shell::desktop::runtime::diagnostics::DiagnosticEvent::MessageSent {
                    channel_id: CHANNEL_COMPOSITOR_REPLAY_ARTIFACT_RECORDED,
                    byte_len: std::mem::size_of::<CompositorReplaySample>(),
                },
            );

            #[cfg(feature = "diagnostics")]
            crate::shell::desktop::runtime::diagnostics::emit_event(
                crate::shell::desktop::runtime::diagnostics::DiagnosticEvent::MessageSent {
                    channel_id: CHANNEL_DIAGNOSTICS_COMPOSITOR_BRIDGE_PROBE_FAILED_FRAME,
                    byte_len: std::mem::size_of::<NodeKey>(),
                },
            );
        }

        #[cfg(feature = "diagnostics")]
        crate::shell::desktop::runtime::diagnostics::emit_span_duration(
            "tile_compositor::content_pass_guarded_callback",
            elapsed,
        );
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
            OverlayAffordanceStyle::EguiAreaStroke => LayerId::new(
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

    pub(crate) fn execute_overlay_affordance_pass(
        ctx: &Context,
        pass_tracker: &CompositorPassTracker,
        overlays: Vec<OverlayStrokePass>,
    ) {
        #[cfg(feature = "diagnostics")]
        let started = std::time::Instant::now();

        for overlay in overlays {
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
            match overlay.style {
                OverlayAffordanceStyle::RectStroke => Self::draw_overlay_stroke(
                    ctx,
                    overlay.node_key,
                    overlay.tile_rect,
                    overlay.rounding,
                    overlay.stroke,
                ),
                OverlayAffordanceStyle::DashedRectStroke => Self::draw_dashed_overlay_stroke(
                    ctx,
                    overlay.node_key,
                    overlay.tile_rect,
                    overlay.stroke,
                ),
                OverlayAffordanceStyle::EguiAreaStroke => Self::draw_overlay_stroke_in_area(
                    ctx,
                    overlay.node_key,
                    overlay.tile_rect,
                    overlay.rounding,
                    overlay.stroke,
                ),
                OverlayAffordanceStyle::ChromeOnly => Self::draw_overlay_chrome_markers(
                    ctx,
                    overlay.node_key,
                    overlay.tile_rect,
                    overlay.stroke,
                ),
            }
            Self::draw_overlay_glyphs(
                ctx,
                overlay.node_key,
                overlay.tile_rect,
                &overlay.glyph_overlays,
                overlay.stroke.color,
                overlay.style,
            );
        }

        #[cfg(feature = "diagnostics")]
        crate::shell::desktop::runtime::diagnostics::emit_span_duration(
            "tile_compositor::overlay_affordance_pass",
            started.elapsed().as_micros() as u64,
        );
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
    use std::collections::HashSet;
    use std::cell::{Cell, RefCell};
    use std::sync::{Mutex, OnceLock};

    use crate::graph::NodeKey;
    use crate::shell::desktop::render_backend::{
        BackendContentBridgeCapabilities, BackendParentRenderCallback, BackendTextureToken,
        UiRenderBackendContract,
        BackendParentRenderRegionInPixels, backend_bridge_test_env_lock,
        backend_content_bridge_mode_label, backend_content_bridge_path,
        clear_backend_bridge_env_for_tests, select_backend_content_bridge_with_capabilities,
        set_backend_bridge_mode_env_for_tests, set_backend_bridge_readiness_gate_for_tests,
    };
    use crate::shell::desktop::runtime::diagnostics::DiagnosticsState;
    use crate::shell::desktop::runtime::registries::{
        CHANNEL_COMPOSITOR_OVERLAY_MODE_COMPOSITED_TEXTURE,
        CHANNEL_COMPOSITOR_OVERLAY_MODE_NATIVE_OVERLAY,
        CHANNEL_COMPOSITOR_OVERLAY_STYLE_CHROME_ONLY, CHANNEL_COMPOSITOR_OVERLAY_STYLE_RECT_STROKE,
        CHANNEL_COMPOSITOR_REPLAY_ARTIFACT_RECORDED, CHANNEL_COMPOSITOR_REPLAY_SAMPLE_RECORDED,
        CHANNEL_DIAGNOSTICS_COMPOSITOR_BRIDGE_CALLBACK_US_SAMPLE,
        CHANNEL_DIAGNOSTICS_COMPOSITOR_BRIDGE_PRESENTATION_US_SAMPLE,
        CHANNEL_DIAGNOSTICS_COMPOSITOR_BRIDGE_PROBE,
        CHANNEL_DIAGNOSTICS_COMPOSITOR_BRIDGE_PROBE_FAILED_FRAME,
        CHANNEL_DIAGNOSTICS_COMPOSITOR_CHAOS, CHANNEL_DIAGNOSTICS_COMPOSITOR_CHAOS_FAIL,
        CHANNEL_DIAGNOSTICS_COMPOSITOR_CHAOS_PASS,
    };
    use crate::shell::desktop::workbench::pane_model::TileRenderMode;
    use egui::Stroke;

    use super::{
        CHANNEL_OVERLAY_PASS_REGISTERED, CHANNEL_PASS_ORDER_VIOLATION,
        COMPOSITOR_REPLAY_RING_CAPACITY, CompositedContentPassOutcome, CompositorAdapter,
        CompositorPassTracker, GlStateSnapshot, OverlayAffordanceStyle, OverlayStrokePass,
        chaos_mode_enabled_from_raw, chaos_probe_passed, clear_content_callbacks_for_tests,
        clear_native_textures_for_tests, clear_replay_samples_for_tests,
        compositor_native_texture_registry, content_callback_registry,
        emit_chaos_probe_outcome, framebuffer_binding_target,
        gl_state_change_flags, gl_state_violated, push_replay_sample, replay_samples_snapshot,
        record_registered_content_bridge_receipt_for_tests,
        run_guarded_callback, run_guarded_callback_with_snapshots,
        run_guarded_callback_with_snapshots_and_perturbation,
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
            _run_ui: impl FnMut(&egui::Context, &mut Self),
        ) {
            panic!("ui frame execution should not be used in compositor retirement tests")
        }

        fn register_texture_token(
            &mut self,
            texture_id: egui::TextureId,
        ) -> BackendTextureToken {
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
                tile_rect: egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(100.0, 60.0)),
                rounding: 4.0,
                stroke: Stroke::new(2.0, egui::Color32::WHITE),
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
    fn compose_registered_content_pass_requires_registered_callback() {
        clear_content_callbacks_for_tests();
        clear_native_textures_for_tests();

        let ctx = egui::Context::default();
        let outcome = CompositorAdapter::compose_registered_content_pass(
            &ctx,
            NodeKey::new(901),
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(80.0, 40.0)),
        );

        assert_eq!(
            outcome,
            CompositedContentPassOutcome::MissingContentCallback
        );
    }

    #[test]
    fn synthetic_viewer_can_register_generic_content_callback() {
        clear_content_callbacks_for_tests();
        clear_native_textures_for_tests();

        let ctx = egui::Context::default();
        let node_key = NodeKey::new(902);
        CompositorAdapter::register_content_callback(
            node_key,
            "test.synthetic_viewer",
            "test.synthetic_mode",
            std::sync::Arc::new(|_, _| {}),
        );

        let outcome = CompositorAdapter::compose_registered_content_pass(
            &ctx,
            node_key,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(80.0, 40.0)),
        );

        assert_eq!(outcome, CompositedContentPassOutcome::Registered);
        assert!(CompositorAdapter::unregister_content_callback(node_key));
        assert!(!CompositorAdapter::unregister_content_callback(node_key));
    }

    #[test]
    fn retire_node_content_resources_releases_callback_and_native_texture() {
        let _guard = resource_retirement_test_lock()
            .lock()
            .expect("resource retirement test lock poisoned");
        clear_content_callbacks_for_tests();
        clear_native_textures_for_tests();

        let node_key = NodeKey::new(905);
        let texture_token = BackendTextureToken(egui::TextureId::Managed(77));
        CompositorAdapter::register_content_callback(
            node_key,
            "test.retire_node",
            "test.retire_node_mode",
            std::sync::Arc::new(|_, _| {}),
        );
        compositor_native_texture_registry()
            .lock()
            .expect("compositor native texture registry mutex poisoned")
            .insert(node_key, texture_token);

        let mut backend = RecordingBackend::default();
        CompositorAdapter::retire_node_content_resources(&mut backend, node_key);

        assert!(
            content_callback_registry()
                .lock()
                .expect("compositor content callback registry mutex poisoned")
                .get(&node_key)
                .is_none(),
            "callback should be removed when retiring node resources"
        );
        assert!(
            compositor_native_texture_registry()
                .lock()
                .expect("compositor native texture registry mutex poisoned")
                .get(&node_key)
                .is_none(),
            "native texture should be removed when retiring node resources"
        );
        assert_eq!(backend.freed_textures, vec![texture_token]);
    }

    #[test]
    fn retire_stale_content_resources_only_prunes_unretained_nodes() {
        let _guard = resource_retirement_test_lock()
            .lock()
            .expect("resource retirement test lock poisoned");
        clear_content_callbacks_for_tests();
        clear_native_textures_for_tests();

        let retained_node = NodeKey::new(906);
        let stale_callback_node = NodeKey::new(907);
        let stale_texture_node = NodeKey::new(908);
        let stale_both_node = NodeKey::new(909);

        for node_key in [retained_node, stale_callback_node, stale_both_node] {
            CompositorAdapter::register_content_callback(
                node_key,
                "test.retire_stale",
                "test.retire_stale_mode",
                std::sync::Arc::new(|_, _| {}),
            );
        }

        let retained_texture = BackendTextureToken(egui::TextureId::Managed(101));
        let stale_texture = BackendTextureToken(egui::TextureId::Managed(102));
        let stale_both_texture = BackendTextureToken(egui::TextureId::Managed(103));
        compositor_native_texture_registry()
            .lock()
            .expect("compositor native texture registry mutex poisoned")
            .extend([
                (retained_node, retained_texture),
                (stale_texture_node, stale_texture),
                (stale_both_node, stale_both_texture),
            ]);

        let mut backend = RecordingBackend::default();
        CompositorAdapter::retire_stale_content_resources(
            &mut backend,
            &HashSet::from([retained_node]),
        );

        let callbacks = content_callback_registry()
            .lock()
            .expect("compositor content callback registry mutex poisoned");
        assert!(callbacks.contains_key(&retained_node));
        assert!(!callbacks.contains_key(&stale_callback_node));
        assert!(!callbacks.contains_key(&stale_both_node));
        drop(callbacks);

        let native_textures = compositor_native_texture_registry()
            .lock()
            .expect("compositor native texture registry mutex poisoned");
        assert_eq!(native_textures.get(&retained_node), Some(&retained_texture));
        assert!(!native_textures.contains_key(&stale_texture_node));
        assert!(!native_textures.contains_key(&stale_both_node));
        drop(native_textures);

        assert_eq!(
            backend.freed_textures.iter().copied().collect::<HashSet<_>>(),
            HashSet::from([stale_texture, stale_both_texture]),
            "only stale native textures should be freed"
        );
    }

    #[test]
    fn selected_bridge_metadata_flows_through_registration_into_diagnostics() {
        let _guard = backend_bridge_test_env_lock()
            .lock()
            .expect("env lock poisoned");
        clear_backend_bridge_env_for_tests();
        clear_content_callbacks_for_tests();
        clear_native_textures_for_tests();
        clear_replay_samples_for_tests();
        set_backend_bridge_mode_env_for_tests("wgpu_preferred");
        set_backend_bridge_readiness_gate_for_tests(true);

        let callback: BackendParentRenderCallback =
            std::sync::Arc::new(|_: &_, _: BackendParentRenderRegionInPixels| {});

        let supported = select_backend_content_bridge_with_capabilities(
            callback.clone(),
            BackendContentBridgeCapabilities {
                supports_wgpu_parent_render_bridge: true,
            },
        );
        let supported_node = NodeKey::new(903);
        CompositorAdapter::register_content_callback(
            supported_node,
            backend_content_bridge_path(supported.mode),
            backend_content_bridge_mode_label(supported.mode),
            std::sync::Arc::new(|_, _| {}),
        );
        let supported_sample = record_registered_content_bridge_receipt_for_tests(supported_node)
            .expect("supported bridge receipt should be recorded");

        let unsupported = select_backend_content_bridge_with_capabilities(
            callback,
            BackendContentBridgeCapabilities {
                supports_wgpu_parent_render_bridge: false,
            },
        );
        let unsupported_node = NodeKey::new(904);
        CompositorAdapter::register_content_callback(
            unsupported_node,
            backend_content_bridge_path(unsupported.mode),
            backend_content_bridge_mode_label(unsupported.mode),
            std::sync::Arc::new(|_, _| {}),
        );
        let unsupported_sample = record_registered_content_bridge_receipt_for_tests(
            unsupported_node,
        )
        .expect("fallback bridge receipt should be recorded");

        let supported_payload =
            DiagnosticsState::bridge_spike_measurement_value_from_samples(&[supported_sample]);
        let unsupported_payload =
            DiagnosticsState::bridge_spike_measurement_value_from_samples(&[unsupported_sample]);

        assert_eq!(
            supported_payload["measurement_contract"]["latest"]["bridge_mode"].as_str(),
            Some("wgpu_preferred_fallback_gl_callback")
        );
        assert_eq!(
            supported_payload["measurement_contract"]["latest"]["bridge_path"].as_str(),
            Some("wgpu.preferred.fallback_gl.render_to_parent_callback")
        );
        assert_eq!(
            unsupported_payload["measurement_contract"]["latest"]["bridge_mode"].as_str(),
            Some("gl_callback")
        );
        assert_eq!(
            unsupported_payload["measurement_contract"]["latest"]["bridge_path"].as_str(),
            Some("gl.render_to_parent_callback")
        );

        clear_backend_bridge_env_for_tests();
        clear_content_callbacks_for_tests();
        clear_replay_samples_for_tests();
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
                tile_rect: egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(100.0, 60.0)),
                rounding: 0.0,
                stroke: Stroke::new(2.0, egui::Color32::WHITE),
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

    #[test]
    fn gl_state_violation_detects_differences() {
        let before = GlStateSnapshot {
            viewport: [0, 0, 100, 100],
            scissor_enabled: false,
            blend_enabled: false,
            active_texture: 0,
            framebuffer_binding: 1,
        };
        let after = GlStateSnapshot {
            viewport: [0, 0, 100, 100],
            scissor_enabled: true,
            blend_enabled: false,
            active_texture: 0,
            framebuffer_binding: 1,
        };
        assert!(gl_state_violated(before, after));
        assert!(!gl_state_violated(before, before));
    }

    #[test]
    fn framebuffer_binding_target_returns_none_for_default_framebuffer() {
        assert_eq!(framebuffer_binding_target(0), None);
        assert_eq!(framebuffer_binding_target(-1), None);
    }

    #[test]
    fn framebuffer_binding_target_returns_handle_for_non_default_framebuffer() {
        let target = framebuffer_binding_target(12)
            .expect("non-default framebuffer binding should produce native handle");
        assert_eq!(target.0.get(), 12_u32);
    }

    #[test]
    fn guarded_callback_restores_state_when_callback_leaks() {
        let before = GlStateSnapshot {
            viewport: [0, 0, 100, 100],
            scissor_enabled: false,
            blend_enabled: false,
            active_texture: 0,
            framebuffer_binding: 1,
        };
        let after = GlStateSnapshot {
            viewport: [10, 20, 300, 200],
            scissor_enabled: true,
            blend_enabled: true,
            active_texture: 2,
            framebuffer_binding: 9,
        };

        let state = RefCell::new(before);
        let restored = Cell::new(false);

        let violated = run_guarded_callback(
            || *state.borrow(),
            || {
                *state.borrow_mut() = after;
            },
            |snapshot| {
                *state.borrow_mut() = snapshot;
                restored.set(true);
            },
        );

        assert!(violated);
        assert!(restored.get());
        assert_eq!(*state.borrow(), before);
    }

    #[test]
    fn guarded_callback_skips_restore_when_state_is_unchanged() {
        let before = GlStateSnapshot {
            viewport: [0, 0, 100, 100],
            scissor_enabled: false,
            blend_enabled: false,
            active_texture: 0,
            framebuffer_binding: 1,
        };

        let state = RefCell::new(before);
        let restored = Cell::new(false);

        let violated = run_guarded_callback(
            || *state.borrow(),
            || {},
            |_| {
                restored.set(true);
            },
        );

        assert!(!violated);
        assert!(!restored.get());
        assert_eq!(*state.borrow(), before);
    }

    #[test]
    fn guarded_callback_with_snapshots_returns_before_and_after_states() {
        let before = GlStateSnapshot {
            viewport: [0, 0, 100, 100],
            scissor_enabled: false,
            blend_enabled: false,
            active_texture: 0,
            framebuffer_binding: 1,
        };
        let after = GlStateSnapshot {
            viewport: [0, 0, 110, 90],
            scissor_enabled: true,
            blend_enabled: false,
            active_texture: 0,
            framebuffer_binding: 2,
        };

        let state = RefCell::new(before);
        let (violated, captured_before, captured_after) = run_guarded_callback_with_snapshots(
            || *state.borrow(),
            || {
                *state.borrow_mut() = after;
            },
            |snapshot| {
                *state.borrow_mut() = snapshot;
            },
        );

        assert!(violated);
        assert_eq!(captured_before, before);
        assert_eq!(captured_after, after);
    }

    #[test]
    fn guarded_callback_perturbation_detects_viewport_invariant() {
        let before = GlStateSnapshot {
            viewport: [0, 0, 100, 100],
            scissor_enabled: false,
            blend_enabled: false,
            active_texture: 0,
            framebuffer_binding: 1,
        };

        let state = RefCell::new(before);
        let (violated, _, after, restore_verified) =
            run_guarded_callback_with_snapshots_and_perturbation(
                || *state.borrow(),
                || {},
                || {
                    state.borrow_mut().viewport = [7, 11, 3, 5];
                },
                |snapshot| {
                    *state.borrow_mut() = snapshot;
                },
            );

        assert!(violated);
        assert!(restore_verified);
        assert_ne!(after.viewport, before.viewport);
        assert_eq!(*state.borrow(), before);
    }

    #[test]
    fn guarded_callback_perturbation_detects_scissor_invariant() {
        let before = GlStateSnapshot {
            viewport: [0, 0, 100, 100],
            scissor_enabled: false,
            blend_enabled: false,
            active_texture: 0,
            framebuffer_binding: 1,
        };

        let state = RefCell::new(before);
        let (violated, _, after, restore_verified) =
            run_guarded_callback_with_snapshots_and_perturbation(
                || *state.borrow(),
                || {},
                || {
                    state.borrow_mut().scissor_enabled = true;
                },
                |snapshot| {
                    *state.borrow_mut() = snapshot;
                },
            );

        assert!(violated);
        assert!(restore_verified);
        assert_ne!(after.scissor_enabled, before.scissor_enabled);
        assert_eq!(*state.borrow(), before);
    }

    #[test]
    fn guarded_callback_perturbation_detects_blend_invariant() {
        let before = GlStateSnapshot {
            viewport: [0, 0, 100, 100],
            scissor_enabled: false,
            blend_enabled: false,
            active_texture: 0,
            framebuffer_binding: 1,
        };

        let state = RefCell::new(before);
        let (violated, _, after, restore_verified) =
            run_guarded_callback_with_snapshots_and_perturbation(
                || *state.borrow(),
                || {},
                || {
                    state.borrow_mut().blend_enabled = true;
                },
                |snapshot| {
                    *state.borrow_mut() = snapshot;
                },
            );

        assert!(violated);
        assert!(restore_verified);
        assert_ne!(after.blend_enabled, before.blend_enabled);
        assert_eq!(*state.borrow(), before);
    }

    #[test]
    fn guarded_callback_perturbation_detects_active_texture_invariant() {
        let before = GlStateSnapshot {
            viewport: [0, 0, 100, 100],
            scissor_enabled: false,
            blend_enabled: false,
            active_texture: 0,
            framebuffer_binding: 1,
        };

        let state = RefCell::new(before);
        let (violated, _, after, restore_verified) =
            run_guarded_callback_with_snapshots_and_perturbation(
                || *state.borrow(),
                || {},
                || {
                    state.borrow_mut().active_texture = 3;
                },
                |snapshot| {
                    *state.borrow_mut() = snapshot;
                },
            );

        assert!(violated);
        assert!(restore_verified);
        assert_ne!(after.active_texture, before.active_texture);
        assert_eq!(*state.borrow(), before);
    }

    #[test]
    fn guarded_callback_perturbation_detects_framebuffer_binding_invariant() {
        let before = GlStateSnapshot {
            viewport: [0, 0, 100, 100],
            scissor_enabled: false,
            blend_enabled: false,
            active_texture: 0,
            framebuffer_binding: 1,
        };

        let state = RefCell::new(before);
        let (violated, _, after, restore_verified) =
            run_guarded_callback_with_snapshots_and_perturbation(
                || *state.borrow(),
                || {},
                || {
                    state.borrow_mut().framebuffer_binding = 9;
                },
                |snapshot| {
                    *state.borrow_mut() = snapshot;
                },
            );

        assert!(violated);
        assert!(restore_verified);
        assert_ne!(after.framebuffer_binding, before.framebuffer_binding);
        assert_eq!(*state.borrow(), before);
    }

    #[test]
    fn guarded_mock_callback_detects_all_gl_state_invariants() {
        let before = GlStateSnapshot {
            viewport: [0, 0, 100, 100],
            scissor_enabled: false,
            blend_enabled: false,
            active_texture: 0,
            framebuffer_binding: 1,
        };

        let state = RefCell::new(before);
        let (violated, captured_before, captured_after, restore_verified) =
            run_guarded_callback_with_snapshots_and_perturbation(
                || *state.borrow(),
                || {
                    // Simulate a backend callback that mutates every tracked GL state field.
                    let mut snapshot = *state.borrow();
                    snapshot.viewport = [11, 13, 97, 89];
                    snapshot.scissor_enabled = true;
                    snapshot.blend_enabled = true;
                    snapshot.active_texture = 3;
                    snapshot.framebuffer_binding = 9;
                    *state.borrow_mut() = snapshot;
                },
                || {},
                |snapshot| {
                    *state.borrow_mut() = snapshot;
                },
            );

        assert!(violated);
        assert!(restore_verified);
        assert_eq!(captured_before, before);
        assert_eq!(*state.borrow(), before);

        let (
            viewport_changed,
            scissor_changed,
            blend_changed,
            active_texture_changed,
            framebuffer_binding_changed,
        ) = gl_state_change_flags(captured_before, captured_after);

        assert!(viewport_changed);
        assert!(scissor_changed);
        assert!(blend_changed);
        assert!(active_texture_changed);
        assert!(framebuffer_binding_changed);
    }

    #[test]
    fn compositor_chaos_env_parser_accepts_truthy_values() {
        assert!(chaos_mode_enabled_from_raw(Some("1")));
        assert!(chaos_mode_enabled_from_raw(Some("true")));
        assert!(chaos_mode_enabled_from_raw(Some("ON")));
        assert!(!chaos_mode_enabled_from_raw(Some("0")));
        assert!(!chaos_mode_enabled_from_raw(Some("no")));
        assert!(!chaos_mode_enabled_from_raw(None));
    }

    #[test]
    fn chaos_probe_pass_and_fail_decision_is_explicit() {
        assert!(chaos_probe_passed(false, false, false));
        assert!(chaos_probe_passed(true, true, true));
        assert!(!chaos_probe_passed(true, false, true));
        assert!(!chaos_probe_passed(true, true, false));
    }

    #[test]
    fn chaos_probe_outcome_emits_channels() {
        let mut diagnostics = DiagnosticsState::new();

        emit_chaos_probe_outcome(true, true);
        emit_chaos_probe_outcome(true, false);

        diagnostics.force_drain_for_tests();
        let snapshot = diagnostics.snapshot_json_for_tests();
        let channel_counts = snapshot
            .get("channels")
            .and_then(|c| c.get("message_counts"))
            .expect("diagnostics snapshot must include message_counts");

        assert!(
            channel_counts
                .get(CHANNEL_DIAGNOSTICS_COMPOSITOR_CHAOS)
                .and_then(|v| v.as_u64())
                .unwrap_or(0)
                > 0
        );
        assert!(
            channel_counts
                .get(CHANNEL_DIAGNOSTICS_COMPOSITOR_CHAOS_PASS)
                .and_then(|v| v.as_u64())
                .unwrap_or(0)
                > 0
        );
        assert!(
            channel_counts
                .get(CHANNEL_DIAGNOSTICS_COMPOSITOR_CHAOS_FAIL)
                .and_then(|v| v.as_u64())
                .unwrap_or(0)
                > 0
        );
    }

    #[test]
    fn replay_ring_is_bounded_to_capacity() {
        clear_replay_samples_for_tests();
        let before = GlStateSnapshot {
            viewport: [0, 0, 100, 100],
            scissor_enabled: false,
            blend_enabled: false,
            active_texture: 0,
            framebuffer_binding: 1,
        };
        let after = GlStateSnapshot {
            viewport: [0, 0, 100, 100],
            scissor_enabled: false,
            blend_enabled: false,
            active_texture: 0,
            framebuffer_binding: 1,
        };

        for index in 0..(COMPOSITOR_REPLAY_RING_CAPACITY + 5) {
            push_replay_sample(super::CompositorReplaySample {
                sequence: index as u64 + 1,
                node_key: NodeKey::new(index + 1),
                duration_us: 5,
                callback_us: 5,
                presentation_us: 5,
                violation: false,
                bridge_path: "test.bridge",
                bridge_mode: "test.bridge_mode",
                tile_rect_px: [0, 0, 100, 100],
                render_size_px: [100, 100],
                chaos_enabled: false,
                restore_verified: true,
                viewport_changed: false,
                scissor_changed: false,
                blend_changed: false,
                active_texture_changed: false,
                framebuffer_binding_changed: false,
                before,
                after,
            });
        }

        let snapshot = replay_samples_snapshot();
        assert_eq!(snapshot.len(), COMPOSITOR_REPLAY_RING_CAPACITY);
        assert_eq!(snapshot.first().map(|s| s.sequence), Some(6));
        assert_eq!(
            snapshot.last().map(|s| s.sequence),
            Some((COMPOSITOR_REPLAY_RING_CAPACITY + 5) as u64)
        );
    }

    #[test]
    fn replay_channels_emit_for_sample_and_violation_artifact() {
        let mut diagnostics = DiagnosticsState::new();
        clear_replay_samples_for_tests();
        let before = GlStateSnapshot {
            viewport: [0, 0, 100, 100],
            scissor_enabled: false,
            blend_enabled: false,
            active_texture: 0,
            framebuffer_binding: 1,
        };
        let after = GlStateSnapshot {
            viewport: [0, 0, 100, 100],
            scissor_enabled: true,
            blend_enabled: false,
            active_texture: 0,
            framebuffer_binding: 1,
        };

        crate::shell::desktop::runtime::diagnostics::emit_event(
            crate::shell::desktop::runtime::diagnostics::DiagnosticEvent::MessageSent {
                channel_id: CHANNEL_COMPOSITOR_REPLAY_SAMPLE_RECORDED,
                byte_len: std::mem::size_of_val(&before),
            },
        );
        crate::shell::desktop::runtime::diagnostics::emit_event(
            crate::shell::desktop::runtime::diagnostics::DiagnosticEvent::MessageSent {
                channel_id: CHANNEL_DIAGNOSTICS_COMPOSITOR_BRIDGE_PROBE,
                byte_len: std::mem::size_of::<NodeKey>(),
            },
        );
        crate::shell::desktop::runtime::diagnostics::emit_event(
            crate::shell::desktop::runtime::diagnostics::DiagnosticEvent::MessageSent {
                channel_id: CHANNEL_DIAGNOSTICS_COMPOSITOR_BRIDGE_CALLBACK_US_SAMPLE,
                byte_len: 27,
            },
        );
        crate::shell::desktop::runtime::diagnostics::emit_event(
            crate::shell::desktop::runtime::diagnostics::DiagnosticEvent::MessageSent {
                channel_id: CHANNEL_DIAGNOSTICS_COMPOSITOR_BRIDGE_PRESENTATION_US_SAMPLE,
                byte_len: 31,
            },
        );
        crate::shell::desktop::runtime::diagnostics::emit_event(
            crate::shell::desktop::runtime::diagnostics::DiagnosticEvent::MessageSent {
                channel_id: CHANNEL_COMPOSITOR_REPLAY_ARTIFACT_RECORDED,
                byte_len: std::mem::size_of_val(&after),
            },
        );
        crate::shell::desktop::runtime::diagnostics::emit_event(
            crate::shell::desktop::runtime::diagnostics::DiagnosticEvent::MessageSent {
                channel_id: CHANNEL_DIAGNOSTICS_COMPOSITOR_BRIDGE_PROBE_FAILED_FRAME,
                byte_len: std::mem::size_of::<NodeKey>(),
            },
        );

        diagnostics.force_drain_for_tests();
        let snapshot = diagnostics.snapshot_json_for_tests();
        let channel_counts = snapshot
            .get("channels")
            .and_then(|c| c.get("message_counts"))
            .expect("diagnostics snapshot must include message_counts");

        assert!(
            channel_counts
                .get(CHANNEL_COMPOSITOR_REPLAY_SAMPLE_RECORDED)
                .and_then(|v| v.as_u64())
                .unwrap_or(0)
                > 0
        );
        assert!(
            channel_counts
                .get(CHANNEL_COMPOSITOR_REPLAY_ARTIFACT_RECORDED)
                .and_then(|v| v.as_u64())
                .unwrap_or(0)
                > 0
        );
        assert!(
            channel_counts
                .get(CHANNEL_DIAGNOSTICS_COMPOSITOR_BRIDGE_PROBE)
                .and_then(|v| v.as_u64())
                .unwrap_or(0)
                > 0
        );
        assert!(
            channel_counts
                .get(CHANNEL_DIAGNOSTICS_COMPOSITOR_BRIDGE_CALLBACK_US_SAMPLE)
                .and_then(|v| v.as_u64())
                .unwrap_or(0)
                > 0
        );
        assert!(
            channel_counts
                .get(CHANNEL_DIAGNOSTICS_COMPOSITOR_BRIDGE_PRESENTATION_US_SAMPLE)
                .and_then(|v| v.as_u64())
                .unwrap_or(0)
                > 0
        );
        assert!(
            channel_counts
                .get(CHANNEL_DIAGNOSTICS_COMPOSITOR_BRIDGE_PROBE_FAILED_FRAME)
                .and_then(|v| v.as_u64())
                .unwrap_or(0)
                > 0
        );
    }
}
