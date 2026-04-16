/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Scene derivation: the pure transformation from `CanvasSceneInput` into
//! a `ProjectedScene`.
//!
//! This is the central pipeline of `graph-canvas`. It consumes host-provided
//! graph/scene inputs and produces a complete draw list and hit-proxy set for
//! one frame. The derivation is:
//!
//! 1. **deterministic** — same inputs always produce the same output
//! 2. **framework-agnostic** — no egui, iced, or winit dependency
//! 3. **projection-aware** — TwoD, TwoPointFive, and Isometric all flow
//!    through the same pipeline
//!
//! The host is responsible for converting its graph model into
//! `CanvasSceneInput` before calling `derive_scene`.

use euclid::default::{Point2D, Rect, Size2D};
use std::hash::Hash;

use crate::camera::{CanvasCamera, CanvasViewport};
use crate::lod::{LodLevel, LodPolicy};
use crate::packet::{Color, HitProxy, ProjectedScene, SceneDrawItem, Stroke};
use crate::projection::{ProjectionConfig, ProjectionMode, project_position};
use crate::scene::{CanvasEdge, CanvasNode, CanvasSceneInput, CanvasSceneObject};
use crate::scripting::SceneObjectHitShape;

/// Configuration for the derivation pipeline.
#[derive(Debug, Clone)]
pub struct DeriveConfig {
    /// LOD policy for node/edge detail levels.
    pub lod_policy: LodPolicy,
    /// Default node fill color when the host doesn't specify one.
    pub default_node_color: Color,
    /// Default edge stroke color.
    pub default_edge_color: Color,
    /// Default edge stroke width in world units.
    pub default_edge_width: f32,
    /// Whether to emit hit proxies (can be disabled for thumbnail/offscreen).
    pub emit_hit_proxies: bool,
    /// Projection tuning parameters.
    pub projection: ProjectionConfig,
}

impl Default for DeriveConfig {
    fn default() -> Self {
        Self {
            lod_policy: LodPolicy::default(),
            default_node_color: Color::new(0.4, 0.6, 0.9, 1.0),
            default_edge_color: Color::new(0.5, 0.5, 0.5, 0.6),
            default_edge_width: 1.5,
            emit_hit_proxies: true,
            projection: ProjectionConfig::default(),
        }
    }
}

/// Optional per-node visual overrides provided by the host.
///
/// The host computes colors, selection rings, etc. from its own theme/state
/// and passes them alongside the `CanvasSceneInput`. This keeps theme logic
/// in the host while letting the derivation pipeline apply the visuals.
#[derive(Debug, Clone)]
pub struct NodeVisualOverride {
    pub fill: Option<Color>,
    pub stroke: Option<Stroke>,
    pub label_color: Option<Color>,
}

impl Default for NodeVisualOverride {
    fn default() -> Self {
        Self {
            fill: None,
            stroke: None,
            label_color: None,
        }
    }
}

/// Derive a `ProjectedScene` from a `CanvasSceneInput`.
///
/// This is the primary entry point for the derivation pipeline.
///
/// - `input`: the scene input for one graph view
/// - `camera`: current camera state (pan, zoom)
/// - `viewport`: the pane rectangle and scale factor
/// - `z_values`: per-node z values derived by the host from `ZSource`. Nodes
///   not present in this map get z=0. The host is responsible for z derivation
///   because it requires graph metadata (BFS depth, recency, UDC class) that
///   graph-canvas doesn't own.
/// - `node_overrides`: per-node visual overrides (colors, strokes). Indexed
///   by position in `input.nodes` for simplicity.
/// - `config`: derivation configuration
pub fn derive_scene<N: Clone + Eq + Hash>(
    input: &CanvasSceneInput<N>,
    camera: &CanvasCamera,
    viewport: &CanvasViewport,
    z_values: &dyn Fn(&N) -> f32,
    node_overrides: &dyn Fn(usize, &N) -> NodeVisualOverride,
    config: &DeriveConfig,
) -> ProjectedScene<N> {
    let projection = input.projection.degrade_if_needed();

    // Compute the world-space viewport rect for culling.
    let world_viewport = world_viewport_rect(camera, viewport);

    let mut world_items = Vec::new();
    let mut hit_proxies = Vec::new();

    // ── Edges (drawn first, behind nodes) ────────────────────────────────
    derive_edges(
        &input.edges,
        &input.nodes,
        camera,
        viewport,
        &world_viewport,
        &projection,
        z_values,
        config,
        &mut world_items,
    );

    // ── Nodes ────────────────────────────────────────────────────────────
    derive_nodes(
        &input.nodes,
        camera,
        viewport,
        &world_viewport,
        &projection,
        z_values,
        node_overrides,
        config,
        &mut world_items,
        &mut hit_proxies,
    );

    // ── Scene objects (scripted) ─────────────────────────────────────────
    let mut overlay_items = Vec::new();
    derive_scene_objects(
        &input.scene_objects,
        camera,
        viewport,
        &world_viewport,
        config,
        &mut world_items,
        &mut overlay_items,
        &mut hit_proxies,
    );

    ProjectedScene {
        background: Vec::new(),
        world: world_items,
        overlays: overlay_items,
        hit_proxies,
    }
}

// ── Internal helpers ─────────────────────────────────────────────────────────

/// Compute the world-space rectangle visible through the current camera.
fn world_viewport_rect(camera: &CanvasCamera, viewport: &CanvasViewport) -> Rect<f32> {
    let tl = camera.screen_to_world(
        Point2D::new(viewport.rect.min_x(), viewport.rect.min_y()),
        viewport,
    );
    let br = camera.screen_to_world(
        Point2D::new(viewport.rect.max_x(), viewport.rect.max_y()),
        viewport,
    );
    Rect::new(
        Point2D::new(tl.x.min(br.x), tl.y.min(br.y)),
        Size2D::new((br.x - tl.x).abs(), (br.y - tl.y).abs()),
    )
}

/// Determine whether a world-space circle is potentially visible in the
/// viewport (conservative — may include slightly off-screen nodes).
fn is_node_visible(world_pos: Point2D<f32>, radius: f32, world_viewport: &Rect<f32>) -> bool {
    let expanded = world_viewport.inflate(radius, radius);
    expanded.contains(world_pos)
}

fn derive_nodes<N: Clone + Eq + Hash>(
    nodes: &[CanvasNode<N>],
    camera: &CanvasCamera,
    viewport: &CanvasViewport,
    world_viewport: &Rect<f32>,
    projection: &ProjectionMode,
    z_values: &dyn Fn(&N) -> f32,
    node_overrides: &dyn Fn(usize, &N) -> NodeVisualOverride,
    config: &DeriveConfig,
    world_items: &mut Vec<SceneDrawItem>,
    hit_proxies: &mut Vec<HitProxy<N>>,
) {
    for (i, node) in nodes.iter().enumerate() {
        // Viewport culling in world space (before projection).
        if !is_node_visible(node.position, node.radius, world_viewport) {
            continue;
        }

        let z = z_values(&node.id);
        let projected = project_position(node.position, z, projection, &config.projection);

        // LOD check after projection (uses screen-space radius).
        let screen_radius = node.radius * projected.depth_scale;
        let lod = config.lod_policy.level_for_node(camera.zoom, screen_radius);
        if !lod.is_visible() {
            continue;
        }

        let screen_pos = camera.world_to_screen(projected.to_point(), viewport);
        let overrides = node_overrides(i, &node.id);

        let fill = overrides.fill.unwrap_or(config.default_node_color);
        let stroke = overrides.stroke;

        // Draw the node circle.
        let draw_radius = screen_radius * camera.zoom;
        world_items.push(SceneDrawItem::Circle {
            center: screen_pos,
            radius: draw_radius,
            fill,
            stroke,
        });

        // Draw the label at Full LOD only.
        if lod == LodLevel::Full {
            if let Some(ref label) = node.label {
                let label_color = overrides
                    .label_color
                    .unwrap_or(Color::new(0.9, 0.9, 0.9, 1.0));
                let font_size = (12.0 * projected.depth_scale * camera.zoom).max(6.0);
                world_items.push(SceneDrawItem::Label {
                    position: Point2D::new(screen_pos.x, screen_pos.y + draw_radius + 4.0),
                    text: label.clone(),
                    font_size,
                    color: label_color,
                });
            }
        }

        // Emit hit proxy.
        if config.emit_hit_proxies {
            hit_proxies.push(HitProxy::Node {
                id: node.id.clone(),
                center: screen_pos,
                radius: draw_radius,
            });
        }
    }
}

fn derive_scene_objects<N: Clone + Eq + Hash>(
    scene_objects: &[CanvasSceneObject],
    camera: &CanvasCamera,
    viewport: &CanvasViewport,
    world_viewport: &Rect<f32>,
    config: &DeriveConfig,
    world_items: &mut Vec<SceneDrawItem>,
    overlay_items: &mut Vec<SceneDrawItem>,
    hit_proxies: &mut Vec<HitProxy<N>>,
) {
    for obj in scene_objects {
        // Viewport culling: check if the object position is within the
        // visible world rect (with some margin for the hit shape).
        let margin = match &obj.hit_shape {
            Some(SceneObjectHitShape::Circle { radius }) => *radius,
            Some(SceneObjectHitShape::Rect { half_extents }) => half_extents.x.max(half_extents.y),
            None => 0.0,
        };
        if !is_node_visible(obj.position, margin, world_viewport) {
            continue;
        }

        let screen_pos = camera.world_to_screen(obj.position, viewport);

        // Offset draw items from object-local to screen space.
        for item in &obj.draw_items {
            world_items.push(offset_draw_item(item, screen_pos));
        }

        // Offset overlay items from object-local to screen space.
        for item in &obj.overlay_items {
            overlay_items.push(offset_draw_item(item, screen_pos));
        }

        // Emit hit proxy if the object has a hit shape.
        if config.emit_hit_proxies {
            if let Some(ref hit_shape) = obj.hit_shape {
                let radius = match hit_shape {
                    SceneObjectHitShape::Circle { radius } => *radius * camera.zoom,
                    SceneObjectHitShape::Rect { half_extents } => {
                        // Use the larger half-extent as the hit radius.
                        half_extents.x.max(half_extents.y) * camera.zoom
                    }
                };
                hit_proxies.push(HitProxy::SceneObject {
                    id: obj.id,
                    center: screen_pos,
                    radius,
                });
            }
        }
    }
}

/// Offset a draw item's position by a screen-space translation.
fn offset_draw_item(item: &SceneDrawItem, offset: Point2D<f32>) -> SceneDrawItem {
    match item {
        SceneDrawItem::Circle {
            center,
            radius,
            fill,
            stroke,
        } => SceneDrawItem::Circle {
            center: Point2D::new(center.x + offset.x, center.y + offset.y),
            radius: *radius,
            fill: *fill,
            stroke: *stroke,
        },
        SceneDrawItem::Line { from, to, stroke } => SceneDrawItem::Line {
            from: Point2D::new(from.x + offset.x, from.y + offset.y),
            to: Point2D::new(to.x + offset.x, to.y + offset.y),
            stroke: *stroke,
        },
        SceneDrawItem::RoundedRect {
            rect,
            corner_radius,
            fill,
            stroke,
        } => SceneDrawItem::RoundedRect {
            rect: Rect::new(
                Point2D::new(rect.origin.x + offset.x, rect.origin.y + offset.y),
                rect.size,
            ),
            corner_radius: *corner_radius,
            fill: *fill,
            stroke: *stroke,
        },
        SceneDrawItem::Label {
            position,
            text,
            font_size,
            color,
        } => SceneDrawItem::Label {
            position: Point2D::new(position.x + offset.x, position.y + offset.y),
            text: text.clone(),
            font_size: *font_size,
            color: *color,
        },
        SceneDrawItem::ImageRef { rect, handle } => SceneDrawItem::ImageRef {
            rect: Rect::new(
                Point2D::new(rect.origin.x + offset.x, rect.origin.y + offset.y),
                rect.size,
            ),
            handle: handle.clone(),
        },
    }
}

fn derive_edges<N: Clone + Eq + Hash>(
    edges: &[CanvasEdge<N>],
    nodes: &[CanvasNode<N>],
    camera: &CanvasCamera,
    viewport: &CanvasViewport,
    world_viewport: &Rect<f32>,
    projection: &ProjectionMode,
    z_values: &dyn Fn(&N) -> f32,
    config: &DeriveConfig,
    world_items: &mut Vec<SceneDrawItem>,
) {
    // Build a position lookup from node id to index for edge endpoint resolution.
    // This avoids O(n*m) lookups when there are many edges.
    let node_index: std::collections::HashMap<&N, usize> =
        nodes.iter().enumerate().map(|(i, n)| (&n.id, i)).collect();

    for edge in edges {
        let (Some(&src_idx), Some(&tgt_idx)) =
            (node_index.get(&edge.source), node_index.get(&edge.target))
        else {
            continue; // Skip edges with missing endpoints.
        };

        let src_node = &nodes[src_idx];
        let tgt_node = &nodes[tgt_idx];

        // Skip edge if both endpoints are outside the viewport.
        let src_visible = is_node_visible(src_node.position, src_node.radius, world_viewport);
        let tgt_visible = is_node_visible(tgt_node.position, tgt_node.radius, world_viewport);
        if !src_visible && !tgt_visible {
            continue;
        }

        let src_z = z_values(&edge.source);
        let tgt_z = z_values(&edge.target);
        let src_proj = project_position(src_node.position, src_z, projection, &config.projection);
        let tgt_proj = project_position(tgt_node.position, tgt_z, projection, &config.projection);

        let src_screen = camera.world_to_screen(src_proj.to_point(), viewport);
        let tgt_screen = camera.world_to_screen(tgt_proj.to_point(), viewport);

        let width = config.default_edge_width * camera.zoom * edge.weight.max(0.1);
        world_items.push(SceneDrawItem::Line {
            from: src_screen,
            to: tgt_screen,
            stroke: Stroke {
                color: config.default_edge_color,
                width,
            },
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::projection::ZSource;
    use crate::scene::{SceneMode, ViewId};
    use crate::scripting::{SceneObjectHitShape, SceneObjectId};

    fn simple_input() -> CanvasSceneInput<u32> {
        CanvasSceneInput {
            view_id: ViewId(1),
            nodes: vec![
                CanvasNode {
                    id: 0,
                    position: Point2D::new(0.0, 0.0),
                    radius: 16.0,
                    label: Some("origin".into()),
                },
                CanvasNode {
                    id: 1,
                    position: Point2D::new(100.0, 0.0),
                    radius: 16.0,
                    label: Some("right".into()),
                },
                CanvasNode {
                    id: 2,
                    position: Point2D::new(0.0, 100.0),
                    radius: 16.0,
                    label: None,
                },
            ],
            edges: vec![
                CanvasEdge {
                    source: 0,
                    target: 1,
                    weight: 1.0,
                },
                CanvasEdge {
                    source: 0,
                    target: 2,
                    weight: 0.5,
                },
            ],
            scene_objects: vec![],
            overlays: vec![],
            scene_mode: SceneMode::Browse,
            projection: ProjectionMode::TwoD,
        }
    }

    fn default_camera() -> CanvasCamera {
        CanvasCamera::default()
    }

    fn default_viewport() -> CanvasViewport {
        CanvasViewport::default()
    }

    fn zero_z(_id: &u32) -> f32 {
        0.0
    }

    fn no_overrides(_idx: usize, _id: &u32) -> NodeVisualOverride {
        NodeVisualOverride::default()
    }

    #[test]
    fn derive_scene_produces_items_for_visible_nodes() {
        let input = simple_input();
        let scene = derive_scene(
            &input,
            &default_camera(),
            &default_viewport(),
            &zero_z,
            &no_overrides,
            &DeriveConfig::default(),
        );
        // 3 nodes visible -> at least 3 circles + labels for nodes with labels.
        let circles: Vec<_> = scene
            .world
            .iter()
            .filter(|item| matches!(item, SceneDrawItem::Circle { .. }))
            .collect();
        assert_eq!(circles.len(), 3);
    }

    #[test]
    fn derive_scene_produces_edges() {
        let input = simple_input();
        let scene = derive_scene(
            &input,
            &default_camera(),
            &default_viewport(),
            &zero_z,
            &no_overrides,
            &DeriveConfig::default(),
        );
        let lines: Vec<_> = scene
            .world
            .iter()
            .filter(|item| matches!(item, SceneDrawItem::Line { .. }))
            .collect();
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn derive_scene_produces_hit_proxies() {
        let input = simple_input();
        let scene = derive_scene(
            &input,
            &default_camera(),
            &default_viewport(),
            &zero_z,
            &no_overrides,
            &DeriveConfig::default(),
        );
        assert_eq!(scene.hit_proxies.len(), 3);
    }

    #[test]
    fn derive_scene_no_hit_proxies_when_disabled() {
        let input = simple_input();
        let config = DeriveConfig {
            emit_hit_proxies: false,
            ..DeriveConfig::default()
        };
        let scene = derive_scene(
            &input,
            &default_camera(),
            &default_viewport(),
            &zero_z,
            &no_overrides,
            &config,
        );
        assert!(scene.hit_proxies.is_empty());
    }

    #[test]
    fn derive_scene_empty_input_produces_empty_scene() {
        let input = CanvasSceneInput::<u32>::default();
        let scene = derive_scene(
            &input,
            &default_camera(),
            &default_viewport(),
            &zero_z,
            &no_overrides,
            &DeriveConfig::default(),
        );
        assert!(scene.world.is_empty());
        assert!(scene.hit_proxies.is_empty());
    }

    #[test]
    fn derive_scene_deterministic() {
        let input = simple_input();
        let camera = default_camera();
        let viewport = default_viewport();
        let config = DeriveConfig::default();

        let a = derive_scene(&input, &camera, &viewport, &zero_z, &no_overrides, &config);
        let b = derive_scene(&input, &camera, &viewport, &zero_z, &no_overrides, &config);
        assert_eq!(a, b);
    }

    #[test]
    fn derive_scene_twopointfive_shifts_deep_nodes() {
        let mut input = simple_input();
        input.projection = ProjectionMode::TwoPointFive {
            z_source: ZSource::Recency { max_depth: 100.0 },
        };
        // Give node 1 a z value of 50.
        let z_fn = |id: &u32| if *id == 1 { 50.0 } else { 0.0 };

        let scene = derive_scene(
            &input,
            &default_camera(),
            &default_viewport(),
            &z_fn,
            &no_overrides,
            &DeriveConfig::default(),
        );

        // Find the circle for node 0 (z=0) and node 1 (z=50).
        let circles: Vec<_> = scene
            .world
            .iter()
            .filter_map(|item| match item {
                SceneDrawItem::Circle { center, radius, .. } => Some((center, radius)),
                _ => None,
            })
            .collect();
        assert!(circles.len() >= 2);

        // Node 1 (z=50) should have a smaller rendered radius than node 0 (z=0)
        // because depth_scale < 1.0 for deeper nodes.
        // We identify them by their hit proxies.
        let proxy_0 = scene
            .hit_proxies
            .iter()
            .find(|p| matches!(p, HitProxy::Node { id: 0, .. }))
            .unwrap();
        let proxy_1 = scene
            .hit_proxies
            .iter()
            .find(|p| matches!(p, HitProxy::Node { id: 1, .. }))
            .unwrap();
        let r0 = match proxy_0 {
            HitProxy::Node { radius, .. } => *radius,
            _ => unreachable!(),
        };
        let r1 = match proxy_1 {
            HitProxy::Node { radius, .. } => *radius,
            _ => unreachable!(),
        };
        assert!(
            r1 < r0,
            "deeper node (z=50) should have smaller radius: r0={}, r1={}",
            r0,
            r1
        );
    }

    #[test]
    fn derive_scene_applies_node_overrides() {
        let input = simple_input();
        let red = Color::new(1.0, 0.0, 0.0, 1.0);
        let override_fn = |idx: usize, _id: &u32| {
            if idx == 0 {
                NodeVisualOverride {
                    fill: Some(red),
                    ..Default::default()
                }
            } else {
                NodeVisualOverride::default()
            }
        };

        let scene = derive_scene(
            &input,
            &default_camera(),
            &default_viewport(),
            &zero_z,
            &override_fn,
            &DeriveConfig::default(),
        );

        // First circle should be red (node 0 gets the override).
        let first_circle = scene
            .world
            .iter()
            .find(|item| matches!(item, SceneDrawItem::Circle { .. }));
        match first_circle {
            Some(SceneDrawItem::Circle { fill, .. }) => {
                assert_eq!(*fill, red);
            }
            _ => panic!("expected a circle"),
        }
    }

    #[test]
    fn derive_scene_culls_offscreen_nodes() {
        let mut input = simple_input();
        // Place node 2 far outside the default viewport.
        input.nodes[2].position = Point2D::new(50000.0, 50000.0);

        let scene = derive_scene(
            &input,
            &default_camera(),
            &default_viewport(),
            &zero_z,
            &no_overrides,
            &DeriveConfig::default(),
        );

        // Only 2 nodes should be visible.
        let circles: Vec<_> = scene
            .world
            .iter()
            .filter(|item| matches!(item, SceneDrawItem::Circle { .. }))
            .collect();
        assert_eq!(circles.len(), 2);
        assert_eq!(scene.hit_proxies.len(), 2);
    }

    #[test]
    fn derive_scene_labels_only_at_full_lod() {
        let mut input = simple_input();
        // All nodes have labels.
        for node in &mut input.nodes {
            node.label = Some("test".into());
        }

        // At very low zoom, LOD should be Reduced or Minimal — no labels.
        let camera = CanvasCamera::new(euclid::default::Vector2D::zero(), 0.05);
        let scene = derive_scene(
            &input,
            &camera,
            &default_viewport(),
            &zero_z,
            &no_overrides,
            &DeriveConfig::default(),
        );

        let labels: Vec<_> = scene
            .world
            .iter()
            .filter(|item| matches!(item, SceneDrawItem::Label { .. }))
            .collect();
        assert_eq!(labels.len(), 0, "no labels at very low zoom");
    }

    #[test]
    fn derive_scene_edges_skip_missing_endpoints() {
        let mut input = simple_input();
        // Add an edge referencing a non-existent node.
        input.edges.push(CanvasEdge {
            source: 0,
            target: 99,
            weight: 1.0,
        });

        let scene = derive_scene(
            &input,
            &default_camera(),
            &default_viewport(),
            &zero_z,
            &no_overrides,
            &DeriveConfig::default(),
        );

        // Only the 2 valid edges should produce lines.
        let lines: Vec<_> = scene
            .world
            .iter()
            .filter(|item| matches!(item, SceneDrawItem::Line { .. }))
            .collect();
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn derive_scene_with_scene_objects() {
        let mut input = simple_input();
        input.scene_objects.push(CanvasSceneObject {
            id: SceneObjectId(10),
            position: Point2D::new(50.0, 50.0),
            draw_items: vec![SceneDrawItem::Circle {
                center: Point2D::new(0.0, 0.0),
                radius: 8.0,
                fill: Color::new(1.0, 0.0, 0.0, 1.0),
                stroke: None,
            }],
            hit_shape: Some(SceneObjectHitShape::Circle { radius: 10.0 }),
            overlay_items: vec![SceneDrawItem::Label {
                position: Point2D::new(0.0, -15.0),
                text: "badge".into(),
                font_size: 10.0,
                color: Color::WHITE,
            }],
        });

        let scene = derive_scene(
            &input,
            &default_camera(),
            &default_viewport(),
            &zero_z,
            &no_overrides,
            &DeriveConfig::default(),
        );

        // Should have the scene object's circle in world items (plus 3 node circles + 2 edges).
        let circles: Vec<_> = scene
            .world
            .iter()
            .filter(|item| matches!(item, SceneDrawItem::Circle { .. }))
            .collect();
        assert_eq!(circles.len(), 4); // 3 nodes + 1 scene object

        // Should have the overlay label.
        assert!(!scene.overlays.is_empty());
        assert!(
            scene
                .overlays
                .iter()
                .any(|item| matches!(item, SceneDrawItem::Label { text, .. } if text == "badge"))
        );

        // Should have a SceneObject hit proxy.
        let obj_proxies: Vec<_> = scene
            .hit_proxies
            .iter()
            .filter(|p| matches!(p, HitProxy::SceneObject { .. }))
            .collect();
        assert_eq!(obj_proxies.len(), 1);
    }

    #[test]
    fn derive_scene_culls_offscreen_scene_objects() {
        let mut input = simple_input();
        input.scene_objects.push(CanvasSceneObject {
            id: SceneObjectId(20),
            position: Point2D::new(50000.0, 50000.0),
            draw_items: vec![SceneDrawItem::Circle {
                center: Point2D::new(0.0, 0.0),
                radius: 5.0,
                fill: Color::WHITE,
                stroke: None,
            }],
            hit_shape: Some(SceneObjectHitShape::Circle { radius: 5.0 }),
            overlay_items: vec![],
        });

        let scene = derive_scene(
            &input,
            &default_camera(),
            &default_viewport(),
            &zero_z,
            &no_overrides,
            &DeriveConfig::default(),
        );

        // The offscreen scene object should not produce a hit proxy.
        let obj_proxies: Vec<_> = scene
            .hit_proxies
            .iter()
            .filter(|p| matches!(p, HitProxy::SceneObject { .. }))
            .collect();
        assert!(obj_proxies.is_empty());
    }

    #[test]
    fn derive_scene_object_without_hit_shape_has_no_proxy() {
        let mut input = simple_input();
        input.scene_objects.push(CanvasSceneObject {
            id: SceneObjectId(30),
            position: Point2D::new(50.0, 50.0),
            draw_items: vec![SceneDrawItem::Circle {
                center: Point2D::new(0.0, 0.0),
                radius: 5.0,
                fill: Color::WHITE,
                stroke: None,
            }],
            hit_shape: None, // Not interactive
            overlay_items: vec![],
        });

        let scene = derive_scene(
            &input,
            &default_camera(),
            &default_viewport(),
            &zero_z,
            &no_overrides,
            &DeriveConfig::default(),
        );

        // Should still have the draw item but no scene object hit proxy.
        let obj_proxies: Vec<_> = scene
            .hit_proxies
            .iter()
            .filter(|p| matches!(p, HitProxy::SceneObject { .. }))
            .collect();
        assert!(obj_proxies.is_empty());

        // But the circle should be in the world items.
        let circles: Vec<_> = scene
            .world
            .iter()
            .filter(|item| matches!(item, SceneDrawItem::Circle { .. }))
            .collect();
        assert_eq!(circles.len(), 4); // 3 nodes + 1 scene object
    }
}
