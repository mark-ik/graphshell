/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Vocabulary types for graph application state.
//!
//! Pure data definitions (enums, structs) shared across the graph-app modules.
//! No logic beyond `Display`/`FromStr` impls lives here.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::str::FromStr;
use std::time::{Instant, SystemTime};

use uuid::Uuid;

use crate::graph::NodeKey;

use super::graph_views::GraphViewId;
use super::graph_mutations::NoteId;
use super::selection::SelectionUpdateMode;
use super::workbench_layout_policy::{
    NavigatorHostScope, SurfaceFirstUsePolicy, SurfaceHostId, UxConfigMode,
    WorkbenchLayoutConstraint,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewRouteTarget {
    GraphPane(GraphViewId),
    Graph(String),
    Note(NoteId),
    Node(Uuid),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NavigatorProjectionSeedSource {
    #[default]
    GraphContainment,
    SavedViewCollections,
    ContainmentRelations,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NavigatorSortMode {
    #[default]
    Manual,
    NameAscending,
    NameDescending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NavigatorProjectionMode {
    #[default]
    Workbench,
    Containment,
    Semantic,
    AllNodes,
}

impl NavigatorProjectionMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Workbench => "Workbench",
            Self::Containment => "Containment",
            Self::Semantic => "Semantic",
            Self::AllNodes => "All Nodes",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigatorProjectionTarget {
    Node(NodeKey),
    SavedView(GraphViewId),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NavigatorProjectionState {
    pub mode: NavigatorProjectionMode,
    pub projection_seed_source: NavigatorProjectionSeedSource,
    pub expanded_rows: HashSet<String>,
    pub collapsed_rows: HashSet<String>,
    pub selected_rows: HashSet<String>,
    pub sort_mode: NavigatorSortMode,
    pub root_filter: Option<String>,
    pub row_targets: HashMap<String, NavigatorProjectionTarget>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphReaderModeState {
    Map {
        focused_node: Option<NodeKey>,
    },
    Room {
        node_key: NodeKey,
        return_map_node: Option<NodeKey>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GraphReaderState {
    pub(crate) mode_override: Option<GraphReaderModeState>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphSearchOrigin {
    Manual,
    SemanticTag,
    AnchorSlice,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphSearchHistoryEntry {
    pub query: String,
    pub filter_mode: bool,
    pub origin: GraphSearchOrigin,
    pub neighborhood_anchor: Option<NodeKey>,
    pub neighborhood_depth: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphSearchRequest {
    pub query: String,
    pub filter_mode: bool,
    pub origin: GraphSearchOrigin,
    pub neighborhood_anchor: Option<NodeKey>,
    pub neighborhood_depth: u8,
    pub record_history: bool,
    pub toast_message: Option<String>,
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
    QuarterPane,
    HalfPane,
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
    /// Fit the camera to the graphlet containing the primary selected node.
    /// Falls back to `FitSelection` when no selection exists, or `Fit` when
    /// the graphlet cannot be resolved.
    FitGraphlet,
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
pub enum ContextCommandSurfacePreference {
    RadialPalette,
    ContextPalette,
}

impl_display_from_str!(ContextCommandSurfacePreference {
    ContextCommandSurfacePreference::RadialPalette => "radial-palette",
    ContextCommandSurfacePreference::ContextPalette => "context-palette",
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WorkbenchDisplayMode {
    #[default]
    Split,
    Dedicated,
}

impl_display_from_str!(WorkbenchDisplayMode {
    WorkbenchDisplayMode::Split => "split",
    WorkbenchDisplayMode::Dedicated => "dedicated",
});

#[derive(Debug, Clone)]
pub enum WorkbenchIntent {
    OpenCommandPalette,
    CloseCommandPalette,
    ToggleCommandPalette,
    SetWorkbenchOverlayVisible {
        visible: bool,
    },
    CloseHelpPanel,
    ToggleHelpPanel,
    CloseRadialMenu,
    ToggleRadialMenu,
    CycleFocusRegion,
    SelectTile {
        tile_id: egui_tiles::TileId,
    },
    UpdateTileSelection {
        tile_id: egui_tiles::TileId,
        mode: SelectionUpdateMode,
    },
    ClearTileSelection,
    GroupSelectedTiles,
    OpenToolPane {
        kind: crate::shell::desktop::workbench::pane_model::ToolPaneState,
    },
    SetWorkbenchDisplayMode {
        mode: WorkbenchDisplayMode,
    },
    SetWorkbenchPinned {
        pinned: bool,
    },
    SetLayoutConstraintDraft {
        surface_host: SurfaceHostId,
        constraint: WorkbenchLayoutConstraint,
    },
    CommitLayoutConstraintDraft {
        surface_host: SurfaceHostId,
    },
    DiscardLayoutConstraintDraft {
        surface_host: SurfaceHostId,
    },
    SetNavigatorHostScope {
        surface_host: SurfaceHostId,
        scope: NavigatorHostScope,
    },
    SetFirstUsePolicy {
        policy: SurfaceFirstUsePolicy,
    },
    SuppressFirstUsePromptForSession {
        surface_host: SurfaceHostId,
    },
    DismissFrameSplitOfferForSession {
        frame_name: String,
    },
    RenameFrame {
        from: String,
        to: String,
    },
    DeleteFrame {
        frame_name: String,
    },
    SaveFrameSnapshotNamed {
        name: String,
    },
    SaveCurrentFrame,
    PruneEmptyFrames,
    RestoreFrame {
        name: String,
    },
    ClosePane {
        pane: crate::shell::desktop::workbench::pane_model::PaneId,
        restore_previous_focus: bool,
    },
    /// Close a node tile and demote its node to `NodeLifecycle::Cold`.
    ///
    /// Unlike `ClosePane`, this preserves all graph edges so the node remains
    /// part of its durable graphlet.  The tile is removed from the tree; the
    /// node's webview is released; the node lifecycle transitions to `Cold`.
    DismissTile {
        pane: crate::shell::desktop::workbench::pane_model::PaneId,
    },
    /// Merge warm tiles of a durable graphlet into a single `Container::Tabs`.
    ///
    /// Triggered after a durable edge (`UserGrouped` or `FrameMember`) is created
    /// between two nodes that already have warm tiles in different containers.
    /// The reconciler computes the full durable graphlet for `node`, finds all
    /// warm members, and moves any tiles that are outside the graphlet's primary
    /// tab container into it.
    ///
    /// This is a no-op if all warm tiles are already in the same container, or if
    /// fewer than two graphlet members have warm tiles.
    ReconcileGraphletTiles {
        node: NodeKey,
    },
    /// Restore a pane-rest member back into its semantic tab group.
    RestorePaneToSemanticTabGroup {
        pane: crate::shell::desktop::workbench::pane_model::PaneId,
        group_id: Uuid,
    },
    /// Collapse a semantic tab group back to pane-rest form while retaining its
    /// semantic membership overlay.
    CollapseSemanticTabGroupToPaneRest {
        group_id: Uuid,
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
        /// Optional: the frame member node that should be made active in
        /// the tile group after opening.  `None` defaults to the first member.
        focus_node: Option<crate::graph::NodeKey>,
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
    FocusGraphView {
        view_id: GraphViewId,
    },
    OpenFrameAsSplit {
        node_key: NodeKey,
        frame_name: String,
    },
    SetFrameSplitOfferSuppressed {
        frame: NodeKey,
        suppressed: bool,
    },
    MoveFrameLayoutHint {
        frame: NodeKey,
        from_index: usize,
        to_index: usize,
    },
    RemoveFrameLayoutHint {
        frame: NodeKey,
        hint_index: usize,
    },
    SetNavigatorSpecialtyView {
        host: SurfaceHostId,
        kind: Option<crate::graph::GraphletKind>,
    },
    TransferSelectedNodesToGraphView {
        source_view: GraphViewId,
        destination_view: GraphViewId,
    },
    ToggleOverviewPlane,
    OpenNoteUrl {
        url: String,
    },
    OpenNodeInPane {
        node: NodeKey,
        pane: crate::shell::desktop::workbench::pane_model::PaneId,
    },
    SelectNavigatorNode {
        node_key: NodeKey,
        row_key: Option<String>,
    },
    ActivateNavigatorNode {
        node_key: NodeKey,
        row_key: Option<String>,
    },
    DismissNavigatorNode {
        node_key: NodeKey,
        row_key: Option<String>,
    },
    SwitchNavigatorNodeSurface {
        node_key: NodeKey,
        row_key: Option<String>,
    },
    SetPanePresentationMode {
        pane: crate::shell::desktop::workbench::pane_model::PaneId,
        mode: crate::shell::desktop::workbench::pane_model::PanePresentationMode,
    },
    PromoteEphemeralPane {
        target_tile_context:
            crate::shell::desktop::workbench::pane_model::FloatingPaneTargetTileContext,
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
    ApplyLayoutConstraint {
        surface_host: SurfaceHostId,
        constraint: WorkbenchLayoutConstraint,
    },
    SetSurfaceConfigMode {
        surface_host: SurfaceHostId,
        mode: UxConfigMode,
    },
    DetachNodeToSplit {
        key: NodeKey,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeTabGroupMetadata {
    pub group_id: Uuid,
    pub pane_ids: Vec<crate::shell::desktop::workbench::pane_model::PaneId>,
    pub active_pane_id: Option<crate::shell::desktop::workbench::pane_model::PaneId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeFrameTabSemantics {
    pub version: u32,
    pub tab_groups: Vec<RuntimeTabGroupMetadata>,
}

#[derive(Debug, Clone, Default)]
pub struct WorkbenchTileSelectionState {
    pub selected_tile_ids: HashSet<egui_tiles::TileId>,
    pub primary_tile_id: Option<egui_tiles::TileId>,
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
