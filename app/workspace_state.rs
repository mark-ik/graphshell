/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Typed sub-state structs extracted from `GraphWorkspace`.

use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
use std::time::{Duration, Instant};

use egui_tiles::TileId;
use uuid::Uuid;

use crate::graph::physics::GraphPhysicsState;
use crate::graph::scene_runtime::{GraphViewSceneRuntime, SceneRegionDragState, SceneRegionId};
use crate::graph::{FrameLayoutHint, Graph, NodeKey};
use crate::registries::atomic::knowledge::SemanticClassVector;
use crate::registries::domain::layout::canvas::CanvasLassoBinding;
use crate::shell::desktop::runtime::caches::RuntimeCaches;

use super::AppCommand;
use super::settings_persistence::NavigatorSidebarSidePreference;
use super::{
    Camera, ClipInspectorState, CommandPaletteShortcut, ContextCommandSurfacePreference,
    EdgeProjectionState, GraphReaderState, GraphSearchHistoryEntry, GraphSearchOrigin,
    GraphViewFrame, GraphViewId, GraphViewLayoutManagerState, GraphViewState, HelpPanelShortcut,
    HistoryManagerTab, HistoryTraversalFailureReason, KeyboardPanInputMode, MemoryPressureLevel,
    NavigatorProjectionState, OmnibarNonAtOrderPreset, OmnibarPreferredScope, PendingCreateToken,
    RadialMenuShortcut, RendererId, RuntimeBlockState, RuntimeFrameTabSemantics, SearchDisplayMode,
    SelectionEdgeProjectionOverride, SelectionScope, SelectionState, SettingsToolPage,
    SurfaceHostId, TagPanelState, ToastAnchorPreference, UndoRedoSnapshot, UxConfigMode,
    ViewDimension, WorkbenchDisplayMode, WorkbenchIntent, WorkbenchLayoutConstraint,
    WorkbenchProfile, WorkspaceUserStylesheetSetting,
};
use crate::graph::GraphletKind;

#[derive(Clone, Debug, PartialEq)]
pub struct VisibleNavigationRegionSet {
    rects: Vec<egui::Rect>,
}

impl VisibleNavigationRegionSet {
    pub(crate) fn from_rects(rects: Vec<egui::Rect>) -> Self {
        Self {
            rects: rects
                .into_iter()
                .filter(|rect| rect_has_area(*rect))
                .collect(),
        }
    }

    pub(crate) fn singleton(rect: egui::Rect) -> Self {
        Self::from_rects(vec![rect])
    }

    pub(crate) fn as_slice(&self) -> &[egui::Rect] {
        &self.rects
    }

    pub(crate) fn len(&self) -> usize {
        self.rects.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.rects.is_empty()
    }

    pub(crate) fn contains_rect(&self, rect: egui::Rect) -> bool {
        self.rects.contains(&rect)
    }

    pub(crate) fn contains_point(&self, point: egui::Pos2) -> bool {
        self.rects.iter().any(|rect| rect.contains(point))
    }

    pub(crate) fn intersects_rect(&self, rect: egui::Rect) -> bool {
        self.rects.iter().any(|region| region.intersects(rect))
    }

    pub(crate) fn clipped_to(&self, clip_rect: egui::Rect) -> Self {
        Self::from_rects(
            self.rects
                .iter()
                .copied()
                .map(|rect| rect.intersect(clip_rect))
                .collect(),
        )
    }

    pub(crate) fn largest_rect(&self) -> Option<egui::Rect> {
        self.rects.iter().copied().max_by(|left, right| {
            rect_area(*left)
                .partial_cmp(&rect_area(*right))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    }

    pub(crate) fn to_vec(&self) -> Vec<egui::Rect> {
        self.rects.clone()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct WorkbenchNavigationGeometry {
    /// Rect available to the workbench after reserved panels have been applied.
    pub content_rect: egui::Rect,

    /// Visible workbench rects after subtracting overlay-only host occlusions.
    pub visible_regions: VisibleNavigationRegionSet,

    /// Host rects that visually occlude the workbench without shrinking `content_rect`.
    pub occluding_host_rects: Vec<egui::Rect>,
}

impl WorkbenchNavigationGeometry {
    pub(crate) fn from_content_rect(
        content_rect: egui::Rect,
        occluding_host_rects: Vec<egui::Rect>,
    ) -> Self {
        let mut visible_rects = vec![content_rect];

        for occlusion in occluding_host_rects.iter().copied() {
            let mut next_visible_rects = Vec::new();
            for visible_rect in visible_rects {
                next_visible_rects.extend(subtract_rect(visible_rect, occlusion));
            }
            visible_rects = next_visible_rects;
        }

        Self {
            content_rect,
            visible_regions: VisibleNavigationRegionSet::from_rects(visible_rects),
            occluding_host_rects,
        }
    }

    pub(crate) fn visible_region_set_or_content(&self) -> VisibleNavigationRegionSet {
        if self.visible_regions.is_empty() {
            VisibleNavigationRegionSet::singleton(self.content_rect)
        } else {
            self.visible_regions.clone()
        }
    }
}

fn subtract_rect(base: egui::Rect, occlusion: egui::Rect) -> Vec<egui::Rect> {
    if !rect_has_area(base) || !base.intersects(occlusion) {
        return vec![base];
    }

    let overlap = base.intersect(occlusion);
    if !rect_has_area(overlap) {
        return vec![base];
    }

    let mut remainder = Vec::with_capacity(4);
    let top = egui::Rect::from_min_max(base.min, egui::pos2(base.max.x, overlap.top()));
    let bottom = egui::Rect::from_min_max(egui::pos2(base.min.x, overlap.bottom()), base.max);
    let left = egui::Rect::from_min_max(
        egui::pos2(base.left(), overlap.top()),
        egui::pos2(overlap.left(), overlap.bottom()),
    );
    let right = egui::Rect::from_min_max(
        egui::pos2(overlap.right(), overlap.top()),
        egui::pos2(base.right(), overlap.bottom()),
    );

    for rect in [top, bottom, left, right] {
        if rect_has_area(rect) {
            remainder.push(rect);
        }
    }

    remainder
}

fn rect_has_area(rect: egui::Rect) -> bool {
    rect.width() > 0.0 && rect.height() > 0.0
}

fn rect_area(rect: egui::Rect) -> f32 {
    rect.width().max(0.0) * rect.height().max(0.0)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SemanticNavigationNodeRuntime {
    pub node_id: Uuid,
    pub current_url: Option<String>,
    pub last_visit_at_ms: u64,
    pub visit_count: usize,
    pub branch_points: usize,
    pub alternate_targets: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SemanticNavigationRuntimeState {
    pub recent_nodes: HashMap<NodeKey, SemanticNavigationNodeRuntime>,
}

/// View-layer runtime state: physics, selection, views, search, history, rendering.
pub struct GraphViewRuntimeState {
    /// Force-directed layout state owned by app/runtime UI controls.
    pub physics: GraphPhysicsState,

    /// Physics running state before user drag/pan interaction began.
    pub(crate) physics_running_before_interaction: Option<bool>,

    /// Canonical selection state keyed by runtime selection scope.
    pub(crate) selection_by_scope: HashMap<SelectionScope, SelectionState>,

    /// Temporary per-selection graphlet projection overrides keyed by
    /// selection scope.
    pub(crate) selection_edge_projections: HashMap<SelectionScope, SelectionEdgeProjectionOverride>,

    /// Bidirectional mapping between renderer instances and graph nodes.
    pub(crate) webview_to_node: HashMap<RendererId, NodeKey>,
    pub(crate) node_to_webview: HashMap<NodeKey, RendererId>,

    /// Explicit embedded-content focus authority for host/content routing.
    pub embedded_content_focus_webview: Option<RendererId>,

    /// Runtime-only block/backoff metadata keyed by graph node.
    pub(crate) runtime_block_state: HashMap<NodeKey, RuntimeBlockState>,

    /// Non-authoritative runtime caches for data-plane acceleration.
    pub(crate) runtime_caches: RuntimeCaches,

    /// Nodes that had webviews before switching to graph view (for restoration).
    pub(crate) active_webview_nodes: Vec<NodeKey>,

    /// Active mapped nodes in LRU order (oldest at index 0, newest at end).
    pub(crate) active_lru: Vec<NodeKey>,

    /// Maximum number of active mapped webviews to retain.
    pub(crate) active_webview_limit: usize,

    /// Warm-cached nodes in LRU order (oldest at index 0, newest at end).
    pub(crate) warm_cache_lru: Vec<NodeKey>,

    /// Maximum number of warm-cached webviews to retain.
    pub(crate) warm_cache_limit: usize,

    /// True while the user is actively interacting (drag/pan) with the graph.
    pub(crate) is_interacting: bool,

    /// Short post-drag decay window to preserve "weight" when physics was paused.
    pub(crate) drag_release_frames_remaining: u8,

    /// Active graph views, keyed by ID.
    pub views: HashMap<GraphViewId, GraphViewState>,

    /// Graph-view layout manager state (slot grid + manager overlay toggle).
    pub graph_view_layout_manager: GraphViewLayoutManagerState,

    /// Last known camera frame per graph view (updated by graph render pass).
    pub graph_view_frames: HashMap<GraphViewId, GraphViewFrame>,

    /// Last rendered graph-canvas rect per visible graph view, expressed in graph space.
    pub graph_view_canvas_rects: HashMap<GraphViewId, egui::Rect>,

    /// Runtime-only per-view scene enrichment state.
    pub scene_runtimes: HashMap<GraphViewId, GraphViewSceneRuntime>,

    /// Per-view graph-canvas interaction engine state (transient, not persisted).
    ///
    /// Created lazily when a view first renders through the graph-canvas path.
    /// Owns the portable `InteractionEngine` that replaced the retired
    /// `egui_graphs` hover/selection/drag tracking in M2.
    #[cfg(not(target_arch = "wasm32"))]
    pub canvas_interaction_engines:
        HashMap<GraphViewId, graph_canvas::engine::InteractionEngine<NodeKey>>,

    /// NodeKey → PaneId mapping, populated by dual-write on pane open.
    ///
    /// Eliminates the need to scan `egui_tiles::Tree` for PaneId lookup in the
    /// compositor, GraphTree layout path, and focus queries. Once egui_tiles is
    /// fully retired (M7), this becomes the sole PaneId authority.
    ///
    /// Placement note (M4 §5 runtime boundary design): the pre-M4 comment
    /// called this "host-side", but the map actually lives on
    /// `workspace.graph_runtime` — the app-layer, host-neutral side of
    /// the runtime/host boundary. That's correct: PaneId identities must
    /// survive the eventual iced migration because both `GraphTree` layout
    /// and pane-target lookups consume them. `TileId` is the egui_tiles-
    /// specific identity that stays on the host and retires in M7; the
    /// NodeKey↔PaneId mapping here does not depend on TileId.
    pub node_pane_ids: HashMap<NodeKey, crate::shell::desktop::workbench::pane_model::PaneId>,

    /// Host-side PaneId → TileRenderMode mapping, refreshed per frame alongside
    /// `node_pane_ids`. Eliminates the need for the compositor to scan tiles for
    /// render mode lookup.
    pub pane_render_modes: HashMap<
        crate::shell::desktop::workbench::pane_model::PaneId,
        crate::shell::desktop::workbench::pane_model::TileRenderMode,
    >,

    /// Host-side PaneId → resolved viewer ID mapping, refreshed per frame.
    /// Eliminates the need for the compositor to scan tiles for viewer ID lookup
    /// during semantic input resolution.
    pub pane_viewer_ids: HashMap<crate::shell::desktop::workbench::pane_model::PaneId, String>,

    /// Cached active pane rects from GraphTree layout, refreshed per frame.
    /// Eliminates the need for callers to scan `tiles_tree.active_tiles()` to
    /// discover visible node panes and their positions.
    pub active_pane_rects: Vec<(
        crate::shell::desktop::workbench::pane_model::PaneId,
        crate::graph::NodeKey,
        egui::Rect,
    )>,

    /// Cached GraphTree tree rows (sidebar projection), refreshed per frame
    /// alongside `active_pane_rects`. Read by the host for navigator sidebar
    /// rendering and by `GraphshellRuntime::project_view_model` when
    /// populating the frame view-model.
    pub cached_tree_rows: Vec<graph_tree::OwnedTreeRow<crate::graph::NodeKey>>,

    /// Cached flat tab order derived from GraphTree, refreshed per frame.
    /// Used for the tab-bar projection in the frame view-model.
    pub cached_tab_order: Vec<graph_tree::TabEntry<crate::graph::NodeKey>>,

    /// Cached split boundaries (draggable gutter handles), refreshed per
    /// frame. Used by both the active compositor pass and the frame
    /// view-model.
    pub cached_split_boundaries: Vec<graph_tree::SplitBoundary<crate::graph::NodeKey>>,

    /// Per-view graph-canvas camera state (transient, not persisted).
    ///
    /// Sole camera authority since M2 retired the `egui_graphs`
    /// MetadataFrame round-trip. Owned by portable `CanvasCamera`,
    /// written back per-frame by `canvas_bridge::run_graph_canvas_frame`.
    pub canvas_cameras: HashMap<GraphViewId, graph_canvas::camera::CanvasCamera>,

    /// Short-lived per-view release impulses used by `Simulate` mode so dragged
    /// node-objects can coast and settle briefly after pointer release.
    pub simulate_release_impulses: HashMap<GraphViewId, HashMap<NodeKey, egui::Vec2>>,

    /// Hovered authored scene region under the pointer, if any.
    pub hovered_scene_region: Option<(GraphViewId, SceneRegionId)>,

    /// Selected authored scene region per view.
    pub selected_scene_regions: HashMap<GraphViewId, SceneRegionId>,

    /// Active authored scene-region drag state.
    pub active_scene_region_drag: Option<SceneRegionDragState>,

    /// Computed visible workbench region after reserved panels and host overlays.
    pub workbench_navigation_geometry: Option<WorkbenchNavigationGeometry>,

    /// The currently focused graph view (target for keyboard zoom/pan).
    pub focused_view: Option<GraphViewId>,

    /// Accessibility Graph Reader mode override and return-path state.
    pub graph_reader_state: GraphReaderState,

    /// Camera state (zoom bounds).
    pub camera: Camera,

    /// Global undo history snapshots.
    pub(crate) undo_stack: Vec<UndoRedoSnapshot>,

    /// Global redo history snapshots.
    pub(crate) redo_stack: Vec<UndoRedoSnapshot>,

    /// Cached hop-distance map from current primary selection for omnibar ranking/signifiers.
    pub(crate) hop_distance_cache: Option<(NodeKey, HashMap<NodeKey, usize>)>,

    /// Node keys excluded by viewport culling on the previous rebuild.
    pub last_culled_node_keys: Option<HashSet<NodeKey>>,

    /// Last sampled runtime memory pressure classification.
    pub(crate) memory_pressure_level: MemoryPressureLevel,

    /// Last sampled available system memory (MiB).
    pub(crate) memory_available_mib: u64,

    /// Last sampled total system memory (MiB).
    pub(crate) memory_total_mib: u64,

    /// Count of traversal append attempts rejected in this runtime session.
    pub(crate) history_recent_traversal_append_failures: u64,

    /// True while history timeline preview mode is active.
    pub(crate) history_preview_mode_active: bool,

    /// True when preview-mode isolation has been violated in this session.
    pub(crate) history_last_preview_isolation_violation: bool,

    /// Tracks active timeline replay and cursor progression.
    pub(crate) history_replay_in_progress: bool,
    pub(crate) history_replay_cursor: Option<usize>,
    pub(crate) history_replay_total_steps: Option<usize>,

    /// Detached graph copy captured when preview mode is entered.
    pub(crate) history_preview_live_graph_snapshot: Option<Graph>,

    /// Detached graph produced by replay-to-timestamp while preview is active.
    pub(crate) history_preview_graph: Option<Graph>,

    /// Most recent history subsystem event timestamp observed this session.
    pub(crate) history_last_event_unix_ms: Option<u64>,

    /// Most recent history error text surfaced to operators.
    pub(crate) history_last_error: Option<String>,

    /// Last traversal/archive failure bucket label.
    pub(crate) history_recent_failure_reason_bucket: Option<HistoryTraversalFailureReason>,

    /// Last known return-to-present outcome summary.
    pub(crate) history_last_return_to_present_result: Option<String>,

    /// Shared runtime projection of semantic navigation memory across graph nodes.
    pub(crate) semantic_navigation: SemanticNavigationRuntimeState,

    /// Cached semantic codes for physics calculations.
    pub semantic_index: HashMap<NodeKey, SemanticClassVector>,
    pub semantic_index_dirty: bool,

    /// Per-view restore target used by reversible semantic depth toggles.
    pub(crate) semantic_depth_restore_dimensions: HashMap<GraphViewId, ViewDimension>,

    /// Display-only semantic tag suggestions surfaced by background agents.
    pub suggested_semantic_tags: HashMap<NodeKey, Vec<String>>,

    /// Last hovered node in graph view (updated by graph render pass).
    pub hovered_graph_node: Option<NodeKey>,

    /// Explicit highlighted edge in graph view (for edge-search targeting).
    pub highlighted_graph_edge: Option<(NodeKey, NodeKey)>,

    /// Selected frame identity from graph-canvas backdrop interaction.
    pub selected_frame_name: Option<String>,

    /// Runtime-only semantics for open frame tile groups keyed by the tabs-container tile id.
    pub(crate) frame_tile_groups: HashMap<TileId, FrameTileGroupRuntimeState>,

    /// Graph-owned hierarchical projection runtime state for navigator.
    pub navigator_projection_state: NavigatorProjectionState,

    /// Independent multi-selection for workspace tabs.
    pub selected_tab_nodes: HashSet<NodeKey>,

    /// Range-select anchor for workspace tab multi-selection.
    pub tab_selection_anchor: Option<NodeKey>,

    /// Graph search display mode (context-preserving highlight vs strict filter).
    pub search_display_mode: SearchDisplayMode,

    /// Current graph search query mirrored from the UI search flow.
    pub active_graph_search_query: String,

    /// Current graph search match count mirrored from the UI search flow.
    pub active_graph_search_match_count: usize,

    /// Source of the active graph search query.
    pub active_graph_search_origin: GraphSearchOrigin,

    /// Optional node whose undirected neighborhood should be included in the active search slice.
    pub active_graph_search_neighborhood_anchor: Option<NodeKey>,

    /// Hop depth for the active neighborhood expansion when an anchor is present.
    pub active_graph_search_neighborhood_depth: u8,

    /// Recent graph search states for breadcrumb restore.
    pub graph_search_history: Vec<GraphSearchHistoryEntry>,

    /// Optional pinned graph search slice for quick restore.
    pub pinned_graph_search: Option<GraphSearchHistoryEntry>,

    /// Non-modal tag editor state for the currently targeted node.
    pub tag_panel_state: Option<TagPanelState>,

    /// Non-modal web clip inspector state for the current extracted page surface.
    pub clip_inspector_state: Option<ClipInspectorState>,

    /// Pending webview highlight-clear request for inspector teardown.
    pub pending_clip_inspector_highlight_clear: Option<RendererId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameHintTabRuntime {
    pub tile_id: TileId,
    pub hint: FrameLayoutHint,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameTileGroupRuntimeState {
    pub frame_anchor: NodeKey,
    pub hint_tabs: Vec<FrameHintTabRuntime>,
}

/// Workbench session state: frame layouts, pending intents, arrangement sync caches.
pub struct WorkbenchSessionState {
    /// Hash of last persisted session frame layout json.
    pub(crate) last_session_workspace_layout_hash: Option<u64>,

    /// Last known live session frame layout JSON for undo checkpoints.
    pub(crate) last_session_workspace_layout_json: Option<String>,

    /// Minimum interval between autosaved session frame writes.
    pub(crate) workspace_autosave_interval: Duration,

    /// Number of previous autosaved session frame revisions to keep.
    pub(crate) workspace_autosave_retention: u8,

    /// Timestamp of last autosaved session frame write.
    pub(crate) last_workspace_autosave_at: Option<Instant>,

    /// Monotonic activation counter for named frame recency tracking.
    pub(crate) workspace_activation_seq: u64,

    /// Per-node most-recent named frame activation metadata keyed by stable node UUID.
    pub(crate) node_last_active_workspace: HashMap<Uuid, (u64, String)>,

    /// UUID-keyed frame membership index (runtime-derived from persisted layouts).
    pub(crate) node_workspace_membership: HashMap<Uuid, BTreeSet<String>>,

    /// Name of the currently loaded named frame/workspace, if any.
    pub(crate) current_workspace_name: Option<String>,

    /// Semantic tab overlay for the currently active named frame/workbench tree.
    pub(crate) current_frame_tab_semantics: Option<RuntimeFrameTabSemantics>,

    /// True while current tile tree was synthesized without a named restore context.
    pub(crate) current_workspace_is_synthesized: bool,

    /// True if graph-mutating action happened since last workspace baseline/save.
    pub(crate) workspace_has_unsaved_changes: bool,

    /// True after we've emitted a warning for the current unsaved workspace state.
    pub(crate) unsaved_workspace_prompt_warned: bool,

    /// Pending workbench-authority intents staged for frame-loop orchestration.
    pub(crate) pending_workbench_intents: Vec<WorkbenchIntent>,

    /// Persisted workbench profile extension carrying layout-policy state.
    pub workbench_profile: WorkbenchProfile,

    /// Runtime-applied layout constraints keyed by concrete surface host.
    pub(crate) active_layout_constraints: HashMap<SurfaceHostId, WorkbenchLayoutConstraint>,

    /// In-progress layout constraints being configured this session but not yet committed.
    pub(crate) draft_layout_constraints: HashMap<SurfaceHostId, WorkbenchLayoutConstraint>,

    /// Active layout configuration mode for workbench surfaces.
    pub(crate) ux_config_mode: UxConfigMode,

    /// Hosts whose first-use prompt should remain hidden for the current session only.
    pub(crate) session_suppressed_first_use_prompts: HashSet<SurfaceHostId>,

    /// Frames whose split-offer affordance was dismissed for the current session only.
    pub(crate) session_dismissed_frame_split_offers: HashSet<String>,

    /// Graph-wide default relation projection for graphlet computation and
    /// projection-aware workbench routing.
    pub edge_projection: EdgeProjectionState,

    /// Ordered app-command queue replacing a subset of hand-managed pending snapshot fields.
    pub(crate) pending_app_commands: VecDeque<AppCommand>,

    /// Accepted child-webview create requests awaiting reconcile-time renderer creation.
    pub(crate) pending_host_create_tokens: HashMap<NodeKey, PendingCreateToken>,

    /// Active Navigator specialty graphlet kind per Navigator host, keyed by
    /// `SurfaceHostId`. When present the host renders a scoped graphlet graph
    /// canvas instead of (or alongside) its normal navigator content.
    /// `None` means no specialty view is active for that host.
    pub(crate) navigator_specialty_views: HashMap<SurfaceHostId, NavigatorSpecialtyView>,
}

/// Runtime state for an active Navigator specialty graphlet view.
#[derive(Debug, Clone)]
pub struct NavigatorSpecialtyView {
    /// Graphlet kind driving this specialty view.
    pub kind: GraphletKind,
    /// The derived `GraphViewId` that holds the graphlet-masked view state.
    pub view_id: GraphViewId,
}

impl WorkbenchSessionState {
    /// Remove all workbench session data keyed by the stable node UUID on node deletion.
    pub(crate) fn on_node_deleted(&mut self, node_id: Uuid) {
        self.node_last_active_workspace.remove(&node_id);
        self.node_workspace_membership.remove(&node_id);
    }
}

#[cfg(test)]
mod tests {
    use super::WorkbenchNavigationGeometry;

    #[test]
    fn workbench_navigation_geometry_splits_content_around_overlay_sidebar() {
        let content_rect = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(400.0, 300.0));
        let overlay_rect =
            egui::Rect::from_min_max(egui::pos2(320.0, 40.0), egui::pos2(400.0, 260.0));

        let geometry =
            WorkbenchNavigationGeometry::from_content_rect(content_rect, vec![overlay_rect]);

        assert_eq!(geometry.occluding_host_rects, vec![overlay_rect]);
        assert_eq!(geometry.visible_regions.len(), 3);
        assert!(
            geometry
                .visible_regions
                .contains_rect(egui::Rect::from_min_max(
                    egui::pos2(0.0, 0.0),
                    egui::pos2(400.0, 40.0),
                ))
        );
        assert!(
            geometry
                .visible_regions
                .contains_rect(egui::Rect::from_min_max(
                    egui::pos2(0.0, 260.0),
                    egui::pos2(400.0, 300.0),
                ))
        );
        assert!(
            geometry
                .visible_regions
                .contains_rect(egui::Rect::from_min_max(
                    egui::pos2(0.0, 40.0),
                    egui::pos2(320.0, 260.0),
                ))
        );
        let largest_rect = geometry.visible_regions.largest_rect();
        assert_eq!(
            largest_rect,
            Some(egui::Rect::from_min_max(
                egui::pos2(0.0, 40.0),
                egui::pos2(320.0, 260.0),
            )),
        );
    }
}

/// Transient chrome overlay flags, shortcuts, and UI preferences.
pub struct ChromeUiState {
    /// Active tab in the History Manager panel.
    pub history_manager_tab: HistoryManagerTab,

    /// Active page in the Settings tool pane.
    pub settings_tool_page: SettingsToolPage,

    /// Whether the graph-scoped settings overlay is open.
    pub show_settings_overlay: bool,

    /// Whether the graph-scoped scene overlay is open.
    pub show_scene_overlay: bool,

    /// Graph view targeted by the scene overlay, if any.
    pub scene_overlay_view: Option<GraphViewId>,

    /// Whether the keyboard shortcut help panel is open.
    pub show_help_panel: bool,

    /// Whether the command palette is open.
    pub show_command_palette: bool,

    /// Whether the contextual command popup is open.
    pub show_context_palette: bool,

    /// Whether the command palette is in contextual list mode.
    pub command_palette_contextual_mode: bool,

    /// Pointer anchor for the contextual command popup.
    pub context_palette_anchor: Option<[f32; 2]>,

    /// Whether the radial command UI is open.
    pub show_radial_menu: bool,

    /// Consolidated action-surface state. Source of truth for
    /// palette/radial open-state; the four bool fields above are
    /// maintained in sync for legacy readers during the 2026-04-20
    /// action surfaces redesign migration.
    pub surface_state: crate::app::action_surface::ActionSurfaceState,

    /// Whether the web clip inspector surface is open.
    pub show_clip_inspector: bool,

    /// Whether the transient workbench overlay is open over the graph surface.
    pub show_workbench_overlay: bool,

    /// Preferred toast anchor location.
    pub toast_anchor_preference: ToastAnchorPreference,

    /// Shortcut binding for command palette.
    pub command_palette_shortcut: CommandPaletteShortcut,

    /// Shortcut binding for help panel.
    pub help_panel_shortcut: HelpPanelShortcut,

    /// Shortcut binding for radial menu.
    pub radial_menu_shortcut: RadialMenuShortcut,

    /// Preferred contextual command surface for secondary-click invocation.
    pub context_command_surface_preference: ContextCommandSurfacePreference,

    /// Keyboard pan speed for graph camera controls.
    pub keyboard_pan_step: f32,

    /// Keyboard pan input mode (WASD + arrows, or arrows-only).
    pub keyboard_pan_input_mode: KeyboardPanInputMode,

    /// Preferred lasso binding for canvas interactions.
    pub lasso_binding_preference: CanvasLassoBinding,

    /// Preferred default non-`@` omnibar scope behavior.
    pub omnibar_preferred_scope: OmnibarPreferredScope,

    /// Non-`@` omnibar ordering preset.
    pub omnibar_non_at_order: OmnibarNonAtOrderPreset,

    /// Maximum omnibar dropdown rows shown before scrolling/truncating.
    /// Clamped at load to `[3, 24]`.
    pub omnibar_dropdown_max_rows: usize,

    /// Fixed height of the top chrome command bar in device-independent
    /// pixels. Clamped at load to `[24.0, 96.0]`.
    pub toolbar_height_dp: f32,

    /// Debounce window for external search-provider suggestion
    /// requests, in milliseconds. Clamped at load to `[0, 2000]`.
    /// A value of `0` disables debouncing (every keystroke issues a
    /// request); the default of `140` matches the prior hardcoded
    /// behavior. Larger values are useful on slow networks or when
    /// conserving API quota.
    pub omnibar_provider_debounce_ms: u64,

    /// Default scope filter applied when the command palette opens
    /// in search mode. The widget resets to this scope on every
    /// open; mid-session changes are remembered within the session
    /// but not persisted. Defaults to `Workbench`.
    pub command_palette_default_scope:
        crate::shell::desktop::ui::command_palette_state::SearchPaletteScope,

    /// Soft cap on how many result rows the command palette shows per
    /// category in search mode. Higher-ranked results surface first;
    /// rows beyond the cap are truncated with a "…" affordance. A
    /// value of `0` disables the cap (show all matches). Clamped at
    /// load to `[0, 100]`.
    pub command_palette_max_per_category: usize,

    /// Ring of recently-executed command-palette actions, most-recent
    /// first. When the palette opens in search mode with an empty
    /// query, the top `command_palette_recents_depth` of this list is
    /// surfaced as a "Recent" section above the categorized results.
    /// Persisted across sessions as a JSON list via serde.
    pub command_palette_recents: Vec<crate::render::action_registry::ActionId>,

    /// Maximum number of recent commands to retain / surface in the
    /// command palette's empty-query roster. `0` disables the
    /// "Recent" section entirely. Clamped at load to `[0, 32]`.
    pub command_palette_recents_depth: usize,

    /// Persistent default Tier 1 category for contextual-palette mode.
    /// When the palette opens in contextual mode and the runtime
    /// session has no in-memory Tier 1 selection yet (first frame
    /// post-restart), this value seeds the selection. Updated to the
    /// user's chosen category every time they click a Tier 1 button,
    /// so next-session the palette reopens on the last-used tier.
    /// `None` means "fall back to the first available category".
    pub command_palette_tier1_default_category:
        Option<crate::render::action_registry::ActionCategory>,

    /// Global Wry backend enable toggle (disabled by default).
    pub wry_enabled: bool,

    /// Default backend for web content when no per-pane override is set.
    pub default_web_viewer_backend: crate::app::DefaultWebViewerBackend,

    /// Preferred Wry realization mode for platforms that support multiple modes.
    pub wry_render_mode_preference: crate::app::WryRenderModePreference,

    /// Workspace-managed user stylesheet entries for Servo-backed WebViews.
    pub workspace_user_stylesheets: Vec<WorkspaceUserStylesheetSetting>,

    /// Whether workspace user stylesheet state has been initialized from persistence or runtime.
    pub workspace_user_stylesheets_initialized: bool,

    /// Whether runtime `UserContentManager` state matches the workspace stylesheet settings.
    pub workspace_user_stylesheets_runtime_synced: bool,

    /// Draft path input for adding a workspace user stylesheet.
    pub workspace_user_stylesheet_add_input: String,

    /// Status message for workspace stylesheet operations.
    pub workspace_user_stylesheet_status_message: Option<String>,

    /// Snapshot refresh cadence in seconds for active webview previews.
    pub webview_preview_active_refresh_secs: u64,

    /// Snapshot refresh cadence in seconds for warm webview previews.
    pub webview_preview_warm_refresh_secs: u64,

    /// Preferred side for the default desktop workbench-scoped Navigator sidebar.
    pub navigator_sidebar_side_preference: NavigatorSidebarSidePreference,

    /// Preferred presentation mode for the workbench surface.
    pub workbench_display_mode: WorkbenchDisplayMode,

    /// Whether the default workbench host stays visible even without hosted panes.
    pub workbench_host_pinned: bool,

    /// Whether form draft capture/replay metadata is enabled.
    pub(crate) form_draft_capture_enabled: bool,

    /// User-configurable focus-ring visual settings (duration, curve,
    /// enabled toggle, color override). Defaults reproduce the historical
    /// 500 ms linear fade with theme-driven color.
    pub focus_ring_settings: super::settings_persistence::FocusRingSettings,

    /// User-configurable node-thumbnail capture settings (enabled kill
    /// switch, target dimensions, resampling filter). Defaults reproduce
    /// the historical 256×192 Triangle-filtered capture.
    pub thumbnail_settings: super::settings_persistence::ThumbnailSettings,

    /// Persisted default registry lens id override for view lens resolution.
    pub(crate) default_registry_lens_id: Option<String>,

    /// Persisted default physics preset selection for graph dynamics controls.
    pub(crate) default_registry_physics_id: Option<String>,

    /// Persisted default theme selection for workspace appearance controls.
    pub(crate) default_registry_theme_id: Option<String>,

    /// User preference for theme mode: follow OS, always light, or always dark.
    pub(crate) theme_mode: crate::app::ThemeMode,

    /// Active filter for the mixed history timeline All tab.
    pub mixed_timeline_filter: crate::services::persistence::types::HistoryTimelineFilter,
}
