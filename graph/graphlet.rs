/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Graphlet computation — durable weakly-connected components.
//!
//! A **graphlet** is the set of nodes reachable from a seed via *durable* edges
//! only: [`EdgeKind::UserGrouped`] and
//! [`EdgeKind::ArrangementRelation`] / [`ArrangementSubKind::FrameMember`].
//!
//! Circumstantial edges (Hyperlink, History, ContainmentRelation, AgentDerived)
//! are deliberately excluded so that graphlets reflect intentional user
//! grouping rather than derived browsing context.  The resulting set is the
//! canonical roster for a tile group: warm nodes map to visible tiles; cold
//! nodes are surfaced only in the omnibar and navigator.
//!
//! Spec: `2026-03-20_arrangement_graph_projection_plan.md §2`

use std::collections::HashSet;

use petgraph::Direction;
use petgraph::visit::EdgeRef;

use crate::graph::{ArrangementSubKind, Graph, NodeKey};
use crate::model::graph::EdgeKind;

/// Return all nodes in the same durable graphlet as `seed`, **excluding** `seed`
/// itself.
///
/// Traversal is undirected and limited to edges that carry
/// [`EdgeKind::UserGrouped`] or
/// [`EdgeKind::ArrangementRelation`]+[`ArrangementSubKind::FrameMember`].
///
/// Returns an empty `Vec` if `seed` is not present in the graph.
pub(crate) fn graphlet_peers_for_node(graph: &Graph, seed: NodeKey) -> Vec<NodeKey> {
    if graph.get_node(seed).is_none() {
        return Vec::new();
    }

    let mut visited = HashSet::from([seed]);
    let mut queue = vec![seed];
    let mut peers = Vec::new();

    while let Some(current) = queue.pop() {
        for neighbor in durable_neighbors(graph, current) {
            if visited.insert(neighbor) {
                peers.push(neighbor);
                queue.push(neighbor);
            }
        }
    }

    peers
}

/// Return only the **direct** durable neighbors of `node` (one-hop, no BFS).
///
/// Useful for checking whether a newly opened node has any graphlet peers
/// without the cost of a full component traversal.
pub(crate) fn direct_durable_neighbors(graph: &Graph, node: NodeKey) -> Vec<NodeKey> {
    durable_neighbors(graph, node).collect()
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Iterator over all nodes reachable from `node` via a single durable edge
/// (in either direction).
fn durable_neighbors(graph: &Graph, node: NodeKey) -> impl Iterator<Item = NodeKey> + '_ {
    // Collect both outgoing and incoming durable edges so the traversal is
    // undirected even though the underlying petgraph is directed.
    let outgoing = graph
        .inner
        .edges_directed(node, Direction::Outgoing)
        .filter(|e| is_durable_edge_weight(e.weight()))
        .map(|e| e.target());

    let incoming = graph
        .inner
        .edges_directed(node, Direction::Incoming)
        .filter(|e| is_durable_edge_weight(e.weight()))
        .map(|e| e.source());

    outgoing.chain(incoming)
}

fn is_durable_edge_weight(payload: &crate::model::graph::EdgePayload) -> bool {
    if payload.kinds.contains(&EdgeKind::UserGrouped) {
        return true;
    }
    if payload.kinds.contains(&EdgeKind::ArrangementRelation) {
        if let Some(arrangement) = &payload.arrangement {
            if arrangement.sub_kinds.contains(&ArrangementSubKind::FrameMember) {
                return true;
            }
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::graph::apply::{GraphDelta, GraphDeltaResult, apply_graph_delta};
    use crate::model::graph::{ArrangementSubKind, EdgeType, Graph};

    fn add_node(graph: &mut Graph, url: &str) -> NodeKey {
        let GraphDeltaResult::NodeAdded(key) = apply_graph_delta(
            graph,
            GraphDelta::AddNode {
                id: None,
                url: url.to_string(),
                position: euclid::default::Point2D::new(0.0, 0.0),
            },
        ) else {
            panic!("expected NodeAdded");
        };
        key
    }

    fn add_edge(graph: &mut Graph, from: NodeKey, to: NodeKey, edge_type: EdgeType) {
        apply_graph_delta(
            graph,
            GraphDelta::AddEdge {
                from,
                to,
                edge_type,
                edge_label: None,
            },
        );
    }

    #[test]
    fn empty_graph_returns_empty() {
        let graph = Graph::new();
        let seed = NodeKey::default();
        let peers = graphlet_peers_for_node(&graph, seed);
        assert!(peers.is_empty());
    }

    #[test]
    fn isolated_node_has_no_peers() {
        let mut graph = Graph::new();
        let a = add_node(&mut graph, "https://a.test/");
        assert!(graphlet_peers_for_node(&graph, a).is_empty());
    }

    #[test]
    fn user_grouped_edge_links_peers() {
        let mut graph = Graph::new();
        let a = add_node(&mut graph, "https://a.test/");
        let b = add_node(&mut graph, "https://b.test/");
        add_edge(&mut graph, a, b, EdgeType::UserGrouped);

        let peers_of_a = graphlet_peers_for_node(&graph, a);
        assert_eq!(peers_of_a, vec![b]);

        let peers_of_b = graphlet_peers_for_node(&graph, b);
        assert_eq!(peers_of_b, vec![a]);
    }

    #[test]
    fn frame_member_edge_links_peers() {
        let mut graph = Graph::new();
        let anchor = add_node(&mut graph, "graphshell://frame/1");
        let a = add_node(&mut graph, "https://a.test/");
        let b = add_node(&mut graph, "https://b.test/");
        add_edge(
            &mut graph,
            anchor,
            a,
            EdgeType::ArrangementRelation(ArrangementSubKind::FrameMember),
        );
        add_edge(
            &mut graph,
            anchor,
            b,
            EdgeType::ArrangementRelation(ArrangementSubKind::FrameMember),
        );

        let peers_of_a = graphlet_peers_for_node(&graph, a);
        // a can reach anchor, and from anchor can reach b
        assert!(peers_of_a.contains(&anchor));
        assert!(peers_of_a.contains(&b));
    }

    #[test]
    fn hyperlink_edge_does_not_link_peers() {
        let mut graph = Graph::new();
        let a = add_node(&mut graph, "https://a.test/");
        let b = add_node(&mut graph, "https://b.test/");
        add_edge(&mut graph, a, b, EdgeType::Hyperlink);

        // Hyperlink is not durable — graphlet peers must be empty
        assert!(graphlet_peers_for_node(&graph, a).is_empty());
        assert!(graphlet_peers_for_node(&graph, b).is_empty());
    }

    #[test]
    fn history_edge_does_not_link_peers() {
        let mut graph = Graph::new();
        let a = add_node(&mut graph, "https://a.test/");
        let b = add_node(&mut graph, "https://b.test/");
        add_edge(&mut graph, a, b, EdgeType::History);
        assert!(graphlet_peers_for_node(&graph, a).is_empty());
    }

    #[test]
    fn tile_group_arrangement_does_not_link_peers() {
        // TileGroup sub-kind is session-only, not durable
        let mut graph = Graph::new();
        let a = add_node(&mut graph, "https://a.test/");
        let b = add_node(&mut graph, "https://b.test/");
        add_edge(
            &mut graph,
            a,
            b,
            EdgeType::ArrangementRelation(ArrangementSubKind::TileGroup),
        );
        assert!(graphlet_peers_for_node(&graph, a).is_empty());
    }

    #[test]
    fn direct_durable_neighbors_one_hop_only() {
        let mut graph = Graph::new();
        let a = add_node(&mut graph, "https://a.test/");
        let b = add_node(&mut graph, "https://b.test/");
        let c = add_node(&mut graph, "https://c.test/");
        add_edge(&mut graph, a, b, EdgeType::UserGrouped);
        add_edge(&mut graph, b, c, EdgeType::UserGrouped);

        // direct neighbors of a: only b (not c)
        let neighbors = direct_durable_neighbors(&graph, a);
        assert_eq!(neighbors, vec![b]);
    }
}
