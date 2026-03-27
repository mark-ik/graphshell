/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// --- Spike Stage 1 receipt (2026-03-27) ---
//
// Question: Is the FR step separable from `&egui::Ui`?
//
// Finding: YES — the algorithm-level step signature is:
//
//   fn step<N, E, Ty, Ix, Dn, De>(
//       &mut self,
//       g: &mut egui_graphs::Graph<N, E, Ty, Ix, Dn, De>,
//       view: egui::Rect,
//   )
//
// `&egui::Ui` only enters through `ForceDirected::next()`, which calls
// `self.alg.step(g, ui.ctx().content_rect())`. The Rect is a plain value
// type — it can be captured before any async boundary.
//
// Question: Is `egui_graphs::Graph` (as used by Graphshell) `Send`?
//
// Finding: NOT PRACTICALLY USABLE ACROSS THREADS — `egui_graphs::Graph`
// lives in egui persistent storage, owned by the egui `Context` memory map
// and keyed by widget ID. Even if the generic type parameters satisfy `Send`
// (no explicit `!Send` impl exists in the struct definition), passing the
// live `EguiGraph` reference to a background thread is structurally
// impossible: it can only be accessed under egui's memory lock, which is
// held for the duration of a frame. Additionally, `GraphNodeShape` contains
// `Option<egui::TextureHandle>`, which ties the type to egui's texture
// registry. Any physics worker must operate on extracted plain data
// (`Vec<(NodeIndex, Pos2, velocity)>`), not on the `EguiGraph` directly.
//
// Consequence: the `&egui::Ui` coupling in `Layout::next()` is NOT the
// primary obstacle. The real constraint is that `EguiGraph` is frame-scoped.
// A worker receives a copy of position/velocity data and returns a copy of
// updated positions. The frame loop applies the results at frame start.

use crate::graph::physics::{ForceAlgorithm, GraphPhysicsLayout, GraphPhysicsState, Layout};

#[derive(Debug, Default)]
pub(crate) struct GraphshellForceDirectedLayout {
    algorithm: GraphPhysicsLayout,
}

impl GraphshellForceDirectedLayout {
    pub(crate) fn new_from_state(state: GraphPhysicsState) -> Self {
        Self {
            algorithm: GraphPhysicsLayout::from_state(state),
        }
    }
}

impl Layout<GraphPhysicsState> for GraphshellForceDirectedLayout {
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
        Ty: petgraph::EdgeType,
        Ix: petgraph::stable_graph::IndexType,
        Dn: egui_graphs::DisplayNode<N, E, Ty, Ix>,
        De: egui_graphs::DisplayEdge<N, E, Ty, Ix, Dn>,
    {
        if g.node_count() == 0 {
            return;
        }

        self.algorithm.step(g, ui.ctx().content_rect());
    }

    fn state(&self) -> GraphPhysicsState {
        self.algorithm.state()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::physics::default_graph_physics_state;

    #[test]
    fn graphshell_force_directed_layout_round_trips_state() {
        let state = default_graph_physics_state();
        let layout = GraphshellForceDirectedLayout::new_from_state(state.clone());
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
}
