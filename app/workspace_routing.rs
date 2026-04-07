use super::*;
use crate::graph::graphlet;
use crate::graph::{ArrangementSubKind, EdgeFamily, NodeKey, RelationSelector};
use crate::util::VersoAddress;
use petgraph::visit::{EdgeRef, IntoEdgeReferences};
use std::collections::{BTreeMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArrangementProjectionGroup {
    pub container_key: NodeKey,
    pub sub_kind: ArrangementSubKind,
    pub id: String,
    pub title: String,
    pub member_keys: Vec<NodeKey>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NavigatorSectionProjection {
    pub mode: NavigatorProjectionMode,
    pub seed_source: NavigatorProjectionSeedSource,
    pub workbench_groups: Vec<ArrangementProjectionGroup>,
    pub saved_views: Vec<GraphViewId>,
    pub semantic_groups: Vec<SemanticProjectionGroup>,
    pub folder_sections: BTreeMap<String, Vec<NodeKey>>,
    pub domain_sections: BTreeMap<String, Vec<NodeKey>>,
    pub unrelated_nodes: Vec<NodeKey>,
    pub recent_nodes: Vec<(NodeKey, u64)>,
    pub all_nodes: Vec<NodeKey>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticProjectionGroup {
    pub title: String,
    pub member_keys: Vec<NodeKey>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewGraphletPartition {
    pub anchor: NodeKey,
    pub members: Vec<NodeKey>,
    pub internal_edges: Vec<(NodeKey, NodeKey)>,
}

impl NavigatorSectionProjection {
    pub fn shows_saved_views_section(&self) -> bool {
        self.seed_source == NavigatorProjectionSeedSource::SavedViewCollections
    }

    pub fn shows_workbench_section(&self) -> bool {
        self.mode == NavigatorProjectionMode::Workbench
    }

    pub fn shows_containment_sections(&self) -> bool {
        matches!(
            self.mode,
            NavigatorProjectionMode::Workbench | NavigatorProjectionMode::Containment
        )
    }

    pub fn shows_semantic_section(&self) -> bool {
        self.mode == NavigatorProjectionMode::Semantic
    }

    pub fn shows_unrelated_section(&self) -> bool {
        matches!(
            self.mode,
            NavigatorProjectionMode::Workbench | NavigatorProjectionMode::Semantic
        )
    }

    pub fn shows_recent_section(&self) -> bool {
        self.mode == NavigatorProjectionMode::Workbench
    }

    pub fn shows_all_nodes_section(&self) -> bool {
        self.mode == NavigatorProjectionMode::AllNodes
    }
}

impl GraphBrowserApp {
    pub fn graphlet_partitions_for_view(&self, view_id: GraphViewId) -> Vec<ViewGraphletPartition> {
        let Some(view_state) = self.workspace.graph_runtime.views.get(&view_id) else {
            return Vec::new();
        };

        let visible_nodes = self.visible_node_keys_for_graphlet_partition(view_state);
        if visible_nodes.is_empty() {
            return Vec::new();
        }

        let selectors = self
            .graph_view_edge_projection_override(view_id)
            .map(|state| state.active_selectors.clone())
            .unwrap_or_else(|| self.workbench_edge_projection().active_selectors.clone());

        let mut ordered_nodes: Vec<NodeKey> = visible_nodes.iter().copied().collect();
        ordered_nodes.sort_by(|left, right| {
            arrangement_member_sort_key(self, *left).cmp(&arrangement_member_sort_key(self, *right))
        });

        let mut visited = HashSet::new();
        let mut partitions = Vec::new();

        for seed in ordered_nodes {
            if visited.contains(&seed) {
                continue;
            }

            let mut members = graphlet::graphlet_members_for_seeds_with_selectors(
                self.domain_graph(),
                &[seed],
                &selectors,
            );
            members.retain(|node| visible_nodes.contains(node));
            members.sort_by(|left, right| {
                arrangement_member_sort_key(self, *left).cmp(&arrangement_member_sort_key(self, *right))
            });
            members.dedup();

            if members.is_empty() {
                continue;
            }

            visited.extend(members.iter().copied());
            let internal_edges = projected_internal_edges(self.domain_graph(), &members, &selectors);
            partitions.push(ViewGraphletPartition {
                anchor: seed,
                members,
                internal_edges,
            });
        }

        partitions.sort_by(|left, right| {
            right
                .members
                .len()
                .cmp(&left.members.len())
                .then_with(|| {
                    arrangement_member_sort_key(self, left.anchor)
                        .cmp(&arrangement_member_sort_key(self, right.anchor))
                })
        });
        partitions
    }

    pub fn graphlet_peers_for_view(
        &self,
        seed: NodeKey,
        view_id: Option<GraphViewId>,
    ) -> Vec<NodeKey> {
        let projection = self.resolved_edge_projection_for_seed(seed, view_id);
        graphlet::graphlet_peers_for_node_with_selectors(
            self.domain_graph(),
            seed,
            &projection.selectors,
        )
    }

    pub fn graphlet_members_for_nodes_in_view(
        &self,
        seed_nodes: &[NodeKey],
        view_id: Option<GraphViewId>,
    ) -> Vec<NodeKey> {
        let projection = self.resolved_edge_projection_for_nodes(seed_nodes, view_id);
        graphlet::graphlet_members_for_seeds_with_selectors(
            self.domain_graph(),
            seed_nodes,
            &projection.selectors,
        )
    }

    pub fn graphlet_peers_for_active_projection(&self, seed: NodeKey) -> Vec<NodeKey> {
        self.graphlet_peers_for_view(seed, self.workspace.graph_runtime.focused_view)
    }

    pub fn graphlet_members_for_active_projection(&self, seed_nodes: &[NodeKey]) -> Vec<NodeKey> {
        self.graphlet_members_for_nodes_in_view(
            seed_nodes,
            self.workspace.graph_runtime.focused_view,
        )
    }

    pub fn arrangement_projection_groups(&self) -> Vec<ArrangementProjectionGroup> {
        let mut groups: HashMap<(NodeKey, ArrangementSubKind), Vec<NodeKey>> = HashMap::new();
        for edge in self.domain_graph().arrangement_edges() {
            if self.domain_graph().get_node(edge.from).is_none()
                || self.domain_graph().get_node(edge.to).is_none()
            {
                continue;
            }
            groups
                .entry((edge.from, edge.sub_kind))
                .or_default()
                .push(edge.to);
        }

        let mut projection = groups
            .into_iter()
            .filter_map(|((container_key, sub_kind), mut member_keys)| {
                let container = self.domain_graph().get_node(container_key)?;
                let (id, title) = arrangement_group_identity(container, sub_kind)?;
                member_keys.sort_by(|left, right| {
                    arrangement_member_sort_key(self, *left)
                        .cmp(&arrangement_member_sort_key(self, *right))
                });
                member_keys.dedup();
                Some(ArrangementProjectionGroup {
                    container_key,
                    sub_kind,
                    id,
                    title,
                    member_keys,
                })
            })
            .collect::<Vec<_>>();

        projection.sort_by(|left, right| {
            arrangement_group_priority(left.sub_kind)
                .cmp(&arrangement_group_priority(right.sub_kind))
                .then_with(|| left.title.cmp(&right.title))
                .then_with(|| left.id.cmp(&right.id))
        });
        projection
    }

    fn visible_node_keys_for_graphlet_partition(
        &self,
        view_state: &GraphViewState,
    ) -> HashSet<NodeKey> {
        let mut visible: HashSet<NodeKey> = self.domain_graph().nodes().map(|(key, _)| key).collect();

        if let Some(expr) = view_state.effective_filter_expr() {
            let matched: HashSet<NodeKey> =
                crate::model::graph::filter::evaluate_filter_result(self.domain_graph(), expr)
                    .result
                    .matched_nodes
                    .into_iter()
                    .collect();
            visible.retain(|node| matched.contains(node));
        }

        if !view_state.tombstones_visible {
            visible.retain(|node| {
                self.domain_graph()
                    .get_node(*node)
                    .is_some_and(|graph_node| graph_node.lifecycle != crate::graph::NodeLifecycle::Tombstone)
            });
        }

        if let Some(mask) = view_state.owned_node_mask() {
            visible.retain(|node| mask.contains(node));
        }

        if let Some(mask) = view_state.graphlet_node_mask.as_ref() {
            visible.retain(|node| mask.contains(node));
        }

        visible
    }

    pub fn arrangement_frame_membership_index(&self) -> HashMap<Uuid, BTreeSet<String>> {
        let mut index: HashMap<Uuid, BTreeSet<String>> = HashMap::new();
        for group in self.arrangement_projection_groups() {
            if group.sub_kind != ArrangementSubKind::FrameMember {
                continue;
            }
            for member_key in group.member_keys {
                let Some(node) = self.domain_graph().get_node(member_key) else {
                    continue;
                };
                index.entry(node.id).or_default().insert(group.id.clone());
            }
        }
        index
    }

    pub fn navigator_section_projection(&self) -> NavigatorSectionProjection {
        let mode = self.navigator_projection_state().mode;
        let seed_source = self.navigator_projection_state().projection_seed_source;
        let workbench_groups = self.arrangement_projection_groups();
        let mut arranged_nodes = HashSet::new();
        for group in &workbench_groups {
            for node_key in &group.member_keys {
                arranged_nodes.insert(*node_key);
            }
        }

        let traversal_timestamps = navigator_recent_traversal_node_timestamps(self);
        let semantic_selector = [RelationSelector::Semantic(
            crate::graph::SemanticSubKind::UserGrouped,
        )];
        let mut semantic_groups = Vec::new();
        let mut semantic_grouped_nodes = HashSet::new();
        for node_key in self.domain_graph().nodes().map(|(key, _)| key) {
            if semantic_grouped_nodes.contains(&node_key) {
                continue;
            }
            let mut members = graphlet::graphlet_members_for_seeds_with_selectors(
                self.domain_graph(),
                &[node_key],
                &semantic_selector,
            );
            members.sort_by(|left, right| {
                arrangement_member_sort_key(self, *left)
                    .cmp(&arrangement_member_sort_key(self, *right))
            });
            members.dedup();
            if members.len() <= 1 {
                continue;
            }
            let title = members
                .first()
                .copied()
                .map(|key| arrangement_member_sort_key(self, key).0)
                .unwrap_or_else(|| format!("Group {}", semantic_groups.len() + 1));
            for member in &members {
                semantic_grouped_nodes.insert(*member);
            }
            semantic_groups.push(SemanticProjectionGroup {
                title,
                member_keys: members,
            });
        }
        semantic_groups.sort_by(|left, right| {
            left.title
                .cmp(&right.title)
                .then_with(|| left.member_keys.len().cmp(&right.member_keys.len()))
        });
        let mut all_nodes: Vec<NodeKey> = self.domain_graph().nodes().map(|(key, _)| key).collect();
        all_nodes.sort_by(|left, right| {
            traversal_timestamps
                .get(right)
                .copied()
                .unwrap_or(0)
                .cmp(&traversal_timestamps.get(left).copied().unwrap_or(0))
                .then_with(|| {
                    arrangement_member_sort_key(self, *left)
                        .cmp(&arrangement_member_sort_key(self, *right))
                })
        });
        let mut recent_nodes: Vec<(NodeKey, u64)> = traversal_timestamps
            .iter()
            .filter_map(|(node_key, timestamp)| {
                (!arranged_nodes.contains(node_key)).then_some((*node_key, *timestamp))
            })
            .collect();
        recent_nodes.sort_by(|left, right| {
            right.1.cmp(&left.1).then_with(|| {
                arrangement_member_sort_key(self, left.0)
                    .cmp(&arrangement_member_sort_key(self, right.0))
            })
        });
        let recent_set: HashSet<NodeKey> = recent_nodes.iter().map(|(key, _)| *key).collect();

        let mut unrelated_nodes: Vec<NodeKey> = self
            .domain_graph()
            .nodes()
            .map(|(key, _)| key)
            .filter(|key| !arranged_nodes.contains(key) && !recent_set.contains(key))
            .collect();
        unrelated_nodes.sort_by(|left, right| {
            arrangement_member_sort_key(self, *left).cmp(&arrangement_member_sort_key(self, *right))
        });

        let mut domain_sections: BTreeMap<String, Vec<NodeKey>> = BTreeMap::new();
        let mut folder_sections: BTreeMap<String, Vec<NodeKey>> = BTreeMap::new();
        for (row_key, target) in &self.navigator_projection_state().row_targets {
            let NavigatorProjectionTarget::Node(node_key) = target else {
                continue;
            };
            if let Some(domain) = containment_domain_from_row_key(row_key) {
                domain_sections
                    .entry(domain.to_string())
                    .or_default()
                    .push(*node_key);
            }
            if let Some(folder) = containment_folder_from_row_key(row_key) {
                folder_sections
                    .entry(folder.to_string())
                    .or_default()
                    .push(*node_key);
            }
        }

        for node_keys in domain_sections.values_mut() {
            node_keys.sort_by(|left, right| {
                arrangement_member_sort_key(self, *left)
                    .cmp(&arrangement_member_sort_key(self, *right))
            });
            node_keys.dedup();
        }
        for node_keys in folder_sections.values_mut() {
            node_keys.sort_by(|left, right| {
                arrangement_member_sort_key(self, *left)
                    .cmp(&arrangement_member_sort_key(self, *right))
            });
            node_keys.dedup();
        }

        let mut saved_views: Vec<GraphViewId> = self
            .navigator_projection_state()
            .row_targets
            .values()
            .filter_map(|target| match target {
                NavigatorProjectionTarget::SavedView(view_id) => Some(*view_id),
                NavigatorProjectionTarget::Node(_) => None,
            })
            .collect();
        saved_views.sort_by_key(|view_id| view_id.as_uuid());
        saved_views.dedup();

        let mut projection = NavigatorSectionProjection {
            mode,
            seed_source,
            workbench_groups,
            saved_views,
            semantic_groups,
            folder_sections,
            domain_sections,
            unrelated_nodes,
            recent_nodes,
            all_nodes,
        };

        if seed_source == NavigatorProjectionSeedSource::SavedViewCollections {
            projection.workbench_groups.clear();
            projection.semantic_groups.clear();
            projection.folder_sections.clear();
            projection.domain_sections.clear();
            projection.unrelated_nodes.clear();
            projection.recent_nodes.clear();
            projection.all_nodes.clear();
            return projection;
        }

        match mode {
            NavigatorProjectionMode::Workbench => {
                projection.semantic_groups.clear();
                projection.all_nodes.clear();
            }
            NavigatorProjectionMode::Containment => {
                projection.workbench_groups.clear();
                projection.semantic_groups.clear();
                projection.unrelated_nodes.clear();
                projection.recent_nodes.clear();
                projection.all_nodes.clear();
            }
            NavigatorProjectionMode::Semantic => {
                projection.workbench_groups.clear();
                projection.folder_sections.clear();
                projection.domain_sections.clear();
                projection.recent_nodes.clear();
                projection.unrelated_nodes = projection
                    .all_nodes
                    .iter()
                    .copied()
                    .filter(|key| {
                        !projection
                            .semantic_groups
                            .iter()
                            .any(|group| group.member_keys.contains(key))
                    })
                    .collect();
                projection.all_nodes.clear();
            }
            NavigatorProjectionMode::AllNodes => {
                projection.workbench_groups.clear();
                projection.semantic_groups.clear();
                projection.folder_sections.clear();
                projection.domain_sections.clear();
                projection.unrelated_nodes.clear();
                projection.recent_nodes.clear();
            }
        }

        projection
    }

    /// Return all nodes in the graphlet induced by `selectors`, excluding `seed`.
    ///
    /// This is the projection-aware graphlet API: callers decide which relation
    /// families/sub-kinds contribute to graphlet connectivity.
    pub fn graphlet_peers_for_selectors(
        &self,
        seed: NodeKey,
        selectors: &[RelationSelector],
    ) -> Vec<NodeKey> {
        graphlet::graphlet_peers_for_node_with_selectors(self.domain_graph(), seed, selectors)
    }

    /// Return all nodes in the same default durable graphlet as `seed`,
    /// excluding `seed`.
    ///
    /// This remains the compatibility/default workbench projection until
    /// per-view and per-selection edge visibility controls are wired into the
    /// workbench graphlet-routing paths.
    pub fn durable_graphlet_peers(&self, seed: NodeKey) -> Vec<NodeKey> {
        graphlet::graphlet_peers_for_node(self.domain_graph(), seed)
    }

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
        self.workspace.workbench_session.current_workspace_name = None;
        self.workspace.workbench_session.current_frame_tab_semantics = None;
        self.workspace
            .workbench_session
            .current_workspace_is_synthesized = true;
        self.workspace
            .workbench_session
            .workspace_has_unsaved_changes = false;
        self.workspace
            .workbench_session
            .unsaved_workspace_prompt_warned = false;
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
            .workbench_session
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
        if let Some((_, recent)) = self
            .workspace
            .workbench_session
            .node_last_active_workspace
            .get(&node.id)
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
            .workbench_session
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
        self.workspace.workbench_session.workspace_activation_seq = self
            .workspace
            .workbench_session
            .workspace_activation_seq
            .saturating_add(1);
        let seq = self.workspace.workbench_session.workspace_activation_seq;
        let workspace_name = workspace_name.to_string();
        self.workspace.workbench_session.current_workspace_name = Some(workspace_name.clone());
        for key in nodes {
            let Some(node) = self.workspace.domain.graph.get_node(key) else {
                continue;
            };
            self.workspace
                .workbench_session
                .node_last_active_workspace
                .insert(node.id, (seq, workspace_name.clone()));
            self.workspace
                .workbench_session
                .node_workspace_membership
                .entry(node.id)
                .or_default()
                .insert(workspace_name.clone());
        }
        self.workspace
            .workbench_session
            .current_workspace_is_synthesized = false;
        self.workspace
            .workbench_session
            .workspace_has_unsaved_changes = false;
        self.workspace
            .workbench_session
            .unsaved_workspace_prompt_warned = false;
        self.workspace.graph_runtime.egui_state_dirty = true;
    }

    /// Mark a named frame snapshot as activated, updating per-node recency.
    pub fn note_frame_activated(
        &mut self,
        frame_name: &str,
        nodes: impl IntoIterator<Item = NodeKey>,
    ) {
        self.note_workspace_activated(frame_name, nodes);
    }

    pub fn current_workspace_name(&self) -> Option<&str> {
        self.workspace
            .workbench_session
            .current_workspace_name
            .as_deref()
    }

    pub fn current_frame_name(&self) -> Option<&str> {
        self.current_workspace_name()
    }

    pub fn current_frame_tab_semantics(&self) -> Option<&RuntimeFrameTabSemantics> {
        self.workspace
            .workbench_session
            .current_frame_tab_semantics
            .as_ref()
    }

    pub fn set_current_frame_tab_semantics(&mut self, semantics: Option<RuntimeFrameTabSemantics>) {
        self.workspace.workbench_session.current_frame_tab_semantics = semantics;
    }

    /// Initialize membership index from desktop-layer workspace scan.
    pub fn init_membership_index(&mut self, index: HashMap<Uuid, BTreeSet<String>>) {
        self.workspace.workbench_session.node_workspace_membership = index;
        self.workspace.graph_runtime.egui_state_dirty = true;
    }

    /// Initialize UUID-keyed workspace activation recency from desktop-layer manifest scan.
    pub fn init_workspace_activation_recency(
        &mut self,
        recency: HashMap<Uuid, (u64, String)>,
        activation_seq: u64,
    ) {
        self.workspace.workbench_session.node_last_active_workspace = recency;
        self.workspace.workbench_session.workspace_activation_seq = activation_seq;
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
            .workbench_session
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
            if let Some((_, recent_workspace)) = node_uuid.and_then(|uuid| {
                self.workspace
                    .workbench_session
                    .node_last_active_workspace
                    .get(&uuid)
            }) && memberships.contains(recent_workspace)
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

    /// Current explicit frame context target for command-surface actions.
    pub fn pending_frame_context_target(&self) -> Option<&str> {
        match self.pending_app_command(|command| {
            matches!(command, AppCommand::FrameContextTarget { .. })
        })? {
            AppCommand::FrameContextTarget { frame_name } => Some(frame_name.as_str()),
            _ => None,
        }
    }

    /// Set/clear explicit frame context target for command-surface actions.
    pub fn set_pending_frame_context_target(&mut self, frame_name: Option<String>) {
        let _ = self.take_pending_app_command(|command| {
            matches!(command, AppCommand::FrameContextTarget { .. })
        });

        if let Some(frame_name) = frame_name {
            self.enqueue_app_command(AppCommand::FrameContextTarget { frame_name });
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

fn arrangement_group_priority(sub_kind: ArrangementSubKind) -> u8 {
    match sub_kind {
        ArrangementSubKind::FrameMember => 0,
        ArrangementSubKind::TileGroup => 1,
        ArrangementSubKind::SplitPair => 2,
    }
}

fn arrangement_group_identity(
    container: &crate::graph::Node,
    sub_kind: ArrangementSubKind,
) -> Option<(String, String)> {
    match VersoAddress::parse(container.url()) {
        Some(VersoAddress::Frame(name)) => Some((name.clone(), container_label(container, &name))),
        Some(VersoAddress::TileGroup(group_id)) => {
            let fallback = if sub_kind == ArrangementSubKind::TileGroup {
                "Tile Group"
            } else {
                sub_kind.as_tag()
            };
            Some((group_id.clone(), container_label(container, fallback)))
        }
        _ => {
            let fallback = sub_kind.as_tag().to_string();
            Some((fallback.clone(), container_label(container, &fallback)))
        }
    }
}

fn container_label(container: &crate::graph::Node, fallback: &str) -> String {
    let title = container.title.trim();
    if !title.is_empty() {
        title.to_string()
    } else {
        fallback.to_string()
    }
}

fn arrangement_member_sort_key(app: &GraphBrowserApp, key: NodeKey) -> (String, usize) {
    let label = app
        .domain_graph()
        .get_node(key)
        .map(|node| {
            let title = node.title.trim();
            if !title.is_empty() {
                title.to_string()
            } else {
                node.url().to_string()
            }
        })
        .unwrap_or_else(|| format!("Node {}", key.index()));
    (label, key.index())
}

fn projected_internal_edges(
    graph: &crate::graph::Graph,
    members: &[NodeKey],
    selectors: &[RelationSelector],
) -> Vec<(NodeKey, NodeKey)> {
    let member_set: HashSet<NodeKey> = members.iter().copied().collect();
    let mut out = Vec::new();

    for edge in graph.inner.edge_references() {
        let from = edge.source();
        let to = edge.target();
        if !member_set.contains(&from) || !member_set.contains(&to) {
            continue;
        }
        if !selectors.iter().any(|selector| edge.weight().has_relation(*selector)) {
            continue;
        }

        let pair = if from.index() <= to.index() {
            (from, to)
        } else {
            (to, from)
        };
        out.push(pair);
    }

    out.sort_by(|left, right| {
        left.0
            .index()
            .cmp(&right.0.index())
            .then_with(|| left.1.index().cmp(&right.1.index()))
    });
    out.dedup();
    out
}

fn containment_domain_from_row_key(row_key: &str) -> Option<&str> {
    row_key.strip_prefix("domain:")?.split('#').next()
}

fn containment_folder_from_row_key(row_key: &str) -> Option<&str> {
    row_key.strip_prefix("folder:")?.split('#').next()
}

fn navigator_recent_traversal_node_timestamps(app: &GraphBrowserApp) -> HashMap<NodeKey, u64> {
    let mut by_node: HashMap<NodeKey, u64> = HashMap::new();
    for edge in app.domain_graph().inner.edge_references() {
        if !edge
            .weight()
            .has_relation(RelationSelector::Family(EdgeFamily::Traversal))
        {
            continue;
        }
        let timestamp = edge.weight().metrics().last_navigated_at.unwrap_or(0);
        if timestamp == 0 {
            continue;
        }
        by_node
            .entry(edge.target())
            .and_modify(|current| *current = (*current).max(timestamp))
            .or_insert(timestamp);
    }
    by_node
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::NavigationTrigger;

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
        assert!(
            !app.workspace
                .workbench_session
                .current_workspace_is_synthesized
        );
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
        assert!(
            app.workspace
                .workbench_session
                .current_workspace_is_synthesized
        );
    }

    #[test]
    fn graphlet_peers_for_view_uses_graph_default_projection() {
        let mut app = GraphBrowserApp::new_for_testing();
        let a = app.add_node_and_sync("https://a.test".to_string(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("https://b.test".to_string(), Point2D::new(1.0, 0.0));
        app.apply_graph_delta_and_sync(crate::graph::apply::GraphDelta::AddEdge {
            from: a,
            to: b,
            edge_type: crate::graph::EdgeType::History,
            edge_label: None,
        });

        app.set_workbench_edge_projection(vec![RelationSelector::Family(
            crate::graph::EdgeFamily::Traversal,
        )]);

        assert_eq!(app.graphlet_peers_for_view(a, None), vec![b]);
    }

    #[test]
    fn graphlet_peers_for_view_prefers_selection_override_for_selected_seed() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_id = GraphViewId::new();
        app.ensure_graph_view_registered(view_id);
        app.set_workspace_focused_view_with_transition(Some(view_id));

        let a = app.add_node_and_sync("https://a.test".to_string(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("https://b.test".to_string(), Point2D::new(1.0, 0.0));
        let _ = app.assert_relation_and_sync(
            a,
            b,
            crate::graph::EdgeAssertion::Semantic {
                sub_kind: crate::graph::SemanticSubKind::Hyperlink,
                label: None,
                decay_progress: None,
            },
        );

        app.select_node(a, false);
        app.set_selection_edge_projection_override(
            Some(view_id),
            Some(vec![RelationSelector::Semantic(
                crate::graph::SemanticSubKind::Hyperlink,
            )]),
        );

        assert_eq!(app.graphlet_peers_for_view(a, Some(view_id)), vec![b]);
    }

    #[test]
    fn graphlet_partitions_for_view_respects_visible_view_scope() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_id = GraphViewId::new();
        app.ensure_graph_view_registered(view_id);

        let a = app.add_node_and_sync("https://partition-a.test".to_string(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("https://partition-b.test".to_string(), Point2D::new(1.0, 0.0));
        let c = app.add_node_and_sync("https://partition-c.test".to_string(), Point2D::new(2.0, 0.0));
        let d = app.add_node_and_sync("https://partition-d.test".to_string(), Point2D::new(3.0, 0.0));
        let hidden = app.add_node_and_sync("https://partition-hidden.test".to_string(), Point2D::new(4.0, 0.0));

        for (from, to) in [(a, b), (c, d), (b, hidden)] {
            app.apply_graph_delta_and_sync(crate::graph::apply::GraphDelta::AddEdge {
                from,
                to,
                edge_type: crate::graph::EdgeType::UserGrouped,
                edge_label: None,
            });
        }

        let view = app.workspace.graph_runtime.views.get_mut(&view_id).unwrap();
        view.owned_node_mask = Some([a, b, c, d].into_iter().collect());

        let partitions = app.graphlet_partitions_for_view(view_id);

        assert_eq!(partitions.len(), 2);
        assert_eq!(partitions[0].members.len(), 2);
        assert_eq!(partitions[1].members.len(), 2);
        assert!(partitions.iter().any(|partition| partition.members == vec![a, b]));
        assert!(partitions.iter().any(|partition| partition.members == vec![c, d]));
        assert!(partitions
            .iter()
            .all(|partition| !partition.members.contains(&hidden)));
        assert!(partitions.iter().all(|partition| partition.internal_edges.len() == 1));
    }

    #[test]
    fn navigator_section_projection_groups_workbench_recent_and_unrelated_nodes() {
        let mut app = GraphBrowserApp::new_for_testing();
        let frame = app.add_node_and_sync(
            VersoAddress::frame("Frame A").to_string(),
            Point2D::new(0.0, 0.0),
        );
        let arranged = app.add_node_and_sync(
            "https://arranged.example".to_string(),
            Point2D::new(1.0, 0.0),
        );
        let source =
            app.add_node_and_sync("https://source.example".to_string(), Point2D::new(2.0, 0.0));
        let recent =
            app.add_node_and_sync("https://recent.example".to_string(), Point2D::new(3.0, 0.0));
        let unrelated = app.add_node_and_sync(
            "https://unrelated.example".to_string(),
            Point2D::new(4.0, 0.0),
        );

        let _ = app.assert_relation_and_sync(
            frame,
            arranged,
            crate::graph::EdgeAssertion::Arrangement {
                sub_kind: ArrangementSubKind::FrameMember,
            },
        );
        assert!(app.push_history_traversal_and_sync(source, recent, NavigationTrigger::Unknown));

        let projection = app.navigator_section_projection();
        assert_eq!(projection.workbench_groups.len(), 1);
        assert_eq!(projection.workbench_groups[0].member_keys, vec![arranged]);
        assert_eq!(projection.recent_nodes.len(), 1);
        assert_eq!(projection.recent_nodes[0].0, recent);
        assert_eq!(projection.unrelated_nodes.len(), 3);
        assert!(projection.unrelated_nodes.contains(&frame));
        assert!(projection.unrelated_nodes.contains(&source));
        assert!(projection.unrelated_nodes.contains(&unrelated));
    }

    #[test]
    fn navigator_section_projection_uses_containment_rows_from_navigator_projection_state() {
        let mut app = GraphBrowserApp::new_for_testing();
        let file_node =
            app.add_node_and_sync("file:///tmp/a.txt".to_string(), Point2D::new(0.0, 0.0));
        let web_node = app.add_node_and_sync(
            "https://example.com/docs".to_string(),
            Point2D::new(1.0, 0.0),
        );

        app.set_navigator_projection_seed_source(
            NavigatorProjectionSeedSource::ContainmentRelations,
        );

        let projection = app.navigator_section_projection();
        assert_eq!(
            projection.folder_sections.get("file:///tmp/"),
            Some(&vec![file_node])
        );
        assert_eq!(
            projection.domain_sections.get("example.com"),
            Some(&vec![web_node])
        );
    }

    #[test]
    fn navigator_section_projection_saved_view_source_exposes_only_saved_views() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_a = GraphViewId::new();
        let view_b = GraphViewId::new();
        app.ensure_graph_view_registered(view_a);
        app.ensure_graph_view_registered(view_b);

        app.set_navigator_projection_seed_source(NavigatorProjectionSeedSource::SavedViewCollections);

        let projection = app.navigator_section_projection();
        let mut expected_views = vec![view_a, view_b];
        expected_views.sort_by_key(|view_id| view_id.as_uuid());
        assert!(projection.shows_saved_views_section());
        assert_eq!(projection.saved_views, expected_views);
        assert!(projection.workbench_groups.is_empty());
        assert!(projection.semantic_groups.is_empty());
        assert!(projection.folder_sections.is_empty());
        assert!(projection.domain_sections.is_empty());
        assert!(projection.unrelated_nodes.is_empty());
        assert!(projection.recent_nodes.is_empty());
        assert!(projection.all_nodes.is_empty());
    }

    #[test]
    fn navigator_section_projection_containment_mode_hides_non_containment_sections() {
        let mut app = GraphBrowserApp::new_for_testing();
        let frame = app.add_node_and_sync(
            VersoAddress::frame("Frame A").to_string(),
            Point2D::new(0.0, 0.0),
        );
        let arranged = app.add_node_and_sync(
            "https://arranged.example".to_string(),
            Point2D::new(1.0, 0.0),
        );
        let source =
            app.add_node_and_sync("https://source.example".to_string(), Point2D::new(2.0, 0.0));
        let recent =
            app.add_node_and_sync("https://recent.example".to_string(), Point2D::new(3.0, 0.0));
        let file_node =
            app.add_node_and_sync("file:///tmp/a.txt".to_string(), Point2D::new(4.0, 0.0));

        let _ = app.assert_relation_and_sync(
            frame,
            arranged,
            crate::graph::EdgeAssertion::Arrangement {
                sub_kind: ArrangementSubKind::FrameMember,
            },
        );
        assert!(app.push_history_traversal_and_sync(source, recent, NavigationTrigger::Unknown));
        app.set_navigator_projection_seed_source(
            NavigatorProjectionSeedSource::ContainmentRelations,
        );
        app.set_navigator_projection_mode(NavigatorProjectionMode::Containment);

        let projection = app.navigator_section_projection();
        assert_eq!(projection.mode, NavigatorProjectionMode::Containment);
        assert!(projection.workbench_groups.is_empty());
        assert!(projection.unrelated_nodes.is_empty());
        assert!(projection.recent_nodes.is_empty());
        assert_eq!(
            projection.folder_sections.get("file:///tmp/"),
            Some(&vec![file_node])
        );
    }

    #[test]
    fn navigator_section_projection_semantic_mode_groups_user_grouped_nodes() {
        let mut app = GraphBrowserApp::new_for_testing();
        let a = app.add_node_and_sync("https://a.example".to_string(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("https://b.example".to_string(), Point2D::new(1.0, 0.0));
        let c = app.add_node_and_sync("https://c.example".to_string(), Point2D::new(2.0, 0.0));

        let _ = app.assert_relation_and_sync(
            a,
            b,
            crate::graph::EdgeAssertion::Semantic {
                sub_kind: crate::graph::SemanticSubKind::UserGrouped,
                label: None,
                decay_progress: None,
            },
        );
        app.set_navigator_projection_mode(NavigatorProjectionMode::Semantic);

        let projection = app.navigator_section_projection();
        assert_eq!(projection.mode, NavigatorProjectionMode::Semantic);
        assert!(projection.workbench_groups.is_empty());
        assert!(projection.folder_sections.is_empty());
        assert!(projection.domain_sections.is_empty());
        assert!(projection.recent_nodes.is_empty());
        assert_eq!(projection.semantic_groups.len(), 1);
        assert_eq!(projection.semantic_groups[0].member_keys, vec![a, b]);
        assert_eq!(projection.unrelated_nodes, vec![c]);
    }

    #[test]
    fn navigator_section_projection_all_nodes_mode_shows_flat_roster_only() {
        let mut app = GraphBrowserApp::new_for_testing();
        let frame = app.add_node_and_sync(
            VersoAddress::frame("Frame A").to_string(),
            Point2D::new(0.0, 0.0),
        );
        let arranged = app.add_node_and_sync(
            "https://arranged.example".to_string(),
            Point2D::new(1.0, 0.0),
        );
        let source =
            app.add_node_and_sync("https://source.example".to_string(), Point2D::new(2.0, 0.0));
        let recent =
            app.add_node_and_sync("https://recent.example".to_string(), Point2D::new(3.0, 0.0));
        let _file_node =
            app.add_node_and_sync("file:///tmp/a.txt".to_string(), Point2D::new(4.0, 0.0));

        let _ = app.assert_relation_and_sync(
            frame,
            arranged,
            crate::graph::EdgeAssertion::Arrangement {
                sub_kind: ArrangementSubKind::FrameMember,
            },
        );
        assert!(app.push_history_traversal_and_sync(source, recent, NavigationTrigger::Unknown));
        app.set_navigator_projection_seed_source(
            NavigatorProjectionSeedSource::ContainmentRelations,
        );
        app.set_navigator_projection_mode(NavigatorProjectionMode::AllNodes);

        let projection = app.navigator_section_projection();
        assert_eq!(projection.mode, NavigatorProjectionMode::AllNodes);
        assert!(projection.workbench_groups.is_empty());
        assert!(projection.folder_sections.is_empty());
        assert!(projection.domain_sections.is_empty());
        assert!(projection.unrelated_nodes.is_empty());
        assert!(projection.recent_nodes.is_empty());
        assert_eq!(projection.all_nodes.len(), 5);
        assert_eq!(projection.all_nodes[0], recent);
    }
}
