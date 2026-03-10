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
use crate::graph::egui_adapter::EguiGraphState;
use crate::graph::{EdgeKind, EdgeType, Graph, NavigationTrigger, NodeKey, Traversal};
use crate::registries::atomic::diagnostics::ChannelConfig;
use crate::registries::atomic::knowledge::SemanticClassVector;
use crate::registries::atomic::lens::{
    LayoutMode, PhysicsProfile, ThemeData, deserialize_optional_theme_data,
};
use crate::registries::domain::layout::canvas::CanvasLassoBinding;
use crate::services::persistence::types::{
    LogEntry, PersistedEdgeType, PersistedNavigationTrigger,
};
use crate::services::persistence::{GraphStore, TimelineIndexEntry};
use crate::shell::desktop::runtime::caches::{CachePolicy, RuntimeCaches};
use crate::shell::desktop::runtime::diagnostics::{DiagnosticEvent, emit_event};
use crate::shell::desktop::runtime::registries::input::{
    InputBindingRemap, InputConflict as InputRemapConflict,
};
use crate::shell::desktop::runtime::registries::{
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
    phase2_reset_input_binding_remaps,
};
#[cfg(not(test))]
use crate::shell::desktop::runtime::registries::{
    CHANNEL_STARTUP_PERSISTENCE_OPEN_STARTED, CHANNEL_STARTUP_PERSISTENCE_OPEN_SUCCEEDED,
    CHANNEL_STARTUP_PERSISTENCE_OPEN_TIMEOUT,
};
use crate::util::{
    GraphAddress, GraphshellSettingsPath, NodeAddress, NoteAddress, VersoAddress, VersoViewTarget,
};
use egui_graphs::FruchtermanReingoldWithCenterGravityState;
use euclid::default::Point2D;
use log::{debug, warn};
// Platform-agnostic renderer handle.
// On desktop this aliases servo::WebViewId so existing callers in the
// desktop module work without any conversion.
// On iOS, Servo is not a dependency, so a standalone opaque type is used.
#[cfg(not(target_os = "ios"))]
use servo::WebViewId;
/// Opaque handle for a renderer instance (webview, PDF viewer, etc.).
/// On desktop: identical to `servo::WebViewId` (type alias, zero cost).
/// On iOS: an opaque counter assigned by the iOS renderer layer.
#[cfg(not(target_os = "ios"))]
pub type RendererId = WebViewId;
#[cfg(target_os = "ios")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RendererId(u64);
use uuid::Uuid;

/// Camera state for zoom bounds enforcement
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Camera {
    pub zoom_min: f32,
    pub zoom_max: f32,
    pub current_zoom: f32,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct GraphViewFrame {
    pub zoom: f32,
    pub pan_x: f32,
    pub pan_y: f32,
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

/// Unique identifier for a graph view pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct GraphViewId(Uuid);

impl GraphViewId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn as_uuid(self) -> Uuid {
        self.0
    }
}

impl Default for GraphViewId {
    fn default() -> Self {
        Self::new()
    }
}

/// Durable identifier for a rich note document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct NoteId(Uuid);

impl NoteId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn as_uuid(self) -> Uuid {
        self.0
    }
}

impl Default for NoteId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenSurfaceSource {
    KeyboardShortcut,
    ChildWebview,
    WindowBootstrap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PendingCreateToken(u64);

impl PendingCreateToken {
    pub fn new(raw: u64) -> Self {
        Self(raw)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostOpenRequest {
    pub url: String,
    pub source: OpenSurfaceSource,
    pub parent_webview_id: Option<RendererId>,
    pub pending_create_token: Option<PendingCreateToken>,
}

#[path = "app/selection.rs"]
mod selection;
pub use selection::{ClipboardCopyKind, ClipboardCopyRequest, SelectionState, SelectionUpdateMode};
pub(crate) use selection::{SelectionScope, UndoRedoSnapshot};

#[path = "app/history.rs"]
mod history;
pub use history::{
    HistoryCaptureStatus, HistoryHealthSummary, HistoryManagerTab, HistoryTraversalFailureReason,
};

#[path = "app/history_runtime.rs"]
mod history_runtime;

#[path = "app/intents.rs"]
mod intents;
pub use intents::{
    AppCommand, BrowserCommand, BrowserCommandTarget, GraphIntent, GraphMutation, RuntimeEvent,
    ViewAction,
};

#[path = "app/workspace_commands.rs"]
mod workspace_commands;

#[path = "app/workspace_routing.rs"]
mod workspace_routing;

#[path = "app/workbench_commands.rs"]
mod workbench_commands;

#[path = "app/focus_selection.rs"]
mod focus_selection;

#[path = "app/graph_views.rs"]
mod graph_views;

#[path = "app/runtime_lifecycle.rs"]
mod runtime_lifecycle;

#[path = "app/graph_mutations.rs"]
mod graph_mutations;

#[path = "app/ux_navigation.rs"]
mod ux_navigation;

#[path = "app/startup_persistence.rs"]
mod startup_persistence;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SettingsToolPage {
    #[default]
    General,
    Persistence,
    Physics,
    Sync,
    Appearance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsRouteTarget {
    History,
    Settings(SettingsToolPage),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolSurfaceReturnTarget {
    Graph(GraphViewId),
    Node(NodeKey),
    Tool(crate::shell::desktop::workbench::pane_model::ToolPaneState),
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
    #[serde(default, deserialize_with = "deserialize_optional_theme_data")]
    pub theme: Option<ThemeData>,
    pub filters: Vec<String>,
}

impl Default for LensConfig {
    fn default() -> Self {
        Self {
            name: "Default".to_string(),
            lens_id: None,
            physics: PhysicsProfile::default(),
            layout: LayoutMode::Free,
            theme: None,
            filters: Vec::new(),
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
    pub egui_state: Option<EguiGraphState>,
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
            egui_state: None,
        }
    }
}

impl GraphViewState {
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
            egui_state: None,
        }
    }

    pub fn new(name: impl Into<String>) -> Self {
        Self::new_with_id(GraphViewId::new(), name)
    }
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
struct PersistedGraphViewLayoutManager {
    version: u32,
    active: bool,
    slots: Vec<GraphViewSlot>,
}

impl PersistedGraphViewLayoutManager {
    const VERSION: u32 = 1;
}

#[derive(Debug, Clone)]
pub struct NoteRecord {
    pub id: NoteId,
    pub title: String,
    pub linked_node: Option<NodeKey>,
    pub source_url: Option<String>,
    pub body: String,
    pub created_at: SystemTime,
    pub updated_at: SystemTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewRouteTarget {
    GraphPane(GraphViewId),
    Graph(String),
    Note(NoteId),
    Node(Uuid),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FileTreeContainmentRelationSource {
    #[default]
    GraphContainment,
    SavedViewCollections,
    ImportedFilesystemProjection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FileTreeSortMode {
    #[default]
    Manual,
    NameAscending,
    NameDescending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileTreeProjectionTarget {
    Node(NodeKey),
    SavedView(GraphViewId),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FileTreeProjectionState {
    pub containment_relation_source: FileTreeContainmentRelationSource,
    pub expanded_rows: HashSet<String>,
    pub collapsed_rows: HashSet<String>,
    pub selected_rows: HashSet<String>,
    pub sort_mode: FileTreeSortMode,
    pub root_filter: Option<String>,
    pub row_targets: HashMap<String, FileTreeProjectionTarget>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeBlockReason {
    CreateRetryExhausted,
    Crash,
}

#[derive(Debug, Clone)]
pub struct RuntimeBlockState {
    pub reason: RuntimeBlockReason,
    pub retry_at: Option<Instant>,
    pub message: Option<String>,
    pub has_backtrace: bool,
    pub blocked_at: SystemTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchDisplayMode {
    Highlight,
    Filter,
}

/// Deterministic mutation intent boundary for graph state updates.
#[derive(Debug, Clone)]
pub enum EdgeCommand {
    ConnectSelectedPair,
    ConnectPair { from: NodeKey, to: NodeKey },
    ConnectBothDirections,
    ConnectBothDirectionsPair { a: NodeKey, b: NodeKey },
    RemoveUserEdge,
    RemoveUserEdgePair { a: NodeKey, b: NodeKey },
    PinSelected,
    UnpinSelected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingTileOpenMode {
    Tab,
    SplitHorizontal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PendingNodeOpenRequest {
    pub key: NodeKey,
    pub mode: PendingTileOpenMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingConnectedOpenScope {
    Neighbors,
    Connected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyboardZoomRequest {
    In,
    Out,
    Reset,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CameraCommand {
    Fit,
    FitSelection,
    SetZoom(f32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryPressureLevel {
    Unknown,
    Normal,
    Warning,
    Critical,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleCause {
    UserSelect,
    ActiveTileVisible,
    SelectedPrewarm,
    WorkspaceRetention,
    ActiveLruEviction,
    WarmLruEviction,
    MemoryPressureWarning,
    MemoryPressureCritical,
    Crash,
    CreateRetryExhausted,
    ExplicitClose,
    NodeRemoval,
    Restore,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameOpenAction {
    RestoreFrame { name: String, node: NodeKey },
    OpenInCurrentFrame { node: NodeKey },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameOpenReason {
    MissingNode,
    PreferredFrame,
    RecentMembership,
    DeterministicMembershipFallback,
    NoMembership,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnsavedFramePromptRequest {
    FrameSwitch {
        name: String,
        focus_node: Option<NodeKey>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnsavedFramePromptAction {
    ProceedWithoutSaving,
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChooseFramePickerMode {
    OpenNodeInFrame,
    AddNodeToFrame,
    AddConnectedSelectionToFrame,
    AddExactSelectionToFrame,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChooseFramePickerRequest {
    pub node: NodeKey,
    pub mode: ChooseFramePickerMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastAnchorPreference {
    TopRight,
    TopLeft,
    BottomRight,
    BottomLeft,
}

impl_display_from_str!(ToastAnchorPreference {
    ToastAnchorPreference::TopRight => "top-right",
    ToastAnchorPreference::TopLeft => "top-left",
    ToastAnchorPreference::BottomRight => "bottom-right",
    ToastAnchorPreference::BottomLeft => "bottom-left",
});

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandPaletteShortcut {
    F2,
    CtrlK,
}

impl_display_from_str!(CommandPaletteShortcut {
    CommandPaletteShortcut::F2 => "f2",
    CommandPaletteShortcut::CtrlK => "ctrl-k",
});

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelpPanelShortcut {
    F1OrQuestion,
    H,
}

impl_display_from_str!(HelpPanelShortcut {
    HelpPanelShortcut::F1OrQuestion => "f1-or-question",
    HelpPanelShortcut::H => "h",
});

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadialMenuShortcut {
    F3,
    R,
}

impl_display_from_str!(RadialMenuShortcut {
    RadialMenuShortcut::F3 => "f3",
    RadialMenuShortcut::R => "r",
});

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyboardPanInputMode {
    WasdAndArrows,
    ArrowsOnly,
}

impl_display_from_str!(KeyboardPanInputMode {
    KeyboardPanInputMode::WasdAndArrows => "wasd-and-arrows",
    KeyboardPanInputMode::ArrowsOnly => "arrows-only",
});

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OmnibarPreferredScope {
    Auto,
    LocalTabs,
    ConnectedNodes,
    ProviderDefault,
    GlobalNodes,
    GlobalTabs,
}

impl_display_from_str!(OmnibarPreferredScope {
    OmnibarPreferredScope::Auto => "auto",
    OmnibarPreferredScope::LocalTabs => "local-tabs",
    OmnibarPreferredScope::ConnectedNodes => "connected-nodes",
    OmnibarPreferredScope::ProviderDefault => "provider-default",
    OmnibarPreferredScope::GlobalNodes => "global-nodes",
    OmnibarPreferredScope::GlobalTabs => "global-tabs",
});

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OmnibarNonAtOrderPreset {
    ContextualThenProviderThenGlobal,
    ProviderThenContextualThenGlobal,
}

impl_display_from_str!(OmnibarNonAtOrderPreset {
    OmnibarNonAtOrderPreset::ContextualThenProviderThenGlobal => "contextual-provider-global",
    OmnibarNonAtOrderPreset::ProviderThenContextualThenGlobal => "provider-contextual-global",
});

#[derive(Debug, Clone)]
pub enum WorkbenchIntent {
    OpenCommandPalette,
    ToggleCommandPalette,
    CycleFocusRegion,
    OpenToolPane {
        kind: crate::shell::desktop::workbench::pane_model::ToolPaneState,
    },
    ClosePane {
        pane: crate::shell::desktop::workbench::pane_model::PaneId,
        restore_previous_focus: bool,
    },
    CloseToolPane {
        kind: crate::shell::desktop::workbench::pane_model::ToolPaneState,
        restore_previous_focus: bool,
    },
    OpenSettingsUrl {
        url: String,
    },
    OpenFrameUrl {
        url: String,
    },
    OpenToolUrl {
        url: String,
    },
    OpenViewUrl {
        url: String,
    },
    OpenGraphUrl {
        url: String,
    },
    OpenNodeUrl {
        url: String,
    },
    OpenClipUrl {
        url: String,
    },
    OpenGraphViewPane {
        view_id: GraphViewId,
        mode: PendingTileOpenMode,
    },
    OpenNoteUrl {
        url: String,
    },
    OpenNodeInPane {
        node: NodeKey,
        pane: crate::shell::desktop::workbench::pane_model::PaneId,
    },
    SwapViewerBackend {
        pane: crate::shell::desktop::workbench::pane_model::PaneId,
        node: NodeKey,
        viewer_id_override: Option<crate::shell::desktop::workbench::pane_model::ViewerId>,
    },
    SetPaneView {
        pane: crate::shell::desktop::workbench::pane_model::PaneId,
        view: crate::shell::desktop::workbench::pane_model::PaneViewState,
    },
    SplitPane {
        source_pane: crate::shell::desktop::workbench::pane_model::PaneId,
        direction: crate::shell::desktop::workbench::pane_model::SplitDirection,
    },
    DetachNodeToSplit {
        key: NodeKey,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UndoBoundaryReason {
    #[default]
    ReducerIntents,
    OpenNodePane,
    OpenConnectedNodes,
    RestoreFrameSnapshot,
    DetachNodeToSplit,
    RestoreGraphSnapshot,
}

#[derive(Debug, Clone, Default)]
pub struct ReducerDispatchContext {
    pub workspace_layout_before: Option<String>,
    pub force_undo_boundary: bool,
    pub undo_boundary_reason: UndoBoundaryReason,
}

#[derive(Default)]
pub struct AppServices {
    persistence: Option<GraphStore>,
    sync_command_tx: Option<tokio_mpsc::Sender<crate::mods::native::verse::SyncCommand>>,
}

impl AppServices {
    fn new(persistence: Option<GraphStore>) -> Self {
        Self {
            persistence,
            sync_command_tx: None,
        }
    }
}

/// Pure, serializable workspace data.
pub struct GraphWorkspace {
    /// Durable domain state remains nested here during the first CLAT extraction.
    pub domain: DomainState,

    /// Force-directed layout state owned by app/runtime UI controls.
    pub physics: FruchtermanReingoldWithCenterGravityState,

    /// Physics running state before user drag/pan interaction began.
    physics_running_before_interaction: Option<bool>,

    /// Canonical selection state keyed by runtime selection scope.
    ///
    /// `SelectionScope::View` stores per-graph-view selection, while
    /// `SelectionScope::Unfocused` carries the selection visible when no graph
    /// view is focused.
    selection_by_scope: HashMap<SelectionScope, SelectionState>,

    /// Bidirectional mapping between renderer instances and graph nodes
    webview_to_node: HashMap<RendererId, NodeKey>,
    node_to_webview: HashMap<NodeKey, RendererId>,
    /// Runtime-only block/backoff metadata keyed by graph node.
    runtime_block_state: HashMap<NodeKey, RuntimeBlockState>,
    /// Non-authoritative runtime caches for data-plane acceleration.
    pub(crate) runtime_caches: RuntimeCaches,

    /// Nodes that had webviews before switching to graph view (for restoration).
    /// Managed by the webview_controller module.
    pub(crate) active_webview_nodes: Vec<NodeKey>,

    /// Active mapped nodes in LRU order (oldest at index 0, newest at end).
    active_lru: Vec<NodeKey>,

    /// Maximum number of active mapped webviews to retain.
    active_webview_limit: usize,

    /// Warm-cached nodes in LRU order (oldest at index 0, newest at end).
    warm_cache_lru: Vec<NodeKey>,

    /// Maximum number of warm-cached webviews to retain.
    warm_cache_limit: usize,

    /// True while the user is actively interacting (drag/pan) with the graph
    pub(crate) is_interacting: bool,

    /// Short post-drag decay window to preserve "weight" when physics was paused.
    drag_release_frames_remaining: u8,

    /// Active tab in the History Manager panel.
    pub history_manager_tab: HistoryManagerTab,
    /// Active page in the Settings tool pane.
    pub settings_tool_page: SettingsToolPage,

    /// Whether the keyboard shortcut help panel is open
    pub show_help_panel: bool,

    /// Whether the command palette is open
    pub show_command_palette: bool,
    /// Whether the radial command UI is open.
    pub show_radial_menu: bool,

    /// Preferred toast anchor location.
    pub toast_anchor_preference: ToastAnchorPreference,
    /// Shortcut binding for command palette.
    pub command_palette_shortcut: CommandPaletteShortcut,
    /// Shortcut binding for help panel.
    pub help_panel_shortcut: HelpPanelShortcut,
    /// Shortcut binding for radial menu.
    pub radial_menu_shortcut: RadialMenuShortcut,
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
    /// Independent multi-selection for workspace tabs.
    pub selected_tab_nodes: HashSet<NodeKey>,
    /// Range-select anchor for workspace tab multi-selection.
    pub tab_selection_anchor: Option<NodeKey>,
    /// Last hovered node in graph view (updated by graph render pass).
    pub hovered_graph_node: Option<NodeKey>,
    /// Graph search display mode (context-preserving highlight vs strict filter).
    pub search_display_mode: SearchDisplayMode,
    /// Graph-owned hierarchical projection runtime state for file-tree navigation.
    ///
    /// This is semantic view-projection state and must not be owned by workbench
    /// arrangement structures.
    pub file_tree_projection_state: FileTreeProjectionState,
    /// Explicit highlighted edge in graph view (for edge-search targeting).
    pub highlighted_graph_edge: Option<(NodeKey, NodeKey)>,
    /// Pending workbench-authority intents staged for frame-loop orchestration.
    pending_workbench_intents: Vec<WorkbenchIntent>,

    /// Ordered app-command queue replacing a subset of hand-managed pending snapshot fields.
    pending_app_commands: VecDeque<AppCommand>,

    /// Accepted child-webview create requests awaiting reconcile-time renderer creation.
    pending_host_create_tokens: HashMap<NodeKey, PendingCreateToken>,

    /// Active graph views, keyed by ID.
    pub views: HashMap<GraphViewId, GraphViewState>,
    /// Graph-view layout manager state (slot grid + manager overlay toggle).
    pub graph_view_layout_manager: GraphViewLayoutManagerState,

    /// Last known camera frame per graph view (updated by graph render pass).
    pub graph_view_frames: HashMap<GraphViewId, GraphViewFrame>,

    /// The currently focused graph view (target for keyboard zoom/pan).
    pub focused_view: Option<GraphViewId>,

    /// Camera state (zoom bounds)
    pub camera: Camera,

    /// Global undo history snapshots.
    undo_stack: Vec<UndoRedoSnapshot>,
    /// Global redo history snapshots.
    redo_stack: Vec<UndoRedoSnapshot>,
    /// Cached hop-distance map from current primary selection for omnibar ranking/signifiers.
    hop_distance_cache: Option<(NodeKey, HashMap<NodeKey, usize>)>,

    /// Hash of last persisted session frame layout json.
    last_session_workspace_layout_hash: Option<u64>,
    /// Last known live session frame layout JSON (runtime `Tree<TileKind>` shape) for undo checkpoints.
    last_session_workspace_layout_json: Option<String>,

    /// Minimum interval between autosaved session frame writes.
    workspace_autosave_interval: Duration,

    /// Number of previous autosaved session frame revisions to keep.
    workspace_autosave_retention: u8,

    /// Timestamp of last autosaved session frame write.
    last_workspace_autosave_at: Option<Instant>,

    /// Monotonic activation counter for named frame recency tracking.
    workspace_activation_seq: u64,

    /// Per-node most-recent named frame activation metadata keyed by stable node UUID.
    node_last_active_workspace: HashMap<Uuid, (u64, String)>,

    /// UUID-keyed frame membership index (runtime-derived from persisted layouts).
    node_workspace_membership: HashMap<Uuid, BTreeSet<String>>,

    /// True while current tile tree was synthesized without a named restore context.
    /// Retained for routing/session bookkeeping only.
    current_workspace_is_synthesized: bool,

    /// True if graph-mutating action happened since last workspace baseline/save.
    workspace_has_unsaved_changes: bool,

    /// True after we've emitted a warning for the current unsaved workspace state.
    unsaved_workspace_prompt_warned: bool,

    /// Cached egui_graphs state (persists across frames for drag/interaction)
    pub egui_state: Option<EguiGraphState>,

    /// Invariant: must only be set directly for non-structural visual changes
    /// (selection, search highlights, viewport culling). All graph structure
    /// changes must go through `apply_graph_delta_and_sync`.
    pub egui_state_dirty: bool,

    /// Node keys excluded by viewport culling on the previous rebuild.
    /// egui_state is only rebuilt when this set changes, to avoid resetting
    /// physics state every frame for nodes that stay in/out of the viewport.
    pub last_culled_node_keys: Option<HashSet<NodeKey>>,

    /// Last sampled runtime memory pressure classification.
    memory_pressure_level: MemoryPressureLevel,
    /// Last sampled available system memory (MiB).
    memory_available_mib: u64,
    /// Last sampled total system memory (MiB).
    memory_total_mib: u64,

    /// Count of traversal append attempts rejected in this runtime session.
    history_recent_traversal_append_failures: u64,
    /// True while history timeline preview mode is active.
    history_preview_mode_active: bool,
    /// True when preview-mode isolation has been violated in this session.
    history_last_preview_isolation_violation: bool,
    /// Tracks active timeline replay and cursor progression.
    history_replay_in_progress: bool,
    history_replay_cursor: Option<usize>,
    history_replay_total_steps: Option<usize>,
    /// Detached graph copy captured when preview mode is entered.
    history_preview_live_graph_snapshot: Option<Graph>,
    /// Detached graph produced by replay-to-timestamp while preview is active.
    history_preview_graph: Option<Graph>,
    /// Most recent history subsystem event timestamp observed this session.
    history_last_event_unix_ms: Option<u64>,
    /// Most recent history error text surfaced to operators.
    history_last_error: Option<String>,
    /// Last traversal/archive failure bucket label.
    history_recent_failure_reason_bucket: Option<HistoryTraversalFailureReason>,
    /// Last known return-to-present outcome summary.
    history_last_return_to_present_result: Option<String>,

    /// Whether form draft capture/replay metadata is enabled.
    form_draft_capture_enabled: bool,

    /// Persisted default registry lens id override for view lens resolution.
    default_registry_lens_id: Option<String>,
    /// Persisted default physics preset selection for graph dynamics controls.
    default_registry_physics_id: Option<String>,
    /// Persisted default theme selection for workspace appearance controls.
    default_registry_theme_id: Option<String>,

    /// Cached semantic codes for physics calculations.
    /// Maps NodeKey -> parsed canonical UDC class vector.
    pub semantic_index: HashMap<NodeKey, SemanticClassVector>,
    pub semantic_index_dirty: bool,

    /// Runtime semantic tags by node key (e.g. "udc:51").
    pub semantic_tags: HashMap<NodeKey, HashSet<String>>,
}

/// Main application state (workspace + runtime services).
pub struct GraphBrowserApp {
    pub workspace: GraphWorkspace,
    services: AppServices,
}

impl GraphBrowserApp {
    fn encode_undo_graph_bytes(graph: &Graph) -> Option<Vec<u8>> {
        rkyv::to_bytes::<rkyv::rancor::Error>(graph)
            .ok()
            .map(|bytes| bytes.as_slice().to_vec())
    }

    fn decode_undo_graph_bytes(graph_bytes: &[u8]) -> Option<Graph> {
        let mut aligned = rkyv::util::AlignedVec::<16>::new();
        aligned.extend_from_slice(graph_bytes);
        rkyv::from_bytes::<Graph, rkyv::rancor::Error>(&aligned).ok()
    }

    fn build_undo_redo_snapshot(
        &self,
        workspace_layout_json: Option<String>,
    ) -> Option<UndoRedoSnapshot> {
        let graph_bytes = Self::encode_undo_graph_bytes(&self.workspace.domain.graph)?;
        Some(UndoRedoSnapshot {
            graph_bytes,
            active_selection: self.focused_selection().clone(),
            selection_by_scope: self.workspace.selection_by_scope.clone(),
            highlighted_graph_edge: self.workspace.highlighted_graph_edge,
            workspace_layout_json,
        })
    }

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
    pub const SETTINGS_KEYBOARD_PAN_STEP_NAME: &'static str =
        "workspace:settings-keyboard-pan-step";
    pub const SETTINGS_KEYBOARD_PAN_INPUT_MODE_NAME: &'static str =
        "workspace:settings-keyboard-pan-input-mode";
    pub const SETTINGS_CAMERA_PAN_INERTIA_ENABLED_NAME: &'static str =
        "workspace:settings-camera-pan-inertia-enabled";
    pub const SETTINGS_CAMERA_PAN_INERTIA_DAMPING_NAME: &'static str =
        "workspace:settings-camera-pan-inertia-damping";
    pub const SETTINGS_LASSO_BINDING_NAME: &'static str = "workspace:settings-lasso-binding";
    pub const SETTINGS_INPUT_BINDING_REMAPS_NAME: &'static str =
        "workspace:settings-input-binding-remaps";
    pub const SETTINGS_OMNIBAR_PREFERRED_SCOPE_NAME: &'static str =
        "workspace:settings-omnibar-preferred-scope";
    pub const SETTINGS_OMNIBAR_NON_AT_ORDER_NAME: &'static str =
        "workspace:settings-omnibar-non-at-order";
    pub const SETTINGS_WRY_ENABLED_NAME: &'static str = "workspace:settings-wry-enabled";
    pub const SETTINGS_REGISTRY_LENS_ID_NAME: &'static str = "workspace:settings-registry-lens-id";
    pub const SETTINGS_REGISTRY_PHYSICS_ID_NAME: &'static str =
        "workspace:settings-registry-physics-id";
    pub const SETTINGS_REGISTRY_THEME_ID_NAME: &'static str =
        "workspace:settings-registry-theme-id";
    pub const SETTINGS_WORKBENCH_SURFACE_PROFILE_ID_NAME: &'static str =
        "workspace:settings-workbench-surface-profile-id";
    pub const SETTINGS_CANVAS_PROFILE_ID_NAME: &'static str =
        "workspace:settings-canvas-profile-id";
    pub const SETTINGS_ACTIVE_WORKFLOW_ID_NAME: &'static str =
        "workspace:settings-active-workflow-id";
    pub const SETTINGS_GRAPH_VIEW_LAYOUT_MANAGER_NAME: &'static str =
        "workspace:settings-graph-view-layout-manager";
    pub const SETTINGS_DIAGNOSTICS_CHANNEL_CONFIG_PREFIX: &'static str =
        "workspace:settings-diagnostics-channel-config:";
    pub const DEFAULT_WORKSPACE_AUTOSAVE_INTERVAL_SECS: u64 = 60;
    pub const DEFAULT_WORKSPACE_AUTOSAVE_RETENTION: u8 = 1;
    pub const DEFAULT_ACTIVE_WEBVIEW_LIMIT: usize = 4;
    pub const DEFAULT_WARM_CACHE_LIMIT: usize = 12;
    pub const DEFAULT_KEYBOARD_PAN_STEP: f32 = 12.0;
    pub const DEFAULT_CAMERA_PAN_INERTIA_ENABLED: bool = true;
    pub const DEFAULT_CAMERA_PAN_INERTIA_DAMPING: f32 = 0.84;
    pub const TAG_PIN: &'static str = "#pin";
    pub const TAG_STARRED: &'static str = "#starred";

    pub fn default_physics_state() -> FruchtermanReingoldWithCenterGravityState {
        let mut state = FruchtermanReingoldWithCenterGravityState::default();
        // Compact, less jittery default:
        // - lower repulsion and ideal distance to avoid flyaway spread
        // - higher attraction to pull distant components back together
        // - lower step magnitude for more granular, predictable motion
        state.base.c_repulse = 0.28;
        state.base.c_attract = 0.22;
        state.base.k_scale = 0.42;
        state.base.dt = 0.03;
        state.base.max_step = 3.0;
        state.base.damping = 0.55;
        // Keep the cluster attracted toward viewport center.
        state.extras.0.params.c = 0.18;
        state
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
                },
                physics: Self::default_physics_state(),
                physics_running_before_interaction: None,
                selection_by_scope: HashMap::new(),
                webview_to_node: HashMap::new(),
                node_to_webview: HashMap::new(),
                runtime_block_state: HashMap::new(),
                runtime_caches: RuntimeCaches::new(CachePolicy::default(), None),
                active_webview_nodes: Vec::new(),
                active_lru: Vec::new(),
                active_webview_limit: Self::DEFAULT_ACTIVE_WEBVIEW_LIMIT,
                warm_cache_lru: Vec::new(),
                warm_cache_limit: Self::DEFAULT_WARM_CACHE_LIMIT,
                is_interacting: false,
                drag_release_frames_remaining: 0,
                history_manager_tab: HistoryManagerTab::Timeline,
                settings_tool_page: SettingsToolPage::General,
                show_help_panel: false,
                show_command_palette: false,
                show_radial_menu: false,
                toast_anchor_preference: ToastAnchorPreference::BottomRight,
                command_palette_shortcut: CommandPaletteShortcut::F2,
                help_panel_shortcut: HelpPanelShortcut::F1OrQuestion,
                radial_menu_shortcut: RadialMenuShortcut::F3,
                keyboard_pan_step: Self::DEFAULT_KEYBOARD_PAN_STEP,
                keyboard_pan_input_mode: KeyboardPanInputMode::WasdAndArrows,
                camera_pan_inertia_enabled: Self::DEFAULT_CAMERA_PAN_INERTIA_ENABLED,
                camera_pan_inertia_damping: Self::DEFAULT_CAMERA_PAN_INERTIA_DAMPING,
                lasso_binding_preference: CanvasLassoBinding::RightDrag,
                omnibar_preferred_scope: OmnibarPreferredScope::Auto,
                omnibar_non_at_order: OmnibarNonAtOrderPreset::ContextualThenProviderThenGlobal,
                wry_enabled: false,
                selected_tab_nodes: HashSet::new(),
                tab_selection_anchor: None,
                hovered_graph_node: None,
                search_display_mode: SearchDisplayMode::Highlight,
                file_tree_projection_state: FileTreeProjectionState::default(),
                highlighted_graph_edge: None,
                pending_workbench_intents: Vec::new(),
                pending_app_commands: VecDeque::new(),
                pending_host_create_tokens: HashMap::new(),
                camera: Camera::new(),
                views: HashMap::new(),
                graph_view_layout_manager: GraphViewLayoutManagerState::default(),
                graph_view_frames: HashMap::new(),
                focused_view: None,
                undo_stack: Vec::new(),
                redo_stack: Vec::new(),
                hop_distance_cache: None,
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
                current_workspace_is_synthesized: false,
                workspace_has_unsaved_changes: false,
                unsaved_workspace_prompt_warned: false,
                egui_state: None,
                egui_state_dirty: true,
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
                form_draft_capture_enabled: std::env::var_os("GRAPHSHELL_ENABLE_FORM_DRAFT")
                    .is_some(),
                default_registry_lens_id: None,
                default_registry_physics_id: None,
                default_registry_theme_id: None,
                semantic_index: HashMap::new(),
                semantic_index_dirty: true,
                semantic_tags: HashMap::new(),
            },
            services: AppServices::new(persistence),
        };
        app.load_persisted_ui_settings();
        app
    }

    /// Create a new graph browser application without persistence (for tests)
    #[cfg(test)]
    pub fn new_for_testing() -> Self {
        Self {
            workspace: GraphWorkspace {
                domain: DomainState {
                    graph: Graph::new(),
                    next_placeholder_id: 0,
                    notes: HashMap::new(),
                },
                physics: Self::default_physics_state(),
                physics_running_before_interaction: None,
                selection_by_scope: HashMap::new(),
                webview_to_node: HashMap::new(),
                node_to_webview: HashMap::new(),
                runtime_block_state: HashMap::new(),
                runtime_caches: RuntimeCaches::new(CachePolicy::default(), None),
                active_webview_nodes: Vec::new(),
                active_lru: Vec::new(),
                active_webview_limit: Self::DEFAULT_ACTIVE_WEBVIEW_LIMIT,
                warm_cache_lru: Vec::new(),
                warm_cache_limit: Self::DEFAULT_WARM_CACHE_LIMIT,
                is_interacting: false,
                drag_release_frames_remaining: 0,
                history_manager_tab: HistoryManagerTab::Timeline,
                settings_tool_page: SettingsToolPage::General,
                show_help_panel: false,
                show_command_palette: false,
                show_radial_menu: false,
                toast_anchor_preference: ToastAnchorPreference::BottomRight,
                command_palette_shortcut: CommandPaletteShortcut::F2,
                help_panel_shortcut: HelpPanelShortcut::F1OrQuestion,
                radial_menu_shortcut: RadialMenuShortcut::F3,
                keyboard_pan_step: Self::DEFAULT_KEYBOARD_PAN_STEP,
                keyboard_pan_input_mode: KeyboardPanInputMode::WasdAndArrows,
                camera_pan_inertia_enabled: Self::DEFAULT_CAMERA_PAN_INERTIA_ENABLED,
                camera_pan_inertia_damping: Self::DEFAULT_CAMERA_PAN_INERTIA_DAMPING,
                lasso_binding_preference: CanvasLassoBinding::RightDrag,
                omnibar_preferred_scope: OmnibarPreferredScope::Auto,
                omnibar_non_at_order: OmnibarNonAtOrderPreset::ContextualThenProviderThenGlobal,
                wry_enabled: false,
                selected_tab_nodes: HashSet::new(),
                tab_selection_anchor: None,
                hovered_graph_node: None,
                search_display_mode: SearchDisplayMode::Highlight,
                file_tree_projection_state: FileTreeProjectionState::default(),
                highlighted_graph_edge: None,
                pending_workbench_intents: Vec::new(),
                pending_app_commands: VecDeque::new(),
                pending_host_create_tokens: HashMap::new(),
                camera: Camera::new(),
                views: HashMap::new(),
                graph_view_layout_manager: GraphViewLayoutManagerState::default(),
                graph_view_frames: HashMap::new(),
                focused_view: None,
                undo_stack: Vec::new(),
                redo_stack: Vec::new(),
                hop_distance_cache: None,
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
                current_workspace_is_synthesized: false,
                workspace_has_unsaved_changes: false,
                unsaved_workspace_prompt_warned: false,
                egui_state: None,
                egui_state_dirty: true,
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
                form_draft_capture_enabled: false,
                default_registry_lens_id: None,
                default_registry_physics_id: None,
                default_registry_theme_id: None,
                semantic_index: HashMap::new(),
                semantic_index_dirty: true,
                semantic_tags: HashMap::new(),
            },
            services: AppServices::new(None),
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

    pub fn set_tab_selection_single(&mut self, key: NodeKey) {
        if self.workspace.domain.graph.get_node(key).is_none() {
            return;
        }
        self.workspace.selected_tab_nodes.clear();
        self.workspace.selected_tab_nodes.insert(key);
        self.workspace.tab_selection_anchor = Some(key);
    }

    pub fn toggle_tab_selection(&mut self, key: NodeKey) {
        if self.workspace.domain.graph.get_node(key).is_none() {
            return;
        }
        if !self.workspace.selected_tab_nodes.remove(&key) {
            self.workspace.selected_tab_nodes.insert(key);
        }
        self.workspace.tab_selection_anchor = Some(key);
    }

    pub fn add_tab_selection_keys(&mut self, keys: impl IntoIterator<Item = NodeKey>) {
        let mut last = None;
        for key in keys {
            if self.workspace.domain.graph.get_node(key).is_none() {
                continue;
            }
            self.workspace.selected_tab_nodes.insert(key);
            last = Some(key);
        }
        if let Some(key) = last {
            self.workspace.tab_selection_anchor = Some(key);
        }
    }

    pub fn file_tree_projection_state(&self) -> &FileTreeProjectionState {
        &self.workspace.file_tree_projection_state
    }

    pub fn set_file_tree_containment_relation_source(
        &mut self,
        source: FileTreeContainmentRelationSource,
    ) {
        self.workspace
            .file_tree_projection_state
            .containment_relation_source = source;
        self.rebuild_file_tree_projection_rows();
    }

    pub fn set_file_tree_sort_mode(&mut self, sort_mode: FileTreeSortMode) {
        self.workspace.file_tree_projection_state.sort_mode = sort_mode;
    }

    pub fn set_file_tree_root_filter(&mut self, root_filter: Option<String>) {
        self.workspace.file_tree_projection_state.root_filter = root_filter;
        self.rebuild_file_tree_projection_rows();
    }

    #[cfg(test)]
    fn upsert_file_tree_row_target(
        &mut self,
        row_key: impl Into<String>,
        target: FileTreeProjectionTarget,
    ) {
        self.workspace
            .file_tree_projection_state
            .row_targets
            .insert(row_key.into(), target);
    }

    pub fn set_file_tree_selected_rows(&mut self, rows: impl IntoIterator<Item = String>) {
        self.workspace.file_tree_projection_state.selected_rows = rows.into_iter().collect();
    }

    pub fn set_file_tree_expanded_rows(&mut self, rows: impl IntoIterator<Item = String>) {
        let expanded_rows: HashSet<String> = rows.into_iter().collect();
        self.workspace.file_tree_projection_state.expanded_rows = expanded_rows.clone();
        self.workspace
            .file_tree_projection_state
            .collapsed_rows
            .retain(|row| !expanded_rows.contains(row));
    }

    pub fn rebuild_file_tree_projection_rows(&mut self) {
        use FileTreeContainmentRelationSource as Source;

        let mut row_targets: HashMap<String, FileTreeProjectionTarget> = HashMap::new();

        match self
            .workspace
            .file_tree_projection_state
            .containment_relation_source
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
                        FileTreeProjectionTarget::Node(key),
                    );
                }
            }
            Source::SavedViewCollections => {
                let mut view_ids: Vec<GraphViewId> = self.workspace.views.keys().copied().collect();
                view_ids.sort_by_key(|view_id| view_id.as_uuid());
                for view_id in view_ids {
                    row_targets.insert(
                        format!("view:{}", view_id.as_uuid()),
                        FileTreeProjectionTarget::SavedView(view_id),
                    );
                }
            }
            Source::ImportedFilesystemProjection => {
                let mut file_rows: Vec<(String, NodeKey, Uuid)> = self
                    .workspace
                    .domain
                    .graph
                    .nodes()
                    .filter_map(|(key, node)| {
                        let parsed = url::Url::parse(&node.url).ok()?;
                        if parsed.scheme() != "file" {
                            return None;
                        }
                        let mut path = parsed.path().to_string();
                        if path.is_empty() {
                            return None;
                        }
                        while path.len() > 1 && path.ends_with('/') {
                            path.pop();
                        }
                        Some((format!("fs:{path}"), key, node.id))
                    })
                    .collect();
                file_rows.sort_by(|(left_path, _, left_id), (right_path, _, right_id)| {
                    left_path
                        .cmp(right_path)
                        .then_with(|| left_id.cmp(right_id))
                });
                for (row_key, key, node_id) in file_rows {
                    row_targets.insert(
                        format!("{row_key}#{node_id}"),
                        FileTreeProjectionTarget::Node(key),
                    );
                }
            }
        }

        if let Some(root_filter) = self
            .workspace
            .file_tree_projection_state
            .root_filter
            .as_deref()
        {
            let filter = root_filter.trim();
            if !filter.is_empty() {
                row_targets.retain(|row_key, _| row_key.contains(filter));
            }
        }

        let valid_rows: HashSet<String> = row_targets.keys().cloned().collect();
        self.workspace.file_tree_projection_state.row_targets = row_targets;
        self.workspace
            .file_tree_projection_state
            .selected_rows
            .retain(|row| valid_rows.contains(row));
        self.workspace
            .file_tree_projection_state
            .expanded_rows
            .retain(|row| valid_rows.contains(row));
        self.workspace
            .file_tree_projection_state
            .collapsed_rows
            .retain(|row| valid_rows.contains(row));
    }

    /// Set whether the user is actively interacting with the graph
    pub fn set_interacting(&mut self, interacting: bool) {
        if self.workspace.is_interacting == interacting {
            return;
        }
        self.workspace.is_interacting = interacting;

        if interacting {
            self.workspace.physics_running_before_interaction =
                Some(self.workspace.physics.base.is_running);
            self.workspace.physics.base.is_running = false;
            self.workspace.drag_release_frames_remaining = 0;
        } else if let Some(was_running) = self.workspace.physics_running_before_interaction.take() {
            if was_running {
                self.workspace.physics.base.is_running = true;
                self.workspace.drag_release_frames_remaining = 0;
            } else if self.camera_position_fit_locked() {
                self.workspace.physics.base.is_running = false;
                self.workspace.drag_release_frames_remaining = 0;
            } else {
                self.workspace.physics.base.is_running = true;
                self.workspace.drag_release_frames_remaining = 10;
            }
        }
    }

    /// Advance frame-local physics housekeeping.
    /// Handles short post-drag inertia decay when simulation was previously paused.
    pub fn tick_frame(&mut self) {
        #[cfg(feature = "tracing")]
        let _tick_span = tracing::trace_span!(
            "graph.tick_frame",
            drag_release_frames_remaining = self.workspace.drag_release_frames_remaining,
            is_interacting = self.workspace.is_interacting,
            physics_running = self.workspace.physics.base.is_running,
        )
        .entered();

        if self.workspace.drag_release_frames_remaining == 0 || self.workspace.is_interacting {
            return;
        }
        self.workspace.drag_release_frames_remaining -= 1;
        if self.workspace.drag_release_frames_remaining == 0 {
            self.workspace.physics.base.is_running = false;
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
            ViewAction::ReheatPhysics => {
                self.workspace.physics.base.is_running = true;
                self.workspace.drag_release_frames_remaining = 0;
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
                    if let Some(focused_view) = self.workspace.focused_view
                        && let Some(view) = self.workspace.views.get_mut(&focused_view)
                    {
                        view.camera.current_zoom = view.camera.clamp(zoom);
                    }
                }
                true
            }
            ViewAction::SetHighlightedEdge { from, to } => {
                let previous = self.workspace.highlighted_graph_edge;
                self.workspace.highlighted_graph_edge = Some((from, to));
                if self.workspace.highlighted_graph_edge != previous {
                    emit_event(DiagnosticEvent::MessageReceived {
                        channel_id: CHANNEL_UX_NAVIGATION_TRANSITION,
                        latency_us: 0,
                    });
                }
                true
            }
            ViewAction::ClearHighlightedEdge => {
                let had_highlighted_edge = self.workspace.highlighted_graph_edge.is_some();
                self.workspace.highlighted_graph_edge = None;
                if had_highlighted_edge {
                    emit_event(DiagnosticEvent::MessageReceived {
                        channel_id: CHANNEL_UX_NAVIGATION_TRANSITION,
                        latency_us: 0,
                    });
                }
                true
            }
            ViewAction::SetNodeFormDraft { key, form_draft } => {
                if !self.workspace.form_draft_capture_enabled {
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
            ViewAction::SetFileTreeContainmentRelationSource { source } => {
                if self
                    .workspace
                    .file_tree_projection_state
                    .containment_relation_source
                    != source
                {
                    self.set_file_tree_containment_relation_source(source);
                    emit_event(DiagnosticEvent::MessageReceived {
                        channel_id: CHANNEL_UX_NAVIGATION_TRANSITION,
                        latency_us: 0,
                    });
                }
                true
            }
            ViewAction::SetFileTreeSortMode { sort_mode } => {
                self.set_file_tree_sort_mode(sort_mode);
                true
            }
            ViewAction::SetFileTreeRootFilter { root_filter } => {
                self.set_file_tree_root_filter(root_filter);
                true
            }
            ViewAction::SetFileTreeSelectedRows { rows } => {
                let next_rows: HashSet<String> = rows.iter().cloned().collect();
                if self.workspace.file_tree_projection_state.selected_rows != next_rows {
                    self.set_file_tree_selected_rows(rows);
                    emit_event(DiagnosticEvent::MessageReceived {
                        channel_id: CHANNEL_UX_NAVIGATION_TRANSITION,
                        latency_us: 0,
                    });
                }
                true
            }
            ViewAction::SetFileTreeExpandedRows { rows } => {
                self.set_file_tree_expanded_rows(rows);
                true
            }
            ViewAction::RebuildFileTreeProjection => {
                self.rebuild_file_tree_projection_rows();
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

    fn has_typed_edge(&self, from: NodeKey, to: NodeKey, edge_type: EdgeType) -> bool {
        self.workspace
            .domain
            .graph
            .edges()
            .any(|edge| edge.from == from && edge.to == to && edge.edge_type == edge_type)
    }

    fn would_create_user_grouped_edge(&self, from: NodeKey, to: NodeKey) -> bool {
        if from == to {
            return false;
        }
        if self.workspace.domain.graph.get_node(from).is_none()
            || self.workspace.domain.graph.get_node(to).is_none()
        {
            return false;
        }
        !self.has_typed_edge(from, to, EdgeType::UserGrouped)
    }

    fn should_capture_undo_checkpoint_for_intent(&self, intent: &GraphIntent) -> bool {
        if matches!(intent, GraphIntent::AcceptHostOpenRequest { .. }) {
            return true;
        }
        let Some(mutation) = intent.as_graph_mutation() else {
            return false;
        };

        match mutation {
            GraphMutation::CreateNodeNearCenter
            | GraphMutation::CreateNodeNearCenterAndOpen { .. }
            | GraphMutation::CreateNodeAtUrl { .. }
            | GraphMutation::CreateNodeAtUrlAndOpen { .. } => true,
            GraphMutation::RemoveSelectedNodes => !self.focused_selection().is_empty(),
            GraphMutation::ClearGraph => self.workspace.domain.graph.node_count() > 0,
            GraphMutation::CreateUserGroupedEdge { from, to, .. } => {
                self.would_create_user_grouped_edge(from, to)
            }
            GraphMutation::CreateUserGroupedEdgeFromPrimarySelection => self
                .selected_pair_in_order()
                .map(|(from, to)| self.would_create_user_grouped_edge(from, to))
                .unwrap_or(false),
            GraphMutation::RemoveEdge {
                from,
                to,
                edge_type,
            } => self.has_typed_edge(from, to, edge_type),
            GraphMutation::SetNodePinned { key, is_pinned } => {
                let Some(node) = self.workspace.domain.graph.get_node(key) else {
                    return false;
                };
                let has_pin_tag = self
                    .workspace
                    .semantic_tags
                    .get(&key)
                    .is_some_and(|tags| tags.contains(Self::TAG_PIN));
                node.is_pinned != is_pinned || has_pin_tag != is_pinned
            }
            GraphMutation::SetNodeUrl { key, new_url } => self
                .workspace
                .domain
                .graph
                .get_node(key)
                .map(|node| node.url != new_url)
                .unwrap_or(false),
            GraphMutation::TagNode { key, tag } => {
                let Some(node) = self.workspace.domain.graph.get_node(key) else {
                    return false;
                };
                if tag == Self::TAG_PIN && !node.is_pinned {
                    return true;
                }
                !self
                    .workspace
                    .semantic_tags
                    .get(&key)
                    .is_some_and(|tags| tags.contains(&tag))
            }
            GraphMutation::UntagNode { key, tag } => {
                if tag == Self::TAG_PIN
                    && self
                        .workspace
                        .domain
                        .graph
                        .get_node(key)
                        .map(|node| node.is_pinned)
                        .unwrap_or(false)
                {
                    return true;
                }
                self.workspace
                    .semantic_tags
                    .get(&key)
                    .is_some_and(|tags| tags.contains(&tag))
            }
            GraphMutation::UpdateNodeMimeHint { key, mime_hint } => self
                .workspace
                .domain
                .graph
                .get_node(key)
                .map(|node| node.mime_hint != mime_hint)
                .unwrap_or(false),
            GraphMutation::UpdateNodeAddressKind { key, kind } => self
                .workspace
                .domain
                .graph
                .get_node(key)
                .map(|node| node.address_kind != kind)
                .unwrap_or(false),
            _ => false,
        }
    }

    fn current_undo_checkpoint_layout_json(&self) -> Option<String> {
        self.workspace
            .last_session_workspace_layout_json
            .clone()
            .or_else(|| self.load_workspace_layout_json(Self::SESSION_WORKSPACE_LAYOUT_NAME))
    }

    fn intent_blocked_during_history_preview(intent: &GraphIntent) -> bool {
        Self::history_preview_blocks_intent(intent)
    }

    fn replay_history_preview_cursor(
        &mut self,
        cursor: usize,
        total_steps: usize,
    ) -> Result<(), String> {
        self.apply_history_preview_cursor(cursor, total_steps)
    }

    fn apply_reducer_intent_internal(&mut self, intent: GraphIntent, allow_undo_capture: bool) {
        if self.workspace.history_preview_mode_active
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
                | GraphIntent::ClearGraph
                | GraphIntent::CreateUserGroupedEdge { .. }
                | GraphIntent::CreateUserGroupedEdgeFromPrimarySelection
                | GraphIntent::RemoveEdge { .. }
                | GraphIntent::SetNodePinned { .. }
                | GraphIntent::SetNodeUrl { .. }
                | GraphIntent::ExecuteEdgeCommand { .. }
                | GraphIntent::TagNode { .. }
                | GraphIntent::UntagNode { .. }
                | GraphIntent::UpdateNodeMimeHint { .. }
                | GraphIntent::UpdateNodeAddressKind { .. }
        ) {
            // Any graph mutation starts a fresh unsaved-change episode for
            // workspace-switch prompt gating.
            self.workspace.unsaved_workspace_prompt_warned = false;
            self.workspace.workspace_has_unsaved_changes = true;
        }

        if self.apply_workspace_only_intent(&intent) {
            return;
        }

        match intent {
            GraphIntent::TogglePhysics => self.toggle_physics(),
            GraphIntent::ToggleCameraPositionFitLock
            | GraphIntent::ToggleCameraZoomFitLock
            | GraphIntent::RequestFitToScreen
            | GraphIntent::RequestZoomIn
            | GraphIntent::RequestZoomOut
            | GraphIntent::RequestZoomReset
            | GraphIntent::RequestZoomToSelected
            | GraphIntent::ReheatPhysics
            | GraphIntent::UpdateSelection { .. }
            | GraphIntent::SelectAll
            | GraphIntent::SetNodePosition { .. }
            | GraphIntent::SetZoom { .. }
            | GraphIntent::SetHighlightedEdge { .. }
            | GraphIntent::ClearHighlightedEdge
            | GraphIntent::SetNodeFormDraft { .. }
            | GraphIntent::SetNodeThumbnail { .. }
            | GraphIntent::SetNodeFavicon { .. }
            | GraphIntent::SetFileTreeContainmentRelationSource { .. }
            | GraphIntent::SetFileTreeSortMode { .. }
            | GraphIntent::SetFileTreeRootFilter { .. }
            | GraphIntent::SetFileTreeSelectedRows { .. }
            | GraphIntent::SetFileTreeExpandedRows { .. }
            | GraphIntent::RebuildFileTreeProjection => {
                unreachable!("workspace-only intents are handled before side-effect reducer match")
            }
            GraphIntent::ToggleHelpPanel => self.toggle_help_panel(),
            GraphIntent::ToggleCommandPalette => {
                self.enqueue_workbench_intent(WorkbenchIntent::ToggleCommandPalette);
            }
            GraphIntent::ToggleRadialMenu => self.toggle_radial_menu(),
            GraphIntent::TraverseBack => {
                let target = BrowserCommandTarget::ChromeProjection {
                    fallback_node: self.focused_selection().primary(),
                };
                self.request_browser_command(target, BrowserCommand::Back);
            }
            GraphIntent::TraverseForward => {
                let target = BrowserCommandTarget::ChromeProjection {
                    fallback_node: self.focused_selection().primary(),
                };
                self.request_browser_command(target, BrowserCommand::Forward);
            }
            GraphIntent::EnterGraphViewLayoutManager => {
                self.workspace.graph_view_layout_manager.active = true;
                self.persist_graph_view_layout_manager_state();
            }
            GraphIntent::ExitGraphViewLayoutManager => {
                self.workspace.graph_view_layout_manager.active = false;
                self.persist_graph_view_layout_manager_state();
            }
            GraphIntent::ToggleGraphViewLayoutManager => {
                self.workspace.graph_view_layout_manager.active =
                    !self.workspace.graph_view_layout_manager.active;
                self.persist_graph_view_layout_manager_state();
            }
            GraphIntent::CreateGraphViewSlot {
                anchor_view,
                direction,
                open_mode,
            } => {
                self.create_graph_view_slot(anchor_view, direction, open_mode);
            }
            GraphIntent::RenameGraphViewSlot { view_id, name } => {
                self.rename_graph_view_slot(view_id, name);
            }
            GraphIntent::MoveGraphViewSlot { view_id, row, col } => {
                self.move_graph_view_slot(view_id, row, col);
            }
            GraphIntent::ArchiveGraphViewSlot { view_id } => {
                self.archive_graph_view_slot(view_id);
            }
            GraphIntent::RestoreGraphViewSlot { view_id, row, col } => {
                self.restore_graph_view_slot(view_id, row, col);
            }
            GraphIntent::RouteGraphViewToWorkbench { view_id, mode } => {
                self.route_graph_view_to_workbench(view_id, mode);
            }
            GraphIntent::Undo => {
                let current_layout = self.current_undo_checkpoint_layout_json();
                let _ = self.perform_undo(current_layout);
            }
            GraphIntent::Redo => {
                let current_layout = self.current_undo_checkpoint_layout_json();
                let _ = self.perform_redo(current_layout);
            }
            GraphIntent::CreateNodeNearCenter => {
                self.create_new_node_near_center();
            }
            GraphIntent::CreateNodeNearCenterAndOpen { mode } => {
                let key = self.create_new_node_near_center();
                self.request_open_node_tile_mode(key, mode);
            }
            GraphIntent::CreateNodeAtUrl { url, position } => {
                let key = self.add_node_and_sync(url, position);
                self.select_node(key, false);
            }
            GraphIntent::CreateNodeAtUrlAndOpen {
                url,
                position,
                mode,
            } => {
                let key = self.add_node_and_sync(url, position);
                self.select_node(key, false);
                self.request_open_node_tile_mode(key, mode);
            }
            GraphIntent::AcceptHostOpenRequest { request } => {
                self.handle_host_open_request(request);
            }
            GraphIntent::CreateNoteForNode { key, title } => {
                let _ = self.create_note_for_node(key, title);
            }
            GraphIntent::RemoveSelectedNodes => self.remove_selected_nodes(),
            GraphIntent::ClearGraph => self.clear_graph(),
            GraphIntent::SelectNode { key, multi_select } => {
                self.select_node(key, multi_select);
                // Single-selecting an unloaded node should prewarm it (without opening a tile).
                if !multi_select
                    && self.focused_selection().primary() == Some(key)
                    && !self.is_crash_blocked(key)
                    && self.get_webview_for_node(key).is_none()
                    && self
                        .workspace
                        .domain
                        .graph
                        .get_node(key)
                        .map(|node| node.lifecycle != crate::graph::NodeLifecycle::Active)
                        .unwrap_or(false)
                {
                    self.promote_node_to_active_with_cause(key, LifecycleCause::SelectedPrewarm);
                }
            }
            GraphIntent::SetInteracting { interacting } => self.set_interacting(interacting),
            GraphIntent::SetViewLens { view_id, lens } => {
                let lens = self.with_registry_lens_defaults(lens);
                let lens = if let Some(lens_id) = lens.lens_id.as_deref() {
                    crate::shell::desktop::runtime::registries::phase2_resolve_lens(lens_id)
                } else if lens.name.starts_with("lens:") {
                    crate::shell::desktop::runtime::registries::phase2_resolve_lens(&lens.name)
                } else {
                    lens
                };
                if let Some(view) = self.workspace.views.get_mut(&view_id) {
                    view.lens = lens;
                }
            }
            GraphIntent::SetViewDimension { view_id, dimension } => {
                if let Some(view) = self.workspace.views.get_mut(&view_id) {
                    // F9 tracked capability: persist view preference now; renderer/runtime
                    // behavior for 3D modes lands in follow-up implementation slices.
                    view.dimension = dimension;
                }
            }
            GraphIntent::SetNodeUrl { key, new_url } => {
                let _ = self.update_node_url_and_log(key, new_url);
            }
            GraphIntent::OpenNodeFrameRouted { key, prefer_frame } => {
                self.apply_open_node_frame_routed(key, prefer_frame);
            }
            GraphIntent::OpenNodeWorkspaceRouted {
                key,
                prefer_workspace,
            } => {
                self.apply_open_node_workspace_routed(key, prefer_workspace);
            }
            GraphIntent::CreateUserGroupedEdge { from, to, label } => {
                self.add_user_grouped_edge_if_missing(from, to, label);
            }
            GraphIntent::RemoveEdge {
                from,
                to,
                edge_type,
            } => {
                let _ = self.remove_edges_and_log(from, to, edge_type);
            }
            GraphIntent::CreateUserGroupedEdgeFromPrimarySelection => {
                self.create_user_grouped_edge_from_primary_selection();
            }
            GraphIntent::GroupNodesBySemanticTags => {
                self.group_nodes_by_semantic_tags();
            }
            GraphIntent::ExecuteEdgeCommand { command } => {
                let intents = self.intents_for_edge_command(command);
                self.apply_reducer_intents(intents);
            }
            GraphIntent::SetNodePinned { key, is_pinned } => {
                self.set_node_pinned_and_log(key, is_pinned);
            }
            GraphIntent::TogglePrimaryNodePin => {
                if let Some(key) = self.focused_selection().primary()
                    && let Some(node) = self.workspace.domain.graph.get_node(key)
                {
                    self.apply_reducer_intents([GraphIntent::SetNodePinned {
                        key,
                        is_pinned: !node.is_pinned,
                    }]);
                }
            }
            GraphIntent::PromoteNodeToActive { key, cause } => {
                self.promote_node_to_active_with_cause(key, cause);
            }
            GraphIntent::DemoteNodeToWarm { key, cause } => {
                self.demote_node_to_warm_with_cause(key, cause);
            }
            GraphIntent::DemoteNodeToCold { key, cause } => {
                self.demote_node_to_cold_with_cause(key, cause);
            }
            GraphIntent::MarkRuntimeBlocked {
                key,
                reason,
                retry_at,
            } => {
                self.mark_runtime_blocked(key, reason, retry_at);
            }
            GraphIntent::ClearRuntimeBlocked { key, cause } => {
                let _ = cause;
                self.clear_runtime_blocked(key);
            }
            GraphIntent::MapWebviewToNode { webview_id, key } => {
                self.map_webview_to_node(webview_id, key);
            }
            GraphIntent::UnmapWebview { webview_id } => {
                let _ = self.unmap_webview(webview_id);
            }
            GraphIntent::WebViewCreated {
                parent_webview_id,
                child_webview_id,
                initial_url,
            } => {
                self.handle_webview_created(parent_webview_id, child_webview_id, initial_url);
            }
            GraphIntent::WebViewUrlChanged {
                webview_id,
                new_url,
            } => {
                self.handle_webview_url_changed(webview_id, new_url);
            }
            GraphIntent::WebViewHistoryChanged {
                webview_id,
                entries,
                current,
            } => {
                self.handle_webview_history_changed(webview_id, entries, current);
            }
            GraphIntent::WebViewScrollChanged {
                webview_id,
                scroll_x,
                scroll_y,
            } => {
                self.handle_webview_scroll_changed(webview_id, scroll_x, scroll_y);
            }
            GraphIntent::WebViewTitleChanged { webview_id, title } => {
                self.handle_webview_title_changed(webview_id, title);
            }
            GraphIntent::WebViewCrashed {
                webview_id,
                reason,
                has_backtrace,
            } => {
                self.handle_webview_crashed(webview_id, reason, has_backtrace);
            }
            GraphIntent::TagNode { key, tag } => {
                if self.workspace.domain.graph.get_node(key).is_some() {
                    if tag == Self::TAG_PIN {
                        self.set_node_pinned_and_log(key, true);
                    }

                    let tags = self.workspace.semantic_tags.entry(key).or_default();
                    if tags.insert(tag) {
                        self.workspace.semantic_index_dirty = true;
                    }
                }
            }
            GraphIntent::UntagNode { key, tag } => {
                if tag == Self::TAG_PIN {
                    self.set_node_pinned_and_log(key, false);
                }

                if let Some(tags) = self.workspace.semantic_tags.get_mut(&key)
                    && tags.remove(&tag)
                {
                    if tags.is_empty() {
                        self.workspace.semantic_tags.remove(&key);
                    }
                    self.workspace.semantic_index_dirty = true;
                }
            }
            GraphIntent::ClearHistoryTimeline
            | GraphIntent::ClearHistoryDissolved
            | GraphIntent::AutoCurateHistoryTimeline { .. }
            | GraphIntent::AutoCurateHistoryDissolved { .. }
            | GraphIntent::ExportHistoryTimeline
            | GraphIntent::ExportHistoryDissolved
            | GraphIntent::EnterHistoryTimelinePreview
            | GraphIntent::ExitHistoryTimelinePreview
            | GraphIntent::HistoryTimelinePreviewIsolationViolation { .. }
            | GraphIntent::HistoryTimelineReplayStarted
            | GraphIntent::HistoryTimelineReplaySetTotal { .. }
            | GraphIntent::HistoryTimelineReplayAdvance { .. }
            | GraphIntent::HistoryTimelineReplayReset
            | GraphIntent::HistoryTimelineReplayProgress { .. }
            | GraphIntent::HistoryTimelineReplayFinished { .. }
            | GraphIntent::HistoryTimelineReturnToPresentFailed { .. } => {
                self.apply_history_runtime_intent(intent)
            }
            GraphIntent::WorkflowActivated { .. } => {}
            GraphIntent::Noop => {}
            GraphIntent::SetMemoryPressureStatus {
                level,
                available_mib,
                total_mib,
            } => {
                self.set_memory_pressure_status(level, available_mib, total_mib);
                crate::shell::desktop::runtime::registries::phase3_propagate_subsystem_health_memory_pressure(
                    level,
                    available_mib,
                    total_mib,
                );
            }
            GraphIntent::ModActivated { mod_id } => {
                crate::shell::desktop::runtime::registries::phase3_route_mod_lifecycle_event(
                    &mod_id, true,
                );
                log::info!("mod activated: {mod_id}");
            }
            GraphIntent::ModLoadFailed { mod_id, reason } => {
                crate::shell::desktop::runtime::registries::phase3_route_mod_lifecycle_event(
                    &mod_id, false,
                );
                log::warn!("mod load failed: {mod_id} ({reason})");
            }
            GraphIntent::ApplyRemoteDelta { entries } => {
                // TODO: Phase 6.2 - sync integrated logic for applying peer log entries
                log::debug!("peer log entries received: {} bytes", entries.len());
            }
            GraphIntent::SyncNow => {
                match self.request_sync_all_trusted_peers(Self::SESSION_WORKSPACE_LAYOUT_NAME) {
                    Ok(enqueued) => {
                        log::info!("manual Verse sync queued for {} peer(s)", enqueued);
                    }
                    Err(error) => {
                        log::warn!("manual Verse sync unavailable: {error}");
                    }
                }
            }
            GraphIntent::TrustPeer {
                peer_id,
                display_name,
            } => match peer_id.parse::<iroh::NodeId>() {
                Ok(node_id) => {
                    crate::mods::native::verse::trust_peer(
                        crate::mods::native::verse::TrustedPeer {
                            node_id,
                            display_name,
                            role: crate::mods::native::verse::PeerRole::Friend,
                            added_at: std::time::SystemTime::now(),
                            last_seen: Some(std::time::SystemTime::now()),
                            workspace_grants: Vec::new(),
                        },
                    );
                    log::info!("paired trusted peer: {peer_id}");
                }
                Err(error) => {
                    log::warn!("invalid peer id for trust-peer '{peer_id}': {error}");
                }
            },
            GraphIntent::GrantWorkspaceAccess {
                peer_id,
                workspace_id,
            } => match peer_id.parse::<iroh::NodeId>() {
                Ok(node_id) => {
                    crate::mods::native::verse::grant_workspace_access(
                        node_id,
                        workspace_id.clone(),
                        crate::mods::native::verse::AccessLevel::ReadWrite,
                    );
                    log::info!(
                        "granting workspace access '{}' to peer {}",
                        workspace_id,
                        peer_id
                    );
                }
                Err(error) => {
                    log::warn!("invalid peer id for grant-workspace-access '{peer_id}': {error}");
                }
            },
            GraphIntent::ForgetDevice { peer_id } => match peer_id.parse::<iroh::NodeId>() {
                Ok(node_id) => {
                    crate::mods::native::verse::revoke_peer(node_id);
                    log::info!("forgetting device: {peer_id}");
                }
                Err(error) => {
                    log::warn!("invalid peer id for forget-device '{peer_id}': {error}");
                }
            },
            GraphIntent::RevokeWorkspaceAccess {
                peer_id,
                workspace_id,
            } => match peer_id.parse::<iroh::NodeId>() {
                Ok(node_id) => {
                    crate::mods::native::verse::revoke_workspace_access(
                        node_id,
                        workspace_id.clone(),
                    );
                    log::info!(
                        "revoking workspace access '{}' for peer {}",
                        workspace_id,
                        peer_id
                    );
                }
                Err(error) => {
                    log::warn!("invalid peer id for revoke-workspace-access '{peer_id}': {error}");
                }
            },
            GraphIntent::UpdateNodeMimeHint { key, mime_hint } => {
                let node_id = self
                    .workspace
                    .domain
                    .graph
                    .get_node(key)
                    .map(|node| node.id);
                let GraphDeltaResult::NodeMetadataUpdated(updated) = self
                    .apply_graph_delta_and_sync(GraphDelta::SetNodeMimeHint {
                        key,
                        mime_hint: mime_hint.clone(),
                    })
                else {
                    unreachable!("mime hint delta must return NodeMetadataUpdated");
                };
                if updated
                    && let Some(store) = &mut self.services.persistence
                    && let Some(node_id) = node_id
                {
                    store.log_mutation(&LogEntry::UpdateNodeMimeHint {
                        node_id: node_id.to_string(),
                        mime_hint,
                    });
                }
            }
            GraphIntent::UpdateNodeAddressKind { key, kind } => {
                let node_id = self
                    .workspace
                    .domain
                    .graph
                    .get_node(key)
                    .map(|node| node.id);
                let GraphDeltaResult::NodeMetadataUpdated(updated) =
                    self.apply_graph_delta_and_sync(GraphDelta::SetNodeAddressKind { key, kind })
                else {
                    unreachable!("address kind delta must return NodeMetadataUpdated");
                };
                if updated
                    && let Some(store) = &mut self.services.persistence
                    && let Some(node_id) = node_id
                {
                    let persisted_kind = match kind {
                        crate::graph::AddressKind::Http => {
                            crate::services::persistence::types::PersistedAddressKind::Http
                        }
                        crate::graph::AddressKind::File => {
                            crate::services::persistence::types::PersistedAddressKind::File
                        }
                        crate::graph::AddressKind::Custom => {
                            crate::services::persistence::types::PersistedAddressKind::Custom
                        }
                    };
                    store.log_mutation(&LogEntry::UpdateNodeAddressKind {
                        node_id: node_id.to_string(),
                        kind: persisted_kind,
                    });
                }
            }
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

    pub fn request_sync_all_trusted_peers(&self, workspace_id: &str) -> Result<usize, String> {
        let Some(tx) = self.services.sync_command_tx.clone() else {
            return Err("sync worker command channel unavailable".to_string());
        };
        let peers = crate::mods::native::verse::get_trusted_peers();
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

    /// Persist serialized tile layout JSON under a workspace name.
    pub fn save_workspace_layout_json(&mut self, name: &str, layout_json: &str) {
        if let Some(store) = &mut self.services.persistence
            && let Err(e) = store.save_workspace_layout_json(name, layout_json)
        {
            warn!("Failed to save frame layout '{name}': {e}");
        }
        if !Self::is_reserved_workspace_layout_name(name) {
            self.workspace.current_workspace_is_synthesized = false;
            self.workspace.workspace_has_unsaved_changes = false;
            self.workspace.unsaved_workspace_prompt_warned = false;
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
        let retention = self.workspace.workspace_autosave_retention;
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
        if self.workspace.last_session_workspace_layout_hash == Some(next_hash) {
            return;
        }
        if let Some(last_at) = self.workspace.last_workspace_autosave_at
            && last_at.elapsed() < self.workspace.workspace_autosave_interval
        {
            return;
        }
        let previous_latest = self.load_workspace_layout_json(Self::SESSION_WORKSPACE_LAYOUT_NAME);
        self.save_workspace_layout_json(Self::SESSION_WORKSPACE_LAYOUT_NAME, layout_json);
        if let Some(previous_latest) = previous_latest {
            self.rotate_session_workspace_history(&previous_latest);
        }
        self.workspace.last_session_workspace_layout_hash = Some(next_hash);
        self.workspace.last_session_workspace_layout_json = Some(layout_json.to_string());
        self.workspace.last_workspace_autosave_at = Some(Instant::now());
    }

    /// Mark currently loaded layout as session baseline to suppress redundant writes.
    pub fn mark_session_workspace_layout_json(&mut self, layout_json: &str) {
        self.workspace.last_session_workspace_layout_hash =
            Some(Self::layout_json_hash(layout_json));
        self.workspace.last_session_workspace_layout_json = Some(layout_json.to_string());
        self.workspace.last_workspace_autosave_at = Some(Instant::now());
    }

    /// Mark currently loaded layout as session baseline to suppress redundant writes.
    pub fn mark_session_frame_layout_json(&mut self, layout_json: &str) {
        self.mark_session_workspace_layout_json(layout_json);
    }

    pub fn last_session_workspace_layout_json(&self) -> Option<&str> {
        self.workspace.last_session_workspace_layout_json.as_deref()
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

    pub fn is_reserved_workspace_layout_name(name: &str) -> bool {
        name == "latest"
            || name == Self::SESSION_WORKSPACE_LAYOUT_NAME
            || name == Self::WORKSPACE_PIN_WORKSPACE_NAME
            || name == Self::WORKSPACE_PIN_PANE_NAME
            || name == Self::SETTINGS_TOAST_ANCHOR_NAME
            || name == Self::SETTINGS_COMMAND_PALETTE_SHORTCUT_NAME
            || name == Self::SETTINGS_HELP_PANEL_SHORTCUT_NAME
            || name == Self::SETTINGS_RADIAL_MENU_SHORTCUT_NAME
            || name == Self::SETTINGS_KEYBOARD_PAN_STEP_NAME
            || name == Self::SETTINGS_KEYBOARD_PAN_INPUT_MODE_NAME
            || name == Self::SETTINGS_CAMERA_PAN_INERTIA_ENABLED_NAME
            || name == Self::SETTINGS_CAMERA_PAN_INERTIA_DAMPING_NAME
            || name == Self::SETTINGS_LASSO_BINDING_NAME
            || name == Self::SETTINGS_INPUT_BINDING_REMAPS_NAME
            || name == Self::SETTINGS_OMNIBAR_PREFERRED_SCOPE_NAME
            || name == Self::SETTINGS_OMNIBAR_NON_AT_ORDER_NAME
            || name == Self::SETTINGS_WRY_ENABLED_NAME
            || name == Self::SETTINGS_WORKBENCH_SURFACE_PROFILE_ID_NAME
            || name == Self::SETTINGS_CANVAS_PROFILE_ID_NAME
            || name == Self::SETTINGS_ACTIVE_WORKFLOW_ID_NAME
            || name == Self::SETTINGS_GRAPH_VIEW_LAYOUT_MANAGER_NAME
            || name.starts_with(Self::SETTINGS_DIAGNOSTICS_CHANNEL_CONFIG_PREFIX)
            || name.starts_with(Self::SESSION_WORKSPACE_PREV_PREFIX)
    }

    pub fn set_toast_anchor_preference(&mut self, preference: ToastAnchorPreference) {
        self.workspace.toast_anchor_preference = preference;
        self.save_toast_anchor_preference();
    }

    fn save_toast_anchor_preference(&mut self) {
        self.save_workspace_layout_json(
            Self::SETTINGS_TOAST_ANCHOR_NAME,
            &self.workspace.toast_anchor_preference.to_string(),
        );
    }

    pub fn set_command_palette_shortcut(&mut self, shortcut: CommandPaletteShortcut) {
        self.workspace.command_palette_shortcut = shortcut;
        self.save_command_palette_shortcut();
    }

    fn save_command_palette_shortcut(&mut self) {
        self.save_workspace_layout_json(
            Self::SETTINGS_COMMAND_PALETTE_SHORTCUT_NAME,
            &self.workspace.command_palette_shortcut.to_string(),
        );
    }

    pub fn set_help_panel_shortcut(&mut self, shortcut: HelpPanelShortcut) {
        self.workspace.help_panel_shortcut = shortcut;
        self.save_help_panel_shortcut();
    }

    fn save_help_panel_shortcut(&mut self) {
        self.save_workspace_layout_json(
            Self::SETTINGS_HELP_PANEL_SHORTCUT_NAME,
            &self.workspace.help_panel_shortcut.to_string(),
        );
    }

    pub fn set_radial_menu_shortcut(&mut self, shortcut: RadialMenuShortcut) {
        self.workspace.radial_menu_shortcut = shortcut;
        self.save_radial_menu_shortcut();
    }

    pub fn keyboard_pan_step(&self) -> f32 {
        self.workspace.keyboard_pan_step
    }

    pub fn set_keyboard_pan_step(&mut self, step: f32) {
        let normalized = step.clamp(1.0, 200.0);
        self.workspace.keyboard_pan_step = normalized;
        self.save_keyboard_pan_step();
    }

    pub fn keyboard_pan_input_mode(&self) -> KeyboardPanInputMode {
        self.workspace.keyboard_pan_input_mode
    }

    pub fn set_keyboard_pan_input_mode(&mut self, mode: KeyboardPanInputMode) {
        self.workspace.keyboard_pan_input_mode = mode;
        self.save_keyboard_pan_input_mode();
    }

    pub fn camera_pan_inertia_enabled(&self) -> bool {
        self.workspace.camera_pan_inertia_enabled
    }

    pub fn set_camera_pan_inertia_enabled(&mut self, enabled: bool) {
        self.workspace.camera_pan_inertia_enabled = enabled;
        self.save_camera_pan_inertia_enabled();
    }

    pub fn camera_pan_inertia_damping(&self) -> f32 {
        self.workspace.camera_pan_inertia_damping
    }

    pub fn set_camera_pan_inertia_damping(&mut self, damping: f32) {
        let normalized = damping.clamp(0.70, 0.99);
        self.workspace.camera_pan_inertia_damping = normalized;
        self.save_camera_pan_inertia_damping();
    }

    pub fn lasso_binding_preference(&self) -> CanvasLassoBinding {
        self.workspace.lasso_binding_preference
    }

    pub fn set_lasso_binding_preference(&mut self, binding: CanvasLassoBinding) {
        self.workspace.lasso_binding_preference = binding;
        self.save_lasso_binding_preference();
    }

    pub fn set_input_binding_remaps(
        &mut self,
        remaps: &[InputBindingRemap],
    ) -> Result<(), InputRemapConflict> {
        phase2_apply_input_binding_remaps(remaps)?;
        self.save_input_binding_remaps(remaps);
        Ok(())
    }

    pub fn input_binding_remaps(&self) -> Vec<InputBindingRemap> {
        self.load_workspace_layout_json(Self::SETTINGS_INPUT_BINDING_REMAPS_NAME)
            .and_then(|raw| Self::decode_input_binding_remaps(&raw).ok())
            .unwrap_or_default()
    }

    fn save_radial_menu_shortcut(&mut self) {
        self.save_workspace_layout_json(
            Self::SETTINGS_RADIAL_MENU_SHORTCUT_NAME,
            &self.workspace.radial_menu_shortcut.to_string(),
        );
    }

    fn save_keyboard_pan_step(&mut self) {
        self.save_workspace_layout_json(
            Self::SETTINGS_KEYBOARD_PAN_STEP_NAME,
            &format!("{:.3}", self.workspace.keyboard_pan_step),
        );
    }

    fn save_keyboard_pan_input_mode(&mut self) {
        self.save_workspace_layout_json(
            Self::SETTINGS_KEYBOARD_PAN_INPUT_MODE_NAME,
            &self.workspace.keyboard_pan_input_mode.to_string(),
        );
    }

    fn save_camera_pan_inertia_enabled(&mut self) {
        self.save_workspace_layout_json(
            Self::SETTINGS_CAMERA_PAN_INERTIA_ENABLED_NAME,
            if self.workspace.camera_pan_inertia_enabled {
                "true"
            } else {
                "false"
            },
        );
    }

    fn save_camera_pan_inertia_damping(&mut self) {
        self.save_workspace_layout_json(
            Self::SETTINGS_CAMERA_PAN_INERTIA_DAMPING_NAME,
            &format!("{:.3}", self.workspace.camera_pan_inertia_damping),
        );
    }

    fn save_lasso_binding_preference(&mut self) {
        self.save_workspace_layout_json(
            Self::SETTINGS_LASSO_BINDING_NAME,
            &self.workspace.lasso_binding_preference.to_string(),
        );
    }

    fn save_input_binding_remaps(&mut self, remaps: &[InputBindingRemap]) {
        let encoded = remaps
            .iter()
            .map(InputBindingRemap::encode)
            .collect::<Vec<_>>()
            .join("\n");
        self.save_workspace_layout_json(Self::SETTINGS_INPUT_BINDING_REMAPS_NAME, &encoded);
    }

    pub fn set_omnibar_preferred_scope(&mut self, scope: OmnibarPreferredScope) {
        self.workspace.omnibar_preferred_scope = scope;
        self.save_omnibar_preferred_scope();
    }

    fn save_omnibar_preferred_scope(&mut self) {
        self.save_workspace_layout_json(
            Self::SETTINGS_OMNIBAR_PREFERRED_SCOPE_NAME,
            &self.workspace.omnibar_preferred_scope.to_string(),
        );
    }

    pub fn set_omnibar_non_at_order(&mut self, order: OmnibarNonAtOrderPreset) {
        self.workspace.omnibar_non_at_order = order;
        self.save_omnibar_non_at_order();
    }

    fn save_omnibar_non_at_order(&mut self) {
        self.save_workspace_layout_json(
            Self::SETTINGS_OMNIBAR_NON_AT_ORDER_NAME,
            &self.workspace.omnibar_non_at_order.to_string(),
        );
    }

    pub fn wry_enabled(&self) -> bool {
        self.workspace.wry_enabled
    }

    pub fn set_wry_enabled(&mut self, enabled: bool) {
        self.workspace.wry_enabled = enabled;
        self.save_wry_enabled();
    }

    fn save_wry_enabled(&mut self) {
        self.save_workspace_layout_json(
            Self::SETTINGS_WRY_ENABLED_NAME,
            if self.workspace.wry_enabled {
                "true"
            } else {
                "false"
            },
        );
    }

    pub fn set_default_registry_lens_id(&mut self, lens_id: Option<&str>) {
        let normalized = Self::normalize_optional_registry_id(lens_id.map(str::to_owned));
        self.workspace.default_registry_lens_id = normalized.clone();
        self.save_workspace_layout_json(
            Self::SETTINGS_REGISTRY_LENS_ID_NAME,
            normalized.as_deref().unwrap_or(""),
        );
    }

    pub fn set_default_registry_physics_id(&mut self, physics_id: Option<&str>) {
        let normalized = Self::normalize_optional_registry_id(physics_id.map(str::to_owned));
        self.workspace.default_registry_physics_id = normalized.clone();
        self.save_workspace_layout_json(
            Self::SETTINGS_REGISTRY_PHYSICS_ID_NAME,
            normalized.as_deref().unwrap_or(""),
        );
    }

    pub fn set_default_registry_theme_id(&mut self, theme_id: Option<&str>) {
        let normalized = Self::normalize_optional_registry_id(theme_id.map(str::to_owned));
        self.workspace.default_registry_theme_id = normalized.clone();
        self.save_workspace_layout_json(
            Self::SETTINGS_REGISTRY_THEME_ID_NAME,
            normalized.as_deref().unwrap_or(""),
        );
    }

    pub fn default_registry_lens_id(&self) -> Option<&str> {
        self.workspace.default_registry_lens_id.as_deref()
    }

    pub fn default_registry_physics_id(&self) -> Option<&str> {
        self.workspace.default_registry_physics_id.as_deref()
    }

    pub fn default_registry_theme_id(&self) -> Option<&str> {
        self.workspace.default_registry_theme_id.as_deref()
    }

    pub fn set_diagnostics_channel_config(&mut self, channel_id: &str, config: &ChannelConfig) {
        let normalized = channel_id.trim().to_ascii_lowercase();
        if normalized.is_empty() {
            return;
        }
        let key = format!(
            "{}{}",
            Self::SETTINGS_DIAGNOSTICS_CHANNEL_CONFIG_PREFIX,
            normalized
        );
        self.save_workspace_layout_json(
            &key,
            &format!(
                "{}|{}|{}",
                if config.enabled { "1" } else { "0" },
                config.sample_rate,
                config.retention_count
            ),
        );
    }

    pub fn diagnostics_channel_configs(&self) -> Vec<(String, ChannelConfig)> {
        self.list_workspace_layout_names()
            .into_iter()
            .filter_map(|key| {
                let channel_id = key
                    .strip_prefix(Self::SETTINGS_DIAGNOSTICS_CHANNEL_CONFIG_PREFIX)?
                    .to_string();
                let raw = self.load_workspace_layout_json(&key)?;
                parse_diagnostics_channel_config(&raw).map(|config| (channel_id, config))
            })
            .collect()
    }

    fn load_persisted_ui_settings(&mut self) {
        let Some(raw) = self.load_workspace_layout_json(Self::SETTINGS_TOAST_ANCHOR_NAME) else {
            return self.load_additional_persisted_ui_settings();
        };
        if let Ok(preference) = raw.parse::<ToastAnchorPreference>() {
            self.workspace.toast_anchor_preference = preference;
        } else {
            warn!("Ignoring invalid persisted toast anchor preference: '{raw}'");
        }
        self.load_additional_persisted_ui_settings();
    }

    fn load_additional_persisted_ui_settings(&mut self) {
        if let Some(raw) =
            self.load_workspace_layout_json(Self::SETTINGS_COMMAND_PALETTE_SHORTCUT_NAME)
        {
            if let Ok(shortcut) = raw.parse::<CommandPaletteShortcut>() {
                self.workspace.command_palette_shortcut = shortcut;
            } else {
                warn!("Ignoring invalid persisted command-palette shortcut: '{raw}'");
            }
        }
        if let Some(raw) = self.load_workspace_layout_json(Self::SETTINGS_HELP_PANEL_SHORTCUT_NAME)
        {
            if let Ok(shortcut) = raw.parse::<HelpPanelShortcut>() {
                self.workspace.help_panel_shortcut = shortcut;
            } else {
                warn!("Ignoring invalid persisted help-panel shortcut: '{raw}'");
            }
        }
        if let Some(raw) = self.load_workspace_layout_json(Self::SETTINGS_RADIAL_MENU_SHORTCUT_NAME)
        {
            if let Ok(shortcut) = raw.parse::<RadialMenuShortcut>() {
                self.workspace.radial_menu_shortcut = shortcut;
            } else {
                warn!("Ignoring invalid persisted radial-menu shortcut: '{raw}'");
            }
        }
        if let Some(raw) = self.load_workspace_layout_json(Self::SETTINGS_KEYBOARD_PAN_STEP_NAME) {
            if let Ok(step) = raw.trim().parse::<f32>() {
                self.workspace.keyboard_pan_step = step.clamp(1.0, 200.0);
            } else {
                warn!("Ignoring invalid persisted keyboard pan step: '{raw}'");
            }
        }
        if let Some(raw) =
            self.load_workspace_layout_json(Self::SETTINGS_KEYBOARD_PAN_INPUT_MODE_NAME)
        {
            if let Ok(mode) = raw.parse::<KeyboardPanInputMode>() {
                self.workspace.keyboard_pan_input_mode = mode;
            } else {
                warn!("Ignoring invalid persisted keyboard pan input mode: '{raw}'");
            }
        }
        if let Some(raw) =
            self.load_workspace_layout_json(Self::SETTINGS_CAMERA_PAN_INERTIA_ENABLED_NAME)
        {
            match raw.trim().to_ascii_lowercase().as_str() {
                "true" | "1" | "yes" | "on" => self.workspace.camera_pan_inertia_enabled = true,
                "false" | "0" | "no" | "off" => self.workspace.camera_pan_inertia_enabled = false,
                _ => warn!("Ignoring invalid persisted camera pan inertia enabled flag: '{raw}'"),
            }
        }
        if let Some(raw) =
            self.load_workspace_layout_json(Self::SETTINGS_CAMERA_PAN_INERTIA_DAMPING_NAME)
        {
            if let Ok(damping) = raw.trim().parse::<f32>() {
                self.workspace.camera_pan_inertia_damping = damping.clamp(0.70, 0.99);
            } else {
                warn!("Ignoring invalid persisted camera pan inertia damping: '{raw}'");
            }
        }
        if let Some(raw) = self.load_workspace_layout_json(Self::SETTINGS_LASSO_BINDING_NAME) {
            if let Ok(binding) = raw.parse::<CanvasLassoBinding>() {
                self.workspace.lasso_binding_preference = binding;
            } else {
                warn!("Ignoring invalid persisted lasso binding preference: '{raw}'");
            }
        }
        self.load_persisted_input_binding_remaps();
        if let Some(raw) =
            self.load_workspace_layout_json(Self::SETTINGS_OMNIBAR_PREFERRED_SCOPE_NAME)
        {
            if let Ok(scope) = raw.parse::<OmnibarPreferredScope>() {
                self.workspace.omnibar_preferred_scope = scope;
            } else {
                warn!("Ignoring invalid persisted omnibar preferred scope: '{raw}'");
            }
        }
        if let Some(raw) = self.load_workspace_layout_json(Self::SETTINGS_OMNIBAR_NON_AT_ORDER_NAME)
        {
            if let Ok(order) = raw.parse::<OmnibarNonAtOrderPreset>() {
                self.workspace.omnibar_non_at_order = order;
            } else {
                warn!("Ignoring invalid persisted omnibar non-@ order preset: '{raw}'");
            }
        }
        if let Some(raw) = self.load_workspace_layout_json(Self::SETTINGS_WRY_ENABLED_NAME) {
            match raw.trim().to_ascii_lowercase().as_str() {
                "true" | "1" | "yes" | "on" => self.workspace.wry_enabled = true,
                "false" | "0" | "no" | "off" => self.workspace.wry_enabled = false,
                _ => warn!("Ignoring invalid persisted wry enabled flag: '{raw}'"),
            }
        }
        self.workspace.default_registry_lens_id = self
            .load_workspace_layout_json(Self::SETTINGS_REGISTRY_LENS_ID_NAME)
            .map(|raw| Self::normalize_optional_registry_id(Some(raw)))
            .unwrap_or(None);
        self.workspace.default_registry_physics_id = self
            .load_workspace_layout_json(Self::SETTINGS_REGISTRY_PHYSICS_ID_NAME)
            .map(|raw| Self::normalize_optional_registry_id(Some(raw)))
            .unwrap_or(None);
        self.workspace.default_registry_theme_id = self
            .load_workspace_layout_json(Self::SETTINGS_REGISTRY_THEME_ID_NAME)
            .map(|raw| Self::normalize_optional_registry_id(Some(raw)))
            .unwrap_or(None);
        let workbench_surface_profile_id = self
            .load_workspace_layout_json(Self::SETTINGS_WORKBENCH_SURFACE_PROFILE_ID_NAME)
            .map(|raw| raw.trim().to_ascii_lowercase())
            .filter(|raw| !raw.is_empty());
        let active_workflow_id = self
            .load_workspace_layout_json(Self::SETTINGS_ACTIVE_WORKFLOW_ID_NAME)
            .map(|raw| raw.trim().to_ascii_lowercase())
            .filter(|raw| !raw.is_empty());
        if let Some(profile_id) = workbench_surface_profile_id.as_deref() {
            crate::shell::desktop::runtime::registries::phase3_set_active_workbench_surface_profile(
                profile_id,
            );
        }
        if let Some(workflow_id) = active_workflow_id.as_deref()
            && let Err(error) = crate::shell::desktop::runtime::registries::phase3_activate_workflow(
                self,
                workflow_id,
            )
        {
            warn!("Ignoring invalid persisted workflow activation '{workflow_id}': {error:?}");
        }
        self.load_graph_view_layout_manager_state();

        crate::registries::atomic::diagnostics::apply_persisted_channel_configs(
            self.diagnostics_channel_configs(),
        );
    }

    fn load_persisted_input_binding_remaps(&mut self) {
        let Some(raw) = self.load_workspace_layout_json(Self::SETTINGS_INPUT_BINDING_REMAPS_NAME)
        else {
            phase2_reset_input_binding_remaps();
            return;
        };

        let remaps = match Self::decode_input_binding_remaps(&raw) {
            Ok(remaps) => remaps,
            Err(_) => {
                warn!("Ignoring invalid persisted input binding remaps");
                phase2_reset_input_binding_remaps();
                return;
            }
        };

        if phase2_apply_input_binding_remaps(&remaps).is_err() {
            warn!("Ignoring persisted input binding remaps that conflict with defaults");
            phase2_reset_input_binding_remaps();
        }
    }

    fn decode_input_binding_remaps(raw: &str) -> Result<Vec<InputBindingRemap>, ()> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Ok(Vec::new());
        }

        trimmed
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(InputBindingRemap::decode)
            .collect()
    }

    fn normalize_optional_registry_id(raw: Option<String>) -> Option<String> {
        raw.and_then(|value| {
            let normalized = value.trim().to_ascii_lowercase();
            (!normalized.is_empty()).then_some(normalized)
        })
    }

    fn with_registry_lens_defaults(&self, mut lens: LensConfig) -> LensConfig {
        if lens.lens_id.is_none() {
            lens.lens_id = self.workspace.default_registry_lens_id.clone();
        }
        lens
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
        self.workspace
            .node_last_active_workspace
            .retain(|_, (_, workspace_name)| workspace_name != name);
        for memberships in self.workspace.node_workspace_membership.values_mut() {
            memberships.remove(name);
        }
        self.workspace
            .node_workspace_membership
            .retain(|_, memberships| !memberships.is_empty());
        self.workspace.egui_state_dirty = true;
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
        self.workspace.last_session_workspace_layout_hash = None;
        self.workspace.last_session_workspace_layout_json = None;
        self.workspace.last_workspace_autosave_at = None;
        Ok(())
    }

    pub fn workspace_autosave_interval_secs(&self) -> u64 {
        self.workspace.workspace_autosave_interval.as_secs()
    }

    pub fn set_workspace_autosave_interval_secs(&mut self, secs: u64) -> Result<(), String> {
        if secs == 0 {
            return Err("Workspace autosave interval must be greater than zero".to_string());
        }
        self.workspace.workspace_autosave_interval = Duration::from_secs(secs);
        Ok(())
    }

    pub fn workspace_autosave_retention(&self) -> u8 {
        self.workspace.workspace_autosave_retention
    }

    pub fn set_workspace_autosave_retention(&mut self, count: u8) -> Result<(), String> {
        if count > 5 {
            return Err("Workspace autosave retention must be between 0 and 5".to_string());
        }
        if count < self.workspace.workspace_autosave_retention
            && let Some(store) = self.services.persistence.as_mut()
        {
            for idx in (count + 1)..=5 {
                let _ = store.delete_workspace_layout(&Self::session_workspace_history_key(idx));
            }
        }
        self.workspace.workspace_autosave_retention = count;
        Ok(())
    }

    /// Whether the current frame has unsaved graph changes.
    pub fn should_prompt_unsaved_workspace_save(&self) -> bool {
        self.workspace.workspace_has_unsaved_changes
    }

    /// Returns true once per unsaved-changes episode to enable one-shot warnings.
    pub fn consume_unsaved_workspace_prompt_warning(&mut self) -> bool {
        if !self.should_prompt_unsaved_workspace_save()
            || self.workspace.unsaved_workspace_prompt_warned
        {
            return false;
        }
        self.workspace.unsaved_workspace_prompt_warned = true;
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

    fn apply_loaded_graph(&mut self, graph: Graph) {
        self.workspace.domain.graph = graph;
        self.reset_selection_state();
        self.workspace.webview_to_node.clear();
        self.workspace.node_to_webview.clear();
        self.workspace.active_lru.clear();
        self.workspace.warm_cache_lru.clear();
        self.workspace.runtime_block_state.clear();
        self.workspace.active_webview_nodes.clear();
        self.workspace.pending_app_commands.clear();
        self.workspace.pending_host_create_tokens.clear();
        self.clear_choose_frame_picker();
        self.set_pending_camera_command(None, Some(CameraCommand::Fit));
        self.clear_pending_wheel_zoom_delta();
        self.workspace.node_workspace_membership.clear();
        self.workspace.views.clear();
        self.workspace.graph_view_frames.clear();
        self.workspace.domain.notes.clear();
        self.set_workspace_focused_view_with_transition(None);
        self.workspace.current_workspace_is_synthesized = false;
        self.workspace.workspace_has_unsaved_changes = false;
        self.workspace.unsaved_workspace_prompt_warned = false;
        self.workspace.domain.next_placeholder_id =
            Self::scan_max_placeholder_id(&self.workspace.domain.graph);
        self.workspace.egui_state = None;
        self.workspace.egui_state_dirty = true;
        self.workspace.semantic_tags.clear();
        self.workspace.semantic_index.clear();
        self.workspace.semantic_index_dirty = true;
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
        self.workspace.webview_to_node.clear();
        self.workspace.node_to_webview.clear();
        self.workspace.active_lru.clear();
        self.workspace.warm_cache_lru.clear();
        self.workspace.runtime_block_state.clear();
        self.workspace.active_webview_nodes.clear();
        self.workspace.pending_app_commands.clear();
        self.clear_choose_frame_picker();
        self.set_pending_camera_command(None, Some(CameraCommand::Fit));
        self.clear_pending_wheel_zoom_delta();
        self.workspace.domain.notes.clear();
        self.workspace.views.clear();
        self.workspace.graph_view_frames.clear();
        self.set_workspace_focused_view_with_transition(None);
        self.workspace.domain.next_placeholder_id = next_placeholder_id;
        self.workspace.egui_state = None;
        self.workspace.egui_state_dirty = true;
        self.workspace.semantic_tags.clear();
        self.workspace.semantic_index.clear();
        self.workspace.semantic_index_dirty = true;
        self.workspace.last_session_workspace_layout_hash = None;
        self.workspace.last_session_workspace_layout_json = None;
        self.workspace.last_workspace_autosave_at = None;
        self.workspace.workspace_activation_seq = 0;
        self.workspace.node_last_active_workspace.clear();
        self.workspace.node_workspace_membership.clear();
        self.workspace.current_workspace_is_synthesized = false;
        self.workspace.workspace_has_unsaved_changes = false;
        self.workspace.unsaved_workspace_prompt_warned = false;
        self.workspace.is_interacting = false;
        self.workspace.physics_running_before_interaction = None;
        self.workspace.toast_anchor_preference = ToastAnchorPreference::BottomRight;
        self.workspace.command_palette_shortcut = CommandPaletteShortcut::F2;
        self.workspace.help_panel_shortcut = HelpPanelShortcut::F1OrQuestion;
        self.workspace.radial_menu_shortcut = RadialMenuShortcut::F3;
        self.workspace.omnibar_preferred_scope = OmnibarPreferredScope::Auto;
        self.workspace.omnibar_non_at_order =
            OmnibarNonAtOrderPreset::ContextualThenProviderThenGlobal;
        self.workspace.wry_enabled = false;
        self.workspace.selected_tab_nodes.clear();
        self.workspace.tab_selection_anchor = None;
        self.load_persisted_ui_settings();
        Ok(())
    }

    /// Toggle force-directed layout simulation.
    pub fn toggle_physics(&mut self) {
        if self.workspace.is_interacting {
            let next = !self
                .workspace
                .physics_running_before_interaction
                .unwrap_or(self.workspace.physics.base.is_running);
            self.workspace.physics_running_before_interaction = Some(next);
            self.workspace.drag_release_frames_remaining = 0;
            return;
        }
        self.workspace.physics.base.is_running = !self.workspace.physics.base.is_running;
        self.workspace.drag_release_frames_remaining = 0;
    }

    /// Update force-directed layout configuration.
    pub fn update_physics_config(&mut self, config: FruchtermanReingoldWithCenterGravityState) {
        self.workspace.physics = config;
    }

    fn apply_graph_delta_and_sync(&mut self, delta: GraphDelta) -> GraphDeltaResult {
        let result = apply_domain_graph_delta(&mut self.workspace.domain.graph, delta.clone());
        if Self::graph_structure_changed(&result) {
            self.clear_hop_distance_cache();
        }
        if let Some(egui_state) = self.workspace.egui_state.as_mut()
            && !egui_state.sync_from_delta(&self.workspace.domain.graph, &delta, &result)
        {
            self.workspace.egui_state_dirty = true;
        }
        result
    }

    fn graph_structure_changed(result: &GraphDeltaResult) -> bool {
        match result {
            GraphDeltaResult::NodeAdded(_) => true,
            GraphDeltaResult::NodeMaybeAdded(maybe) => maybe.is_some(),
            GraphDeltaResult::EdgeAdded(maybe) => maybe.is_some(),
            GraphDeltaResult::NodeRemoved(changed) => *changed,
            GraphDeltaResult::EdgesRemoved(count) => *count > 0,
            GraphDeltaResult::TraversalAppended(_) => false,
            GraphDeltaResult::NodeMetadataUpdated(_) => false,
            GraphDeltaResult::NodeUrlUpdated(_) => false,
        }
    }

    pub fn resolve_settings_route(url: &str) -> Option<SettingsRouteTarget> {
        match VersoAddress::parse(url)? {
            VersoAddress::Settings(GraphshellSettingsPath::History) => {
                Some(SettingsRouteTarget::History)
            }
            VersoAddress::Settings(GraphshellSettingsPath::General) => {
                Some(SettingsRouteTarget::Settings(SettingsToolPage::General))
            }
            VersoAddress::Settings(GraphshellSettingsPath::Persistence) => {
                Some(SettingsRouteTarget::Settings(SettingsToolPage::Persistence))
            }
            VersoAddress::Settings(GraphshellSettingsPath::Physics) => {
                Some(SettingsRouteTarget::Settings(SettingsToolPage::Physics))
            }
            VersoAddress::Settings(GraphshellSettingsPath::Sync) => {
                Some(SettingsRouteTarget::Settings(SettingsToolPage::Sync))
            }
            VersoAddress::Settings(GraphshellSettingsPath::Appearance) => {
                Some(SettingsRouteTarget::Settings(SettingsToolPage::Appearance))
            }
            VersoAddress::Frame(_)
            | VersoAddress::View(_)
            | VersoAddress::Tool { .. }
            | VersoAddress::Clip(_)
            | VersoAddress::Settings(GraphshellSettingsPath::Other(_))
            | VersoAddress::Other { .. } => None,
        }
    }

    pub fn resolve_frame_route(url: &str) -> Option<String> {
        match VersoAddress::parse(url)? {
            VersoAddress::Frame(frame_name) => Some(frame_name),
            VersoAddress::Settings(_)
            | VersoAddress::View(_)
            | VersoAddress::Tool { .. }
            | VersoAddress::Clip(_)
            | VersoAddress::Other { .. } => None,
        }
    }

    pub fn resolve_tool_route(
        url: &str,
    ) -> Option<crate::shell::desktop::workbench::pane_model::ToolPaneState> {
        match VersoAddress::parse(url)? {
            VersoAddress::Tool { name, .. } => match name.as_str() {
                "diagnostics" => Some(crate::shell::desktop::workbench::pane_model::ToolPaneState::Diagnostics),
                "history" => Some(crate::shell::desktop::workbench::pane_model::ToolPaneState::HistoryManager),
                "accessibility" => Some(
                    crate::shell::desktop::workbench::pane_model::ToolPaneState::AccessibilityInspector,
                ),
                "settings" => Some(crate::shell::desktop::workbench::pane_model::ToolPaneState::Settings),
                _ => None,
            },
            VersoAddress::Settings(_)
            | VersoAddress::Frame(_)
            | VersoAddress::View(_)
            | VersoAddress::Clip(_)
            | VersoAddress::Other { .. } => None,
        }
    }

    pub fn resolve_view_route(url: &str) -> Option<ViewRouteTarget> {
        match VersoAddress::parse(url)? {
            VersoAddress::View(VersoViewTarget::Legacy(view_id)) => {
                let parsed = Uuid::parse_str(&view_id).ok()?;
                Some(ViewRouteTarget::GraphPane(GraphViewId(parsed)))
            }
            VersoAddress::View(VersoViewTarget::Graph(graph_id)) => {
                Some(ViewRouteTarget::Graph(graph_id))
            }
            VersoAddress::View(VersoViewTarget::Note(note_id)) => {
                let parsed = Uuid::parse_str(&note_id).ok()?;
                Some(ViewRouteTarget::Note(NoteId(parsed)))
            }
            VersoAddress::View(VersoViewTarget::Node(node_id)) => {
                let parsed = Uuid::parse_str(&node_id).ok()?;
                Some(ViewRouteTarget::Node(parsed))
            }
            VersoAddress::Settings(_)
            | VersoAddress::Frame(_)
            | VersoAddress::Tool { .. }
            | VersoAddress::Clip(_)
            | VersoAddress::Other { .. } => None,
        }
    }

    pub fn resolve_graph_route(url: &str) -> Option<String> {
        GraphAddress::parse(url).map(|address| address.graph_id)
    }

    pub fn resolve_node_route(url: &str) -> Option<Uuid> {
        let address = NodeAddress::parse(url)?;
        Uuid::parse_str(&address.node_id).ok()
    }

    pub fn resolve_clip_route(url: &str) -> Option<String> {
        match VersoAddress::parse(url)? {
            VersoAddress::Clip(clip_id) => Some(clip_id),
            VersoAddress::Settings(_)
            | VersoAddress::Frame(_)
            | VersoAddress::View(_)
            | VersoAddress::Tool { .. }
            | VersoAddress::Other { .. } => None,
        }
    }

    pub fn resolve_note_route(url: &str) -> Option<NoteId> {
        let address = NoteAddress::parse(url)?;
        let parsed = Uuid::parse_str(&address.note_id).ok()?;
        Some(NoteId(parsed))
    }

    pub fn create_note_for_node(&mut self, key: NodeKey, title: Option<String>) -> Option<NoteId> {
        let node = self.workspace.domain.graph.get_node(key)?;
        let now = SystemTime::now();
        let note_id = NoteId::new();
        let resolved_title = title.unwrap_or_else(|| {
            let base = node.title.trim();
            if base.is_empty() {
                format!("Note for {}", node.url)
            } else {
                format!("Note for {base}")
            }
        });
        let note = NoteRecord {
            id: note_id,
            title: resolved_title,
            linked_node: Some(key),
            source_url: Some(node.url.clone()),
            body: String::new(),
            created_at: now,
            updated_at: now,
        };

        self.workspace.domain.notes.insert(note_id, note);
        self.enqueue_app_command(AppCommand::OpenNote { note_id });
        self.request_open_node_tile_mode(key, PendingTileOpenMode::SplitHorizontal);
        Some(note_id)
    }

    pub fn note_record(&self, note_id: NoteId) -> Option<&NoteRecord> {
        self.workspace.domain.notes.get(&note_id)
    }

    pub fn graph_view_layout_manager_active(&self) -> bool {
        self.workspace.graph_view_layout_manager.active
    }

    #[cfg(test)]
    pub fn graph_view_slots_for_tests(&self) -> Vec<GraphViewSlot> {
        self.workspace
            .graph_view_layout_manager
            .slots
            .values()
            .cloned()
            .collect()
    }

    /// Return recent traversal archive entries (descending, newest first).
    pub fn history_manager_timeline_entries(&self, limit: usize) -> Vec<LogEntry> {
        self.services
            .persistence
            .as_ref()
            .map(|store| store.recent_traversal_archive_entries(limit))
            .unwrap_or_default()
    }

    /// Return recent dissolved archive entries (descending, newest first).
    pub fn history_manager_dissolved_entries(&self, limit: usize) -> Vec<LogEntry> {
        self.services
            .persistence
            .as_ref()
            .map(|store| store.recent_dissolved_archive_entries(limit))
            .unwrap_or_default()
    }

    /// Return (traversal_archive_count, dissolved_archive_count).
    pub fn history_manager_archive_counts(&self) -> (usize, usize) {
        self.services
            .persistence
            .as_ref()
            .map(|store| (store.traversal_archive_len(), store.dissolved_archive_len()))
            .unwrap_or((0, 0))
    }

    /// Return timeline index entries for Stage F replay cursors (newest first).
    pub fn history_timeline_index_entries(&self, limit: usize) -> Vec<TimelineIndexEntry> {
        self.services
            .persistence
            .as_ref()
            .map(|store| store.timeline_index_entries(limit))
            .unwrap_or_default()
    }

    /// Return compact history subsystem health fields for History Manager UI.
    pub fn history_health_summary(&self) -> HistoryHealthSummary {
        let (traversal_archive_count, dissolved_archive_count) =
            self.history_manager_archive_counts();
        let capture_status = if self.services.persistence.is_some() {
            HistoryCaptureStatus::Full
        } else {
            HistoryCaptureStatus::DegradedCaptureOnly
        };

        HistoryHealthSummary {
            capture_status,
            recent_traversal_append_failures: self
                .workspace
                .history_recent_traversal_append_failures,
            recent_failure_reason_bucket: self
                .workspace
                .history_recent_failure_reason_bucket
                .map(|reason| reason.as_str().to_string()),
            last_error: self.workspace.history_last_error.clone(),
            traversal_archive_count,
            dissolved_archive_count,
            preview_mode_active: self.workspace.history_preview_mode_active,
            last_preview_isolation_violation: self
                .workspace
                .history_last_preview_isolation_violation,
            replay_in_progress: self.workspace.history_replay_in_progress,
            replay_cursor: self.workspace.history_replay_cursor,
            replay_total_steps: self.workspace.history_replay_total_steps,
            last_return_to_present_result: self
                .workspace
                .history_last_return_to_present_result
                .clone(),
            last_event_unix_ms: self.workspace.history_last_event_unix_ms,
        }
    }

    /// Record an undo boundary for a pure workspace-layout mutation.
    pub fn record_workspace_undo_boundary(
        &mut self,
        workspace_layout_before: Option<String>,
        reason: UndoBoundaryReason,
    ) {
        let layout_before =
            workspace_layout_before.or_else(|| self.current_undo_checkpoint_layout_json());
        self.capture_undo_checkpoint_internal(layout_before, reason);
    }

    /// Capture current global state as an undo checkpoint.
    fn capture_undo_checkpoint(&mut self, workspace_layout_json: Option<String>) {
        self.capture_undo_checkpoint_internal(
            workspace_layout_json,
            UndoBoundaryReason::ReducerIntents,
        );
    }

    fn capture_undo_checkpoint_internal(
        &mut self,
        workspace_layout_json: Option<String>,
        _reason: UndoBoundaryReason,
    ) {
        let Some(snapshot) = self.build_undo_redo_snapshot(workspace_layout_json) else {
            warn!("Failed to serialize graph for undo checkpoint; skipping capture");
            return;
        };
        self.workspace.undo_stack.push(snapshot);
        self.workspace.redo_stack.clear();
        const MAX_UNDO_STEPS: usize = 128;
        if self.workspace.undo_stack.len() > MAX_UNDO_STEPS {
            let excess = self.workspace.undo_stack.len() - MAX_UNDO_STEPS;
            self.workspace.undo_stack.drain(0..excess);
        }
    }

    /// Perform one global undo step using current frame layout as redo checkpoint.
    fn perform_undo(&mut self, current_workspace_layout_json: Option<String>) -> bool {
        let Some(prev) = self.workspace.undo_stack.last().cloned() else {
            return false;
        };
        let Some(prev_graph) = Self::decode_undo_graph_bytes(&prev.graph_bytes) else {
            warn!("Failed to deserialize graph from undo checkpoint");
            return false;
        };
        let Some(redo_snapshot) = self.build_undo_redo_snapshot(current_workspace_layout_json)
        else {
            warn!("Failed to serialize graph for redo checkpoint");
            return false;
        };
        let _ = self.workspace.undo_stack.pop();
        self.workspace.redo_stack.push(redo_snapshot);
        self.apply_loaded_graph(prev_graph);
        self.restore_selection_snapshot(prev.active_selection, prev.selection_by_scope);
        self.workspace.highlighted_graph_edge = prev.highlighted_graph_edge;
        self.set_pending_history_workspace_layout_json(prev.workspace_layout_json);
        true
    }

    /// Perform one global redo step using current frame layout as undo checkpoint.
    fn perform_redo(&mut self, current_workspace_layout_json: Option<String>) -> bool {
        let Some(next) = self.workspace.redo_stack.last().cloned() else {
            return false;
        };
        let Some(next_graph) = Self::decode_undo_graph_bytes(&next.graph_bytes) else {
            warn!("Failed to deserialize graph from redo checkpoint");
            return false;
        };
        let Some(undo_snapshot) = self.build_undo_redo_snapshot(current_workspace_layout_json)
        else {
            warn!("Failed to serialize graph for undo checkpoint during redo");
            return false;
        };
        let _ = self.workspace.redo_stack.pop();
        self.workspace.undo_stack.push(undo_snapshot);
        self.apply_loaded_graph(next_graph);
        self.restore_selection_snapshot(next.active_selection, next.selection_by_scope);
        self.workspace.highlighted_graph_edge = next.highlighted_graph_edge;
        self.set_pending_history_workspace_layout_json(next.workspace_layout_json);
        true
    }

    /// Get the length of the undo stack (for testing).
    pub fn undo_stack_len(&self) -> usize {
        self.workspace.undo_stack.len()
    }

    /// Get the length of the redo stack (for testing).
    pub fn redo_stack_len(&self) -> usize {
        self.workspace.redo_stack.len()
    }

    /// Take pending frame layout restore emitted by undo/redo.
    pub fn take_pending_history_workspace_layout_json(&mut self) -> Option<String> {
        match self.take_pending_app_command(|command| {
            matches!(command, AppCommand::RestoreHistoryWorkspaceLayout { .. })
        })? {
            AppCommand::RestoreHistoryWorkspaceLayout { layout_json } => Some(layout_json),
            _ => None,
        }
    }

    /// Take pending frame layout restore emitted by undo/redo.
    pub fn take_pending_history_frame_layout_json(&mut self) -> Option<String> {
        self.take_pending_history_workspace_layout_json()
    }

    pub fn active_webview_limit(&self) -> usize {
        self.workspace.active_webview_limit
    }

    pub fn warm_cache_limit(&self) -> usize {
        self.workspace.warm_cache_limit
    }

    pub fn lifecycle_counts(&self) -> (usize, usize, usize, usize) {
        let mut active = 0usize;
        let mut warm = 0usize;
        let mut cold = 0usize;
        let mut tombstone = 0usize;
        for (_, node) in self.workspace.domain.graph.nodes() {
            match node.lifecycle {
                crate::graph::NodeLifecycle::Active => active += 1,
                crate::graph::NodeLifecycle::Warm => warm += 1,
                crate::graph::NodeLifecycle::Cold => cold += 1,
                crate::graph::NodeLifecycle::Tombstone => tombstone += 1,
            }
        }
        (active, warm, cold, tombstone)
    }

    pub fn mapped_webview_count(&self) -> usize {
        self.workspace.node_to_webview.len()
    }

    pub fn memory_pressure_level(&self) -> MemoryPressureLevel {
        self.workspace.memory_pressure_level
    }

    #[cfg(test)]
    fn set_form_draft_capture_enabled_for_testing(&mut self, enabled: bool) {
        self.workspace.form_draft_capture_enabled = enabled;
    }

    pub fn memory_available_mib(&self) -> u64 {
        self.workspace.memory_available_mib
    }

    pub fn memory_total_mib(&self) -> u64 {
        self.workspace.memory_total_mib
    }

    pub(crate) fn set_memory_pressure_status(
        &mut self,
        level: MemoryPressureLevel,
        available_mib: u64,
        total_mib: u64,
    ) {
        self.workspace.memory_pressure_level = level;
        self.workspace.memory_available_mib = available_mib;
        self.workspace.memory_total_mib = total_mib;
    }

    /// Scan graph for existing `about:blank#N` placeholder URLs and return
    /// the next available ID (max found + 1, or 0 if none exist).
    fn scan_max_placeholder_id(graph: &Graph) -> u32 {
        let mut max_id = 0u32;
        for (_, node) in graph.nodes() {
            if let Some(fragment) = node.url.strip_prefix("about:blank#") {
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
mod tests {
    use super::*;
    use crate::util::NoteAddress;
    use euclid::default::Point2D;
    use tempfile::TempDir;
    use uuid::Uuid;

    /// Create a unique RendererId for testing.
    fn test_webview_id() -> RendererId {
        #[cfg(not(target_os = "ios"))]
        {
            thread_local! {
                static NS_INSTALLED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
            }
            NS_INSTALLED.with(|cell| {
                if !cell.get() {
                    base::id::PipelineNamespace::install(base::id::PipelineNamespaceId(42));
                    cell.set(true);
                }
            });
            servo::WebViewId::new(base::id::PainterId::next())
        }
        #[cfg(target_os = "ios")]
        {
            static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
            RendererId(COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
        }
    }

    #[test]
    fn create_note_for_node_creates_record_and_queues_note_open() {
        let mut app = GraphBrowserApp::new_for_testing();
        let node_key = app.add_node_and_sync(
            "https://example.com/article".to_string(),
            Point2D::new(0.0, 0.0),
        );
        if let Some(node) = app.workspace.domain.graph.get_node_mut(node_key) {
            node.title = "Example Article".to_string();
        }

        let note_id = app
            .create_note_for_node(node_key, None)
            .expect("note should be created for an existing node");
        let note = app.note_record(note_id).expect("note record should exist");

        assert_eq!(note.title, "Note for Example Article");
        assert_eq!(note.linked_node, Some(node_key));
        assert_eq!(
            note.source_url.as_deref(),
            Some("https://example.com/article")
        );
        assert_eq!(app.take_pending_open_note_request(), Some(note_id));
        assert_eq!(
            app.take_pending_open_node_request(),
            Some(PendingNodeOpenRequest {
                key: node_key,
                mode: PendingTileOpenMode::SplitHorizontal,
            })
        );
    }

    #[test]
    fn resolve_note_route_parses_note_url() {
        let note_id = NoteId::new();
        let note_url = NoteAddress::note(note_id.0.to_string()).to_string();

        assert_eq!(
            GraphBrowserApp::resolve_note_route(&note_url),
            Some(note_id)
        );
    }

    #[test]
    fn resolve_note_route_rejects_invalid_note_url() {
        let note_url = "notes://not-a-uuid";
        assert_eq!(GraphBrowserApp::resolve_note_route(note_url), None);
    }

    #[test]
    fn request_open_note_by_id_queues_note_open() {
        let mut app = GraphBrowserApp::new_for_testing();
        let note_id = NoteId::new();

        app.request_open_note_by_id(note_id);

        assert_eq!(app.take_pending_open_note_request(), Some(note_id));
    }

    #[test]
    fn queued_open_requests_support_peek_before_take() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = NodeKey::new(7);

        app.request_open_node_tile_mode(key, PendingTileOpenMode::Tab);
        assert_eq!(
            app.pending_open_node_request(),
            Some(PendingNodeOpenRequest {
                key,
                mode: PendingTileOpenMode::Tab,
            })
        );
        assert_eq!(
            app.take_pending_open_node_request(),
            Some(PendingNodeOpenRequest {
                key,
                mode: PendingTileOpenMode::Tab,
            })
        );

        app.request_open_connected_from(
            key,
            PendingTileOpenMode::SplitHorizontal,
            PendingConnectedOpenScope::Connected,
        );
        assert_eq!(
            app.pending_open_connected_from(),
            Some((
                key,
                PendingTileOpenMode::SplitHorizontal,
                PendingConnectedOpenScope::Connected,
            ))
        );
        assert_eq!(
            app.take_pending_open_connected_from(),
            Some((
                key,
                PendingTileOpenMode::SplitHorizontal,
                PendingConnectedOpenScope::Connected,
            ))
        );
    }

    #[test]
    fn queued_frame_import_requests_replace_previous_values() {
        let mut app = GraphBrowserApp::new_for_testing();
        let first = NodeKey::new(1);
        let second = NodeKey::new(2);

        app.request_add_node_to_frame(first, "alpha");
        app.request_add_node_to_frame(second, "beta");
        assert_eq!(
            app.take_pending_add_node_to_frame(),
            Some((second, "beta".to_string()))
        );

        app.request_add_connected_to_frame(vec![first], "alpha");
        app.request_add_connected_to_frame(vec![second], "beta");
        assert_eq!(
            app.take_pending_add_connected_to_frame(),
            Some((vec![second], "beta".to_string()))
        );

        app.request_add_exact_nodes_to_frame(vec![first], "alpha");
        app.request_add_exact_nodes_to_frame(vec![second], "beta");
        assert_eq!(
            app.take_pending_add_exact_to_frame(),
            Some((vec![second], "beta".to_string()))
        );
    }

    #[test]
    fn removing_nodes_sanitizes_queued_frame_import_requests() {
        let mut app = GraphBrowserApp::new_for_testing();
        let kept = app.add_node_and_sync(
            "https://example.com/kept".to_string(),
            Point2D::new(0.0, 0.0),
        );
        let removed = app.add_node_and_sync(
            "https://example.com/removed".to_string(),
            Point2D::new(10.0, 0.0),
        );

        app.request_add_node_to_frame(removed, "stale-node");
        app.request_add_connected_to_frame(vec![removed, kept], "mixed-connected");
        app.request_add_exact_nodes_to_frame(vec![removed], "stale-exact");

        app.select_node(removed, false);
        app.remove_selected_nodes();

        assert_eq!(app.take_pending_add_node_to_frame(), None);
        assert_eq!(
            app.take_pending_add_connected_to_frame(),
            Some((vec![kept], "mixed-connected".to_string()))
        );
        assert_eq!(app.take_pending_add_exact_to_frame(), None);
    }

    #[test]
    fn queued_tool_surface_return_target_supports_replace_peek_and_take() {
        let mut app = GraphBrowserApp::new_for_testing();
        let first = ToolSurfaceReturnTarget::Graph(GraphViewId::new());
        let second = ToolSurfaceReturnTarget::Node(NodeKey::new(42));

        app.set_pending_tool_surface_return_target(Some(first.clone()));
        app.set_pending_tool_surface_return_target(Some(second.clone()));

        assert_eq!(
            app.pending_tool_surface_return_target(),
            Some(second.clone())
        );
        assert_eq!(app.take_pending_tool_surface_return_target(), Some(second));
        assert!(app.pending_tool_surface_return_target().is_none());

        app.set_pending_tool_surface_return_target(Some(first));
        app.set_pending_tool_surface_return_target(None);
        assert!(app.take_pending_tool_surface_return_target().is_none());
    }

    #[test]
    fn test_select_node_marks_selection_state() {
        let mut app = GraphBrowserApp::new_for_testing();
        let node_key = app
            .workspace
            .domain
            .graph
            .add_node("test".to_string(), Point2D::new(100.0, 100.0));

        app.select_node(node_key, false);

        // Node should be selected
        assert!(app.focused_selection().contains(&node_key));
    }

    #[test]
    fn test_per_view_selection_isolated_and_restored_on_focus_switch() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_a = GraphViewId::new();
        let view_b = GraphViewId::new();
        app.workspace
            .views
            .insert(view_a, GraphViewState::new_with_id(view_a, "A"));
        app.workspace
            .views
            .insert(view_b, GraphViewState::new_with_id(view_b, "B"));

        let node_a = app
            .workspace
            .domain
            .graph
            .add_node("a".to_string(), Point2D::new(10.0, 10.0));
        let node_b = app
            .workspace
            .domain
            .graph
            .add_node("b".to_string(), Point2D::new(20.0, 20.0));

        app.set_workspace_focused_view_with_transition(Some(view_a));
        app.select_node(node_a, false);

        app.set_workspace_focused_view_with_transition(Some(view_b));
        app.select_node(node_b, false);

        assert_eq!(app.get_single_selected_node_for_view(view_a), Some(node_a));
        assert_eq!(app.get_single_selected_node_for_view(view_b), Some(node_b));

        app.set_workspace_focused_view_with_transition(Some(view_a));
        assert_eq!(app.get_single_selected_node(), Some(node_a));

        app.set_workspace_focused_view_with_transition(Some(view_b));
        assert_eq!(app.get_single_selected_node(), Some(node_b));
    }

    #[test]
    fn undo_snapshot_uses_focused_view_selection_as_active_selection() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_id = GraphViewId::new();
        app.workspace
            .views
            .insert(view_id, GraphViewState::new_with_id(view_id, "Focused"));

        let canonical = app
            .workspace
            .domain
            .graph
            .add_node("canonical".to_string(), Point2D::new(0.0, 0.0));
        let stale = app
            .workspace
            .domain
            .graph
            .add_node("stale".to_string(), Point2D::new(10.0, 0.0));

        app.set_workspace_focused_view_with_transition(Some(view_id));
        app.select_node(canonical, false);

        app.workspace
            .selection_by_scope
            .insert(SelectionScope::Unfocused, {
                let mut selection = SelectionState::new();
                selection.select(stale, false);
                selection
            });

        let snapshot = app.build_undo_redo_snapshot(None).expect("snapshot");
        assert_eq!(snapshot.active_selection.primary(), Some(canonical));
        assert!(snapshot.active_selection.contains(&canonical));
        assert!(!snapshot.active_selection.contains(&stale));
    }

    #[test]
    fn test_request_fit_to_screen() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_id = GraphViewId::new();
        app.workspace
            .views
            .insert(view_id, GraphViewState::new_with_id(view_id, "Focused"));
        app.workspace.focused_view = Some(view_id);

        app.clear_pending_camera_command();
        assert!(app.pending_camera_command().is_none());

        // Request fit to screen
        app.request_fit_to_screen();
        assert_eq!(app.pending_camera_command(), Some(CameraCommand::Fit));
        assert_eq!(app.pending_camera_command_target(), Some(view_id));

        app.clear_pending_camera_command();
        assert!(app.pending_camera_command().is_none());
    }

    #[test]
    fn test_request_fit_to_screen_falls_back_to_single_view_when_unfocused() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_id = GraphViewId::new();
        app.workspace
            .views
            .insert(view_id, GraphViewState::new_with_id(view_id, "OnlyView"));
        app.workspace.focused_view = None;

        app.clear_pending_camera_command();
        assert!(app.pending_camera_command().is_none());

        app.request_fit_to_screen();

        assert_eq!(app.pending_camera_command(), Some(CameraCommand::Fit));
        assert_eq!(app.pending_camera_command_target(), Some(view_id));
    }

    #[test]
    fn test_request_fit_to_screen_without_focus_and_multiple_views_is_noop() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_a = GraphViewId::new();
        let view_b = GraphViewId::new();
        app.workspace
            .views
            .insert(view_a, GraphViewState::new_with_id(view_a, "A"));
        app.workspace
            .views
            .insert(view_b, GraphViewState::new_with_id(view_b, "B"));
        app.workspace.focused_view = None;
        app.workspace.graph_view_frames.clear();

        app.clear_pending_camera_command();
        app.request_fit_to_screen();

        assert!(app.pending_camera_command().is_none());
        assert!(app.pending_camera_command_target().is_none());
    }

    #[test]
    fn test_request_fit_to_screen_without_focus_targets_single_rendered_view() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_a = GraphViewId::new();
        let view_b = GraphViewId::new();
        app.workspace
            .views
            .insert(view_a, GraphViewState::new_with_id(view_a, "A"));
        app.workspace
            .views
            .insert(view_b, GraphViewState::new_with_id(view_b, "B"));
        app.workspace.focused_view = None;
        app.workspace.graph_view_frames.clear();
        app.workspace.graph_view_frames.insert(
            view_b,
            GraphViewFrame {
                zoom: 1.0,
                pan_x: 0.0,
                pan_y: 0.0,
            },
        );

        app.clear_pending_camera_command();
        app.request_fit_to_screen();

        assert_eq!(app.pending_camera_command(), Some(CameraCommand::Fit));
        assert_eq!(app.pending_camera_command_target(), Some(view_b));
    }

    #[test]
    fn test_toggle_camera_fit_locks_request_fit() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_id = GraphViewId::new();
        app.workspace
            .views
            .insert(view_id, GraphViewState::new_with_id(view_id, "Focused"));
        app.workspace.focused_view = Some(view_id);
        app.clear_pending_camera_command();

        app.apply_reducer_intents([
            GraphIntent::ToggleCameraPositionFitLock,
            GraphIntent::ToggleCameraZoomFitLock,
        ]);

        assert!(app.camera_fit_locked());
        assert_eq!(app.pending_camera_command(), Some(CameraCommand::Fit));
        assert_eq!(app.pending_camera_command_target(), Some(view_id));
    }

    #[test]
    fn test_camera_locks_are_scoped_per_graph_view() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_a = GraphViewId::new();
        let view_b = GraphViewId::new();
        app.workspace
            .views
            .insert(view_a, GraphViewState::new_with_id(view_a, "A"));
        app.workspace
            .views
            .insert(view_b, GraphViewState::new_with_id(view_b, "B"));

        app.workspace.focused_view = Some(view_a);
        app.set_camera_fit_locked(true);
        assert!(app.camera_fit_locked());

        app.workspace.focused_view = Some(view_b);
        assert!(!app.camera_position_fit_locked());
        assert!(!app.camera_zoom_fit_locked());

        app.set_camera_position_fit_locked(true);
        assert!(app.camera_position_fit_locked());
        assert!(!app.camera_zoom_fit_locked());

        app.workspace.focused_view = Some(view_a);
        assert!(app.camera_position_fit_locked());
        assert!(app.camera_zoom_fit_locked());
    }

    #[test]
    fn test_unlock_camera_fit_lock_clears_pending_fit_and_restores_zoom_requests() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_id = GraphViewId::new();
        app.workspace
            .views
            .insert(view_id, GraphViewState::new_with_id(view_id, "Focused"));
        app.workspace.focused_view = Some(view_id);

        app.set_camera_fit_locked(true);
        assert_eq!(app.pending_camera_command(), Some(CameraCommand::Fit));

        app.set_camera_fit_locked(false);
        assert!(!app.camera_fit_locked());
        assert!(app.pending_camera_command().is_none());

        app.apply_reducer_intents([GraphIntent::RequestZoomIn]);
        assert_eq!(
            app.take_pending_keyboard_zoom_request(view_id),
            Some(KeyboardZoomRequest::In)
        );
    }

    #[test]
    fn test_zoom_intents_noop_when_camera_fit_lock_enabled() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_id = GraphViewId::new();
        app.workspace
            .views
            .insert(view_id, GraphViewState::new_with_id(view_id, "Focused"));
        app.workspace.focused_view = Some(view_id);

        app.set_camera_fit_locked(true);
        app.clear_pending_camera_command();

        app.apply_reducer_intents([GraphIntent::RequestZoomIn]);
        assert_eq!(app.take_pending_keyboard_zoom_request(view_id), None);
        assert_eq!(app.pending_camera_command(), Some(CameraCommand::Fit));

        app.clear_pending_camera_command();
        app.workspace
            .views
            .get_mut(&view_id)
            .unwrap()
            .camera
            .current_zoom = 2.0;
        app.apply_reducer_intents([GraphIntent::SetZoom { zoom: 0.25 }]);
        assert_eq!(
            app.workspace
                .views
                .get(&view_id)
                .unwrap()
                .camera
                .current_zoom,
            2.0
        );
    }

    #[test]
    fn test_position_fit_lock_does_not_block_manual_zoom() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_id = GraphViewId::new();
        app.workspace
            .views
            .insert(view_id, GraphViewState::new_with_id(view_id, "Focused"));
        app.workspace.focused_view = Some(view_id);

        app.set_camera_position_fit_locked(true);
        app.set_camera_zoom_fit_locked(false);
        app.clear_pending_camera_command();

        app.apply_reducer_intents([GraphIntent::RequestZoomIn]);
        assert_eq!(
            app.take_pending_keyboard_zoom_request(view_id),
            Some(KeyboardZoomRequest::In)
        );
    }

    #[test]
    fn test_zoom_fit_lock_does_not_block_manual_pan_reheat_path() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_id = GraphViewId::new();
        app.workspace
            .views
            .insert(view_id, GraphViewState::new_with_id(view_id, "Focused"));
        app.workspace.focused_view = Some(view_id);
        app.workspace.physics.base.is_running = false;

        app.set_camera_position_fit_locked(false);
        app.set_camera_zoom_fit_locked(true);

        app.set_interacting(true);
        app.set_interacting(false);

        assert!(app.workspace.physics.base.is_running);
        assert_eq!(app.workspace.drag_release_frames_remaining, 10);
    }

    #[test]
    fn test_zoom_intents_queue_keyboard_zoom_requests() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_id = GraphViewId::new();
        app.workspace
            .views
            .insert(view_id, GraphViewState::new_with_id(view_id, "Focused"));
        app.workspace.focused_view = Some(view_id);

        app.apply_reducer_intents([GraphIntent::RequestZoomIn]);
        assert_eq!(
            app.take_pending_keyboard_zoom_request(view_id),
            Some(KeyboardZoomRequest::In)
        );
        assert_eq!(app.take_pending_keyboard_zoom_request(view_id), None);

        app.apply_reducer_intents([GraphIntent::RequestZoomOut]);
        assert_eq!(
            app.take_pending_keyboard_zoom_request(view_id),
            Some(KeyboardZoomRequest::Out)
        );

        app.apply_reducer_intents([GraphIntent::RequestZoomReset]);
        assert_eq!(
            app.take_pending_keyboard_zoom_request(view_id),
            Some(KeyboardZoomRequest::Reset)
        );
    }

    #[test]
    fn test_zoom_intent_targets_single_view_without_focus() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_id = GraphViewId::new();
        app.workspace
            .views
            .insert(view_id, GraphViewState::new_with_id(view_id, "OnlyView"));
        app.workspace.focused_view = None;

        app.apply_reducer_intents([GraphIntent::RequestZoomIn]);

        assert_eq!(
            app.take_pending_keyboard_zoom_request(view_id),
            Some(KeyboardZoomRequest::In)
        );
    }

    #[test]
    fn test_restore_pending_keyboard_zoom_request_requeues_for_retry() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_id = GraphViewId::new();
        app.workspace
            .views
            .insert(view_id, GraphViewState::new_with_id(view_id, "RetryView"));
        app.workspace.focused_view = Some(view_id);

        app.apply_reducer_intents([GraphIntent::RequestZoomIn]);
        let consumed = app.take_pending_keyboard_zoom_request(view_id);
        assert_eq!(consumed, Some(KeyboardZoomRequest::In));
        assert_eq!(app.take_pending_keyboard_zoom_request(view_id), None);

        app.restore_pending_keyboard_zoom_request(view_id, KeyboardZoomRequest::In);
        assert_eq!(
            app.take_pending_keyboard_zoom_request(view_id),
            Some(KeyboardZoomRequest::In)
        );
    }

    #[test]
    fn queued_keyboard_zoom_request_replaces_previous_target() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_a = GraphViewId::new();
        let view_b = GraphViewId::new();
        app.workspace
            .views
            .insert(view_a, GraphViewState::new_with_id(view_a, "A"));
        app.workspace
            .views
            .insert(view_b, GraphViewState::new_with_id(view_b, "B"));

        app.restore_pending_keyboard_zoom_request(view_a, KeyboardZoomRequest::In);
        app.restore_pending_keyboard_zoom_request(view_b, KeyboardZoomRequest::Out);

        assert_eq!(app.take_pending_keyboard_zoom_request(view_a), None);
        assert_eq!(
            app.take_pending_keyboard_zoom_request(view_b),
            Some(KeyboardZoomRequest::Out)
        );
        assert_eq!(app.take_pending_keyboard_zoom_request(view_b), None);
    }

    #[test]
    fn test_zoom_intent_without_focus_and_multiple_views_is_noop() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_a = GraphViewId::new();
        let view_b = GraphViewId::new();
        app.workspace
            .views
            .insert(view_a, GraphViewState::new_with_id(view_a, "A"));
        app.workspace
            .views
            .insert(view_b, GraphViewState::new_with_id(view_b, "B"));
        app.workspace.focused_view = None;

        app.apply_reducer_intents([GraphIntent::RequestZoomIn]);

        assert_eq!(app.take_pending_keyboard_zoom_request(view_a), None);
        assert_eq!(app.take_pending_keyboard_zoom_request(view_b), None);
    }

    #[test]
    fn test_zoom_intent_without_focus_targets_single_rendered_view() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_a = GraphViewId::new();
        let view_b = GraphViewId::new();
        app.workspace
            .views
            .insert(view_a, GraphViewState::new_with_id(view_a, "A"));
        app.workspace
            .views
            .insert(view_b, GraphViewState::new_with_id(view_b, "B"));
        app.workspace.focused_view = None;
        app.workspace.graph_view_frames.clear();
        app.workspace.graph_view_frames.insert(
            view_b,
            GraphViewFrame {
                zoom: 1.0,
                pan_x: 0.0,
                pan_y: 0.0,
            },
        );

        app.apply_reducer_intents([GraphIntent::RequestZoomIn]);

        assert_eq!(app.take_pending_keyboard_zoom_request(view_a), None);
        assert_eq!(
            app.take_pending_keyboard_zoom_request(view_b),
            Some(KeyboardZoomRequest::In)
        );
    }

    #[test]
    fn test_zoom_to_selected_falls_back_to_fit_when_selection_empty() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_id = GraphViewId::new();
        app.workspace
            .views
            .insert(view_id, GraphViewState::new_with_id(view_id, "Focused"));
        app.workspace.focused_view = Some(view_id);
        assert!(app.focused_selection().is_empty());
        app.clear_pending_camera_command();
        assert!(app.pending_camera_command().is_none());

        app.apply_reducer_intents([GraphIntent::RequestZoomToSelected]);

        assert_eq!(app.pending_camera_command(), Some(CameraCommand::Fit));
        assert_eq!(app.pending_camera_command_target(), Some(view_id));
    }

    #[test]
    fn test_zoom_to_selected_falls_back_to_fit_when_single_selected() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_id = GraphViewId::new();
        app.workspace
            .views
            .insert(view_id, GraphViewState::new_with_id(view_id, "Focused"));
        app.workspace.focused_view = Some(view_id);
        let key = app
            .workspace
            .domain
            .graph
            .add_node("test".to_string(), Point2D::new(0.0, 0.0));
        app.select_node(key, false);
        app.clear_pending_camera_command();
        assert!(app.pending_camera_command().is_none());

        app.apply_reducer_intents([GraphIntent::RequestZoomToSelected]);

        assert_eq!(app.pending_camera_command(), Some(CameraCommand::Fit));
        assert_eq!(app.pending_camera_command_target(), Some(view_id));
    }

    #[test]
    fn test_zoom_to_selected_sets_pending_when_multi_selected() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_id = GraphViewId::new();
        app.workspace
            .views
            .insert(view_id, GraphViewState::new_with_id(view_id, "Focused"));
        app.workspace.focused_view = Some(view_id);
        let key_a = app
            .workspace
            .domain
            .graph
            .add_node("a".to_string(), Point2D::new(0.0, 0.0));
        let key_b = app
            .workspace
            .domain
            .graph
            .add_node("b".to_string(), Point2D::new(100.0, 50.0));
        app.select_node(key_a, false);
        app.select_node(key_b, true);
        assert_eq!(app.focused_selection().len(), 2);
        app.clear_pending_camera_command();
        assert!(app.pending_camera_command().is_none());

        app.apply_reducer_intents([GraphIntent::RequestZoomToSelected]);

        assert_eq!(
            app.pending_camera_command(),
            Some(CameraCommand::FitSelection)
        );
        assert_eq!(app.pending_camera_command_target(), Some(view_id));
    }

    #[test]
    fn test_zoom_to_selected_without_focus_and_multiple_views_is_noop() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_a = GraphViewId::new();
        let view_b = GraphViewId::new();
        app.workspace
            .views
            .insert(view_a, GraphViewState::new_with_id(view_a, "A"));
        app.workspace
            .views
            .insert(view_b, GraphViewState::new_with_id(view_b, "B"));
        app.workspace.focused_view = None;
        let key_a = app
            .workspace
            .domain
            .graph
            .add_node("a".to_string(), Point2D::new(0.0, 0.0));
        let key_b = app
            .workspace
            .domain
            .graph
            .add_node("b".to_string(), Point2D::new(100.0, 50.0));
        app.select_node(key_a, false);
        app.select_node(key_b, true);
        app.clear_pending_camera_command();

        app.apply_reducer_intents([GraphIntent::RequestZoomToSelected]);

        assert!(app.pending_camera_command().is_none());
        assert!(app.pending_camera_command_target().is_none());
    }

    #[test]
    fn test_request_camera_command_for_view_rejects_stale_target() {
        let mut app = GraphBrowserApp::new_for_testing();
        let stale_target = GraphViewId::new();
        app.clear_pending_camera_command();

        app.request_camera_command_for_view(Some(stale_target), CameraCommand::Fit);

        assert!(app.pending_camera_command().is_none());
        assert!(app.pending_camera_command_target_raw().is_none());
        assert!(app.pending_camera_command_target().is_none());
    }

    #[test]
    fn test_request_camera_command_for_view_accepts_valid_target() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_id = GraphViewId::new();
        app.workspace
            .views
            .insert(view_id, GraphViewState::new_with_id(view_id, "Focused"));
        app.clear_pending_camera_command();

        app.request_camera_command_for_view(Some(view_id), CameraCommand::FitSelection);

        assert_eq!(
            app.pending_camera_command(),
            Some(CameraCommand::FitSelection)
        );
        assert_eq!(app.pending_camera_command_target_raw(), Some(view_id));
        assert_eq!(app.pending_camera_command_target(), Some(view_id));
    }

    #[test]
    fn queued_camera_command_replaces_previous_target() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_a = GraphViewId::new();
        let view_b = GraphViewId::new();
        app.workspace
            .views
            .insert(view_a, GraphViewState::new_with_id(view_a, "A"));
        app.workspace
            .views
            .insert(view_b, GraphViewState::new_with_id(view_b, "B"));

        app.request_camera_command_for_view(Some(view_a), CameraCommand::Fit);
        app.request_camera_command_for_view(Some(view_b), CameraCommand::FitSelection);

        assert_eq!(
            app.pending_camera_command(),
            Some(CameraCommand::FitSelection)
        );
        assert_eq!(app.pending_camera_command_target_raw(), Some(view_b));
        assert_eq!(app.pending_camera_command_target(), Some(view_b));
    }

    #[test]
    fn test_frame_only_reducer_handles_zoom_and_selection_intents() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_id = GraphViewId::new();
        app.workspace
            .views
            .insert(view_id, GraphViewState::new_with_id(view_id, "Focused"));
        app.workspace.focused_view = Some(view_id);
        let key_a = app
            .workspace
            .domain
            .graph
            .add_node("a".to_string(), Point2D::new(0.0, 0.0));
        let key_b = app
            .workspace
            .domain
            .graph
            .add_node("b".to_string(), Point2D::new(100.0, 50.0));

        assert!(app.apply_workspace_only_intent(&GraphIntent::RequestZoomIn));
        assert_eq!(
            app.take_pending_keyboard_zoom_request(view_id),
            Some(KeyboardZoomRequest::In)
        );

        assert!(
            app.apply_workspace_only_intent(&GraphIntent::UpdateSelection {
                keys: vec![key_a, key_b],
                mode: SelectionUpdateMode::Replace,
            })
        );
        assert_eq!(app.focused_selection().len(), 2);
        assert_eq!(app.focused_selection().primary(), Some(key_b));
    }

    #[test]
    fn test_pending_wheel_zoom_delta_is_scoped_to_target_view() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_a = GraphViewId::new();
        let view_b = GraphViewId::new();

        app.workspace
            .views
            .insert(view_a, GraphViewState::new_with_id(view_a, "A"));
        app.workspace
            .views
            .insert(view_b, GraphViewState::new_with_id(view_b, "B"));

        app.queue_pending_wheel_zoom_delta(view_a, 32.0, Some((100.0, 120.0)));
        assert_eq!(app.pending_wheel_zoom_delta(view_a), 32.0);
        assert_eq!(app.pending_wheel_zoom_delta(view_b), 0.0);
        assert_eq!(
            app.pending_wheel_zoom_anchor_screen(view_a),
            Some((100.0, 120.0))
        );
        assert_eq!(app.pending_wheel_zoom_anchor_screen(view_b), None);

        app.queue_pending_wheel_zoom_delta(view_b, -12.0, Some((300.0, 240.0)));
        assert_eq!(app.pending_wheel_zoom_delta(view_a), 0.0);
        assert_eq!(app.pending_wheel_zoom_delta(view_b), -12.0);
        assert_eq!(app.pending_wheel_zoom_anchor_screen(view_a), None);
        assert_eq!(
            app.pending_wheel_zoom_anchor_screen(view_b),
            Some((300.0, 240.0))
        );
    }

    #[test]
    fn test_clear_pending_wheel_zoom_delta_clears_target() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view = GraphViewId::new();
        app.workspace
            .views
            .insert(view, GraphViewState::new_with_id(view, "A"));

        app.queue_pending_wheel_zoom_delta(view, 24.0, Some((150.0, 80.0)));
        assert_eq!(app.pending_wheel_zoom_delta(view), 24.0);
        assert_eq!(
            app.pending_wheel_zoom_anchor_screen(view),
            Some((150.0, 80.0))
        );

        app.clear_pending_wheel_zoom_delta();
        assert_eq!(app.pending_wheel_zoom_delta(view), 0.0);
        assert_eq!(app.pending_wheel_zoom_anchor_screen(view), None);
    }

    #[test]
    fn test_pending_wheel_zoom_anchor_is_retained_when_followup_delta_has_no_pointer() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view = GraphViewId::new();

        app.workspace
            .views
            .insert(view, GraphViewState::new_with_id(view, "A"));

        app.queue_pending_wheel_zoom_delta(view, 20.0, Some((40.0, 55.0)));
        app.queue_pending_wheel_zoom_delta(view, 10.0, None);

        assert_eq!(app.pending_wheel_zoom_delta(view), 30.0);
        assert_eq!(
            app.pending_wheel_zoom_anchor_screen(view),
            Some((40.0, 55.0))
        );
    }

    #[test]
    fn test_pending_wheel_zoom_anchor_updates_when_new_pointer_is_provided() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view = GraphViewId::new();

        app.workspace
            .views
            .insert(view, GraphViewState::new_with_id(view, "A"));

        app.queue_pending_wheel_zoom_delta(view, 15.0, Some((10.0, 20.0)));
        app.queue_pending_wheel_zoom_delta(view, 5.0, Some((90.0, 120.0)));

        assert_eq!(app.pending_wheel_zoom_delta(view), 20.0);
        assert_eq!(
            app.pending_wheel_zoom_anchor_screen(view),
            Some((90.0, 120.0))
        );
    }

    #[test]
    fn test_pending_wheel_zoom_anchor_clears_when_target_view_changes_without_anchor() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_a = GraphViewId::new();
        let view_b = GraphViewId::new();

        app.workspace
            .views
            .insert(view_a, GraphViewState::new_with_id(view_a, "A"));
        app.workspace
            .views
            .insert(view_b, GraphViewState::new_with_id(view_b, "B"));

        app.queue_pending_wheel_zoom_delta(view_a, 10.0, Some((25.0, 35.0)));
        assert_eq!(
            app.pending_wheel_zoom_anchor_screen(view_a),
            Some((25.0, 35.0))
        );

        app.queue_pending_wheel_zoom_delta(view_b, 6.0, None);

        assert_eq!(app.pending_wheel_zoom_delta(view_a), 0.0);
        assert_eq!(app.pending_wheel_zoom_anchor_screen(view_a), None);
        assert_eq!(app.pending_wheel_zoom_delta(view_b), 6.0);
        assert_eq!(app.pending_wheel_zoom_anchor_screen(view_b), None);
    }

    #[test]
    fn test_frame_only_reducer_excludes_verse_side_effect_intents() {
        let mut app = GraphBrowserApp::new_for_testing();

        assert!(!app.apply_workspace_only_intent(&GraphIntent::SyncNow));
        assert!(
            !app.apply_workspace_only_intent(&GraphIntent::ForgetDevice {
                peer_id: "peer".to_string(),
            })
        );
        assert!(
            !app.apply_workspace_only_intent(&GraphIntent::RevokeWorkspaceAccess {
                peer_id: "peer".to_string(),
                workspace_id: "workspace".to_string(),
            })
        );
    }

    #[test]
    fn graph_intent_category_helpers_expose_view_runtime_and_mutation_seams() {
        assert!(GraphIntent::RequestZoomIn.as_view_action().is_some());
        assert!(GraphIntent::RequestZoomIn.as_runtime_event().is_none());
        assert!(GraphIntent::RequestZoomIn.as_graph_mutation().is_none());

        assert!(GraphIntent::SyncNow.as_runtime_event().is_some());
        assert!(GraphIntent::SyncNow.as_view_action().is_none());
        assert!(GraphIntent::SyncNow.as_graph_mutation().is_none());

        assert!(
            GraphIntent::CreateNodeNearCenter
                .as_graph_mutation()
                .is_some()
        );
        assert!(GraphIntent::CreateNodeNearCenter.as_view_action().is_none());
        assert!(
            GraphIntent::CreateNodeNearCenter
                .as_runtime_event()
                .is_none()
        );
    }

    #[test]
    fn app_command_queue_handles_non_snapshot_requests() {
        let mut app = GraphBrowserApp::new_for_testing();
        let note_id = NoteId::new();

        app.request_open_note_by_id(note_id);
        app.request_open_clip_by_id("clip-queue");
        app.request_prune_empty_workspaces();
        app.request_keep_latest_named_workspaces(3);
        app.request_switch_data_dir("C:/graphshell-data");

        assert_eq!(app.take_pending_open_note_request(), Some(note_id));
        assert_eq!(
            app.take_pending_open_clip_request().as_deref(),
            Some("clip-queue")
        );
        assert!(app.take_pending_prune_empty_workspaces());
        assert_eq!(app.take_pending_keep_latest_named_workspaces(), Some(3));
        assert_eq!(
            app.take_pending_switch_data_dir(),
            Some(PathBuf::from("C:/graphshell-data"))
        );
    }

    #[test]
    fn apply_view_actions_dispatches_without_graph_intent_wrapper() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_id = GraphViewId::new();
        app.workspace
            .views
            .insert(view_id, GraphViewState::new_with_id(view_id, "Focused"));
        app.workspace.focused_view = Some(view_id);

        app.apply_view_actions([ViewAction::RequestZoomIn]);

        assert_eq!(
            app.take_pending_keyboard_zoom_request(view_id),
            Some(KeyboardZoomRequest::In)
        );
    }

    #[test]
    fn apply_runtime_events_dispatches_without_graph_intent_wrapper() {
        use crate::graph::NodeLifecycle;

        let mut app = GraphBrowserApp::new_for_testing();
        let key = app.workspace.domain.graph.add_node(
            "https://runtime.example".to_string(),
            Point2D::new(0.0, 0.0),
        );

        app.apply_runtime_events([RuntimeEvent::PromoteNodeToActive {
            key,
            cause: LifecycleCause::Restore,
        }]);

        assert_eq!(
            app.workspace.domain.graph.get_node(key).unwrap().lifecycle,
            NodeLifecycle::Active
        );
    }

    #[test]
    fn contract_only_trusted_writers_call_graph_topology_mutators() {
        const FORBIDDEN_TOKENS: [&str; 11] = [
            "graph.add_node(",
            "graph.remove_node(",
            "graph.add_edge(",
            "graph.remove_edges(",
            "graph.inner.",
            "graph.get_node_mut(",
            ".add_node_and_sync(",
            ".add_edge_and_sync(",
            ".capture_undo_checkpoint(",
            ".perform_undo(",
            ".perform_redo(",
        ];
        const PERSISTENCE_DURABLE_ESCAPE_HATCH_TOKENS: [&str; 3] = [
            "graph.get_node_mut(",
            "graph.get_edge_mut(",
            "graph.update_node_url(",
        ];
        const RENDER_DURABLE_POSITION_ESCAPE_HATCH_TOKENS: [&str; 1] = ["graph.set_node_position("];
        const RENDER_DOMAIN_BRIDGE_ESCAPE_HATCH_TOKENS: [&str; 2] =
            ["workspace.domain.graph.", "workspace\n        .graph"];
        const RENDER_PROJECTED_READ_ESCAPE_HATCH_TOKENS: [&str; 1] = ["node.position"];
        const PROJECTED_READ_ESCAPE_HATCH_TOKENS: [&str; 1] = ["node.position"];
        const WORKBENCH_DOMAIN_BRIDGE_ESCAPE_HATCH_TOKENS: [&str; 1] = ["workspace.domain.graph"];
        const LIFECYCLE_DOMAIN_BRIDGE_ESCAPE_HATCH_TOKENS: [&str; 1] = ["workspace.domain.graph"];
        const ACTION_DOMAIN_BRIDGE_ESCAPE_HATCH_TOKENS: [&str; 1] = ["workspace.domain.graph"];
        const GUI_FRAME_DOMAIN_BRIDGE_ESCAPE_HATCH_TOKENS: [&str; 1] = ["workspace.domain.graph"];
        const COMMAND_PALETTE_DOMAIN_BRIDGE_ESCAPE_HATCH_TOKENS: [&str; 1] =
            ["workspace.domain.graph"];
        const GUI_ORCHESTRATION_DOMAIN_BRIDGE_ESCAPE_HATCH_TOKENS: [&str; 1] =
            ["workspace.domain.graph"];
        const TOOLBAR_OMNIBAR_DOMAIN_BRIDGE_ESCAPE_HATCH_TOKENS: [&str; 1] =
            ["workspace.domain.graph"];
        const THUMBNAIL_PIPELINE_DOMAIN_BRIDGE_ESCAPE_HATCH_TOKENS: [&str; 1] =
            ["workspace.domain.graph"];
        const GUI_DOMAIN_BRIDGE_ESCAPE_HATCH_TOKENS: [&str; 1] = ["workspace.domain.graph"];
        const PERSISTENCE_OPS_DOMAIN_BRIDGE_ESCAPE_HATCH_TOKENS: [&str; 1] =
            ["workspace.domain.graph"];

        let persistence_runtime_only = include_str!("services/persistence/mod.rs")
            .split("\n#[cfg(test)]")
            .next()
            .unwrap_or_default();
        let webview_controller_runtime_only =
            include_str!("shell/desktop/lifecycle/webview_controller.rs")
                .split("\n#[cfg(test)]")
                .next()
                .unwrap_or_default();
        let webview_backpressure_runtime_only =
            include_str!("shell/desktop/lifecycle/webview_backpressure.rs")
                .split("\n#[cfg(test)]")
                .next()
                .unwrap_or_default();
        let lifecycle_reconcile_runtime_only =
            include_str!("shell/desktop/lifecycle/lifecycle_reconcile.rs")
                .split("\n#[cfg(test)]")
                .next()
                .unwrap_or_default();
        let render_runtime_only = include_str!("render/mod.rs")
            .split("\n#[cfg(test)]")
            .next()
            .unwrap_or_default();
        let action_registry_runtime_only =
            include_str!("shell/desktop/runtime/registries/action.rs")
                .split("\n#[cfg(test)]")
                .next()
                .unwrap_or_default();
        let runtime_registries_runtime_only =
            include_str!("shell/desktop/runtime/registries/mod.rs")
                .split("\n#[cfg(test)]")
                .next()
                .unwrap_or_default();
        let gui_runtime_only = include_str!("shell/desktop/ui/gui.rs")
            .split("\n#[cfg(test)]")
            .next()
            .unwrap_or_default();
        let gui_frame_runtime_only = include_str!("shell/desktop/ui/gui_frame.rs")
            .split("\n#[cfg(test)]")
            .next()
            .unwrap_or_default();
        let gui_orchestration_runtime_only = include_str!("shell/desktop/ui/gui_orchestration.rs")
            .split("\n#[cfg(test)]")
            .next()
            .unwrap_or_default();
        let persistence_ops_runtime_only = include_str!("shell/desktop/ui/persistence_ops.rs")
            .split("\n#[cfg(test)]")
            .next()
            .unwrap_or_default();
        let thumbnail_pipeline_runtime_only =
            include_str!("shell/desktop/ui/thumbnail_pipeline.rs")
                .split("\n#[cfg(test)]")
                .next()
                .unwrap_or_default();
        let command_palette_runtime_only = include_str!("render/command_palette.rs")
            .split("\n#[cfg(test)]")
            .next()
            .unwrap_or_default();
        let tile_behavior_runtime_only = include_str!("shell/desktop/workbench/tile_behavior.rs")
            .split("\n#[cfg(test)]")
            .next()
            .unwrap_or_default();
        let tile_runtime_runtime_only = include_str!("shell/desktop/workbench/tile_runtime.rs")
            .split("\n#[cfg(test)]")
            .next()
            .unwrap_or_default();
        let tile_invariants_runtime_only =
            include_str!("shell/desktop/workbench/tile_invariants.rs")
                .split("\n#[cfg(test)]")
                .next()
                .unwrap_or_default();

        let guarded_sources = [
            (
                "shell/desktop/host/running_app_state.rs",
                include_str!("shell/desktop/host/running_app_state.rs"),
            ),
            (
                "shell/desktop/host/window.rs",
                include_str!("shell/desktop/host/window.rs"),
            ),
            (
                "shell/desktop/lifecycle/lifecycle_reconcile.rs",
                include_str!("shell/desktop/lifecycle/lifecycle_reconcile.rs"),
            ),
            (
                "shell/desktop/lifecycle/webview_controller.rs (runtime section)",
                webview_controller_runtime_only,
            ),
            (
                "shell/desktop/lifecycle/semantic_event_pipeline.rs",
                include_str!("shell/desktop/lifecycle/semantic_event_pipeline.rs"),
            ),
            (
                "shell/desktop/host/event_loop.rs",
                include_str!("shell/desktop/host/event_loop.rs"),
            ),
            (
                "shell/desktop/runtime/registries/action.rs (runtime section)",
                action_registry_runtime_only,
            ),
            (
                "shell/desktop/runtime/registries/mod.rs (runtime section)",
                runtime_registries_runtime_only,
            ),
            ("render/mod.rs (runtime section)", render_runtime_only),
            (
                "shell/desktop/ui/gui.rs (runtime section)",
                gui_runtime_only,
            ),
            (
                "shell/desktop/ui/gui_frame.rs (runtime section)",
                gui_frame_runtime_only,
            ),
            (
                "shell/desktop/ui/gui_orchestration.rs (runtime section)",
                gui_orchestration_runtime_only,
            ),
            (
                "services/persistence/mod.rs (runtime section)",
                persistence_runtime_only,
            ),
        ];

        for (path, source) in guarded_sources {
            for token in FORBIDDEN_TOKENS {
                assert!(
                    !source.contains(token),
                    "trusted-writer boundary violated in {path}: found '{token}'"
                );
            }
        }

        for token in PERSISTENCE_DURABLE_ESCAPE_HATCH_TOKENS {
            assert!(
                !persistence_runtime_only.contains(token),
                "trusted-writer boundary violated in services/persistence/mod.rs (runtime section): found '{token}'"
            );
        }

        for token in RENDER_DURABLE_POSITION_ESCAPE_HATCH_TOKENS {
            assert!(
                !render_runtime_only.contains(token),
                "trusted-writer boundary violated in render/mod.rs (runtime section): found '{token}'"
            );
        }

        for token in RENDER_DOMAIN_BRIDGE_ESCAPE_HATCH_TOKENS {
            assert!(
                !render_runtime_only.contains(token),
                "domain-state CLAT boundary violated in render/mod.rs (runtime section): found '{token}'"
            );
        }

        for token in RENDER_PROJECTED_READ_ESCAPE_HATCH_TOKENS {
            assert!(
                !render_runtime_only.contains(token),
                "projected-position boundary violated in render/mod.rs (runtime section): found '{token}'"
            );
        }

        for (path, source) in [
            (
                "graph_app.rs",
                include_str!("graph_app.rs")
                    .split("\n#[cfg(test)]")
                    .next()
                    .unwrap_or_default(),
            ),
            (
                "shell/desktop/lifecycle/webview_controller.rs (runtime section)",
                webview_controller_runtime_only,
            ),
            (
                "shell/desktop/runtime/registries/action.rs (runtime section)",
                action_registry_runtime_only,
            ),
            (
                "shell/desktop/ui/toolbar/toolbar_omnibar.rs",
                include_str!("shell/desktop/ui/toolbar/toolbar_omnibar.rs"),
            ),
        ] {
            for token in PROJECTED_READ_ESCAPE_HATCH_TOKENS {
                assert!(
                    !source.contains(token),
                    "projected-position boundary violated in {path}: found '{token}'"
                );
            }
        }

        for (path, source) in [
            (
                "shell/desktop/lifecycle/lifecycle_reconcile.rs (runtime section)",
                lifecycle_reconcile_runtime_only,
            ),
            (
                "shell/desktop/lifecycle/webview_backpressure.rs (runtime section)",
                webview_backpressure_runtime_only,
            ),
            (
                "shell/desktop/lifecycle/webview_controller.rs (runtime section)",
                webview_controller_runtime_only,
            ),
            (
                "shell/desktop/workbench/tile_behavior.rs (runtime section)",
                tile_behavior_runtime_only,
            ),
            (
                "shell/desktop/workbench/tile_runtime.rs (runtime section)",
                tile_runtime_runtime_only,
            ),
            (
                "shell/desktop/workbench/tile_invariants.rs (runtime section)",
                tile_invariants_runtime_only,
            ),
            (
                "shell/desktop/workbench/tile_view_ops.rs",
                include_str!("shell/desktop/workbench/tile_view_ops.rs"),
            ),
            (
                "shell/desktop/workbench/ux_tree.rs",
                include_str!("shell/desktop/workbench/ux_tree.rs"),
            ),
            (
                "shell/desktop/workbench/pane_model.rs",
                include_str!("shell/desktop/workbench/pane_model.rs"),
            ),
        ] {
            for token in WORKBENCH_DOMAIN_BRIDGE_ESCAPE_HATCH_TOKENS {
                assert!(
                    !source.contains(token),
                    "domain-state CLAT boundary violated in {path}: found '{token}'"
                );
            }
        }

        for (path, source) in [
            (
                "shell/desktop/lifecycle/lifecycle_reconcile.rs (runtime section)",
                lifecycle_reconcile_runtime_only,
            ),
            (
                "shell/desktop/lifecycle/webview_backpressure.rs (runtime section)",
                webview_backpressure_runtime_only,
            ),
            (
                "shell/desktop/lifecycle/webview_controller.rs (runtime section)",
                webview_controller_runtime_only,
            ),
        ] {
            for token in LIFECYCLE_DOMAIN_BRIDGE_ESCAPE_HATCH_TOKENS {
                assert!(
                    !source.contains(token),
                    "domain-state CLAT boundary violated in {path}: found '{token}'"
                );
            }
        }

        for (path, source) in [(
            "shell/desktop/runtime/registries/action.rs (runtime section)",
            action_registry_runtime_only,
        )] {
            for token in ACTION_DOMAIN_BRIDGE_ESCAPE_HATCH_TOKENS {
                assert!(
                    !source.contains(token),
                    "domain-state CLAT boundary violated in {path}: found '{token}'"
                );
            }
        }

        for (path, source) in [(
            "shell/desktop/ui/gui_frame.rs (runtime section)",
            gui_frame_runtime_only,
        )] {
            for token in GUI_FRAME_DOMAIN_BRIDGE_ESCAPE_HATCH_TOKENS {
                assert!(
                    !source.contains(token),
                    "domain-state CLAT boundary violated in {path}: found '{token}'"
                );
            }
        }

        for (path, source) in [(
            "render/command_palette.rs (runtime section)",
            command_palette_runtime_only,
        )] {
            for token in COMMAND_PALETTE_DOMAIN_BRIDGE_ESCAPE_HATCH_TOKENS {
                assert!(
                    !source.contains(token),
                    "domain-state CLAT boundary violated in {path}: found '{token}'"
                );
            }
        }

        for (path, source) in [(
            "shell/desktop/ui/gui_orchestration.rs (runtime section)",
            gui_orchestration_runtime_only,
        )] {
            for token in GUI_ORCHESTRATION_DOMAIN_BRIDGE_ESCAPE_HATCH_TOKENS {
                assert!(
                    !source.contains(token),
                    "domain-state CLAT boundary violated in {path}: found '{token}'"
                );
            }
        }

        for (path, source) in [(
            "shell/desktop/ui/toolbar/toolbar_omnibar.rs",
            include_str!("shell/desktop/ui/toolbar/toolbar_omnibar.rs"),
        )] {
            for token in TOOLBAR_OMNIBAR_DOMAIN_BRIDGE_ESCAPE_HATCH_TOKENS {
                assert!(
                    !source.contains(token),
                    "domain-state CLAT boundary violated in {path}: found '{token}'"
                );
            }
        }

        for (path, source) in [(
            "shell/desktop/ui/thumbnail_pipeline.rs (runtime section)",
            thumbnail_pipeline_runtime_only,
        )] {
            for token in THUMBNAIL_PIPELINE_DOMAIN_BRIDGE_ESCAPE_HATCH_TOKENS {
                assert!(
                    !source.contains(token),
                    "domain-state CLAT boundary violated in {path}: found '{token}'"
                );
            }
        }

        for (path, source) in [(
            "shell/desktop/ui/gui.rs (runtime section)",
            gui_runtime_only,
        )] {
            for token in GUI_DOMAIN_BRIDGE_ESCAPE_HATCH_TOKENS {
                assert!(
                    !source.contains(token),
                    "domain-state CLAT boundary violated in {path}: found '{token}'"
                );
            }
        }

        for (path, source) in [(
            "shell/desktop/ui/persistence_ops.rs (runtime section)",
            persistence_ops_runtime_only,
        )] {
            for token in PERSISTENCE_OPS_DOMAIN_BRIDGE_ESCAPE_HATCH_TOKENS {
                assert!(
                    !source.contains(token),
                    "domain-state CLAT boundary violated in {path}: found '{token}'"
                );
            }
        }
    }

    #[test]
    fn test_select_node_single() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("test".to_string(), Point2D::new(0.0, 0.0));

        app.select_node(key, false);

        assert_eq!(app.focused_selection().len(), 1);
        assert!(app.focused_selection().contains(&key));
    }

    #[test]
    fn test_select_node_single_click_selected_toggles_off() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("test".to_string(), Point2D::new(0.0, 0.0));

        app.select_node(key, false);
        assert_eq!(app.focused_selection().primary(), Some(key));

        app.select_node(key, false);
        assert!(app.focused_selection().is_empty());
        assert_eq!(app.focused_selection().primary(), None);
    }

    #[test]
    fn test_select_node_multi() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key1 = app
            .workspace
            .domain
            .graph
            .add_node("a".to_string(), Point2D::new(0.0, 0.0));
        let key2 = app
            .workspace
            .domain
            .graph
            .add_node("b".to_string(), Point2D::new(100.0, 0.0));

        app.select_node(key1, false);
        app.select_node(key2, true);

        assert_eq!(app.focused_selection().len(), 2);
        assert!(app.focused_selection().contains(&key1));
        assert!(app.focused_selection().contains(&key2));
    }

    #[test]
    fn test_select_node_multi_click_selected_toggles_off() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key1 = app
            .workspace
            .domain
            .graph
            .add_node("a".to_string(), Point2D::new(0.0, 0.0));
        let key2 = app
            .workspace
            .domain
            .graph
            .add_node("b".to_string(), Point2D::new(100.0, 0.0));

        app.select_node(key1, false);
        app.select_node(key2, true);
        assert_eq!(app.focused_selection().len(), 2);
        assert_eq!(app.focused_selection().primary(), Some(key2));

        // Ctrl-click selected node toggles it off.
        app.select_node(key2, true);
        assert_eq!(app.focused_selection().len(), 1);
        assert!(app.focused_selection().contains(&key1));
        assert!(!app.focused_selection().contains(&key2));
        assert_eq!(app.focused_selection().primary(), Some(key1));
    }

    #[test]
    fn test_select_node_multi_click_only_selected_clears_selection() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("a".to_string(), Point2D::new(0.0, 0.0));

        app.select_node(key, false);
        assert_eq!(app.focused_selection().primary(), Some(key));

        // Ctrl-click selected single node toggles it off, clearing selection.
        app.select_node(key, true);
        assert!(app.focused_selection().is_empty());
        assert_eq!(app.focused_selection().primary(), None);
    }

    #[test]
    fn test_select_node_intent_single_prewarms_cold_node() {
        use crate::graph::NodeLifecycle;

        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("a".to_string(), Point2D::new(0.0, 0.0));
        assert_eq!(
            app.workspace.domain.graph.get_node(key).unwrap().lifecycle,
            NodeLifecycle::Cold
        );

        app.apply_reducer_intents([GraphIntent::SelectNode {
            key,
            multi_select: false,
        }]);

        assert_eq!(
            app.workspace.domain.graph.get_node(key).unwrap().lifecycle,
            NodeLifecycle::Active
        );
    }

    #[test]
    fn test_select_node_intent_toggle_off_does_not_prewarm() {
        use crate::graph::NodeLifecycle;

        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("a".to_string(), Point2D::new(0.0, 0.0));

        app.apply_reducer_intents([GraphIntent::SelectNode {
            key,
            multi_select: false,
        }]);
        app.demote_node_to_cold(key);
        assert_eq!(
            app.workspace.domain.graph.get_node(key).unwrap().lifecycle,
            NodeLifecycle::Cold
        );

        // Clicking the already-selected node toggles it off and should not re-promote.
        app.apply_reducer_intents([GraphIntent::SelectNode {
            key,
            multi_select: false,
        }]);

        assert!(app.focused_selection().is_empty());
        assert_eq!(
            app.workspace.domain.graph.get_node(key).unwrap().lifecycle,
            NodeLifecycle::Cold
        );
    }

    #[test]
    fn test_select_node_intent_multiselect_does_not_prewarm_cold_node() {
        use crate::graph::NodeLifecycle;

        let mut app = GraphBrowserApp::new_for_testing();
        let key1 = app
            .workspace
            .domain
            .graph
            .add_node("a".to_string(), Point2D::new(0.0, 0.0));
        let key2 = app
            .workspace
            .domain
            .graph
            .add_node("b".to_string(), Point2D::new(10.0, 0.0));

        app.apply_reducer_intents([GraphIntent::SelectNode {
            key: key1,
            multi_select: false,
        }]);
        app.demote_node_to_cold(key1);
        assert_eq!(
            app.workspace.domain.graph.get_node(key1).unwrap().lifecycle,
            NodeLifecycle::Cold
        );
        assert_eq!(
            app.workspace.domain.graph.get_node(key2).unwrap().lifecycle,
            NodeLifecycle::Cold
        );

        app.apply_reducer_intents([GraphIntent::SelectNode {
            key: key2,
            multi_select: true,
        }]);

        assert_eq!(
            app.workspace.domain.graph.get_node(key2).unwrap().lifecycle,
            NodeLifecycle::Cold
        );
    }

    #[test]
    fn test_select_node_intent_does_not_prewarm_crashed_node() {
        use crate::graph::NodeLifecycle;

        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("a".to_string(), Point2D::new(0.0, 0.0));
        let webview_id = test_webview_id();
        app.map_webview_to_node(webview_id, key);
        app.apply_reducer_intents([GraphIntent::WebViewCrashed {
            webview_id,
            reason: "boom".to_string(),
            has_backtrace: false,
        }]);
        assert_eq!(
            app.workspace.domain.graph.get_node(key).unwrap().lifecycle,
            NodeLifecycle::Cold
        );
        assert!(app.runtime_crash_state_for_node(key).is_some());

        app.apply_reducer_intents([GraphIntent::SelectNode {
            key,
            multi_select: false,
        }]);

        assert_eq!(
            app.workspace.domain.graph.get_node(key).unwrap().lifecycle,
            NodeLifecycle::Cold
        );
    }

    #[test]
    fn test_selection_revision_increments_on_change() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key1 = app
            .workspace
            .domain
            .graph
            .add_node("a".to_string(), Point2D::new(0.0, 0.0));
        let key2 = app
            .workspace
            .domain
            .graph
            .add_node("b".to_string(), Point2D::new(1.0, 0.0));
        let rev0 = app.focused_selection().revision();

        app.select_node(key1, false);
        let rev1 = app.focused_selection().revision();
        assert!(rev1 > rev0);

        app.select_node(key1, false);
        let rev2 = app.focused_selection().revision();
        assert!(rev2 > rev1);
        assert!(app.focused_selection().is_empty());

        app.select_node(key2, true);
        let rev3 = app.focused_selection().revision();
        assert!(rev3 > rev2);

        app.select_node(key2, true);
        let rev4 = app.focused_selection().revision();
        assert!(rev4 > rev3);
    }

    #[test]
    fn test_update_selection_replace_sets_exact_members() {
        let mut app = GraphBrowserApp::new_for_testing();
        let a = app.add_node_and_sync("a".to_string(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("b".to_string(), Point2D::new(10.0, 0.0));
        let c = app.add_node_and_sync("c".to_string(), Point2D::new(20.0, 0.0));
        app.select_node(a, false);

        app.apply_reducer_intents([GraphIntent::UpdateSelection {
            keys: vec![b, c],
            mode: SelectionUpdateMode::Replace,
        }]);

        assert_eq!(app.focused_selection().len(), 2);
        assert!(!app.focused_selection().contains(&a));
        assert!(app.focused_selection().contains(&b));
        assert!(app.focused_selection().contains(&c));
        assert_eq!(app.focused_selection().primary(), Some(c));
    }

    #[test]
    fn test_update_selection_add_and_toggle() {
        let mut app = GraphBrowserApp::new_for_testing();
        let a = app.add_node_and_sync("a".to_string(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("b".to_string(), Point2D::new(10.0, 0.0));
        app.apply_reducer_intents([GraphIntent::UpdateSelection {
            keys: vec![a],
            mode: SelectionUpdateMode::Replace,
        }]);
        app.apply_reducer_intents([GraphIntent::UpdateSelection {
            keys: vec![b],
            mode: SelectionUpdateMode::Add,
        }]);
        assert!(app.focused_selection().contains(&a));
        assert!(app.focused_selection().contains(&b));
        assert_eq!(app.focused_selection().primary(), Some(b));

        app.apply_reducer_intents([GraphIntent::UpdateSelection {
            keys: vec![a],
            mode: SelectionUpdateMode::Toggle,
        }]);
        assert!(!app.focused_selection().contains(&a));
        assert!(app.focused_selection().contains(&b));
    }

    #[test]
    fn test_intent_webview_created_links_parent_without_direct_selection_mutation() {
        let mut app = GraphBrowserApp::new_for_testing();
        let parent = app
            .workspace
            .domain
            .graph
            .add_node("https://parent.com".into(), Point2D::new(10.0, 20.0));
        let parent_wv = test_webview_id();
        let child_wv = test_webview_id();
        app.map_webview_to_node(parent_wv, parent);

        let edges_before = app.workspace.domain.graph.edge_count();
        app.apply_reducer_intents([GraphIntent::WebViewCreated {
            parent_webview_id: parent_wv,
            child_webview_id: child_wv,
            initial_url: Some("https://child.com".into()),
        }]);

        assert_eq!(app.workspace.domain.graph.edge_count(), edges_before + 1);
        let child = app.get_node_for_webview(child_wv).unwrap();
        assert_eq!(app.get_single_selected_node(), None);
        assert_eq!(
            app.workspace.domain.graph.get_node(child).unwrap().url,
            "https://child.com"
        );
    }

    #[test]
    fn test_intent_webview_created_places_child_near_parent() {
        let mut app = GraphBrowserApp::new_for_testing();
        let parent = app
            .workspace
            .domain
            .graph
            .add_node("https://parent.com".into(), Point2D::new(10.0, 20.0));
        let parent_wv = test_webview_id();
        let child_wv = test_webview_id();
        app.map_webview_to_node(parent_wv, parent);

        app.apply_reducer_intents([GraphIntent::WebViewCreated {
            parent_webview_id: parent_wv,
            child_webview_id: child_wv,
            initial_url: Some("https://child.com".into()),
        }]);

        let child = app.get_node_for_webview(child_wv).unwrap();
        let child_pos = app
            .workspace
            .domain
            .graph
            .get_node(child)
            .unwrap()
            .projected_position();
        // Child should be placed near the parent (not at fallback center 400, 300).
        // The base offset is (+140, +80) plus jitter in [-50, +50].
        // So x is in [100, 200] and y is in [50, 150] relative to parent at (10, 20).
        assert!(child_pos.x >= 10.0 + 140.0 - 50.0 && child_pos.x <= 10.0 + 140.0 + 50.0);
        assert!(child_pos.y >= 20.0 + 80.0 - 50.0 && child_pos.y <= 20.0 + 80.0 + 50.0);
    }

    #[test]
    fn test_intent_webview_created_about_blank_uses_placeholder() {
        let mut app = GraphBrowserApp::new_for_testing();
        let child_wv = test_webview_id();

        app.apply_reducer_intents([GraphIntent::WebViewCreated {
            parent_webview_id: test_webview_id(),
            child_webview_id: child_wv,
            initial_url: Some("about:blank".into()),
        }]);

        let child = app.get_node_for_webview(child_wv).unwrap();
        assert!(
            app.workspace
                .domain
                .graph
                .get_node(child)
                .unwrap()
                .url
                .starts_with("about:blank#")
        );
    }

    #[test]
    fn test_intent_webview_url_changed_updates_existing_mapping() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("https://before.com".into(), Point2D::new(0.0, 0.0));
        let wv = test_webview_id();
        app.map_webview_to_node(wv, key);

        app.apply_reducer_intents([GraphIntent::WebViewUrlChanged {
            webview_id: wv,
            new_url: "https://after.com".into(),
        }]);

        assert_eq!(
            app.workspace.domain.graph.get_node(key).unwrap().url,
            "https://after.com"
        );
        assert_eq!(app.get_node_for_webview(wv), Some(key));
    }

    #[test]
    fn test_webview_url_changed_appends_traversal_between_known_nodes() {
        // Navigating from a known node (a) to another known node (b) via WebViewUrlChanged
        // must append a traversal on the a→b edge. The prior URL must be captured BEFORE
        // update_node_url_and_log overwrites it; otherwise the traversal would be recorded
        // on the wrong edge (b→b self-loop) rather than the correct a→b edge.
        let mut app = GraphBrowserApp::new_for_testing();
        let a = app
            .workspace
            .domain
            .graph
            .add_node("https://a.com".into(), Point2D::new(0.0, 0.0));
        let b = app
            .workspace
            .domain
            .graph
            .add_node("https://b.com".into(), Point2D::new(100.0, 0.0));
        let wv = test_webview_id();
        app.map_webview_to_node(wv, a);

        app.apply_reducer_intents([GraphIntent::WebViewUrlChanged {
            webview_id: wv,
            new_url: "https://b.com".into(),
        }]);

        let edge_key = app
            .workspace
            .domain
            .graph
            .find_edge_key(a, b)
            .expect("traversal edge from a to b should exist");
        let payload = app.workspace.domain.graph.get_edge(edge_key).unwrap();
        assert_eq!(payload.traversals().len(), 1);
        assert_eq!(payload.traversals()[0].trigger, NavigationTrigger::Unknown);
        // No self-loop on b — confirms prior URL was captured before mutation.
        assert!(app.workspace.domain.graph.find_edge_key(b, b).is_none());
    }

    #[test]
    fn test_webview_url_changed_self_loop_navigation_does_not_append_traversal() {
        let mut app = GraphBrowserApp::new_for_testing();
        let a = app
            .workspace
            .domain
            .graph
            .add_node("https://a.com".into(), Point2D::new(0.0, 0.0));
        let wv = test_webview_id();
        app.map_webview_to_node(wv, a);

        app.apply_reducer_intents([GraphIntent::WebViewUrlChanged {
            webview_id: wv,
            new_url: "https://a.com".into(),
        }]);

        let history_edge_count = app
            .workspace
            .domain
            .graph
            .edges()
            .filter(|e| e.edge_type == EdgeType::History)
            .count();
        assert_eq!(history_edge_count, 0);
    }

    #[test]
    fn test_intent_webview_url_changed_ignores_unmapped_webview() {
        let mut app = GraphBrowserApp::new_for_testing();
        let wv = test_webview_id();
        let before = app.workspace.domain.graph.node_count();

        app.apply_reducer_intents([GraphIntent::WebViewUrlChanged {
            webview_id: wv,
            new_url: "https://ignored.com".into(),
        }]);

        assert_eq!(app.workspace.domain.graph.node_count(), before);
        assert_eq!(app.get_node_for_webview(wv), None);
    }

    #[test]
    fn test_intent_webview_history_changed_clamps_index() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("https://a.com".into(), Point2D::new(0.0, 0.0));
        let wv = test_webview_id();
        app.map_webview_to_node(wv, key);

        app.apply_reducer_intents([GraphIntent::WebViewHistoryChanged {
            webview_id: wv,
            entries: vec!["https://a.com".into(), "https://b.com".into()],
            current: 99,
        }]);

        let node = app.workspace.domain.graph.get_node(key).unwrap();
        assert_eq!(node.history_entries.len(), 2);
        assert_eq!(node.history_index, 1);
    }

    #[test]
    fn test_intent_webview_scroll_changed_updates_node_session_scroll() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("https://a.com".into(), Point2D::new(0.0, 0.0));
        let wv = test_webview_id();
        app.map_webview_to_node(wv, key);

        app.apply_reducer_intents([GraphIntent::WebViewScrollChanged {
            webview_id: wv,
            scroll_x: 15.0,
            scroll_y: 320.0,
        }]);

        let node = app.workspace.domain.graph.get_node(key).unwrap();
        assert_eq!(node.session_scroll, Some((15.0, 320.0)));
    }

    #[test]
    fn test_form_draft_restore_feature_flag_guarded() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("https://a.com".into(), Point2D::new(0.0, 0.0));

        app.set_form_draft_capture_enabled_for_testing(false);
        app.apply_reducer_intents([GraphIntent::SetNodeFormDraft {
            key,
            form_draft: Some("draft text".to_string()),
        }]);
        assert_eq!(
            app.workspace
                .domain
                .graph
                .get_node(key)
                .unwrap()
                .session_form_draft,
            None
        );

        app.set_form_draft_capture_enabled_for_testing(true);
        app.apply_reducer_intents([GraphIntent::SetNodeFormDraft {
            key,
            form_draft: Some("draft text".to_string()),
        }]);
        assert_eq!(
            app.workspace
                .domain
                .graph
                .get_node(key)
                .unwrap()
                .session_form_draft,
            Some("draft text".to_string())
        );
    }

    #[test]
    fn test_intent_webview_history_changed_adds_history_edge_on_back() {
        let mut app = GraphBrowserApp::new_for_testing();
        let from = app
            .workspace
            .domain
            .graph
            .add_node("https://a.com".into(), Point2D::new(0.0, 0.0));
        let to = app
            .workspace
            .domain
            .graph
            .add_node("https://b.com".into(), Point2D::new(100.0, 0.0));
        let wv = test_webview_id();
        app.map_webview_to_node(wv, to);
        if let Some(node) = app.workspace.domain.graph.get_node_mut(to) {
            node.history_entries = vec!["https://a.com".into(), "https://b.com".into()];
            node.history_index = 1;
        }

        app.apply_reducer_intents([GraphIntent::WebViewHistoryChanged {
            webview_id: wv,
            entries: vec!["https://a.com".into(), "https://b.com".into()],
            current: 0,
        }]);

        let has_edge = app
            .workspace
            .domain
            .graph
            .edges()
            .any(|e| e.edge_type == EdgeType::History && e.from == to && e.to == from);
        assert!(has_edge);
    }

    #[test]
    fn test_intent_webview_history_changed_does_not_add_edge_on_normal_navigation() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("https://b.com".into(), Point2D::new(0.0, 0.0));
        let wv = test_webview_id();
        app.map_webview_to_node(wv, key);
        if let Some(node) = app.workspace.domain.graph.get_node_mut(key) {
            node.history_entries = vec!["https://a.com".into(), "https://b.com".into()];
            node.history_index = 1;
        }

        app.apply_reducer_intents([GraphIntent::WebViewHistoryChanged {
            webview_id: wv,
            entries: vec![
                "https://a.com".into(),
                "https://b.com".into(),
                "https://c.com".into(),
            ],
            current: 2,
        }]);

        let history_edge_count = app
            .workspace
            .domain
            .graph
            .edges()
            .filter(|e| e.edge_type == EdgeType::History)
            .count();
        assert_eq!(history_edge_count, 0);
    }

    #[test]
    fn test_intent_webview_history_changed_adds_history_edge_on_forward_same_list() {
        let mut app = GraphBrowserApp::new_for_testing();
        let from = app
            .workspace
            .domain
            .graph
            .add_node("https://a.com".into(), Point2D::new(0.0, 0.0));
        let to = app
            .workspace
            .domain
            .graph
            .add_node("https://b.com".into(), Point2D::new(100.0, 0.0));
        let wv = test_webview_id();
        app.map_webview_to_node(wv, from);
        if let Some(node) = app.workspace.domain.graph.get_node_mut(from) {
            node.history_entries = vec!["https://a.com".into(), "https://b.com".into()];
            node.history_index = 0;
        }

        app.apply_reducer_intents([GraphIntent::WebViewHistoryChanged {
            webview_id: wv,
            entries: vec!["https://a.com".into(), "https://b.com".into()],
            current: 1,
        }]);

        let has_edge = app
            .workspace
            .domain
            .graph
            .edges()
            .any(|e| e.edge_type == EdgeType::History && e.from == from && e.to == to);
        assert!(has_edge);
    }

    #[test]
    fn test_intent_webview_history_changed_appends_traversals_on_repeat_navigation() {
        let mut app = GraphBrowserApp::new_for_testing();
        let a = app
            .workspace
            .domain
            .graph
            .add_node("https://a.com".into(), Point2D::new(0.0, 0.0));
        let b = app
            .workspace
            .domain
            .graph
            .add_node("https://b.com".into(), Point2D::new(100.0, 0.0));
        let wv = test_webview_id();
        app.map_webview_to_node(wv, b);
        if let Some(node) = app.workspace.domain.graph.get_node_mut(b) {
            node.history_entries = vec!["https://a.com".into(), "https://b.com".into()];
            node.history_index = 1;
        }

        app.apply_reducer_intents([GraphIntent::WebViewHistoryChanged {
            webview_id: wv,
            entries: vec!["https://a.com".into(), "https://b.com".into()],
            current: 0,
        }]);

        app.apply_reducer_intents([GraphIntent::WebViewHistoryChanged {
            webview_id: wv,
            entries: vec!["https://a.com".into(), "https://b.com".into()],
            current: 1,
        }]);

        let back_edge_key = app
            .workspace
            .domain
            .graph
            .find_edge_key(b, a)
            .expect("back traversal edge");
        let back_payload = app.workspace.domain.graph.get_edge(back_edge_key).unwrap();
        assert_eq!(back_payload.traversals().len(), 1);
        assert_eq!(
            back_payload.traversals()[0].trigger,
            NavigationTrigger::Back
        );

        let forward_edge_key = app
            .workspace
            .domain
            .graph
            .find_edge_key(a, b)
            .expect("forward traversal edge");
        let forward_payload = app
            .workspace
            .domain
            .graph
            .get_edge(forward_edge_key)
            .unwrap();
        assert_eq!(forward_payload.traversals().len(), 1);
        assert_eq!(
            forward_payload.traversals()[0].trigger,
            NavigationTrigger::Forward
        );

        app.apply_reducer_intents([GraphIntent::WebViewHistoryChanged {
            webview_id: wv,
            entries: vec!["https://a.com".into(), "https://b.com".into()],
            current: 0,
        }]);

        let back_payload = app.workspace.domain.graph.get_edge(back_edge_key).unwrap();
        assert_eq!(back_payload.traversals().len(), 2);
        assert_eq!(
            back_payload.traversals()[1].trigger,
            NavigationTrigger::Back
        );
    }

    #[test]
    fn set_and_clear_highlighted_edge_do_not_append_traversal() {
        let mut app = GraphBrowserApp::new_for_testing();
        let a = app
            .workspace
            .domain
            .graph
            .add_node("https://a.com".into(), Point2D::new(0.0, 0.0));
        let b = app
            .workspace
            .domain
            .graph
            .add_node("https://b.com".into(), Point2D::new(100.0, 0.0));
        let wv = test_webview_id();
        app.map_webview_to_node(wv, a);

        app.apply_reducer_intents([GraphIntent::WebViewUrlChanged {
            webview_id: wv,
            new_url: "https://b.com".into(),
        }]);

        let edge_key = app
            .workspace
            .domain
            .graph
            .find_edge_key(a, b)
            .expect("history traversal edge should exist");
        let before = app
            .workspace
            .domain
            .graph
            .get_edge(edge_key)
            .expect("edge payload")
            .traversals()
            .len();

        app.apply_reducer_intents([GraphIntent::SetHighlightedEdge { from: a, to: b }]);
        app.apply_reducer_intents([GraphIntent::ClearHighlightedEdge]);

        let after = app
            .workspace
            .domain
            .graph
            .get_edge(edge_key)
            .expect("edge payload")
            .traversals()
            .len();
        assert_eq!(before, after);
    }

    #[test]
    fn history_health_summary_tracks_capture_status_and_append_failures() {
        let mut app = GraphBrowserApp::new_for_testing();
        let a = app
            .workspace
            .domain
            .graph
            .add_node("https://a.com".into(), Point2D::new(0.0, 0.0));

        let before = app.history_health_summary();
        assert_eq!(
            before.capture_status,
            HistoryCaptureStatus::DegradedCaptureOnly
        );
        assert_eq!(before.recent_traversal_append_failures, 0);
        assert!(before.last_event_unix_ms.is_none());

        assert!(!app.push_history_traversal_and_sync(a, a, NavigationTrigger::Unknown));

        let after = app.history_health_summary();
        assert_eq!(
            after.capture_status,
            HistoryCaptureStatus::DegradedCaptureOnly
        );
        assert_eq!(after.recent_traversal_append_failures, 1);
        assert_eq!(
            after.recent_failure_reason_bucket.as_deref(),
            Some("self_loop")
        );
        assert!(
            after
                .last_error
                .as_deref()
                .is_some_and(|msg| msg.contains("self_loop"))
        );
        assert!(!after.preview_mode_active);
        assert!(!after.last_preview_isolation_violation);
        assert!(after.last_event_unix_ms.is_some());
    }

    #[test]
    fn history_archive_counts_consistent_after_dissolution_and_clear() {
        let dir = TempDir::new().expect("temp dir");
        let mut app = GraphBrowserApp::new_from_dir(dir.path().to_path_buf());

        let a = app.add_node_and_sync("https://a.com".to_string(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("https://b.com".to_string(), Point2D::new(100.0, 0.0));
        let wv = test_webview_id();
        app.map_webview_to_node(wv, a);
        app.apply_reducer_intents([GraphIntent::WebViewUrlChanged {
            webview_id: wv,
            new_url: "https://b.com".into(),
        }]);

        let before = app.history_manager_archive_counts();
        assert_eq!(before.0, 0);
        assert_eq!(before.1, 0);

        app.apply_reducer_intents([GraphIntent::RemoveEdge {
            from: a,
            to: b,
            edge_type: EdgeType::History,
        }]);

        let after_remove = app.history_manager_archive_counts();
        assert_eq!(after_remove.0, 0);
        assert!(after_remove.1 > 0);
        assert_eq!(
            app.history_manager_dissolved_entries(usize::MAX).len(),
            after_remove.1
        );

        app.apply_reducer_intents([GraphIntent::ClearHistoryDissolved]);
        let after_clear = app.history_manager_archive_counts();
        assert_eq!(after_clear.0, 0);
        assert_eq!(after_clear.1, 0);
    }

    #[test]
    fn history_archive_auto_curation_keeps_latest_entries() {
        let dir = TempDir::new().expect("temp dir");
        let mut app = GraphBrowserApp::new_from_dir(dir.path().to_path_buf());
        let from = Uuid::new_v4().to_string();
        let to = Uuid::new_v4().to_string();

        {
            let store = app
                .services
                .persistence
                .as_mut()
                .expect("persistence store should exist");
            for i in 0..6u64 {
                let entry = crate::services::persistence::types::LogEntry::AppendTraversal {
                    from_node_id: from.clone(),
                    to_node_id: to.clone(),
                    timestamp_ms: i,
                    trigger:
                        crate::services::persistence::types::PersistedNavigationTrigger::Unknown,
                };
                store
                    .archive_append_traversal(&entry)
                    .expect("archive traversal should succeed");
                store
                    .archive_dissolved_traversal(&entry)
                    .expect("archive dissolved should succeed");
            }
        }

        app.apply_reducer_intents([
            GraphIntent::AutoCurateHistoryTimeline { keep_latest: 2 },
            GraphIntent::AutoCurateHistoryDissolved { keep_latest: 3 },
        ]);

        let (timeline_count, dissolved_count) = app.history_manager_archive_counts();
        assert_eq!(timeline_count, 2);
        assert_eq!(dissolved_count, 3);

        let timeline = app.history_manager_timeline_entries(usize::MAX);
        assert_eq!(timeline.len(), 2);
        match &timeline[0] {
            crate::services::persistence::types::LogEntry::AppendTraversal {
                timestamp_ms, ..
            } => assert_eq!(*timestamp_ms, 5),
            _ => panic!("expected traversal entry"),
        }
    }

    #[test]
    fn history_timeline_index_entries_are_exposed_from_persistence() {
        let dir = TempDir::new().expect("temp dir");
        let mut app = GraphBrowserApp::new_from_dir(dir.path().to_path_buf());
        let from = Uuid::new_v4().to_string();
        let to = Uuid::new_v4().to_string();

        {
            let store = app
                .services
                .persistence
                .as_mut()
                .expect("persistence store should exist");
            store.log_mutation(
                &crate::services::persistence::types::LogEntry::AppendTraversal {
                    from_node_id: from.clone(),
                    to_node_id: to.clone(),
                    timestamp_ms: 10,
                    trigger:
                        crate::services::persistence::types::PersistedNavigationTrigger::Unknown,
                },
            );
            store.log_mutation(
                &crate::services::persistence::types::LogEntry::AppendTraversal {
                    from_node_id: from,
                    to_node_id: to,
                    timestamp_ms: 20,
                    trigger:
                        crate::services::persistence::types::PersistedNavigationTrigger::Forward,
                },
            );
        }

        let idx = app.history_timeline_index_entries(usize::MAX);
        assert_eq!(idx.len(), 2);
        assert_eq!(idx[0].timestamp_ms, 20);
        assert_eq!(idx[1].timestamp_ms, 10);
        assert!(idx[0].log_position > idx[1].log_position);
    }

    #[test]
    fn history_health_summary_tracks_preview_and_return_to_present_failure() {
        let mut app = GraphBrowserApp::new_for_testing();

        app.apply_reducer_intents([GraphIntent::EnterHistoryTimelinePreview]);
        let preview = app.history_health_summary();
        assert!(preview.preview_mode_active);
        assert!(!preview.last_preview_isolation_violation);
        assert!(!preview.replay_in_progress);
        assert!(preview.replay_cursor.is_none());
        assert!(preview.replay_total_steps.is_none());
        assert!(preview.last_return_to_present_result.is_none());

        app.apply_reducer_intents([GraphIntent::HistoryTimelineReplayStarted]);
        app.apply_reducer_intents([GraphIntent::HistoryTimelineReplayProgress {
            cursor: 2,
            total_steps: 5,
        }]);
        let replay = app.history_health_summary();
        assert!(replay.replay_in_progress);
        assert_eq!(replay.replay_cursor, Some(2));
        assert_eq!(replay.replay_total_steps, Some(5));

        app.apply_reducer_intents([GraphIntent::HistoryTimelinePreviewIsolationViolation {
            detail: "attempted live mutation".to_string(),
        }]);
        let violation = app.history_health_summary();
        assert!(violation.last_preview_isolation_violation);
        assert_eq!(
            violation.recent_failure_reason_bucket.as_deref(),
            Some("preview_isolation_violation")
        );

        app.apply_reducer_intents([GraphIntent::HistoryTimelineReturnToPresentFailed {
            detail: "cursor invalid".to_string(),
        }]);
        let result = app.history_health_summary();
        assert_eq!(
            result.last_return_to_present_result.as_deref(),
            Some("failed: cursor invalid")
        );
        assert_eq!(
            result.recent_failure_reason_bucket.as_deref(),
            Some("return_to_present_failed")
        );
    }

    #[test]
    fn history_preview_blocks_graph_mutations_and_records_isolation_violation() {
        let mut app = GraphBrowserApp::new_for_testing();

        app.apply_reducer_intents([GraphIntent::EnterHistoryTimelinePreview]);
        let before_count = app.workspace.domain.graph.node_count();

        app.apply_reducer_intents([GraphIntent::CreateNodeNearCenter]);

        let after_count = app.workspace.domain.graph.node_count();
        assert_eq!(before_count, after_count);
        let health = app.history_health_summary();
        assert!(health.preview_mode_active);
        assert!(health.last_preview_isolation_violation);
        assert!(
            health
                .last_error
                .as_deref()
                .is_some_and(|msg| msg.contains("preview_isolation_violation"))
        );
    }

    #[test]
    fn history_replay_advance_and_reset_follow_cursor_contract() {
        let mut app = GraphBrowserApp::new_for_testing();

        app.apply_reducer_intents([GraphIntent::EnterHistoryTimelinePreview]);
        app.apply_reducer_intents([GraphIntent::HistoryTimelineReplaySetTotal { total_steps: 5 }]);

        let seeded = app.history_health_summary();
        assert_eq!(seeded.replay_cursor, Some(0));
        assert_eq!(seeded.replay_total_steps, Some(5));

        app.apply_reducer_intents([GraphIntent::HistoryTimelineReplayAdvance { steps: 3 }]);
        let mid = app.history_health_summary();
        assert!(mid.replay_in_progress);
        assert_eq!(mid.replay_cursor, Some(3));

        app.apply_reducer_intents([GraphIntent::HistoryTimelineReplayAdvance { steps: 10 }]);
        let done = app.history_health_summary();
        assert!(!done.replay_in_progress);
        assert_eq!(done.replay_cursor, Some(5));

        app.apply_reducer_intents([GraphIntent::HistoryTimelineReplayReset]);
        let reset = app.history_health_summary();
        assert!(!reset.replay_in_progress);
        assert_eq!(reset.replay_cursor, Some(0));
    }

    #[test]
    fn history_preview_replay_builds_detached_graph_without_mutating_live_state() {
        let dir = TempDir::new().expect("temp dir");
        let mut app = GraphBrowserApp::new_from_dir(dir.path().to_path_buf());
        let from = Uuid::new_v4();
        let to = Uuid::new_v4();
        let later = Uuid::new_v4();

        {
            let store = app
                .services
                .persistence
                .as_mut()
                .expect("persistence store should exist");
            store.log_mutation(&crate::services::persistence::types::LogEntry::AddNode {
                node_id: from.to_string(),
                url: "https://from.example".to_string(),
                position_x: 0.0,
                position_y: 0.0,
            });
            store.log_mutation(&crate::services::persistence::types::LogEntry::AddNode {
                node_id: to.to_string(),
                url: "https://to.example".to_string(),
                position_x: 32.0,
                position_y: 0.0,
            });
            store.log_mutation(
                &crate::services::persistence::types::LogEntry::AppendTraversal {
                    from_node_id: from.to_string(),
                    to_node_id: to.to_string(),
                    timestamp_ms: 1_000,
                    trigger:
                        crate::services::persistence::types::PersistedNavigationTrigger::Forward,
                },
            );
            store.log_mutation(&crate::services::persistence::types::LogEntry::AddNode {
                node_id: later.to_string(),
                url: "https://later.example".to_string(),
                position_x: 64.0,
                position_y: 0.0,
            });
            app.workspace.domain.graph = store.recover().expect("full graph recovery");
        }

        assert_eq!(app.workspace.domain.graph.node_count(), 3);

        app.apply_reducer_intents([GraphIntent::EnterHistoryTimelinePreview]);
        app.apply_reducer_intents([GraphIntent::HistoryTimelineReplaySetTotal { total_steps: 1 }]);
        app.apply_reducer_intents([GraphIntent::HistoryTimelineReplayAdvance { steps: 1 }]);

        assert_eq!(app.workspace.domain.graph.node_count(), 3);
        let preview_graph = app
            .workspace
            .history_preview_graph
            .as_ref()
            .expect("preview graph should be populated");
        assert_eq!(preview_graph.node_count(), 2);
        assert!(preview_graph.get_node_key_by_id(later).is_none());

        app.apply_reducer_intents([GraphIntent::ExitHistoryTimelinePreview]);
        assert_eq!(app.workspace.domain.graph.node_count(), 3);
        assert!(app.workspace.history_preview_graph.is_none());
        assert_eq!(
            app.workspace
                .history_last_return_to_present_result
                .as_deref(),
            Some("restored")
        );
    }

    #[test]
    fn history_preview_replay_does_not_append_persistence_log_entries() {
        let dir = TempDir::new().expect("temp dir");
        let mut app = GraphBrowserApp::new_from_dir(dir.path().to_path_buf());
        let from = Uuid::new_v4();
        let to = Uuid::new_v4();

        {
            let store = app
                .services
                .persistence
                .as_mut()
                .expect("persistence store should exist");
            store.log_mutation(&crate::services::persistence::types::LogEntry::AddNode {
                node_id: from.to_string(),
                url: "https://from.example".to_string(),
                position_x: 0.0,
                position_y: 0.0,
            });
            store.log_mutation(&crate::services::persistence::types::LogEntry::AddNode {
                node_id: to.to_string(),
                url: "https://to.example".to_string(),
                position_x: 32.0,
                position_y: 0.0,
            });
            store.log_mutation(
                &crate::services::persistence::types::LogEntry::AppendTraversal {
                    from_node_id: from.to_string(),
                    to_node_id: to.to_string(),
                    timestamp_ms: 1_000,
                    trigger:
                        crate::services::persistence::types::PersistedNavigationTrigger::Forward,
                },
            );
        }

        let before_log_entries = app
            .services
            .persistence
            .as_ref()
            .expect("persistence store should exist")
            .log_entry_count_for_tests();

        app.apply_reducer_intents([GraphIntent::EnterHistoryTimelinePreview]);
        app.apply_reducer_intents([GraphIntent::HistoryTimelineReplaySetTotal { total_steps: 1 }]);
        app.apply_reducer_intents([GraphIntent::HistoryTimelineReplayAdvance { steps: 1 }]);
        app.apply_reducer_intents([GraphIntent::HistoryTimelineReplayReset]);
        app.apply_reducer_intents([GraphIntent::ExitHistoryTimelinePreview]);

        let after_log_entries = app
            .services
            .persistence
            .as_ref()
            .expect("persistence store should exist")
            .log_entry_count_for_tests();
        assert_eq!(before_log_entries, after_log_entries);
    }

    #[test]
    fn test_intent_create_user_grouped_edge_adds_single_edge() {
        let mut app = GraphBrowserApp::new_for_testing();
        let from = app
            .workspace
            .domain
            .graph
            .add_node("https://from.com".into(), Point2D::new(0.0, 0.0));
        let to = app
            .workspace
            .domain
            .graph
            .add_node("https://to.com".into(), Point2D::new(10.0, 0.0));

        app.apply_reducer_intents([GraphIntent::CreateUserGroupedEdge {
            from,
            to,
            label: None,
        }]);

        let count = app
            .workspace
            .domain
            .graph
            .edges()
            .filter(|e| e.edge_type == EdgeType::UserGrouped && e.from == from && e.to == to)
            .count();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_intent_create_user_grouped_edge_is_idempotent() {
        let mut app = GraphBrowserApp::new_for_testing();
        let from = app
            .workspace
            .domain
            .graph
            .add_node("https://from.com".into(), Point2D::new(0.0, 0.0));
        let to = app
            .workspace
            .domain
            .graph
            .add_node("https://to.com".into(), Point2D::new(10.0, 0.0));

        app.apply_reducer_intents([
            GraphIntent::CreateUserGroupedEdge {
                from,
                to,
                label: None,
            },
            GraphIntent::CreateUserGroupedEdge {
                from,
                to,
                label: None,
            },
        ]);

        let count = app
            .workspace
            .domain
            .graph
            .edges()
            .filter(|e| e.edge_type == EdgeType::UserGrouped && e.from == from && e.to == to)
            .count();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_intent_create_user_grouped_edge_from_primary_selection_noop_for_single_select() {
        let mut app = GraphBrowserApp::new_for_testing();
        let a = app
            .workspace
            .domain
            .graph
            .add_node("https://a.com".into(), Point2D::new(0.0, 0.0));
        app.select_node(a, false);

        app.apply_reducer_intents([GraphIntent::CreateUserGroupedEdgeFromPrimarySelection]);

        let count = app
            .workspace
            .domain
            .graph
            .edges()
            .filter(|e| e.edge_type == EdgeType::UserGrouped)
            .count();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_execute_edge_command_connect_selected_pair() {
        let mut app = GraphBrowserApp::new_for_testing();
        let from = app
            .workspace
            .domain
            .graph
            .add_node("https://from.com".into(), Point2D::new(0.0, 0.0));
        let to = app
            .workspace
            .domain
            .graph
            .add_node("https://to.com".into(), Point2D::new(10.0, 0.0));

        app.select_node(from, false);
        app.select_node(to, true);
        app.workspace.physics.base.is_running = false;

        app.apply_reducer_intents([GraphIntent::ExecuteEdgeCommand {
            command: EdgeCommand::ConnectSelectedPair,
        }]);

        assert!(
            app.workspace
                .domain
                .graph
                .edges()
                .any(|e| e.edge_type == EdgeType::UserGrouped && e.from == from && e.to == to)
        );
        assert!(app.workspace.physics.base.is_running);
    }

    #[test]
    fn test_selection_ordered_pair_uses_first_selected_as_source() {
        let mut app = GraphBrowserApp::new_for_testing();
        let first = app
            .workspace
            .domain
            .graph
            .add_node("https://first.com".into(), Point2D::new(0.0, 0.0));
        let second = app
            .workspace
            .domain
            .graph
            .add_node("https://second.com".into(), Point2D::new(10.0, 0.0));

        app.select_node(first, false);
        app.select_node(second, true);

        assert_eq!(
            app.focused_selection().ordered_pair(),
            Some((first, second))
        );
    }

    #[test]
    fn test_execute_edge_command_remove_user_edge_removes_both_directions() {
        let mut app = GraphBrowserApp::new_for_testing();
        let from = app
            .workspace
            .domain
            .graph
            .add_node("https://from.com".into(), Point2D::new(0.0, 0.0));
        let to = app
            .workspace
            .domain
            .graph
            .add_node("https://to.com".into(), Point2D::new(10.0, 0.0));

        app.add_user_grouped_edge_if_missing(from, to, None);
        app.add_user_grouped_edge_if_missing(to, from, None);
        app.select_node(from, false);
        app.select_node(to, true);
        app.workspace.physics.base.is_running = false;

        app.apply_reducer_intents([GraphIntent::ExecuteEdgeCommand {
            command: EdgeCommand::RemoveUserEdge,
        }]);

        assert!(
            !app.workspace
                .domain
                .graph
                .edges()
                .any(|e| e.edge_type == EdgeType::UserGrouped)
        );
        assert!(app.workspace.physics.base.is_running);
    }

    #[test]
    fn test_execute_edge_command_pin_and_unpin_selected() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("https://pin.com".into(), Point2D::new(0.0, 0.0));
        app.select_node(key, false);

        app.apply_reducer_intents([GraphIntent::ExecuteEdgeCommand {
            command: EdgeCommand::PinSelected,
        }]);
        assert!(
            app.workspace
                .domain
                .graph
                .get_node(key)
                .is_some_and(|node| node.is_pinned)
        );

        app.apply_reducer_intents([GraphIntent::ExecuteEdgeCommand {
            command: EdgeCommand::UnpinSelected,
        }]);
        assert!(
            app.workspace
                .domain
                .graph
                .get_node(key)
                .is_some_and(|node| !node.is_pinned)
        );
    }

    #[test]
    fn test_add_node_and_sync_reheats_physics() {
        let mut app = GraphBrowserApp::new_for_testing();
        app.workspace.physics.base.is_running = false;
        app.workspace.drag_release_frames_remaining = 5;

        app.add_node_and_sync("https://example.com".into(), Point2D::new(0.0, 0.0));

        assert!(app.workspace.physics.base.is_running);
        assert_eq!(app.workspace.drag_release_frames_remaining, 0);
    }

    #[test]
    fn test_reheat_physics_intent_enables_simulation() {
        let mut app = GraphBrowserApp::new_for_testing();
        app.workspace.physics.base.is_running = false;
        app.workspace.drag_release_frames_remaining = 5;

        app.apply_reducer_intents([GraphIntent::ReheatPhysics]);

        assert!(app.workspace.physics.base.is_running);
        assert_eq!(app.workspace.drag_release_frames_remaining, 0);
    }

    #[test]
    fn test_set_camera_fit_lock_clears_pending_drag_release_decay() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_id = GraphViewId::new();
        app.workspace
            .views
            .insert(view_id, GraphViewState::new_with_id(view_id, "Focused"));
        app.workspace.focused_view = Some(view_id);
        app.workspace.drag_release_frames_remaining = 7;

        app.set_camera_fit_locked(true);

        assert!(app.camera_fit_locked());
        assert_eq!(app.workspace.drag_release_frames_remaining, 0);
    }

    #[test]
    fn test_drag_release_keeps_physics_paused_when_camera_fit_lock_enabled() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_id = GraphViewId::new();
        app.workspace
            .views
            .insert(view_id, GraphViewState::new_with_id(view_id, "Focused"));
        app.workspace.focused_view = Some(view_id);
        app.workspace.physics.base.is_running = false;
        app.set_camera_fit_locked(true);

        app.set_interacting(true);
        app.set_interacting(false);

        assert!(!app.workspace.physics.base.is_running);
        assert_eq!(app.workspace.drag_release_frames_remaining, 0);
    }

    #[test]
    fn test_toggle_primary_node_pin_toggles_selected_primary() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("https://pin.com".into(), Point2D::new(0.0, 0.0));
        app.select_node(key, false);

        app.apply_reducer_intents([GraphIntent::TogglePrimaryNodePin]);
        assert!(
            app.workspace
                .domain
                .graph
                .get_node(key)
                .is_some_and(|node| node.is_pinned)
        );

        app.apply_reducer_intents([GraphIntent::TogglePrimaryNodePin]);
        assert!(
            app.workspace
                .domain
                .graph
                .get_node(key)
                .is_some_and(|node| !node.is_pinned)
        );
    }

    #[test]
    fn test_intent_remove_edge_removes_matching_type_only() {
        let mut app = GraphBrowserApp::new_for_testing();
        let from = app.add_node_and_sync("https://from.com".into(), Point2D::new(0.0, 0.0));
        let to = app.add_node_and_sync("https://to.com".into(), Point2D::new(100.0, 0.0));

        let _ = app.add_edge_and_sync(from, to, EdgeType::Hyperlink, None);
        let _ = app.add_edge_and_sync(from, to, EdgeType::UserGrouped, None);

        app.apply_reducer_intents([GraphIntent::RemoveEdge {
            from,
            to,
            edge_type: EdgeType::UserGrouped,
        }]);

        let has_user_grouped = app
            .workspace
            .domain
            .graph
            .edges()
            .any(|e| e.edge_type == EdgeType::UserGrouped && e.from == from && e.to == to);
        let has_hyperlink = app
            .workspace
            .domain
            .graph
            .edges()
            .any(|e| e.edge_type == EdgeType::Hyperlink && e.from == from && e.to == to);
        assert!(!has_user_grouped);
        assert!(has_hyperlink);
    }

    #[test]
    fn test_remove_edges_and_log_reports_removed_count() {
        let mut app = GraphBrowserApp::new_for_testing();
        let from = app.add_node_and_sync("https://from.com".into(), Point2D::new(0.0, 0.0));
        let to = app.add_node_and_sync("https://to.com".into(), Point2D::new(100.0, 0.0));

        let _ = app.add_edge_and_sync(from, to, EdgeType::UserGrouped, None);
        let _ = app.add_edge_and_sync(from, to, EdgeType::UserGrouped, None);

        let removed = app.remove_edges_and_log(from, to, EdgeType::UserGrouped);
        assert_eq!(removed, 1);
        assert_eq!(
            app.workspace
                .domain
                .graph
                .edges()
                .filter(|e| e.edge_type == EdgeType::UserGrouped)
                .count(),
            0
        );
    }

    #[test]
    fn test_history_changed_is_authoritative_when_url_callback_stays_latest() {
        let mut app = GraphBrowserApp::new_for_testing();
        let step1 = app.add_node_and_sync(
            "https://site.example/?step=1".into(),
            Point2D::new(0.0, 0.0),
        );
        let step2 = app.add_node_and_sync(
            "https://site.example/?step=2".into(),
            Point2D::new(10.0, 0.0),
        );
        let wv = test_webview_id();
        app.map_webview_to_node(wv, step2);
        if let Some(node) = app.workspace.domain.graph.get_node_mut(step2) {
            node.history_entries = vec![
                "https://site.example/?step=0".into(),
                "https://site.example/?step=1".into(),
                "https://site.example/?step=2".into(),
            ];
            node.history_index = 2;
        }

        // Mirrors observed delegate behavior: URL callback can stay at the latest route
        // while history callback index moves backward.
        app.apply_reducer_intents([
            GraphIntent::WebViewUrlChanged {
                webview_id: wv,
                new_url: "https://site.example/?step=2".into(),
            },
            GraphIntent::WebViewHistoryChanged {
                webview_id: wv,
                entries: vec![
                    "https://site.example/?step=0".into(),
                    "https://site.example/?step=1".into(),
                    "https://site.example/?step=2".into(),
                ],
                current: 1,
            },
        ]);

        let node = app.workspace.domain.graph.get_node(step2).unwrap();
        assert_eq!(node.history_index, 1);

        let has_edge = app
            .workspace
            .domain
            .graph
            .edges()
            .any(|e| e.edge_type == EdgeType::History && e.from == step2 && e.to == step1);
        assert!(has_edge);
    }

    #[test]
    fn test_intent_webview_title_changed_updates_and_ignores_empty() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("https://title.com".into(), Point2D::new(0.0, 0.0));
        let wv = test_webview_id();
        app.map_webview_to_node(wv, key);
        let original_title = app
            .workspace
            .domain
            .graph
            .get_node(key)
            .unwrap()
            .title
            .clone();

        app.apply_reducer_intents([GraphIntent::WebViewTitleChanged {
            webview_id: wv,
            title: Some("".into()),
        }]);
        assert_eq!(
            app.workspace.domain.graph.get_node(key).unwrap().title,
            original_title
        );

        app.apply_reducer_intents([GraphIntent::WebViewTitleChanged {
            webview_id: wv,
            title: Some("Hello".into()),
        }]);
        assert_eq!(
            app.workspace.domain.graph.get_node(key).unwrap().title,
            "Hello"
        );
    }

    #[test]
    fn test_intent_thumbnail_and_favicon_update_node_metadata() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("https://assets.com".into(), Point2D::new(0.0, 0.0));

        app.apply_reducer_intents([
            GraphIntent::SetNodeThumbnail {
                key,
                png_bytes: vec![1, 2, 3],
                width: 10,
                height: 20,
            },
            GraphIntent::SetNodeFavicon {
                key,
                rgba: vec![255, 0, 0, 255],
                width: 1,
                height: 1,
            },
        ]);

        let node = app.workspace.domain.graph.get_node(key).unwrap();
        assert_eq!(node.thumbnail_png.as_ref().unwrap().len(), 3);
        assert_eq!(node.thumbnail_width, 10);
        assert_eq!(node.thumbnail_height, 20);
        assert_eq!(node.favicon_rgba.as_ref().unwrap().len(), 4);
        assert_eq!(node.favicon_width, 1);
        assert_eq!(node.favicon_height, 1);
    }

    #[test]
    fn test_conflict_delete_dominates_title_update_any_order() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("https://conflict-a.com".into(), Point2D::new(0.0, 0.0));
        let wv = test_webview_id();
        app.map_webview_to_node(wv, key);
        app.select_node(key, false);
        app.apply_reducer_intents([
            GraphIntent::RemoveSelectedNodes,
            GraphIntent::WebViewTitleChanged {
                webview_id: wv,
                title: Some("updated".into()),
            },
        ]);
        assert!(app.workspace.domain.graph.get_node(key).is_none());

        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("https://conflict-b.com".into(), Point2D::new(0.0, 0.0));
        let wv = test_webview_id();
        app.map_webview_to_node(wv, key);
        app.select_node(key, false);
        app.apply_reducer_intents([
            GraphIntent::WebViewTitleChanged {
                webview_id: wv,
                title: Some("updated".into()),
            },
            GraphIntent::RemoveSelectedNodes,
        ]);
        assert!(app.workspace.domain.graph.get_node(key).is_none());
    }

    #[test]
    fn test_conflict_delete_dominates_metadata_updates() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("https://conflict-meta.com".into(), Point2D::new(0.0, 0.0));
        let wv = test_webview_id();
        app.map_webview_to_node(wv, key);
        app.select_node(key, false);

        app.apply_reducer_intents([
            GraphIntent::RemoveSelectedNodes,
            GraphIntent::WebViewHistoryChanged {
                webview_id: wv,
                entries: vec!["https://x.com".into()],
                current: 0,
            },
            GraphIntent::SetNodeThumbnail {
                key,
                png_bytes: vec![1, 2, 3],
                width: 8,
                height: 8,
            },
            GraphIntent::SetNodeFavicon {
                key,
                rgba: vec![0, 0, 0, 255],
                width: 1,
                height: 1,
            },
            GraphIntent::SetNodeUrl {
                key,
                new_url: "https://should-not-apply.com".into(),
            },
        ]);

        assert!(app.workspace.domain.graph.get_node(key).is_none());
    }

    #[test]
    fn test_conflict_last_writer_wins_for_url_updates() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("https://start.com".into(), Point2D::new(0.0, 0.0));
        app.apply_reducer_intents([
            GraphIntent::SetNodeUrl {
                key,
                new_url: "https://first.com".into(),
            },
            GraphIntent::SetNodeUrl {
                key,
                new_url: "https://second.com".into(),
            },
        ]);
        assert_eq!(
            app.workspace.domain.graph.get_node(key).unwrap().url,
            "https://second.com"
        );
    }

    #[test]
    #[ignore]
    fn perf_apply_intent_batch_10k_under_budget() {
        let mut app = GraphBrowserApp::new_for_testing();
        let mut intents = Vec::new();
        for i in 0..10_000 {
            intents.push(GraphIntent::CreateNodeAtUrl {
                url: format!("https://perf/{i}"),
                position: Point2D::new((i % 100) as f32, (i / 100) as f32),
            });
        }
        let start = std::time::Instant::now();
        app.apply_reducer_intents(intents);
        let elapsed = start.elapsed();
        assert_eq!(app.workspace.domain.graph.node_count(), 10_000);
        assert!(
            elapsed < std::time::Duration::from_secs(4),
            "intent batch exceeded budget: {elapsed:?}"
        );
    }

    #[test]
    fn test_camera_defaults() {
        let cam = Camera::new();
        assert_eq!(cam.zoom_min, 0.1);
        assert_eq!(cam.zoom_max, 10.0);
        assert_eq!(cam.current_zoom, 0.8);
    }

    #[test]
    fn test_camera_clamp_within_range() {
        let cam = Camera::new();
        assert_eq!(cam.clamp(1.0), 1.0);
        assert_eq!(cam.clamp(5.0), 5.0);
        assert_eq!(cam.clamp(0.5), 0.5);
    }

    #[test]
    fn test_camera_clamp_below_min() {
        let cam = Camera::new();
        assert_eq!(cam.clamp(0.05), 0.1);
        assert_eq!(cam.clamp(0.0), 0.1);
        assert_eq!(cam.clamp(-1.0), 0.1);
    }

    #[test]
    fn test_camera_clamp_above_max() {
        let cam = Camera::new();
        assert_eq!(cam.clamp(15.0), 10.0);
        assert_eq!(cam.clamp(100.0), 10.0);
    }

    #[test]
    fn test_camera_clamp_at_boundaries() {
        let cam = Camera::new();
        assert_eq!(cam.clamp(0.1), 0.1);
        assert_eq!(cam.clamp(10.0), 10.0);
    }

    #[test]
    fn test_create_multiple_placeholder_nodes_unique_urls() {
        let mut app = GraphBrowserApp::new_for_testing();

        let k1 = app.create_new_node_near_center();
        let k2 = app.create_new_node_near_center();
        let k3 = app.create_new_node_near_center();

        // All three nodes must have distinct URLs
        let url1 = app.workspace.domain.graph.get_node(k1).unwrap().url.clone();
        let url2 = app.workspace.domain.graph.get_node(k2).unwrap().url.clone();
        let url3 = app.workspace.domain.graph.get_node(k3).unwrap().url.clone();

        assert_ne!(url1, url2);
        assert_ne!(url2, url3);
        assert_ne!(url1, url3);

        // All URLs start with about:blank#
        assert!(url1.starts_with("about:blank#"));
        assert!(url2.starts_with("about:blank#"));
        assert!(url3.starts_with("about:blank#"));

        // url_to_node should have 3 distinct entries
        assert_eq!(app.workspace.domain.graph.node_count(), 3);
        assert!(app.workspace.domain.graph.get_node_by_url(&url1).is_some());
        assert!(app.workspace.domain.graph.get_node_by_url(&url2).is_some());
        assert!(app.workspace.domain.graph.get_node_by_url(&url3).is_some());
    }

    #[test]
    fn test_placeholder_id_scan_on_recovery() {
        let mut graph = Graph::new();
        graph.add_node("about:blank#5".to_string(), Point2D::new(0.0, 0.0));
        graph.add_node("about:blank#2".to_string(), Point2D::new(100.0, 0.0));
        graph.add_node("https://example.com".to_string(), Point2D::new(200.0, 0.0));

        let next_id = GraphBrowserApp::scan_max_placeholder_id(&graph);
        // Max is 5, so next should be 6
        assert_eq!(next_id, 6);
    }

    #[test]
    fn test_placeholder_id_scan_empty_graph() {
        let graph = Graph::new();
        assert_eq!(GraphBrowserApp::scan_max_placeholder_id(&graph), 0);
    }

    // --- TEST-1: remove_selected_nodes ---

    #[test]
    fn test_remove_selected_nodes_single() {
        let mut app = GraphBrowserApp::new_for_testing();
        let k1 = app
            .workspace
            .domain
            .graph
            .add_node("a".to_string(), Point2D::new(0.0, 0.0));
        let _k2 = app
            .workspace
            .domain
            .graph
            .add_node("b".to_string(), Point2D::new(100.0, 0.0));

        app.select_node(k1, false);
        app.remove_selected_nodes();

        assert_eq!(app.workspace.domain.graph.node_count(), 1);
        assert!(app.workspace.domain.graph.get_node(k1).is_none());
        assert!(app.focused_selection().is_empty());
    }

    #[test]
    fn test_remove_selected_nodes_multi() {
        let mut app = GraphBrowserApp::new_for_testing();
        let k1 = app
            .workspace
            .domain
            .graph
            .add_node("a".to_string(), Point2D::new(0.0, 0.0));
        let k2 = app
            .workspace
            .domain
            .graph
            .add_node("b".to_string(), Point2D::new(100.0, 0.0));
        let k3 = app
            .workspace
            .domain
            .graph
            .add_node("c".to_string(), Point2D::new(200.0, 0.0));

        app.select_node(k1, false);
        app.select_node(k2, true);
        app.remove_selected_nodes();

        assert_eq!(app.workspace.domain.graph.node_count(), 1);
        assert!(app.workspace.domain.graph.get_node(k3).is_some());
        assert!(app.focused_selection().is_empty());
    }

    #[test]
    fn test_remove_selected_nodes_empty_selection() {
        let mut app = GraphBrowserApp::new_for_testing();
        app.workspace
            .domain
            .graph
            .add_node("a".to_string(), Point2D::new(0.0, 0.0));

        // No selection â€” should be a no-op
        app.remove_selected_nodes();
        assert_eq!(app.workspace.domain.graph.node_count(), 1);
    }

    #[test]
    fn test_remove_selected_nodes_clears_webview_mapping() {
        let mut app = GraphBrowserApp::new_for_testing();
        let k1 = app
            .workspace
            .domain
            .graph
            .add_node("a".to_string(), Point2D::new(0.0, 0.0));

        // Simulate a webview mapping
        let fake_wv_id = test_webview_id();
        app.map_webview_to_node(fake_wv_id, k1);
        assert!(app.get_node_for_webview(fake_wv_id).is_some());

        app.select_node(k1, false);
        app.remove_selected_nodes();

        // Mapping should be cleaned up
        assert!(app.get_node_for_webview(fake_wv_id).is_none());
        assert!(app.get_webview_for_node(k1).is_none());
    }

    // --- TEST-1: clear_graph ---

    #[test]
    fn test_clear_graph_resets_everything() {
        let mut app = GraphBrowserApp::new_for_testing();
        let k1 = app
            .workspace
            .domain
            .graph
            .add_node("a".to_string(), Point2D::new(0.0, 0.0));
        let k2 = app
            .workspace
            .domain
            .graph
            .add_node("b".to_string(), Point2D::new(100.0, 0.0));

        app.select_node(k1, false);
        app.select_node(k2, false);

        let fake_wv_id = test_webview_id();
        app.map_webview_to_node(fake_wv_id, k1);
        app.demote_node_to_warm(k1);
        assert_eq!(app.workspace.warm_cache_lru, vec![k1]);

        app.clear_graph();

        assert_eq!(app.workspace.domain.graph.node_count(), 0);
        assert!(app.focused_selection().is_empty());
        assert!(app.get_node_for_webview(fake_wv_id).is_none());
        assert!(app.workspace.warm_cache_lru.is_empty());
        assert!(!app.workspace.workspace_has_unsaved_changes);
        assert!(!app.should_prompt_unsaved_workspace_save());
    }

    #[test]
    fn test_file_tree_projection_state_defaults_are_graph_owned() {
        let app = GraphBrowserApp::new_for_testing();

        assert_eq!(
            app.file_tree_projection_state().containment_relation_source,
            FileTreeContainmentRelationSource::GraphContainment
        );
        assert_eq!(
            app.file_tree_projection_state().sort_mode,
            FileTreeSortMode::Manual
        );
        assert!(app.file_tree_projection_state().row_targets.is_empty());
        assert!(app.file_tree_projection_state().selected_rows.is_empty());
    }

    #[test]
    fn test_file_tree_projection_rebuild_populates_node_rows_for_graph_source() {
        let mut app = GraphBrowserApp::new_for_testing();
        let node_key = app.workspace.domain.graph.add_node(
            "https://example.com/tree-node".to_string(),
            Point2D::new(0.0, 0.0),
        );
        let node_id = app
            .workspace
            .domain
            .graph
            .get_node(node_key)
            .map(|node| node.id)
            .expect("node must exist");

        app.apply_reducer_intents([GraphIntent::RebuildFileTreeProjection]);

        assert_eq!(
            app.file_tree_projection_state()
                .row_targets
                .get(&format!("node:{node_id}")),
            Some(&FileTreeProjectionTarget::Node(node_key))
        );
    }

    #[test]
    fn test_file_tree_projection_rebuild_populates_saved_view_rows_for_saved_view_source() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_id = GraphViewId::new();
        app.workspace
            .views
            .insert(view_id, GraphViewState::new_with_id(view_id, "Saved View"));

        app.apply_reducer_intents([
            GraphIntent::SetFileTreeContainmentRelationSource {
                source: FileTreeContainmentRelationSource::SavedViewCollections,
            },
            GraphIntent::RebuildFileTreeProjection,
        ]);

        assert_eq!(
            app.file_tree_projection_state()
                .row_targets
                .get(&format!("view:{}", view_id.as_uuid())),
            Some(&FileTreeProjectionTarget::SavedView(view_id))
        );
    }

    #[test]
    fn test_file_tree_projection_rebuild_prunes_stale_selection_and_expansion_rows() {
        let mut app = GraphBrowserApp::new_for_testing();

        app.set_file_tree_selected_rows(["row:stale".to_string()]);
        app.set_file_tree_expanded_rows(["row:stale".to_string()]);

        app.apply_reducer_intents([GraphIntent::RebuildFileTreeProjection]);

        assert!(app.file_tree_projection_state().selected_rows.is_empty());
        assert!(app.file_tree_projection_state().expanded_rows.is_empty());
    }

    #[test]
    fn test_file_tree_projection_rebuild_populates_imported_filesystem_rows_from_file_urls() {
        let mut app = GraphBrowserApp::new_for_testing();
        app.workspace
            .domain
            .graph
            .add_node("file:///tmp/a.txt".to_string(), Point2D::new(0.0, 0.0));
        app.workspace.domain.graph.add_node(
            "https://example.com/not-imported".to_string(),
            Point2D::new(1.0, 0.0),
        );

        app.apply_reducer_intents([
            GraphIntent::SetFileTreeContainmentRelationSource {
                source: FileTreeContainmentRelationSource::ImportedFilesystemProjection,
            },
            GraphIntent::RebuildFileTreeProjection,
        ]);

        let keys: Vec<&String> = app
            .file_tree_projection_state()
            .row_targets
            .keys()
            .collect();
        assert_eq!(keys.len(), 1);
        assert!(keys[0].starts_with("fs:/tmp/a.txt#"));
    }

    #[test]
    fn test_file_tree_projection_root_filter_limits_imported_filesystem_rows() {
        let mut app = GraphBrowserApp::new_for_testing();
        app.workspace
            .domain
            .graph
            .add_node("file:///tmp/a.txt".to_string(), Point2D::new(0.0, 0.0));
        app.workspace
            .domain
            .graph
            .add_node("file:///tmp/b.log".to_string(), Point2D::new(1.0, 0.0));

        app.apply_reducer_intents([
            GraphIntent::SetFileTreeContainmentRelationSource {
                source: FileTreeContainmentRelationSource::ImportedFilesystemProjection,
            },
            GraphIntent::SetFileTreeRootFilter {
                root_filter: Some("a.txt".to_string()),
            },
            GraphIntent::RebuildFileTreeProjection,
        ]);

        let keys: Vec<&String> = app
            .file_tree_projection_state()
            .row_targets
            .keys()
            .collect();
        assert_eq!(keys.len(), 1);
        assert!(keys[0].contains("a.txt"));
    }

    #[test]
    fn test_file_tree_projection_intents_apply_in_workspace_reducer() {
        let mut app = GraphBrowserApp::new_for_testing();

        app.apply_reducer_intents([
            GraphIntent::SetFileTreeContainmentRelationSource {
                source: FileTreeContainmentRelationSource::ImportedFilesystemProjection,
            },
            GraphIntent::SetFileTreeSortMode {
                sort_mode: FileTreeSortMode::NameDescending,
            },
            GraphIntent::SetFileTreeRootFilter {
                root_filter: Some("root:tests".to_string()),
            },
            GraphIntent::SetFileTreeSelectedRows {
                rows: vec!["row:selected".to_string()],
            },
            GraphIntent::SetFileTreeExpandedRows {
                rows: vec!["row:expanded".to_string()],
            },
        ]);

        assert_eq!(
            app.file_tree_projection_state().containment_relation_source,
            FileTreeContainmentRelationSource::ImportedFilesystemProjection
        );
        assert_eq!(
            app.file_tree_projection_state().sort_mode,
            FileTreeSortMode::NameDescending
        );
        assert_eq!(
            app.file_tree_projection_state().root_filter.as_deref(),
            Some("root:tests")
        );
        assert!(
            app.file_tree_projection_state()
                .selected_rows
                .contains("row:selected")
        );
        assert!(
            app.file_tree_projection_state()
                .expanded_rows
                .contains("row:expanded")
        );
    }

    #[test]
    fn test_clear_graph_resets_file_tree_projection_state() {
        let mut app = GraphBrowserApp::new_for_testing();

        app.set_file_tree_containment_relation_source(
            FileTreeContainmentRelationSource::ImportedFilesystemProjection,
        );
        app.set_file_tree_sort_mode(FileTreeSortMode::NameAscending);
        app.set_file_tree_root_filter(Some("root:collections".to_string()));
        app.upsert_file_tree_row_target(
            "row:stale",
            FileTreeProjectionTarget::SavedView(GraphViewId::new()),
        );
        app.set_file_tree_selected_rows(["row:stale".to_string()]);

        app.clear_graph();

        assert_eq!(
            app.file_tree_projection_state().containment_relation_source,
            FileTreeContainmentRelationSource::GraphContainment
        );
        assert_eq!(
            app.file_tree_projection_state().sort_mode,
            FileTreeSortMode::Manual
        );
        assert!(app.file_tree_projection_state().root_filter.is_none());
        assert!(app.file_tree_projection_state().row_targets.is_empty());
        assert!(app.file_tree_projection_state().selected_rows.is_empty());
    }

    // --- TEST-1: create_new_node_near_center ---

    #[test]
    fn test_create_new_node_near_center_empty_graph() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app.create_new_node_near_center();

        assert_eq!(app.workspace.domain.graph.node_count(), 1);
        assert!(app.focused_selection().contains(&key));

        let node = app.workspace.domain.graph.get_node(key).unwrap();
        assert!(node.url.starts_with("about:blank#"));
    }

    #[test]
    fn test_create_new_node_near_center_selects_node() {
        let mut app = GraphBrowserApp::new_for_testing();
        let k1 = app
            .workspace
            .domain
            .graph
            .add_node("existing".to_string(), Point2D::new(0.0, 0.0));
        app.select_node(k1, false);

        let k2 = app.create_new_node_near_center();

        // New node should be selected, old one deselected
        assert_eq!(app.focused_selection().len(), 1);
        assert!(app.focused_selection().contains(&k2));
    }

    // --- TEST-1: demote/promote lifecycle ---

    #[test]
    fn test_promote_and_demote_node_lifecycle() {
        use crate::graph::NodeLifecycle;
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("a".to_string(), Point2D::new(0.0, 0.0));

        // Default lifecycle is Cold
        assert!(matches!(
            app.workspace.domain.graph.get_node(key).unwrap().lifecycle,
            NodeLifecycle::Cold
        ));

        app.promote_node_to_active(key);
        assert!(matches!(
            app.workspace.domain.graph.get_node(key).unwrap().lifecycle,
            NodeLifecycle::Active
        ));

        app.demote_node_to_cold(key);
        assert!(matches!(
            app.workspace.domain.graph.get_node(key).unwrap().lifecycle,
            NodeLifecycle::Cold
        ));
    }

    #[test]
    fn test_demote_clears_webview_mapping() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("a".to_string(), Point2D::new(0.0, 0.0));
        let fake_wv_id = test_webview_id();

        app.map_webview_to_node(fake_wv_id, key);
        assert!(app.get_webview_for_node(key).is_some());

        app.demote_node_to_cold(key);
        assert!(app.get_webview_for_node(key).is_none());
        assert!(app.get_node_for_webview(fake_wv_id).is_none());
    }

    #[test]
    fn test_demote_to_warm_sets_desired_lifecycle_without_mapping() {
        use crate::graph::NodeLifecycle;

        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("a".to_string(), Point2D::new(0.0, 0.0));
        app.promote_node_to_active(key);

        app.demote_node_to_warm(key);
        assert_eq!(
            app.workspace.domain.graph.get_node(key).unwrap().lifecycle,
            NodeLifecycle::Warm
        );
        assert!(app.workspace.warm_cache_lru.is_empty());

        let wv_id = test_webview_id();
        app.map_webview_to_node(wv_id, key);
        app.demote_node_to_warm(key);
        assert_eq!(
            app.workspace.domain.graph.get_node(key).unwrap().lifecycle,
            NodeLifecycle::Warm
        );
        assert_eq!(app.workspace.warm_cache_lru, vec![key]);
    }

    #[test]
    fn test_policy_promote_does_not_auto_reactivate_crashed_node() {
        use crate::graph::NodeLifecycle;

        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("https://a.com".to_string(), Point2D::new(0.0, 0.0));
        let wv = test_webview_id();
        app.map_webview_to_node(wv, key);
        app.apply_reducer_intents([GraphIntent::WebViewCrashed {
            webview_id: wv,
            reason: "boom".to_string(),
            has_backtrace: false,
        }]);
        assert_eq!(
            app.workspace.domain.graph.get_node(key).unwrap().lifecycle,
            NodeLifecycle::Cold
        );
        assert!(app.runtime_crash_state_for_node(key).is_some());

        app.apply_reducer_intents([GraphIntent::PromoteNodeToActive {
            key,
            cause: LifecycleCause::ActiveTileVisible,
        }]);

        assert_eq!(
            app.workspace.domain.graph.get_node(key).unwrap().lifecycle,
            NodeLifecycle::Cold
        );
        assert!(app.runtime_crash_state_for_node(key).is_some());
    }

    #[test]
    fn test_policy_user_select_can_reactivate_and_clear_crash_state() {
        use crate::graph::NodeLifecycle;

        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("https://a.com".to_string(), Point2D::new(0.0, 0.0));
        let wv = test_webview_id();
        app.map_webview_to_node(wv, key);
        app.apply_reducer_intents([GraphIntent::WebViewCrashed {
            webview_id: wv,
            reason: "boom".to_string(),
            has_backtrace: false,
        }]);

        app.apply_reducer_intents([GraphIntent::PromoteNodeToActive {
            key,
            cause: LifecycleCause::UserSelect,
        }]);

        assert_eq!(
            app.workspace.domain.graph.get_node(key).unwrap().lifecycle,
            NodeLifecycle::Active
        );
        assert!(app.runtime_crash_state_for_node(key).is_none());
    }

    #[test]
    fn test_crash_path_requires_explicit_clear_before_auto_reactivate() {
        use crate::graph::NodeLifecycle;

        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("https://a.com".to_string(), Point2D::new(0.0, 0.0));
        let wv = test_webview_id();
        app.map_webview_to_node(wv, key);
        app.apply_reducer_intents([GraphIntent::WebViewCrashed {
            webview_id: wv,
            reason: "boom".to_string(),
            has_backtrace: false,
        }]);

        app.apply_reducer_intents([GraphIntent::PromoteNodeToActive {
            key,
            cause: LifecycleCause::ActiveTileVisible,
        }]);
        assert_eq!(
            app.workspace.domain.graph.get_node(key).unwrap().lifecycle,
            NodeLifecycle::Cold
        );
        assert!(app.runtime_crash_state_for_node(key).is_some());

        app.apply_reducer_intents([GraphIntent::PromoteNodeToActive {
            key,
            cause: LifecycleCause::UserSelect,
        }]);
        assert_eq!(
            app.workspace.domain.graph.get_node(key).unwrap().lifecycle,
            NodeLifecycle::Active
        );
        assert!(app.runtime_crash_state_for_node(key).is_none());
    }

    #[test]
    fn test_policy_explicit_close_clears_crash_state() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("https://a.com".to_string(), Point2D::new(0.0, 0.0));
        let wv = test_webview_id();
        app.map_webview_to_node(wv, key);
        app.apply_reducer_intents([GraphIntent::WebViewCrashed {
            webview_id: wv,
            reason: "boom".to_string(),
            has_backtrace: false,
        }]);
        assert!(app.runtime_crash_state_for_node(key).is_some());

        app.apply_reducer_intents([GraphIntent::DemoteNodeToCold {
            key,
            cause: LifecycleCause::ExplicitClose,
        }]);

        assert!(app.runtime_crash_state_for_node(key).is_none());
    }

    #[test]
    fn test_mark_runtime_blocked_and_expiry_unblocks_node() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("https://a.com".to_string(), Point2D::new(0.0, 0.0));
        let retry_at = Instant::now() + Duration::from_millis(5);
        app.apply_reducer_intents([GraphIntent::MarkRuntimeBlocked {
            key,
            reason: RuntimeBlockReason::CreateRetryExhausted,
            retry_at: Some(retry_at),
        }]);
        assert!(app.is_runtime_blocked(key, Instant::now()));
        assert!(!app.is_runtime_blocked(key, retry_at + Duration::from_millis(1)));
        assert!(app.runtime_block_state_for_node(key).is_none());
    }

    #[test]
    fn test_promote_clears_runtime_block_state() {
        use crate::graph::NodeLifecycle;

        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("https://a.com".to_string(), Point2D::new(0.0, 0.0));
        app.apply_reducer_intents([
            GraphIntent::MarkRuntimeBlocked {
                key,
                reason: RuntimeBlockReason::CreateRetryExhausted,
                retry_at: Some(Instant::now() + Duration::from_secs(1)),
            },
            GraphIntent::PromoteNodeToActive {
                key,
                cause: LifecycleCause::Restore,
            },
        ]);
        assert_eq!(
            app.workspace.domain.graph.get_node(key).unwrap().lifecycle,
            NodeLifecycle::Active
        );
        assert!(app.runtime_block_state_for_node(key).is_none());
    }

    #[test]
    fn test_promote_to_active_removes_warm_cache_membership() {
        use crate::graph::NodeLifecycle;

        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("a".to_string(), Point2D::new(0.0, 0.0));
        let wv_id = test_webview_id();
        app.map_webview_to_node(wv_id, key);
        app.demote_node_to_warm(key);

        assert_eq!(
            app.workspace.domain.graph.get_node(key).unwrap().lifecycle,
            NodeLifecycle::Warm
        );
        assert_eq!(app.workspace.warm_cache_lru, vec![key]);

        app.promote_node_to_active(key);
        assert_eq!(
            app.workspace.domain.graph.get_node(key).unwrap().lifecycle,
            NodeLifecycle::Active
        );
        assert!(app.workspace.warm_cache_lru.is_empty());
    }

    #[test]
    fn test_cache_churn_during_lifecycle_transitions_preserves_lifecycle_contract() {
        use crate::graph::NodeLifecycle;

        let mut app = GraphBrowserApp::new_for_testing();
        let key = app.workspace.domain.graph.add_node(
            "https://cache-lifecycle.example".to_string(),
            Point2D::new(0.0, 0.0),
        );

        for idx in 0..32 {
            app.workspace.runtime_caches.insert_parsed_metadata(
                format!("lifecycle:meta:{idx}"),
                serde_json::json!({"i": idx}),
            );
            app.workspace
                .runtime_caches
                .insert_suggestions(format!("lifecycle:suggest:{idx}"), vec![format!("q{idx}")]);
        }
        let _ = app
            .workspace
            .runtime_caches
            .get_parsed_metadata("lifecycle:meta:0");
        let _ = app
            .workspace
            .runtime_caches
            .get_parsed_metadata("lifecycle:meta:missing");

        app.apply_reducer_intents([GraphIntent::PromoteNodeToActive {
            key,
            cause: LifecycleCause::Restore,
        }]);
        assert_eq!(
            app.workspace.domain.graph.get_node(key).unwrap().lifecycle,
            NodeLifecycle::Active
        );
        assert_eq!(app.lifecycle_counts(), (1, 0, 0, 0));

        for idx in 32..64 {
            app.workspace
                .runtime_caches
                .insert_thumbnail(key, vec![idx as u8; 4]);
        }

        app.apply_reducer_intents([GraphIntent::DemoteNodeToWarm {
            key,
            cause: LifecycleCause::WorkspaceRetention,
        }]);
        assert_eq!(
            app.workspace.domain.graph.get_node(key).unwrap().lifecycle,
            NodeLifecycle::Warm
        );
        assert_eq!(app.lifecycle_counts(), (0, 1, 0, 0));

        app.apply_reducer_intents([GraphIntent::DemoteNodeToCold {
            key,
            cause: LifecycleCause::MemoryPressureCritical,
        }]);
        assert_eq!(
            app.workspace.domain.graph.get_node(key).unwrap().lifecycle,
            NodeLifecycle::Cold
        );
        assert_eq!(app.lifecycle_counts(), (0, 0, 1, 0));
        assert!(app.runtime_block_state_for_node(key).is_none());
    }

    #[test]
    fn test_unmap_webview_removes_warm_cache_membership() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("a".to_string(), Point2D::new(0.0, 0.0));
        let wv_id = test_webview_id();
        app.map_webview_to_node(wv_id, key);
        app.demote_node_to_warm(key);
        assert_eq!(app.workspace.warm_cache_lru, vec![key]);

        let _ = app.unmap_webview(wv_id);
        assert!(app.workspace.warm_cache_lru.is_empty());
    }

    #[test]
    fn test_take_warm_cache_evictions_respects_lru_and_limit() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key_a = app
            .workspace
            .domain
            .graph
            .add_node("a".to_string(), Point2D::new(0.0, 0.0));
        let key_b = app
            .workspace
            .domain
            .graph
            .add_node("b".to_string(), Point2D::new(1.0, 0.0));
        let key_c = app
            .workspace
            .domain
            .graph
            .add_node("c".to_string(), Point2D::new(2.0, 0.0));

        app.map_webview_to_node(test_webview_id(), key_a);
        app.demote_node_to_warm(key_a);
        app.map_webview_to_node(test_webview_id(), key_b);
        app.demote_node_to_warm(key_b);
        app.map_webview_to_node(test_webview_id(), key_c);
        app.demote_node_to_warm(key_c);

        assert_eq!(app.workspace.warm_cache_lru, vec![key_a, key_b, key_c]);

        app.workspace.warm_cache_limit = 2;
        let evicted = app.take_warm_cache_evictions();
        assert_eq!(evicted, vec![key_a]);
        assert_eq!(app.workspace.warm_cache_lru, vec![key_b, key_c]);
    }

    #[test]
    fn test_take_active_webview_evictions_respects_limit_and_protection() {
        use std::collections::HashSet;

        let mut app = GraphBrowserApp::new_for_testing();
        let key_a = app
            .workspace
            .domain
            .graph
            .add_node("a".to_string(), Point2D::new(0.0, 0.0));
        let key_b = app
            .workspace
            .domain
            .graph
            .add_node("b".to_string(), Point2D::new(1.0, 0.0));
        let key_c = app
            .workspace
            .domain
            .graph
            .add_node("c".to_string(), Point2D::new(2.0, 0.0));
        let key_d = app
            .workspace
            .domain
            .graph
            .add_node("d".to_string(), Point2D::new(3.0, 0.0));

        for key in [key_a, key_b, key_c, key_d] {
            app.promote_node_to_active(key);
            app.map_webview_to_node(test_webview_id(), key);
        }

        app.workspace.active_webview_limit = 3;
        let protected = HashSet::from([key_a]);
        let evicted = app.take_active_webview_evictions(&protected);

        assert_eq!(evicted.len(), 1);
        assert!(!protected.contains(&evicted[0]));
    }

    #[test]
    fn test_take_active_webview_evictions_with_lower_limit() {
        use std::collections::HashSet;

        let mut app = GraphBrowserApp::new_for_testing();
        let key_a = app
            .workspace
            .domain
            .graph
            .add_node("a".to_string(), Point2D::new(0.0, 0.0));
        let key_b = app
            .workspace
            .domain
            .graph
            .add_node("b".to_string(), Point2D::new(1.0, 0.0));
        let key_c = app
            .workspace
            .domain
            .graph
            .add_node("c".to_string(), Point2D::new(2.0, 0.0));

        for key in [key_a, key_b, key_c] {
            app.promote_node_to_active(key);
            app.map_webview_to_node(test_webview_id(), key);
        }

        let evicted = app.take_active_webview_evictions_with_limit(1, &HashSet::new());
        assert_eq!(evicted.len(), 2);
    }

    #[test]
    fn test_webview_crashed_demotes_node_and_unmaps_webview() {
        use crate::graph::NodeLifecycle;

        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("a".to_string(), Point2D::new(0.0, 0.0));
        let wv_id = test_webview_id();

        app.promote_node_to_active(key);
        app.map_webview_to_node(wv_id, key);
        assert!(matches!(
            app.workspace.domain.graph.get_node(key).unwrap().lifecycle,
            NodeLifecycle::Active
        ));

        app.apply_reducer_intents([GraphIntent::WebViewCrashed {
            webview_id: wv_id,
            reason: "gpu reset".to_string(),
            has_backtrace: false,
        }]);

        assert!(matches!(
            app.workspace.domain.graph.get_node(key).unwrap().lifecycle,
            NodeLifecycle::Cold
        ));
        assert_eq!(
            app.runtime_crash_state_for_node(key)
                .and_then(|state| state.message.as_deref()),
            Some("gpu reset")
        );
        assert!(app.get_node_for_webview(wv_id).is_none());
        assert!(app.get_webview_for_node(key).is_none());

        app.apply_reducer_intents([GraphIntent::PromoteNodeToActive {
            key,
            cause: LifecycleCause::Restore,
        }]);
        assert!(matches!(
            app.workspace.domain.graph.get_node(key).unwrap().lifecycle,
            NodeLifecycle::Active
        ));
        assert!(app.runtime_crash_state_for_node(key).is_none());
    }

    #[test]
    fn test_clear_graph_clears_runtime_crash_state() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("https://a.com".to_string(), Point2D::new(0.0, 0.0));
        let wv_id = test_webview_id();
        app.map_webview_to_node(wv_id, key);
        app.apply_reducer_intents([GraphIntent::WebViewCrashed {
            webview_id: wv_id,
            reason: "boom".to_string(),
            has_backtrace: true,
        }]);
        assert!(app.runtime_crash_state_for_node(key).is_some());

        app.clear_graph();
        assert!(app.runtime_crash_state_for_node(key).is_none());
    }

    // --- TEST-1: webview mapping ---

    #[test]
    fn test_webview_mapping_bidirectional() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("a".to_string(), Point2D::new(0.0, 0.0));
        let wv_id = test_webview_id();

        app.map_webview_to_node(wv_id, key);

        assert_eq!(app.get_node_for_webview(wv_id), Some(key));
        assert_eq!(app.get_webview_for_node(key), Some(wv_id));
    }

    #[test]
    fn test_unmap_webview() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("a".to_string(), Point2D::new(0.0, 0.0));
        let wv_id = test_webview_id();

        app.map_webview_to_node(wv_id, key);
        let unmapped_key = app.unmap_webview(wv_id);

        assert_eq!(unmapped_key, Some(key));
        assert!(app.get_node_for_webview(wv_id).is_none());
        assert!(app.get_webview_for_node(key).is_none());
    }

    #[test]
    fn test_unmap_nonexistent_webview() {
        let mut app = GraphBrowserApp::new_for_testing();
        let wv_id = test_webview_id();

        assert_eq!(app.unmap_webview(wv_id), None);
    }

    #[test]
    fn test_webview_node_mappings_iterator() {
        let mut app = GraphBrowserApp::new_for_testing();
        let k1 = app
            .workspace
            .domain
            .graph
            .add_node("a".to_string(), Point2D::new(0.0, 0.0));
        let k2 = app
            .workspace
            .domain
            .graph
            .add_node("b".to_string(), Point2D::new(100.0, 0.0));
        let wv1 = test_webview_id();
        let wv2 = test_webview_id();

        app.map_webview_to_node(wv1, k1);
        app.map_webview_to_node(wv2, k2);

        let mappings: Vec<_> = app.webview_node_mappings().collect();
        assert_eq!(mappings.len(), 2);
    }

    // --- TEST-1: get_single_selected_node ---

    #[test]
    fn test_get_single_selected_node_one() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("a".to_string(), Point2D::new(0.0, 0.0));
        app.select_node(key, false);

        assert_eq!(app.get_single_selected_node(), Some(key));
    }

    #[test]
    fn test_get_single_selected_node_none() {
        let app = GraphBrowserApp::new_for_testing();
        assert_eq!(app.get_single_selected_node(), None);
    }

    #[test]
    fn test_get_single_selected_node_multi() {
        let mut app = GraphBrowserApp::new_for_testing();
        let k1 = app
            .workspace
            .domain
            .graph
            .add_node("a".to_string(), Point2D::new(0.0, 0.0));
        let k2 = app
            .workspace
            .domain
            .graph
            .add_node("b".to_string(), Point2D::new(100.0, 0.0));
        app.select_node(k1, false);
        app.select_node(k2, true);

        assert_eq!(app.get_single_selected_node(), None);
    }

    // --- TEST-1: update_node_url_and_log ---

    #[test]
    fn test_update_node_url_and_log() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("old-url".to_string(), Point2D::new(0.0, 0.0));

        let old = app.update_node_url_and_log(key, "new-url".to_string());

        assert_eq!(old, Some("old-url".to_string()));
        assert_eq!(
            app.workspace.domain.graph.get_node(key).unwrap().url,
            "new-url"
        );
        // url_to_node should be updated
        assert!(
            app.workspace
                .domain
                .graph
                .get_node_by_url("new-url")
                .is_some()
        );
        assert!(
            app.workspace
                .domain
                .graph
                .get_node_by_url("old-url")
                .is_none()
        );
    }

    #[test]
    fn test_update_node_url_nonexistent() {
        let mut app = GraphBrowserApp::new_for_testing();
        let fake_key = NodeKey::new(999);

        assert_eq!(app.update_node_url_and_log(fake_key, "x".to_string()), None);
    }

    #[test]
    fn test_new_from_dir_recovers_logged_graph() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().to_path_buf();

        {
            let mut store = GraphStore::open(path.clone()).unwrap();
            let id_a = Uuid::new_v4();
            let id_b = Uuid::new_v4();
            store.log_mutation(&LogEntry::AddNode {
                node_id: id_a.to_string(),
                url: "https://a.com".to_string(),
                position_x: 10.0,
                position_y: 20.0,
            });
            store.log_mutation(&LogEntry::AddNode {
                node_id: id_b.to_string(),
                url: "https://b.com".to_string(),
                position_x: 30.0,
                position_y: 40.0,
            });
            store.log_mutation(&LogEntry::AddEdge {
                from_node_id: id_a.to_string(),
                to_node_id: id_b.to_string(),
                edge_type: PersistedEdgeType::Hyperlink,
                edge_label: None,
            });
        }

        let app = GraphBrowserApp::new_from_dir(path);
        assert!(app.has_recovered_graph());
        assert_eq!(app.workspace.domain.graph.node_count(), 2);
        assert_eq!(app.workspace.domain.graph.edge_count(), 1);
        assert!(
            app.workspace
                .domain
                .graph
                .get_node_by_url("https://a.com")
                .is_some()
        );
        assert!(
            app.workspace
                .domain
                .graph
                .get_node_by_url("https://b.com")
                .is_some()
        );
    }

    #[test]
    fn test_new_from_dir_scans_placeholder_ids_from_recovery() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().to_path_buf();

        {
            let mut store = GraphStore::open(path.clone()).unwrap();
            let id = Uuid::new_v4();
            store.log_mutation(&LogEntry::AddNode {
                node_id: id.to_string(),
                url: "about:blank#5".to_string(),
                position_x: 0.0,
                position_y: 0.0,
            });
        }

        let mut app = GraphBrowserApp::new_from_dir(path);
        let key = app.create_new_node_near_center();
        let node = app.workspace.domain.graph.get_node(key).unwrap();
        assert_eq!(node.url, "about:blank#6");
    }

    #[test]
    fn test_clear_graph_and_persistence_in_memory_reset() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("https://a.com".to_string(), Point2D::new(0.0, 0.0));
        app.select_node(key, false);

        app.clear_graph_and_persistence();

        assert_eq!(app.workspace.domain.graph.node_count(), 0);
        assert!(app.focused_selection().is_empty());
    }

    #[test]
    fn test_clear_graph_and_persistence_wipes_store() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().to_path_buf();

        {
            let mut app = GraphBrowserApp::new_from_dir(path.clone());
            app.add_node_and_sync("https://persisted.com".to_string(), Point2D::new(1.0, 2.0));
            app.take_snapshot();
            app.clear_graph_and_persistence();
        }

        let recovered = GraphBrowserApp::new_from_dir(path);
        assert!(!recovered.has_recovered_graph());
        assert_eq!(recovered.workspace.domain.graph.node_count(), 0);
    }

    #[test]
    fn test_resolve_frame_open_deterministic_fallback_without_recency_match() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("https://example.com".to_string(), Point2D::new(0.0, 0.0));
        let node_id = app.workspace.domain.graph.get_node(key).unwrap().id;

        let mut index = HashMap::new();
        index.insert(
            node_id,
            BTreeSet::from([
                "workspace-z".to_string(),
                "workspace-a".to_string(),
                "workspace-m".to_string(),
            ]),
        );
        app.init_membership_index(index);
        app.workspace
            .node_last_active_workspace
            .insert(node_id, (99, "workspace-missing".to_string()));

        for _ in 0..5 {
            assert_eq!(
                app.resolve_workspace_open(key, None),
                FrameOpenAction::RestoreFrame {
                    name: "workspace-a".to_string(),
                    node: key
                }
            );
        }
    }

    #[test]
    fn test_resolve_frame_open_reason_honors_preferred_frame() {
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

        let (action, reason) = app.resolve_workspace_open_with_reason(key, Some("beta"));
        assert_eq!(
            action,
            FrameOpenAction::RestoreFrame {
                name: "beta".to_string(),
                node: key
            }
        );
        assert_eq!(reason, FrameOpenReason::PreferredFrame);
    }

    #[test]
    fn test_resolve_frame_open_reason_recent_membership() {
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
        app.note_workspace_activated("beta", [key]);

        let (_, reason) = app.resolve_workspace_open_with_reason(key, None);
        assert_eq!(reason, FrameOpenReason::RecentMembership);
    }

    #[test]
    fn test_resolve_frame_open_reason_no_membership() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("https://example.com".to_string(), Point2D::new(0.0, 0.0));
        let (_, reason) = app.resolve_workspace_open_with_reason(key, None);
        assert_eq!(reason, FrameOpenReason::NoMembership);
    }

    #[test]
    fn test_new_from_dir_loads_persisted_toast_anchor_preference() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().to_path_buf();
        {
            let mut store = GraphStore::open(path.clone()).unwrap();
            store
                .save_workspace_layout_json(GraphBrowserApp::SETTINGS_TOAST_ANCHOR_NAME, "top-left")
                .unwrap();
        }

        let app = GraphBrowserApp::new_from_dir(path);
        assert_eq!(
            app.workspace.toast_anchor_preference,
            ToastAnchorPreference::TopLeft
        );
    }

    #[test]
    fn test_keyboard_pan_step_persists_across_restart() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().to_path_buf();

        let mut app = GraphBrowserApp::new_from_dir(path.clone());
        app.set_keyboard_pan_step(27.0);
        drop(app);

        let reopened = GraphBrowserApp::new_from_dir(path);
        assert!((reopened.keyboard_pan_step() - 27.0).abs() < 0.001);
    }

    #[test]
    fn test_keyboard_pan_input_mode_persists_across_restart() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().to_path_buf();

        let mut app = GraphBrowserApp::new_from_dir(path.clone());
        app.set_keyboard_pan_input_mode(KeyboardPanInputMode::ArrowsOnly);
        drop(app);

        let reopened = GraphBrowserApp::new_from_dir(path);
        assert_eq!(
            reopened.keyboard_pan_input_mode(),
            KeyboardPanInputMode::ArrowsOnly
        );
    }

    #[test]
    fn test_camera_pan_inertia_settings_persist_across_restart() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().to_path_buf();

        let mut app = GraphBrowserApp::new_from_dir(path.clone());
        app.set_camera_pan_inertia_enabled(false);
        app.set_camera_pan_inertia_damping(0.92);
        drop(app);

        let reopened = GraphBrowserApp::new_from_dir(path);
        assert!(!reopened.camera_pan_inertia_enabled());
        assert!((reopened.camera_pan_inertia_damping() - 0.92).abs() < 0.001);
    }

    #[test]
    fn test_camera_starts_manual_without_pending_fit_command() {
        let app = GraphBrowserApp::new_for_testing();
        assert!(app.pending_camera_command().is_none());
    }

    #[test]
    fn test_set_omnibar_settings_persist_across_restart() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().to_path_buf();

        let mut app = GraphBrowserApp::new_from_dir(path.clone());
        app.set_omnibar_preferred_scope(OmnibarPreferredScope::ProviderDefault);
        app.set_omnibar_non_at_order(OmnibarNonAtOrderPreset::ProviderThenContextualThenGlobal);
        drop(app);

        let reopened = GraphBrowserApp::new_from_dir(path);
        assert_eq!(
            reopened.workspace.omnibar_preferred_scope,
            OmnibarPreferredScope::ProviderDefault
        );
        assert_eq!(
            reopened.workspace.omnibar_non_at_order,
            OmnibarNonAtOrderPreset::ProviderThenContextualThenGlobal
        );
    }

    #[test]
    fn test_wry_enabled_defaults_to_false() {
        let app = GraphBrowserApp::new_for_testing();
        assert!(!app.wry_enabled());
    }

    #[test]
    fn test_wry_enabled_persists_across_restart() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().to_path_buf();

        let mut app = GraphBrowserApp::new_from_dir(path.clone());
        app.set_wry_enabled(true);
        drop(app);

        let reopened = GraphBrowserApp::new_from_dir(path);
        assert!(reopened.wry_enabled());
    }

    #[test]
    fn test_set_snapshot_interval_secs_updates_store() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().to_path_buf();
        let mut app = GraphBrowserApp::new_from_dir(path);

        app.set_snapshot_interval_secs(45).unwrap();
        assert_eq!(app.snapshot_interval_secs(), Some(45));
    }

    #[test]
    fn test_set_snapshot_interval_secs_without_persistence_fails() {
        let mut app = GraphBrowserApp::new_for_testing();
        assert!(app.set_snapshot_interval_secs(45).is_err());
        assert_eq!(app.snapshot_interval_secs(), None);
    }

    #[test]
    fn test_registry_component_defaults_persist_across_restart() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().to_path_buf();

        let mut app = GraphBrowserApp::new_from_dir(path.clone());
        app.set_default_registry_lens_id(Some("lens:default"));
        app.set_default_registry_physics_id(Some("physics:gas"));
        app.set_default_registry_theme_id(Some("theme:dark"));
        drop(app);

        let reopened = GraphBrowserApp::new_from_dir(path);
        assert_eq!(reopened.default_registry_lens_id(), Some("lens:default"));
        assert_eq!(reopened.default_registry_physics_id(), Some("physics:gas"));
        assert_eq!(reopened.default_registry_theme_id(), Some("theme:dark"));
    }

    #[test]
    fn test_set_view_lens_preserves_direct_values_when_lens_id_missing() {
        let mut app = GraphBrowserApp::new_for_testing();

        let view_id = GraphViewId::new();
        app.workspace
            .views
            .insert(view_id, GraphViewState::new_with_id(view_id, "Test"));

        let lens = LensConfig {
            name: "Custom Lens".to_string(),
            lens_id: None,
            physics: PhysicsProfile::gas(),
            layout: LayoutMode::Grid { gap: 24.0 },
            theme: Some(ThemeData {
                background_rgb: (1, 2, 3),
                accent_rgb: (4, 5, 6),
                font_scale: 1.3,
                stroke_width: 2.0,
            }),
            filters: Vec::new(),
        };

        app.apply_reducer_intents([GraphIntent::SetViewLens { view_id, lens }]);

        let resolved = &app.workspace.views.get(&view_id).unwrap().lens;
        assert_eq!(resolved.physics.name, "Gas");
        assert!(matches!(resolved.layout, LayoutMode::Grid { gap: 24.0 }));
        assert_eq!(
            resolved.theme.as_ref().map(|theme| theme.background_rgb),
            Some((1, 2, 3))
        );
    }

    #[test]
    fn test_set_view_lens_applies_persisted_lens_default_when_lens_id_missing() {
        let mut app = GraphBrowserApp::new_for_testing();
        app.set_default_registry_lens_id(Some("lens:default"));

        let view_id = GraphViewId::new();
        app.workspace
            .views
            .insert(view_id, GraphViewState::new_with_id(view_id, "Test"));

        let lens = LensConfig {
            name: "Custom Lens".to_string(),
            lens_id: None,
            physics: PhysicsProfile::default(),
            layout: LayoutMode::Free,
            theme: None,
            filters: Vec::new(),
        };

        app.apply_reducer_intents([GraphIntent::SetViewLens { view_id, lens }]);

        let resolved = &app.workspace.views.get(&view_id).unwrap().lens;
        assert_eq!(resolved.lens_id.as_deref(), Some("lens:default"));
        assert_eq!(resolved.name, "Default");
        assert_eq!(resolved.physics.name, "Liquid");
        assert!(matches!(resolved.layout, LayoutMode::Free));
        assert_eq!(
            resolved.theme.as_ref().map(|theme| theme.background_rgb),
            Some((20, 20, 25))
        );
    }

    // --- UpdateNodeMimeHint / UpdateNodeAddressKind intent tests ---

    #[test]
    fn update_node_mime_hint_intent_sets_hint_on_node() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("file:///doc.pdf".to_string(), Point2D::new(0.0, 0.0));

        app.apply_reducer_intents([GraphIntent::UpdateNodeMimeHint {
            key,
            mime_hint: Some("application/pdf".to_string()),
        }]);

        let node = app.workspace.domain.graph.get_node(key).unwrap();
        assert_eq!(node.mime_hint.as_deref(), Some("application/pdf"));
    }

    #[test]
    fn set_zoom_updates_focused_view_camera_when_present() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_id = GraphViewId::new();
        app.workspace
            .views
            .insert(view_id, GraphViewState::new_with_id(view_id, "Focused"));
        app.workspace.focused_view = Some(view_id);

        app.apply_reducer_intents([GraphIntent::SetZoom { zoom: 2.5 }]);

        assert!((app.workspace.views[&view_id].camera.current_zoom - 2.5).abs() < 0.0001);
        assert!((app.workspace.camera.current_zoom - Camera::new().current_zoom).abs() < 0.0001);
    }

    #[test]
    fn set_zoom_with_missing_focused_view_is_noop() {
        let mut app = GraphBrowserApp::new_for_testing();
        let missing_view_id = GraphViewId::new();
        app.workspace.focused_view = Some(missing_view_id);
        let before = app.workspace.camera.current_zoom;

        app.apply_reducer_intents([GraphIntent::SetZoom { zoom: 3.0 }]);

        assert!((app.workspace.camera.current_zoom - before).abs() < 0.0001);
    }

    #[test]
    fn update_node_mime_hint_intent_can_clear_hint() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("file:///doc.pdf".to_string(), Point2D::new(0.0, 0.0));

        // Set then clear.
        app.apply_reducer_intents([GraphIntent::UpdateNodeMimeHint {
            key,
            mime_hint: Some("application/pdf".to_string()),
        }]);
        app.apply_reducer_intents([GraphIntent::UpdateNodeMimeHint {
            key,
            mime_hint: None,
        }]);

        let node = app.workspace.domain.graph.get_node(key).unwrap();
        assert!(node.mime_hint.is_none());
    }

    #[test]
    fn update_node_address_kind_intent_sets_kind_on_node() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("https://example.com".to_string(), Point2D::new(0.0, 0.0));

        app.apply_reducer_intents([GraphIntent::UpdateNodeAddressKind {
            key,
            kind: crate::graph::AddressKind::Custom,
        }]);

        let node = app.workspace.domain.graph.get_node(key).unwrap();
        assert_eq!(node.address_kind, crate::graph::AddressKind::Custom);
    }

    #[test]
    fn node_created_with_http_url_has_http_address_kind_after_add_node() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("https://example.com".to_string(), Point2D::new(0.0, 0.0));
        let node = app.workspace.domain.graph.get_node(key).unwrap();
        assert_eq!(node.address_kind, crate::graph::AddressKind::Http);
    }

    #[test]
    fn node_created_with_file_pdf_url_gets_mime_hint_after_add_node() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app.workspace.domain.graph.add_node(
            "file:///home/user/doc.pdf".to_string(),
            Point2D::new(0.0, 0.0),
        );
        let node = app.workspace.domain.graph.get_node(key).unwrap();
        assert_eq!(node.mime_hint.as_deref(), Some("application/pdf"));
        assert_eq!(node.address_kind, crate::graph::AddressKind::File);
    }

    #[test]
    fn update_node_url_and_log_refreshes_mime_hint_and_address_kind() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("https://example.com".to_string(), Point2D::new(0.0, 0.0));

        // Start with HTTP
        assert_eq!(
            app.workspace
                .domain
                .graph
                .get_node(key)
                .unwrap()
                .address_kind,
            crate::graph::AddressKind::Http
        );

        // Navigate to a local PDF file
        app.update_node_url_and_log(key, "file:///home/user/report.pdf".to_string());

        let node = app.workspace.domain.graph.get_node(key).unwrap();
        assert_eq!(node.address_kind, crate::graph::AddressKind::File);
        assert_eq!(node.mime_hint.as_deref(), Some("application/pdf"));
    }

    #[test]
    fn undo_redo_create_node_and_remove_selected_nodes() {
        let mut app = GraphBrowserApp::new_for_testing();

        app.apply_reducer_intents([GraphIntent::CreateNodeNearCenter]);
        assert_eq!(app.workspace.domain.graph.node_count(), 1);
        assert_eq!(app.undo_stack_len(), 1);
        assert_eq!(app.redo_stack_len(), 0);

        app.apply_reducer_intents([GraphIntent::Undo]);
        assert_eq!(app.workspace.domain.graph.node_count(), 0);
        assert_eq!(app.undo_stack_len(), 0);
        assert_eq!(app.redo_stack_len(), 1);

        app.apply_reducer_intents([GraphIntent::Redo]);
        assert_eq!(app.workspace.domain.graph.node_count(), 1);
        assert_eq!(app.undo_stack_len(), 1);
        assert_eq!(app.redo_stack_len(), 0);

        app.apply_reducer_intents([GraphIntent::RemoveSelectedNodes]);
        assert_eq!(app.workspace.domain.graph.node_count(), 0);
        assert_eq!(app.undo_stack_len(), 2);

        app.apply_reducer_intents([GraphIntent::Undo]);
        assert_eq!(app.workspace.domain.graph.node_count(), 1);
        assert_eq!(app.redo_stack_len(), 1);
    }

    #[test]
    fn undo_redo_set_node_url_round_trips_original_value() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("https://old.example".to_string(), Point2D::new(0.0, 0.0));

        app.apply_reducer_intents([GraphIntent::SetNodeUrl {
            key,
            new_url: "https://new.example".to_string(),
        }]);
        assert_eq!(
            app.workspace.domain.graph.get_node(key).unwrap().url,
            "https://new.example"
        );
        assert_eq!(app.undo_stack_len(), 1);

        app.apply_reducer_intents([GraphIntent::Undo]);
        assert_eq!(
            app.workspace.domain.graph.get_node(key).unwrap().url,
            "https://old.example"
        );

        app.apply_reducer_intents([GraphIntent::Redo]);
        assert_eq!(
            app.workspace.domain.graph.get_node(key).unwrap().url,
            "https://new.example"
        );
    }

    #[test]
    fn undo_redo_user_grouped_edge_create_and_remove_round_trip() {
        let mut app = GraphBrowserApp::new_for_testing();
        let from = app
            .workspace
            .domain
            .graph
            .add_node("https://a.example".to_string(), Point2D::new(0.0, 0.0));
        let to = app
            .workspace
            .domain
            .graph
            .add_node("https://b.example".to_string(), Point2D::new(10.0, 0.0));

        app.apply_reducer_intents([GraphIntent::CreateUserGroupedEdge {
            from,
            to,
            label: Some("registry-label".to_string()),
        }]);
        assert!(app.workspace.domain.graph.edges().any(|edge| {
            edge.from == from && edge.to == to && edge.edge_type == EdgeType::UserGrouped
        }));
        let edge_key = app.workspace.domain.graph.find_edge_key(from, to).unwrap();
        let payload = app.workspace.domain.graph.get_edge(edge_key).unwrap();
        assert_eq!(payload.label(), Some("registry-label"));
        assert_eq!(app.undo_stack_len(), 1);

        app.apply_reducer_intents([GraphIntent::Undo]);
        assert!(!app.workspace.domain.graph.edges().any(|edge| {
            edge.from == from && edge.to == to && edge.edge_type == EdgeType::UserGrouped
        }));

        app.apply_reducer_intents([GraphIntent::Redo]);
        assert!(app.workspace.domain.graph.edges().any(|edge| {
            edge.from == from && edge.to == to && edge.edge_type == EdgeType::UserGrouped
        }));

        app.apply_reducer_intents([GraphIntent::RemoveEdge {
            from,
            to,
            edge_type: EdgeType::UserGrouped,
        }]);
        assert!(!app.workspace.domain.graph.edges().any(|edge| {
            edge.from == from && edge.to == to && edge.edge_type == EdgeType::UserGrouped
        }));
        assert_eq!(app.undo_stack_len(), 2);

        app.apply_reducer_intents([GraphIntent::Undo]);
        assert!(app.workspace.domain.graph.edges().any(|edge| {
            edge.from == from && edge.to == to && edge.edge_type == EdgeType::UserGrouped
        }));
    }

    #[test]
    fn undo_redo_queue_history_frame_layout_restore_requests() {
        let mut app = GraphBrowserApp::new_for_testing();
        let before_layout = "{\"frame\":\"before\"}";
        let after_layout = "{\"frame\":\"after\"}";

        app.mark_session_frame_layout_json(before_layout);
        app.apply_reducer_intents([GraphIntent::CreateNodeNearCenter]);
        app.mark_session_frame_layout_json(after_layout);

        app.apply_reducer_intents([GraphIntent::Undo]);
        assert_eq!(
            app.take_pending_history_frame_layout_json(),
            Some(before_layout.to_string())
        );

        app.mark_session_frame_layout_json(before_layout);
        app.apply_reducer_intents([GraphIntent::Redo]);
        assert_eq!(
            app.take_pending_history_frame_layout_json(),
            Some(after_layout.to_string())
        );
    }

    #[test]
    fn set_node_url_noop_does_not_capture_undo_checkpoint() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = app
            .workspace
            .domain
            .graph
            .add_node("https://same.example".to_string(), Point2D::new(0.0, 0.0));

        app.apply_reducer_intents([GraphIntent::SetNodeUrl {
            key,
            new_url: "https://same.example".to_string(),
        }]);

        assert_eq!(app.undo_stack_len(), 0);
        assert_eq!(app.redo_stack_len(), 0);
    }

    #[cfg(feature = "diagnostics")]
    #[test]
    fn stale_camera_target_enqueue_emits_blocked_channel() {
        let mut diagnostics = crate::shell::desktop::runtime::diagnostics::DiagnosticsState::new();
        let mut app = GraphBrowserApp::new_for_testing();
        let stale_target = GraphViewId::new();
        app.clear_pending_camera_command();

        app.request_camera_command_for_view(Some(stale_target), CameraCommand::Fit);

        assert!(app.pending_camera_command().is_none());
        diagnostics.force_drain_for_tests();
        let snapshot = diagnostics.snapshot_json_for_tests().to_string();
        assert!(
            snapshot.contains("runtime.ui.graph.camera_command_blocked_missing_target_view"),
            "expected stale camera target enqueue to emit blocked channel"
        );
    }

    #[test]
    fn pending_workbench_intent_queue_is_explicit_and_drainable() {
        let mut app = GraphBrowserApp::new_for_testing();

        app.enqueue_workbench_intent(WorkbenchIntent::CycleFocusRegion);
        assert_eq!(app.pending_workbench_intent_count_for_tests(), 1);

        let drained = app.take_pending_workbench_intents();
        assert!(matches!(
            drained.as_slice(),
            [WorkbenchIntent::CycleFocusRegion]
        ));
        assert_eq!(app.pending_workbench_intent_count_for_tests(), 0);
    }

    #[test]
    fn detach_node_to_split_requests_flow_through_workbench_intents() {
        let mut app = GraphBrowserApp::new_for_testing();
        let key = NodeKey::new(7);

        app.request_detach_node_to_split(key);

        let drained = app.take_pending_workbench_intents();
        assert!(matches!(
            drained.as_slice(),
            [WorkbenchIntent::DetachNodeToSplit { key: drained_key }] if *drained_key == key
        ));
    }

    #[test]
    fn workbench_intents_do_not_bypass_reducer_mutation_entry() {
        let mut app = GraphBrowserApp::new_for_testing();
        let before_count = app.workspace.domain.graph.node_count();

        app.enqueue_workbench_intent(WorkbenchIntent::OpenGraphUrl {
            url: GraphAddress::graph("missing-graph").to_string(),
        });

        assert_eq!(
            app.workspace.domain.graph.node_count(),
            before_count,
            "enqueuing a workbench intent must not mutate reducer-owned graph state"
        );

        app.apply_reducer_intents([GraphIntent::CreateNodeNearCenter]);

        assert_eq!(
            app.workspace.domain.graph.node_count(),
            before_count + 1,
            "graph mutation must flow through apply_reducer_intents"
        );
    }

    #[test]
    fn graph_view_layout_manager_entry_exit_and_toggle_intents_update_state() {
        let mut app = GraphBrowserApp::new_for_testing();
        assert!(!app.graph_view_layout_manager_active());

        app.apply_reducer_intents([GraphIntent::EnterGraphViewLayoutManager]);
        assert!(app.graph_view_layout_manager_active());

        app.apply_reducer_intents([GraphIntent::ExitGraphViewLayoutManager]);
        assert!(!app.graph_view_layout_manager_active());

        app.apply_reducer_intents([GraphIntent::ToggleGraphViewLayoutManager]);
        assert!(app.graph_view_layout_manager_active());
    }

    #[test]
    fn graph_view_slot_lifecycle_create_rename_move_archive_restore() {
        let mut app = GraphBrowserApp::new_for_testing();
        let anchor = GraphViewId::new();
        app.ensure_graph_view_registered(anchor);

        app.apply_reducer_intents([GraphIntent::CreateGraphViewSlot {
            anchor_view: Some(anchor),
            direction: GraphViewLayoutDirection::Right,
            open_mode: None,
        }]);

        let mut slots = app.graph_view_slots_for_tests();
        assert_eq!(slots.len(), 2);
        let created = slots
            .iter()
            .find(|slot| slot.view_id != anchor)
            .expect("expected created graph-view slot")
            .view_id;

        app.apply_reducer_intents([GraphIntent::RenameGraphViewSlot {
            view_id: created,
            name: "Investigation View".to_string(),
        }]);
        slots = app.graph_view_slots_for_tests();
        assert!(
            slots
                .iter()
                .any(|slot| slot.view_id == created && slot.name == "Investigation View")
        );

        app.apply_reducer_intents([GraphIntent::MoveGraphViewSlot {
            view_id: created,
            row: 3,
            col: 2,
        }]);
        slots = app.graph_view_slots_for_tests();
        assert!(
            slots
                .iter()
                .any(|slot| slot.view_id == created && slot.row == 3 && slot.col == 2)
        );

        app.apply_reducer_intents([GraphIntent::ArchiveGraphViewSlot { view_id: created }]);
        slots = app.graph_view_slots_for_tests();
        assert!(
            slots
                .iter()
                .any(|slot| slot.view_id == created && slot.archived)
        );

        app.apply_reducer_intents([GraphIntent::RestoreGraphViewSlot {
            view_id: created,
            row: 4,
            col: 4,
        }]);
        slots = app.graph_view_slots_for_tests();
        assert!(slots.iter().any(|slot| {
            slot.view_id == created && !slot.archived && slot.row == 4 && slot.col == 4
        }));
    }

    #[test]
    fn graph_view_slot_move_guard_prevents_coordinate_collision() {
        let mut app = GraphBrowserApp::new_for_testing();
        let left = GraphViewId::new();
        let right = GraphViewId::new();
        app.ensure_graph_view_registered(left);
        app.ensure_graph_view_registered(right);

        app.apply_reducer_intents([GraphIntent::MoveGraphViewSlot {
            view_id: left,
            row: 1,
            col: 1,
        }]);
        app.apply_reducer_intents([GraphIntent::MoveGraphViewSlot {
            view_id: right,
            row: 2,
            col: 2,
        }]);

        app.apply_reducer_intents([GraphIntent::MoveGraphViewSlot {
            view_id: right,
            row: 1,
            col: 1,
        }]);

        let slots = app.graph_view_slots_for_tests();
        let right_slot = slots
            .iter()
            .find(|slot| slot.view_id == right)
            .expect("right slot should exist");
        assert_eq!(
            (right_slot.row, right_slot.col),
            (2, 2),
            "move into occupied slot should be rejected"
        );
    }

    #[test]
    fn route_graph_view_to_workbench_enqueues_open_graph_view_pane_intent() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_id = GraphViewId::new();
        app.ensure_graph_view_registered(view_id);

        app.apply_reducer_intents([GraphIntent::RouteGraphViewToWorkbench {
            view_id,
            mode: PendingTileOpenMode::SplitHorizontal,
        }]);

        let drained = app.take_pending_workbench_intents();
        assert!(matches!(
            drained.as_slice(),
            [WorkbenchIntent::OpenGraphViewPane {
                view_id: routed,
                mode: PendingTileOpenMode::SplitHorizontal
            }] if *routed == view_id
        ));
    }

    #[test]
    fn workbench_authority_bridge_intent_is_classified_for_reducer_warning() {
        assert_eq!(
            GraphIntent::RouteGraphViewToWorkbench {
                view_id: GraphViewId::new(),
                mode: PendingTileOpenMode::SplitHorizontal,
            }
            .workbench_authority_bridge_name(),
            Some("RouteGraphViewToWorkbench")
        );
        assert_eq!(
            GraphIntent::CreateNodeNearCenter.workbench_authority_bridge_name(),
            None
        );
    }

    #[test]
    fn persisted_graph_view_layout_manager_shape_round_trips() {
        let view_id = GraphViewId::new();
        let persisted = PersistedGraphViewLayoutManager {
            version: PersistedGraphViewLayoutManager::VERSION,
            active: true,
            slots: vec![GraphViewSlot {
                view_id,
                name: "Primary".to_string(),
                row: 0,
                col: 1,
                archived: false,
            }],
        };

        let json = serde_json::to_string(&persisted).expect("persisted manager should serialize");
        let decoded: PersistedGraphViewLayoutManager =
            serde_json::from_str(&json).expect("persisted manager should deserialize");

        assert_eq!(decoded.version, PersistedGraphViewLayoutManager::VERSION);
        assert!(decoded.active);
        assert_eq!(decoded.slots.len(), 1);
        assert_eq!(decoded.slots[0].view_id, view_id);
        assert_eq!(decoded.slots[0].name, "Primary");
    }

    #[cfg(feature = "diagnostics")]
    #[test]
    fn toggle_command_palette_emits_ux_navigation_transition_channel() {
        let mut diagnostics = crate::shell::desktop::runtime::diagnostics::DiagnosticsState::new();
        let mut app = GraphBrowserApp::new_for_testing();
        assert!(!app.workspace.show_command_palette);

        app.toggle_command_palette();

        assert!(app.workspace.show_command_palette);
        diagnostics.force_drain_for_tests();
        let snapshot = diagnostics.snapshot_json_for_tests().to_string();
        assert!(
            snapshot.contains("ux:navigation_transition"),
            "expected ux:navigation_transition when command palette focus surface toggles"
        );
    }

    #[cfg(feature = "diagnostics")]
    #[test]
    fn toggle_help_panel_emits_ux_navigation_transition_channel() {
        let mut diagnostics = crate::shell::desktop::runtime::diagnostics::DiagnosticsState::new();
        let mut app = GraphBrowserApp::new_for_testing();
        assert!(!app.workspace.show_help_panel);

        app.apply_reducer_intents([GraphIntent::ToggleHelpPanel]);

        assert!(app.workspace.show_help_panel);
        diagnostics.force_drain_for_tests();
        let snapshot = diagnostics.snapshot_json_for_tests().to_string();
        assert!(
            snapshot.contains("ux:navigation_transition"),
            "expected ux:navigation_transition when help panel focus surface toggles"
        );
    }

    #[cfg(feature = "diagnostics")]
    #[test]
    fn toggle_radial_menu_emits_ux_navigation_transition_channel() {
        let mut diagnostics = crate::shell::desktop::runtime::diagnostics::DiagnosticsState::new();
        let mut app = GraphBrowserApp::new_for_testing();
        assert!(!app.workspace.show_radial_menu);

        app.apply_reducer_intents([GraphIntent::ToggleRadialMenu]);

        assert!(app.workspace.show_radial_menu);
        diagnostics.force_drain_for_tests();
        let snapshot = diagnostics.snapshot_json_for_tests().to_string();
        assert!(
            snapshot.contains("ux:navigation_transition"),
            "expected ux:navigation_transition when radial menu focus surface toggles"
        );
    }

    #[cfg(feature = "diagnostics")]
    #[test]
    fn set_file_tree_selected_rows_emits_ux_navigation_transition_channel() {
        let mut diagnostics = crate::shell::desktop::runtime::diagnostics::DiagnosticsState::new();
        let mut app = GraphBrowserApp::new_for_testing();

        app.apply_reducer_intents([GraphIntent::SetFileTreeSelectedRows {
            rows: vec!["row:test".to_string()],
        }]);

        diagnostics.force_drain_for_tests();
        let snapshot = diagnostics.snapshot_json_for_tests().to_string();
        assert!(
            snapshot.contains("ux:navigation_transition"),
            "expected ux:navigation_transition when file tree selected rows change"
        );
    }

    #[cfg(feature = "diagnostics")]
    #[test]
    fn clear_graph_focused_view_reset_emits_ux_navigation_transition_channel() {
        let mut diagnostics = crate::shell::desktop::runtime::diagnostics::DiagnosticsState::new();
        let mut app = GraphBrowserApp::new_for_testing();
        app.workspace.focused_view = Some(GraphViewId::new());

        app.clear_graph();

        assert!(app.workspace.focused_view.is_none());
        diagnostics.force_drain_for_tests();
        let snapshot = diagnostics.snapshot_json_for_tests().to_string();
        assert!(
            snapshot.contains("ux:navigation_transition"),
            "expected ux:navigation_transition when clear_graph resets focused view"
        );
    }

    #[cfg(feature = "diagnostics")]
    #[test]
    fn set_highlighted_edge_emits_ux_navigation_transition_channel() {
        let mut diagnostics = crate::shell::desktop::runtime::diagnostics::DiagnosticsState::new();
        let mut app = GraphBrowserApp::new_for_testing();
        let from = app.add_node_and_sync("from".into(), Point2D::new(0.0, 0.0));
        let to = app.add_node_and_sync("to".into(), Point2D::new(10.0, 0.0));

        app.apply_reducer_intents([GraphIntent::SetHighlightedEdge { from, to }]);

        diagnostics.force_drain_for_tests();
        let snapshot = diagnostics.snapshot_json_for_tests().to_string();
        assert!(
            snapshot.contains("ux:navigation_transition"),
            "expected ux:navigation_transition when edge highlight focus changes"
        );
    }

    #[cfg(feature = "diagnostics")]
    #[test]
    fn webview_url_changed_emits_history_traversal_recorded_channel() {
        let mut diagnostics = crate::shell::desktop::runtime::diagnostics::DiagnosticsState::new();
        let mut app = GraphBrowserApp::new_for_testing();
        let a = app
            .workspace
            .domain
            .graph
            .add_node("https://a.com".into(), Point2D::new(0.0, 0.0));
        let _b = app
            .workspace
            .domain
            .graph
            .add_node("https://b.com".into(), Point2D::new(100.0, 0.0));
        let wv = test_webview_id();
        app.map_webview_to_node(wv, a);

        app.apply_reducer_intents([GraphIntent::WebViewUrlChanged {
            webview_id: wv,
            new_url: "https://b.com".into(),
        }]);

        diagnostics.force_drain_for_tests();
        let snapshot = diagnostics.snapshot_json_for_tests().to_string();
        assert!(
            snapshot.contains("history.traversal.recorded"),
            "expected history.traversal.recorded when traversal append succeeds"
        );
    }

    #[cfg(feature = "diagnostics")]
    #[test]
    fn remove_history_edge_emits_history_archive_dissolved_appended_channel() {
        let mut diagnostics = crate::shell::desktop::runtime::diagnostics::DiagnosticsState::new();
        let dir = TempDir::new().expect("temp dir");
        let mut app = GraphBrowserApp::new_from_dir(dir.path().to_path_buf());

        let a = app.add_node_and_sync("https://a.com".to_string(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("https://b.com".to_string(), Point2D::new(100.0, 0.0));
        let wv = test_webview_id();
        app.map_webview_to_node(wv, a);
        app.apply_reducer_intents([GraphIntent::WebViewUrlChanged {
            webview_id: wv,
            new_url: "https://b.com".into(),
        }]);

        app.apply_reducer_intents([GraphIntent::RemoveEdge {
            from: a,
            to: b,
            edge_type: EdgeType::History,
        }]);

        diagnostics.force_drain_for_tests();
        let snapshot = diagnostics.snapshot_json_for_tests().to_string();
        assert!(
            snapshot.contains("history.archive.dissolved_appended"),
            "expected history.archive.dissolved_appended when dissolution archive receives entries"
        );
    }

    #[cfg(feature = "diagnostics")]
    #[test]
    fn clear_and_export_history_without_persistence_emit_failure_channels() {
        let mut diagnostics = crate::shell::desktop::runtime::diagnostics::DiagnosticsState::new();
        let mut app = GraphBrowserApp::new_for_testing();

        app.apply_reducer_intents([
            GraphIntent::ClearHistoryTimeline,
            GraphIntent::ClearHistoryDissolved,
            GraphIntent::ExportHistoryTimeline,
            GraphIntent::ExportHistoryDissolved,
        ]);

        diagnostics.force_drain_for_tests();
        let snapshot = diagnostics.snapshot_json_for_tests().to_string();
        assert!(
            snapshot.contains("history.archive.clear_failed"),
            "expected history.archive.clear_failed when clear is requested without persistence"
        );
        assert!(
            snapshot.contains("history.archive.export_failed"),
            "expected history.archive.export_failed when export is requested without persistence"
        );
    }

    #[cfg(feature = "diagnostics")]
    #[test]
    fn history_preview_and_replay_intents_emit_timeline_channels() {
        let mut diagnostics = crate::shell::desktop::runtime::diagnostics::DiagnosticsState::new();
        let mut app = GraphBrowserApp::new_for_testing();

        app.apply_reducer_intents([
            GraphIntent::EnterHistoryTimelinePreview,
            GraphIntent::HistoryTimelinePreviewIsolationViolation {
                detail: "forbidden side effect".to_string(),
            },
            GraphIntent::HistoryTimelineReplayStarted,
            GraphIntent::HistoryTimelineReplayFinished {
                succeeded: true,
                error: None,
            },
            GraphIntent::HistoryTimelineReplayFinished {
                succeeded: false,
                error: Some("replay checksum mismatch".to_string()),
            },
            GraphIntent::ExitHistoryTimelinePreview,
            GraphIntent::HistoryTimelineReturnToPresentFailed {
                detail: "state restore mismatch".to_string(),
            },
        ]);

        diagnostics.force_drain_for_tests();
        let snapshot = diagnostics.snapshot_json_for_tests().to_string();
        for channel in [
            "history.timeline.preview_entered",
            "history.timeline.preview_exited",
            "history.timeline.preview_isolation_violation",
            "history.timeline.replay_started",
            "history.timeline.replay_succeeded",
            "history.timeline.replay_failed",
            "history.timeline.return_to_present_failed",
        ] {
            assert!(
                snapshot.contains(channel),
                "expected diagnostics snapshot to contain {channel}"
            );
        }
    }

    #[cfg(feature = "diagnostics")]
    #[test]
    fn clear_highlighted_edge_emits_ux_navigation_transition_channel() {
        let mut diagnostics = crate::shell::desktop::runtime::diagnostics::DiagnosticsState::new();
        let mut app = GraphBrowserApp::new_for_testing();
        let from = app.add_node_and_sync("from".into(), Point2D::new(0.0, 0.0));
        let to = app.add_node_and_sync("to".into(), Point2D::new(10.0, 0.0));
        app.workspace.highlighted_graph_edge = Some((from, to));

        app.apply_reducer_intents([GraphIntent::ClearHighlightedEdge]);

        diagnostics.force_drain_for_tests();
        let snapshot = diagnostics.snapshot_json_for_tests().to_string();
        assert!(
            snapshot.contains("ux:navigation_transition"),
            "expected ux:navigation_transition when edge highlight focus clears"
        );
    }

    #[test]
    fn resolve_clip_route_accepts_both_canonical_and_legacy_internal_schemes() {
        assert_eq!(
            GraphBrowserApp::resolve_clip_route("verso://clip/clip-a").as_deref(),
            Some("clip-a")
        );
        assert_eq!(
            GraphBrowserApp::resolve_clip_route("graphshell://clip/clip-b").as_deref(),
            Some("clip-b")
        );
        assert!(GraphBrowserApp::resolve_clip_route("verso://clip").is_none());
    }

    #[test]
    fn pending_clip_request_queue_roundtrips_single_value() {
        let mut app = GraphBrowserApp::new_for_testing();
        app.request_open_clip_by_id("clip-roundtrip");

        assert_eq!(
            app.take_pending_open_clip_request().as_deref(),
            Some("clip-roundtrip")
        );
        assert!(app.take_pending_open_clip_request().is_none());
    }

    #[test]
    fn queued_node_context_target_supports_replace_peek_and_clear() {
        let mut app = GraphBrowserApp::new_for_testing();
        let first = NodeKey::new(9);
        let second = NodeKey::new(10);

        app.set_pending_node_context_target(Some(first));
        app.set_pending_node_context_target(Some(second));

        assert_eq!(app.pending_node_context_target(), Some(second));

        app.set_pending_node_context_target(None);
        assert!(app.pending_node_context_target().is_none());
    }

    #[test]
    fn queued_choose_frame_picker_supports_replace_clear_and_sanitization() {
        use euclid::default::Point2D;

        let mut app = GraphBrowserApp::new_for_testing();
        let anchor = app.add_node_and_sync("https://anchor.example".into(), Point2D::new(0.0, 0.0));
        let sibling =
            app.add_node_and_sync("https://sibling.example".into(), Point2D::new(20.0, 0.0));

        app.request_choose_frame_picker_for_mode(anchor, ChooseFramePickerMode::AddNodeToFrame);
        assert_eq!(
            app.choose_frame_picker_request(),
            Some(ChooseFramePickerRequest {
                node: anchor,
                mode: ChooseFramePickerMode::AddNodeToFrame,
            })
        );
        assert!(app.choose_frame_picker_exact_nodes().is_none());

        app.request_add_exact_selection_to_frame_picker(vec![sibling, anchor]);
        assert_eq!(
            app.choose_frame_picker_request(),
            Some(ChooseFramePickerRequest {
                node: anchor,
                mode: ChooseFramePickerMode::AddExactSelectionToFrame,
            })
        );
        assert_eq!(
            app.choose_frame_picker_exact_nodes(),
            Some(&[anchor, sibling][..])
        );

        app.select_node(sibling, false);
        app.apply_reducer_intents([GraphIntent::RemoveSelectedNodes]);

        assert_eq!(
            app.choose_frame_picker_request(),
            Some(ChooseFramePickerRequest {
                node: anchor,
                mode: ChooseFramePickerMode::AddExactSelectionToFrame,
            })
        );
        assert_eq!(app.choose_frame_picker_exact_nodes(), Some(&[anchor][..]));

        app.clear_choose_frame_picker();
        assert!(app.choose_frame_picker_request().is_none());
        assert!(app.choose_frame_picker_exact_nodes().is_none());
    }

    #[test]
    fn queued_unsaved_frame_prompt_supports_replace_action_and_resolution() {
        let mut app = GraphBrowserApp::new_for_testing();

        app.request_unsaved_frame_prompt(UnsavedFramePromptRequest::FrameSwitch {
            name: "workspace-a".to_string(),
            focus_node: Some(NodeKey::new(7)),
        });
        app.request_unsaved_frame_prompt(UnsavedFramePromptRequest::FrameSwitch {
            name: "workspace-b".to_string(),
            focus_node: Some(NodeKey::new(8)),
        });

        assert_eq!(
            app.unsaved_frame_prompt_request(),
            Some(&UnsavedFramePromptRequest::FrameSwitch {
                name: "workspace-b".to_string(),
                focus_node: Some(NodeKey::new(8)),
            })
        );

        app.set_unsaved_frame_prompt_action(UnsavedFramePromptAction::ProceedWithoutSaving);

        assert_eq!(
            app.take_unsaved_frame_prompt_resolution(),
            Some((
                UnsavedFramePromptRequest::FrameSwitch {
                    name: "workspace-b".to_string(),
                    focus_node: Some(NodeKey::new(8)),
                },
                UnsavedFramePromptAction::ProceedWithoutSaving,
            ))
        );
        assert!(app.unsaved_frame_prompt_request().is_none());
        assert!(app.take_unsaved_frame_prompt_resolution().is_none());
    }

    #[test]
    fn resolve_graph_route_accepts_graph_scheme() {
        assert_eq!(
            GraphBrowserApp::resolve_graph_route("graph://graph-main").as_deref(),
            Some("graph-main")
        );
        assert!(GraphBrowserApp::resolve_graph_route("graph://").is_none());
    }

    #[test]
    fn resolve_node_route_accepts_node_scheme_with_uuid() {
        let node_id = Uuid::new_v4();
        let route = format!("node://{}", node_id);
        assert_eq!(GraphBrowserApp::resolve_node_route(&route), Some(node_id));
        assert!(GraphBrowserApp::resolve_node_route("node://not-a-uuid").is_none());
    }

    #[test]
    fn resolve_view_route_accepts_graph_target_variant() {
        let route = GraphBrowserApp::resolve_view_route("verso://view/graph/graph-main")
            .expect("view graph route should parse");
        assert!(matches!(
            route,
            ViewRouteTarget::Graph(graph_id) if graph_id == "graph-main"
        ));
    }

    #[test]
    fn resolve_view_route_accepts_node_target_variant() {
        let node_id = Uuid::new_v4();
        let route =
            GraphBrowserApp::resolve_view_route(format!("verso://view/node/{node_id}").as_str())
                .expect("view node route should parse");
        assert!(matches!(route, ViewRouteTarget::Node(parsed) if parsed == node_id));
    }

    #[test]
    fn resolve_view_route_accepts_note_target_variant() {
        let note_id = Uuid::new_v4();
        let route =
            GraphBrowserApp::resolve_view_route(format!("verso://view/note/{note_id}").as_str())
                .expect("view note route should parse");
        assert!(matches!(
            route,
            ViewRouteTarget::Note(parsed) if parsed.as_uuid() == note_id
        ));
    }

    #[test]
    fn opening_help_panel_closes_other_capture_surfaces() {
        let mut app = GraphBrowserApp::new_for_testing();
        app.workspace.show_command_palette = true;
        app.workspace.show_radial_menu = true;
        app.set_pending_node_context_target(Some(NodeKey::new(9)));

        app.apply_reducer_intents([GraphIntent::ToggleHelpPanel]);

        assert!(app.workspace.show_help_panel);
        assert!(!app.workspace.show_command_palette);
        assert!(!app.workspace.show_radial_menu);
        assert!(app.pending_node_context_target().is_none());
    }

    #[test]
    fn opening_command_palette_closes_other_capture_surfaces() {
        let mut app = GraphBrowserApp::new_for_testing();
        app.workspace.show_help_panel = true;
        app.workspace.show_radial_menu = true;
        app.set_pending_node_context_target(Some(NodeKey::new(10)));

        app.open_command_palette();

        assert!(app.workspace.show_command_palette);
        assert!(!app.workspace.show_help_panel);
        assert!(!app.workspace.show_radial_menu);
        assert!(app.pending_node_context_target().is_none());
    }

    #[test]
    fn toggle_command_palette_reducer_path_enqueues_workbench_intent() {
        let mut app = GraphBrowserApp::new_for_testing();

        app.apply_reducer_intents([GraphIntent::ToggleCommandPalette]);

        assert!(!app.workspace.show_command_palette);
        let drained = app.take_pending_workbench_intents();
        assert!(matches!(
            drained.as_slice(),
            [WorkbenchIntent::ToggleCommandPalette]
        ));
    }

    #[test]
    fn opening_radial_menu_closes_other_capture_surfaces() {
        let mut app = GraphBrowserApp::new_for_testing();
        app.workspace.show_help_panel = true;
        app.workspace.show_command_palette = true;

        app.apply_reducer_intents([GraphIntent::ToggleRadialMenu]);

        assert!(app.workspace.show_radial_menu);
        assert!(!app.workspace.show_help_panel);
        assert!(!app.workspace.show_command_palette);
    }
}
