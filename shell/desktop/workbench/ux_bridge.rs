/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::HashMap;

use egui_tiles::Tree;
use serde::Deserialize;

use super::tile_kind::TileKind;
use super::tile_view_ops;
use super::ux_tree::{
    self, UxAction, UxDomainIdentity, UxNodeRole, UxSemanticNode, UxTreeSnapshot,
};
use crate::app::{GraphBrowserApp, LifecycleCause, PendingTileOpenMode, WorkbenchIntent};
use crate::graph::NodeKey;
use crate::shell::desktop::runtime::registries::workbench_surface;

pub(crate) const WEBDRIVER_SCRIPT_PREFIX: &str = "graphshell:ux-bridge:";

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct UxDriver;

impl UxDriver {
    pub(crate) fn get_ux_snapshot_script() -> String {
        webdriver_script(serde_json::json!({ "command": "GetUxSnapshot" }))
    }

    pub(crate) fn find_ux_node_script(selector: &UxNodeSelector) -> String {
        webdriver_script(serde_json::json!({
            "command": "FindUxNode",
            "selector": selector_json(selector),
        }))
    }

    pub(crate) fn get_focus_path_script() -> String {
        webdriver_script(serde_json::json!({ "command": "GetFocusPath" }))
    }

    pub(crate) fn invoke_ux_action_script(selector: &UxNodeSelector, action: UxAction) -> String {
        webdriver_script(serde_json::json!({
            "command": "InvokeUxAction",
            "selector": selector_json(selector),
            "action": action,
        }))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum UxNodeSelector {
    ById(String),
    ByLabel(String),
    ByRole(UxNodeRole),
    ByRoleAndLabel(UxNodeRole, String),
    Focused,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum UxBridgeCommand {
    GetUxSnapshot,
    FindUxNode {
        selector: UxNodeSelector,
    },
    GetFocusPath,
    InvokeUxAction {
        selector: UxNodeSelector,
        action: UxAction,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum UxBridgeResponse {
    Snapshot(UxTreeSnapshot),
    Node(Option<UxSemanticNode>),
    FocusPath(Vec<String>),
    Action {
        status: UxActionStatus,
        ux_node_id: String,
        action: UxAction,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UxActionStatus {
    Applied,
    Queued,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UxBridgeErrorKind {
    SnapshotUnavailable,
    TargetNotFound,
    UnsupportedAction,
    InvalidTransportPayload,
    TransportUnavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UxBridgeError {
    pub(crate) kind: UxBridgeErrorKind,
    pub(crate) message: String,
}

impl UxBridgeError {
    fn snapshot_unavailable() -> Self {
        Self {
            kind: UxBridgeErrorKind::SnapshotUnavailable,
            message: "No published UxTree snapshot is available for bridge queries.".to_string(),
        }
    }

    fn target_not_found(selector: &UxNodeSelector) -> Self {
        Self {
            kind: UxBridgeErrorKind::TargetNotFound,
            message: format!("No UxTree semantic node matched selector {selector:?}."),
        }
    }

    fn unsupported_action(action: UxAction, node: &UxSemanticNode) -> Self {
        Self {
            kind: UxBridgeErrorKind::UnsupportedAction,
            message: format!(
                "UxTree action {action:?} is not supported for node '{}' ({:?}).",
                node.ux_node_id, node.role
            ),
        }
    }

    pub(crate) fn invalid_transport_payload(message: impl Into<String>) -> Self {
        Self {
            kind: UxBridgeErrorKind::InvalidTransportPayload,
            message: message.into(),
        }
    }

    pub(crate) fn transport_unavailable(message: impl Into<String>) -> Self {
        Self {
            kind: UxBridgeErrorKind::TransportUnavailable,
            message: message.into(),
        }
    }
}

pub(crate) fn handle_latest_snapshot_command(
    command: UxBridgeCommand,
) -> Result<UxBridgeResponse, UxBridgeError> {
    let Some(snapshot) = ux_tree::latest_snapshot() else {
        return Err(UxBridgeError::snapshot_unavailable());
    };
    Ok(handle_snapshot_command(&snapshot, command))
}

pub(crate) fn handle_snapshot_command(
    snapshot: &UxTreeSnapshot,
    command: UxBridgeCommand,
) -> UxBridgeResponse {
    match command {
        UxBridgeCommand::GetUxSnapshot => UxBridgeResponse::Snapshot(snapshot.clone()),
        UxBridgeCommand::FindUxNode { selector } => {
            UxBridgeResponse::Node(find_semantic_node(snapshot, &selector).cloned())
        }
        UxBridgeCommand::GetFocusPath => UxBridgeResponse::FocusPath(focus_path(snapshot)),
        UxBridgeCommand::InvokeUxAction { .. } => {
            unreachable!("snapshot-only handler cannot execute mutable bridge actions")
        }
    }
}

pub(crate) fn handle_runtime_command(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    graph_tree: &mut graph_tree::GraphTree<NodeKey>,
    command: UxBridgeCommand,
) -> Result<UxBridgeResponse, UxBridgeError> {
    let snapshot = ux_tree::build_snapshot(tiles_tree, graph_app, 0);
    match command {
        UxBridgeCommand::GetUxSnapshot
        | UxBridgeCommand::FindUxNode { .. }
        | UxBridgeCommand::GetFocusPath => Ok(handle_snapshot_command(&snapshot, command)),
        UxBridgeCommand::InvokeUxAction { selector, action } => {
            let (intent, target) = workbench_intent_for_action(&snapshot, &selector, action)?;
            apply_workbench_intent(graph_app, tiles_tree, graph_tree, &intent);
            ux_tree::publish_snapshot(&ux_tree::build_snapshot(tiles_tree, graph_app, 0));
            Ok(UxBridgeResponse::Action {
                status: UxActionStatus::Applied,
                ux_node_id: target.ux_node_id.clone(),
                action,
            })
        }
    }
}

pub(crate) fn queued_workbench_intent_for_latest_snapshot(
    selector: &UxNodeSelector,
    action: UxAction,
) -> Result<(WorkbenchIntent, UxBridgeResponse), UxBridgeError> {
    let Some(snapshot) = ux_tree::latest_snapshot() else {
        return Err(UxBridgeError::snapshot_unavailable());
    };
    let (intent, target) = workbench_intent_for_action(&snapshot, selector, action)?;
    Ok((
        intent,
        UxBridgeResponse::Action {
            status: UxActionStatus::Queued,
            ux_node_id: target.ux_node_id.clone(),
            action,
        },
    ))
}

pub(crate) fn parse_transport_command(payload: &str) -> Result<UxBridgeCommand, UxBridgeError> {
    let command = serde_json::from_str::<TransportCommand>(payload).map_err(|error| {
        UxBridgeError::invalid_transport_payload(format!(
            "Failed to parse ux bridge payload: {error}"
        ))
    })?;

    Ok(match command {
        TransportCommand::GetUxSnapshot => UxBridgeCommand::GetUxSnapshot,
        TransportCommand::FindUxNode { selector } => UxBridgeCommand::FindUxNode {
            selector: selector.into_selector(),
        },
        TransportCommand::GetFocusPath => UxBridgeCommand::GetFocusPath,
        TransportCommand::InvokeUxAction { selector, action } => UxBridgeCommand::InvokeUxAction {
            selector: selector.into_selector(),
            action,
        },
    })
}

pub(crate) fn response_json(response: &UxBridgeResponse) -> serde_json::Value {
    serde_json::json!({
        "ok": true,
        "response": match response {
            UxBridgeResponse::Snapshot(snapshot) => serde_json::json!({
                "kind": "Snapshot",
                "snapshot": ux_tree::snapshot_json(snapshot),
            }),
            UxBridgeResponse::Node(node) => serde_json::json!({
                "kind": "Node",
                "node": node.as_ref().map(semantic_node_json),
            }),
            UxBridgeResponse::FocusPath(path) => serde_json::json!({
                "kind": "FocusPath",
                "path": path,
            }),
            UxBridgeResponse::Action {
                status,
                ux_node_id,
                action,
            } => serde_json::json!({
                "kind": "Action",
                "status": format!("{:?}", status),
                "ux_node_id": ux_node_id,
                "action": format!("{:?}", action),
            }),
        }
    })
}

pub(crate) fn error_json(error: &UxBridgeError) -> serde_json::Value {
    serde_json::json!({
        "ok": false,
        "error": {
            "kind": format!("{:?}", error.kind),
            "message": error.message,
        }
    })
}

fn webdriver_script(payload: serde_json::Value) -> String {
    format!("{WEBDRIVER_SCRIPT_PREFIX}{payload}")
}

fn selector_json(selector: &UxNodeSelector) -> serde_json::Value {
    match selector {
        UxNodeSelector::ById(ux_node_id) => {
            serde_json::json!({ "kind": "ById", "ux_node_id": ux_node_id })
        }
        UxNodeSelector::ByLabel(label) => serde_json::json!({ "kind": "ByLabel", "label": label }),
        UxNodeSelector::ByRole(role) => serde_json::json!({ "kind": "ByRole", "role": role }),
        UxNodeSelector::ByRoleAndLabel(role, label) => {
            serde_json::json!({ "kind": "ByRoleAndLabel", "role": role, "label": label })
        }
        UxNodeSelector::Focused => serde_json::json!({ "kind": "Focused" }),
    }
}

fn find_semantic_node<'a>(
    snapshot: &'a UxTreeSnapshot,
    selector: &UxNodeSelector,
) -> Option<&'a UxSemanticNode> {
    if matches!(selector, UxNodeSelector::Focused) {
        return focused_semantic_node(snapshot);
    }

    snapshot.semantic_nodes.iter().find(|node| match selector {
        UxNodeSelector::ById(ux_node_id) => node.ux_node_id == *ux_node_id,
        UxNodeSelector::ByLabel(label) => node.label == *label,
        UxNodeSelector::ByRole(role) => node.role == *role,
        UxNodeSelector::ByRoleAndLabel(role, label) => node.role == *role && node.label == *label,
        UxNodeSelector::Focused => unreachable!("focused selector is handled above"),
    })
}

fn workbench_intent_for_action<'a>(
    snapshot: &'a UxTreeSnapshot,
    selector: &UxNodeSelector,
    action: UxAction,
) -> Result<(WorkbenchIntent, &'a UxSemanticNode), UxBridgeError> {
    let target = find_semantic_node(snapshot, selector)
        .ok_or_else(|| UxBridgeError::target_not_found(selector))?;

    let intent = match (&target.domain, action) {
        (UxDomainIdentity::CommandBar { .. }, UxAction::Open | UxAction::Focus)
        | (UxDomainIdentity::Omnibar { .. }, UxAction::Open | UxAction::Focus) => {
            WorkbenchIntent::OpenCommandPalette
        }
        (UxDomainIdentity::CommandPalette { .. }, UxAction::Dismiss) => {
            WorkbenchIntent::CloseCommandPalette
        }
        (UxDomainIdentity::GraphView { graph_view_id, .. }, UxAction::Focus) => {
            WorkbenchIntent::OpenGraphViewPane {
                view_id: *graph_view_id,
                mode: PendingTileOpenMode::Tab,
            }
        }
        (
            UxDomainIdentity::GraphView {
                pane_id: Some(pane_id),
                ..
            },
            UxAction::Close,
        ) => WorkbenchIntent::ClosePane {
            pane: *pane_id,
            restore_previous_focus: true,
        },
        (UxDomainIdentity::Tool { tool_kind, .. }, UxAction::Focus) => {
            WorkbenchIntent::OpenToolPane {
                kind: tool_kind.clone(),
            }
        }
        (UxDomainIdentity::Tool { tool_kind, .. }, UxAction::Close) => {
            WorkbenchIntent::CloseToolPane {
                kind: tool_kind.clone(),
                restore_previous_focus: true,
            }
        }
        (
            UxDomainIdentity::Node {
                node_key,
                pane_id: Some(pane_id),
                ..
            },
            UxAction::Open | UxAction::Focus,
        ) => WorkbenchIntent::OpenNodeInPane {
            node: *node_key,
            pane: *pane_id,
        },
        (
            UxDomainIdentity::Node {
                pane_id: Some(pane_id),
                ..
            },
            UxAction::Close,
        ) => WorkbenchIntent::DismissTile { pane: *pane_id },
        _ => return Err(UxBridgeError::unsupported_action(action, target)),
    };

    Ok((intent, target))
}

fn apply_workbench_intent(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    graph_tree: &mut graph_tree::GraphTree<NodeKey>,
    intent: &WorkbenchIntent,
) {
    use super::graph_tree_dual_write as dual_write;

    match intent {
        WorkbenchIntent::OpenCommandPalette => {
            if graph_app.pending_command_surface_return_target().is_none() {
                graph_app.set_pending_command_surface_return_target(
                    workbench_surface::active_tool_surface_return_target(tiles_tree),
                );
            }
            graph_app.open_command_palette();
        }
        WorkbenchIntent::CloseCommandPalette => {
            graph_app.close_command_palette();
            let target = graph_app.take_pending_command_surface_return_target();
            let _ = workbench_surface::restore_focus_target_or_ensure_active_tile(
                graph_app, tiles_tree, target, true,
            );
        }
        WorkbenchIntent::OpenNodeInPane { node, pane } => {
            if !dual_write::focus_pane(tiles_tree, graph_tree, *pane, Some(*node)) {
                dual_write::open_or_focus_node(tiles_tree, graph_tree, graph_app, *node, None);
            }
        }
        WorkbenchIntent::OpenGraphViewPane { view_id, .. } => {
            // Graph view panes are the canvas itself, not graph node members —
            // outside GraphTree's scope.
            tile_view_ops::open_or_focus_graph_pane_with_mode(
                tiles_tree,
                *view_id,
                tile_view_ops::TileOpenMode::Tab,
            );
        }
        WorkbenchIntent::DismissTile { pane } => {
            dismiss_node_pane(graph_app, tiles_tree, graph_tree, *pane);
        }
        WorkbenchIntent::OpenToolPane { kind } => {
            #[cfg(feature = "diagnostics")]
            {
                dual_write::open_or_focus_tool_pane(tiles_tree, graph_tree, kind.clone());
            }
            #[cfg(not(feature = "diagnostics"))]
            let _ = kind;
        }
        WorkbenchIntent::CloseToolPane { kind, .. } => {
            close_tool_pane(tiles_tree, graph_tree, kind.clone());
        }
        WorkbenchIntent::ClosePane { pane, .. } => {
            close_pane(tiles_tree, graph_tree, *pane);
        }
        other => unreachable!("unexpected bridge workbench intent {other:?}"),
    }
}

fn dismiss_node_pane(
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &mut Tree<TileKind>,
    graph_tree: &mut graph_tree::GraphTree<NodeKey>,
    pane: crate::shell::desktop::workbench::pane_model::PaneId,
) {
    use super::graph_tree_dual_write as dual_write;

    let node_key = tiles_tree.tiles.iter().find_map(|(_, tile)| match tile {
        egui_tiles::Tile::Pane(kind) if kind.pane_id() == pane => {
            kind.node_state().map(|state| state.node)
        }
        _ => None,
    });

    if dual_write::close_pane(tiles_tree, graph_tree, pane, node_key) {
        if let Some(node_key) = node_key {
            graph_app.demote_node_to_cold_with_cause(node_key, LifecycleCause::ExplicitClose);
        }
        dual_write::ensure_active_tile(tiles_tree, graph_tree);
    }
}

fn close_pane(
    tiles_tree: &mut Tree<TileKind>,
    graph_tree: &mut graph_tree::GraphTree<NodeKey>,
    pane: crate::shell::desktop::workbench::pane_model::PaneId,
) {
    use super::graph_tree_dual_write as dual_write;

    if dual_write::close_pane(tiles_tree, graph_tree, pane, None) {
        dual_write::ensure_active_tile(tiles_tree, graph_tree);
    }
}

fn close_tool_pane(
    tiles_tree: &mut Tree<TileKind>,
    graph_tree: &mut graph_tree::GraphTree<NodeKey>,
    kind: crate::shell::desktop::workbench::pane_model::ToolPaneState,
) {
    use super::graph_tree_dual_write as dual_write;

    #[cfg(feature = "diagnostics")]
    {
        if dual_write::close_tool_pane(tiles_tree, graph_tree, kind) {
            dual_write::ensure_active_tile(tiles_tree, graph_tree);
        }
    }
    #[cfg(not(feature = "diagnostics"))]
    let _ = (tiles_tree, graph_tree, kind);
}

fn focus_path(snapshot: &UxTreeSnapshot) -> Vec<String> {
    let Some(focused_node) = focused_semantic_node(snapshot) else {
        return Vec::new();
    };

    let parent_by_id = snapshot
        .semantic_nodes
        .iter()
        .map(|node| (node.ux_node_id.as_str(), node.parent_ux_node_id.as_deref()))
        .collect::<HashMap<_, _>>();

    let mut path = vec![focused_node.ux_node_id.clone()];
    let mut cursor = focused_node.parent_ux_node_id.as_deref();
    while let Some(parent_id) = cursor {
        path.push(parent_id.to_string());
        cursor = parent_by_id.get(parent_id).copied().flatten();
    }
    path.reverse();
    path
}

fn focused_semantic_node(snapshot: &UxTreeSnapshot) -> Option<&UxSemanticNode> {
    let parent_by_id = snapshot
        .semantic_nodes
        .iter()
        .map(|node| (node.ux_node_id.as_str(), node.parent_ux_node_id.as_deref()))
        .collect::<HashMap<_, _>>();

    snapshot
        .semantic_nodes
        .iter()
        .filter(|node| node.state.focused)
        .max_by_key(|node| semantic_depth(&parent_by_id, &node.ux_node_id))
}

fn semantic_depth(parent_by_id: &HashMap<&str, Option<&str>>, ux_node_id: &str) -> usize {
    let mut depth = 0;
    let mut cursor = parent_by_id.get(ux_node_id).copied().flatten();
    while let Some(parent_id) = cursor {
        depth += 1;
        cursor = parent_by_id.get(parent_id).copied().flatten();
    }
    depth
}

fn semantic_node_json(node: &UxSemanticNode) -> serde_json::Value {
    serde_json::json!({
        "ux_node_id": node.ux_node_id,
        "parent_ux_node_id": node.parent_ux_node_id,
        "role": format!("{:?}", node.role),
        "label": node.label,
        "focused": node.state.focused,
        "selected": node.state.selected,
        "blocked": node.state.blocked,
        "degraded": node.state.degraded,
        "allowed_actions": node.allowed_actions.iter().map(|action| format!("{:?}", action)).collect::<Vec<_>>(),
        "domain": format!("{:?}", node.domain),
    })
}

#[derive(Debug, Deserialize)]
#[serde(tag = "command")]
enum TransportCommand {
    GetUxSnapshot,
    FindUxNode {
        selector: TransportSelector,
    },
    GetFocusPath,
    InvokeUxAction {
        selector: TransportSelector,
        action: UxAction,
    },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind")]
enum TransportSelector {
    ById { ux_node_id: String },
    ByLabel { label: String },
    ByRole { role: UxNodeRole },
    ByRoleAndLabel { role: UxNodeRole, label: String },
    Focused,
}

impl TransportSelector {
    fn into_selector(self) -> UxNodeSelector {
        match self {
            Self::ById { ux_node_id } => UxNodeSelector::ById(ux_node_id),
            Self::ByLabel { label } => UxNodeSelector::ByLabel(label),
            Self::ByRole { role } => UxNodeSelector::ByRole(role),
            Self::ByRoleAndLabel { role, label } => UxNodeSelector::ByRoleAndLabel(role, label),
            Self::Focused => UxNodeSelector::Focused,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::shell::desktop::tests::harness::TestRegistry;
    use crate::shell::desktop::ui::toolbar::toolbar_ui::{
        CommandBarSemanticMetadata, CommandRouteEventSequenceMetadata,
        CommandSurfaceSemanticSnapshot, OmnibarMailboxEventSequenceMetadata,
        OmnibarSemanticMetadata, PaletteSurfaceSemanticMetadata,
        clear_command_surface_semantic_snapshot, lock_command_surface_snapshot_tests,
        publish_command_surface_semantic_snapshot,
    };

    fn lock_bridge_tests() -> std::sync::MutexGuard<'static, ()> {
        static UX_BRIDGE_TEST_LOCK: std::sync::OnceLock<std::sync::Mutex<()>> =
            std::sync::OnceLock::new();
        UX_BRIDGE_TEST_LOCK
            .get_or_init(|| std::sync::Mutex::new(()))
            .lock()
            .expect("ux bridge test lock should not be poisoned")
    }

    #[test]
    fn latest_snapshot_bridge_returns_published_snapshot() {
        let _guard = lock_bridge_tests();
        let mut harness = TestRegistry::new();
        let node = harness.add_node("https://ux-bridge.example");
        harness.open_node_tab(node);
        harness.app.select_node(node, false);

        let snapshot = ux_tree::build_snapshot(&harness.tiles_tree, &harness.app, 9);
        ux_tree::publish_snapshot(&snapshot);

        let response = handle_latest_snapshot_command(UxBridgeCommand::GetUxSnapshot)
            .expect("published snapshot should satisfy bridge query");
        match response {
            UxBridgeResponse::Snapshot(bridged_snapshot) => {
                assert_eq!(bridged_snapshot.semantic_version, snapshot.semantic_version);
                assert_eq!(bridged_snapshot.semantic_nodes, snapshot.semantic_nodes);
            }
            other => panic!("expected snapshot bridge response, got {other:?}"),
        }

        ux_tree::clear_snapshot();
    }

    #[test]
    fn focus_path_bridge_returns_root_to_focused_node() {
        let _guard = lock_command_surface_snapshot_tests();
        clear_command_surface_semantic_snapshot();
        publish_command_surface_semantic_snapshot(CommandSurfaceSemanticSnapshot {
            command_bar: CommandBarSemanticMetadata {
                active_pane: None,
                focused_node: None,
                location_focused: true,
                route_events: CommandRouteEventSequenceMetadata::default(),
            },
            omnibar: OmnibarSemanticMetadata {
                active: true,
                focused: false,
                query: Some("focus-path".to_string()),
                match_count: 1,
                provider_status: None,
                active_pane: None,
                focused_node: None,
                mailbox_events: OmnibarMailboxEventSequenceMetadata::default(),
            },
            command_palette: None,
            context_palette: None,
        });

        let harness = TestRegistry::new();
        let snapshot = ux_tree::build_snapshot(&harness.tiles_tree, &harness.app, 10);
        let response = handle_snapshot_command(&snapshot, UxBridgeCommand::GetFocusPath);
        match response {
            UxBridgeResponse::FocusPath(path) => {
                assert_eq!(
                    path.first().map(String::as_str),
                    Some(ux_tree::UX_TREE_WORKBENCH_ROOT_ID)
                );
                assert!(
                    path.len() >= 2,
                    "focus path should include root and focused node"
                );
            }
            other => panic!("expected focus-path response, got {other:?}"),
        }

        clear_command_surface_semantic_snapshot();
    }

    #[test]
    fn find_node_bridge_matches_command_bar_projection() {
        let _guard = lock_command_surface_snapshot_tests();
        clear_command_surface_semantic_snapshot();
        publish_command_surface_semantic_snapshot(CommandSurfaceSemanticSnapshot {
            command_bar: CommandBarSemanticMetadata {
                active_pane: None,
                focused_node: None,
                location_focused: true,
                route_events: CommandRouteEventSequenceMetadata::default(),
            },
            omnibar: OmnibarSemanticMetadata {
                active: true,
                focused: true,
                query: Some("bridge".to_string()),
                match_count: 1,
                provider_status: None,
                active_pane: None,
                focused_node: None,
                mailbox_events: OmnibarMailboxEventSequenceMetadata::default(),
            },
            command_palette: None,
            context_palette: None,
        });

        let harness = TestRegistry::new();
        let snapshot = ux_tree::build_snapshot(&harness.tiles_tree, &harness.app, 11);
        let response = handle_snapshot_command(
            &snapshot,
            UxBridgeCommand::FindUxNode {
                selector: UxNodeSelector::ByRoleAndLabel(
                    UxNodeRole::CommandBar,
                    "Command Bar".to_string(),
                ),
            },
        );
        match response {
            UxBridgeResponse::Node(Some(node)) => {
                assert_eq!(node.role, UxNodeRole::CommandBar);
                assert_eq!(node.label, "Command Bar");
            }
            other => panic!("expected command bar node response, got {other:?}"),
        }

        clear_command_surface_semantic_snapshot();
    }

    #[test]
    fn runtime_bridge_open_and_dismiss_command_palette() {
        let _guard = lock_command_surface_snapshot_tests();
        clear_command_surface_semantic_snapshot();

        let mut harness = TestRegistry::new();
        publish_command_surface_semantic_snapshot(CommandSurfaceSemanticSnapshot {
            command_bar: CommandBarSemanticMetadata {
                active_pane: None,
                focused_node: None,
                location_focused: true,
                route_events: CommandRouteEventSequenceMetadata::default(),
            },
            omnibar: OmnibarSemanticMetadata::default(),
            command_palette: None,
            context_palette: None,
        });

        let open = handle_runtime_command(
            &mut harness.app,
            &mut harness.tiles_tree,
            &mut harness.graph_tree,
            UxBridgeCommand::InvokeUxAction {
                selector: UxNodeSelector::ByRole(UxNodeRole::CommandBar),
                action: UxAction::Open,
            },
        )
        .expect("open bridge action should succeed");
        assert_eq!(
            open,
            UxBridgeResponse::Action {
                status: UxActionStatus::Applied,
                ux_node_id: "uxnode://command/bar/root".to_string(),
                action: UxAction::Open,
            }
        );
        assert!(harness.app.workspace.chrome_ui.show_command_palette);

        publish_command_surface_semantic_snapshot(CommandSurfaceSemanticSnapshot {
            command_bar: CommandBarSemanticMetadata::default(),
            omnibar: OmnibarSemanticMetadata {
                active: true,
                focused: true,
                ..OmnibarSemanticMetadata::default()
            },
            command_palette: Some(PaletteSurfaceSemanticMetadata::default()),
            context_palette: None,
        });

        let dismiss = handle_runtime_command(
            &mut harness.app,
            &mut harness.tiles_tree,
            &mut harness.graph_tree,
            UxBridgeCommand::InvokeUxAction {
                selector: UxNodeSelector::ByRole(UxNodeRole::CommandPalette),
                action: UxAction::Dismiss,
            },
        )
        .expect("dismiss bridge action should succeed");
        assert_eq!(
            dismiss,
            UxBridgeResponse::Action {
                status: UxActionStatus::Applied,
                ux_node_id: "uxnode://command/palette/root".to_string(),
                action: UxAction::Dismiss,
            }
        );
        assert!(!harness.app.workspace.chrome_ui.show_command_palette);

        clear_command_surface_semantic_snapshot();
    }

    #[test]
    fn runtime_bridge_focus_and_close_node_pane() {
        let mut harness = TestRegistry::new();
        let first = harness.add_node("https://ux-bridge-first.example");
        let second = harness.add_node("https://ux-bridge-second.example");
        harness.open_node_tab(first);
        harness.open_node_tab(second);

        let snapshot = ux_tree::build_snapshot(&harness.tiles_tree, &harness.app, 7);
        let second_node = snapshot
            .semantic_nodes
            .iter()
            .find(|node| {
                matches!(
                    node.domain,
                    UxDomainIdentity::Node {
                        node_key,
                        pane_id: Some(_),
                        ..
                    } if node_key == second
                )
            })
            .expect("snapshot should include second node pane");

        let focus = handle_runtime_command(
            &mut harness.app,
            &mut harness.tiles_tree,
            &mut harness.graph_tree,
            UxBridgeCommand::InvokeUxAction {
                selector: UxNodeSelector::ById(second_node.ux_node_id.clone()),
                action: UxAction::Focus,
            },
        )
        .expect("focus bridge action should succeed");
        assert_eq!(
            focus,
            UxBridgeResponse::Action {
                status: UxActionStatus::Applied,
                ux_node_id: second_node.ux_node_id.clone(),
                action: UxAction::Focus,
            }
        );
        let refreshed = ux_tree::build_snapshot(&harness.tiles_tree, &harness.app, 8);
        let focused = refreshed
            .semantic_nodes
            .iter()
            .find(|node| node.ux_node_id == second_node.ux_node_id)
            .expect("focused node pane should still exist");
        assert!(focused.state.focused);

        let close = handle_runtime_command(
            &mut harness.app,
            &mut harness.tiles_tree,
            &mut harness.graph_tree,
            UxBridgeCommand::InvokeUxAction {
                selector: UxNodeSelector::ById(second_node.ux_node_id.clone()),
                action: UxAction::Close,
            },
        )
        .expect("close bridge action should succeed");
        assert_eq!(
            close,
            UxBridgeResponse::Action {
                status: UxActionStatus::Applied,
                ux_node_id: second_node.ux_node_id.clone(),
                action: UxAction::Close,
            }
        );
        let refreshed = ux_tree::build_snapshot(&harness.tiles_tree, &harness.app, 9);
        assert!(
            refreshed
                .semantic_nodes
                .iter()
                .all(|node| node.ux_node_id != second_node.ux_node_id),
            "closed node pane should be removed from the snapshot"
        );
        assert_eq!(
            harness
                .app
                .domain_graph()
                .get_node(second)
                .expect("second node should still exist")
                .lifecycle,
            crate::graph::NodeLifecycle::Cold
        );
    }

    #[cfg(feature = "diagnostics")]
    #[test]
    fn runtime_bridge_focus_and_close_tool_pane() {
        let mut harness = TestRegistry::new();
        harness
            .open_tool_tab(crate::shell::desktop::workbench::pane_model::ToolPaneState::Settings);

        let snapshot = ux_tree::build_snapshot(&harness.tiles_tree, &harness.app, 3);
        let tool = snapshot
            .semantic_nodes
            .iter()
            .find(|node| matches!(node.domain, UxDomainIdentity::Tool { .. }))
            .expect("snapshot should include tool pane semantic node");

        let focus = handle_runtime_command(
            &mut harness.app,
            &mut harness.tiles_tree,
            &mut harness.graph_tree,
            UxBridgeCommand::InvokeUxAction {
                selector: UxNodeSelector::ById(tool.ux_node_id.clone()),
                action: UxAction::Focus,
            },
        )
        .expect("tool focus bridge action should succeed");
        assert_eq!(
            focus,
            UxBridgeResponse::Action {
                status: UxActionStatus::Applied,
                ux_node_id: tool.ux_node_id.clone(),
                action: UxAction::Focus,
            }
        );

        let close = handle_runtime_command(
            &mut harness.app,
            &mut harness.tiles_tree,
            &mut harness.graph_tree,
            UxBridgeCommand::InvokeUxAction {
                selector: UxNodeSelector::ById(tool.ux_node_id.clone()),
                action: UxAction::Close,
            },
        )
        .expect("tool close bridge action should succeed");
        assert_eq!(
            close,
            UxBridgeResponse::Action {
                status: UxActionStatus::Applied,
                ux_node_id: tool.ux_node_id.clone(),
                action: UxAction::Close,
            }
        );
        let refreshed = ux_tree::build_snapshot(&harness.tiles_tree, &harness.app, 4);
        assert!(
            refreshed
                .semantic_nodes
                .iter()
                .all(|node| node.ux_node_id != tool.ux_node_id),
            "closed tool pane should be removed from the snapshot"
        );
    }

    #[test]
    fn runtime_bridge_focus_and_close_graph_surface() {
        let mut harness = TestRegistry::new();
        let second_view = crate::app::GraphViewId::new();
        harness.open_graph_tab(second_view);

        let snapshot = ux_tree::build_snapshot(&harness.tiles_tree, &harness.app, 5);
        let graph_surface = snapshot
            .semantic_nodes
            .iter()
            .find(|node| {
                matches!(
                    node.domain,
                    UxDomainIdentity::GraphView {
                        graph_view_id,
                        pane_id: Some(_),
                    } if graph_view_id == second_view
                )
            })
            .expect("snapshot should include second graph surface");

        let focus = handle_runtime_command(
            &mut harness.app,
            &mut harness.tiles_tree,
            &mut harness.graph_tree,
            UxBridgeCommand::InvokeUxAction {
                selector: UxNodeSelector::ById(graph_surface.ux_node_id.clone()),
                action: UxAction::Focus,
            },
        )
        .expect("graph focus bridge action should succeed");
        assert_eq!(
            focus,
            UxBridgeResponse::Action {
                status: UxActionStatus::Applied,
                ux_node_id: graph_surface.ux_node_id.clone(),
                action: UxAction::Focus,
            }
        );

        let close = handle_runtime_command(
            &mut harness.app,
            &mut harness.tiles_tree,
            &mut harness.graph_tree,
            UxBridgeCommand::InvokeUxAction {
                selector: UxNodeSelector::ById(graph_surface.ux_node_id.clone()),
                action: UxAction::Close,
            },
        )
        .expect("graph close bridge action should succeed");
        assert_eq!(
            close,
            UxBridgeResponse::Action {
                status: UxActionStatus::Applied,
                ux_node_id: graph_surface.ux_node_id.clone(),
                action: UxAction::Close,
            }
        );
        let refreshed = ux_tree::build_snapshot(&harness.tiles_tree, &harness.app, 6);
        assert!(
            refreshed
                .semantic_nodes
                .iter()
                .all(|node| node.ux_node_id != graph_surface.ux_node_id),
            "closed graph pane should be removed from the snapshot"
        );
    }

    #[test]
    fn transport_command_parses_action_payload() {
        let command = parse_transport_command(
            r#"{"command":"InvokeUxAction","selector":{"kind":"ByRole","role":"CommandBar"},"action":"Open"}"#,
        )
        .expect("transport payload should parse");

        assert_eq!(
            command,
            UxBridgeCommand::InvokeUxAction {
                selector: UxNodeSelector::ByRole(UxNodeRole::CommandBar),
                action: UxAction::Open,
            }
        );
    }

    #[test]
    fn ux_driver_emits_prefixed_bridge_scripts() {
        let script = UxDriver::invoke_ux_action_script(
            &UxNodeSelector::ByRole(UxNodeRole::CommandBar),
            UxAction::Open,
        );
        let payload = script
            .strip_prefix(WEBDRIVER_SCRIPT_PREFIX)
            .expect("ux driver should emit the reserved webdriver prefix");

        assert_eq!(
            parse_transport_command(payload).expect("driver payload should parse"),
            UxBridgeCommand::InvokeUxAction {
                selector: UxNodeSelector::ByRole(UxNodeRole::CommandBar),
                action: UxAction::Open,
            }
        );
    }

    #[test]
    fn queued_bridge_action_maps_node_pane_to_open_and_dismiss_intents() {
        let _guard = lock_bridge_tests();
        let mut harness = TestRegistry::new();
        let node = harness.add_node("https://ux-bridge-queued.example");
        harness.open_node_tab(node);

        let snapshot = ux_tree::build_snapshot(&harness.tiles_tree, &harness.app, 4);
        ux_tree::publish_snapshot(&snapshot);

        let node_pane = snapshot
            .semantic_nodes
            .iter()
            .find(|entry| {
                matches!(
                    entry.domain,
                    UxDomainIdentity::Node {
                        node_key,
                        pane_id: Some(_),
                        ..
                    } if node_key == node
                )
            })
            .expect("snapshot should include node pane semantic node");

        let focus = queued_workbench_intent_for_latest_snapshot(
            &UxNodeSelector::ById(node_pane.ux_node_id.clone()),
            UxAction::Focus,
        )
        .expect("queued node focus should succeed");
        assert!(matches!(
            focus.0,
            WorkbenchIntent::OpenNodeInPane { node: queued_node, .. } if queued_node == node
        ));

        let close = queued_workbench_intent_for_latest_snapshot(
            &UxNodeSelector::ById(node_pane.ux_node_id.clone()),
            UxAction::Close,
        )
        .expect("queued node close should succeed");
        assert!(matches!(close.0, WorkbenchIntent::DismissTile { .. }));

        ux_tree::clear_snapshot();
    }

    #[cfg(feature = "diagnostics")]
    #[test]
    fn queued_bridge_action_maps_tool_pane_to_open_and_close_intents() {
        let _guard = lock_bridge_tests();
        let mut harness = TestRegistry::new();
        harness
            .open_tool_tab(crate::shell::desktop::workbench::pane_model::ToolPaneState::Settings);

        let snapshot = ux_tree::build_snapshot(&harness.tiles_tree, &harness.app, 4);
        ux_tree::publish_snapshot(&snapshot);

        let tool = snapshot
            .semantic_nodes
            .iter()
            .find(|entry| matches!(entry.domain, UxDomainIdentity::Tool { .. }))
            .expect("snapshot should include tool pane semantic node");

        let focus = queued_workbench_intent_for_latest_snapshot(
            &UxNodeSelector::ById(tool.ux_node_id.clone()),
            UxAction::Focus,
        )
        .expect("queued tool focus should succeed");
        assert!(matches!(focus.0, WorkbenchIntent::OpenToolPane { .. }));

        let close = queued_workbench_intent_for_latest_snapshot(
            &UxNodeSelector::ById(tool.ux_node_id.clone()),
            UxAction::Close,
        )
        .expect("queued tool close should succeed");
        assert!(matches!(close.0, WorkbenchIntent::CloseToolPane { .. }));

        ux_tree::clear_snapshot();
    }

    #[test]
    fn queued_bridge_action_maps_graph_surface_to_open_and_close_intents() {
        let _guard = lock_bridge_tests();
        let mut harness = TestRegistry::new();
        let view_id = crate::app::GraphViewId::new();
        harness.open_graph_tab(view_id);

        let snapshot = ux_tree::build_snapshot(&harness.tiles_tree, &harness.app, 4);
        ux_tree::publish_snapshot(&snapshot);

        let graph_surface = snapshot
            .semantic_nodes
            .iter()
            .find(|entry| {
                matches!(
                    entry.domain,
                    UxDomainIdentity::GraphView {
                        graph_view_id,
                        pane_id: Some(_),
                    } if graph_view_id == view_id
                )
            })
            .expect("snapshot should include graph surface semantic node");

        let focus = queued_workbench_intent_for_latest_snapshot(
            &UxNodeSelector::ById(graph_surface.ux_node_id.clone()),
            UxAction::Focus,
        )
        .expect("queued graph focus should succeed");
        assert!(
            matches!(focus.0, WorkbenchIntent::OpenGraphViewPane { view_id: queued_view, .. } if queued_view == view_id)
        );

        let close = queued_workbench_intent_for_latest_snapshot(
            &UxNodeSelector::ById(graph_surface.ux_node_id.clone()),
            UxAction::Close,
        )
        .expect("queued graph close should succeed");
        assert!(matches!(close.0, WorkbenchIntent::ClosePane { .. }));

        ux_tree::clear_snapshot();
    }
}
