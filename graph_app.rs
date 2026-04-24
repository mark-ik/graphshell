/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Application state management for the graph browser.

use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
use std::env;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::OnceLock;
#[cfg(not(test))]
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::mpsc as tokio_mpsc;

use crate::domain::DomainState;
use crate::graph::apply::{
    GraphDelta, GraphDeltaResult, apply_graph_delta as apply_domain_graph_delta,
};
use crate::graph::physics::{GraphPhysicsState, default_graph_physics_state};
use crate::graph::{EdgeType, Graph, NavigationTrigger, NodeKey, Traversal};
use crate::registries::atomic::diagnostics::ChannelConfig;
use crate::registries::atomic::lens::{
    LayoutMode, PhysicsProfile, ThemeData, deserialize_optional_theme_data,
};
use crate::registries::domain::layout::canvas::CanvasLassoBinding;
use crate::services::persistence::types::{LogEntry, PersistedNavigationTrigger};
use crate::services::persistence::{GraphStore, TimelineIndexEntry};
use crate::app::runtime_ports::{
    CachePolicy, DiagnosticEvent, InputBinding, InputBindingRemap, InputContext,
    InputRemapConflict, RuntimeCaches, emit_event,
    CHANNEL_HISTORY_ARCHIVE_CLEAR_FAILED, CHANNEL_HISTORY_ARCHIVE_DISSOLVED_APPENDED,
    CHANNEL_HISTORY_ARCHIVE_EXPORT_FAILED, CHANNEL_HISTORY_TIMELINE_PREVIEW_ENTERED,
    CHANNEL_HISTORY_TIMELINE_PREVIEW_EXITED, CHANNEL_HISTORY_TIMELINE_PREVIEW_ISOLATION_VIOLATION,
    CHANNEL_HISTORY_TIMELINE_REPLAY_FAILED, CHANNEL_HISTORY_TIMELINE_REPLAY_STARTED,
    CHANNEL_HISTORY_TIMELINE_REPLAY_SUCCEEDED, CHANNEL_HISTORY_TIMELINE_RETURN_TO_PRESENT_FAILED,
    CHANNEL_HISTORY_TRAVERSAL_RECORD_FAILED, CHANNEL_HISTORY_TRAVERSAL_RECORDED,
    CHANNEL_PERSISTENCE_RECOVER_FAILED, CHANNEL_PERSISTENCE_RECOVER_SUCCEEDED,
    CHANNEL_STARTUP_PERSISTENCE_OPEN_FAILED,
    CHANNEL_UI_GRAPH_CAMERA_COMMAND_BLOCKED_MISSING_TARGET_VIEW,
    CHANNEL_UI_GRAPH_CAMERA_REQUEST_BLOCKED, CHANNEL_UI_GRAPH_KEYBOARD_ZOOM_BLOCKED,
    CHANNEL_UX_NAVIGATION_TRANSITION, phase2_apply_input_binding_remaps,
    phase2_describe_input_bindings, phase2_reset_input_binding_remaps,
};
#[cfg(not(test))]
use crate::app::runtime_ports::{
    CHANNEL_STARTUP_PERSISTENCE_OPEN_STARTED, CHANNEL_STARTUP_PERSISTENCE_OPEN_SUCCEEDED,
    CHANNEL_STARTUP_PERSISTENCE_OPEN_TIMEOUT,
};
use crate::util::{
    GraphAddress, GraphshellSettingsPath, NodeAddress, NoteAddress, VersoAddress, VersoViewTarget,
};
use euclid::default::Point2D;
use log::{debug, warn};
use uuid::Uuid;

macro_rules! impl_display_from_str {
    ($ty:ty { $($variant:path => $value:literal),+ $(,)? }) => {
        impl fmt::Display for $ty {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                match self {
                    $($variant => f.write_str($value),)+
                }
            }
        }

        impl FromStr for $ty {
            type Err = ();

            fn from_str(raw: &str) -> Result<Self, Self::Err> {
                match raw.trim() {
                    $($value => Ok($variant),)+
                    _ => Err(()),
                }
            }
        }
    };
}

#[path = "app/selection.rs"]
mod selection;
pub use selection::{ClipboardCopyKind, ClipboardCopyRequest, SelectionState, SelectionUpdateMode};
pub(crate) use selection::{SelectionScope, UndoRedoSnapshot};

#[path = "app/history.rs"]
mod history;
pub use history::{HistoryCaptureStatus, HistoryManagerTab, HistoryTraversalFailureReason};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistenceHealthSummary {
    pub store_status: &'static str,
    pub recovered_graph: bool,
    pub snapshot_interval_secs: Option<u64>,
    pub last_snapshot_age_secs: Option<u64>,
    pub named_graph_snapshot_count: usize,
    pub workspace_layout_count: usize,
    pub traversal_archive_count: usize,
    pub dissolved_archive_count: usize,
    pub workspace_autosave_interval_secs: u64,
    pub workspace_autosave_retention: u8,
}

#[path = "app/history_runtime.rs"]
mod history_runtime;

#[path = "app/intents.rs"]
mod intents;
pub use intents::{
    AppCommand, BrowserCommand, BrowserCommandTarget, GraphIntent, GraphMutation,
    NodeStatusNoticeRequest, RuntimeEvent, RuntimeUserStylesheetSpec, UiNotificationLevel,
    ViewAction,
};

#[path = "app/clip_capture.rs"]
mod clip_capture;
pub use clip_capture::ClipCaptureData;
pub use clip_capture::{
    ClipInspectorFilter, ClipInspectorState, clip_capture_matches_filter,
    clip_capture_matches_query,
};
pub(crate) use clip_capture::{user_visible_node_title_from_data, user_visible_node_url_from_data};

#[path = "app/agents/mod.rs"]
pub(crate) mod agents;

#[path = "app/workspace_commands.rs"]
mod workspace_commands;

#[path = "app/routing.rs"]
mod routing;
pub use routing::{SettingsRouteTarget, ToolSurfaceReturnTarget};

#[path = "app/workspace_routing.rs"]
mod workspace_routing;
#[allow(unused_imports)]
pub use workspace_routing::ViewGraphletPartition;

#[path = "app/workbench_commands.rs"]
mod workbench_commands;

#[path = "app/arrangement_graph_bridge.rs"]
mod arrangement_graph_bridge;

#[path = "app/focus_selection.rs"]
mod focus_selection;

#[path = "app/graph_views.rs"]
mod graph_views;
#[cfg(test)]
pub use graph_views::GraphViewSlot;
#[cfg(test)]
pub(crate) use graph_views::PersistedGraphViewLayoutManager;
#[allow(unused_imports)]
pub use graph_views::{
    Camera, EdgeProjectionState, GraphViewFrame, GraphViewId, GraphViewLayoutDirection,
    GraphViewLayoutManagerState, GraphViewState, PolicyValueSource, ResolvedLensPreset, SceneMode,
    SelectionEdgeProjectionOverride, SimulateBehaviorPreset, ThreeDMode, ViewDimension, ZSource,
};
pub(crate) use graph_views::{
    default_semantic_depth_dimension, default_view_dimension_for_mode, is_semantic_depth_dimension,
    view_dimension_summary,
};

#[path = "app/graph_layout.rs"]
pub(crate) mod graph_layout;

#[path = "app/runtime_lifecycle.rs"]
mod runtime_lifecycle;
pub use runtime_lifecycle::{HostOpenRequest, OpenSurfaceSource, PendingCreateToken};

#[path = "app/graph_mutations.rs"]
mod graph_mutations;
pub use graph_mutations::{NoteId, NoteRecord};

#[path = "app/ux_navigation.rs"]
mod ux_navigation;
pub use ux_navigation::ModalSurface;

#[path = "app/action_surface.rs"]
pub(crate) mod action_surface;
pub use action_surface::{ActionScope, ActionSurfaceState, Anchor, ScopeTarget};

#[path = "app/startup_persistence.rs"]
mod startup_persistence;

#[path = "app/settings_persistence.rs"]
mod settings_persistence;
#[allow(unused_imports)]
pub use settings_persistence::{
    DefaultWebViewerBackend, FocusRingCurve, FocusRingSettings, NavigatorSidebarSidePreference,
    SettingsToolPage, ThemeMode, ThumbnailAspect, ThumbnailFilter, ThumbnailFormat,
    ThumbnailSettings, WorkspaceUserStylesheetSetting, WryRenderModePreference,
};

#[path = "app/workbench_layout_policy.rs"]
pub(crate) mod workbench_layout_policy;
pub use workbench_layout_policy::{
    NavigatorHostScope, SurfaceFirstUsePolicy, SurfaceHostId, UxConfigMode,
    WorkbenchLayoutConstraint, WorkbenchProfile,
};

#[path = "app/persistence_facade.rs"]
mod persistence_facade;

#[path = "app/storage_interop/mod.rs"]
mod storage_interop;

#[path = "app/workspace_state.rs"]
mod workspace_state;
pub use workspace_state::{
    ChromeUiState, FrameHintTabRuntime, FrameTileGroupRuntimeState, GraphTooltipTarget,
    GraphViewRuntimeState, NavigatorSpecialtyView, SemanticNavigationNodeRuntime,
    SemanticNavigationRuntimeState, VisibleNavigationRegionSet, WorkbenchNavigationGeometry,
    WorkbenchSessionState,
};

#[path = "app/intent_phases.rs"]
mod intent_phases;

#[path = "app/graph_app_types.rs"]
mod graph_app_types;
pub use graph_app_types::*;

#[path = "app/runtime_ports.rs"]
pub(crate) mod runtime_ports;

#[path = "app/renderer_id.rs"]
pub(crate) mod renderer_id;
pub(crate) use renderer_id::RendererId;

#[derive(Default)]
pub struct AppServices {
    persistence: Option<GraphStore>,
    sync_command_tx: Option<tokio_mpsc::Sender<crate::mods::native::verse::SyncCommand>>,
    client_storage_manager:
        Option<crate::mods::native::web_runtime::client_storage::ClientStorageManagerHandle>,
    storage_interop_coordinator: Option<storage_interop::StorageInteropCoordinatorHandle>,
}

impl AppServices {
    fn new(persistence: Option<GraphStore>) -> Self {
        Self {
            persistence,
            sync_command_tx: None,
            client_storage_manager: None,
            storage_interop_coordinator: None,
        }
    }
}

/// Pure, serializable workspace data.
pub struct GraphWorkspace {
    /// Canonical durable graph and domain truth.
    pub domain: DomainState,

    /// View-layer runtime state: physics, selection, views, search, history, rendering.
    pub graph_runtime: GraphViewRuntimeState,

    /// Workbench session state: frame layouts, pending intents, arrangement sync caches.
    pub workbench_session: WorkbenchSessionState,

    /// Transient chrome overlay flags, shortcuts, and UI preferences.
    pub chrome_ui: ChromeUiState,
}

#[derive(Debug, Clone)]
pub struct TagPanelState {
    pub node_key: NodeKey,
    pub text_input: String,
    pub icon_picker_open: bool,
    pub icon_search_input: String,
    pub prefer_pane_anchor: bool,
    pub pending_icon_override: Option<crate::graph::badge::BadgeIcon>,
}

/// Main application state (workspace + runtime services).
pub struct GraphBrowserApp {
    pub workspace: GraphWorkspace,
    workbench_tile_selection: WorkbenchTileSelectionState,
    services: AppServices,
    pub(crate) file_access_policy: crate::prefs::FileAccessPolicy,
}

impl GraphBrowserApp {
    const STARTUP_PERSISTENCE_OPEN_TIMEOUT_MS: u64 = 600;

    pub const SESSION_WORKSPACE_LAYOUT_NAME: &'static str = "workspace:session-latest";
    const SESSION_WORKSPACE_PREV_PREFIX: &'static str = "workspace:session-prev-";
    pub const WORKSPACE_PIN_WORKSPACE_NAME: &'static str = "workspace:pin-workspace-current";
    pub const WORKSPACE_PIN_PANE_NAME: &'static str = "workspace:pin-pane-current";
    pub const SETTINGS_TOAST_ANCHOR_NAME: &'static str = "workspace:settings-toast-anchor";
    pub const SETTINGS_COMMAND_PALETTE_SHORTCUT_NAME: &'static str =
        "workspace:settings-command-palette-shortcut";
    pub const SETTINGS_HELP_PANEL_SHORTCUT_NAME: &'static str =
        "workspace:settings-help-panel-shortcut";
    pub const SETTINGS_RADIAL_MENU_SHORTCUT_NAME: &'static str =
        "workspace:settings-radial-menu-shortcut";
    pub const SETTINGS_CONTEXT_COMMAND_SURFACE_NAME: &'static str =
        "workspace:settings-context-command-surface";
    pub const SETTINGS_KEYBOARD_PAN_STEP_NAME: &'static str =
        "workspace:settings-keyboard-pan-step";
    pub const SETTINGS_KEYBOARD_PAN_INPUT_MODE_NAME: &'static str =
        "workspace:settings-keyboard-pan-input-mode";
    // SETTINGS_CAMERA_PAN_INERTIA_{ENABLED,DAMPING}_NAME removed 2026-04-20.
    // Zombie workspace-global prefs — persisted and displayed but never
    // drove behavior. Pan inertia now lives on `NavigationPolicy`
    // (per-view override + per-graph default). Old layout-json keys
    // `workspace:settings-camera-pan-inertia-*` in legacy workspace
    // snapshots are silently ignored on load.

    pub const SETTINGS_LASSO_BINDING_NAME: &'static str = "workspace:settings-lasso-binding";
    pub const SETTINGS_INPUT_BINDING_REMAPS_NAME: &'static str =
        "workspace:settings-input-binding-remaps";
    pub const SETTINGS_OMNIBAR_PREFERRED_SCOPE_NAME: &'static str =
        "workspace:settings-omnibar-preferred-scope";
    pub const SETTINGS_OMNIBAR_NON_AT_ORDER_NAME: &'static str =
        "workspace:settings-omnibar-non-at-order";
    pub const SETTINGS_OMNIBAR_DROPDOWN_MAX_ROWS_NAME: &'static str =
        "workspace:settings-omnibar-dropdown-max-rows";
    pub const SETTINGS_TOOLBAR_HEIGHT_DP_NAME: &'static str =
        "workspace:settings-toolbar-height-dp";
    pub const SETTINGS_OMNIBAR_PROVIDER_DEBOUNCE_MS_NAME: &'static str =
        "workspace:settings-omnibar-provider-debounce-ms";
    pub const SETTINGS_COMMAND_PALETTE_DEFAULT_SCOPE_NAME: &'static str =
        "workspace:settings-command-palette-default-scope";
    pub const SETTINGS_COMMAND_PALETTE_MAX_PER_CATEGORY_NAME: &'static str =
        "workspace:settings-command-palette-max-per-category";
    pub const SETTINGS_COMMAND_PALETTE_RECENTS_DEPTH_NAME: &'static str =
        "workspace:settings-command-palette-recents-depth";
    pub const SETTINGS_COMMAND_PALETTE_RECENTS_NAME: &'static str =
        "workspace:settings-command-palette-recents";
    pub const SETTINGS_COMMAND_PALETTE_TIER1_DEFAULT_CATEGORY_NAME: &'static str =
        "workspace:settings-command-palette-tier1-default-category";
    pub const SETTINGS_WRY_ENABLED_NAME: &'static str = "workspace:settings-wry-enabled";
    pub const SETTINGS_WEBVIEW_PREVIEW_ACTIVE_REFRESH_SECS_NAME: &'static str =
        "workspace:settings-webview-preview-active-refresh-secs";
    pub const SETTINGS_WEBVIEW_PREVIEW_WARM_REFRESH_SECS_NAME: &'static str =
        "workspace:settings-webview-preview-warm-refresh-secs";
    pub const SETTINGS_NAVIGATOR_SIDEBAR_SIDE_NAME: &'static str =
        "workspace:settings-navigator-sidebar-side";
    pub const SETTINGS_WORKBENCH_DISPLAY_MODE_NAME: &'static str =
        "workspace:settings-workbench-display-mode";
    pub const SETTINGS_WORKBENCH_HOST_PINNED_NAME: &'static str =
        "workspace:settings-workbench-host-pinned";
    pub const SETTINGS_WORKBENCH_PROFILE_STATE_NAME: &'static str =
        "workspace:settings-workbench-profile-state";
    pub const SETTINGS_REGISTRY_LENS_ID_NAME: &'static str = "workspace:settings-registry-lens-id";
    pub const SETTINGS_REGISTRY_PHYSICS_ID_NAME: &'static str =
        "workspace:settings-registry-physics-id";
    pub const SETTINGS_REGISTRY_THEME_ID_NAME: &'static str =
        "workspace:settings-registry-theme-id";
    pub const SETTINGS_THEME_MODE_NAME: &'static str = "workspace:settings-theme-mode";
    pub const SETTINGS_WORKBENCH_SURFACE_PROFILE_ID_NAME: &'static str =
        "workspace:settings-workbench-surface-profile-id";
    pub const SETTINGS_CANVAS_PROFILE_ID_NAME: &'static str =
        "workspace:settings-canvas-profile-id";
    pub const SETTINGS_ACTIVE_WORKFLOW_ID_NAME: &'static str =
        "workspace:settings-active-workflow-id";
    pub const SETTINGS_NOSTR_SIGNER_SETTINGS_NAME: &'static str =
        "workspace:settings-nostr-signer-settings";
    pub const SETTINGS_NOSTR_NIP07_PERMISSIONS_NAME: &'static str =
        "workspace:settings-nostr-nip07-permissions";
    pub const SETTINGS_NOSTR_SUBSCRIPTIONS_NAME: &'static str =
        "workspace:settings-nostr-subscriptions";
    pub const SETTINGS_GRAPH_VIEW_LAYOUT_MANAGER_NAME: &'static str =
        "workspace:settings-graph-view-layout-manager";
    pub const SETTINGS_DIAGNOSTICS_CHANNEL_CONFIG_PREFIX: &'static str =
        "workspace:settings-diagnostics-channel-config:";
    pub const DEFAULT_WORKSPACE_AUTOSAVE_INTERVAL_SECS: u64 = 60;
    pub const DEFAULT_WORKSPACE_AUTOSAVE_RETENTION: u8 = 1;
    pub const DEFAULT_ACTIVE_WEBVIEW_LIMIT: usize = 4;
    pub const DEFAULT_WARM_CACHE_LIMIT: usize = 12;
    pub const DEFAULT_WEBVIEW_PREVIEW_ACTIVE_REFRESH_SECS: u64 = 2;
    pub const DEFAULT_WEBVIEW_PREVIEW_WARM_REFRESH_SECS: u64 = 30;
    /// Default fixed height of the top chrome command bar in device-
    /// independent pixels. Previously hardcoded in `toolbar_ui::TOOLBAR_HEIGHT`;
    /// exposed as a setting in M4 so users can increase the target for
    /// accessibility or shrink it for dense layouts.
    pub const DEFAULT_TOOLBAR_HEIGHT_DP: f32 = 40.0;
    /// Default maximum number of omnibar dropdown rows shown before
    /// scrolling / truncation. Previously hardcoded in
    /// `toolbar_ui::OMNIBAR_DROPDOWN_MAX_ROWS`; exposed as a setting in
    /// M4 so high-resolution users can widen and laptop users can shrink.
    pub const DEFAULT_OMNIBAR_DROPDOWN_MAX_ROWS: usize = 8;
    /// Default debounce window for external search-provider suggestion
    /// requests (DuckDuckGo / Bing / Google), in milliseconds. Larger
    /// values reduce request volume at the cost of snappiness; smaller
    /// values feel instant but issue more requests. Previously
    /// hardcoded in `toolbar_ui::OMNIBAR_PROVIDER_DEBOUNCE_MS`.
    pub const DEFAULT_OMNIBAR_PROVIDER_DEBOUNCE_MS: u64 = 140;
    /// Default soft cap on command-palette search result rows per
    /// category. `0` disables the cap.
    pub const DEFAULT_COMMAND_PALETTE_MAX_PER_CATEGORY: usize = 12;
    /// Default depth of the command-palette recents ring. `0`
    /// disables the "Recent" section entirely.
    pub const DEFAULT_COMMAND_PALETTE_RECENTS_DEPTH: usize = 8;
    pub const DEFAULT_KEYBOARD_PAN_STEP: f32 = 12.0;
    // DEFAULT_CAMERA_PAN_INERTIA_{ENABLED,DAMPING} removed 2026-04-20 —
    // the workspace-global inertia fields they initialized were zombie
    // prefs. Pan inertia defaults now live on
    // `graph_canvas::navigation::NavigationPolicy`.
    pub const TAG_PIN: &'static str = crate::graph::badge::TAG_PIN;
    pub const TAG_STARRED: &'static str = crate::graph::badge::TAG_STARRED;
    pub const TAG_ARCHIVE: &'static str = crate::graph::badge::TAG_ARCHIVE;
    pub const TAG_RESIDENT: &'static str = crate::graph::badge::TAG_RESIDENT;
    pub const TAG_PRIVATE: &'static str = crate::graph::badge::TAG_PRIVATE;
    pub const TAG_NOHISTORY: &'static str = crate::graph::badge::TAG_NOHISTORY;
    pub const TAG_MONITOR: &'static str = crate::graph::badge::TAG_MONITOR;
    pub const TAG_UNREAD: &'static str = crate::graph::badge::TAG_UNREAD;
    pub const TAG_FOCUS: &'static str = crate::graph::badge::TAG_FOCUS;
    pub const TAG_CLIP: &'static str = crate::graph::badge::TAG_CLIP;

    pub fn default_physics_state() -> GraphPhysicsState {
        default_graph_physics_state()
    }

    /// Create a new graph browser application
    pub fn new() -> Self {
        Self::new_from_dir(GraphStore::default_data_dir())
    }

    /// Create a new graph browser application using a specific persistence directory.
    pub fn new_from_dir(data_dir: PathBuf) -> Self {
        let (graph, persistence) = Self::recover_graph_for_startup(data_dir);

        // Scan recovered graph for existing placeholder IDs to avoid collisions
        let next_placeholder_id = Self::scan_max_placeholder_id(&graph);

        let mut app = Self {
            workspace: GraphWorkspace {
                domain: DomainState {
                    graph,
                    next_placeholder_id,
                    notes: HashMap::new(),
                    navigation_policy_default:
                        graph_canvas::navigation::NavigationPolicy::default(),
                    node_style_default: graph_canvas::node_style::NodeStyle::default(),
                    simulate_motion_default: None,
                },
                graph_runtime: GraphViewRuntimeState {
                    physics: Self::default_physics_state(),
                    physics_running_before_interaction: None,
                    selection_by_scope: HashMap::new(),
                    selection_edge_projections: HashMap::new(),
                    webview_to_node: HashMap::new(),
                    node_to_webview: HashMap::new(),
                    embedded_content_focus_webview: None,
                    runtime_block_state: HashMap::new(),
                    runtime_caches: RuntimeCaches::new(CachePolicy::default(), None),
                    active_webview_nodes: Vec::new(),
                    active_lru: Vec::new(),
                    active_webview_limit: Self::DEFAULT_ACTIVE_WEBVIEW_LIMIT,
                    warm_cache_lru: Vec::new(),
                    warm_cache_limit: Self::DEFAULT_WARM_CACHE_LIMIT,
                    is_interacting: false,
                    drag_release_frames_remaining: 0,
                    views: HashMap::new(),
                    graph_view_layout_manager: GraphViewLayoutManagerState::default(),
                    graph_view_frames: HashMap::new(),
                    graph_view_canvas_rects: HashMap::new(),
                    scene_runtimes: HashMap::new(),
                    #[cfg(not(target_arch = "wasm32"))]
                    canvas_interaction_engines: HashMap::new(),
                    canvas_cameras: HashMap::new(),
                    node_pane_ids: HashMap::new(),
                    pane_render_modes: HashMap::new(),
                    pane_viewer_ids: HashMap::new(),
                    active_pane_rects: Vec::new(),
                    cached_tree_rows: Vec::new(),
                    cached_tab_order: Vec::new(),
                    cached_split_boundaries: Vec::new(),
                    simulate_release_impulses: HashMap::new(),
                    hovered_scene_region: None,
                    selected_scene_regions: HashMap::new(),
                    active_scene_region_drag: None,
                    workbench_navigation_geometry: None,
                    focused_view: None,
                    graph_reader_state: GraphReaderState::default(),
                    camera: Camera::new(),
                    undo_stack: Vec::new(),
                    redo_stack: Vec::new(),
                    hop_distance_cache: None,
                    last_culled_node_keys: None,
                    memory_pressure_level: MemoryPressureLevel::Unknown,
                    memory_available_mib: 0,
                    memory_total_mib: 0,
                    history_recent_traversal_append_failures: 0,
                    history_preview_mode_active: false,
                    history_last_preview_isolation_violation: false,
                    history_replay_in_progress: false,
                    history_replay_cursor: None,
                    history_replay_total_steps: None,
                    history_preview_live_graph_snapshot: None,
                    history_preview_graph: None,
                    history_last_event_unix_ms: None,
                    history_last_error: None,
                    history_recent_failure_reason_bucket: None,
                    history_last_return_to_present_result: None,
                    semantic_navigation: SemanticNavigationRuntimeState::default(),
                    semantic_index: HashMap::new(),
                    semantic_index_dirty: true,
                    semantic_depth_restore_dimensions: HashMap::new(),
                    suggested_semantic_tags: HashMap::new(),
                    hovered_graph_node: None,
                    hovered_graph_edge: None,
                    highlighted_graph_edge: None,
                    dismissed_graph_tooltip: None,
                    selected_frame_name: None,
                    frame_tile_groups: HashMap::new(),
                    navigator_projection_state: NavigatorProjectionState::default(),
                    selected_tab_nodes: HashSet::new(),
                    tab_selection_anchor: None,
                    search_display_mode: SearchDisplayMode::Highlight,
                    active_graph_search_query: String::new(),
                    active_graph_search_match_count: 0,
                    active_graph_search_origin: GraphSearchOrigin::Manual,
                    active_graph_search_neighborhood_anchor: None,
                    active_graph_search_neighborhood_depth: 1,
                    graph_search_history: Vec::new(),
                    pinned_graph_search: None,
                    tag_panel_state: None,
                    clip_inspector_state: None,
                    pending_clip_inspector_highlight_clear: None,
                },
                workbench_session: WorkbenchSessionState {
                    last_session_workspace_layout_hash: None,
                    last_session_workspace_layout_json: None,
                    workspace_autosave_interval: Duration::from_secs(
                        Self::DEFAULT_WORKSPACE_AUTOSAVE_INTERVAL_SECS,
                    ),
                    workspace_autosave_retention: Self::DEFAULT_WORKSPACE_AUTOSAVE_RETENTION,
                    last_workspace_autosave_at: None,
                    workspace_activation_seq: 0,
                    node_last_active_workspace: HashMap::new(),
                    node_workspace_membership: HashMap::new(),
                    current_workspace_name: None,
                    current_frame_tab_semantics: None,
                    current_workspace_is_synthesized: false,
                    workspace_has_unsaved_changes: false,
                    unsaved_workspace_prompt_warned: false,
                    pending_workbench_intents: Vec::new(),
                    workbench_profile: WorkbenchProfile::default(),
                    active_layout_constraints: HashMap::new(),
                    draft_layout_constraints: HashMap::new(),
                    ux_config_mode: UxConfigMode::Locked,
                    session_suppressed_first_use_prompts: HashSet::new(),
                    session_dismissed_frame_split_offers: HashSet::new(),
                    edge_projection: EdgeProjectionState::default(),
                    pending_app_commands: VecDeque::new(),
                    pending_host_create_tokens: HashMap::new(),
                    navigator_specialty_views: HashMap::new(),
                },
                chrome_ui: ChromeUiState {
                    history_manager_tab: HistoryManagerTab::Timeline,
                    settings_tool_page: SettingsToolPage::General,
                    show_settings_overlay: false,
                    show_scene_overlay: false,
                    scene_overlay_view: None,
                    show_help_panel: false,
                    show_command_palette: false,
                    show_context_palette: false,
                    command_palette_contextual_mode: false,
                    context_palette_anchor: None,
                    show_radial_menu: false,
                    surface_state: crate::app::action_surface::ActionSurfaceState::default(),
                    show_clip_inspector: false,
                    show_workbench_overlay: false,
                    toast_anchor_preference: ToastAnchorPreference::BottomRight,
                    command_palette_shortcut: CommandPaletteShortcut::F2,
                    help_panel_shortcut: HelpPanelShortcut::F1OrQuestion,
                    radial_menu_shortcut: RadialMenuShortcut::F3,
                    context_command_surface_preference:
                        ContextCommandSurfacePreference::RadialPalette,
                    keyboard_pan_step: Self::DEFAULT_KEYBOARD_PAN_STEP,
                    keyboard_pan_input_mode: KeyboardPanInputMode::WasdAndArrows,
                    lasso_binding_preference: CanvasLassoBinding::default(),
                    omnibar_preferred_scope: OmnibarPreferredScope::Auto,
                    omnibar_non_at_order: OmnibarNonAtOrderPreset::ContextualThenProviderThenGlobal,
                    omnibar_dropdown_max_rows: Self::DEFAULT_OMNIBAR_DROPDOWN_MAX_ROWS,
                    toolbar_height_dp: Self::DEFAULT_TOOLBAR_HEIGHT_DP,
                    omnibar_provider_debounce_ms: Self::DEFAULT_OMNIBAR_PROVIDER_DEBOUNCE_MS,
                    command_palette_default_scope:
                        crate::shell::desktop::ui::command_palette_state::SearchPaletteScope::Workbench,
                    command_palette_max_per_category:
                        Self::DEFAULT_COMMAND_PALETTE_MAX_PER_CATEGORY,
                    command_palette_recents: Vec::new(),
                    command_palette_recents_depth:
                        Self::DEFAULT_COMMAND_PALETTE_RECENTS_DEPTH,
                    command_palette_tier1_default_category: None,
                    wry_enabled: true,
                    default_web_viewer_backend: DefaultWebViewerBackend::Servo,
                    wry_render_mode_preference: WryRenderModePreference::Auto,
                    workspace_user_stylesheets: Vec::new(),
                    workspace_user_stylesheets_initialized: false,
                    workspace_user_stylesheets_runtime_synced: false,
                    workspace_user_stylesheet_add_input: String::new(),
                    workspace_user_stylesheet_status_message: None,
                    webview_preview_active_refresh_secs:
                        Self::DEFAULT_WEBVIEW_PREVIEW_ACTIVE_REFRESH_SECS,
                    webview_preview_warm_refresh_secs:
                        Self::DEFAULT_WEBVIEW_PREVIEW_WARM_REFRESH_SECS,
                    navigator_sidebar_side_preference:
                        settings_persistence::NavigatorSidebarSidePreference::Left,
                    workbench_display_mode: WorkbenchDisplayMode::Split,
                    workbench_host_pinned: false,
                    form_draft_capture_enabled: std::env::var_os("GRAPHSHELL_ENABLE_FORM_DRAFT")
                        .is_some(),
                    focus_ring_settings: settings_persistence::FocusRingSettings::default(),
                    thumbnail_settings: settings_persistence::ThumbnailSettings::default(),
                    default_registry_lens_id: None,
                    default_registry_physics_id: None,
                    default_registry_theme_id: None,
                    theme_mode: crate::app::ThemeMode::System,
                    mixed_timeline_filter:
                        crate::services::persistence::types::HistoryTimelineFilter::default(),
                },
            },
            workbench_tile_selection: WorkbenchTileSelectionState::default(),
            services: AppServices::new(persistence),
            file_access_policy: crate::prefs::FileAccessPolicy::default(),
        };
        app.load_persisted_ui_settings();
        app.rebuild_semantic_navigation_runtime();
        app
    }

    /// Create a new graph browser application without persistence (for tests
    /// and for the M5 iced-host bring-up path).
    #[cfg(any(test, feature = "iced-host"))]
    pub fn new_for_testing() -> Self {
        Self {
            workspace: GraphWorkspace {
                domain: DomainState {
                    graph: Graph::new(),
                    next_placeholder_id: 0,
                    notes: HashMap::new(),
                    navigation_policy_default:
                        graph_canvas::navigation::NavigationPolicy::default(),
                    node_style_default: graph_canvas::node_style::NodeStyle::default(),
                    simulate_motion_default: None,
                },
                graph_runtime: GraphViewRuntimeState {
                    physics: Self::default_physics_state(),
                    physics_running_before_interaction: None,
                    selection_by_scope: HashMap::new(),
                    selection_edge_projections: HashMap::new(),
                    webview_to_node: HashMap::new(),
                    node_to_webview: HashMap::new(),
                    embedded_content_focus_webview: None,
                    runtime_block_state: HashMap::new(),
                    runtime_caches: RuntimeCaches::new(CachePolicy::default(), None),
                    active_webview_nodes: Vec::new(),
                    active_lru: Vec::new(),
                    active_webview_limit: Self::DEFAULT_ACTIVE_WEBVIEW_LIMIT,
                    warm_cache_lru: Vec::new(),
                    warm_cache_limit: Self::DEFAULT_WARM_CACHE_LIMIT,
                    is_interacting: false,
                    drag_release_frames_remaining: 0,
                    views: HashMap::new(),
                    graph_view_layout_manager: GraphViewLayoutManagerState::default(),
                    graph_view_frames: HashMap::new(),
                    graph_view_canvas_rects: HashMap::new(),
                    scene_runtimes: HashMap::new(),
                    #[cfg(not(target_arch = "wasm32"))]
                    canvas_interaction_engines: HashMap::new(),
                    canvas_cameras: HashMap::new(),
                    node_pane_ids: HashMap::new(),
                    pane_render_modes: HashMap::new(),
                    pane_viewer_ids: HashMap::new(),
                    active_pane_rects: Vec::new(),
                    cached_tree_rows: Vec::new(),
                    cached_tab_order: Vec::new(),
                    cached_split_boundaries: Vec::new(),
                    simulate_release_impulses: HashMap::new(),
                    hovered_scene_region: None,
                    selected_scene_regions: HashMap::new(),
                    active_scene_region_drag: None,
                    workbench_navigation_geometry: None,
                    focused_view: None,
                    graph_reader_state: GraphReaderState::default(),
                    camera: Camera::new(),
                    undo_stack: Vec::new(),
                    redo_stack: Vec::new(),
                    hop_distance_cache: None,
                    last_culled_node_keys: None,
                    memory_pressure_level: MemoryPressureLevel::Unknown,
                    memory_available_mib: 0,
                    memory_total_mib: 0,
                    history_recent_traversal_append_failures: 0,
                    history_preview_mode_active: false,
                    history_last_preview_isolation_violation: false,
                    history_replay_in_progress: false,
                    history_replay_cursor: None,
                    history_replay_total_steps: None,
                    history_preview_live_graph_snapshot: None,
                    history_preview_graph: None,
                    history_last_event_unix_ms: None,
                    history_last_error: None,
                    history_recent_failure_reason_bucket: None,
                    history_last_return_to_present_result: None,
                    semantic_navigation: SemanticNavigationRuntimeState::default(),
                    semantic_index: HashMap::new(),
                    semantic_index_dirty: true,
                    semantic_depth_restore_dimensions: HashMap::new(),
                    suggested_semantic_tags: HashMap::new(),
                    hovered_graph_node: None,
                    hovered_graph_edge: None,
                    highlighted_graph_edge: None,
                    dismissed_graph_tooltip: None,
                    selected_frame_name: None,
                    frame_tile_groups: HashMap::new(),
                    navigator_projection_state: NavigatorProjectionState::default(),
                    selected_tab_nodes: HashSet::new(),
                    tab_selection_anchor: None,
                    search_display_mode: SearchDisplayMode::Highlight,
                    active_graph_search_query: String::new(),
                    active_graph_search_match_count: 0,
                    active_graph_search_origin: GraphSearchOrigin::Manual,
                    active_graph_search_neighborhood_anchor: None,
                    active_graph_search_neighborhood_depth: 1,
                    graph_search_history: Vec::new(),
                    pinned_graph_search: None,
                    tag_panel_state: None,
                    clip_inspector_state: None,
                    pending_clip_inspector_highlight_clear: None,
                },
                workbench_session: WorkbenchSessionState {
                    last_session_workspace_layout_hash: None,
                    last_session_workspace_layout_json: None,
                    workspace_autosave_interval: Duration::from_secs(
                        Self::DEFAULT_WORKSPACE_AUTOSAVE_INTERVAL_SECS,
                    ),
                    workspace_autosave_retention: Self::DEFAULT_WORKSPACE_AUTOSAVE_RETENTION,
                    last_workspace_autosave_at: None,
                    workspace_activation_seq: 0,
                    node_last_active_workspace: HashMap::new(),
                    node_workspace_membership: HashMap::new(),
                    current_workspace_name: None,
                    current_frame_tab_semantics: None,
                    current_workspace_is_synthesized: false,
                    workspace_has_unsaved_changes: false,
                    unsaved_workspace_prompt_warned: false,
                    pending_workbench_intents: Vec::new(),
                    workbench_profile: WorkbenchProfile::default(),
                    active_layout_constraints: HashMap::new(),
                    draft_layout_constraints: HashMap::new(),
                    ux_config_mode: UxConfigMode::Locked,
                    session_suppressed_first_use_prompts: HashSet::new(),
                    session_dismissed_frame_split_offers: HashSet::new(),
                    edge_projection: EdgeProjectionState::default(),
                    pending_app_commands: VecDeque::new(),
                    pending_host_create_tokens: HashMap::new(),
                    navigator_specialty_views: HashMap::new(),
                },
                chrome_ui: ChromeUiState {
                    history_manager_tab: HistoryManagerTab::Timeline,
                    settings_tool_page: SettingsToolPage::General,
                    show_settings_overlay: false,
                    show_scene_overlay: false,
                    scene_overlay_view: None,
                    show_help_panel: false,
                    show_command_palette: false,
                    show_context_palette: false,
                    command_palette_contextual_mode: false,
                    context_palette_anchor: None,
                    show_radial_menu: false,
                    surface_state: crate::app::action_surface::ActionSurfaceState::default(),
                    show_clip_inspector: false,
                    show_workbench_overlay: false,
                    toast_anchor_preference: ToastAnchorPreference::BottomRight,
                    command_palette_shortcut: CommandPaletteShortcut::F2,
                    help_panel_shortcut: HelpPanelShortcut::F1OrQuestion,
                    radial_menu_shortcut: RadialMenuShortcut::F3,
                    context_command_surface_preference:
                        ContextCommandSurfacePreference::RadialPalette,
                    keyboard_pan_step: Self::DEFAULT_KEYBOARD_PAN_STEP,
                    keyboard_pan_input_mode: KeyboardPanInputMode::WasdAndArrows,
                    lasso_binding_preference: CanvasLassoBinding::default(),
                    omnibar_preferred_scope: OmnibarPreferredScope::Auto,
                    omnibar_non_at_order: OmnibarNonAtOrderPreset::ContextualThenProviderThenGlobal,
                    omnibar_dropdown_max_rows: Self::DEFAULT_OMNIBAR_DROPDOWN_MAX_ROWS,
                    toolbar_height_dp: Self::DEFAULT_TOOLBAR_HEIGHT_DP,
                    omnibar_provider_debounce_ms: Self::DEFAULT_OMNIBAR_PROVIDER_DEBOUNCE_MS,
                    command_palette_default_scope:
                        crate::shell::desktop::ui::command_palette_state::SearchPaletteScope::Workbench,
                    command_palette_max_per_category:
                        Self::DEFAULT_COMMAND_PALETTE_MAX_PER_CATEGORY,
                    command_palette_recents: Vec::new(),
                    command_palette_recents_depth:
                        Self::DEFAULT_COMMAND_PALETTE_RECENTS_DEPTH,
                    command_palette_tier1_default_category: None,
                    wry_enabled: true,
                    default_web_viewer_backend: DefaultWebViewerBackend::Servo,
                    wry_render_mode_preference: WryRenderModePreference::Auto,
                    workspace_user_stylesheets: Vec::new(),
                    workspace_user_stylesheets_initialized: false,
                    workspace_user_stylesheets_runtime_synced: false,
                    workspace_user_stylesheet_add_input: String::new(),
                    workspace_user_stylesheet_status_message: None,
                    webview_preview_active_refresh_secs:
                        Self::DEFAULT_WEBVIEW_PREVIEW_ACTIVE_REFRESH_SECS,
                    webview_preview_warm_refresh_secs:
                        Self::DEFAULT_WEBVIEW_PREVIEW_WARM_REFRESH_SECS,
                    navigator_sidebar_side_preference:
                        settings_persistence::NavigatorSidebarSidePreference::Left,
                    workbench_display_mode: WorkbenchDisplayMode::Split,
                    workbench_host_pinned: false,
                    form_draft_capture_enabled: false,
                    focus_ring_settings: settings_persistence::FocusRingSettings::default(),
                    thumbnail_settings: settings_persistence::ThumbnailSettings::default(),
                    default_registry_lens_id: None,
                    default_registry_physics_id: None,
                    default_registry_theme_id: None,
                    theme_mode: crate::app::ThemeMode::System,
                    mixed_timeline_filter:
                        crate::services::persistence::types::HistoryTimelineFilter::default(),
                },
            },
            workbench_tile_selection: WorkbenchTileSelectionState::default(),
            services: AppServices::new(None),
            file_access_policy: crate::prefs::FileAccessPolicy::default(),
        }
    }

    /// Whether the graph was recovered from persistence (has nodes on startup)
    pub fn has_recovered_graph(&self) -> bool {
        self.workspace.domain.graph.node_count() > 0
    }

    pub fn domain_graph(&self) -> &Graph {
        &self.workspace.domain.graph
    }

    pub fn domain_graph_mut(&mut self) -> &mut Graph {
        &mut self.workspace.domain.graph
    }

    /// Graph surface that should currently be rendered.
    ///
    /// During Stage F history preview, rendering reads from the detached replay
    /// graph so live graph truth remains untouched until "Return to Present".
    pub fn render_graph(&self) -> &Graph {
        if self.workspace.graph_runtime.history_preview_mode_active {
            self.workspace
                .graph_runtime
                .history_preview_graph
                .as_ref()
                .unwrap_or(&self.workspace.domain.graph)
        } else {
            &self.workspace.domain.graph
        }
    }

    pub fn canonical_tags_for_node(&self, key: NodeKey) -> HashSet<String> {
        self.workspace
            .domain
            .graph
            .node_tags(key)
            .cloned()
            .unwrap_or_default()
    }

    pub fn canonical_tags_for_node_sorted(&self, key: NodeKey) -> Vec<String> {
        let mut tags = self
            .workspace
            .domain
            .graph
            .node_tags(key)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .collect::<Vec<_>>();
        tags.sort();
        tags
    }

    pub fn node_has_canonical_tag(&self, key: NodeKey, tag: &str) -> bool {
        self.workspace
            .domain
            .graph
            .node_tags(key)
            .is_some_and(|tags| tags.contains(tag))
    }

    pub fn node_tag_presentation(
        &self,
        key: NodeKey,
    ) -> Option<&crate::graph::badge::NodeTagPresentationState> {
        self.workspace.domain.graph.node_tag_presentation(key)
    }

    pub fn set_node_tag_icon_override(
        &mut self,
        key: NodeKey,
        tag: &str,
        icon: Option<crate::graph::badge::BadgeIcon>,
    ) -> bool {
        self.workspace
            .domain
            .graph
            .set_node_tag_icon_override(key, tag, icon)
    }

    pub fn graph_reader_mode(&self) -> Option<GraphReaderModeState> {
        match self
            .workspace
            .graph_runtime
            .graph_reader_state
            .mode_override
        {
            Some(GraphReaderModeState::Map { focused_node }) => Some(GraphReaderModeState::Map {
                focused_node: focused_node
                    .filter(|node_key| self.workspace.domain.graph.get_node(*node_key).is_some()),
            }),
            Some(GraphReaderModeState::Room {
                node_key,
                return_map_node,
            }) if self.workspace.domain.graph.get_node(node_key).is_some() => {
                Some(GraphReaderModeState::Room {
                    node_key,
                    return_map_node: return_map_node.filter(|node_key| {
                        self.workspace.domain.graph.get_node(*node_key).is_some()
                    }),
                })
            }
            _ => self
                .get_single_selected_node()
                .map(|node_key| GraphReaderModeState::Room {
                    node_key,
                    return_map_node: Some(node_key),
                })
                .or_else(|| {
                    (self.workspace.domain.graph.node_count() > 0)
                        .then_some(GraphReaderModeState::Map { focused_node: None })
                }),
        }
    }

    pub fn graph_reader_focus_map_node(&mut self, node_key: NodeKey) {
        if self.workspace.domain.graph.get_node(node_key).is_none() {
            return;
        }
        self.workspace
            .graph_runtime
            .graph_reader_state
            .mode_override = Some(GraphReaderModeState::Map {
            focused_node: Some(node_key),
        });
    }

    pub fn graph_reader_enter_room(&mut self, node_key: NodeKey) {
        if self.workspace.domain.graph.get_node(node_key).is_none() {
            return;
        }
        let return_map_node = match self.graph_reader_mode() {
            Some(GraphReaderModeState::Map { focused_node }) => focused_node.or(Some(node_key)),
            Some(GraphReaderModeState::Room {
                node_key: current_room,
                return_map_node,
            }) => return_map_node.or(Some(current_room)),
            None => Some(node_key),
        };
        self.select_node(node_key, false);
        self.workspace
            .graph_runtime
            .graph_reader_state
            .mode_override = Some(GraphReaderModeState::Room {
            node_key,
            return_map_node,
        });
    }

    pub fn graph_reader_return_to_map(&mut self) {
        let focused_node = match self.graph_reader_mode() {
            Some(GraphReaderModeState::Room {
                node_key,
                return_map_node,
            }) => return_map_node.or(Some(node_key)),
            Some(GraphReaderModeState::Map { focused_node }) => focused_node,
            None => None,
        };
        self.workspace
            .graph_runtime
            .graph_reader_state
            .mode_override = Some(GraphReaderModeState::Map { focused_node });
    }

    pub fn set_tab_selection_single(&mut self, key: NodeKey) {
        if self.workspace.domain.graph.get_node(key).is_none() {
            return;
        }
        self.workspace.graph_runtime.selected_tab_nodes.clear();
        self.workspace.graph_runtime.selected_tab_nodes.insert(key);
        self.workspace.graph_runtime.tab_selection_anchor = Some(key);
    }

    pub fn toggle_tab_selection(&mut self, key: NodeKey) {
        if self.workspace.domain.graph.get_node(key).is_none() {
            return;
        }
        if !self.workspace.graph_runtime.selected_tab_nodes.remove(&key) {
            self.workspace.graph_runtime.selected_tab_nodes.insert(key);
        }
        self.workspace.graph_runtime.tab_selection_anchor = Some(key);
    }

    pub fn add_tab_selection_keys(&mut self, keys: impl IntoIterator<Item = NodeKey>) {
        let mut last = None;
        for key in keys {
            if self.workspace.domain.graph.get_node(key).is_none() {
                continue;
            }
            self.workspace.graph_runtime.selected_tab_nodes.insert(key);
            last = Some(key);
        }
        if let Some(key) = last {
            self.workspace.graph_runtime.tab_selection_anchor = Some(key);
        }
    }

    pub fn navigator_projection_state(&self) -> &NavigatorProjectionState {
        &self.workspace.graph_runtime.navigator_projection_state
    }

    pub fn set_navigator_projection_seed_source(&mut self, source: NavigatorProjectionSeedSource) {
        self.workspace
            .graph_runtime
            .navigator_projection_state
            .projection_seed_source = source;
        self.rebuild_navigator_projection_rows();
    }

    pub fn set_navigator_sort_mode(&mut self, sort_mode: NavigatorSortMode) {
        self.workspace
            .graph_runtime
            .navigator_projection_state
            .sort_mode = sort_mode;
    }

    pub fn set_navigator_projection_mode(&mut self, mode: NavigatorProjectionMode) {
        self.workspace.graph_runtime.navigator_projection_state.mode = mode;
    }

    pub fn set_navigator_root_filter(&mut self, root_filter: Option<String>) {
        self.workspace
            .graph_runtime
            .navigator_projection_state
            .root_filter = root_filter;
        self.rebuild_navigator_projection_rows();
    }

    #[cfg(test)]
    fn upsert_navigator_row_target(
        &mut self,
        row_key: impl Into<String>,
        target: NavigatorProjectionTarget,
    ) {
        self.workspace
            .graph_runtime
            .navigator_projection_state
            .row_targets
            .insert(row_key.into(), target);
    }

    pub fn set_navigator_selected_rows(&mut self, rows: impl IntoIterator<Item = String>) {
        self.workspace
            .graph_runtime
            .navigator_projection_state
            .selected_rows = rows.into_iter().collect();
    }

    pub fn set_navigator_expanded_rows(&mut self, rows: impl IntoIterator<Item = String>) {
        let expanded_rows: HashSet<String> = rows.into_iter().collect();
        self.workspace
            .graph_runtime
            .navigator_projection_state
            .expanded_rows = expanded_rows.clone();
        self.workspace
            .graph_runtime
            .navigator_projection_state
            .collapsed_rows
            .retain(|row| !expanded_rows.contains(row));
    }

    fn navigator_structural_row_keys(&self) -> HashSet<String> {
        let projection = self.navigator_section_projection();
        let mut rows = HashSet::new();

        if projection.shows_saved_views_section() {
            rows.insert("section:saved_views".to_string());
        }

        if projection.shows_workbench_section() {
            rows.insert("section:workbench".to_string());
            for group in &projection.workbench_groups {
                rows.insert(format!("group:workbench:{}", group.id));
            }
        }

        if projection.shows_containment_sections() {
            rows.insert("section:folders".to_string());
            for folder in projection.folder_sections.keys() {
                rows.insert(format!("group:folder:{folder}"));
            }

            rows.insert("section:domain".to_string());
            for domain in projection.domain_sections.keys() {
                rows.insert(format!("group:domain:{domain}"));
            }
        }

        if projection.shows_semantic_section() {
            rows.insert("section:semantic".to_string());
            for group in &projection.semantic_groups {
                rows.insert(format!("group:semantic:{}", group.title));
            }
        }

        if projection.shows_unrelated_section() {
            rows.insert("section:unrelated".to_string());
        }

        if projection.shows_recent_section() {
            rows.insert("section:recent".to_string());
        }

        if projection.shows_all_nodes_section() {
            rows.insert("section:all_nodes".to_string());
        }

        rows
    }

    pub fn rebuild_navigator_projection_rows(&mut self) {
        use NavigatorProjectionSeedSource as Source;

        let mut row_targets: HashMap<String, NavigatorProjectionTarget> = HashMap::new();
        let projection_mode = self.workspace.graph_runtime.navigator_projection_state.mode;

        match projection_mode {
            NavigatorProjectionMode::Semantic | NavigatorProjectionMode::AllNodes => {
                let mut nodes: Vec<(NodeKey, Uuid)> = self
                    .workspace
                    .domain
                    .graph
                    .nodes()
                    .map(|(key, node)| (key, node.id))
                    .collect();
                nodes.sort_by_key(|(_, node_id)| *node_id);
                for (key, node_id) in nodes {
                    row_targets.insert(
                        format!("node:{node_id}"),
                        NavigatorProjectionTarget::Node(key),
                    );
                }
            }
            NavigatorProjectionMode::Workbench | NavigatorProjectionMode::Containment => {
                match self
                    .workspace
                    .graph_runtime
                    .navigator_projection_state
                    .projection_seed_source
                {
                    Source::GraphContainment => {
                        let mut nodes: Vec<(NodeKey, Uuid)> = self
                            .workspace
                            .domain
                            .graph
                            .nodes()
                            .map(|(key, node)| (key, node.id))
                            .collect();
                        nodes.sort_by_key(|(_, node_id)| *node_id);
                        for (key, node_id) in nodes {
                            row_targets.insert(
                                format!("node:{node_id}"),
                                NavigatorProjectionTarget::Node(key),
                            );
                        }
                    }
                    Source::SavedViewCollections => {
                        let mut view_ids: Vec<GraphViewId> =
                            self.workspace.graph_runtime.views.keys().copied().collect();
                        view_ids.sort_by_key(|view_id| view_id.as_uuid());
                        for view_id in view_ids {
                            row_targets.insert(
                                format!("view:{}", view_id.as_uuid()),
                                NavigatorProjectionTarget::SavedView(view_id),
                            );
                        }
                    }
                    Source::ContainmentRelations => {
                        // Derive containment rows directly from node URLs so that folder/domain
                        // groups appear even when no explicit ContainmentRelation edges exist
                        // (e.g. a single file node without a corresponding parent node).
                        let mut containment_rows: Vec<(String, NodeKey, Uuid)> = self
                            .workspace
                            .domain
                            .graph
                            .nodes()
                            .filter_map(|(key, node)| {
                                let Ok(parsed) = url::Url::parse(node.url()) else {
                                    return None;
                                };
                                let row_prefix = match parsed.scheme() {
                                    "file" => {
                                        // Group by parent directory path.
                                        let mut parent = parsed.clone();
                                        parent.set_query(None);
                                        parent.set_fragment(None);
                                        let mut segments: Vec<String> = parent
                                            .path_segments()
                                            .map(|parts| {
                                                parts
                                                    .filter(|s| !s.is_empty())
                                                    .map(ToString::to_string)
                                                    .collect::<Vec<_>>()
                                            })
                                            .unwrap_or_default();
                                        if segments.is_empty() {
                                            return None;
                                        }
                                        segments.pop();
                                        let parent_path = if segments.is_empty() {
                                            "/".to_string()
                                        } else {
                                            format!("/{}/", segments.join("/"))
                                        };
                                        parent.set_path(&parent_path);
                                        format!("folder:{parent}")
                                    }
                                    "http" | "https" => {
                                        // Group by domain.
                                        let host = parsed.host_str()?.to_ascii_lowercase();
                                        format!("domain:{host}")
                                    }
                                    _ => return None,
                                };
                                Some((row_prefix, key, node.id))
                            })
                            .collect();
                        containment_rows.sort_by(
                            |(left_path, _, left_id), (right_path, _, right_id)| {
                                left_path
                                    .cmp(right_path)
                                    .then_with(|| left_id.cmp(right_id))
                            },
                        );
                        containment_rows
                            .dedup_by_key(|(row_prefix, key, _)| (row_prefix.clone(), *key));
                        for (row_key, key, node_id) in containment_rows {
                            row_targets.insert(
                                format!("{row_key}#{node_id}"),
                                NavigatorProjectionTarget::Node(key),
                            );
                        }
                    }
                }
            }
        }

        if let Some(root_filter) = self
            .workspace
            .graph_runtime
            .navigator_projection_state
            .root_filter
            .as_deref()
        {
            let filter = root_filter.trim();
            if !filter.is_empty() {
                row_targets.retain(|row_key, _| row_key.contains(filter));
            }
        }

        self.workspace
            .graph_runtime
            .navigator_projection_state
            .row_targets = row_targets;
        let mut valid_rows: HashSet<String> = self
            .workspace
            .graph_runtime
            .navigator_projection_state
            .row_targets
            .keys()
            .cloned()
            .collect();
        valid_rows.extend(self.navigator_structural_row_keys());
        self.workspace
            .graph_runtime
            .navigator_projection_state
            .selected_rows
            .retain(|row| valid_rows.contains(row));
        self.workspace
            .graph_runtime
            .navigator_projection_state
            .expanded_rows
            .retain(|row| valid_rows.contains(row));
        self.workspace
            .graph_runtime
            .navigator_projection_state
            .collapsed_rows
            .retain(|row| valid_rows.contains(row));
    }

    pub fn rebuild_file_tree_projection_rows(&mut self) {
        self.rebuild_navigator_projection_rows();
    }

    /// Set whether the user is actively interacting with the graph
    pub fn set_interacting(&mut self, interacting: bool) {
        if self.workspace.graph_runtime.is_interacting == interacting {
            return;
        }
        self.workspace.graph_runtime.is_interacting = interacting;

        if interacting {
            self.workspace
                .graph_runtime
                .physics_running_before_interaction =
                Some(self.workspace.graph_runtime.physics.is_running);
            self.workspace.graph_runtime.physics.is_running = false;
            self.workspace.graph_runtime.drag_release_frames_remaining = 0;
        } else if let Some(was_running) = self
            .workspace
            .graph_runtime
            .physics_running_before_interaction
            .take()
        {
            if was_running {
                self.workspace.graph_runtime.physics.is_running = true;
                self.workspace.graph_runtime.drag_release_frames_remaining = 0;
            } else if self.camera_position_fit_locked() {
                self.workspace.graph_runtime.physics.is_running = false;
                self.workspace.graph_runtime.drag_release_frames_remaining = 0;
            } else {
                self.workspace.graph_runtime.physics.is_running = true;
                self.workspace.graph_runtime.drag_release_frames_remaining = 10;
            }
        }
    }

    /// Advance frame-local physics housekeeping.
    /// Handles short post-drag inertia decay when simulation was previously paused.
    pub fn tick_frame(&mut self) {
        #[cfg(feature = "tracing")]
        let _tick_span = tracing::trace_span!(
            "graph.tick_frame",
            drag_release_frames_remaining =
                self.workspace.graph_runtime.drag_release_frames_remaining,
            is_interacting = self.workspace.graph_runtime.is_interacting,
            physics_running = self.workspace.graph_runtime.physics.is_running,
        )
        .entered();

        if self.workspace.graph_runtime.drag_release_frames_remaining == 0
            || self.workspace.graph_runtime.is_interacting
        {
            return;
        }
        self.workspace.graph_runtime.drag_release_frames_remaining -= 1;
        if self.workspace.graph_runtime.drag_release_frames_remaining == 0 {
            self.workspace.graph_runtime.physics.is_running = false;
        }
    }

    /// Apply a batch of reducer intents deterministically in insertion order.
    pub fn apply_reducer_intents<I, T>(&mut self, intents: I)
    where
        I: IntoIterator<Item = T>,
        T: Into<GraphIntent>,
    {
        for intent in intents {
            self.apply_reducer_intent_internal(intent.into(), true);
        }
    }

    pub fn apply_view_actions<I>(&mut self, actions: I)
    where
        I: IntoIterator<Item = ViewAction>,
    {
        self.apply_reducer_intents(actions);
    }

    pub fn apply_graph_mutations<I>(&mut self, mutations: I)
    where
        I: IntoIterator<Item = GraphMutation>,
    {
        self.apply_reducer_intents(mutations);
    }

    pub fn apply_runtime_events<I>(&mut self, events: I)
    where
        I: IntoIterator<Item = RuntimeEvent>,
    {
        self.apply_reducer_intents(events);
    }

    /// Apply a batch of reducer intents with reducer-owned undo-boundary context.
    pub fn apply_reducer_intents_with_context<I, T>(
        &mut self,
        intents: I,
        ctx: ReducerDispatchContext,
    ) where
        I: IntoIterator<Item = T>,
        T: Into<GraphIntent>,
    {
        let intents: Vec<GraphIntent> = intents.into_iter().map(Into::into).collect();
        if intents.is_empty() {
            return;
        }

        #[cfg(feature = "tracing")]
        let apply_started = Instant::now();

        #[cfg(feature = "tracing")]
        let _apply_span = tracing::trace_span!(
            "graph.apply_reducer_intents_with_context",
            intent_count = intents.len(),
            force_undo_boundary = ctx.force_undo_boundary,
            undo_reason = ?ctx.undo_boundary_reason,
        )
        .entered();

        let should_capture = ctx.force_undo_boundary
            || intents
                .iter()
                .any(|intent| self.should_capture_undo_checkpoint_for_intent(intent));

        if should_capture {
            let layout_before = ctx
                .workspace_layout_before
                .or_else(|| self.current_undo_checkpoint_layout_json());
            self.capture_undo_checkpoint_internal(layout_before, ctx.undo_boundary_reason);
        }

        for intent in intents {
            self.apply_reducer_intent_internal(intent, false);
        }

        #[cfg(feature = "tracing")]
        tracing::trace!(
            target: "graphshell::perf",
            elapsed_us = apply_started.elapsed().as_micros() as u64,
            "graph.apply_reducer_intents_with_context.complete"
        );
    }

    #[cfg(test)]
    pub(crate) fn apply_intents<I, T>(&mut self, intents: I)
    where
        I: IntoIterator<Item = T>,
        T: Into<GraphIntent>,
    {
        self.apply_reducer_intents(intents);
    }

    fn apply_view_action(&mut self, action: ViewAction) -> bool {
        match action {
            ViewAction::ToggleCameraPositionFitLock => {
                self.set_camera_position_fit_locked(!self.camera_position_fit_locked());
                true
            }
            ViewAction::ToggleCameraZoomFitLock => {
                self.set_camera_zoom_fit_locked(!self.camera_zoom_fit_locked());
                true
            }
            ViewAction::RequestFitToScreen => {
                self.request_fit_to_screen();
                true
            }
            ViewAction::RequestZoomIn => {
                if self.camera_zoom_fit_locked() {
                    self.request_fit_to_screen();
                } else {
                    self.queue_keyboard_zoom_request(KeyboardZoomRequest::In);
                }
                true
            }
            ViewAction::RequestZoomOut => {
                if self.camera_zoom_fit_locked() {
                    self.request_fit_to_screen();
                } else {
                    self.queue_keyboard_zoom_request(KeyboardZoomRequest::Out);
                }
                true
            }
            ViewAction::RequestZoomReset => {
                if self.camera_zoom_fit_locked() {
                    self.request_fit_to_screen();
                } else {
                    self.queue_keyboard_zoom_request(KeyboardZoomRequest::Reset);
                }
                true
            }
            ViewAction::RequestZoomToSelected => {
                if self.camera_position_fit_locked()
                    || self.camera_zoom_fit_locked()
                    || self.focused_selection().len() < 2
                {
                    self.request_fit_to_screen();
                } else {
                    self.request_camera_command(CameraCommand::FitSelection);
                }
                true
            }
            ViewAction::RequestZoomToGraphlet => {
                if self.camera_position_fit_locked() || self.camera_zoom_fit_locked() {
                    self.request_fit_to_screen();
                } else {
                    self.request_camera_command(CameraCommand::FitGraphlet);
                }
                true
            }
            ViewAction::ReheatPhysics => {
                self.workspace.graph_runtime.physics.is_running = true;
                self.workspace.graph_runtime.drag_release_frames_remaining = 0;
                true
            }
            ViewAction::UpdateSelection { keys, mode } => {
                self.update_focused_selection(keys, mode);
                true
            }
            ViewAction::SelectAll => {
                let all_keys: Vec<NodeKey> = self
                    .workspace
                    .domain
                    .graph
                    .nodes()
                    .map(|(k, _)| k)
                    .collect();
                self.update_focused_selection(all_keys, SelectionUpdateMode::Replace);
                true
            }
            ViewAction::SetNodePosition { key, position } => {
                let _ = self.workspace.domain.graph.set_node_position(key, position);
                true
            }
            ViewAction::SetZoom { zoom } => {
                if !self.camera_zoom_fit_locked() {
                    if let Some(focused_view) = self.workspace.graph_runtime.focused_view
                        && let Some(view) =
                            self.workspace.graph_runtime.views.get_mut(&focused_view)
                    {
                        view.camera.current_zoom = view.camera.clamp(zoom);
                    }
                }
                true
            }
            ViewAction::SetHighlightedEdge { from, to } => {
                let previous = self.workspace.graph_runtime.highlighted_graph_edge;
                self.workspace.graph_runtime.highlighted_graph_edge = Some((from, to));
                if self.workspace.graph_runtime.highlighted_graph_edge != previous {
                    emit_event(DiagnosticEvent::MessageReceived {
                        channel_id: CHANNEL_UX_NAVIGATION_TRANSITION,
                        latency_us: 0,
                    });
                }
                true
            }
            ViewAction::ClearHighlightedEdge => {
                let had_highlighted_edge = self
                    .workspace
                    .graph_runtime
                    .highlighted_graph_edge
                    .is_some();
                self.workspace.graph_runtime.highlighted_graph_edge = None;
                if had_highlighted_edge {
                    emit_event(DiagnosticEvent::MessageReceived {
                        channel_id: CHANNEL_UX_NAVIGATION_TRANSITION,
                        latency_us: 0,
                    });
                }
                true
            }
            ViewAction::SetSelectedFrame { frame_name } => {
                let previous = self.workspace.graph_runtime.selected_frame_name.clone();
                self.workspace.graph_runtime.selected_frame_name = frame_name;
                if self.workspace.graph_runtime.selected_frame_name != previous {
                    emit_event(DiagnosticEvent::MessageReceived {
                        channel_id: CHANNEL_UX_NAVIGATION_TRANSITION,
                        latency_us: 0,
                    });
                }
                true
            }
            ViewAction::SetNodeFormDraft { key, form_draft } => {
                if !self.workspace.chrome_ui.form_draft_capture_enabled {
                    return true;
                }
                let _ = self
                    .workspace
                    .domain
                    .graph
                    .set_node_form_draft(key, form_draft);
                true
            }
            ViewAction::SetNodeThumbnail {
                key,
                png_bytes,
                width,
                height,
            } => {
                let GraphDeltaResult::NodeMetadataUpdated(_updated) = self
                    .apply_graph_delta_and_sync(GraphDelta::SetNodeThumbnail {
                        key,
                        png_bytes,
                        width,
                        height,
                    })
                else {
                    unreachable!("thumbnail delta must return NodeMetadataUpdated");
                };
                true
            }
            ViewAction::SetNodeFavicon {
                key,
                rgba,
                width,
                height,
            } => {
                let GraphDeltaResult::NodeMetadataUpdated(_updated) = self
                    .apply_graph_delta_and_sync(GraphDelta::SetNodeFavicon {
                        key,
                        rgba,
                        width,
                        height,
                    })
                else {
                    unreachable!("favicon delta must return NodeMetadataUpdated");
                };
                true
            }
            ViewAction::SetWorkbenchEdgeProjection { selectors } => {
                self.set_workbench_edge_projection(selectors);
                true
            }
            ViewAction::SetViewEdgeProjectionOverride { view_id, selectors } => {
                self.set_graph_view_edge_projection_override(view_id, selectors);
                true
            }
            ViewAction::SetSelectionEdgeProjectionOverride { view_id, selectors } => {
                self.set_selection_edge_projection_override(view_id, selectors);
                true
            }
            ViewAction::SetNavigatorProjectionSeedSource { source } => {
                if self
                    .workspace
                    .graph_runtime
                    .navigator_projection_state
                    .projection_seed_source
                    != source
                {
                    self.set_navigator_projection_seed_source(source);
                    emit_event(DiagnosticEvent::MessageReceived {
                        channel_id: CHANNEL_UX_NAVIGATION_TRANSITION,
                        latency_us: 0,
                    });
                }
                true
            }
            ViewAction::SetNavigatorProjectionMode { mode } => {
                if self.workspace.graph_runtime.navigator_projection_state.mode != mode {
                    self.set_navigator_projection_mode(mode);
                    emit_event(DiagnosticEvent::MessageReceived {
                        channel_id: CHANNEL_UX_NAVIGATION_TRANSITION,
                        latency_us: 0,
                    });
                }
                true
            }
            ViewAction::SetNavigatorSortMode { sort_mode } => {
                self.set_navigator_sort_mode(sort_mode);
                true
            }
            ViewAction::SetNavigatorRootFilter { root_filter } => {
                self.set_navigator_root_filter(root_filter);
                true
            }
            ViewAction::SetNavigatorSelectedRows { rows } => {
                let next_rows: HashSet<String> = rows.iter().cloned().collect();
                if self
                    .workspace
                    .graph_runtime
                    .navigator_projection_state
                    .selected_rows
                    != next_rows
                {
                    self.set_navigator_selected_rows(rows);
                    emit_event(DiagnosticEvent::MessageReceived {
                        channel_id: CHANNEL_UX_NAVIGATION_TRANSITION,
                        latency_us: 0,
                    });
                }
                true
            }
            ViewAction::SetNavigatorExpandedRows { rows } => {
                self.set_navigator_expanded_rows(rows);
                true
            }
            ViewAction::RebuildNavigatorProjection => {
                self.rebuild_navigator_projection_rows();
                true
            }
        }
    }

    fn apply_workspace_only_intent(&mut self, intent: &GraphIntent) -> bool {
        let Some(action) = intent.as_view_action() else {
            return false;
        };
        self.apply_view_action(action)
    }

    fn apply_reducer_intent_internal(&mut self, intent: GraphIntent, allow_undo_capture: bool) {
        if self.workspace.graph_runtime.history_preview_mode_active
            && Self::intent_blocked_during_history_preview(&intent)
        {
            self.apply_reducer_intents([GraphIntent::HistoryTimelinePreviewIsolationViolation {
                detail: format!("blocked intent during preview: {:?}", intent),
            }]);
            return;
        }

        if let Some(bridge_name) = intent.workbench_authority_bridge_name() {
            log::warn!(
                "workbench-authority bridge intent reached apply_reducer_intents(): {bridge_name}; forwarding to pending workbench intents"
            );
        }

        if allow_undo_capture && self.should_capture_undo_checkpoint_for_intent(&intent) {
            self.capture_undo_checkpoint_internal(
                self.current_undo_checkpoint_layout_json(),
                UndoBoundaryReason::ReducerIntents,
            );
        }

        if matches!(
            intent,
            GraphIntent::CreateNodeNearCenter
                | GraphIntent::CreateNodeNearCenterAndOpen { .. }
                | GraphIntent::CreateNodeAtUrl { .. }
                | GraphIntent::CreateNodeAtUrlAndOpen { .. }
                | GraphIntent::AcceptHostOpenRequest { .. }
                | GraphIntent::RemoveSelectedNodes
                | GraphIntent::MarkTombstoneForSelected
                | GraphIntent::RestoreGhostNode { .. }
                | GraphIntent::ClearGraph
                | GraphIntent::CreateUserGroupedEdge { .. }
                | GraphIntent::DeleteImportRecord { .. }
                | GraphIntent::SuppressImportRecordMembership { .. }
                | GraphIntent::PromoteImportRecordToUserGroup { .. }
                | GraphIntent::CreateUserGroupedEdgeFromPrimarySelection
                | GraphIntent::RemoveEdge { .. }
                | GraphIntent::SetNodePinned { .. }
                | GraphIntent::SetNodeUrl { .. }
                | GraphIntent::ExecuteEdgeCommand { .. }
                | GraphIntent::TagNode { .. }
                | GraphIntent::UntagNode { .. }
                | GraphIntent::AssignClassification { .. }
                | GraphIntent::UnassignClassification { .. }
                | GraphIntent::AcceptClassification { .. }
                | GraphIntent::RejectClassification { .. }
                | GraphIntent::SetPrimaryClassification { .. }
                | GraphIntent::UpdateNodeMimeHint { .. }
                | GraphIntent::UpdateNodeViewerOverride { .. }
                | GraphIntent::RecordFrameLayoutHint { .. }
                | GraphIntent::RemoveFrameLayoutHint { .. }
                | GraphIntent::MoveFrameLayoutHint { .. }
                | GraphIntent::SetFrameSplitOfferSuppressed { .. }
        ) {
            // Any graph mutation starts a fresh unsaved-change episode for
            // workspace-switch prompt gating.
            self.workspace
                .workbench_session
                .unsaved_workspace_prompt_warned = false;
            self.workspace
                .workbench_session
                .workspace_has_unsaved_changes = true;
        }

        if self.handle_workspace_view_intent(&intent) {
            return;
        }
        if self.handle_workbench_bridge_intent(&intent) {
            return;
        }
        if self.handle_runtime_lifecycle_intent(intent.clone()) {
            return;
        }
        self.handle_domain_graph_intent(intent);
    }

    /// Toggle force-directed layout simulation.
    pub fn toggle_physics(&mut self) {
        if self.workspace.graph_runtime.is_interacting {
            let next = !self
                .workspace
                .graph_runtime
                .physics_running_before_interaction
                .unwrap_or(self.workspace.graph_runtime.physics.is_running);
            self.workspace
                .graph_runtime
                .physics_running_before_interaction = Some(next);
            self.workspace.graph_runtime.drag_release_frames_remaining = 0;
            return;
        }
        self.workspace.graph_runtime.physics.is_running =
            !self.workspace.graph_runtime.physics.is_running;
        self.workspace.graph_runtime.drag_release_frames_remaining = 0;
    }

    /// Update force-directed layout configuration.
    pub fn update_physics_config(&mut self, config: GraphPhysicsState) {
        self.workspace.graph_runtime.physics = config;
    }

    fn apply_physics_profile(&mut self, profile_id: &str, profile: &PhysicsProfile) {
        let was_running = self.workspace.graph_runtime.physics.is_running;
        let mut config = Self::default_physics_state();
        profile.apply_to_state(&mut config);
        config.is_running = was_running || !self.workspace.graph_runtime.is_interacting;
        config.last_avg_displacement = None;
        config.step_count = 0;
        self.workspace.graph_runtime.physics = config;
        self.workspace.graph_runtime.drag_release_frames_remaining = 0;

        if self.workspace.graph_runtime.is_interacting {
            self.workspace
                .graph_runtime
                .physics_running_before_interaction = Some(true);
        } else {
            self.workspace.graph_runtime.physics.is_running = true;
        }

        for view in self.workspace.graph_runtime.views.values_mut() {
            view.apply_physics_policy(
                profile_id,
                profile.clone(),
                crate::app::graph_views::PolicyValueSource::WorkspaceDefault,
            );
        }
    }

    pub fn graph_view_layout_manager_active(&self) -> bool {
        self.workspace
            .graph_runtime
            .graph_view_layout_manager
            .active
    }

    #[cfg(test)]
    pub fn set_import_records_for_tests(
        &mut self,
        import_records: Vec<crate::graph::ImportRecord>,
    ) -> bool {
        self.workspace
            .domain
            .graph
            .set_import_records(import_records)
    }

    #[cfg(test)]
    pub fn graph_view_slots_for_tests(&self) -> Vec<GraphViewSlot> {
        self.workspace
            .graph_runtime
            .graph_view_layout_manager
            .slots
            .values()
            .cloned()
            .collect()
    }

    #[cfg(test)]
    fn set_form_draft_capture_enabled_for_testing(&mut self, enabled: bool) {
        self.workspace.chrome_ui.form_draft_capture_enabled = enabled;
    }

    /// Scan graph for existing `about:blank#N` placeholder URLs and return
    /// the next available ID (max found + 1, or 0 if none exist).
    fn scan_max_placeholder_id(graph: &Graph) -> u32 {
        let mut max_id = 0u32;
        for (_, node) in graph.nodes() {
            if let Some(fragment) = node.url().strip_prefix("about:blank#") {
                if let Ok(id) = fragment.parse::<u32>() {
                    max_id = max_id.max(id + 1);
                }
            }
        }
        max_id
    }

    /// Generate a unique placeholder URL for a new node.
    fn next_placeholder_url(&mut self) -> String {
        let url = format!("about:blank#{}", self.workspace.domain.next_placeholder_id);
        self.workspace.domain.next_placeholder_id += 1;
        url
    }
}

fn parse_diagnostics_channel_config(raw: &str) -> Option<ChannelConfig> {
    let mut parts = raw.split('|');
    let enabled_raw = parts.next()?.trim();
    let sample_rate_raw = parts.next()?.trim();
    let retention_raw = parts.next()?.trim();

    let enabled = matches!(enabled_raw, "1" | "true" | "TRUE" | "True");
    let sample_rate = sample_rate_raw.parse::<f32>().ok()?.clamp(0.0, 1.0);
    let retention_count = retention_raw.parse::<usize>().ok()?.max(1);

    Some(ChannelConfig {
        enabled,
        sample_rate,
        retention_count,
    })
}

impl Default for GraphBrowserApp {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "graph_app_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "app/sanctioned_writes_tests.rs"]
mod sanctioned_writes_tests;
