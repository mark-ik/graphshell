/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Rapier2D Layout adapter — bridges [`RapierSceneWorld`] to the
//! [`super::Layout`] trait so the existing scene-physics runtime becomes a
//! peer of other layouts rather than a parallel pipeline.
//!
//! **Persistent world**: the layout keeps a single [`RapierSceneWorld`]
//! alive across frames so rigid-body momentum, angular velocity, and
//! joint-constraint state accumulate the way users expect. The world is
//! rebuilt only when topology (nodes / edges / pinned set / dragging set)
//! changes. Between rebuilds, dynamic non-dragged nodes are owned by
//! rapier; pinned and mid-drag nodes are driven kinematically by the host.
//!
//! Available behind the `simulate` feature flag (which pulls in rapier2d).

use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use euclid::default::{Point2D, Vector2D};
use serde::{Deserialize, Serialize};

use super::{Layout, LayoutExtras};
#[allow(unused_imports)]
use crate::camera::CanvasViewport;
use crate::scene::CanvasSceneInput;
use crate::scene_composition::{
    ColliderSpec, EdgeJointSpec, NodeAvatarBinding, PhysicsMaterial, SceneBodyKind,
};
use crate::scene_physics::NodeSnapshot;
use crate::simulate::RapierSceneWorld;

/// State for the rapier adapter. Step count, integration tuning, and
/// world-gravity parameters. The persistent world itself lives on
/// [`RapierLayout`] (not `State`) because it is not serde-round-trippable.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct RapierLayoutState {
    pub step_count: u64,
    /// Simulation timestep in seconds. Default 1/60.
    pub dt: f32,
    /// World-space gravity applied each step, in units/sec². For graph
    /// canvases this is usually `(0, 0)`.
    pub gravity_x: f32,
    pub gravity_y: f32,
}

/// How each graph edge maps to a physics joint in the rapier world.
///
/// Determines the constraint force profile — spring-like (restoring),
/// rope-like (taut above a max), distance-like (fixed both ways), or
/// disabled (topology edges, no physics coupling).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum EdgeJoint {
    /// Hooke-style spring. Pulls toward `rest_length` whether stretched
    /// or compressed. Soft constraint.
    Spring {
        rest_length: f32,
        stiffness: f32,
        damping: f32,
    },
    /// Rope — only pulls when stretched beyond `max_length`; no force
    /// when slack. Implemented as a spring with only above-rest-length
    /// force (approximated via high stiffness + short rest).
    Rope { max_length: f32, stiffness: f32 },
    /// Fixed distance — keeps nodes exactly `length` apart in either
    /// direction. Implemented as a very-stiff spring; true distance
    /// constraints require `rapier2d`'s joint API which this adapter
    /// treats as a stiff-spring approximation for now.
    Distance { length: f32 },
    /// No joint. Edges are topology-only; nodes don't feel any force
    /// from graph adjacency.
    None,
}

impl Default for EdgeJoint {
    fn default() -> Self {
        Self::Spring {
            rest_length: 100.0,
            stiffness: 50.0,
            damping: 1.0,
        }
    }
}

/// How pinned nodes and unpinned nodes map to rapier body kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BodyKindPolicy {
    /// Pinned → `Static` (immovable). Unpinned → `Dynamic`. Default.
    /// Pinned nodes act as rigid anchors other nodes can collide with.
    /// Note: under this policy, dragged pinned nodes still don't move —
    /// use `PinnedKinematic` for draggable pins.
    PinnedStatic,
    /// Pinned → `KinematicPositionBased`. Unpinned → `Dynamic`. Pinned
    /// nodes are user-controlled (drag-set-position) but still rendered
    /// by the physics pipeline; useful when the host drives pinned
    /// positions every frame.
    PinnedKinematic,
    /// All → `KinematicPositionBased`. No dynamic simulation — the host
    /// drives every node position externally; rapier just runs collision
    /// detection and events. Suitable for read-only physics overlays.
    AllKinematic,
}

impl Default for BodyKindPolicy {
    fn default() -> Self {
        Self::PinnedStatic
    }
}

/// Tuning for default body/collider construction when the caller does not
/// supply bindings in [`LayoutExtras`] (via a future extension slot).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RapierLayoutConfig {
    /// Node radius used when auto-generating ball colliders from scene nodes.
    pub default_radius: f32,
    /// How each graph edge maps to a physics joint.
    pub edge_joint: EdgeJoint,
    /// How pinned / unpinned nodes map to rapier body kinds.
    pub body_kind_policy: BodyKindPolicy,
    pub material: PhysicsMaterial,
}

impl Default for RapierLayoutConfig {
    fn default() -> Self {
        Self {
            default_radius: 16.0,
            edge_joint: EdgeJoint::default(),
            body_kind_policy: BodyKindPolicy::default(),
            material: PhysicsMaterial::default(),
        }
    }
}

/// Persistent-world rapier adapter. Owns a [`RapierSceneWorld`] across
/// frames; rebuilds only on topology change.
///
/// Drag handoff uses rapier's `set_body_type` to flip nodes between
/// `KinematicPositionBased` (while being dragged) and `Dynamic` (free
/// flight) in place, so the drag-motion velocity survives release —
/// the body carries the momentum the user "threw" it with rather than
/// stopping dead the frame the pointer lifts. Dragging is therefore
/// **not** part of the topology hash: transitioning drag state never
/// triggers a world rebuild.
pub struct RapierLayout<N>
where
    N: Clone + Eq + Hash,
{
    pub config: RapierLayoutConfig,
    world: Option<RapierSceneWorld<N>>,
    /// Hash of the scene topology (sorted node ids + sorted edge pairs +
    /// sorted pinned set) the current world was built from. Rebuild if
    /// it drifts. Zero before the first build.
    world_topology_hash: u64,
    /// Nodes that were actively dragged on the previous step. Compared
    /// against the current frame's `dragging` set to detect drag
    /// start/end transitions without rebuilding the world.
    prior_dragging: std::collections::HashSet<N>,
    /// Most recent host-driven position of each actively-dragged node.
    /// Paired with `last_drag_velocity` to produce the drag-motion
    /// velocity that seeds the newly-dynamic body on release.
    last_drag_position: HashMap<N, Point2D<f32>>,
    /// Computed per-second velocity for each actively-dragged node,
    /// derived from the inter-frame position delta during the drag.
    /// Captured while the drag is ongoing so a release frame with
    /// zero motion still hands off the prior drag velocity.
    last_drag_velocity: HashMap<N, Vector2D<f32>>,
}

impl<N> std::fmt::Debug for RapierLayout<N>
where
    N: Clone + Eq + Hash,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RapierLayout")
            .field("config", &self.config)
            .field("world_present", &self.world.is_some())
            .field("world_topology_hash", &self.world_topology_hash)
            .finish()
    }
}

impl<N> Default for RapierLayout<N>
where
    N: Clone + Eq + Hash,
{
    fn default() -> Self {
        Self {
            config: RapierLayoutConfig::default(),
            world: None,
            world_topology_hash: 0,
            prior_dragging: std::collections::HashSet::new(),
            last_drag_position: HashMap::new(),
            last_drag_velocity: HashMap::new(),
        }
    }
}

impl<N> RapierLayout<N>
where
    N: Clone + Eq + Hash,
{
    pub fn new(config: RapierLayoutConfig) -> Self {
        Self {
            config,
            world: None,
            world_topology_hash: 0,
            prior_dragging: std::collections::HashSet::new(),
            last_drag_position: HashMap::new(),
            last_drag_velocity: HashMap::new(),
        }
    }

    /// True when the layout has a persistent world built from at least
    /// one previous step. Tests use this to assert rebuild paths.
    pub fn has_world(&self) -> bool {
        self.world.is_some()
    }

    /// Topology hash of the current world; zero before the first build.
    pub fn current_topology_hash(&self) -> u64 {
        self.world_topology_hash
    }
}

impl<N> Layout<N> for RapierLayout<N>
where
    N: Clone + Eq + Hash + Ord,
{
    type State = RapierLayoutState;

    fn step(
        &mut self,
        scene: &CanvasSceneInput<N>,
        state: &mut Self::State,
        _dt_override: f32,
        _viewport: &CanvasViewport,
        extras: &LayoutExtras<N>,
    ) -> HashMap<N, Vector2D<f32>> {
        state.step_count = state.step_count.saturating_add(1);
        if scene.nodes.is_empty() {
            // Drop any persistent world — nothing to simulate.
            self.world = None;
            self.world_topology_hash = 0;
            self.prior_dragging.clear();
            self.last_drag_position.clear();
            self.last_drag_velocity.clear();
            return HashMap::new();
        }

        // Decide whether to rebuild the persistent world from scratch.
        // `dragging` is intentionally NOT in the hash — drag transitions
        // flip body type in place via set_body_type so velocity survives.
        let topology_hash = topology_hash(scene, extras);
        let must_rebuild = self.world.is_none() || self.world_topology_hash != topology_hash;

        if must_rebuild {
            self.world = Some(build_world(&self.config, scene, extras, state));
            self.world_topology_hash = topology_hash;
            // World rebuilt from scratch — any prior drag-velocity memory
            // is meaningless (bodies are fresh); reset the handoff cache.
            self.last_drag_position.clear();
            self.last_drag_velocity.clear();
        }

        let world = self.world.as_mut().expect("world built above");

        // Configure per-step simulation parameters.
        world.set_timestep(if state.dt > 0.0 { state.dt } else { 1.0 / 60.0 });

        // Handle drag state transitions in place. A node that just
        // started being dragged flips kinematic. A node that just
        // stopped being dragged flips back to Dynamic and inherits
        // the drag-motion velocity recorded on the last drag frame,
        // so it "throws" instead of stopping dead on release. Both
        // transitions preserve translation and avoid a world rebuild.
        //
        // Velocity is sampled during *ongoing* drag frames — using
        // only the release-frame delta would lose the drag's momentum
        // when the user lifts the pointer without moving.
        let dt = if state.dt > 0.0 { state.dt } else { 1.0 / 60.0 };
        for node in &scene.nodes {
            let was_dragging = self.prior_dragging.contains(&node.id);
            let is_dragging = extras.dragging.contains(&node.id);
            match (was_dragging, is_dragging) {
                (false, true) => {
                    // Drag start: flip to kinematic. Skip nodes that
                    // are already Static under a `PinnedStatic` policy —
                    // we'd lose the "immovable anchor" semantics.
                    if self.config.body_kind_policy != BodyKindPolicy::PinnedStatic
                        || !extras.pinned.contains(&node.id)
                    {
                        world.mark_body_kinematic(&node.id);
                    }
                }
                (true, true) => {
                    // Ongoing drag: sample velocity from the prior
                    // drag frame's position to now. If there was no
                    // prior drag position (start-of-drag frame counts
                    // as `(false, true)`, not here), leave the cached
                    // velocity untouched.
                    if let Some(last_pos) = self.last_drag_position.get(&node.id).copied() {
                        let delta = node.position - last_pos;
                        let velocity = Vector2D::new(delta.x / dt, delta.y / dt);
                        self.last_drag_velocity.insert(node.id.clone(), velocity);
                    }
                }
                (true, false) => {
                    // Drag release: pick up the last recorded drag
                    // velocity, then flip to Dynamic with that linvel.
                    // Skip handoff for pinned nodes — release leaves
                    // their policy kind unchanged.
                    if !extras.pinned.contains(&node.id) {
                        let handoff = self
                            .last_drag_velocity
                            .get(&node.id)
                            .copied()
                            .unwrap_or_else(Vector2D::zero);
                        world.hand_off_kinematic_to_dynamic(&node.id, handoff);
                    }
                }
                _ => {}
            }
        }

        // Sync host-driven positions in before stepping. For dynamic
        // non-dragged non-pinned nodes we rebase the "initial position"
        // so that `read_positions()` returns the delta accumulated by
        // this step only. For pinned and dragging nodes we drive the
        // kinematic target.
        for node in &scene.nodes {
            let is_pinned = extras.pinned.contains(&node.id);
            let is_dragging = extras.dragging.contains(&node.id);
            if is_dragging {
                // Remember the drag position for the next step's
                // release-velocity computation.
                self.last_drag_position
                    .insert(node.id.clone(), node.position);
            }
            if is_pinned || is_dragging {
                // Drive the kinematic target; rapier interpolates.
                world.set_kinematic_position(&node.id, node.position);
                // Re-anchor the initial so read_positions never reports
                // a delta for this kinematic node.
                world.rebase_initial_position(&node.id, node.position);
            } else {
                // Dynamic body: let rapier own the current position.
                // Re-anchor the initial to the scene's current position
                // so `read_positions()` reports the step's delta only.
                let current = world.get_position(&node.id).unwrap_or(node.position);
                world.rebase_initial_position(&node.id, current);
            }
        }

        world.step(Vector2D::new(state.gravity_x, state.gravity_y));

        // Read deltas for non-kinematic nodes only. Kinematic (pinned or
        // dragging) bodies are owned by the host — any delta on them is
        // spurious and must be filtered.
        let mut deltas = world.read_positions();
        deltas.retain(|key, _| {
            !extras.pinned.contains(key) && !extras.dragging.contains(key)
        });

        // Remember this frame's dragging set for the next step's
        // transition detection. Also drop `last_drag_position` +
        // `last_drag_velocity` entries for nodes no longer being
        // dragged — they've been consumed into the handoff (or were
        // never dragged) and would stale otherwise.
        self.prior_dragging = extras.dragging.iter().cloned().collect();
        self.last_drag_position
            .retain(|key, _| extras.dragging.contains(key));
        self.last_drag_velocity
            .retain(|key, _| extras.dragging.contains(key));

        deltas
    }
}

/// Decide which rapier body kind to use for a given node given the
/// per-node pinned/dragging flags and the policy.
///
/// Nodes actively mid-drag are always kinematic (regardless of policy)
/// so the host can drive their position each frame. Pinned nodes follow
/// the policy. All other nodes are dynamic unless `AllKinematic` forces
/// everything to kinematic.
fn effective_body_kind(
    policy: BodyKindPolicy,
    pinned: bool,
    dragging: bool,
) -> SceneBodyKind {
    if dragging {
        return SceneBodyKind::KinematicPositionBased;
    }
    match policy {
        BodyKindPolicy::PinnedStatic => {
            if pinned {
                SceneBodyKind::Static
            } else {
                SceneBodyKind::Dynamic
            }
        }
        BodyKindPolicy::PinnedKinematic => {
            if pinned {
                SceneBodyKind::KinematicPositionBased
            } else {
                SceneBodyKind::Dynamic
            }
        }
        BodyKindPolicy::AllKinematic => SceneBodyKind::KinematicPositionBased,
    }
}

fn edge_joint_spec_for(joint: EdgeJoint) -> EdgeJointSpec {
    match joint {
        EdgeJoint::Spring {
            rest_length,
            stiffness,
            damping,
        } => EdgeJointSpec {
            rest_length,
            stiffness,
            damping,
        },
        EdgeJoint::Rope {
            max_length,
            stiffness,
        } => EdgeJointSpec {
            rest_length: max_length * 0.5,
            stiffness,
            damping: 0.5,
        },
        EdgeJoint::Distance { length } => EdgeJointSpec {
            rest_length: length,
            stiffness: 1000.0,
            damping: 2.0,
        },
        EdgeJoint::None => EdgeJointSpec {
            rest_length: 0.0,
            stiffness: 0.0,
            damping: 0.0,
        },
    }
}

/// Deterministic topology hash for rebuild detection.
///
/// Covers sorted node ids, sorted directed edge pairs, sorted pinned set,
/// and sorted dragging set. Ordering guarantees identical input → identical
/// hash even when the underlying `CanvasSceneInput` vec order drifts.
fn topology_hash<N>(scene: &CanvasSceneInput<N>, extras: &LayoutExtras<N>) -> u64
where
    N: Clone + Eq + Hash + Ord,
{
    let mut node_ids: Vec<&N> = scene.nodes.iter().map(|n| &n.id).collect();
    node_ids.sort();
    let mut edge_pairs: Vec<(&N, &N)> = scene
        .edges
        .iter()
        .map(|e| (&e.source, &e.target))
        .collect();
    edge_pairs.sort();
    let mut pinned: Vec<&N> = extras.pinned.iter().collect();
    pinned.sort();
    // `dragging` is deliberately not in the hash — drag transitions
    // flip body type in place so linvel survives release.

    let mut hasher = DefaultHasher::new();
    0xD15C_A87Cu64.hash(&mut hasher); // domain tag for this hash family
    node_ids.hash(&mut hasher);
    edge_pairs.hash(&mut hasher);
    pinned.hash(&mut hasher);
    hasher.finish()
}

fn build_world<N>(
    config: &RapierLayoutConfig,
    scene: &CanvasSceneInput<N>,
    extras: &LayoutExtras<N>,
    state: &RapierLayoutState,
) -> RapierSceneWorld<N>
where
    N: Clone + Eq + Hash,
{
    let snapshots: Vec<NodeSnapshot<N>> = scene
        .nodes
        .iter()
        .map(|node| NodeSnapshot {
            id: node.id.clone(),
            position: node.position,
            radius: if node.radius > 0.0 {
                node.radius
            } else {
                config.default_radius
            },
            pinned: extras.pinned.contains(&node.id),
        })
        .collect();

    let bindings: Vec<NodeAvatarBinding<N>> = scene
        .nodes
        .iter()
        .map(|node| {
            let pinned = extras.pinned.contains(&node.id);
            let dragging = extras.dragging.contains(&node.id);
            let radius = if node.radius > 0.0 {
                node.radius
            } else {
                config.default_radius
            };
            NodeAvatarBinding {
                node_id: node.id.clone(),
                collider: ColliderSpec::circle(radius),
                body_kind: effective_body_kind(config.body_kind_policy, pinned, dragging),
                material: config.material,
            }
        })
        .collect();

    let edge_bindings: Vec<(N, N, EdgeJointSpec)> = match config.edge_joint {
        EdgeJoint::None => Vec::new(),
        joint => scene
            .edges
            .iter()
            .map(|edge| {
                (
                    edge.source.clone(),
                    edge.target.clone(),
                    edge_joint_spec_for(joint),
                )
            })
            .collect(),
    };

    let mut world = RapierSceneWorld::new();
    world.set_timestep(if state.dt > 0.0 { state.dt } else { 1.0 / 60.0 });
    world.build_bodies(&bindings, &snapshots);
    world.build_edge_joints(&edge_bindings);
    world
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::projection::ProjectionMode;
    use crate::scene::{CanvasEdge, CanvasNode, SceneMode, ViewId};
    use euclid::default::{Point2D, Rect, Size2D};

    fn viewport() -> CanvasViewport {
        CanvasViewport {
            rect: Rect::new(Point2D::new(0.0, 0.0), Size2D::new(1000.0, 1000.0)),
            scale_factor: 1.0,
        }
    }

    fn scene(nodes: Vec<(u32, f32, f32)>, edges: Vec<(u32, u32)>) -> CanvasSceneInput<u32> {
        CanvasSceneInput {
            view_id: ViewId(0),
            nodes: nodes
                .into_iter()
                .map(|(id, x, y)| CanvasNode {
                    id,
                    position: Point2D::new(x, y),
                    radius: 16.0,
                    label: None,
                })
                .collect(),
            edges: edges
                .into_iter()
                .map(|(s, t)| CanvasEdge {
                    source: s,
                    target: t,
                    weight: 1.0,
                })
                .collect(),
            scene_objects: Vec::new(),
            overlays: Vec::new(),
            scene_mode: SceneMode::Browse,
            projection: ProjectionMode::TwoD,
        }
    }

    #[test]
    fn rapier_adapter_reports_zero_delta_for_isolated_stationary_node() {
        let mut layout = RapierLayout::<u32>::new(RapierLayoutConfig::default());
        let mut state = RapierLayoutState::default();
        let input = scene(vec![(0, 100.0, 100.0)], vec![]);
        let deltas = layout.step(&input, &mut state, 0.0, &viewport(), &LayoutExtras::default());
        // Single node with no forces and zero gravity should not move.
        assert!(deltas.is_empty());
    }

    #[test]
    fn rapier_adapter_pinned_node_emits_no_delta() {
        let mut layout = RapierLayout::<u32>::new(RapierLayoutConfig::default());
        let mut state = RapierLayoutState {
            gravity_y: 100.0,
            ..Default::default()
        };
        let input = scene(vec![(0, 0.0, 0.0)], vec![]);
        let mut extras = LayoutExtras::default();
        extras.pinned.insert(0u32);
        let deltas = layout.step(&input, &mut state, 0.0, &viewport(), &extras);
        assert!(!deltas.contains_key(&0));
    }

    #[test]
    fn rapier_adapter_spring_pulls_nodes_toward_rest_length() {
        let mut layout = RapierLayout::<u32>::new(RapierLayoutConfig {
            edge_joint: EdgeJoint::Spring {
                rest_length: 50.0,
                stiffness: 200.0,
                damping: 0.5,
            },
            ..Default::default()
        });
        let mut state = RapierLayoutState {
            dt: 1.0 / 60.0,
            ..Default::default()
        };
        let input = scene(vec![(0, 0.0, 0.0), (1, 500.0, 0.0)], vec![(0, 1)]);
        let deltas = layout.step(&input, &mut state, 0.0, &viewport(), &LayoutExtras::default());
        assert!(!deltas.is_empty());
    }

    #[test]
    fn rapier_adapter_edge_joint_none_emits_no_coupling() {
        let mut layout = RapierLayout::<u32>::new(RapierLayoutConfig {
            edge_joint: EdgeJoint::None,
            ..Default::default()
        });
        let mut state = RapierLayoutState {
            dt: 1.0 / 60.0,
            gravity_y: 0.0,
            ..Default::default()
        };
        let input = scene(vec![(0, 0.0, 0.0), (1, 500.0, 0.0)], vec![(0, 1)]);
        let deltas = layout.step(&input, &mut state, 0.0, &viewport(), &LayoutExtras::default());
        assert!(deltas.is_empty(), "got non-empty deltas with EdgeJoint::None: {deltas:?}");
    }

    #[test]
    fn rapier_adapter_all_kinematic_policy_produces_no_dynamic_motion() {
        let mut layout = RapierLayout::<u32>::new(RapierLayoutConfig {
            body_kind_policy: BodyKindPolicy::AllKinematic,
            ..Default::default()
        });
        let mut state = RapierLayoutState {
            gravity_y: 100.0,
            dt: 1.0 / 60.0,
            ..Default::default()
        };
        let input = scene(vec![(0, 0.0, 0.0)], vec![]);
        let deltas = layout.step(&input, &mut state, 0.0, &viewport(), &LayoutExtras::default());
        assert!(deltas.is_empty());
    }

    // ── Persistent-world coverage ──────────────────────────────────────

    #[test]
    fn rapier_adapter_reuses_world_across_steps_with_unchanged_topology() {
        let mut layout = RapierLayout::<u32>::new(RapierLayoutConfig::default());
        let mut state = RapierLayoutState {
            dt: 1.0 / 60.0,
            ..Default::default()
        };
        let input = scene(vec![(0, 0.0, 0.0), (1, 100.0, 0.0)], vec![(0, 1)]);
        let _ = layout.step(&input, &mut state, 0.0, &viewport(), &LayoutExtras::default());
        let hash_after_first = layout.current_topology_hash();
        assert!(hash_after_first != 0, "first step should have built a world");

        let _ = layout.step(&input, &mut state, 0.0, &viewport(), &LayoutExtras::default());
        assert_eq!(
            layout.current_topology_hash(),
            hash_after_first,
            "unchanged topology must not change the hash"
        );
        assert!(
            layout.has_world(),
            "world must persist across steps with unchanged topology"
        );
    }

    #[test]
    fn rapier_adapter_rebuilds_world_on_topology_change() {
        let mut layout = RapierLayout::<u32>::new(RapierLayoutConfig::default());
        let mut state = RapierLayoutState {
            dt: 1.0 / 60.0,
            ..Default::default()
        };
        let input_a = scene(vec![(0, 0.0, 0.0), (1, 100.0, 0.0)], vec![(0, 1)]);
        let _ = layout.step(&input_a, &mut state, 0.0, &viewport(), &LayoutExtras::default());
        let hash_a = layout.current_topology_hash();

        // Add a node.
        let input_b = scene(
            vec![(0, 0.0, 0.0), (1, 100.0, 0.0), (2, 200.0, 0.0)],
            vec![(0, 1)],
        );
        let _ = layout.step(&input_b, &mut state, 0.0, &viewport(), &LayoutExtras::default());
        assert_ne!(
            hash_a,
            layout.current_topology_hash(),
            "added node must change topology hash"
        );
    }

    #[test]
    fn rapier_adapter_rebuilds_on_pinned_change() {
        let mut layout = RapierLayout::<u32>::new(RapierLayoutConfig::default());
        let mut state = RapierLayoutState {
            dt: 1.0 / 60.0,
            ..Default::default()
        };
        let input = scene(vec![(0, 0.0, 0.0)], vec![]);
        let _ = layout.step(&input, &mut state, 0.0, &viewport(), &LayoutExtras::default());
        let hash_unpinned = layout.current_topology_hash();

        let mut extras = LayoutExtras::default();
        extras.pinned.insert(0u32);
        let _ = layout.step(&input, &mut state, 0.0, &viewport(), &extras);
        assert_ne!(hash_unpinned, layout.current_topology_hash());
    }

    #[test]
    fn rapier_adapter_does_not_rebuild_on_dragging_change() {
        // Drag transitions flip body type in place (set_body_type) so
        // velocity survives across release. The topology hash must not
        // react to dragging — otherwise a rebuild would discard the
        // exact linvel we want to preserve.
        let mut layout = RapierLayout::<u32>::new(RapierLayoutConfig::default());
        let mut state = RapierLayoutState {
            dt: 1.0 / 60.0,
            ..Default::default()
        };
        let input = scene(vec![(0, 0.0, 0.0)], vec![]);
        let _ = layout.step(&input, &mut state, 0.0, &viewport(), &LayoutExtras::default());
        let hash_idle = layout.current_topology_hash();

        let mut extras = LayoutExtras::default();
        extras.dragging.insert(0u32);
        let _ = layout.step(&input, &mut state, 0.0, &viewport(), &extras);
        assert_eq!(
            hash_idle,
            layout.current_topology_hash(),
            "drag transitions must not change the topology hash"
        );
    }

    #[test]
    fn rapier_adapter_dragging_node_emits_no_delta() {
        let mut layout = RapierLayout::<u32>::new(RapierLayoutConfig::default());
        let mut state = RapierLayoutState {
            dt: 1.0 / 60.0,
            gravity_y: 100.0,
            ..Default::default()
        };
        let input = scene(vec![(0, 0.0, 0.0)], vec![]);
        let mut extras = LayoutExtras::default();
        extras.dragging.insert(0u32);
        let deltas = layout.step(&input, &mut state, 0.0, &viewport(), &extras);
        assert!(
            !deltas.contains_key(&0),
            "dragging node must be kinematic and filtered from delta output"
        );
    }

    #[test]
    fn rapier_adapter_momentum_accumulates_across_frames() {
        // A spring pulling two nodes together should produce monotonically
        // larger displacement as steps accumulate in the same world. With
        // the old rebuild-per-step adapter each step started from the
        // scene's stale positions and produced identical deltas.
        let mut layout = RapierLayout::<u32>::new(RapierLayoutConfig {
            edge_joint: EdgeJoint::Spring {
                rest_length: 10.0,
                stiffness: 300.0,
                damping: 0.5,
            },
            ..Default::default()
        });
        let mut state = RapierLayoutState {
            dt: 1.0 / 60.0,
            ..Default::default()
        };
        let input = scene(vec![(0, 0.0, 0.0), (1, 500.0, 0.0)], vec![(0, 1)]);
        let d1 = layout
            .step(&input, &mut state, 0.0, &viewport(), &LayoutExtras::default())
            .get(&1)
            .copied()
            .unwrap_or_default();
        let d2 = layout
            .step(&input, &mut state, 0.0, &viewport(), &LayoutExtras::default())
            .get(&1)
            .copied()
            .unwrap_or_default();
        // Second frame must have larger displacement than the first —
        // momentum/velocity carries over when the world persists.
        assert!(
            d2.length() > d1.length(),
            "expected accumulating momentum: frame1 |d|={} frame2 |d|={}",
            d1.length(),
            d2.length()
        );
    }

    #[test]
    fn rapier_adapter_empty_scene_drops_world() {
        let mut layout = RapierLayout::<u32>::new(RapierLayoutConfig::default());
        let mut state = RapierLayoutState {
            dt: 1.0 / 60.0,
            ..Default::default()
        };
        let input = scene(vec![(0, 0.0, 0.0)], vec![]);
        let _ = layout.step(&input, &mut state, 0.0, &viewport(), &LayoutExtras::default());
        assert!(layout.has_world());

        let empty = scene(vec![], vec![]);
        let _ = layout.step(&empty, &mut state, 0.0, &viewport(), &LayoutExtras::default());
        assert!(!layout.has_world(), "empty scene must drop the world");
        assert_eq!(layout.current_topology_hash(), 0);
    }

    #[test]
    fn rapier_adapter_drag_release_carries_velocity_into_free_flight() {
        // Simulate a user dragging node 0 rightward across three frames,
        // then releasing. After release the body must be Dynamic with a
        // non-zero +x linvel, carrying the drag motion forward into
        // subsequent free-flight steps rather than coming to rest at
        // the release position.
        use rapier2d::prelude::RigidBodyType;

        let mut layout = RapierLayout::<u32>::new(RapierLayoutConfig {
            edge_joint: EdgeJoint::None, // isolate velocity-handoff from spring pull
            ..Default::default()
        });
        let mut state = RapierLayoutState {
            dt: 1.0 / 60.0,
            ..Default::default()
        };
        let mut extras = LayoutExtras::default();
        extras.dragging.insert(0u32);

        // Frame 1: drag starts at (0, 0). Node position tracks the host.
        let input = scene(vec![(0, 0.0, 0.0)], vec![]);
        let _ = layout.step(&input, &mut state, 0.0, &viewport(), &extras);
        // Frame 2: drag moves to (10, 0). The body is kinematic, so
        // rapier holds it at that position.
        let input = scene(vec![(0, 10.0, 0.0)], vec![]);
        let _ = layout.step(&input, &mut state, 0.0, &viewport(), &extras);
        // Frame 3: drag moves to (30, 0). Delta this frame = 20 units
        // → expected handoff velocity on release ≈ 20 / (1/60) = 1200
        // units/sec along +x.
        let input = scene(vec![(0, 30.0, 0.0)], vec![]);
        let _ = layout.step(&input, &mut state, 0.0, &viewport(), &extras);

        // Release: remove from dragging set, keep scene position the
        // same (the user lifted the pointer without moving).
        let input = scene(vec![(0, 30.0, 0.0)], vec![]);
        let release_deltas = layout.step(
            &input,
            &mut state,
            0.0,
            &viewport(),
            &LayoutExtras::default(),
        );

        // The body must be Dynamic now — the in-place flip happened,
        // not a rebuild.
        let world = layout
            .world
            .as_ref()
            .expect("world persists across drag release");
        assert_eq!(
            world.body_type(&0u32),
            Some(RigidBodyType::Dynamic),
            "release must flip body to Dynamic"
        );

        // With the handoff velocity applied, the release-frame step
        // must report a non-zero +x delta — proving the body kept
        // moving rather than stopping dead.
        let delta = release_deltas
            .get(&0)
            .copied()
            .expect("released node must report a delta");
        assert!(
            delta.x > 0.5,
            "expected substantial +x carry on release, got {delta:?}"
        );
    }

    #[test]
    fn rapier_adapter_drag_start_flips_body_to_kinematic_without_rebuild() {
        use rapier2d::prelude::RigidBodyType;

        // PinnedKinematic policy so non-pinned nodes are Dynamic at rest.
        let mut layout = RapierLayout::<u32>::new(RapierLayoutConfig {
            body_kind_policy: BodyKindPolicy::PinnedKinematic,
            ..Default::default()
        });
        let mut state = RapierLayoutState {
            dt: 1.0 / 60.0,
            ..Default::default()
        };
        let input = scene(vec![(0, 0.0, 0.0)], vec![]);

        let _ = layout.step(&input, &mut state, 0.0, &viewport(), &LayoutExtras::default());
        let hash_before = layout.current_topology_hash();
        let world = layout.world.as_ref().unwrap();
        assert_eq!(world.body_type(&0u32), Some(RigidBodyType::Dynamic));

        // Drag start — flip in place, no rebuild.
        let mut extras = LayoutExtras::default();
        extras.dragging.insert(0u32);
        let _ = layout.step(&input, &mut state, 0.0, &viewport(), &extras);
        let world = layout.world.as_ref().unwrap();
        assert_eq!(
            world.body_type(&0u32),
            Some(RigidBodyType::KinematicPositionBased),
            "drag start must flip body to kinematic"
        );
        assert_eq!(
            layout.current_topology_hash(),
            hash_before,
            "drag start must not trigger a world rebuild"
        );
    }
}
