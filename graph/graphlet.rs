/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Graphlet computation — weakly-connected components under an edge projection.
//!
//! A **graphlet** is the set of nodes reachable from a seed by traversing the
//! edge relations that are currently considered active for that projection.
//!
//! The selector-driven helpers in this module are the canonical implementation.
//! The older durable-only helpers remain as compatibility defaults for current
//! workbench arrangement flows, where graphlets are still derived from semantic
//! `UserGrouped` relations and arrangement `FrameMember` relations.
//!
//! This matches the graphlet model in the archived arrangement plan: graphlet
//! membership depends on the active edge projection, not on a permanently fixed
//! family allowlist.
//!
//! Live spec: `design_docs/graphshell_docs/implementation_strategy/workbench/graphlet_projection_binding_spec.md`
//! Historical background: `design_docs/archive_docs/checkpoint_2026-03-21/2026-03-20_arrangement_graph_projection_plan.md §2`

use std::collections::HashSet;

use petgraph::Direction;
use petgraph::visit::EdgeRef;

use crate::graph::{ArrangementSubKind, Graph, NodeKey};
use crate::model::graph::{EdgePayload, RelationSelector, SemanticSubKind};

/// Return all nodes in the same default durable graphlet as `seed`, excluding
/// `seed` itself.
///
/// This is the compatibility/default workbench projection: semantic
/// `UserGrouped` relations plus arrangement `FrameMember` relations.
pub(crate) fn graphlet_peers_for_node(graph: &Graph, seed: NodeKey) -> Vec<NodeKey> {
    graphlet_peers_for_node_with_filter(graph, seed, is_default_durable_edge_weight)
}

/// Return all nodes in the same graphlet as `seed` under a selector-driven
/// edge projection, excluding `seed` itself.
///
/// The graphlet boundary changes with the supplied selectors.
pub(crate) fn graphlet_peers_for_node_with_selectors(
    graph: &Graph,
    seed: NodeKey,
    selectors: &[RelationSelector],
) -> Vec<NodeKey> {
    graphlet_peers_for_node_with_filter(graph, seed, |payload| {
        payload_matches_any_selector(payload, selectors)
    })
}

/// Return all nodes in the same graphlet as any of `seeds` under a
/// selector-driven edge projection, including the seed nodes themselves.
pub(crate) fn graphlet_members_for_seeds_with_selectors(
    graph: &Graph,
    seeds: &[NodeKey],
    selectors: &[RelationSelector],
) -> Vec<NodeKey> {
    graphlet_members_for_seeds_with_filter(graph, seeds, |payload| {
        payload_matches_any_selector(payload, selectors)
    })
}

/// Return all nodes in the same graphlet as `seed` under an arbitrary edge
/// inclusion predicate, excluding `seed` itself.
pub(crate) fn graphlet_peers_for_node_with_filter<F>(
    graph: &Graph,
    seed: NodeKey,
    include_edge: F,
) -> Vec<NodeKey>
where
    F: Fn(&EdgePayload) -> bool,
{
    if graph.get_node(seed).is_none() {
        return Vec::new();
    }

    let mut visited = HashSet::from([seed]);
    let mut queue = vec![seed];
    let mut peers = Vec::new();

    while let Some(current) = queue.pop() {
        for neighbor in projected_neighbors(graph, current, &include_edge) {
            if visited.insert(neighbor) {
                peers.push(neighbor);
                queue.push(neighbor);
            }
        }
    }

    peers
}

/// Return all nodes in the same graphlet as any of `seeds` under an arbitrary
/// edge inclusion predicate, including the seed nodes themselves.
pub(crate) fn graphlet_members_for_seeds_with_filter<F>(
    graph: &Graph,
    seeds: &[NodeKey],
    include_edge: F,
) -> Vec<NodeKey>
where
    F: Fn(&EdgePayload) -> bool,
{
    let mut visited = HashSet::new();
    let mut queue = Vec::new();
    let mut members = Vec::new();

    for &seed in seeds {
        if graph.get_node(seed).is_none() {
            continue;
        }
        if visited.insert(seed) {
            queue.push(seed);
            members.push(seed);
        }
    }

    while let Some(current) = queue.pop() {
        for neighbor in projected_neighbors(graph, current, &include_edge) {
            if visited.insert(neighbor) {
                members.push(neighbor);
                queue.push(neighbor);
            }
        }
    }

    members
}

/// Return only the direct neighbors of `node` under the default durable
/// graphlet projection.
pub(crate) fn direct_durable_neighbors(graph: &Graph, node: NodeKey) -> Vec<NodeKey> {
    direct_graphlet_neighbors_with_filter(graph, node, is_default_durable_edge_weight)
}

/// Return only the direct neighbors of `node` under a selector-driven graphlet
/// projection.
pub(crate) fn direct_graphlet_neighbors_with_selectors(
    graph: &Graph,
    node: NodeKey,
    selectors: &[RelationSelector],
) -> Vec<NodeKey> {
    direct_graphlet_neighbors_with_filter(graph, node, |payload| {
        payload_matches_any_selector(payload, selectors)
    })
}

/// Return only the direct neighbors of `node` under an arbitrary edge
/// inclusion predicate.
pub(crate) fn direct_graphlet_neighbors_with_filter<F>(
    graph: &Graph,
    node: NodeKey,
    include_edge: F,
) -> Vec<NodeKey>
where
    F: Fn(&EdgePayload) -> bool,
{
    projected_neighbors(graph, node, &include_edge).collect()
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Iterator over all nodes reachable from `node` via a single included edge
/// (in either direction).
fn projected_neighbors<'a, F>(
    graph: &'a Graph,
    node: NodeKey,
    include_edge: &'a F,
) -> impl Iterator<Item = NodeKey> + 'a
where
    F: Fn(&EdgePayload) -> bool + 'a,
{
    // Collect both outgoing and incoming durable edges so the traversal is
    // undirected even though the underlying petgraph is directed.
    let outgoing = graph
        .inner
        .edges_directed(node, Direction::Outgoing)
        .filter(|e| include_edge(e.weight()))
        .map(|e| e.target());

    let incoming = graph
        .inner
        .edges_directed(node, Direction::Incoming)
        .filter(|e| include_edge(e.weight()))
        .map(|e| e.source());

    outgoing.chain(incoming)
}

fn is_default_durable_edge_weight(payload: &EdgePayload) -> bool {
    if payload.has_relation(RelationSelector::Semantic(SemanticSubKind::UserGrouped)) {
        return true;
    }
    if payload.has_arrangement_sub_kind(ArrangementSubKind::FrameMember) {
        return true;
    }
    false
}

fn payload_matches_any_selector(payload: &EdgePayload, selectors: &[RelationSelector]) -> bool {
    selectors
        .iter()
        .copied()
        .any(|selector| payload.has_relation(selector))
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

    fn assert_relation(graph: &mut Graph, from: NodeKey, to: NodeKey, assertion: crate::graph::EdgeAssertion) {
        apply_graph_delta(
            graph,
            GraphDelta::AssertRelation {
                from,
                to,
                assertion,
            },
        );
    }

    fn add_history_edge(graph: &mut Graph, from: NodeKey, to: NodeKey) {
        apply_graph_delta(
            graph,
            GraphDelta::AddEdge {
                from,
                to,
                edge_type: EdgeType::History,
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
        assert_relation(
            &mut graph,
            a,
            b,
            crate::graph::EdgeAssertion::Semantic {
                sub_kind: crate::graph::SemanticSubKind::UserGrouped,
                label: None,
                decay_progress: None,
            },
        );

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
        assert_relation(
            &mut graph,
            anchor,
            a,
            crate::graph::EdgeAssertion::Arrangement {
                sub_kind: ArrangementSubKind::FrameMember,
            },
        );
        assert_relation(
            &mut graph,
            anchor,
            b,
            crate::graph::EdgeAssertion::Arrangement {
                sub_kind: ArrangementSubKind::FrameMember,
            },
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
        assert_relation(
            &mut graph,
            a,
            b,
            crate::graph::EdgeAssertion::Semantic {
                sub_kind: crate::graph::SemanticSubKind::Hyperlink,
                label: None,
                decay_progress: None,
            },
        );

        // Hyperlink is not durable — graphlet peers must be empty
        assert!(graphlet_peers_for_node(&graph, a).is_empty());
        assert!(graphlet_peers_for_node(&graph, b).is_empty());

        let peers = graphlet_peers_for_node_with_selectors(
            &graph,
            a,
            &[RelationSelector::Semantic(SemanticSubKind::Hyperlink)],
        );
        assert_eq!(peers, vec![b]);
    }

    #[test]
    fn history_edge_does_not_link_peers() {
        let mut graph = Graph::new();
        let a = add_node(&mut graph, "https://a.test/");
        let b = add_node(&mut graph, "https://b.test/");
        add_history_edge(&mut graph, a, b);
        assert!(graphlet_peers_for_node(&graph, a).is_empty());

        let peers = graphlet_peers_for_node_with_selectors(
            &graph,
            a,
            &[RelationSelector::Family(crate::graph::EdgeFamily::Traversal)],
        );
        assert_eq!(peers, vec![b]);
    }

    #[test]
    fn tile_group_arrangement_does_not_link_peers() {
        // TileGroup sub-kind is session-only, not durable
        let mut graph = Graph::new();
        let a = add_node(&mut graph, "https://a.test/");
        let b = add_node(&mut graph, "https://b.test/");
        assert_relation(
            &mut graph,
            a,
            b,
            crate::graph::EdgeAssertion::Arrangement {
                sub_kind: ArrangementSubKind::TileGroup,
            },
        );
        assert!(graphlet_peers_for_node(&graph, a).is_empty());
    }

    #[test]
    fn direct_durable_neighbors_one_hop_only() {
        let mut graph = Graph::new();
        let a = add_node(&mut graph, "https://a.test/");
        let b = add_node(&mut graph, "https://b.test/");
        let c = add_node(&mut graph, "https://c.test/");
        assert_relation(
            &mut graph,
            a,
            b,
            crate::graph::EdgeAssertion::Semantic {
                sub_kind: crate::graph::SemanticSubKind::UserGrouped,
                label: None,
                decay_progress: None,
            },
        );
        assert_relation(
            &mut graph,
            b,
            c,
            crate::graph::EdgeAssertion::Semantic {
                sub_kind: crate::graph::SemanticSubKind::UserGrouped,
                label: None,
                decay_progress: None,
            },
        );

        // direct neighbors of a: only b (not c)
        let neighbors = direct_durable_neighbors(&graph, a);
        assert_eq!(neighbors, vec![b]);
    }

    #[test]
    fn direct_neighbors_follow_selector_projection() {
        let mut graph = Graph::new();
        let a = add_node(&mut graph, "https://a.test/");
        let b = add_node(&mut graph, "https://b.test/");
        assert_relation(
            &mut graph,
            a,
            b,
            crate::graph::EdgeAssertion::Semantic {
                sub_kind: crate::graph::SemanticSubKind::Hyperlink,
                label: None,
                decay_progress: None,
            },
        );

        let neighbors = direct_graphlet_neighbors_with_selectors(
            &graph,
            a,
            &[RelationSelector::Semantic(SemanticSubKind::Hyperlink)],
        );
        assert_eq!(neighbors, vec![b]);
    }

    #[test]
    fn graphlet_members_for_multiple_seeds_follows_selector_projection() {
        let mut graph = Graph::new();
        let a = add_node(&mut graph, "https://a.test/");
        let b = add_node(&mut graph, "https://b.test/");
        let c = add_node(&mut graph, "https://c.test/");
        add_history_edge(&mut graph, a, b);
        add_history_edge(&mut graph, b, c);

        let mut members = graphlet_members_for_seeds_with_selectors(
            &graph,
            &[a, c],
            &[RelationSelector::Family(crate::graph::EdgeFamily::Traversal)],
        );
        members.sort_by_key(|key| key.index());
        assert_eq!(members, vec![a, b, c]);
    }
}
