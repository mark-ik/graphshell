use std::path::PathBuf;
use std::time::Instant;

use euclid::default::Point2D;

use crate::graph::{NodeKey, RelationSelector};

use super::{
    CameraCommand, ChooseFramePickerRequest, ClipboardCopyRequest, EdgeCommand, GraphSearchRequest,
    GraphViewId, GraphViewLayoutDirection, HostOpenRequest, KeyboardZoomRequest, LensConfig,
    LifecycleCause, MemoryPressureLevel, NavigatorContainmentRelationSource, NavigatorSortMode,
    NoteId, PendingConnectedOpenScope, PendingNodeOpenRequest, PendingTileOpenMode, RendererId,
    SelectionUpdateMode, ToolSurfaceReturnTarget, UnsavedFramePromptAction,
    UnsavedFramePromptRequest, ViewDimension,
};
use crate::shell::desktop::workbench::pane_model::{
    FloatingPaneTargetTileContext, PaneId, PanePresentationMode,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserCommand {
    Back,
    Forward,
    Reload,
    StopLoad,
    ZoomIn,
    ZoomOut,
    ZoomReset,
    Close,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserCommandTarget {
    FocusedInput,
    ChromeProjection { fallback_node: Option<NodeKey> },
}

#[derive(Debug, Clone, PartialEq)]
pub enum AppCommand {
    BrowserCommand {
        command: BrowserCommand,
        target: BrowserCommandTarget,
    },
    ReloadAll,
    CameraCommand {
        command: CameraCommand,
        target_view: Option<GraphViewId>,
    },
    WheelZoom {
        target_view: GraphViewId,
        delta: f32,
        anchor_screen: Option<(f32, f32)>,
    },
    KeyboardZoom {
        request: KeyboardZoomRequest,
        target_view: GraphViewId,
    },
    UnsavedWorkspacePrompt {
        request: UnsavedFramePromptRequest,
        action: Option<UnsavedFramePromptAction>,
    },
    ChooseWorkspacePicker {
        request: ChooseFramePickerRequest,
        exact_nodes: Option<Vec<NodeKey>>,
    },
    NodeContextTarget {
        target: NodeKey,
    },
    FrameContextTarget {
        frame_name: String,
    },
    ApplyGraphSearch {
        request: GraphSearchRequest,
    },
    CommandSurfaceReturnTarget {
        target: ToolSurfaceReturnTarget,
    },
    TransientSurfaceReturnTarget {
        target: ToolSurfaceReturnTarget,
    },
    ToolSurfaceReturnTarget {
        target: ToolSurfaceReturnTarget,
    },
    RestoreTransientSurfaceFocus,
    SaveWorkspaceSnapshot,
    SaveWorkspaceSnapshotNamed {
        name: String,
    },
    RestoreWorkspaceSnapshotNamed {
        name: String,
    },
    RestoreHistoryWorkspaceLayout {
        layout_json: String,
    },
    RestoreWorkspaceOpen {
        request: PendingNodeOpenRequest,
    },
    AddNodeToWorkspace {
        node: NodeKey,
        workspace_name: String,
    },
    AddConnectedToWorkspace {
        seed_nodes: Vec<NodeKey>,
        workspace_name: String,
    },
    AddExactToWorkspace {
        nodes: Vec<NodeKey>,
        workspace_name: String,
    },
    OpenConnected {
        source: NodeKey,
        mode: PendingTileOpenMode,
        scope: PendingConnectedOpenScope,
    },
    OpenNode {
        request: PendingNodeOpenRequest,
    },
    SaveGraphSnapshotNamed {
        name: String,
    },
    RestoreGraphSnapshotNamed {
        name: String,
    },
    RestoreGraphSnapshotLatest,
    DeleteGraphSnapshotNamed {
        name: String,
    },
    OpenNote {
        note_id: NoteId,
    },
    OpenClip {
        clip_id: String,
    },
    PruneEmptyWorkspaces,
    KeepLatestNamedWorkspaces {
        keep: usize,
    },
    ClipboardCopy {
        request: ClipboardCopyRequest,
    },
    ProtocolProbe {
        key: NodeKey,
        url: Option<String>,
    },
    SwitchDataDir {
        path: PathBuf,
    },
}

#[derive(Debug, Clone)]
pub enum ViewAction {
    ToggleCameraPositionFitLock,
    ToggleCameraZoomFitLock,
    RequestFitToScreen,
    RequestZoomIn,
    RequestZoomOut,
    RequestZoomReset,
    RequestZoomToSelected,
    RequestZoomToGraphlet,
    ReheatPhysics,
    UpdateSelection {
        keys: Vec<NodeKey>,
        mode: SelectionUpdateMode,
    },
    SelectAll,
    SetNodePosition {
        key: NodeKey,
        position: Point2D<f32>,
    },
    SetZoom {
        zoom: f32,
    },
    SetHighlightedEdge {
        from: NodeKey,
        to: NodeKey,
    },
    ClearHighlightedEdge,
    SetSelectedFrame {
        frame_name: Option<String>,
    },
    SetNodeFormDraft {
        key: NodeKey,
        form_draft: Option<String>,
    },
    SetNodeThumbnail {
        key: NodeKey,
        png_bytes: Vec<u8>,
        width: u32,
        height: u32,
    },
    SetNodeFavicon {
        key: NodeKey,
        rgba: Vec<u8>,
        width: u32,
        height: u32,
    },
    SetWorkbenchEdgeProjection {
        selectors: Vec<RelationSelector>,
    },
    SetViewEdgeProjectionOverride {
        view_id: GraphViewId,
        selectors: Option<Vec<RelationSelector>>,
    },
    SetSelectionEdgeProjectionOverride {
        view_id: Option<GraphViewId>,
        selectors: Option<Vec<RelationSelector>>,
    },
    SetNavigatorContainmentRelationSource {
        source: NavigatorContainmentRelationSource,
    },
    SetNavigatorSortMode {
        sort_mode: NavigatorSortMode,
    },
    SetNavigatorRootFilter {
        root_filter: Option<String>,
    },
    SetNavigatorSelectedRows {
        rows: Vec<String>,
    },
    SetNavigatorExpandedRows {
        rows: Vec<String>,
    },
    RebuildNavigatorProjection,
}

/// Navigator-shaped adapter intents that map directly to existing GraphIntent
/// Navigator projection operations.
#[derive(Debug, Clone)]
pub enum NavigatorProjectionIntent {
    SetContainmentRelationSource {
        source: NavigatorContainmentRelationSource,
    },
    SetSortMode {
        sort_mode: NavigatorSortMode,
    },
    SetRootFilter {
        root_filter: Option<String>,
    },
    SetSelectedRows {
        rows: Vec<String>,
    },
    SetExpandedRows {
        rows: Vec<String>,
    },
    RebuildProjection,
}

impl From<NavigatorProjectionIntent> for GraphIntent {
    fn from(value: NavigatorProjectionIntent) -> Self {
        match value {
            NavigatorProjectionIntent::SetContainmentRelationSource { source } => {
                GraphIntent::SetNavigatorContainmentRelationSource { source }
            }
            NavigatorProjectionIntent::SetSortMode { sort_mode } => {
                GraphIntent::SetNavigatorSortMode { sort_mode }
            }
            NavigatorProjectionIntent::SetRootFilter { root_filter } => {
                GraphIntent::SetNavigatorRootFilter { root_filter }
            }
            NavigatorProjectionIntent::SetSelectedRows { rows } => {
                GraphIntent::SetNavigatorSelectedRows { rows }
            }
            NavigatorProjectionIntent::SetExpandedRows { rows } => {
                GraphIntent::SetNavigatorExpandedRows { rows }
            }
            NavigatorProjectionIntent::RebuildProjection => GraphIntent::RebuildNavigatorProjection,
        }
    }
}

#[derive(Debug, Clone)]
pub enum GraphMutation {
    CreateNoteForNode {
        key: NodeKey,
        title: Option<String>,
    },
    CreateNodeNearCenter,
    CreateNodeNearCenterAndOpen {
        mode: PendingTileOpenMode,
    },
    CreateNodeAtUrl {
        url: String,
        position: Point2D<f32>,
    },
    CreateNodeAtUrlAndOpen {
        url: String,
        position: Point2D<f32>,
        mode: PendingTileOpenMode,
    },
    RemoveSelectedNodes,
    /// Soft-delete selected nodes: transitions them to `NodeLifecycle::Tombstone`
    /// (Ghost Node state) instead of removing them from the graph.
    MarkTombstoneForSelected,
    /// Restore a single Ghost Node from `NodeLifecycle::Tombstone → Cold`.
    RestoreGhostNode {
        key: NodeKey,
    },
    ClearGraph,
    SetNodeUrl {
        key: NodeKey,
        new_url: String,
    },
    TagNode {
        key: NodeKey,
        tag: String,
    },
    UntagNode {
        key: NodeKey,
        tag: String,
    },
    CreateUserGroupedEdge {
        from: NodeKey,
        to: NodeKey,
        label: Option<String>,
    },
    DeleteImportRecord {
        record_id: String,
    },
    SuppressImportRecordMembership {
        record_id: String,
        key: NodeKey,
    },
    PromoteImportRecordToUserGroup {
        record_id: String,
        anchor: NodeKey,
    },
    RemoveEdge {
        from: NodeKey,
        to: NodeKey,
        selector: RelationSelector,
    },
    CreateUserGroupedEdgeFromPrimarySelection,
    ExecuteEdgeCommand {
        command: EdgeCommand,
    },
    SetNodePinned {
        key: NodeKey,
        is_pinned: bool,
    },
    ForgetDevice {
        peer_id: String,
    },
    RevokeWorkspaceAccess {
        peer_id: String,
        workspace_id: String,
    },
    UpdateNodeMimeHint {
        key: NodeKey,
        mime_hint: Option<String>,
    },
}

#[derive(Debug, Clone)]
pub enum RuntimeEvent {
    PromoteNodeToActive {
        key: NodeKey,
        cause: LifecycleCause,
    },
    DemoteNodeToCold {
        key: NodeKey,
        cause: LifecycleCause,
    },
    DemoteNodeToWarm {
        key: NodeKey,
        cause: LifecycleCause,
    },
    MarkRuntimeBlocked {
        key: NodeKey,
        reason: super::RuntimeBlockReason,
        retry_at: Option<Instant>,
    },
    ClearRuntimeBlocked {
        key: NodeKey,
        cause: LifecycleCause,
    },
    MapWebviewToNode {
        webview_id: RendererId,
        key: NodeKey,
    },
    UnmapWebview {
        webview_id: RendererId,
    },
    WebViewCreated {
        parent_webview_id: RendererId,
        child_webview_id: RendererId,
        initial_url: Option<String>,
    },
    WebViewUrlChanged {
        webview_id: RendererId,
        new_url: String,
    },
    WebViewHistoryChanged {
        webview_id: RendererId,
        entries: Vec<String>,
        current: usize,
    },
    WebViewScrollChanged {
        webview_id: RendererId,
        scroll_x: f32,
        scroll_y: f32,
    },
    WebViewTitleChanged {
        webview_id: RendererId,
        title: Option<String>,
    },
    WebViewCrashed {
        webview_id: RendererId,
        reason: String,
        has_backtrace: bool,
    },
    HostOpenRequest {
        request: HostOpenRequest,
    },
    ClearHistoryTimeline,
    ClearHistoryDissolved,
    ExportHistoryTimeline,
    ExportHistoryDissolved,
    SetMemoryPressureStatus {
        level: MemoryPressureLevel,
        available_mib: u64,
        total_mib: u64,
    },
    ModActivated {
        mod_id: String,
    },
    ModLoadFailed {
        mod_id: String,
        reason: String,
    },
    ApplyRemoteDelta {
        entries: Vec<u8>,
    },
    SyncNow,
    TrustPeer {
        peer_id: String,
        display_name: String,
    },
    GrantWorkspaceAccess {
        peer_id: String,
        workspace_id: String,
    },
}

#[derive(Debug, Clone)]
pub enum GraphIntent {
    TogglePhysics,
    /// Toggle per-view ghost node (tombstone) visibility.
    ToggleGhostNodes,
    ToggleCameraPositionFitLock,
    ToggleCameraZoomFitLock,
    RequestFitToScreen,
    RequestZoomIn,
    RequestZoomOut,
    RequestZoomReset,
    RequestZoomToSelected,
    RequestZoomToGraphlet,
    ReheatPhysics,
    ToggleHelpPanel,
    ToggleCommandPalette,
    ToggleRadialMenu,
    TraverseBack,
    TraverseForward,
    EnterGraphViewLayoutManager,
    ExitGraphViewLayoutManager,
    ToggleGraphViewLayoutManager,
    CreateGraphViewSlot {
        anchor_view: Option<GraphViewId>,
        direction: GraphViewLayoutDirection,
        open_mode: Option<PendingTileOpenMode>,
    },
    RenameGraphViewSlot {
        view_id: GraphViewId,
        name: String,
    },
    MoveGraphViewSlot {
        view_id: GraphViewId,
        row: i32,
        col: i32,
    },
    ArchiveGraphViewSlot {
        view_id: GraphViewId,
    },
    RestoreGraphViewSlot {
        view_id: GraphViewId,
        row: i32,
        col: i32,
    },
    RouteGraphViewToWorkbench {
        view_id: GraphViewId,
        mode: PendingTileOpenMode,
    },
    FocusGraphView {
        view_id: GraphViewId,
    },
    CreateNoteForNode {
        key: NodeKey,
        title: Option<String>,
    },
    Undo,
    Redo,
    CreateNodeNearCenter,
    CreateNodeNearCenterAndOpen {
        mode: PendingTileOpenMode,
    },
    CreateNodeAtUrl {
        url: String,
        position: Point2D<f32>,
    },
    CreateNodeAtUrlAndOpen {
        url: String,
        position: Point2D<f32>,
        mode: PendingTileOpenMode,
    },
    AcceptHostOpenRequest {
        request: HostOpenRequest,
    },
    RemoveSelectedNodes,
    MarkTombstoneForSelected,
    RestoreGhostNode {
        key: NodeKey,
    },
    ClearGraph,
    SelectNode {
        key: NodeKey,
        multi_select: bool,
    },
    UpdateSelection {
        keys: Vec<NodeKey>,
        mode: SelectionUpdateMode,
    },
    SelectAll,
    SetInteracting {
        interacting: bool,
    },
    SetNodePosition {
        key: NodeKey,
        position: Point2D<f32>,
    },
    SetZoom {
        zoom: f32,
    },
    SetViewLens {
        view_id: GraphViewId,
        lens: LensConfig,
    },
    SetViewFilter {
        view_id: GraphViewId,
        expr: Option<crate::model::graph::filter::FacetExpr>,
    },
    ClearViewFilter {
        view_id: GraphViewId,
    },
    #[allow(dead_code)]
    SetViewDimension {
        view_id: GraphViewId,
        dimension: ViewDimension,
    },
    ToggleSemanticDepthView {
        view_id: GraphViewId,
    },
    SetPhysicsProfile {
        profile_id: String,
    },
    SetTheme {
        theme_id: String,
    },
    SetNodeUrl {
        key: NodeKey,
        new_url: String,
    },
    TagNode {
        key: NodeKey,
        tag: String,
    },
    UntagNode {
        key: NodeKey,
        tag: String,
    },
    /// Assign a provenance-bearing classification record to a node (Stage A enrichment).
    AssignClassification {
        key: NodeKey,
        classification: crate::model::graph::NodeClassification,
    },
    /// Remove a classification record by `(scheme, value)` identity.
    UnassignClassification {
        key: NodeKey,
        scheme: crate::model::graph::ClassificationScheme,
        value: String,
    },
    /// Accept a previously `Suggested` classification (updates status to `Accepted`).
    AcceptClassification {
        key: NodeKey,
        scheme: crate::model::graph::ClassificationScheme,
        value: String,
    },
    /// Reject a classification (updates status to `Rejected`; record is retained for audit).
    RejectClassification {
        key: NodeKey,
        scheme: crate::model::graph::ClassificationScheme,
        value: String,
    },
    /// Promote a classification to primary for its scheme.
    SetPrimaryClassification {
        key: NodeKey,
        scheme: crate::model::graph::ClassificationScheme,
        value: String,
    },
    SuggestNodeTags {
        key: NodeKey,
        suggestions: Vec<String>,
    },
    OpenNodeFrameRouted {
        key: NodeKey,
        prefer_frame: Option<String>,
    },
    SetPanePresentationMode {
        pane: PaneId,
        mode: PanePresentationMode,
    },
    PromoteEphemeralPane {
        target_tile_context: FloatingPaneTargetTileContext,
    },
    OpenNodeWorkspaceRouted {
        key: NodeKey,
        prefer_workspace: Option<String>,
    },
    CreateUserGroupedEdge {
        from: NodeKey,
        to: NodeKey,
        label: Option<String>,
    },
    DeleteImportRecord {
        record_id: String,
    },
    SuppressImportRecordMembership {
        record_id: String,
        key: NodeKey,
    },
    PromoteImportRecordToUserGroup {
        record_id: String,
        anchor: NodeKey,
    },
    RemoveEdge {
        from: NodeKey,
        to: NodeKey,
        selector: RelationSelector,
    },
    CreateUserGroupedEdgeFromPrimarySelection,
    GroupNodesBySemanticTags,
    ExecuteEdgeCommand {
        command: EdgeCommand,
    },
    SetHighlightedEdge {
        from: NodeKey,
        to: NodeKey,
    },
    ClearHighlightedEdge,
    SetSelectedFrame {
        frame_name: Option<String>,
    },
    SetNodePinned {
        key: NodeKey,
        is_pinned: bool,
    },
    TogglePrimaryNodePin,
    PromoteNodeToActive {
        key: NodeKey,
        cause: LifecycleCause,
    },
    DemoteNodeToCold {
        key: NodeKey,
        cause: LifecycleCause,
    },
    DemoteNodeToWarm {
        key: NodeKey,
        cause: LifecycleCause,
    },
    MarkRuntimeBlocked {
        key: NodeKey,
        reason: super::RuntimeBlockReason,
        retry_at: Option<Instant>,
    },
    ClearRuntimeBlocked {
        key: NodeKey,
        cause: LifecycleCause,
    },
    MapWebviewToNode {
        webview_id: RendererId,
        key: NodeKey,
    },
    UnmapWebview {
        webview_id: RendererId,
    },
    WebViewCreated {
        parent_webview_id: RendererId,
        child_webview_id: RendererId,
        initial_url: Option<String>,
    },
    WebViewUrlChanged {
        webview_id: RendererId,
        new_url: String,
    },
    WebViewHistoryChanged {
        webview_id: RendererId,
        entries: Vec<String>,
        current: usize,
    },
    WebViewScrollChanged {
        webview_id: RendererId,
        scroll_x: f32,
        scroll_y: f32,
    },
    SetNodeFormDraft {
        key: NodeKey,
        form_draft: Option<String>,
    },
    WebViewTitleChanged {
        webview_id: RendererId,
        title: Option<String>,
    },
    WebViewCrashed {
        webview_id: RendererId,
        reason: String,
        has_backtrace: bool,
    },
    SetNodeThumbnail {
        key: NodeKey,
        png_bytes: Vec<u8>,
        width: u32,
        height: u32,
    },
    SetNodeFavicon {
        key: NodeKey,
        rgba: Vec<u8>,
        width: u32,
        height: u32,
    },
    SetWorkbenchEdgeProjection {
        selectors: Vec<RelationSelector>,
    },
    SetViewEdgeProjectionOverride {
        view_id: GraphViewId,
        selectors: Option<Vec<RelationSelector>>,
    },
    SetSelectionEdgeProjectionOverride {
        view_id: Option<GraphViewId>,
        selectors: Option<Vec<RelationSelector>>,
    },
    ClearHistoryTimeline,
    ClearHistoryDissolved,
    AutoCurateHistoryTimeline {
        keep_latest: usize,
    },
    AutoCurateHistoryDissolved {
        keep_latest: usize,
    },
    ExportHistoryTimeline,
    ExportHistoryDissolved,
    EnterHistoryTimelinePreview,
    ExitHistoryTimelinePreview,
    HistoryTimelinePreviewIsolationViolation {
        detail: String,
    },
    HistoryTimelineReplayStarted,
    HistoryTimelineReplaySetTotal {
        total_steps: usize,
    },
    HistoryTimelineReplayAdvance {
        steps: usize,
    },
    HistoryTimelineReplayReset,
    HistoryTimelineReplayProgress {
        cursor: usize,
        total_steps: usize,
    },
    HistoryTimelineReplayFinished {
        succeeded: bool,
        error: Option<String>,
    },
    HistoryTimelineReturnToPresentFailed {
        detail: String,
    },
    WorkflowActivated {
        workflow_id: String,
    },
    PersistNostrSubscriptions,
    /// Inbound Nostr event received from a relay subscription.
    ///
    /// Carries the subscription handle ID and the signed event so consumers
    /// can route or persist the event through intent authorities rather than
    /// mutating graph state directly from the relay worker.
    NostrEventReceived {
        subscription_id: String,
        event_id: String,
        pubkey: String,
        created_at: u64,
        kind: u16,
        content: String,
        tags: Vec<Vec<String>>,
    },
    Noop,
    SetMemoryPressureStatus {
        level: MemoryPressureLevel,
        available_mib: u64,
        total_mib: u64,
    },
    ModActivated {
        mod_id: String,
    },
    ModLoadFailed {
        mod_id: String,
        reason: String,
    },
    ApplyRemoteDelta {
        entries: Vec<u8>,
    },
    SyncNow,
    TrustPeer {
        peer_id: String,
        display_name: String,
    },
    GrantWorkspaceAccess {
        peer_id: String,
        workspace_id: String,
    },
    ForgetDevice {
        peer_id: String,
    },
    RevokeWorkspaceAccess {
        peer_id: String,
        workspace_id: String,
    },
    UpdateNodeMimeHint {
        key: NodeKey,
        mime_hint: Option<String>,
    },
    RecordFrameLayoutHint {
        frame: NodeKey,
        hint: crate::graph::FrameLayoutHint,
    },
    RemoveFrameLayoutHint {
        frame: NodeKey,
        hint_index: usize,
    },
    MoveFrameLayoutHint {
        frame: NodeKey,
        from_index: usize,
        to_index: usize,
    },
    SetFrameSplitOfferSuppressed {
        frame: NodeKey,
        suppressed: bool,
    },
    SetNavigatorContainmentRelationSource {
        source: NavigatorContainmentRelationSource,
    },
    SetNavigatorSortMode {
        sort_mode: NavigatorSortMode,
    },
    SetNavigatorRootFilter {
        root_filter: Option<String>,
    },
    SetNavigatorSelectedRows {
        rows: Vec<String>,
    },
    SetNavigatorExpandedRows {
        rows: Vec<String>,
    },
    RebuildNavigatorProjection,
    /// Activate or clear a graphlet specialty view on a Navigator host.
    ///
    /// `kind = Some(k)` derives and activates an ego/corridor/component/etc.
    /// graphlet anchored at the current primary selection.
    /// `kind = None` clears the specialty view for the given host.
    SetNavigatorSpecialtyView {
        host: crate::app::SurfaceHostId,
        kind: Option<crate::graph::GraphletKind>,
    },
    /// Bridge intent: open a frame as a tile group in the workbench.
    ///
    /// Carries a `verso://frame/<name>` URL and forwards to
    /// `WorkbenchIntent::OpenFrameUrl` via the workbench bridge phase.
    /// Use this from canvas input handlers that cannot access `tiles_tree`
    /// directly (e.g. `intents_from_graph_actions`).
    ///
    /// `focus_node` optionally identifies which frame member tile should be
    /// made active after the group is opened or focused.
    OpenFrameTileGroup {
        url: String,
        focus_node: Option<NodeKey>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceGrantRequirement {
    None,
    ReadWrite,
}

impl GraphIntent {
    pub(crate) fn workspace_grant_requirement(&self) -> WorkspaceGrantRequirement {
        match self {
            Self::CreateNoteForNode { .. }
            | Self::CreateNodeNearCenter
            | Self::CreateNodeNearCenterAndOpen { .. }
            | Self::CreateNodeAtUrl { .. }
            | Self::CreateNodeAtUrlAndOpen { .. }
            | Self::RemoveSelectedNodes
            | Self::MarkTombstoneForSelected
            | Self::RestoreGhostNode { .. }
            | Self::ClearGraph
            | Self::SetNodeUrl { .. }
            | Self::TagNode { .. }
            | Self::UntagNode { .. }
            | Self::AssignClassification { .. }
            | Self::UnassignClassification { .. }
            | Self::AcceptClassification { .. }
            | Self::RejectClassification { .. }
            | Self::SetPrimaryClassification { .. }
            | Self::CreateUserGroupedEdge { .. }
            | Self::DeleteImportRecord { .. }
            | Self::SuppressImportRecordMembership { .. }
            | Self::PromoteImportRecordToUserGroup { .. }
            | Self::RemoveEdge { .. }
            | Self::CreateUserGroupedEdgeFromPrimarySelection
            | Self::SetNodePinned { .. }
            | Self::ForgetDevice { .. }
            | Self::RevokeWorkspaceAccess { .. }
            | Self::ApplyRemoteDelta { .. }
            | Self::TrustPeer { .. }
            | Self::GrantWorkspaceAccess { .. }
            | Self::UpdateNodeMimeHint { .. }
            | Self::RecordFrameLayoutHint { .. }
            | Self::RemoveFrameLayoutHint { .. }
            | Self::MoveFrameLayoutHint { .. }
            | Self::SetFrameSplitOfferSuppressed { .. } => WorkspaceGrantRequirement::ReadWrite,
            Self::TogglePhysics
            | Self::ToggleGhostNodes
            | Self::ToggleCameraPositionFitLock
            | Self::ToggleCameraZoomFitLock
            | Self::RequestFitToScreen
            | Self::RequestZoomIn
            | Self::RequestZoomOut
            | Self::RequestZoomReset
            | Self::RequestZoomToSelected
            | Self::RequestZoomToGraphlet
            | Self::ReheatPhysics
            | Self::ToggleHelpPanel
            | Self::ToggleCommandPalette
            | Self::ToggleRadialMenu
            | Self::TraverseBack
            | Self::TraverseForward
            | Self::EnterGraphViewLayoutManager
            | Self::ExitGraphViewLayoutManager
            | Self::ToggleGraphViewLayoutManager
            | Self::CreateGraphViewSlot { .. }
            | Self::RenameGraphViewSlot { .. }
            | Self::MoveGraphViewSlot { .. }
            | Self::ArchiveGraphViewSlot { .. }
            | Self::RestoreGraphViewSlot { .. }
            | Self::RouteGraphViewToWorkbench { .. }
            | Self::FocusGraphView { .. }
            | Self::Undo
            | Self::Redo
            | Self::AcceptHostOpenRequest { .. }
            | Self::SelectNode { .. }
            | Self::UpdateSelection { .. }
            | Self::SelectAll
            | Self::SetInteracting { .. }
            | Self::SetNodePosition { .. }
            | Self::SetZoom { .. }
            | Self::SetViewLens { .. }
            | Self::SetViewFilter { .. }
            | Self::ClearViewFilter { .. }
            | Self::SetViewDimension { .. }
            | Self::ToggleSemanticDepthView { .. }
            | Self::SetPhysicsProfile { .. }
            | Self::SetTheme { .. }
            | Self::SuggestNodeTags { .. }
            | Self::OpenNodeFrameRouted { .. }
            | Self::SetPanePresentationMode { .. }
            | Self::PromoteEphemeralPane { .. }
            | Self::OpenNodeWorkspaceRouted { .. }
            | Self::GroupNodesBySemanticTags
            | Self::ExecuteEdgeCommand { .. }
            | Self::SetHighlightedEdge { .. }
            | Self::ClearHighlightedEdge
            | Self::SetSelectedFrame { .. }
            | Self::TogglePrimaryNodePin
            | Self::PromoteNodeToActive { .. }
            | Self::DemoteNodeToCold { .. }
            | Self::DemoteNodeToWarm { .. }
            | Self::MarkRuntimeBlocked { .. }
            | Self::ClearRuntimeBlocked { .. }
            | Self::MapWebviewToNode { .. }
            | Self::UnmapWebview { .. }
            | Self::WebViewCreated { .. }
            | Self::WebViewUrlChanged { .. }
            | Self::WebViewHistoryChanged { .. }
            | Self::WebViewScrollChanged { .. }
            | Self::SetNodeFormDraft { .. }
            | Self::WebViewTitleChanged { .. }
            | Self::WebViewCrashed { .. }
            | Self::SetNodeThumbnail { .. }
            | Self::SetNodeFavicon { .. }
            | Self::SetWorkbenchEdgeProjection { .. }
            | Self::SetViewEdgeProjectionOverride { .. }
            | Self::SetSelectionEdgeProjectionOverride { .. }
            | Self::ClearHistoryTimeline
            | Self::ClearHistoryDissolved
            | Self::AutoCurateHistoryTimeline { .. }
            | Self::AutoCurateHistoryDissolved { .. }
            | Self::ExportHistoryTimeline
            | Self::ExportHistoryDissolved
            | Self::EnterHistoryTimelinePreview
            | Self::ExitHistoryTimelinePreview
            | Self::HistoryTimelinePreviewIsolationViolation { .. }
            | Self::HistoryTimelineReplayStarted
            | Self::HistoryTimelineReplaySetTotal { .. }
            | Self::HistoryTimelineReplayAdvance { .. }
            | Self::HistoryTimelineReplayReset
            | Self::HistoryTimelineReplayProgress { .. }
            | Self::HistoryTimelineReplayFinished { .. }
            | Self::HistoryTimelineReturnToPresentFailed { .. }
            | Self::WorkflowActivated { .. }
            | Self::PersistNostrSubscriptions
            | Self::NostrEventReceived { .. }
            | Self::Noop
            | Self::SetMemoryPressureStatus { .. }
            | Self::ModActivated { .. }
            | Self::ModLoadFailed { .. }
            | Self::SyncNow
            | Self::SetNavigatorContainmentRelationSource { .. }
            | Self::SetNavigatorSortMode { .. }
            | Self::SetNavigatorRootFilter { .. }
            | Self::SetNavigatorSelectedRows { .. }
            | Self::SetNavigatorExpandedRows { .. }
            | Self::RebuildNavigatorProjection
            | Self::SetNavigatorSpecialtyView { .. }
            | Self::OpenFrameTileGroup { .. } => WorkspaceGrantRequirement::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_grant_requirement_marks_graph_and_trust_mutations_as_readwrite() {
        assert_eq!(
            GraphIntent::CreateNodeNearCenter.workspace_grant_requirement(),
            WorkspaceGrantRequirement::ReadWrite
        );
        assert_eq!(
            GraphIntent::TrustPeer {
                peer_id: "peer-a".to_string(),
                display_name: "Peer A".to_string(),
            }
            .workspace_grant_requirement(),
            WorkspaceGrantRequirement::ReadWrite
        );
        assert_eq!(
            GraphIntent::GrantWorkspaceAccess {
                peer_id: "peer-a".to_string(),
                workspace_id: "workspace-a".to_string(),
            }
            .workspace_grant_requirement(),
            WorkspaceGrantRequirement::ReadWrite
        );
        assert_eq!(
            GraphIntent::ApplyRemoteDelta {
                entries: vec![1, 2, 3],
            }
            .workspace_grant_requirement(),
            WorkspaceGrantRequirement::ReadWrite
        );
    }

    #[test]
    fn workspace_grant_requirement_leaves_local_only_navigation_and_ui_intents_unclassified() {
        assert_eq!(
            GraphIntent::ToggleHelpPanel.workspace_grant_requirement(),
            WorkspaceGrantRequirement::None
        );
        assert_eq!(
            GraphIntent::SetViewFilter {
                view_id: GraphViewId::new(),
                expr: None,
            }
            .workspace_grant_requirement(),
            WorkspaceGrantRequirement::None
        );
        assert_eq!(
            GraphIntent::HistoryTimelineReplayStarted.workspace_grant_requirement(),
            WorkspaceGrantRequirement::None
        );
    }
}

impl From<ViewAction> for GraphIntent {
    fn from(value: ViewAction) -> Self {
        match value {
            ViewAction::ToggleCameraPositionFitLock => Self::ToggleCameraPositionFitLock,
            ViewAction::ToggleCameraZoomFitLock => Self::ToggleCameraZoomFitLock,
            ViewAction::RequestFitToScreen => Self::RequestFitToScreen,
            ViewAction::RequestZoomIn => Self::RequestZoomIn,
            ViewAction::RequestZoomOut => Self::RequestZoomOut,
            ViewAction::RequestZoomReset => Self::RequestZoomReset,
            ViewAction::RequestZoomToSelected => Self::RequestZoomToSelected,
            ViewAction::RequestZoomToGraphlet => Self::RequestZoomToGraphlet,
            ViewAction::ReheatPhysics => Self::ReheatPhysics,
            ViewAction::UpdateSelection { keys, mode } => Self::UpdateSelection { keys, mode },
            ViewAction::SelectAll => Self::SelectAll,
            ViewAction::SetNodePosition { key, position } => {
                Self::SetNodePosition { key, position }
            }
            ViewAction::SetZoom { zoom } => Self::SetZoom { zoom },
            ViewAction::SetHighlightedEdge { from, to } => Self::SetHighlightedEdge { from, to },
            ViewAction::ClearHighlightedEdge => Self::ClearHighlightedEdge,
            ViewAction::SetSelectedFrame { frame_name } => Self::SetSelectedFrame { frame_name },
            ViewAction::SetNodeFormDraft { key, form_draft } => {
                Self::SetNodeFormDraft { key, form_draft }
            }
            ViewAction::SetNodeThumbnail {
                key,
                png_bytes,
                width,
                height,
            } => Self::SetNodeThumbnail {
                key,
                png_bytes,
                width,
                height,
            },
            ViewAction::SetNodeFavicon {
                key,
                rgba,
                width,
                height,
            } => Self::SetNodeFavicon {
                key,
                rgba,
                width,
                height,
            },
            ViewAction::SetWorkbenchEdgeProjection { selectors } => {
                Self::SetWorkbenchEdgeProjection { selectors }
            }
            ViewAction::SetViewEdgeProjectionOverride { view_id, selectors } => {
                Self::SetViewEdgeProjectionOverride { view_id, selectors }
            }
            ViewAction::SetSelectionEdgeProjectionOverride { view_id, selectors } => {
                Self::SetSelectionEdgeProjectionOverride { view_id, selectors }
            }
            ViewAction::SetNavigatorContainmentRelationSource { source } => {
                Self::SetNavigatorContainmentRelationSource { source }
            }
            ViewAction::SetNavigatorSortMode { sort_mode } => {
                Self::SetNavigatorSortMode { sort_mode }
            }
            ViewAction::SetNavigatorRootFilter { root_filter } => {
                Self::SetNavigatorRootFilter { root_filter }
            }
            ViewAction::SetNavigatorSelectedRows { rows } => {
                Self::SetNavigatorSelectedRows { rows }
            }
            ViewAction::SetNavigatorExpandedRows { rows } => {
                Self::SetNavigatorExpandedRows { rows }
            }
            ViewAction::RebuildNavigatorProjection => Self::RebuildNavigatorProjection,
        }
    }
}

impl From<GraphMutation> for GraphIntent {
    fn from(value: GraphMutation) -> Self {
        match value {
            GraphMutation::CreateNoteForNode { key, title } => {
                Self::CreateNoteForNode { key, title }
            }
            GraphMutation::CreateNodeNearCenter => Self::CreateNodeNearCenter,
            GraphMutation::CreateNodeNearCenterAndOpen { mode } => {
                Self::CreateNodeNearCenterAndOpen { mode }
            }
            GraphMutation::CreateNodeAtUrl { url, position } => {
                Self::CreateNodeAtUrl { url, position }
            }
            GraphMutation::CreateNodeAtUrlAndOpen {
                url,
                position,
                mode,
            } => Self::CreateNodeAtUrlAndOpen {
                url,
                position,
                mode,
            },
            GraphMutation::RemoveSelectedNodes => Self::RemoveSelectedNodes,
            GraphMutation::MarkTombstoneForSelected => Self::MarkTombstoneForSelected,
            GraphMutation::RestoreGhostNode { key } => Self::RestoreGhostNode { key },
            GraphMutation::ClearGraph => Self::ClearGraph,
            GraphMutation::SetNodeUrl { key, new_url } => Self::SetNodeUrl { key, new_url },
            GraphMutation::TagNode { key, tag } => Self::TagNode { key, tag },
            GraphMutation::UntagNode { key, tag } => Self::UntagNode { key, tag },
            GraphMutation::CreateUserGroupedEdge { from, to, label } => {
                Self::CreateUserGroupedEdge { from, to, label }
            }
            GraphMutation::DeleteImportRecord { record_id } => {
                Self::DeleteImportRecord { record_id }
            }
            GraphMutation::SuppressImportRecordMembership { record_id, key } => {
                Self::SuppressImportRecordMembership { record_id, key }
            }
            GraphMutation::PromoteImportRecordToUserGroup { record_id, anchor } => {
                Self::PromoteImportRecordToUserGroup { record_id, anchor }
            }
            GraphMutation::RemoveEdge { from, to, selector } => {
                Self::RemoveEdge { from, to, selector }
            }
            GraphMutation::CreateUserGroupedEdgeFromPrimarySelection => {
                Self::CreateUserGroupedEdgeFromPrimarySelection
            }
            GraphMutation::ExecuteEdgeCommand { command } => Self::ExecuteEdgeCommand { command },
            GraphMutation::SetNodePinned { key, is_pinned } => {
                Self::SetNodePinned { key, is_pinned }
            }
            GraphMutation::ForgetDevice { peer_id } => Self::ForgetDevice { peer_id },
            GraphMutation::RevokeWorkspaceAccess {
                peer_id,
                workspace_id,
            } => Self::RevokeWorkspaceAccess {
                peer_id,
                workspace_id,
            },
            GraphMutation::UpdateNodeMimeHint { key, mime_hint } => {
                Self::UpdateNodeMimeHint { key, mime_hint }
            }
        }
    }
}

impl From<RuntimeEvent> for GraphIntent {
    fn from(value: RuntimeEvent) -> Self {
        match value {
            RuntimeEvent::PromoteNodeToActive { key, cause } => {
                Self::PromoteNodeToActive { key, cause }
            }
            RuntimeEvent::DemoteNodeToCold { key, cause } => Self::DemoteNodeToCold { key, cause },
            RuntimeEvent::DemoteNodeToWarm { key, cause } => Self::DemoteNodeToWarm { key, cause },
            RuntimeEvent::MarkRuntimeBlocked {
                key,
                reason,
                retry_at,
            } => Self::MarkRuntimeBlocked {
                key,
                reason,
                retry_at,
            },
            RuntimeEvent::ClearRuntimeBlocked { key, cause } => {
                Self::ClearRuntimeBlocked { key, cause }
            }
            RuntimeEvent::MapWebviewToNode { webview_id, key } => {
                Self::MapWebviewToNode { webview_id, key }
            }
            RuntimeEvent::UnmapWebview { webview_id } => Self::UnmapWebview { webview_id },
            RuntimeEvent::WebViewCreated {
                parent_webview_id,
                child_webview_id,
                initial_url,
            } => Self::WebViewCreated {
                parent_webview_id,
                child_webview_id,
                initial_url,
            },
            RuntimeEvent::WebViewUrlChanged {
                webview_id,
                new_url,
            } => Self::WebViewUrlChanged {
                webview_id,
                new_url,
            },
            RuntimeEvent::WebViewHistoryChanged {
                webview_id,
                entries,
                current,
            } => Self::WebViewHistoryChanged {
                webview_id,
                entries,
                current,
            },
            RuntimeEvent::WebViewScrollChanged {
                webview_id,
                scroll_x,
                scroll_y,
            } => Self::WebViewScrollChanged {
                webview_id,
                scroll_x,
                scroll_y,
            },
            RuntimeEvent::WebViewTitleChanged { webview_id, title } => {
                Self::WebViewTitleChanged { webview_id, title }
            }
            RuntimeEvent::WebViewCrashed {
                webview_id,
                reason,
                has_backtrace,
            } => Self::WebViewCrashed {
                webview_id,
                reason,
                has_backtrace,
            },
            RuntimeEvent::HostOpenRequest { request } => Self::AcceptHostOpenRequest { request },
            RuntimeEvent::ClearHistoryTimeline => Self::ClearHistoryTimeline,
            RuntimeEvent::ClearHistoryDissolved => Self::ClearHistoryDissolved,
            RuntimeEvent::ExportHistoryTimeline => Self::ExportHistoryTimeline,
            RuntimeEvent::ExportHistoryDissolved => Self::ExportHistoryDissolved,
            RuntimeEvent::SetMemoryPressureStatus {
                level,
                available_mib,
                total_mib,
            } => Self::SetMemoryPressureStatus {
                level,
                available_mib,
                total_mib,
            },
            RuntimeEvent::ModActivated { mod_id } => Self::ModActivated { mod_id },
            RuntimeEvent::ModLoadFailed { mod_id, reason } => {
                Self::ModLoadFailed { mod_id, reason }
            }
            RuntimeEvent::ApplyRemoteDelta { entries } => Self::ApplyRemoteDelta { entries },
            RuntimeEvent::SyncNow => Self::SyncNow,
            RuntimeEvent::TrustPeer {
                peer_id,
                display_name,
            } => Self::TrustPeer {
                peer_id,
                display_name,
            },
            RuntimeEvent::GrantWorkspaceAccess {
                peer_id,
                workspace_id,
            } => Self::GrantWorkspaceAccess {
                peer_id,
                workspace_id,
            },
        }
    }
}

impl GraphIntent {
    pub(crate) fn workbench_authority_bridge_name(&self) -> Option<&'static str> {
        match self {
            Self::RouteGraphViewToWorkbench { .. } => Some("RouteGraphViewToWorkbench"),
            Self::ToggleCommandPalette => Some("ToggleCommandPalette"),
            _ => None,
        }
    }

    pub(crate) fn as_view_action(&self) -> Option<ViewAction> {
        match self {
            Self::ToggleCameraPositionFitLock => Some(ViewAction::ToggleCameraPositionFitLock),
            Self::ToggleCameraZoomFitLock => Some(ViewAction::ToggleCameraZoomFitLock),
            Self::RequestFitToScreen => Some(ViewAction::RequestFitToScreen),
            Self::RequestZoomIn => Some(ViewAction::RequestZoomIn),
            Self::RequestZoomOut => Some(ViewAction::RequestZoomOut),
            Self::RequestZoomReset => Some(ViewAction::RequestZoomReset),
            Self::RequestZoomToSelected => Some(ViewAction::RequestZoomToSelected),
            Self::RequestZoomToGraphlet => Some(ViewAction::RequestZoomToGraphlet),
            Self::ReheatPhysics => Some(ViewAction::ReheatPhysics),
            Self::UpdateSelection { keys, mode } => Some(ViewAction::UpdateSelection {
                keys: keys.clone(),
                mode: *mode,
            }),
            Self::SelectAll => Some(ViewAction::SelectAll),
            Self::SetNodePosition { key, position } => Some(ViewAction::SetNodePosition {
                key: *key,
                position: *position,
            }),
            Self::SetZoom { zoom } => Some(ViewAction::SetZoom { zoom: *zoom }),
            Self::SetHighlightedEdge { from, to } => Some(ViewAction::SetHighlightedEdge {
                from: *from,
                to: *to,
            }),
            Self::ClearHighlightedEdge => Some(ViewAction::ClearHighlightedEdge),
            Self::SetSelectedFrame { frame_name } => Some(ViewAction::SetSelectedFrame {
                frame_name: frame_name.clone(),
            }),
            Self::SetNodeFormDraft { key, form_draft } => Some(ViewAction::SetNodeFormDraft {
                key: *key,
                form_draft: form_draft.clone(),
            }),
            Self::SetNodeThumbnail {
                key,
                png_bytes,
                width,
                height,
            } => Some(ViewAction::SetNodeThumbnail {
                key: *key,
                png_bytes: png_bytes.clone(),
                width: *width,
                height: *height,
            }),
            Self::SetNodeFavicon {
                key,
                rgba,
                width,
                height,
            } => Some(ViewAction::SetNodeFavicon {
                key: *key,
                rgba: rgba.clone(),
                width: *width,
                height: *height,
            }),
            Self::SetWorkbenchEdgeProjection { selectors } => {
                Some(ViewAction::SetWorkbenchEdgeProjection {
                    selectors: selectors.clone(),
                })
            }
            Self::SetViewEdgeProjectionOverride { view_id, selectors } => {
                Some(ViewAction::SetViewEdgeProjectionOverride {
                    view_id: *view_id,
                    selectors: selectors.clone(),
                })
            }
            Self::SetSelectionEdgeProjectionOverride { view_id, selectors } => {
                Some(ViewAction::SetSelectionEdgeProjectionOverride {
                    view_id: *view_id,
                    selectors: selectors.clone(),
                })
            }
            Self::SetNavigatorContainmentRelationSource { source } => {
                Some(ViewAction::SetNavigatorContainmentRelationSource { source: *source })
            }
            Self::SetNavigatorSortMode { sort_mode } => Some(ViewAction::SetNavigatorSortMode {
                sort_mode: *sort_mode,
            }),
            Self::SetNavigatorRootFilter { root_filter } => {
                Some(ViewAction::SetNavigatorRootFilter {
                    root_filter: root_filter.clone(),
                })
            }
            Self::SetNavigatorSelectedRows { rows } => {
                Some(ViewAction::SetNavigatorSelectedRows { rows: rows.clone() })
            }
            Self::SetNavigatorExpandedRows { rows } => {
                Some(ViewAction::SetNavigatorExpandedRows { rows: rows.clone() })
            }
            Self::RebuildNavigatorProjection => Some(ViewAction::RebuildNavigatorProjection),
            _ => None,
        }
    }

    pub(crate) fn as_graph_mutation(&self) -> Option<GraphMutation> {
        match self {
            Self::CreateNoteForNode { key, title } => Some(GraphMutation::CreateNoteForNode {
                key: *key,
                title: title.clone(),
            }),
            Self::CreateNodeNearCenter => Some(GraphMutation::CreateNodeNearCenter),
            Self::CreateNodeNearCenterAndOpen { mode } => {
                Some(GraphMutation::CreateNodeNearCenterAndOpen { mode: *mode })
            }
            Self::CreateNodeAtUrl { url, position } => Some(GraphMutation::CreateNodeAtUrl {
                url: url.clone(),
                position: *position,
            }),
            Self::CreateNodeAtUrlAndOpen {
                url,
                position,
                mode,
            } => Some(GraphMutation::CreateNodeAtUrlAndOpen {
                url: url.clone(),
                position: *position,
                mode: *mode,
            }),
            Self::AcceptHostOpenRequest { .. } => None,
            Self::RemoveSelectedNodes => Some(GraphMutation::RemoveSelectedNodes),
            Self::MarkTombstoneForSelected => Some(GraphMutation::MarkTombstoneForSelected),
            Self::RestoreGhostNode { key } => Some(GraphMutation::RestoreGhostNode { key: *key }),
            Self::ClearGraph => Some(GraphMutation::ClearGraph),
            Self::SetNodeUrl { key, new_url } => Some(GraphMutation::SetNodeUrl {
                key: *key,
                new_url: new_url.clone(),
            }),
            Self::TagNode { key, tag } => Some(GraphMutation::TagNode {
                key: *key,
                tag: tag.clone(),
            }),
            Self::UntagNode { key, tag } => Some(GraphMutation::UntagNode {
                key: *key,
                tag: tag.clone(),
            }),
            Self::AssignClassification { .. }
            | Self::UnassignClassification { .. }
            | Self::AcceptClassification { .. }
            | Self::RejectClassification { .. }
            | Self::SetPrimaryClassification { .. } => None,
            Self::CreateUserGroupedEdge { from, to, label } => {
                Some(GraphMutation::CreateUserGroupedEdge {
                    from: *from,
                    to: *to,
                    label: label.clone(),
                })
            }
            Self::DeleteImportRecord { record_id } => Some(GraphMutation::DeleteImportRecord {
                record_id: record_id.clone(),
            }),
            Self::SuppressImportRecordMembership { record_id, key } => {
                Some(GraphMutation::SuppressImportRecordMembership {
                    record_id: record_id.clone(),
                    key: *key,
                })
            }
            Self::PromoteImportRecordToUserGroup { record_id, anchor } => {
                Some(GraphMutation::PromoteImportRecordToUserGroup {
                    record_id: record_id.clone(),
                    anchor: *anchor,
                })
            }
            Self::RemoveEdge { from, to, selector } => Some(GraphMutation::RemoveEdge {
                from: *from,
                to: *to,
                selector: *selector,
            }),
            Self::CreateUserGroupedEdgeFromPrimarySelection => {
                Some(GraphMutation::CreateUserGroupedEdgeFromPrimarySelection)
            }
            Self::ExecuteEdgeCommand { command } => Some(GraphMutation::ExecuteEdgeCommand {
                command: command.clone(),
            }),
            Self::SetNodePinned { key, is_pinned } => Some(GraphMutation::SetNodePinned {
                key: *key,
                is_pinned: *is_pinned,
            }),
            Self::ForgetDevice { peer_id } => Some(GraphMutation::ForgetDevice {
                peer_id: peer_id.clone(),
            }),
            Self::RevokeWorkspaceAccess {
                peer_id,
                workspace_id,
            } => Some(GraphMutation::RevokeWorkspaceAccess {
                peer_id: peer_id.clone(),
                workspace_id: workspace_id.clone(),
            }),
            Self::UpdateNodeMimeHint { key, mime_hint } => {
                Some(GraphMutation::UpdateNodeMimeHint {
                    key: *key,
                    mime_hint: mime_hint.clone(),
                })
            }
            Self::RecordFrameLayoutHint { .. }
            | Self::RemoveFrameLayoutHint { .. }
            | Self::MoveFrameLayoutHint { .. }
            | Self::SetFrameSplitOfferSuppressed { .. } => None,
            _ => None,
        }
    }

    pub(crate) fn as_runtime_event(&self) -> Option<RuntimeEvent> {
        match self {
            Self::PromoteNodeToActive { key, cause } => Some(RuntimeEvent::PromoteNodeToActive {
                key: *key,
                cause: *cause,
            }),
            Self::DemoteNodeToCold { key, cause } => Some(RuntimeEvent::DemoteNodeToCold {
                key: *key,
                cause: *cause,
            }),
            Self::DemoteNodeToWarm { key, cause } => Some(RuntimeEvent::DemoteNodeToWarm {
                key: *key,
                cause: *cause,
            }),
            Self::MarkRuntimeBlocked {
                key,
                reason,
                retry_at,
            } => Some(RuntimeEvent::MarkRuntimeBlocked {
                key: *key,
                reason: *reason,
                retry_at: *retry_at,
            }),
            Self::ClearRuntimeBlocked { key, cause } => Some(RuntimeEvent::ClearRuntimeBlocked {
                key: *key,
                cause: *cause,
            }),
            Self::MapWebviewToNode { webview_id, key } => Some(RuntimeEvent::MapWebviewToNode {
                webview_id: *webview_id,
                key: *key,
            }),
            Self::UnmapWebview { webview_id } => Some(RuntimeEvent::UnmapWebview {
                webview_id: *webview_id,
            }),
            Self::WebViewCreated {
                parent_webview_id,
                child_webview_id,
                initial_url,
            } => Some(RuntimeEvent::WebViewCreated {
                parent_webview_id: *parent_webview_id,
                child_webview_id: *child_webview_id,
                initial_url: initial_url.clone(),
            }),
            Self::WebViewUrlChanged {
                webview_id,
                new_url,
            } => Some(RuntimeEvent::WebViewUrlChanged {
                webview_id: *webview_id,
                new_url: new_url.clone(),
            }),
            Self::WebViewHistoryChanged {
                webview_id,
                entries,
                current,
            } => Some(RuntimeEvent::WebViewHistoryChanged {
                webview_id: *webview_id,
                entries: entries.clone(),
                current: *current,
            }),
            Self::WebViewScrollChanged {
                webview_id,
                scroll_x,
                scroll_y,
            } => Some(RuntimeEvent::WebViewScrollChanged {
                webview_id: *webview_id,
                scroll_x: *scroll_x,
                scroll_y: *scroll_y,
            }),
            Self::WebViewTitleChanged { webview_id, title } => {
                Some(RuntimeEvent::WebViewTitleChanged {
                    webview_id: *webview_id,
                    title: title.clone(),
                })
            }
            Self::WebViewCrashed {
                webview_id,
                reason,
                has_backtrace,
            } => Some(RuntimeEvent::WebViewCrashed {
                webview_id: *webview_id,
                reason: reason.clone(),
                has_backtrace: *has_backtrace,
            }),
            Self::ClearHistoryTimeline => Some(RuntimeEvent::ClearHistoryTimeline),
            Self::ClearHistoryDissolved => Some(RuntimeEvent::ClearHistoryDissolved),
            Self::ExportHistoryTimeline => Some(RuntimeEvent::ExportHistoryTimeline),
            Self::ExportHistoryDissolved => Some(RuntimeEvent::ExportHistoryDissolved),
            Self::SetMemoryPressureStatus {
                level,
                available_mib,
                total_mib,
            } => Some(RuntimeEvent::SetMemoryPressureStatus {
                level: *level,
                available_mib: *available_mib,
                total_mib: *total_mib,
            }),
            Self::ModActivated { mod_id } => Some(RuntimeEvent::ModActivated {
                mod_id: mod_id.clone(),
            }),
            Self::ModLoadFailed { mod_id, reason } => Some(RuntimeEvent::ModLoadFailed {
                mod_id: mod_id.clone(),
                reason: reason.clone(),
            }),
            Self::ApplyRemoteDelta { entries } => Some(RuntimeEvent::ApplyRemoteDelta {
                entries: entries.clone(),
            }),
            Self::SyncNow => Some(RuntimeEvent::SyncNow),
            Self::TrustPeer {
                peer_id,
                display_name,
            } => Some(RuntimeEvent::TrustPeer {
                peer_id: peer_id.clone(),
                display_name: display_name.clone(),
            }),
            Self::GrantWorkspaceAccess {
                peer_id,
                workspace_id,
            } => Some(RuntimeEvent::GrantWorkspaceAccess {
                peer_id: peer_id.clone(),
                workspace_id: workspace_id.clone(),
            }),
            _ => None,
        }
    }
}
