/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::HashMap;

use euclid::default::{Point2D, Rect, Size2D, Vector2D};
use graph_canvas::camera::CanvasViewport;
use graph_canvas::layout as gclayout;
use graph_canvas::scene::{CanvasEdge, CanvasNode, CanvasSceneInput, SceneMode, ViewId};
use petgraph::visit::{EdgeRef as PetgraphEdgeRef, IntoEdgeReferences};

use crate::app::GraphBrowserApp;
use crate::graph::NodeKey;
use crate::registries::atomic::knowledge::SemanticClassVector;

#[allow(unused_imports)]
pub use graph_canvas::layout::ForceDirected as GraphPhysicsLayout;
pub use graph_canvas::layout::{ForceDirectedState as GraphPhysicsState, Layout, LayoutExtras};

/// Build a minimal `CanvasSceneInput` from the app's domain graph for use
/// with graph-canvas layout passes. Physics extras only read positions and
/// edges; labels and overlays are omitted.
pub(crate) fn scene_input_for_physics_pub(app: &GraphBrowserApp) -> CanvasSceneInput<NodeKey> {
    scene_input_for_physics(app)
}

pub(crate) fn scene_bounds_viewport_pub(scene: &CanvasSceneInput<NodeKey>) -> CanvasViewport {
    scene_bounds_viewport(scene)
}

pub(crate) fn pinned_set_pub(app: &GraphBrowserApp) -> std::collections::HashSet<NodeKey> {
    pinned_set(app)
}

pub(crate) fn apply_canvas_deltas_pub(
    app: &mut GraphBrowserApp,
    deltas: HashMap<NodeKey, Vector2D<f32>>,
) {
    apply_canvas_deltas(app, deltas);
}

fn scene_input_for_physics(app: &GraphBrowserApp) -> CanvasSceneInput<NodeKey> {
    let graph = app.domain_graph();
    let nodes: Vec<CanvasNode<NodeKey>> = graph
        .nodes()
        .filter_map(|(key, _node)| {
            graph.node_projected_position(key).map(|pos| CanvasNode {
                id: key,
                position: pos,
                radius: 16.0,
                label: None,
            })
        })
        .collect();
    let edges: Vec<CanvasEdge<NodeKey>> = graph
        .inner
        .edge_references()
        .map(|e| CanvasEdge {
            source: e.source(),
            target: e.target(),
            weight: 1.0,
        })
        .collect();
    CanvasSceneInput {
        view_id: ViewId(0),
        nodes,
        edges,
        scene_objects: Vec::new(),
        overlays: Vec::new(),
        scene_mode: SceneMode::Browse,
        projection: graph_canvas::projection::ProjectionMode::TwoD,
    }
}

/// A viewport that covers the full position extent of the scene, so
/// extras that are nominally viewport-independent still have a reasonable
/// fallback area to reference.
fn scene_bounds_viewport(scene: &CanvasSceneInput<NodeKey>) -> CanvasViewport {
    if scene.nodes.is_empty() {
        return CanvasViewport {
            rect: Rect::new(Point2D::new(0.0, 0.0), Size2D::new(1.0, 1.0)),
            scale_factor: 1.0,
        };
    }
    let mut min = scene.nodes[0].position;
    let mut max = scene.nodes[0].position;
    for node in scene.nodes.iter().skip(1) {
        min.x = min.x.min(node.position.x);
        min.y = min.y.min(node.position.y);
        max.x = max.x.max(node.position.x);
        max.y = max.y.max(node.position.y);
    }
    let w = (max.x - min.x).max(1.0);
    let h = (max.y - min.y).max(1.0);
    CanvasViewport {
        rect: Rect::new(min, Size2D::new(w, h)),
        scale_factor: 1.0,
    }
}

/// Collect registrable-domain keys for every node that has a URL, for use
/// with [`gclayout::DomainClustering`].
fn domain_by_node_for(app: &GraphBrowserApp) -> HashMap<NodeKey, String> {
    app.domain_graph()
        .nodes()
        .filter_map(|(key, node)| registrable_domain_key(node.url()).map(|d| (key, d)))
        .collect()
}

/// Fold the workspace's semantic index into a pairwise similarity map for
/// [`gclayout::SemanticClustering`]. Only pairs with similarity above the
/// floor are inserted; other pairs default to zero.
fn semantic_similarity_map(
    app: &GraphBrowserApp,
    similarity_floor: f32,
) -> HashMap<(NodeKey, NodeKey), f32> {
    let tagged: Vec<(NodeKey, SemanticClassVector)> = app
        .workspace
        .graph_runtime
        .semantic_index
        .iter()
        .map(|(&key, vector)| (key, vector.clone()))
        .collect();
    let mut out = HashMap::with_capacity(tagged.len() * tagged.len().saturating_sub(1) / 2);
    for i in 0..tagged.len() {
        for j in (i + 1)..tagged.len() {
            let sim = semantic_pair_similarity(&tagged[i].1, &tagged[j].1);
            if sim >= similarity_floor {
                out.insert((tagged[i].0, tagged[j].0), sim);
            }
        }
    }
    out
}

fn apply_canvas_deltas(
    app: &mut GraphBrowserApp,
    deltas: HashMap<NodeKey, Vector2D<f32>>,
) {
    if deltas.is_empty() {
        return;
    }
    let egui_deltas: HashMap<NodeKey, egui::Vec2> = deltas
        .into_iter()
        .map(|(key, d)| (key, egui::Vec2::new(d.x, d.y)))
        .collect();
    apply_position_deltas(app, egui_deltas);
}

fn pinned_set(app: &GraphBrowserApp) -> std::collections::HashSet<NodeKey> {
    app.domain_graph()
        .nodes()
        .filter_map(|(key, node)| node.is_pinned.then_some(key))
        .collect()
}

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
    state.c_repulse = tuning.repulsion_strength;
    state.c_attract = tuning.attraction_strength;
    state.damping = tuning.damping;
    state.c_gravity = tuning.gravity_strength;
}

pub(crate) fn default_graph_physics_state() -> GraphPhysicsState {
    let mut state = GraphPhysicsState::default();
    apply_graph_physics_tuning(&mut state, GraphPhysicsTuning::default());
    state.k_scale = 0.42;
    state.dt = 0.03;
    state.max_step = 3.0;
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
}

pub(crate) fn apply_degree_repulsion_forces(
    app: &mut GraphBrowserApp,
    config: DegreeRepulsionConfig,
) {
    if !app.workspace.graph_runtime.physics.is_running {
        return;
    }
    let scene = scene_input_for_physics(app);
    if scene.nodes.len() < 2 {
        return;
    }
    let viewport = scene_bounds_viewport(&scene);
    let mut extras = LayoutExtras::<NodeKey>::default();
    extras.pinned = pinned_set(app);
    let mut layout = gclayout::DegreeRepulsion::new(gclayout::DegreeRepulsionConfig {
        radius_px: config.radius_px,
        strength: config.strength,
        ..gclayout::DegreeRepulsionConfig::mild()
    });
    let mut state = gclayout::StatelessPassState::default();
    let deltas = layout.step(&scene, &mut state, 0.0, &viewport, &extras);
    apply_canvas_deltas(app, deltas);
}

pub(crate) fn apply_domain_clustering_forces(
    app: &mut GraphBrowserApp,
    config: DomainClusteringConfig,
) {
    if !app.workspace.graph_runtime.physics.is_running {
        return;
    }
    let scene = scene_input_for_physics(app);
    if scene.nodes.is_empty() {
        return;
    }
    let viewport = scene_bounds_viewport(&scene);
    let mut extras = LayoutExtras::<NodeKey>::default();
    extras.pinned = pinned_set(app);
    extras.domain_by_node = domain_by_node_for(app);
    let mut layout = gclayout::DomainClustering::<NodeKey>::new(gclayout::DomainClusteringConfig {
        strength: config.strength,
        ..Default::default()
    });
    let mut state = gclayout::StatelessPassState::default();
    let deltas = layout.step(&scene, &mut state, 0.0, &viewport, &extras);
    apply_canvas_deltas(app, deltas);
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
    if !app.workspace.graph_runtime.physics.is_running {
        return;
    }
    if app.workspace.graph_runtime.semantic_index.is_empty() {
        return;
    }
    let scene = scene_input_for_physics(app);
    if scene.nodes.len() < 2 {
        return;
    }
    let viewport = scene_bounds_viewport(&scene);
    let mut extras = LayoutExtras::<NodeKey>::default();
    extras.pinned = pinned_set(app);
    extras.semantic_similarity = semantic_similarity_map(app, config.similarity_floor);
    if extras.semantic_similarity.is_empty() {
        return;
    }
    let mut layout = gclayout::SemanticClustering::new(gclayout::SemanticClusteringConfig {
        strength: config.strength,
        similarity_floor: config.similarity_floor,
        ..Default::default()
    });
    let mut state = gclayout::StatelessPassState::default();
    let deltas = layout.step(&scene, &mut state, 0.0, &viewport, &extras);
    apply_canvas_deltas(app, deltas);
}

pub(crate) fn apply_hub_pull_forces(app: &mut GraphBrowserApp, config: HubPullConfig) {
    if !app.workspace.graph_runtime.physics.is_running {
        return;
    }
    let scene = scene_input_for_physics(app);
    if scene.nodes.len() < 2 {
        return;
    }
    let viewport = scene_bounds_viewport(&scene);
    let mut extras = LayoutExtras::<NodeKey>::default();
    extras.pinned = pinned_set(app);
    let mut layout = gclayout::HubPull::new(gclayout::HubPullConfig {
        radius_px: config.radius_px,
        strength: config.strength,
        degree_floor: config.degree_floor,
        ..Default::default()
    });
    let mut state = gclayout::StatelessPassState::default();
    let deltas = layout.step(&scene, &mut state, 0.0, &viewport, &extras);
    apply_canvas_deltas(app, deltas);
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

        assert_eq!(state.c_repulse, 0.7);
        assert_eq!(state.c_attract, 0.15);
        assert_eq!(state.damping, 0.42);
        assert_eq!(state.c_gravity, 0.31);
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
        app.workspace.graph_runtime.physics.is_running = true;

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
        app.workspace.graph_runtime.physics.is_running = true;

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
        app.workspace.graph_runtime.physics.is_running = true;

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
        drift_app.workspace.graph_runtime.physics.is_running = true;
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
            .physics.is_running = true;
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
        drift_app.workspace.graph_runtime.physics.is_running = true;
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
            .physics.is_running = true;
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
        settle_app.workspace.graph_runtime.physics.is_running = true;
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
            .physics.is_running = true;
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
