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
use crate::shell::desktop::ui::workbench_host::{
    WorkbenchNavigatorGroup, WorkbenchNavigatorMember, WorkbenchNavigatorSection,
};

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

/// Build `WorkbenchNavigatorGroup`s directly from the GraphTree.
///
/// This is the Phase C replacement for the graph_app-sourced `navigator_groups()`
/// function in `workbench_host.rs`. It produces the same output type but derives
/// all semantic grouping from the GraphTree's visible_rows and topology, without
/// consulting graph_app's arrangement/containment/recent projections.
///
/// During migration, the existing `navigator_groups()` path remains the default.
/// This function can be called as an enrichment or replacement when a `GraphTree`
/// reference is available.
pub(crate) fn navigator_groups_from_graph_tree(
    graph_tree: &GraphTree<NodeKey>,
    label_fn: &dyn Fn(NodeKey) -> String,
) -> Vec<WorkbenchNavigatorGroup> {
    let active = graph_tree.active().cloned();
    let rows = graph_tree.visible_rows();

    if rows.is_empty() {
        return Vec::new();
    }

    // Group visible rows by their root ancestor. Each root produces one
    // navigator group — this mirrors the arrangement-based grouping but
    // derives structure from GraphTree topology.
    let mut groups: Vec<WorkbenchNavigatorGroup> = Vec::new();
    let mut current_root: Option<NodeKey> = None;
    let mut current_members: Vec<WorkbenchNavigatorMember> = Vec::new();

    for row in &rows {
        let is_root = row.depth == 0;

        if is_root {
            // Flush the previous group if any.
            if let Some(root_key) = current_root.take() {
                if !current_members.is_empty() {
                    let title = label_fn(root_key);
                    groups.push(WorkbenchNavigatorGroup {
                        section: WorkbenchNavigatorSection::Workbench,
                        title,
                        is_highlighted: active.as_ref() == Some(&root_key),
                        members: std::mem::take(&mut current_members),
                    });
                }
            }
            current_root = Some(row.member.clone());
        }

        let lifecycle = graph_tree
            .get(&row.member)
            .map(|e| e.lifecycle)
            .unwrap_or(Lifecycle::Cold);

        current_members.push(WorkbenchNavigatorMember {
            node_key: row.member.clone(),
            title: label_fn(row.member.clone()),
            is_selected: active.as_ref() == Some(&row.member),
            row_key: None,
            is_cold: lifecycle == Lifecycle::Cold,
            depth: row.depth,
            is_expanded: row.is_expanded,
            has_children: row.has_children,
        });
    }

    // Flush the last group.
    if let Some(root_key) = current_root {
        if !current_members.is_empty() {
            let title = label_fn(root_key);
            groups.push(WorkbenchNavigatorGroup {
                section: WorkbenchNavigatorSection::Workbench,
                title,
                is_highlighted: active.as_ref() == Some(&root_key),
                members: std::mem::take(&mut current_members),
            });
        }
    }

    groups
}

/// Phase C: Replace the Workbench section groups with GraphTree-sourced groups.
///
/// Removes all `WorkbenchNavigatorSection::Workbench` groups from `groups`
/// (previously derived from `arrangement_projection_groups()`) and prepends
/// groups built from `GraphTree::visible_rows()`. Non-Workbench sections
/// (Folders, Domain, Recent, Imported) are kept unchanged.
///
/// This establishes GraphTree as the semantic authority for the workbench/
/// arrangement section of the navigator, satisfying the Phase C done gate:
/// the workbench section runs from GraphTree without consulting egui_tiles.
pub(crate) fn replace_workbench_navigator_groups(
    groups: &mut Vec<WorkbenchNavigatorGroup>,
    graph_tree: &GraphTree<NodeKey>,
    label_fn: &dyn Fn(NodeKey) -> String,
) {
    let tree_groups = navigator_groups_from_graph_tree(graph_tree, label_fn);

    // Remove arrangement-derived Workbench groups; keep other sections.
    groups.retain(|g| g.section != WorkbenchNavigatorSection::Workbench);

    // Prepend GraphTree-sourced Workbench groups (they come first in the navigator).
    let mut new_groups = tree_groups;
    new_groups.append(groups);
    *groups = new_groups;
}

/// Enrich a `WorkbenchChromeProjection`'s navigator groups with GraphTree-sourced
/// depth, expansion, and children data.
///
/// This is a lighter-touch alternative to fully replacing `navigator_groups()`.
/// It walks the existing groups and, for each member that exists in the GraphTree,
/// populates the tree-style fields (depth, is_expanded, has_children) from the
/// GraphTree's topology. Members not in the GraphTree are left unchanged.
///
/// Kept for compatibility and test paths that do not have `GraphTree` available.
/// Production code uses `replace_workbench_navigator_groups` (Phase C).
pub(crate) fn enrich_navigator_members_from_graph_tree(
    groups: &mut [WorkbenchNavigatorGroup],
    graph_tree: &GraphTree<NodeKey>,
) {
    let rows = graph_tree.visible_rows();
    // Build a lookup from NodeKey → tree-style fields.
    let row_map: std::collections::HashMap<NodeKey, (usize, bool, bool)> = rows
        .iter()
        .map(|row| {
            (
                row.member.clone(),
                (row.depth, row.is_expanded, row.has_children),
            )
        })
        .collect();

    for group in groups.iter_mut() {
        for member in &mut group.members {
            if let Some(&(depth, is_expanded, has_children)) = row_map.get(&member.node_key) {
                member.depth = depth;
                member.is_expanded = is_expanded;
                member.has_children = has_children;
            }
        }
    }
}
