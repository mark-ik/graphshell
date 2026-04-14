/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Dual-write adapter: keeps `GraphTree` in sync with `egui_tiles` mutations.
//!
//! During Phase B of the egui_tiles → GraphTree migration, every tile
//! mutation site must also dispatch the corresponding `NavAction` to the
//! `GraphTree`. This module centralizes that forwarding so call sites
//! need a single `dual_write::*` call instead of two inline calls.
//!
//! Direction: `tiles_tree` is still the rendering owner; `GraphTree` is
//! the semantic shadow. Phase D will flip this — `GraphTree` becomes
//! the authority and tiles become the shadow.
//!
//! **Lifecycle**: This entire module is transitional. Once Phase D
//! completes, the `tile_view_ops` calls inside each wrapper get removed,
//! leaving only the `graph_tree_commands` calls (which become the
//! canonical command path).

use egui_tiles::Tree;
use graph_tree::{GraphTree, NavResult};

use crate::app::GraphBrowserApp;
use crate::graph::NodeKey;

use super::graph_tree_binding;
use super::graph_tree_commands;
use super::tile_kind::TileKind;
use super::tile_view_ops::{self, TileOpenMode};

/// Open or focus a node pane in both trees.
///
/// Tiles-side: calls `open_or_focus_node_pane`.
/// GraphTree-side: calls `attach_traversal` (which handles already-present).
pub(crate) fn open_or_focus_node(
    tiles_tree: &mut Tree<TileKind>,
    graph_tree: &mut GraphTree<NodeKey>,
    graph_app: &GraphBrowserApp,
    node_key: NodeKey,
    source: Option<NodeKey>,
) -> NavResult<NodeKey> {
    tile_view_ops::open_or_focus_node_pane(tiles_tree, graph_app, node_key);
    graph_tree_commands::attach_traversal(graph_tree, node_key, source)
}

/// Open or focus a node pane with a specific open mode.
pub(crate) fn open_or_focus_node_with_mode(
    tiles_tree: &mut Tree<TileKind>,
    graph_tree: &mut GraphTree<NodeKey>,
    graph_app: &GraphBrowserApp,
    node_key: NodeKey,
    source: Option<NodeKey>,
    mode: TileOpenMode,
) -> NavResult<NodeKey> {
    tile_view_ops::open_or_focus_node_pane_with_mode(tiles_tree, graph_app, node_key, mode);
    graph_tree_commands::attach_traversal(graph_tree, node_key, source)
}

/// Close a pane by PaneId, dismissing the corresponding node in GraphTree.
///
/// Returns `true` if the tile was actually closed.
pub(crate) fn close_pane(
    tiles_tree: &mut Tree<TileKind>,
    graph_tree: &mut GraphTree<NodeKey>,
    pane_id: super::pane_model::PaneId,
    node_key: Option<NodeKey>,
) -> bool {
    let closed = tile_view_ops::close_pane(tiles_tree, pane_id);
    if closed {
        if let Some(nk) = node_key {
            // Fork detection: if the dismissed member belongs to a linked
            // graphlet, this manual dismissal diverges from the spec.
            if let Some((gid, reason)) =
                graph_tree_binding::check_fork_on_manual_mutation(graph_tree, &nk, "dismiss")
            {
                log::info!(
                    "dual_write: manual dismiss of {:?} forks linked graphlet {} — {}",
                    nk, gid, reason,
                );
                graph_tree_binding::apply_fork(graph_tree, gid, reason);
            }
            graph_tree_commands::dismiss_node(graph_tree, nk);
        }
    }
    closed
}

/// Focus a pane by PaneId, activating the corresponding node in GraphTree.
pub(crate) fn focus_pane(
    tiles_tree: &mut Tree<TileKind>,
    graph_tree: &mut GraphTree<NodeKey>,
    pane_id: super::pane_model::PaneId,
    node_key: Option<NodeKey>,
) -> bool {
    let focused = tile_view_ops::focus_pane(tiles_tree, pane_id);
    if focused {
        if let Some(nk) = node_key {
            graph_tree_commands::activate_node(graph_tree, nk);
        }
    }
    focused
}

/// Ensure there's an active tile. If recovery activates one, sync to GraphTree.
pub(crate) fn ensure_active_tile(
    tiles_tree: &mut Tree<TileKind>,
    graph_tree: &mut GraphTree<NodeKey>,
) -> bool {
    let changed = tile_view_ops::ensure_active_tile(tiles_tree);
    if changed {
        // Find what became active and sync.
        if let Some(node_key) = find_active_node_from_tiles(tiles_tree) {
            graph_tree_commands::activate_node(graph_tree, node_key);
        }
    }
    changed
}

/// Cycle focus region in both trees.
pub(crate) fn cycle_focus_region(
    tiles_tree: &mut Tree<TileKind>,
    graph_tree: &mut GraphTree<NodeKey>,
) -> bool {
    let changed = tile_view_ops::cycle_focus_region(tiles_tree);
    if changed {
        if let Some(node_key) = find_active_node_from_tiles(tiles_tree) {
            graph_tree_commands::activate_node(graph_tree, node_key);
        }
    }
    changed
}

/// Open or focus a tool pane. Tool panes don't have GraphTree members
/// (they're not graph nodes), but we include this for call-site symmetry.
#[cfg(feature = "diagnostics")]
pub(crate) fn open_or_focus_tool_pane(
    tiles_tree: &mut Tree<TileKind>,
    _graph_tree: &mut GraphTree<NodeKey>,
    kind: super::pane_model::ToolPaneState,
) {
    tile_view_ops::open_or_focus_tool_pane(tiles_tree, kind);
    // No GraphTree action — tool panes are shell-only, not graph members.
}

/// Open or focus a tool pane (non-diagnostics build).
#[cfg(not(feature = "diagnostics"))]
pub(crate) fn open_or_focus_tool_pane(
    tiles_tree: &mut Tree<TileKind>,
    _graph_tree: &mut GraphTree<NodeKey>,
    kind: super::pane_model::ToolPaneState,
) {
    tile_view_ops::open_or_focus_tool_pane(tiles_tree, kind);
}

/// Close a tool pane.
#[cfg(feature = "diagnostics")]
pub(crate) fn close_tool_pane(
    tiles_tree: &mut Tree<TileKind>,
    _graph_tree: &mut GraphTree<NodeKey>,
    kind: super::pane_model::ToolPaneState,
) -> bool {
    tile_view_ops::close_tool_pane(tiles_tree, kind)
    // No GraphTree action — tool panes are shell-only.
}

/// Dismiss all floating panes.
pub(crate) fn dismiss_floating_panes(
    tiles_tree: &mut Tree<TileKind>,
    _graph_tree: &mut GraphTree<NodeKey>,
) {
    tile_view_ops::dismiss_floating_panes(tiles_tree);
    // Floating panes are transient presentation — not tracked in GraphTree.
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extract the active node from the tile tree (for sync after tile-side focus changes).
fn find_active_node_from_tiles(tiles_tree: &Tree<TileKind>) -> Option<NodeKey> {
    use egui_tiles::Tile;
    // The tile tree's "active" is whatever was most recently focused.
    // Walk tiles looking for the one that's active.
    for (_tile_id, tile) in tiles_tree.tiles.iter() {
        if let Tile::Pane(kind) = tile {
            if let Some(state) = kind.node_state() {
                // We can't easily tell which tile is "active" from the tile tree
                // alone without the behavior's active_id. Use the first node pane
                // as a fallback — the caller should pass the active node explicitly
                // when available.
                return Some(state.node);
            }
        }
    }
    None
}

