/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Egui host painter for `graph-canvas` projected scenes.
//!
//! Converts `ProjectedScene` draw items into egui `Painter` calls. This is
//! the egui-specific rendering adapter — the graph-canvas crate produces
//! framework-agnostic `SceneDrawItem`s, and this module paints them into an
//! egui UI surface.
//!
//! When iced becomes the primary host, the equivalent module will convert
//! `SceneDrawItem`s into iced `Canvas` or Vello commands instead. The
//! graph-canvas crate itself does not change.

use egui::{Color32, Pos2, Rect, CornerRadius, Stroke as EguiStroke, Vec2};

use graph_canvas::packet::{Color, ProjectedScene, SceneDrawItem, Stroke};

/// Paint a complete `ProjectedScene` into the given egui `Ui`.
///
/// Draws layers in order: background → world → overlays.
/// Hit proxies are not drawn — they are consumed by the interaction engine.
pub fn paint_projected_scene<N>(ui: &mut egui::Ui, scene: &ProjectedScene<N>) {
    let painter = ui.painter();

    for item in &scene.background {
        paint_draw_item(painter, item);
    }
    for item in &scene.world {
        paint_draw_item(painter, item);
    }
    for item in &scene.overlays {
        paint_draw_item(painter, item);
    }
}

fn paint_draw_item(painter: &egui::Painter, item: &SceneDrawItem) {
    match item {
        SceneDrawItem::Circle {
            center,
            radius,
            fill,
            stroke,
        } => {
            let center = to_pos2(*center);
            let fill = to_color32(*fill);
            let stroke = stroke
                .map(|s| to_egui_stroke(s))
                .unwrap_or(EguiStroke::NONE);
            painter.circle(center, *radius, fill, stroke);
        }
        SceneDrawItem::Line { from, to, stroke } => {
            let points = [to_pos2(*from), to_pos2(*to)];
            painter.line_segment(points, to_egui_stroke(*stroke));
        }
        SceneDrawItem::RoundedRect {
            rect,
            corner_radius,
            fill,
            stroke,
        } => {
            let rect = to_egui_rect(*rect);
            let rounding = CornerRadius::same((*corner_radius) as u8);
            let fill = to_color32(*fill);
            let stroke = stroke
                .map(|s| to_egui_stroke(s))
                .unwrap_or(EguiStroke::NONE);
            painter.rect(rect, rounding, fill, stroke, egui::StrokeKind::Outside);
        }
        SceneDrawItem::Label {
            position,
            text,
            font_size,
            color,
        } => {
            let pos = to_pos2(*position);
            let color = to_color32(*color);
            let font = egui::FontId::proportional(*font_size);
            painter.text(
                pos,
                egui::Align2::CENTER_CENTER,
                text,
                font,
                color,
            );
        }
        SceneDrawItem::ImageRef { .. } => {
            // Image rendering requires texture handle resolution from the host.
            // Deferred until the host texture registry is wired.
        }
    }
}

// ── Type conversions ──────────────────────────────────────────────────────

fn to_pos2(p: euclid::default::Point2D<f32>) -> Pos2 {
    Pos2::new(p.x, p.y)
}

fn to_egui_rect(r: euclid::default::Rect<f32>) -> Rect {
    Rect::from_min_size(
        Pos2::new(r.origin.x, r.origin.y),
        Vec2::new(r.size.width, r.size.height),
    )
}

fn to_color32(c: Color) -> Color32 {
    Color32::from_rgba_unmultiplied(
        (c.r * 255.0) as u8,
        (c.g * 255.0) as u8,
        (c.b * 255.0) as u8,
        (c.a * 255.0) as u8,
    )
}

fn to_egui_stroke(s: Stroke) -> EguiStroke {
    EguiStroke::new(s.width, to_color32(s.color))
}
