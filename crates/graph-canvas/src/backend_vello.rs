/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Vello backend for graph-canvas.
//!
//! Converts a `ProjectedScene`'s draw items into `vello::Scene` commands.
//! This module is available behind the `vello` feature flag.
//!
//! The backend does not own the GPU rendering pipeline — it builds a
//! `vello::Scene` which the host passes to its `vello::Renderer`. This
//! keeps the wgpu device/queue/surface lifecycle in the host layer.

use std::hash::Hash;

use peniko::color::{AlphaColor, Srgb};
use peniko::kurbo;
use peniko::Fill;
use vello::Scene;

use crate::camera::CanvasViewport;
use crate::packet::{Color, ProjectedScene, SceneDrawItem, Stroke};

// ── Geometry conversion ─────────────────────────────────────────────────────

/// Convert a euclid Point2D<f32> to a kurbo Point (f64).
fn to_kurbo_point(p: euclid::default::Point2D<f32>) -> kurbo::Point {
    kurbo::Point::new(p.x as f64, p.y as f64)
}

/// Convert a graph-canvas `Color` to a peniko sRGB color.
fn to_peniko_color(c: Color) -> AlphaColor<Srgb> {
    AlphaColor::new([c.r, c.g, c.b, c.a])
}

/// Convert a graph-canvas `Stroke` to a kurbo stroke style + brush color.
fn to_kurbo_stroke(s: &Stroke) -> (kurbo::Stroke, AlphaColor<Srgb>) {
    (
        kurbo::Stroke::new(s.width as f64),
        to_peniko_color(s.color),
    )
}

// ── Scene rendering ─────────────────────────────────────────────────────────

/// Render a single `SceneDrawItem` into a `vello::Scene`.
fn render_draw_item(scene: &mut Scene, item: &SceneDrawItem, transform: kurbo::Affine) {
    match item {
        SceneDrawItem::Circle {
            center,
            radius,
            fill,
            stroke,
        } => {
            let circle = kurbo::Circle::new(to_kurbo_point(*center), *radius as f64);
            scene.fill(Fill::NonZero, transform, to_peniko_color(*fill), None, &circle);
            if let Some(s) = stroke {
                let (stroke_style, color) = to_kurbo_stroke(s);
                scene.stroke(&stroke_style, transform, color, None, &circle);
            }
        }
        SceneDrawItem::Line { from, to, stroke } => {
            let line = kurbo::Line::new(to_kurbo_point(*from), to_kurbo_point(*to));
            let (stroke_style, color) = to_kurbo_stroke(stroke);
            scene.stroke(&stroke_style, transform, color, None, &line);
        }
        SceneDrawItem::RoundedRect {
            rect,
            corner_radius,
            fill,
            stroke,
        } => {
            let kurbo_rect = kurbo::Rect::new(
                rect.origin.x as f64,
                rect.origin.y as f64,
                (rect.origin.x + rect.size.width) as f64,
                (rect.origin.y + rect.size.height) as f64,
            );
            let rounded = kurbo::RoundedRect::from_rect(kurbo_rect, *corner_radius as f64);
            scene.fill(Fill::NonZero, transform, to_peniko_color(*fill), None, &rounded);
            if let Some(s) = stroke {
                let (stroke_style, color) = to_kurbo_stroke(s);
                scene.stroke(&stroke_style, transform, color, None, &rounded);
            }
        }
        SceneDrawItem::Label {
            position,
            text: _,
            font_size: _,
            color: _,
        } => {
            // Text rendering requires a font context and glyph shaping,
            // which is host-specific. The Vello backend emits a placeholder
            // dot at the label position. The host should render text using
            // its own text pipeline (skrifa/parley/fontique) or overlay
            // labels via the UI framework.
            let dot = kurbo::Circle::new(to_kurbo_point(*position), 2.0);
            scene.fill(
                Fill::NonZero,
                transform,
                to_peniko_color(Color::new(0.7, 0.7, 0.7, 0.5)),
                None,
                &dot,
            );
        }
        SceneDrawItem::ImageRef { rect: _, handle: _ } => {
            // Image rendering requires the host to resolve the ImageHandle
            // to a vello ImageData. Skipped in the base backend — the host
            // should override or post-process image items.
        }
    }
}

/// Render a full `ProjectedScene` into a `vello::Scene`.
///
/// This is the primary entry point for the Vello backend. The caller
/// provides a mutable `Scene` (typically reset at the start of the frame)
/// and the projected scene from `derive_scene()`.
///
/// The transform is applied to all draw items — use it for viewport offset
/// and DPI scaling.
pub fn render_projected_scene<N: Clone + Eq + Hash>(
    scene: &mut Scene,
    projected: &ProjectedScene<N>,
    _viewport: &CanvasViewport,
) {
    let transform = kurbo::Affine::IDENTITY;

    // Background layer
    for item in &projected.background {
        render_draw_item(scene, item, transform);
    }

    // World layer (nodes, edges)
    for item in &projected.world {
        render_draw_item(scene, item, transform);
    }

    // Overlay layer
    for item in &projected.overlays {
        render_draw_item(scene, item, transform);
    }
}

/// Report the capabilities of the Vello backend.
pub fn vello_capabilities() -> crate::backend::CanvasBackendCapabilities {
    crate::backend::CanvasBackendCapabilities {
        two_point_five: true,
        isometric: true,
        images: false, // Requires host-side ImageHandle resolution
        labels: false, // Requires host-side text pipeline
        anti_aliased_strokes: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packet::{HitProxy, ProjectedScene, SceneDrawItem};
    use euclid::default::{Point2D, Rect, Size2D};

    fn sample_scene() -> ProjectedScene<u32> {
        ProjectedScene {
            background: vec![SceneDrawItem::RoundedRect {
                rect: Rect::new(Point2D::origin(), Size2D::new(800.0, 600.0)),
                corner_radius: 0.0,
                fill: Color::new(0.1, 0.1, 0.15, 1.0),
                stroke: None,
            }],
            world: vec![
                SceneDrawItem::Line {
                    from: Point2D::new(100.0, 200.0),
                    to: Point2D::new(300.0, 200.0),
                    stroke: Stroke {
                        color: Color::new(0.5, 0.5, 0.5, 0.6),
                        width: 1.5,
                    },
                },
                SceneDrawItem::Circle {
                    center: Point2D::new(100.0, 200.0),
                    radius: 16.0,
                    fill: Color::new(0.4, 0.6, 0.9, 1.0),
                    stroke: Some(Stroke {
                        color: Color::new(0.2, 0.4, 0.7, 1.0),
                        width: 2.0,
                    }),
                },
                SceneDrawItem::Circle {
                    center: Point2D::new(300.0, 200.0),
                    radius: 16.0,
                    fill: Color::new(0.9, 0.4, 0.3, 1.0),
                    stroke: None,
                },
                SceneDrawItem::Label {
                    position: Point2D::new(100.0, 220.0),
                    text: "Node A".into(),
                    font_size: 14.0,
                    color: Color::WHITE,
                },
            ],
            overlays: vec![SceneDrawItem::RoundedRect {
                rect: Rect::new(Point2D::new(50.0, 50.0), Size2D::new(200.0, 200.0)),
                corner_radius: 2.0,
                fill: Color::new(0.3, 0.5, 0.9, 0.15),
                stroke: Some(Stroke {
                    color: Color::new(0.3, 0.5, 0.9, 0.5),
                    width: 1.0,
                }),
            }],
            hit_proxies: vec![
                HitProxy::Node {
                    id: 0,
                    center: Point2D::new(100.0, 200.0),
                    radius: 16.0,
                },
                HitProxy::Node {
                    id: 1,
                    center: Point2D::new(300.0, 200.0),
                    radius: 16.0,
                },
            ],
        }
    }

    #[test]
    fn render_does_not_panic() {
        let projected = sample_scene();
        let viewport = CanvasViewport::default();
        let mut scene = Scene::new();
        render_projected_scene(&mut scene, &projected, &viewport);
        // If we get here, rendering succeeded.
    }

    #[test]
    fn render_empty_scene() {
        let projected = ProjectedScene::<u32>::default();
        let viewport = CanvasViewport::default();
        let mut scene = Scene::new();
        render_projected_scene(&mut scene, &projected, &viewport);
    }

    #[test]
    fn color_conversion_roundtrip() {
        let c = Color::new(0.5, 0.3, 0.8, 0.9);
        let peniko = to_peniko_color(c);
        let components = peniko.components;
        assert!((components[0] - 0.5).abs() < 0.001);
        assert!((components[1] - 0.3).abs() < 0.001);
        assert!((components[2] - 0.8).abs() < 0.001);
        assert!((components[3] - 0.9).abs() < 0.001);
    }

    #[test]
    fn point_conversion() {
        let euclid_pt = euclid::default::Point2D::new(42.5f32, -17.3f32);
        let kurbo_pt = to_kurbo_point(euclid_pt);
        assert!((kurbo_pt.x - 42.5).abs() < 0.001);
        assert!((kurbo_pt.y - (-17.3)).abs() < 0.001);
    }

    #[test]
    fn capabilities_report() {
        let caps = vello_capabilities();
        assert!(caps.two_point_five);
        assert!(caps.isometric);
        assert!(caps.anti_aliased_strokes);
        assert!(!caps.images);
        assert!(!caps.labels);
    }
}

