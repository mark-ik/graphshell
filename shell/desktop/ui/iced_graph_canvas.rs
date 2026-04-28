/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Shim that bridges the standalone
//! [`iced-graph-canvas-viewer`](../../../../crates/iced-graph-canvas-viewer)
//! crate into the graphshell iced host.
//!
//! The viewer (camera, pan/zoom, scene derivation, painter) lives in
//! its own portable crate so it can be developed, tested, and demoed
//! without dragging in graphshell's heavy Servo/webrender dependency
//! tree. This module is the host-side glue — it owns the
//! `GraphBrowserApp` → `CanvasSceneInput<NodeKey>` conversion that
//! the viewer crate doesn't know about.
//!
//! Mirrors the shim pattern used by `iced_middlenet_viewer`.

use graph_canvas::projection::ViewDimension;

use crate::app::{GraphBrowserApp, GraphViewId, SceneMode};

// Re-export the viewer's public surface so the host's other modules
// (`iced_app`) keep their existing import paths
// (`super::iced_graph_canvas::GraphCanvasMessage`, etc).
pub(crate) use iced_graph_canvas_viewer::{
    GraphCanvasMessage, GraphCanvasProgram, GraphCanvasState,
};

/// Default node radius used when iced builds a `CanvasSceneInput`.
/// Mirrors the egui host's policy default for the bring-up path; when
/// iced consumes a live `NodeStylePolicy` this routes through the same
/// resolver.
const DEFAULT_NODE_RADIUS: f32 = 16.0;

/// Snapshot the shared graph into a portable `CanvasSceneInput` and
/// wrap it in a viewer `GraphCanvasProgram`. The host owns the
/// view_id (`IcedHost::view_id`) so camera persistence keys on a
/// stable identity across `view()` rebuilds.
///
/// Equivalent to the previous `GraphCanvasProgram::from_graph_app`
/// method; moved here as a free function so the
/// `iced-graph-canvas-viewer` crate stays graphshell-app-agnostic.
pub(crate) fn from_graph_app(app: &GraphBrowserApp, view_id: GraphViewId) -> GraphCanvasProgram {
    let scene_input = crate::app::canvas_scene::build_scene_input(
        app.render_graph(),
        view_id,
        SceneMode::default(),
        &ViewDimension::default(),
        None, // visible_nodes: None → all nodes visible
        DEFAULT_NODE_RADIUS,
    );
    GraphCanvasProgram::new(scene_input)
}
