/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Iced painter for `graph-canvas` projected scenes.
//!
//! Mirrors graphshell's `render::canvas_egui_painter` for the iced
//! host — converts `ProjectedScene` draw items into iced
//! `canvas::Frame` calls so both hosts paint the same portable
//! packet through framework-specific primitives.
//!
//! Pairing: the egui painter consumes an `egui::Ui` and walks its
//! `Painter`; this module consumes an
//! `iced::widget::canvas::Frame` and walks its
//! path / fill / stroke / fill_text APIs. The graph-canvas crate
//! does not change between hosts.

use iced::alignment;
use iced::border::Radius;
use iced::widget::canvas::{self, Path, Stroke, Text};
use iced::{Color, Pixels, Point, Renderer, Size};

use graph_canvas::packet::{
    Color as PacketColor, ProjectedScene, SceneDrawItem, Stroke as PacketStroke,
};

/// Paint a complete `ProjectedScene` into the given iced canvas frame.
///
/// Draws layers in order: background → world → overlays. Matches
/// the egui painter's layer ordering. Hit proxies are not drawn —
/// they are consumed by the interaction engine on the iced side
/// once input translation lands.
pub fn paint_projected_scene<N>(
    frame: &mut canvas::Frame<Renderer>,
    scene: &ProjectedScene<N>,
) {
    for item in &scene.background {
        paint_draw_item(frame, item);
    }
    for item in &scene.world {
        paint_draw_item(frame, item);
    }
    for item in &scene.overlays {
        paint_draw_item(frame, item);
    }
}

fn paint_draw_item(frame: &mut canvas::Frame<Renderer>, item: &SceneDrawItem) {
    match item {
        SceneDrawItem::Circle {
            center,
            radius,
            fill,
            stroke,
        } => {
            let path = Path::circle(to_point(*center), *radius);
            frame.fill(&path, to_iced_color(*fill));
            if let Some(s) = stroke {
                frame.stroke(&path, to_iced_stroke(*s));
            }
        }
        SceneDrawItem::Line { from, to, stroke } => {
            let path = Path::line(to_point(*from), to_point(*to));
            frame.stroke(&path, to_iced_stroke(*stroke));
        }
        SceneDrawItem::RoundedRect {
            rect,
            corner_radius,
            fill,
            stroke,
        } => {
            let top_left = Point::new(rect.origin.x, rect.origin.y);
            let size = Size::new(rect.size.width, rect.size.height);
            let path = if *corner_radius > 0.0 {
                let radius = Radius::from(*corner_radius);
                Path::new(|builder| {
                    builder.rounded_rectangle(top_left, size, radius);
                })
            } else {
                Path::rectangle(top_left, size)
            };
            frame.fill(&path, to_iced_color(*fill));
            if let Some(s) = stroke {
                frame.stroke(&path, to_iced_stroke(*s));
            }
        }
        SceneDrawItem::Label {
            position,
            text,
            font_size,
            color,
        } => {
            // Match the egui painter's `Align2::CENTER_CENTER` so
            // both hosts anchor labels identically. iced 0.14
            // typed `align_x` as `iced_core::text::Alignment`,
            // which iced doesn't re-export without the `advanced`
            // feature — we construct via `From<alignment::Horizontal>`
            // and let inference resolve.
            frame.fill_text(Text {
                content: text.clone(),
                position: to_point(*position),
                color: to_iced_color(*color),
                size: Pixels(*font_size),
                align_x: alignment::Horizontal::Center.into(),
                align_y: alignment::Vertical::Center,
                ..Text::default()
            });
        }
        SceneDrawItem::ImageRef { .. } => {
            // Image rendering requires texture handle resolution
            // from the host; deferred until the iced host texture
            // registry lands (parallel to the egui side).
        }
    }
}

// ── Type conversions ──────────────────────────────────────────────────────

fn to_point(p: euclid::default::Point2D<f32>) -> Point {
    Point::new(p.x, p.y)
}

fn to_iced_color(c: PacketColor) -> Color {
    Color::from_rgba(c.r, c.g, c.b, c.a)
}

fn to_iced_stroke(s: PacketStroke) -> Stroke<'static> {
    Stroke::default()
        .with_width(s.width)
        .with_color(to_iced_color(s.color))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_roundtrip_preserves_channels() {
        let c = PacketColor::new(0.25, 0.5, 0.75, 0.9);
        let iced = to_iced_color(c);
        assert!((iced.r - 0.25).abs() < 1e-6);
        assert!((iced.g - 0.5).abs() < 1e-6);
        assert!((iced.b - 0.75).abs() < 1e-6);
        assert!((iced.a - 0.9).abs() < 1e-6);
    }
}
