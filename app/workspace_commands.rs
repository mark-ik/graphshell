use super::*;

impl GraphBrowserApp {
    pub(crate) fn enqueue_app_command(&mut self, command: AppCommand) {
        self.workspace.pending_app_commands.push_back(command);
    }

    pub(crate) fn request_browser_command(
        &mut self,
        target: BrowserCommandTarget,
        command: BrowserCommand,
    ) {
        self.enqueue_app_command(AppCommand::BrowserCommand { command, target });
    }

    pub(crate) fn request_reload_all(&mut self) {
        self.enqueue_app_command(AppCommand::ReloadAll);
    }

    pub(crate) fn take_pending_browser_command(
        &mut self,
    ) -> Option<(BrowserCommandTarget, BrowserCommand)> {
        match self.take_pending_app_command(|command| {
            matches!(command, AppCommand::BrowserCommand { .. })
        })? {
            AppCommand::BrowserCommand { command, target } => Some((target, command)),
            _ => None,
        }
    }

    pub(crate) fn take_pending_reload_all(&mut self) -> bool {
        self.take_pending_app_command(|command| matches!(command, AppCommand::ReloadAll))
            .is_some()
    }

    pub(crate) fn set_pending_camera_command(
        &mut self,
        target_view: Option<GraphViewId>,
        command: Option<CameraCommand>,
    ) {
        let _ = self
            .take_pending_app_command(|queued| matches!(queued, AppCommand::CameraCommand { .. }));

        if let Some(command) = command {
            self.enqueue_app_command(AppCommand::CameraCommand {
                command,
                target_view,
            });
        }
    }

    pub(crate) fn set_pending_keyboard_zoom_request(
        &mut self,
        target_view: Option<GraphViewId>,
        request: Option<KeyboardZoomRequest>,
    ) {
        let _ = self
            .take_pending_app_command(|command| matches!(command, AppCommand::KeyboardZoom { .. }));

        if let (Some(target_view), Some(request)) = (target_view, request) {
            self.enqueue_app_command(AppCommand::KeyboardZoom {
                request,
                target_view,
            });
        }
    }

    pub(crate) fn set_pending_wheel_zoom_delta(
        &mut self,
        target_view: Option<GraphViewId>,
        delta: Option<f32>,
        anchor_screen: Option<(f32, f32)>,
    ) {
        let existing = self
            .take_pending_app_command(|command| matches!(command, AppCommand::WheelZoom { .. }));

        let (Some(target_view), Some(mut delta)) = (target_view, delta) else {
            return;
        };

        let mut anchor_screen = anchor_screen;
        if let Some(AppCommand::WheelZoom {
            target_view: existing_target,
            delta: existing_delta,
            anchor_screen: existing_anchor,
        }) = existing
            && existing_target == target_view
        {
            delta += existing_delta;
            if anchor_screen.is_none() {
                anchor_screen = existing_anchor;
            }
        }

        self.enqueue_app_command(AppCommand::WheelZoom {
            target_view,
            delta,
            anchor_screen,
        });
    }

    pub(crate) fn set_pending_workspace_restore_open_request(
        &mut self,
        request: Option<PendingNodeOpenRequest>,
    ) {
        let _ = self.take_pending_app_command(|command| {
            matches!(command, AppCommand::RestoreWorkspaceOpen { .. })
        });

        if let Some(request) = request {
            self.enqueue_app_command(AppCommand::RestoreWorkspaceOpen { request });
        }
    }

    pub(crate) fn set_pending_unsaved_workspace_prompt(
        &mut self,
        request: Option<UnsavedFramePromptRequest>,
        action: Option<UnsavedFramePromptAction>,
    ) {
        let _ = self.take_pending_app_command(|command| {
            matches!(command, AppCommand::UnsavedWorkspacePrompt { .. })
        });

        if let Some(request) = request {
            self.enqueue_app_command(AppCommand::UnsavedWorkspacePrompt { request, action });
        }
    }

    pub(crate) fn set_pending_choose_workspace_picker(
        &mut self,
        request: Option<ChooseFramePickerRequest>,
        exact_nodes: Option<Vec<NodeKey>>,
    ) {
        let _ = self.take_pending_app_command(|command| {
            matches!(command, AppCommand::ChooseWorkspacePicker { .. })
        });

        if let Some(request) = request {
            self.enqueue_app_command(AppCommand::ChooseWorkspacePicker {
                request,
                exact_nodes,
            });
        }
    }

    pub(crate) fn set_pending_history_workspace_layout_json(
        &mut self,
        layout_json: Option<String>,
    ) {
        let _ = self.take_pending_app_command(|command| {
            matches!(command, AppCommand::RestoreHistoryWorkspaceLayout { .. })
        });

        if let Some(layout_json) = layout_json {
            self.enqueue_app_command(AppCommand::RestoreHistoryWorkspaceLayout { layout_json });
        }
    }

    pub(crate) fn set_pending_graph_search_request(&mut self, request: Option<GraphSearchRequest>) {
        let _ = self.take_pending_app_command(|command| {
            matches!(command, AppCommand::ApplyGraphSearch { .. })
        });

        if let Some(request) = request {
            self.enqueue_app_command(AppCommand::ApplyGraphSearch { request });
        }
    }

    pub(crate) fn sanitize_pending_frame_import_commands(&mut self) {
        let mut retained_commands =
            VecDeque::with_capacity(self.workspace.pending_app_commands.len());

        while let Some(command) = self.workspace.pending_app_commands.pop_front() {
            let retained = match command {
                AppCommand::AddNodeToWorkspace {
                    node,
                    workspace_name,
                } => self.workspace.domain.graph.get_node(node).map(|_| {
                    AppCommand::AddNodeToWorkspace {
                        node,
                        workspace_name,
                    }
                }),
                AppCommand::AddConnectedToWorkspace {
                    seed_nodes,
                    workspace_name,
                } => {
                    let seed_nodes = seed_nodes
                        .into_iter()
                        .filter(|key| self.workspace.domain.graph.get_node(*key).is_some())
                        .collect::<Vec<_>>();
                    (!seed_nodes.is_empty()).then_some(AppCommand::AddConnectedToWorkspace {
                        seed_nodes,
                        workspace_name,
                    })
                }
                AppCommand::AddExactToWorkspace {
                    nodes,
                    workspace_name,
                } => {
                    let nodes = nodes
                        .into_iter()
                        .filter(|key| self.workspace.domain.graph.get_node(*key).is_some())
                        .collect::<Vec<_>>();
                    (!nodes.is_empty()).then_some(AppCommand::AddExactToWorkspace {
                        nodes,
                        workspace_name,
                    })
                }
                AppCommand::ChooseWorkspacePicker {
                    request,
                    exact_nodes,
                } => self.workspace.domain.graph.get_node(request.node).map(|_| {
                    let exact_nodes = exact_nodes
                        .map(|nodes| {
                            nodes
                                .into_iter()
                                .filter(|key| self.workspace.domain.graph.get_node(*key).is_some())
                                .collect::<Vec<_>>()
                        })
                        .filter(|nodes| !nodes.is_empty());
                    AppCommand::ChooseWorkspacePicker {
                        request,
                        exact_nodes,
                    }
                }),
                other => Some(other),
            };

            if let Some(command) = retained {
                retained_commands.push_back(command);
            }
        }

        self.workspace.pending_app_commands = retained_commands;
    }

    pub(crate) fn pending_app_command<P>(&self, mut predicate: P) -> Option<&AppCommand>
    where
        P: FnMut(&AppCommand) -> bool,
    {
        self.workspace
            .pending_app_commands
            .iter()
            .find(|command| predicate(command))
    }

    pub(crate) fn take_pending_app_command<P>(&mut self, predicate: P) -> Option<AppCommand>
    where
        P: FnMut(&AppCommand) -> bool,
    {
        let index = self
            .workspace
            .pending_app_commands
            .iter()
            .position(predicate)?;
        self.workspace.pending_app_commands.remove(index)
    }

    /// Request saving current frame (tile layout) snapshot.
    pub fn request_save_workspace_snapshot(&mut self) {
        self.enqueue_app_command(AppCommand::SaveWorkspaceSnapshot);
    }

    /// Request saving current frame (tile layout) snapshot.
    pub fn request_save_frame_snapshot(&mut self) {
        self.request_save_workspace_snapshot();
    }

    /// Take and clear pending frame save request.
    pub fn take_pending_save_workspace_snapshot(&mut self) -> bool {
        self.take_pending_app_command(|command| {
            matches!(command, AppCommand::SaveWorkspaceSnapshot)
        })
        .is_some()
    }

    /// Take and clear pending frame save request.
    pub fn take_pending_save_frame_snapshot(&mut self) -> bool {
        self.take_pending_save_workspace_snapshot()
    }

    pub fn request_graph_search(&mut self, query: impl Into<String>, filter_mode: bool) {
        self.request_graph_search_with_context(query, filter_mode, GraphSearchOrigin::Manual, None);
    }

    pub fn request_graph_search_with_origin(
        &mut self,
        query: impl Into<String>,
        filter_mode: bool,
        origin: GraphSearchOrigin,
    ) {
        self.request_graph_search_with_context(query, filter_mode, origin, None);
    }

    pub fn request_graph_search_with_context(
        &mut self,
        query: impl Into<String>,
        filter_mode: bool,
        origin: GraphSearchOrigin,
        neighborhood_anchor: Option<NodeKey>,
    ) {
        self.request_graph_search_with_options(
            query,
            filter_mode,
            origin,
            neighborhood_anchor,
            1,
            true,
            None,
        );
    }

    pub fn request_graph_search_with_options(
        &mut self,
        query: impl Into<String>,
        filter_mode: bool,
        origin: GraphSearchOrigin,
        neighborhood_anchor: Option<NodeKey>,
        neighborhood_depth: u8,
        record_history: bool,
        toast_message: Option<String>,
    ) {
        let query = query.into();
        let trimmed = query.trim().to_string();
        let has_neighborhood = neighborhood_anchor.is_some();
        let neighborhood_depth = if has_neighborhood {
            neighborhood_depth.clamp(1, 2)
        } else {
            1
        };

        self.set_pending_graph_search_request(Some(GraphSearchRequest {
            query: trimmed,
            filter_mode,
            origin,
            neighborhood_anchor,
            neighborhood_depth,
            record_history,
            toast_message,
        }));
    }

    pub fn take_pending_graph_search_request(&mut self) -> Option<GraphSearchRequest> {
        match self.take_pending_app_command(|command| {
            matches!(command, AppCommand::ApplyGraphSearch { .. })
        })? {
            AppCommand::ApplyGraphSearch { request } => Some(request),
            _ => None,
        }
    }

    /// Request saving a named frame snapshot.
    pub fn request_save_workspace_snapshot_named(&mut self, name: impl Into<String>) {
        self.enqueue_app_command(AppCommand::SaveWorkspaceSnapshotNamed { name: name.into() });
    }

    /// Request saving a named frame snapshot.
    pub fn request_save_frame_snapshot_named(&mut self, name: impl Into<String>) {
        self.request_save_workspace_snapshot_named(name);
    }

    /// Take and clear pending named frame save request.
    pub fn take_pending_save_workspace_snapshot_named(&mut self) -> Option<String> {
        match self.take_pending_app_command(|command| {
            matches!(command, AppCommand::SaveWorkspaceSnapshotNamed { .. })
        })? {
            AppCommand::SaveWorkspaceSnapshotNamed { name } => Some(name),
            _ => None,
        }
    }

    /// Take and clear pending named frame save request.
    pub fn take_pending_save_frame_snapshot_named(&mut self) -> Option<String> {
        self.take_pending_save_workspace_snapshot_named()
    }

    /// Request restoring a named frame snapshot.
    pub fn request_restore_workspace_snapshot_named(&mut self, name: impl Into<String>) {
        self.enqueue_app_command(AppCommand::RestoreWorkspaceSnapshotNamed { name: name.into() });
    }

    /// Request restoring a named frame snapshot.
    pub fn request_restore_frame_snapshot_named(&mut self, name: impl Into<String>) {
        self.request_restore_workspace_snapshot_named(name);
    }

    /// Take and clear pending named frame restore request.
    pub fn take_pending_restore_workspace_snapshot_named(&mut self) -> Option<String> {
        match self.take_pending_app_command(|command| {
            matches!(command, AppCommand::RestoreWorkspaceSnapshotNamed { .. })
        })? {
            AppCommand::RestoreWorkspaceSnapshotNamed { name } => Some(name),
            _ => None,
        }
    }

    /// Take and clear pending named frame restore request.
    pub fn take_pending_restore_frame_snapshot_named(&mut self) -> Option<String> {
        self.take_pending_restore_workspace_snapshot_named()
    }

    /// Take and clear one-shot open request for routed frame restore.
    pub fn take_pending_workspace_restore_open_request(
        &mut self,
    ) -> Option<PendingNodeOpenRequest> {
        match self.take_pending_app_command(|command| {
            matches!(command, AppCommand::RestoreWorkspaceOpen { .. })
        })? {
            AppCommand::RestoreWorkspaceOpen { request } => Some(request),
            _ => None,
        }
    }

    /// Take and clear one-shot open request for routed frame restore.
    pub fn take_pending_frame_restore_open_request(&mut self) -> Option<PendingNodeOpenRequest> {
        self.take_pending_workspace_restore_open_request()
    }

    /// Request saving a named graph snapshot.
    pub fn request_save_graph_snapshot_named(&mut self, name: impl Into<String>) {
        self.enqueue_app_command(AppCommand::SaveGraphSnapshotNamed { name: name.into() });
    }

    /// Take and clear pending named graph save request.
    pub fn take_pending_save_graph_snapshot_named(&mut self) -> Option<String> {
        match self.take_pending_app_command(|command| {
            matches!(command, AppCommand::SaveGraphSnapshotNamed { .. })
        })? {
            AppCommand::SaveGraphSnapshotNamed { name } => Some(name),
            _ => None,
        }
    }

    /// Request restoring a named graph snapshot.
    pub fn request_restore_graph_snapshot_named(&mut self, name: impl Into<String>) {
        self.enqueue_app_command(AppCommand::RestoreGraphSnapshotNamed { name: name.into() });
    }

    /// Take and clear pending named graph restore request.
    pub fn take_pending_restore_graph_snapshot_named(&mut self) -> Option<String> {
        match self.take_pending_app_command(|command| {
            matches!(command, AppCommand::RestoreGraphSnapshotNamed { .. })
        })? {
            AppCommand::RestoreGraphSnapshotNamed { name } => Some(name),
            _ => None,
        }
    }

    /// Request restoring autosaved latest graph snapshot/replay state.
    pub fn request_restore_graph_snapshot_latest(&mut self) {
        self.enqueue_app_command(AppCommand::RestoreGraphSnapshotLatest);
    }

    /// Take and clear pending autosaved graph restore request.
    pub fn take_pending_restore_graph_snapshot_latest(&mut self) -> bool {
        self.take_pending_app_command(|command| {
            matches!(command, AppCommand::RestoreGraphSnapshotLatest)
        })
        .is_some()
    }

    /// Request deleting a named graph snapshot.
    pub fn request_delete_graph_snapshot_named(&mut self, name: impl Into<String>) {
        self.enqueue_app_command(AppCommand::DeleteGraphSnapshotNamed { name: name.into() });
    }

    /// Take and clear pending named graph delete request.
    pub fn take_pending_delete_graph_snapshot_named(&mut self) -> Option<String> {
        match self.take_pending_app_command(|command| {
            matches!(command, AppCommand::DeleteGraphSnapshotNamed { .. })
        })? {
            AppCommand::DeleteGraphSnapshotNamed { name } => Some(name),
            _ => None,
        }
    }

    /// Request detaching a node's pane into split layout.
    pub fn request_detach_node_to_split(&mut self, key: NodeKey) {
        self.enqueue_workbench_intent(WorkbenchIntent::DetachNodeToSplit { key });
    }

    /// Request batch prune of empty named frame snapshots.
    pub fn request_prune_empty_workspaces(&mut self) {
        self.enqueue_app_command(AppCommand::PruneEmptyWorkspaces);
    }

    /// Request batch prune of empty named frame snapshots.
    pub fn request_prune_empty_frames(&mut self) {
        self.request_prune_empty_workspaces();
    }

    /// Take pending empty-workspace prune request.
    pub fn take_pending_prune_empty_workspaces(&mut self) -> bool {
        self.take_pending_app_command(|command| matches!(command, AppCommand::PruneEmptyWorkspaces))
            .is_some()
    }

    /// Take pending empty-frame prune request.
    pub fn take_pending_prune_empty_frames(&mut self) -> bool {
        self.take_pending_prune_empty_workspaces()
    }

    /// Request keeping latest N named frame snapshots.
    pub fn request_keep_latest_named_workspaces(&mut self, keep: usize) {
        self.enqueue_app_command(AppCommand::KeepLatestNamedWorkspaces { keep });
    }

    /// Request keeping latest N named frame snapshots.
    pub fn request_keep_latest_named_frames(&mut self, keep: usize) {
        self.request_keep_latest_named_workspaces(keep);
    }

    /// Take pending keep-latest-N named frame snapshots request.
    pub fn take_pending_keep_latest_named_workspaces(&mut self) -> Option<usize> {
        match self.take_pending_app_command(|command| {
            matches!(command, AppCommand::KeepLatestNamedWorkspaces { .. })
        })? {
            AppCommand::KeepLatestNamedWorkspaces { keep } => Some(keep),
            _ => None,
        }
    }

    /// Take pending keep-latest-N named frame snapshots request.
    pub fn take_pending_keep_latest_named_frames(&mut self) -> Option<usize> {
        self.take_pending_keep_latest_named_workspaces()
    }

    pub fn request_copy_node_url(&mut self, key: NodeKey) {
        self.enqueue_app_command(AppCommand::ClipboardCopy {
            request: ClipboardCopyRequest {
                key,
                kind: ClipboardCopyKind::Url,
            },
        });
    }

    pub fn request_copy_node_title(&mut self, key: NodeKey) {
        self.enqueue_app_command(AppCommand::ClipboardCopy {
            request: ClipboardCopyRequest {
                key,
                kind: ClipboardCopyKind::Title,
            },
        });
    }

    pub fn take_pending_clipboard_copy(&mut self) -> Option<ClipboardCopyRequest> {
        match self.take_pending_app_command(|command| {
            matches!(command, AppCommand::ClipboardCopy { .. })
        })? {
            AppCommand::ClipboardCopy { request } => Some(request),
            _ => None,
        }
    }

    pub fn request_switch_data_dir(&mut self, path: impl AsRef<Path>) {
        self.enqueue_app_command(AppCommand::SwitchDataDir {
            path: path.as_ref().to_path_buf(),
        });
    }

    pub fn take_pending_switch_data_dir(&mut self) -> Option<PathBuf> {
        match self.take_pending_app_command(|command| {
            matches!(command, AppCommand::SwitchDataDir { .. })
        })? {
            AppCommand::SwitchDataDir { path } => Some(path),
            _ => None,
        }
    }

    pub fn request_open_clip_by_id(&mut self, clip_id: impl Into<String>) {
        self.enqueue_app_command(AppCommand::OpenClip {
            clip_id: clip_id.into(),
        });
    }

    pub fn take_pending_open_note_request(&mut self) -> Option<NoteId> {
        match self
            .take_pending_app_command(|command| matches!(command, AppCommand::OpenNote { .. }))?
        {
            AppCommand::OpenNote { note_id } => Some(note_id),
            _ => None,
        }
    }

    pub fn request_open_note_by_id(&mut self, note_id: NoteId) {
        self.enqueue_app_command(AppCommand::OpenNote { note_id });
    }

    pub fn take_pending_open_clip_request(&mut self) -> Option<String> {
        match self
            .take_pending_app_command(|command| matches!(command, AppCommand::OpenClip { .. }))?
        {
            AppCommand::OpenClip { clip_id } => Some(clip_id),
            _ => None,
        }
    }

    pub(crate) fn set_pending_protocol_probe(&mut self, key: NodeKey, url: Option<String>) {
        let _ = self.take_pending_app_command(|command| {
            matches!(command, AppCommand::ProtocolProbe { key: pending_key, .. } if *pending_key == key)
        });
        self.enqueue_app_command(AppCommand::ProtocolProbe { key, url });
    }

    pub(crate) fn take_pending_protocol_probe(&mut self) -> Option<(NodeKey, Option<String>)> {
        match self.take_pending_app_command(|command| {
            matches!(command, AppCommand::ProtocolProbe { .. })
        })? {
            AppCommand::ProtocolProbe { key, url } => Some((key, url)),
            _ => None,
        }
    }

    pub fn set_pending_tool_surface_return_target(
        &mut self,
        target: Option<ToolSurfaceReturnTarget>,
    ) {
        let _ = self.take_pending_app_command(|command| {
            matches!(command, AppCommand::ToolSurfaceReturnTarget { .. })
        });

        if let Some(target) = target {
            self.enqueue_app_command(AppCommand::ToolSurfaceReturnTarget { target });
        }
    }

    pub fn take_pending_tool_surface_return_target(&mut self) -> Option<ToolSurfaceReturnTarget> {
        match self.take_pending_app_command(|command| {
            matches!(command, AppCommand::ToolSurfaceReturnTarget { .. })
        })? {
            AppCommand::ToolSurfaceReturnTarget { target } => Some(target),
            _ => None,
        }
    }

    pub fn pending_tool_surface_return_target(&self) -> Option<ToolSurfaceReturnTarget> {
        match self.pending_app_command(|command| {
            matches!(command, AppCommand::ToolSurfaceReturnTarget { .. })
        })? {
            AppCommand::ToolSurfaceReturnTarget { target } => Some(target.clone()),
            _ => None,
        }
    }

    pub fn set_pending_command_surface_return_target(
        &mut self,
        target: Option<ToolSurfaceReturnTarget>,
    ) {
        let _ = self.take_pending_app_command(|command| {
            matches!(command, AppCommand::CommandSurfaceReturnTarget { .. })
        });

        if let Some(target) = target {
            self.enqueue_app_command(AppCommand::CommandSurfaceReturnTarget { target });
        }
    }

    pub fn take_pending_command_surface_return_target(
        &mut self,
    ) -> Option<ToolSurfaceReturnTarget> {
        match self.take_pending_app_command(|command| {
            matches!(command, AppCommand::CommandSurfaceReturnTarget { .. })
        })? {
            AppCommand::CommandSurfaceReturnTarget { target } => Some(target),
            _ => None,
        }
    }

    pub fn pending_command_surface_return_target(&self) -> Option<ToolSurfaceReturnTarget> {
        match self.pending_app_command(|command| {
            matches!(command, AppCommand::CommandSurfaceReturnTarget { .. })
        })? {
            AppCommand::CommandSurfaceReturnTarget { target } => Some(target.clone()),
            _ => None,
        }
    }

    pub fn set_pending_transient_surface_return_target(
        &mut self,
        target: Option<ToolSurfaceReturnTarget>,
    ) {
        let _ = self.take_pending_app_command(|command| {
            matches!(command, AppCommand::TransientSurfaceReturnTarget { .. })
        });

        if let Some(target) = target {
            self.enqueue_app_command(AppCommand::TransientSurfaceReturnTarget { target });
        }
    }

    pub fn take_pending_transient_surface_return_target(
        &mut self,
    ) -> Option<ToolSurfaceReturnTarget> {
        match self.take_pending_app_command(|command| {
            matches!(command, AppCommand::TransientSurfaceReturnTarget { .. })
        })? {
            AppCommand::TransientSurfaceReturnTarget { target } => Some(target),
            _ => None,
        }
    }

    pub fn pending_transient_surface_return_target(&self) -> Option<ToolSurfaceReturnTarget> {
        match self.pending_app_command(|command| {
            matches!(command, AppCommand::TransientSurfaceReturnTarget { .. })
        })? {
            AppCommand::TransientSurfaceReturnTarget { target } => Some(target.clone()),
            _ => None,
        }
    }

    pub fn request_restore_transient_surface_focus(&mut self) {
        let _ = self.take_pending_app_command(|command| {
            matches!(command, AppCommand::RestoreTransientSurfaceFocus)
        });
        self.enqueue_app_command(AppCommand::RestoreTransientSurfaceFocus);
    }

    pub fn take_pending_restore_transient_surface_focus(&mut self) -> bool {
        self.take_pending_app_command(|command| {
            matches!(command, AppCommand::RestoreTransientSurfaceFocus)
        })
        .is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graph_search_request_replaces_previous_request() {
        let mut app = GraphBrowserApp::new_for_testing();

        app.request_graph_search("udc:51", true);
        app.request_graph_search("udc:519.6", false);

        assert_eq!(
            app.take_pending_graph_search_request(),
            Some(GraphSearchRequest {
                query: "udc:519.6".to_string(),
                filter_mode: false,
                origin: GraphSearchOrigin::Manual,
                neighborhood_anchor: None,
                neighborhood_depth: 1,
                record_history: true,
                toast_message: None,
            })
        );
        assert!(app.take_pending_graph_search_request().is_none());
    }

    #[test]
    fn graph_search_request_can_explicitly_clear_query() {
        let mut app = GraphBrowserApp::new_for_testing();

        app.request_graph_search("", false);

        assert_eq!(
            app.take_pending_graph_search_request(),
            Some(GraphSearchRequest {
                query: String::new(),
                filter_mode: false,
                origin: GraphSearchOrigin::Manual,
                neighborhood_anchor: None,
                neighborhood_depth: 1,
                record_history: true,
                toast_message: None,
            })
        );
    }
}
