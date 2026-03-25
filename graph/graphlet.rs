/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Graphlet computation — spec types, derivation engine, and BFS primitives.
//!
//! A **graphlet** is a bounded, meaningful graph subset used for navigation,
//! understanding, comparison, or staged work. It is defined by a `GraphletSpec`
//! (kind + anchors + scope + selectors + optional ranking) and is resolved into
//! a `ResolvedGraphlet` (member nodes, included edges, frontier nodes) by
//! `derive_graphlet`.
//!
//! Canonical spec: `design_docs/graphshell_docs/technical_architecture/graphlet_model.md`
//!
//! ## Derivation algorithms by kind
//!
//! | Kind | Algorithm |
//! |---|---|
//! | `Ego { radius }` | hop-bounded BFS from anchors under `selectors` |
//! | `Corridor` | shortest undirected path between anchor[0] and anchor[1] under `selectors` |
//! | `Component` | full weakly-connected component containing anchors under `selectors` |
//! | `Loop` | strongly connected component(s) containing anchors (directed) |
//! | `Frontier` | ego graphlet at radius 1 + ranked candidate expansion boundary |
//! | `Facet` | all nodes reachable from anchors whose edges satisfy `selectors` (= component under that projection) |
//! | `Session` | `Facet` restricted to `Traversal` family edges |
//! | `Bridge` | nodes on *any* shortest path between the two anchor sets |
//! | `WorkbenchCorrespondence` | supplied anchor set treated as complete members |
//!
//! The older durable-only helpers remain as compatibility defaults for current
//! workbench arrangement flows (semantic `UserGrouped` + `FrameMember`).
//!
//! Live spec: `design_docs/graphshell_docs/implementation_strategy/workbench/graphlet_projection_binding_spec.md`
//! Historical background: `design_docs/archive_docs/checkpoint_2026-03-21/2026-03-20_arrangement_graph_projection_plan.md §2`

use std::collections::HashSet;

use petgraph::Direction;
use petgraph::visit::EdgeRef;

use crate::graph::{ArrangementSubKind, EdgeFamily, Graph, NodeKey};
use crate::model::graph::{EdgePayload, RelationSelector, SemanticSubKind};

// ---------------------------------------------------------------------------
// Public spec types
// ---------------------------------------------------------------------------

/// The kind of graphlet to derive, as defined in `graphlet_model.md §4–§5`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GraphletKind {
    /// Anchor(s) plus hop-bounded neighbourhood.
    /// `radius = 1` gives direct neighbours only.
    Ego { radius: u8 },
    /// Shortest undirected path between `anchors[0]` and `anchors[1]`.
    Corridor,
    /// Full weakly-connected component containing the anchors under `selectors`.
    Component,
    /// Strongly-connected component(s) containing the anchors (directed).
    Loop,
    /// One-hop ego graphlet plus a ranked candidate expansion boundary.
    Frontier,
    /// All nodes reachable from anchors via edges matching `selectors`.
    Facet,
    /// Traversal-family projection: browsing session / temporal slice.
    Session,
    /// Nodes on any shortest path between the two anchor sets.
    Bridge,
    /// Anchors are the complete membership (Workbench correspondence view).
    WorkbenchCorrespondence,
}

/// Maximum traversal depth used when no explicit scope is set.
const DEFAULT_MAX_HOPS: usize = 256;

/// Scope that bounds graphlet derivation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum GraphletScope {
    /// No bound — traverse the entire connected region under the selectors.
    Unbounded,
    /// Restrict membership to nodes within `max_hops` of any anchor.
    MaxHops(usize),
    /// Restrict membership to this explicit allow-list.
    NodeSet(Vec<NodeKey>),
}

/// Optional ranking hint for frontier derivation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RankingPolicy {
    /// Rank by ascending hop distance from anchors.
    ByHopDistance,
    /// Rank by descending in-degree among frontier candidates.
    ByInDegree,
}

/// Declarative description of a graphlet to derive.
///
/// The exact byte layout may evolve. The invariant is that derivation is
/// explicit and inspectable rather than a side effect hidden inside UI code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GraphletSpec {
    pub(crate) kind: GraphletKind,
    /// Seed nodes the graphlet is derived from.
    pub(crate) anchors: Vec<NodeKey>,
    /// Optional traversal bound applied during derivation.
    pub(crate) scope: GraphletScope,
    /// Edge selectors that define the projection boundary.
    /// An empty selector list means "all edges" (unbounded edge projection).
    pub(crate) selectors: Vec<RelationSelector>,
    /// Optional ranking hint for `Frontier` derivation.
    pub(crate) ranking: Option<RankingPolicy>,
}

/// The result of deriving a graphlet from a `GraphletSpec`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedGraphlet {
    /// The spec this was derived from.
    pub(crate) spec: GraphletSpec,
    /// Nodes inside the graphlet boundary.
    pub(crate) members: Vec<NodeKey>,
    /// Edge keys of edges whose both endpoints are in `members`.
    pub(crate) edges: Vec<crate::graph::EdgeKey>,
    /// Nodes adjacent to `members` but not themselves members (the expansion
    /// boundary). Non-empty only for `Frontier` kind.
    pub(crate) frontier: Vec<NodeKey>,
}

// ---------------------------------------------------------------------------
// Derivation entry point
// ---------------------------------------------------------------------------

/// Derive a `ResolvedGraphlet` from the graph and a spec.
///
/// This is the canonical derivation path. All graphlet computations that need
/// to be inspectable or cacheable should go through this function.
pub(crate) fn derive_graphlet(graph: &Graph, spec: GraphletSpec) -> ResolvedGraphlet {
    let members = derive_members(graph, &spec);
    let edges = collect_internal_edges(graph, &members, &spec.selectors);
    let frontier = if matches!(spec.kind, GraphletKind::Frontier) {
        collect_frontier(graph, &members, &spec.selectors)
    } else {
        Vec::new()
    };
    ResolvedGraphlet { spec, members, edges, frontier }
}

// ---------------------------------------------------------------------------
// Kind-dispatch member derivation
// ---------------------------------------------------------------------------

fn derive_members(graph: &Graph, spec: &GraphletSpec) -> Vec<NodeKey> {
    // Filter anchors to only those that exist in the graph.
    let valid_anchors: Vec<NodeKey> = spec
        .anchors
        .iter()
        .copied()
        .filter(|&a| graph.get_node(a).is_some())
        .collect();

    if valid_anchors.is_empty() {
        return Vec::new();
    }

    let raw = match spec.kind {
        GraphletKind::Ego { radius } => derive_ego(graph, &valid_anchors, &spec.selectors, radius),
        GraphletKind::Corridor => derive_corridor(graph, &valid_anchors, &spec.selectors),
        GraphletKind::Component | GraphletKind::Facet => {
            derive_component(graph, &valid_anchors, &spec.selectors)
        }
        GraphletKind::Loop => derive_loop(graph, &valid_anchors),
        GraphletKind::Frontier => {
            // Frontier is a one-hop ego; frontier boundary is computed separately.
            derive_ego(graph, &valid_anchors, &spec.selectors, 1)
        }
        GraphletKind::Session => derive_session(graph, &valid_anchors),
        GraphletKind::Bridge => derive_bridge(graph, &valid_anchors, &spec.selectors),
        GraphletKind::WorkbenchCorrespondence => valid_anchors.clone(),
    };

    apply_scope(graph, raw, spec)
}

// ---------------------------------------------------------------------------
// Per-kind derivation helpers
// ---------------------------------------------------------------------------

/// Hop-bounded BFS from all anchors.
fn derive_ego(
    graph: &Graph,
    anchors: &[NodeKey],
    selectors: &[RelationSelector],
    radius: u8,
) -> Vec<NodeKey> {
    let max_hops = radius as usize;
    let mut visited: HashSet<NodeKey> = anchors.iter().copied().collect();
    let mut current_frontier: Vec<NodeKey> = anchors.to_vec();
    let mut members: Vec<NodeKey> = anchors.to_vec();

    for _ in 0..max_hops {
        let mut next_frontier = Vec::new();
        for &node in &current_frontier {
            for neighbor in selector_neighbors(graph, node, selectors) {
                if visited.insert(neighbor) {
                    members.push(neighbor);
                    next_frontier.push(neighbor);
                }
            }
        }
        if next_frontier.is_empty() {
            break;
        }
        current_frontier = next_frontier;
    }

    members
}

/// Shortest undirected path between anchors[0] and anchors[1].
/// Returns the path nodes, or just anchors if no path exists.
fn derive_corridor(graph: &Graph, anchors: &[NodeKey], selectors: &[RelationSelector]) -> Vec<NodeKey> {
    if anchors.len() < 2 {
        return anchors.to_vec();
    }
    let from = anchors[0];
    let to = anchors[1];

    // Use selector-aware BFS to find the shortest path under the projection.
    let path = selector_shortest_path(graph, from, to, selectors);
    if let Some(p) = path {
        p
    } else {
        // Unreachable under selectors — return the two anchors as the degenerate corridor.
        vec![from, to]
    }
}

/// Full weakly-connected component from anchors under the selector projection.
fn derive_component(
    graph: &Graph,
    anchors: &[NodeKey],
    selectors: &[RelationSelector],
) -> Vec<NodeKey> {
    if selectors.is_empty() {
        graphlet_members_for_seeds_with_filter(graph, anchors, |_| true)
    } else {
        graphlet_members_for_seeds_with_selectors(graph, anchors, selectors)
    }
}

/// Strongly-connected component containing any anchor (directed graph).
fn derive_loop(graph: &Graph, anchors: &[NodeKey]) -> Vec<NodeKey> {
    let anchor_set: HashSet<NodeKey> = anchors.iter().copied().collect();
    let sccs = graph.strongly_connected_components();
    let mut members = Vec::new();
    for scc in sccs {
        if scc.iter().any(|n| anchor_set.contains(n)) {
            for node in scc {
                if !members.contains(&node) {
                    members.push(node);
                }
            }
        }
    }
    if members.is_empty() {
        anchors.to_vec()
    } else {
        members
    }
}

/// Session graphlet — traversal-family edge projection from anchors.
fn derive_session(graph: &Graph, anchors: &[NodeKey]) -> Vec<NodeKey> {
    let session_selectors = [RelationSelector::Family(EdgeFamily::Traversal)];
    graphlet_members_for_seeds_with_selectors(graph, anchors, &session_selectors)
}

/// Bridge — nodes on any shortest path between anchor set A and anchor set B.
/// With 2+ anchors, treats [0] as source set and [1..] as target set.
fn derive_bridge(
    graph: &Graph,
    anchors: &[NodeKey],
    selectors: &[RelationSelector],
) -> Vec<NodeKey> {
    if anchors.len() < 2 {
        return anchors.to_vec();
    }
    let source = anchors[0];
    let targets = &anchors[1..];
    let mut on_any_path: HashSet<NodeKey> = HashSet::new();

    for &target in targets {
        if let Some(path) = selector_shortest_path(graph, source, target, selectors) {
            for node in path {
                on_any_path.insert(node);
            }
        }
    }

    if on_any_path.is_empty() {
        anchors.to_vec()
    } else {
        on_any_path.into_iter().collect()
    }
}

// ---------------------------------------------------------------------------
// Scope application
// ---------------------------------------------------------------------------

fn apply_scope(graph: &Graph, members: Vec<NodeKey>, spec: &GraphletSpec) -> Vec<NodeKey> {
    match &spec.scope {
        GraphletScope::Unbounded => members,
        GraphletScope::MaxHops(max) => {
            if spec.anchors.is_empty() {
                return members;
            }
            // Compute hop distances from all valid anchors and keep nodes within range.
            let mut min_hop: std::collections::HashMap<NodeKey, usize> =
                std::collections::HashMap::new();
            for &anchor in &spec.anchors {
                if graph.get_node(anchor).is_none() {
                    continue;
                }
                for (node, dist) in graph.hop_distances_from(anchor) {
                    let entry = min_hop.entry(node).or_insert(usize::MAX);
                    if dist < *entry {
                        *entry = dist;
                    }
                }
            }
            members
                .into_iter()
                .filter(|n| min_hop.get(n).copied().unwrap_or(usize::MAX) <= *max)
                .collect()
        }
        GraphletScope::NodeSet(allowed) => {
            let allowed_set: HashSet<NodeKey> = allowed.iter().copied().collect();
            members.into_iter().filter(|n| allowed_set.contains(n)).collect()
        }
    }
}

// ---------------------------------------------------------------------------
// Edge collection and frontier
// ---------------------------------------------------------------------------

/// Collect edges whose both endpoints are in `members` and match `selectors`.
fn collect_internal_edges(
    graph: &Graph,
    members: &[NodeKey],
    selectors: &[RelationSelector],
) -> Vec<crate::graph::EdgeKey> {
    let member_set: HashSet<NodeKey> = members.iter().copied().collect();
    let mut edges = Vec::new();
    for &node in members {
        for edge_ref in graph.inner.edges_directed(node, Direction::Outgoing) {
            if !member_set.contains(&edge_ref.target()) {
                continue;
            }
            if selectors.is_empty() || payload_matches_any_selector(edge_ref.weight(), selectors) {
                edges.push(edge_ref.id());
            }
        }
    }
    edges
}

/// Nodes adjacent to `members` (under `selectors`) that are not themselves members.
fn collect_frontier(
    graph: &Graph,
    members: &[NodeKey],
    selectors: &[RelationSelector],
) -> Vec<NodeKey> {
    let member_set: HashSet<NodeKey> = members.iter().copied().collect();
    let mut frontier: HashSet<NodeKey> = HashSet::new();
    for &node in members {
        for neighbor in selector_neighbors(graph, node, selectors) {
            if !member_set.contains(&neighbor) {
                frontier.insert(neighbor);
            }
        }
    }
    frontier.into_iter().collect()
}

// ---------------------------------------------------------------------------
// Selector-aware traversal helpers
// ---------------------------------------------------------------------------

/// One-hop undirected neighbors of `node` that pass `selectors`.
/// An empty selector list admits all edges.
fn selector_neighbors<'a>(
    graph: &'a Graph,
    node: NodeKey,
    selectors: &'a [RelationSelector],
) -> impl Iterator<Item = NodeKey> + 'a {
    let outgoing = graph
        .inner
        .edges_directed(node, Direction::Outgoing)
        .filter(|e| selectors.is_empty() || payload_matches_any_selector(e.weight(), selectors))
        .map(|e| e.target());
    let incoming = graph
        .inner
        .edges_directed(node, Direction::Incoming)
        .filter(|e| selectors.is_empty() || payload_matches_any_selector(e.weight(), selectors))
        .map(|e| e.source());
    outgoing.chain(incoming)
}

/// Selector-aware BFS shortest path between `from` and `to`.
/// Returns `None` if no path exists under the projection.
fn selector_shortest_path(
    graph: &Graph,
    from: NodeKey,
    to: NodeKey,
    selectors: &[RelationSelector],
) -> Option<Vec<NodeKey>> {
    if selectors.is_empty() {
        // Fall through to the graph's own A* which uses all edges.
        return graph.shortest_path(from, to);
    }
    // Selector-filtered BFS.
    use std::collections::VecDeque;
    let mut visited: HashSet<NodeKey> = HashSet::from([from]);
    let mut queue: VecDeque<(NodeKey, Vec<NodeKey>)> = VecDeque::new();
    queue.push_back((from, vec![from]));
    while let Some((current, path)) = queue.pop_front() {
        if current == to {
            return Some(path);
        }
        for neighbor in selector_neighbors(graph, current, selectors) {
            if visited.insert(neighbor) {
                let mut new_path = path.clone();
                new_path.push(neighbor);
                queue.push_back((neighbor, new_path));
            }
        }
    }
    None
}

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

    fn assert_relation(
        graph: &mut Graph,
        from: NodeKey,
        to: NodeKey,
        assertion: crate::graph::EdgeAssertion,
    ) {
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
            &[RelationSelector::Family(
                crate::graph::EdgeFamily::Traversal,
            )],
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
            &[RelationSelector::Family(
                crate::graph::EdgeFamily::Traversal,
            )],
        );
        members.sort_by_key(|key| key.index());
        assert_eq!(members, vec![a, b, c]);
    }

    // -----------------------------------------------------------------------
    // derive_graphlet tests
    // -----------------------------------------------------------------------

    fn user_grouped_spec(anchors: Vec<NodeKey>, kind: GraphletKind) -> GraphletSpec {
        GraphletSpec {
            kind,
            anchors,
            scope: GraphletScope::Unbounded,
            selectors: vec![RelationSelector::Semantic(SemanticSubKind::UserGrouped)],
            ranking: None,
        }
    }

    fn hyperlink_spec(anchors: Vec<NodeKey>, kind: GraphletKind) -> GraphletSpec {
        GraphletSpec {
            kind,
            anchors,
            scope: GraphletScope::Unbounded,
            selectors: vec![RelationSelector::Semantic(SemanticSubKind::Hyperlink)],
            ranking: None,
        }
    }

    #[test]
    fn derive_ego_radius_1_returns_direct_neighbors_only() {
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
        let spec = user_grouped_spec(vec![a], GraphletKind::Ego { radius: 1 });
        let resolved = derive_graphlet(&graph, spec);
        assert!(resolved.members.contains(&a));
        assert!(resolved.members.contains(&b));
        // c is two hops away — should not be included at radius 1
        assert!(!resolved.members.contains(&c));
    }

    #[test]
    fn derive_ego_radius_2_includes_two_hop_nodes() {
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
        let spec = user_grouped_spec(vec![a], GraphletKind::Ego { radius: 2 });
        let resolved = derive_graphlet(&graph, spec);
        assert!(resolved.members.contains(&a));
        assert!(resolved.members.contains(&b));
        assert!(resolved.members.contains(&c));
    }

    #[test]
    fn derive_corridor_finds_shortest_path() {
        let mut graph = Graph::new();
        let a = add_node(&mut graph, "https://a.test/");
        let b = add_node(&mut graph, "https://b.test/");
        let c = add_node(&mut graph, "https://c.test/");
        // a → b → c (user grouped)
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
        let spec = user_grouped_spec(vec![a, c], GraphletKind::Corridor);
        let resolved = derive_graphlet(&graph, spec);
        // corridor must contain all nodes on the a→b→c path
        assert!(resolved.members.contains(&a));
        assert!(resolved.members.contains(&b));
        assert!(resolved.members.contains(&c));
    }

    #[test]
    fn derive_corridor_with_unreachable_nodes_returns_anchors() {
        let mut graph = Graph::new();
        let a = add_node(&mut graph, "https://a.test/");
        let b = add_node(&mut graph, "https://b.test/");
        // no edges — b unreachable from a under user-grouped selectors
        let spec = user_grouped_spec(vec![a, b], GraphletKind::Corridor);
        let resolved = derive_graphlet(&graph, spec);
        assert!(resolved.members.contains(&a));
        assert!(resolved.members.contains(&b));
    }

    #[test]
    fn derive_component_includes_full_connected_region() {
        let mut graph = Graph::new();
        let a = add_node(&mut graph, "https://a.test/");
        let b = add_node(&mut graph, "https://b.test/");
        let c = add_node(&mut graph, "https://c.test/");
        let d = add_node(&mut graph, "https://d.test/"); // isolated
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
        let spec = user_grouped_spec(vec![a], GraphletKind::Component);
        let resolved = derive_graphlet(&graph, spec);
        assert!(resolved.members.contains(&a));
        assert!(resolved.members.contains(&b));
        assert!(resolved.members.contains(&c));
        assert!(!resolved.members.contains(&d));
    }

    #[test]
    fn derive_frontier_members_excludes_second_hop() {
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
        let spec = user_grouped_spec(vec![a], GraphletKind::Frontier);
        let resolved = derive_graphlet(&graph, spec);
        // frontier members = one-hop ego: a and b
        assert!(resolved.members.contains(&a));
        assert!(resolved.members.contains(&b));
        assert!(!resolved.members.contains(&c));
        // c is the expansion boundary
        assert!(resolved.frontier.contains(&c));
    }

    #[test]
    fn derive_workbench_correspondence_returns_anchors_as_members() {
        let mut graph = Graph::new();
        let a = add_node(&mut graph, "https://a.test/");
        let b = add_node(&mut graph, "https://b.test/");
        let spec = GraphletSpec {
            kind: GraphletKind::WorkbenchCorrespondence,
            anchors: vec![a, b],
            scope: GraphletScope::Unbounded,
            selectors: vec![],
            ranking: None,
        };
        let resolved = derive_graphlet(&graph, spec);
        assert!(resolved.members.contains(&a));
        assert!(resolved.members.contains(&b));
        assert_eq!(resolved.members.len(), 2);
    }

    #[test]
    fn derive_graphlet_internal_edges_are_collected() {
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
        let spec = user_grouped_spec(vec![a], GraphletKind::Component);
        let resolved = derive_graphlet(&graph, spec);
        assert_eq!(resolved.edges.len(), 1);
    }

    #[test]
    fn derive_graphlet_scope_max_hops_bounds_ego() {
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
        // ego with radius 2 normally reaches c, but MaxHops(1) on scope restricts it
        let spec = GraphletSpec {
            kind: GraphletKind::Ego { radius: 2 },
            anchors: vec![a],
            scope: GraphletScope::MaxHops(1),
            selectors: vec![RelationSelector::Semantic(SemanticSubKind::UserGrouped)],
            ranking: None,
        };
        let resolved = derive_graphlet(&graph, spec);
        assert!(resolved.members.contains(&a));
        assert!(resolved.members.contains(&b));
        assert!(!resolved.members.contains(&c));
    }

    #[test]
    fn derive_session_follows_traversal_edges() {
        let mut graph = Graph::new();
        let a = add_node(&mut graph, "https://a.test/");
        let b = add_node(&mut graph, "https://b.test/");
        let c = add_node(&mut graph, "https://c.test/");
        add_history_edge(&mut graph, a, b);
        add_history_edge(&mut graph, b, c);
        let spec = GraphletSpec {
            kind: GraphletKind::Session,
            anchors: vec![a],
            scope: GraphletScope::Unbounded,
            selectors: vec![],
            ranking: None,
        };
        let resolved = derive_graphlet(&graph, spec);
        assert!(resolved.members.contains(&a));
        assert!(resolved.members.contains(&b));
        assert!(resolved.members.contains(&c));
    }
}
