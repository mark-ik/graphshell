/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::{HashMap, HashSet};

use accesskit::{Node, NodeId, Role, TreeUpdate};
use egui::Context;
use log::warn;
use servo::WebViewId;

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

pub(super) fn webview_accessibility_label(webview_id: WebViewId, tree_update: &TreeUpdate) -> String {
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

    let root_node_id = select_webview_a11y_root_node_id(&allowed_node_ids, tree_update.focus, &nodes);

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

fn inject_single_webview_a11y_update(ctx: &Context, webview_id: WebViewId, tree_update: &TreeUpdate) {
    let plan = build_webview_a11y_graft_plan(webview_id, tree_update);
    let anchor_id = webview_accessibility_anchor_id(webview_id);

    inject_webview_a11y_plan_nodes(ctx, webview_id, &plan.nodes);
    inject_webview_a11y_anchor_node(ctx, anchor_id, &plan.anchor_label);
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

fn inject_webview_a11y_anchor_node(ctx: &Context, anchor_id: egui::Id, anchor_label: &str) {
    ctx.accesskit_node_builder(anchor_id, |builder| {
        builder.set_role(egui::accesskit::Role::Document);
        builder.set_label(anchor_label.to_owned());
    });
}

fn inject_webview_a11y_plan_nodes(ctx: &Context, webview_id: WebViewId, nodes: &[WebViewA11yNodePlan]) {
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
