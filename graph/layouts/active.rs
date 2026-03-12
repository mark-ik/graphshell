/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use crate::graph::layouts::barnes_hut_force_directed::BarnesHutForceDirectedLayout;
use crate::graph::layouts::graphshell_force_directed::GraphshellForceDirectedLayout;
use crate::graph::physics::{GraphPhysicsState, Layout, LayoutState, default_graph_physics_state};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub(crate) enum ActiveLayoutKind {
    #[default]
    ForceDirected,
    BarnesHut,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ActiveLayoutState {
    pub(crate) kind: ActiveLayoutKind,
    pub(crate) physics: GraphPhysicsState,
}

impl Default for ActiveLayoutState {
    fn default() -> Self {
        Self {
            kind: ActiveLayoutKind::ForceDirected,
            physics: default_graph_physics_state(),
        }
    }
}

impl LayoutState for ActiveLayoutState {}

#[derive(Debug)]
pub(crate) enum ActiveLayout {
    ForceDirected(GraphshellForceDirectedLayout),
    BarnesHut(BarnesHutForceDirectedLayout),
}

impl Default for ActiveLayout {
    fn default() -> Self {
        Self::ForceDirected(GraphshellForceDirectedLayout::default())
    }
}

impl ActiveLayout {
    pub(crate) fn new_from_state(state: ActiveLayoutState) -> Self {
        match state.kind {
            ActiveLayoutKind::ForceDirected => {
                Self::ForceDirected(GraphshellForceDirectedLayout::new_from_state(state.physics))
            }
            ActiveLayoutKind::BarnesHut => {
                Self::BarnesHut(BarnesHutForceDirectedLayout::new_from_state(state.physics))
            }
        }
    }

    pub(crate) fn kind(&self) -> ActiveLayoutKind {
        match self {
            Self::ForceDirected(_) => ActiveLayoutKind::ForceDirected,
            Self::BarnesHut(_) => ActiveLayoutKind::BarnesHut,
        }
    }
}

impl Layout<ActiveLayoutState> for ActiveLayout {
    fn from_state(state: ActiveLayoutState) -> impl Layout<ActiveLayoutState> {
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
        match self {
            Self::ForceDirected(layout) => layout.next(g, ui),
            Self::BarnesHut(layout) => layout.next(g, ui),
        }
    }

    fn state(&self) -> ActiveLayoutState {
        match self {
            Self::ForceDirected(layout) => ActiveLayoutState {
                kind: ActiveLayoutKind::ForceDirected,
                physics: layout.state(),
            },
            Self::BarnesHut(layout) => ActiveLayoutState {
                kind: ActiveLayoutKind::BarnesHut,
                physics: layout.state(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::physics::default_graph_physics_state;

    #[test]
    fn active_layout_defaults_to_force_directed() {
        let state = ActiveLayoutState::default();
        let layout = ActiveLayout::new_from_state(state);

        assert_eq!(layout.kind(), ActiveLayoutKind::ForceDirected);
    }

    #[test]
    fn active_layout_constructs_barnes_hut_variant() {
        let state = ActiveLayoutState {
            kind: ActiveLayoutKind::BarnesHut,
            physics: default_graph_physics_state(),
        };
        let layout = ActiveLayout::new_from_state(state);

        assert_eq!(layout.kind(), ActiveLayoutKind::BarnesHut);
    }
}
