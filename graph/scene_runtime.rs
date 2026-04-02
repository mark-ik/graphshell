/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::HashMap;

use crate::app::{GraphBrowserApp, GraphViewId};
use crate::graph::NodeKey;
use crate::graph::physics::apply_position_deltas;
use crate::util::CoordBridge;
use parry2d::math::Pose;
use parry2d::query::intersection_test;
use parry2d::shape::Ball;

const DEFAULT_NODE_PADDING: f32 = 4.0;
const MAX_REGION_DELTA_PER_PASS: f32 = 18.0;
const NODE_SEPARATION_PASSES: usize = 3;
const MIN_SCENE_REGION_RADIUS: f32 = 24.0;
const MIN_SCENE_RECT_SIDE: f32 = 48.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct SceneRegionId(uuid::Uuid);

impl SceneRegionId {
    pub(crate) fn new() -> Self {
        Self(uuid::Uuid::new_v4())
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
    pub(crate) last_pointer_canvas_pos: egui::Pos2,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum SceneRegionShape {
    Circle { center: egui::Pos2, radius: f32 },
    Rect { rect: egui::Rect },
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
    pub(crate) fn circle(
        center: egui::Pos2,
        radius: f32,
        effect: SceneRegionEffect,
    ) -> Self {
        Self {
            id: SceneRegionId::new(),
            label: None,
            shape: SceneRegionShape::Circle { center, radius },
            effect,
            visible: true,
        }
    }

    pub(crate) fn rect(rect: egui::Rect, effect: SceneRegionEffect) -> Self {
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
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct GraphViewSceneRuntime {
    pub(crate) regions: Vec<SceneRegionRuntime>,
    pub(crate) bounds_override: Option<egui::Rect>,
}

pub(crate) fn apply_scene_runtime_pass(
    app: &mut GraphBrowserApp,
    view_id: GraphViewId,
    profile_collision_policy: SceneCollisionPolicy,
) {
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
            apply_viewport_containment(app, bounds, profile_collision_policy.node_padding);
        }
    }

    if !runtime.regions.is_empty() {
        apply_region_effects(app, &runtime.regions, profile_collision_policy.node_padding);
    }
}

fn apply_node_separation(app: &mut GraphBrowserApp, padding: f32) {
    let nodes = movable_node_snapshots(app);
    if nodes.len() < 2 {
        return;
    }

    let mut positions: HashMap<NodeKey, egui::Pos2> =
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
                    egui::vec2(1.0, 0.0)
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

    let deltas: HashMap<NodeKey, egui::Vec2> = nodes
        .iter()
        .filter_map(|node| {
            let next = positions.get(&node.key).copied()?;
            let delta = next - node.position;
            (delta.length_sq() > f32::EPSILON).then_some((node.key, delta))
        })
        .collect();
    apply_position_deltas(app, deltas);
}

fn apply_viewport_containment(app: &mut GraphBrowserApp, bounds: egui::Rect, padding: f32) {
    let mut deltas = HashMap::new();
    for node in movable_node_snapshots(app) {
        if node.pinned {
            continue;
        }
        let inset = node.radius + padding;
        if bounds.width() <= inset * 2.0 || bounds.height() <= inset * 2.0 {
            continue;
        }

        let clamped = egui::Pos2::new(
            node.position.x.clamp(bounds.left() + inset, bounds.right() - inset),
            node.position.y.clamp(bounds.top() + inset, bounds.bottom() - inset),
        );
        let delta = clamped - node.position;
        if delta.length_sq() > f32::EPSILON {
            deltas.insert(node.key, delta);
        }
    }
    apply_position_deltas(app, deltas);
}

fn apply_region_effects(
    app: &mut GraphBrowserApp,
    regions: &[SceneRegionRuntime],
    padding: f32,
) {
    let nodes = movable_node_snapshots(app);
    if nodes.is_empty() {
        return;
    }

    let mut deltas: HashMap<NodeKey, egui::Vec2> = HashMap::new();
    for node in nodes {
        if node.pinned {
            continue;
        }
        let mut delta = egui::Vec2::ZERO;
        for region in regions {
            if !region.visible {
                continue;
            }
            delta += region_delta_for_node(region, node.position, node.radius + padding);
        }
        if delta.length() > MAX_REGION_DELTA_PER_PASS {
            delta = delta.normalized() * MAX_REGION_DELTA_PER_PASS;
        }
        if delta.length_sq() > f32::EPSILON {
            deltas.insert(node.key, delta);
        }
    }
    apply_position_deltas(app, deltas);
}

fn region_delta_for_node(
    region: &SceneRegionRuntime,
    position: egui::Pos2,
    padded_radius: f32,
) -> egui::Vec2 {
    match region.effect {
        SceneRegionEffect::Attractor { strength } => {
            if !shape_contains(region.shape, position) {
                return egui::Vec2::ZERO;
            }
            let center = shape_center(region.shape);
            (center - position) * strength
        }
        SceneRegionEffect::Repulsor { strength } => {
            if !shape_contains(region.shape, position) {
                return egui::Vec2::ZERO;
            }
            let center = shape_center(region.shape);
            let away = position - center;
            if away.length_sq() <= f32::EPSILON {
                egui::vec2(strength.max(1.0), 0.0)
            } else {
                away.normalized() * strength
            }
        }
        SceneRegionEffect::Dampener { factor } => {
            if !shape_contains(region.shape, position) {
                return egui::Vec2::ZERO;
            }
            let center = shape_center(region.shape);
            (center - position) * -(factor.abs() * 0.1)
        }
        SceneRegionEffect::Wall => wall_pushout_delta(region.shape, position, padded_radius),
    }
}

fn wall_pushout_delta(
    shape: SceneRegionShape,
    position: egui::Pos2,
    padded_radius: f32,
) -> egui::Vec2 {
    match shape {
        SceneRegionShape::Circle { center, radius } => {
            let delta = position - center;
            let distance = delta.length();
            let min_distance = radius + padded_radius;
            if distance >= min_distance {
                return egui::Vec2::ZERO;
            }
            let normal = if distance > f32::EPSILON {
                delta / distance
            } else {
                egui::vec2(1.0, 0.0)
            };
            normal * (min_distance - distance)
        }
        SceneRegionShape::Rect { rect } => {
            if !rect.expand(padded_radius).contains(position) {
                return egui::Vec2::ZERO;
            }
            let left = position.x - rect.left();
            let right = rect.right() - position.x;
            let top = position.y - rect.top();
            let bottom = rect.bottom() - position.y;
            let min_side = left.min(right).min(top).min(bottom);
            if (min_side - left).abs() <= f32::EPSILON {
                egui::vec2(-(left + padded_radius), 0.0)
            } else if (min_side - right).abs() <= f32::EPSILON {
                egui::vec2(right + padded_radius, 0.0)
            } else if (min_side - top).abs() <= f32::EPSILON {
                egui::vec2(0.0, -(top + padded_radius))
            } else {
                egui::vec2(0.0, bottom + padded_radius)
            }
        }
    }
}

fn shape_center(shape: SceneRegionShape) -> egui::Pos2 {
    match shape {
        SceneRegionShape::Circle { center, .. } => center,
        SceneRegionShape::Rect { rect } => rect.center(),
    }
}

pub(crate) fn shape_contains(shape: SceneRegionShape, position: egui::Pos2) -> bool {
    match shape {
        SceneRegionShape::Circle { center, radius } => (position - center).length() <= radius,
        SceneRegionShape::Rect { rect } => rect.contains(position),
    }
}

pub(crate) fn translate_region(region: &mut SceneRegionRuntime, delta: egui::Vec2) {
    region.shape = translate_shape(region.shape, delta);
}

pub(crate) fn resize_region_to_pointer(
    region: &mut SceneRegionRuntime,
    handle: SceneRegionResizeHandle,
    pointer_canvas_pos: egui::Pos2,
) {
    region.shape = resize_shape_to_pointer(region.shape, handle, pointer_canvas_pos);
}

pub(crate) fn translate_shape(shape: SceneRegionShape, delta: egui::Vec2) -> SceneRegionShape {
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
    pointer_canvas_pos: egui::Pos2,
) -> SceneRegionShape {
    match (shape, handle) {
        (SceneRegionShape::Circle { center, .. }, SceneRegionResizeHandle::CircleRadius) => {
            SceneRegionShape::Circle {
                center,
                radius: (pointer_canvas_pos - center).length().max(MIN_SCENE_REGION_RADIUS),
            }
        }
        (SceneRegionShape::Rect { rect }, handle) => {
            let (fixed, sign_x, sign_y) = match handle {
                SceneRegionResizeHandle::RectTopLeft => (rect.right_bottom(), -1.0, -1.0),
                SceneRegionResizeHandle::RectTopRight => (rect.left_bottom(), 1.0, -1.0),
                SceneRegionResizeHandle::RectBottomLeft => (rect.right_top(), -1.0, 1.0),
                SceneRegionResizeHandle::RectBottomRight => (rect.left_top(), 1.0, 1.0),
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
                rect: egui::Rect::from_two_pos(fixed, moving),
            }
        }
        (shape, _) => shape,
    }
}

#[derive(Debug, Clone, Copy)]
struct NodeSnapshot {
    key: NodeKey,
    position: egui::Pos2,
    radius: f32,
    pinned: bool,
}

fn movable_node_snapshots(app: &GraphBrowserApp) -> Vec<NodeSnapshot> {
    app.domain_graph()
        .nodes()
        .filter_map(|(key, node)| {
            let position = app.domain_graph().node_projected_position(key)?.to_pos2();
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
    if let Some(state) = app.workspace.graph_runtime.egui_state.as_ref()
        && let Some(node) = state.graph.node(key)
    {
        return node.display().radius();
    }

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

    #[test]
    fn node_separation_moves_overlapping_nodes_apart() {
        let mut app = GraphBrowserApp::new_for_testing();
        let profile = PhysicsProfile::solid();
        let a = app.add_node_and_sync("https://a.example".into(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("https://b.example".into(), Point2D::new(4.0, 0.0));

        let before_a = app.domain_graph().node_projected_position(a).unwrap();
        let before_b = app.domain_graph().node_projected_position(b).unwrap();

        apply_scene_runtime_pass(&mut app, GraphViewId::new(), profile.scene_collision_policy());

        let after_a = app.domain_graph().node_projected_position(a).unwrap();
        let after_b = app.domain_graph().node_projected_position(b).unwrap();
        assert!((after_b.x - after_a.x) > (before_b.x - before_a.x));
    }

    #[test]
    fn pinned_nodes_do_not_move_during_node_separation() {
        let mut app = GraphBrowserApp::new_for_testing();
        let profile = PhysicsProfile::solid();
        let pinned = app.add_node_and_sync("https://pinned.example".into(), Point2D::new(0.0, 0.0));
        let other = app.add_node_and_sync("https://other.example".into(), Point2D::new(4.0, 0.0));
        app.domain_graph_mut().get_node_mut(pinned).unwrap().is_pinned = true;

        let before_pinned = app.domain_graph().node_projected_position(pinned).unwrap();
        let before_other = app.domain_graph().node_projected_position(other).unwrap();

        apply_scene_runtime_pass(&mut app, GraphViewId::new(), profile.scene_collision_policy());

        let after_pinned = app.domain_graph().node_projected_position(pinned).unwrap();
        let after_other = app.domain_graph().node_projected_position(other).unwrap();
        assert_eq!(after_pinned, before_pinned);
        assert!(after_other.x > before_other.x);
    }

    #[test]
    fn viewport_containment_clamps_nodes_inside_canvas_bounds() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_id = GraphViewId::new();
        let node = app.add_node_and_sync("https://outside.example".into(), Point2D::new(200.0, 200.0));
        app.workspace.graph_runtime.graph_view_canvas_rects.insert(
            view_id,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(100.0, 100.0)),
        );

        apply_scene_runtime_pass(
            &mut app,
            view_id,
            SceneCollisionPolicy {
                node_separation_enabled: false,
                viewport_containment_enabled: true,
                node_padding: DEFAULT_NODE_PADDING,
            },
        );

        let after = app.domain_graph().node_projected_position(node).unwrap();
        assert!(after.x <= 100.0);
        assert!(after.y <= 100.0);
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
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(100.0, 100.0)),
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
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(100.0, 100.0)),
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
    fn scene_runtime_is_isolated_per_view() {
        let mut app = GraphBrowserApp::new_for_testing();
        let source_view = GraphViewId::new();
        let untouched_view = GraphViewId::new();
        let node = app.add_node_and_sync("https://shared.example".into(), Point2D::new(20.0, 20.0));
        app.workspace.graph_runtime.scene_runtimes.insert(
            source_view,
            GraphViewSceneRuntime {
                regions: vec![SceneRegionRuntime::rect(
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(100.0, 100.0)),
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
                center: egui::pos2(10.0, 20.0),
                radius: 40.0,
            },
            egui::vec2(5.0, -3.0),
        );
        assert_eq!(
            translated,
            SceneRegionShape::Circle {
                center: egui::pos2(15.0, 17.0),
                radius: 40.0,
            }
        );
    }

    #[test]
    fn translate_shape_moves_rect_bounds() {
        let translated = translate_shape(
            SceneRegionShape::Rect {
                rect: egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(10.0, 20.0)),
            },
            egui::vec2(3.0, 4.0),
        );
        assert_eq!(
            translated,
            SceneRegionShape::Rect {
                rect: egui::Rect::from_min_max(egui::pos2(3.0, 4.0), egui::pos2(13.0, 24.0)),
            }
        );
    }

    #[test]
    fn resize_shape_updates_circle_radius_from_pointer_distance() {
        let resized = resize_shape_to_pointer(
            SceneRegionShape::Circle {
                center: egui::pos2(10.0, 10.0),
                radius: 30.0,
            },
            SceneRegionResizeHandle::CircleRadius,
            egui::pos2(70.0, 10.0),
        );
        assert_eq!(
            resized,
            SceneRegionShape::Circle {
                center: egui::pos2(10.0, 10.0),
                radius: 60.0,
            }
        );
    }

    #[test]
    fn resize_shape_updates_rect_from_dragged_corner() {
        let resized = resize_shape_to_pointer(
            SceneRegionShape::Rect {
                rect: egui::Rect::from_min_max(egui::pos2(10.0, 20.0), egui::pos2(100.0, 120.0)),
            },
            SceneRegionResizeHandle::RectTopLeft,
            egui::pos2(0.0, 10.0),
        );
        assert_eq!(
            resized,
            SceneRegionShape::Rect {
                rect: egui::Rect::from_min_max(egui::pos2(0.0, 10.0), egui::pos2(100.0, 120.0)),
            }
        );
    }
}
