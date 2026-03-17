use super::*;

impl GraphBrowserApp {
    /// Request camera fit on next render frame.
    pub fn request_fit_to_screen(&mut self) {
        self.request_camera_command(CameraCommand::Fit);
    }

    fn camera_lock_target_view(&self) -> Option<GraphViewId> {
        self.resolve_camera_target_view()
    }

    pub fn camera_position_fit_locked(&self) -> bool {
        self.camera_lock_target_view()
            .and_then(|view_id| self.workspace.graph_runtime.views.get(&view_id))
            .is_some_and(|view| view.position_fit_locked)
    }

    pub fn camera_zoom_fit_locked(&self) -> bool {
        self.camera_lock_target_view()
            .and_then(|view_id| self.workspace.graph_runtime.views.get(&view_id))
            .is_some_and(|view| view.zoom_fit_locked)
    }

    pub fn camera_fit_locked(&self) -> bool {
        self.camera_position_fit_locked() && self.camera_zoom_fit_locked()
    }

    pub fn set_camera_position_fit_locked(&mut self, locked: bool) {
        let Some(view_id) = self.camera_lock_target_view() else {
            return;
        };
        let was_locked = self
            .workspace
            .graph_runtime
            .views
            .get(&view_id)
            .is_some_and(|view| view.position_fit_locked);
        if was_locked == locked {
            return;
        }
        if let Some(view) = self.workspace.graph_runtime.views.get_mut(&view_id) {
            view.position_fit_locked = locked;
        }
        if locked {
            self.workspace.graph_runtime.drag_release_frames_remaining = 0;
            self.request_fit_to_screen();
        } else if matches!(
            self.pending_app_command(|command| {
                matches!(command, AppCommand::CameraCommand { .. })
            }),
            Some(AppCommand::CameraCommand {
                command: CameraCommand::Fit,
                ..
            })
        ) {
            self.clear_pending_camera_command();
        }
        log::debug!(
            "camera_position_fit_lock(view={:?}): {} -> {} (pending_camera={:?}, pending_target={:?}, physics_running={}, interacting={})",
            view_id,
            was_locked,
            locked,
            self.pending_camera_command(),
            self.pending_camera_command_target_raw(),
            self.workspace.graph_runtime.physics.base.is_running,
            self.workspace.graph_runtime.is_interacting,
        );
    }

    pub fn set_camera_zoom_fit_locked(&mut self, locked: bool) {
        let Some(view_id) = self.camera_lock_target_view() else {
            return;
        };
        let was_locked = self
            .workspace
            .graph_runtime
            .views
            .get(&view_id)
            .is_some_and(|view| view.zoom_fit_locked);
        if was_locked == locked {
            return;
        }
        if let Some(view) = self.workspace.graph_runtime.views.get_mut(&view_id) {
            view.zoom_fit_locked = locked;
        }
        if locked {
            self.request_fit_to_screen();
        }
        log::debug!(
            "camera_zoom_fit_lock(view={:?}): {} -> {} (pending_camera={:?}, pending_target={:?}, physics_running={}, interacting={})",
            view_id,
            was_locked,
            locked,
            self.pending_camera_command(),
            self.pending_camera_command_target_raw(),
            self.workspace.graph_runtime.physics.base.is_running,
            self.workspace.graph_runtime.is_interacting,
        );
    }

    pub fn set_camera_fit_locked(&mut self, locked: bool) {
        self.set_camera_position_fit_locked(locked);
        self.set_camera_zoom_fit_locked(locked);
    }

    fn next_graph_view_slot_name(&self) -> String {
        let count = self.workspace.graph_runtime.graph_view_layout_manager.slots.len() + 1;
        format!("Graph View {count}")
    }

    fn graph_view_slot_position_occupied(
        &self,
        row: i32,
        col: i32,
        except_view: Option<GraphViewId>,
    ) -> bool {
        self.workspace
            .graph_runtime
            .graph_view_layout_manager
            .slots
            .values()
            .any(|slot| {
                !slot.archived
                    && Some(slot.view_id) != except_view
                    && slot.row == row
                    && slot.col == col
            })
    }

    fn next_free_graph_view_slot_position(&self) -> (i32, i32) {
        for row in 0..64 {
            for col in 0..64 {
                if !self.graph_view_slot_position_occupied(row, col, None) {
                    return (row, col);
                }
            }
        }
        (0, 0)
    }

    fn ensure_graph_view_slot_exists(&mut self, view_id: GraphViewId) {
        if self
            .workspace
            .graph_runtime
            .graph_view_layout_manager
            .slots
            .contains_key(&view_id)
        {
            return;
        }

        let name = self
            .workspace
            .graph_runtime
            .views
            .get(&view_id)
            .map(|view| view.name.clone())
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| self.next_graph_view_slot_name());
        let (row, col) = self.next_free_graph_view_slot_position();
        self.workspace.graph_runtime.graph_view_layout_manager.slots.insert(
            view_id,
            GraphViewSlot {
                view_id,
                name,
                row,
                col,
                archived: false,
            },
        );
    }

    pub fn ensure_graph_view_registered(&mut self, view_id: GraphViewId) {
        let had_view = self.workspace.graph_runtime.views.contains_key(&view_id);
        let had_slot = self
            .workspace
            .graph_runtime
            .graph_view_layout_manager
            .slots
            .contains_key(&view_id);
        if !self.workspace.graph_runtime.views.contains_key(&view_id) {
            let name = self.next_graph_view_slot_name();
            let mut state = GraphViewState::new_with_id(view_id, name);
            state.local_simulation = Some(LocalSimulation {
                positions: self
                    .workspace
                    .domain
                    .graph
                    .nodes()
                    .map(|(key, node)| (key, node.projected_position()))
                    .collect(),
            });
            self.workspace.graph_runtime.views.insert(view_id, state);
        } else if self.workspace.graph_runtime.views[&view_id].local_simulation.is_none() {
            let positions = self
                .workspace
                .domain
                .graph
                .nodes()
                .map(|(key, node)| (key, node.projected_position()))
                .collect();
            if let Some(view) = self.workspace.graph_runtime.views.get_mut(&view_id) {
                view.local_simulation = Some(LocalSimulation { positions });
            }
        }

        self.ensure_graph_view_slot_exists(view_id);
        if !had_view || !had_slot {
            self.persist_graph_view_layout_manager_state();
        }
    }

    pub(crate) fn persist_graph_view_layout_manager_state(&mut self) {
        let persisted = PersistedGraphViewLayoutManager {
            version: PersistedGraphViewLayoutManager::VERSION,
            active: self.workspace.graph_runtime.graph_view_layout_manager.active,
            slots: self
                .workspace
                .graph_runtime
                .graph_view_layout_manager
                .slots
                .values()
                .cloned()
                .collect(),
        };
        if let Ok(json) = serde_json::to_string(&persisted) {
            self.save_workspace_layout_json(Self::SETTINGS_GRAPH_VIEW_LAYOUT_MANAGER_NAME, &json);
        }
    }

    pub(crate) fn load_graph_view_layout_manager_state(&mut self) {
        let Some(raw) =
            self.load_workspace_layout_json(Self::SETTINGS_GRAPH_VIEW_LAYOUT_MANAGER_NAME)
        else {
            return;
        };
        let Ok(persisted) = serde_json::from_str::<PersistedGraphViewLayoutManager>(&raw) else {
            warn!("Ignoring invalid persisted graph-view layout manager state payload");
            return;
        };
        if persisted.version != PersistedGraphViewLayoutManager::VERSION {
            warn!(
                "Ignoring unsupported graph-view layout manager state version: {}",
                persisted.version
            );
            return;
        }

        let mut slots = HashMap::new();
        for slot in persisted.slots {
            slots.insert(slot.view_id, slot);
        }
        self.workspace.graph_runtime.graph_view_layout_manager.active = persisted.active;
        self.workspace.graph_runtime.graph_view_layout_manager.slots = slots;
    }

    pub(crate) fn create_graph_view_slot(
        &mut self,
        anchor_view: Option<GraphViewId>,
        direction: GraphViewLayoutDirection,
        open_mode: Option<PendingTileOpenMode>,
    ) {
        let view_id = GraphViewId::new();
        let mut state = GraphViewState::new_with_id(view_id, self.next_graph_view_slot_name());
        state.local_simulation = Some(LocalSimulation {
            positions: self
                .workspace
                .domain
                .graph
                .nodes()
                .map(|(key, node)| (key, node.projected_position()))
                .collect(),
        });
        self.workspace.graph_runtime.views.insert(view_id, state.clone());

        let (row, col) = if let Some(anchor_id) = anchor_view {
            if let Some(anchor_slot) = self
                .workspace
                .graph_runtime
                .graph_view_layout_manager
                .slots
                .get(&anchor_id)
            {
                let (target_row, target_col) = match direction {
                    GraphViewLayoutDirection::Up => (anchor_slot.row - 1, anchor_slot.col),
                    GraphViewLayoutDirection::Down => (anchor_slot.row + 1, anchor_slot.col),
                    GraphViewLayoutDirection::Left => (anchor_slot.row, anchor_slot.col - 1),
                    GraphViewLayoutDirection::Right => (anchor_slot.row, anchor_slot.col + 1),
                };
                if self.graph_view_slot_position_occupied(target_row, target_col, None) {
                    self.next_free_graph_view_slot_position()
                } else {
                    (target_row, target_col)
                }
            } else {
                self.next_free_graph_view_slot_position()
            }
        } else {
            self.next_free_graph_view_slot_position()
        };

        self.workspace.graph_runtime.graph_view_layout_manager.slots.insert(
            view_id,
            GraphViewSlot {
                view_id,
                name: state.name,
                row,
                col,
                archived: false,
            },
        );

        if let Some(mode) = open_mode {
            self.enqueue_workbench_intent(WorkbenchIntent::OpenGraphViewPane { view_id, mode });
        }
        self.persist_graph_view_layout_manager_state();
    }

    pub(crate) fn rename_graph_view_slot(&mut self, view_id: GraphViewId, name: String) {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return;
        }
        if let Some(slot) = self
            .workspace
            .graph_runtime
            .graph_view_layout_manager
            .slots
            .get_mut(&view_id)
        {
            slot.name = trimmed.to_string();
            if let Some(view) = self.workspace.graph_runtime.views.get_mut(&view_id) {
                view.name = slot.name.clone();
            }
            self.persist_graph_view_layout_manager_state();
        }
    }

    pub(crate) fn refresh_registry_backed_view_lenses(&mut self) -> usize {
        let refreshes: Vec<(GraphViewId, LensConfig)> = self
            .workspace
            .graph_runtime
            .views
            .iter()
            .filter_map(|(&view_id, view)| {
                let requested = view
                    .lens
                    .lens_id
                    .as_deref()
                    .filter(|lens_id| !lens_id.trim().is_empty())
                    .map(str::to_string)
                    .or_else(|| {
                        view.lens
                            .name
                            .starts_with("lens:")
                            .then(|| view.lens.name.to_ascii_lowercase())
                    })?;
                Some((
                    view_id,
                    crate::shell::desktop::runtime::registries::phase2_resolve_lens(&requested),
                ))
            })
            .collect();

        for (view_id, lens) in &refreshes {
            if let Some(view) = self.workspace.graph_runtime.views.get_mut(view_id) {
                let layout_algorithm_id = view.lens.layout_algorithm_id.clone();
                view.lens = lens.clone();
                view.lens.layout_algorithm_id = layout_algorithm_id;
            }
        }

        refreshes.len()
    }

    pub(crate) fn move_graph_view_slot(&mut self, view_id: GraphViewId, row: i32, col: i32) {
        if self.graph_view_slot_position_occupied(row, col, Some(view_id)) {
            return;
        }
        if let Some(slot) = self
            .workspace
            .graph_runtime
            .graph_view_layout_manager
            .slots
            .get_mut(&view_id)
        {
            slot.row = row;
            slot.col = col;
            self.persist_graph_view_layout_manager_state();
        }
    }

    pub(crate) fn archive_graph_view_slot(&mut self, view_id: GraphViewId) {
        if let Some(slot) = self
            .workspace
            .graph_runtime
            .graph_view_layout_manager
            .slots
            .get_mut(&view_id)
        {
            slot.archived = true;
            if self.workspace.graph_runtime.focused_view == Some(view_id) {
                self.set_workspace_focused_view_with_transition(None);
            }
            self.persist_graph_view_layout_manager_state();
        }
    }

    pub(crate) fn restore_graph_view_slot(&mut self, view_id: GraphViewId, row: i32, col: i32) {
        self.ensure_graph_view_registered(view_id);
        let (next_row, next_col) =
            if self.graph_view_slot_position_occupied(row, col, Some(view_id)) {
                self.next_free_graph_view_slot_position()
            } else {
                (row, col)
            };
        if let Some(slot) = self
            .workspace
            .graph_runtime
            .graph_view_layout_manager
            .slots
            .get_mut(&view_id)
        {
            slot.archived = false;
            slot.row = next_row;
            slot.col = next_col;
            self.persist_graph_view_layout_manager_state();
        }
    }

    pub(crate) fn route_graph_view_to_workbench(
        &mut self,
        view_id: GraphViewId,
        mode: PendingTileOpenMode,
    ) {
        self.ensure_graph_view_registered(view_id);
        if self
            .workspace
            .graph_runtime
            .graph_view_layout_manager
            .slots
            .get(&view_id)
            .is_some_and(|slot| slot.archived)
        {
            return;
        }
        self.enqueue_workbench_intent(WorkbenchIntent::OpenGraphViewPane { view_id, mode });
    }

    pub fn reconcile_workspace_graph_views(
        &mut self,
        live_graph_views: &HashSet<GraphViewId>,
        fallback_focused_view: Option<GraphViewId>,
    ) {
        let registered_views: HashSet<GraphViewId> = self
            .workspace
            .graph_runtime
            .graph_view_layout_manager
            .slots
            .keys()
            .copied()
            .collect();
        self.workspace.graph_runtime.views.retain(|view_id, _| {
            live_graph_views.contains(view_id) || registered_views.contains(view_id)
        });
        self.workspace
            .graph_runtime
            .graph_view_frames
            .retain(|view_id, _| live_graph_views.contains(view_id));
        self.retain_selection_scopes_for_graph_views(live_graph_views, &registered_views);

        if self
            .workspace
            .graph_runtime
            .focused_view
            .is_some_and(|view_id| !live_graph_views.contains(&view_id))
        {
            self.set_workspace_focused_view_with_transition(
                fallback_focused_view.filter(|view_id| live_graph_views.contains(view_id)),
            );
        }

        let _ = self.take_pending_app_command(|command| {
            matches!(
                command,
                AppCommand::CameraCommand {
                    target_view: Some(target_view),
                    ..
                } if !live_graph_views.contains(target_view)
            )
        });

        let _ = self.take_pending_app_command(|command| {
            matches!(
                command,
                AppCommand::KeyboardZoom { target_view, .. }
                    if !live_graph_views.contains(target_view)
            )
        });

        let _ = self.take_pending_app_command(|command| {
            matches!(
                command,
                AppCommand::WheelZoom { target_view, .. }
                    if !live_graph_views.contains(target_view)
            )
        });
    }

    fn resolve_camera_target_view(&self) -> Option<GraphViewId> {
        let focused = self
            .workspace
            .graph_runtime
            .focused_view
            .filter(|id| self.workspace.graph_runtime.views.contains_key(id));
        if focused.is_some() {
            return focused;
        }

        let mut rendered_views = self
            .workspace
            .graph_runtime
            .graph_view_frames
            .keys()
            .copied()
            .filter(|id| self.workspace.graph_runtime.views.contains_key(id));
        let rendered_first = rendered_views.next();
        if let Some(rendered_only) = rendered_first
            && rendered_views.next().is_none()
        {
            return Some(rendered_only);
        }

        if self.workspace.graph_runtime.views.len() == 1 {
            return self.workspace.graph_runtime.views.keys().next().copied();
        }

        None
    }

    pub fn request_camera_command(&mut self, command: CameraCommand) {
        let target_view = self.resolve_camera_target_view();
        if target_view.is_none() {
            emit_event(DiagnosticEvent::MessageReceived {
                channel_id: CHANNEL_UI_GRAPH_CAMERA_REQUEST_BLOCKED,
                latency_us: 0,
            });
            return;
        }
        self.request_camera_command_for_view(target_view, command);
    }

    pub fn request_camera_command_for_view(
        &mut self,
        target_view: Option<GraphViewId>,
        command: CameraCommand,
    ) {
        if let Some(target_view) = target_view
            && !self.workspace.graph_runtime.views.contains_key(&target_view)
        {
            emit_event(DiagnosticEvent::MessageReceived {
                channel_id: CHANNEL_UI_GRAPH_CAMERA_COMMAND_BLOCKED_MISSING_TARGET_VIEW,
                latency_us: 0,
            });
            return;
        }

        self.set_pending_camera_command(target_view, Some(command));
    }

    pub fn take_pending_keyboard_zoom_request(
        &mut self,
        view_id: GraphViewId,
    ) -> Option<KeyboardZoomRequest> {
        match self.take_pending_app_command(|command| {
            matches!(
                command,
                AppCommand::KeyboardZoom { target_view, .. } if *target_view == view_id
            )
        })? {
            AppCommand::KeyboardZoom { request, .. } => Some(request),
            _ => None,
        }
    }

    pub fn restore_pending_keyboard_zoom_request(
        &mut self,
        target_view: GraphViewId,
        request: KeyboardZoomRequest,
    ) {
        self.set_pending_keyboard_zoom_request(Some(target_view), Some(request));
    }

    pub(crate) fn queue_keyboard_zoom_request(&mut self, request: KeyboardZoomRequest) {
        let Some(target_view) = self.resolve_camera_target_view() else {
            emit_event(DiagnosticEvent::MessageReceived {
                channel_id: CHANNEL_UI_GRAPH_KEYBOARD_ZOOM_BLOCKED,
                latency_us: 0,
            });
            return;
        };

        self.set_pending_keyboard_zoom_request(Some(target_view), Some(request));
    }

    pub fn pending_camera_command(&self) -> Option<CameraCommand> {
        match self
            .pending_app_command(|command| matches!(command, AppCommand::CameraCommand { .. }))?
        {
            AppCommand::CameraCommand { command, .. } => Some(*command),
            _ => None,
        }
    }

    pub fn pending_camera_command_target_raw(&self) -> Option<GraphViewId> {
        match self
            .pending_app_command(|command| matches!(command, AppCommand::CameraCommand { .. }))?
        {
            AppCommand::CameraCommand { target_view, .. } => *target_view,
            _ => None,
        }
    }

    pub fn pending_camera_command_target(&self) -> Option<GraphViewId> {
        self.pending_camera_command_target_raw()
            .filter(|id| self.workspace.graph_runtime.views.contains_key(id))
    }

    pub fn clear_pending_camera_command(&mut self) {
        self.set_pending_camera_command(None, None);
    }

    pub fn queue_pending_wheel_zoom_delta(
        &mut self,
        target_view: GraphViewId,
        delta: f32,
        anchor_screen: Option<(f32, f32)>,
    ) {
        self.set_pending_wheel_zoom_delta(Some(target_view), Some(delta), anchor_screen);
    }

    pub fn pending_wheel_zoom_delta(&self, view_id: GraphViewId) -> f32 {
        match self.pending_app_command(|command| matches!(command, AppCommand::WheelZoom { .. })) {
            Some(AppCommand::WheelZoom {
                target_view, delta, ..
            }) if *target_view == view_id => *delta,
            _ => 0.0,
        }
    }

    pub fn pending_wheel_zoom_anchor_screen(&self, view_id: GraphViewId) -> Option<(f32, f32)> {
        match self.pending_app_command(|command| matches!(command, AppCommand::WheelZoom { .. })) {
            Some(AppCommand::WheelZoom {
                target_view,
                anchor_screen,
                ..
            }) if *target_view == view_id => *anchor_screen,
            _ => None,
        }
    }

    pub fn clear_pending_wheel_zoom_delta(&mut self) {
        self.set_pending_wheel_zoom_delta(None, None, None);
    }
}
