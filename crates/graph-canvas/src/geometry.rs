/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Parry2D geometry queries for scene interaction.
//!
//! Provides point queries, shape intersection, and nearest-body lookups for
//! projected hit testing. These work in Browse and Arrange modes without
//! requiring a Rapier simulation world.
//!
//! Available behind the `physics` feature flag.

use euclid::default::Point2D;
use parry2d::math::{Pose, Vector};
use parry2d::query;
use parry2d::shape::SharedShape;

use crate::scene_composition::ColliderSpec;

// ── Coordinate conversion ──────────────────────────────────────────────────

/// Convert a euclid Point2D to a parry2d Vector (Vec2).
fn to_parry_vec(p: Point2D<f32>) -> Vector {
    Vector::new(p.x, p.y)
}

// ── Collider conversion ────────────────────────────────────────────────────

/// Convert a `ColliderSpec` to a parry2d `SharedShape`.
///
/// Returns `None` for specs that can't be represented (e.g. empty convex hull).
pub fn collider_to_shape(spec: &ColliderSpec) -> Option<SharedShape> {
    match spec {
        ColliderSpec::Circle { radius } => Some(SharedShape::ball(*radius)),
        ColliderSpec::Rect { half_extents } => {
            Some(SharedShape::cuboid(half_extents.x, half_extents.y))
        }
        ColliderSpec::Capsule {
            half_height,
            radius,
        } => Some(SharedShape::capsule_y(*half_height, *radius)),
        ColliderSpec::ConvexHull { points } => {
            let parry_points: Vec<Vector> = points.iter().map(|p| to_parry_vec(*p)).collect();
            SharedShape::convex_hull(&parry_points)
        }
        ColliderSpec::Compound(specs) => {
            let shapes: Vec<(Pose, SharedShape)> = specs
                .iter()
                .filter_map(|s| collider_to_shape(s).map(|shape| (Pose::IDENTITY, shape)))
                .collect();
            if shapes.is_empty() {
                return None;
            }
            Some(SharedShape::compound(shapes))
        }
    }
}

// ── Point query ────────────────────────────────────────────────────────────

/// Result of a point query against a set of scene bodies.
#[derive(Debug, Clone)]
pub struct PointQueryHit<N> {
    /// The body that was hit.
    pub id: N,
    /// Distance from the query point to the body surface. 0.0 if inside.
    pub distance: f32,
}

/// A positioned shape for geometry queries.
#[derive(Debug, Clone)]
pub struct PositionedBody<N> {
    pub id: N,
    pub position: Point2D<f32>,
    pub shape: SharedShape,
}

/// Find all bodies whose shapes contain the given point.
///
/// Returns hits sorted by distance (closest first).
pub fn point_query<N: Clone>(
    bodies: &[PositionedBody<N>],
    point: Point2D<f32>,
) -> Vec<PointQueryHit<N>> {
    let query_point = to_parry_vec(point);
    let mut hits: Vec<PointQueryHit<N>> = bodies
        .iter()
        .filter_map(|body| {
            let pose = Pose::from_translation(Vector::new(body.position.x, body.position.y));
            let proj = body.shape.project_point(&pose, query_point, true);
            let distance = if proj.is_inside {
                0.0
            } else {
                (proj.point - query_point).length()
            };
            // Only report hits where the point is inside.
            (proj.is_inside).then_some(PointQueryHit {
                id: body.id.clone(),
                distance,
            })
        })
        .collect();
    hits.sort_by(|a, b| {
        a.distance
            .partial_cmp(&b.distance)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    hits
}

/// Find the closest body to the given point, with a maximum search distance.
///
/// This is useful for "fuzzy" hit testing where the pointer is near but not
/// exactly on a body.
pub fn nearest_body<N: Clone>(
    bodies: &[PositionedBody<N>],
    point: Point2D<f32>,
    max_distance: f32,
) -> Option<PointQueryHit<N>> {
    let query_point = to_parry_vec(point);
    let mut best: Option<PointQueryHit<N>> = None;
    for body in bodies {
        let pose = Pose::from_translation(Vector::new(body.position.x, body.position.y));
        let proj = body.shape.project_point(&pose, query_point, true);
        let distance = if proj.is_inside {
            0.0
        } else {
            (proj.point - query_point).length()
        };
        if distance > max_distance {
            continue;
        }
        if best.as_ref().is_none_or(|b| distance < b.distance) {
            best = Some(PointQueryHit {
                id: body.id.clone(),
                distance,
            });
        }
    }
    best
}

// ── Intersection test ──────────────────────────────────────────────────────

/// Test whether two positioned shapes overlap.
pub fn shapes_overlap(
    pos_a: Point2D<f32>,
    shape_a: &SharedShape,
    pos_b: Point2D<f32>,
    shape_b: &SharedShape,
) -> bool {
    let pose_a = Pose::from_translation(Vector::new(pos_a.x, pos_a.y));
    let pose_b = Pose::from_translation(Vector::new(pos_b.x, pos_b.y));
    query::intersection_test(&pose_a, shape_a.as_ref(), &pose_b, shape_b.as_ref())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collider_to_shape_circle() {
        let spec = ColliderSpec::circle(10.0);
        let shape = collider_to_shape(&spec);
        assert!(shape.is_some());
    }

    #[test]
    fn collider_to_shape_rect() {
        let spec = ColliderSpec::rect(20.0, 10.0);
        let shape = collider_to_shape(&spec);
        assert!(shape.is_some());
    }

    #[test]
    fn collider_to_shape_capsule() {
        let spec = ColliderSpec::Capsule {
            half_height: 15.0,
            radius: 5.0,
        };
        let shape = collider_to_shape(&spec);
        assert!(shape.is_some());
    }

    #[test]
    fn collider_to_shape_convex_hull() {
        let spec = ColliderSpec::ConvexHull {
            points: vec![
                Point2D::new(0.0, 0.0),
                Point2D::new(10.0, 0.0),
                Point2D::new(5.0, 10.0),
            ],
        };
        let shape = collider_to_shape(&spec);
        assert!(shape.is_some());
    }

    #[test]
    fn collider_to_shape_compound() {
        let spec = ColliderSpec::Compound(vec![
            ColliderSpec::circle(5.0),
            ColliderSpec::rect(10.0, 8.0),
        ]);
        let shape = collider_to_shape(&spec);
        assert!(shape.is_some());
    }

    #[test]
    fn point_query_hit_inside_circle() {
        let shape = SharedShape::ball(20.0);
        let bodies = vec![PositionedBody {
            id: 0u32,
            position: Point2D::new(100.0, 100.0),
            shape,
        }];
        let hits = point_query(&bodies, Point2D::new(105.0, 100.0));
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, 0);
        assert_eq!(hits[0].distance, 0.0);
    }

    #[test]
    fn point_query_miss_outside_circle() {
        let shape = SharedShape::ball(10.0);
        let bodies = vec![PositionedBody {
            id: 0u32,
            position: Point2D::new(100.0, 100.0),
            shape,
        }];
        let hits = point_query(&bodies, Point2D::new(200.0, 200.0));
        assert!(hits.is_empty());
    }

    #[test]
    fn nearest_body_finds_closest() {
        let bodies = vec![
            PositionedBody {
                id: 0u32,
                position: Point2D::new(100.0, 100.0),
                shape: SharedShape::ball(10.0),
            },
            PositionedBody {
                id: 1,
                position: Point2D::new(130.0, 100.0),
                shape: SharedShape::ball(10.0),
            },
        ];
        let result = nearest_body(&bodies, Point2D::new(115.0, 100.0), 20.0);
        assert!(result.is_some());
    }

    #[test]
    fn nearest_body_respects_max_distance() {
        let bodies = vec![PositionedBody {
            id: 0u32,
            position: Point2D::new(100.0, 100.0),
            shape: SharedShape::ball(10.0),
        }];
        let result = nearest_body(&bodies, Point2D::new(200.0, 200.0), 5.0);
        assert!(result.is_none());
    }

    #[test]
    fn shapes_overlap_true() {
        let a = SharedShape::ball(20.0);
        let b = SharedShape::ball(20.0);
        assert!(shapes_overlap(
            Point2D::new(0.0, 0.0),
            &a,
            Point2D::new(30.0, 0.0),
            &b
        ));
    }

    #[test]
    fn shapes_overlap_false() {
        let a = SharedShape::ball(10.0);
        let b = SharedShape::ball(10.0);
        assert!(!shapes_overlap(
            Point2D::new(0.0, 0.0),
            &a,
            Point2D::new(100.0, 0.0),
            &b
        ));
    }

    #[test]
    fn shapes_overlap_mixed_types() {
        let circle = SharedShape::ball(15.0);
        let rect = SharedShape::cuboid(20.0, 10.0);
        assert!(shapes_overlap(
            Point2D::new(0.0, 0.0),
            &circle,
            Point2D::new(30.0, 0.0),
            &rect
        ));
    }
}

