use super::*;
use crate::graph::{ArrangementSubKind, NodeKey, RelationSelector, SemanticSubKind};

fn default_edge_projection_selectors() -> Vec<RelationSelector> {
    vec![
        RelationSelector::Semantic(SemanticSubKind::UserGrouped),
        RelationSelector::Arrangement(ArrangementSubKind::FrameMember),
    ]
}

fn sanitize_edge_projection_selectors(
    selectors: impl IntoIterator<Item = RelationSelector>,
) -> Vec<RelationSelector> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for selector in selectors {
        if seen.insert(selector) {
            out.push(selector);
        }
    }
    out
}

fn sorted_unique_node_keys(nodes: impl IntoIterator<Item = NodeKey>) -> Vec<NodeKey> {
    let mut out: Vec<NodeKey> = nodes.into_iter().collect();
    out.sort_by_key(|key| key.index());
    out.dedup();
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeProjectionSource {
    WorkbenchDefault,
    GraphViewOverride,
    SelectionOverride,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EdgeProjectionState {
    #[serde(default = "default_edge_projection_selectors")]
    pub active_selectors: Vec<RelationSelector>,
}

impl Default for EdgeProjectionState {
    fn default() -> Self {
        Self {
            active_selectors: default_edge_projection_selectors(),
        }
    }
}

impl EdgeProjectionState {
    pub fn new(selectors: Vec<RelationSelector>) -> Self {
        Self {
            active_selectors: sanitize_edge_projection_selectors(selectors),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionEdgeProjectionOverride {
    pub selected_nodes: Vec<NodeKey>,
    pub selectors: Vec<RelationSelector>,
}

impl SelectionEdgeProjectionOverride {
    pub fn new(selected_nodes: Vec<NodeKey>, selectors: Vec<RelationSelector>) -> Option<Self> {
        let selected_nodes = sorted_unique_node_keys(selected_nodes);
        if selected_nodes.is_empty() {
            return None;
        }
        Some(Self {
            selected_nodes,
            selectors: sanitize_edge_projection_selectors(selectors),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedEdgeProjection {
    pub source: EdgeProjectionSource,
    pub selectors: Vec<RelationSelector>,
}

/// Camera state for zoom bounds enforcement
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Camera {
    pub zoom_min: f32,
    pub zoom_max: f32,
    pub current_zoom: f32,
}

impl Camera {
    pub fn new() -> Self {
        Self {
            zoom_min: 0.1,
            zoom_max: 10.0,
            current_zoom: 0.8,
        }
    }

    /// Clamp a zoom value to the allowed range
    pub fn clamp(&self, zoom: f32) -> f32 {
        zoom.clamp(self.zoom_min, self.zoom_max)
    }
}

impl Default for Camera {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct GraphViewFrame {
    pub zoom: f32,
    pub pan_x: f32,
    pub pan_y: f32,
}

/// Unique identifier for a graph view pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct GraphViewId(uuid::Uuid);

impl GraphViewId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }

    pub(crate) fn from_uuid(id: uuid::Uuid) -> Self {
        Self(id)
    }

    pub fn as_uuid(self) -> uuid::Uuid {
        self.0
    }
}

impl Default for GraphViewId {
    fn default() -> Self {
        Self::new()
    }
}

impl GraphBrowserApp {
    pub fn workbench_edge_projection(&self) -> &EdgeProjectionState {
        &self.workspace.workbench_session.edge_projection
    }

    pub fn graph_view_edge_projection_override(
        &self,
        view_id: GraphViewId,
    ) -> Option<&EdgeProjectionState> {
        self.workspace
            .graph_runtime
            .views
            .get(&view_id)
            .and_then(|view| view.edge_projection_override.as_ref())
    }

    fn selection_scope_for_view(view_id: Option<GraphViewId>) -> SelectionScope {
        view_id
            .map(SelectionScope::View)
            .unwrap_or(SelectionScope::Unfocused)
    }

    fn selection_node_keys_for_scope(&self, scope: SelectionScope) -> Vec<NodeKey> {
        let mut nodes: Vec<NodeKey> = self
            .workspace
            .graph_runtime
            .selection_by_scope
            .get(&scope)
            .into_iter()
            .flat_map(|selection| selection.iter().copied())
            .collect();
        nodes.sort_by_key(|key| key.index());
        nodes.dedup();
        nodes
    }

    pub(crate) fn clear_selection_edge_projection_override_for_scope(
        &mut self,
        scope: SelectionScope,
    ) {
        self.workspace
            .graph_runtime
            .selection_edge_projections
            .remove(&scope);
    }

    pub(crate) fn sync_selection_edge_projection_override_for_scope(
        &mut self,
        scope: SelectionScope,
    ) {
        let Some(existing) = self
            .workspace
            .graph_runtime
            .selection_edge_projections
            .get(&scope)
            .cloned()
        else {
            return;
        };
        let current_nodes = self.selection_node_keys_for_scope(scope);
        if current_nodes != existing.selected_nodes {
            self.clear_selection_edge_projection_override_for_scope(scope);
        }
    }

    pub(crate) fn sync_all_selection_edge_projection_overrides(&mut self) {
        let scopes: Vec<SelectionScope> = self
            .workspace
            .graph_runtime
            .selection_edge_projections
            .keys()
            .copied()
            .collect();
        for scope in scopes {
            self.sync_selection_edge_projection_override_for_scope(scope);
        }
    }

    pub fn set_workbench_edge_projection(&mut self, selectors: Vec<RelationSelector>) {
        self.workspace.workbench_session.edge_projection = EdgeProjectionState::new(selectors);
        self.workspace.graph_runtime.egui_state_dirty = true;
    }

    pub fn set_graph_view_edge_projection_override(
        &mut self,
        view_id: GraphViewId,
        selectors: Option<Vec<RelationSelector>>,
    ) {
        let Some(view) = self.workspace.graph_runtime.views.get_mut(&view_id) else {
            return;
        };
        view.edge_projection_override = selectors.map(EdgeProjectionState::new);
        self.workspace.graph_runtime.egui_state_dirty = true;
    }

    pub fn set_selection_edge_projection_override(
        &mut self,
        view_id: Option<GraphViewId>,
        selectors: Option<Vec<RelationSelector>>,
    ) {
        let scope = Self::selection_scope_for_view(view_id);
        match selectors.and_then(|selectors| {
            SelectionEdgeProjectionOverride::new(
                self.selection_node_keys_for_scope(scope),
                selectors,
            )
        }) {
            Some(override_state) => {
                self.workspace
                    .graph_runtime
                    .selection_edge_projections
                    .insert(scope, override_state);
            }
            None => {
                self.clear_selection_edge_projection_override_for_scope(scope);
            }
        }
        self.workspace.graph_runtime.egui_state_dirty = true;
    }

    pub fn resolved_edge_projection_for_nodes(
        &self,
        nodes: &[NodeKey],
        view_id: Option<GraphViewId>,
    ) -> ResolvedEdgeProjection {
        let scope = Self::selection_scope_for_view(view_id);
        let requested_nodes = sorted_unique_node_keys(nodes.iter().copied());
        if !requested_nodes.is_empty()
            && let Some(selection_override) = self
                .workspace
                .graph_runtime
                .selection_edge_projections
                .get(&scope)
        {
            let selection_nodes: HashSet<NodeKey> =
                selection_override.selected_nodes.iter().copied().collect();
            if requested_nodes
                .iter()
                .all(|node| selection_nodes.contains(node))
            {
                return ResolvedEdgeProjection {
                    source: EdgeProjectionSource::SelectionOverride,
                    selectors: selection_override.selectors.clone(),
                };
            }
        }

        if let Some(view_id) = view_id
            && let Some(view_override) = self.graph_view_edge_projection_override(view_id)
        {
            return ResolvedEdgeProjection {
                source: EdgeProjectionSource::GraphViewOverride,
                selectors: view_override.active_selectors.clone(),
            };
        }

        ResolvedEdgeProjection {
            source: EdgeProjectionSource::WorkbenchDefault,
            selectors: self.workbench_edge_projection().active_selectors.clone(),
        }
    }

    pub fn resolved_edge_projection_for_seed(
        &self,
        seed: NodeKey,
        view_id: Option<GraphViewId>,
    ) -> ResolvedEdgeProjection {
        self.resolved_edge_projection_for_nodes(&[seed], view_id)
    }

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
        let count = self
            .workspace
            .graph_runtime
            .graph_view_layout_manager
            .slots
            .len()
            + 1;
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
        self.workspace
            .graph_runtime
            .graph_view_layout_manager
            .slots
            .insert(
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
        } else if self.workspace.graph_runtime.views[&view_id]
            .local_simulation
            .is_none()
        {
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
            active: self
                .workspace
                .graph_runtime
                .graph_view_layout_manager
                .active,
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
        self.workspace
            .graph_runtime
            .graph_view_layout_manager
            .active = persisted.active;
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
        self.workspace
            .graph_runtime
            .views
            .insert(view_id, state.clone());

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

        self.workspace
            .graph_runtime
            .graph_view_layout_manager
            .slots
            .insert(
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
            && !self
                .workspace
                .graph_runtime
                .views
                .contains_key(&target_view)
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LocalSimulation {
    pub positions: HashMap<NodeKey, Point2D<f32>>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LensConfig {
    pub name: String,
    pub lens_id: Option<String>,
    pub physics: PhysicsProfile,
    pub layout: LayoutMode,
    #[serde(default = "crate::app::graph_layout::default_free_layout_algorithm_id")]
    pub layout_algorithm_id: String,
    #[serde(default, deserialize_with = "deserialize_optional_theme_data")]
    pub theme: Option<ThemeData>,
    /// Structured faceted filter expression (spec: faceted_filter_surface_spec.md §5.3).
    ///
    /// `None` means no active filter (all nodes visible).
    /// Replaces the legacy `filters: Vec<String>` field.
    #[serde(default)]
    pub filter_expr: Option<crate::model::graph::filter::FacetExpr>,
    /// Legacy flat-string filter list — retained for backward-compatible deserialization only.
    /// Do not write new code against this field; use `filter_expr` instead.
    #[serde(default, rename = "filters", skip_serializing_if = "Vec::is_empty")]
    pub filters_legacy: Vec<String>,
    #[serde(skip, default)]
    pub overlay_descriptor: Option<crate::registries::atomic::lens::LensOverlayDescriptor>,
}

impl Default for LensConfig {
    fn default() -> Self {
        Self {
            name: "Default".to_string(),
            lens_id: None,
            physics: PhysicsProfile::default(),
            layout: LayoutMode::Free,
            layout_algorithm_id: crate::app::graph_layout::default_free_layout_algorithm_id(),
            theme: None,
            filter_expr: None,
            filters_legacy: Vec::new(),
            overlay_descriptor: None,
        }
    }
}

/// How z-coordinates are assigned to nodes when a graph view is in a 3D mode.
///
/// `ZSource` is part of `GraphViewState` — it is a per-view configuration.
/// z-positions are ephemeral: they are recomputed from this source + node metadata on
/// every 2D→3D switch and are never persisted independently.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, Default)]
pub enum ZSource {
    /// All nodes coplanar — soft 3D visual effect only.
    #[default]
    Zero,
    /// Recent nodes float to front; `max_depth` controls the maximum z offset.
    Recency { max_depth: f32 },
    /// Root nodes at z=0; deeper BFS nodes further back; `scale` controls layer spacing.
    BfsDepth { scale: f32 },
    /// UDC main class determines z layer; `scale` controls layer spacing.
    UdcLevel { scale: f32 },
    /// Per-node z override sourced from node metadata.
    Manual,
}

/// Sub-mode for a 3D graph view.
///
/// Ordered by implementation complexity — `TwoPointFive` is purely visual and the
/// lowest-cost starting point; `Standard` is the highest-fidelity, highest-complexity mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ThreeDMode {
    /// 2.5D: fixed top-down perspective; z is visual-only depth offset.
    /// Navigation remains 2D (pan/zoom). No camera tilt. Mobile-compatible.
    TwoPointFive,
    /// Isometric: quantized z layers, fixed-angle projection.
    /// Layer separation reveals hierarchical/temporal structure.
    Isometric,
    /// Standard 3D: reorientable arcball camera, arbitrary z.
    /// Highest fidelity; most complex interaction model.
    Standard,
}

/// Dimension mode for a graph view pane.
///
/// Owned by `GraphViewState` and persisted with the view snapshot.
/// The z-positions cache (`z_positions: HashMap<NodeKey, f32>`) derived from
/// `ThreeD { z_source }` is ephemeral — recomputed on each 2D→3D switch and
/// never stored separately.  Snapshot degradation rule: if a persisted snapshot
/// contains `ThreeD` but 3D rendering is unavailable (e.g., unsupported platform),
/// the view falls back to `TwoD`; (x, y) positions are preserved unchanged.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, Default)]
pub enum ViewDimension {
    /// Standard 2D planar graph (default).
    #[default]
    TwoD,
    /// 3D graph with the given sub-mode and z-source.
    ThreeD { mode: ThreeDMode, z_source: ZSource },
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct GraphViewState {
    pub id: GraphViewId,
    pub name: String,
    pub camera: Camera,
    #[serde(default)]
    pub position_fit_locked: bool,
    #[serde(default)]
    pub zoom_fit_locked: bool,
    pub lens: LensConfig,
    pub local_simulation: Option<LocalSimulation>,
    /// The rendering dimension for this view (2D or 3D sub-mode).
    ///
    /// Persisted with the view state so that reopening a frame restores the
    /// user's last dimension choice.  Snapshot degradation: falls back to `TwoD`
    /// if 3D rendering is unavailable on the target platform.
    #[serde(default)]
    pub dimension: ViewDimension,
    #[serde(skip)]
    pub last_layout_algorithm_id: Option<String>,
    #[serde(skip)]
    pub egui_state: Option<EguiGraphState>,
    /// Active PMEST facet filter expression for this view.
    ///
    /// When `Some`, the graph render pass filters visible nodes to those whose
    /// [`facet_projection_for_node`] evaluates the expression to `true`.
    /// `None` means all nodes are visible (no filter active).
    #[serde(default)]
    pub active_filter: Option<crate::model::graph::filter::FacetExpr>,
    /// Optional per-view relation projection override used for graphlet
    /// computation and projection-aware workbench routing.
    #[serde(default)]
    pub edge_projection_override: Option<EdgeProjectionState>,
    /// Per-view ghost node visibility.  When `true`, nodes in
    /// `NodeLifecycle::Tombstone` state are rendered as faint ghost nodes.
    /// Defaults to `false` (tombstoned nodes are hidden from the render pass).
    #[serde(default)]
    pub tombstones_visible: bool,
}

impl std::fmt::Debug for GraphViewState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GraphViewState")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("camera", &self.camera)
            .field("position_fit_locked", &self.position_fit_locked)
            .field("zoom_fit_locked", &self.zoom_fit_locked)
            .field("lens", &self.lens)
            .field("local_simulation", &self.local_simulation)
            .field("dimension", &self.dimension)
            .field("last_layout_algorithm_id", &self.last_layout_algorithm_id)
            .field("active_filter", &self.active_filter)
            .field("edge_projection_override", &self.edge_projection_override)
            .finish_non_exhaustive()
    }
}

impl Clone for GraphViewState {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            name: self.name.clone(),
            camera: self.camera.clone(),
            position_fit_locked: self.position_fit_locked,
            zoom_fit_locked: self.zoom_fit_locked,
            lens: self.lens.clone(),
            local_simulation: self.local_simulation.clone(),
            dimension: self.dimension.clone(),
            last_layout_algorithm_id: self.last_layout_algorithm_id.clone(),
            egui_state: None,
            active_filter: self.active_filter.clone(),
            edge_projection_override: self.edge_projection_override.clone(),
            tombstones_visible: self.tombstones_visible,
        }
    }
}

impl GraphViewState {
    pub fn effective_filter_expr(&self) -> Option<&crate::model::graph::filter::FacetExpr> {
        self.active_filter
            .as_ref()
            .or(self.lens.filter_expr.as_ref())
    }

    pub fn new_with_id(id: GraphViewId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            camera: Camera::new(),
            position_fit_locked: false,
            zoom_fit_locked: false,
            lens: LensConfig::default(),
            local_simulation: None,
            dimension: ViewDimension::default(),
            last_layout_algorithm_id: None,
            egui_state: None,
            active_filter: None,
            edge_projection_override: None,
            tombstones_visible: false,
        }
    }

    pub fn new(name: impl Into<String>) -> Self {
        Self::new_with_id(GraphViewId::new(), name)
    }
}

pub(crate) fn default_semantic_depth_dimension() -> ViewDimension {
    ViewDimension::ThreeD {
        mode: ThreeDMode::TwoPointFive,
        z_source: ZSource::UdcLevel { scale: 48.0 },
    }
}

pub(crate) fn is_semantic_depth_dimension(dimension: &ViewDimension) -> bool {
    matches!(
        dimension,
        ViewDimension::ThreeD {
            mode: ThreeDMode::TwoPointFive,
            z_source: ZSource::UdcLevel { .. },
        }
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum GraphViewLayoutDirection {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GraphViewSlot {
    pub view_id: GraphViewId,
    pub name: String,
    pub row: i32,
    pub col: i32,
    #[serde(default)]
    pub archived: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub struct GraphViewLayoutManagerState {
    #[serde(default)]
    pub active: bool,
    #[serde(default)]
    pub slots: HashMap<GraphViewId, GraphViewSlot>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct PersistedGraphViewLayoutManager {
    pub(crate) version: u32,
    pub(crate) active: bool,
    pub(crate) slots: Vec<GraphViewSlot>,
}

impl PersistedGraphViewLayoutManager {
    pub(crate) const VERSION: u32 = 1;
}
