/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

use egui_tiles::{Container, LinearDir, Tile, TileId, Tiles, Tree};
use log::{debug, warn};
use servo::{OffscreenRenderingContext, WebViewId};
use uuid::Uuid;

use crate::app::GraphViewId;
use crate::app::{GraphBrowserApp, GraphIntent, WorkbenchProfile};
use crate::graph::{DominantEdge, FrameLayoutHint, NodeKey, SplitOrientation};
use crate::shell::desktop::host::window::EmbedderWindow;
use crate::shell::desktop::lifecycle::webview_controller;
use crate::shell::desktop::workbench::tile_kind::TileKind;
use crate::shell::desktop::workbench::tile_runtime;
use crate::util::VersoAddress;

/// Persisted pane identifier used inside frame bundle schema.
///
/// Distinct from runtime `pane_model::PaneId` (UUID-backed) and scoped only to
/// a single serialized layout tree. Persistence snapshots are derived from the
/// live runtime `Tree<TileKind>`; they are not a second canonical workbench
/// tree that runtime state should follow.
pub(crate) type PersistedPaneId = u64;

/// Backward-compatible local alias retained while migrating callsites.
pub(crate) type PaneId = PersistedPaneId;
type RuntimePaneId = crate::shell::desktop::workbench::pane_model::PaneId;

const FRAME_TAB_SEMANTICS_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum PersistedPaneTile {
    Graph,
    Pane(PaneId),
    /// Legacy read-compat for historical frame layouts that persisted a
    /// diagnostics pane directly in layout tiles.
    ///
    /// This variant is deserialize-only compatibility and is never written by
    /// current bundle serialization. Runtime restore maps it through the
    /// generic `Tool { kind }` pane path.
    #[serde(rename = "Diagnostic")]
    LegacyDiagnostic,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct FrameLayout {
    pub tree: Tree<PersistedPaneTile>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum PaneContent {
    Graph,
    /// Node viewer pane bound to a graph node (viewer backend is resolved at
    /// runtime by `ViewerRegistry`). Serde alias preserves backward-compat
    /// deserialization of frame snapshots saved before the `WebViewNode` → `NodePane`
    /// terminology rename.
    #[serde(alias = "WebViewNode")]
    NodePane {
        node_uuid: Uuid,
    },
    /// Tool pane (diagnostics, history manager, settings, etc.).
    Tool {
        kind: crate::shell::desktop::workbench::pane_model::ToolPaneState,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct FrameManifest {
    pub panes: BTreeMap<PaneId, PaneContent>,
    pub member_node_uuids: BTreeSet<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct FrameTabSemantics {
    pub version: u32,
    pub tab_groups: Vec<TabGroupMetadata>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct TabGroupMetadata {
    pub group_id: Uuid,
    pub pane_ids: Vec<PaneId>,
    pub active_pane_id: Option<PaneId>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct FrameMetadata {
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    pub last_activated_at_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct PersistedFrame {
    pub version: u32,
    pub name: String,
    pub layout: FrameLayout,
    pub manifest: FrameManifest,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame_tab_semantics: Option<FrameTabSemantics>,
    pub metadata: FrameMetadata,
    #[serde(default)]
    pub workbench_profile: WorkbenchProfile,
}

pub(crate) type WorkspaceLayout = FrameLayout;
pub(crate) type WorkspaceManifest = FrameManifest;
pub(crate) type WorkspaceMetadata = FrameMetadata;
pub(crate) type PersistedWorkspace = PersistedFrame;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FrameTabSemanticsRepair {
    RepairedManifestMembership,
    UpgradedVersion {
        from: u32,
        to: u32,
    },
    RegeneratedDuplicateGroupId {
        prior_group_id: Uuid,
        replacement_group_id: Uuid,
    },
    DroppedMissingPaneId {
        group_id: Uuid,
        pane_id: PaneId,
    },
    DroppedDuplicatePaneWithinGroup {
        group_id: Uuid,
        pane_id: PaneId,
    },
    RemovedLaterDuplicateMembership {
        group_id: Uuid,
        pane_id: PaneId,
    },
    RemovedEmptyGroup {
        group_id: Uuid,
    },
    ResetInvalidActivePane {
        group_id: Uuid,
        pane_id: PaneId,
    },
    RemovedEmptyOverlay,
}

impl FrameTabSemanticsRepair {
    pub(crate) fn describe(&self) -> String {
        match self {
            Self::RepairedManifestMembership => {
                "repaired manifest membership before semantic tab repair".to_string()
            }
            Self::UpgradedVersion { from, to } => {
                format!("upgraded FrameTabSemantics version {from} -> {to}")
            }
            Self::RegeneratedDuplicateGroupId {
                prior_group_id,
                replacement_group_id,
            } => {
                format!(
                    "regenerated duplicate tab group id {prior_group_id} as {replacement_group_id}"
                )
            }
            Self::DroppedMissingPaneId { group_id, pane_id } => {
                format!("dropped missing pane id {pane_id} from tab group {group_id}")
            }
            Self::DroppedDuplicatePaneWithinGroup { group_id, pane_id } => {
                format!("dropped duplicate pane id {pane_id} within tab group {group_id}")
            }
            Self::RemovedLaterDuplicateMembership { group_id, pane_id } => {
                format!(
                    "removed pane id {pane_id} from later duplicate membership in tab group {group_id}"
                )
            }
            Self::RemovedEmptyGroup { group_id } => {
                format!("removed empty tab group {group_id} after pane repair")
            }
            Self::ResetInvalidActivePane { group_id, pane_id } => {
                format!("reset invalid active pane {pane_id} for tab group {group_id}")
            }
            Self::RemovedEmptyOverlay => "removed empty FrameTabSemantics overlay".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FrameTabSemanticsRepairReport {
    pub frame_name: String,
    pub repairs: Vec<FrameTabSemanticsRepair>,
    pub user_warning: Option<String>,
}

impl FrameTabSemanticsRepairReport {
    fn new(frame_name: impl Into<String>, repairs: Vec<FrameTabSemanticsRepair>) -> Self {
        let frame_name = frame_name.into();
        let user_warning = (!repairs.is_empty()).then(|| {
            let details = repairs
                .iter()
                .map(FrameTabSemanticsRepair::describe)
                .collect::<Vec<_>>()
                .join("; ");
            format!("Frame '{frame_name}': repaired semantic tab metadata: {details}")
        });
        Self {
            frame_name,
            repairs,
            user_warning,
        }
    }

    pub(crate) fn has_repairs(&self) -> bool {
        !self.repairs.is_empty()
    }
}

pub(crate) fn log_frame_tab_semantics_repair_report(
    report: &FrameTabSemanticsRepairReport,
) {
    for repair in &report.repairs {
        debug!("frame '{}': {}", report.frame_name, repair.describe());
    }
    if let Some(warning) = &report.user_warning {
        warn!("{warning}");
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FrameBundleError {
    MissingManifestPane {
        pane_id: PaneId,
    },
    MembershipMismatch {
        declared: BTreeSet<Uuid>,
        derived: BTreeSet<Uuid>,
    },
}

impl std::fmt::Display for FrameBundleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingManifestPane { pane_id } => {
                write!(
                    f,
                    "frame layout references missing manifest pane id {pane_id}"
                )
            }
            Self::MembershipMismatch { .. } => {
                write!(
                    f,
                    "frame manifest declared membership does not match pane-derived membership"
                )
            }
        }
    }
}

impl std::error::Error for FrameBundleError {}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn persisted_layout_referenced_pane_ids(layout: &FrameLayout) -> BTreeSet<PaneId> {
    layout
        .tree
        .tiles
        .iter()
        .filter_map(|(_, tile)| match tile {
            Tile::Pane(PersistedPaneTile::Pane(id)) => Some(*id),
            _ => None,
        })
        .collect()
}

pub(crate) fn derive_membership_from_manifest(manifest: &FrameManifest) -> BTreeSet<Uuid> {
    manifest
        .panes
        .values()
        .filter_map(|pane| match pane {
            PaneContent::NodePane { node_uuid } => Some(*node_uuid),
            PaneContent::Graph => None,
            PaneContent::Tool { .. } => None,
        })
        .collect()
}

fn visit_layout_tiles_preorder(
    tree: &Tree<PersistedPaneTile>,
    tile_id: TileId,
    out: &mut Vec<TileId>,
) {
    let Some(tile) = tree.tiles.get(tile_id) else {
        return;
    };

    match tile {
        Tile::Pane(_) => {}
        Tile::Container(Container::Tabs(tabs)) => {
            out.push(tile_id);
            for child in &tabs.children {
                visit_layout_tiles_preorder(tree, *child, out);
            }
        }
        Tile::Container(Container::Linear(linear)) => {
            for child in &linear.children {
                visit_layout_tiles_preorder(tree, *child, out);
            }
        }
        Tile::Container(Container::Grid(grid)) => {
            for child in grid.children() {
                visit_layout_tiles_preorder(tree, *child, out);
            }
        }
    }
}

fn persisted_pane_id_for_tile(tree: &Tree<PersistedPaneTile>, tile_id: TileId) -> Option<PaneId> {
    match tree.tiles.get(tile_id) {
        Some(Tile::Pane(PersistedPaneTile::Pane(pane_id))) => Some(*pane_id),
        _ => None,
    }
}

fn derived_tab_group_metadata_from_layout(tree: &Tree<PersistedPaneTile>) -> Vec<TabGroupMetadata> {
    let mut tabs_container_ids = Vec::new();
    if let Some(root) = tree.root() {
        visit_layout_tiles_preorder(tree, root, &mut tabs_container_ids);
    }

    tabs_container_ids
        .into_iter()
        .filter_map(|tabs_tile_id| {
            let Tile::Container(Container::Tabs(tabs)) = tree.tiles.get(tabs_tile_id)? else {
                return None;
            };

            let pane_ids: Vec<_> = tabs
                .children
                .iter()
                .filter_map(|child| persisted_pane_id_for_tile(tree, *child))
                .collect();
            if pane_ids.is_empty() {
                return None;
            }

            let active_pane_id = tabs
                .active
                .and_then(|active_tile| persisted_pane_id_for_tile(tree, active_tile));

            Some(TabGroupMetadata {
                group_id: Uuid::new_v4(),
                pane_ids,
                active_pane_id,
            })
        })
        .collect()
}

pub(crate) fn derive_frame_tab_semantics_from_layout(
    layout: &FrameLayout,
) -> Option<FrameTabSemantics> {
    let tab_groups = derived_tab_group_metadata_from_layout(&layout.tree);
    if tab_groups.is_empty() {
        None
    } else {
        Some(FrameTabSemantics {
            version: FRAME_TAB_SEMANTICS_VERSION,
            tab_groups,
        })
    }
}

fn runtime_pane_id_for_tile(tree: &Tree<TileKind>, tile_id: TileId) -> Option<RuntimePaneId> {
    match tree.tiles.get(tile_id) {
        Some(Tile::Pane(tile)) => Some(tile.pane_id()),
        _ => None,
    }
}

fn visit_runtime_tiles_preorder(tree: &Tree<TileKind>, tile_id: TileId, out: &mut Vec<TileId>) {
    let Some(tile) = tree.tiles.get(tile_id) else {
        return;
    };

    match tile {
        Tile::Pane(_) => {}
        Tile::Container(Container::Tabs(tabs)) => {
            out.push(tile_id);
            for child in &tabs.children {
                visit_runtime_tiles_preorder(tree, *child, out);
            }
        }
        Tile::Container(Container::Linear(linear)) => {
            for child in &linear.children {
                visit_runtime_tiles_preorder(tree, *child, out);
            }
        }
        Tile::Container(Container::Grid(grid)) => {
            for child in grid.children() {
                visit_runtime_tiles_preorder(tree, *child, out);
            }
        }
    }
}

fn derived_runtime_tab_group_metadata_from_tree(
    tree: &Tree<TileKind>,
) -> Vec<crate::app::RuntimeTabGroupMetadata> {
    let mut tabs_container_ids = Vec::new();
    if let Some(root) = tree.root() {
        visit_runtime_tiles_preorder(tree, root, &mut tabs_container_ids);
    }

    tabs_container_ids
        .into_iter()
        .filter_map(|tabs_tile_id| {
            let Tile::Container(Container::Tabs(tabs)) = tree.tiles.get(tabs_tile_id)? else {
                return None;
            };

            let pane_ids: Vec<_> = tabs
                .children
                .iter()
                .filter_map(|child| runtime_pane_id_for_tile(tree, *child))
                .collect();
            if pane_ids.is_empty() {
                return None;
            }

            let active_pane_id = tabs
                .active
                .and_then(|active_tile| runtime_pane_id_for_tile(tree, active_tile));

            Some(crate::app::RuntimeTabGroupMetadata {
                group_id: Uuid::new_v4(),
                pane_ids,
                active_pane_id,
            })
        })
        .collect()
}

pub(crate) fn derive_runtime_frame_tab_semantics_from_tree(
    tree: &Tree<TileKind>,
) -> Option<crate::app::RuntimeFrameTabSemantics> {
    let tab_groups = derived_runtime_tab_group_metadata_from_tree(tree);
    if tab_groups.is_empty() {
        None
    } else {
        Some(crate::app::RuntimeFrameTabSemantics {
            version: FRAME_TAB_SEMANTICS_VERSION,
            tab_groups,
        })
    }
}

fn persisted_group_membership_key(pane_ids: &[PaneId]) -> BTreeSet<PaneId> {
    pane_ids.iter().copied().collect()
}

fn project_runtime_frame_tab_semantics_to_persisted(
    semantics: &crate::app::RuntimeFrameTabSemantics,
    pane_id_map: &HashMap<RuntimePaneId, PaneId>,
) -> Option<FrameTabSemantics> {
    let tab_groups: Vec<_> = semantics
        .tab_groups
        .iter()
        .filter_map(|group| {
            let pane_ids: Vec<_> = group
                .pane_ids
                .iter()
                .filter_map(|pane_id| pane_id_map.get(pane_id).copied())
                .collect();
            if pane_ids.is_empty() {
                return None;
            }
            Some(TabGroupMetadata {
                group_id: group.group_id,
                active_pane_id: group
                    .active_pane_id
                    .and_then(|pane_id| pane_id_map.get(&pane_id).copied())
                    .filter(|pane_id| pane_ids.contains(pane_id)),
                pane_ids,
            })
        })
        .collect();

    if tab_groups.is_empty() {
        None
    } else {
        Some(FrameTabSemantics {
            version: FRAME_TAB_SEMANTICS_VERSION,
            tab_groups,
        })
    }
}

fn merge_persisted_frame_tab_semantics(
    projected: Option<FrameTabSemantics>,
    derived: Option<FrameTabSemantics>,
) -> Option<FrameTabSemantics> {
    match (projected, derived) {
        (None, derived) => derived,
        (Some(projected), None) => Some(projected),
        (Some(projected), Some(derived)) => {
            let mut derived_by_membership: HashMap<BTreeSet<PaneId>, TabGroupMetadata> = derived
                .tab_groups
                .into_iter()
                .map(|group| (persisted_group_membership_key(&group.pane_ids), group))
                .collect();
            let mut tab_groups = Vec::new();

            for group in projected.tab_groups {
                let key = persisted_group_membership_key(&group.pane_ids);
                if let Some(derived_group) = derived_by_membership.remove(&key) {
                    tab_groups.push(TabGroupMetadata {
                        group_id: group.group_id,
                        pane_ids: derived_group.pane_ids,
                        active_pane_id: derived_group.active_pane_id,
                    });
                } else {
                    tab_groups.push(group);
                }
            }

            tab_groups.extend(derived_by_membership.into_values());
            if tab_groups.is_empty() {
                None
            } else {
                Some(FrameTabSemantics {
                    version: FRAME_TAB_SEMANTICS_VERSION,
                    tab_groups,
                })
            }
        }
    }
}

fn resolved_frame_tab_semantics_for_bundle(
    graph_app: &GraphBrowserApp,
    derived: Option<FrameTabSemantics>,
    pane_id_map: &HashMap<RuntimePaneId, PaneId>,
) -> Option<FrameTabSemantics> {
    let projected = graph_app
        .current_frame_tab_semantics()
        .and_then(|semantics| {
            project_runtime_frame_tab_semantics_to_persisted(semantics, pane_id_map)
        });
    merge_persisted_frame_tab_semantics(projected, derived)
}

pub(crate) fn semantic_tab_groups_for_frame(bundle: &PersistedWorkspace) -> Vec<TabGroupMetadata> {
    bundle
        .frame_tab_semantics
        .as_ref()
        .map(|semantics| semantics.tab_groups.clone())
        .or_else(|| derive_frame_tab_semantics_from_layout(&bundle.layout).map(|s| s.tab_groups))
        .unwrap_or_default()
}

pub(crate) fn saved_tab_node_keys_for_frame_bundle(
    graph_app: &GraphBrowserApp,
    bundle: &PersistedWorkspace,
) -> HashSet<NodeKey> {
    let pane_ids: BTreeSet<_> = semantic_tab_groups_for_frame(bundle)
        .into_iter()
        .flat_map(|group| group.pane_ids.into_iter())
        .collect();

    pane_ids
        .into_iter()
        .filter_map(|pane_id| match bundle.manifest.panes.get(&pane_id) {
            Some(PaneContent::NodePane { node_uuid }) => graph_app
                .workspace
                .domain
                .graph
                .get_node_key_by_id(*node_uuid),
            _ => None,
        })
        .collect()
}

fn repair_frame_tab_semantics(bundle: &mut PersistedWorkspace) -> Vec<FrameTabSemanticsRepair> {
    let Some(semantics) = bundle.frame_tab_semantics.as_mut() else {
        return Vec::new();
    };

    let mut repairs = Vec::new();
    let valid_pane_ids: BTreeSet<_> = bundle.manifest.panes.keys().copied().collect();
    let mut seen_group_ids = HashSet::new();
    let mut seen_pane_ids = BTreeSet::new();
    let mut repaired_groups = Vec::new();

    if semantics.version != FRAME_TAB_SEMANTICS_VERSION {
        repairs.push(FrameTabSemanticsRepair::UpgradedVersion {
            from: semantics.version,
            to: FRAME_TAB_SEMANTICS_VERSION,
        });
        semantics.version = FRAME_TAB_SEMANTICS_VERSION;
    }

    for group in &semantics.tab_groups {
        let mut repaired_group = group.clone();

        if !seen_group_ids.insert(repaired_group.group_id) {
            let prior_group_id = repaired_group.group_id;
            repaired_group.group_id = Uuid::new_v4();
            repairs.push(FrameTabSemanticsRepair::RegeneratedDuplicateGroupId {
                prior_group_id,
                replacement_group_id: repaired_group.group_id,
            });
        }

        let mut local_seen = BTreeSet::new();
        let mut repaired_pane_ids = Vec::new();
        for pane_id in &repaired_group.pane_ids {
            if !valid_pane_ids.contains(pane_id) {
                repairs.push(FrameTabSemanticsRepair::DroppedMissingPaneId {
                    group_id: repaired_group.group_id,
                    pane_id: *pane_id,
                });
                continue;
            }
            if !local_seen.insert(*pane_id) {
                repairs.push(FrameTabSemanticsRepair::DroppedDuplicatePaneWithinGroup {
                    group_id: repaired_group.group_id,
                    pane_id: *pane_id,
                });
                continue;
            }
            if !seen_pane_ids.insert(*pane_id) {
                repairs.push(FrameTabSemanticsRepair::RemovedLaterDuplicateMembership {
                    group_id: repaired_group.group_id,
                    pane_id: *pane_id,
                });
                continue;
            }
            repaired_pane_ids.push(*pane_id);
        }
        repaired_group.pane_ids = repaired_pane_ids;

        if repaired_group.pane_ids.is_empty() {
            repairs.push(FrameTabSemanticsRepair::RemovedEmptyGroup {
                group_id: repaired_group.group_id,
            });
            continue;
        }

        if let Some(active_pane_id) = repaired_group.active_pane_id
            && !repaired_group.pane_ids.contains(&active_pane_id)
        {
            repairs.push(FrameTabSemanticsRepair::ResetInvalidActivePane {
                group_id: repaired_group.group_id,
                pane_id: active_pane_id,
            });
            repaired_group.active_pane_id = None;
        }

        repaired_groups.push(repaired_group);
    }

    semantics.tab_groups = repaired_groups;
    if semantics.tab_groups.is_empty() {
        bundle.frame_tab_semantics = None;
        repairs.push(FrameTabSemanticsRepair::RemovedEmptyOverlay);
    }

    repairs
}

fn repair_frame_bundle_before_semantic_read(
    bundle: &mut PersistedWorkspace,
) -> Result<Vec<FrameTabSemanticsRepair>, FrameBundleError> {
    let mut repairs = Vec::new();
    if let Err(err) = validate_frame_bundle(bundle) {
        match err {
            FrameBundleError::MembershipMismatch { .. } => {
                repair_manifest_membership(bundle);
                repairs.push(FrameTabSemanticsRepair::RepairedManifestMembership);
            }
            _ => return Err(err),
        }
    }
    repairs.extend(repair_frame_tab_semantics(bundle));
    Ok(repairs)
}

fn runtime_tile_from_manifest_pane(
    graph_app: &GraphBrowserApp,
    pane_content: &PaneContent,
    restored_nodes: &mut Vec<NodeKey>,
) -> Option<TileKind> {
    match pane_content {
        PaneContent::Graph => Some(TileKind::Graph(
            crate::shell::desktop::workbench::pane_model::GraphPaneRef::new(GraphViewId::default()),
        )),
        PaneContent::NodePane { node_uuid } => graph_app
            .workspace
            .domain
            .graph
            .get_node_key_by_id(*node_uuid)
            .map(|node_key| {
                restored_nodes.push(node_key);
                TileKind::Node(node_key.into())
            }),
        PaneContent::Tool { kind } => {
            #[cfg(feature = "diagnostics")]
            {
                Some(TileKind::Tool(
                    crate::shell::desktop::workbench::pane_model::ToolPaneRef::new(kind.clone()),
                ))
            }
            #[cfg(not(feature = "diagnostics"))]
            {
                let _ = kind;
                None
            }
        }
    }
}

fn replace_child_in_parent(
    tree: &mut Tree<TileKind>,
    parent_id: TileId,
    old_child: TileId,
    new_child: TileId,
) {
    let Some(parent_tile) = tree.tiles.get_mut(parent_id) else {
        return;
    };

    match parent_tile {
        Tile::Container(Container::Tabs(tabs)) => {
            let was_active = tabs.active == Some(old_child);
            if let Some(index) = tabs.children.iter().position(|child| *child == old_child) {
                tabs.children[index] = new_child;
                if was_active {
                    tabs.set_active(new_child);
                }
            }
        }
        Tile::Container(Container::Linear(linear)) => {
            if let Some(index) = linear.children.iter().position(|child| *child == old_child) {
                linear.children[index] = new_child;
                linear.shares.replace_with(old_child, new_child);
            }
        }
        Tile::Container(Container::Grid(grid)) => {
            let index = grid
                .children()
                .enumerate()
                .find_map(|(index, child)| (*child == old_child).then_some(index));
            if let Some(index) = index {
                let _ = grid.replace_at(index, new_child);
            }
        }
        Tile::Pane(_) => {}
    }
}

fn tile_is_attached(tree: &Tree<TileKind>, tile_id: TileId) -> bool {
    tree.root() == Some(tile_id) || tree.tiles.parent_of(tile_id).is_some()
}

fn tabs_container_for_exact_members(
    tree: &Tree<TileKind>,
    ordered_tile_ids: &[TileId],
) -> Option<TileId> {
    let first = *ordered_tile_ids.first()?;
    let parent_id = tree.tiles.parent_of(first)?;
    let Tile::Container(Container::Tabs(tabs)) = tree.tiles.get(parent_id)? else {
        return None;
    };

    if ordered_tile_ids
        .iter()
        .all(|tile_id| tree.tiles.parent_of(*tile_id) == Some(parent_id))
        && tabs.children.len() == ordered_tile_ids.len()
        && tabs
            .children
            .iter()
            .all(|child| ordered_tile_ids.contains(child))
    {
        Some(parent_id)
    } else {
        None
    }
}

fn restore_semantic_tab_groups_into_runtime_tree(
    graph_app: &GraphBrowserApp,
    bundle: &PersistedWorkspace,
    runtime_tree: &mut Tree<TileKind>,
    restored_nodes: &mut Vec<NodeKey>,
) -> Vec<String> {
    let Some(semantics) = bundle.frame_tab_semantics.as_ref() else {
        return Vec::new();
    };

    let mut pane_tile_ids: HashMap<PaneId, TileId> = bundle
        .layout
        .tree
        .tiles
        .iter()
        .filter_map(|(tile_id, tile)| match tile {
            Tile::Pane(PersistedPaneTile::Pane(pane_id))
                if runtime_tree.tiles.get(*tile_id).is_some() =>
            {
                Some((*pane_id, *tile_id))
            }
            _ => None,
        })
        .collect();
    let mut repairs = Vec::new();

    for group in &semantics.tab_groups {
        let mut ordered_tile_ids = Vec::new();
        let mut attached_tiles = Vec::new();

        for pane_id in &group.pane_ids {
            let tile_id = if let Some(tile_id) = pane_tile_ids
                .get(pane_id)
                .copied()
                .filter(|tile_id| runtime_tree.tiles.get(*tile_id).is_some())
            {
                tile_id
            } else {
                let Some(pane_content) = bundle.manifest.panes.get(pane_id) else {
                    continue;
                };
                let Some(runtime_tile) =
                    runtime_tile_from_manifest_pane(graph_app, pane_content, restored_nodes)
                else {
                    repairs.push(format!(
                        "skipped semantic tab member {pane_id} in group {} because runtime content could not be realized",
                        group.group_id
                    ));
                    continue;
                };
                let tile_id = runtime_tree.tiles.insert_pane(runtime_tile);
                pane_tile_ids.insert(*pane_id, tile_id);
                tile_id
            };

            ordered_tile_ids.push(tile_id);
            if tile_is_attached(runtime_tree, tile_id) {
                attached_tiles.push(tile_id);
            }
        }

        if ordered_tile_ids.len() <= 1 {
            continue;
        }

        let active_child = group
            .active_pane_id
            .and_then(|pane_id| pane_tile_ids.get(&pane_id).copied())
            .filter(|tile_id| ordered_tile_ids.contains(tile_id));

        if let Some(tabs_id) = tabs_container_for_exact_members(runtime_tree, &ordered_tile_ids) {
            if let Some(Tile::Container(Container::Tabs(tabs))) =
                runtime_tree.tiles.get_mut(tabs_id)
            {
                tabs.children = ordered_tile_ids.clone();
                if let Some(active_child) = active_child {
                    tabs.set_active(active_child);
                }
            }
            continue;
        }

        let mut unique_attached: Vec<_> = attached_tiles
            .into_iter()
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        let anchor_tile = if unique_attached.len() == 1 {
            unique_attached.pop()
        } else if unique_attached.is_empty() && runtime_tree.root().is_none() {
            ordered_tile_ids.first().copied()
        } else {
            repairs.push(format!(
                "skipped semantic tab restore for group {} because {} attached members were present",
                group.group_id,
                unique_attached.len()
            ));
            continue;
        };
        let Some(anchor_tile) = anchor_tile else {
            continue;
        };

        let anchor_parent = runtime_tree.tiles.parent_of(anchor_tile);
        let anchor_was_root = runtime_tree.root() == Some(anchor_tile);
        let restored_group = runtime_tree.tiles.insert_tab_tile(ordered_tile_ids.clone());
        if let Some(Tile::Container(Container::Tabs(tabs))) =
            runtime_tree.tiles.get_mut(restored_group)
        {
            if let Some(active_child) = active_child {
                tabs.set_active(active_child);
            }
        }

        if let Some(parent_id) = anchor_parent {
            replace_child_in_parent(runtime_tree, parent_id, anchor_tile, restored_group);
        } else if anchor_was_root || runtime_tree.root().is_none() {
            runtime_tree.root = Some(restored_group);
        } else {
            repairs.push(format!(
                "skipped semantic tab restore for group {} because anchor tile was detached",
                group.group_id
            ));
            continue;
        }

        repairs.push(format!(
            "restored semantic tab group {} from pane rest with {} panes",
            group.group_id,
            ordered_tile_ids.len()
        ));
    }

    repairs
}

pub(crate) fn validate_frame_bundle(bundle: &PersistedWorkspace) -> Result<(), FrameBundleError> {
    for pane_id in persisted_layout_referenced_pane_ids(&bundle.layout) {
        if !bundle.manifest.panes.contains_key(&pane_id) {
            return Err(FrameBundleError::MissingManifestPane { pane_id });
        }
    }
    let derived = derive_membership_from_manifest(&bundle.manifest);
    if derived != bundle.manifest.member_node_uuids {
        return Err(FrameBundleError::MembershipMismatch {
            declared: bundle.manifest.member_node_uuids.clone(),
            derived,
        });
    }
    Ok(())
}

pub(crate) fn repair_manifest_membership(bundle: &mut PersistedWorkspace) {
    bundle.manifest.member_node_uuids = derive_membership_from_manifest(&bundle.manifest);
}

fn runtime_tree_to_bundle(
    graph_app: &GraphBrowserApp,
    name: &str,
    tree: &Tree<TileKind>,
    prior_metadata: Option<WorkspaceMetadata>,
) -> Result<PersistedWorkspace, String> {
    let mut serde_tree: Tree<serde_json::Value> =
        serde_json::from_value(serde_json::to_value(tree).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;

    let mut panes = BTreeMap::new();
    let mut runtime_to_persisted_pane_ids = HashMap::new();
    for (tile_id, tile) in serde_tree.tiles.iter_mut() {
        let Tile::Pane(pane_value) = tile else {
            continue;
        };
        let runtime_pane: TileKind =
            serde_json::from_value(pane_value.clone()).map_err(|e| e.to_string())?;
        let runtime_pane_id = runtime_pane.pane_id();
        let persisted_pane = match runtime_pane {
            TileKind::Pane(_) => {
                continue;
            }
            TileKind::Graph(_) => {
                let pane_id = tile_id.0;
                panes.insert(pane_id, PaneContent::Graph);
                runtime_to_persisted_pane_ids.insert(runtime_pane_id, pane_id);
                PersistedPaneTile::Pane(pane_id)
            }
            TileKind::Node(state) => {
                let node = graph_app
                    .workspace
                    .domain
                    .graph
                    .get_node(state.node)
                    .ok_or_else(|| {
                        format!(
                            "frame snapshot contains stale node key {}",
                            state.node.index()
                        )
                    })?;
                let pane_id = tile_id.0;
                panes.insert(pane_id, PaneContent::NodePane { node_uuid: node.id });
                runtime_to_persisted_pane_ids.insert(state.pane_id, pane_id);
                PersistedPaneTile::Pane(pane_id)
            }
            #[cfg(feature = "diagnostics")]
            TileKind::Tool(tool_ref) => {
                let pane_id = tile_id.0;
                panes.insert(
                    pane_id,
                    PaneContent::Tool {
                        kind: tool_ref.kind.clone(),
                    },
                );
                runtime_to_persisted_pane_ids.insert(tool_ref.pane_id, pane_id);
                PersistedPaneTile::Pane(pane_id)
            }
        };
        *pane_value = serde_json::to_value(persisted_pane).map_err(|e| e.to_string())?;
    }

    let layout_tree: Tree<PersistedPaneTile> =
        serde_json::from_value(serde_json::to_value(serde_tree).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let mut manifest = FrameManifest {
        panes,
        member_node_uuids: BTreeSet::new(),
    };
    manifest.member_node_uuids = derive_membership_from_manifest(&manifest);
    let derived_frame_tab_semantics = derive_frame_tab_semantics_from_layout(&FrameLayout {
        tree: layout_tree.clone(),
    });
    let frame_tab_semantics = resolved_frame_tab_semantics_for_bundle(
        graph_app,
        derived_frame_tab_semantics,
        &runtime_to_persisted_pane_ids,
    );

    let now = now_unix_ms();
    let metadata = match prior_metadata {
        Some(prior) => FrameMetadata {
            created_at_ms: if prior.created_at_ms == 0 {
                now
            } else {
                prior.created_at_ms
            },
            updated_at_ms: now,
            last_activated_at_ms: prior.last_activated_at_ms,
        },
        None => FrameMetadata {
            created_at_ms: now,
            updated_at_ms: now,
            last_activated_at_ms: None,
        },
    };

    Ok(PersistedWorkspace {
        version: 1,
        name: name.to_string(),
        layout: FrameLayout { tree: layout_tree },
        manifest,
        frame_tab_semantics,
        metadata,
        workbench_profile: graph_app.workbench_profile().clone(),
    })
}

pub(crate) fn serialize_named_frame_bundle(
    graph_app: &GraphBrowserApp,
    name: &str,
    tree: &Tree<TileKind>,
) -> Result<String, String> {
    let prior_metadata = load_named_frame_bundle(graph_app, name)
        .ok()
        .map(|b| b.metadata);
    let bundle = runtime_tree_to_bundle(graph_app, name, tree, prior_metadata)?;
    serde_json::to_string(&bundle).map_err(|e| e.to_string())
}

pub(crate) fn save_named_frame_bundle(
    graph_app: &mut GraphBrowserApp,
    name: &str,
    tree: &Tree<TileKind>,
) -> Result<(), String> {
    let bundle_json = serialize_named_frame_bundle(graph_app, name, tree)?;
    graph_app.save_workspace_layout_json(name, &bundle_json);
    graph_app.sync_named_workbench_frame_graph_representation(name, tree);
    let frame_layout_sync_intents = frame_layout_sync_intents_for_name(graph_app, name, tree);
    if !frame_layout_sync_intents.is_empty() {
        graph_app.apply_reducer_intents(frame_layout_sync_intents);
    }
    let membership_index = build_membership_index_from_layouts(graph_app);
    graph_app.init_membership_index(membership_index);
    crate::shell::desktop::runtime::registries::phase3_publish_workbench_projection_refresh_requested(
        "frame_snapshot_saved",
    );
    Ok(())
}

pub(crate) fn load_named_frame_bundle(
    graph_app: &GraphBrowserApp,
    name: &str,
) -> Result<PersistedWorkspace, String> {
    let json = graph_app
        .load_workspace_layout_json(name)
        .ok_or_else(|| format!("frame snapshot '{name}' not found"))?;
    let mut bundle: PersistedWorkspace = serde_json::from_str(&json).map_err(|e| e.to_string())?;
    let repairs = repair_frame_bundle_before_semantic_read(&mut bundle).map_err(|e| e.to_string())?;
    let report = FrameTabSemanticsRepairReport::new(bundle.name.clone(), repairs);
    log_frame_tab_semantics_repair_report(&report);
    Ok(bundle)
}

pub(crate) fn repair_named_frame_tab_semantics(
    graph_app: &mut GraphBrowserApp,
    name: &str,
) -> Result<FrameTabSemanticsRepairReport, String> {
    let json = graph_app
        .load_workspace_layout_json(name)
        .ok_or_else(|| format!("frame snapshot '{name}' not found"))?;
    let mut bundle: PersistedWorkspace = serde_json::from_str(&json).map_err(|e| e.to_string())?;
    let repairs = repair_frame_bundle_before_semantic_read(&mut bundle).map_err(|e| e.to_string())?;
    let report = FrameTabSemanticsRepairReport::new(bundle.name.clone(), repairs);

    if !report.has_repairs() {
        return Ok(report);
    }

    let bundle_json = serde_json::to_string(&bundle).map_err(|e| e.to_string())?;
    graph_app.save_workspace_layout_json(name, &bundle_json);
    refresh_workbench_projection_from_manifests(graph_app)?;
    crate::shell::desktop::runtime::registries::phase3_publish_workbench_projection_refresh_requested(
        "frame_tab_semantics_repaired",
    );
    Ok(report)
}

pub(crate) fn apply_workbench_profile_from_bundle(
    graph_app: &mut GraphBrowserApp,
    bundle: &PersistedWorkspace,
) {
    graph_app.set_workbench_profile(bundle.workbench_profile.clone());
}

pub(crate) fn runtime_frame_tab_semantics_from_restored_bundle(
    _graph_app: &GraphBrowserApp,
    bundle: &PersistedWorkspace,
    runtime_tree: &Tree<TileKind>,
) -> Option<crate::app::RuntimeFrameTabSemantics> {
    let mut repaired = bundle.clone();
    if repair_frame_bundle_before_semantic_read(&mut repaired).is_err() {
        return derive_runtime_frame_tab_semantics_from_tree(runtime_tree);
    }

    let persisted_semantics = repaired
        .frame_tab_semantics
        .or_else(|| derive_frame_tab_semantics_from_layout(&repaired.layout))?;
    let persisted_to_runtime: HashMap<PaneId, RuntimePaneId> = repaired
        .layout
        .tree
        .tiles
        .iter()
        .filter_map(|(tile_id, tile)| match tile {
            Tile::Pane(PersistedPaneTile::Pane(pane_id)) => runtime_tree
                .tiles
                .get(*tile_id)
                .and_then(|runtime_tile| match runtime_tile {
                    Tile::Pane(tile) => Some((*pane_id, tile.pane_id())),
                    _ => None,
                }),
            _ => None,
        })
        .collect();

    let tab_groups: Vec<_> = persisted_semantics
        .tab_groups
        .into_iter()
        .filter_map(|group| {
            let pane_ids: Vec<_> = group
                .pane_ids
                .into_iter()
                .filter_map(|pane_id| persisted_to_runtime.get(&pane_id).copied())
                .collect();
            if pane_ids.is_empty() {
                return None;
            }
            Some(crate::app::RuntimeTabGroupMetadata {
                group_id: group.group_id,
                active_pane_id: group
                    .active_pane_id
                    .and_then(|pane_id| persisted_to_runtime.get(&pane_id).copied())
                    .filter(|pane_id| pane_ids.contains(pane_id)),
                pane_ids,
            })
        })
        .collect();

    if tab_groups.is_empty() {
        None
    } else {
        Some(crate::app::RuntimeFrameTabSemantics {
            version: persisted_semantics.version,
            tab_groups,
        })
    }
}

pub(crate) fn restore_runtime_tree_from_frame_bundle(
    graph_app: &GraphBrowserApp,
    bundle: &PersistedWorkspace,
) -> Result<(Tree<TileKind>, Vec<NodeKey>), String> {
    let mut repaired = bundle.clone();
    let repairs =
        repair_frame_bundle_before_semantic_read(&mut repaired).map_err(|e| e.to_string())?;
    let report = FrameTabSemanticsRepairReport::new(repaired.name.clone(), repairs);
    log_frame_tab_semantics_repair_report(&report);

    let mut serde_tree: Tree<serde_json::Value> = serde_json::from_value(
        serde_json::to_value(&repaired.layout.tree).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;

    let mut restored_nodes = Vec::new();
    let mut missing_tile_ids = Vec::new();
    for (tile_id, tile) in serde_tree.tiles.iter_mut() {
        let Tile::Pane(pane_value) = tile else {
            continue;
        };
        let persisted_pane: PersistedPaneTile =
            serde_json::from_value(pane_value.clone()).map_err(|e| e.to_string())?;
        let runtime_pane = match persisted_pane {
            PersistedPaneTile::Graph => Some(TileKind::Graph(
                crate::shell::desktop::workbench::pane_model::GraphPaneRef::new(
                    GraphViewId::default(),
                ),
            )),
            PersistedPaneTile::LegacyDiagnostic => {
                #[cfg(feature = "diagnostics")]
                {
                    Some(TileKind::Tool(
                        crate::shell::desktop::workbench::pane_model::ToolPaneRef::new(
                            crate::shell::desktop::workbench::pane_model::ToolPaneState::Diagnostics,
                        ),
                    ))
                }
                #[cfg(not(feature = "diagnostics"))]
                {
                    missing_tile_ids.push(*tile_id);
                    None
                }
            }
            PersistedPaneTile::Pane(pane_id) => match repaired.manifest.panes.get(&pane_id) {
                Some(pane_content) => {
                    match runtime_tile_from_manifest_pane(
                        graph_app,
                        pane_content,
                        &mut restored_nodes,
                    ) {
                        Some(runtime_tile) => Some(runtime_tile),
                        None => {
                            missing_tile_ids.push(*tile_id);
                            None
                        }
                    }
                }
                None => return Err(format!("missing manifest pane id {pane_id}")),
            },
        };

        if let Some(runtime_pane) = runtime_pane {
            *pane_value = serde_json::to_value(runtime_pane).map_err(|e| e.to_string())?;
        }
    }

    let mut runtime_tree: Tree<TileKind> =
        serde_json::from_value(serde_json::to_value(serde_tree).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;

    for tile_id in missing_tile_ids {
        let _ = runtime_tree.remove_recursively(tile_id);
    }
    tile_runtime::prune_stale_node_pane_keys_only(&mut runtime_tree, graph_app);
    for repair in restore_semantic_tab_groups_into_runtime_tree(
        graph_app,
        &repaired,
        &mut runtime_tree,
        &mut restored_nodes,
    ) {
        warn!("frame '{}': {repair}", repaired.name);
    }
    let has_graph_pane = runtime_tree
        .tiles
        .iter()
        .any(|(_, tile)| matches!(tile, Tile::Pane(TileKind::Graph(_))));
    if restored_nodes.is_empty() && !has_graph_pane {
        runtime_tree.root = None;
    }

    Ok((runtime_tree, restored_nodes))
}

fn frame_layout_hints_for_name(graph_app: &GraphBrowserApp, name: &str) -> Vec<FrameLayoutHint> {
    let frame_url = VersoAddress::frame(name.to_string()).to_string();
    graph_app
        .domain_graph()
        .get_node_by_url(&frame_url)
        .and_then(|(frame_key, _)| graph_app.domain_graph().frame_layout_hints(frame_key))
        .map(|hints| hints.to_vec())
        .unwrap_or_default()
}

pub(crate) fn frame_key_for_name(graph_app: &GraphBrowserApp, name: &str) -> Option<NodeKey> {
    let frame_url = VersoAddress::frame(name.to_string()).to_string();
    graph_app
        .domain_graph()
        .get_node_by_url(&frame_url)
        .map(|(frame_key, _)| frame_key)
}

fn ordered_live_frame_member_keys(graph_app: &GraphBrowserApp, name: &str) -> Vec<NodeKey> {
    let mut member_keys = graph_app
        .arrangement_projection_groups()
        .into_iter()
        .find(|group| {
            group.sub_kind == crate::graph::ArrangementSubKind::FrameMember && group.id == name
        })
        .map(|group| group.member_keys)
        .unwrap_or_default();

    let mut seen = HashSet::new();
    member_keys
        .retain(|key| graph_app.domain_graph().get_node(*key).is_some() && seen.insert(*key));
    member_keys
}

fn frame_name_for_key(graph_app: &GraphBrowserApp, frame_key: NodeKey) -> Option<String> {
    let frame_url = graph_app
        .domain_graph()
        .get_node(frame_key)?
        .url()
        .to_string();
    GraphBrowserApp::resolve_frame_route(&frame_url)
}

pub(crate) fn ordered_live_frame_member_keys_for_anchor(
    graph_app: &GraphBrowserApp,
    frame_key: NodeKey,
) -> Vec<NodeKey> {
    frame_name_for_key(graph_app, frame_key)
        .map(|name| ordered_live_frame_member_keys(graph_app, &name))
        .unwrap_or_default()
}

fn resolve_frame_layout_member_key(
    graph_app: &GraphBrowserApp,
    member_id: &str,
    frame_members: &HashSet<NodeKey>,
) -> Option<NodeKey> {
    let member_uuid = Uuid::parse_str(member_id).ok()?;
    let member_key = graph_app.domain_graph().get_node_key_by_id(member_uuid)?;
    frame_members.contains(&member_key).then_some(member_key)
}

fn insert_frame_member_pane(tiles: &mut Tiles<TileKind>, node_key: NodeKey) -> TileId {
    tiles.insert_pane(TileKind::Node(node_key.into()))
}

fn frame_layout_member_id(graph_app: &GraphBrowserApp, node_key: NodeKey) -> Option<String> {
    graph_app
        .domain_graph()
        .get_node(node_key)
        .map(|node| node.id.to_string())
}

fn frame_layout_leaf_node_key(tree: &Tree<TileKind>, tile_id: TileId) -> Option<NodeKey> {
    match tree.tiles.get(tile_id) {
        Some(Tile::Pane(TileKind::Node(state))) => Some(state.node),
        Some(Tile::Pane(TileKind::Pane(
            crate::shell::desktop::workbench::pane_model::PaneViewState::Node(state),
        ))) => Some(state.node),
        _ => None,
    }
}

fn frame_layout_linear_leaf_members(
    tree: &Tree<TileKind>,
    tile_id: TileId,
    dir: LinearDir,
) -> Option<Vec<NodeKey>> {
    let Tile::Container(Container::Linear(linear)) = tree.tiles.get(tile_id)? else {
        return None;
    };
    if linear.dir != dir {
        return None;
    }
    linear
        .children
        .iter()
        .copied()
        .map(|child| frame_layout_leaf_node_key(tree, child))
        .collect()
}

fn frame_layout_member_ids(
    graph_app: &GraphBrowserApp,
    members: impl IntoIterator<Item = NodeKey>,
) -> Option<Vec<String>> {
    members
        .into_iter()
        .map(|node_key| frame_layout_member_id(graph_app, node_key))
        .collect()
}

fn unique_member_count(members: &[NodeKey]) -> usize {
    members.iter().copied().collect::<HashSet<_>>().len()
}

pub(crate) fn derive_frame_layout_hint_from_tile(
    graph_app: &GraphBrowserApp,
    tree: &Tree<TileKind>,
    tile_id: TileId,
) -> Option<FrameLayoutHint> {
    let Tile::Container(Container::Linear(linear)) = tree.tiles.get(tile_id)? else {
        return None;
    };

    match (linear.dir, linear.children.as_slice()) {
        (LinearDir::Horizontal, [left, right]) => {
            if let (Some(first), Some(second)) = (
                frame_layout_leaf_node_key(tree, *left),
                frame_layout_leaf_node_key(tree, *right),
            ) && unique_member_count(&[first, second]) == 2
            {
                let members = frame_layout_member_ids(graph_app, [first, second])?;
                return Some(FrameLayoutHint::SplitHalf {
                    first: members[0].clone(),
                    second: members[1].clone(),
                    orientation: SplitOrientation::Vertical,
                });
            }

            if let Some(dominant) = frame_layout_leaf_node_key(tree, *left)
                && let Some(wings) =
                    frame_layout_linear_leaf_members(tree, *right, LinearDir::Vertical)
                && unique_member_count(&[dominant, wings[0], wings[1]]) == 3
            {
                let members = frame_layout_member_ids(graph_app, [dominant, wings[0], wings[1]])?;
                return Some(FrameLayoutHint::SplitTriptych {
                    dominant: members[0].clone(),
                    dominant_edge: DominantEdge::Left,
                    wings: [members[1].clone(), members[2].clone()],
                });
            }

            if let Some(dominant) = frame_layout_leaf_node_key(tree, *right)
                && let Some(wings) =
                    frame_layout_linear_leaf_members(tree, *left, LinearDir::Vertical)
                && unique_member_count(&[wings[0], wings[1], dominant]) == 3
            {
                let members = frame_layout_member_ids(graph_app, [dominant, wings[0], wings[1]])?;
                return Some(FrameLayoutHint::SplitTriptych {
                    dominant: members[0].clone(),
                    dominant_edge: DominantEdge::Right,
                    wings: [members[1].clone(), members[2].clone()],
                });
            }

            None
        }
        (LinearDir::Vertical, [top, bottom]) => {
            if let (Some(first), Some(second)) = (
                frame_layout_leaf_node_key(tree, *top),
                frame_layout_leaf_node_key(tree, *bottom),
            ) && unique_member_count(&[first, second]) == 2
            {
                let members = frame_layout_member_ids(graph_app, [first, second])?;
                return Some(FrameLayoutHint::SplitHalf {
                    first: members[0].clone(),
                    second: members[1].clone(),
                    orientation: SplitOrientation::Horizontal,
                });
            }

            if let Some(dominant) = frame_layout_leaf_node_key(tree, *top)
                && let Some(wings) =
                    frame_layout_linear_leaf_members(tree, *bottom, LinearDir::Horizontal)
                && unique_member_count(&[dominant, wings[0], wings[1]]) == 3
            {
                let members = frame_layout_member_ids(graph_app, [dominant, wings[0], wings[1]])?;
                return Some(FrameLayoutHint::SplitTriptych {
                    dominant: members[0].clone(),
                    dominant_edge: DominantEdge::Top,
                    wings: [members[1].clone(), members[2].clone()],
                });
            }

            if let Some(dominant) = frame_layout_leaf_node_key(tree, *bottom)
                && let Some(wings) =
                    frame_layout_linear_leaf_members(tree, *top, LinearDir::Horizontal)
                && unique_member_count(&[wings[0], wings[1], dominant]) == 3
            {
                let members = frame_layout_member_ids(graph_app, [dominant, wings[0], wings[1]])?;
                return Some(FrameLayoutHint::SplitTriptych {
                    dominant: members[0].clone(),
                    dominant_edge: DominantEdge::Bottom,
                    wings: [members[1].clone(), members[2].clone()],
                });
            }

            let top_row = frame_layout_linear_leaf_members(tree, *top, LinearDir::Horizontal);
            let bottom_row = frame_layout_linear_leaf_members(tree, *bottom, LinearDir::Horizontal);
            if let (Some(top_row), Some(bottom_row)) = (top_row, bottom_row) {
                let members = [top_row[0], top_row[1], bottom_row[0], bottom_row[1]];
                if unique_member_count(&members) == 4 {
                    let member_ids = frame_layout_member_ids(graph_app, members)?;
                    return Some(FrameLayoutHint::SplitQuartered {
                        top_left: member_ids[0].clone(),
                        top_right: member_ids[1].clone(),
                        bottom_left: member_ids[2].clone(),
                        bottom_right: member_ids[3].clone(),
                    });
                }
            }

            None
        }
        (LinearDir::Horizontal, [first, second, third]) => {
            let members = [
                frame_layout_leaf_node_key(tree, *first)?,
                frame_layout_leaf_node_key(tree, *second)?,
                frame_layout_leaf_node_key(tree, *third)?,
            ];
            if unique_member_count(&members) != 3 {
                return None;
            }
            let member_ids = frame_layout_member_ids(graph_app, members)?;
            Some(FrameLayoutHint::SplitPamphlet {
                members: [
                    member_ids[0].clone(),
                    member_ids[1].clone(),
                    member_ids[2].clone(),
                ],
                orientation: SplitOrientation::Vertical,
            })
        }
        (LinearDir::Vertical, [first, second, third]) => {
            let members = [
                frame_layout_leaf_node_key(tree, *first)?,
                frame_layout_leaf_node_key(tree, *second)?,
                frame_layout_leaf_node_key(tree, *third)?,
            ];
            if unique_member_count(&members) != 3 {
                return None;
            }
            let member_ids = frame_layout_member_ids(graph_app, members)?;
            Some(FrameLayoutHint::SplitPamphlet {
                members: [
                    member_ids[0].clone(),
                    member_ids[1].clone(),
                    member_ids[2].clone(),
                ],
                orientation: SplitOrientation::Horizontal,
            })
        }
        _ => None,
    }
}

fn frame_layout_hints_from_tabs_tile(
    graph_app: &GraphBrowserApp,
    tree: &Tree<TileKind>,
    tabs_tile_id: TileId,
) -> Option<Vec<FrameLayoutHint>> {
    let Tile::Container(Container::Tabs(tabs)) = tree.tiles.get(tabs_tile_id)? else {
        return None;
    };

    Some(
        tabs.children
            .iter()
            .copied()
            .filter_map(|child| derive_frame_layout_hint_from_tile(graph_app, tree, child))
            .collect(),
    )
}

fn frame_layout_hints_from_runtime_tree(
    graph_app: &GraphBrowserApp,
    tree: &Tree<TileKind>,
) -> Option<Vec<FrameLayoutHint>> {
    frame_layout_hints_from_tabs_tile(graph_app, tree, tree.root()?)
}

pub(crate) fn frame_layout_hint_summary(hint: &FrameLayoutHint) -> String {
    match hint {
        FrameLayoutHint::SplitHalf {
            orientation,
            first: _,
            second: _,
        } => match orientation {
            SplitOrientation::Vertical => "Split half (columns)".to_string(),
            SplitOrientation::Horizontal => "Split half (rows)".to_string(),
        },
        FrameLayoutHint::SplitPamphlet { orientation, .. } => match orientation {
            SplitOrientation::Vertical => "Pamphlet (3 columns)".to_string(),
            SplitOrientation::Horizontal => "Pamphlet (3 rows)".to_string(),
        },
        FrameLayoutHint::SplitTriptych { dominant_edge, .. } => match dominant_edge {
            DominantEdge::Left => "Triptych (left dominant)".to_string(),
            DominantEdge::Right => "Triptych (right dominant)".to_string(),
            DominantEdge::Top => "Triptych (top dominant)".to_string(),
            DominantEdge::Bottom => "Triptych (bottom dominant)".to_string(),
        },
        FrameLayoutHint::SplitQuartered { .. } => "Quartered grid".to_string(),
    }
}

fn frame_layout_sync_intents_for_name(
    graph_app: &GraphBrowserApp,
    frame_name: &str,
    tree: &Tree<TileKind>,
) -> Vec<GraphIntent> {
    let Some(frame_key) = frame_key_for_name(graph_app, frame_name) else {
        return Vec::new();
    };
    let Some(derived_hints) = frame_layout_hints_from_runtime_tree(graph_app, tree) else {
        return Vec::new();
    };

    let existing_hints = graph_app
        .domain_graph()
        .frame_layout_hints(frame_key)
        .map(|hints| hints.to_vec())
        .unwrap_or_default();
    if existing_hints == derived_hints {
        return Vec::new();
    }

    let mut intents = Vec::with_capacity(existing_hints.len() + derived_hints.len());
    for hint_index in (0..existing_hints.len()).rev() {
        intents.push(GraphIntent::RemoveFrameLayoutHint {
            frame: frame_key,
            hint_index,
        });
    }
    intents.extend(
        derived_hints
            .into_iter()
            .map(|hint| GraphIntent::RecordFrameLayoutHint {
                frame: frame_key,
                hint,
            }),
    );
    intents
}

pub(crate) fn frame_layout_sync_intents_for_current_frame(
    graph_app: &GraphBrowserApp,
    tree: &Tree<TileKind>,
) -> Vec<GraphIntent> {
    let Some(frame_name) = graph_app.current_frame_name() else {
        return Vec::new();
    };
    frame_layout_sync_intents_for_name(graph_app, frame_name, tree)
}

fn build_frame_layout_hint_tile(
    graph_app: &GraphBrowserApp,
    tiles: &mut Tiles<TileKind>,
    hint: &FrameLayoutHint,
    frame_members: &HashSet<NodeKey>,
) -> Option<(TileId, Vec<NodeKey>)> {
    match hint {
        FrameLayoutHint::SplitHalf {
            first,
            second,
            orientation,
        } => {
            let first = resolve_frame_layout_member_key(graph_app, first, frame_members)?;
            let second = resolve_frame_layout_member_key(graph_app, second, frame_members)?;
            if first == second {
                return None;
            }

            let children = vec![
                insert_frame_member_pane(tiles, first),
                insert_frame_member_pane(tiles, second),
            ];
            // `Vertical` means side-by-side columns; `Horizontal` means stacked rows.
            let split = match orientation {
                SplitOrientation::Vertical => tiles.insert_horizontal_tile(children),
                SplitOrientation::Horizontal => tiles.insert_vertical_tile(children),
            };
            Some((split, vec![first, second]))
        }
        FrameLayoutHint::SplitPamphlet {
            members,
            orientation,
        } => {
            let members = members
                .iter()
                .map(|member| resolve_frame_layout_member_key(graph_app, member, frame_members))
                .collect::<Option<Vec<_>>>()?;
            let unique = members.iter().copied().collect::<HashSet<_>>();
            if unique.len() != 3 {
                return None;
            }

            let children = members
                .iter()
                .copied()
                .map(|member| insert_frame_member_pane(tiles, member))
                .collect::<Vec<_>>();
            let split = match orientation {
                SplitOrientation::Vertical => tiles.insert_horizontal_tile(children),
                SplitOrientation::Horizontal => tiles.insert_vertical_tile(children),
            };
            Some((split, members))
        }
        FrameLayoutHint::SplitTriptych {
            dominant,
            dominant_edge,
            wings,
        } => {
            let dominant = resolve_frame_layout_member_key(graph_app, dominant, frame_members)?;
            let first_wing = resolve_frame_layout_member_key(graph_app, &wings[0], frame_members)?;
            let second_wing = resolve_frame_layout_member_key(graph_app, &wings[1], frame_members)?;
            let members = vec![dominant, first_wing, second_wing];
            if members.iter().copied().collect::<HashSet<_>>().len() != 3 {
                return None;
            }

            let dominant_tile = insert_frame_member_pane(tiles, dominant);
            let wing_tiles = vec![
                insert_frame_member_pane(tiles, first_wing),
                insert_frame_member_pane(tiles, second_wing),
            ];
            let split = match dominant_edge {
                DominantEdge::Left => {
                    let wings = tiles.insert_vertical_tile(wing_tiles);
                    tiles.insert_horizontal_tile(vec![dominant_tile, wings])
                }
                DominantEdge::Right => {
                    let wings = tiles.insert_vertical_tile(wing_tiles);
                    tiles.insert_horizontal_tile(vec![wings, dominant_tile])
                }
                DominantEdge::Top => {
                    let wings = tiles.insert_horizontal_tile(wing_tiles);
                    tiles.insert_vertical_tile(vec![dominant_tile, wings])
                }
                DominantEdge::Bottom => {
                    let wings = tiles.insert_horizontal_tile(wing_tiles);
                    tiles.insert_vertical_tile(vec![wings, dominant_tile])
                }
            };
            Some((split, members))
        }
        FrameLayoutHint::SplitQuartered {
            top_left,
            top_right,
            bottom_left,
            bottom_right,
        } => {
            let members = vec![
                resolve_frame_layout_member_key(graph_app, top_left, frame_members)?,
                resolve_frame_layout_member_key(graph_app, top_right, frame_members)?,
                resolve_frame_layout_member_key(graph_app, bottom_left, frame_members)?,
                resolve_frame_layout_member_key(graph_app, bottom_right, frame_members)?,
            ];
            if members.iter().copied().collect::<HashSet<_>>().len() != 4 {
                return None;
            }

            let top_left = insert_frame_member_pane(tiles, members[0]);
            let top_right = insert_frame_member_pane(tiles, members[1]);
            let bottom_left = insert_frame_member_pane(tiles, members[2]);
            let bottom_right = insert_frame_member_pane(tiles, members[3]);
            let top_row = tiles.insert_horizontal_tile(vec![top_left, top_right]);
            let bottom_row = tiles.insert_horizontal_tile(vec![bottom_left, bottom_right]);
            let split = tiles.insert_vertical_tile(vec![top_row, bottom_row]);
            Some((split, members))
        }
    }
}

pub(crate) fn materialize_frame_tile_group_tabs(
    graph_app: &GraphBrowserApp,
    frame_anchor: NodeKey,
    tiles: &mut Tiles<TileKind>,
) -> Option<(
    Vec<TileId>,
    Vec<NodeKey>,
    Vec<crate::app::FrameHintTabRuntime>,
)> {
    let member_keys = ordered_live_frame_member_keys_for_anchor(graph_app, frame_anchor);
    if member_keys.is_empty() {
        return None;
    }

    let frame_members = member_keys.iter().copied().collect::<HashSet<_>>();
    let mut covered_members = HashSet::new();
    let mut tab_tiles = Vec::new();
    let mut hint_tabs = Vec::new();
    let hints = graph_app
        .domain_graph()
        .frame_layout_hints(frame_anchor)
        .map(|hints| hints.to_vec())
        .unwrap_or_default();

    for hint in hints {
        let Some((hint_tile, hint_members)) =
            build_frame_layout_hint_tile(graph_app, tiles, &hint, &frame_members)
        else {
            continue;
        };
        covered_members.extend(hint_members);
        tab_tiles.push(hint_tile);
        hint_tabs.push(crate::app::FrameHintTabRuntime {
            tile_id: hint_tile,
            hint,
        });
    }

    tab_tiles.extend(
        member_keys
            .iter()
            .copied()
            .filter(|member| !covered_members.contains(member))
            .map(|member| insert_frame_member_pane(tiles, member)),
    );

    Some((tab_tiles, member_keys, hint_tabs))
}

pub(crate) fn register_frame_tile_group_runtime(
    graph_app: &mut GraphBrowserApp,
    tree: &Tree<TileKind>,
    group_id: TileId,
    frame_anchor: NodeKey,
) {
    let hint_tabs = frame_layout_hints_from_tabs_tile(graph_app, tree, group_id)
        .unwrap_or_default()
        .into_iter()
        .zip(
            match tree.tiles.get(group_id) {
                Some(Tile::Container(Container::Tabs(tabs))) => tabs.children.clone(),
                _ => Vec::new(),
            }
            .into_iter()
            .filter(|child| derive_frame_layout_hint_from_tile(graph_app, tree, *child).is_some()),
        )
        .map(|(hint, tile_id)| crate::app::FrameHintTabRuntime { tile_id, hint })
        .collect();

    graph_app.workspace.graph_runtime.frame_tile_groups.insert(
        group_id,
        crate::app::FrameTileGroupRuntimeState {
            frame_anchor,
            hint_tabs,
        },
    );
}

pub(crate) fn refresh_frame_tile_group_runtime(
    graph_app: &mut GraphBrowserApp,
    tree: &Tree<TileKind>,
) {
    let registered: Vec<(TileId, NodeKey)> = graph_app
        .workspace
        .graph_runtime
        .frame_tile_groups
        .iter()
        .map(|(group_id, state)| (*group_id, state.frame_anchor))
        .collect();

    graph_app.workspace.graph_runtime.frame_tile_groups.clear();
    for (group_id, frame_anchor) in registered {
        if matches!(
            tree.tiles.get(group_id),
            Some(Tile::Container(Container::Tabs(_)))
        ) {
            register_frame_tile_group_runtime(graph_app, tree, group_id, frame_anchor);
        }
    }
}

pub(crate) fn frame_layout_sync_intents_for_registered_frame_groups(
    graph_app: &GraphBrowserApp,
    tree: &Tree<TileKind>,
) -> Vec<GraphIntent> {
    let mut intents = Vec::new();

    for (group_id, state) in &graph_app.workspace.graph_runtime.frame_tile_groups {
        let Some(derived_hints) = frame_layout_hints_from_tabs_tile(graph_app, tree, *group_id)
        else {
            continue;
        };
        let existing_hints = graph_app
            .domain_graph()
            .frame_layout_hints(state.frame_anchor)
            .map(|hints| hints.to_vec())
            .unwrap_or_default();
        if existing_hints == derived_hints {
            continue;
        }

        for hint_index in (0..existing_hints.len()).rev() {
            intents.push(GraphIntent::RemoveFrameLayoutHint {
                frame: state.frame_anchor,
                hint_index,
            });
        }
        intents.extend(
            derived_hints
                .into_iter()
                .map(|hint| GraphIntent::RecordFrameLayoutHint {
                    frame: state.frame_anchor,
                    hint,
                }),
        );
    }

    intents
}

pub(crate) fn frame_hint_tab_info(
    graph_app: &GraphBrowserApp,
    tile_id: TileId,
) -> Option<(NodeKey, usize, FrameLayoutHint)> {
    for state in graph_app.workspace.graph_runtime.frame_tile_groups.values() {
        for (index, hint_tab) in state.hint_tabs.iter().enumerate() {
            if hint_tab.tile_id == tile_id {
                return Some((state.frame_anchor, index, hint_tab.hint.clone()));
            }
        }
    }
    None
}

pub(crate) fn synthesize_runtime_tree_from_graph_frame(
    graph_app: &GraphBrowserApp,
    name: &str,
) -> Result<(Tree<TileKind>, Vec<NodeKey>), String> {
    let Some(frame_key) = frame_key_for_name(graph_app, name) else {
        return Err(format!("frame snapshot '{name}' not found"));
    };

    let mut tiles = Tiles::default();
    let Some((tab_tiles, member_keys, _hint_tabs)) =
        materialize_frame_tile_group_tabs(graph_app, frame_key, &mut tiles)
    else {
        return Err(format!("frame snapshot '{name}' not found"));
    };

    let root = tiles.insert_tab_tile(tab_tiles);
    let tree = Tree::new("graphshell_workspace_layout", root, tiles);
    Ok((tree, member_keys))
}

pub(crate) fn build_membership_index_from_frame_manifests(
    graph_app: &GraphBrowserApp,
) -> HashMap<Uuid, BTreeSet<String>> {
    let mut index: HashMap<Uuid, BTreeSet<String>> = HashMap::new();
    for workspace_name in graph_app.list_workspace_layout_names() {
        if GraphBrowserApp::is_reserved_workspace_layout_name(&workspace_name) {
            continue;
        }
        let Ok(mut bundle) = load_named_frame_bundle(graph_app, &workspace_name) else {
            continue;
        };
        if validate_frame_bundle(&bundle).is_err() {
            repair_manifest_membership(&mut bundle);
        }
        for uuid in &bundle.manifest.member_node_uuids {
            index
                .entry(*uuid)
                .or_default()
                .insert(workspace_name.clone());
        }
    }
    index
}

pub(crate) fn refresh_frame_membership_cache_from_manifests(
    graph_app: &mut GraphBrowserApp,
) -> Result<(), String> {
    let membership_index = build_membership_index_from_frame_manifests(graph_app);
    graph_app.init_membership_index(membership_index);
    Ok(())
}

fn refresh_frame_arrangement_projection_from_manifests(graph_app: &mut GraphBrowserApp) {
    let manifest_frame_names: BTreeSet<String> = graph_app
        .list_workspace_layout_names()
        .into_iter()
        .filter(|name| !GraphBrowserApp::is_reserved_workspace_layout_name(name))
        .collect();

    let stale_frame_titles = graph_app
        .arrangement_projection_groups()
        .into_iter()
        .filter(|group| {
            group.sub_kind == crate::graph::ArrangementSubKind::FrameMember
                && !manifest_frame_names.contains(&group.title)
        })
        .map(|group| group.title)
        .collect::<Vec<_>>();

    for stale_title in stale_frame_titles {
        graph_app.remove_named_workbench_frame_graph_representation(&stale_title);
    }

    for frame_name in manifest_frame_names {
        let bundle = match load_named_frame_bundle(graph_app, &frame_name) {
            Ok(bundle) => bundle,
            Err(error) => {
                warn!(
                    "Skipping arrangement projection refresh for frame '{frame_name}': failed to load bundle: {error}"
                );
                continue;
            }
        };

        let tree = match restore_runtime_tree_from_frame_bundle(graph_app, &bundle) {
            Ok((tree, _)) => tree,
            Err(error) => {
                warn!(
                    "Skipping arrangement projection refresh for frame '{frame_name}': failed to restore runtime tree: {error}"
                );
                continue;
            }
        };

        graph_app.sync_named_workbench_frame_graph_representation(&frame_name, &tree);
    }
}

pub(crate) fn refresh_workbench_projection_from_manifests(
    graph_app: &mut GraphBrowserApp,
) -> Result<(), String> {
    graph_app.rebuild_navigator_projection_rows();
    refresh_frame_membership_cache_from_manifests(graph_app)?;
    refresh_frame_arrangement_projection_from_manifests(graph_app);
    Ok(())
}

pub(crate) fn build_frame_activation_recency_from_frame_manifests(
    graph_app: &GraphBrowserApp,
) -> (HashMap<Uuid, (u64, String)>, u64) {
    let mut recency = HashMap::new();
    let mut activation_seq = 0u64;

    for workspace_name in graph_app.list_workspace_layout_names() {
        if GraphBrowserApp::is_reserved_workspace_layout_name(&workspace_name) {
            continue;
        }
        let Ok(mut bundle) = load_named_frame_bundle(graph_app, &workspace_name) else {
            continue;
        };
        if validate_frame_bundle(&bundle).is_err() {
            repair_manifest_membership(&mut bundle);
        }
        let Some(last_activated) = bundle.metadata.last_activated_at_ms else {
            continue;
        };
        activation_seq = activation_seq.max(last_activated);
        for uuid in &bundle.manifest.member_node_uuids {
            match recency.get(uuid) {
                Some((existing, _)) if *existing >= last_activated => {}
                _ => {
                    recency.insert(*uuid, (last_activated, workspace_name.clone()));
                }
            }
        }
    }

    (recency, activation_seq)
}

pub(crate) fn mark_named_frame_bundle_activated(
    graph_app: &mut GraphBrowserApp,
    name: &str,
) -> Result<(), String> {
    let mut bundle = load_named_frame_bundle(graph_app, name)?;
    let now = now_unix_ms();
    bundle.metadata.updated_at_ms = now;
    bundle.metadata.last_activated_at_ms = Some(now);
    let bundle_json = serde_json::to_string(&bundle).map_err(|e| e.to_string())?;
    graph_app.save_workspace_layout_json(name, &bundle_json);
    Ok(())
}

pub(crate) fn validate_workspace_bundle(
    bundle: &PersistedWorkspace,
) -> Result<(), FrameBundleError> {
    validate_frame_bundle(bundle)
}

pub(crate) fn serialize_named_workspace_bundle(
    graph_app: &GraphBrowserApp,
    name: &str,
    tree: &Tree<TileKind>,
) -> Result<String, String> {
    serialize_named_frame_bundle(graph_app, name, tree)
}

pub(crate) fn save_named_workspace_bundle(
    graph_app: &mut GraphBrowserApp,
    name: &str,
    tree: &Tree<TileKind>,
) -> Result<(), String> {
    save_named_frame_bundle(graph_app, name, tree)
}

pub(crate) fn load_named_workspace_bundle(
    graph_app: &GraphBrowserApp,
    name: &str,
) -> Result<PersistedWorkspace, String> {
    load_named_frame_bundle(graph_app, name)
}

pub(crate) fn restore_runtime_tree_from_workspace_bundle(
    graph_app: &GraphBrowserApp,
    bundle: &PersistedWorkspace,
) -> Result<(Tree<TileKind>, Vec<NodeKey>), String> {
    restore_runtime_tree_from_frame_bundle(graph_app, bundle)
}

pub(crate) fn build_membership_index_from_workspace_manifests(
    graph_app: &GraphBrowserApp,
) -> HashMap<Uuid, BTreeSet<String>> {
    build_membership_index_from_frame_manifests(graph_app)
}

pub(crate) fn refresh_workspace_membership_cache_from_manifests(
    graph_app: &mut GraphBrowserApp,
) -> Result<(), String> {
    refresh_frame_membership_cache_from_manifests(graph_app)
}

pub(crate) fn build_workspace_activation_recency_from_workspace_manifests(
    graph_app: &GraphBrowserApp,
) -> (HashMap<Uuid, (u64, String)>, u64) {
    build_frame_activation_recency_from_frame_manifests(graph_app)
}

pub(crate) fn mark_named_workspace_bundle_activated(
    graph_app: &mut GraphBrowserApp,
    name: &str,
) -> Result<(), String> {
    mark_named_frame_bundle_activated(graph_app, name)
}

pub(crate) fn restore_tiles_tree_from_persistence(graph_app: &GraphBrowserApp) -> Tree<TileKind> {
    let mut tiles = Tiles::default();
    let graph_tile_id = tiles.insert_pane(TileKind::Graph(
        crate::shell::desktop::workbench::pane_model::GraphPaneRef::new(GraphViewId::default()),
    ));
    let mut tiles_tree = Tree::new("graphshell_tiles", graph_tile_id, tiles);
    if let Some(layout_json) = graph_app.load_tile_layout_json()
        && let Ok(mut restored_tree) = serde_json::from_str::<Tree<TileKind>>(&layout_json)
    {
        tile_runtime::prune_stale_node_pane_keys_only(&mut restored_tree, graph_app);
        if restored_tree.root().is_some() {
            tiles_tree = restored_tree;
        }
    }
    tiles_tree
}

fn workspace_nodes_from_tree(tree: &Tree<TileKind>) -> Vec<NodeKey> {
    tree.tiles
        .iter()
        .filter_map(|(_, tile)| match tile {
            egui_tiles::Tile::Pane(TileKind::Node(state)) => Some(state.node),
            _ => None,
        })
        .collect()
}

/// Rebuild the UUID-keyed frame membership index from persisted named frame layouts.
///
/// Reserved autosave/session frame keys are intentionally excluded so routing decisions
/// operate on user-meaningful named frame snapshots.
pub(crate) fn build_membership_index_from_layouts(
    graph_app: &GraphBrowserApp,
) -> HashMap<Uuid, BTreeSet<String>> {
    let graph_backed = graph_app.arrangement_frame_membership_index();
    if !graph_backed.is_empty() {
        graph_app.emit_arrangement_projection_health();
        return graph_backed;
    }

    graph_app.emit_arrangement_missing_family_fallback();

    let mut index: HashMap<Uuid, BTreeSet<String>> = HashMap::new();

    for workspace_name in graph_app.list_workspace_layout_names() {
        if GraphBrowserApp::is_reserved_workspace_layout_name(&workspace_name) {
            continue;
        }
        let Some(layout_json) = graph_app.load_workspace_layout_json(&workspace_name) else {
            continue;
        };
        let Ok(mut tree) = serde_json::from_str::<Tree<TileKind>>(&layout_json) else {
            warn!("Skipping frame snapshot '{workspace_name}': invalid layout json");
            continue;
        };
        tile_runtime::prune_stale_node_pane_keys_only(&mut tree, graph_app);
        for node_key in workspace_nodes_from_tree(&tree) {
            let Some(node) = graph_app.domain_graph().get_node(node_key) else {
                continue;
            };
            index
                .entry(node.id)
                .or_default()
                .insert(workspace_name.clone());
        }
    }

    index
}

pub(crate) fn switch_persistence_store(
    graph_app: &mut GraphBrowserApp,
    window: &EmbedderWindow,
    tiles_tree: &mut Tree<TileKind>,
    tile_rendering_contexts: &mut HashMap<NodeKey, Rc<OffscreenRenderingContext>>,
    tile_favicon_textures: &mut HashMap<NodeKey, (u64, egui::TextureHandle)>,
    favicon_textures: &mut HashMap<WebViewId, (egui::TextureHandle, egui::load::SizedTexture)>,
    lifecycle_intents: &mut Vec<GraphIntent>,
    data_dir: PathBuf,
) -> Result<(), String> {
    // Preflight the new directory first so failed switches are non-destructive.
    crate::services::persistence::GraphStore::open(data_dir.clone()).map_err(|e| e.to_string())?;
    let snapshot_interval_secs = graph_app.snapshot_interval_secs();

    lifecycle_intents.extend(webview_controller::close_all_webviews(graph_app, window));
    tile_runtime::reset_runtime_webview_state(
        tiles_tree,
        tile_rendering_contexts,
        tile_favicon_textures,
        favicon_textures,
    );

    graph_app.switch_persistence_dir(data_dir)?;
    if let Some(secs) = snapshot_interval_secs {
        graph_app.set_snapshot_interval_secs(secs)?;
    }
    *tiles_tree = restore_tiles_tree_from_persistence(graph_app);
    let membership_index = build_membership_index_from_layouts(graph_app);
    graph_app.init_membership_index(membership_index);
    Ok(())
}

/// Delete named frame snapshots that become empty after stale-node pruning.
pub(crate) fn prune_empty_named_workspaces(graph_app: &mut GraphBrowserApp) -> usize {
    let mut names_to_delete = Vec::new();
    for workspace_name in graph_app.list_workspace_layout_names() {
        if GraphBrowserApp::is_reserved_workspace_layout_name(&workspace_name) {
            continue;
        }
        let Some(layout_json) = graph_app.load_workspace_layout_json(&workspace_name) else {
            continue;
        };
        let Ok(mut tree) = serde_json::from_str::<Tree<TileKind>>(&layout_json) else {
            warn!("Skipping frame snapshot '{workspace_name}': invalid layout json");
            continue;
        };
        tile_runtime::prune_stale_node_pane_keys_only(&mut tree, graph_app);
        if workspace_nodes_from_tree(&tree).is_empty() {
            names_to_delete.push(workspace_name);
        }
    }
    let mut deleted = 0usize;
    for name in names_to_delete {
        if graph_app.delete_workspace_layout(&name).is_ok() {
            deleted += 1;
        }
    }
    if deleted > 0 {
        let membership_index = build_membership_index_from_layouts(graph_app);
        graph_app.init_membership_index(membership_index);
    }
    deleted
}

/// Keep only the latest N named frame snapshots by activation recency.
pub(crate) fn keep_latest_named_workspaces(graph_app: &mut GraphBrowserApp, keep: usize) -> usize {
    let mut names: Vec<String> = graph_app
        .list_workspace_layout_names()
        .into_iter()
        .filter(|name| !GraphBrowserApp::is_reserved_workspace_layout_name(name))
        .collect();
    names.sort_by(|a, b| {
        graph_app
            .frame_recency_seq_for_name(b)
            .cmp(&graph_app.frame_recency_seq_for_name(a))
            .then_with(|| a.cmp(b))
    });
    let names_to_delete: Vec<String> = names.into_iter().skip(keep).collect();
    let mut deleted = 0usize;
    for name in names_to_delete {
        if graph_app.delete_workspace_layout(&name).is_ok() {
            deleted += 1;
        }
    }
    if deleted > 0 {
        let membership_index = build_membership_index_from_layouts(graph_app);
        graph_app.init_membership_index(membership_index);
    }
    deleted
}

pub(crate) fn parse_data_dir_input(raw: &str) -> Option<PathBuf> {
    let trimmed = raw.trim().trim_matches('"').trim_matches('\'').trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(PathBuf::from(trimmed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{DominantEdge, FrameLayoutHint};
    use crate::shell::desktop::ui::workbench_host::WorkbenchChromeProjection;
    use crate::shell::desktop::workbench::pane_model::{GraphPaneRef, ToolPaneState};
    use crate::util::VersoAddress;
    use egui_tiles::{Container, LinearDir, TileId, Tiles, Tree};
    use euclid::default::Point2D;
    use tempfile::TempDir;

    fn workspace_layout_json_with_nodes(node_keys: &[NodeKey]) -> String {
        let mut tiles = Tiles::default();
        let mut children = Vec::new();
        children
            .push(tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(GraphViewId::default()))));
        for node_key in node_keys {
            children.push(tiles.insert_pane(TileKind::Node((*node_key).into())));
        }
        let root = if children.len() == 1 {
            children[0]
        } else {
            tiles.insert_tab_tile(children)
        };
        let tree = Tree::new("workspace_test", root, tiles);
        serde_json::to_string(&tree).expect("frame layout should serialize")
    }

    fn frame_layout_node_id(app: &GraphBrowserApp, key: NodeKey) -> String {
        app.domain_graph()
            .get_node(key)
            .expect("member node should exist")
            .id
            .to_string()
    }

    fn assert_node_tile(tree: &Tree<TileKind>, tile_id: TileId, expected: NodeKey) {
        match tree.tiles.get(tile_id) {
            Some(Tile::Pane(TileKind::Node(state))) => assert_eq!(state.node, expected),
            other => panic!("expected node pane for {expected:?}, got {other:?}"),
        }
    }

    fn frame_key_by_name(app: &GraphBrowserApp, name: &str) -> NodeKey {
        let frame_url = VersoAddress::frame(name.to_string()).to_string();
        app.domain_graph()
            .get_node_by_url(&frame_url)
            .map(|(frame_key, _)| frame_key)
            .expect("frame anchor should exist")
    }

    #[test]
    fn test_build_membership_index_from_layouts_skips_reserved_and_stale_nodes() {
        let dir = TempDir::new().unwrap();
        let mut app = GraphBrowserApp::new_from_dir(dir.path().to_path_buf());
        let a = app.add_node_and_sync("https://a.example".into(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("https://b.example".into(), Point2D::new(1.0, 0.0));
        let a_id = app.workspace.domain.graph.get_node(a).unwrap().id;
        let b_id = app.workspace.domain.graph.get_node(b).unwrap().id;
        let stale = NodeKey::new(999_999);

        app.save_workspace_layout_json(
            "workspace-alpha",
            &workspace_layout_json_with_nodes(&[a, b]),
        );
        app.save_workspace_layout_json("workspace-beta", &workspace_layout_json_with_nodes(&[b]));
        app.save_workspace_layout_json(
            "workspace-stale",
            &workspace_layout_json_with_nodes(&[stale]),
        );
        app.save_workspace_layout_json("latest", &workspace_layout_json_with_nodes(&[a]));

        let index = build_membership_index_from_layouts(&app);
        assert_eq!(
            index.get(&a_id),
            Some(&BTreeSet::from(["workspace-alpha".to_string()]))
        );
        assert_eq!(
            index.get(&b_id),
            Some(&BTreeSet::from([
                "workspace-alpha".to_string(),
                "workspace-beta".to_string()
            ]))
        );
        assert_eq!(index.len(), 2);
    }

    #[test]
    fn test_prune_empty_named_workspaces_rebuilds_membership_index() {
        let dir = TempDir::new().unwrap();
        let mut app = GraphBrowserApp::new_from_dir(dir.path().to_path_buf());
        let live = app.add_node_and_sync("https://live.example".into(), Point2D::new(0.0, 0.0));
        let live_id = app.workspace.domain.graph.get_node(live).unwrap().id;
        let stale = NodeKey::new(888_888);

        app.save_workspace_layout_json(
            "workspace-keep",
            &workspace_layout_json_with_nodes(&[live]),
        );
        app.save_workspace_layout_json(
            "workspace-empty",
            &workspace_layout_json_with_nodes(&[stale]),
        );
        app.init_membership_index(HashMap::from([(
            live_id,
            BTreeSet::from(["workspace-keep".to_string(), "workspace-empty".to_string()]),
        )]));

        let deleted = prune_empty_named_workspaces(&mut app);
        assert_eq!(deleted, 1);
        assert!(app.load_workspace_layout_json("workspace-empty").is_none());
        assert!(app.load_workspace_layout_json("workspace-keep").is_some());
        assert_eq!(
            app.membership_for_node(live_id),
            &BTreeSet::from(["workspace-keep".to_string()])
        );
    }

    #[test]
    fn test_keep_latest_named_workspaces_rebuilds_membership_index() {
        let dir = TempDir::new().unwrap();
        let mut app = GraphBrowserApp::new_from_dir(dir.path().to_path_buf());
        let node = app.add_node_and_sync("https://node.example".into(), Point2D::new(0.0, 0.0));
        let node_id = app.workspace.domain.graph.get_node(node).unwrap().id;

        app.save_workspace_layout_json("workspace-old", &workspace_layout_json_with_nodes(&[node]));
        app.save_workspace_layout_json("workspace-mid", &workspace_layout_json_with_nodes(&[node]));
        app.save_workspace_layout_json("workspace-new", &workspace_layout_json_with_nodes(&[node]));
        app.note_frame_activated("workspace-old", [node]);
        app.note_frame_activated("workspace-mid", [node]);
        app.note_frame_activated("workspace-new", [node]);

        let deleted = keep_latest_named_workspaces(&mut app, 1);
        assert_eq!(deleted, 2);
        assert!(app.load_workspace_layout_json("workspace-old").is_none());
        assert!(app.load_workspace_layout_json("workspace-mid").is_none());
        assert!(app.load_workspace_layout_json("workspace-new").is_some());
        assert_eq!(
            app.membership_for_node(node_id),
            &BTreeSet::from(["workspace-new".to_string()])
        );
    }

    #[test]
    fn test_frame_bundle_serialization_excludes_diagnostics_payload() {
        let dir = TempDir::new().unwrap();
        let app = GraphBrowserApp::new_from_dir(dir.path().to_path_buf());
        let tree: Tree<TileKind> = serde_json::from_str(&workspace_layout_json_with_nodes(&[]))
            .expect("frame tree should deserialize");

        let json = serialize_named_workspace_bundle(&app, "workspace-clean", &tree)
            .expect("frame bundle should serialize");
        let value: serde_json::Value =
            serde_json::from_str(&json).expect("bundle json should parse");
        let root = value.as_object().expect("bundle should be json object");

        assert!(root.contains_key("version"));
        assert!(root.contains_key("name"));
        assert!(root.contains_key("layout"));
        assert!(root.contains_key("manifest"));
        assert!(root.contains_key("metadata"));
        assert!(root.contains_key("workbench_profile"));

        assert!(!root.contains_key("diagnostic_graph"));
        assert!(!root.contains_key("compositor_state"));
        assert!(!root.contains_key("event_ring"));
        assert!(!root.contains_key("channels"));
        assert!(!root.contains_key("spans"));
        assert!(!root.contains_key("recent_intents"));
    }

    #[test]
    fn test_frame_bundle_serialization_uses_pane_model_terms_not_legacy_aliases() {
        let dir = TempDir::new().unwrap();
        let mut app = GraphBrowserApp::new_from_dir(dir.path().to_path_buf());
        let node = app.add_node_and_sync("https://schema.example".into(), Point2D::new(0.0, 0.0));

        let mut tiles = Tiles::default();
        let graph = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(GraphViewId::default())));
        let node_pane = tiles.insert_pane(TileKind::Node(node.into()));
        let root = tiles.insert_tab_tile(vec![graph, node_pane]);
        let tree = Tree::new("workspace-schema-terms", root, tiles);

        let json = serialize_named_workspace_bundle(&app, "workspace-schema-terms", &tree)
            .expect("frame bundle should serialize");

        assert!(json.contains("\"NodePane\""));
        assert!(!json.contains("\"WebViewNode\""));
        assert!(!json.contains("\"Diagnostic\""));
    }

    #[test]
    fn test_frame_bundle_payload_stays_clean_after_restart() {
        let dir = TempDir::new().unwrap();
        let data_dir = dir.path().to_path_buf();
        let workspace_name = "workspace-restart-clean";

        {
            let mut app = GraphBrowserApp::new_from_dir(data_dir.clone());
            let node =
                app.add_node_and_sync("https://restart.example".into(), Point2D::new(0.0, 0.0));
            let mut tiles = Tiles::default();
            let graph =
                tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(GraphViewId::default())));
            let webview = tiles.insert_pane(TileKind::Node(node.into()));
            let root = tiles.insert_tab_tile(vec![graph, webview]);
            let tree = Tree::new("restart_bundle", root, tiles);

            save_named_workspace_bundle(&mut app, workspace_name, &tree)
                .expect("save frame bundle");
        }

        let app = GraphBrowserApp::new_from_dir(data_dir);
        let json = app
            .load_workspace_layout_json(workspace_name)
            .expect("frame bundle json should exist");
        let value: serde_json::Value = serde_json::from_str(&json).expect("bundle json parse");
        let root = value.as_object().expect("bundle should be object");

        assert!(root.contains_key("layout"));
        assert!(root.contains_key("manifest"));
        assert!(root.contains_key("metadata"));
        assert!(root.contains_key("workbench_profile"));
        assert!(!root.contains_key("diagnostic_graph"));
        assert!(!root.contains_key("channels"));
        assert!(!root.contains_key("spans"));
    }

    #[test]
    fn synthesize_runtime_tree_from_graph_frame_uses_graph_membership_when_bundle_missing() {
        let dir = TempDir::new().unwrap();
        let mut app = GraphBrowserApp::new_from_dir(dir.path().to_path_buf());
        let a = app.add_node_and_sync("https://a.example".into(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("https://b.example".into(), Point2D::new(1.0, 0.0));

        let mut tiles = Tiles::default();
        let a_tile = tiles.insert_pane(TileKind::Node(a.into()));
        let b_tile = tiles.insert_pane(TileKind::Node(b.into()));
        let root = tiles.insert_tab_tile(vec![a_tile, b_tile]);
        let tree = Tree::new("frame_graph_members", root, tiles);
        app.sync_named_workbench_frame_graph_representation("workspace-graph-backed", &tree);

        let (restored_tree, restored_nodes) =
            synthesize_runtime_tree_from_graph_frame(&app, "workspace-graph-backed")
                .expect("graph-backed frame should synthesize");

        assert_eq!(restored_nodes, vec![a, b]);
        let restored_member_count = restored_tree
            .tiles
            .iter()
            .filter(|(_, tile)| matches!(tile, Tile::Pane(TileKind::Node(_))))
            .count();
        assert_eq!(restored_member_count, 2);
    }

    #[test]
    fn synthesize_runtime_tree_from_graph_frame_materializes_split_hint_tabs_and_spillover() {
        let dir = TempDir::new().unwrap();
        let mut app = GraphBrowserApp::new_from_dir(dir.path().to_path_buf());
        let a = app.add_node_and_sync("https://triptych-a.example".into(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("https://triptych-b.example".into(), Point2D::new(1.0, 0.0));
        let c = app.add_node_and_sync("https://triptych-c.example".into(), Point2D::new(2.0, 0.0));
        let d = app.add_node_and_sync("https://triptych-d.example".into(), Point2D::new(3.0, 0.0));

        let mut tiles = Tiles::default();
        let a_tile = tiles.insert_pane(TileKind::Node(a.into()));
        let b_tile = tiles.insert_pane(TileKind::Node(b.into()));
        let c_tile = tiles.insert_pane(TileKind::Node(c.into()));
        let d_tile = tiles.insert_pane(TileKind::Node(d.into()));
        let root = tiles.insert_tab_tile(vec![a_tile, b_tile, c_tile, d_tile]);
        let tree = Tree::new("frame_graph_triptych_members", root, tiles);
        app.sync_named_workbench_frame_graph_representation("workspace-graph-triptych", &tree);

        let frame_url = VersoAddress::frame("workspace-graph-triptych").to_string();
        let (frame_key, _) = app
            .domain_graph()
            .get_node_by_url(&frame_url)
            .expect("frame anchor should exist");
        app.apply_reducer_intents([GraphIntent::RecordFrameLayoutHint {
            frame: frame_key,
            hint: FrameLayoutHint::SplitTriptych {
                dominant: frame_layout_node_id(&app, a),
                dominant_edge: DominantEdge::Left,
                wings: [frame_layout_node_id(&app, b), frame_layout_node_id(&app, c)],
            },
        }]);

        let (restored_tree, restored_nodes) =
            synthesize_runtime_tree_from_graph_frame(&app, "workspace-graph-triptych")
                .expect("graph-backed frame should synthesize");

        assert_eq!(restored_nodes, vec![a, b, c, d]);

        let root_id = restored_tree.root().expect("tabs root");
        let root_tabs = match restored_tree.tiles.get(root_id) {
            Some(Tile::Container(Container::Tabs(tabs))) => tabs,
            other => panic!("expected tabs root, got {other:?}"),
        };
        assert_eq!(root_tabs.children.len(), 2);

        let hint_tab = root_tabs.children[0];
        let spillover_tab = root_tabs.children[1];
        let triptych = match restored_tree.tiles.get(hint_tab) {
            Some(Tile::Container(Container::Linear(linear))) => linear,
            other => panic!("expected split tab, got {other:?}"),
        };
        assert_eq!(triptych.dir, LinearDir::Horizontal);
        assert_eq!(triptych.children.len(), 2);
        assert_node_tile(&restored_tree, triptych.children[0], a);

        let wings = match restored_tree.tiles.get(triptych.children[1]) {
            Some(Tile::Container(Container::Linear(linear))) => linear,
            other => panic!("expected wing split, got {other:?}"),
        };
        assert_eq!(wings.dir, LinearDir::Vertical);
        assert_eq!(wings.children.len(), 2);
        assert_node_tile(&restored_tree, wings.children[0], b);
        assert_node_tile(&restored_tree, wings.children[1], c);
        assert_node_tile(&restored_tree, spillover_tab, d);
    }

    #[test]
    fn synthesize_runtime_tree_from_graph_frame_skips_stale_layout_hints() {
        let dir = TempDir::new().unwrap();
        let mut app = GraphBrowserApp::new_from_dir(dir.path().to_path_buf());
        let a = app.add_node_and_sync("https://stale-a.example".into(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("https://stale-b.example".into(), Point2D::new(1.0, 0.0));
        let c = app.add_node_and_sync("https://stale-c.example".into(), Point2D::new(2.0, 0.0));

        let mut tiles = Tiles::default();
        let a_tile = tiles.insert_pane(TileKind::Node(a.into()));
        let b_tile = tiles.insert_pane(TileKind::Node(b.into()));
        let c_tile = tiles.insert_pane(TileKind::Node(c.into()));
        let root = tiles.insert_tab_tile(vec![a_tile, b_tile, c_tile]);
        let tree = Tree::new("frame_graph_stale_hint_members", root, tiles);
        app.sync_named_workbench_frame_graph_representation("workspace-graph-stale-hint", &tree);

        let frame_url = VersoAddress::frame("workspace-graph-stale-hint").to_string();
        let (frame_key, _) = app
            .domain_graph()
            .get_node_by_url(&frame_url)
            .expect("frame anchor should exist");
        app.apply_reducer_intents([GraphIntent::RecordFrameLayoutHint {
            frame: frame_key,
            hint: FrameLayoutHint::SplitHalf {
                first: frame_layout_node_id(&app, a),
                second: Uuid::new_v4().to_string(),
                orientation: SplitOrientation::Horizontal,
            },
        }]);

        let (restored_tree, restored_nodes) =
            synthesize_runtime_tree_from_graph_frame(&app, "workspace-graph-stale-hint")
                .expect("graph-backed frame should synthesize");

        assert_eq!(restored_nodes, vec![a, b, c]);

        let root_id = restored_tree.root().expect("tabs root");
        let root_tabs = match restored_tree.tiles.get(root_id) {
            Some(Tile::Container(Container::Tabs(tabs))) => tabs,
            other => panic!("expected tabs root, got {other:?}"),
        };
        assert_eq!(root_tabs.children.len(), 3);
        assert_node_tile(&restored_tree, root_tabs.children[0], a);
        assert_node_tile(&restored_tree, root_tabs.children[1], b);
        assert_node_tile(&restored_tree, root_tabs.children[2], c);
    }

    #[test]
    fn save_named_frame_bundle_records_triptych_hint_from_runtime_tree() {
        let dir = TempDir::new().unwrap();
        let mut app = GraphBrowserApp::new_from_dir(dir.path().to_path_buf());
        let a = app.add_node_and_sync("https://record-a.example".into(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("https://record-b.example".into(), Point2D::new(1.0, 0.0));
        let c = app.add_node_and_sync("https://record-c.example".into(), Point2D::new(2.0, 0.0));
        let d = app.add_node_and_sync("https://record-d.example".into(), Point2D::new(3.0, 0.0));

        let mut flat_tiles = Tiles::default();
        let flat_a = flat_tiles.insert_pane(TileKind::Node(a.into()));
        let flat_b = flat_tiles.insert_pane(TileKind::Node(b.into()));
        let flat_c = flat_tiles.insert_pane(TileKind::Node(c.into()));
        let flat_d = flat_tiles.insert_pane(TileKind::Node(d.into()));
        let root = flat_tiles.insert_tab_tile(vec![flat_a, flat_b, flat_c, flat_d]);
        let flat_tree = Tree::new("frame_record_seed", root, flat_tiles);
        app.sync_named_workbench_frame_graph_representation(
            "workspace-record-triptych",
            &flat_tree,
        );

        let mut tiles = Tiles::default();
        let dominant = tiles.insert_pane(TileKind::Node(a.into()));
        let wing_a = tiles.insert_pane(TileKind::Node(b.into()));
        let wing_b = tiles.insert_pane(TileKind::Node(c.into()));
        let spillover = tiles.insert_pane(TileKind::Node(d.into()));
        let wings = tiles.insert_vertical_tile(vec![wing_a, wing_b]);
        let triptych = tiles.insert_horizontal_tile(vec![wings, dominant]);
        let root = tiles.insert_tab_tile(vec![triptych, spillover]);
        let tree = Tree::new("frame_record_triptych", root, tiles);

        save_named_frame_bundle(&mut app, "workspace-record-triptych", &tree)
            .expect("save frame bundle");

        let hints = app
            .domain_graph()
            .frame_layout_hints(frame_key_by_name(&app, "workspace-record-triptych"))
            .expect("recorded hints should exist");
        assert_eq!(
            hints,
            &[FrameLayoutHint::SplitTriptych {
                dominant: frame_layout_node_id(&app, a),
                dominant_edge: DominantEdge::Right,
                wings: [frame_layout_node_id(&app, b), frame_layout_node_id(&app, c)],
            }]
        );
    }

    #[test]
    fn frame_layout_sync_intents_for_current_frame_remove_deleted_split_tabs() {
        let dir = TempDir::new().unwrap();
        let mut app = GraphBrowserApp::new_from_dir(dir.path().to_path_buf());
        let a = app.add_node_and_sync("https://delete-a.example".into(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("https://delete-b.example".into(), Point2D::new(1.0, 0.0));
        let c = app.add_node_and_sync("https://delete-c.example".into(), Point2D::new(2.0, 0.0));

        let mut seeded_tiles = Tiles::default();
        let seeded_a = seeded_tiles.insert_pane(TileKind::Node(a.into()));
        let seeded_b = seeded_tiles.insert_pane(TileKind::Node(b.into()));
        let seeded_c = seeded_tiles.insert_pane(TileKind::Node(c.into()));
        let root = seeded_tiles.insert_tab_tile(vec![seeded_a, seeded_b, seeded_c]);
        let seeded_tree = Tree::new("frame_delete_seed", root, seeded_tiles);
        app.sync_named_workbench_frame_graph_representation("workspace-delete-split", &seeded_tree);
        let frame_key = frame_key_by_name(&app, "workspace-delete-split");
        app.apply_reducer_intents([GraphIntent::RecordFrameLayoutHint {
            frame: frame_key,
            hint: FrameLayoutHint::SplitHalf {
                first: frame_layout_node_id(&app, a),
                second: frame_layout_node_id(&app, b),
                orientation: SplitOrientation::Vertical,
            },
        }]);
        app.note_frame_activated("workspace-delete-split", [a, b, c]);

        let mut tiles = Tiles::default();
        let tile_a = tiles.insert_pane(TileKind::Node(a.into()));
        let tile_b = tiles.insert_pane(TileKind::Node(b.into()));
        let tile_c = tiles.insert_pane(TileKind::Node(c.into()));
        let root = tiles.insert_tab_tile(vec![tile_a, tile_b, tile_c]);
        let tree = Tree::new("frame_delete_split", root, tiles);

        let intents = frame_layout_sync_intents_for_current_frame(&app, &tree);
        assert_eq!(intents.len(), 1);
        match &intents[0] {
            GraphIntent::RemoveFrameLayoutHint { frame, hint_index } => {
                assert_eq!(*frame, frame_key);
                assert_eq!(*hint_index, 0);
            }
            other => panic!("expected RemoveFrameLayoutHint, got {other:?}"),
        }
    }

    #[test]
    fn frame_layout_sync_intents_for_registered_frame_groups_records_new_split_tabs() {
        let dir = TempDir::new().unwrap();
        let mut app = GraphBrowserApp::new_from_dir(dir.path().to_path_buf());
        let a = app.add_node_and_sync(
            "https://registered-a.example".into(),
            Point2D::new(0.0, 0.0),
        );
        let b = app.add_node_and_sync(
            "https://registered-b.example".into(),
            Point2D::new(1.0, 0.0),
        );
        let c = app.add_node_and_sync(
            "https://registered-c.example".into(),
            Point2D::new(2.0, 0.0),
        );

        let mut seeded_tiles = Tiles::default();
        let seeded_a = seeded_tiles.insert_pane(TileKind::Node(a.into()));
        let seeded_b = seeded_tiles.insert_pane(TileKind::Node(b.into()));
        let seeded_c = seeded_tiles.insert_pane(TileKind::Node(c.into()));
        let seeded_root = seeded_tiles.insert_tab_tile(vec![seeded_a, seeded_b, seeded_c]);
        let seeded_tree = Tree::new("frame_registered_seed", seeded_root, seeded_tiles);
        let frame_key = app.sync_named_workbench_frame_graph_representation(
            "workspace-registered-split",
            &seeded_tree,
        );

        let mut tiles = Tiles::default();
        let tile_a = tiles.insert_pane(TileKind::Node(a.into()));
        let tile_b = tiles.insert_pane(TileKind::Node(b.into()));
        let tile_c = tiles.insert_pane(TileKind::Node(c.into()));
        let split = tiles.insert_horizontal_tile(vec![tile_a, tile_b]);
        let root = tiles.insert_tab_tile(vec![split, tile_c]);
        let tree = Tree::new("frame_registered_split", root, tiles);

        register_frame_tile_group_runtime(&mut app, &tree, root, frame_key);
        let intents = frame_layout_sync_intents_for_registered_frame_groups(&app, &tree);

        assert_eq!(intents.len(), 1);
        match &intents[0] {
            GraphIntent::RecordFrameLayoutHint { frame, hint } => {
                assert_eq!(*frame, frame_key);
                assert_eq!(
                    *hint,
                    FrameLayoutHint::SplitHalf {
                        first: frame_layout_node_id(&app, a),
                        second: frame_layout_node_id(&app, b),
                        orientation: SplitOrientation::Vertical,
                    }
                );
            }
            other => panic!("expected RecordFrameLayoutHint, got {other:?}"),
        }
    }

    #[test]
    fn test_frame_bundle_round_trips_workbench_profile_layout_constraints() {
        let dir = TempDir::new().unwrap();
        let mut app = GraphBrowserApp::new_from_dir(dir.path().to_path_buf());
        app.set_workbench_layout_constraint(
            crate::app::SurfaceHostId::Navigator(
                crate::app::workbench_layout_policy::NavigatorHostId::Top,
            ),
            crate::app::WorkbenchLayoutConstraint::anchored_split(
                crate::app::SurfaceHostId::Navigator(
                    crate::app::workbench_layout_policy::NavigatorHostId::Top,
                ),
                crate::app::workbench_layout_policy::AnchorEdge::Top,
                0.25,
            ),
        );

        let tree: Tree<TileKind> = serde_json::from_str(&workspace_layout_json_with_nodes(&[]))
            .expect("frame tree should deserialize");

        save_named_workspace_bundle(&mut app, "workspace-layout-profile", &tree)
            .expect("frame bundle should save");

        let bundle = load_named_workspace_bundle(&app, "workspace-layout-profile")
            .expect("frame bundle should load");
        let constraint = bundle
            .workbench_profile
            .layout_constraints
            .get(&crate::app::SurfaceHostId::Navigator(
                crate::app::workbench_layout_policy::NavigatorHostId::Top,
            ))
            .expect("layout constraint should round trip");

        assert!(matches!(
            constraint,
            crate::app::WorkbenchLayoutConstraint::AnchoredSplit {
                anchor_edge: crate::app::workbench_layout_policy::AnchorEdge::Top,
                ..
            }
        ));

        let mut restored_app = GraphBrowserApp::new_from_dir(dir.path().to_path_buf());
        apply_workbench_profile_from_bundle(&mut restored_app, &bundle);
        assert!(
            restored_app
                .workspace
                .workbench_session
                .active_layout_constraints
                .contains_key(&crate::app::SurfaceHostId::Navigator(
                    crate::app::workbench_layout_policy::NavigatorHostId::Top,
                ))
        );
    }

    #[test]
    fn save_named_frame_bundle_persists_graph_frame_node_and_member_edges() {
        let dir = TempDir::new().unwrap();
        let mut app = GraphBrowserApp::new_from_dir(dir.path().to_path_buf());
        let node = app.add_node_and_sync(
            "https://frame-member.example".into(),
            Point2D::new(0.0, 0.0),
        );
        let view_id = GraphViewId::default();

        let mut tiles = Tiles::default();
        let graph = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(view_id)));
        let node_pane = tiles.insert_pane(TileKind::Node(node.into()));
        let root = tiles.insert_tab_tile(vec![graph, node_pane]);
        let tree = Tree::new("workspace-frame-graph-sync", root, tiles);

        save_named_frame_bundle(&mut app, "workspace-frame-graph-sync", &tree)
            .expect("save frame bundle");

        let frame_url = VersoAddress::frame("workspace-frame-graph-sync").to_string();
        let (frame_key, frame_node) = app
            .domain_graph()
            .get_node_by_url(&frame_url)
            .expect("frame node should be created");
        assert_eq!(frame_node.title, "workspace-frame-graph-sync");
        let view_url = VersoAddress::view(view_id.as_uuid().to_string()).to_string();
        let (view_key, _) = app
            .domain_graph()
            .get_node_by_url(&view_url)
            .expect("graph view member node should be created");
        assert!(app.domain_graph().arrangement_edges().any(|edge| {
            edge.sub_kind == crate::graph::ArrangementSubKind::FrameMember
                && edge.from == frame_key
                && edge.to == view_key
        }));
        assert!(app.domain_graph().arrangement_edges().any(|edge| {
            edge.sub_kind == crate::graph::ArrangementSubKind::FrameMember
                && edge.from == frame_key
                && edge.to == node
        }));
    }

    #[test]
    fn save_named_frame_bundle_publishes_projection_refresh_signal() {
        use std::sync::Arc;
        use std::sync::Mutex;

        let dir = TempDir::new().unwrap();
        let mut app = GraphBrowserApp::new_from_dir(dir.path().to_path_buf());
        let observed = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::clone(&observed);
        let observer_id = crate::shell::desktop::runtime::registries::phase3_subscribe_signal(
            crate::shell::desktop::runtime::registries::signal_routing::SignalTopic::RegistryEvent,
            move |signal| {
                if let crate::shell::desktop::runtime::registries::signal_routing::SignalKind::RegistryEvent(
                        crate::shell::desktop::runtime::registries::signal_routing::RegistryEventSignal::WorkbenchProjectionRefreshRequested {
                            reason,
                        },
                    ) = &signal.kind
                    {
                        seen.lock()
                            .expect("observer lock poisoned")
                            .push(reason.clone());
                    }
                Ok(())
            },
        );

        let node = app.add_node_and_sync(
            "https://frame-refresh.example".into(),
            Point2D::new(0.0, 0.0),
        );
        let mut tiles = Tiles::default();
        let graph = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(GraphViewId::default())));
        let node_pane = tiles.insert_pane(TileKind::Node(node.into()));
        let root = tiles.insert_tab_tile(vec![graph, node_pane]);
        let tree = Tree::new("workspace-frame-refresh", root, tiles);

        save_named_frame_bundle(&mut app, "workspace-frame-refresh", &tree)
            .expect("save frame bundle");

        assert!(
            observed
                .lock()
                .expect("observer lock poisoned")
                .iter()
                .any(|reason| reason == "frame_snapshot_saved")
        );
        assert!(crate::shell::desktop::runtime::registries::phase3_unsubscribe_signal(
            crate::shell::desktop::runtime::registries::signal_routing::SignalTopic::RegistryEvent,
            observer_id,
        ));
    }

    #[test]
    fn refresh_workbench_projection_from_manifests_updates_navigator_rows_and_arrangement_projection()
     {
        let dir = TempDir::new().unwrap();
        let mut app = GraphBrowserApp::new_from_dir(dir.path().to_path_buf());
        let node = app.add_node_and_sync(
            "https://refresh-projection.example".into(),
            Point2D::new(0.0, 0.0),
        );
        let view_id = GraphViewId::default();
        app.ensure_graph_view_registered(view_id);

        let mut tiles = Tiles::default();
        let graph = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(view_id)));
        let node_pane = tiles.insert_pane(TileKind::Node(node.into()));
        let root = tiles.insert_tab_tile(vec![graph, node_pane]);
        let tree = Tree::new("workspace-refresh-projection", root, tiles);

        save_named_frame_bundle(&mut app, "workspace-refresh-projection", &tree)
            .expect("save frame bundle");
        app.remove_named_workbench_frame_graph_representation("workspace-refresh-projection");

        assert!(
            app.arrangement_projection_groups()
                .into_iter()
                .all(|group| {
                    group.sub_kind != crate::graph::ArrangementSubKind::FrameMember
                        || group.title != "workspace-refresh-projection"
                })
        );

        app.set_navigator_projection_seed_source(
            crate::app::NavigatorProjectionSeedSource::ContainmentRelations,
        );

        app.apply_reducer_intents([GraphIntent::SetNodeUrl {
            key: node,
            new_url: "file:///docs/workspace-refresh-projection.md".to_string(),
        }]);
        // Add a parent-path node so a ContainmentRelation(UrlPath) edge is created,
        // which is what produces "folder:" keys in the navigator projection.
        app.add_node_and_sync("file:///docs/".to_string(), Point2D::new(10.0, 0.0));

        refresh_workbench_projection_from_manifests(&mut app)
            .expect("refresh workbench projection should succeed");

        assert!(
            app.navigator_projection_state()
                .row_targets
                .keys()
                .any(|row| row.starts_with("folder:")),
            "refresh should rebuild navigator projection rows"
        );

        let projection = WorkbenchChromeProjection::from_tree(&app, &tree, None);
        assert!(projection.pane_entries.iter().any(|entry| {
            entry
                .arrangement_memberships
                .iter()
                .any(|membership| membership == "Frame: workspace-refresh-projection")
        }));
    }

    #[test]
    fn delete_workspace_layout_removes_graph_frame_node() {
        let dir = TempDir::new().unwrap();
        let mut app = GraphBrowserApp::new_from_dir(dir.path().to_path_buf());
        let node = app.add_node_and_sync(
            "https://frame-delete.example".into(),
            Point2D::new(0.0, 0.0),
        );

        let mut tiles = Tiles::default();
        let graph = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(GraphViewId::default())));
        let node_pane = tiles.insert_pane(TileKind::Node(node.into()));
        let root = tiles.insert_tab_tile(vec![graph, node_pane]);
        let tree = Tree::new("workspace-frame-delete", root, tiles);

        save_named_frame_bundle(&mut app, "workspace-frame-delete", &tree).expect("save frame");
        app.delete_workspace_layout("workspace-frame-delete")
            .expect("delete frame snapshot");

        let frame_url = VersoAddress::frame("workspace-frame-delete").to_string();
        assert!(app.domain_graph().get_node_by_url(&frame_url).is_none());
    }

    #[test]
    fn delete_workspace_layout_publishes_projection_refresh_signal() {
        use std::sync::Arc;
        use std::sync::Mutex;

        let dir = TempDir::new().unwrap();
        let mut app = GraphBrowserApp::new_from_dir(dir.path().to_path_buf());
        let observed = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::clone(&observed);
        let observer_id = crate::shell::desktop::runtime::registries::phase3_subscribe_signal(
            crate::shell::desktop::runtime::registries::signal_routing::SignalTopic::RegistryEvent,
            move |signal| {
                if let crate::shell::desktop::runtime::registries::signal_routing::SignalKind::RegistryEvent(
                        crate::shell::desktop::runtime::registries::signal_routing::RegistryEventSignal::WorkbenchProjectionRefreshRequested {
                            reason,
                        },
                    ) = &signal.kind
                    {
                        seen.lock()
                            .expect("observer lock poisoned")
                            .push(reason.clone());
                    }
                Ok(())
            },
        );

        let node = app.add_node_and_sync(
            "https://frame-delete-signal.example".into(),
            Point2D::new(0.0, 0.0),
        );
        let mut tiles = Tiles::default();
        let graph = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(GraphViewId::default())));
        let node_pane = tiles.insert_pane(TileKind::Node(node.into()));
        let root = tiles.insert_tab_tile(vec![graph, node_pane]);
        let tree = Tree::new("workspace-frame-delete-signal", root, tiles);

        save_named_frame_bundle(&mut app, "workspace-frame-delete-signal", &tree)
            .expect("save frame");
        observed.lock().expect("observer lock poisoned").clear();

        app.delete_workspace_layout("workspace-frame-delete-signal")
            .expect("delete frame snapshot");

        assert!(
            observed
                .lock()
                .expect("observer lock poisoned")
                .iter()
                .any(|reason| reason == "frame_snapshot_deleted")
        );
        assert!(crate::shell::desktop::runtime::registries::phase3_unsubscribe_signal(
            crate::shell::desktop::runtime::registries::signal_routing::SignalTopic::RegistryEvent,
            observer_id,
        ));
    }

    #[test]
    fn test_restore_runtime_tree_accepts_legacy_diagnostic_layout_tile() {
        let dir = TempDir::new().unwrap();
        let app = GraphBrowserApp::new_from_dir(dir.path().to_path_buf());

        let mut tiles = Tiles::default();
        let graph = tiles.insert_pane(PersistedPaneTile::Graph);
        let legacy_diagnostic = tiles.insert_pane(PersistedPaneTile::LegacyDiagnostic);
        let root = tiles.insert_tab_tile(vec![graph, legacy_diagnostic]);
        let layout_tree = Tree::new("legacy_diagnostic_bundle", root, tiles);

        let bundle = PersistedWorkspace {
            version: 1,
            name: "legacy-diagnostic".to_string(),
            layout: WorkspaceLayout { tree: layout_tree },
            manifest: WorkspaceManifest {
                panes: BTreeMap::new(),
                member_node_uuids: BTreeSet::new(),
            },
            frame_tab_semantics: None,
            metadata: WorkspaceMetadata {
                created_at_ms: 1,
                updated_at_ms: 1,
                last_activated_at_ms: None,
            },
            workbench_profile: WorkbenchProfile::default(),
        };

        let (restored, _) = restore_runtime_tree_from_workspace_bundle(&app, &bundle)
            .expect("legacy diagnostic layout should restore");

        let has_graph = restored
            .tiles
            .iter()
            .any(|(_, tile)| matches!(tile, Tile::Pane(TileKind::Graph(_))));
        assert!(has_graph);

        #[cfg(feature = "diagnostics")]
        {
            let has_tool = restored.tiles.iter().any(|(_, tile)| {
                matches!(
                    tile,
                    Tile::Pane(TileKind::Tool(tool)) if tool.kind == ToolPaneState::Diagnostics
                )
            });
            assert!(
                has_tool,
                "legacy diagnostic tile should map to Tool::Diagnostics"
            );
        }
    }

    #[test]
    fn test_load_named_workspace_bundle_accepts_legacy_webviewnode_alias() {
        let dir = TempDir::new().unwrap();
        let mut app = GraphBrowserApp::new_from_dir(dir.path().to_path_buf());
        let node_key =
            app.add_node_and_sync("https://legacy.example".into(), Point2D::new(0.0, 0.0));
        let node_uuid = app.workspace.domain.graph.get_node(node_key).unwrap().id;

        let mut runtime_tiles = Tiles::default();
        let graph =
            runtime_tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(GraphViewId::default())));
        let node = runtime_tiles.insert_pane(TileKind::Node(node_key.into()));
        let root = runtime_tiles.insert_tab_tile(vec![graph, node]);
        let runtime_tree = Tree::new("workspace-legacy-alias", root, runtime_tiles);

        let canonical_json =
            serialize_named_workspace_bundle(&app, "workspace-legacy-alias", &runtime_tree)
                .expect("canonical frame bundle should serialize");
        let bundle_json = canonical_json.replace("\"NodePane\"", "\"WebViewNode\"");

        app.save_workspace_layout_json("workspace-legacy-alias", &bundle_json);

        let loaded = load_named_workspace_bundle(&app, "workspace-legacy-alias")
            .expect("legacy WebViewNode alias should deserialize");
        assert!(matches!(
            loaded.manifest.panes.get(&2),
            Some(PaneContent::NodePane { node_uuid: id }) if *id == node_uuid
        ));
    }

    #[test]
    fn save_named_frame_bundle_derives_frame_tab_semantics_for_tabs() {
        let dir = TempDir::new().unwrap();
        let mut app = GraphBrowserApp::new_from_dir(dir.path().to_path_buf());
        let a = app.add_node_and_sync("https://tabs-a.example".into(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("https://tabs-b.example".into(), Point2D::new(20.0, 0.0));

        let mut tiles = Tiles::default();
        let graph = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(GraphViewId::default())));
        let pane_a = tiles.insert_pane(TileKind::Node(a.into()));
        let pane_b = tiles.insert_pane(TileKind::Node(b.into()));
        let root = tiles.insert_tab_tile(vec![graph, pane_a, pane_b]);
        if let Some(Tile::Container(Container::Tabs(tabs))) = tiles.get_mut(root) {
            tabs.set_active(pane_b);
        }
        let tree = Tree::new("workspace-frame-semantics", root, tiles);

        save_named_frame_bundle(&mut app, "workspace-frame-semantics", &tree)
            .expect("save frame bundle");
        let loaded =
            load_named_frame_bundle(&app, "workspace-frame-semantics").expect("load frame bundle");

        let semantics = loaded
            .frame_tab_semantics
            .as_ref()
            .expect("tab semantics should be persisted");
        assert_eq!(semantics.version, FRAME_TAB_SEMANTICS_VERSION);
        assert_eq!(semantics.tab_groups.len(), 1);
        let group = &semantics.tab_groups[0];
        assert_eq!(
            group.pane_ids.len(),
            3,
            "graph and node panes should be tracked"
        );
        assert_eq!(group.active_pane_id, Some(3));
        assert!(matches!(
            loaded.manifest.panes.get(&1),
            Some(PaneContent::Graph)
        ));
        assert!(matches!(
            loaded.manifest.panes.get(&2),
            Some(PaneContent::NodePane { .. })
        ));
        assert!(matches!(
            loaded.manifest.panes.get(&3),
            Some(PaneContent::NodePane { .. })
        ));
    }

    #[test]
    fn load_named_frame_bundle_repairs_invalid_frame_tab_semantics() {
        let dir = TempDir::new().unwrap();
        let mut app = GraphBrowserApp::new_from_dir(dir.path().to_path_buf());
        let node_key =
            app.add_node_and_sync("https://repair-semantics.example".into(), Point2D::zero());
        let node_uuid = app
            .workspace
            .domain
            .graph
            .get_node(node_key)
            .expect("node")
            .id;

        let mut tiles = Tiles::default();
        let root = tiles.insert_pane(PersistedPaneTile::Pane(1));
        let bundle = PersistedWorkspace {
            version: 1,
            name: "repair-semantics".to_string(),
            layout: WorkspaceLayout {
                tree: Tree::new("repair-semantics-tree", root, tiles),
            },
            manifest: WorkspaceManifest {
                panes: BTreeMap::from([(1, PaneContent::NodePane { node_uuid })]),
                member_node_uuids: BTreeSet::from([node_uuid]),
            },
            frame_tab_semantics: Some(FrameTabSemantics {
                version: 0,
                tab_groups: vec![
                    TabGroupMetadata {
                        group_id: Uuid::new_v4(),
                        pane_ids: vec![1, 1, 999],
                        active_pane_id: Some(999),
                    },
                    TabGroupMetadata {
                        group_id: Uuid::new_v4(),
                        pane_ids: vec![1],
                        active_pane_id: Some(1),
                    },
                ],
            }),
            metadata: WorkspaceMetadata {
                created_at_ms: 1,
                updated_at_ms: 1,
                last_activated_at_ms: None,
            },
            workbench_profile: WorkbenchProfile::default(),
        };
        app.save_workspace_layout_json(
            "repair-semantics",
            &serde_json::to_string(&bundle).expect("serialize bundle"),
        );

        let loaded = load_named_frame_bundle(&app, "repair-semantics").expect("load frame bundle");
        let semantics = loaded
            .frame_tab_semantics
            .as_ref()
            .expect("repair should preserve non-empty semantics");
        assert_eq!(semantics.version, FRAME_TAB_SEMANTICS_VERSION);
        assert_eq!(semantics.tab_groups.len(), 1);
        assert_eq!(semantics.tab_groups[0].pane_ids, vec![1]);
        assert_eq!(semantics.tab_groups[0].active_pane_id, None);
    }

    #[test]
    fn restore_runtime_tree_from_frame_bundle_rewraps_pane_rest_semantic_tabs() {
        let dir = TempDir::new().unwrap();
        let mut app = GraphBrowserApp::new_from_dir(dir.path().to_path_buf());
        let a = app.add_node_and_sync("https://pane-rest-a.example".into(), Point2D::zero());
        let b = app.add_node_and_sync(
            "https://pane-rest-b.example".into(),
            Point2D::new(16.0, 0.0),
        );
        let a_uuid = app.workspace.domain.graph.get_node(a).expect("node a").id;
        let b_uuid = app.workspace.domain.graph.get_node(b).expect("node b").id;

        let mut tiles = Tiles::default();
        let root = tiles.insert_pane(PersistedPaneTile::Pane(1));
        let bundle = PersistedWorkspace {
            version: 1,
            name: "pane-rest-restore".to_string(),
            layout: WorkspaceLayout {
                tree: Tree::new("pane-rest-restore-tree", root, tiles),
            },
            manifest: WorkspaceManifest {
                panes: BTreeMap::from([
                    (1, PaneContent::NodePane { node_uuid: a_uuid }),
                    (2, PaneContent::NodePane { node_uuid: b_uuid }),
                ]),
                member_node_uuids: BTreeSet::from([a_uuid, b_uuid]),
            },
            frame_tab_semantics: Some(FrameTabSemantics {
                version: FRAME_TAB_SEMANTICS_VERSION,
                tab_groups: vec![TabGroupMetadata {
                    group_id: Uuid::new_v4(),
                    pane_ids: vec![1, 2],
                    active_pane_id: Some(2),
                }],
            }),
            metadata: WorkspaceMetadata {
                created_at_ms: 1,
                updated_at_ms: 1,
                last_activated_at_ms: None,
            },
            workbench_profile: WorkbenchProfile::default(),
        };

        let (restored_tree, restored_nodes) =
            restore_runtime_tree_from_frame_bundle(&app, &bundle).expect("restore frame bundle");

        assert!(restored_nodes.contains(&a));
        assert!(restored_nodes.contains(&b));

        let root_id = restored_tree.root().expect("restored root");
        let tabs = match restored_tree.tiles.get(root_id) {
            Some(Tile::Container(Container::Tabs(tabs))) => tabs,
            other => panic!("expected restored semantic tabs at root, got {other:?}"),
        };
        assert_eq!(tabs.children.len(), 2);

        let restored_members: Vec<_> = tabs
            .children
            .iter()
            .filter_map(|child| match restored_tree.tiles.get(*child) {
                Some(Tile::Pane(TileKind::Node(state))) => Some(state.node),
                _ => None,
            })
            .collect();
        assert_eq!(restored_members, vec![a, b]);

        let active_node =
            tabs.active
                .and_then(|active_tile| match restored_tree.tiles.get(active_tile) {
                    Some(Tile::Pane(TileKind::Node(state))) => Some(state.node),
                    _ => None,
                });
        assert_eq!(active_node, Some(b));
    }

    #[test]
    fn repair_named_frame_tab_semantics_persists_repaired_overlay() {
        let dir = TempDir::new().unwrap();
        let mut app = GraphBrowserApp::new_from_dir(dir.path().to_path_buf());
        let node_key =
            app.add_node_and_sync("https://persist-repair.example".into(), Point2D::zero());
        let node_uuid = app
            .workspace
            .domain
            .graph
            .get_node(node_key)
            .expect("node")
            .id;

        let mut tiles = Tiles::default();
        let root = tiles.insert_pane(PersistedPaneTile::Pane(1));
        let bundle = PersistedWorkspace {
            version: 1,
            name: "persist-repair-semantics".to_string(),
            layout: WorkspaceLayout {
                tree: Tree::new("persist-repair-tree", root, tiles),
            },
            manifest: WorkspaceManifest {
                panes: BTreeMap::from([(1, PaneContent::NodePane { node_uuid })]),
                member_node_uuids: BTreeSet::from([node_uuid]),
            },
            frame_tab_semantics: Some(FrameTabSemantics {
                version: 0,
                tab_groups: vec![
                    TabGroupMetadata {
                        group_id: Uuid::new_v4(),
                        pane_ids: vec![1, 1, 999],
                        active_pane_id: Some(999),
                    },
                    TabGroupMetadata {
                        group_id: Uuid::new_v4(),
                        pane_ids: vec![1],
                        active_pane_id: Some(1),
                    },
                ],
            }),
            metadata: WorkspaceMetadata {
                created_at_ms: 1,
                updated_at_ms: 1,
                last_activated_at_ms: None,
            },
            workbench_profile: WorkbenchProfile::default(),
        };
        app.save_workspace_layout_json(
            "persist-repair-semantics",
            &serde_json::to_string(&bundle).expect("serialize bundle"),
        );

        let report = repair_named_frame_tab_semantics(&mut app, "persist-repair-semantics")
            .expect("repair named frame semantics");
        assert!(report.has_repairs());
        assert!(matches!(
            report.repairs.first(),
            Some(FrameTabSemanticsRepair::UpgradedVersion {
                from: 0,
                to: FRAME_TAB_SEMANTICS_VERSION
            })
        ));
        assert!(report.repairs.iter().any(|repair| {
            matches!(
                repair,
                FrameTabSemanticsRepair::DroppedDuplicatePaneWithinGroup {
                    pane_id: 1,
                    ..
                }
            )
        }));
        assert!(report.repairs.iter().any(|repair| {
            matches!(
                repair,
                FrameTabSemanticsRepair::DroppedMissingPaneId {
                    pane_id: 999,
                    ..
                }
            )
        }));
        assert!(report.repairs.iter().any(|repair| {
            matches!(
                repair,
                FrameTabSemanticsRepair::RemovedLaterDuplicateMembership {
                    pane_id: 1,
                    ..
                }
            )
        }));
        assert!(report.user_warning.is_some());

        let persisted_json = app
            .load_workspace_layout_json("persist-repair-semantics")
            .expect("persisted frame json");
        let persisted: PersistedWorkspace =
            serde_json::from_str(&persisted_json).expect("deserialize repaired bundle");
        let semantics = persisted
            .frame_tab_semantics
            .as_ref()
            .expect("repaired semantics should remain present");
        assert_eq!(semantics.version, FRAME_TAB_SEMANTICS_VERSION);
        assert_eq!(semantics.tab_groups.len(), 1);
        assert_eq!(semantics.tab_groups[0].pane_ids, vec![1]);
        assert_eq!(semantics.tab_groups[0].active_pane_id, None);
    }

    #[test]
    fn repair_named_frame_tab_semantics_reports_manifest_repair_once() {
        let dir = TempDir::new().unwrap();
        let mut app = GraphBrowserApp::new_from_dir(dir.path().to_path_buf());
        let node_key =
            app.add_node_and_sync("https://manifest-repair.example".into(), Point2D::zero());
        let node_uuid = app
            .workspace
            .domain
            .graph
            .get_node(node_key)
            .expect("node")
            .id;

        let mut tiles = Tiles::default();
        let root = tiles.insert_pane(PersistedPaneTile::Pane(1));
        let bundle = PersistedWorkspace {
            version: 1,
            name: "manifest-repair-semantics".to_string(),
            layout: WorkspaceLayout {
                tree: Tree::new("manifest-repair-tree", root, tiles),
            },
            manifest: WorkspaceManifest {
                panes: BTreeMap::from([(1, PaneContent::NodePane { node_uuid })]),
                member_node_uuids: BTreeSet::new(),
            },
            frame_tab_semantics: None,
            metadata: WorkspaceMetadata {
                created_at_ms: 1,
                updated_at_ms: 1,
                last_activated_at_ms: None,
            },
            workbench_profile: WorkbenchProfile::default(),
        };
        app.save_workspace_layout_json(
            "manifest-repair-semantics",
            &serde_json::to_string(&bundle).expect("serialize bundle"),
        );

        let report = repair_named_frame_tab_semantics(&mut app, "manifest-repair-semantics")
            .expect("repair named frame semantics");

        assert_eq!(
            report.repairs,
            vec![FrameTabSemanticsRepair::RepairedManifestMembership]
        );
        assert_eq!(
            report.user_warning,
            Some(
                "Frame 'manifest-repair-semantics': repaired semantic tab metadata: repaired manifest membership before semantic tab repair"
                    .to_string()
            )
        );
    }

    #[test]
    fn save_named_frame_bundle_preserves_collapsed_runtime_semantic_tabs() {
        let dir = TempDir::new().unwrap();
        let mut app = GraphBrowserApp::new_from_dir(dir.path().to_path_buf());
        let a = app.add_node_and_sync("https://collapsed-save-a.example".into(), Point2D::zero());
        let b = app.add_node_and_sync(
            "https://collapsed-save-b.example".into(),
            Point2D::new(24.0, 0.0),
        );

        let mut tiles = Tiles::default();
        let a_tile = tiles.insert_pane(TileKind::Node(a.into()));
        let b_tile = tiles.insert_pane(TileKind::Node(b.into()));
        let root = tiles.insert_tab_tile(vec![a_tile, b_tile]);
        let mut tree = Tree::new("collapsed_runtime_semantics_save", root, tiles);

        let semantics =
            derive_runtime_frame_tab_semantics_from_tree(&tree).expect("runtime semantics");
        let group_id = semantics.tab_groups[0].group_id;
        app.set_current_frame_tab_semantics(Some(semantics));
        assert!(
            crate::shell::desktop::workbench::tile_view_ops::collapse_semantic_tab_group_to_pane_rest(
                &mut tree,
                &mut app,
                group_id,
            )
        );

        save_named_frame_bundle(&mut app, "workspace-collapsed-runtime-semantics", &tree)
            .expect("save frame");

        let bundle = load_named_frame_bundle(&app, "workspace-collapsed-runtime-semantics")
            .expect("load frame");
        let tab_groups = semantic_tab_groups_for_frame(&bundle);
        assert_eq!(tab_groups.len(), 1);
        assert_eq!(tab_groups[0].pane_ids.len(), 2);
        assert_eq!(
            saved_tab_node_keys_for_frame_bundle(&app, &bundle),
            HashSet::from([a, b])
        );
    }
}
