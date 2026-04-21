/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Host-neutral pane-tree walking for [`super::ux_tree::build_snapshot`].
//!
//! Motivation (M6 §5.1): `build_snapshot` currently threads a
//! `&Tree<TileKind>` through its pane-walk (`push_nodes`). That ties the
//! uxtree builder to egui_tiles. This module defines a host-neutral
//! trait `PaneTreeWalker` that abstracts pane enumeration + resolution
//! so the same snapshot builder works from either an egui_tiles tree
//! or a GraphTree-backed source.
//!
//! ## Shape
//!
//! - [`UxPaneHandle`]: opaque per-pane identifier. Encoded as `u64` so
//!   the trait is dyn-compatible without lifetimes.
//! - [`PaneTreeWalker`]: methods every host implements.
//! - [`ResolvedPane`]: owned enum describing one tree node (a leaf
//!   pane carrying a `TileKind` payload, or a container with children).
//! - [`TilesTreeWalker`]: the tiles-tree-backed implementation.
//!
//! `TileKind` is graphshell-owned (not egui-specific), so both hosts
//! can produce `ResolvedPane::Pane` entries. The GraphTree-backed
//! walker synthesizes `TileKind` values from the `graph_runtime` pane
//! caches.
//!
//! ## Status
//!
//! M6 §5.1 step 2. Trait + tiles-backed impl land here; push_nodes
//! migration to the trait follows in a later commit. Nothing consumes
//! `PaneTreeWalker` yet.

use std::collections::HashSet;

use egui_tiles::{Container, Tile, TileId, Tree};

use crate::app::GraphBrowserApp;
use crate::shell::desktop::workbench::tile_kind::TileKind;

/// Opaque per-pane handle. Hosts pack whatever identity they need
/// (TileId's `u64` bits on egui, PaneId's hash on iced) into this.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct UxPaneHandle(pub u64);

/// Container variants the workbench tree can produce.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ContainerKind {
    Tabs,
    Linear,
    Grid,
}

/// Owned resolution of a pane handle.
///
/// The enum owns its data (no borrows) so `dyn PaneTreeWalker` is
/// object-safe and ergonomic to hand to a walking visitor.
pub(crate) enum ResolvedPane {
    /// Leaf pane carrying a `TileKind` payload — the host-neutral
    /// pane-content enum. Snapshots match on this to emit
    /// GraphSurface/NodePane/ToolPane semantic entries.
    Pane {
        ux_node_id: String,
        payload: TileKind,
    },
    /// Container — tabbed, linear split, or grid — with ordered
    /// children.
    Container {
        ux_node_id: String,
        kind: ContainerKind,
        children: Vec<UxPaneHandle>,
        /// Optional host-specific label override (currently used for
        /// the workbench-root "Frame:" label).
        label: Option<String>,
    },
}

/// Abstraction over pane-tree enumeration.
///
/// Each host provides one. `build_snapshot` and its pane-walk
/// (`push_nodes`) will migrate onto this trait in follow-on commits so
/// the uxtree builder stops taking `&Tree<TileKind>` directly.
pub(crate) trait PaneTreeWalker {
    /// Root pane handle, if the tree is non-empty.
    fn root(&self) -> Option<UxPaneHandle>;

    /// Resolve a handle into owned data. Returns `None` if the handle
    /// is unknown to this walker.
    fn resolve(&self, graph_app: &GraphBrowserApp, handle: UxPaneHandle) -> Option<ResolvedPane>;

    /// Whether the given handle is "active" in the host's layout (e.g.,
    /// visible on top of a tab stack, focused split child).
    fn is_active(&self, handle: UxPaneHandle) -> bool;
}

// ---------------------------------------------------------------------------
// TilesTreeWalker — egui_tiles-backed implementation
// ---------------------------------------------------------------------------

/// Walker backed by an `egui_tiles::Tree<TileKind>`. Preserves the
/// stable `"uxnode://workbench/tile/#N/..."` identity scheme used by
/// the pre-refactor pane walk so existing snapshot tests still pass.
pub(crate) struct TilesTreeWalker<'a> {
    tree: &'a Tree<TileKind>,
    active: HashSet<TileId>,
}

impl<'a> TilesTreeWalker<'a> {
    pub(crate) fn new(tree: &'a Tree<TileKind>) -> Self {
        let active = tree.active_tiles().into_iter().collect();
        Self { tree, active }
    }

    fn handle_for(tile_id: TileId) -> UxPaneHandle {
        // egui_tiles' TileId is `u64` internally. `Debug` prints
        // `#N`, so we preserve the numeric identity by extracting via
        // the trip through the u64-compatible conversion.
        //
        // TileId exposes `TileId::from_u64` but not the inverse; it
        // does impl `Hash`, so we use a minimal bit-cast trick via
        // unsafe-free formatting parse. Cheaper: format + parse. This
        // is only called during snapshot building and is bounded by
        // tree size.
        let formatted = format!("{tile_id:?}");
        let numeric = formatted.trim_start_matches('#');
        UxPaneHandle(numeric.parse::<u64>().unwrap_or(0))
    }

    fn tile_id_for(handle: UxPaneHandle) -> TileId {
        TileId::from_u64(handle.0)
    }
}

impl<'a> PaneTreeWalker for TilesTreeWalker<'a> {
    fn root(&self) -> Option<UxPaneHandle> {
        self.tree.root().map(Self::handle_for)
    }

    fn resolve(&self, graph_app: &GraphBrowserApp, handle: UxPaneHandle) -> Option<ResolvedPane> {
        let tile_id = Self::tile_id_for(handle);
        let tile = self.tree.tiles.get(tile_id)?;
        let ux_node_id = super::ux_tree::ux_node_id_for_tile(tile_id, tile);

        match tile {
            Tile::Pane(kind) => Some(ResolvedPane::Pane {
                ux_node_id,
                payload: kind.clone(),
            }),
            Tile::Container(container) => match container {
                Container::Tabs(tabs) => Some(ResolvedPane::Container {
                    ux_node_id,
                    kind: ContainerKind::Tabs,
                    children: tabs
                        .children
                        .iter()
                        .copied()
                        .map(Self::handle_for)
                        .collect(),
                    label: tabs_root_label(self.tree, graph_app, tile_id, tabs.children.len()),
                }),
                Container::Linear(linear) => Some(ResolvedPane::Container {
                    ux_node_id,
                    kind: ContainerKind::Linear,
                    children: linear
                        .children
                        .iter()
                        .copied()
                        .map(Self::handle_for)
                        .collect(),
                    label: None,
                }),
                Container::Grid(grid) => Some(ResolvedPane::Container {
                    ux_node_id,
                    kind: ContainerKind::Grid,
                    children: grid.children().copied().map(Self::handle_for).collect(),
                    label: None,
                }),
            },
        }
    }

    fn is_active(&self, handle: UxPaneHandle) -> bool {
        self.active.contains(&Self::tile_id_for(handle))
    }
}

/// Host-specific label for the root tabs container ("Frame: foo (N)").
/// Extracted from `current_frame_tab_container_label` so the walker can
/// embed the label in the resolved data without the downstream consumer
/// needing to re-check the tree.
fn tabs_root_label(
    tree: &Tree<TileKind>,
    graph_app: &GraphBrowserApp,
    tile_id: TileId,
    child_count: usize,
) -> Option<String> {
    (tree.root() == Some(tile_id))
        .then(|| graph_app.current_frame_name())
        .flatten()
        .map(|frame_name| format!("Frame: {frame_name} ({child_count})"))
}

// ---------------------------------------------------------------------------
// GraphTreeWalker — GraphTree-backed implementation (partial, panes only)
// ---------------------------------------------------------------------------

use crate::graph::NodeKey;

/// Sentinel handle value for the synthetic workbench-root container
/// emitted by `GraphTreeWalker`. Real member handles encode
/// `NodeKey::index()` which is `usize`-width; reserving `u64::MAX`
/// keeps the namespaces disjoint.
const GRAPH_TREE_SYNTHETIC_ROOT: u64 = u64::MAX;

/// Walker backed by `graph_tree::GraphTree<NodeKey>` — the workbench
/// membership authority.
///
/// **Partial**: emits only NodePane entries (one per `GraphTree`
/// member) plus a single synthetic Linear container grouping the
/// topological roots. Tabs/Grid containers, Graph panes, and Tool
/// panes are not yet represented — GraphTree's member type is
/// `NodeKey`, so graph panes (keyed by `GraphViewId`) live outside the
/// membership model. Follow-on work adds either a widened
/// `GraphTree::Member` type or a separate side-channel for
/// non-node-keyed panes.
///
/// The identity scheme is distinct from `TilesTreeWalker`'s — uses
/// `"uxnode://workbench/member/<idx>"` rather than
/// `"uxnode://workbench/tile/#N/..."` — so snapshots from the two
/// walkers don't collide.
pub(crate) struct GraphTreeWalker<'a> {
    tree: &'a graph_tree::GraphTree<NodeKey>,
    roots: Vec<NodeKey>,
}

impl<'a> GraphTreeWalker<'a> {
    pub(crate) fn new(tree: &'a graph_tree::GraphTree<NodeKey>) -> Self {
        let roots = tree.topology().roots().to_vec();
        Self { tree, roots }
    }

    fn handle_for(key: NodeKey) -> UxPaneHandle {
        UxPaneHandle(key.index() as u64)
    }

    fn node_key_for(handle: UxPaneHandle) -> Option<NodeKey> {
        if handle.0 == GRAPH_TREE_SYNTHETIC_ROOT {
            None
        } else {
            Some(NodeKey::new(handle.0 as usize))
        }
    }
}

impl<'a> PaneTreeWalker for GraphTreeWalker<'a> {
    fn root(&self) -> Option<UxPaneHandle> {
        if self.tree.member_count() == 0 {
            None
        } else {
            Some(UxPaneHandle(GRAPH_TREE_SYNTHETIC_ROOT))
        }
    }

    fn resolve(&self, graph_app: &GraphBrowserApp, handle: UxPaneHandle) -> Option<ResolvedPane> {
        if handle.0 == GRAPH_TREE_SYNTHETIC_ROOT {
            return Some(ResolvedPane::Container {
                ux_node_id: "uxnode://workbench/member/root".to_string(),
                kind: ContainerKind::Linear,
                children: self.roots.iter().copied().map(Self::handle_for).collect(),
                label: None,
            });
        }

        let node_key = Self::node_key_for(handle)?;
        if !self.tree.contains(&node_key) {
            return None;
        }

        // Children from topology — recursion happens via children_of.
        let children: Vec<_> = self
            .tree
            .children_of(&node_key)
            .iter()
            .copied()
            .map(Self::handle_for)
            .collect();

        let ux_node_id = format!("uxnode://workbench/member/{}", node_key.index());

        // Leaf node: synthesize a NodePane TileKind from graph_runtime
        // caches. Render mode defaults to Placeholder if not cached.
        if children.is_empty() {
            let runtime = &graph_app.workspace.graph_runtime;
            let pane_id = runtime
                .node_pane_ids
                .get(&node_key)
                .copied()
                .unwrap_or_else(PaneId::new);
            let render_mode = runtime
                .pane_render_modes
                .get(&pane_id)
                .copied()
                .unwrap_or_default();
            let mut state =
                crate::shell::desktop::workbench::pane_model::NodePaneState::for_node(node_key);
            state.pane_id = pane_id;
            state.render_mode = render_mode;
            return Some(ResolvedPane::Pane {
                ux_node_id,
                payload: TileKind::Node(state),
            });
        }

        // Non-leaf member: emit as a linear container of its
        // sub-members. Consistent with GraphTree's hierarchical shape
        // (parent/child relationships from topology).
        Some(ResolvedPane::Container {
            ux_node_id,
            kind: ContainerKind::Linear,
            children,
            label: None,
        })
    }

    fn is_active(&self, handle: UxPaneHandle) -> bool {
        if handle.0 == GRAPH_TREE_SYNTHETIC_ROOT {
            // The synthetic workbench root is always "focused" in the
            // sense the tiles walker treats `active` — it's the top of
            // the workbench spine.
            return true;
        }
        let Some(node_key) = Self::node_key_for(handle) else {
            return false;
        };
        self.tree.active() == Some(&node_key)
    }
}

use crate::shell::desktop::workbench::pane_model::PaneId;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::GraphViewId;
    use crate::shell::desktop::workbench::pane_model::GraphPaneRef;
    use egui_tiles::Tiles;

    #[test]
    fn empty_tree_has_no_root() {
        let tree = Tree::new(
            "empty",
            TileId::from_u64(0),
            egui_tiles::Tiles::<TileKind>::default(),
        );
        let walker = TilesTreeWalker::new(&tree);
        // Tree::new with an unknown root yields None from .root() if
        // the tile isn't registered — which matches the "empty" shape.
        let handle = walker.root();
        let app = GraphBrowserApp::new_for_testing();
        if let Some(h) = handle {
            assert!(walker.resolve(&app, h).is_none());
        }
    }

    #[test]
    fn single_graph_pane_resolves_to_pane_kind() {
        let mut tiles = Tiles::default();
        let graph_tile = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(GraphViewId::new())));
        let tree = Tree::new("single_pane", graph_tile, tiles);
        let walker = TilesTreeWalker::new(&tree);
        let app = GraphBrowserApp::new_for_testing();

        let root = walker.root().expect("non-empty tree has a root");
        match walker.resolve(&app, root).expect("root resolves") {
            ResolvedPane::Pane { payload, .. } => {
                assert!(matches!(payload, TileKind::Graph(_)));
            }
            other => panic!("expected Pane, got {:?}", discriminant(&other)),
        }
    }

    #[test]
    fn tiles_walker_roundtrips_handle_for_tile_id() {
        // Construct a known TileId and ensure handle_for/tile_id_for
        // roundtrip through the formatted-u64 encoding.
        let id = TileId::from_u64(42);
        let handle = TilesTreeWalker::handle_for(id);
        assert_eq!(handle, UxPaneHandle(42));
        assert_eq!(TilesTreeWalker::tile_id_for(handle), id);
    }

    /// Minimal debug-name accessor so panic messages surface the
    /// variant without implementing Debug on ResolvedPane.
    fn discriminant(pane: &ResolvedPane) -> &'static str {
        match pane {
            ResolvedPane::Pane { .. } => "Pane",
            ResolvedPane::Container { .. } => "Container",
        }
    }

    // -----------------------------------------------------------------------
    // GraphTreeWalker tests
    // -----------------------------------------------------------------------

    #[test]
    fn graph_tree_walker_empty_tree_has_no_root() {
        let tree = graph_tree::GraphTree::<NodeKey>::new(
            graph_tree::LayoutMode::TreeStyleTabs,
            graph_tree::ProjectionLens::Traversal,
        );
        let walker = GraphTreeWalker::new(&tree);
        assert!(walker.root().is_none());
    }

    #[test]
    fn graph_tree_walker_emits_synthetic_container_for_populated_tree() {
        use graph_tree::{Lifecycle, MemberEntry, Provenance, TreeTopology};

        let app = GraphBrowserApp::new_for_testing();
        // Synthesize a node key directly; the walker only needs it to
        // be a valid NodeIndex for encoding purposes.
        let node = NodeKey::new(0);

        let mut topology = TreeTopology::<NodeKey>::new();
        topology.attach_root(node);
        let tree = graph_tree::GraphTree::<NodeKey>::from_members(
            vec![(
                node,
                MemberEntry::new(Lifecycle::Active, Provenance::Anchor),
            )],
            topology,
            Vec::new(),
            graph_tree::LayoutMode::TreeStyleTabs,
            graph_tree::ProjectionLens::Traversal,
        );

        let walker = GraphTreeWalker::new(&tree);
        let root = walker.root().expect("populated tree has a root");
        assert_eq!(root, UxPaneHandle(GRAPH_TREE_SYNTHETIC_ROOT));

        match walker.resolve(&app, root).expect("root resolves") {
            ResolvedPane::Container { kind, children, .. } => {
                assert_eq!(kind, ContainerKind::Linear);
                assert_eq!(children.len(), 1);
            }
            other => panic!(
                "expected synthetic root Container, got {}",
                discriminant(&other)
            ),
        }
    }

    #[test]
    fn graph_tree_walker_handle_roundtrip() {
        let key = NodeKey::new(7);
        let handle = GraphTreeWalker::handle_for(key);
        assert_eq!(handle, UxPaneHandle(7));
        assert_eq!(GraphTreeWalker::node_key_for(handle), Some(key));
        assert_eq!(
            GraphTreeWalker::node_key_for(UxPaneHandle(GRAPH_TREE_SYNTHETIC_ROOT)),
            None
        );
    }
}
