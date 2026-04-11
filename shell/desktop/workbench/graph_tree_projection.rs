/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Navigator projection bridge: maps between `ProjectionLens` and
//! `WorkbenchNavigatorSection`.
//!
//! The Navigator's sections (Workbench, Folders, Domain, Recent, etc.)
//! correspond to GraphTree's `ProjectionLens` variants. This module
//! provides the mapping layer so the navigator can be driven by the
//! GraphTree's lens system instead of ad hoc section computation.
//!
//! During migration, both paths coexist. The existing `navigator_groups()`
//! function in `workbench_host.rs` produces sections from graph_app state.
//! This module produces equivalent sections from GraphTree visible_rows().

use graph_tree::{GraphTree, Lifecycle, ProjectionLens};

use crate::graph::NodeKey;
use crate::shell::desktop::ui::workbench_host::WorkbenchNavigatorSection;

/// Map a `ProjectionLens` to the navigator sections it drives.
///
/// A lens may produce multiple sections (e.g., Containment produces
/// both Folders and Domain groups).
pub(crate) fn sections_for_lens(lens: &ProjectionLens) -> Vec<WorkbenchNavigatorSection> {
    match lens {
        ProjectionLens::Traversal => vec![WorkbenchNavigatorSection::Workbench],
        ProjectionLens::Arrangement => vec![WorkbenchNavigatorSection::Workbench],
        ProjectionLens::Containment => vec![
            WorkbenchNavigatorSection::Folders,
            WorkbenchNavigatorSection::Domain,
        ],
        ProjectionLens::Semantic => vec![WorkbenchNavigatorSection::Unrelated],
        ProjectionLens::Recency => vec![WorkbenchNavigatorSection::Recent],
        ProjectionLens::All => vec![
            WorkbenchNavigatorSection::Workbench,
            WorkbenchNavigatorSection::Folders,
            WorkbenchNavigatorSection::Domain,
            WorkbenchNavigatorSection::Recent,
        ],
    }
}

/// Map a `WorkbenchNavigatorSection` to the most appropriate `ProjectionLens`.
pub(crate) fn lens_for_section(section: WorkbenchNavigatorSection) -> ProjectionLens {
    match section {
        WorkbenchNavigatorSection::Workbench => ProjectionLens::Arrangement,
        WorkbenchNavigatorSection::Folders => ProjectionLens::Containment,
        WorkbenchNavigatorSection::Domain => ProjectionLens::Containment,
        WorkbenchNavigatorSection::Recent => ProjectionLens::Recency,
        WorkbenchNavigatorSection::Unrelated => ProjectionLens::Semantic,
        WorkbenchNavigatorSection::Imported => ProjectionLens::All,
    }
}

/// Build navigator member entries from GraphTree visible rows.
///
/// This produces a flat list of members as they appear in the current
/// projection lens, suitable for rendering in the navigator sidebar.
/// Depth information is preserved for indented tree-style rendering.
pub(crate) fn navigator_members_from_tree(
    graph_tree: &GraphTree<NodeKey>,
    label_fn: &dyn Fn(NodeKey) -> String,
) -> Vec<NavigatorMemberEntry> {
    let active = graph_tree.active().cloned();
    let rows = graph_tree.visible_rows();

    rows.iter()
        .map(|row| {
            let lifecycle = graph_tree
                .get(&row.member)
                .map(|e| e.lifecycle)
                .unwrap_or(Lifecycle::Cold);

            NavigatorMemberEntry {
                node_key: row.member.clone(),
                title: label_fn(row.member.clone()),
                is_selected: active.as_ref() == Some(&row.member),
                is_cold: lifecycle == Lifecycle::Cold,
                depth: row.depth,
                is_expanded: row.is_expanded,
                has_children: row.has_children,
            }
        })
        .collect()
}

/// A navigator member entry derived from GraphTree state.
///
/// Parallel to `WorkbenchNavigatorMember` but includes tree-style
/// depth and expansion info for hierarchical rendering.
#[derive(Clone, Debug)]
pub(crate) struct NavigatorMemberEntry {
    pub(crate) node_key: NodeKey,
    pub(crate) title: String,
    pub(crate) is_selected: bool,
    pub(crate) is_cold: bool,
    pub(crate) depth: usize,
    pub(crate) is_expanded: bool,
    pub(crate) has_children: bool,
}
