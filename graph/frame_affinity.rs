/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Frame-affinity organizational behavior — data model, force derivation, and
//! post-physics force application.
//!
//! Spec: `layout_behaviors_and_physics_spec.md §4`
//!
//! **First execution slice** (2026-03-18, `lane:layout-semantics #99`):
//! - Derive `FrameAffinityRegion`s from `ArrangementRelation(FrameMember)` edges.
//! - Apply soft centroid-attraction force through the post-physics hook.
//! - Expose `frame_affinity_backdrops()` for the render layer backdrop pass.
//!
//! Force magnitude: `FamilyPhysicsPolicy.arrangement_weight` × distance factor.
//! Gate: `GraphPhysicsExtensionConfig.frame_affinity` (defaults `false` until
//! `CanvasRegistry.zones_enabled` is wired).

use std::collections::HashMap;

use egui::{Color32, Pos2, Vec2};
use petgraph::Direction;
use petgraph::visit::EdgeRef;

use crate::graph::{ArrangementSubKind, Graph, NodeKey};
use crate::model::graph::EdgeKind;

/// Default soft-attraction coefficient for frame-affinity force.
///
/// Chosen to produce a gentle grouping bias without overwhelming physics.
/// Governed by `FamilyPhysicsPolicy.arrangement_weight` once that binding
/// lands (spec §5.1); until then this constant is authoritative.
const DEFAULT_ARRANGEMENT_WEIGHT: f32 = 0.5;

/// Minimum membership count for a region to be rendered or force-applied.
const MIN_FRAME_MEMBER_COUNT: usize = 2;

/// A derived runtime region representing one frame's affinity cluster.
///
/// Computed from `ArrangementRelation(FrameMember)` edges each frame.
/// Not persisted — reconstructed from graph state on demand.
#[derive(Debug, Clone)]
pub(crate) struct FrameAffinityRegion {
    /// The frame anchor node (source of the `FrameMember` edges).
    pub(crate) frame_anchor: NodeKey,
    /// Member nodes belonging to this frame.
    pub(crate) members: Vec<NodeKey>,
    /// Centroid of member positions in graph space.
    pub(crate) centroid: Pos2,
    /// Force magnitude for this region (arrangement_weight × region-specific factor).
    pub(crate) strength: f32,
    /// Stable per-frame color derived from the anchor key index.
    pub(crate) color: Color32,
}

/// Derive all `FrameAffinityRegion`s from `ArrangementRelation(FrameMember)` edges.
///
/// For each node that is the **source** of at least one outgoing
/// `ArrangementRelation(FrameMember)` edge, we treat it as a frame anchor and
/// collect all targets as frame members.  Only regions with ≥ 2 members are
/// returned (singletons have no grouping semantics worth visualising).
///
/// Positions are read from `graph.node_projected_position`.
pub(crate) fn derive_frame_affinity_regions(graph: &Graph) -> Vec<FrameAffinityRegion> {
    let mut anchor_to_members: HashMap<NodeKey, Vec<NodeKey>> = HashMap::new();

    for (key, _) in graph.nodes() {
        for edge in graph.inner.edges_directed(key, Direction::Outgoing) {
            if edge.weight().kinds.contains(&EdgeKind::ArrangementRelation)
                && edge
                    .weight()
                    .arrangement
                    .as_ref()
                    .is_some_and(|a| a.sub_kinds.contains(&ArrangementSubKind::FrameMember))
            {
                anchor_to_members
                    .entry(key)
                    .or_default()
                    .push(edge.target());
            }
        }
    }

    anchor_to_members
        .into_iter()
        .filter(|(_, members)| members.len() >= MIN_FRAME_MEMBER_COUNT)
        .enumerate()
        .filter_map(|(idx, (anchor, members))| {
            let positions: Vec<Pos2> = members
                .iter()
                .filter_map(|&m| {
                    graph
                        .node_projected_position(m)
                        .map(|p| Pos2::new(p.x, p.y))
                })
                .collect();

            if positions.is_empty() {
                return None;
            }

            let centroid = {
                let sum = positions
                    .iter()
                    .fold(Vec2::ZERO, |acc, &p| acc + p.to_vec2());
                Pos2::new(
                    sum.x / positions.len() as f32,
                    sum.y / positions.len() as f32,
                )
            };

            Some(FrameAffinityRegion {
                frame_anchor: anchor,
                members,
                centroid,
                strength: DEFAULT_ARRANGEMENT_WEIGHT,
                color: stable_frame_color(idx),
            })
        })
        .collect()
}

/// Apply soft centroid-attraction forces for all frame-affinity regions.
///
/// For each member node, adds a displacement toward its frame centroid,
/// scaled by `region.strength × distance`.  Pinned nodes are skipped.
/// The force is applied to both the domain graph's projected positions and
/// the egui_graphs node locations (via `app.workspace.graph_runtime.egui_state`).
pub(crate) fn apply_frame_affinity_forces(
    app: &mut crate::app::GraphBrowserApp,
    regions: &[FrameAffinityRegion],
    strength_override: Option<f32>,
) {
    if !app.workspace.graph_runtime.physics.base.is_running {
        return;
    }

    let strength_factor = strength_override.unwrap_or(1.0);
    let mut position_deltas: HashMap<NodeKey, Vec2> = HashMap::new();

    for region in regions {
        for &member in &region.members {
            if app
                .domain_graph()
                .get_node(member)
                .is_some_and(|n| n.is_pinned)
            {
                continue;
            }
            if let Some(pos) = app.domain_graph().node_projected_position(member) {
                let member_pos = Vec2::new(pos.x, pos.y);
                let centroid_vec = Vec2::new(region.centroid.x, region.centroid.y);
                let delta = centroid_vec - member_pos;
                let force = delta * region.strength * strength_factor;
                *position_deltas.entry(member).or_insert(Vec2::ZERO) += force;
            }
        }
    }

    // Apply to domain graph projected positions
    let projected_positions: Vec<(NodeKey, euclid::default::Point2D<f32>)> = position_deltas
        .iter()
        .filter_map(|(&key, &delta)| {
            app.domain_graph().node_projected_position(key).map(|pos| {
                (
                    key,
                    euclid::default::Point2D::new(pos.x + delta.x, pos.y + delta.y),
                )
            })
        })
        .collect();

    for (key, next_pos) in &projected_positions {
        let _ = app
            .domain_graph_mut()
            .set_node_projected_position(*key, *next_pos);
    }

    // Sync egui_graphs node locations to match
    if let Some(state_mut) = app.workspace.graph_runtime.egui_state.as_mut() {
        for (key, next_pos) in projected_positions {
            if let Some(egui_node) = state_mut.graph.node_mut(key) {
                egui_node.set_location(egui::Pos2::new(next_pos.x, next_pos.y));
            }
        }
    }
}

/// Stable per-frame color derived from the ordinal index of the anchor.
///
/// Uses a small fixed palette — sufficient for the first slice.  Per-frame
/// stable identity colors (keyed by `FrameId`) are future work.
fn stable_frame_color(index: usize) -> Color32 {
    // Distinct, muted hues suitable for semi-transparent backdrops
    const PALETTE: &[Color32] = &[
        Color32::from_rgb(100, 150, 240), // muted blue
        Color32::from_rgb(240, 140, 80),  // muted orange
        Color32::from_rgb(100, 200, 130), // muted green
        Color32::from_rgb(200, 100, 200), // muted purple
        Color32::from_rgb(200, 190, 80),  // muted yellow
        Color32::from_rgb(100, 200, 210), // muted teal
        Color32::from_rgb(210, 100, 120), // muted red
        Color32::from_rgb(180, 150, 100), // muted brown
    ];
    PALETTE[index % PALETTE.len()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::graph::apply::{GraphDelta, GraphDeltaResult, apply_graph_delta};
    use crate::model::graph::{ArrangementSubKind, EdgeType, Graph};

    fn add_node(graph: &mut Graph, url: &str) -> NodeKey {
        let GraphDeltaResult::NodeAdded(key) = apply_graph_delta(
            graph,
            GraphDelta::AddNode {
                id: None,
                url: url.to_string(),
                position: euclid::default::Point2D::new(0.0, 0.0),
            },
        ) else {
            panic!("expected NodeAdded");
        };
        key
    }

    fn add_frame_member_edge(graph: &mut Graph, anchor: NodeKey, member: NodeKey) {
        apply_graph_delta(
            graph,
            GraphDelta::AddEdge {
                from: anchor,
                to: member,
                edge_type: EdgeType::ArrangementRelation(ArrangementSubKind::FrameMember),
                edge_label: None,
            },
        );
    }

    #[test]
    fn derive_regions_empty_graph() {
        let graph = Graph::new();
        let regions = derive_frame_affinity_regions(&graph);
        assert!(regions.is_empty());
    }

    #[test]
    fn derive_regions_single_member_excluded() {
        let mut graph = Graph::new();
        let anchor = add_node(&mut graph, "graphshell://frame/1");
        let member = add_node(&mut graph, "https://a.test/");
        add_frame_member_edge(&mut graph, anchor, member);
        // Only 1 member — below MIN_FRAME_MEMBER_COUNT
        let regions = derive_frame_affinity_regions(&graph);
        assert!(regions.is_empty());
    }

    #[test]
    fn derive_regions_two_members_included() {
        let mut graph = Graph::new();
        let anchor = add_node(&mut graph, "graphshell://frame/1");
        let a = add_node(&mut graph, "https://a.test/");
        let b = add_node(&mut graph, "https://b.test/");
        // Set non-overlapping positions so centroid is meaningful
        let _ = graph.set_node_projected_position(a, euclid::default::Point2D::new(10.0, 0.0));
        let _ = graph.set_node_projected_position(b, euclid::default::Point2D::new(-10.0, 0.0));
        add_frame_member_edge(&mut graph, anchor, a);
        add_frame_member_edge(&mut graph, anchor, b);
        let regions = derive_frame_affinity_regions(&graph);
        assert_eq!(regions.len(), 1);
        let region = &regions[0];
        assert_eq!(region.frame_anchor, anchor);
        assert_eq!(region.members.len(), 2);
        // Centroid should be near (0, 0)
        assert!(
            region.centroid.x.abs() < 1.0,
            "centroid x should be ~0, got {}",
            region.centroid.x
        );
    }

    #[test]
    fn stable_frame_color_wraps() {
        // Ensure the palette wraps without panic
        for i in 0..20 {
            let _ = stable_frame_color(i);
        }
    }
}
