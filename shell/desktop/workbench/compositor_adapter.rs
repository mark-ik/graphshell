/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::HashSet;
use std::sync::Arc;

use egui::{Context, Id, LayerId, PaintCallback, Rect as EguiRect, Stroke, StrokeKind};
use egui_glow::CallbackFn;
use crate::graph::NodeKey;

const CHANNEL_CONTENT_PASS_REGISTERED: &str = "tile_compositor.content_pass_registered";
const CHANNEL_OVERLAY_PASS_REGISTERED: &str = "tile_compositor.overlay_pass_registered";
const CHANNEL_PASS_ORDER_VIOLATION: &str = "tile_compositor.pass_order_violation";
const CHANNEL_INVALID_TILE_RECT: &str = "tile_compositor.invalid_tile_rect";

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
        LayerId::new(egui::Order::Middle, Id::new(("graphshell_webview", node_key)))
    }

    pub(crate) fn overlay_layer(node_key: NodeKey) -> LayerId {
        LayerId::new(egui::Order::Foreground, Id::new(("graphshell_overlay", node_key)))
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

    pub(crate) fn draw_overlay_stroke(
        ctx: &Context,
        node_key: NodeKey,
        tile_rect: EguiRect,
        rounding: f32,
        stroke: Stroke,
    ) {
        #[cfg(feature = "diagnostics")]
        let started = std::time::Instant::now();

        ctx.layer_painter(Self::overlay_layer(node_key)).rect_stroke(
            tile_rect.shrink(1.0),
            rounding,
            stroke,
            StrokeKind::Inside,
        );

        #[cfg(feature = "diagnostics")]
        crate::shell::desktop::runtime::diagnostics::emit_span_duration(
            "tile_compositor::overlay_pass_draw",
            started.elapsed().as_micros() as u64,
        );
    }

    pub(crate) fn report_invalid_tile_rect(node_key: NodeKey) {
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
    use std::cell::RefCell;

    use crate::graph::NodeKey;

    use super::CompositorPassTracker;

    #[test]
    fn pass_scheduler_runs_content_before_overlay() {
        let order = RefCell::new(Vec::new());
        {
            let mut content = || order.borrow_mut().push("content");
            let mut overlay = || order.borrow_mut().push("overlay");
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
}
