/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Canonical pane-hosted view payload model.
//!
//! Establishes the workbench pane as a universal host for view surfaces:
//! - **Graph panes**: spatial graph viewport with independent camera, Lens, and layout mode.
//! - **Node viewer panes**: node content rendered via the selected viewer backend.
//! - **Tool panes**: diagnostic, history, accessibility, and settings surfaces.
//!
//! This is the canonical type model for P5 "Pane-hosted multi-view architecture".
//! Dispatch, persistence, and intent routing in subsequent phases (P6–P8) operate
//! on `PaneViewState` variants rather than backend-specific tile assumptions.
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

/// Opaque viewer backend identifier.
///
/// Examples: `"viewer:servo"`, `"viewer:wry"`, `"viewer:plaintext"`, `"viewer:pdf"`.
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

/// Layout mode for a graph pane.
///
/// **Canonical**: reads shared node positions from global physics / manual layout.
/// Multiple canonical graph panes show the same positions with independent cameras.
///
/// **Divergent**: owns a `LocalSimulation` shadow position set and local physics.
/// Does not mutate shared graph positions unless explicitly committed.
///
/// See `2026-02-22_multi_graph_pane_plan.md §3` for full transition semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub(crate) enum ViewLayoutMode {
    /// Reads from shared canonical graph positions; independent camera only.
    #[default]
    Canonical,
    /// Owns a private local simulation; positions diverge from shared state until committed.
    Divergent,
}

/// Graph pane reference payload.
///
/// Identifies which `GraphViewState` (camera, Lens, layout mode) is active in this pane.
/// The graph data itself (`GraphWorkspace.graph`) remains shared across all graph panes.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct GraphPaneRef {
    /// The graph view state driving this pane's camera, Lens, and layout.
    pub graph_view_id: GraphViewId,
}

impl GraphPaneRef {
    pub(crate) fn new(graph_view_id: GraphViewId) -> Self {
        Self { graph_view_id }
    }
}

/// Node viewer pane payload.
///
/// Carries which node to display and an optional explicit viewer backend override.
/// Canonical viewer selection (based on `mime_hint`, `address_kind`, user policy)
/// is delegated to `ViewerRegistry`; `viewer_id_override` is an explicit user/intent override only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize)]
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
    /// The node to render in this pane.
    pub node: NodeKey,
    /// Optional explicit viewer backend override. `None` delegates to `ViewerRegistry`.
    pub viewer_id_override: Option<ViewerId>,
    /// Runtime-authoritative render pipeline mode for this pane.
    #[serde(default)]
    pub render_mode: TileRenderMode,
}

impl NodePaneState {
    pub(crate) fn for_node(node: NodeKey) -> Self {
        Self {
            node,
            viewer_id_override: None,
            render_mode: TileRenderMode::Placeholder,
        }
    }

    pub(crate) fn with_viewer(node: NodeKey, viewer_id: ViewerId) -> Self {
        Self {
            node,
            viewer_id_override: Some(viewer_id),
            render_mode: TileRenderMode::Placeholder,
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
        node: NodeKey,
        viewer_id_override: Option<ViewerId>,
        #[serde(default)]
        render_mode: TileRenderMode,
    },
}

impl From<NodePaneStateCompat> for NodePaneState {
    fn from(compat: NodePaneStateCompat) -> Self {
        match compat {
            NodePaneStateCompat::Legacy(node) => Self {
                node,
                viewer_id_override: None,
                render_mode: TileRenderMode::Placeholder,
            },
            NodePaneStateCompat::Current {
                node,
                viewer_id_override,
                render_mode,
            } => {
                Self {
                    node,
                    viewer_id_override,
                    render_mode,
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
    /// Application and workspace settings.
    Settings,
}

impl ToolPaneState {
    pub(crate) fn title(&self) -> &'static str {
        match self {
            Self::Diagnostics => "Diagnostics",
            Self::HistoryManager => "History",
            Self::AccessibilityInspector => "Accessibility",
            Self::Settings => "Settings",
        }
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
    Tool(ToolPaneState),
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
        let id = ViewerId::new("viewer:servo");
        assert_eq!(id.as_str(), "viewer:servo");
        let json = serde_json::to_string(&id).unwrap();
        let back: ViewerId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, back);
    }

    #[test]
    fn view_layout_mode_default_is_canonical() {
        assert_eq!(ViewLayoutMode::default(), ViewLayoutMode::Canonical);
    }

    #[test]
    fn view_layout_mode_round_trips() {
        for mode in [ViewLayoutMode::Canonical, ViewLayoutMode::Divergent] {
            let json = serde_json::to_string(&mode).unwrap();
            let back: ViewLayoutMode = serde_json::from_str(&json).unwrap();
            assert_eq!(mode, back);
        }
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
        assert!(state.viewer_id_override.is_none());
        assert_eq!(state.render_mode, TileRenderMode::Placeholder);
    }

    #[test]
    fn tool_pane_titles_are_stable_per_variant() {
        assert_eq!(ToolPaneState::Diagnostics.title(), "Diagnostics");
        assert_eq!(ToolPaneState::HistoryManager.title(), "History");
        assert_eq!(ToolPaneState::AccessibilityInspector.title(), "Accessibility");
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
        assert_eq!(state.render_mode, TileRenderMode::Placeholder);
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
        assert_eq!(state.render_mode, TileRenderMode::Placeholder);
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
        assert_eq!(state.render_mode, TileRenderMode::Placeholder);
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
        let view = PaneViewState::Tool(ToolPaneState::Diagnostics);
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
