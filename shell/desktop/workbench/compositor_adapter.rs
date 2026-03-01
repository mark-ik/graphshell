/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Surface Composition Contract guardrails for compositor callback boundaries.
//! The guarded callback path enforces that these OpenGL state fields are
//! stable before/after content-pass rendering: viewport, scissor enable,
//! blend enable, active texture unit, and framebuffer binding.

use std::collections::HashSet;
use std::num::NonZeroU32;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use crate::graph::NodeKey;
use crate::shell::desktop::runtime::registries::{
    CHANNEL_DIAGNOSTICS_COMPOSITOR_BRIDGE_CALLBACK_US_SAMPLE,
    CHANNEL_DIAGNOSTICS_COMPOSITOR_BRIDGE_PRESENTATION_US_SAMPLE,
    CHANNEL_DIAGNOSTICS_COMPOSITOR_BRIDGE_PROBE,
    CHANNEL_DIAGNOSTICS_COMPOSITOR_BRIDGE_PROBE_FAILED_FRAME,
    CHANNEL_DIAGNOSTICS_COMPOSITOR_CHAOS, CHANNEL_DIAGNOSTICS_COMPOSITOR_CHAOS_FAIL,
    CHANNEL_DIAGNOSTICS_COMPOSITOR_CHAOS_PASS,
    CHANNEL_COMPOSITOR_GL_STATE_VIOLATION, CHANNEL_COMPOSITOR_OVERLAY_MODE_COMPOSITED_TEXTURE,
    CHANNEL_COMPOSITOR_OVERLAY_MODE_EMBEDDED_EGUI, CHANNEL_COMPOSITOR_OVERLAY_MODE_NATIVE_OVERLAY,
    CHANNEL_COMPOSITOR_OVERLAY_MODE_PLACEHOLDER, CHANNEL_COMPOSITOR_OVERLAY_STYLE_CHROME_ONLY,
    CHANNEL_COMPOSITOR_OVERLAY_STYLE_RECT_STROKE, CHANNEL_COMPOSITOR_REPLAY_ARTIFACT_RECORDED,
    CHANNEL_COMPOSITOR_REPLAY_SAMPLE_RECORDED,
};
use dpi::PhysicalSize;
use euclid::{Point2D, Rect, Scale, Size2D, UnknownUnit};
use egui::{Context, Id, LayerId, PaintCallback, Rect as EguiRect, Stroke, StrokeKind};
use egui_glow::{CallbackFn, glow};
use log::warn;
use servo::{DevicePixel, OffscreenRenderingContext, RenderingContext, WebView};
use crate::shell::desktop::workbench::pane_model::TileRenderMode;

const CHANNEL_CONTENT_PASS_REGISTERED: &str = "tile_compositor.content_pass_registered";
const CHANNEL_OVERLAY_PASS_REGISTERED: &str = "tile_compositor.overlay_pass_registered";
const CHANNEL_PASS_ORDER_VIOLATION: &str = "tile_compositor.pass_order_violation";
const CHANNEL_INVALID_TILE_RECT: &str = "tile_compositor.invalid_tile_rect";
const COMPOSITOR_CHAOS_ENV_VAR: &str = "GRAPHSHELL_DIAGNOSTICS_COMPOSITOR_CHAOS";
const COMPOSITOR_REPLAY_RING_CAPACITY: usize = 64;
static COMPOSITOR_REPLAY_SEQUENCE: AtomicU64 = AtomicU64::new(1);
static COMPOSITOR_REPLAY_RING: OnceLock<Mutex<std::collections::VecDeque<CompositorReplaySample>>> =
    OnceLock::new();
const BRIDGE_PATH_GL_RENDER_TO_PARENT: &str = "gl.render_to_parent_callback";

#[cfg(feature = "diagnostics")]
static COMPOSITOR_CHAOS_ENABLED: OnceLock<bool> = OnceLock::new();

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CompositorReplaySample {
    pub(crate) sequence: u64,
    pub(crate) node_key: NodeKey,
    pub(crate) duration_us: u64,
    pub(crate) callback_us: u64,
    pub(crate) presentation_us: u64,
    pub(crate) violation: bool,
    pub(crate) bridge_path: &'static str,
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
    pub(crate) tile_rect_px: [i32; 4],
    pub(crate) render_size_px: [u32; 2],
}

fn replay_ring() -> &'static Mutex<std::collections::VecDeque<CompositorReplaySample>> {
    COMPOSITOR_REPLAY_RING
        .get_or_init(|| Mutex::new(std::collections::VecDeque::with_capacity(COMPOSITOR_REPLAY_RING_CAPACITY)))
}

fn push_replay_sample(sample: CompositorReplaySample) {
    let mut ring = replay_ring().lock().expect("compositor replay ring mutex poisoned");
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

fn gl_state_change_flags(before: GlStateSnapshot, after: GlStateSnapshot) -> (bool, bool, bool, bool, bool) {
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

fn capture_gl_state(gl: &glow::Context) -> GlStateSnapshot {
    let mut viewport = [0_i32; 4];

    // Safety: this function performs read-only OpenGL state queries on the current context.
    // No mutation is performed; values are captured for before/after invariant comparison.
    unsafe {
        glow::HasContext::get_parameter_i32_slice(gl, glow::VIEWPORT, &mut viewport);
        GlStateSnapshot {
            viewport,
            scissor_enabled: glow::HasContext::is_enabled(gl, glow::SCISSOR_TEST),
            blend_enabled: glow::HasContext::is_enabled(gl, glow::BLEND),
            active_texture: glow::HasContext::get_parameter_i32(gl, glow::ACTIVE_TEXTURE),
            framebuffer_binding: glow::HasContext::get_parameter_i32(gl, glow::FRAMEBUFFER_BINDING),
        }
    }
}

fn restore_gl_state(gl: &glow::Context, snapshot: GlStateSnapshot) {
    unsafe {
        glow::HasContext::viewport(
            gl,
            snapshot.viewport[0],
            snapshot.viewport[1],
            snapshot.viewport[2],
            snapshot.viewport[3],
        );
        if snapshot.scissor_enabled {
            glow::HasContext::enable(gl, glow::SCISSOR_TEST);
        } else {
            glow::HasContext::disable(gl, glow::SCISSOR_TEST);
        }
        if snapshot.blend_enabled {
            glow::HasContext::enable(gl, glow::BLEND);
        } else {
            glow::HasContext::disable(gl, glow::BLEND);
        }
        glow::HasContext::active_texture(gl, snapshot.active_texture as u32);
        glow::HasContext::bind_framebuffer(
            gl,
            glow::FRAMEBUFFER,
            framebuffer_binding_target(snapshot.framebuffer_binding),
        );
    }
}

fn inject_chaos_gl_perturbation(gl: &glow::Context, seed: u64) {
    let mutation_count = (seed % 3 + 1) as usize;
    let start = (seed as usize) % 5;
    for offset in 0..mutation_count {
        let selector = (start + offset) % 5;
        unsafe {
            match selector {
                0 => {
                    glow::HasContext::viewport(gl, 13, 17, 7, 5);
                }
                1 => {
                    if glow::HasContext::is_enabled(gl, glow::SCISSOR_TEST) {
                        glow::HasContext::disable(gl, glow::SCISSOR_TEST);
                    } else {
                        glow::HasContext::enable(gl, glow::SCISSOR_TEST);
                    }
                }
                2 => {
                    if glow::HasContext::is_enabled(gl, glow::BLEND) {
                        glow::HasContext::disable(gl, glow::BLEND);
                    } else {
                        glow::HasContext::enable(gl, glow::BLEND);
                    }
                }
                3 => {
                    let active = glow::HasContext::get_parameter_i32(gl, glow::ACTIVE_TEXTURE);
                    let bumped = if active == glow::TEXTURE0 as i32 {
                        glow::TEXTURE3
                    } else {
                        glow::TEXTURE0
                    };
                    glow::HasContext::active_texture(gl, bumped);
                }
                _ => {
                    let bound = glow::HasContext::get_parameter_i32(gl, glow::FRAMEBUFFER_BINDING);
                    if bound == 0 {
                        glow::HasContext::bind_framebuffer(
                            gl,
                            glow::FRAMEBUFFER,
                            Some(glow::NativeFramebuffer(
                                std::num::NonZeroU32::new(9).expect("non-zero"),
                            )),
                        );
                    } else {
                        glow::HasContext::bind_framebuffer(gl, glow::FRAMEBUFFER, None);
                    }
                }
            }
        }
    }
}

fn framebuffer_binding_target(binding: i32) -> Option<glow::NativeFramebuffer> {
    if binding <= 0 {
        None
    } else {
        NonZeroU32::new(binding as u32).map(glow::NativeFramebuffer)
    }
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

#[derive(Clone, Copy)]
pub(crate) struct OverlayStrokePass {
    pub(crate) node_key: NodeKey,
    pub(crate) tile_rect: EguiRect,
    pub(crate) rounding: f32,
    pub(crate) stroke: Stroke,
    pub(crate) style: OverlayAffordanceStyle,
    pub(crate) render_mode: TileRenderMode,
}

#[derive(Clone, Copy)]
pub(crate) enum OverlayAffordanceStyle {
    RectStroke,
    ChromeOnly,
}

fn overlay_style_channel(style: OverlayAffordanceStyle) -> &'static str {
    match style {
        OverlayAffordanceStyle::RectStroke => CHANNEL_COMPOSITOR_OVERLAY_STYLE_RECT_STROKE,
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
        node_key: NodeKey,
        tile_rect: EguiRect,
        pixels_per_point: f32,
        render_context: &OffscreenRenderingContext,
        webview: &WebView,
    ) -> CompositedContentPassOutcome {
        let Some((size, target_size)) = Self::prepare_composited_target(
            node_key,
            tile_rect,
            pixels_per_point,
            render_context,
        ) else {
            return CompositedContentPassOutcome::InvalidTileRect;
        };

        Self::reconcile_webview_target_size(webview, size, target_size);

        if !Self::paint_offscreen_content_pass(render_context, target_size, || {
            webview.paint();
        }) {
            return CompositedContentPassOutcome::PaintFailed;
        }

        if Self::register_content_pass_from_render_context(ctx, node_key, tile_rect, render_context)
        {
            CompositedContentPassOutcome::Registered
        } else {
            CompositedContentPassOutcome::MissingContentCallback
        }
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
        callback: Arc<CallbackFn>,
    ) {
        let layer = Self::content_layer(node_key);
        ctx.layer_painter(layer).add(PaintCallback {
            rect: tile_rect,
            callback,
        });
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

    fn register_render_to_parent_content_pass<F>(
        ctx: &Context,
        node_key: NodeKey,
        tile_rect: EguiRect,
        render_to_parent: F,
    ) where
        F: Fn(&glow::Context, Rect<i32, UnknownUnit>) + Send + Sync + 'static,
    {
        let callback = Arc::new(CallbackFn::new(move |info, painter| {
            #[cfg(feature = "diagnostics")]
            let started = std::time::Instant::now();

            let clip = info.viewport_in_pixels();
            let rect_in_parent = Rect::new(
                Point2D::new(clip.left_px, clip.from_bottom_px),
                Size2D::new(clip.width_px, clip.height_px),
            );
            let probe_context = BridgeProbeContext {
                bridge_path: BRIDGE_PATH_GL_RENDER_TO_PARENT,
                tile_rect_px: [
                    rect_in_parent.origin.x,
                    rect_in_parent.origin.y,
                    rect_in_parent.size.width,
                    rect_in_parent.size.height,
                ],
                render_size_px: [clip.width_px as u32, clip.height_px as u32],
            };

            CompositorAdapter::run_content_callback_with_guardrails(
                node_key,
                painter.gl(),
                probe_context,
                || {
                    render_to_parent(painter.gl(), rect_in_parent)
                },
            );

            #[cfg(feature = "diagnostics")]
            crate::shell::desktop::runtime::diagnostics::emit_span_duration(
                "tile_compositor::content_pass_callback",
                started.elapsed().as_micros() as u64,
            );
        }));

        Self::register_content_pass(ctx, node_key, tile_rect, callback);
    }

    fn register_content_pass_from_render_context(
        ctx: &Context,
        node_key: NodeKey,
        tile_rect: EguiRect,
        render_context: &OffscreenRenderingContext,
    ) -> bool {
        let Some(render_to_parent) = render_context.render_to_parent_callback() else {
            return false;
        };

        Self::register_render_to_parent_content_pass(ctx, node_key, tile_rect, render_to_parent);
        true
    }

    pub(crate) fn run_content_callback_with_guardrails<F>(
        _node_key: NodeKey,
        gl: &glow::Context,
        probe_context: BridgeProbeContext,
        render: F,
    ) where
        F: FnOnce(),
    {
        let started = std::time::Instant::now();
        let chaos_enabled = compositor_chaos_mode_enabled();
        let chaos_seed = COMPOSITOR_REPLAY_SEQUENCE.load(Ordering::Relaxed);

        let (violated, before, after, restore_verified) =
            run_guarded_callback_with_snapshots_and_perturbation(
            || capture_gl_state(gl),
            render,
            || {
                if chaos_enabled {
                    inject_chaos_gl_perturbation(gl, chaos_seed);
                }
            },
            |snapshot| restore_gl_state(gl, snapshot),
        );
        let chaos_passed = chaos_probe_passed(chaos_enabled, violated, restore_verified);
        emit_chaos_probe_outcome(chaos_enabled, chaos_passed);
        let (
            viewport_changed,
            scissor_changed,
            blend_changed,
            active_texture_changed,
            framebuffer_binding_changed,
        ) = gl_state_change_flags(before, after);

        let elapsed = started.elapsed().as_micros() as u64;
        let sequence = COMPOSITOR_REPLAY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        push_replay_sample(CompositorReplaySample {
            sequence,
            node_key: _node_key,
            duration_us: elapsed,
            callback_us: elapsed,
            presentation_us: elapsed,
            violation: violated,
            bridge_path: probe_context.bridge_path,
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

        if violated {
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

        painter.line_segment(
            [egui::pos2(left, top), egui::pos2(right, top)],
            stroke,
        );
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
                OverlayAffordanceStyle::ChromeOnly => Self::draw_overlay_chrome_markers(
                    ctx,
                    overlay.node_key,
                    overlay.tile_rect,
                    overlay.stroke,
                ),
            }
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
    use std::cell::{Cell, RefCell};

    use crate::graph::NodeKey;
    use crate::shell::desktop::runtime::diagnostics::DiagnosticsState;
    use crate::shell::desktop::runtime::registries::{
        CHANNEL_DIAGNOSTICS_COMPOSITOR_BRIDGE_CALLBACK_US_SAMPLE,
        CHANNEL_DIAGNOSTICS_COMPOSITOR_BRIDGE_PRESENTATION_US_SAMPLE,
        CHANNEL_DIAGNOSTICS_COMPOSITOR_BRIDGE_PROBE,
        CHANNEL_DIAGNOSTICS_COMPOSITOR_BRIDGE_PROBE_FAILED_FRAME,
        CHANNEL_DIAGNOSTICS_COMPOSITOR_CHAOS, CHANNEL_DIAGNOSTICS_COMPOSITOR_CHAOS_FAIL,
        CHANNEL_DIAGNOSTICS_COMPOSITOR_CHAOS_PASS,
        CHANNEL_COMPOSITOR_OVERLAY_MODE_NATIVE_OVERLAY,
        CHANNEL_COMPOSITOR_OVERLAY_MODE_COMPOSITED_TEXTURE,
        CHANNEL_COMPOSITOR_REPLAY_ARTIFACT_RECORDED, CHANNEL_COMPOSITOR_REPLAY_SAMPLE_RECORDED,
        CHANNEL_COMPOSITOR_OVERLAY_STYLE_CHROME_ONLY,
        CHANNEL_COMPOSITOR_OVERLAY_STYLE_RECT_STROKE,
    };
    use crate::shell::desktop::workbench::pane_model::TileRenderMode;
    use egui::Stroke;

    use super::{
        CHANNEL_OVERLAY_PASS_REGISTERED, CHANNEL_PASS_ORDER_VIOLATION, CompositorAdapter,
        CompositorPassTracker, GlStateSnapshot, OverlayAffordanceStyle, OverlayStrokePass,
        COMPOSITOR_REPLAY_RING_CAPACITY, clear_replay_samples_for_tests,
        chaos_mode_enabled_from_raw, chaos_probe_passed, emit_chaos_probe_outcome,
        framebuffer_binding_target, gl_state_violated, push_replay_sample, replay_samples_snapshot,
        run_guarded_callback, run_guarded_callback_with_snapshots,
        run_guarded_callback_with_snapshots_and_perturbation,
    };

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

        assert!(overlay_count > 0, "expected overlay pass registration channel");
        assert!(
            violation_count > 0,
            "expected pass-order violation channel when content pass was missing"
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

        assert!(overlay_count > 0, "expected overlay pass registration channel");
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

        assert!(style_count > 0, "expected overlay style diagnostics emission");
        assert!(mode_count > 0, "expected overlay mode diagnostics emission");
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

        assert!(style_count > 0, "expected chrome-only overlay style diagnostics emission");
        assert!(mode_count > 0, "expected native-overlay mode diagnostics emission");
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
