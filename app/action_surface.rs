/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Action-surface state: the single enum that replaces the legacy bool
//! soup for `command_palette` / `context_palette` / `radial_menu`
//! visibility. Introduced by the 2026-04-20 action surfaces redesign
//! plan (`aspect_control/2026-04-20_action_surfaces_redesign_plan.md`).
//!
//! This module is the shell-side home for the new vocabulary. Phase E
//! of the redesign plan lifts these types to `crates/graph-canvas` so
//! the iced host can consume them directly. For now they live here.

use crate::graph::NodeKey;

use super::GraphViewId;

/// Scope under which an action surface was opened. Determines which
/// registry actions appear and drives the `close_on_scope_transition`
/// invariant when focus, graph state, or the target changes.
///
/// `ActionScope` is the explicit answer to the "palette reappeared
/// unexpectedly" bug: by capturing the originating scope alongside
/// the open-state, a state change that should close the surface
/// (graph cleared, target deleted, focus moved to a different view)
/// becomes observable instead of leaking through a bare bool.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum ActionScope {
    /// Global scope — palette opened via `Ctrl+K` with no target.
    /// Survives focus and graph-state changes.
    #[default]
    Global,
    /// Graph scope — opened from a graph view, optionally targeting a
    /// specific node or frame.
    Graph {
        view_id: GraphViewId,
        target: ScopeTarget,
    },
    /// Workbench scope — opened from a workbench pane or tab chrome.
    /// The pane identity is not yet threaded here; follow-on.
    Workbench,
}

/// Per-graph-scope target. `None` means "graph-scoped but no node or
/// frame picked" (e.g., free-space right-click inside a graph view).
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum ScopeTarget {
    #[default]
    None,
    Node(NodeKey),
    Frame(String),
}

/// Where an action surface anchors itself. Resolved to a screen point
/// at render time — `Target` variants track their target across camera
/// moves, `ViewportPoint` is pinned to a screen location, `ScreenCenter`
/// centers in the active viewport (global palette).
///
/// The "contextual palette followed the cursor" bug came from the
/// legacy pipeline storing a `[f32; 2]` screen-point at click time.
/// New target-aware sites should emit `Anchor::Target*` so the
/// positioning naturally glues to the object clicked.
#[derive(Clone, Debug, PartialEq)]
pub enum Anchor {
    /// Anchored to a graph node; resolver looks up the node's current
    /// world position and projects to screen via the active camera.
    TargetNode(NodeKey),
    /// Anchored to a frame by name; resolver looks up the frame's
    /// centroid.
    TargetFrame(String),
    /// Pinned to a screen-space point (free-space right-click, tab
    /// chrome right-click — cases with no canvas target).
    ViewportPoint { x: f32, y: f32 },
    /// Centered in the active viewport (global palette, modal).
    ScreenCenter,
}

impl Anchor {
    /// Build a `ViewportPoint` anchor from an `egui` pointer position.
    pub fn viewport_point(pos: [f32; 2]) -> Self {
        Self::ViewportPoint {
            x: pos[0],
            y: pos[1],
        }
    }

    /// Screen-space point if this anchor already resolves without
    /// needing graph/camera context. `None` for target-anchored
    /// variants (callers must resolve against the live camera).
    pub fn resolved_screen_point(&self) -> Option<[f32; 2]> {
        match self {
            Self::ViewportPoint { x, y } => Some([*x, *y]),
            Self::TargetNode(_) | Self::TargetFrame(_) | Self::ScreenCenter => None,
        }
    }
}

/// The consolidated action-surface state. Mutual exclusion between
/// the global palette, the contextual palette, and the radial menu
/// is enforced by the type (only one variant can be active at a
/// time), replacing the prior four-bool soup in `ChromeUiState`.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum ActionSurfaceState {
    #[default]
    Closed,
    PaletteGlobal,
    PaletteContextual {
        scope: ActionScope,
        anchor: Anchor,
    },
    Radial {
        scope: ActionScope,
        anchor: Anchor,
    },
}

impl ActionSurfaceState {
    pub fn is_open(&self) -> bool {
        !matches!(self, Self::Closed)
    }

    pub fn is_palette_global(&self) -> bool {
        matches!(self, Self::PaletteGlobal)
    }

    pub fn is_palette_contextual(&self) -> bool {
        matches!(self, Self::PaletteContextual { .. })
    }

    pub fn is_any_palette(&self) -> bool {
        self.is_palette_global() || self.is_palette_contextual()
    }

    pub fn is_radial(&self) -> bool {
        matches!(self, Self::Radial { .. })
    }

    pub fn scope(&self) -> Option<&ActionScope> {
        match self {
            Self::Closed | Self::PaletteGlobal => None,
            Self::PaletteContextual { scope, .. } | Self::Radial { scope, .. } => Some(scope),
        }
    }

    pub fn anchor(&self) -> Option<&Anchor> {
        match self {
            Self::Closed | Self::PaletteGlobal => None,
            Self::PaletteContextual { anchor, .. } | Self::Radial { anchor, .. } => Some(anchor),
        }
    }

    /// True when the surface's scope targets a specific node that has
    /// just been removed. The node-deletion hook uses this to close
    /// dangling surfaces.
    pub fn targets_node(&self, removed: NodeKey) -> bool {
        match self.scope() {
            Some(ActionScope::Graph {
                target: ScopeTarget::Node(n),
                ..
            }) => *n == removed,
            _ => false,
        }
    }

    /// True when the surface's scope depends on graph state that a
    /// `clear_graph` just invalidated.
    pub fn is_graph_scoped(&self) -> bool {
        matches!(self.scope(), Some(ActionScope::Graph { .. }))
    }

    /// True when the surface's scope is for a graph view other than
    /// `current`. The focus-change hook uses this to close surfaces
    /// left over on a view the user just navigated away from.
    ///
    /// Returns `false` for `PaletteGlobal` (survives view changes) and
    /// for `Workbench` scope (its lifecycle is tied to the pane, not
    /// the active view).
    pub fn is_in_other_view(&self, current: GraphViewId) -> bool {
        match self.scope() {
            Some(ActionScope::Graph { view_id, .. }) => *view_id != current,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::NodeKey;

    fn node(a: usize) -> NodeKey {
        NodeKey::new(a)
    }

    fn contextual_on_node(view_id: GraphViewId, target: NodeKey) -> ActionSurfaceState {
        ActionSurfaceState::PaletteContextual {
            scope: ActionScope::Graph {
                view_id,
                target: ScopeTarget::Node(target),
            },
            anchor: Anchor::TargetNode(target),
        }
    }

    fn contextual_on_frame(view_id: GraphViewId, frame: &str) -> ActionSurfaceState {
        ActionSurfaceState::PaletteContextual {
            scope: ActionScope::Graph {
                view_id,
                target: ScopeTarget::Frame(frame.to_string()),
            },
            anchor: Anchor::TargetFrame(frame.to_string()),
        }
    }

    fn radial_on_node(view_id: GraphViewId, target: NodeKey) -> ActionSurfaceState {
        ActionSurfaceState::Radial {
            scope: ActionScope::Graph {
                view_id,
                target: ScopeTarget::Node(target),
            },
            anchor: Anchor::TargetNode(target),
        }
    }

    #[test]
    fn default_is_closed() {
        assert_eq!(ActionSurfaceState::default(), ActionSurfaceState::Closed);
        assert!(!ActionSurfaceState::default().is_open());
    }

    #[test]
    fn palette_global_has_no_scope_or_anchor() {
        let s = ActionSurfaceState::PaletteGlobal;
        assert!(s.is_open());
        assert!(s.is_palette_global());
        assert!(s.is_any_palette());
        assert!(s.scope().is_none());
        assert!(s.anchor().is_none());
    }

    #[test]
    fn contextual_on_node_reports_scope_and_anchor() {
        let s = contextual_on_node(GraphViewId::new(), node(42));
        assert!(s.is_palette_contextual());
        assert!(s.is_any_palette());
        assert!(!s.is_radial());
        assert_eq!(s.anchor(), Some(&Anchor::TargetNode(node(42))));
        assert!(matches!(
            s.scope(),
            Some(ActionScope::Graph {
                target: ScopeTarget::Node(n),
                ..
            }) if *n == node(42)
        ));
    }

    #[test]
    fn node_deletion_closes_matching_scope_only() {
        let v = GraphViewId::new();
        let s = contextual_on_node(v, node(42));
        assert!(s.targets_node(node(42)));
        assert!(!s.targets_node(node(99)));

        let frame_s = contextual_on_frame(v, "alpha");
        assert!(!frame_s.targets_node(node(42)));

        let global = ActionSurfaceState::PaletteGlobal;
        assert!(!global.targets_node(node(42)));
    }

    #[test]
    fn graph_clear_closes_all_graph_scoped_surfaces() {
        let v = GraphViewId::new();
        assert!(contextual_on_node(v, node(1)).is_graph_scoped());
        assert!(contextual_on_frame(v, "alpha").is_graph_scoped());
        assert!(radial_on_node(v, node(1)).is_graph_scoped());
        assert!(!ActionSurfaceState::PaletteGlobal.is_graph_scoped());
        assert!(!ActionSurfaceState::Closed.is_graph_scoped());
    }

    #[test]
    fn focus_change_closes_surfaces_scoped_to_other_views() {
        let v1 = GraphViewId::new();
        let v2 = GraphViewId::new();
        let s = contextual_on_node(v1, node(42));
        assert!(s.is_in_other_view(v2));
        assert!(!s.is_in_other_view(v1));

        assert!(!ActionSurfaceState::PaletteGlobal.is_in_other_view(v2));
    }

    #[test]
    fn radial_and_contextual_cannot_be_open_simultaneously() {
        let v = GraphViewId::new();
        let a = radial_on_node(v, node(1));
        assert!(a.is_radial());
        assert!(!a.is_any_palette());

        let b = contextual_on_node(v, node(1));
        assert!(b.is_any_palette());
        assert!(!b.is_radial());
    }

    #[test]
    fn anchor_viewport_point_resolves_without_camera() {
        let a = Anchor::viewport_point([12.0, 34.0]);
        assert_eq!(a.resolved_screen_point(), Some([12.0, 34.0]));

        let t = Anchor::TargetNode(node(1));
        assert!(t.resolved_screen_point().is_none());
    }
}
