/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::HashMap;

use crate::app::GraphBrowserApp;
use crate::graph::NodeKey;
use crate::registries::atomic::knowledge::SemanticClassVector;
use crate::util::CoordBridge;

#[allow(unused_imports)]
pub use egui_graphs::FruchtermanReingoldState as FrBaseState;
pub use egui_graphs::FruchtermanReingoldWithCenterGravity as GraphPhysicsLayout;
pub use egui_graphs::FruchtermanReingoldWithCenterGravityState as GraphPhysicsState;
#[allow(unused_imports)]
pub use egui_graphs::FruchtermanReingoldWithExtras as GraphPhysicsExtrasLayout;
#[allow(unused_imports)]
pub use egui_graphs::FruchtermanReingoldWithExtrasState as GraphPhysicsExtrasState;
#[allow(unused_imports)]
pub use egui_graphs::{
    CenterGravity, CenterGravityParams, Extra, ForceAlgorithm, Layout, LayoutState,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct GraphPhysicsTuning {
    pub(crate) repulsion_strength: f32,
    pub(crate) attraction_strength: f32,
    pub(crate) gravity_strength: f32,
    pub(crate) damping: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DegreeRepulsionConfig {
    pub radius_px: f32,
    pub strength: f32,
}

impl DegreeRepulsionConfig {
    pub const fn mild() -> Self {
        Self {
            radius_px: 220.0,
            strength: 4.0,
        }
    }

    pub const fn medium() -> Self {
        Self {
            radius_px: 220.0,
            strength: 8.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DomainClusteringConfig {
    pub strength: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SemanticClusteringConfig {
    pub strength: f32,
    pub similarity_floor: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct HubPullConfig {
    pub radius_px: f32,
    pub strength: f32,
    pub degree_floor: usize,
}

impl Default for HubPullConfig {
    fn default() -> Self {
        Self {
            radius_px: 260.0,
            strength: 0.05,
            degree_floor: 3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct GraphPhysicsExtensionConfig {
    pub(crate) degree_repulsion: Option<DegreeRepulsionConfig>,
    pub(crate) domain_clustering: Option<DomainClusteringConfig>,
    pub(crate) semantic_clustering: Option<SemanticClusteringConfig>,
    pub(crate) hub_pull: Option<HubPullConfig>,
    /// Enable frame-affinity soft-attraction post-physics force.
    ///
    /// Derived from `CanvasRegistry.zones_enabled` at call site.  Defaults
    /// `false`; wired to the registry gate once `lane:layout-semantics` is
    /// fully executed.
    pub(crate) frame_affinity_enabled: bool,
}

impl GraphPhysicsExtensionConfig {
    pub(crate) fn any_enabled(self) -> bool {
        self.degree_repulsion.is_some()
            || self.domain_clustering.is_some()
            || self.semantic_clustering.is_some()
            || self.hub_pull.is_some()
            || self.frame_affinity_enabled
    }
}

impl Default for GraphPhysicsTuning {
    fn default() -> Self {
        Self {
            repulsion_strength: 0.28,
            attraction_strength: 0.22,
            gravity_strength: 0.18,
            damping: 0.55,
        }
    }
}

pub(crate) fn apply_graph_physics_tuning(
    state: &mut GraphPhysicsState,
    tuning: GraphPhysicsTuning,
) {
    state.base.c_repulse = tuning.repulsion_strength;
    state.base.c_attract = tuning.attraction_strength;
    state.base.damping = tuning.damping;
    state.extras.0.params.c = tuning.gravity_strength;
}

pub(crate) fn default_graph_physics_state() -> GraphPhysicsState {
    let mut state = GraphPhysicsState::default();
    apply_graph_physics_tuning(&mut state, GraphPhysicsTuning::default());
    state.base.k_scale = 0.42;
    state.base.dt = 0.03;
    state.base.max_step = 3.0;
    state
}

pub(crate) fn apply_graph_physics_extensions(
    app: &mut GraphBrowserApp,
    extensions: Option<GraphPhysicsExtensionConfig>,
) {
    let Some(extensions) = extensions else {
        return;
    };
    if !extensions.any_enabled() {
        return;
    }

    if let Some(config) = extensions.degree_repulsion {
        apply_degree_repulsion_forces(app, config);
    }

    if let Some(config) = extensions.domain_clustering {
        apply_domain_clustering_forces(app, config);
    }

    apply_semantic_clustering_forces(app, extensions.semantic_clustering);

    if let Some(config) = extensions.hub_pull {
        apply_hub_pull_forces(app, config);
    }

    if extensions.frame_affinity_enabled {
        let regions =
            crate::graph::frame_affinity::derive_frame_affinity_regions(app.domain_graph());
        crate::graph::frame_affinity::apply_frame_affinity_forces(app, &regions, None);
    }
}

pub(crate) fn apply_position_deltas(
    app: &mut GraphBrowserApp,
    position_deltas: HashMap<NodeKey, egui::Vec2>,
) {
    if position_deltas.is_empty() {
        return;
    }

    for (key, delta) in &position_deltas {
        if let Some(node) = app.domain_graph().get_node(*key)
            && !node.is_pinned
            && let Some(position) = app.domain_graph().node_projected_position(*key)
        {
            let next_pos =
                euclid::default::Point2D::new(position.x + delta.x, position.y + delta.y);
            let _ = app
                .domain_graph_mut()
                .set_node_projected_position(*key, next_pos);
        }
    }

    let projected_positions: Vec<_> = position_deltas
        .iter()
        .filter_map(|(key, _delta)| {
            let node = app.domain_graph().get_node(*key)?;
            if node.is_pinned {
                return None;
            }
            let position = app.domain_graph().node_projected_position(*key)?;
            Some((*key, position))
        })
        .collect();
    if let Some(state_mut) = app.workspace.graph_runtime.egui_state.as_mut() {
        for (key, position) in projected_positions {
            if let Some(egui_node) = state_mut.graph.node_mut(key) {
                egui_node.set_location(position.to_pos2());
            }
        }
    }
}

pub(crate) fn apply_degree_repulsion_forces(
    app: &mut GraphBrowserApp,
    config: DegreeRepulsionConfig,
) {
    if !app.workspace.graph_runtime.physics.base.is_running {
        return;
    }

    let nodes: Vec<_> = app.domain_graph().nodes().map(|(key, _)| key).collect();
    if nodes.len() < 2 {
        return;
    }

    let degrees: HashMap<NodeKey, usize> = nodes
        .iter()
        .map(|&key| (key, app.domain_graph().inner.edges(key).count()))
        .collect();
    let positions: HashMap<NodeKey, egui::Pos2> = nodes
        .iter()
        .filter_map(|&key| {
            app.domain_graph()
                .node_projected_position(key)
                .map(|pos| (key, pos.to_pos2()))
        })
        .collect();

    let mut position_deltas: HashMap<NodeKey, egui::Vec2> = HashMap::new();

    for i in 0..nodes.len() {
        for j in (i + 1)..nodes.len() {
            let key_a = nodes[i];
            let key_b = nodes[j];
            let (Some(pos_a), Some(pos_b)) = (positions.get(&key_a), positions.get(&key_b)) else {
                continue;
            };

            let delta = *pos_b - *pos_a;
            let distance = delta.length();
            if distance <= 1.0 || distance > config.radius_px {
                continue;
            }

            let degree_a = degrees.get(&key_a).copied().unwrap_or(0);
            let degree_b = degrees.get(&key_b).copied().unwrap_or(0);
            let max_degree = degree_a.max(degree_b);
            if max_degree <= 1 {
                continue;
            }

            let proximity = 1.0 - (distance / config.radius_px);
            let degree_bonus = (max_degree as f32).ln_1p();
            let push = delta.normalized() * proximity * degree_bonus * config.strength;

            *position_deltas.entry(key_a).or_insert(egui::Vec2::ZERO) -= push;
            *position_deltas.entry(key_b).or_insert(egui::Vec2::ZERO) += push;
        }
    }

    apply_position_deltas(app, position_deltas);
}

pub(crate) fn apply_domain_clustering_forces(
    app: &mut GraphBrowserApp,
    config: DomainClusteringConfig,
) {
    if !app.workspace.graph_runtime.physics.base.is_running {
        return;
    }

    let mut domain_members: HashMap<String, Vec<(NodeKey, egui::Pos2)>> = HashMap::new();
    for (key, node) in app.domain_graph().nodes() {
        let Some(domain_key) = registrable_domain_key(node.url()) else {
            continue;
        };
        let Some(position) = app.domain_graph().node_projected_position(key) else {
            continue;
        };
        domain_members
            .entry(domain_key)
            .or_default()
            .push((key, position.to_pos2()));
    }

    let mut position_deltas: HashMap<NodeKey, egui::Vec2> = HashMap::new();
    for members in domain_members.into_values() {
        if members.len() < 2 {
            continue;
        }

        let centroid = members
            .iter()
            .fold(egui::Vec2::ZERO, |acc, (_, pos)| acc + pos.to_vec2())
            / members.len() as f32;

        for (key, position) in members {
            let delta = centroid - position.to_vec2();
            *position_deltas.entry(key).or_insert(egui::Vec2::ZERO) += delta * config.strength;
        }
    }

    apply_position_deltas(app, position_deltas);
}

fn registrable_domain_key(url: &str) -> Option<String> {
    let parsed = url::Url::parse(url).ok()?;
    let host = parsed
        .host_str()?
        .trim_start_matches("www.")
        .to_ascii_lowercase();
    if host.parse::<std::net::IpAddr>().is_ok() {
        return Some(host);
    }

    let labels: Vec<&str> = host
        .split('.')
        .filter(|segment| !segment.is_empty())
        .collect();
    if labels.len() <= 2 {
        return Some(host);
    }

    let common_country_slds = ["ac", "co", "com", "edu", "gov", "net", "org"];
    let tail_len = if labels.last().is_some_and(|tld| tld.len() == 2)
        && labels
            .get(labels.len().saturating_sub(2))
            .is_some_and(|sld| common_country_slds.contains(sld))
        && labels.len() >= 3
    {
        3
    } else {
        2
    };

    Some(labels[labels.len() - tail_len..].join("."))
}

pub(crate) fn apply_semantic_clustering_forces(
    app: &mut GraphBrowserApp,
    semantic_config: Option<SemanticClusteringConfig>,
) {
    let Some(config) = semantic_config else {
        return;
    };

    if config.strength < 1e-6 {
        return;
    }

    if !app.workspace.graph_runtime.physics.base.is_running {
        return;
    }

    if app.workspace.graph_runtime.semantic_index.is_empty() {
        return;
    }

    let tagged_nodes: Vec<(NodeKey, SemanticClassVector)> = app
        .workspace
        .graph_runtime
        .semantic_index
        .iter()
        .map(|(&key, vector)| (key, vector.clone()))
        .collect();

    if tagged_nodes.len() < 2 {
        return;
    }

    let mut position_deltas: HashMap<NodeKey, egui::Vec2> = HashMap::new();

    for i in 0..tagged_nodes.len() {
        for j in (i + 1)..tagged_nodes.len() {
            let (key_a, vector_a) = &tagged_nodes[i];
            let (key_b, vector_b) = &tagged_nodes[j];

            let similarity = semantic_pair_similarity(vector_a, vector_b);
            if similarity < config.similarity_floor {
                continue;
            }

            let pos_a = app.domain_graph().node_projected_position(*key_a);
            let pos_b = app.domain_graph().node_projected_position(*key_b);

            if let (Some(pa), Some(pb)) = (pos_a, pos_b) {
                let delta = egui::Vec2::new(pb.x - pa.x, pb.y - pa.y);
                let force = delta * similarity * config.strength;

                *position_deltas.entry(*key_a).or_insert(egui::Vec2::ZERO) += force;
                *position_deltas.entry(*key_b).or_insert(egui::Vec2::ZERO) -= force;
            }
        }
    }

    apply_position_deltas(app, position_deltas);
}

pub(crate) fn apply_hub_pull_forces(app: &mut GraphBrowserApp, config: HubPullConfig) {
    if !app.workspace.graph_runtime.physics.base.is_running {
        return;
    }

    let nodes: Vec<_> = app.domain_graph().nodes().map(|(key, _)| key).collect();
    if nodes.len() < 2 {
        return;
    }

    let degrees: HashMap<NodeKey, usize> = nodes
        .iter()
        .map(|&key| (key, app.domain_graph().inner.edges(key).count()))
        .collect();
    let positions: HashMap<NodeKey, egui::Pos2> = nodes
        .iter()
        .filter_map(|&key| {
            app.domain_graph()
                .node_projected_position(key)
                .map(|pos| (key, pos.to_pos2()))
        })
        .collect();

    let mut position_deltas: HashMap<NodeKey, egui::Vec2> = HashMap::new();

    for i in 0..nodes.len() {
        for j in (i + 1)..nodes.len() {
            let key_a = nodes[i];
            let key_b = nodes[j];
            let (Some(pos_a), Some(pos_b)) = (positions.get(&key_a), positions.get(&key_b)) else {
                continue;
            };

            let degree_a = degrees.get(&key_a).copied().unwrap_or(0);
            let degree_b = degrees.get(&key_b).copied().unwrap_or(0);
            if degree_a == degree_b {
                continue;
            }

            let (hub_pos, hub_degree, leaf_degree, leaf_key, leaf_pos) = if degree_a > degree_b {
                (*pos_a, degree_a, degree_b, key_b, *pos_b)
            } else {
                (*pos_b, degree_b, degree_a, key_a, *pos_a)
            };

            if hub_degree < config.degree_floor {
                continue;
            }

            let delta = hub_pos - leaf_pos;
            let distance = delta.length();
            if distance <= 1.0 || distance > config.radius_px {
                continue;
            }

            let proximity = 1.0 - (distance / config.radius_px);
            let degree_gap = hub_degree.saturating_sub(leaf_degree).max(1) as f32;
            let pull =
                delta * proximity * (hub_degree as f32).ln_1p() * degree_gap * config.strength;
            *position_deltas.entry(leaf_key).or_insert(egui::Vec2::ZERO) += pull;
        }
    }

    apply_position_deltas(app, position_deltas);
}

fn semantic_pair_similarity(a: &SemanticClassVector, b: &SemanticClassVector) -> f32 {
    if a.classes.is_empty() || b.classes.is_empty() {
        return 0.0;
    }

    let mut best = 0.0_f32;
    for ca in &a.classes {
        for cb in &b.classes {
            let similarity = 1.0 - ca.distance(cb);
            if similarity > best {
                best = similarity;
            }
        }
    }
    best
}

/// Headless physics scenario helpers used by both unit tests and the scenario suite.
///
/// These helpers compute computable properties from an [`EguiGraph`] position
/// snapshot without rendering. They correspond to the properties defined in
/// `design_docs/graphshell_docs/implementation_strategy/canvas/2026-03-14_canvas_behavior_contract.md §2`.
#[cfg(test)]
pub(crate) mod scenario_helpers {
    use crate::model::graph::egui_adapter::EguiGraph;

    /// Default canvas rect used by headless scenario tests.
    pub(crate) fn test_canvas() -> egui::Rect {
        egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::new(800.0, 600.0))
    }

    /// Average displacement proxy for kinetic energy.
    ///
    /// Maps to `last_avg_displacement` from [`FrBaseState`]. Returns `None`
    /// when no steps have been taken yet.
    pub(crate) fn last_avg_displacement(layout_state: &super::GraphPhysicsState) -> Option<f32> {
        layout_state.base.last_avg_displacement
    }

    /// Returns true when `last_avg_displacement < threshold` for the given state.
    ///
    /// Used as the convergence check per spec §2.1 (KE < convergence_threshold).
    pub(crate) fn is_converged(layout_state: &super::GraphPhysicsState, threshold: f32) -> bool {
        layout_state
            .base
            .last_avg_displacement
            .is_some_and(|avg| avg < threshold)
    }

    /// Count node pairs whose bounding circles overlap (spec §2.2).
    ///
    /// `node_radius` is the node display radius; `overlap_margin` is the extra
    /// clearance (spec default 4.0 px).
    pub(crate) fn overlap_count(g: &EguiGraph, node_radius: f32, overlap_margin: f32) -> usize {
        let positions: Vec<_> = g
            .g()
            .node_indices()
            .filter_map(|idx| g.g().node_weight(idx).map(|n| n.location()))
            .collect();
        let min_dist = 2.0 * (node_radius + overlap_margin);
        let mut count = 0;
        for i in 0..positions.len() {
            for j in (i + 1)..positions.len() {
                if (positions[i] - positions[j]).length() < min_dist {
                    count += 1;
                }
            }
        }
        count
    }

    /// Mean edge length across all edges in the graph (spec §2.4).
    pub(crate) fn mean_edge_length(g: &EguiGraph) -> f32 {
        let mut total = 0.0_f32;
        let mut count = 0_usize;
        for edge in g.g().edge_indices() {
            let Some((a, b)) = g.g().edge_endpoints(edge) else {
                continue;
            };
            let pos_a = g.g().node_weight(a).map(|n| n.location());
            let pos_b = g.g().node_weight(b).map(|n| n.location());
            if let (Some(pa), Some(pb)) = (pos_a, pos_b) {
                total += (pa - pb).length();
                count += 1;
            }
        }
        if count == 0 {
            0.0
        } else {
            total / count as f32
        }
    }

    /// Edge length coefficient of variation (spec §2.4).
    pub(crate) fn edge_len_cv(g: &EguiGraph) -> f32 {
        let lengths: Vec<f32> = g
            .g()
            .edge_indices()
            .filter_map(|e| {
                let (a, b) = g.g().edge_endpoints(e)?;
                let pa = g.g().node_weight(a)?.location();
                let pb = g.g().node_weight(b)?.location();
                Some((pa - pb).length())
            })
            .collect();
        if lengths.len() < 2 {
            return 0.0;
        }
        let mean = lengths.iter().sum::<f32>() / lengths.len() as f32;
        if mean < f32::EPSILON {
            return 0.0;
        }
        let variance =
            lengths.iter().map(|l| (l - mean).powi(2)).sum::<f32>() / lengths.len() as f32;
        variance.sqrt() / mean
    }

    /// Node positions as a flat Vec for post-convergence measurements.
    pub(crate) fn node_positions(g: &EguiGraph) -> Vec<egui::Pos2> {
        g.g()
            .node_indices()
            .filter_map(|idx| g.g().node_weight(idx).map(|n| n.location()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registries::atomic::knowledge::{CompactCode, SemanticClassVector};
    use crate::registries::atomic::lens::PhysicsProfile;

    fn node_distance(app: &GraphBrowserApp, a: NodeKey, b: NodeKey) -> f32 {
        let pa = app.domain_graph().node_projected_position(a).unwrap();
        let pb = app.domain_graph().node_projected_position(b).unwrap();
        ((pb.x - pa.x).powi(2) + (pb.y - pa.y).powi(2)).sqrt()
    }

    #[test]
    fn apply_graph_physics_tuning_updates_force_directed_state() {
        let mut state = GraphPhysicsState::default();
        let tuning = GraphPhysicsTuning {
            repulsion_strength: 0.7,
            attraction_strength: 0.15,
            gravity_strength: 0.31,
            damping: 0.42,
        };

        apply_graph_physics_tuning(&mut state, tuning);

        assert_eq!(state.base.c_repulse, 0.7);
        assert_eq!(state.base.c_attract, 0.15);
        assert_eq!(state.base.damping, 0.42);
        assert_eq!(state.extras.0.params.c, 0.31);
    }

    #[test]
    fn graph_physics_extension_config_preserves_parameterized_helpers() {
        let config = GraphPhysicsExtensionConfig {
            degree_repulsion: Some(DegreeRepulsionConfig::mild()),
            domain_clustering: None,
            semantic_clustering: Some(SemanticClusteringConfig {
                strength: 0.17,
                similarity_floor: 0.10,
            }),
            hub_pull: Some(HubPullConfig::default()),
            frame_affinity_enabled: false,
        };

        assert_eq!(config.degree_repulsion, Some(DegreeRepulsionConfig::mild()));
        assert_eq!(
            config.semantic_clustering,
            Some(SemanticClusteringConfig {
                strength: 0.17,
                similarity_floor: 0.10,
            })
        );
        assert_eq!(config.hub_pull, Some(HubPullConfig::default()));
    }

    #[test]
    fn graph_physics_extension_config_reports_enabled_extensions() {
        let disabled = GraphPhysicsExtensionConfig {
            degree_repulsion: None,
            domain_clustering: None,
            semantic_clustering: None,
            hub_pull: None,
            frame_affinity_enabled: false,
        };
        let enabled = GraphPhysicsExtensionConfig {
            degree_repulsion: None,
            domain_clustering: Some(DomainClusteringConfig { strength: 0.08 }),
            semantic_clustering: None,
            hub_pull: None,
            frame_affinity_enabled: false,
        };

        assert!(!disabled.any_enabled());
        assert!(enabled.any_enabled());
    }

    #[test]
    fn registrable_domain_key_uses_common_etld_plus_one_heuristic() {
        assert_eq!(
            registrable_domain_key("https://www.docs.example.co.uk/page"),
            Some("example.co.uk".to_string())
        );
        assert_eq!(
            registrable_domain_key("https://blog.example.com/post"),
            Some("example.com".to_string())
        );
    }

    #[test]
    fn degree_repulsion_moves_high_degree_hub_neighbors_apart() {
        let mut app = GraphBrowserApp::new_for_testing();
        app.workspace.graph_runtime.physics.base.is_running = true;

        let hub = app.add_node_and_sync(
            "https://hub.example".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );
        let left = app.add_node_and_sync(
            "https://left.example".to_string(),
            euclid::default::Point2D::new(-5.0, 0.0),
        );
        let right = app.add_node_and_sync(
            "https://right.example".to_string(),
            euclid::default::Point2D::new(5.0, 0.0),
        );
        let extra = app.add_node_and_sync(
            "https://extra.example".to_string(),
            euclid::default::Point2D::new(0.0, 20.0),
        );

        app.add_edge_and_sync(hub, left, crate::graph::EdgeType::Hyperlink, None);
        app.add_edge_and_sync(hub, right, crate::graph::EdgeType::Hyperlink, None);
        app.add_edge_and_sync(hub, extra, crate::graph::EdgeType::Hyperlink, None);

        let before_left = app.domain_graph().node_projected_position(left).unwrap();
        let before_right = app.domain_graph().node_projected_position(right).unwrap();

        apply_degree_repulsion_forces(&mut app, DegreeRepulsionConfig::medium());

        let after_left = app.domain_graph().node_projected_position(left).unwrap();
        let after_right = app.domain_graph().node_projected_position(right).unwrap();
        let before_distance = before_right.x - before_left.x;
        let after_distance = after_right.x - after_left.x;

        assert!(after_distance > before_distance);
    }

    #[test]
    fn domain_clustering_pulls_same_domain_nodes_toward_shared_centroid() {
        let mut app = GraphBrowserApp::new_for_testing();
        app.workspace.graph_runtime.physics.base.is_running = true;

        let a = app.add_node_and_sync(
            "https://a.example.com/one".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );
        let b = app.add_node_and_sync(
            "https://b.example.com/two".to_string(),
            euclid::default::Point2D::new(100.0, 0.0),
        );

        let before_a = app.domain_graph().node_projected_position(a).unwrap();
        let before_b = app.domain_graph().node_projected_position(b).unwrap();

        apply_domain_clustering_forces(&mut app, DomainClusteringConfig { strength: 0.08 });

        let after_a = app.domain_graph().node_projected_position(a).unwrap();
        let after_b = app.domain_graph().node_projected_position(b).unwrap();

        assert!(after_a.x > before_a.x);
        assert!(after_b.x < before_b.x);
    }

    #[test]
    fn hub_pull_moves_leaf_toward_nearby_hub() {
        let mut app = GraphBrowserApp::new_for_testing();
        app.workspace.graph_runtime.physics.base.is_running = true;

        let hub = app.add_node_and_sync(
            "https://hub.example".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );
        let leaf = app.add_node_and_sync(
            "https://leaf.example".to_string(),
            euclid::default::Point2D::new(80.0, 0.0),
        );
        let extra_a = app.add_node_and_sync(
            "https://extra-a.example".to_string(),
            euclid::default::Point2D::new(-20.0, 20.0),
        );
        let extra_b = app.add_node_and_sync(
            "https://extra-b.example".to_string(),
            euclid::default::Point2D::new(20.0, 20.0),
        );

        app.add_edge_and_sync(hub, leaf, crate::graph::EdgeType::Hyperlink, None);
        app.add_edge_and_sync(hub, extra_a, crate::graph::EdgeType::Hyperlink, None);
        app.add_edge_and_sync(hub, extra_b, crate::graph::EdgeType::Hyperlink, None);

        let before_leaf = app.domain_graph().node_projected_position(leaf).unwrap();
        apply_hub_pull_forces(&mut app, HubPullConfig::default());
        let after_leaf = app.domain_graph().node_projected_position(leaf).unwrap();

        assert!(after_leaf.x < before_leaf.x);
    }

    #[test]
    fn archipelago_profile_reduces_same_domain_distance_vs_drift() {
        let mut drift_app = GraphBrowserApp::new_for_testing();
        drift_app.workspace.graph_runtime.physics.base.is_running = true;
        let drift_a = drift_app.add_node_and_sync(
            "https://a.example.com/one".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );
        let drift_b = drift_app.add_node_and_sync(
            "https://b.example.com/two".to_string(),
            euclid::default::Point2D::new(120.0, 0.0),
        );

        let mut archipelago_app = GraphBrowserApp::new_for_testing();
        archipelago_app
            .workspace
            .graph_runtime
            .physics
            .base
            .is_running = true;
        let arch_a = archipelago_app.add_node_and_sync(
            "https://a.example.com/one".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );
        let arch_b = archipelago_app.add_node_and_sync(
            "https://b.example.com/two".to_string(),
            euclid::default::Point2D::new(120.0, 0.0),
        );

        apply_graph_physics_extensions(
            &mut drift_app,
            Some(PhysicsProfile::drift().graph_physics_extensions(false)),
        );
        apply_graph_physics_extensions(
            &mut archipelago_app,
            Some(PhysicsProfile::archipelago().graph_physics_extensions(false)),
        );

        assert!(
            node_distance(&archipelago_app, arch_a, arch_b)
                < node_distance(&drift_app, drift_a, drift_b)
        );
    }

    #[test]
    fn resonance_profile_reduces_semantic_pair_distance_vs_drift() {
        let vector = SemanticClassVector::from_codes(vec![CompactCode(vec![5, 1, 2])]);

        let mut drift_app = GraphBrowserApp::new_for_testing();
        drift_app.workspace.graph_runtime.physics.base.is_running = true;
        let drift_a = drift_app.add_node_and_sync(
            "https://alpha.example".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );
        let drift_b = drift_app.add_node_and_sync(
            "https://beta.example".to_string(),
            euclid::default::Point2D::new(100.0, 0.0),
        );
        drift_app
            .workspace
            .graph_runtime
            .semantic_index
            .insert(drift_a, vector.clone());
        drift_app
            .workspace
            .graph_runtime
            .semantic_index
            .insert(drift_b, vector.clone());

        let mut resonance_app = GraphBrowserApp::new_for_testing();
        resonance_app
            .workspace
            .graph_runtime
            .physics
            .base
            .is_running = true;
        let resonance_a = resonance_app.add_node_and_sync(
            "https://alpha.example".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );
        let resonance_b = resonance_app.add_node_and_sync(
            "https://beta.example".to_string(),
            euclid::default::Point2D::new(100.0, 0.0),
        );
        resonance_app
            .workspace
            .graph_runtime
            .semantic_index
            .insert(resonance_a, vector.clone());
        resonance_app
            .workspace
            .graph_runtime
            .semantic_index
            .insert(resonance_b, vector);

        apply_graph_physics_extensions(
            &mut drift_app,
            Some(PhysicsProfile::drift().graph_physics_extensions(false)),
        );
        apply_graph_physics_extensions(
            &mut resonance_app,
            Some(PhysicsProfile::resonance().graph_physics_extensions(false)),
        );

        assert!(
            node_distance(&resonance_app, resonance_a, resonance_b)
                < node_distance(&drift_app, drift_a, drift_b)
        );
    }

    #[test]
    fn constellation_profile_keeps_leaves_closer_to_hub_than_settle() {
        let mut settle_app = GraphBrowserApp::new_for_testing();
        settle_app.workspace.graph_runtime.physics.base.is_running = true;
        let settle_hub = settle_app.add_node_and_sync(
            "https://hub.example".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );
        let settle_leaf_a = settle_app.add_node_and_sync(
            "https://leaf-a.example".to_string(),
            euclid::default::Point2D::new(180.0, -20.0),
        );
        let settle_leaf_b = settle_app.add_node_and_sync(
            "https://leaf-b.example".to_string(),
            euclid::default::Point2D::new(186.0, 18.0),
        );
        let settle_leaf_c = settle_app.add_node_and_sync(
            "https://leaf-c.example".to_string(),
            euclid::default::Point2D::new(194.0, 0.0),
        );

        for leaf in [settle_leaf_a, settle_leaf_b, settle_leaf_c] {
            settle_app.add_edge_and_sync(settle_hub, leaf, crate::graph::EdgeType::Hyperlink, None);
        }

        let mut constellation_app = GraphBrowserApp::new_for_testing();
        constellation_app
            .workspace
            .graph_runtime
            .physics
            .base
            .is_running = true;
        let constellation_hub = constellation_app.add_node_and_sync(
            "https://hub.example".to_string(),
            euclid::default::Point2D::new(0.0, 0.0),
        );
        let constellation_leaf_a = constellation_app.add_node_and_sync(
            "https://leaf-a.example".to_string(),
            euclid::default::Point2D::new(180.0, -20.0),
        );
        let constellation_leaf_b = constellation_app.add_node_and_sync(
            "https://leaf-b.example".to_string(),
            euclid::default::Point2D::new(186.0, 18.0),
        );
        let constellation_leaf_c = constellation_app.add_node_and_sync(
            "https://leaf-c.example".to_string(),
            euclid::default::Point2D::new(194.0, 0.0),
        );

        for leaf in [
            constellation_leaf_a,
            constellation_leaf_b,
            constellation_leaf_c,
        ] {
            constellation_app.add_edge_and_sync(
                constellation_hub,
                leaf,
                crate::graph::EdgeType::Hyperlink,
                None,
            );
        }

        apply_graph_physics_extensions(
            &mut settle_app,
            Some(PhysicsProfile::settle().graph_physics_extensions(false)),
        );
        apply_graph_physics_extensions(
            &mut constellation_app,
            Some(PhysicsProfile::constellation().graph_physics_extensions(false)),
        );

        let settle_avg = [settle_leaf_a, settle_leaf_b, settle_leaf_c]
            .into_iter()
            .map(|leaf| node_distance(&settle_app, settle_hub, leaf))
            .sum::<f32>()
            / 3.0;
        let constellation_avg = [
            constellation_leaf_a,
            constellation_leaf_b,
            constellation_leaf_c,
        ]
        .into_iter()
        .map(|leaf| node_distance(&constellation_app, constellation_hub, leaf))
        .sum::<f32>()
            / 3.0;

        assert!(constellation_avg < settle_avg);
    }
}
