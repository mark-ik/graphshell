/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use euclid::default::Point2D;
use uuid::Uuid;

use super::{AddressKind, EdgeKey, EdgeType, Graph, NodeKey, Traversal};

#[derive(Debug, Clone)]
pub enum GraphDelta {
    AddNode {
        id: Option<Uuid>,
        url: String,
        position: Point2D<f32>,
    },
    AddEdge {
        from: NodeKey,
        to: NodeKey,
        edge_type: EdgeType,
    },
    RemoveNode {
        key: NodeKey,
    },
    ReplayAddNodeWithIdIfMissing {
        id: Uuid,
        url: String,
        position: Point2D<f32>,
    },
    ReplayAddEdgeByIds {
        from_id: Uuid,
        to_id: Uuid,
        edge_type: EdgeType,
    },
    ReplayRemoveNodeById {
        node_id: Uuid,
    },
    ReplayRemoveEdgesByIds {
        from_id: Uuid,
        to_id: Uuid,
        edge_type: EdgeType,
    },
    RemoveEdges {
        from: NodeKey,
        to: NodeKey,
        edge_type: EdgeType,
    },
    AppendTraversal {
        from: NodeKey,
        to: NodeKey,
        traversal: Traversal,
    },
    SetNodeTitle {
        key: NodeKey,
        title: String,
    },
    SetNodeUrl {
        key: NodeKey,
        new_url: String,
    },
    SetNodeThumbnail {
        key: NodeKey,
        png_bytes: Vec<u8>,
        width: u32,
        height: u32,
    },
    SetNodeFavicon {
        key: NodeKey,
        rgba: Vec<u8>,
        width: u32,
        height: u32,
    },
    SetNodeMimeHint {
        key: NodeKey,
        mime_hint: Option<String>,
    },
    SetNodeAddressKind {
        key: NodeKey,
        kind: AddressKind,
    },
    SetNodePinned {
        key: NodeKey,
        is_pinned: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphDeltaResult {
    NodeAdded(NodeKey),
    NodeMaybeAdded(Option<NodeKey>),
    EdgeAdded(Option<EdgeKey>),
    NodeRemoved(bool),
    EdgesRemoved(usize),
    TraversalAppended(bool),
    NodeMetadataUpdated(bool),
    NodeUrlUpdated(Option<String>),
}

pub fn apply_graph_delta(graph: &mut Graph, delta: GraphDelta) -> GraphDeltaResult {
    match delta {
        GraphDelta::AddNode { id, url, position } => {
            let key = if let Some(id) = id {
                graph.add_node_with_id(id, url, position)
            } else {
                graph.add_node(url, position)
            };
            GraphDeltaResult::NodeAdded(key)
        }
        GraphDelta::AddEdge {
            from,
            to,
            edge_type,
        } => GraphDeltaResult::EdgeAdded(graph.add_edge(from, to, edge_type)),
        GraphDelta::RemoveNode { key } => GraphDeltaResult::NodeRemoved(graph.remove_node(key)),
        GraphDelta::ReplayAddNodeWithIdIfMissing { id, url, position } => {
            GraphDeltaResult::NodeMaybeAdded(
                graph.replay_add_node_with_id_if_missing(id, url, position),
            )
        }
        GraphDelta::ReplayAddEdgeByIds {
            from_id,
            to_id,
            edge_type,
        } => GraphDeltaResult::EdgeAdded(graph.replay_add_edge_by_ids(from_id, to_id, edge_type)),
        GraphDelta::ReplayRemoveNodeById { node_id } => {
            GraphDeltaResult::NodeRemoved(graph.replay_remove_node_by_id(node_id))
        }
        GraphDelta::ReplayRemoveEdgesByIds {
            from_id,
            to_id,
            edge_type,
        } => GraphDeltaResult::EdgesRemoved(
            graph.replay_remove_edges_by_ids(from_id, to_id, edge_type),
        ),
        GraphDelta::RemoveEdges {
            from,
            to,
            edge_type,
        } => GraphDeltaResult::EdgesRemoved(graph.remove_edges(from, to, edge_type)),
        GraphDelta::AppendTraversal {
            from,
            to,
            traversal,
        } => GraphDeltaResult::TraversalAppended(graph.push_traversal(from, to, traversal)),
        GraphDelta::SetNodeTitle { key, title } => {
            GraphDeltaResult::NodeMetadataUpdated(graph.set_node_title(key, title))
        }
        GraphDelta::SetNodeUrl { key, new_url } => {
            GraphDeltaResult::NodeUrlUpdated(graph.update_node_url(key, new_url))
        }
        GraphDelta::SetNodeThumbnail {
            key,
            png_bytes,
            width,
            height,
        } => GraphDeltaResult::NodeMetadataUpdated(
            graph.set_node_thumbnail(key, png_bytes, width, height),
        ),
        GraphDelta::SetNodeFavicon {
            key,
            rgba,
            width,
            height,
        } => {
            GraphDeltaResult::NodeMetadataUpdated(graph.set_node_favicon(key, rgba, width, height))
        }
        GraphDelta::SetNodeMimeHint { key, mime_hint } => {
            GraphDeltaResult::NodeMetadataUpdated(graph.set_node_mime_hint(key, mime_hint))
        }
        GraphDelta::SetNodeAddressKind { key, kind } => {
            GraphDeltaResult::NodeMetadataUpdated(graph.set_node_address_kind(key, kind))
        }
        GraphDelta::SetNodePinned { key, is_pinned } => {
            GraphDeltaResult::NodeMetadataUpdated(graph.set_node_pinned(key, is_pinned))
        }
    }
}
