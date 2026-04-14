/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Camera state and viewport for the graph canvas.

use euclid::default::{Point2D, Rect, Size2D, Vector2D};
use serde::{Deserialize, Serialize};

/// Viewport rectangle and display scale factor for a canvas pane.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanvasViewport {
    /// The pane rectangle in logical (host-framework) coordinates.
    pub rect: Rect<f32>,
    /// Display scale factor (e.g. 2.0 for Retina / HiDPI).
    pub scale_factor: f32,
}

impl CanvasViewport {
    pub fn new(origin: Point2D<f32>, size: Size2D<f32>, scale_factor: f32) -> Self {
        Self {
            rect: Rect::new(origin, size),
            scale_factor,
        }
    }

    pub fn size(&self) -> Size2D<f32> {
        self.rect.size
    }

    pub fn center(&self) -> Point2D<f32> {
        self.rect.center()
    }
}

impl Default for CanvasViewport {
    fn default() -> Self {
        Self {
            rect: Rect::new(Point2D::origin(), Size2D::new(800.0, 600.0)),
            scale_factor: 1.0,
        }
    }
}

/// Camera state: pan offset and zoom level.
///
/// The camera transforms canvas (world) coordinates to screen coordinates.
/// `pan` is the world-space offset of the viewport center, and `zoom` is a
/// multiplicative scale factor (1.0 = no zoom).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanvasCamera {
    /// Pan offset in world coordinates.
    pub pan: Vector2D<f32>,
    /// Zoom level (1.0 = identity, >1.0 = zoomed in).
    pub zoom: f32,
}

impl CanvasCamera {
    pub fn new(pan: Vector2D<f32>, zoom: f32) -> Self {
        Self { pan, zoom }
    }

    /// Convert a point from world space to screen space given a viewport.
    pub fn world_to_screen(&self, world_pos: Point2D<f32>, viewport: &CanvasViewport) -> Point2D<f32> {
        let center = viewport.center();
        Point2D::new(
            (world_pos.x + self.pan.x) * self.zoom + center.x,
            (world_pos.y + self.pan.y) * self.zoom + center.y,
        )
    }

    /// Convert a point from screen space to world space given a viewport.
    pub fn screen_to_world(&self, screen_pos: Point2D<f32>, viewport: &CanvasViewport) -> Point2D<f32> {
        let center = viewport.center();
        Point2D::new(
            (screen_pos.x - center.x) / self.zoom - self.pan.x,
            (screen_pos.y - center.y) / self.zoom - self.pan.y,
        )
    }
}

impl Default for CanvasCamera {
    fn default() -> Self {
        Self {
            pan: Vector2D::zero(),
            zoom: 1.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_world_screen_roundtrip() {
        let viewport = CanvasViewport::default();
        let camera = CanvasCamera::new(Vector2D::new(10.0, -20.0), 2.0);
        let world = Point2D::new(50.0, 75.0);
        let screen = camera.world_to_screen(world, &viewport);
        let back = camera.screen_to_world(screen, &viewport);
        assert!((back.x - world.x).abs() < 1e-4);
        assert!((back.y - world.y).abs() < 1e-4);
    }

    #[test]
    fn identity_camera_preserves_center() {
        let viewport = CanvasViewport::default();
        let camera = CanvasCamera::default();
        let origin = Point2D::origin();
        let screen = camera.world_to_screen(origin, &viewport);
        // World origin maps to viewport center with identity camera.
        assert!((screen.x - viewport.center().x).abs() < 1e-4);
        assert!((screen.y - viewport.center().y).abs() < 1e-4);
    }

    #[test]
    fn serde_roundtrip_viewport() {
        let vp = CanvasViewport::new(Point2D::new(10.0, 20.0), Size2D::new(1920.0, 1080.0), 2.0);
        let json = serde_json::to_string(&vp).unwrap();
        let back: CanvasViewport = serde_json::from_str(&json).unwrap();
        assert_eq!(vp, back);
    }

    #[test]
    fn serde_roundtrip_camera() {
        let cam = CanvasCamera::new(Vector2D::new(-5.0, 3.0), 1.5);
        let json = serde_json::to_string(&cam).unwrap();
        let back: CanvasCamera = serde_json::from_str(&json).unwrap();
        assert_eq!(cam, back);
    }
}

