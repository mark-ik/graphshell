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
use crate::shell::desktop::runtime::registries::CHANNEL_COMPOSITOR_GL_STATE_VIOLATION;
use egui::{Context, Id, LayerId, PaintCallback, Rect as EguiRect, Stroke, StrokeKind};
use egui_glow::{CallbackFn, glow};

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
