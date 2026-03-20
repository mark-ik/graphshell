/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Typed sub-state structs extracted from `GraphWorkspace`.

use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
use std::time::{Duration, Instant};

use uuid::Uuid;

use crate::graph::egui_adapter::EguiGraphState;
use crate::graph::physics::GraphPhysicsState;
use crate::graph::{Graph, NodeKey};
use crate::registries::atomic::knowledge::SemanticClassVector;
use crate::registries::domain::layout::canvas::CanvasLassoBinding;
use crate::shell::desktop::runtime::caches::RuntimeCaches;

use super::AppCommand;
use super::{
    Camera, ClipInspectorState, CommandPaletteShortcut, ContextCommandSurfacePreference,
    GraphReaderState, GraphSearchHistoryEntry, GraphSearchOrigin, GraphViewFrame, GraphViewId,
    GraphViewLayoutManagerState, GraphViewState, HelpPanelShortcut, HistoryManagerTab,
    HistoryTraversalFailureReason, KeyboardPanInputMode, MemoryPressureLevel,
    NavigatorProjectionState, OmnibarNonAtOrderPreset, OmnibarPreferredScope, PendingCreateToken,
    RadialMenuShortcut, RendererId, RuntimeBlockState, SearchDisplayMode, SelectionScope,
    SelectionState, SettingsToolPage, TagPanelState, ToastAnchorPreference, UndoRedoSnapshot,
    ViewDimension, WorkbenchIntent,
};

/// View-layer runtime state: physics, selection, views, search, history, rendering.
pub struct GraphViewRuntimeState {
    /// Force-directed layout state owned by app/runtime UI controls.
    pub physics: GraphPhysicsState,

    /// Physics running state before user drag/pan interaction began.
    pub(crate) physics_running_before_interaction: Option<bool>,

    /// Canonical selection state keyed by runtime selection scope.
    pub(crate) selection_by_scope: HashMap<SelectionScope, SelectionState>,

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

    /// Cached egui_graphs state (persists across frames for drag/interaction).
    pub egui_state: Option<EguiGraphState>,

    /// Invariant: must only be set directly for non-structural visual changes.
    pub egui_state_dirty: bool,

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

    /// True while current tile tree was synthesized without a named restore context.
    pub(crate) current_workspace_is_synthesized: bool,

    /// True if graph-mutating action happened since last workspace baseline/save.
    pub(crate) workspace_has_unsaved_changes: bool,

    /// True after we've emitted a warning for the current unsaved workspace state.
    pub(crate) unsaved_workspace_prompt_warned: bool,

    /// Pending workbench-authority intents staged for frame-loop orchestration.
    pub(crate) pending_workbench_intents: Vec<WorkbenchIntent>,

    /// Ordered app-command queue replacing a subset of hand-managed pending snapshot fields.
    pub(crate) pending_app_commands: VecDeque<AppCommand>,

    /// Accepted child-webview create requests awaiting reconcile-time renderer creation.
    pub(crate) pending_host_create_tokens: HashMap<NodeKey, PendingCreateToken>,
}

impl WorkbenchSessionState {
    /// Remove all workbench session data keyed by the stable node UUID on node deletion.
    pub(crate) fn on_node_deleted(&mut self, node_id: Uuid) {
        self.node_last_active_workspace.remove(&node_id);
        self.node_workspace_membership.remove(&node_id);
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

    /// Whether the web clip inspector surface is open.
    pub show_clip_inspector: bool,

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

    /// Whether camera panning keeps slight inertia after manual input ends.
    pub camera_pan_inertia_enabled: bool,

    /// Damping factor for camera pan inertia (lower settles faster).
    pub camera_pan_inertia_damping: f32,

    /// Preferred lasso binding for canvas interactions.
    pub lasso_binding_preference: CanvasLassoBinding,

    /// Preferred default non-`@` omnibar scope behavior.
    pub omnibar_preferred_scope: OmnibarPreferredScope,

    /// Non-`@` omnibar ordering preset.
    pub omnibar_non_at_order: OmnibarNonAtOrderPreset,

    /// Global Wry backend enable toggle (disabled by default).
    pub wry_enabled: bool,

    /// Whether the Workbench Sidebar stays visible even without hosted panes.
    pub workbench_sidebar_pinned: bool,

    /// Whether form draft capture/replay metadata is enabled.
    pub(crate) form_draft_capture_enabled: bool,

    /// Persisted default registry lens id override for view lens resolution.
    pub(crate) default_registry_lens_id: Option<String>,

    /// Persisted default physics preset selection for graph dynamics controls.
    pub(crate) default_registry_physics_id: Option<String>,

    /// Persisted default theme selection for workspace appearance controls.
    pub(crate) default_registry_theme_id: Option<String>,

    /// Active filter for the mixed history timeline All tab.
    pub mixed_timeline_filter: crate::services::persistence::types::HistoryTimelineFilter,
}
