/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Graph rendering module routed through the portable `graph-canvas` seam.
//!
//! `render_graph_canvas_in_ui` is the live graph-pane entry point: it
//! delegates scene derivation, hit testing, and interaction to
//! `graph-canvas` via `canvas_bridge::run_graph_canvas_frame`, and uses
//! `canvas_egui_painter` / `canvas_bridge::collect_canvas_events` as the
//! host-local paint + input adapters. The retired `egui_graphs` path is
//! preserved only in historical references.

use crate::app::{
    ChooseFramePickerMode, GraphBrowserApp, GraphIntent, GraphTooltipTarget, SearchDisplayMode,
    SelectionUpdateMode, SimulateBehaviorPreset, UnsavedFramePromptAction,
    UnsavedFramePromptRequest, ViewAction,
};
use crate::graph::scene_runtime::SceneCollisionPolicy;
use crate::graph::{EdgeType, NodeKey};
use crate::registries::domain::layout::canvas::CanvasLassoBinding;
use crate::shell::desktop::runtime::diagnostics::{DiagnosticEvent, emit_event};
use crate::shell::desktop::runtime::registries::{
    CHANNEL_UI_GRAPH_SELECTION_AMBIGUOUS_HIT, phase3_resolve_active_theme,
};
use crate::shell::desktop::ui::toolbar::toolbar_ui::CommandBarFocusTarget;
use crate::shell::desktop::ui::toolbar_routing;
use crate::util::{GraphshellSettingsPath, VersoAddress};
use egui::{Frame, Ui, Window};
use euclid::default::Point2D;
use graph_canvas::packet::HitProxy;
use std::collections::{HashMap, HashSet};

pub(crate) mod canvas_bridge;
pub(crate) mod canvas_egui_painter;
mod canvas_visuals;
mod graph_info;
mod panels;
mod reducer_bridge;
pub(crate) mod semantic_tags;
mod spatial_index;
#[cfg(test)]
use crate::app::{ThreeDMode, ViewDimension, ZSource};
#[cfg(test)]
use crate::graph::NodeLifecycle;
#[cfg(test)]
use canvas_visuals::filtered_graph_for_search;
use canvas_visuals::visible_nodes_for_view_filters;
#[cfg(test)]
use canvas_visuals::{
    canvas_rect_from_view_frame, effective_graph_screen_rect, graph_visible_screen_rects,
    hovered_adjacency_set, lifecycle_color, viewport_culled_graph_for_canvas_rect,
    viewport_culling_metrics_for_canvas_rect, viewport_culling_selection_for_canvas_rect,
};
use graph_info::draw_graph_info;
#[cfg(test)]
use graph_info::{graph_view_semantic_depth_status_badge, selected_node_enrichment_summary};
#[cfg(test)]
pub(crate) use panels::history_manager_entry_limit_for_tests;
pub use panels::{
    render_clip_inspector_panel, render_help_panel, render_history_manager_in_ui,
    render_navigator_tool_pane_in_ui, render_scene_overlay_panel,
    render_settings_node_viewer_in_ui, render_settings_overlay_panel,
    render_settings_tool_pane_in_ui_with_control_panel,
};
use reducer_bridge::{apply_reducer_graph_intents_hardened, apply_ui_intents_with_checkpoint};
#[cfg(test)]
use semantic_tags::{ranked_tag_suggestions, reserved_tag_warning};
#[cfg(test)]
use std::time::{Duration, SystemTime};

pub(crate) mod action_registry;
mod command_palette;
mod command_profile;
pub(crate) mod radial_menu;

pub(crate) fn dispatch_action_id(
    app: &mut GraphBrowserApp,
    action_id: action_registry::ActionId,
    pair_context: Option<(NodeKey, NodeKey)>,
    source_context: Option<NodeKey>,
    focused_pane_node: Option<NodeKey>,
    focused_pane_id: Option<crate::shell::desktop::workbench::pane_model::PaneId>,
) {
    let mut intents = Vec::new();
    command_palette::execute_action(
        app,
        action_id,
        pair_context,
        source_context,
        &mut intents,
        focused_pane_node,
        focused_pane_id,
    );
    apply_ui_intents_with_checkpoint(app, intents);
}

pub use command_palette::render_command_palette_panel;
pub use radial_menu::render_radial_command_menu;

/// Graph interaction action (resolved from graph-canvas engine actions).
///
/// Decouples action production (graph-canvas `CanvasAction` emission inside
/// `canvas_bridge::run_graph_canvas_frame`) from action application (pure
/// state mutation here), keeping graph interactions testable without an
/// egui rendering context.
pub enum GraphAction {
    FocusNode(NodeKey),
    FocusFrame {
        frame_name: String,
        member_key: NodeKey,
    },
    FocusNodeSplit(NodeKey),
    DragStart,
    DragEnd(NodeKey, Point2D<f32>),
    MoveNode(NodeKey, Point2D<f32>),
    SelectNode {
        key: NodeKey,
        multi_select: bool,
    },
    SelectFrame(String),
    LassoSelect {
        keys: Vec<NodeKey>,
        mode: SelectionUpdateMode,
    },
    SetHighlightedEdge {
        from: NodeKey,
        to: NodeKey,
    },
    ClearHighlightedEdge,
    ClearSelection,
    Zoom(f32),
}

fn set_focused_view_with_transition(
    app: &mut GraphBrowserApp,
    focused_view: Option<crate::app::GraphViewId>,
) {
    // Delegate to the canonical setter so the scope-close invariant
    // (action_surface closes on view change) runs consistently.
    app.set_workspace_focused_view_with_transition(focused_view);
}

fn action_handles_primary_click(action: &GraphAction) -> bool {
    matches!(
        action,
        GraphAction::FocusNode(_)
            | GraphAction::FocusFrame { .. }
            | GraphAction::FocusNodeSplit(_)
            | GraphAction::DragStart
            | GraphAction::DragEnd(_, _)
            | GraphAction::MoveNode(_, _)
            | GraphAction::SelectNode { .. }
            | GraphAction::SelectFrame(_)
            | GraphAction::LassoSelect { .. }
            | GraphAction::SetHighlightedEdge { .. }
    )
}

fn should_clear_selection_on_background_click(
    pointer_primary_clicked: bool,
    modifiers: egui::Modifiers,
    hovered_graph_node: Option<NodeKey>,
    graph_handled_primary_click: bool,
    radial_open: bool,
    lasso_active: bool,
) -> bool {
    pointer_primary_clicked
        && !modifiers.any()
        && hovered_graph_node.is_none()
        && !graph_handled_primary_click
        && !radial_open
        && !lasso_active
}

fn scene_arrange_mode_active(app: &GraphBrowserApp, view_id: crate::app::GraphViewId) -> bool {
    app.graph_view_scene_mode(view_id) == crate::app::SceneMode::Arrange
}

fn simulate_scene_collision_policy(
    app: &GraphBrowserApp,
    view_id: crate::app::GraphViewId,
    base: SceneCollisionPolicy,
) -> SceneCollisionPolicy {
    if app.graph_view_scene_mode(view_id) != crate::app::SceneMode::Simulate {
        return base;
    }

    match app.graph_view_simulate_behavior_preset(view_id) {
        SimulateBehaviorPreset::Float => SceneCollisionPolicy {
            node_separation_enabled: true,
            viewport_containment_enabled: true,
            node_padding: base.node_padding.max(6.0),
            region_effect_scale: base.region_effect_scale.max(0.8),
            containment_response_scale: base.containment_response_scale.max(0.45),
        },
        SimulateBehaviorPreset::Packed => SceneCollisionPolicy {
            node_separation_enabled: true,
            viewport_containment_enabled: true,
            node_padding: base.node_padding.max(12.0),
            region_effect_scale: base.region_effect_scale.max(1.1),
            containment_response_scale: base.containment_response_scale.max(1.0),
        },
        SimulateBehaviorPreset::Magnetic => SceneCollisionPolicy {
            node_separation_enabled: true,
            viewport_containment_enabled: true,
            node_padding: base.node_padding.max(8.0),
            region_effect_scale: base.region_effect_scale.max(1.85),
            containment_response_scale: base.containment_response_scale.max(0.7),
        },
    }
}

fn node_key_or_emit_ambiguous_hit(node_key: Option<NodeKey>) -> Option<NodeKey> {
    if node_key.is_none() {
        emit_event(DiagnosticEvent::MessageReceived {
            channel_id: CHANNEL_UI_GRAPH_SELECTION_AMBIGUOUS_HIT,
            latency_us: 0,
        });
    }
    node_key
}

fn handle_hovered_node_secondary_click(
    ctx: &egui::Context,
    app: &mut GraphBrowserApp,
    view_id: crate::app::GraphViewId,
    target: NodeKey,
    pointer: Option<egui::Pos2>,
) {
    match app.context_command_surface_preference() {
        crate::app::ContextCommandSurfacePreference::RadialPalette => {
            app.set_pending_node_context_target(None);
            app.set_pending_frame_context_target(None);
            if app.pending_transient_surface_return_target().is_none() {
                app.set_pending_transient_surface_return_target(Some(
                    crate::app::ToolSurfaceReturnTarget::Graph(view_id),
                ));
            }
            if !app.workspace.chrome_ui.show_radial_menu {
                app.workspace.chrome_ui.surface_state = crate::app::ActionSurfaceState::Radial {
                    scope: crate::app::ActionScope::Graph {
                        view_id,
                        target: crate::app::ScopeTarget::Node(target),
                    },
                    anchor: crate::app::Anchor::TargetNode(target),
                };
                let _ = toolbar_routing::request_radial_menu_toggle(
                    app,
                    CommandBarFocusTarget::new(None, Some(target)),
                );
            }
            if let Some(pointer) = pointer {
                ctx.data_mut(|d| {
                    d.insert_persisted(egui::Id::new("radial_menu_center"), pointer);
                });
            }
        }
        crate::app::ContextCommandSurfacePreference::ContextPalette => {
            app.set_pending_node_context_target(Some(target));
            app.set_pending_frame_context_target(None);
            if app.pending_command_surface_return_target().is_none() {
                app.set_pending_command_surface_return_target(Some(
                    crate::app::ToolSurfaceReturnTarget::Graph(view_id),
                ));
            }
            app.set_context_palette_anchor(pointer.map(|pos| [pos.x, pos.y]));
            app.open_palette_contextual(
                crate::app::ActionScope::Graph {
                    view_id,
                    target: crate::app::ScopeTarget::Node(target),
                },
                crate::app::Anchor::TargetNode(target),
            );
        }
    }
}

fn handle_frame_backdrop_secondary_click(
    ctx: &egui::Context,
    app: &mut GraphBrowserApp,
    view_id: crate::app::GraphViewId,
    frame_name: String,
    pointer: Option<egui::Pos2>,
) {
    match app.context_command_surface_preference() {
        crate::app::ContextCommandSurfacePreference::RadialPalette => {
            app.set_pending_node_context_target(None);
            app.set_pending_frame_context_target(Some(frame_name.clone()));
            if app.pending_transient_surface_return_target().is_none() {
                app.set_pending_transient_surface_return_target(Some(
                    crate::app::ToolSurfaceReturnTarget::Graph(view_id),
                ));
            }
            if !app.workspace.chrome_ui.show_radial_menu {
                app.workspace.chrome_ui.surface_state = crate::app::ActionSurfaceState::Radial {
                    scope: crate::app::ActionScope::Graph {
                        view_id,
                        target: crate::app::ScopeTarget::Frame(frame_name.clone()),
                    },
                    anchor: crate::app::Anchor::TargetFrame(frame_name.clone()),
                };
                let _ = toolbar_routing::request_radial_menu_toggle(
                    app,
                    CommandBarFocusTarget::default(),
                );
            }
            if let Some(pointer) = pointer {
                ctx.data_mut(|d| {
                    d.insert_persisted(egui::Id::new("radial_menu_center"), pointer);
                });
            }
        }
        crate::app::ContextCommandSurfacePreference::ContextPalette => {
            app.set_pending_node_context_target(None);
            app.set_pending_frame_context_target(Some(frame_name.clone()));
            if app.pending_command_surface_return_target().is_none() {
                app.set_pending_command_surface_return_target(Some(
                    crate::app::ToolSurfaceReturnTarget::Graph(view_id),
                ));
            }
            app.set_context_palette_anchor(pointer.map(|pos| [pos.x, pos.y]));
            app.open_palette_contextual(
                crate::app::ActionScope::Graph {
                    view_id,
                    target: crate::app::ScopeTarget::Frame(frame_name.clone()),
                },
                crate::app::Anchor::TargetFrame(frame_name),
            );
        }
    }
}

/// Render graph info and controls hint overlay text into the current UI.
pub fn render_graph_info_in_ui(
    ui: &mut Ui,
    app: &mut GraphBrowserApp,
    view_id: crate::app::GraphViewId,
) {
    draw_graph_info(ui, app, view_id);
}

pub(crate) fn canvas_lasso_binding_label(binding: CanvasLassoBinding) -> &'static str {
    match binding {
        CanvasLassoBinding::RightDrag => "Right-Drag Lasso",
        CanvasLassoBinding::ShiftLeftDrag => "Shift+Left-Drag Lasso (Default)",
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SpatialNavigationDirection {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Clone, Copy, Debug)]
struct GraphNodeScreenProxy {
    key: NodeKey,
    center: Point2D<f32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct GraphSurfaceTooltipContent {
    title: String,
    details: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GraphSurfaceSelectAllMode {
    All,
    Visible,
}

impl GraphNodeScreenProxy {
    fn from_hit_proxy(proxy: &HitProxy<NodeKey>) -> Option<Self> {
        match proxy {
            HitProxy::Node { id, center, .. } => Some(Self {
                key: *id,
                center: *center,
            }),
            _ => None,
        }
    }
}

fn graph_surface_focus_id(view_id: crate::app::GraphViewId) -> egui::Id {
    egui::Id::new(("graph_surface_canvas", view_id))
}

fn spatial_navigation_direction_from_input(
    ctx: &egui::Context,
) -> Option<SpatialNavigationDirection> {
    ctx.input(|input| {
        if input.key_pressed(egui::Key::ArrowLeft) {
            Some(SpatialNavigationDirection::Left)
        } else if input.key_pressed(egui::Key::ArrowRight) {
            Some(SpatialNavigationDirection::Right)
        } else if input.key_pressed(egui::Key::ArrowUp) {
            Some(SpatialNavigationDirection::Up)
        } else if input.key_pressed(egui::Key::ArrowDown) {
            Some(SpatialNavigationDirection::Down)
        } else {
            None
        }
    })
}

fn graph_surface_select_all_mode_from_input(
    ctx: &egui::Context,
) -> Option<GraphSurfaceSelectAllMode> {
    ctx.input(|input| {
        if crate::input::graph_view_action_binding_pressed(
            input,
            crate::shell::desktop::runtime::registries::input::action_id::graph::SELECT_VISIBLE,
        ) {
            Some(GraphSurfaceSelectAllMode::Visible)
        } else if crate::input::graph_view_action_binding_pressed(
            input,
            crate::shell::desktop::runtime::registries::input::action_id::graph::SELECT_ALL,
        ) {
            Some(GraphSurfaceSelectAllMode::All)
        } else {
            None
        }
    })
}

fn graph_surface_hover_tooltip_target(
    app: &GraphBrowserApp,
    graph_hovered: bool,
) -> Option<GraphTooltipTarget> {
    if !graph_hovered {
        return None;
    }

    app.workspace
        .graph_runtime
        .hovered_graph_node
        .map(GraphTooltipTarget::Node)
        .or_else(|| {
            app.workspace
                .graph_runtime
                .hovered_graph_edge
                .map(|(from, to)| GraphTooltipTarget::Edge { from, to })
        })
}

fn graph_surface_focus_tooltip_target(
    app: &GraphBrowserApp,
    view_id: crate::app::GraphViewId,
    graph_surface_focused: bool,
) -> Option<GraphTooltipTarget> {
    if !graph_surface_focused || app.workspace.graph_runtime.focused_view != Some(view_id) {
        return None;
    }

    app.focused_selection()
        .primary()
        .map(GraphTooltipTarget::Node)
}

fn graph_surface_current_tooltip_target(
    app: &GraphBrowserApp,
    view_id: crate::app::GraphViewId,
    graph_surface_focused: bool,
    graph_hovered: bool,
) -> Option<GraphTooltipTarget> {
    graph_surface_hover_tooltip_target(app, graph_hovered)
        .or_else(|| graph_surface_focus_tooltip_target(app, view_id, graph_surface_focused))
}

fn next_graph_surface_tooltip_dismissed_target(
    dismissed: Option<GraphTooltipTarget>,
    current: Option<GraphTooltipTarget>,
    dismiss_requested: bool,
) -> Option<GraphTooltipTarget> {
    let mut next = dismissed.filter(|target| Some(*target) == current);
    if dismiss_requested && current.is_some() {
        next = current;
    }
    next
}

fn graph_surface_node_title(app: &GraphBrowserApp, node_key: NodeKey) -> Option<String> {
    let node = app.domain_graph().get_node(node_key)?;
    Some(app.user_visible_node_title(node_key).unwrap_or_else(|| {
        if node.title.is_empty() {
            node.url().to_string()
        } else {
            node.title.clone()
        }
    }))
}

fn graph_surface_node_lifecycle_label(lifecycle: crate::graph::NodeLifecycle) -> &'static str {
    match lifecycle {
        crate::graph::NodeLifecycle::Active => "Active",
        crate::graph::NodeLifecycle::Warm => "Warm",
        crate::graph::NodeLifecycle::Cold => "Cold",
        crate::graph::NodeLifecycle::Tombstone => "Ghost Node",
    }
}

fn graph_surface_node_semantic_tags(app: &GraphBrowserApp, node_key: NodeKey) -> Vec<String> {
    crate::shell::desktop::runtime::registries::knowledge::tags_for_node(app, &node_key)
        .into_iter()
        .map(|tag| crate::render::semantic_tags::semantic_tag_display_label(&tag))
        .collect()
}

fn graph_surface_node_state_summary(app: &GraphBrowserApp, node_key: NodeKey) -> Option<String> {
    let mut states = Vec::new();
    if app.focused_selection().primary() == Some(node_key) {
        states.push("focused".to_string());
    } else if app.focused_selection().contains(&node_key) {
        states.push("selected".to_string());
    }
    if app.runtime_block_state_for_node(node_key).is_some() {
        states.push("blocked".to_string());
    }
    if app.runtime_crash_state_for_node(node_key).is_some() {
        states.push("degraded".to_string());
    }

    (!states.is_empty()).then(|| states.join(", "))
}

fn graph_surface_edge_kind_labels(payload: &crate::graph::EdgePayload) -> Vec<&'static str> {
    let mut labels = Vec::new();
    if payload.has_edge_kind(EdgeType::Hyperlink) {
        labels.push("Hyperlink");
    }
    if payload.has_edge_kind(EdgeType::History) {
        labels.push("History");
    }
    if payload.has_edge_kind(EdgeType::UserGrouped) {
        labels.push("User grouped");
    }
    labels
}

fn graph_surface_tooltip_content(
    app: &GraphBrowserApp,
    target: GraphTooltipTarget,
) -> Option<GraphSurfaceTooltipContent> {
    match target {
        GraphTooltipTarget::Node(node_key) => {
            let node = app.domain_graph().get_node(node_key)?;
            let mut details = vec![format!(
                "Lifecycle: {}",
                graph_surface_node_lifecycle_label(node.lifecycle)
            )];
            if let Some(state) = graph_surface_node_state_summary(app, node_key) {
                details.push(format!("State: {state}"));
            }
            let semantic_tags = graph_surface_node_semantic_tags(app, node_key);
            if !semantic_tags.is_empty() {
                details.push(format!("Semantic tags: {}", semantic_tags.join(", ")));
            }
            if !node.title.is_empty() && node.title != node.url().to_string() {
                details.push(format!("URL: {}", node.url()));
            }

            Some(GraphSurfaceTooltipContent {
                title: graph_surface_node_title(app, node_key)?,
                details,
            })
        }
        GraphTooltipTarget::Edge { from, to } => {
            let from_title = graph_surface_node_title(app, from)?;
            let to_title = graph_surface_node_title(app, to)?;
            let edge_key = app.domain_graph().find_edge_key(from, to)?;
            let payload = app.domain_graph().get_edge(edge_key)?;
            let mut details = Vec::new();
            let kinds = graph_surface_edge_kind_labels(payload);
            if !kinds.is_empty() {
                details.push(format!("Kinds: {}", kinds.join(", ")));
            }
            if let Some(label) = payload.label().filter(|label| !label.trim().is_empty()) {
                details.push(format!("Label: {label}"));
            }

            Some(GraphSurfaceTooltipContent {
                title: format!("{from_title} -> {to_title}"),
                details,
            })
        }
    }
}

fn graph_surface_tooltip_anchor(
    target: GraphTooltipTarget,
    hit_proxies: &[HitProxy<NodeKey>],
) -> Option<egui::Pos2> {
    hit_proxies.iter().find_map(|proxy| match (target, proxy) {
        (GraphTooltipTarget::Node(node_key), HitProxy::Node { id, center, .. })
            if *id == node_key =>
        {
            Some(egui::pos2(center.x, center.y))
        }
        (
            GraphTooltipTarget::Edge { from, to },
            HitProxy::Edge {
                source,
                target,
                midpoint,
                ..
            },
        ) if *source == from && *target == to => Some(egui::pos2(midpoint.x, midpoint.y)),
        _ => None,
    })
}

fn render_graph_surface_tooltip(
    ui: &Ui,
    view_id: crate::app::GraphViewId,
    graph_rect: egui::Rect,
    anchor: Option<egui::Pos2>,
    content: &GraphSurfaceTooltipContent,
) {
    let mut position = anchor
        .map(|anchor| anchor + egui::vec2(16.0, 16.0))
        .unwrap_or_else(|| graph_rect.left_top() + egui::vec2(16.0, 16.0));
    let max_x = (graph_rect.right() - 292.0).max(graph_rect.left() + 12.0);
    let max_y = (graph_rect.bottom() - 140.0).max(graph_rect.top() + 12.0);
    position.x = position.x.clamp(graph_rect.left() + 12.0, max_x);
    position.y = position.y.clamp(graph_rect.top() + 12.0, max_y);

    egui::Area::new(egui::Id::new(("graph_surface_tooltip", view_id)))
        .order(egui::Order::Tooltip)
        .interactable(false)
        .fixed_pos(position)
        .show(ui.ctx(), |ui| {
            Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_max_width(280.0);
                ui.label(egui::RichText::new(&content.title).strong());
                for detail in &content.details {
                    ui.small(detail);
                }
            });
        });
}

fn graph_surface_accessibility_tooltip_target(
    app: &GraphBrowserApp,
    view_id: crate::app::GraphViewId,
) -> Option<GraphTooltipTarget> {
    if app.workspace.graph_runtime.focused_view != Some(view_id) {
        return None;
    }

    app.workspace
        .graph_runtime
        .hovered_graph_node
        .map(GraphTooltipTarget::Node)
        .or_else(|| {
            app.workspace
                .graph_runtime
                .hovered_graph_edge
                .map(|(from, to)| GraphTooltipTarget::Edge { from, to })
        })
        .or_else(|| {
            app.focused_selection()
                .primary()
                .map(GraphTooltipTarget::Node)
        })
}

pub(crate) fn graph_surface_tooltip_accessibility_description(
    app: &GraphBrowserApp,
    view_id: crate::app::GraphViewId,
) -> Option<String> {
    let target = graph_surface_accessibility_tooltip_target(app, view_id)?;
    if app.workspace.graph_runtime.dismissed_graph_tooltip == Some(target) {
        return None;
    }

    let content = graph_surface_tooltip_content(app, target)?;
    let mut description = format!("Current tooltip: {}", content.title);
    if !content.details.is_empty() {
        description.push_str(". ");
        description.push_str(&content.details.join(". "));
    }
    Some(description)
}

pub(crate) fn graph_surface_accessibility_description(
    app: &GraphBrowserApp,
    view_id: crate::app::GraphViewId,
) -> String {
    let mut parts = vec![
        "Graph canvas. Tab moves between the graph surface and the selected content. Arrow keys move between visible nodes. Shift plus Arrow extends the current selection."
            .to_string(),
    ];
    if let Some(tooltip_description) = graph_surface_tooltip_accessibility_description(app, view_id)
    {
        parts.push(tooltip_description);
    }
    parts.join(" ")
}

fn fallback_spatial_navigation_origin(
    direction: SpatialNavigationDirection,
    nodes: &[GraphNodeScreenProxy],
) -> Option<Point2D<f32>> {
    if nodes.is_empty() {
        return None;
    }

    let (sum_x, sum_y) = nodes.iter().fold((0.0, 0.0), |(acc_x, acc_y), node| {
        (acc_x + node.center.x, acc_y + node.center.y)
    });
    let centroid = Point2D::new(sum_x / nodes.len() as f32, sum_y / nodes.len() as f32);

    // When the user has no selection yet, seed the search origin on the
    // *opposite* edge to the direction of travel — otherwise the forward
    // filter inside `graph_surface_navigation_target` drops every
    // candidate and navigation is a no-op. For a rightward press we seed
    // at the leftmost node so the next target is the nearest visible
    // neighbour walking rightward.
    let candidate = match direction {
        SpatialNavigationDirection::Left => nodes.iter().max_by(|left, right| {
            left.center.x.total_cmp(&right.center.x).then_with(|| {
                (right.center.y - centroid.y)
                    .abs()
                    .total_cmp(&(left.center.y - centroid.y).abs())
            })
        }),
        SpatialNavigationDirection::Right => nodes.iter().min_by(|left, right| {
            left.center.x.total_cmp(&right.center.x).then_with(|| {
                (left.center.y - centroid.y)
                    .abs()
                    .total_cmp(&(right.center.y - centroid.y).abs())
            })
        }),
        SpatialNavigationDirection::Up => nodes.iter().max_by(|left, right| {
            left.center.y.total_cmp(&right.center.y).then_with(|| {
                (right.center.x - centroid.x)
                    .abs()
                    .total_cmp(&(left.center.x - centroid.x).abs())
            })
        }),
        SpatialNavigationDirection::Down => nodes.iter().min_by(|left, right| {
            left.center.y.total_cmp(&right.center.y).then_with(|| {
                (left.center.x - centroid.x)
                    .abs()
                    .total_cmp(&(right.center.x - centroid.x).abs())
            })
        }),
    };

    candidate.copied().map(|node| node.center)
}

fn directional_components(
    direction: SpatialNavigationDirection,
    from: Point2D<f32>,
    to: Point2D<f32>,
) -> (f32, f32) {
    let delta_x = to.x - from.x;
    let delta_y = to.y - from.y;
    match direction {
        SpatialNavigationDirection::Left => (-delta_x, delta_y.abs()),
        SpatialNavigationDirection::Right => (delta_x, delta_y.abs()),
        SpatialNavigationDirection::Up => (-delta_y, delta_x.abs()),
        SpatialNavigationDirection::Down => (delta_y, delta_x.abs()),
    }
}

fn graph_surface_navigation_target(
    hit_proxies: &[HitProxy<NodeKey>],
    current: Option<NodeKey>,
    direction: SpatialNavigationDirection,
) -> Option<NodeKey> {
    let nodes: Vec<_> = hit_proxies
        .iter()
        .filter_map(GraphNodeScreenProxy::from_hit_proxy)
        .collect();
    if nodes.is_empty() {
        return None;
    }

    let origin = current
        .and_then(|key| {
            nodes
                .iter()
                .find(|node| node.key == key)
                .map(|node| node.center)
        })
        .or_else(|| fallback_spatial_navigation_origin(direction, &nodes))?;

    nodes
        .iter()
        .filter(|candidate| Some(candidate.key) != current)
        .filter_map(|candidate| {
            let (forward, off_axis) = directional_components(direction, origin, candidate.center);
            if forward <= 0.0 {
                return None;
            }

            let distance_sq =
                (candidate.center.x - origin.x).powi(2) + (candidate.center.y - origin.y).powi(2);
            let angular_penalty = off_axis / forward.max(1.0);
            Some((candidate.key, angular_penalty, distance_sq))
        })
        .min_by(|left, right| {
            left.1
                .total_cmp(&right.1)
                .then_with(|| left.2.total_cmp(&right.2))
        })
        .map(|(key, _, _)| key)
}

fn graph_surface_all_selectable_node_keys(
    app: &GraphBrowserApp,
    view_id: crate::app::GraphViewId,
    search_matches: &HashSet<NodeKey>,
    search_display_mode: SearchDisplayMode,
    search_query_active: bool,
) -> Vec<NodeKey> {
    let mut keys: Vec<NodeKey> = visible_nodes_for_view_filters(
        app,
        view_id,
        search_matches,
        search_display_mode,
        search_query_active,
    )
    .map(|visible| visible.into_iter().collect())
    .unwrap_or_else(|| app.render_graph().nodes().map(|(key, _)| key).collect());
    keys.sort_by_key(|key| key.index());
    keys
}

fn graph_surface_visible_node_keys(hit_proxies: &[HitProxy<NodeKey>]) -> Vec<NodeKey> {
    let mut keys: Vec<NodeKey> = hit_proxies
        .iter()
        .filter_map(GraphNodeScreenProxy::from_hit_proxy)
        .map(|node| node.key)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    keys.sort_by_key(|key| key.index());
    keys
}

fn apply_graph_surface_select_all(
    app: &mut GraphBrowserApp,
    view_id: crate::app::GraphViewId,
    keys: Vec<NodeKey>,
) {
    if keys.is_empty() {
        return;
    }

    set_focused_view_with_transition(app, Some(view_id));
    app.update_focused_selection(keys, SelectionUpdateMode::Replace);
}

fn apply_graph_surface_select_all_shortcut(
    ctx: &egui::Context,
    app: &mut GraphBrowserApp,
    view_id: crate::app::GraphViewId,
    search_matches: &HashSet<NodeKey>,
    search_display_mode: SearchDisplayMode,
    search_query_active: bool,
    hit_proxies: &[HitProxy<NodeKey>],
) -> bool {
    let Some(mode) = graph_surface_select_all_mode_from_input(ctx) else {
        return false;
    };

    let keys = match mode {
        GraphSurfaceSelectAllMode::All => graph_surface_all_selectable_node_keys(
            app,
            view_id,
            search_matches,
            search_display_mode,
            search_query_active,
        ),
        GraphSurfaceSelectAllMode::Visible => graph_surface_visible_node_keys(hit_proxies),
    };
    apply_graph_surface_select_all(app, view_id, keys);
    true
}

fn apply_graph_surface_arrow_navigation(
    ctx: &egui::Context,
    app: &mut GraphBrowserApp,
    view_id: crate::app::GraphViewId,
    hit_proxies: &[HitProxy<NodeKey>],
) {
    let Some(direction) = spatial_navigation_direction_from_input(ctx) else {
        return;
    };

    let current = app.focused_selection().primary();
    if let Some(next) = graph_surface_navigation_target(hit_proxies, current, direction) {
        let extend_selection = ctx.input(|input| input.modifiers.shift);
        apply_graph_surface_navigation_selection(app, view_id, next, extend_selection);
    }
}

fn apply_graph_surface_navigation_selection(
    app: &mut GraphBrowserApp,
    view_id: crate::app::GraphViewId,
    next: NodeKey,
    extend_selection: bool,
) {
    set_focused_view_with_transition(app, Some(view_id));
    if extend_selection {
        if app.focused_selection().contains(&next) {
            app.promote_focused_selection_primary(next);
        } else {
            app.update_focused_selection(vec![next], SelectionUpdateMode::Add);
        }
    } else {
        app.update_focused_selection(vec![next], SelectionUpdateMode::Replace);
    }
}

/// Render graph content via the portable `graph-canvas` pipeline.
///
/// Live graph-pane entry point since M2 retired `egui_graphs`. Uses
/// `graph-canvas` for scene derivation, hit testing, and interaction;
/// the egui host only handles painting (via `canvas_egui_painter`) and
/// input translation (via `canvas_bridge`).
#[cfg(not(target_arch = "wasm32"))]
pub fn render_graph_canvas_in_ui(
    ui: &mut Ui,
    app: &mut GraphBrowserApp,
    view_id: crate::app::GraphViewId,
    search_matches: &HashSet<NodeKey>,
    _active_search_match: Option<NodeKey>,
    search_display_mode: SearchDisplayMode,
    search_query_active: bool,
    graph_surface_focused: bool,
) -> Vec<GraphAction> {
    let graph_rect = ui.max_rect();
    let response = ui.interact(
        graph_rect,
        graph_surface_focus_id(view_id),
        egui::Sense::click_and_drag(),
    );
    let scale_factor = ui.ctx().pixels_per_point();
    let viewport = canvas_bridge::viewport_from_egui_rect(graph_rect, scale_factor);
    let events = canvas_bridge::collect_canvas_events(ui);
    let output = canvas_bridge::run_graph_canvas_frame(
        app,
        view_id,
        search_matches,
        search_display_mode,
        search_query_active,
        viewport,
        &events,
    );
    if graph_surface_focused && app.workspace.graph_runtime.focused_view == Some(view_id) {
        if !response.has_focus() {
            response.request_focus();
        }
        let handled_select_all = apply_graph_surface_select_all_shortcut(
            ui.ctx(),
            app,
            view_id,
            search_matches,
            search_display_mode,
            search_query_active,
            &output.scene.hit_proxies,
        );
        if !handled_select_all {
            apply_graph_surface_arrow_navigation(ui.ctx(), app, view_id, &output.scene.hit_proxies);
        }
    }

    let current_tooltip_target = graph_surface_current_tooltip_target(
        app,
        view_id,
        graph_surface_focused,
        response.hovered(),
    );
    let dismiss_requested = ui.ctx().input(|input| input.key_pressed(egui::Key::Escape));
    let dismissed_tooltip_target = next_graph_surface_tooltip_dismissed_target(
        app.workspace.graph_runtime.dismissed_graph_tooltip,
        current_tooltip_target,
        dismiss_requested,
    );
    app.workspace.graph_runtime.dismissed_graph_tooltip = dismissed_tooltip_target;

    canvas_egui_painter::paint_projected_scene(ui, &output.scene);
    if let Some(target) =
        current_tooltip_target.filter(|target| Some(*target) != dismissed_tooltip_target)
        && let Some(content) = graph_surface_tooltip_content(app, target)
    {
        render_graph_surface_tooltip(
            ui,
            view_id,
            graph_rect,
            graph_surface_tooltip_anchor(target, &output.scene.hit_proxies),
            &content,
        );
    }
    output.graph_actions
}

/// Convert resolved graph actions to graph intents without applying them.
pub fn intents_from_graph_actions(actions: Vec<GraphAction>) -> Vec<GraphIntent> {
    let mut intents = Vec::with_capacity(actions.len());
    for action in actions {
        match action {
            GraphAction::FocusNode(key) => {
                intents.push(GraphIntent::OpenNodeFrameRouted {
                    key,
                    prefer_frame: None,
                });
            }
            GraphAction::FocusFrame {
                frame_name,
                member_key,
            } => {
                intents.push(
                    ViewAction::SetSelectedFrame {
                        frame_name: Some(frame_name.clone()),
                    }
                    .into(),
                );
                intents.push(GraphIntent::OpenFrameTileGroup {
                    url: crate::util::VersoAddress::frame(&frame_name).to_string(),
                    focus_node: Some(member_key),
                });
            }
            GraphAction::FocusNodeSplit(key) => {
                intents.push(GraphIntent::SelectNode {
                    key,
                    multi_select: false,
                });
            }
            GraphAction::DragStart => {
                intents.push(GraphIntent::SetInteracting { interacting: true });
            }
            GraphAction::DragEnd(key, pos) => {
                intents.push(GraphIntent::SetInteracting { interacting: false });
                intents.push(ViewAction::SetNodePosition { key, position: pos }.into());
            }
            GraphAction::MoveNode(key, pos) => {
                intents.push(ViewAction::SetNodePosition { key, position: pos }.into());
            }
            GraphAction::SelectNode { key, multi_select } => {
                intents.push(GraphIntent::SelectNode { key, multi_select });
            }
            GraphAction::SelectFrame(frame_name) => {
                intents.push(
                    ViewAction::UpdateSelection {
                        keys: Vec::new(),
                        mode: SelectionUpdateMode::Replace,
                    }
                    .into(),
                );
                intents.push(
                    ViewAction::SetSelectedFrame {
                        frame_name: Some(frame_name),
                    }
                    .into(),
                );
            }
            GraphAction::LassoSelect { keys, mode } => {
                intents.push(ViewAction::UpdateSelection { keys, mode }.into());
            }
            GraphAction::SetHighlightedEdge { from, to } => {
                intents.push(ViewAction::SetHighlightedEdge { from, to }.into());
            }
            GraphAction::ClearHighlightedEdge => {
                intents.push(ViewAction::ClearHighlightedEdge.into());
            }
            GraphAction::ClearSelection => {
                intents.push(
                    ViewAction::UpdateSelection {
                        keys: Vec::new(),
                        mode: SelectionUpdateMode::Replace,
                    }
                    .into(),
                );
            }
            GraphAction::Zoom(new_zoom) => {
                intents.push(ViewAction::SetZoom { zoom: new_zoom }.into());
            }
        }
    }
    intents
}

fn record_simulate_release_impulses(
    app: &mut GraphBrowserApp,
    impulses: HashMap<NodeKey, egui::Vec2>,
) {
    let Some(view_id) = app.workspace.graph_runtime.focused_view else {
        return;
    };

    if app.graph_view_scene_mode(view_id) != crate::app::SceneMode::Simulate {
        app.workspace
            .graph_runtime
            .simulate_release_impulses
            .remove(&view_id);
        return;
    }

    if impulses.is_empty() {
        return;
    }

    app.workspace
        .graph_runtime
        .simulate_release_impulses
        .insert(
            view_id,
            impulses
                .into_iter()
                .map(|(key, impulse)| {
                    (
                        key,
                        graphshell_core::geometry::PortableVector::new(impulse.x, impulse.y),
                    )
                })
                .collect(),
        );
}

pub fn render_choose_frame_picker(ctx: &egui::Context, app: &mut GraphBrowserApp) -> bool {
    let open_settings_tool_pane = false;
    let Some(request) = app.choose_frame_picker_request() else {
        return false;
    };
    let target = request.node;
    if app.workspace.domain.graph.get_node(target).is_none() {
        app.clear_choose_frame_picker();
        return false;
    }
    let mut selected_frame: Option<String> = None;
    let mut close = false;
    let mut memberships = match request.mode {
        ChooseFramePickerMode::OpenNodeInFrame => app.sorted_frames_for_node_key(target),
        ChooseFramePickerMode::AddNodeToFrame => {
            let mut all = app
                .list_workspace_layout_names()
                .into_iter()
                .filter(|name| !GraphBrowserApp::is_reserved_workspace_layout_name(name))
                .collect::<Vec<_>>();
            all.sort();
            all
        }
        ChooseFramePickerMode::AddConnectedSelectionToFrame => {
            let mut all = app
                .list_workspace_layout_names()
                .into_iter()
                .filter(|name| !GraphBrowserApp::is_reserved_workspace_layout_name(name))
                .collect::<Vec<_>>();
            all.sort();
            all
        }
        ChooseFramePickerMode::AddExactSelectionToFrame => {
            let mut all = app
                .list_workspace_layout_names()
                .into_iter()
                .filter(|name| !GraphBrowserApp::is_reserved_workspace_layout_name(name))
                .collect::<Vec<_>>();
            all.sort();
            all
        }
    };
    let title = app
        .workspace
        .domain
        .graph
        .get_node(target)
        .map(|node| format!("Choose Frame: {}", node.title))
        .unwrap_or_else(|| "Choose Frame".to_string());
    Window::new(title)
        .collapsible(false)
        .resizable(true)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .default_width(300.0)
        .default_height(360.0)
        .show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let theme_tokens = phase3_resolve_active_theme(app.default_registry_theme_id()).tokens;
                    if memberships.is_empty() {
                        let msg = match request.mode {
                            ChooseFramePickerMode::OpenNodeInFrame => {
                                "No frame memberships for this node."
                            }
                            ChooseFramePickerMode::AddNodeToFrame => {
                                "No named frames available. Save one first."
                            }
                            ChooseFramePickerMode::AddConnectedSelectionToFrame => {
                                "No named frames available. Save one first."
                            }
                            ChooseFramePickerMode::AddExactSelectionToFrame => {
                                "No named frames available. Save one first."
                            }
                        };
                        ui.label(
                            egui::RichText::new(msg)
                                .small()
                                .color(theme_tokens.command_notice),
                        );
                    } else {
                        memberships.dedup();
                        let header = match request.mode {
                            ChooseFramePickerMode::OpenNodeInFrame => "Open in frame:",
                            ChooseFramePickerMode::AddNodeToFrame => "Add node to frame:",
                            ChooseFramePickerMode::AddConnectedSelectionToFrame => {
                                "Add connected nodes to frame:"
                            }
                            ChooseFramePickerMode::AddExactSelectionToFrame => {
                                "Add selected nodes to frame:"
                            }
                        };
                        ui.label(
                            egui::RichText::new(header)
                                .small()
                                .color(theme_tokens.radial_chrome_text),
                        );
                        for name in &memberships {
                            if ui.button(name).clicked() {
                                selected_frame = Some(name.clone());
                                close = true;
                            }
                        }
                    }
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("Open Persistence Overlay").clicked() {
                            crate::shell::desktop::runtime::registries::phase3_publish_settings_route_requested(
                                &VersoAddress::settings(GraphshellSettingsPath::Persistence)
                                    .to_string(),
                            );
                        }
                        if ui.button("Close").clicked() {
                            close = true;
                        }
                    });
                });
        });
    if let Some(name) = selected_frame {
        match request.mode {
            ChooseFramePickerMode::OpenNodeInFrame => {
                apply_reducer_graph_intents_hardened(
                    app,
                    [GraphIntent::OpenNodeFrameRouted {
                        key: target,
                        prefer_frame: Some(name),
                    }],
                );
            }
            ChooseFramePickerMode::AddNodeToFrame => {
                app.request_add_node_to_frame(target, name);
            }
            ChooseFramePickerMode::AddConnectedSelectionToFrame => {
                let focused_selection = app.focused_selection();
                let mut seed_nodes: Vec<NodeKey> = if focused_selection.is_empty() {
                    vec![target]
                } else {
                    focused_selection.iter().copied().collect()
                };
                if !seed_nodes.contains(&target) {
                    seed_nodes.push(target);
                }
                app.request_add_connected_to_frame(seed_nodes, name);
            }
            ChooseFramePickerMode::AddExactSelectionToFrame => {
                let mut nodes = app
                    .choose_frame_picker_exact_nodes()
                    .map(|keys| keys.to_vec())
                    .unwrap_or_else(|| vec![target]);
                nodes.retain(|key| app.workspace.domain.graph.get_node(*key).is_some());
                nodes.sort_by_key(|key| key.index());
                nodes.dedup();
                if !nodes.is_empty() {
                    app.request_add_exact_nodes_to_frame(nodes, name);
                }
            }
        }
    }
    if close {
        app.clear_choose_frame_picker();
    }
    open_settings_tool_pane
}

pub fn render_unsaved_frame_prompt(ctx: &egui::Context, app: &mut GraphBrowserApp) -> bool {
    let open_settings_tool_pane = false;
    let Some(request) = app.unsaved_frame_prompt_request().cloned() else {
        return false;
    };
    let mut action: Option<UnsavedFramePromptAction> = None;
    Window::new("Unsaved Frame Changes")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .default_width(380.0)
        .show(ctx, |ui| {
            match &request {
                UnsavedFramePromptRequest::FrameSwitch { name, .. } => {
                    ui.label(format!(
                        "This frame has unsaved graph changes.\nSwitch to '{name}' without saving?"
                    ));
                }
            }
            ui.separator();
            if ui.button("Open Persistence Overlay").clicked() {
                crate::shell::desktop::runtime::registries::phase3_publish_settings_route_requested(
                    &VersoAddress::settings(GraphshellSettingsPath::Persistence).to_string(),
                );
            }
            ui.horizontal(|ui| {
                if ui.button("Proceed Without Saving").clicked() {
                    action = Some(UnsavedFramePromptAction::ProceedWithoutSaving);
                }
                if ui.button("Cancel").clicked() {
                    action = Some(UnsavedFramePromptAction::Cancel);
                }
            });
        });
    if let Some(action) = action {
        app.set_unsaved_frame_prompt_action(action);
    }
    open_settings_tool_pane
}

pub fn render_choose_workspace_picker(ctx: &egui::Context, app: &mut GraphBrowserApp) -> bool {
    render_choose_frame_picker(ctx, app)
}

pub fn render_unsaved_workspace_prompt(ctx: &egui::Context, app: &mut GraphBrowserApp) -> bool {
    render_unsaved_frame_prompt(ctx, app)
}

/// Resolve pair edge command context using precedence:
/// selected pair > (selected primary + explicit context target) > (selected primary + hovered node)
/// > (selected primary + focused pane node).
fn resolve_pair_command_context(
    app: &GraphBrowserApp,
    hovered_node: Option<NodeKey>,
    focused_pane_node: Option<NodeKey>,
) -> Option<(NodeKey, NodeKey)> {
    let selection = app.focused_selection();

    if let Some((from, to)) = selection.ordered_pair() {
        return Some((from, to));
    }

    if selection.len() == 1 {
        let from = selection.primary()?;
        if let Some(to) = app.pending_node_context_target().filter(|to| *to != from) {
            return Some((from, to));
        }
        if let Some(to) = hovered_node.filter(|to| *to != from) {
            return Some((from, to));
        }
        if let Some(to) = focused_pane_node.filter(|to| *to != from) {
            return Some((from, to));
        }
    }

    None
}

fn resolve_source_node_context(
    app: &GraphBrowserApp,
    hovered_node: Option<NodeKey>,
    focused_pane_node: Option<NodeKey>,
) -> Option<NodeKey> {
    app.pending_node_context_target()
        .or(app.focused_selection().primary())
        .or(hovered_node)
        .or(focused_pane_node)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{GraphViewId, SearchDisplayMode, WorkbenchIntent};
    use crate::registries::atomic::lens::LayoutMode;
    use crate::shell::desktop::runtime::diagnostics::DiagnosticsState;
    use std::hint::black_box;
    use std::time::Instant;

    fn test_app() -> GraphBrowserApp {
        GraphBrowserApp::new_for_testing()
    }

    #[test]
    fn test_focus_node_action() {
        let mut app = test_app();
        let key = app.add_node_and_sync("https://example.com".into(), Point2D::new(0.0, 0.0));

        let intents = intents_from_graph_actions(vec![GraphAction::FocusNode(key)]);
        app.apply_reducer_intents(intents);

        assert!(app.focused_selection().contains(&key));
    }

    #[test]
    fn test_drag_start_sets_interacting() {
        let mut app = test_app();
        assert!(!app.workspace.graph_runtime.is_interacting);

        let intents = intents_from_graph_actions(vec![GraphAction::DragStart]);
        app.apply_reducer_intents(intents);

        assert!(app.workspace.graph_runtime.is_interacting);
    }

    #[test]
    fn focused_view_change_emits_ux_navigation_transition_channel() {
        let mut app = test_app();
        let view_id = crate::app::GraphViewId::new();
        let mut diagnostics = DiagnosticsState::new();

        set_focused_view_with_transition(&mut app, Some(view_id));

        diagnostics.force_drain_for_tests();
        let snapshot = diagnostics.snapshot_json_for_tests().to_string();
        assert!(
            snapshot.contains("ux:navigation_transition"),
            "expected ux:navigation_transition when focused view changes"
        );
    }

    #[test]
    fn test_drag_end_clears_interacting_and_updates_position() {
        let mut app = test_app();
        let key = app.add_node_and_sync("https://example.com".into(), Point2D::new(0.0, 0.0));
        app.set_interacting(true);

        let intents =
            intents_from_graph_actions(vec![GraphAction::DragEnd(key, Point2D::new(150.0, 250.0))]);
        app.apply_reducer_intents(intents);

        assert!(!app.workspace.graph_runtime.is_interacting);
        let node = app.workspace.domain.graph.get_node(key).unwrap();
        assert_eq!(node.projected_position(), Point2D::new(150.0, 250.0));
    }

    #[test]
    fn test_move_node_updates_position() {
        let mut app = test_app();
        let key = app.add_node_and_sync("https://example.com".into(), Point2D::new(0.0, 0.0));

        let intents =
            intents_from_graph_actions(vec![GraphAction::MoveNode(key, Point2D::new(42.0, 84.0))]);
        app.apply_reducer_intents(intents);

        let node = app.workspace.domain.graph.get_node(key).unwrap();
        assert_eq!(node.projected_position(), Point2D::new(42.0, 84.0));
    }

    #[test]
    fn test_select_node_action() {
        let mut app = test_app();
        let key = app.add_node_and_sync("https://example.com".into(), Point2D::new(0.0, 0.0));

        let intents = intents_from_graph_actions(vec![GraphAction::SelectNode {
            key,
            multi_select: false,
        }]);
        app.apply_reducer_intents(intents);

        assert!(app.focused_selection().contains(&key));
    }

    #[test]
    fn test_select_node_action_ctrl_click_adds_to_selection() {
        let mut app = test_app();
        let a = app.add_node_and_sync("a".into(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("b".into(), Point2D::new(100.0, 0.0));

        // Single-click selects a.
        let intents = intents_from_graph_actions(vec![GraphAction::SelectNode {
            key: a,
            multi_select: false,
        }]);
        app.apply_reducer_intents(intents);

        // Ctrl+Click adds b without deselecting a.
        let intents = intents_from_graph_actions(vec![GraphAction::SelectNode {
            key: b,
            multi_select: true,
        }]);
        app.apply_reducer_intents(intents);

        assert!(app.focused_selection().contains(&a));
        assert!(app.focused_selection().contains(&b));
        assert_eq!(app.focused_selection().len(), 2);
    }

    #[test]
    fn test_select_node_action_ctrl_click_toggles_off_selected() {
        let mut app = test_app();
        let a = app.add_node_and_sync("a".into(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("b".into(), Point2D::new(100.0, 0.0));

        // Select both nodes.
        app.apply_reducer_intents(intents_from_graph_actions(vec![
            GraphAction::SelectNode {
                key: a,
                multi_select: false,
            },
            GraphAction::SelectNode {
                key: b,
                multi_select: true,
            },
        ]));
        assert_eq!(app.focused_selection().len(), 2);

        // Ctrl+Click a again → toggles a out of the selection.
        app.apply_reducer_intents(intents_from_graph_actions(vec![GraphAction::SelectNode {
            key: a,
            multi_select: true,
        }]));

        assert!(!app.focused_selection().contains(&a));
        assert!(app.focused_selection().contains(&b));
        assert_eq!(app.focused_selection().len(), 1);
    }

    #[test]
    fn test_select_node_single_click_does_not_affect_multi_selection() {
        let mut app = test_app();
        let a = app.add_node_and_sync("a".into(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("b".into(), Point2D::new(100.0, 0.0));
        let c = app.add_node_and_sync("c".into(), Point2D::new(200.0, 0.0));

        // Select a and b via multi-select.
        app.apply_reducer_intents(intents_from_graph_actions(vec![
            GraphAction::SelectNode {
                key: a,
                multi_select: false,
            },
            GraphAction::SelectNode {
                key: b,
                multi_select: true,
            },
        ]));

        // Single click c (no modifier): replaces selection with just c.
        app.apply_reducer_intents(intents_from_graph_actions(vec![GraphAction::SelectNode {
            key: c,
            multi_select: false,
        }]));

        assert!(!app.focused_selection().contains(&a));
        assert!(!app.focused_selection().contains(&b));
        assert!(app.focused_selection().contains(&c));
        assert_eq!(app.focused_selection().len(), 1);
    }

    #[test]
    fn test_lasso_select_action_maps_to_intent() {
        let mut app = test_app();
        let a = app.add_node_and_sync("a".into(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("b".into(), Point2D::new(10.0, 0.0));

        let intents = intents_from_graph_actions(vec![GraphAction::LassoSelect {
            keys: vec![a, b],
            mode: SelectionUpdateMode::Replace,
        }]);
        app.apply_reducer_intents(intents);

        assert_eq!(app.focused_selection().len(), 2);
        assert!(app.focused_selection().contains(&a));
        assert!(app.focused_selection().contains(&b));
        assert_eq!(app.focused_selection().primary(), Some(b));
    }

    #[test]
    fn test_clear_selection_action_maps_to_intent() {
        let mut app = test_app();
        let a = app.add_node_and_sync("a".into(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("b".into(), Point2D::new(10.0, 0.0));

        app.apply_reducer_intents(intents_from_graph_actions(vec![GraphAction::LassoSelect {
            keys: vec![a, b],
            mode: SelectionUpdateMode::Replace,
        }]));
        assert_eq!(app.focused_selection().len(), 2);

        app.apply_reducer_intents(intents_from_graph_actions(vec![
            GraphAction::ClearSelection,
        ]));
        assert!(app.focused_selection().is_empty());
        assert_eq!(app.focused_selection().primary(), None);
    }

    #[test]
    fn test_select_frame_action_maps_to_intent_and_clears_node_selection() {
        let mut app = test_app();
        let key = app.add_node_and_sync(
            "https://example.com/selected".into(),
            Point2D::new(0.0, 0.0),
        );
        app.select_node(key, false);

        let intents = intents_from_graph_actions(vec![GraphAction::SelectFrame(
            "workspace-alpha".to_string(),
        )]);
        app.apply_reducer_intents(intents);

        assert!(app.focused_selection().is_empty());
        assert_eq!(app.selected_frame_name(), Some("workspace-alpha"));
    }

    #[test]
    fn test_focus_frame_action_maps_to_selected_frame_then_open_tile_group() {
        let member_key = NodeKey::new(9);
        let expected_url = crate::util::VersoAddress::frame("workspace-alpha").to_string();
        let intents = intents_from_graph_actions(vec![GraphAction::FocusFrame {
            frame_name: "workspace-alpha".to_string(),
            member_key,
        }]);

        assert_eq!(intents.len(), 2);
        assert!(matches!(
            &intents[0],
            GraphIntent::SetSelectedFrame { frame_name }
                if frame_name.as_deref() == Some("workspace-alpha")
        ));
        assert!(
            matches!(
                &intents[1],
                GraphIntent::OpenFrameTileGroup { url, focus_node }
                    if url == &expected_url && *focus_node == Some(member_key)
            ),
            "expected OpenFrameTileGroup with url={expected_url} and focus_node=Some({member_key:?}), got {:?}",
            &intents[1]
        );
    }

    #[test]
    fn test_set_highlighted_edge_action_maps_to_intent() {
        let mut app = test_app();
        let from = app.add_node_and_sync("from".into(), Point2D::new(0.0, 0.0));
        let to = app.add_node_and_sync("to".into(), Point2D::new(10.0, 0.0));

        let intents =
            intents_from_graph_actions(vec![GraphAction::SetHighlightedEdge { from, to }]);
        app.apply_reducer_intents(intents);

        assert_eq!(
            app.workspace.graph_runtime.highlighted_graph_edge,
            Some((from, to))
        );
    }

    #[test]
    fn test_clear_highlighted_edge_action_maps_to_intent() {
        let mut app = test_app();
        let from = app.add_node_and_sync("from".into(), Point2D::new(0.0, 0.0));
        let to = app.add_node_and_sync("to".into(), Point2D::new(10.0, 0.0));
        app.workspace.graph_runtime.highlighted_graph_edge = Some((from, to));

        let intents = intents_from_graph_actions(vec![GraphAction::ClearHighlightedEdge]);
        app.apply_reducer_intents(intents);

        assert!(app.workspace.graph_runtime.highlighted_graph_edge.is_none());
    }

    #[test]
    fn test_primary_click_handler_detects_selection_action() {
        let actions = vec![GraphAction::SelectNode {
            key: NodeKey::new(0),
            multi_select: false,
        }];

        assert!(actions.iter().any(action_handles_primary_click));
    }

    #[test]
    fn test_primary_click_handler_detects_frame_actions() {
        let actions = vec![
            GraphAction::SelectFrame("workspace-alpha".to_string()),
            GraphAction::FocusFrame {
                frame_name: "workspace-alpha".to_string(),
                member_key: NodeKey::new(7),
            },
        ];

        assert!(actions.iter().any(action_handles_primary_click));
    }

    #[test]
    fn test_primary_click_handler_ignores_zoom_only_actions() {
        let actions = vec![GraphAction::Zoom(1.2)];

        assert!(!actions.iter().any(action_handles_primary_click));
    }

    #[test]
    fn test_node_key_or_emit_ambiguous_hit_emits_diagnostic_on_none() {
        let mut diagnostics = DiagnosticsState::new();

        let resolved = node_key_or_emit_ambiguous_hit(None);

        assert!(resolved.is_none());
        diagnostics.force_drain_for_tests();
        let snapshot = diagnostics.snapshot_json_for_tests();
        let channel_count = snapshot
            .get("channels")
            .and_then(|c| c.get("message_counts"))
            .and_then(|m| m.get(CHANNEL_UI_GRAPH_SELECTION_AMBIGUOUS_HIT))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        assert_eq!(channel_count, 1);
    }

    #[test]
    fn test_node_key_or_emit_ambiguous_hit_does_not_emit_for_valid_node() {
        let mut diagnostics = DiagnosticsState::new();
        let key = NodeKey::new(42);

        let resolved = node_key_or_emit_ambiguous_hit(Some(key));

        assert_eq!(resolved, Some(key));
        diagnostics.force_drain_for_tests();
        let snapshot = diagnostics.snapshot_json_for_tests();
        let channel_count = snapshot
            .get("channels")
            .and_then(|c| c.get("message_counts"))
            .and_then(|m| m.get(CHANNEL_UI_GRAPH_SELECTION_AMBIGUOUS_HIT))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        assert_eq!(channel_count, 0);
    }

    #[test]
    fn test_background_click_clear_selection_requires_no_modifiers() {
        let modifiers = egui::Modifiers::CTRL;

        assert!(!should_clear_selection_on_background_click(
            true, modifiers, None, false, false, false,
        ));
    }

    #[test]
    fn test_background_click_clear_selection_blocked_when_radial_open() {
        assert!(!should_clear_selection_on_background_click(
            true,
            egui::Modifiers::NONE,
            None,
            false,
            true,
            false,
        ));
    }

    #[test]
    fn test_background_click_clear_selection_allowed_when_plain_click() {
        assert!(should_clear_selection_on_background_click(
            true,
            egui::Modifiers::NONE,
            None,
            false,
            false,
            false,
        ));
    }

    #[test]
    fn visible_graph_screen_rects_clip_to_navigation_geometry() {
        let mut app = GraphBrowserApp::new_for_testing();
        let screen_rect = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(400.0, 300.0));
        app.workspace.graph_runtime.workbench_navigation_geometry =
            Some(crate::app::WorkbenchNavigationGeometry::from_content_rect(
                screen_rect,
                vec![egui::Rect::from_min_max(
                    egui::pos2(320.0, 40.0),
                    egui::pos2(400.0, 260.0),
                )],
            ));

        let visible_regions = graph_visible_screen_rects(screen_rect, &app);

        assert_eq!(visible_regions.len(), 3);
        assert!(visible_regions.contains_rect(egui::Rect::from_min_max(
            egui::pos2(0.0, 40.0),
            egui::pos2(320.0, 260.0),
        )));
    }

    #[test]
    fn effective_graph_screen_rect_prefers_largest_visible_navigation_region() {
        let mut app = GraphBrowserApp::new_for_testing();
        let screen_rect = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(400.0, 300.0));
        app.workspace.graph_runtime.workbench_navigation_geometry =
            Some(crate::app::WorkbenchNavigationGeometry::from_content_rect(
                screen_rect,
                vec![egui::Rect::from_min_max(
                    egui::pos2(320.0, 40.0),
                    egui::pos2(400.0, 260.0),
                )],
            ));

        assert_eq!(
            effective_graph_screen_rect(screen_rect, &app),
            Some(egui::Rect::from_min_max(
                egui::pos2(0.0, 40.0),
                egui::pos2(320.0, 260.0),
            )),
        );
    }

    #[test]
    fn test_zoom_action_clamps() {
        let mut app = test_app();

        let intents = intents_from_graph_actions(vec![GraphAction::Zoom(0.01)]);
        app.apply_reducer_intents(intents);

        // Should be clamped to min zoom
        assert!(
            app.workspace.graph_runtime.camera.current_zoom
                >= app.workspace.graph_runtime.camera.zoom_min
        );
    }

    #[test]
    fn test_multiple_actions_sequence() {
        let mut app = test_app();
        let view_id = crate::app::GraphViewId::new();
        app.workspace.graph_runtime.views.insert(
            view_id,
            crate::app::GraphViewState::new_with_id(view_id, "Focused"),
        );
        app.workspace.graph_runtime.focused_view = Some(view_id);
        let k1 = app.add_node_and_sync("a".into(), Point2D::new(0.0, 0.0));
        let k2 = app.add_node_and_sync("b".into(), Point2D::new(100.0, 100.0));

        let intents = intents_from_graph_actions(vec![
            GraphAction::SelectNode {
                key: k1,
                multi_select: false,
            },
            GraphAction::MoveNode(k2, Point2D::new(200.0, 300.0)),
            GraphAction::Zoom(1.5),
        ]);
        app.apply_reducer_intents(intents);

        assert!(app.focused_selection().contains(&k1));
        assert_eq!(
            app.workspace
                .domain
                .graph
                .get_node(k2)
                .unwrap()
                .projected_position(),
            Point2D::new(200.0, 300.0)
        );
        assert!(
            (app.workspace.graph_runtime.views[&view_id]
                .camera
                .current_zoom
                - 1.5)
                .abs()
                < 0.01
        );
    }

    #[test]
    fn test_empty_actions_is_noop() {
        let mut app = test_app();
        let key = app.add_node_and_sync("a".into(), Point2D::new(50.0, 60.0));
        let pos_before = app
            .workspace
            .domain
            .graph
            .get_node(key)
            .unwrap()
            .projected_position();

        let intents = intents_from_graph_actions(vec![]);
        app.apply_reducer_intents(intents);

        assert_eq!(
            app.workspace
                .domain
                .graph
                .get_node(key)
                .unwrap()
                .projected_position(),
            pos_before
        );
    }

    #[test]
    fn test_pair_command_context_prefers_selected_pair() {
        let mut app = test_app();
        let a = app.add_node_and_sync("a".into(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("b".into(), Point2D::new(100.0, 0.0));
        app.select_node(b, false);
        app.select_node(a, true);

        let resolved = resolve_pair_command_context(&app, Some(b), Some(b));
        assert_eq!(resolved, Some((b, a)));
    }

    #[test]
    fn test_pair_command_context_falls_back_to_focused_node() {
        let mut app = test_app();
        let a = app.add_node_and_sync("a".into(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("b".into(), Point2D::new(100.0, 0.0));
        app.select_node(a, false);

        let resolved = resolve_pair_command_context(&app, None, Some(b));
        assert_eq!(resolved, Some((a, b)));
    }

    #[test]
    fn test_pair_command_context_prefers_hover_over_focused_for_single_select() {
        let mut app = test_app();
        let a = app.add_node_and_sync("a".into(), Point2D::new(0.0, 0.0));
        let hovered = app.add_node_and_sync("hovered".into(), Point2D::new(100.0, 0.0));
        let focused = app.add_node_and_sync("focused".into(), Point2D::new(200.0, 0.0));
        app.select_node(a, false);

        let resolved = resolve_pair_command_context(&app, Some(hovered), Some(focused));
        assert_eq!(resolved, Some((a, hovered)));
    }

    #[test]
    fn test_pair_command_context_prefers_pending_target_over_hover_and_focused() {
        let mut app = test_app();
        let a = app.add_node_and_sync("a".into(), Point2D::new(0.0, 0.0));
        let pending = app.add_node_and_sync("pending".into(), Point2D::new(50.0, 0.0));
        let hovered = app.add_node_and_sync("hovered".into(), Point2D::new(100.0, 0.0));
        let focused = app.add_node_and_sync("focused".into(), Point2D::new(200.0, 0.0));
        app.select_node(a, false);
        app.set_pending_node_context_target(Some(pending));

        let resolved = resolve_pair_command_context(&app, Some(hovered), Some(focused));
        assert_eq!(resolved, Some((a, pending)));
    }

    #[test]
    fn test_source_context_prefers_pending_then_selected_then_hover_then_focused() {
        let mut app = test_app();
        let pending = app.add_node_and_sync("pending".into(), Point2D::new(-10.0, 0.0));
        let selected = app.add_node_and_sync("selected".into(), Point2D::new(0.0, 0.0));
        let hovered = app.add_node_and_sync("hovered".into(), Point2D::new(10.0, 0.0));
        let focused = app.add_node_and_sync("focused".into(), Point2D::new(20.0, 0.0));

        app.set_pending_node_context_target(Some(pending));
        app.select_node(selected, false);
        assert_eq!(
            resolve_source_node_context(&app, Some(hovered), Some(focused)),
            Some(pending)
        );
        app.set_pending_node_context_target(None);
        assert_eq!(
            resolve_source_node_context(&app, Some(hovered), Some(focused)),
            Some(selected)
        );
        app.clear_selection();
        assert_eq!(
            resolve_source_node_context(&app, Some(hovered), Some(focused)),
            Some(hovered)
        );
        assert_eq!(
            resolve_source_node_context(&app, None, Some(focused)),
            Some(focused)
        );
    }

    #[test]
    fn graph_surface_navigation_prefers_nearest_aligned_candidate() {
        let current = NodeKey::new(1);
        let straight = NodeKey::new(2);
        let diagonal = NodeKey::new(3);
        let proxies = vec![
            HitProxy::Node {
                id: current,
                center: Point2D::new(100.0, 100.0),
                radius: 16.0,
            },
            HitProxy::Node {
                id: straight,
                center: Point2D::new(160.0, 100.0),
                radius: 16.0,
            },
            HitProxy::Node {
                id: diagonal,
                center: Point2D::new(140.0, 135.0),
                radius: 16.0,
            },
        ];

        let target = graph_surface_navigation_target(
            &proxies,
            Some(current),
            SpatialNavigationDirection::Right,
        );

        assert_eq!(target, Some(straight));
    }

    #[test]
    fn graph_surface_navigation_seeds_from_visible_nodes_when_selection_missing() {
        let leftmost = NodeKey::new(11);
        let center = NodeKey::new(12);
        let rightmost = NodeKey::new(13);
        let proxies = vec![
            HitProxy::Node {
                id: leftmost,
                center: Point2D::new(40.0, 120.0),
                radius: 16.0,
            },
            HitProxy::Node {
                id: center,
                center: Point2D::new(120.0, 120.0),
                radius: 16.0,
            },
            HitProxy::Node {
                id: rightmost,
                center: Point2D::new(220.0, 120.0),
                radius: 16.0,
            },
        ];

        let target =
            graph_surface_navigation_target(&proxies, None, SpatialNavigationDirection::Right);

        assert_eq!(target, Some(center));
    }

    #[test]
    fn graph_surface_navigation_selection_extends_with_shift() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_id = GraphViewId::new();
        let first =
            app.add_node_and_sync("https://first.example".to_string(), Point2D::new(0.0, 0.0));
        let second = app.add_node_and_sync(
            "https://second.example".to_string(),
            Point2D::new(140.0, 0.0),
        );
        app.workspace.graph_runtime.focused_view = Some(view_id);
        app.update_focused_selection(vec![first], SelectionUpdateMode::Replace);

        apply_graph_surface_navigation_selection(&mut app, view_id, second, true);

        assert!(app.focused_selection().contains(&first));
        assert!(app.focused_selection().contains(&second));
        assert_eq!(app.focused_selection().primary(), Some(second));
    }

    #[test]
    fn graph_surface_navigation_selection_promotes_existing_target_when_extending() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_id = GraphViewId::new();
        let first =
            app.add_node_and_sync("https://first.example".to_string(), Point2D::new(0.0, 0.0));
        let second = app.add_node_and_sync(
            "https://second.example".to_string(),
            Point2D::new(140.0, 0.0),
        );
        app.workspace.graph_runtime.focused_view = Some(view_id);
        app.update_focused_selection(vec![first, second], SelectionUpdateMode::Replace);

        apply_graph_surface_navigation_selection(&mut app, view_id, first, true);

        assert!(app.focused_selection().contains(&first));
        assert!(app.focused_selection().contains(&second));
        assert_eq!(app.focused_selection().primary(), Some(first));
    }

    #[test]
    fn graph_surface_all_selectable_node_keys_respects_active_filter_matches() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_id = GraphViewId::new();
        let alpha =
            app.add_node_and_sync("https://alpha.example".to_string(), Point2D::new(0.0, 0.0));
        let beta =
            app.add_node_and_sync("https://beta.example".to_string(), Point2D::new(50.0, 0.0));
        let gamma = app.add_node_and_sync(
            "https://gamma.example".to_string(),
            Point2D::new(100.0, 0.0),
        );
        let search_matches = HashSet::from([beta, gamma]);

        let keys = graph_surface_all_selectable_node_keys(
            &app,
            view_id,
            &search_matches,
            SearchDisplayMode::Filter,
            true,
        );

        assert_eq!(keys, vec![beta, gamma]);
        assert!(!keys.contains(&alpha));
    }

    #[test]
    fn graph_surface_visible_node_keys_deduplicate_and_sort_by_key() {
        let low = NodeKey::new(3);
        let high = NodeKey::new(9);
        let proxies = vec![
            HitProxy::Node {
                id: high,
                center: Point2D::new(90.0, 50.0),
                radius: 12.0,
            },
            HitProxy::Node {
                id: low,
                center: Point2D::new(30.0, 50.0),
                radius: 12.0,
            },
            HitProxy::Node {
                id: high,
                center: Point2D::new(90.0, 50.0),
                radius: 12.0,
            },
        ];

        assert_eq!(graph_surface_visible_node_keys(&proxies), vec![low, high]);
    }

    #[test]
    fn graph_surface_tooltip_content_includes_node_lifecycle_and_tags() {
        let mut app = GraphBrowserApp::new_for_testing();
        let node_key = app.add_node_and_sync(
            "https://focused.example".to_string(),
            Point2D::new(0.0, 0.0),
        );
        let node = app
            .domain_graph_mut()
            .get_node_mut(node_key)
            .expect("test node should exist");
        node.title = "Focused Example".to_string();
        node.lifecycle = crate::graph::NodeLifecycle::Warm;
        node.tags.insert("#research".to_string());
        app.select_node(node_key, false);

        let content = graph_surface_tooltip_content(&app, GraphTooltipTarget::Node(node_key))
            .expect("node tooltip content should exist");

        assert_eq!(content.title, "Focused Example");
        assert!(
            content
                .details
                .iter()
                .any(|detail| detail == "Lifecycle: Warm")
        );
        assert!(
            content
                .details
                .iter()
                .any(|detail| detail.contains("Semantic tags: #research"))
        );
        assert!(
            content
                .details
                .iter()
                .any(|detail| detail.contains("State: focused"))
        );
    }

    #[test]
    fn graph_surface_tooltip_content_includes_edge_kinds_and_label() {
        let mut app = GraphBrowserApp::new_for_testing();
        let from =
            app.add_node_and_sync("https://alpha.example".to_string(), Point2D::new(0.0, 0.0));
        let to =
            app.add_node_and_sync("https://beta.example".to_string(), Point2D::new(100.0, 0.0));
        app.domain_graph_mut()
            .get_node_mut(from)
            .expect("source node should exist")
            .title = "Alpha".to_string();
        app.domain_graph_mut()
            .get_node_mut(to)
            .expect("target node should exist")
            .title = "Beta".to_string();
        app.domain_graph_mut()
            .add_edge(
                from,
                to,
                EdgeType::UserGrouped,
                Some("tab-group".to_string()),
            )
            .expect("edge should be created");

        let content = graph_surface_tooltip_content(&app, GraphTooltipTarget::Edge { from, to })
            .expect("edge tooltip content should exist");

        assert_eq!(content.title, "Alpha -> Beta");
        assert!(
            content
                .details
                .iter()
                .any(|detail| detail.contains("Kinds: User grouped"))
        );
        assert!(
            content
                .details
                .iter()
                .any(|detail| detail.contains("Label: tab-group"))
        );
    }

    #[test]
    fn graph_surface_tooltip_dismissal_clears_when_target_changes() {
        let alpha = GraphTooltipTarget::Node(NodeKey::new(1));
        let beta = GraphTooltipTarget::Node(NodeKey::new(2));

        assert_eq!(
            next_graph_surface_tooltip_dismissed_target(Some(alpha), Some(alpha), false),
            Some(alpha)
        );
        assert_eq!(
            next_graph_surface_tooltip_dismissed_target(Some(alpha), Some(beta), false),
            None
        );
        assert_eq!(
            next_graph_surface_tooltip_dismissed_target(None, Some(beta), true),
            Some(beta)
        );
    }

    #[test]
    fn apply_graph_surface_select_all_replaces_selection_with_target_keys() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_id = GraphViewId::new();
        let first =
            app.add_node_and_sync("https://first.example".to_string(), Point2D::new(0.0, 0.0));
        let second = app.add_node_and_sync(
            "https://second.example".to_string(),
            Point2D::new(140.0, 0.0),
        );
        let third = app.add_node_and_sync(
            "https://third.example".to_string(),
            Point2D::new(280.0, 0.0),
        );
        app.workspace.graph_runtime.focused_view = Some(view_id);
        app.update_focused_selection(vec![first], SelectionUpdateMode::Replace);

        apply_graph_surface_select_all(&mut app, view_id, vec![second, third]);

        assert!(!app.focused_selection().contains(&first));
        assert!(app.focused_selection().contains(&second));
        assert!(app.focused_selection().contains(&third));
        assert_eq!(app.focused_selection().primary(), Some(third));
    }

    #[test]
    fn secondary_click_on_node_opens_radial_palette_when_preferred() {
        let mut app = test_app();
        let ctx = egui::Context::default();
        let view_id = crate::app::GraphViewId::new();
        let target = NodeKey::new(21);
        let pointer = egui::pos2(144.0, 233.0);
        app.set_context_command_surface_preference(
            crate::app::ContextCommandSurfacePreference::RadialPalette,
        );
        app.workspace.chrome_ui.show_context_palette = true;
        app.workspace.chrome_ui.command_palette_contextual_mode = true;

        handle_hovered_node_secondary_click(&ctx, &mut app, view_id, target, Some(pointer));

        assert!(!app.workspace.chrome_ui.show_radial_menu);
        assert!(app.workspace.chrome_ui.show_context_palette);
        assert!(app.pending_node_context_target().is_none());
        assert!(app.workspace.chrome_ui.command_palette_contextual_mode);
        assert!(matches!(
            app.take_pending_workbench_intents().as_slice(),
            [WorkbenchIntent::ToggleRadialMenu]
        ));
        let stored_center =
            ctx.data_mut(|d| d.get_persisted::<egui::Pos2>(egui::Id::new("radial_menu_center")));
        assert_eq!(stored_center, Some(pointer));
    }

    #[test]
    fn secondary_click_on_node_opens_context_palette_when_preferred() {
        let mut app = test_app();
        let ctx = egui::Context::default();
        let view_id = crate::app::GraphViewId::new();
        let target = NodeKey::new(22);
        app.set_context_command_surface_preference(
            crate::app::ContextCommandSurfacePreference::ContextPalette,
        );
        app.workspace.chrome_ui.show_radial_menu = true;

        handle_hovered_node_secondary_click(&ctx, &mut app, view_id, target, None);

        assert!(app.workspace.chrome_ui.show_context_palette);
        assert!(!app.workspace.chrome_ui.show_radial_menu);
        assert!(app.workspace.chrome_ui.command_palette_contextual_mode);
        assert_eq!(app.pending_node_context_target(), Some(target));
        assert_eq!(
            app.pending_command_surface_return_target(),
            Some(crate::app::ToolSurfaceReturnTarget::Graph(view_id))
        );
    }

    #[test]
    fn secondary_click_on_frame_opens_context_palette_when_preferred() {
        let mut app = test_app();
        let ctx = egui::Context::default();
        let view_id = crate::app::GraphViewId::new();
        app.set_context_command_surface_preference(
            crate::app::ContextCommandSurfacePreference::ContextPalette,
        );

        handle_frame_backdrop_secondary_click(
            &ctx,
            &mut app,
            view_id,
            "workspace-alpha".to_string(),
            None,
        );

        assert!(app.workspace.chrome_ui.show_context_palette);
        assert!(app.workspace.chrome_ui.command_palette_contextual_mode);
        assert_eq!(app.pending_frame_context_target(), Some("workspace-alpha"));
        assert_eq!(app.pending_node_context_target(), None);
    }

    #[test]
    fn secondary_click_on_frame_opens_radial_palette_when_preferred() {
        let mut app = test_app();
        let ctx = egui::Context::default();
        let view_id = crate::app::GraphViewId::new();
        let pointer = egui::pos2(88.0, 144.0);
        app.set_context_command_surface_preference(
            crate::app::ContextCommandSurfacePreference::RadialPalette,
        );
        app.workspace.chrome_ui.show_context_palette = true;
        app.workspace.chrome_ui.command_palette_contextual_mode = true;

        handle_frame_backdrop_secondary_click(
            &ctx,
            &mut app,
            view_id,
            "workspace-alpha".to_string(),
            Some(pointer),
        );

        assert_eq!(app.pending_frame_context_target(), Some("workspace-alpha"));
        assert_eq!(app.pending_node_context_target(), None);
        assert!(matches!(
            app.take_pending_workbench_intents().as_slice(),
            [WorkbenchIntent::ToggleRadialMenu]
        ));
        let stored_center =
            ctx.data_mut(|d| d.get_persisted::<egui::Pos2>(egui::Id::new("radial_menu_center")));
        assert_eq!(stored_center, Some(pointer));
    }

    #[test]
    fn test_neighbor_set_computation() {
        let mut app = test_app();
        let a = app.add_node_and_sync("a".into(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("b".into(), Point2D::new(10.0, 0.0));
        let c = app.add_node_and_sync("c".into(), Point2D::new(20.0, 0.0));
        let _ = app.assert_relation_and_sync(
            a,
            b,
            crate::graph::EdgeAssertion::Semantic {
                sub_kind: crate::graph::SemanticSubKind::Hyperlink,
                label: None,
                decay_progress: None,
            },
        );
        let _ = app.assert_relation_and_sync(
            c,
            a,
            crate::graph::EdgeAssertion::Semantic {
                sub_kind: crate::graph::SemanticSubKind::Hyperlink,
                label: None,
                decay_progress: None,
            },
        );

        let set = hovered_adjacency_set(&app, Some(a));
        assert!(set.contains(&a));
        assert!(set.contains(&b));
        assert!(set.contains(&c));
    }

    #[test]
    fn test_search_filter_mode_hides_nodes() {
        let mut app = test_app();
        let a = app.add_node_and_sync("alpha".into(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("beta".into(), Point2D::new(10.0, 0.0));
        app.workspace.graph_runtime.search_display_mode = SearchDisplayMode::Filter;
        let matches = HashSet::from([a]);
        let filtered = filtered_graph_for_search(&app, &matches);
        assert!(filtered.get_node(a).is_some());
        assert!(filtered.get_node(b).is_none());
    }

    #[test]
    fn selected_node_enrichment_summary_includes_tags_suggestions_and_anchor() {
        let mut app = test_app();
        let math = app.add_node_and_sync("https://example.com/math".into(), Point2D::new(0.0, 0.0));
        let numerical = app.add_node_and_sync(
            "https://example.com/numerical".into(),
            Point2D::new(10.0, 0.0),
        );
        if let Some(node) = app.workspace.domain.graph.get_node_mut(math) {
            node.title = "Mathematics".into();
        }
        if let Some(node) = app.workspace.domain.graph.get_node_mut(numerical) {
            node.title = "Numerical Methods".into();
        }

        let _ = app
            .workspace
            .domain
            .graph
            .insert_node_tag(math, "udc:51".to_string());
        let _ = app
            .workspace
            .domain
            .graph
            .insert_node_tag(numerical, "udc:519.6".to_string());
        app.workspace
            .graph_runtime
            .suggested_semantic_tags
            .insert(numerical, vec!["udc:519.8".to_string()]);
        app.workspace.graph_runtime.semantic_index_dirty = true;
        let _ = crate::shell::desktop::runtime::registries::knowledge::reconcile_semantics(
            &mut app,
            &crate::shell::desktop::runtime::registries::knowledge::KnowledgeRegistry::default(),
        );

        let summary = selected_node_enrichment_summary(&app, numerical).expect("summary");

        assert!(
            summary
                .display_tags
                .iter()
                .any(|tag| tag.chip.label.contains("Computational mathematics"))
        );
        assert!(
            summary
                .display_tags
                .iter()
                .any(|tag| tag.status.contains("KnowledgeRegistry canonical"))
        );
        assert!(
            summary
                .suggested_tags
                .iter()
                .any(|tag| tag.chip.label.contains("Control"))
                || !summary.suggested_tags.is_empty()
        );
        assert!(
            summary
                .suggested_tags
                .iter()
                .all(|tag| !tag.reason.is_empty())
        );
        let anchor = summary.placement_anchor.expect("placement anchor");
        assert_eq!(anchor.key, math);
        assert_eq!(anchor.label, "Mathematics");
        assert_eq!(anchor.slice_tag.as_deref(), Some("udc:51"));
        assert!(summary.semantic_lens_available);
        assert!(!summary.semantic_lens_active);
    }

    #[test]
    fn graph_view_semantic_depth_status_badge_only_reports_udc_depth_mode() {
        let mut app = test_app();
        let view_id = crate::app::GraphViewId::new();
        let mut view = crate::app::GraphViewState::new_with_id(view_id, "Semantic");
        view.dimension = ViewDimension::ThreeD {
            mode: ThreeDMode::TwoPointFive,
            z_source: ZSource::UdcLevel { scale: 48.0 },
        };
        app.workspace.graph_runtime.views.insert(view_id, view);

        let badge = graph_view_semantic_depth_status_badge(&app, view_id);

        assert_eq!(badge.map(|(label, _)| label), Some("UDC Depth"));
    }

    #[test]
    fn graph_view_semantic_depth_status_badge_ignores_other_view_dimensions() {
        let mut app = test_app();
        let view_id = crate::app::GraphViewId::new();
        let mut view = crate::app::GraphViewState::new_with_id(view_id, "Standard");
        view.dimension = ViewDimension::ThreeD {
            mode: ThreeDMode::Isometric,
            z_source: ZSource::BfsDepth { scale: 12.0 },
        };
        app.workspace.graph_runtime.views.insert(view_id, view);

        assert_eq!(graph_view_semantic_depth_status_badge(&app, view_id), None);
    }

    #[test]
    fn ranked_tag_suggestions_include_existing_and_knowledge_matches() {
        let mut app = test_app();
        let node = app.add_node_and_sync("https://example.com".into(), Point2D::new(0.0, 0.0));
        let _ = app
            .workspace
            .domain
            .graph
            .insert_node_tag(node, "#starred".to_string());
        let other =
            app.add_node_and_sync("https://example.com/math".into(), Point2D::new(10.0, 0.0));
        let _ = app
            .workspace
            .domain
            .graph
            .insert_node_tag(other, "research".to_string());

        let existing_tag_suggestions = ranked_tag_suggestions(&app, node, "rese");
        let existing_queries = existing_tag_suggestions
            .iter()
            .map(|chip| chip.query.clone())
            .collect::<Vec<_>>();
        assert!(existing_queries.contains(&"research".to_string()));

        let knowledge_suggestions = ranked_tag_suggestions(&app, node, "math");
        let knowledge_queries = knowledge_suggestions
            .iter()
            .map(|chip| chip.query.clone())
            .collect::<Vec<_>>();
        assert!(
            knowledge_queries
                .iter()
                .any(|query| query.starts_with("udc:"))
        );
    }

    #[test]
    fn reserved_tag_warning_flags_unknown_hash_tag() {
        assert_eq!(
            reserved_tag_warning("#custom"),
            Some("Unknown #tag will be accepted as user-defined.".to_string())
        );
        assert_eq!(reserved_tag_warning(GraphBrowserApp::TAG_PIN), None);
        assert_eq!(reserved_tag_warning("research"), None);
    }

    #[test]
    fn viewport_culling_metrics_reduce_visible_set_and_submission_units() {
        let mut app = test_app();
        let mut keys = Vec::new();
        for row in 0..20 {
            for col in 0..20 {
                let key = app.add_node_and_sync(
                    format!("https://example.com/{row}/{col}"),
                    Point2D::new(col as f32 * 50.0, row as f32 * 50.0),
                );
                keys.push(key);
            }
        }

        for pair in keys.windows(2) {
            let _ = app.assert_relation_and_sync(
                pair[0],
                pair[1],
                crate::graph::EdgeAssertion::Semantic {
                    sub_kind: crate::graph::SemanticSubKind::Hyperlink,
                    label: None,
                    decay_progress: None,
                },
            );
        }

        let canvas_rect = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(300.0, 300.0));
        let metrics =
            viewport_culling_metrics_for_canvas_rect(&app.workspace.domain.graph, canvas_rect)
                .expect("culling metrics should be available for dense graph viewport");

        assert!(metrics.visible_nodes < metrics.total_nodes);
        assert!(metrics.submitted_nodes < metrics.total_nodes);
        assert!(metrics.removed_nodes > 0);
        assert!(metrics.culled_submission_units < metrics.full_submission_units);
    }

    #[test]
    fn canvas_rect_from_view_frame_respects_pan_and_zoom() {
        let screen = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(200.0, 200.0));
        let frame = crate::app::GraphViewFrame {
            zoom: 2.0,
            pan_x: 50.0,
            pan_y: 20.0,
        };

        let canvas = canvas_rect_from_view_frame(screen, frame)
            .expect("non-zero zoom should produce a canvas rect");

        assert!((canvas.min.x + 25.0).abs() < 0.001);
        assert!((canvas.min.y + 10.0).abs() < 0.001);
        assert!((canvas.max.x - 75.0).abs() < 0.001);
        assert!((canvas.max.y - 90.0).abs() < 0.001);
    }

    #[test]
    fn viewport_culling_differs_for_distinct_graph_view_frames() {
        let mut app = test_app();
        let mut keys = Vec::new();
        for idx in 0..20 {
            keys.push(app.add_node_and_sync(
                format!("https://pane.example/{idx}"),
                Point2D::new(idx as f32 * 120.0, 0.0),
            ));
        }

        let screen = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(300.0, 200.0));
        let near_origin = crate::app::GraphViewFrame {
            zoom: 1.0,
            pan_x: 0.0,
            pan_y: 0.0,
        };
        let shifted = crate::app::GraphViewFrame {
            zoom: 1.0,
            pan_x: -1200.0,
            pan_y: 0.0,
        };

        let rect_a = canvas_rect_from_view_frame(screen, near_origin).unwrap();
        let rect_b = canvas_rect_from_view_frame(screen, shifted).unwrap();
        let selection_a =
            viewport_culling_selection_for_canvas_rect(&app.workspace.domain.graph, rect_a)
                .expect("expected culling selection for first view frame");
        let selection_b =
            viewport_culling_selection_for_canvas_rect(&app.workspace.domain.graph, rect_b)
                .expect("expected culling selection for second view frame");

        assert!(
            selection_a.visible.contains(&keys[0]),
            "origin view should include early nodes"
        );
        assert!(
            !selection_b.visible.contains(&keys[0]),
            "shifted view should exclude early nodes"
        );
        assert!(
            selection_b.visible.contains(&keys[10]),
            "shifted view should include later nodes"
        );
    }
}
