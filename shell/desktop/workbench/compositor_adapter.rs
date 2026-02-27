/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Surface Composition Contract guardrails for compositor callback boundaries.
//! The guarded callback path enforces that these OpenGL state fields are
//! stable before/after content-pass rendering: viewport, scissor enable,
//! blend enable, active texture unit, and framebuffer binding.

use std::collections::HashSet;
use std::sync::Arc;

use crate::graph::NodeKey;
use crate::shell::desktop::runtime::registries::{
    CHANNEL_COMPOSITOR_GL_STATE_VIOLATION, CHANNEL_COMPOSITOR_OVERLAY_MODE_COMPOSITED_TEXTURE,
    CHANNEL_COMPOSITOR_OVERLAY_MODE_EMBEDDED_EGUI, CHANNEL_COMPOSITOR_OVERLAY_MODE_NATIVE_OVERLAY,
    CHANNEL_COMPOSITOR_OVERLAY_MODE_PLACEHOLDER, CHANNEL_COMPOSITOR_OVERLAY_STYLE_CHROME_ONLY,
    CHANNEL_COMPOSITOR_OVERLAY_STYLE_RECT_STROKE,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct GlStateSnapshot {
    viewport: [i32; 4],
    scissor_enabled: bool,
    blend_enabled: bool,
    active_texture: i32,
    framebuffer_binding: i32,
}

fn gl_state_violated(before: GlStateSnapshot, after: GlStateSnapshot) -> bool {
    before != after
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
        glow::HasContext::bind_framebuffer(gl, glow::FRAMEBUFFER, None);
    }
}

fn run_guarded_callback<Capture, Render, Restore>(
    mut capture: Capture,
    render: Render,
    mut restore: Restore,
) -> bool
where
    Capture: FnMut() -> GlStateSnapshot,
    Render: FnOnce(),
    Restore: FnMut(GlStateSnapshot),
{
    let before = capture();
    render();
    let after = capture();
    if gl_state_violated(before, after) {
        restore(before);
        return true;
    }
    false
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

    pub(crate) fn record_overlay_pass(&self, node_key: NodeKey) {
        #[cfg(feature = "diagnostics")]
        crate::shell::desktop::runtime::diagnostics::emit_event(
            crate::shell::desktop::runtime::diagnostics::DiagnosticEvent::MessageSent {
                channel_id: CHANNEL_OVERLAY_PASS_REGISTERED,
                byte_len: std::mem::size_of::<NodeKey>(),
            },
        );

        if !self.content_pass_nodes.contains(&node_key) {
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

impl CompositorAdapter {
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

    pub(crate) fn register_render_to_parent_content_pass<F>(
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

            CompositorAdapter::run_content_callback_with_guardrails(node_key, painter.gl(), || {
                render_to_parent(painter.gl(), rect_in_parent)
            });

            #[cfg(feature = "diagnostics")]
            crate::shell::desktop::runtime::diagnostics::emit_span_duration(
                "tile_compositor::content_pass_callback",
                started.elapsed().as_micros() as u64,
            );
        }));

        Self::register_content_pass(ctx, node_key, tile_rect, callback);
    }

    pub(crate) fn register_content_pass_from_render_context(
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
        render: F,
    ) where
        F: FnOnce(),
    {
        #[cfg(feature = "diagnostics")]
        let started = std::time::Instant::now();

        if run_guarded_callback(
            || capture_gl_state(gl),
            render,
            |snapshot| restore_gl_state(gl, snapshot),
        ) {
            #[cfg(feature = "diagnostics")]
            crate::shell::desktop::runtime::diagnostics::emit_event(
                crate::shell::desktop::runtime::diagnostics::DiagnosticEvent::MessageSent {
                    channel_id: CHANNEL_COMPOSITOR_GL_STATE_VIOLATION,
                    byte_len: std::mem::size_of::<NodeKey>()
                        + std::mem::size_of::<GlStateSnapshot>(),
                },
            );
        }

        #[cfg(feature = "diagnostics")]
        crate::shell::desktop::runtime::diagnostics::emit_span_duration(
            "tile_compositor::content_pass_guarded_callback",
            started.elapsed().as_micros() as u64,
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
            pass_tracker.record_overlay_pass(overlay.node_key);
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

    use super::{CompositorPassTracker, GlStateSnapshot, gl_state_violated, run_guarded_callback};

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
        tracker.record_overlay_pass(NodeKey::new(1));
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
}
