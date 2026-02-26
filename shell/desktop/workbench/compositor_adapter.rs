/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::sync::Arc;

use egui::{Context, Id, LayerId, PaintCallback, Rect as EguiRect, Stroke, StrokeKind};
use egui_glow::CallbackFn;
use crate::graph::NodeKey;

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
        ctx.layer_painter(Self::overlay_layer(node_key)).rect_stroke(
            tile_rect.shrink(1.0),
            rounding,
            stroke,
            StrokeKind::Inside,
        );
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

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
}
