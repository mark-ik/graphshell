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
                anchors: anchors.iter().map(|k| format!("{:?}", k)).collect(),
                primary_anchor: anchors.first().map(|k| format!("{:?}", k)),
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
