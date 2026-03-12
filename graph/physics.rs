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

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct GraphPhysicsExtensionConfig {
    pub(crate) degree_repulsion: bool,
    pub(crate) domain_clustering: bool,
    pub(crate) semantic_clustering: bool,
    pub(crate) semantic_strength: f32,
}

impl GraphPhysicsExtensionConfig {
    pub(crate) fn semantic_clustering_args(self) -> Option<(bool, f32)> {
        Some((self.semantic_clustering, self.semantic_strength))
    }

    pub(crate) fn any_enabled(self) -> bool {
        self.degree_repulsion || self.domain_clustering || self.semantic_clustering
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

    apply_semantic_clustering_forces(app, extensions.semantic_clustering_args());
}

pub(crate) fn apply_semantic_clustering_forces(
    app: &mut GraphBrowserApp,
    semantic_config: Option<(bool, f32)>,
) {
    let (enabled, strength) = if let Some((enabled, strength)) = semantic_config {
        (enabled, strength)
    } else {
        (false, 0.05)
    };

    if !enabled || strength < 1e-6 {
        return;
    }

    if !app.workspace.physics.base.is_running {
        return;
    }

    if app.workspace.semantic_index.is_empty() {
        return;
    }

    let tagged_nodes: Vec<(NodeKey, SemanticClassVector)> = app
        .workspace
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
            if similarity < 0.1 {
                continue;
            }

            let pos_a = app.domain_graph().node_projected_position(*key_a);
            let pos_b = app.domain_graph().node_projected_position(*key_b);

            if let (Some(pa), Some(pb)) = (pos_a, pos_b) {
                let delta = egui::Vec2::new(pb.x - pa.x, pb.y - pa.y);
                let force = delta * similarity * strength;

                *position_deltas.entry(*key_a).or_insert(egui::Vec2::ZERO) += force;
                *position_deltas.entry(*key_b).or_insert(egui::Vec2::ZERO) -= force;
            }
        }
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
    if let Some(state_mut) = app.workspace.egui_state.as_mut() {
        for (key, position) in projected_positions {
            if let Some(egui_node) = state_mut.graph.node_mut(key) {
                egui_node.set_location(position.to_pos2());
            }
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn graph_physics_extension_config_exposes_semantic_clustering_args() {
        let config = GraphPhysicsExtensionConfig {
            degree_repulsion: true,
            domain_clustering: false,
            semantic_clustering: true,
            semantic_strength: 0.17,
        };

        assert_eq!(config.semantic_clustering_args(), Some((true, 0.17)));
    }

    #[test]
    fn graph_physics_extension_config_reports_enabled_extensions() {
        let disabled = GraphPhysicsExtensionConfig {
            degree_repulsion: false,
            domain_clustering: false,
            semantic_clustering: false,
            semantic_strength: 0.17,
        };
        let enabled = GraphPhysicsExtensionConfig {
            degree_repulsion: false,
            domain_clustering: true,
            semantic_clustering: false,
            semantic_strength: 0.17,
        };

        assert!(!disabled.any_enabled());
        assert!(enabled.any_enabled());
    }
}
