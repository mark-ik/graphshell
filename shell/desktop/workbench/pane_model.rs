/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Canonical pane-hosted view payload model.
//!
//! Establishes the workbench pane as a universal host for view surfaces:
//! - **Graph panes**: spatial graph viewport with independent camera and Lens.
//! - **Node viewer panes**: node content rendered via the selected viewer backend.
//! - **Tool panes**: diagnostic, history, accessibility, and settings surfaces.
//!
//! This is the canonical type model for P5 "Pane-hosted multi-view architecture".
//! It is not a second canonical layout tree. Two authority layers are distinct:
//!
//! - **Layout authority** — split geometry, tab order, active-tile identity —
//!   remains in the runtime `egui_tiles::Tree<TileKind>`. The tile tree owns
//!   *how* nodes are arranged on screen.
//! - **Membership authority** — which nodes belong together and whether each is
//!   currently presented — lives in the graph layer: `UserGrouped` and
//!   `FrameMember` edges carry durable membership; `NodeLifecycle` carries
//!   presence state. The tile tree is a *projection* of this membership, not the
//!   source of truth. See
//!   `design_docs/graphshell_docs/implementation_strategy/workbench/2026-03-20_arrangement_graph_projection_plan.md`.
//!
//! This module defines the pane payload/schema carried inside the tile tree and
//! by persistence formats. Dispatch, persistence, and intent routing operate on
//! `PaneViewState` variants rather than backend-specific tile assumptions.
//!
//! **Source refs**:
//! - `design_docs/graphshell_docs/implementation_strategy/2026-02-22_multi_graph_pane_plan.md`
//! - `design_docs/graphshell_docs/implementation_strategy/2026-02-24_immediate_priorities.md:46`

use crate::app::GraphViewId;
use crate::graph::NodeKey;

/// Opaque stable identifier for a workbench pane.
///
/// Distinct from `egui_tiles::TileId` (layout tree identity) and `PaneId` in
/// `persistence_ops` (the legacy u64 persistence key). This is the canonical
/// pane identity for the pane-hosted view architecture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub(crate) struct PaneId(uuid::Uuid);

impl PaneId {
    pub(crate) fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

impl Default for PaneId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for PaneId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "pane:{}", self.0)
    }
}

/// Presentation/chrome mode for a workbench pane.
///
/// This is workbench-owned UI state and does not change graph identity.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub(crate) enum PanePresentationMode {
    /// Full tile chrome with normal tile-tree mobility.
    #[default]
    Tiled,
    /// Reduced chrome with position-locked interaction.
    Docked,
    /// Chromeless overlay presentation used by ephemeral panes before promotion.
    Floating,
    /// Content-only presentation; reserved for future use.
    Fullscreen,
}

/// Placement context for promoting a floating pane into the tile tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub(crate) enum FloatingPaneTargetTileContext {
    TabGroup,
    Split,
    BareGraph,
}

/// Opaque viewer backend identifier.
///
/// Examples: `"viewer:webview"`, `"viewer:wry"`, `"viewer:plaintext"`, `"viewer:pdf"`.
/// Canonical selection is resolved by `ViewerRegistry`; this type carries explicit overrides only.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub(crate) struct ViewerId(String);

impl ViewerId {
    pub(crate) fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ViewerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub(crate) enum ViewerSwitchReason {
    #[default]
    UserRequested,
    RecoveryPromptAccepted,
    PolicyPinned,
}

/// Graph pane reference payload.
///
/// Identifies which `GraphViewState` (camera + Lens + per-view local layout state) is active in this pane.
/// The graph data itself (`GraphWorkspace.domain.graph`) remains shared across all graph panes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(from = "GraphPaneRefCompat")]
pub(crate) struct GraphPaneRef {
    /// Stable pane identity for this graph-hosting workbench pane.
    pub pane_id: PaneId,
    /// The graph view state driving this pane's camera, Lens, and layout.
    pub graph_view_id: GraphViewId,
    /// Chrome/presentation mode for this pane.
    #[serde(default)]
    pub presentation_mode: PanePresentationMode,
}

impl GraphPaneRef {
    pub(crate) fn new(graph_view_id: GraphViewId) -> Self {
        Self {
            pane_id: PaneId::new(),
            graph_view_id,
            presentation_mode: PanePresentationMode::Tiled,
        }
    }

    pub(crate) fn view_id(self) -> GraphViewId {
        self.graph_view_id
    }
}

#[derive(serde::Deserialize)]
#[serde(untagged)]
enum GraphPaneRefCompat {
    Legacy(GraphViewId),
    Current {
        #[serde(default)]
        pane_id: Option<PaneId>,
        graph_view_id: GraphViewId,
        #[serde(default)]
        presentation_mode: PanePresentationMode,
    },
}

impl From<GraphPaneRefCompat> for GraphPaneRef {
    fn from(compat: GraphPaneRefCompat) -> Self {
        match compat {
            GraphPaneRefCompat::Legacy(graph_view_id) => Self::new(graph_view_id),
            GraphPaneRefCompat::Current {
                pane_id,
                graph_view_id,
                presentation_mode,
            } => Self {
                pane_id: pane_id.unwrap_or_default(),
                graph_view_id,
                presentation_mode,
            },
        }
    }
}

impl PartialEq<GraphViewId> for GraphPaneRef {
    fn eq(&self, other: &GraphViewId) -> bool {
        self.graph_view_id == *other
    }
}

/// Node viewer pane payload.
///
/// Carries which node to display and an optional explicit viewer backend override.
/// Canonical viewer selection (based on `mime_hint`, `address_kind`, user policy)
/// is delegated to `ViewerRegistry`; `viewer_id_override` is an explicit user/intent override only.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub(crate) enum TileRenderMode {
    /// Viewer renders to a Graphshell-owned composited texture (e.g. Servo).
    CompositedTexture,
    /// Viewer uses an OS-native overlay window (e.g. Wry).
    NativeOverlay,
    /// Viewer renders directly into egui UI.
    EmbeddedEgui,
    /// Viewer is unavailable or unresolved for this pane.
    #[default]
    Placeholder,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(from = "NodePaneStateCompat")]
pub(crate) struct NodePaneState {
    /// Stable pane identity for this node-hosting workbench pane.
    pub pane_id: PaneId,
    /// The node to render in this pane.
    pub node: NodeKey,
    /// Optional explicit viewer backend override. `None` delegates to `ViewerRegistry`.
    pub viewer_id_override: Option<ViewerId>,
    /// Most recent explicit backend-switch reason for this pane, if any.
    #[serde(default)]
    pub viewer_switch_reason: ViewerSwitchReason,
    /// Runtime-authoritative render pipeline mode for this pane.
    #[serde(default)]
    pub render_mode: TileRenderMode,
    /// Chrome/presentation mode for this pane.
    #[serde(default)]
    pub presentation_mode: PanePresentationMode,
    /// Whether the node history panel is expanded in this pane.
    #[serde(default)]
    pub show_node_history: bool,
    /// Whether the node audit panel is expanded in this pane.
    #[serde(default)]
    pub show_node_audit: bool,
    /// Cached resolved viewer ID computed at pane attach / refresh.
    /// When `Some`, skips per-frame `preferred_viewer_id_for_content` resolution.
    #[serde(skip)]
    pub resolved_viewer_id: Option<String>,
    /// Cached verso-resolved route for this pane's node, carrying the
    /// chosen engine, middlenet lane, reason, and ownership. Populated
    /// alongside `resolved_viewer_id` on pane attach / refresh.
    /// `None` when verso does not route this content (specialized
    /// non-web viewers: images, PDFs, plaintext, directory listings).
    #[serde(skip)]
    pub resolved_route: Option<::verso::VersoResolvedRoute>,
}

impl NodePaneState {
    pub(crate) fn for_node(node: NodeKey) -> Self {
        Self {
            pane_id: PaneId::new(),
            node,
            viewer_id_override: None,
            viewer_switch_reason: ViewerSwitchReason::PolicyPinned,
            render_mode: TileRenderMode::Placeholder,
            presentation_mode: PanePresentationMode::Tiled,
            show_node_history: false,
            show_node_audit: false,
            resolved_viewer_id: None,
            resolved_route: None,
        }
    }

    pub(crate) fn with_viewer(node: NodeKey, viewer_id: ViewerId) -> Self {
        Self {
            pane_id: PaneId::new(),
            node,
            viewer_id_override: Some(viewer_id),
            viewer_switch_reason: ViewerSwitchReason::UserRequested,
            render_mode: TileRenderMode::Placeholder,
            presentation_mode: PanePresentationMode::Tiled,
            show_node_history: false,
            show_node_audit: false,
            resolved_viewer_id: None,
            resolved_route: None,
        }
    }
}

impl From<NodeKey> for NodePaneState {
    fn from(node: NodeKey) -> Self {
        Self::for_node(node)
    }
}

/// Serde compatibility shim: deserializes both the legacy `NodeKey` format (a plain u32)
/// and the current `NodePaneState` struct format.
#[derive(serde::Deserialize)]
#[serde(untagged)]
enum NodePaneStateCompat {
    /// Legacy: bare `NodeKey` value (old `TileKind::WebView(NodeKey)` format).
    Legacy(NodeKey),
    /// Current: full `NodePaneState` struct.
    Current {
        #[serde(default)]
        pane_id: Option<PaneId>,
        node: NodeKey,
        viewer_id_override: Option<ViewerId>,
        #[serde(default)]
        viewer_switch_reason: ViewerSwitchReason,
        #[serde(default)]
        render_mode: TileRenderMode,
        #[serde(default)]
        presentation_mode: PanePresentationMode,
    },
}

impl From<NodePaneStateCompat> for NodePaneState {
    fn from(compat: NodePaneStateCompat) -> Self {
        match compat {
            NodePaneStateCompat::Legacy(node) => Self {
                pane_id: PaneId::new(),
                node,
                viewer_id_override: None,
                viewer_switch_reason: ViewerSwitchReason::PolicyPinned,
                render_mode: TileRenderMode::Placeholder,
                presentation_mode: PanePresentationMode::Tiled,
                show_node_history: false,
                show_node_audit: false,
                resolved_viewer_id: None,
                resolved_route: None,
            },
            NodePaneStateCompat::Current {
                pane_id,
                node,
                viewer_id_override,
                viewer_switch_reason,
                render_mode,
                presentation_mode,
            } => {
                let normalized_switch_reason = if viewer_id_override.is_none()
                    && matches!(viewer_switch_reason, ViewerSwitchReason::UserRequested)
                {
                    ViewerSwitchReason::PolicyPinned
                } else {
                    viewer_switch_reason
                };
                Self {
                    pane_id: pane_id.unwrap_or_default(),
                    node,
                    viewer_id_override,
                    viewer_switch_reason: normalized_switch_reason,
                    render_mode,
                    presentation_mode,
                    show_node_history: false,
                    show_node_audit: false,
                    resolved_viewer_id: None,
                    resolved_route: None,
                }
            }
        }
    }
}

/// Tool pane content variant.
///
/// Determines which tool surface is rendered in a tool pane.
/// New tool surfaces can be added as variants here; the pane model remains stable.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum ToolPaneState {
    /// Engine topology, compositor state, and diagnostics inspector.
    Diagnostics,
    /// Traversal history timeline and dissolved node archive.
    HistoryManager,
    /// Accessibility inspection surface.
    AccessibilityInspector,
    /// Legacy file-tree projection surface, now presented as Navigator.
    FileTree,
    /// Application and workspace settings.
    Settings,
}

impl ToolPaneState {
    pub(crate) fn navigator_surface() -> Self {
        Self::FileTree
    }

    pub(crate) fn is_navigator_surface(&self) -> bool {
        matches!(self, Self::FileTree)
    }

    pub(crate) fn is_file_tree_surface(&self) -> bool {
        self.is_navigator_surface()
    }

    pub(crate) fn title(&self) -> &'static str {
        match self {
            Self::Diagnostics => "Diagnostics",
            Self::HistoryManager => "History",
            Self::AccessibilityInspector => "Accessibility",
            Self::FileTree => "Navigator",
            Self::Settings => "Settings",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(from = "ToolPaneRefCompat")]
pub(crate) struct ToolPaneRef {
    /// Stable pane identity for this tool-hosting workbench pane.
    pub pane_id: PaneId,
    /// Which tool surface this pane renders.
    pub kind: ToolPaneState,
    /// Chrome/presentation mode for this pane.
    #[serde(default)]
    pub presentation_mode: PanePresentationMode,
}

impl ToolPaneRef {
    pub(crate) fn new(kind: ToolPaneState) -> Self {
        Self {
            pane_id: PaneId::new(),
            kind,
            presentation_mode: PanePresentationMode::Tiled,
        }
    }

    pub(crate) fn title(&self) -> &'static str {
        self.kind.title()
    }
}

#[derive(serde::Deserialize)]
#[serde(untagged)]
enum ToolPaneRefCompat {
    Legacy(ToolPaneState),
    Current {
        #[serde(default)]
        pane_id: Option<PaneId>,
        kind: ToolPaneState,
        #[serde(default)]
        presentation_mode: PanePresentationMode,
    },
}

impl From<ToolPaneRefCompat> for ToolPaneRef {
    fn from(compat: ToolPaneRefCompat) -> Self {
        match compat {
            ToolPaneRefCompat::Legacy(kind) => Self::new(kind),
            ToolPaneRefCompat::Current {
                pane_id,
                kind,
                presentation_mode,
            } => Self {
                pane_id: pane_id.unwrap_or_default(),
                kind,
                presentation_mode,
            },
        }
    }
}

impl PartialEq<ToolPaneState> for ToolPaneRef {
    fn eq(&self, other: &ToolPaneState) -> bool {
        self.kind == *other
    }
}

/// Pane-hosted view payload.
///
/// Determines how a workbench pane renders and routes input.
/// Dispatch should switch on this type rather than on backend-specific tile assumptions.
///
/// ```text
/// match pane.view {
///     PaneViewState::Graph(ref graph_ref) => render_graph_pane(graph_ref.graph_view_id, …),
///     PaneViewState::Node(ref node_pane)  => render_node_viewer_pane(node_pane.node, …),
///     PaneViewState::Tool(ref tool_pane)  => render_tool_pane(tool_pane, …),
/// }
/// ```
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) enum PaneViewState {
    /// Renders a graph viewport with independent camera, Lens, and layout mode.
    Graph(GraphPaneRef),
    /// Renders a node using the selected viewer backend (Servo texture, Wry overlay, native).
    Node(NodePaneState),
    /// Renders a tool surface (diagnostics, history, accessibility, settings, etc.).
    Tool(ToolPaneRef),
}

impl PaneViewState {
    /// Returns the node key if this is a `Node` pane.
    pub(crate) fn node_key(&self) -> Option<NodeKey> {
        match self {
            Self::Node(state) => Some(state.node),
            _ => None,
        }
    }

    /// Returns the graph view id if this is a `Graph` pane.
    pub(crate) fn graph_view_id(&self) -> Option<GraphViewId> {
        match self {
            Self::Graph(graph_ref) => Some(graph_ref.graph_view_id),
            _ => None,
        }
    }

    /// Returns the pane id regardless of pane view type.
    pub(crate) fn pane_id(&self) -> PaneId {
        match self {
            Self::Graph(graph_ref) => graph_ref.pane_id,
            Self::Node(state) => state.pane_id,
            Self::Tool(tool_ref) => tool_ref.pane_id,
        }
    }

    pub(crate) fn presentation_mode(&self) -> PanePresentationMode {
        match self {
            Self::Graph(graph_ref) => graph_ref.presentation_mode,
            Self::Node(state) => state.presentation_mode,
            Self::Tool(tool_ref) => tool_ref.presentation_mode,
        }
    }

    pub(crate) fn set_presentation_mode(&mut self, mode: PanePresentationMode) {
        match self {
            Self::Graph(graph_ref) => graph_ref.presentation_mode = mode,
            Self::Node(state) => state.presentation_mode = mode,
            Self::Tool(tool_ref) => tool_ref.presentation_mode = mode,
        }
    }
}

/// Direction for pane split operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SplitDirection {
    Horizontal,
    Vertical,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::GraphViewId;

    #[test]
    fn pane_id_is_unique() {
        let a = PaneId::new();
        let b = PaneId::new();
        assert_ne!(a, b, "each PaneId should be unique");
    }

    #[test]
    fn pane_id_default_is_unique() {
        let a = PaneId::default();
        let b = PaneId::default();
        assert_ne!(a, b, "each default PaneId should be unique");
    }

    #[test]
    fn viewer_id_round_trips() {
        let id = ViewerId::new("viewer:webview");
        assert_eq!(id.as_str(), "viewer:webview");
        let json = serde_json::to_string(&id).unwrap();
        let back: ViewerId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, back);
    }

    #[test]
    fn graph_pane_ref_round_trips() {
        let id = GraphViewId::new();
        let pane_ref = GraphPaneRef::new(id);
        let json = serde_json::to_string(&pane_ref).unwrap();
        let back: GraphPaneRef = serde_json::from_str(&json).unwrap();
        assert_eq!(pane_ref, back);
    }

    #[test]
    fn node_pane_state_for_node_has_no_viewer_override() {
        use petgraph::stable_graph::NodeIndex;
        let key = NodeIndex::new(0);
        let state = NodePaneState::for_node(key);
        assert_eq!(state.node, key);
        assert_ne!(state.pane_id, PaneId::default());
        assert!(state.viewer_id_override.is_none());
        assert_eq!(state.viewer_switch_reason, ViewerSwitchReason::PolicyPinned);
        assert_eq!(state.render_mode, TileRenderMode::Placeholder);
        assert_eq!(state.presentation_mode, PanePresentationMode::Tiled);
    }

    #[test]
    fn tool_pane_titles_are_stable_per_variant() {
        assert_eq!(ToolPaneState::Diagnostics.title(), "Diagnostics");
        assert_eq!(ToolPaneState::HistoryManager.title(), "History");
        assert_eq!(
            ToolPaneState::AccessibilityInspector.title(),
            "Accessibility"
        );
        assert_eq!(ToolPaneState::FileTree.title(), "Navigator");
        assert_eq!(ToolPaneState::Settings.title(), "Settings");
    }

    #[test]
    fn node_pane_state_with_viewer_carries_override() {
        use petgraph::stable_graph::NodeIndex;
        let key = NodeIndex::new(1);
        let viewer = ViewerId::new("viewer:wry");
        let state = NodePaneState::with_viewer(key, viewer.clone());
        assert_eq!(state.node, key);
        assert_eq!(state.viewer_id_override, Some(viewer));
        assert_eq!(
            state.viewer_switch_reason,
            ViewerSwitchReason::UserRequested
        );
        assert_eq!(state.render_mode, TileRenderMode::Placeholder);
        assert_eq!(state.presentation_mode, PanePresentationMode::Tiled);
    }

    #[test]
    fn node_pane_state_round_trips_current_format() {
        use petgraph::stable_graph::NodeIndex;
        let key = NodeIndex::new(2);
        let state = NodePaneState::for_node(key);
        let json = serde_json::to_string(&state).unwrap();
        let back: NodePaneState = serde_json::from_str(&json).unwrap();
        assert_eq!(state, back);
    }

    #[test]
    fn node_pane_state_deserializes_legacy_bare_node_key() {
        // Legacy format: bare NodeKey (u32 index) from old TileKind::WebView(NodeKey).
        // Petgraph serializes NodeIndex<u32> as a u32 value.
        let legacy_json = "3";
        let state: NodePaneState = serde_json::from_str(legacy_json).unwrap();
        use petgraph::stable_graph::NodeIndex;
        assert_eq!(state.node, NodeIndex::new(3));
        assert!(state.viewer_id_override.is_none());
        assert_eq!(state.viewer_switch_reason, ViewerSwitchReason::PolicyPinned);
        assert_eq!(state.render_mode, TileRenderMode::Placeholder);
        assert_eq!(state.presentation_mode, PanePresentationMode::Tiled);
    }

    #[test]
    fn node_pane_state_deserializes_without_render_mode_field() {
        let json = r#"{"node":4,"viewer_id_override":"viewer:webview"}"#;
        let state: NodePaneState = serde_json::from_str(json).unwrap();
        use petgraph::stable_graph::NodeIndex;
        assert_eq!(state.node, NodeIndex::new(4));
        assert_eq!(
            state.viewer_id_override,
            Some(ViewerId::new("viewer:webview"))
        );
        assert_eq!(
            state.viewer_switch_reason,
            ViewerSwitchReason::UserRequested
        );
        assert_eq!(state.render_mode, TileRenderMode::Placeholder);
        assert_eq!(state.presentation_mode, PanePresentationMode::Tiled);
    }

    #[test]
    fn node_pane_state_deserializes_without_pane_id_field() {
        let json = r#"{"node":6,"viewer_id_override":null,"render_mode":"Placeholder"}"#;
        let state: NodePaneState = serde_json::from_str(json).unwrap();
        use petgraph::stable_graph::NodeIndex;
        assert_eq!(state.node, NodeIndex::new(6));
        assert!(state.viewer_id_override.is_none());
        assert_eq!(state.viewer_switch_reason, ViewerSwitchReason::PolicyPinned);
        assert_eq!(state.render_mode, TileRenderMode::Placeholder);
        assert_eq!(state.presentation_mode, PanePresentationMode::Tiled);
    }

    #[test]
    fn pane_view_state_presentation_mode_mutates_across_variants() {
        let view_id = GraphViewId::new();
        let mut graph = PaneViewState::Graph(GraphPaneRef::new(view_id));
        graph.set_presentation_mode(PanePresentationMode::Docked);
        assert_eq!(graph.presentation_mode(), PanePresentationMode::Docked);

        let mut tool = PaneViewState::Tool(ToolPaneRef::new(ToolPaneState::Settings));
        tool.set_presentation_mode(PanePresentationMode::Floating);
        assert_eq!(tool.presentation_mode(), PanePresentationMode::Floating);
    }

    #[test]
    fn pane_view_state_graph_round_trips() {
        let id = GraphViewId::new();
        let view = PaneViewState::Graph(GraphPaneRef::new(id));
        let json = serde_json::to_string(&view).unwrap();
        let back: PaneViewState = serde_json::from_str(&json).unwrap();
        assert_eq!(view, back);
    }

    #[test]
    fn pane_view_state_node_round_trips() {
        use petgraph::stable_graph::NodeIndex;
        let key = NodeIndex::new(0);
        let view = PaneViewState::Node(NodePaneState::for_node(key));
        let json = serde_json::to_string(&view).unwrap();
        let back: PaneViewState = serde_json::from_str(&json).unwrap();
        assert_eq!(view, back);
    }

    #[test]
    fn pane_view_state_tool_round_trips() {
        let view = PaneViewState::Tool(ToolPaneRef::new(ToolPaneState::Diagnostics));
        let json = serde_json::to_string(&view).unwrap();
        let back: PaneViewState = serde_json::from_str(&json).unwrap();
        assert_eq!(view, back);
    }

    #[test]
    fn pane_view_state_node_key_accessor() {
        use petgraph::stable_graph::NodeIndex;
        let key = NodeIndex::new(5);
        let view = PaneViewState::Node(NodePaneState::for_node(key));
        assert_eq!(view.node_key(), Some(key));
        assert!(view.graph_view_id().is_none());
    }

    #[test]
    fn pane_view_state_graph_view_id_accessor() {
        let id = GraphViewId::new();
        let view = PaneViewState::Graph(GraphPaneRef::new(id));
        assert_eq!(view.graph_view_id(), Some(id));
        assert!(view.node_key().is_none());
    }
}
