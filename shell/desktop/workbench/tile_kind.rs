/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Tile kinds used by egui_tiles layout.
//!
//! `TileKind` variants correspond to `PaneViewState` payload kinds.
//! Dispatch on `TileKind` is the workbench-layer expression of pane view payload dispatch.
//! See `pane_model.rs` for the canonical pane payload model.
//!
//! Layout authority lives in the runtime `egui_tiles::Tree<TileKind>` itself.
//! `TileKind`/`PaneViewState` define what each pane contains, not an alternate
//! canonical tree that the runtime layout should diverge from.

use crate::app::GraphViewId;
use crate::shell::desktop::workbench::pane_model::{
    GraphPaneRef, NodePaneState, PaneId, TileRenderMode, ToolPaneRef, ToolPaneState,
};

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) enum TileKind {
    /// A graph pane: renders a spatial graph viewport with independent camera and Lens.
    Graph(GraphPaneRef),
    /// A node viewer pane: renders a node via the selected viewer backend.
    ///
    /// Serde alias `"WebView"` preserves backward compatibility with persisted tile layouts
    /// created before this variant was renamed from `WebView(NodeKey)`.
    #[serde(alias = "WebView")]
    Node(NodePaneState),
    /// A tool pane: diagnostics inspector, history manager, settings, etc.
    #[cfg(feature = "diagnostics")]
    Tool(ToolPaneRef),
}

impl TileKind {
    pub(crate) fn pane_id(&self) -> PaneId {
        match self {
            Self::Graph(graph_ref) => graph_ref.pane_id,
            Self::Node(node_state) => node_state.pane_id,
            #[cfg(feature = "diagnostics")]
            Self::Tool(tool_ref) => tool_ref.pane_id,
        }
    }

    pub(crate) fn graph_view_id(&self) -> Option<GraphViewId> {
        match self {
            Self::Graph(graph_ref) => Some(graph_ref.graph_view_id),
            _ => None,
        }
    }

    pub(crate) fn node_render_mode(&self) -> Option<TileRenderMode> {
        match self {
            Self::Node(node_state) => Some(node_state.render_mode),
            _ => None,
        }
    }

    #[cfg(feature = "diagnostics")]
    pub(crate) fn tool_kind(&self) -> Option<&ToolPaneState> {
        match self {
            Self::Tool(tool_ref) => Some(&tool_ref.kind),
            _ => None,
        }
    }
}
