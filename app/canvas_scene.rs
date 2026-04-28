/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Host-neutral graph→canvas scene conversion.
//!
//! Extracted from `render/canvas_bridge` so that no-Servo builds
//! (`iced-host` without `servo-engine`) can call `build_scene_input`
//! without pulling in the `render` module.

use std::collections::HashSet;

use graph_canvas::projection::{ProjectionMode, ViewDimension};
use graph_canvas::scene::{CanvasEdge, CanvasNode, CanvasSceneInput, SceneMode, ViewId};
use petgraph::visit::{EdgeRef as PetgraphEdgeRef, IntoEdgeReferences};

use super::{GraphViewId, SceneMode as AppSceneMode};
use crate::graph::{Graph, NodeKey};

/// Build a `CanvasSceneInput` from the domain graph and view state.
///
/// Called once per frame for each active graph view. Iced calls this
/// from `iced_graph_canvas::from_graph_app`; egui delegates via
/// `render::canvas_bridge::build_scene_input`.
pub(crate) fn build_scene_input(
    graph: &Graph,
    view_id: GraphViewId,
    scene_mode: AppSceneMode,
    dimension: &ViewDimension,
    visible_nodes: Option<&HashSet<NodeKey>>,
    default_node_radius: f32,
) -> CanvasSceneInput<NodeKey> {
    let nodes: Vec<CanvasNode<NodeKey>> = graph
        .nodes()
        .filter(|(key, _)| visible_nodes.is_none_or(|mask| mask.contains(key)))
        .map(|(key, node)| CanvasNode {
            id: key,
            position: node.projected_position(),
            radius: default_node_radius,
            label: Some(node.title.clone()),
        })
        .collect();

    let edges: Vec<CanvasEdge<NodeKey>> = graph
        .inner
        .edge_references()
        .filter(|edge| {
            visible_nodes.is_none_or(|mask| {
                mask.contains(&edge.source()) && mask.contains(&edge.target())
            })
        })
        .map(|e| CanvasEdge {
            source: e.source(),
            target: e.target(),
            weight: 1.0,
        })
        .collect();

    CanvasSceneInput {
        view_id: view_id_to_canvas(view_id),
        nodes,
        edges,
        scene_objects: Vec::new(),
        overlays: Vec::new(),
        scene_mode: scene_mode_to_canvas(scene_mode),
        projection: ProjectionMode::from_view_dimension(dimension),
    }
}

pub(crate) fn scene_mode_to_canvas(mode: AppSceneMode) -> SceneMode {
    match mode {
        AppSceneMode::Browse => SceneMode::Browse,
        AppSceneMode::Arrange => SceneMode::Arrange,
        AppSceneMode::Simulate => SceneMode::Simulate,
    }
}

pub(crate) fn view_id_to_canvas(id: GraphViewId) -> ViewId {
    let uuid = id.as_uuid();
    let bytes = uuid.as_bytes();
    let lower = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
    ViewId(lower)
}
