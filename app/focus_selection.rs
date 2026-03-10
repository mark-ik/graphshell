use super::*;

impl GraphBrowserApp {
    pub fn clear_hop_distance_cache(&mut self) {
        self.workspace.hop_distance_cache = None;
    }

    pub fn cached_hop_distances_for_context(
        &mut self,
        context: NodeKey,
    ) -> HashMap<NodeKey, usize> {
        if self.workspace.domain.graph.get_node(context).is_none() {
            return HashMap::new();
        }
        if let Some((cached_context, cached)) = self.workspace.hop_distance_cache.as_ref()
            && *cached_context == context
        {
            return cached.clone();
        }
        let distances = self.workspace.domain.graph.hop_distances_from(context);
        self.workspace.hop_distance_cache = Some((context, distances.clone()));
        distances
    }

    fn empty_selection() -> &'static SelectionState {
        static EMPTY_SELECTION: std::sync::OnceLock<SelectionState> = std::sync::OnceLock::new();
        EMPTY_SELECTION.get_or_init(SelectionState::new)
    }

    fn invalidate_hop_distance_cache_on_primary_change(
        &mut self,
        previous_primary: Option<NodeKey>,
    ) {
        if previous_primary != self.focused_selection().primary() {
            self.clear_hop_distance_cache();
        }
    }

    fn current_selection_scope(&self) -> SelectionScope {
        self.workspace
            .focused_view
            .map(SelectionScope::View)
            .unwrap_or(SelectionScope::Unfocused)
    }

    fn selection_for_scope(&self, scope: SelectionScope) -> &SelectionState {
        self.workspace
            .selection_by_scope
            .get(&scope)
            .unwrap_or(Self::empty_selection())
    }

    fn current_selection_mut(&mut self) -> &mut SelectionState {
        self.workspace
            .selection_by_scope
            .entry(self.current_selection_scope())
            .or_default()
    }

    fn set_unfocused_selection(&mut self, selection: SelectionState) {
        if selection.is_empty() {
            self.workspace
                .selection_by_scope
                .remove(&SelectionScope::Unfocused);
        } else {
            self.workspace
                .selection_by_scope
                .insert(SelectionScope::Unfocused, selection);
        }
    }

    pub fn clear_selection(&mut self) {
        let previous_primary = self.focused_selection().primary();
        if let Some(selection) = self
            .workspace
            .selection_by_scope
            .get_mut(&self.current_selection_scope())
        {
            selection.clear();
        }
        self.invalidate_hop_distance_cache_on_primary_change(previous_primary);
        self.workspace.egui_state_dirty = true;
    }

    pub(crate) fn reset_selection_state(&mut self) {
        let previous_primary = self.focused_selection().primary();
        self.workspace.selection_by_scope.clear();
        self.invalidate_hop_distance_cache_on_primary_change(previous_primary);
        self.workspace.egui_state_dirty = true;
    }

    pub(crate) fn prune_selection_to_existing_nodes(&mut self) {
        let previous_primary = self.focused_selection().primary();
        for selection in self.workspace.selection_by_scope.values_mut() {
            selection.retain_nodes(|key| self.workspace.domain.graph.get_node(key).is_some());
        }
        self.workspace
            .selection_by_scope
            .retain(|_, selection| !selection.is_empty());
        self.invalidate_hop_distance_cache_on_primary_change(previous_primary);
        self.workspace.egui_state_dirty = true;
    }

    pub(crate) fn retain_selection_scopes_for_graph_views(
        &mut self,
        live_graph_views: &HashSet<GraphViewId>,
        registered_views: &HashSet<GraphViewId>,
    ) {
        let previous_primary = self.focused_selection().primary();
        self.workspace
            .selection_by_scope
            .retain(|scope, _| match scope {
                SelectionScope::Unfocused => true,
                SelectionScope::View(view_id) => {
                    live_graph_views.contains(view_id) || registered_views.contains(view_id)
                }
            });
        self.invalidate_hop_distance_cache_on_primary_change(previous_primary);
        self.workspace.egui_state_dirty = true;
    }

    pub fn select_node(&mut self, key: NodeKey, multi_select: bool) {
        if self.workspace.domain.graph.get_node(key).is_none() {
            return;
        }
        self.select_in_focused_view(key, multi_select);
    }

    pub(crate) fn select_in_focused_view(&mut self, key: NodeKey, multi_select: bool) {
        let previous_primary = self.focused_selection().primary();
        self.current_selection_mut().select(key, multi_select);
        self.invalidate_hop_distance_cache_on_primary_change(previous_primary);
        self.workspace.egui_state_dirty = true;
    }

    pub(crate) fn update_focused_selection(
        &mut self,
        keys: Vec<NodeKey>,
        mode: SelectionUpdateMode,
    ) {
        let previous_primary = self.focused_selection().primary();
        self.current_selection_mut().update_many(keys, mode);
        self.invalidate_hop_distance_cache_on_primary_change(previous_primary);
        self.workspace.egui_state_dirty = true;
    }

    pub(crate) fn restore_selection_snapshot(
        &mut self,
        active_selection: SelectionState,
        selection_by_scope: HashMap<SelectionScope, SelectionState>,
    ) {
        let previous_primary = self.focused_selection().primary();
        self.workspace.selection_by_scope = selection_by_scope;
        self.set_unfocused_selection(active_selection);
        self.invalidate_hop_distance_cache_on_primary_change(previous_primary);
        self.workspace.egui_state_dirty = true;
    }

    pub fn selection_for_view(&self, view_id: GraphViewId) -> &SelectionState {
        self.selection_for_scope(SelectionScope::View(view_id))
    }

    pub fn focused_selection(&self) -> &SelectionState {
        self.selection_for_scope(self.current_selection_scope())
    }

    pub fn get_single_selected_node_for_view(&self, view_id: GraphViewId) -> Option<NodeKey> {
        let selected = self.selection_for_view(view_id);
        if selected.len() == 1 {
            selected.primary()
        } else {
            None
        }
    }

    pub(crate) fn set_workspace_focused_view_with_transition(
        &mut self,
        focused_view: Option<GraphViewId>,
    ) {
        let previous_primary = self.focused_selection().primary();
        let previous_focused_view = self.workspace.focused_view;
        self.workspace.focused_view = focused_view;
        self.invalidate_hop_distance_cache_on_primary_change(previous_primary);
        self.workspace.egui_state_dirty = true;
        if self.workspace.focused_view != previous_focused_view {
            self.emit_ux_navigation_transition();
        }
    }
}
