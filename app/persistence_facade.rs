use super::*;

impl GraphBrowserApp {
    pub fn persistence_health_summary(&self) -> PersistenceHealthSummary {
        let (
            store_status,
            snapshot_interval_secs,
            last_snapshot_age_secs,
            named_graph_snapshot_count,
            workspace_layout_count,
            traversal_archive_count,
            dissolved_archive_count,
        ) = if let Some(store) = self.services.persistence.as_ref() {
            (
                "active",
                Some(store.snapshot_interval_secs()),
                Some(store.last_snapshot_age_secs()),
                store.list_named_graph_snapshot_names().len(),
                store.list_workspace_layout_names().len(),
                store.traversal_archive_len(),
                store.dissolved_archive_len(),
            )
        } else {
            ("failed", None, None, 0, 0, 0, 0)
        };

        PersistenceHealthSummary {
            store_status,
            recovered_graph: self.has_recovered_graph(),
            snapshot_interval_secs,
            last_snapshot_age_secs,
            named_graph_snapshot_count,
            workspace_layout_count,
            traversal_archive_count,
            dissolved_archive_count,
            workspace_autosave_interval_secs: self.workspace_autosave_interval_secs(),
            workspace_autosave_retention: self.workspace_autosave_retention(),
        }
    }

    /// Check if it's time for a periodic snapshot
    pub fn check_periodic_snapshot(&mut self) {
        if let Some(store) = &mut self.services.persistence {
            store.check_periodic_snapshot(&self.workspace.domain.graph);
        }
    }

    /// Configure periodic persistence snapshot interval in seconds.
    pub fn set_snapshot_interval_secs(&mut self, secs: u64) -> Result<(), String> {
        let store = self
            .services
            .persistence
            .as_mut()
            .ok_or_else(|| "Persistence is not available".to_string())?;
        store
            .set_snapshot_interval_secs(secs)
            .map_err(|e| e.to_string())
    }

    /// Current periodic persistence snapshot interval in seconds, if persistence is enabled.
    pub fn snapshot_interval_secs(&self) -> Option<u64> {
        self.services
            .persistence
            .as_ref()
            .map(|store| store.snapshot_interval_secs())
    }

    /// Take an immediate snapshot (e.g., on shutdown)
    pub fn take_snapshot(&mut self) {
        if let Some(store) = &mut self.services.persistence {
            store.take_snapshot(&self.workspace.domain.graph);
        }
    }

    /// Persist serialized tile layout JSON.
    pub fn save_tile_layout_json(&mut self, layout_json: &str) {
        if let Some(store) = &mut self.services.persistence
            && let Err(e) = store.save_tile_layout_json(layout_json)
        {
            warn!("Failed to save tile layout: {e}");
        }
    }

    pub fn set_sync_command_tx(
        &mut self,
        tx: Option<tokio_mpsc::Sender<crate::mods::native::verse::SyncCommand>>,
    ) {
        self.services.sync_command_tx = tx;
    }

    pub fn set_client_storage_manager(
        &mut self,
        manager: Option<crate::mods::native::verso::client_storage::ClientStorageManagerHandle>,
    ) {
        self.services.client_storage_manager = manager;
    }

    pub fn set_storage_interop_coordinator(
        &mut self,
        coordinator: Option<storage_interop::StorageInteropCoordinatorHandle>,
    ) {
        self.services.storage_interop_coordinator = coordinator;
    }

    pub fn has_client_storage_manager(&self) -> bool {
        self.services.client_storage_manager.is_some()
    }

    pub fn has_storage_interop_coordinator(&self) -> bool {
        self.services.storage_interop_coordinator.is_some()
    }

    pub fn request_sync_all_trusted_peers(&self, workspace_id: &str) -> Result<usize, String> {
        let Some(tx) = self.services.sync_command_tx.clone() else {
            return Err("sync worker command channel unavailable".to_string());
        };
        let peers = crate::shell::desktop::runtime::registries::phase3_trusted_peers();
        let mut enqueued = 0usize;
        for peer in peers {
            if tx
                .try_send(crate::mods::native::verse::SyncCommand::SyncWorkspace {
                    peer: peer.node_id,
                    workspace_id: workspace_id.to_string(),
                })
                .is_ok()
            {
                enqueued += 1;
            }
        }
        Ok(enqueued)
    }

    /// Load serialized tile layout JSON from persistence.
    pub fn load_tile_layout_json(&self) -> Option<String> {
        self.services
            .persistence
            .as_ref()
            .and_then(|store| store.load_tile_layout_json())
    }

    /// Load or generate the stable workbench view UUID from persistence.
    ///
    /// Phase F: called once at startup; the returned UUID keys all subsequent
    /// `save_graph_tree_json` / `load_graph_tree_json` calls.
    pub fn load_or_ensure_workbench_view_id(&mut self) -> Option<GraphViewId> {
        let store = self.services.persistence.as_mut()?;
        match store.load_or_ensure_workbench_view_id() {
            Ok(uuid) => Some(GraphViewId::from_uuid(uuid)),
            Err(e) => {
                log::warn!("Failed to load/generate workbench view id: {e}");
                None
            }
        }
    }

    /// Persist serialized GraphTree JSON, keyed by the workbench view UUID.
    pub fn save_graph_tree_json(&mut self, view_id: GraphViewId, json: &str) {
        if let Some(store) = &mut self.services.persistence
            && let Err(e) = store.save_graph_tree_json(view_id.as_uuid(), json)
        {
            log::warn!("Failed to save graph tree: {e}");
        }
    }

    /// Load serialized GraphTree JSON for the given workbench view UUID.
    pub fn load_graph_tree_json(&self, view_id: GraphViewId) -> Option<String> {
        self.services
            .persistence
            .as_ref()
            .and_then(|store| store.load_graph_tree_json(view_id.as_uuid()))
    }

    /// Persist serialized tile layout JSON under a workspace name.
    pub fn save_workspace_layout_json(&mut self, name: &str, layout_json: &str) {
        if let Some(store) = &mut self.services.persistence
            && let Err(e) = store.save_workspace_layout_json(name, layout_json)
        {
            warn!("Failed to save frame layout '{name}': {e}");
        }
        if !Self::is_reserved_workspace_layout_name(name) {
            self.workspace
                .workbench_session
                .current_workspace_is_synthesized = false;
            self.workspace
                .workbench_session
                .workspace_has_unsaved_changes = false;
            self.workspace
                .workbench_session
                .unsaved_workspace_prompt_warned = false;
        }
    }

    fn layout_json_hash(layout_json: &str) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        layout_json.hash(&mut hasher);
        hasher.finish()
    }

    fn session_workspace_history_key(index: u8) -> String {
        format!("{}{index}", Self::SESSION_WORKSPACE_PREV_PREFIX)
    }

    fn rotate_session_workspace_history(&mut self, latest_layout_before_overwrite: &str) {
        let retention = self
            .workspace
            .workbench_session
            .workspace_autosave_retention;
        if retention == 0 {
            return;
        }

        for idx in (1..retention).rev() {
            let from_key = Self::session_workspace_history_key(idx);
            let to_key = Self::session_workspace_history_key(idx + 1);
            if let Some(layout) = self.load_workspace_layout_json(&from_key) {
                self.save_workspace_layout_json(&to_key, &layout);
            }
        }
        let first_key = Self::session_workspace_history_key(1);
        self.save_workspace_layout_json(&first_key, latest_layout_before_overwrite);
    }

    /// Persist reserved session frame layout only when the live runtime layout changes.
    ///
    /// The persisted payload for `SESSION_WORKSPACE_LAYOUT_NAME` is the canonical
    /// runtime `egui_tiles::Tree<TileKind>` JSON.
    pub fn save_session_workspace_layout_json_if_changed(&mut self, layout_json: &str) {
        let next_hash = Self::layout_json_hash(layout_json);
        if self
            .workspace
            .workbench_session
            .last_session_workspace_layout_hash
            == Some(next_hash)
        {
            return;
        }
        if let Some(last_at) = self.workspace.workbench_session.last_workspace_autosave_at
            && last_at.elapsed() < self.workspace.workbench_session.workspace_autosave_interval
        {
            return;
        }
        let previous_latest = self.load_workspace_layout_json(Self::SESSION_WORKSPACE_LAYOUT_NAME);
        self.save_workspace_layout_json(Self::SESSION_WORKSPACE_LAYOUT_NAME, layout_json);
        if let Some(previous_latest) = previous_latest {
            self.rotate_session_workspace_history(&previous_latest);
        }
        self.workspace
            .workbench_session
            .last_session_workspace_layout_hash = Some(next_hash);
        self.workspace
            .workbench_session
            .last_session_workspace_layout_json = Some(layout_json.to_string());
        self.workspace.workbench_session.last_workspace_autosave_at = Some(Instant::now());
    }

    /// Mark currently loaded layout as session baseline to suppress redundant writes.
    pub fn mark_session_workspace_layout_json(&mut self, layout_json: &str) {
        self.workspace
            .workbench_session
            .last_session_workspace_layout_hash = Some(Self::layout_json_hash(layout_json));
        self.workspace
            .workbench_session
            .last_session_workspace_layout_json = Some(layout_json.to_string());
        self.workspace.workbench_session.last_workspace_autosave_at = Some(Instant::now());
    }

    /// Mark currently loaded layout as session baseline to suppress redundant writes.
    pub fn mark_session_frame_layout_json(&mut self, layout_json: &str) {
        self.mark_session_workspace_layout_json(layout_json);
    }

    pub fn last_session_workspace_layout_json(&self) -> Option<&str> {
        self.workspace
            .workbench_session
            .last_session_workspace_layout_json
            .as_deref()
    }

    /// Load serialized tile layout JSON by workspace name.
    pub fn load_workspace_layout_json(&self, name: &str) -> Option<String> {
        self.services
            .persistence
            .as_ref()
            .and_then(|store| store.load_workspace_layout_json(name))
    }

    /// List persisted frame layout names in stable order.
    pub fn list_workspace_layout_names(&self) -> Vec<String> {
        self.services
            .persistence
            .as_ref()
            .map(|store| store.list_workspace_layout_names())
            .unwrap_or_default()
    }

    /// Delete a persisted frame layout by name.
    pub fn delete_workspace_layout(&mut self, name: &str) -> Result<(), String> {
        if Self::is_reserved_workspace_layout_name(name) {
            return Err(format!("Cannot delete reserved workspace '{name}'"));
        }
        self.services
            .persistence
            .as_mut()
            .ok_or_else(|| "Persistence is not enabled".to_string())?
            .delete_workspace_layout(name)
            .map_err(|e| e.to_string())?;
        self.remove_named_workbench_frame_graph_representation(name);
        self.workspace
            .workbench_session
            .node_last_active_workspace
            .retain(|_, (_, workspace_name)| workspace_name != name);
        for memberships in self
            .workspace
            .workbench_session
            .node_workspace_membership
            .values_mut()
        {
            memberships.remove(name);
        }
        self.workspace
            .workbench_session
            .node_workspace_membership
            .retain(|_, memberships| !memberships.is_empty());
        if self
            .workspace
            .workbench_session
            .current_workspace_name
            .as_deref()
            == Some(name)
        {
            self.workspace.workbench_session.current_workspace_name = None;
        }
        if self.workspace.graph_runtime.selected_frame_name.as_deref() == Some(name) {
            self.workspace.graph_runtime.selected_frame_name = None;
        }
        self.workspace
            .workbench_session
            .session_dismissed_frame_split_offers
            .remove(name);
        if self.pending_frame_context_target() == Some(name) {
            self.set_pending_frame_context_target(None);
        }
        self.workspace.graph_runtime.egui_state_dirty = true;
        crate::shell::desktop::runtime::registries::phase3_publish_workbench_projection_refresh_requested(
            "frame_snapshot_deleted",
        );
        Ok(())
    }

    /// Rename a persisted frame layout and keep graph/runtime references aligned.
    pub fn rename_workspace_layout(&mut self, from: &str, to: &str) -> Result<(), String> {
        let from = from.trim();
        let to = to.trim();
        if from.is_empty() || to.is_empty() {
            return Err("Frame names cannot be empty".to_string());
        }
        if from == to {
            return Ok(());
        }
        if Self::is_reserved_workspace_layout_name(from)
            || Self::is_reserved_workspace_layout_name(to)
        {
            return Err("Cannot rename reserved workspace layouts".to_string());
        }
        if self.load_workspace_layout_json(to).is_some() {
            return Err(format!("A frame named '{to}' already exists"));
        }

        let layout_json = self
            .load_workspace_layout_json(from)
            .ok_or_else(|| format!("No persisted frame named '{from}'"))?;
        let was_current_workspace = self
            .workspace
            .workbench_session
            .current_workspace_name
            .as_deref()
            == Some(from);
        let was_selected_frame =
            self.workspace.graph_runtime.selected_frame_name.as_deref() == Some(from);
        let had_session_split_dismissal = self
            .workspace
            .workbench_session
            .session_dismissed_frame_split_offers
            .contains(from);
        let mut bundle: crate::shell::desktop::ui::persistence_ops::PersistedWorkspace =
            serde_json::from_str(&layout_json).map_err(|e| e.to_string())?;
        bundle.name = to.to_string();
        let renamed_json = serde_json::to_string_pretty(&bundle).map_err(|e| e.to_string())?;
        self.save_workspace_layout_json(to, &renamed_json);
        self.delete_workspace_layout(from)?;

        for (_, workspace_name) in self
            .workspace
            .workbench_session
            .node_last_active_workspace
            .values_mut()
        {
            if workspace_name == from {
                *workspace_name = to.to_string();
            }
        }
        for memberships in self
            .workspace
            .workbench_session
            .node_workspace_membership
            .values_mut()
        {
            if memberships.remove(from) {
                memberships.insert(to.to_string());
            }
        }
        if was_current_workspace {
            self.workspace.workbench_session.current_workspace_name = Some(to.to_string());
        }
        if was_selected_frame {
            self.workspace.graph_runtime.selected_frame_name = Some(to.to_string());
        }
        if had_session_split_dismissal {
            self.workspace
                .workbench_session
                .session_dismissed_frame_split_offers
                .insert(to.to_string());
        }
        crate::shell::desktop::runtime::registries::phase3_publish_workbench_projection_refresh_requested(
            "frame_snapshot_renamed",
        );
        Ok(())
    }

    /// Delete the reserved session frame snapshot and reset hash baseline.
    pub fn clear_session_workspace_layout(&mut self) -> Result<(), String> {
        let mut names_to_delete = vec![Self::SESSION_WORKSPACE_LAYOUT_NAME.to_string()];
        for idx in 1..=5 {
            names_to_delete.push(Self::session_workspace_history_key(idx));
        }
        let store = self
            .services
            .persistence
            .as_mut()
            .ok_or_else(|| "Persistence is not enabled".to_string())?;
        for name in names_to_delete {
            let _ = store.delete_workspace_layout(&name);
        }
        self.workspace
            .workbench_session
            .last_session_workspace_layout_hash = None;
        self.workspace
            .workbench_session
            .last_session_workspace_layout_json = None;
        self.workspace.workbench_session.last_workspace_autosave_at = None;
        Ok(())
    }

    pub fn workspace_autosave_interval_secs(&self) -> u64 {
        self.workspace
            .workbench_session
            .workspace_autosave_interval
            .as_secs()
    }

    pub fn set_workspace_autosave_interval_secs(&mut self, secs: u64) -> Result<(), String> {
        if secs == 0 {
            return Err("Workspace autosave interval must be greater than zero".to_string());
        }
        self.workspace.workbench_session.workspace_autosave_interval = Duration::from_secs(secs);
        Ok(())
    }

    pub fn workspace_autosave_retention(&self) -> u8 {
        self.workspace
            .workbench_session
            .workspace_autosave_retention
    }

    pub fn set_workspace_autosave_retention(&mut self, count: u8) -> Result<(), String> {
        if count > 5 {
            return Err("Workspace autosave retention must be between 0 and 5".to_string());
        }
        if count
            < self
                .workspace
                .workbench_session
                .workspace_autosave_retention
            && let Some(store) = self.services.persistence.as_mut()
        {
            for idx in (count + 1)..=5 {
                let _ = store.delete_workspace_layout(&Self::session_workspace_history_key(idx));
            }
        }
        self.workspace
            .workbench_session
            .workspace_autosave_retention = count;
        Ok(())
    }

    /// Whether the current frame has unsaved graph changes.
    pub fn should_prompt_unsaved_workspace_save(&self) -> bool {
        self.workspace
            .workbench_session
            .workspace_has_unsaved_changes
    }

    /// Returns true once per unsaved-changes episode to enable one-shot warnings.
    pub fn consume_unsaved_workspace_prompt_warning(&mut self) -> bool {
        if !self.should_prompt_unsaved_workspace_save()
            || self
                .workspace
                .workbench_session
                .unsaved_workspace_prompt_warned
        {
            return false;
        }
        self.workspace
            .workbench_session
            .unsaved_workspace_prompt_warned = true;
        true
    }

    /// Persist a named full-graph snapshot.
    pub fn save_named_graph_snapshot(&mut self, name: &str) -> Result<(), String> {
        self.services
            .persistence
            .as_mut()
            .ok_or_else(|| "Persistence is not enabled".to_string())?
            .save_named_graph_snapshot(name, &self.workspace.domain.graph)
            .map_err(|e| e.to_string())
    }

    /// Load a named full-graph snapshot and reset runtime mappings.
    pub fn load_named_graph_snapshot(&mut self, name: &str) -> Result<(), String> {
        let graph = self
            .services
            .persistence
            .as_ref()
            .ok_or_else(|| "Persistence is not enabled".to_string())?
            .load_named_graph_snapshot(name)
            .ok_or_else(|| format!("Named graph snapshot '{name}' not found"))?;

        self.apply_loaded_graph(graph);
        Ok(())
    }

    /// Load a named full-graph snapshot without mutating runtime state.
    pub fn peek_named_graph_snapshot(&self, name: &str) -> Option<Graph> {
        self.services
            .persistence
            .as_ref()
            .and_then(|store| store.load_named_graph_snapshot(name))
    }

    /// Load autosaved latest graph snapshot/replay state.
    pub fn load_latest_graph_snapshot(&mut self) -> Result<(), String> {
        let graph = self
            .services
            .persistence
            .as_ref()
            .ok_or_else(|| "Persistence is not enabled".to_string())?
            .recover()
            .ok_or_else(|| "Latest graph snapshot is not available".to_string())?;

        self.apply_loaded_graph(graph);
        Ok(())
    }

    /// Load autosaved latest graph snapshot/replay state without mutating runtime state.
    pub fn peek_latest_graph_snapshot(&self) -> Option<Graph> {
        self.services
            .persistence
            .as_ref()
            .and_then(|store| store.recover())
    }

    /// Whether an autosaved latest graph snapshot/replay state can be restored.
    pub fn has_latest_graph_snapshot(&self) -> bool {
        self.services
            .persistence
            .as_ref()
            .and_then(|store| store.recover())
            .is_some()
    }

    pub(crate) fn apply_loaded_graph(&mut self, graph: Graph) {
        self.workspace.domain.graph = graph;
        self.reset_selection_state();
        self.workspace.graph_runtime.webview_to_node.clear();
        self.workspace.graph_runtime.node_to_webview.clear();
        self.workspace.graph_runtime.active_lru.clear();
        self.workspace.graph_runtime.warm_cache_lru.clear();
        self.workspace.graph_runtime.runtime_block_state.clear();
        self.workspace.graph_runtime.active_webview_nodes.clear();
        self.workspace
            .workbench_session
            .pending_app_commands
            .clear();
        self.workspace
            .workbench_session
            .pending_host_create_tokens
            .clear();
        self.clear_choose_frame_picker();
        self.set_pending_camera_command(None, Some(CameraCommand::Fit));
        self.clear_pending_wheel_zoom_delta();
        self.workspace
            .workbench_session
            .node_workspace_membership
            .clear();
        self.workspace.workbench_session.current_workspace_name = None;
        self.workspace.graph_runtime.views.clear();
        self.workspace.graph_runtime.graph_view_frames.clear();
        self.workspace.graph_runtime.graph_view_canvas_rects.clear();
        self.workspace.graph_runtime.workbench_navigation_geometry = None;
        self.workspace.domain.notes.clear();
        self.set_workspace_focused_view_with_transition(None);
        self.workspace
            .workbench_session
            .current_workspace_is_synthesized = false;
        self.workspace
            .workbench_session
            .workspace_has_unsaved_changes = false;
        self.workspace
            .workbench_session
            .unsaved_workspace_prompt_warned = false;
        self.workspace.domain.next_placeholder_id =
            Self::scan_max_placeholder_id(&self.workspace.domain.graph);
        self.workspace.graph_runtime.egui_state = None;
        self.workspace.graph_runtime.egui_state_dirty = true;
        self.workspace.graph_runtime.semantic_index.clear();
        self.workspace.graph_runtime.semantic_index_dirty = true;
        self.workspace
            .graph_runtime
            .active_graph_search_query
            .clear();
        self.workspace.graph_runtime.active_graph_search_match_count = 0;
        self.workspace.graph_runtime.active_graph_search_origin = GraphSearchOrigin::Manual;
        self.workspace
            .graph_runtime
            .active_graph_search_neighborhood_anchor = None;
        self.workspace
            .graph_runtime
            .active_graph_search_neighborhood_depth = 1;
        self.workspace.graph_runtime.graph_search_history.clear();
        self.workspace.graph_runtime.pinned_graph_search = None;
    }

    /// List named full-graph snapshots.
    pub fn list_named_graph_snapshot_names(&self) -> Vec<String> {
        self.services
            .persistence
            .as_ref()
            .map(|store| store.list_named_graph_snapshot_names())
            .unwrap_or_default()
    }

    /// Delete a named full-graph snapshot.
    pub fn delete_named_graph_snapshot(&mut self, name: &str) -> Result<(), String> {
        self.services
            .persistence
            .as_mut()
            .ok_or_else(|| "Persistence is not enabled".to_string())?
            .delete_named_graph_snapshot(name)
            .map_err(|e| e.to_string())
    }

    /// Switch persistence backing store at runtime and reload graph state from it.
    pub fn switch_persistence_dir(&mut self, data_dir: PathBuf) -> Result<(), String> {
        let store = GraphStore::open(data_dir).map_err(|e| e.to_string())?;
        let graph = store.recover().unwrap_or_else(Graph::new);
        let next_placeholder_id = Self::scan_max_placeholder_id(&graph);

        self.workspace.domain.graph = graph;
        self.services.persistence = Some(store);
        self.reset_selection_state();
        self.workspace.graph_runtime.webview_to_node.clear();
        self.workspace.graph_runtime.node_to_webview.clear();
        self.workspace.graph_runtime.active_lru.clear();
        self.workspace.graph_runtime.warm_cache_lru.clear();
        self.workspace.graph_runtime.runtime_block_state.clear();
        self.workspace.graph_runtime.active_webview_nodes.clear();
        self.workspace
            .workbench_session
            .pending_app_commands
            .clear();
        self.clear_choose_frame_picker();
        self.set_pending_camera_command(None, Some(CameraCommand::Fit));
        self.clear_pending_wheel_zoom_delta();
        self.workspace.domain.notes.clear();
        self.workspace.graph_runtime.views.clear();
        self.workspace.graph_runtime.graph_view_frames.clear();
        self.workspace.graph_runtime.graph_view_canvas_rects.clear();
        self.workspace.graph_runtime.workbench_navigation_geometry = None;
        self.set_workspace_focused_view_with_transition(None);
        self.workspace.domain.next_placeholder_id = next_placeholder_id;
        self.workspace.graph_runtime.egui_state = None;
        self.workspace.graph_runtime.egui_state_dirty = true;
        self.workspace.graph_runtime.semantic_index.clear();
        self.workspace.graph_runtime.semantic_index_dirty = true;
        self.workspace
            .graph_runtime
            .active_graph_search_query
            .clear();
        self.workspace.graph_runtime.active_graph_search_match_count = 0;
        self.workspace.graph_runtime.active_graph_search_origin = GraphSearchOrigin::Manual;
        self.workspace
            .graph_runtime
            .active_graph_search_neighborhood_anchor = None;
        self.workspace
            .graph_runtime
            .active_graph_search_neighborhood_depth = 1;
        self.workspace.graph_runtime.graph_search_history.clear();
        self.workspace.graph_runtime.pinned_graph_search = None;
        self.workspace
            .workbench_session
            .last_session_workspace_layout_hash = None;
        self.workspace
            .workbench_session
            .last_session_workspace_layout_json = None;
        self.workspace.workbench_session.last_workspace_autosave_at = None;
        self.workspace.workbench_session.workspace_activation_seq = 0;
        self.workspace
            .workbench_session
            .node_last_active_workspace
            .clear();
        self.workspace
            .workbench_session
            .node_workspace_membership
            .clear();
        self.workspace.workbench_session.current_workspace_name = None;
        self.workspace
            .workbench_session
            .current_workspace_is_synthesized = false;
        self.workspace
            .workbench_session
            .workspace_has_unsaved_changes = false;
        self.workspace
            .workbench_session
            .unsaved_workspace_prompt_warned = false;
        self.workspace.graph_runtime.is_interacting = false;
        self.workspace
            .graph_runtime
            .physics_running_before_interaction = None;
        self.workspace.chrome_ui.toast_anchor_preference = ToastAnchorPreference::BottomRight;
        self.workspace.chrome_ui.command_palette_shortcut = CommandPaletteShortcut::F2;
        self.workspace.chrome_ui.help_panel_shortcut = HelpPanelShortcut::F1OrQuestion;
        self.workspace.chrome_ui.radial_menu_shortcut = RadialMenuShortcut::F3;
        self.workspace.chrome_ui.omnibar_preferred_scope = OmnibarPreferredScope::Auto;
        self.workspace.chrome_ui.omnibar_non_at_order =
            OmnibarNonAtOrderPreset::ContextualThenProviderThenGlobal;
        self.workspace.chrome_ui.wry_enabled = true;
        self.workspace.chrome_ui.navigator_sidebar_side_preference =
            super::settings_persistence::NavigatorSidebarSidePreference::Left;
        self.workspace.chrome_ui.workbench_host_pinned = false;
        self.workspace.graph_runtime.selected_tab_nodes.clear();
        self.workspace.graph_runtime.tab_selection_anchor = None;
        self.load_persisted_ui_settings();
        Ok(())
    }
}

