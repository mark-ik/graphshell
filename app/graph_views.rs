use super::*;
use crate::graph::scene_runtime::{
    GraphViewSceneRuntime, SceneRegionId, SceneRegionResizeHandle, SceneRegionRuntime,
    resize_region_to_pointer, translate_region,
};
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

fn emit_graph_view_region_mutation_diagnostic(byte_len: usize) {
    emit_event(DiagnosticEvent::MessageSent {
        channel_id:
            crate::app::runtime_ports::registries::CHANNEL_UI_GRAPH_VIEW_REGION_MUTATION_APPLIED,
        byte_len: byte_len.max(1),
    });
}

fn emit_graph_view_transfer_succeeded_diagnostic(byte_len: usize) {
    emit_event(DiagnosticEvent::MessageSent {
        channel_id: crate::app::runtime_ports::registries::CHANNEL_UI_GRAPH_VIEW_TRANSFER_SUCCEEDED,
        byte_len: byte_len.max(1),
    });
}

fn emit_graph_view_transfer_blocked_diagnostic() {
    emit_event(DiagnosticEvent::MessageReceived {
        channel_id: crate::app::runtime_ports::registries::CHANNEL_UI_GRAPH_VIEW_TRANSFER_BLOCKED,
        latency_us: 0,
    });
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

// `GraphViewId` moved to `graphshell_core::graph` in M4 slice 10
// (2026-04-22) alongside `NodeKey`. Re-exported at the original path
// so callers resolve unchanged.
pub use graphshell_core::graph::GraphViewId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub enum SceneMode {
    #[default]
    Browse,
    Arrange,
    Simulate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub enum SimulateBehaviorPreset {
    #[default]
    Float,
    Packed,
    Magnetic,
}

impl GraphBrowserApp {
    pub fn graph_view_scene_mode(&self, view_id: GraphViewId) -> SceneMode {
        self.workspace
            .graph_runtime
            .views
            .get(&view_id)
            .map(|view| view.scene_mode)
            .unwrap_or_default()
    }

    pub fn set_graph_view_scene_mode(&mut self, view_id: GraphViewId, mode: SceneMode) {
        if !self.workspace.graph_runtime.views.contains_key(&view_id) {
            let name = self.next_graph_view_slot_name();
            self.workspace
                .graph_runtime
                .views
                .insert(view_id, GraphViewState::new_with_id(view_id, name));
        }
        let view = self
            .workspace
            .graph_runtime
            .views
            .get_mut(&view_id)
            .expect("view inserted above");
        if view.scene_mode == mode {
            return;
        }
        view.scene_mode = mode;
    }

    pub fn graph_view_scene_reveal_nodes(&self, view_id: GraphViewId) -> bool {
        self.workspace
            .graph_runtime
            .views
            .get(&view_id)
            .is_some_and(|view| view.scene_reveal_nodes)
    }

    pub fn set_graph_view_scene_reveal_nodes(&mut self, view_id: GraphViewId, enabled: bool) {
        if !self.workspace.graph_runtime.views.contains_key(&view_id) {
            let name = self.next_graph_view_slot_name();
            self.workspace
                .graph_runtime
                .views
                .insert(view_id, GraphViewState::new_with_id(view_id, name));
        }
        let view = self
            .workspace
            .graph_runtime
            .views
            .get_mut(&view_id)
            .expect("view inserted above");
        if view.scene_reveal_nodes == enabled {
            return;
        }
        view.scene_reveal_nodes = enabled;
    }

    pub fn graph_view_scene_relation_xray(&self, view_id: GraphViewId) -> bool {
        self.workspace
            .graph_runtime
            .views
            .get(&view_id)
            .is_some_and(|view| view.scene_relation_xray)
    }

    pub fn set_graph_view_scene_relation_xray(&mut self, view_id: GraphViewId, enabled: bool) {
        if !self.workspace.graph_runtime.views.contains_key(&view_id) {
            let name = self.next_graph_view_slot_name();
            self.workspace
                .graph_runtime
                .views
                .insert(view_id, GraphViewState::new_with_id(view_id, name));
        }
        let view = self
            .workspace
            .graph_runtime
            .views
            .get_mut(&view_id)
            .expect("view inserted above");
        if view.scene_relation_xray == enabled {
            return;
        }
        view.scene_relation_xray = enabled;
    }

    pub fn graph_view_simulate_behavior_preset(
        &self,
        view_id: GraphViewId,
    ) -> SimulateBehaviorPreset {
        self.workspace
            .graph_runtime
            .views
            .get(&view_id)
            .map(|view| view.simulate_behavior_preset)
            .unwrap_or_default()
    }

    pub fn set_graph_view_simulate_behavior_preset(
        &mut self,
        view_id: GraphViewId,
        preset: SimulateBehaviorPreset,
    ) {
        if !self.workspace.graph_runtime.views.contains_key(&view_id) {
            let name = self.next_graph_view_slot_name();
            self.workspace
                .graph_runtime
                .views
                .insert(view_id, GraphViewState::new_with_id(view_id, name));
        }
        let view = self
            .workspace
            .graph_runtime
            .views
            .get_mut(&view_id)
            .expect("view inserted above");
        if view.simulate_behavior_preset == preset {
            return;
        }
        view.simulate_behavior_preset = preset;
    }

    pub fn graph_view_scene_runtime(&self, view_id: GraphViewId) -> Option<&GraphViewSceneRuntime> {
        self.workspace.graph_runtime.scene_runtimes.get(&view_id)
    }

    /// Resolve the effective navigation policy for a view. Order of
    /// precedence: view-level `navigation_policy_override` → the
    /// per-graph `DomainState::navigation_policy_default` → the
    /// portable `NavigationPolicy::default()` baseline.
    ///
    /// Hosts (egui today, iced once it comes up) call this per frame
    /// to read camera / input / inertia knobs in one place rather than
    /// threading individual fields through the render bridge.
    pub fn resolve_navigation_policy(
        &self,
        view_id: GraphViewId,
    ) -> graph_canvas::navigation::NavigationPolicy {
        if let Some(view) = self.workspace.graph_runtime.views.get(&view_id)
            && let Some(policy) = view.navigation_policy_override
        {
            return policy;
        }
        self.workspace.domain.navigation_policy_default
    }

    /// Set a per-view navigation policy override. Pass `None` to
    /// revert the view to the per-graph default.
    pub fn set_graph_view_navigation_policy_override(
        &mut self,
        view_id: GraphViewId,
        policy: Option<graph_canvas::navigation::NavigationPolicy>,
    ) {
        if let Some(view) = self.workspace.graph_runtime.views.get_mut(&view_id) {
            view.navigation_policy_override = policy;
        }
    }

    /// Set the per-graph default navigation policy. All views without
    /// their own override inherit this.
    pub fn set_navigation_policy_default(
        &mut self,
        policy: graph_canvas::navigation::NavigationPolicy,
    ) {
        self.workspace.domain.navigation_policy_default = policy;
    }

    /// Resolve the effective node style for a view. Precedence order:
    /// view-level `node_style_override` → per-graph `node_style_default`
    /// on `DomainState` → `NodeStyle::default()` baseline.
    ///
    /// Called once per frame from the render bridge to pick up node
    /// radius, selection fills, and search-hit highlight colors.
    pub fn resolve_node_style(&self, view_id: GraphViewId) -> graph_canvas::node_style::NodeStyle {
        if let Some(view) = self.workspace.graph_runtime.views.get(&view_id)
            && let Some(style) = view.node_style_override
        {
            return style;
        }
        self.workspace.domain.node_style_default
    }

    /// Set a per-view node-style override. `None` reverts to the
    /// per-graph default.
    pub fn set_graph_view_node_style_override(
        &mut self,
        view_id: GraphViewId,
        style: Option<graph_canvas::node_style::NodeStyle>,
    ) {
        if let Some(view) = self.workspace.graph_runtime.views.get_mut(&view_id) {
            view.node_style_override = style;
        }
    }

    /// Set the per-graph default node style. All views without their
    /// own override inherit this.
    pub fn set_node_style_default(&mut self, style: graph_canvas::node_style::NodeStyle) {
        self.workspace.domain.node_style_default = style;
    }

    /// Resolve the effective simulate-motion profile for a view.
    /// Precedence:
    /// 1. view-level `simulate_motion_override`
    /// 2. per-graph `DomainState::simulate_motion_default`
    /// 3. `SimulateMotionProfile::for_preset(view.simulate_behavior_preset)`
    ///    — preserves the preset-driven fallback that predates this policy.
    pub fn resolve_simulate_motion_profile(
        &self,
        view_id: GraphViewId,
    ) -> graph_canvas::scene_physics::SimulateMotionProfile {
        if let Some(view) = self.workspace.graph_runtime.views.get(&view_id) {
            if let Some(profile) = view.simulate_motion_override {
                return profile;
            }
            if let Some(profile) = self.workspace.domain.simulate_motion_default {
                return profile;
            }
            // The app-side `SimulateBehaviorPreset` duplicates the
            // portable one in graph-canvas pre-dating this policy;
            // map across for `for_preset`. Follow-on: dedupe the
            // enums once a settings UI surfaces the preset picker.
            let portable_preset = match view.simulate_behavior_preset {
                SimulateBehaviorPreset::Float => {
                    graph_canvas::scene_physics::SimulateBehaviorPreset::Float
                }
                SimulateBehaviorPreset::Packed => {
                    graph_canvas::scene_physics::SimulateBehaviorPreset::Packed
                }
                SimulateBehaviorPreset::Magnetic => {
                    graph_canvas::scene_physics::SimulateBehaviorPreset::Magnetic
                }
            };
            return graph_canvas::scene_physics::SimulateMotionProfile::for_preset(portable_preset);
        }
        // No such view — return the app-wide graph default or baseline.
        self.workspace
            .domain
            .simulate_motion_default
            .unwrap_or_default()
    }

    /// Set a per-view simulate-motion override. `None` reverts to the
    /// per-graph default (or the preset-driven fallback).
    pub fn set_graph_view_simulate_motion_override(
        &mut self,
        view_id: GraphViewId,
        profile: Option<graph_canvas::scene_physics::SimulateMotionProfile>,
    ) {
        if let Some(view) = self.workspace.graph_runtime.views.get_mut(&view_id) {
            view.simulate_motion_override = profile;
        }
    }

    /// Set the per-graph default simulate-motion profile. `None` drops
    /// the per-graph override so views fall back to preset-driven.
    pub fn set_simulate_motion_default(
        &mut self,
        profile: Option<graph_canvas::scene_physics::SimulateMotionProfile>,
    ) {
        self.workspace.domain.simulate_motion_default = profile;
    }

    pub fn graph_view_selected_scene_region(&self, view_id: GraphViewId) -> Option<SceneRegionId> {
        self.workspace
            .graph_runtime
            .selected_scene_regions
            .get(&view_id)
            .copied()
            .filter(|region_id| {
                self.workspace
                    .graph_runtime
                    .scene_runtimes
                    .get(&view_id)
                    .is_some_and(|runtime| {
                        runtime.regions.iter().any(|region| region.id == *region_id)
                    })
            })
    }

    pub fn graph_view_scene_region(
        &self,
        view_id: GraphViewId,
        region_id: SceneRegionId,
    ) -> Option<&SceneRegionRuntime> {
        self.workspace
            .graph_runtime
            .scene_runtimes
            .get(&view_id)
            .and_then(|runtime| runtime.regions.iter().find(|region| region.id == region_id))
    }

    fn ensure_graph_view_scene_runtime_entry(
        &mut self,
        view_id: GraphViewId,
    ) -> &mut GraphViewSceneRuntime {
        self.workspace
            .graph_runtime
            .scene_runtimes
            .entry(view_id)
            .or_default()
    }

    pub fn set_graph_view_selected_scene_region(
        &mut self,
        view_id: GraphViewId,
        region_id: Option<SceneRegionId>,
    ) {
        if let Some(region_id) = region_id {
            let exists = self
                .workspace
                .graph_runtime
                .scene_runtimes
                .get(&view_id)
                .is_some_and(|runtime| runtime.regions.iter().any(|region| region.id == region_id));
            if exists {
                self.workspace
                    .graph_runtime
                    .selected_scene_regions
                    .insert(view_id, region_id);
            } else {
                self.workspace
                    .graph_runtime
                    .selected_scene_regions
                    .remove(&view_id);
            }
        } else {
            self.workspace
                .graph_runtime
                .selected_scene_regions
                .remove(&view_id);
        }
    }

    pub fn set_graph_view_scene_bounds_override(
        &mut self,
        view_id: GraphViewId,
        bounds: Option<egui::Rect>,
    ) {
        self.ensure_graph_view_registered(view_id);
        if let Some(bounds) = bounds {
            self.ensure_graph_view_scene_runtime_entry(view_id)
                .bounds_override = Some(bounds);
        } else if let Some(runtime) = self
            .workspace
            .graph_runtime
            .scene_runtimes
            .get_mut(&view_id)
        {
            runtime.bounds_override = None;
            if runtime.regions.is_empty() {
                self.workspace.graph_runtime.scene_runtimes.remove(&view_id);
            }
        }
    }

    pub fn set_graph_view_scene_regions(
        &mut self,
        view_id: GraphViewId,
        regions: Vec<SceneRegionRuntime>,
    ) {
        self.ensure_graph_view_registered(view_id);
        if regions.is_empty() {
            if let Some(runtime) = self
                .workspace
                .graph_runtime
                .scene_runtimes
                .get_mut(&view_id)
            {
                runtime.regions.clear();
                if runtime.bounds_override.is_none() {
                    self.workspace.graph_runtime.scene_runtimes.remove(&view_id);
                }
            }
        } else {
            self.ensure_graph_view_scene_runtime_entry(view_id).regions = regions;
        }
        if self.graph_view_selected_scene_region(view_id).is_none() {
            self.workspace
                .graph_runtime
                .selected_scene_regions
                .remove(&view_id);
        }
    }

    pub fn add_graph_view_scene_region(
        &mut self,
        view_id: GraphViewId,
        region: SceneRegionRuntime,
    ) {
        self.ensure_graph_view_registered(view_id);
        self.ensure_graph_view_scene_runtime_entry(view_id)
            .regions
            .push(region);
    }

    pub fn translate_graph_view_scene_region(
        &mut self,
        view_id: GraphViewId,
        region_id: SceneRegionId,
        delta: egui::Vec2,
    ) -> bool {
        if delta.length_sq() <= f32::EPSILON {
            return false;
        }
        let Some(runtime) = self
            .workspace
            .graph_runtime
            .scene_runtimes
            .get_mut(&view_id)
        else {
            self.workspace
                .graph_runtime
                .selected_scene_regions
                .remove(&view_id);
            return false;
        };
        let Some(region) = runtime
            .regions
            .iter_mut()
            .find(|region| region.id == region_id)
        else {
            self.workspace
                .graph_runtime
                .selected_scene_regions
                .remove(&view_id);
            return false;
        };
        translate_region(region, delta);
        true
    }

    pub fn resize_graph_view_scene_region_to_pointer(
        &mut self,
        view_id: GraphViewId,
        region_id: SceneRegionId,
        handle: SceneRegionResizeHandle,
        pointer_canvas_pos: egui::Pos2,
    ) -> bool {
        let Some(runtime) = self
            .workspace
            .graph_runtime
            .scene_runtimes
            .get_mut(&view_id)
        else {
            self.workspace
                .graph_runtime
                .selected_scene_regions
                .remove(&view_id);
            return false;
        };
        let Some(region) = runtime
            .regions
            .iter_mut()
            .find(|region| region.id == region_id)
        else {
            self.workspace
                .graph_runtime
                .selected_scene_regions
                .remove(&view_id);
            return false;
        };
        resize_region_to_pointer(region, handle, pointer_canvas_pos);
        true
    }

    pub fn replace_graph_view_scene_region(
        &mut self,
        view_id: GraphViewId,
        region: SceneRegionRuntime,
    ) -> bool {
        let Some(runtime) = self
            .workspace
            .graph_runtime
            .scene_runtimes
            .get_mut(&view_id)
        else {
            self.workspace
                .graph_runtime
                .selected_scene_regions
                .remove(&view_id);
            return false;
        };
        let Some(existing) = runtime
            .regions
            .iter_mut()
            .find(|existing| existing.id == region.id)
        else {
            self.workspace
                .graph_runtime
                .selected_scene_regions
                .remove(&view_id);
            return false;
        };
        *existing = region;
        true
    }

    pub fn remove_graph_view_scene_region(
        &mut self,
        view_id: GraphViewId,
        region_id: SceneRegionId,
    ) -> bool {
        let Some(runtime) = self
            .workspace
            .graph_runtime
            .scene_runtimes
            .get_mut(&view_id)
        else {
            self.workspace
                .graph_runtime
                .selected_scene_regions
                .remove(&view_id);
            return false;
        };
        let original_len = runtime.regions.len();
        runtime.regions.retain(|region| region.id != region_id);
        let removed = runtime.regions.len() != original_len;
        if !removed {
            return false;
        }
        if runtime.regions.is_empty() && runtime.bounds_override.is_none() {
            self.workspace.graph_runtime.scene_runtimes.remove(&view_id);
        }
        if self.graph_view_selected_scene_region(view_id) == Some(region_id) {
            self.workspace
                .graph_runtime
                .selected_scene_regions
                .remove(&view_id);
        }
        self.workspace.graph_runtime.hovered_scene_region =
            self.workspace.graph_runtime.hovered_scene_region.filter(
                |(hovered_view_id, hovered_region_id)| {
                    *hovered_view_id != view_id || *hovered_region_id != region_id
                },
            );
        self.workspace.graph_runtime.active_scene_region_drag = self
            .workspace
            .graph_runtime
            .active_scene_region_drag
            .filter(|drag| drag.view_id != view_id || drag.region_id != region_id);
        true
    }

    pub fn clear_graph_view_scene_runtime(&mut self, view_id: GraphViewId) {
        self.workspace.graph_runtime.scene_runtimes.remove(&view_id);
        self.workspace
            .graph_runtime
            .selected_scene_regions
            .remove(&view_id);
        self.workspace.graph_runtime.hovered_scene_region = self
            .workspace
            .graph_runtime
            .hovered_scene_region
            .filter(|(hovered_view_id, _)| *hovered_view_id != view_id);
        self.workspace.graph_runtime.active_scene_region_drag = self
            .workspace
            .graph_runtime
            .active_scene_region_drag
            .filter(|drag| drag.view_id != view_id);
    }

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
            .and_then(|view| {
                view.relation_policy
                    .edge_projection_override
                    .as_ref()
                    .or(view.edge_projection_override.as_ref())
            })
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
    }

    pub fn set_graph_view_edge_projection_override(
        &mut self,
        view_id: GraphViewId,
        selectors: Option<Vec<RelationSelector>>,
    ) {
        let Some(view) = self.workspace.graph_runtime.views.get_mut(&view_id) else {
            return;
        };
        view.apply_edge_projection_policy_override(selectors.map(EdgeProjectionState::new));
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
            self.workspace.graph_runtime.physics.is_running,
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
            self.workspace.graph_runtime.physics.is_running,
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

    fn graph_view_slot_at_position(
        &self,
        row: i32,
        col: i32,
        except_view: Option<GraphViewId>,
    ) -> Option<GraphViewId> {
        self.workspace
            .graph_runtime
            .graph_view_layout_manager
            .slots
            .values()
            .find_map(|slot| {
                (!slot.archived
                    && Some(slot.view_id) != except_view
                    && slot.row == row
                    && slot.col == col)
                    .then_some(slot.view_id)
            })
    }

    fn graph_view_slot_position_occupied(
        &self,
        row: i32,
        col: i32,
        except_view: Option<GraphViewId>,
    ) -> bool {
        self.graph_view_slot_at_position(row, col, except_view)
            .is_some()
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

    fn graph_view_ownership_partition_active(&self) -> bool {
        self.workspace
            .graph_runtime
            .views
            .values()
            .any(|view| view.graphlet_node_mask.is_none() && view.owned_node_mask.is_some())
    }

    fn initialize_owned_node_mask_for_new_view(&self) -> Option<HashSet<NodeKey>> {
        self.graph_view_ownership_partition_active()
            .then(HashSet::new)
    }

    fn active_ordinary_graph_view_ids(&self) -> Vec<GraphViewId> {
        self.workspace
            .graph_runtime
            .graph_view_layout_manager
            .slots
            .values()
            .filter(|slot| !slot.archived)
            .filter_map(|slot| {
                self.workspace
                    .graph_runtime
                    .views
                    .get(&slot.view_id)
                    .filter(|view| view.graphlet_node_mask.is_none())
                    .map(|_| slot.view_id)
            })
            .collect()
    }

    fn all_domain_node_keys(&self) -> HashSet<NodeKey> {
        self.workspace
            .domain
            .graph
            .nodes()
            .map(|(key, _)| key)
            .collect()
    }

    fn materialize_graph_view_node_ownership(&mut self, source_view: GraphViewId) {
        let ordinary_views = self.active_ordinary_graph_view_ids();
        if ordinary_views.is_empty() || !ordinary_views.contains(&source_view) {
            return;
        }

        let all_nodes = self.all_domain_node_keys();
        let mut explicitly_owned = HashSet::new();
        for view_id in &ordinary_views {
            if let Some(mask) = self
                .workspace
                .graph_runtime
                .views
                .get(view_id)
                .and_then(|view| view.owned_node_mask.as_ref())
            {
                explicitly_owned.extend(mask.iter().copied());
            }
        }

        let unowned: HashSet<NodeKey> = all_nodes.difference(&explicitly_owned).copied().collect();

        for view_id in ordinary_views {
            if let Some(view) = self.workspace.graph_runtime.views.get_mut(&view_id) {
                if view.owned_node_mask.is_none() {
                    view.owned_node_mask = Some(HashSet::new());
                }
                if view_id == source_view
                    && let Some(mask) = view.owned_node_mask.as_mut()
                {
                    mask.extend(unowned.iter().copied());
                }
            }
        }
    }

    pub fn graph_view_owned_node_count(&self, view_id: GraphViewId) -> Option<usize> {
        self.workspace
            .graph_runtime
            .views
            .get(&view_id)
            .and_then(|view| view.owned_node_mask.as_ref())
            .map(HashSet::len)
    }

    pub fn graph_view_has_explicit_ownership(&self, view_id: GraphViewId) -> bool {
        self.workspace
            .graph_runtime
            .views
            .get(&view_id)
            .is_some_and(|view| view.owned_node_mask.is_some())
    }

    pub fn graph_view_external_link_count(&self, view_id: GraphViewId) -> usize {
        let ownership: HashMap<NodeKey, GraphViewId> = self
            .workspace
            .graph_runtime
            .views
            .iter()
            .filter(|(_, view)| view.graphlet_node_mask.is_none())
            .flat_map(|(&owned_view_id, view)| {
                view.owned_node_mask.iter().flat_map(move |mask| {
                    mask.iter().copied().map(move |node| (node, owned_view_id))
                })
            })
            .collect();
        if ownership.is_empty() {
            return 0;
        }

        self.workspace
            .domain
            .graph
            .edges()
            .filter(|edge| {
                let Some(from_view) = ownership.get(&edge.from) else {
                    return false;
                };
                let Some(to_view) = ownership.get(&edge.to) else {
                    return false;
                };
                (*from_view == view_id && *to_view != view_id)
                    || (*to_view == view_id && *from_view != view_id)
            })
            .count()
    }

    pub(crate) fn transfer_selected_nodes_to_graph_view(
        &mut self,
        source_view: GraphViewId,
        destination_view: GraphViewId,
    ) {
        if source_view == destination_view {
            emit_graph_view_transfer_blocked_diagnostic();
            return;
        }

        let Some(source_slot) = self
            .workspace
            .graph_runtime
            .graph_view_layout_manager
            .slots
            .get(&source_view)
        else {
            emit_graph_view_transfer_blocked_diagnostic();
            return;
        };
        let Some(destination_slot) = self
            .workspace
            .graph_runtime
            .graph_view_layout_manager
            .slots
            .get(&destination_view)
        else {
            emit_graph_view_transfer_blocked_diagnostic();
            return;
        };
        if source_slot.archived || destination_slot.archived {
            emit_graph_view_transfer_blocked_diagnostic();
            return;
        }
        if self
            .workspace
            .graph_runtime
            .views
            .get(&source_view)
            .is_some_and(|view| view.graphlet_node_mask.is_some())
            || self
                .workspace
                .graph_runtime
                .views
                .get(&destination_view)
                .is_some_and(|view| view.graphlet_node_mask.is_some())
        {
            emit_graph_view_transfer_blocked_diagnostic();
            return;
        }

        let selected_nodes =
            sorted_unique_node_keys(self.selection_for_view(source_view).iter().copied());
        if selected_nodes.is_empty() {
            emit_graph_view_transfer_blocked_diagnostic();
            return;
        }

        self.materialize_graph_view_node_ownership(source_view);
        let selected_set: HashSet<NodeKey> = selected_nodes.iter().copied().collect();
        for view_id in self.active_ordinary_graph_view_ids() {
            if let Some(mask) = self
                .workspace
                .graph_runtime
                .views
                .get_mut(&view_id)
                .and_then(|view| view.owned_node_mask.as_mut())
            {
                mask.retain(|node| !selected_set.contains(node));
            }
        }

        if let Some(view) = self
            .workspace
            .graph_runtime
            .views
            .get_mut(&destination_view)
        {
            let mask = view.owned_node_mask.get_or_insert_with(HashSet::new);
            mask.extend(selected_nodes.iter().copied());
        }

        let source_scope = SelectionScope::View(source_view);
        let destination_scope = SelectionScope::View(destination_view);
        if let Some(selection) = self
            .workspace
            .graph_runtime
            .selection_by_scope
            .get_mut(&source_scope)
        {
            selection.retain_nodes(|node| !selected_set.contains(&node));
        }
        self.workspace
            .graph_runtime
            .selection_by_scope
            .retain(|_, selection| !selection.is_empty());

        let mut destination_selection = SelectionState::new();
        destination_selection.update_many(selected_nodes, SelectionUpdateMode::Replace);
        self.workspace
            .graph_runtime
            .selection_by_scope
            .insert(destination_scope, destination_selection);
        self.sync_selection_edge_projection_override_for_scope(source_scope);
        self.sync_selection_edge_projection_override_for_scope(destination_scope);
        self.set_workspace_focused_view_with_transition(Some(destination_view));
        emit_graph_view_transfer_succeeded_diagnostic(selected_set.len());
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
            state.owned_node_mask = self.initialize_owned_node_mask_for_new_view();
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
        state.owned_node_mask = self.initialize_owned_node_mask_for_new_view();
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
        emit_graph_view_region_mutation_diagnostic(1);
    }

    pub(crate) fn rename_graph_view_slot(&mut self, view_id: GraphViewId, name: String) {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return;
        }
        let mut persisted_name_len = None;
        if let Some(slot) = self
            .workspace
            .graph_runtime
            .graph_view_layout_manager
            .slots
            .get_mut(&view_id)
        {
            slot.name = trimmed.to_string();
            persisted_name_len = Some(slot.name.len());
            if let Some(view) = self.workspace.graph_runtime.views.get_mut(&view_id) {
                view.name = slot.name.clone();
            }
        }
        if let Some(name_len) = persisted_name_len {
            self.persist_graph_view_layout_manager_state();
            emit_graph_view_region_mutation_diagnostic(name_len);
        }
    }

    pub(crate) fn refresh_registry_backed_view_lenses(&mut self) -> usize {
        let refreshes: Vec<(GraphViewId, ResolvedLensPreset)> = self
            .workspace
            .graph_runtime
            .views
            .iter()
            .filter_map(|(&view_id, view)| {
                let requested = view
                    .resolved_lens_id()
                    .filter(|lens_id| !lens_id.trim().is_empty())
                    .map(str::to_string)
                    .or_else(|| {
                        view.resolved_lens_display_name()
                            .starts_with("lens:")
                            .then(|| view.resolved_lens_display_name().to_ascii_lowercase())
                    })?;
                Some((
                    view_id,
                    crate::app::runtime_ports::registries::phase2_resolve_lens(&requested),
                ))
            })
            .collect();

        let refresh_count = refreshes.len();
        for (view_id, lens) in refreshes {
            if let Some(view) = self.workspace.graph_runtime.views.get_mut(&view_id) {
                view.apply_resolved_lens_identity(lens);
            }
        }

        refresh_count
    }

    pub(crate) fn move_graph_view_slot(&mut self, view_id: GraphViewId, row: i32, col: i32) {
        let Some((current_row, current_col)) = self
            .workspace
            .graph_runtime
            .graph_view_layout_manager
            .slots
            .get(&view_id)
            .map(|slot| (slot.row, slot.col))
        else {
            return;
        };
        if current_row == row && current_col == col {
            return;
        }

        let occupant_view_id = self.graph_view_slot_at_position(row, col, Some(view_id));
        if let Some(occupant_view_id) = occupant_view_id
            && let Some(occupant_slot) = self
                .workspace
                .graph_runtime
                .graph_view_layout_manager
                .slots
                .get_mut(&occupant_view_id)
        {
            occupant_slot.row = current_row;
            occupant_slot.col = current_col;
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
            emit_graph_view_region_mutation_diagnostic(1);
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
            emit_graph_view_region_mutation_diagnostic(1);
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
            emit_graph_view_region_mutation_diagnostic(1);
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

#[derive(Debug, Clone, serde::Deserialize)]
struct LegacyLensConfig {
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

/// Temporary deserialization shim for pre-decomposition `GraphViewState`
/// snapshots that stored a nested `lens` bundle directly on the view.
#[derive(Debug, Clone)]
pub struct ResolvedLensPreset {
    pub lens_id: String,
    pub display_name: String,
    pub physics: PhysicsProfile,
    pub layout: LayoutMode,
    pub layout_algorithm_id: String,
    pub theme: Option<ThemeData>,
    pub filter_expr: Option<crate::model::graph::filter::FacetExpr>,
    pub filters_legacy: Vec<String>,
    pub overlay_descriptor: Option<crate::registries::atomic::lens::LensOverlayDescriptor>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ViewLensState {
    #[serde(default)]
    pub base_lens_id: Option<String>,
    #[serde(default)]
    pub display_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PolicyValueSource {
    RegistryDefault,
    WorkspaceDefault,
    LensPreset(String),
    ViewOverride,
    LegacySnapshot,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ViewLayoutPolicy {
    pub mode: LayoutMode,
    pub algorithm_id: String,
    #[serde(default = "default_registry_policy_value_source")]
    pub source: PolicyValueSource,
}

impl Default for ViewLayoutPolicy {
    fn default() -> Self {
        Self {
            mode: LayoutMode::Free,
            algorithm_id: crate::app::graph_layout::default_free_layout_algorithm_id(),
            source: default_registry_policy_value_source(),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ViewPhysicsPolicy {
    #[serde(default)]
    pub profile_id: Option<String>,
    pub profile: PhysicsProfile,
    #[serde(default = "default_registry_policy_value_source")]
    pub source: PolicyValueSource,
}

impl Default for ViewPhysicsPolicy {
    fn default() -> Self {
        let resolution = crate::registries::atomic::lens::resolve_physics_profile(
            crate::registries::atomic::lens::PHYSICS_ID_DEFAULT,
        );
        Self {
            profile_id: Some(resolution.resolved_id),
            profile: resolution.profile,
            source: default_registry_policy_value_source(),
        }
    }
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ViewFilterPolicy {
    #[serde(default)]
    pub lens_filter_expr: Option<crate::model::graph::filter::FacetExpr>,
    #[serde(default)]
    pub lens_filter_source: Option<PolicyValueSource>,
    #[serde(default)]
    pub active_filter_override: Option<crate::model::graph::filter::FacetExpr>,
    #[serde(default)]
    pub active_filter_override_source: Option<PolicyValueSource>,
    #[serde(default)]
    pub legacy_filters: Vec<String>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ViewOverlayPolicy {
    #[serde(skip, default)]
    pub overlay_descriptor: Option<crate::registries::atomic::lens::LensOverlayDescriptor>,
    #[serde(default)]
    pub source: Option<PolicyValueSource>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ViewPresentationPolicy {
    #[serde(default, deserialize_with = "deserialize_optional_theme_data")]
    pub theme: Option<ThemeData>,
    #[serde(default)]
    pub source: Option<PolicyValueSource>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ViewRelationPolicy {
    #[serde(default)]
    pub edge_projection_override: Option<EdgeProjectionState>,
}

fn default_registry_policy_value_source() -> PolicyValueSource {
    PolicyValueSource::RegistryDefault
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

#[derive(serde::Serialize)]
pub struct GraphViewState {
    pub id: GraphViewId,
    pub name: String,
    pub camera: Camera,
    #[serde(default)]
    pub position_fit_locked: bool,
    #[serde(default)]
    pub zoom_fit_locked: bool,
    #[serde(default)]
    pub lens_state: ViewLensState,
    #[serde(default)]
    pub layout_policy: ViewLayoutPolicy,
    #[serde(default)]
    pub physics_policy: ViewPhysicsPolicy,
    #[serde(default)]
    pub filter_policy: ViewFilterPolicy,
    #[serde(default)]
    pub overlay_policy: ViewOverlayPolicy,
    #[serde(default)]
    pub presentation_policy: ViewPresentationPolicy,
    #[serde(default)]
    pub relation_policy: ViewRelationPolicy,
    /// Per-view navigation-policy override. `None` falls back to the
    /// per-graph default resolved via `DomainState::navigation_policy_default`.
    /// See `resolve_navigation_policy(app, view_id)`.
    #[serde(default)]
    pub navigation_policy_override: Option<graph_canvas::navigation::NavigationPolicy>,
    /// Per-view node-style override (node radius, selection / search-hit
    /// appearance). `None` falls back to the per-graph default resolved
    /// via `DomainState::node_style_default`. See
    /// `resolve_node_style(app, view_id)`.
    #[serde(default)]
    pub node_style_override: Option<graph_canvas::node_style::NodeStyle>,
    /// Per-view simulate-motion-profile override. `None` falls back
    /// first to the per-graph `DomainState::simulate_motion_default`,
    /// then to `SimulateMotionProfile::for_preset(simulate_behavior_preset)`
    /// so existing preset pickers keep working untouched. See
    /// `resolve_simulate_motion_profile(app, view_id)`.
    #[serde(default)]
    pub simulate_motion_override: Option<graph_canvas::scene_physics::SimulateMotionProfile>,
    pub local_simulation: Option<LocalSimulation>,
    /// The rendering dimension for this view (2D or 3D sub-mode).
    ///
    /// Persisted with the view state so that reopening a frame restores the
    /// user's last dimension choice.  Snapshot degradation: falls back to `TwoD`
    /// if 3D rendering is unavailable on the target platform.
    #[serde(default)]
    pub dimension: ViewDimension,
    /// User-facing scene interaction mode for this graph view.
    #[serde(default)]
    pub scene_mode: SceneMode,
    /// Whether simulate mode should explicitly halo all node-objects for legibility.
    #[serde(default)]
    pub scene_reveal_nodes: bool,
    /// Whether simulate mode should draw a scoped relation x-ray around the current focus node.
    #[serde(default)]
    pub scene_relation_xray: bool,
    /// Lightweight per-view behavior preset for the current non-Rapier simulate mode.
    #[serde(default)]
    pub simulate_behavior_preset: SimulateBehaviorPreset,
    #[serde(skip)]
    pub last_layout_algorithm_id: Option<String>,
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
    /// Optional persisted ownership mask for ordinary graph views.
    ///
    /// When `Some`, the render pass restricts visible nodes to this explicit
    /// owned set (intersected with any other active filters). `None` preserves
    /// the historical "show all nodes" behavior until a view participates in
    /// the explicit transfer flow.
    #[serde(default)]
    pub owned_node_mask: Option<std::collections::HashSet<crate::graph::NodeKey>>,
    /// Optional graphlet node-set mask.  When `Some`, the render pass restricts
    /// visible nodes to this set (intersected with any other active filters).
    ///
    /// Used by `GraphCanvasHostCtx::NavigatorSpecialty` views to show only the
    /// members of the derived graphlet.  Cleared when the specialty view closes.
    /// Not persisted (`skip`) — derived from graphlet state each session.
    #[serde(skip)]
    pub graphlet_node_mask: Option<std::collections::HashSet<crate::graph::NodeKey>>,
}

impl std::fmt::Debug for GraphViewState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GraphViewState")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("camera", &self.camera)
            .field("position_fit_locked", &self.position_fit_locked)
            .field("zoom_fit_locked", &self.zoom_fit_locked)
            .field("lens_state", &self.lens_state)
            .field("layout_policy", &self.layout_policy)
            .field("physics_policy", &self.physics_policy)
            .field("filter_policy", &self.filter_policy)
            .field("overlay_policy", &self.overlay_policy)
            .field("presentation_policy", &self.presentation_policy)
            .field("relation_policy", &self.relation_policy)
            .field("local_simulation", &self.local_simulation)
            .field("dimension", &self.dimension)
            .field("scene_mode", &self.scene_mode)
            .field("scene_reveal_nodes", &self.scene_reveal_nodes)
            .field("scene_relation_xray", &self.scene_relation_xray)
            .field("simulate_behavior_preset", &self.simulate_behavior_preset)
            .field("last_layout_algorithm_id", &self.last_layout_algorithm_id)
            .field(
                "navigation_policy_override",
                &self.navigation_policy_override,
            )
            .field("node_style_override", &self.node_style_override)
            .field("simulate_motion_override", &self.simulate_motion_override)
            .field("active_filter", &self.active_filter)
            .field("edge_projection_override", &self.edge_projection_override)
            .field(
                "owned_node_mask_len",
                &self
                    .owned_node_mask
                    .as_ref()
                    .map(std::collections::HashSet::len),
            )
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
            lens_state: self.lens_state.clone(),
            layout_policy: self.layout_policy.clone(),
            physics_policy: self.physics_policy.clone(),
            filter_policy: self.filter_policy.clone(),
            overlay_policy: self.overlay_policy.clone(),
            presentation_policy: self.presentation_policy.clone(),
            relation_policy: self.relation_policy.clone(),
            navigation_policy_override: self.navigation_policy_override,
            node_style_override: self.node_style_override,
            simulate_motion_override: self.simulate_motion_override,
            local_simulation: self.local_simulation.clone(),
            dimension: self.dimension.clone(),
            scene_mode: self.scene_mode,
            scene_reveal_nodes: self.scene_reveal_nodes,
            scene_relation_xray: self.scene_relation_xray,
            simulate_behavior_preset: self.simulate_behavior_preset,
            last_layout_algorithm_id: self.last_layout_algorithm_id.clone(),
            active_filter: self.active_filter.clone(),
            edge_projection_override: self.edge_projection_override.clone(),
            tombstones_visible: self.tombstones_visible,
            owned_node_mask: self.owned_node_mask.clone(),
            // graphlet_node_mask is session-derived; not cloned across views
            graphlet_node_mask: None,
        }
    }
}

#[derive(serde::Deserialize)]
struct GraphViewStateSerde {
    id: GraphViewId,
    name: String,
    camera: Camera,
    #[serde(default)]
    position_fit_locked: bool,
    #[serde(default)]
    zoom_fit_locked: bool,
    #[serde(default)]
    lens_state: ViewLensState,
    #[serde(default)]
    layout_policy: ViewLayoutPolicy,
    #[serde(default)]
    physics_policy: ViewPhysicsPolicy,
    #[serde(default)]
    filter_policy: ViewFilterPolicy,
    #[serde(default)]
    overlay_policy: ViewOverlayPolicy,
    #[serde(default)]
    presentation_policy: ViewPresentationPolicy,
    #[serde(default)]
    relation_policy: ViewRelationPolicy,
    #[serde(default)]
    navigation_policy_override: Option<graph_canvas::navigation::NavigationPolicy>,
    #[serde(default)]
    node_style_override: Option<graph_canvas::node_style::NodeStyle>,
    #[serde(default)]
    simulate_motion_override: Option<graph_canvas::scene_physics::SimulateMotionProfile>,
    local_simulation: Option<LocalSimulation>,
    #[serde(default)]
    dimension: ViewDimension,
    #[serde(default)]
    scene_mode: SceneMode,
    #[serde(default)]
    scene_reveal_nodes: bool,
    #[serde(default)]
    scene_relation_xray: bool,
    #[serde(default)]
    simulate_behavior_preset: SimulateBehaviorPreset,
    #[serde(skip)]
    last_layout_algorithm_id: Option<String>,
    #[serde(skip)]
    #[serde(default)]
    active_filter: Option<crate::model::graph::filter::FacetExpr>,
    #[serde(default)]
    edge_projection_override: Option<EdgeProjectionState>,
    #[serde(default)]
    tombstones_visible: bool,
    #[serde(default)]
    owned_node_mask: Option<std::collections::HashSet<crate::graph::NodeKey>>,
    #[serde(skip)]
    graphlet_node_mask: Option<std::collections::HashSet<crate::graph::NodeKey>>,
    #[serde(default, rename = "lens")]
    legacy_lens: Option<LegacyLensConfig>,
}

impl<'de> serde::Deserialize<'de> for GraphViewState {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let helper = GraphViewStateSerde::deserialize(deserializer)?;
        let mut view = Self {
            id: helper.id,
            name: helper.name,
            camera: helper.camera,
            position_fit_locked: helper.position_fit_locked,
            zoom_fit_locked: helper.zoom_fit_locked,
            lens_state: helper.lens_state,
            layout_policy: helper.layout_policy,
            physics_policy: helper.physics_policy,
            filter_policy: helper.filter_policy,
            overlay_policy: helper.overlay_policy,
            presentation_policy: helper.presentation_policy,
            relation_policy: helper.relation_policy,
            navigation_policy_override: helper.navigation_policy_override,
            node_style_override: helper.node_style_override,
            simulate_motion_override: helper.simulate_motion_override,
            local_simulation: helper.local_simulation,
            dimension: helper.dimension,
            scene_mode: helper.scene_mode,
            scene_reveal_nodes: helper.scene_reveal_nodes,
            scene_relation_xray: helper.scene_relation_xray,
            simulate_behavior_preset: helper.simulate_behavior_preset,
            last_layout_algorithm_id: helper.last_layout_algorithm_id,
            active_filter: helper.active_filter,
            edge_projection_override: helper.edge_projection_override,
            tombstones_visible: helper.tombstones_visible,
            owned_node_mask: helper.owned_node_mask,
            graphlet_node_mask: helper.graphlet_node_mask,
        };

        if let Some(legacy_lens) = helper.legacy_lens {
            let policy_fields_missing = view.lens_state.display_name.trim().is_empty()
                && view.lens_state.base_lens_id.is_none()
                && matches!(view.layout_policy.mode, LayoutMode::Free)
                && view.layout_policy.algorithm_id
                    == crate::app::graph_layout::default_free_layout_algorithm_id()
                && view.physics_policy.profile == PhysicsProfile::default()
                && view.presentation_policy.theme.is_none()
                && view.filter_policy.lens_filter_expr.is_none()
                && view.filter_policy.legacy_filters.is_empty()
                && view.overlay_policy.overlay_descriptor.is_none();
            if policy_fields_missing {
                view.hydrate_policy_surfaces_from_legacy_lens(legacy_lens);
            }
        }

        let resolved_profile_id = view
            .physics_policy
            .profile_id
            .as_deref()
            .map(crate::registries::atomic::lens::resolve_physics_profile)
            .unwrap_or_else(|| {
                let hinted = crate::registries::atomic::lens::canonical_physics_profile_id_hint(
                    &view.physics_policy.profile,
                );
                crate::registries::atomic::lens::resolve_physics_profile(hinted)
            });
        view.physics_policy.profile_id = Some(resolved_profile_id.resolved_id.clone());
        view.physics_policy.profile = resolved_profile_id.profile;

        Ok(view)
    }
}

impl GraphViewState {
    fn hydrate_policy_surfaces_from_legacy_lens(&mut self, legacy_lens: LegacyLensConfig) {
        self.lens_state = ViewLensState {
            base_lens_id: legacy_lens.lens_id,
            display_name: legacy_lens.name,
        };
        self.layout_policy = ViewLayoutPolicy {
            mode: legacy_lens.layout,
            algorithm_id: legacy_lens.layout_algorithm_id,
            source: PolicyValueSource::LegacySnapshot,
        };
        let resolution = crate::registries::atomic::lens::resolve_physics_profile(
            crate::registries::atomic::lens::canonical_physics_profile_id_hint(
                &legacy_lens.physics,
            ),
        );
        self.physics_policy = ViewPhysicsPolicy {
            profile_id: Some(resolution.resolved_id),
            profile: resolution.profile,
            source: PolicyValueSource::LegacySnapshot,
        };
        self.filter_policy = ViewFilterPolicy {
            lens_filter_expr: legacy_lens.filter_expr,
            lens_filter_source: Some(PolicyValueSource::LegacySnapshot),
            active_filter_override: self.active_filter.clone(),
            active_filter_override_source: self
                .active_filter
                .as_ref()
                .map(|_| PolicyValueSource::LegacySnapshot),
            legacy_filters: legacy_lens.filters_legacy,
        };
        self.overlay_policy = ViewOverlayPolicy {
            overlay_descriptor: legacy_lens.overlay_descriptor,
            source: Some(PolicyValueSource::LegacySnapshot),
        };
        self.presentation_policy = ViewPresentationPolicy {
            theme: legacy_lens.theme,
            source: Some(PolicyValueSource::LegacySnapshot),
        };
        self.relation_policy = ViewRelationPolicy {
            edge_projection_override: self.edge_projection_override.clone(),
        };
    }

    pub fn apply_layout_policy_override(
        &mut self,
        mode: LayoutMode,
        algorithm_id: impl Into<String>,
    ) {
        self.apply_layout_policy(mode, algorithm_id, PolicyValueSource::ViewOverride);
    }

    pub fn apply_layout_policy(
        &mut self,
        mode: LayoutMode,
        algorithm_id: impl Into<String>,
        source: PolicyValueSource,
    ) {
        let algorithm_id = algorithm_id.into();
        self.layout_policy = ViewLayoutPolicy {
            mode: mode.clone(),
            algorithm_id: algorithm_id.clone(),
            source,
        };
    }

    pub fn apply_physics_policy_override(
        &mut self,
        profile_id: impl Into<String>,
        profile: PhysicsProfile,
    ) {
        self.apply_physics_policy(profile_id, profile, PolicyValueSource::ViewOverride);
    }

    pub fn apply_physics_policy(
        &mut self,
        profile_id: impl Into<String>,
        profile: PhysicsProfile,
        source: PolicyValueSource,
    ) {
        let profile_id = profile_id.into();
        self.physics_policy = ViewPhysicsPolicy {
            profile_id: Some(profile_id),
            profile: profile.clone(),
            source,
        };
    }

    pub fn apply_resolved_lens_identity(&mut self, resolved: ResolvedLensPreset) {
        let lens_id = resolved.lens_id.clone();
        self.lens_state = ViewLensState {
            base_lens_id: Some(lens_id.clone()),
            display_name: resolved.display_name.clone(),
        };
        self.filter_policy.lens_filter_expr = resolved.filter_expr.clone();
        self.filter_policy.lens_filter_source = self
            .filter_policy
            .lens_filter_expr
            .as_ref()
            .map(|_| PolicyValueSource::LensPreset(lens_id.clone()));
        self.filter_policy.legacy_filters = resolved.filters_legacy.clone();
        self.overlay_policy.overlay_descriptor = resolved.overlay_descriptor.clone();
        self.overlay_policy.source = self
            .overlay_policy
            .overlay_descriptor
            .as_ref()
            .map(|_| PolicyValueSource::LensPreset(lens_id.clone()));
        self.presentation_policy.theme = resolved.theme.clone();
        self.presentation_policy.source = self
            .presentation_policy
            .theme
            .as_ref()
            .map(|_| PolicyValueSource::LensPreset(lens_id));
    }

    pub fn apply_filter_override(&mut self, expr: Option<crate::model::graph::filter::FacetExpr>) {
        self.filter_policy.active_filter_override = expr.clone();
        self.filter_policy.active_filter_override_source =
            expr.as_ref().map(|_| PolicyValueSource::ViewOverride);
        self.active_filter = expr.clone();
    }

    pub fn apply_edge_projection_policy_override(
        &mut self,
        override_state: Option<EdgeProjectionState>,
    ) {
        self.relation_policy.edge_projection_override = override_state.clone();
        self.edge_projection_override = override_state;
    }

    pub fn resolved_layout_mode(&self) -> &LayoutMode {
        &self.layout_policy.mode
    }

    pub fn resolved_layout_algorithm_id(&self) -> &str {
        self.layout_policy.algorithm_id.as_str()
    }

    pub fn resolved_layout_source(&self) -> &PolicyValueSource {
        &self.layout_policy.source
    }

    pub fn resolved_physics_profile(&self) -> &PhysicsProfile {
        &self.physics_policy.profile
    }

    pub fn resolved_physics_source(&self) -> &PolicyValueSource {
        &self.physics_policy.source
    }

    pub fn resolved_lens_id(&self) -> Option<&str> {
        self.lens_state.base_lens_id.as_deref()
    }

    pub fn resolved_lens_display_name(&self) -> &str {
        self.lens_state.display_name.as_str()
    }

    pub fn resolved_physics_profile_id(&self) -> Option<&str> {
        self.physics_policy.profile_id.as_deref()
    }

    pub fn resolved_theme(&self) -> Option<&ThemeData> {
        self.presentation_policy.theme.as_ref()
    }

    pub fn resolved_theme_source(&self) -> Option<&PolicyValueSource> {
        self.presentation_policy.source.as_ref()
    }

    pub fn resolved_overlay_descriptor(
        &self,
    ) -> Option<&crate::registries::atomic::lens::LensOverlayDescriptor> {
        self.overlay_policy.overlay_descriptor.as_ref()
    }

    pub fn resolved_overlay_source(&self) -> Option<&PolicyValueSource> {
        self.overlay_policy.source.as_ref()
    }

    pub fn resolved_filter_count(&self) -> usize {
        self.filter_policy
            .active_filter_override
            .as_ref()
            .or(self.filter_policy.lens_filter_expr.as_ref())
            .is_some() as usize
            + self.filter_policy.legacy_filters.len()
    }

    pub fn effective_filter_expr(&self) -> Option<&crate::model::graph::filter::FacetExpr> {
        self.filter_policy
            .active_filter_override
            .as_ref()
            .or(self.filter_policy.lens_filter_expr.as_ref())
            .or(self.active_filter.as_ref())
    }

    pub fn owned_node_mask(&self) -> Option<&std::collections::HashSet<crate::graph::NodeKey>> {
        self.owned_node_mask.as_ref()
    }

    pub fn effective_filter_source(&self) -> Option<&PolicyValueSource> {
        self.filter_policy
            .active_filter_override_source
            .as_ref()
            .or(self.filter_policy.lens_filter_source.as_ref())
    }

    pub fn new_with_id(id: GraphViewId, name: impl Into<String>) -> Self {
        let mut view = Self {
            id,
            name: name.into(),
            camera: Camera::new(),
            position_fit_locked: false,
            zoom_fit_locked: false,
            lens_state: ViewLensState::default(),
            layout_policy: ViewLayoutPolicy::default(),
            physics_policy: ViewPhysicsPolicy::default(),
            filter_policy: ViewFilterPolicy::default(),
            overlay_policy: ViewOverlayPolicy::default(),
            presentation_policy: ViewPresentationPolicy::default(),
            relation_policy: ViewRelationPolicy::default(),
            navigation_policy_override: None,
            node_style_override: None,
            simulate_motion_override: None,
            local_simulation: None,
            dimension: ViewDimension::default(),
            scene_mode: SceneMode::Browse,
            scene_reveal_nodes: false,
            scene_relation_xray: false,
            simulate_behavior_preset: SimulateBehaviorPreset::default(),
            last_layout_algorithm_id: None,
            active_filter: None,
            edge_projection_override: None,
            tombstones_visible: false,
            owned_node_mask: None,
            graphlet_node_mask: None,
        };
        view.lens_state.display_name = "Default".to_string();
        view
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

pub(crate) fn default_view_dimension_for_mode(mode: ThreeDMode) -> ViewDimension {
    match mode {
        ThreeDMode::TwoPointFive => ViewDimension::ThreeD {
            mode,
            z_source: ZSource::Recency { max_depth: 8.0 },
        },
        ThreeDMode::Isometric => ViewDimension::ThreeD {
            mode,
            z_source: ZSource::BfsDepth { scale: 12.0 },
        },
        ThreeDMode::Standard => ViewDimension::ThreeD {
            mode,
            z_source: ZSource::Zero,
        },
    }
}

pub(crate) fn view_dimension_summary(dimension: &ViewDimension) -> (String, String, bool) {
    if is_semantic_depth_dimension(dimension) {
        return (
            "Depth".to_string(),
            "Semantic depth layering is active as a reversible 2.5D UDC preset for this graph view."
                .to_string(),
            true,
        );
    }

    match dimension {
        ViewDimension::TwoD => (
            "2D".to_string(),
            "Standard 2D planar graph view.".to_string(),
            false,
        ),
        ViewDimension::ThreeD { mode, z_source } => {
            let z_source_label = match z_source {
                ZSource::Zero => "flat z",
                ZSource::Recency { .. } => "recency depth",
                ZSource::BfsDepth { .. } => "BFS depth",
                ZSource::UdcLevel { .. } => "UDC depth",
                ZSource::Manual => "manual z",
            };
            let (label, mode_label) = match mode {
                ThreeDMode::TwoPointFive => ("2.5D", "fixed-camera 2.5D projection"),
                ThreeDMode::Isometric => ("Iso", "fixed-angle isometric projection"),
                ThreeDMode::Standard => ("3D", "standard 3D view state"),
            };
            (
                label.to_string(),
                format!("{mode_label} using {z_source_label} for derived z placement."),
                false,
            )
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refresh_registry_backed_view_lenses_preserves_active_filter_override() {
        let mut app = crate::app::GraphBrowserApp::new_for_testing();
        let view_id = GraphViewId::new();
        let mut view = GraphViewState::new_with_id(view_id, "Lens");
        view.lens_state.base_lens_id =
            Some(crate::registries::atomic::lens::LENS_ID_DEFAULT.to_string());
        view.apply_filter_override(Some(crate::model::graph::filter::FacetExpr::Predicate(
            crate::model::graph::filter::FacetPredicate {
                facet_key: crate::model::graph::filter::facet_keys::TITLE.to_string(),
                operator: crate::model::graph::filter::FacetOperator::Eq,
                operand: crate::model::graph::filter::FacetOperand::Scalar(
                    crate::model::graph::filter::FacetScalar::Text("alpha".to_string()),
                ),
            },
        )));
        app.workspace.graph_runtime.views.insert(view_id, view);

        let refreshed = app.refresh_registry_backed_view_lenses();
        let view = app
            .workspace
            .graph_runtime
            .views
            .get(&view_id)
            .expect("view should remain registered");

        assert_eq!(refreshed, 1);
        assert!(view.active_filter.is_some());
        assert!(view.filter_policy.active_filter_override.is_some());
        assert!(view.filter_policy.lens_filter_expr.is_none());
    }

    #[test]
    fn apply_physics_policy_override_updates_policy_fields() {
        let mut view = GraphViewState::new("Physics");

        view.apply_physics_policy_override(
            crate::registries::atomic::lens::PHYSICS_ID_SCATTER,
            crate::registries::atomic::lens::PhysicsProfile::scatter(),
        );

        assert_eq!(
            view.resolved_physics_profile_id(),
            Some(crate::registries::atomic::lens::PHYSICS_ID_SCATTER)
        );
        assert_eq!(view.physics_policy.profile.name, "Scatter");
        assert_eq!(view.resolved_physics_profile().name, "Scatter");
        assert_eq!(view.physics_policy.source, PolicyValueSource::ViewOverride);
    }

    #[test]
    fn move_graph_view_slot_swaps_with_occupied_slot() {
        let mut app = crate::app::GraphBrowserApp::new_for_testing();
        let left = GraphViewId::new();
        let right = GraphViewId::new();
        app.ensure_graph_view_registered(left);
        app.ensure_graph_view_registered(right);

        {
            let slots = &mut app.workspace.graph_runtime.graph_view_layout_manager.slots;
            let left_slot = slots.get_mut(&left).expect("left slot should exist");
            left_slot.row = 0;
            left_slot.col = 0;
            let right_slot = slots.get_mut(&right).expect("right slot should exist");
            right_slot.row = 0;
            right_slot.col = 1;
        }

        app.move_graph_view_slot(left, 0, 1);

        let slots = &app.workspace.graph_runtime.graph_view_layout_manager.slots;
        let left_slot = slots.get(&left).expect("left slot should exist");
        let right_slot = slots.get(&right).expect("right slot should exist");
        assert_eq!((left_slot.row, left_slot.col), (0, 1));
        assert_eq!((right_slot.row, right_slot.col), (0, 0));
    }

    #[test]
    fn apply_resolved_lens_identity_preserves_layout_and_physics_overrides() {
        let mut view = GraphViewState::new("Lens");
        view.apply_layout_policy_override(
            crate::registries::atomic::lens::LayoutMode::Grid { gap: 24.0 },
            crate::app::graph_layout::GRAPH_LAYOUT_GRID,
        );
        view.apply_physics_policy_override(
            crate::registries::atomic::lens::PHYSICS_ID_SCATTER,
            crate::registries::atomic::lens::PhysicsProfile::scatter(),
        );

        let resolved = crate::app::runtime_ports::registries::phase2_resolve_lens(
            crate::registries::atomic::lens::LENS_ID_SEMANTIC_OVERLAY,
        );
        view.apply_resolved_lens_identity(resolved);

        assert_eq!(
            view.resolved_lens_id(),
            Some(crate::registries::atomic::lens::LENS_ID_SEMANTIC_OVERLAY)
        );
        assert_eq!(view.resolved_lens_display_name(), "Semantic Overlay");
        assert_eq!(
            view.resolved_layout_mode(),
            &crate::registries::atomic::lens::LayoutMode::Grid { gap: 24.0 }
        );
        assert_eq!(
            view.resolved_physics_profile_id(),
            Some(crate::registries::atomic::lens::PHYSICS_ID_SCATTER)
        );
        assert!(view.resolved_overlay_descriptor().is_some());
        assert_eq!(view.resolved_filter_count(), 1);
        assert_eq!(
            view.overlay_policy.source,
            Some(PolicyValueSource::LensPreset(
                crate::registries::atomic::lens::LENS_ID_SEMANTIC_OVERLAY.to_string()
            ))
        );
        assert_eq!(view.filter_policy.lens_filter_source, None);
        assert_eq!(view.layout_policy.source, PolicyValueSource::ViewOverride);
        assert_eq!(view.physics_policy.source, PolicyValueSource::ViewOverride);
    }

    #[test]
    fn set_view_lens_id_preserves_existing_overrides() {
        let mut app = crate::app::GraphBrowserApp::new_for_testing();
        let view_id = GraphViewId::new();
        let mut view = GraphViewState::new_with_id(view_id, "Lens");
        view.apply_layout_policy_override(
            crate::registries::atomic::lens::LayoutMode::Grid { gap: 24.0 },
            crate::app::graph_layout::GRAPH_LAYOUT_GRID,
        );
        view.apply_physics_policy_override(
            crate::registries::atomic::lens::PHYSICS_ID_SCATTER,
            crate::registries::atomic::lens::PhysicsProfile::scatter(),
        );
        view.apply_filter_override(Some(crate::model::graph::filter::FacetExpr::Predicate(
            crate::model::graph::filter::FacetPredicate {
                facet_key: crate::model::graph::filter::facet_keys::TITLE.to_string(),
                operator: crate::model::graph::filter::FacetOperator::Eq,
                operand: crate::model::graph::filter::FacetOperand::Scalar(
                    crate::model::graph::filter::FacetScalar::Text("alpha".to_string()),
                ),
            },
        )));
        app.workspace.graph_runtime.views.insert(view_id, view);

        app.apply_reducer_intents([crate::app::GraphIntent::SetViewLensId {
            view_id,
            lens_id: crate::registries::atomic::lens::LENS_ID_SEMANTIC_OVERLAY.to_string(),
        }]);

        let view = app
            .workspace
            .graph_runtime
            .views
            .get(&view_id)
            .expect("view should remain registered");
        assert_eq!(
            view.resolved_lens_id(),
            Some(crate::registries::atomic::lens::LENS_ID_SEMANTIC_OVERLAY)
        );
        assert_eq!(
            view.resolved_layout_mode(),
            &crate::registries::atomic::lens::LayoutMode::Grid { gap: 24.0 }
        );
        assert_eq!(
            view.resolved_physics_profile_id(),
            Some(crate::registries::atomic::lens::PHYSICS_ID_SCATTER)
        );
        assert!(view.filter_policy.active_filter_override.is_some());
        assert!(view.resolved_overlay_descriptor().is_some());
    }

    #[test]
    fn deserialize_legacy_lens_snapshot_upgrades_into_policy_surfaces() {
        let json = format!(
            r#"{{
                "id":"{}",
                "name":"Legacy",
                "camera":{{"zoom_min":0.1,"zoom_max":10.0,"current_zoom":0.8}},
                "position_fit_locked":false,
                "zoom_fit_locked":false,
                "lens":{{
                    "name":"Legacy Lens",
                    "lens_id":"lens:default",
                    "physics":{{
                        "name":"Gas",
                        "repulsion_strength":0.8,
                        "attraction_strength":0.05,
                        "gravity_strength":0.0,
                        "damping":0.8,
                        "degree_repulsion":true,
                        "domain_clustering":false,
                        "semantic_clustering":false,
                        "semantic_strength":0.0,
                        "auto_pause":false
                    }},
                    "layout":{{"Grid":{{"gap":24.0}}}},
                    "layout_algorithm_id":"{}",
                    "theme":null,
                    "filter_expr":null,
                    "filters":["legacy"],
                    "overlay_descriptor":null
                }},
                "active_filter":null,
                "edge_projection_override":null,
                "tombstones_visible":false,
                "dimension":"TwoD"
            }}"#,
            GraphViewId::new().as_uuid(),
            crate::app::graph_layout::GRAPH_LAYOUT_GRID
        );

        let view: GraphViewState =
            serde_json::from_str(&json).expect("legacy graph view snapshot should deserialize");

        assert_eq!(view.resolved_lens_id(), Some("lens:default"));
        assert_eq!(view.resolved_lens_display_name(), "Legacy Lens");
        assert!(matches!(
            view.resolved_layout_mode(),
            crate::registries::atomic::lens::LayoutMode::Grid { gap: 24.0 }
        ));
        assert_eq!(
            view.resolved_layout_algorithm_id(),
            crate::app::graph_layout::GRAPH_LAYOUT_GRID
        );
        assert_eq!(view.resolved_physics_profile_id(), Some("physics:scatter"));
        assert_eq!(view.resolved_physics_profile().name, "Scatter");
        assert_eq!(
            view.filter_policy.legacy_filters,
            vec!["legacy".to_string()]
        );
        assert_eq!(view.layout_policy.source, PolicyValueSource::LegacySnapshot);
        assert_eq!(
            view.physics_policy.source,
            PolicyValueSource::LegacySnapshot
        );
        assert_eq!(view.filter_policy.active_filter_override_source, None);
    }

    #[test]
    fn graph_view_state_defaults_to_browse_scene_mode() {
        let view = GraphViewState::new("Scene");
        assert_eq!(view.scene_mode, SceneMode::Browse);
    }

    #[test]
    fn graph_view_state_scene_mode_round_trips_through_serde() {
        let view_id = GraphViewId::new();
        let mut view = GraphViewState::new_with_id(view_id, "Scene");
        view.scene_mode = SceneMode::Arrange;
        view.scene_reveal_nodes = true;
        view.scene_relation_xray = true;
        view.simulate_behavior_preset = SimulateBehaviorPreset::Magnetic;

        let json = serde_json::to_string(&view).expect("graph view should serialize");
        let decoded: GraphViewState =
            serde_json::from_str(&json).expect("graph view should deserialize");

        assert_eq!(decoded.scene_mode, SceneMode::Arrange);
        assert!(decoded.scene_reveal_nodes);
        assert!(decoded.scene_relation_xray);
        assert_eq!(
            decoded.simulate_behavior_preset,
            SimulateBehaviorPreset::Magnetic
        );
    }

    #[test]
    fn set_graph_view_scene_mode_creates_missing_view_and_marks_dirty() {
        let mut app = crate::app::GraphBrowserApp::new_for_testing();
        let view_id = GraphViewId::new();

        app.set_graph_view_scene_mode(view_id, SceneMode::Arrange);

        assert_eq!(app.graph_view_scene_mode(view_id), SceneMode::Arrange);
        assert!(app.workspace.graph_runtime.views.contains_key(&view_id));
    }

    #[test]
    fn graph_view_state_scene_simulate_toggles_default_off() {
        let view = GraphViewState::new("Scene");
        assert!(!view.scene_reveal_nodes);
        assert!(!view.scene_relation_xray);
        assert_eq!(view.simulate_behavior_preset, SimulateBehaviorPreset::Float);
    }

    #[test]
    fn set_graph_view_scene_simulate_toggles_create_missing_view_and_mark_dirty() {
        let mut app = crate::app::GraphBrowserApp::new_for_testing();
        let view_id = GraphViewId::new();

        app.set_graph_view_scene_reveal_nodes(view_id, true);
        app.set_graph_view_scene_relation_xray(view_id, true);
        app.set_graph_view_simulate_behavior_preset(view_id, SimulateBehaviorPreset::Packed);

        assert!(app.graph_view_scene_reveal_nodes(view_id));
        assert!(app.graph_view_scene_relation_xray(view_id));
        assert_eq!(
            app.graph_view_simulate_behavior_preset(view_id),
            SimulateBehaviorPreset::Packed
        );
        assert!(app.workspace.graph_runtime.views.contains_key(&view_id));
    }

    #[test]
    fn apply_filter_override_marks_view_override_provenance() {
        let mut view = GraphViewState::new("Filter");
        let expr = crate::model::graph::filter::FacetExpr::Predicate(
            crate::model::graph::filter::FacetPredicate {
                facet_key: crate::model::graph::filter::facet_keys::TITLE.to_string(),
                operator: crate::model::graph::filter::FacetOperator::Eq,
                operand: crate::model::graph::filter::FacetOperand::Scalar(
                    crate::model::graph::filter::FacetScalar::Text("alpha".to_string()),
                ),
            },
        );

        view.apply_filter_override(Some(expr));

        assert_eq!(
            view.effective_filter_source(),
            Some(&PolicyValueSource::ViewOverride)
        );

        view.apply_filter_override(None);

        assert_eq!(view.effective_filter_source(), None);
    }

    #[test]
    fn set_graph_view_scene_regions_registers_runtime_regions() {
        let mut app = crate::app::GraphBrowserApp::new_for_testing();
        let view_id = GraphViewId::new();
        let region = crate::graph::scene_runtime::SceneRegionRuntime::rect(
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(64.0, 64.0)),
            crate::graph::scene_runtime::SceneRegionEffect::Attractor { strength: 0.2 },
        );

        app.set_graph_view_scene_regions(view_id, vec![region.clone()]);

        let runtime = app
            .graph_view_scene_runtime(view_id)
            .expect("scene runtime should exist");
        assert_eq!(runtime.regions, vec![region]);
        assert!(app.workspace.graph_runtime.views.contains_key(&view_id));
    }

    #[test]
    fn clearing_scene_regions_removes_empty_runtime_entry() {
        let mut app = crate::app::GraphBrowserApp::new_for_testing();
        let view_id = GraphViewId::new();
        app.set_graph_view_scene_regions(
            view_id,
            vec![crate::graph::scene_runtime::SceneRegionRuntime::rect(
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(64.0, 64.0)),
                crate::graph::scene_runtime::SceneRegionEffect::Wall,
            )],
        );

        app.set_graph_view_scene_regions(view_id, Vec::new());

        assert!(app.graph_view_scene_runtime(view_id).is_none());
    }

    #[test]
    fn clearing_scene_regions_preserves_bounds_override_runtime() {
        let mut app = crate::app::GraphBrowserApp::new_for_testing();
        let view_id = GraphViewId::new();
        app.set_graph_view_scene_bounds_override(
            view_id,
            Some(egui::Rect::from_min_max(
                egui::pos2(0.0, 0.0),
                egui::pos2(100.0, 100.0),
            )),
        );
        app.add_graph_view_scene_region(
            view_id,
            crate::graph::scene_runtime::SceneRegionRuntime::rect(
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(64.0, 64.0)),
                crate::graph::scene_runtime::SceneRegionEffect::Wall,
            ),
        );

        app.set_graph_view_scene_regions(view_id, Vec::new());

        let runtime = app
            .graph_view_scene_runtime(view_id)
            .expect("bounds override should keep runtime entry alive");
        assert!(runtime.regions.is_empty());
        assert!(runtime.bounds_override.is_some());
    }

    #[test]
    fn clear_graph_view_scene_runtime_removes_all_scene_state() {
        let mut app = crate::app::GraphBrowserApp::new_for_testing();
        let view_id = GraphViewId::new();
        app.set_graph_view_scene_bounds_override(
            view_id,
            Some(egui::Rect::from_min_max(
                egui::pos2(0.0, 0.0),
                egui::pos2(100.0, 100.0),
            )),
        );
        app.add_graph_view_scene_region(
            view_id,
            crate::graph::scene_runtime::SceneRegionRuntime::circle(
                egui::pos2(20.0, 20.0),
                12.0,
                crate::graph::scene_runtime::SceneRegionEffect::Repulsor { strength: 4.0 },
            ),
        );

        app.clear_graph_view_scene_runtime(view_id);

        assert!(app.graph_view_scene_runtime(view_id).is_none());
    }

    #[test]
    fn translate_graph_view_scene_region_moves_region_shape() {
        let mut app = crate::app::GraphBrowserApp::new_for_testing();
        let view_id = GraphViewId::new();
        let region = crate::graph::scene_runtime::SceneRegionRuntime::rect(
            egui::Rect::from_min_max(egui::pos2(10.0, 20.0), egui::pos2(40.0, 60.0)),
            crate::graph::scene_runtime::SceneRegionEffect::Wall,
        );
        let region_id = region.id;
        app.add_graph_view_scene_region(view_id, region);

        assert!(app.translate_graph_view_scene_region(view_id, region_id, egui::vec2(5.0, -4.0)));

        let runtime = app
            .graph_view_scene_runtime(view_id)
            .expect("scene runtime should still exist");
        assert_eq!(
            runtime.regions[0].shape,
            crate::graph::scene_runtime::SceneRegionShape::Rect {
                rect: egui::Rect::from_min_max(egui::pos2(15.0, 16.0), egui::pos2(45.0, 56.0)),
            }
        );
    }

    #[test]
    fn clearing_scene_runtime_removes_selected_scene_region() {
        let mut app = crate::app::GraphBrowserApp::new_for_testing();
        let view_id = GraphViewId::new();
        let region = crate::graph::scene_runtime::SceneRegionRuntime::circle(
            egui::pos2(20.0, 20.0),
            12.0,
            crate::graph::scene_runtime::SceneRegionEffect::Repulsor { strength: 4.0 },
        );
        let region_id = region.id;
        app.add_graph_view_scene_region(view_id, region);
        app.set_graph_view_selected_scene_region(view_id, Some(region_id));

        app.clear_graph_view_scene_runtime(view_id);

        assert_eq!(app.graph_view_selected_scene_region(view_id), None);
    }

    #[test]
    fn resize_graph_view_scene_region_to_pointer_updates_circle_radius() {
        let mut app = crate::app::GraphBrowserApp::new_for_testing();
        let view_id = GraphViewId::new();
        let region = crate::graph::scene_runtime::SceneRegionRuntime::circle(
            egui::pos2(20.0, 20.0),
            12.0,
            crate::graph::scene_runtime::SceneRegionEffect::Repulsor { strength: 4.0 },
        );
        let region_id = region.id;
        app.add_graph_view_scene_region(view_id, region);

        assert!(app.resize_graph_view_scene_region_to_pointer(
            view_id,
            region_id,
            crate::graph::scene_runtime::SceneRegionResizeHandle::CircleRadius,
            egui::pos2(80.0, 20.0),
        ));

        let runtime = app
            .graph_view_scene_runtime(view_id)
            .expect("scene runtime should still exist");
        assert_eq!(
            runtime.regions[0].shape,
            crate::graph::scene_runtime::SceneRegionShape::Circle {
                center: egui::pos2(20.0, 20.0),
                radius: 60.0,
            }
        );
    }

    #[test]
    fn replace_graph_view_scene_region_updates_label_and_effect() {
        let mut app = crate::app::GraphBrowserApp::new_for_testing();
        let view_id = GraphViewId::new();
        let region = crate::graph::scene_runtime::SceneRegionRuntime::circle(
            egui::pos2(20.0, 20.0),
            12.0,
            crate::graph::scene_runtime::SceneRegionEffect::Repulsor { strength: 4.0 },
        );
        let region_id = region.id;
        app.add_graph_view_scene_region(view_id, region.clone());

        let mut updated = region;
        updated.label = Some("Focus Basin".to_string());
        updated.effect =
            crate::graph::scene_runtime::SceneRegionEffect::Attractor { strength: 0.3 };

        assert!(app.replace_graph_view_scene_region(view_id, updated.clone()));
        assert_eq!(
            app.graph_view_scene_region(view_id, region_id),
            Some(&updated)
        );
    }

    #[test]
    fn remove_graph_view_scene_region_clears_selection_when_deleted() {
        let mut app = crate::app::GraphBrowserApp::new_for_testing();
        let view_id = GraphViewId::new();
        let region = crate::graph::scene_runtime::SceneRegionRuntime::circle(
            egui::pos2(20.0, 20.0),
            12.0,
            crate::graph::scene_runtime::SceneRegionEffect::Repulsor { strength: 4.0 },
        );
        let region_id = region.id;
        app.add_graph_view_scene_region(view_id, region);
        app.set_graph_view_selected_scene_region(view_id, Some(region_id));

        assert!(app.remove_graph_view_scene_region(view_id, region_id));
        assert_eq!(app.graph_view_selected_scene_region(view_id), None);
        assert!(app.graph_view_scene_runtime(view_id).is_none());
    }
}
