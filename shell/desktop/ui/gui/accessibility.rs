/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::{HashMap, HashSet};
#[cfg(feature = "diagnostics")]
use std::sync::{Mutex, OnceLock};

use accesskit::{Action, Node, NodeId, Role, TreeUpdate};
use egui::Context;
use log::warn;
use servo::WebViewId;

use crate::app::{GraphBrowserApp, GraphReaderModeState, GraphViewId};
use crate::graph::NodeKey;
use crate::shell::desktop::workbench::tile_compositor::{
    self, LifecycleTreatment, TileAffordanceAnnotation,
};
use crate::shell::desktop::workbench::ux_tree::{
    self, UxDomainIdentity, UxNodeRole, UxSemanticNode, UxTreeSnapshot,
};

#[cfg(feature = "diagnostics")]
#[derive(Debug, Clone)]
pub(crate) struct WebViewAccessibilityBridgeHealthSnapshot {
    pub(crate) update_queue_size: usize,
    pub(crate) anchor_count: usize,
    pub(crate) dropped_update_count: usize,
    pub(crate) focus_target: Option<String>,
    pub(crate) degradation_state: String,
}

#[cfg(feature = "diagnostics")]
#[derive(Debug, Clone, Default)]
struct WebViewAccessibilityBridgeHealthState {
    update_queue_size: usize,
    dropped_update_count: usize,
    focus_target: Option<String>,
    degradation_state: WebViewAccessibilityBridgeDegradationState,
}

#[cfg(feature = "diagnostics")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum WebViewAccessibilityBridgeDegradationState {
    #[default]
    None,
    Warning,
    Error,
}

#[cfg(feature = "diagnostics")]
impl WebViewAccessibilityBridgeDegradationState {
    fn label(self) -> String {
        match self {
            Self::None => "none".to_string(),
            Self::Warning => "warning".to_string(),
            Self::Error => "error".to_string(),
        }
    }
}

#[cfg(feature = "diagnostics")]
fn webview_accessibility_bridge_health_state(
) -> &'static Mutex<WebViewAccessibilityBridgeHealthState> {
    static STATE: OnceLock<Mutex<WebViewAccessibilityBridgeHealthState>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(WebViewAccessibilityBridgeHealthState::default()))
}

#[cfg(feature = "diagnostics")]
fn with_webview_accessibility_bridge_health_state<R>(
    f: impl FnOnce(&mut WebViewAccessibilityBridgeHealthState) -> R,
) -> R {
    let mut guard = webview_accessibility_bridge_health_state()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    f(&mut guard)
}

#[cfg(feature = "diagnostics")]
pub(super) fn record_webview_a11y_update_queued(
    webview_id: WebViewId,
    tree_update: &TreeUpdate,
    replaced_existing: bool,
    update_queue_size: usize,
) {
    with_webview_accessibility_bridge_health_state(|state| {
        state.update_queue_size = update_queue_size;
        if replaced_existing {
            state.dropped_update_count = state.dropped_update_count.saturating_add(1);
        }
        state.focus_target = Some(webview_accessibility_label(webview_id, tree_update));
    });
}

#[cfg(feature = "diagnostics")]
pub(super) fn webview_accessibility_bridge_health_snapshot(
    active_anchor_count: usize,
) -> WebViewAccessibilityBridgeHealthSnapshot {
    with_webview_accessibility_bridge_health_state(|state| {
        WebViewAccessibilityBridgeHealthSnapshot {
            update_queue_size: state.update_queue_size,
            anchor_count: active_anchor_count,
            dropped_update_count: state.dropped_update_count,
            focus_target: state.focus_target.clone(),
            degradation_state: state.degradation_state.label(),
        }
    })
}

#[cfg(feature = "diagnostics")]
fn record_webview_a11y_queue_drained() {
    with_webview_accessibility_bridge_health_state(|state| {
        state.update_queue_size = 0;
    });
}

#[cfg(feature = "diagnostics")]
fn record_webview_a11y_plan_degradation(plan: &WebViewA11yGraftPlan) {
    let degradation_state = if plan.nodes.is_empty() || plan.root_node_id.is_none() {
        WebViewAccessibilityBridgeDegradationState::Error
    } else if plan.dropped_node_count > 0 || plan.conversion_fallback_count > 0 {
        WebViewAccessibilityBridgeDegradationState::Warning
    } else {
        WebViewAccessibilityBridgeDegradationState::None
    };

    with_webview_accessibility_bridge_health_state(|state| {
        state.degradation_state = degradation_state;
    });
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TileAffordanceAccessibilityProjection {
    pub(crate) focus_annotation: bool,
    pub(crate) selection_annotation: bool,
    pub(crate) aria_busy: bool,
    pub(crate) lifecycle_label: &'static str,
    pub(crate) status_tokens: Vec<String>,
    pub(crate) glyph_descriptions: Vec<String>,
}

// This is a read-only projection derived from compositor output.
// It is not an accessibility authority and must remain subordinate to
// the canonical host-side UX/accessibility projection path.
pub(super) fn selected_node_affordance_projection(
    node_key: NodeKey,
) -> Option<TileAffordanceAccessibilityProjection> {
    let annotations = tile_compositor::latest_tile_affordance_annotations();
    selected_node_affordance_projection_from_annotations(node_key, &annotations)
}

pub(crate) fn selected_node_affordance_projection_from_annotations(
    node_key: NodeKey,
    annotations: &[TileAffordanceAnnotation],
) -> Option<TileAffordanceAccessibilityProjection> {
    annotations
        .iter()
        .find(|annotation| annotation.node_key == node_key)
        .map(project_tile_affordance_annotation)
}

fn project_tile_affordance_annotation(
    annotation: &TileAffordanceAnnotation,
) -> TileAffordanceAccessibilityProjection {
    let mut status_tokens = Vec::new();
    if annotation.focus_ring_rendered {
        status_tokens.push("focused".to_string());
    }
    if annotation.selection_ring_rendered {
        status_tokens.push("selected".to_string());
    }

    let (lifecycle_label, aria_busy) = match annotation.lifecycle_treatment {
        LifecycleTreatment::Active => ("active", false),
        LifecycleTreatment::Warm => {
            status_tokens.push("warm".to_string());
            ("warm", false)
        }
        LifecycleTreatment::Cold => {
            status_tokens.push("cold".to_string());
            ("cold", false)
        }
        LifecycleTreatment::Tombstone => {
            status_tokens.push("tombstone".to_string());
            ("tombstone", false)
        }
        LifecycleTreatment::RuntimeBlocked => {
            status_tokens.push("runtime-blocked".to_string());
            ("runtime-blocked", true)
        }
    };
    if aria_busy {
        status_tokens.push("aria-busy".to_string());
    }

    TileAffordanceAccessibilityProjection {
        focus_annotation: annotation.focus_ring_rendered,
        selection_annotation: annotation.selection_ring_rendered,
        aria_busy,
        lifecycle_label,
        status_tokens,
        glyph_descriptions: annotation.lens_glyphs_rendered.clone(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct UxTreeA11yNodePlan {
    pub(super) ux_node_id: String,
    pub(super) parent_ux_node_id: Option<String>,
    pub(super) role: egui::accesskit::Role,
    pub(super) label: String,
    pub(super) description: Option<String>,
    pub(super) state_description: Option<String>,
    pub(super) selected: bool,
    pub(super) busy: bool,
    pub(super) disabled: bool,
    pub(super) action_route: Option<UxTreeA11yActionRoute>,
    pub(super) attached_child_ids: Vec<egui::Id>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum UxTreeA11yActionRoute {
    GraphReaderMapItem { node_key: NodeKey },
    GraphReaderRoomRoot,
    GraphReaderRoomItem { node_key: NodeKey },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum UxTreeAccesskitDispatch {
    FocusGraphSurface,
    FocusGraphReaderMapItem { node_key: NodeKey },
    EnterGraphReaderRoom { node_key: NodeKey },
    ReturnGraphReaderToMap,
    Unsupported,
}

pub(super) struct UxTreeA11yGraftPlan {
    pub(super) anchor_label: String,
    pub(super) nodes: Vec<UxTreeA11yNodePlan>,
}

pub(super) fn inject_uxtree_a11y_updates(ctx: &Context, graph_app: &GraphBrowserApp) {
    let Some(snapshot) = ux_tree::latest_snapshot() else {
        return;
    };

    let annotations = tile_compositor::latest_tile_affordance_annotations();
    let plan = build_uxtree_a11y_graft_plan(&snapshot, &annotations, graph_app);

    inject_uxtree_a11y_anchor_node(
        ctx,
        uxtree_accessibility_anchor_id(),
        &plan.anchor_label,
        collect_root_child_accesskit_node_ids(&plan.nodes),
    );
    inject_uxtree_a11y_plan_nodes(ctx, &plan.nodes);
}

pub(super) fn build_uxtree_a11y_graft_plan(
    snapshot: &UxTreeSnapshot,
    annotations: &[TileAffordanceAnnotation],
    graph_app: &GraphBrowserApp,
) -> UxTreeA11yGraftPlan {
    let mut nodes: Vec<UxTreeA11yNodePlan> = snapshot
        .semantic_nodes
        .iter()
        .map(|node| build_uxtree_a11y_node_plan(node, annotations))
        .collect();
    attach_webview_anchors_to_uxtree_nodes(&mut nodes, snapshot, graph_app);
    nodes.extend(build_graph_reader_a11y_nodes(
        snapshot,
        annotations,
        graph_app,
    ));

    UxTreeA11yGraftPlan {
        anchor_label: uxtree_accessibility_anchor_label(snapshot),
        nodes,
    }
}

fn uxtree_accessibility_anchor_id() -> egui::Id {
    egui::Id::new("uxtree_accessibility_anchor")
}

pub(super) fn uxtree_accessibility_node_id(ux_node_id: &str) -> egui::Id {
    egui::Id::new(("uxtree_accessibility_node", ux_node_id))
}

fn uxtree_accessibility_anchor_label(snapshot: &UxTreeSnapshot) -> String {
    let focused_label = snapshot
        .semantic_nodes
        .iter()
        .find(|node| node.state.focused)
        .map(|node| node.label.as_str())
        .unwrap_or("none");
    format!(
        "Workbench semantic projection ({} node(s); focused: {focused_label})",
        snapshot.semantic_nodes.len()
    )
}

fn build_uxtree_a11y_node_plan(
    node: &UxSemanticNode,
    annotations: &[TileAffordanceAnnotation],
) -> UxTreeA11yNodePlan {
    let affordance = projected_affordance_for_uxtree_node(node, annotations);
    UxTreeA11yNodePlan {
        ux_node_id: node.ux_node_id.clone(),
        parent_ux_node_id: node.parent_ux_node_id.clone(),
        role: map_uxtree_role_to_accesskit_role(node.role),
        label: node.label.clone(),
        description: uxtree_affordance_description(affordance.as_ref()),
        state_description: uxtree_state_description(node, affordance.as_ref()),
        selected: node.state.selected,
        busy: node.state.blocked
            || affordance
                .as_ref()
                .is_some_and(|projection| projection.aria_busy),
        disabled: node.state.blocked,
        action_route: None,
        attached_child_ids: Vec::new(),
    }
}

fn attach_webview_anchors_to_uxtree_nodes(
    nodes: &mut [UxTreeA11yNodePlan],
    snapshot: &UxTreeSnapshot,
    graph_app: &GraphBrowserApp,
) {
    let mut attachments: HashMap<String, Vec<egui::Id>> = HashMap::new();
    for (webview_id, node_key) in graph_app.webview_node_mappings() {
        let Some(parent_ux_node_id) = find_webview_parent_ux_node_id(snapshot, node_key) else {
            continue;
        };
        attachments
            .entry(parent_ux_node_id)
            .or_default()
            .push(webview_accessibility_anchor_id(webview_id));
    }

    for node in nodes {
        if let Some(child_ids) = attachments.remove(&node.ux_node_id) {
            node.attached_child_ids.extend(child_ids);
        }
    }
}

fn find_webview_parent_ux_node_id(snapshot: &UxTreeSnapshot, node_key: NodeKey) -> Option<String> {
    snapshot
        .semantic_nodes
        .iter()
        .find(|node| {
            node.role == UxNodeRole::NodePane
                && matches!(node.domain, UxDomainIdentity::Node { node_key: mapped } if mapped == node_key)
        })
        .or_else(|| {
            snapshot.semantic_nodes.iter().find(|node| {
                node.role == UxNodeRole::GraphNode
                    && matches!(node.domain, UxDomainIdentity::Node { node_key: mapped } if mapped == node_key)
            })
        })
        .map(|node| node.ux_node_id.clone())
}

fn build_graph_reader_a11y_nodes(
    snapshot: &UxTreeSnapshot,
    annotations: &[TileAffordanceAnnotation],
    graph_app: &GraphBrowserApp,
) -> Vec<UxTreeA11yNodePlan> {
    let Some((graph_surface_id, graph_view_id)) = focused_graph_surface(snapshot, graph_app) else {
        return Vec::new();
    };

    let mut nodes = Vec::new();
    let map_root_id = format!("a11y://graph-reader/{graph_view_id:?}/map");
    let map_node_keys = sorted_graph_reader_node_keys(graph_app);
    nodes.push(UxTreeA11yNodePlan {
        ux_node_id: map_root_id.clone(),
        parent_ux_node_id: Some(graph_surface_id.clone()),
        role: egui::accesskit::Role::Tree,
        label: format!("Graph Reader Map ({} nodes)", map_node_keys.len()),
        description: Some(
            "Deterministic graph linearization entry point for keyboard and screen-reader traversal."
                .to_string(),
        ),
        state_description: Some("partial".to_string()),
        selected: false,
        busy: false,
        disabled: false,
        action_route: None,
        attached_child_ids: Vec::new(),
    });
    for node_key in map_node_keys {
        if let Some(item_plan) =
            build_graph_reader_map_item_plan(&map_root_id, node_key, graph_app, annotations)
        {
            nodes.push(item_plan);
        }
    }

    if let Some(GraphReaderModeState::Room { node_key, .. }) = graph_app.graph_reader_mode() {
        nodes.extend(build_graph_reader_room_nodes(
            &graph_surface_id,
            graph_view_id,
            node_key,
            graph_app,
            annotations,
        ));
    }

    nodes
}

fn focused_graph_surface(
    snapshot: &UxTreeSnapshot,
    graph_app: &GraphBrowserApp,
) -> Option<(String, GraphViewId)> {
    let focused_view = graph_app.workspace.graph_runtime.focused_view;
    snapshot
        .semantic_nodes
        .iter()
        .find_map(|node| match node.domain {
            UxDomainIdentity::GraphView { graph_view_id }
                if node.role == UxNodeRole::GraphSurface && focused_view == Some(graph_view_id) =>
            {
                Some((node.ux_node_id.clone(), graph_view_id))
            }
            _ => None,
        })
        .or_else(|| {
            snapshot
                .semantic_nodes
                .iter()
                .find_map(|node| match node.domain {
                    UxDomainIdentity::GraphView { graph_view_id }
                        if node.role == UxNodeRole::GraphSurface =>
                    {
                        Some((node.ux_node_id.clone(), graph_view_id))
                    }
                    _ => None,
                })
        })
}

fn sorted_graph_reader_node_keys(graph_app: &GraphBrowserApp) -> Vec<NodeKey> {
    let mut keys: Vec<NodeKey> = graph_app
        .domain_graph()
        .nodes()
        .map(|(key, _)| key)
        .collect();
    keys.sort_by_key(|key| key.index());
    keys
}

fn build_graph_reader_map_item_plan(
    map_root_id: &str,
    node_key: NodeKey,
    graph_app: &GraphBrowserApp,
    annotations: &[TileAffordanceAnnotation],
) -> Option<UxTreeA11yNodePlan> {
    let node = graph_app.domain_graph().get_node(node_key)?;
    let affordance = selected_node_affordance_projection_from_annotations(node_key, annotations);
    let label = if node.title.is_empty() {
        node.url().to_string()
    } else {
        node.title.clone()
    };
    Some(UxTreeA11yNodePlan {
        ux_node_id: format!("{map_root_id}/node/{node_key:?}"),
        parent_ux_node_id: Some(map_root_id.to_string()),
        role: egui::accesskit::Role::TreeItem,
        label,
        description: uxtree_affordance_description(affordance.as_ref()),
        state_description: graph_reader_node_state_description(
            node_key,
            graph_app,
            affordance.as_ref(),
        ),
        selected: graph_app.focused_selection().contains(&node_key),
        busy: graph_app.runtime_block_state_for_node(node_key).is_some(),
        disabled: false,
        action_route: Some(UxTreeA11yActionRoute::GraphReaderMapItem { node_key }),
        attached_child_ids: Vec::new(),
    })
}

fn build_graph_reader_room_nodes(
    graph_surface_id: &str,
    graph_view_id: GraphViewId,
    selected_key: NodeKey,
    graph_app: &GraphBrowserApp,
    annotations: &[TileAffordanceAnnotation],
) -> Vec<UxTreeA11yNodePlan> {
    let Some(node) = graph_app.domain_graph().get_node(selected_key) else {
        return Vec::new();
    };

    let mut nodes = Vec::new();
    let label = if node.title.is_empty() {
        node.url().to_string()
    } else {
        node.title.clone()
    };
    let room_root_id = format!("a11y://graph-reader/{graph_view_id:?}/room/{selected_key:?}");
    nodes.push(UxTreeA11yNodePlan {
        ux_node_id: room_root_id.clone(),
        parent_ux_node_id: Some(graph_surface_id.to_string()),
        role: egui::accesskit::Role::Group,
        label: format!("Graph Reader Room: {label}"),
        description: Some("Focused node room context with grouped connected doors.".to_string()),
        state_description: Some("partial".to_string()),
        selected: false,
        busy: false,
        disabled: false,
        action_route: Some(UxTreeA11yActionRoute::GraphReaderRoomRoot),
        attached_child_ids: Vec::new(),
    });

    for (group_label, suffix, group_nodes) in graph_reader_directed_groups(graph_app, selected_key)
    {
        if group_nodes.is_empty() {
            continue;
        }
        let group_id = format!("{room_root_id}/{suffix}");
        nodes.push(UxTreeA11yNodePlan {
            ux_node_id: group_id.clone(),
            parent_ux_node_id: Some(room_root_id.clone()),
            role: egui::accesskit::Role::Group,
            label: format!("{group_label} ({})", group_nodes.len()),
            description: None,
            state_description: None,
            selected: false,
            busy: false,
            disabled: false,
            action_route: None,
            attached_child_ids: Vec::new(),
        });
        for neighbor in group_nodes {
            if let Some(item_plan) =
                build_graph_reader_room_item_plan(&group_id, neighbor, graph_app, annotations)
            {
                nodes.push(item_plan);
            }
        }
    }

    nodes
}

fn graph_reader_directed_groups(
    graph_app: &GraphBrowserApp,
    selected_key: NodeKey,
) -> Vec<(&'static str, &'static str, Vec<NodeKey>)> {
    let mut outgoing: Vec<NodeKey> = graph_app
        .domain_graph()
        .out_neighbors(selected_key)
        .collect();
    let mut incoming: Vec<NodeKey> = graph_app
        .domain_graph()
        .in_neighbors(selected_key)
        .collect();
    outgoing.sort_by_key(|key| key.index());
    incoming.sort_by_key(|key| key.index());

    let outgoing_set: HashSet<NodeKey> = outgoing.iter().copied().collect();
    let incoming_set: HashSet<NodeKey> = incoming.iter().copied().collect();

    let mut bidirectional: Vec<NodeKey> =
        outgoing_set.intersection(&incoming_set).copied().collect();
    bidirectional.sort_by_key(|key| key.index());

    let outgoing_only: Vec<NodeKey> = outgoing
        .into_iter()
        .filter(|key| !incoming_set.contains(key))
        .collect();
    let incoming_only: Vec<NodeKey> = incoming
        .into_iter()
        .filter(|key| !outgoing_set.contains(key))
        .collect();

    vec![
        ("Bidirectional Doors", "bidirectional", bidirectional),
        ("Outgoing Doors", "outgoing", outgoing_only),
        ("Incoming Doors", "incoming", incoming_only),
    ]
}

fn build_graph_reader_room_item_plan(
    group_id: &str,
    node_key: NodeKey,
    graph_app: &GraphBrowserApp,
    annotations: &[TileAffordanceAnnotation],
) -> Option<UxTreeA11yNodePlan> {
    let node = graph_app.domain_graph().get_node(node_key)?;
    let affordance = selected_node_affordance_projection_from_annotations(node_key, annotations);
    let label = if node.title.is_empty() {
        node.url().to_string()
    } else {
        node.title.clone()
    };
    Some(UxTreeA11yNodePlan {
        ux_node_id: format!("{group_id}/node/{node_key:?}"),
        parent_ux_node_id: Some(group_id.to_string()),
        role: egui::accesskit::Role::TreeItem,
        label,
        description: uxtree_affordance_description(affordance.as_ref()),
        state_description: graph_reader_node_state_description(
            node_key,
            graph_app,
            affordance.as_ref(),
        ),
        selected: false,
        busy: graph_app.runtime_block_state_for_node(node_key).is_some(),
        disabled: false,
        action_route: Some(UxTreeA11yActionRoute::GraphReaderRoomItem { node_key }),
        attached_child_ids: Vec::new(),
    })
}

pub(super) fn resolve_uxtree_accesskit_action(
    graph_app: &GraphBrowserApp,
    req: &egui::accesskit::ActionRequest,
) -> Option<UxTreeAccesskitDispatch> {
    let snapshot = ux_tree::latest_snapshot()?;
    let annotations = tile_compositor::latest_tile_affordance_annotations();
    let plan = build_uxtree_a11y_graft_plan(&snapshot, &annotations, graph_app);
    resolve_uxtree_accesskit_action_for_plan(&plan, req)
}

pub(super) fn resolve_uxtree_accesskit_action_for_plan(
    plan: &UxTreeA11yGraftPlan,
    req: &egui::accesskit::ActionRequest,
) -> Option<UxTreeAccesskitDispatch> {
    let node = plan.nodes.iter().find(|node| {
        accesskit_node_id_from_egui_id(uxtree_accessibility_node_id(&node.ux_node_id))
            == req.target_node
    })?;

    match (node.action_route, req.action) {
        (Some(UxTreeA11yActionRoute::GraphReaderMapItem { node_key }), Action::Focus) => {
            Some(UxTreeAccesskitDispatch::FocusGraphReaderMapItem { node_key })
        }
        (Some(UxTreeA11yActionRoute::GraphReaderMapItem { node_key }), Action::Click) => {
            Some(UxTreeAccesskitDispatch::EnterGraphReaderRoom { node_key })
        }
        (Some(UxTreeA11yActionRoute::GraphReaderRoomRoot), Action::Focus)
        | (Some(UxTreeA11yActionRoute::GraphReaderRoomItem { .. }), Action::Focus) => {
            Some(UxTreeAccesskitDispatch::FocusGraphSurface)
        }
        (Some(UxTreeA11yActionRoute::GraphReaderRoomRoot), Action::Click) => {
            Some(UxTreeAccesskitDispatch::ReturnGraphReaderToMap)
        }
        (Some(UxTreeA11yActionRoute::GraphReaderRoomItem { node_key }), Action::Click) => {
            Some(UxTreeAccesskitDispatch::EnterGraphReaderRoom { node_key })
        }
        _ if node.ux_node_id.starts_with("a11y://graph-reader/") => {
            warn!(
                "uxtree_accessibility: unsupported graph reader action {:?} for {}",
                req.action, node.ux_node_id
            );
            Some(UxTreeAccesskitDispatch::Unsupported)
        }
        _ => None,
    }
}

fn graph_reader_node_state_description(
    node_key: NodeKey,
    graph_app: &GraphBrowserApp,
    affordance: Option<&TileAffordanceAccessibilityProjection>,
) -> Option<String> {
    let mut states = Vec::new();
    if graph_app.focused_selection().primary() == Some(node_key) {
        states.push("focused-room-target".to_string());
    } else if graph_app.focused_selection().contains(&node_key) {
        states.push("selected".to_string());
    }
    if graph_app.runtime_block_state_for_node(node_key).is_some() {
        states.push("blocked".to_string());
    }
    if graph_app.runtime_crash_state_for_node(node_key).is_some() {
        states.push("degraded".to_string());
    }
    if let Some(affordance) = affordance {
        for token in &affordance.status_tokens {
            let rendered_token = format!("rendered {token}");
            if !states.iter().any(|state| state == &rendered_token) {
                states.push(rendered_token);
            }
        }
    }

    (!states.is_empty()).then(|| states.join(", "))
}

fn projected_affordance_for_uxtree_node(
    node: &UxSemanticNode,
    annotations: &[TileAffordanceAnnotation],
) -> Option<TileAffordanceAccessibilityProjection> {
    match &node.domain {
        UxDomainIdentity::Node { node_key } => {
            selected_node_affordance_projection_from_annotations(*node_key, annotations)
        }
        _ => None,
    }
}

fn map_uxtree_role_to_accesskit_role(role: UxNodeRole) -> egui::accesskit::Role {
    match role {
        UxNodeRole::Workbench => egui::accesskit::Role::GenericContainer,
        UxNodeRole::SplitContainer => egui::accesskit::Role::Group,
        UxNodeRole::TabContainer => egui::accesskit::Role::TabList,
        UxNodeRole::GraphSurface => egui::accesskit::Role::ScrollView,
        UxNodeRole::GraphNode => egui::accesskit::Role::TreeItem,
        UxNodeRole::NodePane => egui::accesskit::Role::Pane,
        UxNodeRole::RadialPalette => egui::accesskit::Role::Group,
        UxNodeRole::RadialTierRing => egui::accesskit::Role::Group,
        UxNodeRole::RadialSector => egui::accesskit::Role::MenuItem,
        UxNodeRole::RadialSummary => egui::accesskit::Role::Status,
        UxNodeRole::GraphViewLensScope => egui::accesskit::Role::Group,
        UxNodeRole::NavigatorProjection => egui::accesskit::Role::Tree,
        UxNodeRole::FileTreeProjection => egui::accesskit::Role::Tree,
        UxNodeRole::RouteOpenBoundary => egui::accesskit::Role::Group,
        #[cfg(feature = "diagnostics")]
        UxNodeRole::ToolPane => egui::accesskit::Role::Pane,
    }
}

fn uxtree_state_description(
    node: &UxSemanticNode,
    affordance: Option<&TileAffordanceAccessibilityProjection>,
) -> Option<String> {
    let mut states = Vec::new();
    if node.state.focused {
        states.push("focused".to_string());
    }
    if node.state.selected {
        states.push("selected".to_string());
    }
    if node.state.blocked {
        states.push("blocked".to_string());
    }
    if node.state.degraded {
        states.push("degraded".to_string());
    }
    if let Some(affordance) = affordance {
        for token in &affordance.status_tokens {
            let rendered_token = format!("rendered {token}");
            if !states.iter().any(|state| state == &rendered_token) {
                states.push(rendered_token);
            }
        }
    }

    (!states.is_empty()).then(|| states.join(", "))
}

fn uxtree_affordance_description(
    affordance: Option<&TileAffordanceAccessibilityProjection>,
) -> Option<String> {
    let affordance = affordance?;
    let mut details = Vec::new();
    if affordance.lifecycle_label != "active" {
        details.push(format!(
            "rendered lifecycle: {}",
            affordance.lifecycle_label
        ));
    }
    if !affordance.glyph_descriptions.is_empty() {
        details.push(format!(
            "rendered glyphs: {}",
            affordance.glyph_descriptions.join(", ")
        ));
    }

    (!details.is_empty()).then(|| details.join("; "))
}

pub(super) struct WebViewA11yNodePlan {
    pub(super) node_id: NodeId,
    pub(super) role: egui::accesskit::Role,
    pub(super) label: Option<String>,
}

pub(super) struct WebViewA11yGraftPlan {
    pub(super) anchor_label: String,
    pub(super) root_node_id: Option<NodeId>,
    pub(super) nodes: Vec<WebViewA11yNodePlan>,
    pub(super) dropped_node_count: usize,
    pub(super) conversion_fallback_count: usize,
}

pub(super) fn webview_accessibility_anchor_id(webview_id: WebViewId) -> egui::Id {
    egui::Id::new(webview_accessibility_anchor_id_key(webview_id))
}

fn webview_accessibility_anchor_id_key(webview_id: WebViewId) -> (&'static str, WebViewId) {
    ("webview_accessibility_anchor", webview_id)
}

fn webview_accessibility_node_id(webview_id: WebViewId, node_id: NodeId) -> egui::Id {
    egui::Id::new(webview_accessibility_node_id_key(webview_id, node_id))
}

fn webview_accessibility_node_id_key(
    webview_id: WebViewId,
    node_id: NodeId,
) -> (&'static str, WebViewId, u64) {
    ("webview_accessibility_node", webview_id, node_id.0)
}

fn is_reserved_webview_accessibility_node_id(node_id: NodeId) -> bool {
    is_webview_accessibility_root_node_id_value(node_id.0)
        || is_webview_accessibility_max_node_id_value(node_id.0)
}

fn is_webview_accessibility_root_node_id_value(node_id_value: u64) -> bool {
    node_id_value == 0
}

fn is_webview_accessibility_max_node_id_value(node_id_value: u64) -> bool {
    node_id_value == u64::MAX
}

pub(super) fn webview_accessibility_label(
    webview_id: WebViewId,
    tree_update: &TreeUpdate,
) -> String {
    if let Some(label) = focused_webview_accessibility_label(tree_update) {
        return format_embedded_web_content_label(label);
    }

    if let Some(label) = first_nonempty_webview_accessibility_label(tree_update) {
        return format_embedded_web_content_label(label);
    }

    format_webview_accessibility_fallback_label(webview_id, tree_update.nodes.len())
}

fn focused_webview_accessibility_label(tree_update: &TreeUpdate) -> Option<&str> {
    tree_update
        .nodes
        .iter()
        .find(|(node_id, _)| *node_id == tree_update.focus)
        .and_then(|(_, node)| node.label())
        .filter(|label| !label.trim().is_empty())
}

fn format_embedded_web_content_label(label: &str) -> String {
    format!("Embedded web content: {label}")
}

fn first_nonempty_webview_accessibility_label(tree_update: &TreeUpdate) -> Option<&str> {
    tree_update
        .nodes
        .iter()
        .find_map(|(_, node)| node.label().filter(|label| !label.trim().is_empty()))
}

fn format_webview_accessibility_fallback_label(webview_id: WebViewId, node_count: usize) -> String {
    format!(
        "Embedded web content (webview {:?}, {} accessibility node update(s))",
        webview_id, node_count
    )
}

fn convert_webview_accessibility_role(role: Role) -> (egui::accesskit::Role, bool) {
    let role_name = webview_accessibility_role_name(role);
    if let Some(mapped) = map_known_webview_accessibility_role_name(role_name.as_str()) {
        (mapped, false)
    } else {
        fallback_webview_accessibility_role()
    }
}

fn map_known_webview_accessibility_role_name(role_name: &str) -> Option<egui::accesskit::Role> {
    match role_name {
        "Document" => Some(egui::accesskit::Role::Document),
        "Paragraph" => Some(egui::accesskit::Role::Paragraph),
        "Label" => Some(egui::accesskit::Role::Label),
        "Link" => Some(egui::accesskit::Role::Link),
        "List" => Some(egui::accesskit::Role::List),
        "ListItem" => Some(egui::accesskit::Role::ListItem),
        "Heading" => Some(egui::accesskit::Role::Heading),
        "Image" => Some(egui::accesskit::Role::Image),
        "Button" => Some(egui::accesskit::Role::Button),
        "TextInput" => Some(egui::accesskit::Role::TextInput),
        "StaticText" => Some(egui::accesskit::Role::Label),
        "Unknown" => Some(egui::accesskit::Role::Unknown),
        _ => None,
    }
}

fn webview_accessibility_role_name(role: Role) -> String {
    format!("{role:?}")
}

fn fallback_webview_accessibility_role() -> (egui::accesskit::Role, bool) {
    (egui::accesskit::Role::GenericContainer, true)
}

pub(super) fn build_webview_a11y_graft_plan(
    webview_id: WebViewId,
    tree_update: &TreeUpdate,
) -> WebViewA11yGraftPlan {
    let allowed_node_ids = collect_allowed_webview_a11y_node_ids(tree_update);
    let (nodes, conversion_fallback_count) =
        build_webview_a11y_node_plans(tree_update, &allowed_node_ids);

    let root_node_id =
        select_webview_a11y_root_node_id(&allowed_node_ids, tree_update.focus, &nodes);

    compose_webview_a11y_graft_plan(
        webview_id,
        tree_update,
        nodes,
        root_node_id,
        conversion_fallback_count,
    )
}

fn compose_webview_a11y_graft_plan(
    webview_id: WebViewId,
    tree_update: &TreeUpdate,
    nodes: Vec<WebViewA11yNodePlan>,
    root_node_id: Option<NodeId>,
    conversion_fallback_count: usize,
) -> WebViewA11yGraftPlan {
    let dropped_node_count = tree_update.nodes.len().saturating_sub(nodes.len());
    WebViewA11yGraftPlan {
        anchor_label: webview_accessibility_label(webview_id, tree_update),
        root_node_id,
        dropped_node_count,
        conversion_fallback_count,
        nodes,
    }
}

fn build_webview_a11y_node_plans(
    tree_update: &TreeUpdate,
    allowed_node_ids: &HashSet<NodeId>,
) -> (Vec<WebViewA11yNodePlan>, usize) {
    let mut nodes = Vec::with_capacity(allowed_node_ids.len());
    let mut conversion_fallback_count = 0;
    for (node_id, node) in &tree_update.nodes {
        if !allowed_node_ids.contains(node_id) {
            continue;
        }

        let (role, used_fallback) = convert_webview_accessibility_role(node.role());
        if used_fallback {
            conversion_fallback_count += 1;
        }

        let label = normalized_webview_a11y_node_label(node);
        nodes.push(WebViewA11yNodePlan {
            node_id: *node_id,
            role,
            label,
        });
    }

    (nodes, conversion_fallback_count)
}

fn collect_allowed_webview_a11y_node_ids(tree_update: &TreeUpdate) -> HashSet<NodeId> {
    let mut allowed_node_ids = HashSet::with_capacity(tree_update.nodes.len());
    for (node_id, _) in &tree_update.nodes {
        if !is_reserved_webview_accessibility_node_id(*node_id) {
            allowed_node_ids.insert(*node_id);
        }
    }
    allowed_node_ids
}

fn normalized_webview_a11y_node_label(node: &Node) -> Option<String> {
    node.label()
        .map(str::trim)
        .filter(|label| !label.is_empty())
        .map(str::to_owned)
}

fn select_webview_a11y_root_node_id(
    allowed_node_ids: &HashSet<NodeId>,
    focus_node_id: NodeId,
    nodes: &[WebViewA11yNodePlan],
) -> Option<NodeId> {
    if allowed_node_ids.contains(&focus_node_id) {
        Some(focus_node_id)
    } else {
        nodes.first().map(|node| node.node_id)
    }
}

pub(super) fn inject_webview_a11y_updates(
    ctx: &Context,
    pending: &mut HashMap<WebViewId, TreeUpdate>,
) {
    if has_no_pending_webview_a11y_updates(pending) {
        return;
    }

    inject_all_pending_webview_a11y_updates(ctx, pending);

    #[cfg(feature = "diagnostics")]
    record_webview_a11y_queue_drained();
}

fn inject_all_pending_webview_a11y_updates(
    ctx: &Context,
    pending: &mut HashMap<WebViewId, TreeUpdate>,
) {
    for (webview_id, tree_update) in pending.drain() {
        inject_single_webview_a11y_update(ctx, webview_id, &tree_update);
    }
}

fn has_no_pending_webview_a11y_updates(pending: &HashMap<WebViewId, TreeUpdate>) -> bool {
    pending.is_empty()
}

fn inject_single_webview_a11y_update(
    ctx: &Context,
    webview_id: WebViewId,
    tree_update: &TreeUpdate,
) {
    let plan = build_webview_a11y_graft_plan(webview_id, tree_update);
    let anchor_id = webview_accessibility_anchor_id(webview_id);

    #[cfg(feature = "diagnostics")]
    record_webview_a11y_plan_degradation(&plan);

    inject_webview_a11y_plan_nodes(ctx, webview_id, &plan.nodes);
    inject_webview_a11y_anchor_node(
        ctx,
        anchor_id,
        &plan.anchor_label,
        plan.root_node_id
            .map(|node_id| {
                vec![accesskit_node_id_from_egui_id(
                    webview_accessibility_node_id(webview_id, node_id),
                )]
            })
            .unwrap_or_default(),
    );
    warn_webview_a11y_plan_degradation(webview_id, &plan);
}

fn warn_webview_a11y_plan_degradation(webview_id: WebViewId, plan: &WebViewA11yGraftPlan) {
    warn_webview_a11y_document_root_degradation(webview_id, plan);
    warn_webview_a11y_dropped_nodes(webview_id, plan.dropped_node_count);
    warn_webview_a11y_role_conversion_fallback(webview_id, plan.conversion_fallback_count);
}

fn warn_webview_a11y_document_root_degradation(webview_id: WebViewId, plan: &WebViewA11yGraftPlan) {
    if plan.nodes.is_empty() {
        warn!(
            "Runtime viewer accessibility injection used degraded synthesized document node for {:?}: incoming tree update had no nodes",
            webview_id
        );
    } else if plan.root_node_id.is_none() {
        warn!(
            "Runtime viewer accessibility injection used degraded synthesized document node for {:?}: no injectable root node was found",
            webview_id
        );
    }
}

fn warn_webview_a11y_dropped_nodes(webview_id: WebViewId, dropped_node_count: usize) {
    if dropped_node_count > 0 {
        warn!(
            "Runtime viewer accessibility injection dropped {} reserved node(s) for {:?}",
            dropped_node_count, webview_id
        );
    }
}

fn warn_webview_a11y_role_conversion_fallback(
    webview_id: WebViewId,
    conversion_fallback_count: usize,
) {
    if conversion_fallback_count > 0 {
        warn!(
            "Runtime viewer accessibility injection used degraded role conversion fallback for {} node(s) in {:?}",
            conversion_fallback_count, webview_id
        );
    }
}

fn inject_webview_a11y_anchor_node(
    ctx: &Context,
    anchor_id: egui::Id,
    anchor_label: &str,
    child_node_ids: Vec<NodeId>,
) {
    ctx.accesskit_node_builder(anchor_id, |builder| {
        builder.set_role(egui::accesskit::Role::Document);
        builder.set_label(anchor_label.to_owned());
        if !child_node_ids.is_empty() {
            builder.set_children(child_node_ids);
        }
    });
}

fn inject_uxtree_a11y_anchor_node(
    ctx: &Context,
    anchor_id: egui::Id,
    anchor_label: &str,
    child_node_ids: Vec<NodeId>,
) {
    ctx.accesskit_node_builder(anchor_id, |builder| {
        builder.set_role(egui::accesskit::Role::GenericContainer);
        builder.set_label(anchor_label.to_owned());
        if !child_node_ids.is_empty() {
            builder.set_children(child_node_ids);
        }
    });
}

fn inject_uxtree_a11y_plan_nodes(ctx: &Context, nodes: &[UxTreeA11yNodePlan]) {
    for node in nodes {
        inject_uxtree_a11y_plan_node(ctx, nodes, node);
    }
}

fn inject_uxtree_a11y_plan_node(
    ctx: &Context,
    nodes: &[UxTreeA11yNodePlan],
    node: &UxTreeA11yNodePlan,
) {
    let node_id = uxtree_accessibility_node_id(&node.ux_node_id);
    let role = node.role;
    let label = node.label.clone();
    let description = node.description.clone();
    let state_description = node.state_description.clone();
    let selected = node.selected;
    let busy = node.busy;
    let disabled = node.disabled;
    let child_node_ids =
        collect_child_accesskit_node_ids(nodes, &node.ux_node_id, &node.attached_child_ids);

    ctx.accesskit_node_builder(node_id, |builder| {
        builder.set_role(role);
        builder.set_label(label.clone());
        if let Some(description) = &description {
            builder.set_description(description.clone());
        }
        if let Some(state_description) = &state_description {
            builder.set_state_description(state_description.clone());
        }
        if selected {
            builder.set_selected(true);
        }
        if busy {
            builder.set_busy();
        }
        if disabled {
            builder.set_disabled();
        }
        if node.action_route.is_some() {
            builder.add_action(Action::Focus);
            builder.add_action(Action::Click);
        }
        if !child_node_ids.is_empty() {
            builder.set_children(child_node_ids.clone());
        }
    });
}

fn collect_root_child_accesskit_node_ids(nodes: &[UxTreeA11yNodePlan]) -> Vec<NodeId> {
    nodes
        .iter()
        .filter(|node| node.parent_ux_node_id.is_none())
        .map(|node| accesskit_node_id_from_egui_id(uxtree_accessibility_node_id(&node.ux_node_id)))
        .collect()
}

fn collect_child_accesskit_node_ids(
    nodes: &[UxTreeA11yNodePlan],
    parent_ux_node_id: &str,
    attached_child_ids: &[egui::Id],
) -> Vec<NodeId> {
    let mut child_ids: Vec<NodeId> = nodes
        .iter()
        .filter(|node| node.parent_ux_node_id.as_deref() == Some(parent_ux_node_id))
        .map(|node| accesskit_node_id_from_egui_id(uxtree_accessibility_node_id(&node.ux_node_id)))
        .collect();
    child_ids.extend(
        attached_child_ids
            .iter()
            .copied()
            .map(accesskit_node_id_from_egui_id),
    );
    child_ids
}

pub(super) fn accesskit_node_id_from_egui_id(id: egui::Id) -> NodeId {
    NodeId(id.value())
}

fn inject_webview_a11y_plan_nodes(
    ctx: &Context,
    webview_id: WebViewId,
    nodes: &[WebViewA11yNodePlan],
) {
    for node in nodes {
        inject_webview_a11y_plan_node(ctx, webview_id, node);
    }
}

fn inject_webview_a11y_plan_node(ctx: &Context, webview_id: WebViewId, node: &WebViewA11yNodePlan) {
    let node_id = webview_accessibility_node_id(webview_id, node.node_id);
    let role = node.role;
    let label = node.label.clone();

    ctx.accesskit_node_builder(node_id, |builder| {
        builder.set_role(role);
        if let Some(label) = &label {
            builder.set_label(label.clone());
        }
    });
}
