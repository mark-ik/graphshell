/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Iced canvas widget that renders graph-canvas `ProjectedScene`s.
//!
//! Mirrors the iced graph-canvas pieces of the host adapter
//! (`iced_graph_canvas` + `iced_canvas_painter` modules in graphshell)
//! as a portable, Servo-free, webrender-free standalone crate.
//! Depends only on `iced` 0.14 + `graph-canvas` (framework-agnostic) +
//! `graphshell-core` (for `NodeKey`).
//!
//! **Iced-idiomatic deviation from the egui host**: camera state lives
//! in `canvas::Program::State` (a [`GraphCanvasState`]) rather than
//! being computed per-frame by the host. iced's `canvas::Program::update`
//! handles wheel-zoom and pointer-drag by mutating the canvas-local
//! camera and capturing the events so they don't bubble up to the app
//! subscription.
//!
//! The two hosts agree on the portable contract (`CanvasSceneInput`,
//! `CanvasCamera`, `CanvasViewport`, `ProjectedScene`) but not on where
//! camera state lives — that's a framework concern.
//!
//! Hosts construct a [`GraphCanvasProgram`] from a pre-built
//! `CanvasSceneInput<NodeKey>` (the host owns the conversion from
//! whatever app-state graph it has). The viewer emits
//! [`GraphCanvasMessage::CameraChanged`] when camera state mutates;
//! hosts map via `iced::Element::map`.
//!
//! Internal painter module exposes
//! [`painter::paint_projected_scene`] for callers who want to paint a
//! `ProjectedScene` into an iced canvas `Frame` directly without
//! going through `GraphCanvasProgram` (e.g., custom widgets that
//! compose multiple scenes).

pub mod painter;

use iced::mouse::{self, Cursor};
use iced::widget::canvas::{self, Action};
use iced::{Point, Rectangle, Renderer, Size, Theme};

use euclid::default::{Rect as EuclidRect, Size2D, Vector2D};
use graph_canvas::camera::{CanvasCamera, CanvasViewport};
use graph_canvas::derive::{DeriveConfig, NodeVisualOverride, derive_scene};
use graph_canvas::packet::ProjectedScene;
use graph_canvas::scene::CanvasSceneInput;

use graphshell_core::graph::NodeKey;

/// Messages the graph canvas publishes up to the host. Hosts map
/// these into their own message type via `iced::Element::map`.
///
/// Canvas-local state (camera, drag origin) lives in
/// [`GraphCanvasState`], but camera changes need to round-trip into
/// the host's runtime state (e.g. a per-view camera map) so they
/// survive widget destruction and other surfaces see them. The canvas
/// emits one of these after every pan/zoom.
#[derive(Debug, Clone, PartialEq)]
pub enum GraphCanvasMessage {
    /// Camera state changed (pan, zoom). Hosts persist these onto
    /// their per-view camera map.
    CameraChanged { pan: Vector2D<f32>, zoom: f32 },
}

/// Padding around the graph bounding box when fit-to-bounds frames
/// the camera. Matches the egui host's default to keep static-graph
/// output visually consistent across hosts.
const FIT_PADDING_RATIO: f32 = 1.08;

const FIT_ZOOM_MIN: f32 = 0.1;
const FIT_ZOOM_MAX: f32 = 10.0;
const FIT_FALLBACK_ZOOM: f32 = 1.0;

/// Zoom multiplier applied per wheel-scroll unit. Symmetric: a scroll
/// up by one unit multiplies zoom by `WHEEL_ZOOM_STEP`; down divides.
/// Tuned to feel responsive without making a single scroll notch jarring.
const WHEEL_ZOOM_STEP: f32 = 1.1;

/// Canvas widget program backed by a portable `CanvasSceneInput`.
///
/// The program owns only the scene *input*; camera state lives in
/// [`GraphCanvasState`] (the `canvas::Program::State`). Hosts
/// construct a fresh program each frame from current graph state;
/// iced's retained widget lifecycle keeps the camera state alive
/// across rebuilds.
#[derive(Debug, Clone)]
pub struct GraphCanvasProgram {
    scene_input: CanvasSceneInput<NodeKey>,
}

/// Persistent canvas-local state owned by iced across frames.
///
/// `canvas::Program::State: Default + 'static`. iced constructs one
/// instance per canvas widget and passes it mutably to `update` and
/// immutably to `draw`.
///
/// - `camera: None` means "auto-fit to bounds in `draw`". The first
///   meaningful user interaction (wheel or drag) seeds the camera
///   from the current fit-to-bounds result before applying the delta,
///   so the user sees smooth continuation from auto-fit into manual
///   navigation.
/// - `drag_origin` is `Some(p)` while the primary button is held
///   inside the canvas; cleared on button release.
#[derive(Debug, Default, Clone)]
pub struct GraphCanvasState {
    camera: Option<CanvasCamera>,
    drag_origin: Option<Point>,
}

impl GraphCanvasProgram {
    /// Construct a program around a pre-built `CanvasSceneInput`.
    /// Hosts call `graph_canvas::scene::CanvasSceneInput` builders
    /// (or their own equivalents like `canvas_bridge::build_scene_input`)
    /// to derive the input from app-side graph state, then pass it
    /// here.
    pub fn new(scene_input: CanvasSceneInput<NodeKey>) -> Self {
        Self { scene_input }
    }

    /// Fresh program with an empty scene input. Used as a fallback
    /// when iced constructs the canvas before a runtime is wired.
    pub fn empty() -> Self {
        Self {
            scene_input: CanvasSceneInput {
                view_id: graph_canvas::scene::ViewId(0),
                nodes: Vec::new(),
                edges: Vec::new(),
                scene_objects: Vec::new(),
                overlays: Vec::new(),
                scene_mode: graph_canvas::scene::SceneMode::default(),
                projection: graph_canvas::projection::ProjectionMode::default(),
            },
        }
    }

    /// Read-only access to the scene input, for parity tests that
    /// want to feed the same input through a reference path.
    pub fn scene_input(&self) -> &CanvasSceneInput<NodeKey> {
        &self.scene_input
    }

    /// Derive a `ProjectedScene<NodeKey>` given an explicit camera
    /// and viewport. Used by `draw` (with `state.camera` or the
    /// fit-to-bounds fallback) and by parity tests (with a
    /// test-constructed camera).
    pub fn project_scene_with(
        &self,
        camera: &CanvasCamera,
        viewport: &CanvasViewport,
    ) -> Option<ProjectedScene<NodeKey>> {
        if self.scene_input.nodes.is_empty() {
            return None;
        }
        Some(derive_scene(
            &self.scene_input,
            camera,
            viewport,
            &|_node| 0.0,
            &|_idx, _id| NodeVisualOverride::default(),
            &DeriveConfig::default(),
        ))
    }

    /// Convenience: derive a scene using the auto-fit camera for the
    /// given canvas size. Kept for test ergonomics and parity tests.
    pub fn project_scene(&self, bounds: Size) -> Option<ProjectedScene<NodeKey>> {
        let (camera, viewport) = self.fit_camera_and_viewport(bounds)?;
        self.project_scene_with(&camera, &viewport)
    }

    /// Build a viewport matching `bounds` plus a `CanvasCamera`
    /// fit-to-bounds against the scene's world-space bounding box.
    /// Returns `None` when there are no nodes or the viewport has
    /// zero area.
    pub fn fit_camera_and_viewport(
        &self,
        bounds: Size,
    ) -> Option<(CanvasCamera, CanvasViewport)> {
        let world_bounds = self.world_bounds()?;
        let viewport = CanvasViewport::new(
            euclid::default::Point2D::origin(),
            Size2D::new(bounds.width, bounds.height),
            1.0,
        );

        let mut camera = CanvasCamera::default();
        let fitted = camera.fit_to_bounds(
            world_bounds,
            &viewport,
            FIT_PADDING_RATIO,
            FIT_ZOOM_MIN,
            FIT_ZOOM_MAX,
            FIT_FALLBACK_ZOOM,
        );
        if !fitted {
            return None;
        }
        Some((camera, viewport))
    }

    /// World-space bounding box of node positions inclusive of radii.
    fn world_bounds(&self) -> Option<EuclidRect<f32>> {
        if self.scene_input.nodes.is_empty() {
            return None;
        }
        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;
        for node in &self.scene_input.nodes {
            let r = node.radius;
            min_x = min_x.min(node.position.x - r);
            min_y = min_y.min(node.position.y - r);
            max_x = max_x.max(node.position.x + r);
            max_y = max_y.max(node.position.y + r);
        }
        let origin = euclid::default::Point2D::new(min_x, min_y);
        let size = Size2D::new(max_x - min_x, max_y - min_y);
        Some(EuclidRect::new(origin, size))
    }

    /// Seed a mutable camera: return the State's camera if set,
    /// otherwise the fit-to-bounds camera. Used by `update` so that
    /// the first user interaction transitions smoothly from auto-fit
    /// into manual camera control.
    fn camera_for_update(&self, state: &GraphCanvasState, bounds: Size) -> Option<CanvasCamera> {
        state
            .camera
            .clone()
            .or_else(|| self.fit_camera_and_viewport(bounds).map(|(c, _)| c))
    }
}

impl canvas::Program<GraphCanvasMessage> for GraphCanvasProgram {
    type State = GraphCanvasState;

    fn update(
        &self,
        state: &mut Self::State,
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: Cursor,
    ) -> Option<Action<GraphCanvasMessage>> {
        match event {
            canvas::Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                if cursor.position_in(bounds).is_none() {
                    return None;
                }
                let (_dx, dy) = match delta {
                    mouse::ScrollDelta::Lines { x, y } | mouse::ScrollDelta::Pixels { x, y } => {
                        (*x, *y)
                    }
                };
                let mut camera = self.camera_for_update(state, bounds.size())?;
                let factor = WHEEL_ZOOM_STEP.powf(dy);
                let new_zoom = (camera.zoom * factor).clamp(FIT_ZOOM_MIN, FIT_ZOOM_MAX);
                camera.zoom = new_zoom;
                camera.pan_velocity = Vector2D::zero();
                let pan = camera.pan;
                let zoom = camera.zoom;
                state.camera = Some(camera);
                Some(Action::publish(GraphCanvasMessage::CameraChanged { pan, zoom }).and_capture())
            }
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let Some(p) = cursor.position_in(bounds) else {
                    return None;
                };
                state.drag_origin = Some(p);
                Some(Action::capture())
            }
            canvas::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                let origin = state.drag_origin?;
                let Some(now) = cursor.position_in(bounds) else {
                    return None;
                };
                let dx = now.x - origin.x;
                let dy = now.y - origin.y;
                if dx == 0.0 && dy == 0.0 {
                    return None;
                }
                let mut camera = self.camera_for_update(state, bounds.size())?;
                camera.pan += Vector2D::new(dx / camera.zoom, dy / camera.zoom);
                let pan = camera.pan;
                let zoom = camera.zoom;
                state.camera = Some(camera);
                state.drag_origin = Some(now);
                Some(Action::publish(GraphCanvasMessage::CameraChanged { pan, zoom }).and_capture())
            }
            canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                if state.drag_origin.take().is_some() {
                    Some(Action::capture())
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn draw(
        &self,
        state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: Cursor,
    ) -> Vec<canvas::Geometry<Renderer>> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());

        let Some((fit_camera, viewport)) = self.fit_camera_and_viewport(bounds.size()) else {
            return vec![frame.into_geometry()];
        };
        let camera = state.camera.clone().unwrap_or(fit_camera);

        let Some(scene) = self.project_scene_with(&camera, &viewport) else {
            return vec![frame.into_geometry()];
        };
        painter::paint_projected_scene(&mut frame, &scene);
        vec![frame.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        state: &Self::State,
        bounds: Rectangle,
        cursor: Cursor,
    ) -> mouse::Interaction {
        if state.drag_origin.is_some() {
            mouse::Interaction::Grabbing
        } else if cursor.position_in(bounds).is_some() {
            mouse::Interaction::Grab
        } else {
            mouse::Interaction::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use graph_canvas::scene::{CanvasEdge, CanvasNode, ViewId};

    fn sample_input() -> CanvasSceneInput<NodeKey> {
        let nodes = vec![
            CanvasNode {
                id: NodeKey::new(0),
                position: euclid::default::Point2D::new(-50.0, 0.0),
                radius: 16.0,
                label: Some("a".into()),
            },
            CanvasNode {
                id: NodeKey::new(1),
                position: euclid::default::Point2D::new(50.0, 0.0),
                radius: 16.0,
                label: Some("b".into()),
            },
        ];
        let edges = vec![CanvasEdge {
            source: NodeKey::new(0),
            target: NodeKey::new(1),
            weight: 1.0,
        }];
        CanvasSceneInput {
            view_id: ViewId(0),
            nodes,
            edges,
            scene_objects: Vec::new(),
            overlays: Vec::new(),
            scene_mode: graph_canvas::scene::SceneMode::default(),
            projection: graph_canvas::projection::ProjectionMode::default(),
        }
    }

    #[test]
    fn empty_program_has_no_projection() {
        let program = GraphCanvasProgram::empty();
        assert!(program.scene_input.nodes.is_empty());
        assert!(program.world_bounds().is_none());
        assert!(program.project_scene(Size::new(400.0, 300.0)).is_none());
    }

    #[test]
    fn new_with_scene_input_keeps_node_count() {
        let program = GraphCanvasProgram::new(sample_input());
        assert_eq!(program.scene_input.nodes.len(), 2);
    }

    #[test]
    fn project_scene_yields_canvas_local_coordinates() {
        let program = GraphCanvasProgram::new(sample_input());
        let bounds = Size::new(400.0, 300.0);
        let scene = program.project_scene(bounds).expect("non-empty");
        for item in &scene.world {
            if let graph_canvas::packet::SceneDrawItem::Circle { center, .. } = item {
                assert!(center.x >= 0.0 && center.x <= bounds.width);
                assert!(center.y >= 0.0 && center.y <= bounds.height);
            }
        }
    }

    #[test]
    fn fit_camera_and_viewport_reflects_bounds() {
        let program = GraphCanvasProgram::new(sample_input());
        let (camera, viewport) = program
            .fit_camera_and_viewport(Size::new(800.0, 600.0))
            .expect("non-empty");
        assert!(camera.zoom > 0.0);
        assert_eq!(viewport.rect.size.width, 800.0);
        assert_eq!(viewport.rect.size.height, 600.0);
    }

    #[test]
    fn project_scene_returns_none_for_zero_bounds() {
        let program = GraphCanvasProgram::new(sample_input());
        assert!(program.project_scene(Size::new(0.0, 0.0)).is_none());
    }

    #[test]
    fn iced_projection_matches_reference_derivation() {
        let program = GraphCanvasProgram::new(sample_input());
        let bounds = Size::new(600.0, 400.0);
        let iced_scene = program.project_scene(bounds).expect("non-empty");
        let (camera, viewport) = program.fit_camera_and_viewport(bounds).expect("non-empty");
        let reference_scene = derive_scene(
            program.scene_input(),
            &camera,
            &viewport,
            &|_node| 0.0,
            &|_idx, _id| NodeVisualOverride::default(),
            &DeriveConfig::default(),
        );
        assert_eq!(iced_scene, reference_scene);
    }

    fn run_update(
        program: &GraphCanvasProgram,
        state: &mut GraphCanvasState,
        event: canvas::Event,
        cursor_in: Point,
    ) -> Option<Action<GraphCanvasMessage>> {
        let cursor = Cursor::Available(cursor_in);
        let bounds = Rectangle {
            x: 0.0,
            y: 0.0,
            width: 400.0,
            height: 300.0,
        };
        <GraphCanvasProgram as canvas::Program<GraphCanvasMessage>>::update(
            program, state, &event, bounds, cursor,
        )
    }

    #[test]
    fn wheel_scroll_inside_bounds_publishes_camera_changed() {
        let program = GraphCanvasProgram::new(sample_input());
        let mut state = GraphCanvasState::default();
        let action = run_update(
            &program,
            &mut state,
            canvas::Event::Mouse(mouse::Event::WheelScrolled {
                delta: mouse::ScrollDelta::Lines { x: 0.0, y: 1.0 },
            }),
            Point::new(200.0, 150.0),
        )
        .expect("should produce an action");
        let (msg, _redraw, status) = action.into_inner();
        assert!(matches!(msg, Some(GraphCanvasMessage::CameraChanged { .. })));
        assert_eq!(status, iced::event::Status::Captured);
    }

    #[test]
    fn primary_drag_pans_camera() {
        let program = GraphCanvasProgram::new(sample_input());
        let mut state = GraphCanvasState::default();
        let _ = run_update(
            &program,
            &mut state,
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            Point::new(100.0, 100.0),
        );
        let _ = run_update(
            &program,
            &mut state,
            canvas::Event::Mouse(mouse::Event::CursorMoved {
                position: Point::new(120.0, 105.0),
            }),
            Point::new(120.0, 105.0),
        );
        assert!(state.camera.is_some());
        let pan = state.camera.as_ref().unwrap().pan;
        assert!(pan.x > 0.0 && pan.y > 0.0);
    }
}
