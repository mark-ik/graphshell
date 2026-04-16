/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Command routing bridge: translates workbench commands into `NavAction`s.
//!
//! During the parallel phase, these functions apply NavActions to the
//! GraphTree alongside existing tile_view_ops calls. Once GraphTree
//! becomes the layout authority, the tile_view_ops calls will be removed
//! and these functions will be the sole command path.
//!
//! Each function takes `&mut GraphTree<NodeKey>` and returns `NavResult`
//! containing any intents the host should process.

use graph_tree::{
    FocusCycleRegion, FocusDirection, GraphTree, LayoutMode, Lifecycle, NavAction, NavResult,
    ProjectionLens, Provenance,
};

use crate::graph::{NodeKey, NodeLifecycle};

use super::graph_tree_sync::to_graph_tree_lifecycle;

/// Attach a node to the GraphTree as a traversal child of `source`.
///
/// Corresponds to `tile_view_ops::open_or_focus_node_pane` and variants.
pub(crate) fn attach_traversal(
    graph_tree: &mut GraphTree<NodeKey>,
    node_key: NodeKey,
    source: Option<NodeKey>,
) -> NavResult<NodeKey> {
    if graph_tree.contains(&node_key) {
        // Already present — just activate.
        return graph_tree.apply(NavAction::Activate(node_key));
    }

    let provenance = match source {
        Some(src) if graph_tree.contains(&src) => Provenance::Traversal {
            source: src,
            edge_kind: None,
        },
        _ => Provenance::Anchor,
    };

    let attach_result = graph_tree.apply(NavAction::Attach {
        member: node_key,
        provenance,
    });

    // Set initial lifecycle and activate.
    graph_tree.apply(NavAction::SetLifecycle(node_key, Lifecycle::Active));
    let activate_result = graph_tree.apply(NavAction::Activate(node_key));

    // Merge intents from both operations.
    NavResult {
        intents: attach_result
            .intents
            .into_iter()
            .chain(activate_result.intents)
            .collect(),
        structure_changed: true,
        session_changed: true,
    }
}

/// Dismiss a node from the GraphTree.
///
/// Corresponds to `tile_view_ops::close_pane` for node panes.
pub(crate) fn dismiss_node(
    graph_tree: &mut GraphTree<NodeKey>,
    node_key: NodeKey,
) -> NavResult<NodeKey> {
    graph_tree.apply(NavAction::Dismiss(node_key))
}

/// Activate (focus) a node.
///
/// Corresponds to tile focus operations.
pub(crate) fn activate_node(
    graph_tree: &mut GraphTree<NodeKey>,
    node_key: NodeKey,
) -> NavResult<NodeKey> {
    graph_tree.apply(NavAction::Activate(node_key))
}

/// Cycle focus to next/previous visible member.
///
/// Corresponds to `tile_view_ops::cycle_focus_region`.
pub(crate) fn cycle_focus(
    graph_tree: &mut GraphTree<NodeKey>,
    direction: FocusDirection,
) -> NavResult<NodeKey> {
    graph_tree.apply(NavAction::CycleFocus(direction))
}

/// Cycle focus within a specific tree region.
pub(crate) fn cycle_focus_region(
    graph_tree: &mut GraphTree<NodeKey>,
    region: FocusCycleRegion,
) -> NavResult<NodeKey> {
    graph_tree.apply(NavAction::CycleFocusRegion(region))
}

/// Toggle expand/collapse of a tree node.
///
/// Corresponds to tree-style tab sidebar expand/collapse.
pub(crate) fn toggle_expand(
    graph_tree: &mut GraphTree<NodeKey>,
    node_key: NodeKey,
) -> NavResult<NodeKey> {
    graph_tree.apply(NavAction::ToggleExpand(node_key))
}

/// Reveal a member by expanding all its ancestors.
///
/// Corresponds to "reveal in sidebar" operations.
pub(crate) fn reveal(graph_tree: &mut GraphTree<NodeKey>, node_key: NodeKey) -> NavResult<NodeKey> {
    graph_tree.apply(NavAction::Reveal(node_key))
}

/// Update lifecycle for a node.
pub(crate) fn set_lifecycle(
    graph_tree: &mut GraphTree<NodeKey>,
    node_key: NodeKey,
    lifecycle: NodeLifecycle,
) -> NavResult<NodeKey> {
    graph_tree.apply(NavAction::SetLifecycle(
        node_key,
        to_graph_tree_lifecycle(lifecycle),
    ))
}

/// Reparent a node under a new parent.
///
/// Corresponds to drag-reparent in tree-style sidebar.
pub(crate) fn reparent(
    graph_tree: &mut GraphTree<NodeKey>,
    node_key: NodeKey,
    new_parent: NodeKey,
) -> NavResult<NodeKey> {
    graph_tree.apply(NavAction::Reparent {
        member: node_key,
        new_parent,
    })
}

/// Detach a node (and optionally its subtree).
///
/// Corresponds to `tile_view_ops::close_pane` with recursive option.
pub(crate) fn detach(
    graph_tree: &mut GraphTree<NodeKey>,
    node_key: NodeKey,
    recursive: bool,
) -> NavResult<NodeKey> {
    graph_tree.apply(NavAction::Detach {
        member: node_key,
        recursive,
    })
}

/// Switch layout mode.
pub(crate) fn set_layout_mode(
    graph_tree: &mut GraphTree<NodeKey>,
    mode: LayoutMode,
) -> NavResult<NodeKey> {
    graph_tree.apply(NavAction::SetLayoutMode(mode))
}

/// Switch projection lens.
pub(crate) fn set_lens(
    graph_tree: &mut GraphTree<NodeKey>,
    lens: ProjectionLens,
) -> NavResult<NodeKey> {
    graph_tree.apply(NavAction::SetLens(lens))
}
