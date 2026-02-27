/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

use egui_tiles::{Tile, Tiles, Tree};
use log::warn;
use servo::{OffscreenRenderingContext, WebViewId};
use uuid::Uuid;

use crate::app::GraphViewId;
use crate::app::{GraphBrowserApp, GraphIntent};
use crate::graph::NodeKey;
use crate::shell::desktop::host::window::EmbedderWindow;
use crate::shell::desktop::lifecycle::webview_controller;
use crate::shell::desktop::workbench::tile_kind::TileKind;
use crate::shell::desktop::workbench::tile_runtime;

/// Persisted pane identifier used inside workspace bundle schema.
///
/// Distinct from runtime `pane_model::PaneId` (UUID-backed) and scoped only to
/// a single serialized layout tree.
pub(crate) type PersistedPaneId = u64;

/// Backward-compatible local alias retained while migrating callsites.
pub(crate) type PaneId = PersistedPaneId;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum PersistedPaneTile {
    Graph,
    Pane(PaneId),
    /// Legacy read-compat for historical workspace layouts that persisted a
    /// diagnostics pane directly in layout tiles.
    ///
    /// This variant is deserialize-only compatibility and is never written by
    /// current bundle serialization. Runtime restore maps it through the
    /// generic `Tool { kind }` pane path.
    #[serde(rename = "Diagnostic")]
    LegacyDiagnostic,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct WorkspaceLayout {
    pub tree: Tree<PersistedPaneTile>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum PaneContent {
    Graph,
    /// Node viewer pane bound to a graph node (viewer backend is resolved at
    /// runtime by `ViewerRegistry`). Serde alias preserves backward-compat
    /// deserialization of workspaces saved before the `WebViewNode` → `NodePane`
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
pub(crate) struct WorkspaceManifest {
    pub panes: BTreeMap<PaneId, PaneContent>,
    pub member_node_uuids: BTreeSet<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct WorkspaceMetadata {
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    pub last_activated_at_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct PersistedWorkspace {
    pub version: u32,
    pub name: String,
    pub layout: WorkspaceLayout,
    pub manifest: WorkspaceManifest,
    pub metadata: WorkspaceMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WorkspaceBundleError {
    MissingManifestPane {
        pane_id: PaneId,
    },
    MembershipMismatch {
        declared: BTreeSet<Uuid>,
        derived: BTreeSet<Uuid>,
    },
}

impl std::fmt::Display for WorkspaceBundleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingManifestPane { pane_id } => {
                write!(
                    f,
                    "workspace layout references missing manifest pane id {pane_id}"
                )
            }
            Self::MembershipMismatch { .. } => {
                write!(
                    f,
                    "workspace manifest declared membership does not match pane-derived membership"
                )
            }
        }
    }
}

impl std::error::Error for WorkspaceBundleError {}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn persisted_layout_referenced_pane_ids(layout: &WorkspaceLayout) -> BTreeSet<PaneId> {
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

pub(crate) fn derive_membership_from_manifest(manifest: &WorkspaceManifest) -> BTreeSet<Uuid> {
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

pub(crate) fn validate_workspace_bundle(
    bundle: &PersistedWorkspace,
) -> Result<(), WorkspaceBundleError> {
    for pane_id in persisted_layout_referenced_pane_ids(&bundle.layout) {
        if !bundle.manifest.panes.contains_key(&pane_id) {
            return Err(WorkspaceBundleError::MissingManifestPane { pane_id });
        }
    }
    let derived = derive_membership_from_manifest(&bundle.manifest);
    if derived != bundle.manifest.member_node_uuids {
        return Err(WorkspaceBundleError::MembershipMismatch {
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
    for (tile_id, tile) in serde_tree.tiles.iter_mut() {
        let Tile::Pane(pane_value) = tile else {
            continue;
        };
        let runtime_pane: TileKind =
            serde_json::from_value(pane_value.clone()).map_err(|e| e.to_string())?;
        let persisted_pane = match runtime_pane {
            TileKind::Graph(_) => PersistedPaneTile::Graph,
            TileKind::Node(state) => {
                let node = graph_app
                    .workspace
                    .graph
                    .get_node(state.node)
                    .ok_or_else(|| {
                        format!("workspace contains stale node key {}", state.node.index())
                    })?;
                let pane_id = tile_id.0;
                panes.insert(pane_id, PaneContent::NodePane { node_uuid: node.id });
                PersistedPaneTile::Pane(pane_id)
            }
            #[cfg(feature = "diagnostics")]
            TileKind::Tool(tool_state) => {
                let pane_id = tile_id.0;
                panes.insert(pane_id, PaneContent::Tool { kind: tool_state });
                PersistedPaneTile::Pane(pane_id)
            }
        };
        *pane_value = serde_json::to_value(persisted_pane).map_err(|e| e.to_string())?;
    }

    let layout_tree: Tree<PersistedPaneTile> =
        serde_json::from_value(serde_json::to_value(serde_tree).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let mut manifest = WorkspaceManifest {
        panes,
        member_node_uuids: BTreeSet::new(),
    };
    manifest.member_node_uuids = derive_membership_from_manifest(&manifest);

    let now = now_unix_ms();
    let metadata = match prior_metadata {
        Some(prior) => WorkspaceMetadata {
            created_at_ms: if prior.created_at_ms == 0 {
                now
            } else {
                prior.created_at_ms
            },
            updated_at_ms: now,
            last_activated_at_ms: prior.last_activated_at_ms,
        },
        None => WorkspaceMetadata {
            created_at_ms: now,
            updated_at_ms: now,
            last_activated_at_ms: None,
        },
    };

    Ok(PersistedWorkspace {
        version: 1,
        name: name.to_string(),
        layout: WorkspaceLayout { tree: layout_tree },
        manifest,
        metadata,
    })
}

pub(crate) fn serialize_named_workspace_bundle(
    graph_app: &GraphBrowserApp,
    name: &str,
    tree: &Tree<TileKind>,
) -> Result<String, String> {
    let prior_metadata = load_named_workspace_bundle(graph_app, name)
        .ok()
        .map(|b| b.metadata);
    let bundle = runtime_tree_to_bundle(graph_app, name, tree, prior_metadata)?;
    serde_json::to_string(&bundle).map_err(|e| e.to_string())
}

pub(crate) fn save_named_workspace_bundle(
    graph_app: &mut GraphBrowserApp,
    name: &str,
    tree: &Tree<TileKind>,
) -> Result<(), String> {
    let bundle_json = serialize_named_workspace_bundle(graph_app, name, tree)?;
    graph_app.save_workspace_layout_json(name, &bundle_json);
    Ok(())
}

pub(crate) fn load_named_workspace_bundle(
    graph_app: &GraphBrowserApp,
    name: &str,
) -> Result<PersistedWorkspace, String> {
    let json = graph_app
        .load_workspace_layout_json(name)
        .ok_or_else(|| format!("workspace '{name}' not found"))?;
    let mut bundle: PersistedWorkspace = serde_json::from_str(&json).map_err(|e| e.to_string())?;
    if let Err(err) = validate_workspace_bundle(&bundle) {
        match err {
            WorkspaceBundleError::MembershipMismatch { .. } => {
                repair_manifest_membership(&mut bundle);
            }
            _ => return Err(err.to_string()),
        }
    }
    Ok(bundle)
}

pub(crate) fn restore_runtime_tree_from_workspace_bundle(
    graph_app: &GraphBrowserApp,
    bundle: &PersistedWorkspace,
) -> Result<(Tree<TileKind>, Vec<NodeKey>), String> {
    let mut repaired = bundle.clone();
    if let Err(err) = validate_workspace_bundle(&repaired) {
        match err {
            WorkspaceBundleError::MembershipMismatch { .. } => {
                repair_manifest_membership(&mut repaired)
            }
            _ => return Err(err.to_string()),
        }
    }

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
            PersistedPaneTile::Graph => Some(TileKind::Graph(GraphViewId::default())),
            PersistedPaneTile::LegacyDiagnostic => {
                #[cfg(feature = "diagnostics")]
                {
                    Some(TileKind::Tool(
                        crate::shell::desktop::workbench::pane_model::ToolPaneState::Diagnostics,
                    ))
                }
                #[cfg(not(feature = "diagnostics"))]
                {
                    missing_tile_ids.push(*tile_id);
                    None
                }
            }
            PersistedPaneTile::Pane(pane_id) => match repaired.manifest.panes.get(&pane_id) {
                Some(PaneContent::Graph) => Some(TileKind::Graph(GraphViewId::default())),
                Some(PaneContent::NodePane { node_uuid }) => {
                    if let Some(node_key) = graph_app.workspace.graph.get_node_key_by_id(*node_uuid)
                    {
                        restored_nodes.push(node_key);
                        Some(TileKind::Node(node_key.into()))
                    } else {
                        missing_tile_ids.push(*tile_id);
                        None
                    }
                }
                Some(PaneContent::Tool { kind }) => {
                    #[cfg(feature = "diagnostics")]
                    {
                        Some(TileKind::Tool(kind.clone()))
                    }
                    #[cfg(not(feature = "diagnostics"))]
                    {
                        let _ = kind;
                        missing_tile_ids.push(*tile_id);
                        None
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
    let has_graph_pane = runtime_tree
        .tiles
        .iter()
        .any(|(_, tile)| matches!(tile, Tile::Pane(TileKind::Graph(_))));
    if restored_nodes.is_empty() && !has_graph_pane {
        runtime_tree.root = None;
    }

    Ok((runtime_tree, restored_nodes))
}

pub(crate) fn build_membership_index_from_workspace_manifests(
    graph_app: &GraphBrowserApp,
) -> HashMap<Uuid, BTreeSet<String>> {
    let mut index: HashMap<Uuid, BTreeSet<String>> = HashMap::new();
    for workspace_name in graph_app.list_workspace_layout_names() {
        if GraphBrowserApp::is_reserved_workspace_layout_name(&workspace_name) {
            continue;
        }
        let Ok(mut bundle) = load_named_workspace_bundle(graph_app, &workspace_name) else {
            continue;
        };
        if validate_workspace_bundle(&bundle).is_err() {
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

pub(crate) fn refresh_workspace_membership_cache_from_manifests(
    graph_app: &mut GraphBrowserApp,
) -> Result<(), String> {
    let membership_index = build_membership_index_from_workspace_manifests(graph_app);
    graph_app.init_membership_index(membership_index);
    Ok(())
}

pub(crate) fn build_workspace_activation_recency_from_workspace_manifests(
    graph_app: &GraphBrowserApp,
) -> (HashMap<Uuid, (u64, String)>, u64) {
    let mut recency = HashMap::new();
    let mut activation_seq = 0u64;

    for workspace_name in graph_app.list_workspace_layout_names() {
        if GraphBrowserApp::is_reserved_workspace_layout_name(&workspace_name) {
            continue;
        }
        let Ok(mut bundle) = load_named_workspace_bundle(graph_app, &workspace_name) else {
            continue;
        };
        if validate_workspace_bundle(&bundle).is_err() {
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

pub(crate) fn mark_named_workspace_bundle_activated(
    graph_app: &mut GraphBrowserApp,
    name: &str,
) -> Result<(), String> {
    let mut bundle = load_named_workspace_bundle(graph_app, name)?;
    let now = now_unix_ms();
    bundle.metadata.updated_at_ms = now;
    bundle.metadata.last_activated_at_ms = Some(now);
    let bundle_json = serde_json::to_string(&bundle).map_err(|e| e.to_string())?;
    graph_app.save_workspace_layout_json(name, &bundle_json);
    Ok(())
}

pub(crate) fn restore_tiles_tree_from_persistence(graph_app: &GraphBrowserApp) -> Tree<TileKind> {
    let mut tiles = Tiles::default();
    let graph_tile_id = tiles.insert_pane(TileKind::Graph(GraphViewId::default()));
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

/// Rebuild the UUID-keyed workspace membership index from persisted named workspace layouts.
///
/// Reserved autosave/session workspace keys are intentionally excluded so routing decisions
/// operate on user-meaningful named workspaces.
pub(crate) fn build_membership_index_from_layouts(
    graph_app: &GraphBrowserApp,
) -> HashMap<Uuid, BTreeSet<String>> {
    let mut index: HashMap<Uuid, BTreeSet<String>> = HashMap::new();

    for workspace_name in graph_app.list_workspace_layout_names() {
        if GraphBrowserApp::is_reserved_workspace_layout_name(&workspace_name) {
            continue;
        }
        let Some(layout_json) = graph_app.load_workspace_layout_json(&workspace_name) else {
            continue;
        };
        let Ok(mut tree) = serde_json::from_str::<Tree<TileKind>>(&layout_json) else {
            warn!("Skipping workspace '{workspace_name}': invalid layout json");
            continue;
        };
        tile_runtime::prune_stale_node_pane_keys_only(&mut tree, graph_app);
        for node_key in workspace_nodes_from_tree(&tree) {
            let Some(node) = graph_app.workspace.graph.get_node(node_key) else {
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

/// Delete named workspaces that become empty after stale-node pruning.
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
            warn!("Skipping workspace '{workspace_name}': invalid layout json");
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

/// Keep only the latest N named workspaces by activation recency.
pub(crate) fn keep_latest_named_workspaces(graph_app: &mut GraphBrowserApp, keep: usize) -> usize {
    let mut names: Vec<String> = graph_app
        .list_workspace_layout_names()
        .into_iter()
        .filter(|name| !GraphBrowserApp::is_reserved_workspace_layout_name(name))
        .collect();
    names.sort_by(|a, b| {
        graph_app
            .workspace_recency_seq_for_name(b)
            .cmp(&graph_app.workspace_recency_seq_for_name(a))
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
    use egui_tiles::{Tiles, Tree};
    use euclid::default::Point2D;
    use tempfile::TempDir;

    fn workspace_layout_json_with_nodes(node_keys: &[NodeKey]) -> String {
        let mut tiles = Tiles::default();
        let mut children = Vec::new();
        children.push(tiles.insert_pane(TileKind::Graph(GraphViewId::default())));
        for node_key in node_keys {
            children.push(tiles.insert_pane(TileKind::Node((*node_key).into())));
        }
        let root = if children.len() == 1 {
            children[0]
        } else {
            tiles.insert_tab_tile(children)
        };
        let tree = Tree::new("workspace_test", root, tiles);
        serde_json::to_string(&tree).expect("workspace layout should serialize")
    }

    #[test]
    fn test_build_membership_index_from_layouts_skips_reserved_and_stale_nodes() {
        let dir = TempDir::new().unwrap();
        let mut app = GraphBrowserApp::new_from_dir(dir.path().to_path_buf());
        let a = app.add_node_and_sync("https://a.example".into(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("https://b.example".into(), Point2D::new(1.0, 0.0));
        let a_id = app.workspace.graph.get_node(a).unwrap().id;
        let b_id = app.workspace.graph.get_node(b).unwrap().id;
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
        let live_id = app.workspace.graph.get_node(live).unwrap().id;
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
        let node_id = app.workspace.graph.get_node(node).unwrap().id;

        app.save_workspace_layout_json("workspace-old", &workspace_layout_json_with_nodes(&[node]));
        app.save_workspace_layout_json("workspace-mid", &workspace_layout_json_with_nodes(&[node]));
        app.save_workspace_layout_json("workspace-new", &workspace_layout_json_with_nodes(&[node]));
        app.note_workspace_activated("workspace-old", [node]);
        app.note_workspace_activated("workspace-mid", [node]);
        app.note_workspace_activated("workspace-new", [node]);

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
    fn test_workspace_bundle_serialization_excludes_diagnostics_payload() {
        let dir = TempDir::new().unwrap();
        let app = GraphBrowserApp::new_from_dir(dir.path().to_path_buf());
        let tree: Tree<TileKind> = serde_json::from_str(&workspace_layout_json_with_nodes(&[]))
            .expect("workspace tree should deserialize");

        let json = serialize_named_workspace_bundle(&app, "workspace-clean", &tree)
            .expect("workspace bundle should serialize");
        let value: serde_json::Value =
            serde_json::from_str(&json).expect("bundle json should parse");
        let root = value.as_object().expect("bundle should be json object");

        assert!(root.contains_key("version"));
        assert!(root.contains_key("name"));
        assert!(root.contains_key("layout"));
        assert!(root.contains_key("manifest"));
        assert!(root.contains_key("metadata"));

        assert!(!root.contains_key("diagnostic_graph"));
        assert!(!root.contains_key("compositor_state"));
        assert!(!root.contains_key("event_ring"));
        assert!(!root.contains_key("channels"));
        assert!(!root.contains_key("spans"));
        assert!(!root.contains_key("recent_intents"));
    }

    #[test]
    fn test_workspace_bundle_serialization_uses_pane_model_terms_not_legacy_aliases() {
        let dir = TempDir::new().unwrap();
        let mut app = GraphBrowserApp::new_from_dir(dir.path().to_path_buf());
        let node = app.add_node_and_sync("https://schema.example".into(), Point2D::new(0.0, 0.0));

        let mut tiles = Tiles::default();
        let graph = tiles.insert_pane(TileKind::Graph(GraphViewId::default()));
        let node_pane = tiles.insert_pane(TileKind::Node(node.into()));
        let root = tiles.insert_tab_tile(vec![graph, node_pane]);
        let tree = Tree::new("workspace-schema-terms", root, tiles);

        let json = serialize_named_workspace_bundle(&app, "workspace-schema-terms", &tree)
            .expect("workspace bundle should serialize");

        assert!(json.contains("\"NodePane\""));
        assert!(!json.contains("\"WebViewNode\""));
        assert!(!json.contains("\"Diagnostic\""));
    }

    #[test]
    fn test_workspace_bundle_payload_stays_clean_after_restart() {
        let dir = TempDir::new().unwrap();
        let data_dir = dir.path().to_path_buf();
        let workspace_name = "workspace-restart-clean";

        {
            let mut app = GraphBrowserApp::new_from_dir(data_dir.clone());
            let node =
                app.add_node_and_sync("https://restart.example".into(), Point2D::new(0.0, 0.0));
            let mut tiles = Tiles::default();
            let graph = tiles.insert_pane(TileKind::Graph(GraphViewId::default()));
            let webview = tiles.insert_pane(TileKind::Node(node.into()));
            let root = tiles.insert_tab_tile(vec![graph, webview]);
            let tree = Tree::new("restart_bundle", root, tiles);

            save_named_workspace_bundle(&mut app, workspace_name, &tree)
                .expect("save workspace bundle");
        }

        let app = GraphBrowserApp::new_from_dir(data_dir);
        let json = app
            .load_workspace_layout_json(workspace_name)
            .expect("workspace bundle json should exist");
        let value: serde_json::Value = serde_json::from_str(&json).expect("bundle json parse");
        let root = value.as_object().expect("bundle should be object");

        assert!(root.contains_key("layout"));
        assert!(root.contains_key("manifest"));
        assert!(root.contains_key("metadata"));
        assert!(!root.contains_key("diagnostic_graph"));
        assert!(!root.contains_key("channels"));
        assert!(!root.contains_key("spans"));
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
            metadata: WorkspaceMetadata {
                created_at_ms: 1,
                updated_at_ms: 1,
                last_activated_at_ms: None,
            },
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
                    Tile::Pane(TileKind::Tool(
                        crate::shell::desktop::workbench::pane_model::ToolPaneState::Diagnostics
                    ))
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
        let node_uuid = app.workspace.graph.get_node(node_key).unwrap().id;

        let mut runtime_tiles = Tiles::default();
        let graph = runtime_tiles.insert_pane(TileKind::Graph(GraphViewId::default()));
        let node = runtime_tiles.insert_pane(TileKind::Node(node_key.into()));
        let root = runtime_tiles.insert_tab_tile(vec![graph, node]);
        let runtime_tree = Tree::new("workspace-legacy-alias", root, runtime_tiles);

        let canonical_json =
            serialize_named_workspace_bundle(&app, "workspace-legacy-alias", &runtime_tree)
                .expect("canonical workspace bundle should serialize");
        let bundle_json = canonical_json.replace("\"NodePane\"", "\"WebViewNode\"");

        app.save_workspace_layout_json("workspace-legacy-alias", &bundle_json);

        let loaded = load_named_workspace_bundle(&app, "workspace-legacy-alias")
            .expect("legacy WebViewNode alias should deserialize");
        assert!(matches!(
            loaded.manifest.panes.get(&2),
            Some(PaneContent::NodePane { node_uuid: id }) if *id == node_uuid
        ));
    }
}
