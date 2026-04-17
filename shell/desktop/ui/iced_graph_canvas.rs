/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Minimal iced graph canvas — M5.4 "first real surface".
//!
//! Renders the shared graph (from `GraphshellRuntime.graph_app.domain_graph()`)
//! as circles + lines on an `iced::widget::canvas::Canvas`. Not yet wired to
//! the full `graph_canvas` crate's `CanvasBackend` trait — that's the proper
//! integration once iced can host arbitrary scene packets. This module
//! proves the minimal path: "iced can paint graphshell data".
//!
//! Sized with a viewport-scaling transform: node positions come from the
//! domain graph in logical coordinates (Point2D<f32>); the draw pass
//! computes a bounding box, then scales/centers the graph into the
//! canvas bounds with 24 px padding.
//!
//! **Scope limits**:
//! - No hit testing, no interaction (M5.4 is "render only").
//! - No labels yet (iced canvas text needs font state threaded through).
//! - Edges are drawn thin; node circles are uniform radius.
//! - Reads a frozen snapshot at `view()` build time — no per-frame physics.
//!
//! Follow-on work converts this into a real `CanvasBackend<NodeKey>` impl
//! so the runtime's `ProjectedScene` drives both hosts identically.

use iced::mouse;
use iced::widget::canvas::{self, Path, Stroke};
use iced::{Color, Point, Rectangle, Renderer, Size, Theme};

use crate::app::GraphBrowserApp;

/// One node in the snapshot passed to the canvas program.
#[derive(Debug, Clone, Copy)]
struct NodeSnap {
    x: f32,
    y: f32,
}

/// Iced canvas program rendering a graph snapshot.
///
/// Holds owned data (no borrows) so it can be embedded in `iced::Element`
/// without lifetime parameters. `view()` in [`super::iced_app::IcedApp`]
/// rebuilds the snapshot every frame from the live runtime state.
#[derive(Debug, Clone, Default)]
pub(crate) struct GraphCanvasProgram {
    nodes: Vec<NodeSnap>,
    /// Edges expressed as `(from_index, to_index)` into `nodes`.
    edges: Vec<(usize, usize)>,
}

impl GraphCanvasProgram {
    /// Build a snapshot from the current domain graph state. Collects
    /// every node's projected position + every hyperlink-family edge.
    pub(crate) fn from_graph_app(app: &GraphBrowserApp) -> Self {
        let graph = app.domain_graph();

        // Collect nodes, remembering the source `NodeKey` so edges can
        // be translated to snapshot indices.
        let mut nodes = Vec::new();
        let mut key_to_idx = std::collections::HashMap::new();
        for (key, node) in graph.nodes() {
            let pos = node.projected_position();
            nodes.push(NodeSnap { x: pos.x, y: pos.y });
            key_to_idx.insert(key, nodes.len() - 1);
        }

        // Collect edges; drop any whose endpoints aren't in the snapshot
        // (defensive — shouldn't happen, but snapshot and iteration are
        // not atomic in general).
        let edges = graph
            .edges()
            .filter_map(|edge| {
                let from = *key_to_idx.get(&edge.from)?;
                let to = *key_to_idx.get(&edge.to)?;
                Some((from, to))
            })
            .collect();

        Self { nodes, edges }
    }

    /// Compute the axis-aligned bounding box of all node positions, or
    /// `None` if the graph is empty.
    fn bounding_box(&self) -> Option<(f32, f32, f32, f32)> {
        if self.nodes.is_empty() {
            return None;
        }
        let first = self.nodes[0];
        let mut min_x = first.x;
        let mut min_y = first.y;
        let mut max_x = first.x;
        let mut max_y = first.y;
        for n in &self.nodes[1..] {
            min_x = min_x.min(n.x);
            min_y = min_y.min(n.y);
            max_x = max_x.max(n.x);
            max_y = max_y.max(n.y);
        }
        Some((min_x, min_y, max_x, max_y))
    }

    /// Compute a (scale, offset_x, offset_y) transform mapping domain
    /// coordinates into the supplied canvas bounds with uniform padding.
    /// Returns `None` for an empty graph.
    fn viewport_transform(&self, bounds: Size) -> Option<(f32, f32, f32)> {
        const PADDING: f32 = 24.0;
        const DEFAULT_SCALE: f32 = 1.0;

        let (min_x, min_y, max_x, max_y) = self.bounding_box()?;
        let span_x = (max_x - min_x).max(1e-3);
        let span_y = (max_y - min_y).max(1e-3);

        let avail_w = (bounds.width - 2.0 * PADDING).max(1.0);
        let avail_h = (bounds.height - 2.0 * PADDING).max(1.0);

        let scale = (avail_w / span_x)
            .min(avail_h / span_y)
            .max(0.0)
            .min(100.0);
        let scale = if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            DEFAULT_SCALE
        };

        // Center the scaled bounding box in the available area.
        let scaled_w = span_x * scale;
        let scaled_h = span_y * scale;
        let offset_x = PADDING + (avail_w - scaled_w) * 0.5 - min_x * scale;
        let offset_y = PADDING + (avail_h - scaled_h) * 0.5 - min_y * scale;

        Some((scale, offset_x, offset_y))
    }
}

impl<Message> canvas::Program<Message> for GraphCanvasProgram {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry<Renderer>> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());

        let Some((scale, ox, oy)) = self.viewport_transform(bounds.size()) else {
            // Empty graph — draw nothing but still return a valid frame.
            return vec![frame.into_geometry()];
        };

        // Edges first so nodes render on top.
        let edge_color = Color::from_rgba(0.55, 0.55, 0.62, 0.85);
        for (from, to) in &self.edges {
            let (Some(a), Some(b)) = (self.nodes.get(*from), self.nodes.get(*to)) else {
                continue;
            };
            let p1 = Point::new(a.x * scale + ox, a.y * scale + oy);
            let p2 = Point::new(b.x * scale + ox, b.y * scale + oy);
            let path = Path::line(p1, p2);
            frame.stroke(
                &path,
                Stroke::default().with_width(1.25).with_color(edge_color),
            );
        }

        // Nodes.
        let node_fill = Color::from_rgb(0.35, 0.60, 0.95);
        let node_stroke = Color::from_rgb(0.10, 0.20, 0.40);
        let radius = 6.0;
        for node in &self.nodes {
            let center = Point::new(node.x * scale + ox, node.y * scale + oy);
            let path = Path::circle(center, radius);
            frame.fill(&path, node_fill);
            frame.stroke(
                &path,
                Stroke::default().with_width(1.0).with_color(node_stroke),
            );
        }

        vec![frame.into_geometry()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_snapshot_has_no_bounding_box() {
        let program = GraphCanvasProgram::default();
        assert!(program.bounding_box().is_none());
    }

    #[test]
    fn bounding_box_covers_all_nodes() {
        let program = GraphCanvasProgram {
            nodes: vec![
                NodeSnap { x: -10.0, y: 5.0 },
                NodeSnap { x: 20.0, y: 15.0 },
                NodeSnap { x: 0.0, y: -3.0 },
            ],
            edges: Vec::new(),
        };
        let (min_x, min_y, max_x, max_y) = program
            .bounding_box()
            .expect("non-empty graph has bounds");
        assert_eq!(min_x, -10.0);
        assert_eq!(min_y, -3.0);
        assert_eq!(max_x, 20.0);
        assert_eq!(max_y, 15.0);
    }

    #[test]
    fn single_node_viewport_transform_centers() {
        let program = GraphCanvasProgram {
            nodes: vec![NodeSnap { x: 0.0, y: 0.0 }],
            edges: Vec::new(),
        };
        let (scale, ox, oy) = program
            .viewport_transform(Size::new(400.0, 300.0))
            .expect("should produce transform");
        // Scale is finite and positive.
        assert!(scale.is_finite() && scale > 0.0);
        // Single node at (0,0) maps somewhere inside the padding margin.
        let px = 0.0 * scale + ox;
        let py = 0.0 * scale + oy;
        assert!((24.0..=376.0).contains(&px), "px was {px}");
        assert!((24.0..=276.0).contains(&py), "py was {py}");
    }

    #[test]
    fn from_graph_app_snapshots_nodes_and_edges() {
        let mut app = GraphBrowserApp::new_for_testing();
        let a = app.add_node_and_sync(
            "https://a.example/".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );
        let b = app.add_node_and_sync(
            "https://b.example/".to_string(),
            euclid::default::Point2D::new(10.0, 0.0),
        );
        app.add_edge_and_sync(a, b, crate::graph::EdgeType::Hyperlink, None);

        let program = GraphCanvasProgram::from_graph_app(&app);
        assert_eq!(program.nodes.len(), 2);
        assert!(!program.edges.is_empty());
    }
}
