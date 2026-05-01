/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Standalone demo of the iced-graph-canvas-viewer.
//!
//! Builds a small `CanvasSceneInput<NodeKey>` with a handful of
//! nodes/edges and renders it through the viewer. Lets you visually
//! validate pan/zoom (drag with primary button, wheel to zoom)
//! without the rest of the graphshell stack.
//!
//! Run with:
//!
//! ```bash
//! cargo run -p iced-graph-canvas-viewer --example demo
//! ```

use euclid::default::{Point2D, Vector2D};
use graph_canvas::scene::{CanvasEdge, CanvasNode, CanvasSceneInput, ViewId};
use graphshell_core::graph::NodeKey;
use iced::widget::{canvas, column, container, text};
use iced::{Color, Element, Length, Task};

use iced_graph_canvas_viewer::{GraphCanvasMessage, GraphCanvasProgram};

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title(|_: &App| "iced-graph-canvas-viewer demo".to_string())
        .run()
}

struct App {
    program: GraphCanvasProgram,
    last_camera: Option<(Vector2D<f32>, f32)>,
}

impl App {
    fn new() -> (Self, Task<Message>) {
        (
            Self {
                program: GraphCanvasProgram::new(fixture_input()),
                last_camera: None,
            },
            Task::none(),
        )
    }

    fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::Canvas(GraphCanvasMessage::CameraChanged { pan, zoom }) => {
                self.last_camera = Some((pan, zoom));
            }
            Message::Canvas(GraphCanvasMessage::RightClicked { .. }) => {
                // Demo: ignore. The full host opens a context menu
                // against the resolved node here.
            }
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        let canvas_widget = canvas(&self.program)
            .width(Length::Fill)
            .height(Length::Fill);
        let canvas_element: Element<'_, GraphCanvasMessage> = canvas_widget.into();
        let canvas_element: Element<'_, Message> = canvas_element.map(Message::Canvas);

        let camera_status = match self.last_camera {
            Some((pan, zoom)) => format!(
                "camera: pan=({:.1}, {:.1})  zoom={:.3}",
                pan.x, pan.y, zoom
            ),
            None => "camera: auto-fit (drag to pan, wheel to zoom)".to_string(),
        };

        let body = column![
            text("iced-graph-canvas-viewer demo").size(22.0),
            text(camera_status)
                .size(12.0)
                .color(Color::from_rgb(0.6, 0.6, 0.7)),
            canvas_element,
        ]
        .spacing(12);

        container(body)
            .padding(20)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}

#[derive(Debug, Clone)]
enum Message {
    Canvas(GraphCanvasMessage),
}

/// Hand-built scene with a few nodes + edges. Positions are
/// scattered so the auto-fit camera has something to frame.
fn fixture_input() -> CanvasSceneInput<NodeKey> {
    let nodes = vec![
        CanvasNode {
            id: NodeKey::new(0),
            position: Point2D::new(-100.0, -50.0),
            radius: 16.0,
            label: Some("alpha".into()),
        },
        CanvasNode {
            id: NodeKey::new(1),
            position: Point2D::new(80.0, -40.0),
            radius: 16.0,
            label: Some("beta".into()),
        },
        CanvasNode {
            id: NodeKey::new(2),
            position: Point2D::new(-60.0, 70.0),
            radius: 16.0,
            label: Some("gamma".into()),
        },
        CanvasNode {
            id: NodeKey::new(3),
            position: Point2D::new(120.0, 90.0),
            radius: 20.0,
            label: Some("delta".into()),
        },
        CanvasNode {
            id: NodeKey::new(4),
            position: Point2D::new(0.0, 0.0),
            radius: 24.0,
            label: Some("center".into()),
        },
    ];
    let edges = vec![
        CanvasEdge {
            source: NodeKey::new(0),
            target: NodeKey::new(4),
            weight: 1.0,
        },
        CanvasEdge {
            source: NodeKey::new(1),
            target: NodeKey::new(4),
            weight: 1.0,
        },
        CanvasEdge {
            source: NodeKey::new(2),
            target: NodeKey::new(4),
            weight: 1.0,
        },
        CanvasEdge {
            source: NodeKey::new(3),
            target: NodeKey::new(4),
            weight: 1.0,
        },
        CanvasEdge {
            source: NodeKey::new(0),
            target: NodeKey::new(2),
            weight: 1.0,
        },
        CanvasEdge {
            source: NodeKey::new(1),
            target: NodeKey::new(3),
            weight: 1.0,
        },
    ];
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
