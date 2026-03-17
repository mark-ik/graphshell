/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Arrangement→graph reconciler: the single authorised path from workbench
//! arrangement state into graph structure mutations.
//!
//! # Boundary contract
//!
//! Callers must build an [`ArrangementSnapshot`] from their tile-tree state
//! and call [`GraphBrowserApp::apply_arrangement_snapshot`].  The snapshot
//! carries plain data — no live tree borrows — so the boundary is explicit.
//! The return value [`ArrangementGraphDelta`] names exactly what changed in
//! the graph, giving callers visibility without requiring them to diff the
//! graph themselves.
//!
//! Helper methods below (`ensure_internal_surface_node`,
//! `replace_internal_surface_membership_edges`, etc.) are **private** to this
//! module.  Callers outside `arrangement_graph_bridge` must not reach into
//! graph structure via those helpers directly.

use euclid::default::Point2D;
use uuid::Uuid;

use crate::graph::apply::{GraphDelta, GraphDeltaResult};
use crate::graph::{ArrangementSubKind, EdgeType, NodeKey};
use crate::shell::desktop::workbench::tile_kind::TileKind;
use crate::util::VersoAddress;

use super::*;

// ── Public data types ────────────────────────────────────────────────────────

/// Plain-data snapshot of a workbench arrangement that needs to be reflected
/// into graph truth.
///
/// Callers extract this from a live tile tree before calling
/// [`GraphBrowserApp::apply_arrangement_snapshot`].  Keeping it as plain data
/// means the reconciler takes no borrow of the tile tree and the call site is
/// unambiguous about what information crosses the boundary.
#[derive(Debug, Clone)]
pub enum ArrangementSnapshot {
    /// A named frame whose membership should be synced to the graph.
    Frame {
        /// Canonical frame name (used to derive the `VersoAddress::frame` URL).
        name: String,
        /// Pane tile-kinds extracted from the live tree (all `Tile::Pane` leaves).
        pane_tile_kinds: Vec<TileKind>,
    },
    /// A tile group formed from a selection of pane tile-kinds.
    TileGroup {
        /// Pane tile-kinds in the group (only `Tile::Pane` leaves from the selection).
        pane_tile_kinds: Vec<TileKind>,
    },
    /// Remove a named frame's graph representation entirely.
    RemoveFrame {
        /// Frame name whose graph node should be removed.
        name: String,
    },
}

/// Description of graph mutations produced by
/// [`GraphBrowserApp::apply_arrangement_snapshot`].
#[derive(Debug, Clone)]
pub struct ArrangementGraphDelta {
    /// The frame or group node that was created or updated.
    /// `None` for `RemoveFrame` snapshots or when no eligible members exist.
    pub container_node: Option<NodeKey>,
    /// Nodes that were added as members (edges created).
    pub members_added: Vec<NodeKey>,
    /// Nodes that were removed as members (edges removed).
    pub members_removed: Vec<NodeKey>,
}

// ── Public entrypoint ────────────────────────────────────────────────────────

impl GraphBrowserApp {
    /// Apply an arrangement snapshot to graph truth.
    ///
    /// This is the **single authorised** path from workbench arrangement state
    /// into graph structure mutations.  All callers that previously called
    /// `sync_named_workbench_frame_graph_representation`,
    /// `remove_named_workbench_frame_graph_representation`, or
    /// `persist_workbench_tile_group` directly must go through this entrypoint
    /// instead.
    pub(crate) fn apply_arrangement_snapshot(
        &mut self,
        snapshot: &ArrangementSnapshot,
    ) -> ArrangementGraphDelta {
        match snapshot {
            ArrangementSnapshot::Frame { name, pane_tile_kinds } => {
                self.reconcile_frame_snapshot(name, pane_tile_kinds)
            }
            ArrangementSnapshot::TileGroup { pane_tile_kinds } => {
                self.reconcile_tile_group_snapshot(pane_tile_kinds)
            }
            ArrangementSnapshot::RemoveFrame { name } => {
                self.reconcile_remove_frame(name)
            }
        }
    }
}

// ── Private reconciler implementations ───────────────────────────────────────

impl GraphBrowserApp {
    fn reconcile_frame_snapshot(
        &mut self,
        name: &str,
        pane_tile_kinds: &[TileKind],
    ) -> ArrangementGraphDelta {
        let member_node_keys =
            self.resolve_arrangement_tile_member_nodes(pane_tile_kinds.to_vec());

        // Capture existing members before mutation for delta reporting.
        let frame_url = VersoAddress::frame(name.to_string()).to_string();
        let existing_members: Vec<NodeKey> = if let Some((frame_key, _)) =
            self.domain_graph().get_node_by_url(&frame_url)
        {
            self.domain_graph()
                .edges()
                .filter(|e| {
                    e.from == frame_key
                        && matches!(
                            e.edge_type,
                            EdgeType::UserGrouped | EdgeType::ArrangementRelation(_)
                        )
                })
                .map(|e| e.to)
                .collect()
        } else {
            Vec::new()
        };

        let frame_key = self.ensure_internal_surface_node(
            frame_url,
            name.to_string(),
            self.arrangement_centroid_position(&member_node_keys),
        );
        self.replace_internal_surface_membership_edges(
            frame_key,
            &member_node_keys,
            ArrangementSubKind::FrameMember,
        );

        self.emit_ux_navigation_transition();
        self.emit_arrangement_projection_health();
        crate::shell::desktop::runtime::registries::phase3_publish_workbench_projection_refresh_requested(
            "frame_membership_changed",
        );

        let desired_set: std::collections::HashSet<NodeKey> =
            member_node_keys.iter().copied().collect();
        let existing_set: std::collections::HashSet<NodeKey> =
            existing_members.iter().copied().collect();
        ArrangementGraphDelta {
            container_node: Some(frame_key),
            members_added: desired_set.difference(&existing_set).copied().collect(),
            members_removed: existing_set.difference(&desired_set).copied().collect(),
        }
    }

    fn reconcile_tile_group_snapshot(
        &mut self,
        pane_tile_kinds: &[TileKind],
    ) -> ArrangementGraphDelta {
        let member_node_keys =
            self.resolve_arrangement_tile_member_nodes(pane_tile_kinds.to_vec());
        if member_node_keys.is_empty() {
            return ArrangementGraphDelta {
                container_node: None,
                members_added: Vec::new(),
                members_removed: Vec::new(),
            };
        }

        let group_key = self.ensure_internal_surface_node(
            VersoAddress::tile_group(Uuid::new_v4().to_string()).to_string(),
            "Tile Group".to_string(),
            self.arrangement_centroid_position(&member_node_keys),
        );
        self.replace_internal_surface_membership_edges(
            group_key,
            &member_node_keys,
            ArrangementSubKind::TileGroup,
        );

        self.emit_ux_navigation_transition();
        self.emit_arrangement_projection_health();
        crate::shell::desktop::runtime::registries::phase3_publish_workbench_projection_refresh_requested(
            "tile_group_membership_changed",
        );

        ArrangementGraphDelta {
            container_node: Some(group_key),
            members_added: member_node_keys.clone(),
            members_removed: Vec::new(),
        }
    }

    fn reconcile_remove_frame(&mut self, name: &str) -> ArrangementGraphDelta {
        let frame_url = VersoAddress::frame(name.to_string()).to_string();
        let Some((frame_key, _)) = self.domain_graph().get_node_by_url(&frame_url) else {
            return ArrangementGraphDelta {
                container_node: None,
                members_added: Vec::new(),
                members_removed: Vec::new(),
            };
        };
        // Capture members before removal for delta reporting.
        let removed_members: Vec<NodeKey> = self
            .domain_graph()
            .edges()
            .filter(|e| {
                e.from == frame_key
                    && matches!(
                        e.edge_type,
                        EdgeType::UserGrouped | EdgeType::ArrangementRelation(_)
                    )
            })
            .map(|e| e.to)
            .collect();

        self.remove_internal_surface_node(frame_key);

        ArrangementGraphDelta {
            container_node: Some(frame_key),
            members_added: Vec::new(),
            members_removed: removed_members,
        }
    }
}

// ── Private helpers (not callable outside this module) ────────────────────────

impl GraphBrowserApp {
    fn resolve_arrangement_tile_member_nodes(
        &mut self,
        tile_kinds: Vec<TileKind>,
    ) -> Vec<NodeKey> {
        let mut member_node_keys = Vec::new();
        for tile_kind in tile_kinds {
            let Some(member_key) = self.arrangement_tile_graph_identity(&tile_kind) else {
                continue;
            };
            if !member_node_keys.contains(&member_key) {
                member_node_keys.push(member_key);
            }
        }
        member_node_keys
    }

    fn arrangement_tile_graph_identity(&mut self, tile_kind: &TileKind) -> Option<NodeKey> {
        match tile_kind {
            TileKind::Pane(
                crate::shell::desktop::workbench::pane_model::PaneViewState::Node(state),
            ) => Some(state.node),
            TileKind::Pane(
                crate::shell::desktop::workbench::pane_model::PaneViewState::Graph(graph_ref),
            ) => Some(self.ensure_internal_surface_node(
                VersoAddress::view(graph_ref.graph_view_id.as_uuid().to_string()).to_string(),
                "Graph View".to_string(),
                self.suggested_new_node_position(None),
            )),
            #[cfg(feature = "diagnostics")]
            TileKind::Pane(
                crate::shell::desktop::workbench::pane_model::PaneViewState::Tool(tool_ref),
            ) => Some(self.ensure_internal_surface_node(
                VersoAddress::Other {
                    category: "tool-pane".to_string(),
                    segments: vec![
                        arrangement_tool_pane_segment(&tool_ref.kind).to_string(),
                        tool_ref.pane_id.to_string(),
                    ],
                }
                .to_string(),
                tool_ref.title().to_string(),
                self.suggested_new_node_position(None),
            )),
            TileKind::Node(state) => Some(state.node),
            TileKind::Graph(graph_ref) => Some(self.ensure_internal_surface_node(
                VersoAddress::view(graph_ref.graph_view_id.as_uuid().to_string()).to_string(),
                "Graph View".to_string(),
                self.suggested_new_node_position(None),
            )),
            #[cfg(feature = "diagnostics")]
            TileKind::Tool(tool_ref) => Some(self.ensure_internal_surface_node(
                VersoAddress::Other {
                    category: "tool-pane".to_string(),
                    segments: vec![
                        arrangement_tool_pane_segment(&tool_ref.kind).to_string(),
                        tool_ref.pane_id.to_string(),
                    ],
                }
                .to_string(),
                tool_ref.title().to_string(),
                self.suggested_new_node_position(None),
            )),
        }
    }

    fn arrangement_centroid_position(&self, member_node_keys: &[NodeKey]) -> Point2D<f32> {
        let mut count = 0usize;
        let mut sum_x = 0.0f32;
        let mut sum_y = 0.0f32;
        for key in member_node_keys {
            if let Some(pos) = self.domain_graph().node_projected_position(*key) {
                count += 1;
                sum_x += pos.x;
                sum_y += pos.y;
            }
        }
        if count > 0 {
            return Point2D::new(sum_x / count as f32, sum_y / count as f32);
        }
        self.suggested_new_node_position(None)
    }

    fn replace_internal_surface_membership_edges(
        &mut self,
        container_key: NodeKey,
        member_node_keys: &[NodeKey],
        sub_kind: ArrangementSubKind,
    ) {
        let desired_members: std::collections::HashSet<NodeKey> =
            member_node_keys.iter().copied().collect();
        let existing_members: Vec<(NodeKey, EdgeType)> = self
            .domain_graph()
            .edges()
            .filter(|edge| {
                edge.from == container_key
                    && matches!(
                        edge.edge_type,
                        EdgeType::UserGrouped | EdgeType::ArrangementRelation(_)
                    )
            })
            .map(|edge| (edge.to, edge.edge_type))
            .collect();

        if sub_kind == ArrangementSubKind::FrameMember {
            for (member_key, edge_type) in &existing_members {
                if !desired_members.contains(member_key) || *edge_type == EdgeType::UserGrouped {
                    let _ = self.remove_edges_and_log(container_key, *member_key, *edge_type);
                }
            }
            for member_key in member_node_keys {
                self.promote_arrangement_relation_to_frame_membership(container_key, *member_key);
            }
            return;
        }

        for (member_key, edge_type) in existing_members {
            let _ = self.remove_edges_and_log(container_key, member_key, edge_type);
        }
        for member_key in member_node_keys {
            self.add_arrangement_relation_if_missing(container_key, *member_key, sub_kind);
        }
    }

    fn ensure_internal_surface_node(
        &mut self,
        url: String,
        title: String,
        position: Point2D<f32>,
    ) -> NodeKey {
        if let Some((key, _)) = self.domain_graph().get_node_by_url(&url) {
            self.set_node_title_and_log_if_changed(key, title);
            return key;
        }
        let key = self.add_node_and_sync(url, position);
        self.set_node_title_and_log_if_changed(key, title);
        key
    }

    fn set_node_title_and_log_if_changed(&mut self, key: NodeKey, title: String) {
        let GraphDeltaResult::NodeMetadataUpdated(changed) =
            self.apply_graph_delta_and_sync(GraphDelta::SetNodeTitle { key, title })
        else {
            unreachable!("title delta must return NodeMetadataUpdated");
        };
        if changed {
            self.log_title_mutation(key);
        }
    }

    pub(crate) fn remove_internal_surface_node(&mut self, key: NodeKey) {
        let node_id = self
            .workspace
            .domain
            .graph
            .get_node(key)
            .map(|node| node.id);
        if let Some(store) = &mut self.services.persistence
            && let Some(node_id) = node_id
        {
            store.log_mutation(&crate::services::persistence::types::LogEntry::RemoveNode {
                node_id: node_id.to_string(),
            });
        }
        self.workspace
            .graph_runtime
            .runtime_block_state
            .remove(&key);
        self.workspace
            .graph_runtime
            .suggested_semantic_tags
            .remove(&key);
        if let Some(node_id) = node_id {
            self.workspace.workbench_session.on_node_deleted(node_id);
        }
        if let Some(store) = &mut self.services.persistence {
            let _ = store.dissolve_and_remove_node(&mut self.workspace.domain.graph, key);
        } else {
            let _ = self.apply_graph_delta_and_sync(GraphDelta::RemoveNode { key });
        }
    }
}

// ── Private free function ─────────────────────────────────────────────────────

#[cfg(feature = "diagnostics")]
fn arrangement_tool_pane_segment(
    kind: &crate::shell::desktop::workbench::pane_model::ToolPaneState,
) -> &'static str {
    match kind {
        crate::shell::desktop::workbench::pane_model::ToolPaneState::Diagnostics => "diagnostics",
        crate::shell::desktop::workbench::pane_model::ToolPaneState::HistoryManager => "history",
        crate::shell::desktop::workbench::pane_model::ToolPaneState::AccessibilityInspector => {
            "accessibility"
        }
        crate::shell::desktop::workbench::pane_model::ToolPaneState::FileTree => "file-tree",
        crate::shell::desktop::workbench::pane_model::ToolPaneState::Settings => "settings",
    }
}
