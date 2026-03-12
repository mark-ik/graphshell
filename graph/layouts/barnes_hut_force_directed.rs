/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use crate::graph::physics::{GraphPhysicsState, Layout};
use egui::{Pos2, Rect, Vec2, pos2};
use petgraph::{EdgeType, csr::IndexType, stable_graph::NodeIndex};

const BARNES_HUT_THETA: f32 = 0.75;
const BARNES_HUT_MAX_DEPTH: usize = 8;
const BARNES_HUT_MIN_CELL_SIZE: f32 = 8.0;

#[derive(Debug, Default)]
pub(crate) struct BarnesHutForceDirectedLayout {
    state: GraphPhysicsState,
    scratch_disp: Vec<Vec2>,
}

impl BarnesHutForceDirectedLayout {
    pub(crate) fn new_from_state(state: GraphPhysicsState) -> Self {
        Self {
            state,
            scratch_disp: Vec::new(),
        }
    }
}

impl Layout<GraphPhysicsState> for BarnesHutForceDirectedLayout {
    fn from_state(state: GraphPhysicsState) -> impl Layout<GraphPhysicsState> {
        Self::new_from_state(state)
    }

    fn next<N, E, Ty, Ix, Dn, De>(
        &mut self,
        g: &mut egui_graphs::Graph<N, E, Ty, Ix, Dn, De>,
        ui: &egui::Ui,
    ) where
        N: Clone,
        E: Clone,
        Ty: EdgeType,
        Ix: IndexType,
        Dn: egui_graphs::DisplayNode<N, E, Ty, Ix>,
        De: egui_graphs::DisplayEdge<N, E, Ty, Ix, Dn>,
    {
        if !self.state.base.is_running || g.node_count() == 0 {
            return;
        }

        let Some(k) = prepare_constants(
            ui.ctx().content_rect(),
            g.node_count(),
            self.state.base.k_scale,
        ) else {
            return;
        };

        let indices: Vec<_> = g.g().node_indices().collect();
        if self.scratch_disp.len() == indices.len() {
            self.scratch_disp.fill(Vec2::ZERO);
        } else {
            self.scratch_disp.resize(indices.len(), Vec2::ZERO);
        }

        let positions: Vec<_> = indices
            .iter()
            .filter_map(|&idx| g.g().node_weight(idx).map(|node| node.location()))
            .collect();
        if positions.len() != indices.len() {
            return;
        }

        let bounds = quadtree_bounds(ui.ctx().content_rect(), &positions);
        let mut tree = BarnesHutNode::new(bounds);
        for body in 0..positions.len() {
            tree.insert(body, &positions, 0);
        }

        let repulsion_constant = self.state.base.c_repulse * (k * k);
        for (body, position) in positions.iter().copied().enumerate() {
            tree.accumulate_force(
                body,
                position,
                repulsion_constant,
                self.state.base.epsilon,
                BARNES_HUT_THETA,
                &mut self.scratch_disp[body],
            );
        }

        compute_attraction(
            g,
            &indices,
            &mut self.scratch_disp,
            k,
            self.state.base.epsilon,
            self.state.base.c_attract,
        );
        apply_center_gravity(
            &positions,
            &mut self.scratch_disp,
            ui.ctx().content_rect(),
            self.state.extras.0.enabled,
            self.state.extras.0.params.c,
        );
        let avg = apply_displacements(
            g,
            &indices,
            &self.scratch_disp,
            self.state.base.dt,
            self.state.base.damping,
            self.state.base.max_step,
        );
        self.state.base.last_avg_displacement = avg;
        self.state.base.step_count += 1;
    }

    fn state(&self) -> GraphPhysicsState {
        self.state.clone()
    }
}

fn prepare_constants(canvas: Rect, node_count: usize, k_scale: f32) -> Option<f32> {
    if node_count == 0 {
        return None;
    }
    let n = node_count as f32;
    let area = canvas.area().max(1.0);
    let k_ideal = (area / n).sqrt();
    let k = k_ideal * k_scale;
    if !k.is_finite() {
        return None;
    }
    Some(k)
}

fn quadtree_bounds(view: Rect, positions: &[Pos2]) -> Rect {
    let mut min = positions.first().copied().unwrap_or_else(|| pos2(0.0, 0.0));
    let mut max = min;
    for position in positions {
        min.x = min.x.min(position.x);
        min.y = min.y.min(position.y);
        max.x = max.x.max(position.x);
        max.y = max.y.max(position.y);
    }
    min.x = min.x.min(view.min.x);
    min.y = min.y.min(view.min.y);
    max.x = max.x.max(view.max.x);
    max.y = max.y.max(view.max.y);

    let center = pos2((min.x + max.x) * 0.5, (min.y + max.y) * 0.5);
    let half_extent = ((max.x - min.x).max(max.y - min.y) * 0.5).max(BARNES_HUT_MIN_CELL_SIZE);
    Rect::from_center_size(center, Vec2::splat(half_extent * 2.0 + 16.0))
}

fn compute_attraction<N, E, Ty, Ix, Dn, De>(
    g: &egui_graphs::Graph<N, E, Ty, Ix, Dn, De>,
    indices: &[NodeIndex<Ix>],
    disp: &mut [Vec2],
    k: f32,
    epsilon: f32,
    c_attract: f32,
) where
    N: Clone,
    E: Clone,
    Ty: EdgeType,
    Ix: IndexType,
    Dn: egui_graphs::DisplayNode<N, E, Ty, Ix>,
    De: egui_graphs::DisplayEdge<N, E, Ty, Ix, Dn>,
{
    for (vec_pos, &idx) in indices.iter().enumerate() {
        let Some(node) = g.g().node_weight(idx) else {
            continue;
        };
        let loc = node.location();
        for nbr in g.g().neighbors_undirected(idx) {
            let Some(neighbor) = g.g().node_weight(nbr) else {
                continue;
            };
            let delta = neighbor.location() - loc;
            let distance = delta.length().max(epsilon);
            let force = c_attract * (distance * distance) / k;
            disp[vec_pos] += (delta / distance) * force;
        }
    }
}

fn apply_center_gravity(
    positions: &[Pos2],
    disp: &mut [Vec2],
    area: Rect,
    enabled: bool,
    strength: f32,
) {
    if !enabled || strength == 0.0 {
        return;
    }

    let center = area.center();
    for (vec_pos, position) in positions.iter().enumerate() {
        disp[vec_pos] += (center - *position) * strength;
    }
}

fn apply_displacements<N, E, Ty, Ix, Dn, De>(
    g: &mut egui_graphs::Graph<N, E, Ty, Ix, Dn, De>,
    indices: &[NodeIndex<Ix>],
    disp: &[Vec2],
    dt: f32,
    damping: f32,
    max_step: f32,
) -> Option<f32>
where
    N: Clone,
    E: Clone,
    Ty: EdgeType,
    Ix: IndexType,
    Dn: egui_graphs::DisplayNode<N, E, Ty, Ix>,
    De: egui_graphs::DisplayEdge<N, E, Ty, Ix, Dn>,
{
    if indices.is_empty() {
        return Some(0.0);
    }

    let mut sum = 0.0_f32;
    let mut count = 0_usize;
    for (vec_pos, &idx) in indices.iter().enumerate() {
        let mut step = disp[vec_pos] * dt * damping;
        let len = step.length();
        if len > max_step {
            step = step.normalized() * max_step;
        }
        let Some(node) = g.g().node_weight(idx) else {
            continue;
        };
        let new_loc = node.location() + step;
        if !new_loc.x.is_finite() || !new_loc.y.is_finite() {
            continue;
        }
        if let Some(node) = g.g_mut().node_weight_mut(idx) {
            node.set_location(new_loc);
            sum += len.min(max_step);
            count += 1;
        }
    }

    if count == 0 {
        None
    } else {
        Some(sum / count as f32)
    }
}

#[derive(Debug)]
struct BarnesHutNode {
    bounds: Rect,
    mass: f32,
    center_of_mass: Pos2,
    body: Option<usize>,
    children: [Option<Box<BarnesHutNode>>; 4],
}

impl BarnesHutNode {
    fn new(bounds: Rect) -> Self {
        Self {
            bounds,
            mass: 0.0,
            center_of_mass: bounds.center(),
            body: None,
            children: Default::default(),
        }
    }

    fn is_leaf(&self) -> bool {
        self.children.iter().all(Option::is_none)
    }

    fn insert(&mut self, body: usize, positions: &[Pos2], depth: usize) {
        let position = positions[body];
        if self.mass == 0.0 && self.body.is_none() && self.is_leaf() {
            self.mass = 1.0;
            self.center_of_mass = position;
            self.body = Some(body);
            return;
        }

        let prev_mass = self.mass;
        self.mass += 1.0;
        self.center_of_mass = pos2(
            (self.center_of_mass.x * prev_mass + position.x) / self.mass,
            (self.center_of_mass.y * prev_mass + position.y) / self.mass,
        );

        if depth >= BARNES_HUT_MAX_DEPTH || self.bounds.width() <= BARNES_HUT_MIN_CELL_SIZE {
            self.body = None;
            return;
        }

        if self.is_leaf() {
            if let Some(existing) = self.body.take() {
                if (positions[existing] - position).length_sq() <= f32::EPSILON {
                    return;
                }
                self.insert_into_child(existing, positions, depth + 1);
            } else {
                return;
            }
        }

        self.insert_into_child(body, positions, depth + 1);
    }

    fn insert_into_child(&mut self, body: usize, positions: &[Pos2], depth: usize) {
        let child_index = self.child_index(positions[body]);
        let child_bounds = self.child_bounds(child_index);
        let child = self.children[child_index]
            .get_or_insert_with(|| Box::new(BarnesHutNode::new(child_bounds)));
        child.insert(body, positions, depth);
    }

    fn child_index(&self, position: Pos2) -> usize {
        let center = self.bounds.center();
        let east = position.x >= center.x;
        let south = position.y >= center.y;
        match (east, south) {
            (false, false) => 0,
            (true, false) => 1,
            (false, true) => 2,
            (true, true) => 3,
        }
    }

    fn child_bounds(&self, child_index: usize) -> Rect {
        let center = self.bounds.center();
        match child_index {
            0 => Rect::from_min_max(self.bounds.min, center),
            1 => Rect::from_min_max(
                pos2(center.x, self.bounds.min.y),
                pos2(self.bounds.max.x, center.y),
            ),
            2 => Rect::from_min_max(
                pos2(self.bounds.min.x, center.y),
                pos2(center.x, self.bounds.max.y),
            ),
            _ => Rect::from_min_max(center, self.bounds.max),
        }
    }

    fn accumulate_force(
        &self,
        body: usize,
        target: Pos2,
        repulsion_constant: f32,
        epsilon: f32,
        theta: f32,
        disp: &mut Vec2,
    ) {
        if self.mass == 0.0 {
            return;
        }

        if self.is_leaf() {
            if self.body == Some(body) {
                return;
            }
            let delta = target - self.center_of_mass;
            let distance = delta.length().max(epsilon);
            if distance <= epsilon {
                return;
            }
            let force = repulsion_constant * self.mass / distance;
            *disp += (delta / distance) * force;
            return;
        }

        let delta = target - self.center_of_mass;
        let distance = delta.length().max(epsilon);
        let cell_size = self
            .bounds
            .width()
            .max(self.bounds.height())
            .max(BARNES_HUT_MIN_CELL_SIZE);
        if !self.bounds.contains(target) && cell_size / distance < theta {
            let force = repulsion_constant * self.mass / distance;
            *disp += (delta / distance) * force;
            return;
        }

        for child in self.children.iter().flatten() {
            child.accumulate_force(body, target, repulsion_constant, epsilon, theta, disp);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::physics::default_graph_physics_state;

    #[test]
    fn barnes_hut_layout_round_trips_state() {
        let state = default_graph_physics_state();
        let layout = BarnesHutForceDirectedLayout::new_from_state(state.clone());
        let round_trip = layout.state();

        assert_eq!(round_trip.base.c_repulse, state.base.c_repulse);
        assert_eq!(round_trip.base.c_attract, state.base.c_attract);
        assert_eq!(round_trip.base.damping, state.base.damping);
        assert_eq!(round_trip.base.k_scale, state.base.k_scale);
        assert_eq!(round_trip.base.dt, state.base.dt);
        assert_eq!(round_trip.base.max_step, state.base.max_step);
        assert_eq!(round_trip.extras.0.enabled, state.extras.0.enabled);
        assert_eq!(round_trip.extras.0.params.c, state.extras.0.params.c);
    }

    #[test]
    fn barnes_hut_tree_accumulates_repulsion_from_remote_mass() {
        let positions = vec![pos2(0.0, 0.0), pos2(80.0, 0.0), pos2(120.0, 0.0)];
        let mut tree = BarnesHutNode::new(Rect::from_center_size(
            pos2(60.0, 0.0),
            Vec2::new(256.0, 256.0),
        ));
        for body in 0..positions.len() {
            tree.insert(body, &positions, 0);
        }

        let mut disp = Vec2::ZERO;
        tree.accumulate_force(0, positions[0], 100.0, 0.001, BARNES_HUT_THETA, &mut disp);

        assert!(disp.x < 0.0);
        assert!(disp.length() > 0.0);
    }
}
