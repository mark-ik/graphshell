use super::*;
#[cfg(feature = "egui-host")]
use egui_tiles::{Tile, Tree};

use crate::app::runtime_ports::diagnostics::{DiagnosticEvent, emit_event};
use crate::app::runtime_ports::registries::{
    CHANNEL_UX_LAYOUT_CONSTRAINT_CONFLICT, CHANNEL_UX_LAYOUT_CONSTRAINT_DRIFT,
};
use crate::app::workbench_layout_policy::evaluate_layout_policy_report;
use crate::shell::desktop::workbench::pane_model::PaneId;
use crate::shell::desktop::workbench::ux_tree::UxTreeSnapshot;

use super::arrangement_graph_bridge::ArrangementSnapshot;
#[cfg(feature = "egui-host")]
use crate::shell::desktop::workbench::tile_kind::TileKind;

fn resolved_active_layout_constraints(
    profile: &WorkbenchProfile,
) -> std::collections::HashMap<SurfaceHostId, WorkbenchLayoutConstraint> {
    resolved_active_layout_constraints_with_drafts(profile, &HashMap::new())
}

fn resolved_active_layout_constraints_with_drafts(
    profile: &WorkbenchProfile,
    draft_constraints: &HashMap<SurfaceHostId, WorkbenchLayoutConstraint>,
) -> std::collections::HashMap<SurfaceHostId, WorkbenchLayoutConstraint> {
    let mut combined_constraints = profile.layout_constraints.clone();
    for (surface_host, constraint) in draft_constraints {
        if matches!(constraint, WorkbenchLayoutConstraint::Unconstrained) {
            combined_constraints.remove(surface_host);
        } else {
            combined_constraints.insert(surface_host.clone(), constraint.clone());
        }
    }

    let mut edge_claims = std::collections::HashMap::<
        crate::app::workbench_layout_policy::AnchorEdge,
        Vec<SurfaceHostId>,
    >::new();
    for (surface_host, constraint) in &combined_constraints {
        let WorkbenchLayoutConstraint::AnchoredSplit { anchor_edge, .. } = constraint else {
            continue;
        };
        edge_claims
            .entry(*anchor_edge)
            .or_default()
            .push(surface_host.clone());
    }

    let conflicting_hosts = edge_claims
        .into_values()
        .filter(|hosts| hosts.len() > 1)
        .flatten()
        .collect::<std::collections::HashSet<_>>();

    combined_constraints
        .iter()
        .filter(|(surface_host, constraint)| {
            !matches!(constraint, WorkbenchLayoutConstraint::Unconstrained)
                && !conflicting_hosts.contains(*surface_host)
        })
        .map(|(surface_host, constraint)| (surface_host.clone(), constraint.clone()))
        .collect()
}

impl GraphBrowserApp {
    fn replace_workbench_profile(&mut self, profile: WorkbenchProfile, persist: bool) {
        self.workspace.workbench_session.workbench_profile = profile;
        self.workspace
            .workbench_session
            .draft_layout_constraints
            .clear();
        self.workspace
            .workbench_session
            .session_suppressed_first_use_prompts
            .clear();
        self.recompute_active_layout_constraints();
        if persist {
            self.save_workbench_profile_state();
        }
    }

    fn recompute_active_layout_constraints(&mut self) {
        self.workspace.workbench_session.active_layout_constraints =
            resolved_active_layout_constraints_with_drafts(
                &self.workspace.workbench_session.workbench_profile,
                &self.workspace.workbench_session.draft_layout_constraints,
            );
    }

    pub fn enqueue_workbench_intent(&mut self, intent: WorkbenchIntent) {
        self.workspace
            .workbench_session
            .pending_workbench_intents
            .push(intent);
    }

    pub fn extend_workbench_intents<I>(&mut self, intents: I)
    where
        I: IntoIterator<Item = WorkbenchIntent>,
    {
        self.workspace
            .workbench_session
            .pending_workbench_intents
            .extend(intents);
    }

    pub fn take_pending_workbench_intents(&mut self) -> Vec<WorkbenchIntent> {
        std::mem::take(&mut self.workspace.workbench_session.pending_workbench_intents)
    }

    pub fn workbench_profile(&self) -> &WorkbenchProfile {
        &self.workspace.workbench_session.workbench_profile
    }

    pub fn set_workbench_profile(&mut self, profile: WorkbenchProfile) {
        self.replace_workbench_profile(profile, true);
    }

    pub(crate) fn restore_workbench_profile(&mut self, profile: WorkbenchProfile) {
        self.replace_workbench_profile(profile, false);
    }

    pub fn set_workbench_layout_constraint(
        &mut self,
        surface_host: SurfaceHostId,
        constraint: WorkbenchLayoutConstraint,
    ) {
        self.workspace
            .workbench_session
            .workbench_profile
            .set_layout_constraint(surface_host.clone(), constraint.clone());
        self.workspace
            .workbench_session
            .draft_layout_constraints
            .remove(&surface_host);
        let _ = (surface_host, constraint);
        self.recompute_active_layout_constraints();
        self.save_workbench_profile_state();
    }

    pub fn set_workbench_layout_constraint_draft(
        &mut self,
        surface_host: SurfaceHostId,
        constraint: WorkbenchLayoutConstraint,
    ) {
        self.workspace
            .workbench_session
            .draft_layout_constraints
            .insert(surface_host, constraint);
        self.recompute_active_layout_constraints();
    }

    pub fn commit_workbench_layout_constraint_draft(&mut self, surface_host: &SurfaceHostId) {
        if let Some(constraint) = self
            .workspace
            .workbench_session
            .draft_layout_constraints
            .remove(surface_host)
        {
            self.workspace
                .workbench_session
                .workbench_profile
                .set_layout_constraint(surface_host.clone(), constraint);
            self.recompute_active_layout_constraints();
            self.save_workbench_profile_state();
        }
    }

    pub fn discard_workbench_layout_constraint_draft(&mut self, surface_host: &SurfaceHostId) {
        self.workspace
            .workbench_session
            .draft_layout_constraints
            .remove(surface_host);
        self.recompute_active_layout_constraints();
    }

    pub fn workbench_layout_constraint_for_host(
        &self,
        surface_host: &SurfaceHostId,
    ) -> Option<&WorkbenchLayoutConstraint> {
        self.workspace
            .workbench_session
            .draft_layout_constraints
            .get(surface_host)
            .or_else(|| {
                self.workspace
                    .workbench_session
                    .workbench_profile
                    .layout_constraints
                    .get(surface_host)
            })
    }

    pub fn workbench_layout_constraint_draft_for_host(
        &self,
        surface_host: &SurfaceHostId,
    ) -> Option<&WorkbenchLayoutConstraint> {
        self.workspace
            .workbench_session
            .draft_layout_constraints
            .get(surface_host)
    }

    pub fn set_surface_first_use_policy(&mut self, policy: SurfaceFirstUsePolicy) {
        self.workspace
            .workbench_session
            .workbench_profile
            .set_first_use_policy(policy);
        self.save_workbench_profile_state();
    }

    pub fn navigator_host_scope(&self, surface_host: &SurfaceHostId) -> NavigatorHostScope {
        self.workspace
            .workbench_session
            .workbench_profile
            .navigator_host_scope(surface_host)
    }

    pub fn set_navigator_host_scope(
        &mut self,
        surface_host: SurfaceHostId,
        scope: NavigatorHostScope,
    ) {
        self.workspace
            .workbench_session
            .workbench_profile
            .set_navigator_host_scope(surface_host, scope);
        self.save_workbench_profile_state();
    }

    pub fn suppress_first_use_prompt_for_session(&mut self, surface_host: SurfaceHostId) {
        self.workspace
            .workbench_session
            .session_suppressed_first_use_prompts
            .insert(surface_host);
    }

    pub fn is_first_use_prompt_suppressed_for_session(&self, surface_host: &SurfaceHostId) -> bool {
        self.workspace
            .workbench_session
            .session_suppressed_first_use_prompts
            .contains(surface_host)
    }

    pub fn dismiss_frame_split_offer_for_session(&mut self, frame_name: impl Into<String>) {
        self.workspace
            .workbench_session
            .session_dismissed_frame_split_offers
            .insert(frame_name.into());
    }

    pub fn is_frame_split_offer_dismissed_for_session(&self, frame_name: &str) -> bool {
        self.workspace
            .workbench_session
            .session_dismissed_frame_split_offers
            .contains(frame_name)
    }

    pub fn selected_frame_name(&self) -> Option<&str> {
        self.workspace.graph_runtime.selected_frame_name.as_deref()
    }

    pub fn primary_navigator_surface_host(&self) -> SurfaceHostId {
        if let UxConfigMode::Configuring { surface_host } =
            &self.workspace.workbench_session.ux_config_mode
        {
            return surface_host.clone();
        }

        let mut navigator_hosts = self
            .workspace
            .workbench_session
            .active_layout_constraints
            .keys()
            .filter(|surface_host| matches!(surface_host, SurfaceHostId::Navigator(_)))
            .cloned()
            .collect::<Vec<_>>();
        navigator_hosts.sort_by_key(|surface_host| match surface_host {
            SurfaceHostId::Navigator(crate::app::workbench_layout_policy::NavigatorHostId::Top) => {
                0
            }
            SurfaceHostId::Navigator(
                crate::app::workbench_layout_policy::NavigatorHostId::Bottom,
            ) => 1,
            SurfaceHostId::Navigator(
                crate::app::workbench_layout_policy::NavigatorHostId::Left,
            ) => 2,
            SurfaceHostId::Navigator(
                crate::app::workbench_layout_policy::NavigatorHostId::Right,
            ) => 3,
            SurfaceHostId::Role(_) => 4,
        });
        navigator_hosts
            .into_iter()
            .next()
            .unwrap_or_else(|| self.preferred_default_navigator_surface_host())
    }

    pub fn visible_navigator_surface_hosts(&self) -> Vec<SurfaceHostId> {
        let mut navigator_hosts = self
            .workspace
            .workbench_session
            .active_layout_constraints
            .keys()
            .filter(|surface_host| matches!(surface_host, SurfaceHostId::Navigator(_)))
            .cloned()
            .collect::<Vec<_>>();
        navigator_hosts.sort_by_key(|surface_host| match surface_host {
            SurfaceHostId::Navigator(crate::app::workbench_layout_policy::NavigatorHostId::Top) => {
                0
            }
            SurfaceHostId::Navigator(
                crate::app::workbench_layout_policy::NavigatorHostId::Bottom,
            ) => 1,
            SurfaceHostId::Navigator(
                crate::app::workbench_layout_policy::NavigatorHostId::Left,
            ) => 2,
            SurfaceHostId::Navigator(
                crate::app::workbench_layout_policy::NavigatorHostId::Right,
            ) => 3,
            SurfaceHostId::Role(_) => 4,
        });

        if navigator_hosts.is_empty() {
            navigator_hosts.push(self.preferred_default_navigator_surface_host());
        }

        navigator_hosts
    }

    pub fn targetable_navigator_surface_host(&self) -> Option<SurfaceHostId> {
        if let UxConfigMode::Configuring { surface_host } =
            &self.workspace.workbench_session.ux_config_mode
        {
            return Some(surface_host.clone());
        }

        let navigator_hosts = self.visible_navigator_surface_hosts();
        (navigator_hosts.len() == 1).then(|| navigator_hosts[0].clone())
    }

    pub fn has_ambiguous_navigator_surface_host_target(&self) -> bool {
        matches!(
            self.workspace.workbench_session.ux_config_mode,
            UxConfigMode::Locked
        ) && self.visible_navigator_surface_hosts().len() > 1
    }

    pub fn set_workbench_surface_config_mode(&mut self, mode: UxConfigMode) {
        self.workspace.workbench_session.ux_config_mode = mode;
    }

    pub fn evaluate_workbench_layout_policy(
        &self,
        snapshot: &UxTreeSnapshot,
    ) -> Vec<WorkbenchIntent> {
        let mut effective_profile = self.workspace.workbench_session.workbench_profile.clone();
        for (surface_host, constraint) in &self.workspace.workbench_session.draft_layout_constraints
        {
            effective_profile.set_layout_constraint(surface_host.clone(), constraint.clone());
        }
        let report = evaluate_layout_policy_report(snapshot, &effective_profile);
        if report.diagnostics.conflict_count > 0 {
            emit_event(DiagnosticEvent::MessageSent {
                channel_id: CHANNEL_UX_LAYOUT_CONSTRAINT_CONFLICT,
                byte_len: report.diagnostics.conflict_count,
            });
        }
        if report.diagnostics.drift_count > 0 {
            emit_event(DiagnosticEvent::MessageSent {
                channel_id: CHANNEL_UX_LAYOUT_CONSTRAINT_DRIFT,
                byte_len: report.diagnostics.drift_count,
            });
        }
        report.intents
    }

    #[cfg(test)]
    pub fn pending_workbench_intent_count_for_tests(&self) -> usize {
        self.workspace
            .workbench_session
            .pending_workbench_intents
            .len()
    }

    pub fn workbench_tile_selection(&self) -> &WorkbenchTileSelectionState {
        &self.workbench_tile_selection
    }

    pub fn clear_workbench_tile_selection(&mut self) {
        self.workbench_tile_selection.selected_pane_ids.clear();
        self.workbench_tile_selection.primary_pane_id = None;
    }

    pub fn select_workbench_pane(&mut self, pane_id: PaneId) {
        self.update_workbench_pane_selection(pane_id, SelectionUpdateMode::Replace);
    }

    pub fn update_workbench_pane_selection(&mut self, pane_id: PaneId, mode: SelectionUpdateMode) {
        match mode {
            SelectionUpdateMode::Replace => {
                self.workbench_tile_selection.selected_pane_ids.clear();
                self.workbench_tile_selection
                    .selected_pane_ids
                    .insert(pane_id);
                self.workbench_tile_selection.primary_pane_id = Some(pane_id);
            }
            SelectionUpdateMode::Add => {
                self.workbench_tile_selection
                    .selected_pane_ids
                    .insert(pane_id);
                self.workbench_tile_selection.primary_pane_id = Some(pane_id);
            }
            SelectionUpdateMode::Toggle => {
                if self
                    .workbench_tile_selection
                    .selected_pane_ids
                    .remove(&pane_id)
                {
                    if self.workbench_tile_selection.primary_pane_id == Some(pane_id) {
                        self.workbench_tile_selection.primary_pane_id = self
                            .workbench_tile_selection
                            .selected_pane_ids
                            .iter()
                            .copied()
                            .next();
                    }
                } else {
                    self.workbench_tile_selection
                        .selected_pane_ids
                        .insert(pane_id);
                    self.workbench_tile_selection.primary_pane_id = Some(pane_id);
                }
            }
        }
    }

    pub fn prune_workbench_pane_selection_to_live_set(
        &mut self,
        live_pane_ids: &std::collections::HashSet<PaneId>,
    ) {
        self.workbench_tile_selection
            .selected_pane_ids
            .retain(|pane_id| live_pane_ids.contains(pane_id));
        if self
            .workbench_tile_selection
            .primary_pane_id
            .is_some_and(|pane_id| {
                !self
                    .workbench_tile_selection
                    .selected_pane_ids
                    .contains(&pane_id)
            })
        {
            self.workbench_tile_selection.primary_pane_id = self
                .workbench_tile_selection
                .selected_pane_ids
                .iter()
                .copied()
                .next();
        }
    }

    pub fn update_workbench_pane_selection_if_live(
        &mut self,
        live_pane_ids: &std::collections::HashSet<PaneId>,
        pane_id: PaneId,
        mode: SelectionUpdateMode,
    ) -> bool {
        self.prune_workbench_pane_selection_to_live_set(live_pane_ids);
        if !live_pane_ids.contains(&pane_id) {
            return false;
        }
        self.update_workbench_pane_selection(pane_id, mode);
        true
    }

    #[cfg(feature = "egui-host")]
    pub fn prune_workbench_pane_selection(&mut self, tiles_tree: &Tree<TileKind>) {
        let live_pane_ids: HashSet<PaneId> = tiles_tree
            .tiles
            .iter()
            .filter_map(|(_, tile)| match tile {
                Tile::Pane(kind) => Some(kind.pane_id()),
                _ => None,
            })
            .collect();
        self.prune_workbench_pane_selection_to_live_set(&live_pane_ids);
    }

    /// Persist a tile-group arrangement for the given selection.
    ///
    /// Extracts pane tile-kinds from the tree and selection, then delegates
    /// to the arrangement→graph bridge via [`ArrangementSnapshot::TileGroup`].
    #[cfg(feature = "egui-host")]
    pub(crate) fn persist_workbench_tile_group(
        &mut self,
        tiles_tree: &Tree<TileKind>,
        selected_pane_ids: &std::collections::HashSet<PaneId>,
    ) -> Option<NodeKey> {
        let pane_tile_kinds: Vec<TileKind> = tiles_tree
            .tiles
            .iter()
            .filter_map(|(_, tile)| match tile {
                Tile::Pane(kind) if selected_pane_ids.contains(&kind.pane_id()) => {
                    Some(kind.clone())
                }
                _ => None,
            })
            .collect();

        let snapshot = ArrangementSnapshot::TileGroup { pane_tile_kinds };
        let delta = self.apply_arrangement_snapshot(&snapshot);
        delta.container_node
    }

    /// Sync the graph representation of a named workbench frame.
    ///
    /// Extracts all pane tile-kinds from the tree, then delegates to the
    /// arrangement→graph bridge via [`ArrangementSnapshot::Frame`].
    #[cfg(feature = "egui-host")]
    pub(crate) fn sync_named_workbench_frame_graph_representation(
        &mut self,
        name: &str,
        tiles_tree: &Tree<TileKind>,
    ) -> NodeKey {
        let pane_tile_kinds: Vec<TileKind> = tiles_tree
            .tiles
            .iter()
            .filter_map(|(_, tile)| match tile {
                Tile::Pane(kind) => Some(kind.clone()),
                _ => None,
            })
            .collect();

        let snapshot = ArrangementSnapshot::Frame {
            name: name.to_string(),
            pane_tile_kinds,
        };
        let delta = self.apply_arrangement_snapshot(&snapshot);
        // Frame snapshots always produce a container node.
        delta
            .container_node
            .expect("frame snapshot must produce a container node")
    }

    /// Remove the graph representation of a named workbench frame.
    ///
    /// Delegates to the arrangement→graph bridge via
    /// [`ArrangementSnapshot::RemoveFrame`].
    pub(crate) fn remove_named_workbench_frame_graph_representation(&mut self, name: &str) {
        let snapshot = ArrangementSnapshot::RemoveFrame {
            name: name.to_string(),
        };
        let _ = self.apply_arrangement_snapshot(&snapshot);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::runtime_ports::diagnostics::DiagnosticsState;
    use crate::app::workbench_layout_policy::{AnchorEdge, NavigatorHostId};
    use crate::shell::desktop::workbench::ux_tree::{
        UxAction, UxDomainIdentity, UxNodeRole, UxNodeState, UxPresentationNode, UxSemanticNode,
        UxTraceSummary, UxTreeSnapshot,
    };

    fn channel_count(snapshot: &serde_json::Value, channel: &str) -> u64 {
        snapshot
            .get("channels")
            .and_then(|c| c.get("message_counts"))
            .and_then(|m| m.get(channel))
            .and_then(|v| v.as_u64())
            .unwrap_or(0)
    }

    fn navigator_snapshot(
        hosts: &[(SurfaceHostId, AnchorEdge, Option<[f32; 4]>)],
    ) -> UxTreeSnapshot {
        UxTreeSnapshot {
            semantic_version: 1,
            presentation_version: 1,
            trace_version: 1,
            semantic_nodes: hosts
                .iter()
                .enumerate()
                .map(|(index, (host, anchor_edge, _bounds))| UxSemanticNode {
                    ux_node_id: format!("uxnode://navigator/{index}"),
                    parent_ux_node_id: None,
                    role: UxNodeRole::NavigatorProjection,
                    label: "Navigator".to_string(),
                    state: UxNodeState {
                        focused: false,
                        selected: false,
                        blocked: false,
                        degraded: false,
                    },
                    allowed_actions: vec![UxAction::Focus],
                    domain: UxDomainIdentity::NavigatorProjection {
                        host: host.clone(),
                        anchor_edge: *anchor_edge,
                        form_factor: match anchor_edge {
                            AnchorEdge::Top | AnchorEdge::Bottom => "toolbar".to_string(),
                            AnchorEdge::Left | AnchorEdge::Right => "sidebar".to_string(),
                        },
                        scope: "both".to_string(),
                        projection_mode: "Workbench".to_string(),
                        projection_seed_source: "graph-containment".to_string(),
                        sort_mode: "manual".to_string(),
                        root_filter: None,
                        row_count: 0,
                        selected_count: 0,
                        expanded_count: 0,
                        collapsed_count: 0,
                        workbench_group_count: 0,
                        workbench_member_count: 0,
                        unrelated_count: 0,
                        recent_count: 0,
                    },
                })
                .collect(),
            presentation_nodes: hosts
                .iter()
                .enumerate()
                .map(
                    |(index, (_host, _anchor_edge, bounds))| UxPresentationNode {
                        ux_node_id: format!("uxnode://navigator/{index}"),
                        bounds: *bounds,
                        render_mode: None,
                        z_pass: "workbench.navigator.projection",
                        style_flags: vec!["surface:navigator"],
                        transient_flags: Vec::new(),
                    },
                )
                .collect(),
            trace_nodes: Vec::new(),
            trace_summary: UxTraceSummary {
                build_duration_us: 0,
                route_events_observed: 0,
                diagnostics_events_observed: 0,
            },
        }
    }

    fn named_surface_snapshot(
        ux_node_id: &str,
        label: &str,
        style_flags: Vec<&'static str>,
    ) -> UxTreeSnapshot {
        UxTreeSnapshot {
            semantic_version: 1,
            presentation_version: 1,
            trace_version: 1,
            semantic_nodes: vec![UxSemanticNode {
                ux_node_id: ux_node_id.to_string(),
                parent_ux_node_id: None,
                role: UxNodeRole::Workbench,
                label: label.to_string(),
                state: UxNodeState {
                    focused: false,
                    selected: false,
                    blocked: false,
                    degraded: false,
                },
                allowed_actions: vec![UxAction::Focus],
                domain: UxDomainIdentity::Workbench,
            }],
            presentation_nodes: vec![UxPresentationNode {
                ux_node_id: ux_node_id.to_string(),
                bounds: Some([0.0, 0.0, 300.0, 300.0]),
                render_mode: None,
                z_pass: "workbench.surface",
                style_flags,
                transient_flags: Vec::new(),
            }],
            trace_nodes: Vec::new(),
            trace_summary: UxTraceSummary {
                build_duration_us: 0,
                route_events_observed: 0,
                diagnostics_events_observed: 0,
            },
        }
    }

    #[cfg(feature = "diagnostics")]
    #[test]
    fn app_layout_policy_evaluation_emits_conflict_diagnostic_when_live_hosts_claim_same_edge() {
        let mut diagnostics = DiagnosticsState::new();
        let mut app = GraphBrowserApp::new_for_testing();
        let top_host = SurfaceHostId::Navigator(NavigatorHostId::Top);
        let bottom_host = SurfaceHostId::Navigator(NavigatorHostId::Bottom);
        app.set_workbench_layout_constraint(
            top_host.clone(),
            WorkbenchLayoutConstraint::anchored_split(top_host.clone(), AnchorEdge::Top, 0.2),
        );
        app.set_workbench_layout_constraint(
            bottom_host.clone(),
            WorkbenchLayoutConstraint::anchored_split(bottom_host.clone(), AnchorEdge::Top, 0.3),
        );

        let intents = app.evaluate_workbench_layout_policy(&navigator_snapshot(&[
            (top_host, AnchorEdge::Top, None),
            (bottom_host, AnchorEdge::Top, None),
        ]));

        assert!(intents.is_empty());
        diagnostics.force_drain_for_tests();
        let snapshot = diagnostics.snapshot_json_for_tests();
        assert_eq!(
            channel_count(&snapshot, CHANNEL_UX_LAYOUT_CONSTRAINT_CONFLICT),
            1
        );
    }

    #[cfg(feature = "diagnostics")]
    #[test]
    fn app_layout_policy_evaluation_emits_drift_diagnostic_when_live_bounds_collapse() {
        let mut diagnostics = DiagnosticsState::new();
        let mut app = GraphBrowserApp::new_for_testing();
        let host = SurfaceHostId::Navigator(NavigatorHostId::Left);
        app.set_workbench_layout_constraint(
            host.clone(),
            WorkbenchLayoutConstraint::anchored_split(host.clone(), AnchorEdge::Left, 0.25),
        );

        let intents = app.evaluate_workbench_layout_policy(&navigator_snapshot(&[(
            host,
            AnchorEdge::Left,
            Some([0.0, 0.0, 1.0, 300.0]),
        )]));

        assert_eq!(intents.len(), 1);
        diagnostics.force_drain_for_tests();
        let snapshot = diagnostics.snapshot_json_for_tests();
        assert_eq!(
            channel_count(&snapshot, CHANNEL_UX_LAYOUT_CONSTRAINT_DRIFT),
            1
        );
    }

    #[test]
    fn app_layout_policy_evaluation_excludes_fully_locked_surface_hosts() {
        let mut app = GraphBrowserApp::new_for_testing();
        let host = SurfaceHostId::Role(crate::app::workbench_layout_policy::SurfaceRole::Named(
            "locked-pane".to_string(),
        ));
        app.set_workbench_layout_constraint(
            host.clone(),
            WorkbenchLayoutConstraint::anchored_split(host.clone(), AnchorEdge::Right, 0.2),
        );

        let intents = app.evaluate_workbench_layout_policy(&named_surface_snapshot(
            "locked-pane",
            "Locked Pane",
            vec!["surface:node", "lock:fully-locked"],
        ));

        assert!(intents.is_empty());
    }

    #[test]
    fn app_layout_policy_evaluation_excludes_floating_and_fullscreen_surface_hosts() {
        let mut app = GraphBrowserApp::new_for_testing();
        let floating_host = SurfaceHostId::Role(
            crate::app::workbench_layout_policy::SurfaceRole::Named("floating-pane".to_string()),
        );
        let fullscreen_host = SurfaceHostId::Role(
            crate::app::workbench_layout_policy::SurfaceRole::Named("fullscreen-pane".to_string()),
        );
        app.set_workbench_layout_constraint(
            floating_host.clone(),
            WorkbenchLayoutConstraint::anchored_split(floating_host.clone(), AnchorEdge::Left, 0.2),
        );
        app.set_workbench_layout_constraint(
            fullscreen_host.clone(),
            WorkbenchLayoutConstraint::anchored_split(
                fullscreen_host.clone(),
                AnchorEdge::Top,
                0.2,
            ),
        );

        let floating_intents = app.evaluate_workbench_layout_policy(&named_surface_snapshot(
            "floating-pane",
            "Floating Pane",
            vec!["surface:node", "presentation:floating"],
        ));
        let fullscreen_intents = app.evaluate_workbench_layout_policy(&named_surface_snapshot(
            "fullscreen-pane",
            "Fullscreen Pane",
            vec!["surface:node", "presentation:fullscreen"],
        ));

        assert!(floating_intents.is_empty());
        assert!(fullscreen_intents.is_empty());
    }
}
