/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Tile kinds used by egui_tiles layout.
//!
//! `TileKind` variants correspond to `PaneViewState` payload kinds.
//! Dispatch on `TileKind` is the workbench-layer expression of pane view payload dispatch.
//! See `pane_model.rs` for the canonical pane payload model.

use crate::app::GraphViewId;
use crate::shell::desktop::workbench::pane_model::{NodePaneState, ToolPaneState};

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) enum TileKind {
    /// A graph pane: renders a spatial graph viewport with independent camera and Lens.
    Graph(GraphViewId),
    /// A node viewer pane: renders a node via the selected viewer backend.
    ///
    /// Serde alias `"WebView"` preserves backward compatibility with persisted tile layouts
    /// created before this variant was renamed from `WebView(NodeKey)`.
    #[serde(alias = "WebView")]
    Node(NodePaneState),
    /// A tool pane: diagnostics inspector, history manager, settings, etc.
    #[cfg(feature = "diagnostics")]
    Tool(ToolPaneState),
}
