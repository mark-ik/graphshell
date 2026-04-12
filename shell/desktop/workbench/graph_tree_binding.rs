/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Graphlet binding bridge: maps app-layer graphlet concepts to GraphTree
//! `GraphletRef` entries.
//!
//! When Graphshell derives graphlets (ego, corridor, component, etc.)
//! from the domain graph, this module registers them in the GraphTree
//! as `GraphletRef` entries with appropriate binding state. Frame groups
//! in the workbench map to `UnlinkedSession` or `Linked` graphlets.
//!
//! This bridge replaces the ad-hoc graphlet-to-tile correspondence that
//! currently lives in `tile_grouping.rs` and frame layout hints.

use graph_tree::{
    GraphTree, GraphletBinding, GraphletId, GraphletKind as TreeGraphletKind, GraphletRef,
    GraphletSpec as TreeGraphletSpec,
};

use crate::graph::{self, NodeKey};

/// Map an app-layer `GraphletKind` to graph-tree's `GraphletKind`.
pub(crate) fn to_tree_graphlet_kind(kind: &graph::GraphletKind) -> TreeGraphletKind {
    match kind {
        graph::GraphletKind::Ego { radius } => TreeGraphletKind::Ego { radius: *radius },
        graph::GraphletKind::Corridor => TreeGraphletKind::Corridor,
        graph::GraphletKind::Component => TreeGraphletKind::Component,
        graph::GraphletKind::Loop => TreeGraphletKind::Loop,
        graph::GraphletKind::Frontier => TreeGraphletKind::Frontier,
        graph::GraphletKind::Facet => TreeGraphletKind::Facet,
        graph::GraphletKind::Session => TreeGraphletKind::Session,
        graph::GraphletKind::Bridge => TreeGraphletKind::Bridge,
        graph::GraphletKind::WorkbenchCorrespondence => {
            TreeGraphletKind::WorkbenchCorrespondence
        }
    }
}

/// Register a session-only graphlet (unlinked, no canonical spec).
///
/// Used for ad-hoc frame groups created by user arrangement.
pub(crate) fn register_session_graphlet(
    graph_tree: &mut GraphTree<NodeKey>,
    id: GraphletId,
    anchor: Option<NodeKey>,
    members: &[NodeKey],
) {
    let mut graphlet = GraphletRef::new_session(id);
    if let Some(anchor) = anchor {
        graphlet = graphlet.with_anchor(anchor);
    }

    // Register graphlet in the tree's graphlet index.
    graph_tree.add_graphlet(graphlet);

    // Tag members with graphlet membership.
    for &member in members {
        if let Some(entry) = graph_tree.get_mut(&member) {
            if !entry.graphlet_membership.contains(&id) {
                entry.graphlet_membership.push(id);
            }
        }
    }
}

/// Register a derived graphlet with a linked binding.
///
/// Used when a graphlet is derived from a canonical spec (ego, corridor, etc.).
pub(crate) fn register_linked_graphlet(
    graph_tree: &mut GraphTree<NodeKey>,
    id: GraphletId,
    kind: &graph::GraphletKind,
    anchors: Vec<NodeKey>,
    members: &[NodeKey],
) {
    let tree_kind = to_tree_graphlet_kind(kind);
    let graphlet = GraphletRef {
        id,
        anchors: anchors.clone(),
        primary_anchor: anchors.first().cloned(),
        binding: GraphletBinding::Linked {
            spec: TreeGraphletSpec {
                kind: tree_kind.clone(),
                anchors: anchors.iter().map(|k| k.index().to_string()).collect(),
                primary_anchor: anchors.first().map(|k| k.index().to_string()),
                selectors: Vec::new(),
            },
        },
        kind: Some(tree_kind),
    };

    graph_tree.add_graphlet(graphlet);

    for &member in members {
        if let Some(entry) = graph_tree.get_mut(&member) {
            if !entry.graphlet_membership.contains(&id) {
                entry.graphlet_membership.push(id);
            }
        }
    }
}

/// Clear all graphlet registrations from the GraphTree.
///
/// Used before a full re-derivation pass.
pub(crate) fn clear_graphlets(graph_tree: &mut GraphTree<NodeKey>) {
    graph_tree.graphlets_mut().clear();

    // Clear membership tags on all members.
    let member_keys: Vec<NodeKey> = graph_tree.members().map(|(k, _)| *k).collect();
    for key in member_keys {
        if let Some(entry) = graph_tree.get_mut(&key) {
            entry.graphlet_membership.clear();
        }
    }
}

// ---------------------------------------------------------------------------
// Reconciliation bridge
// ---------------------------------------------------------------------------

/// Reconcile a linked graphlet against graph-truth membership.
///
/// Returns a reconciliation proposal if the tree has drifted from graph truth,
/// or `None` if the graphlet is already synchronized.
///
/// The host should present the proposal to the user (or auto-apply via policy)
/// and then call `apply_reconciliation_choice` with the chosen outcome.
pub(crate) fn reconcile_linked_graphlet(
    graph_tree: &GraphTree<NodeKey>,
    graphlet_id: GraphletId,
    graph_truth_members: &[NodeKey],
) -> Option<graph_tree::ReconciliationProposal<NodeKey>> {
    graph_tree::reconciliation::propose_reconciliation(
        graph_tree,
        graphlet_id,
        graph_truth_members,
        "graph truth changed",
    )
}

/// Apply a reconciliation choice to the tree.
///
/// Returns tree intents for the host to act on (activation/dismissal requests).
pub(crate) fn apply_reconciliation_choice(
    graph_tree: &mut GraphTree<NodeKey>,
    proposal: &graph_tree::ReconciliationProposal<NodeKey>,
    choice: graph_tree::ReconciliationChoice,
) -> Vec<graph_tree::TreeIntent<NodeKey>> {
    graph_tree::reconciliation::apply_reconciliation(graph_tree, proposal, choice)
}

/// Check if a manual mutation on a member should trigger a fork of its
/// linked graphlet. Returns the graphlet ID and reason if so.
///
/// Called by dual-write paths before applying dismiss/detach on a member
/// that might belong to a linked graphlet.
pub(crate) fn check_fork_on_manual_mutation(
    graph_tree: &GraphTree<NodeKey>,
    member: &NodeKey,
    operation: &str,
) -> Option<(GraphletId, String)> {
    graph_tree::reconciliation::detect_fork_on_manual_override(graph_tree, member, operation)
}

/// Apply a fork transition on a linked graphlet.
///
/// Converts the graphlet from `Linked` to `Forked`, preserving the parent spec.
pub(crate) fn apply_fork(
    graph_tree: &mut GraphTree<NodeKey>,
    graphlet_id: GraphletId,
    reason: String,
) {
    graph_tree::reconciliation::apply_fork(graph_tree, graphlet_id, reason);
}
