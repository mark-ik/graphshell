use super::*;

impl GraphBrowserApp {
    pub(crate) fn apply_open_node_frame_routed(
        &mut self,
        key: NodeKey,
        prefer_frame: Option<String>,
    ) {
        debug!("app: applying OpenNodeFrameRouted for {:?}", key);
        self.apply_routed_workspace_open(key, prefer_frame.as_deref());
    }

    pub(crate) fn apply_open_node_workspace_routed(
        &mut self,
        key: NodeKey,
        prefer_workspace: Option<String>,
    ) {
        debug!("app: applying OpenNodeWorkspaceRouted for {:?}", key);
        self.apply_routed_workspace_open(key, prefer_workspace.as_deref());
    }

    fn apply_routed_workspace_open(&mut self, key: NodeKey, prefer_workspace: Option<&str>) {
        self.select_node(key, false);
        match self.resolve_workspace_open(key, prefer_workspace) {
            FrameOpenAction::RestoreFrame { name, .. } => {
                self.set_pending_workspace_restore_open_request(Some(PendingNodeOpenRequest {
                    key,
                    mode: PendingTileOpenMode::Tab,
                }));
                self.request_restore_workspace_snapshot_named(name);
            }
            FrameOpenAction::OpenInCurrentFrame { .. } => {
                self.mark_current_workspace_synthesized();
                self.set_pending_workspace_restore_open_request(None);
                self.request_open_node_tile_mode(key, PendingTileOpenMode::Tab);
            }
        }
    }

    /// Queue/replace an unsaved-frame prompt request.
    pub fn request_unsaved_frame_prompt(&mut self, request: UnsavedFramePromptRequest) {
        self.set_pending_unsaved_workspace_prompt(Some(request), None);
    }

    /// Queue/replace an unsaved-workspace prompt request.
    pub fn request_unsaved_workspace_prompt(&mut self, request: UnsavedFramePromptRequest) {
        self.request_unsaved_frame_prompt(request);
    }

    /// Inspect active unsaved-frame prompt request.
    pub fn unsaved_frame_prompt_request(&self) -> Option<&UnsavedFramePromptRequest> {
        match self.pending_app_command(|command| {
            matches!(command, AppCommand::UnsavedWorkspacePrompt { .. })
        })? {
            AppCommand::UnsavedWorkspacePrompt { request, .. } => Some(request),
            _ => None,
        }
    }

    /// Inspect active unsaved-workspace prompt request.
    pub fn unsaved_workspace_prompt_request(&self) -> Option<&UnsavedFramePromptRequest> {
        self.unsaved_frame_prompt_request()
    }

    /// Capture user action from unsaved-frame prompt UI.
    pub fn set_unsaved_frame_prompt_action(&mut self, action: UnsavedFramePromptAction) {
        let Some(request) = self.unsaved_frame_prompt_request().cloned() else {
            return;
        };
        self.set_pending_unsaved_workspace_prompt(Some(request), Some(action));
    }

    /// Capture user action from unsaved-workspace prompt UI.
    pub fn set_unsaved_workspace_prompt_action(&mut self, action: UnsavedFramePromptAction) {
        self.set_unsaved_frame_prompt_action(action);
    }

    /// Resolve and clear active unsaved-frame prompt when an action was chosen.
    pub fn take_unsaved_frame_prompt_resolution(
        &mut self,
    ) -> Option<(UnsavedFramePromptRequest, UnsavedFramePromptAction)> {
        match self.take_pending_app_command(|command| {
            matches!(
                command,
                AppCommand::UnsavedWorkspacePrompt {
                    action: Some(_),
                    ..
                }
            )
        })? {
            AppCommand::UnsavedWorkspacePrompt {
                request,
                action: Some(action),
            } => Some((request, action)),
            _ => None,
        }
    }

    /// Resolve and clear active unsaved-workspace prompt when an action was chosen.
    pub fn take_unsaved_workspace_prompt_resolution(
        &mut self,
    ) -> Option<(UnsavedFramePromptRequest, UnsavedFramePromptAction)> {
        self.take_unsaved_frame_prompt_resolution()
    }

    /// Mark the current frame context as synthesized from runtime actions.
    pub fn mark_current_workspace_synthesized(&mut self) {
        self.workspace.current_workspace_is_synthesized = true;
        self.workspace.workspace_has_unsaved_changes = false;
        self.workspace.unsaved_workspace_prompt_warned = false;
    }

    /// Mark the current frame context as synthesized from runtime actions.
    pub fn mark_current_frame_synthesized(&mut self) {
        self.mark_current_workspace_synthesized();
    }

    /// Workspace-activation recency sequence for a node (higher = more recent).
    pub fn workspace_recency_seq_for_node(&self, key: NodeKey) -> u64 {
        let Some(node) = self.workspace.domain.graph.get_node(key) else {
            return 0;
        };
        self.workspace
            .node_last_active_workspace
            .get(&node.id)
            .map(|(seq, _)| *seq)
            .unwrap_or(0)
    }

    /// Frame-activation recency sequence for a node (higher = more recent).
    pub fn frame_recency_seq_for_node(&self, key: NodeKey) -> u64 {
        self.workspace_recency_seq_for_node(key)
    }

    /// Frame memberships for a node sorted by recency (most recent first), then name.
    pub fn sorted_frames_for_node_key(&self, key: NodeKey) -> Vec<String> {
        let mut names: Vec<String> = self.frames_for_node_key(key).iter().cloned().collect();
        let Some(node) = self.workspace.domain.graph.get_node(key) else {
            return names;
        };
        if let Some((_, recent)) = self.workspace.node_last_active_workspace.get(&node.id)
            && let Some(idx) = names.iter().position(|name| name == recent)
        {
            let recent = names.remove(idx);
            names.insert(0, recent);
        }
        names
    }

    pub fn sorted_workspaces_for_node_key(&self, key: NodeKey) -> Vec<String> {
        self.sorted_frames_for_node_key(key)
    }

    /// Last activation sequence associated with a workspace name.
    pub fn workspace_recency_seq_for_name(&self, workspace_name: &str) -> u64 {
        self.workspace
            .node_last_active_workspace
            .values()
            .filter_map(|(seq, name)| (name == workspace_name).then_some(*seq))
            .max()
            .unwrap_or(0)
    }

    /// Last activation sequence associated with a frame snapshot name.
    pub fn frame_recency_seq_for_name(&self, frame_name: &str) -> u64 {
        self.workspace_recency_seq_for_name(frame_name)
    }

    /// Mark a named frame snapshot as activated, updating per-node recency.
    pub fn note_workspace_activated(
        &mut self,
        workspace_name: &str,
        nodes: impl IntoIterator<Item = NodeKey>,
    ) {
        self.workspace.workspace_activation_seq =
            self.workspace.workspace_activation_seq.saturating_add(1);
        let seq = self.workspace.workspace_activation_seq;
        let workspace_name = workspace_name.to_string();
        for key in nodes {
            let Some(node) = self.workspace.domain.graph.get_node(key) else {
                continue;
            };
            self.workspace
                .node_last_active_workspace
                .insert(node.id, (seq, workspace_name.clone()));
            self.workspace
                .node_workspace_membership
                .entry(node.id)
                .or_default()
                .insert(workspace_name.clone());
        }
        self.workspace.current_workspace_is_synthesized = false;
        self.workspace.workspace_has_unsaved_changes = false;
        self.workspace.unsaved_workspace_prompt_warned = false;
        self.workspace.egui_state_dirty = true;
    }

    /// Mark a named frame snapshot as activated, updating per-node recency.
    pub fn note_frame_activated(
        &mut self,
        frame_name: &str,
        nodes: impl IntoIterator<Item = NodeKey>,
    ) {
        self.note_workspace_activated(frame_name, nodes);
    }

    /// Initialize membership index from desktop-layer workspace scan.
    pub fn init_membership_index(&mut self, index: HashMap<Uuid, BTreeSet<String>>) {
        self.workspace.node_workspace_membership = index;
        self.workspace.egui_state_dirty = true;
    }

    /// Initialize UUID-keyed workspace activation recency from desktop-layer manifest scan.
    pub fn init_workspace_activation_recency(
        &mut self,
        recency: HashMap<Uuid, (u64, String)>,
        activation_seq: u64,
    ) {
        self.workspace.node_last_active_workspace = recency;
        self.workspace.workspace_activation_seq = activation_seq;
    }

    /// Initialize UUID-keyed frame activation recency from desktop-layer manifest scan.
    pub fn init_frame_activation_recency(
        &mut self,
        recency: HashMap<Uuid, (u64, String)>,
        activation_seq: u64,
    ) {
        self.init_workspace_activation_recency(recency, activation_seq);
    }

    fn empty_workspace_membership() -> &'static BTreeSet<String> {
        static EMPTY: OnceLock<BTreeSet<String>> = OnceLock::new();
        EMPTY.get_or_init(BTreeSet::new)
    }

    /// Frame membership set for a stable node UUID.
    pub fn membership_for_node(&self, uuid: Uuid) -> &BTreeSet<String> {
        self.workspace
            .node_workspace_membership
            .get(&uuid)
            .unwrap_or_else(|| Self::empty_workspace_membership())
    }

    /// Frame membership set for a NodeKey in the current graph.
    pub fn frames_for_node_key(&self, key: NodeKey) -> &BTreeSet<String> {
        let Some(node) = self.workspace.domain.graph.get_node(key) else {
            return Self::empty_workspace_membership();
        };
        self.membership_for_node(node.id)
    }

    /// Frame membership set for a NodeKey in the current graph.
    pub fn workspaces_for_node_key(&self, key: NodeKey) -> &BTreeSet<String> {
        self.frames_for_node_key(key)
    }

    /// Resolve workspace-aware node-open behavior with deterministic fallback.
    fn resolve_frame_open_with_reason(
        &self,
        node: NodeKey,
        prefer_frame: Option<&str>,
    ) -> (FrameOpenAction, FrameOpenReason) {
        if self.workspace.domain.graph.get_node(node).is_none() {
            return (
                FrameOpenAction::OpenInCurrentFrame { node },
                FrameOpenReason::MissingNode,
            );
        }
        let memberships = self.frames_for_node_key(node);
        let node_uuid = self.workspace.domain.graph.get_node(node).map(|n| n.id);

        if let Some(preferred_name) = prefer_frame
            && memberships.contains(preferred_name)
        {
            return (
                FrameOpenAction::RestoreFrame {
                    name: preferred_name.to_string(),
                    node,
                },
                FrameOpenReason::PreferredFrame,
            );
        }

        if !memberships.is_empty() {
            if let Some((_, recent_workspace)) =
                node_uuid.and_then(|uuid| self.workspace.node_last_active_workspace.get(&uuid))
                && memberships.contains(recent_workspace)
            {
                return (
                    FrameOpenAction::RestoreFrame {
                        name: recent_workspace.clone(),
                        node,
                    },
                    FrameOpenReason::RecentMembership,
                );
            }
            if let Some(name) = memberships.iter().next() {
                return (
                    FrameOpenAction::RestoreFrame {
                        name: name.clone(),
                        node,
                    },
                    FrameOpenReason::DeterministicMembershipFallback,
                );
            }
        }

        (
            FrameOpenAction::OpenInCurrentFrame { node },
            FrameOpenReason::NoMembership,
        )
    }

    /// Resolve workspace-aware node-open behavior with deterministic fallback.
    pub fn resolve_frame_open(&self, node: NodeKey, prefer_frame: Option<&str>) -> FrameOpenAction {
        let node_uuid = self.workspace.domain.graph.get_node(node).map(|n| n.id);
        let (action, reason) = self.resolve_frame_open_with_reason(node, prefer_frame);
        match (&action, reason) {
            (FrameOpenAction::OpenInCurrentFrame { .. }, FrameOpenReason::MissingNode) => {
                debug!(
                    "frame routing: node {:?} missing in graph; falling back to current frame",
                    node
                );
            }
            (FrameOpenAction::RestoreFrame { name, .. }, FrameOpenReason::PreferredFrame) => {
                debug!(
                    "frame routing: node {:?} ({:?}) using explicit preferred frame '{}'",
                    node, node_uuid, name
                );
            }
            (FrameOpenAction::RestoreFrame { name, .. }, FrameOpenReason::RecentMembership) => {
                debug!(
                    "frame routing: node {:?} ({:?}) selected recent frame '{}'",
                    node, node_uuid, name
                );
            }
            (
                FrameOpenAction::RestoreFrame { name, .. },
                FrameOpenReason::DeterministicMembershipFallback,
            ) => {
                debug!(
                    "frame routing: node {:?} ({:?}) selected deterministic fallback frame '{}'",
                    node, node_uuid, name
                );
            }
            (FrameOpenAction::OpenInCurrentFrame { .. }, FrameOpenReason::NoMembership) => {
                debug!(
                    "frame routing: node {:?} ({:?}) has no memberships; opening in current frame",
                    node, node_uuid
                );
            }
            _ => {
                debug!(
                    "frame routing: node {:?} ({:?}) resolved {:?} via {:?}",
                    node, node_uuid, action, reason
                );
            }
        }
        action
    }

    pub fn resolve_workspace_open(
        &self,
        node: NodeKey,
        prefer_workspace: Option<&str>,
    ) -> FrameOpenAction {
        self.resolve_frame_open(node, prefer_workspace)
    }

    pub fn resolve_workspace_open_with_reason(
        &self,
        node: NodeKey,
        prefer_workspace: Option<&str>,
    ) -> (FrameOpenAction, FrameOpenReason) {
        self.resolve_frame_open_with_reason(node, prefer_workspace)
    }

    /// Current explicit node context target for command-surface actions.
    pub fn pending_node_context_target(&self) -> Option<NodeKey> {
        match self.pending_app_command(|command| {
            matches!(command, AppCommand::NodeContextTarget { .. })
        })? {
            AppCommand::NodeContextTarget { target } => Some(*target),
            _ => None,
        }
    }

    /// Set/clear explicit node context target for command-surface actions.
    pub fn set_pending_node_context_target(&mut self, target: Option<NodeKey>) {
        let _ = self.take_pending_app_command(|command| {
            matches!(command, AppCommand::NodeContextTarget { .. })
        });

        if let Some(target) = target {
            self.enqueue_app_command(AppCommand::NodeContextTarget { target });
        }
    }

    /// Request opening the frame picker for a node and mode.
    pub fn request_choose_frame_picker_for_mode(
        &mut self,
        key: NodeKey,
        mode: ChooseFramePickerMode,
    ) {
        self.set_pending_choose_workspace_picker(
            Some(ChooseFramePickerRequest { node: key, mode }),
            None,
        );
    }

    /// Request opening the frame picker to open a node in a frame.
    pub fn request_choose_frame_picker(&mut self, key: NodeKey) {
        self.request_choose_frame_picker_for_mode(key, ChooseFramePickerMode::OpenNodeInFrame);
    }

    /// Request opening the "Choose Workspace..." picker to open a node in a workspace.
    pub fn request_choose_workspace_picker(&mut self, key: NodeKey) {
        self.request_choose_frame_picker(key);
    }

    /// Request opening the frame picker to add node tab membership.
    pub fn request_add_node_to_frame_picker(&mut self, key: NodeKey) {
        self.request_choose_frame_picker_for_mode(key, ChooseFramePickerMode::AddNodeToFrame);
    }

    pub fn request_add_node_to_workspace_picker(&mut self, key: NodeKey) {
        self.request_add_node_to_frame_picker(key);
    }

    /// Request opening the frame picker to add connected nodes.
    pub fn request_add_connected_to_frame_picker(&mut self, key: NodeKey) {
        self.request_choose_frame_picker_for_mode(
            key,
            ChooseFramePickerMode::AddConnectedSelectionToFrame,
        );
    }

    pub fn request_add_connected_to_workspace_picker(&mut self, key: NodeKey) {
        self.request_add_connected_to_frame_picker(key);
    }

    /// Request opening the frame picker to add an exact node set.
    pub fn request_add_exact_selection_to_frame_picker(&mut self, mut keys: Vec<NodeKey>) {
        keys.retain(|key| self.workspace.domain.graph.get_node(*key).is_some());
        keys.sort_by_key(|key| key.index());
        keys.dedup();
        let Some(anchor) = keys.first().copied() else {
            return;
        };
        self.set_pending_choose_workspace_picker(
            Some(ChooseFramePickerRequest {
                node: anchor,
                mode: ChooseFramePickerMode::AddExactSelectionToFrame,
            }),
            Some(keys),
        );
    }

    /// Request opening the "Choose Workspace..." picker to add an exact node set.
    pub fn request_add_exact_selection_to_workspace_picker(&mut self, keys: Vec<NodeKey>) {
        self.request_add_exact_selection_to_frame_picker(keys);
    }

    /// Active request for frame picker.
    pub fn choose_frame_picker_request(&self) -> Option<ChooseFramePickerRequest> {
        match self.pending_app_command(|command| {
            matches!(command, AppCommand::ChooseWorkspacePicker { .. })
        })? {
            AppCommand::ChooseWorkspacePicker { request, .. } => Some(*request),
            _ => None,
        }
    }

    /// Active request for "Choose Workspace..." picker.
    pub fn choose_workspace_picker_request(&self) -> Option<ChooseFramePickerRequest> {
        self.choose_frame_picker_request()
    }

    /// Close frame picker.
    pub fn clear_choose_frame_picker(&mut self) {
        self.set_pending_choose_workspace_picker(None, None);
    }

    /// Close "Choose Workspace..." picker.
    pub fn clear_choose_workspace_picker(&mut self) {
        self.clear_choose_frame_picker();
    }

    /// Request adding `node` to named frame snapshot `frame_name`.
    pub fn request_add_node_to_frame(&mut self, node: NodeKey, frame_name: impl Into<String>) {
        let _ = self.take_pending_app_command(|command| {
            matches!(command, AppCommand::AddNodeToWorkspace { .. })
        });
        self.enqueue_app_command(AppCommand::AddNodeToWorkspace {
            node,
            workspace_name: frame_name.into(),
        });
    }

    /// Request adding `node` to named frame snapshot `workspace_name`.
    pub fn request_add_node_to_workspace(
        &mut self,
        node: NodeKey,
        workspace_name: impl Into<String>,
    ) {
        self.request_add_node_to_frame(node, workspace_name);
    }

    /// Take and clear pending add-node-to-frame request.
    pub fn take_pending_add_node_to_frame(&mut self) -> Option<(NodeKey, String)> {
        match self.take_pending_app_command(|command| {
            matches!(command, AppCommand::AddNodeToWorkspace { .. })
        })? {
            AppCommand::AddNodeToWorkspace {
                node,
                workspace_name,
            } => Some((node, workspace_name)),
            _ => None,
        }
    }

    /// Take and clear pending add-node-to-workspace request.
    pub fn take_pending_add_node_to_workspace(&mut self) -> Option<(NodeKey, String)> {
        self.take_pending_add_node_to_frame()
    }

    /// Request adding nodes connected to `seed_nodes` into named frame snapshot `frame_name`.
    pub fn request_add_connected_to_frame(
        &mut self,
        seed_nodes: Vec<NodeKey>,
        frame_name: impl Into<String>,
    ) {
        let _ = self.take_pending_app_command(|command| {
            matches!(command, AppCommand::AddConnectedToWorkspace { .. })
        });
        self.enqueue_app_command(AppCommand::AddConnectedToWorkspace {
            seed_nodes,
            workspace_name: frame_name.into(),
        });
    }

    /// Request adding nodes connected to `seed_nodes` into named frame snapshot `workspace_name`.
    pub fn request_add_connected_to_workspace(
        &mut self,
        seed_nodes: Vec<NodeKey>,
        workspace_name: impl Into<String>,
    ) {
        self.request_add_connected_to_frame(seed_nodes, workspace_name);
    }

    /// Take and clear pending add-connected-to-frame request.
    pub fn take_pending_add_connected_to_frame(&mut self) -> Option<(Vec<NodeKey>, String)> {
        match self.take_pending_app_command(|command| {
            matches!(command, AppCommand::AddConnectedToWorkspace { .. })
        })? {
            AppCommand::AddConnectedToWorkspace {
                seed_nodes,
                workspace_name,
            } => Some((seed_nodes, workspace_name)),
            _ => None,
        }
    }

    /// Take and clear pending add-connected-to-workspace request.
    pub fn take_pending_add_connected_to_workspace(&mut self) -> Option<(Vec<NodeKey>, String)> {
        self.take_pending_add_connected_to_frame()
    }

    /// Current explicit node set associated with active frame-picker flow.
    pub fn choose_frame_picker_exact_nodes(&self) -> Option<&[NodeKey]> {
        match self.pending_app_command(|command| {
            matches!(command, AppCommand::ChooseWorkspacePicker { .. })
        })? {
            AppCommand::ChooseWorkspacePicker { exact_nodes, .. } => exact_nodes.as_deref(),
            _ => None,
        }
    }

    /// Current explicit node set associated with active choose-workspace picker flow.
    pub fn choose_workspace_picker_exact_nodes(&self) -> Option<&[NodeKey]> {
        self.choose_frame_picker_exact_nodes()
    }

    /// Request adding an exact node set into named frame snapshot `frame_name`.
    pub fn request_add_exact_nodes_to_frame(
        &mut self,
        nodes: Vec<NodeKey>,
        frame_name: impl Into<String>,
    ) {
        let _ = self.take_pending_app_command(|command| {
            matches!(command, AppCommand::AddExactToWorkspace { .. })
        });
        self.enqueue_app_command(AppCommand::AddExactToWorkspace {
            nodes,
            workspace_name: frame_name.into(),
        });
    }

    /// Request adding an exact node set into named frame snapshot `workspace_name`.
    pub fn request_add_exact_nodes_to_workspace(
        &mut self,
        nodes: Vec<NodeKey>,
        workspace_name: impl Into<String>,
    ) {
        self.request_add_exact_nodes_to_frame(nodes, workspace_name);
    }

    /// Take and clear pending exact-add-to-frame request.
    pub fn take_pending_add_exact_to_frame(&mut self) -> Option<(Vec<NodeKey>, String)> {
        match self.take_pending_app_command(|command| {
            matches!(command, AppCommand::AddExactToWorkspace { .. })
        })? {
            AppCommand::AddExactToWorkspace {
                nodes,
                workspace_name,
            } => Some((nodes, workspace_name)),
            _ => None,
        }
    }

    /// Take and clear pending exact-add-to-workspace request.
    pub fn take_pending_add_exact_to_workspace(&mut self) -> Option<(Vec<NodeKey>, String)> {
        self.take_pending_add_exact_to_frame()
    }

    /// Request opening connected nodes for a given source node, tile mode, and scope.
    pub fn request_open_connected_from(
        &mut self,
        source: NodeKey,
        mode: PendingTileOpenMode,
        scope: PendingConnectedOpenScope,
    ) {
        let _ = self.take_pending_app_command(|command| {
            matches!(command, AppCommand::OpenConnected { .. })
        });
        self.enqueue_app_command(AppCommand::OpenConnected {
            source,
            mode,
            scope,
        });
    }

    /// Take and clear pending connected-open request.
    pub fn take_pending_open_connected_from(
        &mut self,
    ) -> Option<(NodeKey, PendingTileOpenMode, PendingConnectedOpenScope)> {
        match self.take_pending_app_command(|command| {
            matches!(command, AppCommand::OpenConnected { .. })
        })? {
            AppCommand::OpenConnected {
                source,
                mode,
                scope,
            } => Some((source, mode, scope)),
            _ => None,
        }
    }

    /// Peek pending connected-open request without consuming it.
    pub fn pending_open_connected_from(
        &self,
    ) -> Option<(NodeKey, PendingTileOpenMode, PendingConnectedOpenScope)> {
        match self
            .pending_app_command(|command| matches!(command, AppCommand::OpenConnected { .. }))?
        {
            AppCommand::OpenConnected {
                source,
                mode,
                scope,
            } => Some((*source, *mode, *scope)),
            _ => None,
        }
    }

    /// Request opening a specific node as a tile in the given mode.
    pub fn request_open_node_tile_mode(&mut self, key: NodeKey, mode: PendingTileOpenMode) {
        let _ =
            self.take_pending_app_command(|command| matches!(command, AppCommand::OpenNode { .. }));
        self.enqueue_app_command(AppCommand::OpenNode {
            request: PendingNodeOpenRequest { key, mode },
        });
    }

    /// Take and clear pending node-open request.
    pub fn take_pending_open_node_request(&mut self) -> Option<PendingNodeOpenRequest> {
        match self
            .take_pending_app_command(|command| matches!(command, AppCommand::OpenNode { .. }))?
        {
            AppCommand::OpenNode { request } => Some(request),
            _ => None,
        }
    }

    /// Peek pending node-open request without consuming it.
    pub fn pending_open_node_request(&self) -> Option<PendingNodeOpenRequest> {
        match self.pending_app_command(|command| matches!(command, AppCommand::OpenNode { .. }))? {
            AppCommand::OpenNode { request } => Some(*request),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_node_frame_routed_queues_restore_request_for_preferred_frame() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("https://example.com".to_string(), Point2D::new(0.0, 0.0));
        let node_id = app.workspace.domain.graph.get_node(key).unwrap().id;
        app.init_membership_index(HashMap::from([(
            node_id,
            BTreeSet::from(["alpha".to_string(), "beta".to_string()]),
        )]));

        app.apply_reducer_intents([GraphIntent::OpenNodeFrameRouted {
            key,
            prefer_frame: Some("beta".to_string()),
        }]);

        assert_eq!(app.focused_selection().primary(), Some(key));
        assert_eq!(
            app.take_pending_restore_frame_snapshot_named(),
            Some("beta".to_string())
        );
        assert_eq!(
            app.take_pending_frame_restore_open_request(),
            Some(PendingNodeOpenRequest {
                key,
                mode: PendingTileOpenMode::Tab,
            })
        );
        assert_eq!(app.pending_open_node_request(), None);
        assert!(!app.workspace.current_workspace_is_synthesized);
    }

    #[test]
    fn open_node_workspace_routed_without_membership_queues_current_workspace_open() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("https://example.com".to_string(), Point2D::new(0.0, 0.0));
        app.set_pending_workspace_restore_open_request(Some(PendingNodeOpenRequest {
            key,
            mode: PendingTileOpenMode::SplitHorizontal,
        }));

        app.apply_reducer_intents([GraphIntent::OpenNodeWorkspaceRouted {
            key,
            prefer_workspace: Some("missing".to_string()),
        }]);

        assert_eq!(app.focused_selection().primary(), Some(key));
        assert_eq!(app.take_pending_restore_workspace_snapshot_named(), None);
        assert_eq!(app.take_pending_workspace_restore_open_request(), None);
        assert_eq!(
            app.take_pending_open_node_request(),
            Some(PendingNodeOpenRequest {
                key,
                mode: PendingTileOpenMode::Tab,
            })
        );
        assert!(app.workspace.current_workspace_is_synthesized);
    }
}
