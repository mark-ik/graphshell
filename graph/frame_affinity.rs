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

use graph_canvas::packet::Color;
use graphshell_core::geometry::PortablePoint;
use petgraph::Direction;
use petgraph::visit::EdgeRef;

use crate::graph::{ArrangementSubKind, Graph, NodeKey};
use crate::model::graph::RelationSelector;

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
    pub(crate) centroid: PortablePoint,
    /// Force magnitude for this region (arrangement_weight × region-specific factor).
    pub(crate) strength: f32,
    /// Stable per-frame color derived from the anchor key index.
    pub(crate) color: Color,
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
            if edge.weight().has_relation(RelationSelector::Arrangement(
                ArrangementSubKind::FrameMember,
            )) {
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
            let positions: Vec<PortablePoint> = members
                .iter()
                .filter_map(|&m| graph.node_projected_position(m))
                .collect();

            if positions.is_empty() {
                return None;
            }

            let centroid = {
                let sum = positions.iter().fold(
                    graphshell_core::geometry::PortableVector::new(0.0, 0.0),
                    |acc, &p| acc + p.to_vector(),
                );
                PortablePoint::new(
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
/// Delegates to [`graph_canvas::layout::FrameAffinity`]: each region's
/// members are pulled toward the centroid of the member set, scaled by
/// `region.strength × strength_override`. Pinned nodes are skipped.
pub(crate) fn apply_frame_affinity_forces(
    app: &mut crate::app::GraphBrowserApp,
    regions: &[FrameAffinityRegion],
    strength_override: Option<f32>,
) {
    use graph_canvas::layout::{self as gclayout, Layout as _};

    if !app.workspace.graph_runtime.physics.is_running || regions.is_empty() {
        return;
    }

    let scene = crate::graph::physics::scene_input_for_physics_pub(app);
    if scene.nodes.is_empty() {
        return;
    }
    let viewport = crate::graph::physics::scene_bounds_viewport_pub(&scene);
    let mut extras = graph_canvas::layout::LayoutExtras::<NodeKey>::default();
    extras.pinned = crate::graph::physics::pinned_set_pub(app);
    extras.frame_regions = regions
        .iter()
        .map(|region| gclayout::FrameRegion {
            anchor: region.frame_anchor,
            members: region.members.clone(),
            strength: region.strength,
        })
        .collect();

    let mut layout = gclayout::FrameAffinity::new(gclayout::FrameAffinityConfig {
        global_strength: strength_override.unwrap_or(1.0),
        ..Default::default()
    });
    let mut state = gclayout::StatelessPassState::default();
    let deltas = layout.step(&scene, &mut state, 0.0, &viewport, &extras);
    crate::graph::physics::apply_canvas_deltas_pub(app, deltas);
}

/// Stable per-frame color derived from the ordinal index of the anchor.
///
/// Uses a small fixed palette — sufficient for the first slice.  Per-frame
/// stable identity colors (keyed by `FrameId`) are future work.
fn stable_frame_color(index: usize) -> Color {
    // Distinct, muted hues suitable for semi-transparent backdrops
    const PALETTE: &[Color] = &[
        Color::new(100.0 / 255.0, 150.0 / 255.0, 240.0 / 255.0, 1.0), // muted blue
        Color::new(240.0 / 255.0, 140.0 / 255.0, 80.0 / 255.0, 1.0),  // muted orange
        Color::new(100.0 / 255.0, 200.0 / 255.0, 130.0 / 255.0, 1.0), // muted green
        Color::new(200.0 / 255.0, 100.0 / 255.0, 200.0 / 255.0, 1.0), // muted purple
        Color::new(200.0 / 255.0, 190.0 / 255.0, 80.0 / 255.0, 1.0),  // muted yellow
        Color::new(100.0 / 255.0, 200.0 / 255.0, 210.0 / 255.0, 1.0), // muted teal
        Color::new(210.0 / 255.0, 100.0 / 255.0, 120.0 / 255.0, 1.0), // muted red
        Color::new(180.0 / 255.0, 150.0 / 255.0, 100.0 / 255.0, 1.0), // muted brown
    ];
    PALETTE[index % PALETTE.len()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::graph::apply::{GraphDelta, GraphDeltaResult, apply_graph_delta};
    use crate::model::graph::{ArrangementSubKind, Graph};

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
            GraphDelta::AssertRelation {
                from: anchor,
                to: member,
                assertion: crate::graph::EdgeAssertion::Arrangement {
                    sub_kind: ArrangementSubKind::FrameMember,
                },
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
