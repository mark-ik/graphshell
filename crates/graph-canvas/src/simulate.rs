/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Rapier2D simulation world for `SceneMode::Simulate`.
//!
//! This module is available behind the `simulate` feature flag. It provides
//! a `RapierSceneWorld` that wraps a full rapier2d physics pipeline and
//! exposes a simple build/step/read interface.
//!
//! The world is constructed from `NodeAvatarBinding`s and scene regions.
//! Each frame, the host calls `step()` and reads back position deltas and
//! events. Graph truth is never mutated — the host decides what to do with
//! the deltas.

use euclid::default::{Point2D, Vector2D};
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::mpsc;

use rapier2d::prelude::*;

use crate::geometry::collider_to_shape;
use crate::scene_composition::{EdgeJointSpec, NodeAvatarBinding, SceneBodyKind};
use crate::scene_physics::{NodeSnapshot, SceneEvent};
use crate::scene_region::{SceneRegion, SceneRegionId, SceneRegionShape};

/// Convert euclid Point2D to rapier/parry Vec2.
fn to_vec2(p: Point2D<f32>) -> parry2d::math::Vector {
    parry2d::math::Vector::new(p.x, p.y)
}

/// Convert euclid Vector2D to rapier/parry Vec2.
fn vec2d_to_vec2(v: Vector2D<f32>) -> parry2d::math::Vector {
    parry2d::math::Vector::new(v.x, v.y)
}

// ── World carrier ──────────────────────────────────────────────────────────

/// A Rapier2D physics world for one graph view's Simulate mode.
///
/// The world owns all rapier state and provides a high-level interface:
/// - `build_bodies()` constructs bodies and colliders from avatar bindings
/// - `step()` advances the simulation one frame
/// - `read_positions()` extracts position deltas since construction
/// - `read_events()` extracts contact/trigger events from the last step
pub struct RapierSceneWorld<N: Clone + Eq + Hash> {
    rigid_body_set: RigidBodySet,
    collider_set: ColliderSet,
    integration_parameters: IntegrationParameters,
    physics_pipeline: PhysicsPipeline,
    island_manager: IslandManager,
    broad_phase: DefaultBroadPhase,
    narrow_phase: NarrowPhase,
    impulse_joint_set: ImpulseJointSet,
    multibody_joint_set: MultibodyJointSet,
    ccd_solver: CCDSolver,
    /// Map from node ID to rapier rigid body handle.
    body_map: HashMap<N, RigidBodyHandle>,
    /// Map from rapier collider handle to region ID (for trigger events).
    region_sensor_map: HashMap<ColliderHandle, SceneRegionId>,
    /// Map from rapier collider handle to node ID (for contact events).
    collider_node_map: HashMap<ColliderHandle, N>,
    /// Original positions at construction time (for computing deltas).
    initial_positions: HashMap<N, Point2D<f32>>,
    /// Accumulated events from the last step.
    pending_events: Vec<SceneEvent<N>>,
}

impl<N: Clone + Eq + Hash> RapierSceneWorld<N> {
    /// Create a new empty world with default integration parameters.
    pub fn new() -> Self {
        Self {
            rigid_body_set: RigidBodySet::new(),
            collider_set: ColliderSet::new(),
            integration_parameters: IntegrationParameters::default(),
            physics_pipeline: PhysicsPipeline::new(),
            island_manager: IslandManager::new(),
            broad_phase: DefaultBroadPhase::new(),
            narrow_phase: NarrowPhase::new(),
            impulse_joint_set: ImpulseJointSet::new(),
            multibody_joint_set: MultibodyJointSet::new(),
            ccd_solver: CCDSolver::new(),
            body_map: HashMap::new(),
            region_sensor_map: HashMap::new(),
            collider_node_map: HashMap::new(),
            initial_positions: HashMap::new(),
            pending_events: Vec::new(),
        }
    }

    /// Set the simulation timestep (default is 1/60).
    pub fn set_timestep(&mut self, dt: f32) {
        self.integration_parameters.dt = dt;
    }

    /// Build bodies from node avatar bindings and their current positions.
    ///
    /// Each binding maps a node to a collider shape, material, and body kind.
    /// The `snapshots` provide the current positions.
    pub fn build_bodies(
        &mut self,
        bindings: &[NodeAvatarBinding<N>],
        snapshots: &[NodeSnapshot<N>],
    ) {
        let position_map: HashMap<&N, Point2D<f32>> =
            snapshots.iter().map(|s| (&s.id, s.position)).collect();

        for binding in bindings {
            let Some(&position) = position_map.get(&binding.node_id) else {
                continue;
            };

            let translation = to_vec2(position);

            // Create the rigid body.
            let body = match binding.body_kind {
                SceneBodyKind::Dynamic => RigidBodyBuilder::dynamic(),
                SceneBodyKind::Static => RigidBodyBuilder::fixed(),
                SceneBodyKind::KinematicPositionBased => {
                    RigidBodyBuilder::kinematic_position_based()
                }
                SceneBodyKind::KinematicVelocityBased => {
                    RigidBodyBuilder::kinematic_velocity_based()
                }
            }
            .translation(translation)
            .linear_damping(binding.material.linear_damping)
            .angular_damping(binding.material.angular_damping)
            .gravity_scale(binding.material.gravity_scale)
            .build();

            let body_handle = self.rigid_body_set.insert(body);
            self.body_map.insert(binding.node_id.clone(), body_handle);
            self.initial_positions
                .insert(binding.node_id.clone(), position);

            // Create the collider.
            if let Some(shape) = collider_to_shape(&binding.collider) {
                let collider = ColliderBuilder::new(shape)
                    .density(binding.material.density)
                    .friction(binding.material.friction)
                    .restitution(binding.material.restitution)
                    .active_events(ActiveEvents::COLLISION_EVENTS)
                    .build();
                let collider_handle = self.collider_set.insert_with_parent(
                    collider,
                    body_handle,
                    &mut self.rigid_body_set,
                );
                self.collider_node_map
                    .insert(collider_handle, binding.node_id.clone());
            }
        }
    }

    /// Add scene regions as sensor colliders (for trigger events).
    ///
    /// Wall regions become static solid colliders; other regions become sensors.
    pub fn build_regions(&mut self, regions: &[SceneRegion]) {
        for region in regions {
            if !region.visible {
                continue;
            }

            let (position, shape) = match region.shape {
                SceneRegionShape::Circle { center, radius } => (center, SharedShape::ball(radius)),
                SceneRegionShape::Rect { rect } => {
                    let center = Point2D::new(
                        rect.origin.x + rect.size.width * 0.5,
                        rect.origin.y + rect.size.height * 0.5,
                    );
                    let shape = SharedShape::cuboid(rect.size.width * 0.5, rect.size.height * 0.5);
                    (center, shape)
                }
            };

            let is_wall = matches!(region.effect, crate::scene_region::SceneRegionEffect::Wall);

            let body = RigidBodyBuilder::fixed()
                .translation(to_vec2(position))
                .build();
            let body_handle = self.rigid_body_set.insert(body);

            let collider = ColliderBuilder::new(shape)
                .sensor(!is_wall)
                .active_events(ActiveEvents::COLLISION_EVENTS)
                .build();
            let collider_handle = self.collider_set.insert_with_parent(
                collider,
                body_handle,
                &mut self.rigid_body_set,
            );

            if !is_wall {
                self.region_sensor_map.insert(collider_handle, region.id);
            }
        }
    }

    /// Add spring joints for graph edges.
    pub fn build_edge_joints(&mut self, edges: &[(N, N, EdgeJointSpec)]) {
        for (source, target, spec) in edges {
            let Some(&handle_a) = self.body_map.get(source) else {
                continue;
            };
            let Some(&handle_b) = self.body_map.get(target) else {
                continue;
            };

            let joint =
                SpringJointBuilder::new(spec.rest_length, spec.stiffness, spec.damping).build();
            self.impulse_joint_set
                .insert(handle_a, handle_b, joint, true);
        }
    }

    /// Step the simulation forward one frame.
    ///
    /// The `gravity` parameter is in world units per second squared.
    /// For a typical 2D graph canvas, use `Vector2D::zero()`.
    pub fn step(&mut self, gravity: Vector2D<f32>) {
        let gravity_vec = vec2d_to_vec2(gravity);

        // Collect events via std::sync::mpsc channel.
        let (collision_send, collision_recv) = mpsc::channel();
        let (contact_force_send, _contact_force_recv) = mpsc::channel();
        let event_handler = ChannelEventCollector::new(collision_send, contact_force_send);

        self.physics_pipeline.step(
            gravity_vec,
            &self.integration_parameters,
            &mut self.island_manager,
            &mut self.broad_phase,
            &mut self.narrow_phase,
            &mut self.rigid_body_set,
            &mut self.collider_set,
            &mut self.impulse_joint_set,
            &mut self.multibody_joint_set,
            &mut self.ccd_solver,
            &(),
            &event_handler,
        );

        // Process collision events into SceneEvents.
        self.pending_events.clear();
        while let Ok(event) = collision_recv.try_recv() {
            match event {
                CollisionEvent::Started(h1, h2, _flags) => {
                    self.process_collision_event(h1, h2, true);
                }
                CollisionEvent::Stopped(h1, h2, _flags) => {
                    self.process_collision_event(h1, h2, false);
                }
            }
        }
    }

    /// Process a raw rapier collision event into typed SceneEvents.
    fn process_collision_event(&mut self, h1: ColliderHandle, h2: ColliderHandle, started: bool) {
        let n1 = self.collider_node_map.get(&h1).cloned();
        let n2 = self.collider_node_map.get(&h2).cloned();
        let r1 = self.region_sensor_map.get(&h1).copied();
        let r2 = self.region_sensor_map.get(&h2).copied();

        // Node-node contact.
        if let (Some(a), Some(b)) = (n1.clone(), n2.clone()) {
            if started {
                self.pending_events.push(SceneEvent::ContactBegin { a, b });
            } else {
                self.pending_events.push(SceneEvent::ContactEnd { a, b });
            }
            return;
        }

        // Node entering/exiting a region sensor.
        let (node, region) = match (n1, n2, r1, r2) {
            (Some(node), None, _, Some(region)) => (node, region),
            (None, Some(node), Some(region), _) => (node, region),
            _ => return,
        };

        if started {
            self.pending_events
                .push(SceneEvent::TriggerEnter { node, region });
        } else {
            self.pending_events
                .push(SceneEvent::TriggerExit { node, region });
        }
    }

    /// Read position deltas since the world was constructed.
    ///
    /// Returns a map of node ID → position delta (current - initial).
    /// Only includes nodes whose position has changed beyond epsilon.
    pub fn read_positions(&self) -> HashMap<N, Vector2D<f32>> {
        let mut deltas = HashMap::new();
        for (node_id, &body_handle) in &self.body_map {
            let Some(body) = self.rigid_body_set.get(body_handle) else {
                continue;
            };
            let Some(&initial) = self.initial_positions.get(node_id) else {
                continue;
            };
            let trans = body.translation();
            let current = Point2D::new(trans.x, trans.y);
            let delta = current - initial;
            if delta.length() > f32::EPSILON {
                deltas.insert(node_id.clone(), delta);
            }
        }
        deltas
    }

    /// Read and drain pending scene events from the last step.
    pub fn read_events(&mut self) -> Vec<SceneEvent<N>> {
        std::mem::take(&mut self.pending_events)
    }

    /// Apply an impulse to a node's body.
    pub fn apply_impulse(&mut self, node_id: &N, impulse: Vector2D<f32>) {
        if let Some(&handle) = self.body_map.get(node_id) {
            if let Some(body) = self.rigid_body_set.get_mut(handle) {
                body.apply_impulse(vec2d_to_vec2(impulse), true);
            }
        }
    }

    /// Set a kinematic body's target position (for user dragging).
    pub fn set_kinematic_position(&mut self, node_id: &N, position: Point2D<f32>) {
        if let Some(&handle) = self.body_map.get(node_id) {
            if let Some(body) = self.rigid_body_set.get_mut(handle) {
                body.set_next_kinematic_translation(to_vec2(position));
            }
        }
    }

    /// Reset the stored "initial position" for a node to a new anchor,
    /// without moving the body. Persistent-world adapters use this to
    /// re-anchor between frames so `read_positions()` returns per-step
    /// deltas rather than cumulative deltas since world construction.
    pub fn rebase_initial_position(&mut self, node_id: &N, position: Point2D<f32>) {
        if let Some(initial) = self.initial_positions.get_mut(node_id) {
            *initial = position;
        }
    }

    /// Directly teleport a dynamic body to a new translation, resetting its
    /// velocity. Used by persistent-world adapters when the host moves a
    /// non-dragged node by external means (e.g., layout swap, snap, undo).
    /// For per-frame drag the caller should use `set_kinematic_position`
    /// instead and mark the body kinematic.
    pub fn set_dynamic_position(&mut self, node_id: &N, position: Point2D<f32>) {
        if let Some(&handle) = self.body_map.get(node_id) {
            if let Some(body) = self.rigid_body_set.get_mut(handle) {
                body.set_translation(to_vec2(position), true);
                body.set_linvel(vec2d_to_vec2(Vector2D::zero()), true);
                body.set_angvel(0.0, true);
            }
        }
    }

    /// Switch a node's rigid body to `KinematicPositionBased`. Used by
    /// persistent-world adapters when a node starts being dragged so
    /// the host can drive its position each frame via
    /// [`Self::set_kinematic_position`] without waking up the solver.
    ///
    /// No-op if the body is already kinematic. Does not modify
    /// translation or velocity.
    pub fn mark_body_kinematic(&mut self, node_id: &N) {
        if let Some(&handle) = self.body_map.get(node_id) {
            if let Some(body) = self.rigid_body_set.get_mut(handle) {
                if body.body_type() != RigidBodyType::KinematicPositionBased {
                    body.set_body_type(RigidBodyType::KinematicPositionBased, true);
                }
            }
        }
    }

    /// Switch a node's rigid body from `KinematicPositionBased` (or
    /// `Static`) to `Dynamic` and seed its linear velocity. Persistent-
    /// world adapters use this on drag release so the body carries the
    /// drag-motion velocity into free flight instead of coming to rest
    /// the frame the drag ends.
    ///
    /// The velocity is set explicitly rather than relying on rapier's
    /// internal kinematic-derived linvel, because kinematic bodies
    /// expose that linvel only during the step and hosts computing it
    /// from the visible drag delta produce a more predictable handoff.
    /// No-op on nodes that have no body.
    pub fn hand_off_kinematic_to_dynamic(
        &mut self,
        node_id: &N,
        handoff_velocity: Vector2D<f32>,
    ) {
        if let Some(&handle) = self.body_map.get(node_id) {
            if let Some(body) = self.rigid_body_set.get_mut(handle) {
                if body.body_type() != RigidBodyType::Dynamic {
                    body.set_body_type(RigidBodyType::Dynamic, true);
                }
                body.set_linvel(vec2d_to_vec2(handoff_velocity), true);
            }
        }
    }

    /// Current rapier body type for a node, if any.
    pub fn body_type(&self, node_id: &N) -> Option<RigidBodyType> {
        let &handle = self.body_map.get(node_id)?;
        self.rigid_body_set.get(handle).map(|b| b.body_type())
    }

    /// Get the current simulated position of a node.
    pub fn get_position(&self, node_id: &N) -> Option<Point2D<f32>> {
        let &handle = self.body_map.get(node_id)?;
        let body = self.rigid_body_set.get(handle)?;
        let trans = body.translation();
        Some(Point2D::new(trans.x, trans.y))
    }

    /// Number of node bodies in the world.
    pub fn body_count(&self) -> usize {
        self.body_map.len()
    }

    /// Whether a node has a body in the world.
    pub fn contains_node(&self, node_id: &N) -> bool {
        self.body_map.contains_key(node_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene_composition::{ColliderSpec, NodeAvatarBinding, PhysicsMaterial};
    use crate::scene_physics::NodeSnapshot;
    use crate::scene_region::{SceneRegion, SceneRegionEffect, SceneRegionId};

    fn node_snap(id: u32, x: f32, y: f32, radius: f32) -> NodeSnapshot<u32> {
        NodeSnapshot {
            id,
            position: Point2D::new(x, y),
            radius,
            pinned: false,
        }
    }

    #[test]
    fn empty_world_has_no_bodies() {
        let world = RapierSceneWorld::<u32>::new();
        assert_eq!(world.body_count(), 0);
    }

    #[test]
    fn build_bodies_from_bindings() {
        let mut world = RapierSceneWorld::new();
        let bindings = vec![
            NodeAvatarBinding::circle(0u32, 16.0),
            NodeAvatarBinding::circle(1, 12.0),
        ];
        let snapshots = vec![
            node_snap(0, 100.0, 100.0, 16.0),
            node_snap(1, 200.0, 100.0, 12.0),
        ];
        world.build_bodies(&bindings, &snapshots);
        assert_eq!(world.body_count(), 2);
        assert!(world.contains_node(&0));
        assert!(world.contains_node(&1));
    }

    #[test]
    fn step_without_panic() {
        let mut world = RapierSceneWorld::new();
        let bindings = vec![NodeAvatarBinding::circle(0u32, 16.0)];
        let snapshots = vec![node_snap(0, 100.0, 100.0, 16.0)];
        world.build_bodies(&bindings, &snapshots);
        world.step(Vector2D::zero());
    }

    #[test]
    fn read_positions_after_step() {
        let mut world = RapierSceneWorld::new();
        let bindings = vec![NodeAvatarBinding::circle(0u32, 16.0)];
        let snapshots = vec![node_snap(0, 100.0, 100.0, 16.0)];
        world.build_bodies(&bindings, &snapshots);

        // With no forces and zero gravity, position should remain at initial.
        world.step(Vector2D::zero());
        let deltas = world.read_positions();
        if let Some(d) = deltas.get(&0) {
            assert!(d.length() < 0.1);
        }
    }

    #[test]
    fn gravity_moves_body_down() {
        let mut world = RapierSceneWorld::new();
        let bindings = vec![NodeAvatarBinding {
            node_id: 0u32,
            collider: ColliderSpec::circle(10.0),
            material: PhysicsMaterial {
                gravity_scale: 1.0,
                ..PhysicsMaterial::default()
            },
            body_kind: SceneBodyKind::Dynamic,
        }];
        let snapshots = vec![node_snap(0, 100.0, 100.0, 10.0)];
        world.build_bodies(&bindings, &snapshots);

        for _ in 0..10 {
            world.step(Vector2D::new(0.0, 100.0));
        }

        let pos = world.get_position(&0).unwrap();
        assert!(pos.y > 100.0);
    }

    #[test]
    fn pinned_node_uses_static_body() {
        let mut world = RapierSceneWorld::new();
        let bindings = vec![NodeAvatarBinding::pinned_circle(0u32, 16.0)];
        let snapshots = vec![node_snap(0, 100.0, 100.0, 16.0)];
        world.build_bodies(&bindings, &snapshots);

        for _ in 0..10 {
            world.step(Vector2D::new(0.0, 100.0));
        }

        let pos = world.get_position(&0).unwrap();
        assert!((pos.x - 100.0).abs() < 0.01);
        assert!((pos.y - 100.0).abs() < 0.01);
    }

    #[test]
    fn apply_impulse_moves_body() {
        let mut world = RapierSceneWorld::new();
        let bindings = vec![NodeAvatarBinding::circle(0u32, 10.0)];
        let snapshots = vec![node_snap(0, 100.0, 100.0, 10.0)];
        world.build_bodies(&bindings, &snapshots);

        world.apply_impulse(&0, Vector2D::new(100.0, 0.0));
        for _ in 0..5 {
            world.step(Vector2D::zero());
        }

        let pos = world.get_position(&0).unwrap();
        assert!(pos.x > 100.0);
    }

    #[test]
    fn build_edge_joints() {
        let mut world = RapierSceneWorld::new();
        let bindings = vec![
            NodeAvatarBinding::circle(0u32, 10.0),
            NodeAvatarBinding::circle(1, 10.0),
        ];
        let snapshots = vec![node_snap(0, 0.0, 0.0, 10.0), node_snap(1, 200.0, 0.0, 10.0)];
        world.build_bodies(&bindings, &snapshots);
        world.build_edge_joints(&[(0, 1, EdgeJointSpec::default())]);

        for _ in 0..50 {
            world.step(Vector2D::zero());
        }

        let pos_a = world.get_position(&0).unwrap();
        let pos_b = world.get_position(&1).unwrap();
        let distance = (pos_b - pos_a).length();
        assert!(distance < 200.0);
    }

    #[test]
    fn build_regions_wall() {
        let mut world = RapierSceneWorld::new();
        let bindings = vec![NodeAvatarBinding::circle(0u32, 10.0)];
        let snapshots = vec![node_snap(0, 50.0, 50.0, 10.0)];
        world.build_bodies(&bindings, &snapshots);

        let regions = vec![SceneRegion::circle(
            SceneRegionId(1),
            Point2D::new(50.0, 50.0),
            20.0,
            SceneRegionEffect::Wall,
        )];
        world.build_regions(&regions);

        world.step(Vector2D::zero());
        // Mainly testing no panic.
    }
}
