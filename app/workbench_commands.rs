use super::*;
use egui_tiles::{Tile, TileId, Tree};
use euclid::default::Point2D;
use uuid::Uuid;

use crate::graph::{ArrangementSubKind, EdgeType, NodeKey};
#[cfg(feature = "diagnostics")]
use crate::shell::desktop::workbench::pane_model::ToolPaneState;
use crate::shell::desktop::workbench::tile_kind::TileKind;
use crate::util::VersoAddress;

impl GraphBrowserApp {
    pub fn enqueue_workbench_intent(&mut self, intent: WorkbenchIntent) {
        self.workspace.pending_workbench_intents.push(intent);
    }

    pub fn extend_workbench_intents<I>(&mut self, intents: I)
    where
        I: IntoIterator<Item = WorkbenchIntent>,
    {
        self.workspace.pending_workbench_intents.extend(intents);
    }

    pub fn take_pending_workbench_intents(&mut self) -> Vec<WorkbenchIntent> {
        std::mem::take(&mut self.workspace.pending_workbench_intents)
    }

    #[cfg(test)]
    pub fn pending_workbench_intent_count_for_tests(&self) -> usize {
        self.workspace.pending_workbench_intents.len()
    }

    pub fn workbench_tile_selection(&self) -> &WorkbenchTileSelectionState {
        &self.workbench_tile_selection
    }

    pub fn clear_workbench_tile_selection(&mut self) {
        self.workbench_tile_selection.selected_tile_ids.clear();
        self.workbench_tile_selection.primary_tile_id = None;
    }

    pub fn select_workbench_tile(&mut self, tile_id: TileId) {
        self.update_workbench_tile_selection(tile_id, SelectionUpdateMode::Replace);
    }

    pub fn update_workbench_tile_selection(&mut self, tile_id: TileId, mode: SelectionUpdateMode) {
        match mode {
            SelectionUpdateMode::Replace => {
                self.workbench_tile_selection.selected_tile_ids.clear();
                self.workbench_tile_selection
                    .selected_tile_ids
                    .insert(tile_id);
                self.workbench_tile_selection.primary_tile_id = Some(tile_id);
            }
            SelectionUpdateMode::Add => {
                self.workbench_tile_selection
                    .selected_tile_ids
                    .insert(tile_id);
                self.workbench_tile_selection.primary_tile_id = Some(tile_id);
            }
            SelectionUpdateMode::Toggle => {
                if self
                    .workbench_tile_selection
                    .selected_tile_ids
                    .remove(&tile_id)
                {
                    if self.workbench_tile_selection.primary_tile_id == Some(tile_id) {
                        self.workbench_tile_selection.primary_tile_id = self
                            .workbench_tile_selection
                            .selected_tile_ids
                            .iter()
                            .copied()
                            .next();
                    }
                } else {
                    self.workbench_tile_selection
                        .selected_tile_ids
                        .insert(tile_id);
                    self.workbench_tile_selection.primary_tile_id = Some(tile_id);
                }
            }
        }
    }

    pub fn prune_workbench_tile_selection(&mut self, tiles_tree: &Tree<TileKind>) {
        self.workbench_tile_selection
            .selected_tile_ids
            .retain(|tile_id| matches!(tiles_tree.tiles.get(*tile_id), Some(Tile::Pane(_))));
        if self
            .workbench_tile_selection
            .primary_tile_id
            .is_some_and(|tile_id| {
                !self
                    .workbench_tile_selection
                    .selected_tile_ids
                    .contains(&tile_id)
            })
        {
            self.workbench_tile_selection.primary_tile_id = self
                .workbench_tile_selection
                .selected_tile_ids
                .iter()
                .copied()
                .next();
        }
    }

    pub(crate) fn persist_workbench_tile_group(
        &mut self,
        tiles_tree: &Tree<TileKind>,
        selected_tile_ids: &std::collections::HashSet<TileId>,
    ) -> Option<NodeKey> {
        let tile_kinds: Vec<TileKind> = selected_tile_ids
            .iter()
            .filter_map(|tile_id| match tiles_tree.tiles.get(*tile_id) {
                Some(Tile::Pane(kind)) => Some(kind.clone()),
                _ => None,
            })
            .collect();
        let member_node_keys = self.resolve_workbench_tile_member_nodes(tile_kinds);
        if member_node_keys.is_empty() {
            return None;
        }

        let group_key = self.ensure_internal_surface_node(
            VersoAddress::tile_group(Uuid::new_v4().to_string()).to_string(),
            "Tile Group".to_string(),
            self.workbench_group_position(&member_node_keys),
        );
        self.replace_internal_surface_membership_edges(
            group_key,
            &member_node_keys,
            ArrangementSubKind::TileGroup,
        );
        Some(group_key)
    }

    pub(crate) fn sync_named_workbench_frame_graph_representation(
        &mut self,
        name: &str,
        tiles_tree: &Tree<TileKind>,
    ) -> NodeKey {
        let tile_kinds: Vec<TileKind> = tiles_tree
            .tiles
            .iter()
            .filter_map(|(_, tile)| match tile {
                Tile::Pane(kind) => Some(kind.clone()),
                _ => None,
            })
            .collect();
        let member_node_keys = self.resolve_workbench_tile_member_nodes(tile_kinds);
        let frame_key = self.ensure_internal_surface_node(
            VersoAddress::frame(name.to_string()).to_string(),
            name.to_string(),
            self.workbench_group_position(&member_node_keys),
        );
        self.replace_internal_surface_membership_edges(
            frame_key,
            &member_node_keys,
            ArrangementSubKind::FrameMember,
        );
        frame_key
    }

    pub(crate) fn remove_named_workbench_frame_graph_representation(&mut self, name: &str) {
        let frame_url = VersoAddress::frame(name.to_string()).to_string();
        let Some((frame_key, _)) = self.domain_graph().get_node_by_url(&frame_url) else {
            return;
        };
        self.remove_internal_surface_node(frame_key);
    }

    fn resolve_workbench_tile_member_nodes(&mut self, tile_kinds: Vec<TileKind>) -> Vec<NodeKey> {
        let mut member_node_keys = Vec::new();
        for tile_kind in tile_kinds {
            let Some(member_key) = self.ensure_workbench_tile_graph_identity(&tile_kind) else {
                continue;
            };
            if !member_node_keys.contains(&member_key) {
                member_node_keys.push(member_key);
            }
        }
        member_node_keys
    }

    fn replace_internal_surface_membership_edges(
        &mut self,
        container_key: NodeKey,
        member_node_keys: &[NodeKey],
        sub_kind: ArrangementSubKind,
    ) {
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
        for (member_key, edge_type) in existing_members {
            let _ = self.remove_edges_and_log(container_key, member_key, edge_type);
        }
        for member_key in member_node_keys {
            self.add_arrangement_relation_if_missing(container_key, *member_key, sub_kind);
        }
    }

    fn ensure_workbench_tile_graph_identity(&mut self, tile_kind: &TileKind) -> Option<NodeKey> {
        match tile_kind {
            TileKind::Node(state) => Some(state.node),
            TileKind::Graph(graph_ref) => Some(self.ensure_internal_surface_node(
                VersoAddress::view(graph_ref.graph_view_id.as_uuid().to_string()).to_string(),
                "Graph View".to_string(),
                self.suggested_new_node_position(None),
            )),
            #[cfg(feature = "diagnostics")]
            TileKind::Tool(tool_ref) => Some(
                self.ensure_internal_surface_node(
                    VersoAddress::Other {
                        category: "tool-pane".to_string(),
                        segments: vec![
                            tool_pane_route_segment(&tool_ref.kind).to_string(),
                            tool_ref.pane_id.to_string(),
                        ],
                    }
                    .to_string(),
                    tool_ref.title().to_string(),
                    self.suggested_new_node_position(None),
                ),
            ),
        }
    }

    fn workbench_group_position(&self, member_node_keys: &[NodeKey]) -> Point2D<f32> {
        let mut position_count = 0usize;
        let mut sum_x = 0.0f32;
        let mut sum_y = 0.0f32;
        for member_key in member_node_keys {
            if let Some(position) = self.domain_graph().node_projected_position(*member_key) {
                position_count += 1;
                sum_x += position.x;
                sum_y += position.y;
            }
        }
        if position_count > 0 {
            return Point2D::new(sum_x / position_count as f32, sum_y / position_count as f32);
        }
        self.suggested_new_node_position(None)
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

    fn remove_internal_surface_node(&mut self, key: NodeKey) {
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
        self.workspace.runtime_block_state.remove(&key);
        self.workspace.suggested_semantic_tags.remove(&key);
        if let Some(node_id) = node_id {
            self.workspace.node_last_active_workspace.remove(&node_id);
            self.workspace.node_workspace_membership.remove(&node_id);
        }
        if let Some(store) = &mut self.services.persistence {
            let _ = store.dissolve_and_remove_node(&mut self.workspace.domain.graph, key);
        } else {
            let _ = self.apply_graph_delta_and_sync(GraphDelta::RemoveNode { key });
        }
    }
}

#[cfg(feature = "diagnostics")]
fn tool_pane_route_segment(kind: &ToolPaneState) -> &'static str {
    match kind {
        ToolPaneState::Diagnostics => "diagnostics",
        ToolPaneState::HistoryManager => "history",
        ToolPaneState::AccessibilityInspector => "accessibility",
        ToolPaneState::FileTree => "file-tree",
        ToolPaneState::Settings => "settings",
    }
}
