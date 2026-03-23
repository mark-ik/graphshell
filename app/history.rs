#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HistoryManagerTab {
    #[default]
    Timeline,
    Dissolved,
    /// Mixed multi-track timeline (all history tracks, filtered).
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryCaptureStatus {
    Full,
    DegradedCaptureOnly,
}

impl HistoryCaptureStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::DegradedCaptureOnly => "degraded-capture-only",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryTraversalFailureReason {
    MissingOldUrl,
    MissingNewUrl,
    SameUrl,
    NonHistoryTransition,
    MissingEndpoint,
    SelfLoop,
    GraphRejected,
    PersistenceUnavailable,
    ExportWriteFailed,
    ExportReadFailed,
    HomeDirectoryUnavailable,
    PreviewIsolationViolation,
    ReplayFailed,
    ReturnToPresentFailed,
}

impl HistoryTraversalFailureReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MissingOldUrl => "missing_old_url",
            Self::MissingNewUrl => "missing_new_url",
            Self::SameUrl => "same_url",
            Self::NonHistoryTransition => "non_history_transition",
            Self::MissingEndpoint => "missing_endpoint",
            Self::SelfLoop => "self_loop",
            Self::GraphRejected => "graph_rejected",
            Self::PersistenceUnavailable => "persistence_unavailable",
            Self::ExportWriteFailed => "export_write_failed",
            Self::ExportReadFailed => "export_read_failed",
            Self::HomeDirectoryUnavailable => "home_directory_unavailable",
            Self::PreviewIsolationViolation => "preview_isolation_violation",
            Self::ReplayFailed => "replay_failed",
            Self::ReturnToPresentFailed => "return_to_present_failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryHealthSummary {
    pub capture_status: HistoryCaptureStatus,
    pub recent_traversal_append_failures: u64,
    pub recent_failure_reason_bucket: Option<String>,
    pub last_error: Option<String>,
    pub traversal_archive_count: usize,
    pub dissolved_archive_count: usize,
    pub preview_mode_active: bool,
    pub last_preview_isolation_violation: bool,
    pub replay_in_progress: bool,
    pub replay_cursor: Option<usize>,
    pub replay_total_steps: Option<usize>,
    pub last_return_to_present_result: Option<String>,
    pub last_event_unix_ms: Option<u64>,
}

use log::warn;

impl super::GraphBrowserApp {
    fn encode_undo_graph_bytes(graph: &super::Graph) -> Option<Vec<u8>> {
        rkyv::to_bytes::<rkyv::rancor::Error>(graph)
            .ok()
            .map(|bytes| bytes.as_slice().to_vec())
    }

    fn decode_undo_graph_bytes(graph_bytes: &[u8]) -> Option<super::Graph> {
        let mut aligned = rkyv::util::AlignedVec::<16>::new();
        aligned.extend_from_slice(graph_bytes);
        rkyv::from_bytes::<super::Graph, rkyv::rancor::Error>(&aligned).ok()
    }

    pub(crate) fn build_undo_redo_snapshot(
        &self,
        workspace_layout_json: Option<String>,
    ) -> Option<super::UndoRedoSnapshot> {
        let graph_bytes = Self::encode_undo_graph_bytes(&self.workspace.domain.graph)?;
        Some(super::UndoRedoSnapshot {
            graph_bytes,
            active_selection: self.focused_selection().clone(),
            selection_by_scope: self.workspace.graph_runtime.selection_by_scope.clone(),
            highlighted_graph_edge: self.workspace.graph_runtime.highlighted_graph_edge,
            workspace_layout_json,
        })
    }

    pub(crate) fn has_relation(
        &self,
        from: super::NodeKey,
        to: super::NodeKey,
        selector: crate::graph::RelationSelector,
    ) -> bool {
        self.workspace
            .domain
            .graph
            .find_edge_key(from, to)
            .and_then(|edge_key| self.workspace.domain.graph.get_edge(edge_key))
            .is_some_and(|payload| payload.has_relation(selector))
    }

    fn would_create_user_grouped_edge(&self, from: super::NodeKey, to: super::NodeKey) -> bool {
        if from == to {
            return false;
        }
        if self.workspace.domain.graph.get_node(from).is_none()
            || self.workspace.domain.graph.get_node(to).is_none()
        {
            return false;
        }
        !self.has_relation(
            from,
            to,
            crate::graph::RelationSelector::Semantic(crate::graph::SemanticSubKind::UserGrouped),
        )
    }

    fn would_promote_import_record_to_user_group(
        &self,
        record_id: &str,
        anchor: super::NodeKey,
    ) -> bool {
        let member_keys = self
            .workspace
            .domain
            .graph
            .import_record_member_keys(record_id);
        if !member_keys.contains(&anchor) {
            return false;
        }
        member_keys.into_iter().any(|member| {
            member != anchor
                && (self.would_create_user_grouped_edge(anchor, member)
                    || self.would_create_user_grouped_edge(member, anchor))
        })
    }

    pub(crate) fn should_capture_undo_checkpoint_for_intent(
        &self,
        intent: &super::GraphIntent,
    ) -> bool {
        if matches!(intent, super::GraphIntent::AcceptHostOpenRequest { .. }) {
            return true;
        }
        let Some(mutation) = intent.as_graph_mutation() else {
            return false;
        };

        match mutation {
            super::GraphMutation::CreateNodeNearCenter
            | super::GraphMutation::CreateNodeNearCenterAndOpen { .. }
            | super::GraphMutation::CreateNodeAtUrl { .. }
            | super::GraphMutation::CreateNodeAtUrlAndOpen { .. } => true,
            super::GraphMutation::RemoveSelectedNodes => !self.focused_selection().is_empty(),
            super::GraphMutation::MarkTombstoneForSelected => {
                self.focused_selection().iter().any(|key| {
                    self.workspace
                        .domain
                        .graph
                        .get_node(*key)
                        .is_some_and(|n| n.lifecycle != crate::graph::NodeLifecycle::Tombstone)
                })
            }
            super::GraphMutation::RestoreGhostNode { key } => self
                .workspace
                .domain
                .graph
                .get_node(key)
                .is_some_and(|n| n.lifecycle == crate::graph::NodeLifecycle::Tombstone),
            super::GraphMutation::ClearGraph => self.workspace.domain.graph.node_count() > 0,
            super::GraphMutation::CreateUserGroupedEdge { from, to, .. } => {
                self.would_create_user_grouped_edge(from, to)
            }
            super::GraphMutation::DeleteImportRecord { record_id } => self
                .workspace
                .domain
                .graph
                .import_records()
                .iter()
                .any(|record| record.record_id == record_id),
            super::GraphMutation::SuppressImportRecordMembership { record_id, key } => self
                .workspace
                .domain
                .graph
                .import_record_member_keys(&record_id)
                .contains(&key),
            super::GraphMutation::PromoteImportRecordToUserGroup { record_id, anchor } => {
                self.would_promote_import_record_to_user_group(&record_id, anchor)
            }
            super::GraphMutation::CreateUserGroupedEdgeFromPrimarySelection => self
                .selected_pair_in_order()
                .map(|(from, to)| self.would_create_user_grouped_edge(from, to))
                .unwrap_or(false),
            super::GraphMutation::RemoveEdge {
                from,
                to,
                selector,
            } => self.has_relation(from, to, selector),
            super::GraphMutation::SetNodePinned { key, is_pinned } => {
                let Some(node) = self.workspace.domain.graph.get_node(key) else {
                    return false;
                };
                let has_pin_tag = node.tags.contains(Self::TAG_PIN);
                node.is_pinned != is_pinned || has_pin_tag != is_pinned
            }
            super::GraphMutation::SetNodeUrl { key, new_url } => self
                .workspace
                .domain
                .graph
                .get_node(key)
                .map(|node| node.url != new_url)
                .unwrap_or(false),
            super::GraphMutation::TagNode { key, tag } => {
                let Some(node) = self.workspace.domain.graph.get_node(key) else {
                    return false;
                };
                if tag == Self::TAG_PIN && !node.is_pinned {
                    return true;
                }
                !node.tags.contains(&tag)
            }
            super::GraphMutation::UntagNode { key, tag } => {
                if tag == Self::TAG_PIN
                    && self
                        .workspace
                        .domain
                        .graph
                        .get_node(key)
                        .map(|node| node.is_pinned)
                        .unwrap_or(false)
                {
                    return true;
                }
                self.workspace
                    .domain
                    .graph
                    .get_node(key)
                    .is_some_and(|node| node.tags.contains(&tag))
            }
            super::GraphMutation::UpdateNodeMimeHint { key, mime_hint } => self
                .workspace
                .domain
                .graph
                .get_node(key)
                .map(|node| node.mime_hint != mime_hint)
                .unwrap_or(false),
            super::GraphMutation::UpdateNodeAddressKind { key, kind } => self
                .workspace
                .domain
                .graph
                .get_node(key)
                .map(|node| node.address_kind != kind)
                .unwrap_or(false),
            _ => false,
        }
    }

    pub(crate) fn current_undo_checkpoint_layout_json(&self) -> Option<String> {
        self.workspace
            .workbench_session
            .last_session_workspace_layout_json
            .clone()
            .or_else(|| self.load_workspace_layout_json(Self::SESSION_WORKSPACE_LAYOUT_NAME))
    }

    pub(crate) fn intent_blocked_during_history_preview(intent: &super::GraphIntent) -> bool {
        Self::history_preview_blocks_intent(intent)
    }

    pub(crate) fn replay_history_preview_cursor(
        &mut self,
        cursor: usize,
        total_steps: usize,
    ) -> Result<(), String> {
        self.apply_history_preview_cursor(cursor, total_steps)
    }

    /// Return recent traversal archive entries (descending, newest first).
    pub fn history_manager_timeline_entries(&self, limit: usize) -> Vec<super::LogEntry> {
        self.services
            .persistence
            .as_ref()
            .map(|store| store.recent_traversal_archive_entries(limit))
            .unwrap_or_default()
    }

    /// Return recent dissolved archive entries (descending, newest first).
    pub fn history_manager_dissolved_entries(&self, limit: usize) -> Vec<super::LogEntry> {
        self.services
            .persistence
            .as_ref()
            .map(|store| store.recent_dissolved_archive_entries(limit))
            .unwrap_or_default()
    }

    /// Return (traversal_archive_count, dissolved_archive_count).
    pub fn history_manager_archive_counts(&self) -> (usize, usize) {
        self.services
            .persistence
            .as_ref()
            .map(|store| (store.traversal_archive_len(), store.dissolved_archive_len()))
            .unwrap_or((0, 0))
    }

    /// Return per-node audit history entries (newest first).
    /// Returns `LogEntry::AppendNodeAuditEvent` records for the given node, up to `limit`.
    pub fn node_audit_history_entries(
        &self,
        node_id: uuid::Uuid,
        limit: usize,
    ) -> Vec<super::LogEntry> {
        self.services
            .persistence
            .as_ref()
            .map(|store| store.node_audit_history(&node_id.to_string(), limit))
            .unwrap_or_default()
    }

    /// Return per-node address navigation history entries (newest first).
    /// Returns `LogEntry::NavigateNode` records for the given node, up to `limit`.
    pub fn node_navigation_history_entries(
        &self,
        node_id: uuid::Uuid,
        limit: usize,
    ) -> Vec<super::LogEntry> {
        self.services
            .persistence
            .as_ref()
            .map(|store| store.node_navigation_history(&node_id.to_string(), limit))
            .unwrap_or_default()
    }

    /// Return mixed-timeline entries across all history tracks, filtered and sorted newest-first.
    pub fn mixed_timeline_entries(
        &self,
        filter: &crate::services::persistence::types::HistoryTimelineFilter,
        limit: usize,
    ) -> Vec<crate::services::persistence::types::HistoryTimelineEvent> {
        self.services
            .persistence
            .as_ref()
            .map(|store| store.mixed_timeline_entries(filter, limit))
            .unwrap_or_default()
    }

    /// Return timeline index entries for Stage F replay cursors (newest first).
    pub fn history_timeline_index_entries(&self, limit: usize) -> Vec<super::TimelineIndexEntry> {
        self.services
            .persistence
            .as_ref()
            .map(|store| store.timeline_index_entries(limit))
            .unwrap_or_default()
    }

    /// Return compact history subsystem health fields for History Manager UI.
    pub fn history_health_summary(&self) -> HistoryHealthSummary {
        let (traversal_archive_count, dissolved_archive_count) =
            self.history_manager_archive_counts();
        let capture_status = if self.services.persistence.is_some() {
            HistoryCaptureStatus::Full
        } else {
            HistoryCaptureStatus::DegradedCaptureOnly
        };

        HistoryHealthSummary {
            capture_status,
            recent_traversal_append_failures: self
                .workspace
                .graph_runtime
                .history_recent_traversal_append_failures,
            recent_failure_reason_bucket: self
                .workspace
                .graph_runtime
                .history_recent_failure_reason_bucket
                .map(|reason| reason.as_str().to_string()),
            last_error: self.workspace.graph_runtime.history_last_error.clone(),
            traversal_archive_count,
            dissolved_archive_count,
            preview_mode_active: self.workspace.graph_runtime.history_preview_mode_active,
            last_preview_isolation_violation: self
                .workspace
                .graph_runtime
                .history_last_preview_isolation_violation,
            replay_in_progress: self.workspace.graph_runtime.history_replay_in_progress,
            replay_cursor: self.workspace.graph_runtime.history_replay_cursor,
            replay_total_steps: self.workspace.graph_runtime.history_replay_total_steps,
            last_return_to_present_result: self
                .workspace
                .graph_runtime
                .history_last_return_to_present_result
                .clone(),
            last_event_unix_ms: self.workspace.graph_runtime.history_last_event_unix_ms,
        }
    }

    /// Record an undo boundary for a pure workspace-layout mutation.
    pub fn record_workspace_undo_boundary(
        &mut self,
        workspace_layout_before: Option<String>,
        reason: super::UndoBoundaryReason,
    ) {
        let layout_before =
            workspace_layout_before.or_else(|| self.current_undo_checkpoint_layout_json());
        self.capture_undo_checkpoint_internal(layout_before, reason);
    }

    /// Capture current global state as an undo checkpoint.
    fn capture_undo_checkpoint(&mut self, workspace_layout_json: Option<String>) {
        self.capture_undo_checkpoint_internal(
            workspace_layout_json,
            super::UndoBoundaryReason::ReducerIntents,
        );
    }

    pub(crate) fn capture_undo_checkpoint_internal(
        &mut self,
        workspace_layout_json: Option<String>,
        _reason: super::UndoBoundaryReason,
    ) {
        let Some(snapshot) = self.build_undo_redo_snapshot(workspace_layout_json) else {
            warn!("Failed to serialize graph for undo checkpoint; skipping capture");
            return;
        };
        self.workspace.graph_runtime.undo_stack.push(snapshot);
        self.workspace.graph_runtime.redo_stack.clear();
        const MAX_UNDO_STEPS: usize = 128;
        if self.workspace.graph_runtime.undo_stack.len() > MAX_UNDO_STEPS {
            let excess = self.workspace.graph_runtime.undo_stack.len() - MAX_UNDO_STEPS;
            self.workspace.graph_runtime.undo_stack.drain(0..excess);
        }
    }

    /// Perform one global undo step using current frame layout as redo checkpoint.
    pub(crate) fn perform_undo(&mut self, current_workspace_layout_json: Option<String>) -> bool {
        let Some(prev) = self.workspace.graph_runtime.undo_stack.last().cloned() else {
            return false;
        };
        let Some(prev_graph) = Self::decode_undo_graph_bytes(&prev.graph_bytes) else {
            warn!("Failed to deserialize graph from undo checkpoint");
            return false;
        };
        let Some(redo_snapshot) = self.build_undo_redo_snapshot(current_workspace_layout_json)
        else {
            warn!("Failed to serialize graph for redo checkpoint");
            return false;
        };
        let _ = self.workspace.graph_runtime.undo_stack.pop();
        self.workspace.graph_runtime.redo_stack.push(redo_snapshot);
        self.apply_loaded_graph(prev_graph);
        self.restore_selection_snapshot(prev.active_selection, prev.selection_by_scope);
        self.workspace.graph_runtime.highlighted_graph_edge = prev.highlighted_graph_edge;
        self.set_pending_history_workspace_layout_json(prev.workspace_layout_json);
        true
    }

    /// Perform one global redo step using current frame layout as undo checkpoint.
    pub(crate) fn perform_redo(&mut self, current_workspace_layout_json: Option<String>) -> bool {
        let Some(next) = self.workspace.graph_runtime.redo_stack.last().cloned() else {
            return false;
        };
        let Some(next_graph) = Self::decode_undo_graph_bytes(&next.graph_bytes) else {
            warn!("Failed to deserialize graph from redo checkpoint");
            return false;
        };
        let Some(undo_snapshot) = self.build_undo_redo_snapshot(current_workspace_layout_json)
        else {
            warn!("Failed to serialize graph for undo checkpoint during redo");
            return false;
        };
        let _ = self.workspace.graph_runtime.redo_stack.pop();
        self.workspace.graph_runtime.undo_stack.push(undo_snapshot);
        self.apply_loaded_graph(next_graph);
        self.restore_selection_snapshot(next.active_selection, next.selection_by_scope);
        self.workspace.graph_runtime.highlighted_graph_edge = next.highlighted_graph_edge;
        self.set_pending_history_workspace_layout_json(next.workspace_layout_json);
        true
    }

    /// Get the length of the undo stack (for testing).
    pub fn undo_stack_len(&self) -> usize {
        self.workspace.graph_runtime.undo_stack.len()
    }

    /// Get the length of the redo stack (for testing).
    pub fn redo_stack_len(&self) -> usize {
        self.workspace.graph_runtime.redo_stack.len()
    }

    /// Take pending frame layout restore emitted by undo/redo.
    pub fn take_pending_history_workspace_layout_json(&mut self) -> Option<String> {
        match self.take_pending_app_command(|command| {
            matches!(
                command,
                super::AppCommand::RestoreHistoryWorkspaceLayout { .. }
            )
        })? {
            super::AppCommand::RestoreHistoryWorkspaceLayout { layout_json } => Some(layout_json),
            _ => None,
        }
    }

    /// Take pending frame layout restore emitted by undo/redo.
    pub fn take_pending_history_frame_layout_json(&mut self) -> Option<String> {
        self.take_pending_history_workspace_layout_json()
    }
}
