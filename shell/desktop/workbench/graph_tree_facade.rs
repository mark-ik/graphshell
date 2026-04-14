/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Workbench layout facade: abstracts over the layout tree implementation.
//!
//! This trait captures the queries and mutations that the shell, compositor,
//! and navigator actually need from the layout tree. During migration, both
//! `egui_tiles::Tree<TileKind>` and `GraphTree<NodeKey>` implement this
//! trait (or the call sites use the facade adapter functions below).
//!
//! Once all call sites speak through this facade, the `egui_tiles` backend
//! can be removed by deleting the `tiles_tree` field and its adapter.
//!
//! ## Remaining coupling points (63 files, ~1646 references)
//!
//! High-traffic categories:
//!
//! 1. **Pane identity and lookup** (~400 refs)
//!    - `tile_id_for_pane`, `pane_id` on TileKind, tile iteration
//!    - Migration: PaneId becomes a field on MemberEntry
//!
//! 2. **Layout rect queries** (~200 refs)
//!    - `tiles_tree.tiles.rect(tile_id)`, `active_node_pane_rects`
//!    - Migration: `compute_layout().pane_rects` (bridge already exists)
//!
//! 3. **Tile mutations** (~350 refs)
//!    - `insert_pane`, `remove_recursively`, focus/activate, tab management
//!    - Migration: NavAction dispatch (Phase 4c bridge exists)
//!
//! 4. **Tile tree threading through frame pipeline** (~300 refs)
//!    - `tiles_tree` passed through 6 arg structs and ~20 functions
//!    - Migration: Replace with `&mut GraphTree<NodeKey>` (most invasive)
//!
//! 5. **Persistence** (~100 refs)
//!    - Serialize/deserialize Tree<TileKind>
//!    - Migration: Phase 5 (done — parallel persistence in place)
//!
//! 6. **Compositor** (~200 refs)
//!    - Content callbacks keyed by tile_id, GL state, overlay passes
//!    - Migration: Rekey from TileId to NodeKey (most complex)

use graph_tree::GraphTree;

use crate::graph::NodeKey;
use crate::shell::desktop::workbench::pane_model::PaneId;

/// Active pane summary for compositor and focus management.
#[derive(Clone, Debug)]
pub(crate) struct ActivePaneSummary {
    pub(crate) pane_id: PaneId,
    pub(crate) node_key: NodeKey,
    pub(crate) rect: egui::Rect,
}

/// Produce active pane summaries from the GraphTree.
///
/// This is the GraphTree equivalent of `tile_compositor::active_node_pane_rects`,
/// looking up PaneId from the tile tree during migration.
pub(crate) fn active_pane_summaries(
    graph_tree: &GraphTree<NodeKey>,
    pane_id_lookup: &dyn Fn(NodeKey) -> Option<PaneId>,
    available: graph_tree::Rect,
) -> Vec<ActivePaneSummary> {
    let layout = graph_tree.compute_layout(available);
    layout
        .pane_rects
        .iter()
        .filter_map(|(member, rect)| {
            let pane_id = pane_id_lookup(*member)?;
            Some(ActivePaneSummary {
                pane_id,
                node_key: *member,
                rect: egui::Rect::from_min_size(
                    egui::pos2(rect.x, rect.y),
                    egui::vec2(rect.w, rect.h),
                ),
            })
        })
        .collect()
}

/// Focused node resolution from GraphTree.
///
/// Returns the currently active member if it exists, or falls back to
/// the first active-lifecycle member.
pub(crate) fn focused_node(graph_tree: &GraphTree<NodeKey>) -> Option<NodeKey> {
    graph_tree.active().cloned().or_else(|| {
        graph_tree
            .members()
            .find(|(_, entry)| entry.is_active())
            .map(|(k, _)| *k)
    })
}

/// Member count by lifecycle.
pub(crate) fn lifecycle_summary(
    graph_tree: &GraphTree<NodeKey>,
) -> (usize, usize, usize) {
    (
        graph_tree.active_count(),
        graph_tree.warm_count(),
        graph_tree.cold_count(),
    )
}

