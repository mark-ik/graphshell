/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::HashMap;

use crate::app::{GraphBrowserApp, GraphViewId, SceneMode};
use crate::graph::NodeKey;
use crate::graph::physics::apply_position_deltas;
use graphshell_core::geometry::{PortablePoint, PortableRect, PortableVector};
use parry2d::math::Pose;
use parry2d::query::intersection_test;
use parry2d::shape::Ball;

#[cfg(test)]
use crate::app::SimulateBehaviorPreset;

const DEFAULT_NODE_PADDING: f32 = 4.0;
const MAX_REGION_DELTA_PER_PASS: f32 = 18.0;
const NODE_SEPARATION_PASSES: usize = 3;
const MIN_SCENE_REGION_RADIUS: f32 = 24.0;
const MIN_SCENE_RECT_SIDE: f32 = 48.0;

// `SimulateMotionProfile` + the preset → profile mapping used to live
// here as a private app-side duplicate of the portable type in
// `graph_canvas::scene_physics`. Removed 2026-04-20 in favor of
// `GraphBrowserApp::resolve_simulate_motion_profile(view_id)`, which
// threads per-view overrides and per-graph defaults through the same
// resolver shape as `NavigationPolicy` and `NodeStyle`.
use graph_canvas::scene_physics::SimulateMotionProfile;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct SceneRegionId(uuid::Uuid);

impl SceneRegionId {
    pub(crate) fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }

    /// Stable lower-64-bit projection of the underlying UUID. Used when
    /// bridging to crates that key scene regions by `u64` (graph-canvas
    /// overlay emission), so the portable id round-trips deterministically
    /// even though it collapses 128 → 64 bits. Collision risk is
    /// negligible at the scale of per-view region sets.
    pub(crate) fn as_u64_low(self) -> u64 {
        self.0.as_u128() as u64
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum SceneRegionDragMode {
    Move,
    Resize(SceneRegionResizeHandle),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SceneRegionResizeHandle {
    CircleRadius,
    RectTopLeft,
    RectTopRight,
    RectBottomLeft,
    RectBottomRight,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SceneRegionDragState {
    pub(crate) view_id: GraphViewId,
    pub(crate) region_id: SceneRegionId,
    pub(crate) mode: SceneRegionDragMode,
    pub(crate) last_pointer_canvas_pos: PortablePoint,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum SceneRegionShape {
    Circle { center: PortablePoint, radius: f32 },
    Rect { rect: PortableRect },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum SceneRegionEffect {
    Attractor { strength: f32 },
    Repulsor { strength: f32 },
    Dampener { factor: f32 },
    Wall,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SceneRegionRuntime {
    pub(crate) id: SceneRegionId,
    pub(crate) label: Option<String>,
    pub(crate) shape: SceneRegionShape,
    pub(crate) effect: SceneRegionEffect,
    pub(crate) visible: bool,
}

impl SceneRegionRuntime {
    pub(crate) fn circle(center: PortablePoint, radius: f32, effect: SceneRegionEffect) -> Self {
        Self {
            id: SceneRegionId::new(),
            label: None,
            shape: SceneRegionShape::Circle { center, radius },
            effect,
            visible: true,
        }
    }

    pub(crate) fn rect(rect: PortableRect, effect: SceneRegionEffect) -> Self {
        Self {
            id: SceneRegionId::new(),
            label: None,
            shape: SceneRegionShape::Rect { rect },
            effect,
            visible: true,
        }
    }

    pub(crate) fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub(crate) fn with_visibility(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SceneCollisionPolicy {
    pub(crate) node_separation_enabled: bool,
    pub(crate) viewport_containment_enabled: bool,
    pub(crate) node_padding: f32,
    pub(crate) region_effect_scale: f32,
    pub(crate) containment_response_scale: f32,
}

impl SceneCollisionPolicy {
    pub(crate) fn enabled(self) -> bool {
        self.node_separation_enabled || self.viewport_containment_enabled
    }
}

impl Default for SceneCollisionPolicy {
    fn default() -> Self {
        Self {
            node_separation_enabled: false,
            viewport_containment_enabled: false,
            node_padding: DEFAULT_NODE_PADDING,
            region_effect_scale: 1.0,
            containment_response_scale: 1.0,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct GraphViewSceneRuntime {
    pub(crate) regions: Vec<SceneRegionRuntime>,
    pub(crate) bounds_override: Option<PortableRect>,
}

pub(crate) fn apply_scene_runtime_pass(
    app: &mut GraphBrowserApp,
    view_id: GraphViewId,
    profile_collision_policy: SceneCollisionPolicy,
) {
    apply_simulate_release_impulses(app, view_id);

    let runtime = app
        .workspace
        .graph_runtime
        .scene_runtimes
        .get(&view_id)
        .cloned()
        .unwrap_or_default();

    if profile_collision_policy.node_separation_enabled {
        apply_node_separation(app, profile_collision_policy.node_padding);
    }

    if profile_collision_policy.viewport_containment_enabled {
        let bounds = runtime.bounds_override.or_else(|| {
            app.workspace
                .graph_runtime
                .graph_view_canvas_rects
                .get(&view_id)
                .copied()
        });
        if let Some(bounds) = bounds {
            apply_viewport_containment(
                app,
                bounds,
                profile_collision_policy.node_padding,
                profile_collision_policy.containment_response_scale,
            );
        }
    }

    if !runtime.regions.is_empty() {
        apply_region_effects(
            app,
            &runtime.regions,
            profile_collision_policy.node_padding,
            profile_collision_policy.region_effect_scale,
            profile_collision_policy.containment_response_scale,
        );
    }
}

fn apply_simulate_release_impulses(app: &mut GraphBrowserApp, view_id: GraphViewId) {
    if app.graph_view_scene_mode(view_id) != SceneMode::Simulate {
        app.workspace
            .graph_runtime
            .simulate_release_impulses
            .remove(&view_id);
        return;
    }

    let motion_profile: SimulateMotionProfile = app.resolve_simulate_motion_profile(view_id);

    let remaining_frames = app.workspace.graph_runtime.drag_release_frames_remaining;
    if remaining_frames == 0 || app.workspace.graph_runtime.is_interacting {
        return;
    }

    let Some(impulses) = app
        .workspace
        .graph_runtime
        .simulate_release_impulses
        .get(&view_id)
        .cloned()
    else {
        return;
    };

    let frame_scale = (remaining_frames as f32 / 10.0).clamp(0.1, 1.0);
    let deltas: HashMap<NodeKey, PortableVector> = impulses
        .iter()
        .filter_map(|(key, impulse)| {
            let delta = *impulse * frame_scale * motion_profile.release_impulse_scale;
            (delta.square_length() > f32::EPSILON).then_some((*key, delta))
        })
        .collect();
    apply_position_deltas(app, deltas);

    let mut next = HashMap::new();
    for (key, impulse) in impulses {
        let decayed = impulse * motion_profile.release_decay;
        if decayed.length() >= motion_profile.min_impulse {
            next.insert(key, decayed);
        }
    }

    if next.is_empty() {
        app.workspace
            .graph_runtime
            .simulate_release_impulses
            .remove(&view_id);
    } else {
        app.workspace
            .graph_runtime
            .simulate_release_impulses
            .insert(view_id, next);
    }
}

fn apply_node_separation(app: &mut GraphBrowserApp, padding: f32) {
    let nodes = movable_node_snapshots(app);
    if nodes.len() < 2 {
        return;
    }

    let mut positions: HashMap<NodeKey, PortablePoint> =
        nodes.iter().map(|node| (node.key, node.position)).collect();
    let radii: HashMap<NodeKey, f32> = nodes.iter().map(|node| (node.key, node.radius)).collect();
    let pinned: HashMap<NodeKey, bool> = nodes.iter().map(|node| (node.key, node.pinned)).collect();

    for _ in 0..NODE_SEPARATION_PASSES {
        let mut changed = false;
        for i in 0..nodes.len() {
            for j in (i + 1)..nodes.len() {
                let key_a = nodes[i].key;
                let key_b = nodes[j].key;
                let pos_a = positions.get(&key_a).copied().unwrap_or(nodes[i].position);
                let pos_b = positions.get(&key_b).copied().unwrap_or(nodes[j].position);
                let radius_a = radii.get(&key_a).copied().unwrap_or(nodes[i].radius) + padding;
                let radius_b = radii.get(&key_b).copied().unwrap_or(nodes[j].radius) + padding;
                let ball_a = Ball::new(radius_a);
                let ball_b = Ball::new(radius_b);
                let iso_a = Pose::translation(pos_a.x, pos_a.y);
                let iso_b = Pose::translation(pos_b.x, pos_b.y);

                let intersects =
                    intersection_test(&iso_a, &ball_a, &iso_b, &ball_b).unwrap_or(false);
                if !intersects {
                    continue;
                }

                let delta = pos_b - pos_a;
                let distance = delta.length();
                let min_distance = radius_a + radius_b;
                let normal = if distance > f32::EPSILON {
                    delta / distance
                } else {
                    PortableVector::new(1.0, 0.0)
                };
                let overlap = (min_distance - distance).max(0.0) + 0.5;
                if overlap <= f32::EPSILON {
                    continue;
                }

                let a_pinned = pinned.get(&key_a).copied().unwrap_or(false);
                let b_pinned = pinned.get(&key_b).copied().unwrap_or(false);
                if a_pinned && b_pinned {
                    continue;
                }

                if a_pinned {
                    let next = pos_b + normal * overlap;
                    positions.insert(key_b, next);
                } else if b_pinned {
                    let next = pos_a - normal * overlap;
                    positions.insert(key_a, next);
                } else {
                    let push = normal * (overlap * 0.5);
                    positions.insert(key_a, pos_a - push);
                    positions.insert(key_b, pos_b + push);
                }
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    let deltas: HashMap<NodeKey, PortableVector> = nodes
        .iter()
        .filter_map(|node| {
            let next = positions.get(&node.key).copied()?;
            let delta = next - node.position;
            (delta.square_length() > f32::EPSILON).then_some((node.key, delta))
        })
        .collect();
    apply_position_deltas(app, deltas);
}

fn apply_viewport_containment(
    app: &mut GraphBrowserApp,
    bounds: PortableRect,
    padding: f32,
    response_scale: f32,
) {
    let mut deltas = HashMap::new();
    for node in movable_node_snapshots(app) {
        if node.pinned {
            continue;
        }
        let inset = node.radius + padding;
        if rect_width(bounds) <= inset * 2.0 || rect_height(bounds) <= inset * 2.0 {
            continue;
        }

        let clamped = PortablePoint::new(
            node.position
                .x
                .clamp(rect_left(bounds) + inset, rect_right(bounds) - inset),
            node.position
                .y
                .clamp(rect_top(bounds) + inset, rect_bottom(bounds) - inset),
        );
        let delta = (clamped - node.position) * response_scale.max(0.0);
        if delta.square_length() > f32::EPSILON {
            deltas.insert(node.key, delta);
        }
    }
    apply_position_deltas(app, deltas);
}

fn apply_region_effects(
    app: &mut GraphBrowserApp,
    regions: &[SceneRegionRuntime],
    padding: f32,
    effect_scale: f32,
    containment_response_scale: f32,
) {
    let nodes = movable_node_snapshots(app);
    if nodes.is_empty() {
        return;
    }

    let mut deltas: HashMap<NodeKey, PortableVector> = HashMap::new();
    for node in nodes {
        if node.pinned {
            continue;
        }
        let mut delta = zero_vector();
        for region in regions {
            if !region.visible {
                continue;
            }
            delta += region_delta_for_node(
                region,
                node.position,
                node.radius + padding,
                containment_response_scale,
            ) * effect_scale;
        }
        if delta.length() > MAX_REGION_DELTA_PER_PASS {
            delta = delta.normalize() * MAX_REGION_DELTA_PER_PASS;
        }
        if delta.square_length() > f32::EPSILON {
            deltas.insert(node.key, delta);
        }
    }
    apply_position_deltas(app, deltas);
}

fn region_delta_for_node(
    region: &SceneRegionRuntime,
    position: PortablePoint,
    padded_radius: f32,
    containment_response_scale: f32,
) -> PortableVector {
    match region.effect {
        SceneRegionEffect::Attractor { strength } => {
            if !shape_contains(region.shape, position) {
                return zero_vector();
            }
            let center = shape_center(region.shape);
            (center - position) * strength
        }
        SceneRegionEffect::Repulsor { strength } => {
            if !shape_contains(region.shape, position) {
                return zero_vector();
            }
            let center = shape_center(region.shape);
            let away = position - center;
            if away.square_length() <= f32::EPSILON {
                PortableVector::new(strength.max(1.0), 0.0)
            } else {
                away.normalize() * strength
            }
        }
        SceneRegionEffect::Dampener { factor } => {
            if !shape_contains(region.shape, position) {
                return zero_vector();
            }
            let center = shape_center(region.shape);
            (center - position) * -(factor.abs() * 0.1)
        }
        SceneRegionEffect::Wall => {
            wall_pushout_delta(region.shape, position, padded_radius) * containment_response_scale
        }
    }
}

fn wall_pushout_delta(
    shape: SceneRegionShape,
    position: PortablePoint,
    padded_radius: f32,
) -> PortableVector {
    match shape {
        SceneRegionShape::Circle { center, radius } => {
            let delta = position - center;
            let distance = delta.length();
            let min_distance = radius + padded_radius;
            if distance >= min_distance {
                return zero_vector();
            }
            let normal = if distance > f32::EPSILON {
                delta / distance
            } else {
                PortableVector::new(1.0, 0.0)
            };
            normal * (min_distance - distance)
        }
        SceneRegionShape::Rect { rect } => {
            if !expanded_rect_contains(rect, padded_radius, position) {
                return zero_vector();
            }
            let left = position.x - rect_left(rect);
            let right = rect_right(rect) - position.x;
            let top = position.y - rect_top(rect);
            let bottom = rect_bottom(rect) - position.y;
            let min_side = left.min(right).min(top).min(bottom);
            if (min_side - left).abs() <= f32::EPSILON {
                PortableVector::new(-(left + padded_radius), 0.0)
            } else if (min_side - right).abs() <= f32::EPSILON {
                PortableVector::new(right + padded_radius, 0.0)
            } else if (min_side - top).abs() <= f32::EPSILON {
                PortableVector::new(0.0, -(top + padded_radius))
            } else {
                PortableVector::new(0.0, bottom + padded_radius)
            }
        }
    }
}

fn shape_center(shape: SceneRegionShape) -> PortablePoint {
    match shape {
        SceneRegionShape::Circle { center, .. } => center,
        SceneRegionShape::Rect { rect } => rect_center(rect),
    }
}

pub(crate) fn shape_contains(shape: SceneRegionShape, position: PortablePoint) -> bool {
    match shape {
        SceneRegionShape::Circle { center, radius } => (position - center).length() <= radius,
        SceneRegionShape::Rect { rect } => rect.contains(position),
    }
}

pub(crate) fn translate_region(region: &mut SceneRegionRuntime, delta: PortableVector) {
    region.shape = translate_shape(region.shape, delta);
}

pub(crate) fn resize_region_to_pointer(
    region: &mut SceneRegionRuntime,
    handle: SceneRegionResizeHandle,
    pointer_canvas_pos: PortablePoint,
) {
    region.shape = resize_shape_to_pointer(region.shape, handle, pointer_canvas_pos);
}

pub(crate) fn translate_shape(shape: SceneRegionShape, delta: PortableVector) -> SceneRegionShape {
    match shape {
        SceneRegionShape::Circle { center, radius } => SceneRegionShape::Circle {
            center: center + delta,
            radius,
        },
        SceneRegionShape::Rect { rect } => SceneRegionShape::Rect {
            rect: rect.translate(delta),
        },
    }
}

pub(crate) fn resize_shape_to_pointer(
    shape: SceneRegionShape,
    handle: SceneRegionResizeHandle,
    pointer_canvas_pos: PortablePoint,
) -> SceneRegionShape {
    match (shape, handle) {
        (SceneRegionShape::Circle { center, .. }, SceneRegionResizeHandle::CircleRadius) => {
            SceneRegionShape::Circle {
                center,
                radius: (pointer_canvas_pos - center)
                    .length()
                    .max(MIN_SCENE_REGION_RADIUS),
            }
        }
        (SceneRegionShape::Rect { rect }, handle) => {
            let (fixed, sign_x, sign_y) = match handle {
                SceneRegionResizeHandle::RectTopLeft => (rect_right_bottom(rect), -1.0, -1.0),
                SceneRegionResizeHandle::RectTopRight => (rect_left_bottom(rect), 1.0, -1.0),
                SceneRegionResizeHandle::RectBottomLeft => (rect_right_top(rect), -1.0, 1.0),
                SceneRegionResizeHandle::RectBottomRight => (rect_left_top(rect), 1.0, 1.0),
                SceneRegionResizeHandle::CircleRadius => return SceneRegionShape::Rect { rect },
            };
            let mut moving = pointer_canvas_pos;
            if (moving.x - fixed.x).abs() < MIN_SCENE_RECT_SIDE {
                moving.x = fixed.x + sign_x * MIN_SCENE_RECT_SIDE;
            }
            if (moving.y - fixed.y).abs() < MIN_SCENE_RECT_SIDE {
                moving.y = fixed.y + sign_y * MIN_SCENE_RECT_SIDE;
            }
            SceneRegionShape::Rect {
                rect: rect_from_two_points(fixed, moving),
            }
        }
        (shape, _) => shape,
    }
}

fn zero_vector() -> PortableVector {
    PortableVector::new(0.0, 0.0)
}

fn rect_left(rect: PortableRect) -> f32 {
    rect.origin.x
}

fn rect_top(rect: PortableRect) -> f32 {
    rect.origin.y
}

fn rect_width(rect: PortableRect) -> f32 {
    rect.size.width
}

fn rect_height(rect: PortableRect) -> f32 {
    rect.size.height
}

fn rect_right(rect: PortableRect) -> f32 {
    rect.origin.x + rect.size.width
}

fn rect_bottom(rect: PortableRect) -> f32 {
    rect.origin.y + rect.size.height
}

fn rect_left_top(rect: PortableRect) -> PortablePoint {
    PortablePoint::new(rect_left(rect), rect_top(rect))
}

fn rect_right_top(rect: PortableRect) -> PortablePoint {
    PortablePoint::new(rect_right(rect), rect_top(rect))
}

fn rect_left_bottom(rect: PortableRect) -> PortablePoint {
    PortablePoint::new(rect_left(rect), rect_bottom(rect))
}

fn rect_right_bottom(rect: PortableRect) -> PortablePoint {
    PortablePoint::new(rect_right(rect), rect_bottom(rect))
}

fn rect_center(rect: PortableRect) -> PortablePoint {
    PortablePoint::new(
        rect_left(rect) + rect_width(rect) * 0.5,
        rect_top(rect) + rect_height(rect) * 0.5,
    )
}

fn rect_from_min_max(min_x: f32, min_y: f32, max_x: f32, max_y: f32) -> PortableRect {
    PortableRect::new(
        PortablePoint::new(min_x, min_y),
        graphshell_core::geometry::PortableSize::new(
            (max_x - min_x).max(0.0),
            (max_y - min_y).max(0.0),
        ),
    )
}

fn rect_from_two_points(a: PortablePoint, b: PortablePoint) -> PortableRect {
    rect_from_min_max(a.x.min(b.x), a.y.min(b.y), a.x.max(b.x), a.y.max(b.y))
}

fn expanded_rect_contains(rect: PortableRect, amount: f32, point: PortablePoint) -> bool {
    point.x >= rect_left(rect) - amount
        && point.x <= rect_right(rect) + amount
        && point.y >= rect_top(rect) - amount
        && point.y <= rect_bottom(rect) + amount
}

#[derive(Debug, Clone, Copy)]
struct NodeSnapshot {
    key: NodeKey,
    position: PortablePoint,
    radius: f32,
    pinned: bool,
}

fn movable_node_snapshots(app: &GraphBrowserApp) -> Vec<NodeSnapshot> {
    app.domain_graph()
        .nodes()
        .filter_map(|(key, node)| {
            let position = app.domain_graph().node_projected_position(key)?;
            Some(NodeSnapshot {
                key,
                position,
                radius: resolve_node_radius(app, key).max(1.0),
                pinned: node.is_pinned,
            })
        })
        .collect()
}

fn resolve_node_radius(app: &GraphBrowserApp, key: NodeKey) -> f32 {
    app.domain_graph()
        .get_node(key)
        .map(|node| match node.lifecycle {
            crate::graph::NodeLifecycle::Active => 18.0,
            crate::graph::NodeLifecycle::Warm => 16.5,
            crate::graph::NodeLifecycle::Cold => 15.0,
            crate::graph::NodeLifecycle::Tombstone => 14.0,
        })
        .unwrap_or(16.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registries::atomic::lens::PhysicsProfile;
    use euclid::default::Point2D;

    fn test_rect(min_x: f32, min_y: f32, max_x: f32, max_y: f32) -> PortableRect {
        rect_from_min_max(min_x, min_y, max_x, max_y)
    }

    #[test]
    fn node_separation_moves_overlapping_nodes_apart() {
        let mut app = GraphBrowserApp::new_for_testing();
        let profile = PhysicsProfile::settle();
        let a = app.add_node_and_sync("https://a.example".into(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("https://b.example".into(), Point2D::new(4.0, 0.0));

        let before_a = app.domain_graph().node_projected_position(a).unwrap();
        let before_b = app.domain_graph().node_projected_position(b).unwrap();

        apply_scene_runtime_pass(
            &mut app,
            GraphViewId::new(),
            profile.scene_collision_policy(),
        );

        let after_a = app.domain_graph().node_projected_position(a).unwrap();
        let after_b = app.domain_graph().node_projected_position(b).unwrap();
        assert!((after_b.x - after_a.x) > (before_b.x - before_a.x));
    }

    #[test]
    fn pinned_nodes_do_not_move_during_node_separation() {
        let mut app = GraphBrowserApp::new_for_testing();
        let profile = PhysicsProfile::settle();
        let pinned = app.add_node_and_sync("https://pinned.example".into(), Point2D::new(0.0, 0.0));
        let other = app.add_node_and_sync("https://other.example".into(), Point2D::new(4.0, 0.0));
        app.domain_graph_mut()
            .get_node_mut(pinned)
            .unwrap()
            .is_pinned = true;

        let before_pinned = app.domain_graph().node_projected_position(pinned).unwrap();
        let before_other = app.domain_graph().node_projected_position(other).unwrap();

        apply_scene_runtime_pass(
            &mut app,
            GraphViewId::new(),
            profile.scene_collision_policy(),
        );

        let after_pinned = app.domain_graph().node_projected_position(pinned).unwrap();
        let after_other = app.domain_graph().node_projected_position(other).unwrap();
        assert_eq!(after_pinned, before_pinned);
        assert!(after_other.x > before_other.x);
    }

    #[test]
    fn viewport_containment_clamps_nodes_inside_canvas_bounds() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_id = GraphViewId::new();
        let node =
            app.add_node_and_sync("https://outside.example".into(), Point2D::new(200.0, 200.0));
        app.workspace
            .graph_runtime
            .graph_view_canvas_rects
            .insert(view_id, test_rect(0.0, 0.0, 100.0, 100.0));

        apply_scene_runtime_pass(
            &mut app,
            view_id,
            SceneCollisionPolicy {
                node_separation_enabled: false,
                viewport_containment_enabled: true,
                node_padding: DEFAULT_NODE_PADDING,
                region_effect_scale: 1.0,
                containment_response_scale: 1.0,
            },
        );

        let after = app.domain_graph().node_projected_position(node).unwrap();
        assert!(after.x <= 100.0);
        assert!(after.y <= 100.0);
    }

    #[test]
    fn lower_containment_response_leaves_more_drift_after_one_pass() {
        let bounds = test_rect(0.0, 0.0, 100.0, 100.0);

        let mut loose_app = GraphBrowserApp::new_for_testing();
        let loose_view = GraphViewId::new();
        let loose_node =
            loose_app.add_node_and_sync("https://loose.example".into(), Point2D::new(200.0, 200.0));
        loose_app
            .workspace
            .graph_runtime
            .graph_view_canvas_rects
            .insert(loose_view, bounds);
        apply_scene_runtime_pass(
            &mut loose_app,
            loose_view,
            SceneCollisionPolicy {
                node_separation_enabled: false,
                viewport_containment_enabled: true,
                node_padding: DEFAULT_NODE_PADDING,
                region_effect_scale: 1.0,
                containment_response_scale: 0.45,
            },
        );
        let loose_after = loose_app
            .domain_graph()
            .node_projected_position(loose_node)
            .unwrap();

        let mut firm_app = GraphBrowserApp::new_for_testing();
        let firm_view = GraphViewId::new();
        let firm_node =
            firm_app.add_node_and_sync("https://firm.example".into(), Point2D::new(200.0, 200.0));
        firm_app
            .workspace
            .graph_runtime
            .graph_view_canvas_rects
            .insert(firm_view, bounds);
        apply_scene_runtime_pass(
            &mut firm_app,
            firm_view,
            SceneCollisionPolicy {
                node_separation_enabled: false,
                viewport_containment_enabled: true,
                node_padding: DEFAULT_NODE_PADDING,
                region_effect_scale: 1.0,
                containment_response_scale: 1.0,
            },
        );
        let firm_after = firm_app
            .domain_graph()
            .node_projected_position(firm_node)
            .unwrap();

        assert!(loose_after.x > firm_after.x);
        assert!(loose_after.y > firm_after.y);
    }

    #[test]
    fn region_attractor_pulls_node_toward_region_center() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_id = GraphViewId::new();
        let node = app.add_node_and_sync("https://inside.example".into(), Point2D::new(20.0, 20.0));
        app.workspace.graph_runtime.scene_runtimes.insert(
            view_id,
            GraphViewSceneRuntime {
                regions: vec![SceneRegionRuntime::rect(
                    test_rect(0.0, 0.0, 100.0, 100.0),
                    SceneRegionEffect::Attractor { strength: 0.2 },
                )],
                bounds_override: None,
            },
        );

        apply_scene_runtime_pass(&mut app, view_id, SceneCollisionPolicy::default());

        let after = app.domain_graph().node_projected_position(node).unwrap();
        assert!(after.x > 20.0);
        assert!(after.y > 20.0);
    }

    #[test]
    fn region_repulsor_pushes_node_away_from_region_center() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_id = GraphViewId::new();
        let node = app.add_node_and_sync("https://inside.example".into(), Point2D::new(30.0, 30.0));
        app.workspace.graph_runtime.scene_runtimes.insert(
            view_id,
            GraphViewSceneRuntime {
                regions: vec![SceneRegionRuntime::rect(
                    test_rect(0.0, 0.0, 100.0, 100.0),
                    SceneRegionEffect::Repulsor { strength: 6.0 },
                )],
                bounds_override: None,
            },
        );

        apply_scene_runtime_pass(&mut app, view_id, SceneCollisionPolicy::default());

        let after = app.domain_graph().node_projected_position(node).unwrap();
        assert!(after.x < 30.0);
        assert!(after.y < 30.0);
    }

    #[test]
    fn region_effect_scale_amplifies_scene_region_motion() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_id = GraphViewId::new();
        let node = app.add_node_and_sync("https://inside.example".into(), Point2D::new(20.0, 20.0));
        app.workspace.graph_runtime.scene_runtimes.insert(
            view_id,
            GraphViewSceneRuntime {
                regions: vec![SceneRegionRuntime::rect(
                    test_rect(0.0, 0.0, 100.0, 100.0),
                    SceneRegionEffect::Attractor { strength: 0.2 },
                )],
                bounds_override: None,
            },
        );

        apply_scene_runtime_pass(
            &mut app,
            view_id,
            SceneCollisionPolicy {
                region_effect_scale: 2.0,
                ..SceneCollisionPolicy::default()
            },
        );

        let after = app.domain_graph().node_projected_position(node).unwrap();
        assert!(after.x > 26.0);
        assert!(after.y > 26.0);
    }

    #[test]
    fn wall_regions_respect_containment_response_scale() {
        let wall =
            SceneRegionRuntime::rect(test_rect(0.0, 0.0, 100.0, 100.0), SceneRegionEffect::Wall);

        let mut loose_app = GraphBrowserApp::new_for_testing();
        let loose_view = GraphViewId::new();
        let loose_node = loose_app.add_node_and_sync(
            "https://wall-loose.example".into(),
            Point2D::new(50.0, 50.0),
        );
        loose_app.workspace.graph_runtime.scene_runtimes.insert(
            loose_view,
            GraphViewSceneRuntime {
                regions: vec![wall.clone()],
                bounds_override: None,
            },
        );
        apply_scene_runtime_pass(
            &mut loose_app,
            loose_view,
            SceneCollisionPolicy {
                containment_response_scale: 0.05,
                ..SceneCollisionPolicy::default()
            },
        );
        let loose_after = loose_app
            .domain_graph()
            .node_projected_position(loose_node)
            .unwrap();

        let mut firm_app = GraphBrowserApp::new_for_testing();
        let firm_view = GraphViewId::new();
        let firm_node = firm_app
            .add_node_and_sync("https://wall-firm.example".into(), Point2D::new(50.0, 50.0));
        firm_app.workspace.graph_runtime.scene_runtimes.insert(
            firm_view,
            GraphViewSceneRuntime {
                regions: vec![wall],
                bounds_override: None,
            },
        );
        apply_scene_runtime_pass(
            &mut firm_app,
            firm_view,
            SceneCollisionPolicy {
                containment_response_scale: 0.2,
                ..SceneCollisionPolicy::default()
            },
        );
        let firm_after = firm_app
            .domain_graph()
            .node_projected_position(firm_node)
            .unwrap();

        let loose_displacement = (loose_after - Point2D::new(50.0, 50.0)).length();
        let firm_displacement = (firm_after - Point2D::new(50.0, 50.0)).length();
        assert!(firm_displacement > loose_displacement);
    }

    #[test]
    fn simulate_release_impulses_coast_then_decay() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_id = GraphViewId::new();
        let node = app.add_node_and_sync("https://coast.example".into(), Point2D::new(20.0, 20.0));
        app.set_graph_view_scene_mode(view_id, SceneMode::Simulate);
        app.workspace.graph_runtime.drag_release_frames_remaining = 5;
        app.workspace
            .graph_runtime
            .simulate_release_impulses
            .insert(
                view_id,
                HashMap::from([(node, PortableVector::new(10.0, 0.0))]),
            );

        apply_scene_runtime_pass(&mut app, view_id, SceneCollisionPolicy::default());

        let after = app.domain_graph().node_projected_position(node).unwrap();
        assert!(after.x > 24.0);
        let stored = app
            .workspace
            .graph_runtime
            .simulate_release_impulses
            .get(&view_id)
            .and_then(|impulses| impulses.get(&node))
            .copied()
            .expect("impulse should decay and remain stored");
        assert!(stored.x < 10.0);
        assert!(stored.x > 0.0);
    }

    #[test]
    fn float_preset_coasts_farther_than_packed() {
        let mut float_app = GraphBrowserApp::new_for_testing();
        let float_view = GraphViewId::new();
        let float_node =
            float_app.add_node_and_sync("https://float.example".into(), Point2D::new(20.0, 20.0));
        float_app.set_graph_view_scene_mode(float_view, SceneMode::Simulate);
        float_app
            .set_graph_view_simulate_behavior_preset(float_view, SimulateBehaviorPreset::Float);
        float_app
            .workspace
            .graph_runtime
            .drag_release_frames_remaining = 5;
        float_app
            .workspace
            .graph_runtime
            .simulate_release_impulses
            .insert(
                float_view,
                HashMap::from([(float_node, PortableVector::new(10.0, 0.0))]),
            );
        apply_scene_runtime_pass(&mut float_app, float_view, SceneCollisionPolicy::default());
        let float_after = float_app
            .domain_graph()
            .node_projected_position(float_node)
            .unwrap();

        let mut packed_app = GraphBrowserApp::new_for_testing();
        let packed_view = GraphViewId::new();
        let packed_node =
            packed_app.add_node_and_sync("https://packed.example".into(), Point2D::new(20.0, 20.0));
        packed_app.set_graph_view_scene_mode(packed_view, SceneMode::Simulate);
        packed_app
            .set_graph_view_simulate_behavior_preset(packed_view, SimulateBehaviorPreset::Packed);
        packed_app
            .workspace
            .graph_runtime
            .drag_release_frames_remaining = 5;
        packed_app
            .workspace
            .graph_runtime
            .simulate_release_impulses
            .insert(
                packed_view,
                HashMap::from([(packed_node, PortableVector::new(10.0, 0.0))]),
            );
        apply_scene_runtime_pass(
            &mut packed_app,
            packed_view,
            SceneCollisionPolicy::default(),
        );
        let packed_after = packed_app
            .domain_graph()
            .node_projected_position(packed_node)
            .unwrap();

        assert!(float_after.x > packed_after.x);
    }

    #[test]
    fn simulate_release_impulses_clear_outside_simulate_mode() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_id = GraphViewId::new();
        let node = app.add_node_and_sync("https://quiet.example".into(), Point2D::new(20.0, 20.0));
        app.workspace.graph_runtime.drag_release_frames_remaining = 5;
        app.workspace
            .graph_runtime
            .simulate_release_impulses
            .insert(
                view_id,
                HashMap::from([(node, PortableVector::new(10.0, 0.0))]),
            );

        apply_scene_runtime_pass(&mut app, view_id, SceneCollisionPolicy::default());

        let after = app.domain_graph().node_projected_position(node).unwrap();
        assert_eq!(after, Point2D::new(20.0, 20.0));
        assert!(
            !app.workspace
                .graph_runtime
                .simulate_release_impulses
                .contains_key(&view_id)
        );
    }

    #[test]
    fn scene_runtime_is_isolated_per_view() {
        let mut app = GraphBrowserApp::new_for_testing();
        let source_view = GraphViewId::new();
        let untouched_view = GraphViewId::new();
        let node = app.add_node_and_sync("https://shared.example".into(), Point2D::new(20.0, 20.0));
        app.workspace.graph_runtime.scene_runtimes.insert(
            source_view,
            GraphViewSceneRuntime {
                regions: vec![SceneRegionRuntime::rect(
                    test_rect(0.0, 0.0, 100.0, 100.0),
                    SceneRegionEffect::Attractor { strength: 0.2 },
                )],
                bounds_override: None,
            },
        );

        apply_scene_runtime_pass(&mut app, untouched_view, SceneCollisionPolicy::default());

        let after = app.domain_graph().node_projected_position(node).unwrap();
        assert_eq!(after, Point2D::new(20.0, 20.0));
    }

    #[test]
    fn translate_shape_moves_circle_center_without_changing_radius() {
        let translated = translate_shape(
            SceneRegionShape::Circle {
                center: PortablePoint::new(10.0, 20.0),
                radius: 40.0,
            },
            PortableVector::new(5.0, -3.0),
        );
        assert_eq!(
            translated,
            SceneRegionShape::Circle {
                center: PortablePoint::new(15.0, 17.0),
                radius: 40.0,
            }
        );
    }

    #[test]
    fn translate_shape_moves_rect_bounds() {
        let translated = translate_shape(
            SceneRegionShape::Rect {
                rect: test_rect(0.0, 0.0, 10.0, 20.0),
            },
            PortableVector::new(3.0, 4.0),
        );
        assert_eq!(
            translated,
            SceneRegionShape::Rect {
                rect: test_rect(3.0, 4.0, 13.0, 24.0),
            }
        );
    }

    #[test]
    fn resize_shape_updates_circle_radius_from_pointer_distance() {
        let resized = resize_shape_to_pointer(
            SceneRegionShape::Circle {
                center: PortablePoint::new(10.0, 10.0),
                radius: 30.0,
            },
            SceneRegionResizeHandle::CircleRadius,
            PortablePoint::new(70.0, 10.0),
        );
        assert_eq!(
            resized,
            SceneRegionShape::Circle {
                center: PortablePoint::new(10.0, 10.0),
                radius: 60.0,
            }
        );
    }

    #[test]
    fn resize_shape_updates_rect_from_dragged_corner() {
        let resized = resize_shape_to_pointer(
            SceneRegionShape::Rect {
                rect: test_rect(10.0, 20.0, 100.0, 120.0),
            },
            SceneRegionResizeHandle::RectTopLeft,
            PortablePoint::new(0.0, 10.0),
        );
        assert_eq!(
            resized,
            SceneRegionShape::Rect {
                rect: test_rect(0.0, 10.0, 100.0, 120.0),
            }
        );
    }
}
