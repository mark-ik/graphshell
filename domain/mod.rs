use std::collections::HashMap;

use crate::app::{NoteId, NoteRecord};
use crate::graph::Graph;

/// Durable domain state owned by the app but independent of workbench/runtime layout.
pub struct DomainState {
    /// The canonical durable graph truth.
    pub graph: Graph,
    /// Counter for unique placeholder URLs (about:blank#1, about:blank#2, ...).
    /// Prevents `url_to_node` clobbering when pressing N multiple times.
    pub(super) next_placeholder_id: u32,
    /// Durable note documents keyed by note identity.
    pub(super) notes: HashMap<NoteId, NoteRecord>,
    /// Per-graph default navigation policy. Graph views fall back to
    /// this when their `navigation_policy_override` is `None`. Lives on
    /// `DomainState` (not `Graph`) because it's a user-tunable feel
    /// knob, not graph topology. Host-neutral and shared between the
    /// egui and iced hosts.
    pub navigation_policy_default: graph_canvas::navigation::NavigationPolicy,
    /// Per-graph default node style. Graph views fall back to this when
    /// their `node_style_override` is `None`. Same lifecycle rationale
    /// as `navigation_policy_default`.
    pub node_style_default: graph_canvas::node_style::NodeStyle,
    /// Per-graph default simulate-motion profile. When set, graph views
    /// whose `simulate_motion_override` is `None` fall back here before
    /// falling back to `SimulateMotionProfile::for_preset(view.simulate_behavior_preset)`.
    /// `None` preserves the preset-driven fallback that predates this
    /// policy — pre-existing preset pickers keep working unchanged.
    pub simulate_motion_default: Option<graph_canvas::scene_physics::SimulateMotionProfile>,
}
