/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Navigator context projection — the per-frame read-only struct that Navigator
//! produces and Shell reads for omnibar display mode.
//!
//! Navigator owns the content; Shell owns the rendering context and widget.
//! The seam is this struct. See `shell_composition_model_spec.md §5.1`.

use crate::app::{GraphBrowserApp, GraphViewId};
use crate::graph::NodeKey;

/// Produced by Navigator each frame. Read by Shell for omnibar display mode.
///
/// Navigator owns the content (breadcrumb, graphlet label, scope badge).
/// Shell owns the omnibar widget frame, input handling, and rendering context.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct NavigatorContextProjection {
    /// Stable breadcrumb path: active scope root + containment ancestry if present.
    /// Uses containment ancestry, NOT shortest path (per unified_view_model §15).
    /// None if no meaningful graph context is active.
    pub(crate) breadcrumb: Option<BreadcrumbPath>,

    /// Active graphlet label if a named or pinned graphlet is active.
    pub(crate) graphlet_label: Option<String>,

    /// Compact scope badge text for input mode (one word or short phrase).
    /// Shown even when omnibar is in input mode.
    pub(crate) scope_badge: Option<String>,

    /// The active graph view ID and its display name, for the view tab strip.
    /// Navigator owns the tab display; Shell renders it from this projection.
    pub(crate) active_view: Option<(GraphViewId, String)>,

    /// All available graph views when more than one exists — drives the tab strip.
    /// Sorted for stable display order. Empty when only one view exists.
    pub(crate) extra_views: Vec<(GraphViewId, String)>,
}

/// Ordered breadcrumb path tokens from scope root to active address.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct BreadcrumbPath {
    /// Ordered tokens: [scope_root?, containment_ancestors*, active_node_address]
    pub(crate) tokens: Vec<BreadcrumbToken>,
}

/// A single token in the breadcrumb path.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct BreadcrumbToken {
    pub(crate) label: String,
    /// None for scope roots that have no backing graph node.
    pub(crate) node_key: Option<NodeKey>,
}

/// Compute the `NavigatorContextProjection` for the current frame.
///
/// Called once per frame before rendering the CommandBar. Shell reads the
/// result; Navigator's logic runs here but does not own the render widget.
pub(crate) fn compute_navigator_context(graph_app: &GraphBrowserApp) -> NavigatorContextProjection {
    let runtime = &graph_app.workspace.graph_runtime;

    // Collect all views, sorted for stable tab order.
    let mut all_views: Vec<(GraphViewId, String)> = runtime
        .views
        .iter()
        .map(|(id, view)| {
            let name = view.name.trim().to_string();
            let label = if name.is_empty() {
                "Graph".to_string()
            } else {
                name
            };
            (*id, label)
        })
        .collect();
    all_views.sort_by_key(|(id, _)| id.as_uuid());

    // Active view: focused_view if set, else the sole view, else None.
    let focused_id = runtime.focused_view.or_else(|| {
        (all_views.len() == 1)
            .then(|| all_views.first().map(|(id, _)| *id))
            .flatten()
    });
    let active_view = focused_id.and_then(|fid| {
        all_views
            .iter()
            .find(|(id, _)| *id == fid)
            .map(|(id, label)| (*id, label.clone()))
    });

    // Extra views: all views except the active one (only populated when > 1 view).
    let extra_views = if all_views.len() > 1 {
        all_views
            .iter()
            .filter(|(id, _)| Some(*id) != focused_id)
            .cloned()
            .collect()
    } else {
        Vec::new()
    };

    // Scope badge: name of active view (compact, for input mode beside the text field).
    let scope_badge = active_view
        .as_ref()
        .map(|(_, label)| label.clone())
        .filter(|s| s != "Graph");

    // Breadcrumb: for now, derive from the focused primary selected node if any.
    // A full containment-ancestry traversal is the Phase 3+ target; for now we
    // emit a single-token breadcrumb for the focused node so the seam is live.
    let breadcrumb = graph_app
        .focused_selection()
        .primary()
        .and_then(|key| graph_app.domain_graph().get_node(key).map(|n| (key, n)))
        .map(|(key, node)| {
            let label = graph_app
                .user_visible_node_title(key)
                .filter(|label| !label.trim().is_empty())
                .unwrap_or_else(|| {
                    if node.url().is_empty() {
                        "node".to_string()
                    } else {
                        node.url().to_string()
                    }
                });
            BreadcrumbPath {
                tokens: vec![BreadcrumbToken {
                    label,
                    node_key: Some(key),
                }],
            }
        });

    NavigatorContextProjection {
        breadcrumb,
        graphlet_label: None,
        scope_badge,
        active_view,
        extra_views,
    }
}
