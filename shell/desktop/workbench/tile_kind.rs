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
    GraphPaneRef, NodePaneState, PaneId, PanePresentationMode, PaneViewState, TileRenderMode,
};
#[cfg(feature = "diagnostics")]
use crate::shell::desktop::workbench::pane_model::{ToolPaneRef, ToolPaneState};

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) enum TileKind {
    /// Generic pane payload carrier used for pane-hosted surfaces that are not yet in the
    /// canonical tiled runtime path, such as floating ephemeral panes.
    Pane(PaneViewState),
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
            Self::Pane(view) => view.pane_id(),
            Self::Graph(graph_ref) => graph_ref.pane_id,
            Self::Node(node_state) => node_state.pane_id,
            #[cfg(feature = "diagnostics")]
            Self::Tool(tool_ref) => tool_ref.pane_id,
        }
    }

    pub(crate) fn graph_view_id(&self) -> Option<GraphViewId> {
        match self {
            Self::Pane(view) => view.graph_view_id(),
            Self::Graph(graph_ref) => Some(graph_ref.graph_view_id),
            _ => None,
        }
    }

    pub(crate) fn node_render_mode(&self) -> Option<TileRenderMode> {
        match self {
            Self::Pane(PaneViewState::Node(node_state)) => Some(node_state.render_mode),
            Self::Node(node_state) => Some(node_state.render_mode),
            _ => None,
        }
    }

    pub(crate) fn pane_view(&self) -> Option<&PaneViewState> {
        match self {
            Self::Pane(view) => Some(view),
            _ => None,
        }
    }

    pub(crate) fn pane_view_mut(&mut self) -> Option<&mut PaneViewState> {
        match self {
            Self::Pane(view) => Some(view),
            _ => None,
        }
    }

    pub(crate) fn node_state(&self) -> Option<&NodePaneState> {
        match self {
            Self::Pane(PaneViewState::Node(state)) => Some(state),
            Self::Node(state) => Some(state),
            _ => None,
        }
    }

    pub(crate) fn node_state_mut(&mut self) -> Option<&mut NodePaneState> {
        match self {
            Self::Pane(PaneViewState::Node(state)) => Some(state),
            Self::Node(state) => Some(state),
            _ => None,
        }
    }

    pub(crate) fn graph_ref(&self) -> Option<&GraphPaneRef> {
        match self {
            Self::Pane(PaneViewState::Graph(graph_ref)) => Some(graph_ref),
            Self::Graph(graph_ref) => Some(graph_ref),
            _ => None,
        }
    }

    #[cfg(feature = "diagnostics")]
    pub(crate) fn tool_ref(&self) -> Option<&ToolPaneRef> {
        match self {
            Self::Pane(PaneViewState::Tool(tool_ref)) => Some(tool_ref),
            Self::Tool(tool_ref) => Some(tool_ref),
            _ => None,
        }
    }

    pub(crate) fn presentation_mode(&self) -> PanePresentationMode {
        match self {
            Self::Pane(view) => view.presentation_mode(),
            Self::Graph(graph_ref) => graph_ref.presentation_mode,
            Self::Node(node_state) => node_state.presentation_mode,
            #[cfg(feature = "diagnostics")]
            Self::Tool(tool_ref) => tool_ref.presentation_mode,
        }
    }

    pub(crate) fn is_floating(&self) -> bool {
        self.presentation_mode() == PanePresentationMode::Floating
    }

    #[cfg(feature = "diagnostics")]
    pub(crate) fn tool_kind(&self) -> Option<&ToolPaneState> {
        match self {
            Self::Pane(PaneViewState::Tool(tool_ref)) => Some(&tool_ref.kind),
            Self::Tool(tool_ref) => Some(&tool_ref.kind),
            _ => None,
        }
    }
}
