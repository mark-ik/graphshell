// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use crate::graphlet::GraphletId;
use crate::member::Lifecycle;
use crate::topology::TreeRow;
use crate::MemberId;
use crate::Rect;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// How the tree's spatial layout is computed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayoutMode {
    /// Tree-style tabs + single focused content pane. **Default.**
    /// The tree IS the navigation; one member is active at a time.
    TreeStyleTabs,
    /// Flat tab bar: warm/active members as tabs, topology-ordered.
    FlatTabs,
    /// Split panes: active members get taffy-computed rects.
    /// Supports min/max, flex grow/shrink, nested splits.
    SplitPanes,
}

impl Default for LayoutMode {
    fn default() -> Self {
        Self::TreeStyleTabs
    }
}

/// Result of layout computation.
#[derive(Clone, Debug)]
pub struct LayoutResult<N: MemberId> {
    /// Pane rectangles (SplitPanes mode only).
    pub pane_rects: HashMap<N, Rect>,
    /// Tab ordering (FlatTabs mode; also populated in other modes).
    pub tab_order: Vec<TabEntry<N>>,
    /// Tree rows (always populated — powers the sidebar in every mode).
    pub tree_rows: Vec<OwnedTreeRow<N>>,
    /// Currently active member.
    pub active: Option<N>,
}

/// Tab bar entry.
#[derive(Clone, Debug)]
pub struct TabEntry<N: MemberId> {
    pub member: N,
    pub lifecycle: Lifecycle,
    pub is_anchor: bool,
    pub depth: usize,
    pub graphlet_id: Option<GraphletId>,
}

/// Owned version of `TreeRow` for storage in `LayoutResult`.
#[derive(Clone, Debug)]
pub struct OwnedTreeRow<N: MemberId> {
    pub member: N,
    pub depth: usize,
    pub is_expanded: bool,
    pub has_children: bool,
    pub is_last_sibling: bool,
    pub graphlet_id: Option<GraphletId>,
}

impl<'a, N: MemberId> From<TreeRow<'a, N>> for OwnedTreeRow<N> {
    fn from(row: TreeRow<'a, N>) -> Self {
        Self {
            member: row.member.clone(),
            depth: row.depth,
            is_expanded: row.is_expanded,
            has_children: row.has_children,
            is_last_sibling: row.is_last_sibling,
            graphlet_id: row.graphlet_id,
        }
    }
}

/// Framework adapter trait. Each framework implements this once.
/// The tree does the heavy lifting; the adapter just paints.
pub trait GraphTreeRenderer<N: MemberId> {
    type Ctx;
    type Out;

    fn render_tree_tabs(
        &mut self,
        tree: &crate::tree::GraphTree<N>,
        rows: &[OwnedTreeRow<N>],
        ctx: &mut Self::Ctx,
    ) -> Self::Out;

    fn render_flat_tabs(
        &mut self,
        tree: &crate::tree::GraphTree<N>,
        tabs: &[TabEntry<N>],
        ctx: &mut Self::Ctx,
    ) -> Self::Out;

    fn render_pane_chrome(
        &mut self,
        tree: &crate::tree::GraphTree<N>,
        rects: &HashMap<N, Rect>,
        ctx: &mut Self::Ctx,
    ) -> Self::Out;
}
