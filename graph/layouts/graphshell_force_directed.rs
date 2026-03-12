/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

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
