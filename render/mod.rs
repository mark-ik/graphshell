/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Graph rendering module using egui_graphs.
//!
//! Delegates graph visualization and interaction to the egui_graphs crate,
//! which provides built-in navigation (zoom/pan), node dragging, and selection.

use crate::app::{
    CameraCommand, ChooseFramePickerMode, GraphBrowserApp, GraphIntent, HistoryManagerTab,
    KeyboardZoomRequest, SearchDisplayMode, SelectionUpdateMode, UnsavedFramePromptAction,
    UnsavedFramePromptRequest,
};
use crate::graph::egui_adapter::{EguiGraphState, GraphEdgeShape, GraphNodeShape};
use crate::graph::{NodeKey, NodeLifecycle};
use crate::registries::domain::layout::LayoutDomainRegistry;
use crate::registries::domain::layout::canvas::{CANVAS_PROFILE_DEFAULT, CanvasLassoBinding};
use crate::registries::domain::layout::viewer_surface::VIEWER_SURFACE_DEFAULT;
use crate::registries::domain::layout::workbench_surface::WORKBENCH_SURFACE_DEFAULT;
use crate::shell::desktop::runtime::diagnostics::{DiagnosticEvent, emit_event};
use crate::shell::desktop::runtime::registries::CHANNEL_UI_HISTORY_MANAGER_LIMIT;
use egui::{Color32, Stroke, Ui, Vec2, Window};
use egui_graphs::events::Event;
use egui_graphs::{
    FruchtermanReingoldWithCenterGravity, FruchtermanReingoldWithCenterGravityState, GraphView,
    LayoutForceDirected, MetadataFrame, SettingsInteraction, SettingsNavigation, SettingsStyle,
    get_layout_state, set_layout_state,
};
use euclid::default::Point2D;
use petgraph::stable_graph::NodeIndex;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::env;
use std::rc::Rc;
use std::sync::OnceLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

mod spatial_index;
use spatial_index::NodeSpatialIndex;

pub(crate) mod action_registry;
mod command_palette;
mod radial_menu;
pub use command_palette::render_command_palette_panel;
pub use radial_menu::render_radial_command_menu;

/// Graph interaction action (resolved from egui_graphs events).
///
/// Decouples event conversion (needs `egui_state` for NodeIndexâ†’NodeKey
/// lookups) from action application (pure state mutation), making
/// graph interactions testable without an egui rendering context.
pub enum GraphAction {
    FocusNode(NodeKey),
    FocusNodeSplit(NodeKey),
    DragStart,
    DragEnd(NodeKey, Point2D<f32>),
    MoveNode(NodeKey, Point2D<f32>),
    SelectNode {
        key: NodeKey,
        multi_select: bool,
    },
    LassoSelect {
        keys: Vec<NodeKey>,
        mode: SelectionUpdateMode,
    },
    ClearSelection,
    Zoom(f32),
}

fn action_handles_primary_click(action: &GraphAction) -> bool {
    matches!(
        action,
        GraphAction::FocusNode(_)
            | GraphAction::FocusNodeSplit(_)
            | GraphAction::DragStart
            | GraphAction::DragEnd(_, _)
            | GraphAction::MoveNode(_, _)
            | GraphAction::SelectNode { .. }
            | GraphAction::LassoSelect { .. }
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

/// Render graph info and controls hint overlay text into the current UI.
pub fn render_graph_info_in_ui(ui: &mut Ui, app: &GraphBrowserApp) {
    draw_graph_info(ui, app);
}

fn canvas_navigation_settings(
    profile: &crate::registries::domain::layout::canvas::CanvasSurfaceProfile,
) -> SettingsNavigation {
    SettingsNavigation::new()
        .with_fit_to_screen_enabled(profile.navigation.fit_to_screen_enabled)
        // Keep egui_graphs navigation disabled so camera movement is owned by
        // `handle_custom_navigation` only. This avoids duplicate pan/zoom paths
        // fighting each other under active pointer drag.
        .with_zoom_and_pan_enabled(false)
}

fn canvas_interaction_settings(
    profile: &crate::registries::domain::layout::canvas::CanvasSurfaceProfile,
    radial_open: bool,
    right_button_down: bool,
    multi_select_modifier: bool,
) -> SettingsInteraction {
    let is_tree_topology = profile.topology.policy_id == "topology:tree";
    let selection_enabled =
        profile.interaction.node_selection_enabled && !radial_open && !right_button_down;

    SettingsInteraction::new()
        .with_dragging_enabled(
            profile.interaction.dragging_enabled
                && !radial_open
                && !right_button_down
                && !is_tree_topology,
        )
        .with_node_selection_enabled(selection_enabled)
        .with_node_selection_multi_enabled(selection_enabled && multi_select_modifier)
        .with_node_clicking_enabled(profile.interaction.node_clicking_enabled && !radial_open)
}

fn canvas_style_settings(
    profile: &crate::registries::domain::layout::canvas::CanvasSurfaceProfile,
) -> SettingsStyle {
    SettingsStyle::new().with_labels_always(profile.style.labels_always)
}

fn canvas_lasso_binding_label(binding: CanvasLassoBinding) -> &'static str {
    match binding {
        CanvasLassoBinding::RightDrag => "Right-Drag Lasso",
        CanvasLassoBinding::ShiftLeftDrag => "Shift+Left-Drag Lasso",
    }
}

/// Render graph content and return resolved interaction actions.
///
/// This lets callers customize how specific actions are handled
/// (e.g. routing double-click to tile opening instead of detail view).
pub fn render_graph_in_ui_collect_actions(
    ui: &mut Ui,
    app: &mut GraphBrowserApp,
    view_id: crate::app::GraphViewId,
    search_matches: &HashSet<NodeKey>,
    active_search_match: Option<NodeKey>,
    search_display_mode: SearchDisplayMode,
    search_query_active: bool,
) -> Vec<GraphAction> {
    // Ensure a GraphViewState exists for this pane so per-view camera, lens,
    // and layout mode can be stored independently.
    if !app.workspace.views.contains_key(&view_id) {
        app.workspace.views.insert(
            view_id,
            crate::app::GraphViewState::new_with_id(view_id, "Graph View"),
        );
    }

    if app
        .workspace
        .focused_view
        .is_some_and(|focused| !app.workspace.views.contains_key(&focused))
    {
        app.workspace.focused_view = None;
    }

    let ctrl_pressed = ui.input(|i| i.modifiers.ctrl || i.modifiers.command);
    let right_button_down = ui.input(|i| i.pointer.secondary_down());
    let radial_open = app.workspace.show_radial_menu;
    let filtered_graph =
        if matches!(search_display_mode, SearchDisplayMode::Filter) && search_query_active {
            Some(filtered_graph_for_search(app, search_matches))
        } else {
            None
        };

    let layout_domain = LayoutDomainRegistry::default();
    let layout_profile = layout_domain.resolve_profile(
        CANVAS_PROFILE_DEFAULT,
        WORKBENCH_SURFACE_DEFAULT,
        VIEWER_SURFACE_DEFAULT,
    );
    let canvas_profile = &layout_profile.canvas.profile;

    // Viewport culling: compute visible node set from previous-frame camera
    // metadata and exclude off-screen nodes before rebuilding egui_state.
    // Gated by the canvas performance policy toggle.
    let culled_graph =
        if canvas_profile.performance.viewport_culling_enabled && filtered_graph.is_none() {
            viewport_culled_graph(ui, app, view_id)
        } else {
            None
        };

    let graph_for_render = culled_graph
        .as_ref()
        .or(filtered_graph.as_ref())
        .unwrap_or(&app.workspace.graph);

    // Build or reuse egui_graphs state (rebuild always when filtering or culling is active).
    if app.workspace.egui_state.is_none()
        || app.workspace.egui_state_dirty
        || filtered_graph.is_some()
        || culled_graph.is_some()
    {
        let crashed_nodes: HashSet<NodeKey> = app.crash_blocked_node_keys().collect();
        let memberships_by_uuid: HashMap<Uuid, Vec<String>> = graph_for_render
            .nodes()
            .map(|(_, node)| {
                (
                    node.id,
                    app.membership_for_node(node.id)
                        .iter()
                        .cloned()
                        .collect::<Vec<_>>(),
                )
            })
            .collect();
        app.workspace.egui_state = Some(EguiGraphState::from_graph_with_memberships(
            graph_for_render,
            &app.workspace.selected_nodes,
            app.workspace.selected_nodes.primary(),
            &crashed_nodes,
            &memberships_by_uuid,
        ));
        app.workspace.egui_state_dirty = false;
    }

    apply_search_node_visuals(
        app,
        search_matches,
        active_search_match,
        search_query_active,
    );

    // Event collection buffer
    let events: Rc<RefCell<Vec<Event>>> = Rc::new(RefCell::new(Vec::new()));

    // Graph settings resolved from layout domain canvas surface profile.
    let nav = canvas_navigation_settings(canvas_profile);
    let interaction =
        canvas_interaction_settings(canvas_profile, radial_open, right_button_down, ctrl_pressed);
    let style = canvas_style_settings(canvas_profile);

    // Resolve physics state and lens from the view (or default to global).
    let (mut physics_state, lens_config) = if let Some(view) = app.workspace.views.get(&view_id) {
        let state = if let Some(local) = &view.local_simulation {
            local.physics.clone()
        } else {
            app.workspace.physics.clone()
        };
        (state, Some(&view.lens))
    } else {
        (app.workspace.physics.clone(), None)
    };

    if let Some(lens) = lens_config {
        lens.physics.apply_to_state(&mut physics_state);
    }

    // Keep egui_graphs layout cache aligned with app-owned FR state.
    set_layout_state::<FruchtermanReingoldWithCenterGravityState>(ui, physics_state, None);

    // Intercept wheel input before GraphView renders so parent scroll handling
    // cannot consume the delta first.
    let graph_rect = ui.max_rect();
    if ui.rect_contains_pointer(graph_rect) {
        let mut captured_wheel_zoom = false;
        ui.input_mut(|input| {
            let scroll_delta = if input.smooth_scroll_delta.y.abs() > f32::EPSILON {
                input.smooth_scroll_delta.y
            } else {
                input.raw_scroll_delta.y
            };

            if scroll_delta.abs() <= f32::EPSILON {
                return;
            }

            let ctrl_pressed = input.modifiers.ctrl || input.modifiers.command;
            let should_capture = canvas_profile.should_capture_wheel_zoom(ctrl_pressed);

            if should_capture {
                captured_wheel_zoom = true;
                app.queue_pending_wheel_zoom_delta(view_id, scroll_delta);
                input.smooth_scroll_delta.y = 0.0;
                input.raw_scroll_delta.y = 0.0;
            } else {
                emit_event(DiagnosticEvent::MessageReceived {
                    channel_id: "runtime.ui.graph.wheel_zoom_not_captured",
                    latency_us: 0,
                });
            }
        });
        if captured_wheel_zoom {
            app.workspace.focused_view = Some(view_id);
        }
    }

    // Render the graph (nested scope for mutable borrow)
    let response = {
        let state = app
            .workspace
            .egui_state
            .as_mut()
            .expect("egui_state should be initialized");

        ui.add(
            &mut GraphView::<
                _,
                _,
                _,
                _,
                GraphNodeShape,
                GraphEdgeShape,
                FruchtermanReingoldWithCenterGravityState,
                LayoutForceDirected<FruchtermanReingoldWithCenterGravity>,
            >::new(&mut state.graph)
            .with_navigations(&nav)
            .with_interactions(&interaction)
            .with_styles(&style)
            .with_event_sink(&events),
        )
    }; // Drop mutable borrow of app.workspace.egui_state here

    // Pull latest FR state from egui_graphs after this frame's layout step.
    let new_physics = get_layout_state::<FruchtermanReingoldWithCenterGravityState>(ui, None);
    if let Some(view) = app.workspace.views.get_mut(&view_id)
        && let Some(local) = &mut view.local_simulation
    {
        local.physics = new_physics;
    } else {
        app.workspace.physics = new_physics;
    }

    // Apply semantic clustering forces if enabled (UDC Phase 2)
    let semantic_config = app.workspace.views.get(&view_id).map(|v| {
        (
            v.lens.physics.semantic_clustering,
            v.lens.physics.semantic_strength,
        )
    });
    apply_semantic_clustering_forces(app, semantic_config);

    app.workspace.hovered_graph_node = app.workspace.egui_state.as_ref().and_then(|state| {
        state
            .graph
            .hovered_node()
            .and_then(|idx| state.get_key(idx))
    });
    let metadata_id = response.id.with("metadata");
    let lasso = collect_lasso_action(
        ui,
        app,
        !radial_open,
        metadata_id,
        canvas_profile.interaction.lasso_binding,
    );

    if ui.input(|i| i.pointer.secondary_clicked())
        && !lasso.suppress_context_menu
        && let Some(target) = app.workspace.hovered_graph_node
    {
        app.set_pending_node_context_target(Some(target));
        app.workspace.show_radial_menu = true;
        if let Some(pointer) = ui.input(|i| i.pointer.latest_pos()) {
            ui.ctx().data_mut(|d| {
                d.insert_persisted(egui::Id::new("radial_menu_center"), pointer);
            });
        }
    }
    if ui.input(|i| i.pointer.primary_clicked())
        && let Some(target) = app.workspace.hovered_graph_node
        && let Some(pointer) = ui.input(|i| i.pointer.latest_pos())
        && let Some(state) = app.workspace.egui_state.as_ref()
        && let Some(node) = state.graph.node(target)
        && node.display().workspace_membership_count() > 0
    {
        let meta_id = response.id.with("metadata");
        let (circle_center, circle_radius) = if let Some(meta) = ui
            .ctx()
            .data_mut(|d| d.get_persisted::<MetadataFrame>(meta_id))
        {
            (
                meta.canvas_to_screen_pos(node.location()),
                meta.canvas_to_screen_size(node.display().radius()),
            )
        } else {
            (node.location(), node.display().radius())
        };
        if node
            .display()
            .workspace_badge_hit_rect_screen(circle_center, circle_radius)
            .is_some_and(|rect| rect.contains(pointer))
        {
            app.request_choose_frame_picker(target);
        }
    }
    draw_highlighted_edge_overlay(ui, app, response.id);
    draw_hovered_node_tooltip(ui, app, response.id);
    draw_hovered_edge_tooltip(ui, app, response.id);

    // Custom navigation handling (Zoom/Pan/Fit)
    // We use the widget ID from the response to target the correct MetadataFrame.
    // Do not let a sticky radial-menu flag disable graph camera controls.
    // The radial menu renderer runs later in the frame and should own its own clicks,
    // but pan/zoom/fit must remain available for stabilization.
    let custom_zoom = handle_custom_navigation(
        ui,
        &response,
        metadata_id,
        app,
        true,
        view_id,
        canvas_profile,
        radial_open,
        right_button_down,
    );

    if let Some(meta) = ui
        .ctx()
        .data_mut(|d| d.get_persisted::<MetadataFrame>(metadata_id))
    {
        app.workspace.graph_view_frames.insert(
            view_id,
            crate::app::GraphViewFrame {
                zoom: meta.zoom,
                pan_x: meta.pan.x,
                pan_y: meta.pan.y,
            },
        );
    }

    let split_open_modifier = ui.input(|i| i.modifiers.shift);
    let mut actions = collect_graph_actions(app, &events, split_open_modifier, ctrl_pressed);
    let graph_handled_primary_click = actions.iter().any(action_handles_primary_click);
    let clear_selection_on_background_click = ui.input(|i| {
        should_clear_selection_on_background_click(
            i.pointer.primary_clicked(),
            i.modifiers,
            app.workspace.hovered_graph_node,
            graph_handled_primary_click,
            radial_open,
            lasso.action.is_some(),
        )
    });
    if clear_selection_on_background_click {
        actions.push(GraphAction::ClearSelection);
    }
    if let Some(lasso_action) = lasso.action {
        actions.push(lasso_action);
    }
    if let Some(zoom) = custom_zoom {
        actions.push(GraphAction::Zoom(zoom));
    }
    if clear_selection_on_background_click || !actions.is_empty() {
        app.workspace.focused_view = Some(view_id);
    }
    actions
}

fn draw_hovered_edge_tooltip(ui: &Ui, app: &GraphBrowserApp, widget_id: egui::Id) {
    if app.workspace.hovered_graph_node.is_some() {
        return;
    }
    let Some(pointer) = ui.input(|i| i.pointer.hover_pos()) else {
        return;
    };
    let Some(state) = app.workspace.egui_state.as_ref() else {
        return;
    };
    let meta_id = widget_id.with("metadata");
    let Some(meta) = ui
        .ctx()
        .data_mut(|d| d.get_persisted::<MetadataFrame>(meta_id))
    else {
        return;
    };
    let Some(edge_id) = state.graph.edge_by_screen_pos(&meta, pointer) else {
        return;
    };
    let Some((from, to)) = state.graph.edge_endpoints(edge_id) else {
        return;
    };

    let ab_payload = app
        .workspace
        .graph
        .find_edge_key(from, to)
        .and_then(|k| app.workspace.graph.get_edge(k));
    let ba_payload = app
        .workspace
        .graph
        .find_edge_key(to, from)
        .and_then(|k| app.workspace.graph.get_edge(k));

    let ab_count = ab_payload.map(|p| p.traversals.len()).unwrap_or(0);
    let ba_count = ba_payload.map(|p| p.traversals.len()).unwrap_or(0);
    let total = ab_count + ba_count;
    if total == 0 {
        return;
    }

    let latest_ts = ab_payload
        .into_iter()
        .flat_map(|p| p.traversals.iter().map(|t| t.timestamp_ms))
        .chain(
            ba_payload
                .into_iter()
                .flat_map(|p| p.traversals.iter().map(|t| t.timestamp_ms)),
        )
        .max();

    let from_label = app
        .workspace
        .graph
        .get_node(from)
        .map(|n| n.title.as_str())
        .filter(|t| !t.is_empty())
        .or_else(|| app.workspace.graph.get_node(from).map(|n| n.url.as_str()))
        .unwrap_or("unknown");
    let to_label = app
        .workspace
        .graph
        .get_node(to)
        .map(|n| n.title.as_str())
        .filter(|t| !t.is_empty())
        .or_else(|| app.workspace.graph.get_node(to).map(|n| n.url.as_str()))
        .unwrap_or("unknown");

    let latest_text = latest_ts
        .and_then(|ms| {
            UNIX_EPOCH
                .checked_add(Duration::from_millis(ms))
                .and_then(|ts| ts.duration_since(UNIX_EPOCH).ok())
                .map(|d| format!("{}s", d.as_secs()))
        })
        .unwrap_or_else(|| "unknown".to_string());

    egui::Area::new(widget_id.with("edge_hover_tooltip"))
        .order(egui::Order::Tooltip)
        .fixed_pos(pointer + Vec2::new(14.0, 14.0))
        .show(ui.ctx(), |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_max_width(320.0);
                ui.label(egui::RichText::new("Traversal Edge").strong());
                ui.label(format!("{from_label} <-> {to_label}"));
                ui.separator();
                ui.label(format!("{from_label} -> {to_label}: {ab_count}"));
                ui.label(format!("{to_label} -> {from_label}: {ba_count}"));
                ui.label(format!("Total traversals: {total}"));
                ui.label(format!("Latest traversal: {latest_text}"));
            });
        });
}

fn draw_highlighted_edge_overlay(ui: &mut Ui, app: &GraphBrowserApp, widget_id: egui::Id) {
    let Some((from, to)) = app.workspace.highlighted_graph_edge else {
        return;
    };
    let Some(state) = app.workspace.egui_state.as_ref() else {
        return;
    };
    let Some(from_node) = state.graph.node(from) else {
        return;
    };
    let Some(to_node) = state.graph.node(to) else {
        return;
    };
    let meta_id = widget_id.with("metadata");
    let (from_screen, to_screen) = if let Some(meta) = ui
        .ctx()
        .data_mut(|d| d.get_persisted::<MetadataFrame>(meta_id))
    {
        (
            meta.canvas_to_screen_pos(from_node.location()),
            meta.canvas_to_screen_pos(to_node.location()),
        )
    } else {
        (from_node.location(), to_node.location())
    };
    ui.painter().line_segment(
        [from_screen, to_screen],
        Stroke::new(6.0, Color32::from_rgba_unmultiplied(10, 30, 40, 120)),
    );
    ui.painter().line_segment(
        [from_screen, to_screen],
        Stroke::new(5.0, Color32::from_rgb(80, 220, 255)),
    );
    // Draw endpoint markers so edge-search selection is obvious even on dense graphs.
    ui.painter()
        .circle_filled(from_screen, 6.0, Color32::from_rgb(80, 220, 255));
    ui.painter()
        .circle_filled(to_screen, 6.0, Color32::from_rgb(80, 220, 255));
}

fn draw_hovered_node_tooltip(ui: &Ui, app: &GraphBrowserApp, widget_id: egui::Id) {
    let Some(key) = app.workspace.hovered_graph_node else {
        return;
    };
    let Some(node) = app.workspace.graph.get_node(key) else {
        return;
    };
    let pointer_pos = ui.input(|i| i.pointer.latest_pos());

    let lifecycle_text = if app.is_crash_blocked(key) {
        "Crashed".to_string()
    } else {
        match node.lifecycle {
            NodeLifecycle::Active => "Active".to_string(),
            NodeLifecycle::Warm => "Warm".to_string(),
            NodeLifecycle::Cold => "Cold".to_string(),
        }
    };
    let last_visited_text = format_last_visited(node.last_visited);
    let workspace_memberships: Vec<String> =
        app.membership_for_node(node.id).iter().cloned().collect();
    let anchor = pointer_pos
        .or_else(|| {
            app.workspace.egui_state.as_ref().and_then(|state| {
                state.graph.node(key).map(|n| {
                    let meta_id = widget_id.with("metadata");
                    if let Some(meta) = ui
                        .ctx()
                        .data_mut(|d| d.get_persisted::<MetadataFrame>(meta_id))
                    {
                        meta.canvas_to_screen_pos(n.location())
                    } else {
                        n.location()
                    }
                })
            })
        })
        .unwrap_or_else(|| ui.max_rect().center());

    egui::Area::new(widget_id.with("node_hover_tooltip"))
        .order(egui::Order::Tooltip)
        .fixed_pos(anchor + egui::vec2(14.0, 14.0))
        .interactable(false)
        .show(ui.ctx(), |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_min_width(240.0);
                ui.strong(if node.title.is_empty() {
                    &node.url
                } else {
                    &node.title
                });
                if !node.title.is_empty() && node.title != node.url {
                    ui.label(&node.url);
                }
                ui.small(format!("Last visited: {last_visited_text}"));
                ui.small(format!("Lifecycle: {lifecycle_text}"));
                if !workspace_memberships.is_empty() {
                    ui.separator();
                    ui.small(format!("Workspaces ({})", workspace_memberships.len()));
                    for workspace in &workspace_memberships {
                        ui.small(format!("- {workspace}"));
                    }
                }
            });
        });
}

fn format_last_visited(last_visited: SystemTime) -> String {
    let now = SystemTime::now();
    format_last_visited_with_now(last_visited, now)
}

fn format_last_visited_with_now(last_visited: SystemTime, now: SystemTime) -> String {
    let Ok(elapsed) = now.duration_since(last_visited) else {
        return "just now".to_string();
    };
    format_elapsed_ago(elapsed)
}

fn format_elapsed_ago(elapsed: Duration) -> String {
    let secs = elapsed.as_secs();
    if secs < 5 {
        return "just now".to_string();
    }
    if secs < 60 {
        return format!("{secs}s ago");
    }
    if secs < 60 * 60 {
        return format!("{}m ago", secs / 60);
    }
    if secs < 60 * 60 * 24 {
        return format!("{}h ago", secs / (60 * 60));
    }
    if secs < 60 * 60 * 24 * 7 {
        return format!("{}d ago", secs / (60 * 60 * 24));
    }
    format!("{}w ago", secs / (60 * 60 * 24 * 7))
}

fn filtered_graph_for_search(
    app: &GraphBrowserApp,
    search_matches: &HashSet<NodeKey>,
) -> crate::graph::Graph {
    let mut filtered = app.workspace.graph.clone();
    let to_remove: Vec<NodeKey> = filtered
        .nodes()
        .map(|(key, _)| key)
        .filter(|key| !search_matches.contains(key))
        .collect();
    for key in to_remove {
        filtered.remove_node(key);
    }
    filtered
}

#[derive(Debug, Clone)]
struct ViewportCullingSelection {
    visible: HashSet<NodeKey>,
    extended: HashSet<NodeKey>,
}

#[derive(Debug, Clone, Copy)]
struct ViewportCullingMetrics {
    total_nodes: usize,
    visible_nodes: usize,
    submitted_nodes: usize,
    removed_nodes: usize,
    full_submission_units: usize,
    culled_submission_units: usize,
}

fn estimated_submission_units(graph: &crate::graph::Graph) -> usize {
    graph.node_count() + graph.edge_count()
}

fn viewport_culling_selection_for_canvas_rect(
    graph: &crate::graph::Graph,
    canvas_rect: egui::Rect,
) -> Option<ViewportCullingSelection> {
    const DEFAULT_NODE_RADIUS: f32 = 5.0;
    let index = NodeSpatialIndex::build(graph.nodes().map(|(key, node)| {
        let pos = egui::Pos2::new(node.position.x, node.position.y);
        (key, pos, DEFAULT_NODE_RADIUS)
    }));

    let visible: HashSet<NodeKey> = index
        .nodes_in_canvas_rect(canvas_rect)
        .into_iter()
        .collect();
    if visible.is_empty() || visible.len() >= graph.node_count() {
        return None;
    }

    let mut extended = visible.clone();
    for edge in graph.edges() {
        if visible.contains(&edge.from) || visible.contains(&edge.to) {
            extended.insert(edge.from);
            extended.insert(edge.to);
        }
    }

    if extended.len() >= graph.node_count() {
        return None;
    }

    Some(ViewportCullingSelection { visible, extended })
}

fn viewport_culling_metrics_for_canvas_rect(
    graph: &crate::graph::Graph,
    canvas_rect: egui::Rect,
) -> Option<ViewportCullingMetrics> {
    let selection = viewport_culling_selection_for_canvas_rect(graph, canvas_rect)?;
    let culled_edge_count = graph
        .edges()
        .filter(|edge| {
            selection.extended.contains(&edge.from) && selection.extended.contains(&edge.to)
        })
        .count();

    Some(ViewportCullingMetrics {
        total_nodes: graph.node_count(),
        visible_nodes: selection.visible.len(),
        submitted_nodes: selection.extended.len(),
        removed_nodes: graph.node_count().saturating_sub(selection.extended.len()),
        full_submission_units: estimated_submission_units(graph),
        culled_submission_units: selection.extended.len() + culled_edge_count,
    })
}

fn viewport_culled_graph_for_canvas_rect(
    graph: &crate::graph::Graph,
    canvas_rect: egui::Rect,
) -> Option<crate::graph::Graph> {
    let selection = viewport_culling_selection_for_canvas_rect(graph, canvas_rect)?;

    let mut culled = graph.clone();
    let to_remove: Vec<NodeKey> = culled
        .nodes()
        .map(|(key, _)| key)
        .filter(|key| !selection.extended.contains(key))
        .collect();
    for key in to_remove {
        culled.remove_node(key);
    }

    Some(culled)
}

/// Compute a viewport-culled graph containing only the nodes visible in the
/// current frame's viewport (plus any additional nodes needed to keep edges
/// intact).  Returns `None` when culling is not applicable — e.g. when there
/// is no previous-frame camera metadata yet, when the graph is small enough
/// that culling has no effect, or when the canvas rect cannot be computed.
///
/// **Edge ghost-endpoint policy**: If either endpoint of an edge is visible,
/// both endpoints are included in the culled graph so that the renderer
/// never sees an edge with a missing endpoint.
fn viewport_culled_graph(
    ui: &Ui,
    app: &GraphBrowserApp,
    view_id: crate::app::GraphViewId,
) -> Option<crate::graph::Graph> {
    let frame = app.workspace.graph_view_frames.get(&view_id)?;
    let canvas_rect = canvas_rect_from_view_frame(ui.max_rect(), *frame)?;

    viewport_culled_graph_for_canvas_rect(&app.workspace.graph, canvas_rect)
}

fn canvas_rect_from_view_frame(
    screen_rect: egui::Rect,
    frame: crate::app::GraphViewFrame,
) -> Option<egui::Rect> {
    if frame.zoom.abs() < f32::EPSILON {
        return None;
    }

    let pan = egui::vec2(frame.pan_x, frame.pan_y);
    let canvas_min = (screen_rect.min.to_vec2() - pan) / frame.zoom;
    let canvas_max = (screen_rect.max.to_vec2() - pan) / frame.zoom;
    let canvas_rect = egui::Rect::from_min_max(canvas_min.to_pos2(), canvas_max.to_pos2());
    Some(canvas_rect)
}

fn lifecycle_color(lifecycle: NodeLifecycle) -> Color32 {
    match lifecycle {
        NodeLifecycle::Active => Color32::from_rgb(100, 200, 255),
        NodeLifecycle::Warm => Color32::from_rgb(120, 170, 205),
        NodeLifecycle::Cold => Color32::from_rgb(140, 140, 165),
    }
}

fn apply_search_node_visuals(
    app: &mut GraphBrowserApp,
    search_matches: &HashSet<NodeKey>,
    active_search_match: Option<NodeKey>,
    search_query_active: bool,
) {
    let hovered = app.workspace.hovered_graph_node;
    let highlighted_edge = app.workspace.highlighted_graph_edge;
    let search_mode = app.workspace.search_display_mode;
    let adjacency_set = hovered_adjacency_set(app, hovered);
    let colors: Vec<(NodeKey, Color32)> = app
        .workspace
        .graph
        .nodes()
        .map(|(key, node)| {
            let mut color = lifecycle_color(node.lifecycle);
            if app.is_crash_blocked(key) {
                color = Color32::from_rgb(205, 112, 82);
            }

            let search_match = search_query_active && search_matches.contains(&key);
            let search_miss = search_query_active && !search_matches.contains(&key);
            if search_match {
                color = if active_search_match == Some(key) {
                    Color32::from_rgb(140, 255, 140)
                } else {
                    Color32::from_rgb(95, 220, 130)
                };
            } else if search_miss && matches!(search_mode, SearchDisplayMode::Highlight) {
                color = color.gamma_multiply(0.45);
            }

            if hovered.is_some() && !adjacency_set.contains(&key) {
                color = color.gamma_multiply(0.4);
            }
            if hovered == Some(key) {
                // Visual cue for command-target disambiguation while hovering.
                color = Color32::from_rgb(255, 150, 80);
            }
            if let Some((from, to)) = highlighted_edge
                && (key == from || key == to)
            {
                color = Color32::from_rgb(80, 220, 255);
            }
            if app.workspace.selected_nodes.primary() == Some(key) {
                color = Color32::from_rgb(255, 200, 100);
            } else if app.workspace.selected_nodes.contains(&key) && hovered != Some(key) {
                color = if app.is_crash_blocked(key) {
                    Color32::from_rgb(205, 112, 82)
                } else {
                    lifecycle_color(node.lifecycle)
                };
            }
            (key, color)
        })
        .collect();

    let Some(state) = app.workspace.egui_state.as_mut() else {
        return;
    };
    for (key, color) in colors {
        if let Some(node) = state.graph.node_mut(key) {
            node.set_color(color);
        }
    }

    let edge_dimming: Vec<_> = state
        .graph
        .edges_iter()
        .map(|(edge_key, _)| {
            let mut dim = false;
            if hovered.is_some()
                && let Some((from, to)) = state.graph.edge_endpoints(edge_key)
            {
                dim = !adjacency_set.contains(&from) && !adjacency_set.contains(&to);
            }
            if search_query_active
                && matches!(search_mode, SearchDisplayMode::Highlight)
                && let Some((from, to)) = state.graph.edge_endpoints(edge_key)
                && (!search_matches.contains(&from) || !search_matches.contains(&to))
            {
                dim = true;
            }
            (edge_key, dim)
        })
        .collect();
    for (edge_key, dim) in edge_dimming {
        if let Some(edge) = state.graph.edge_mut(edge_key) {
            edge.display_mut().set_dimmed(dim);
        }
    }
}

fn hovered_adjacency_set(app: &GraphBrowserApp, hovered: Option<NodeKey>) -> HashSet<NodeKey> {
    hovered
        .map(|hover_key| {
            app.workspace
                .graph
                .out_neighbors(hover_key)
                .chain(app.workspace.graph.in_neighbors(hover_key))
                .chain(std::iter::once(hover_key))
                .collect()
        })
        .unwrap_or_default()
}

/// Handle custom navigation (Zoom/Pan/Fit) by manipulating MetadataFrame directly.
/// This bypasses egui_graphs built-in navigation to support custom bindings.
fn handle_custom_navigation(
    ui: &Ui,
    response: &egui::Response,
    metadata_id: egui::Id,
    app: &mut GraphBrowserApp,
    enabled: bool,
    view_id: crate::app::GraphViewId,
    canvas_profile: &crate::registries::domain::layout::canvas::CanvasSurfaceProfile,
    radial_open: bool,
    right_button_down: bool,
) -> Option<f32> {
    if !enabled {
        return None;
    }

    // Apply pending durable camera command.
    let camera_zoom = apply_pending_camera_command(ui, app, metadata_id, view_id, canvas_profile);

    // Apply keyboard zoom for this pane when a pending request explicitly targets it.
    let keyboard_zoom = apply_pending_keyboard_zoom_request(
        ui,
        app,
        metadata_id,
        view_id,
        canvas_profile.navigation.keyboard_zoom_step,
    );

    // Apply pre-intercepted wheel zoom delta.
    let wheel_zoom = apply_pending_wheel_zoom(
        ui,
        response,
        metadata_id,
        app,
        view_id,
        &canvas_profile.navigation,
    );

    let pointer_inside = response.contains_pointer() || response.dragged();
    let (primary_down, shift_down) = ui.input(|i| (i.pointer.primary_down(), i.modifiers.shift));
    let lasso_primary_drag_active =
        matches!(canvas_profile.interaction.lasso_binding, CanvasLassoBinding::ShiftLeftDrag)
            && shift_down;

    // Pan with Left Mouse Button on background
    // Note: We check if we are NOT hovering a node to allow node dragging.
    // app.workspace.hovered_graph_node is updated before this function in render_graph_in_ui_collect_actions.
    if canvas_profile.allows_background_pan(
        app.workspace.hovered_graph_node.is_none(),
        pointer_inside,
        primary_down,
        lasso_primary_drag_active,
        radial_open,
        right_button_down,
    ) {
        let delta = ui.input(|i| i.pointer.delta());
        apply_background_pan(ui.ctx(), metadata_id, app, view_id, delta);
    }

    // Clamp zoom bounds using per-view camera if available, else global camera.
    let zoom_min = app
        .workspace
        .views
        .get(&view_id)
        .map(|v| v.camera.zoom_min)
        .unwrap_or(app.workspace.camera.zoom_min);
    let zoom_max = app
        .workspace
        .views
        .get(&view_id)
        .map(|v| v.camera.zoom_max)
        .unwrap_or(app.workspace.camera.zoom_max);
    ui.ctx().data_mut(|data| {
        if let Some(mut meta) = data.get_persisted::<MetadataFrame>(metadata_id) {
            let clamped = meta.zoom.clamp(zoom_min, zoom_max);
            if (meta.zoom - clamped).abs() > f32::EPSILON {
                meta.zoom = clamped;
            }
            let current_zoom = meta.zoom;
            data.insert_persisted(metadata_id, meta);
            // Keep per-view current_zoom in sync.
            if let Some(view) = app.workspace.views.get_mut(&view_id) {
                view.camera.current_zoom = current_zoom;
            }
        }
    });

    camera_zoom.or(keyboard_zoom).or(wheel_zoom)
}

fn apply_background_pan(
    ctx: &egui::Context,
    metadata_id: egui::Id,
    app: &mut GraphBrowserApp,
    view_id: crate::app::GraphViewId,
    delta: Vec2,
) -> bool {
    if delta == Vec2::ZERO {
        return false;
    }

    app.workspace.focused_view = Some(view_id);
    let mut applied = false;
    ctx.data_mut(|data| {
        if let Some(mut meta) = data.get_persisted::<MetadataFrame>(metadata_id) {
            meta.pan += delta;
            data.insert_persisted(metadata_id, meta);
            applied = true;
        }
    });
    applied
}

fn apply_pending_keyboard_zoom_request(
    ui: &Ui,
    app: &mut GraphBrowserApp,
    metadata_id: egui::Id,
    view_id: crate::app::GraphViewId,
    keyboard_zoom_step: f32,
) -> Option<f32> {
    let Some(request) = app.take_pending_keyboard_zoom_request(view_id) else {
        return None;
    };

    let step = keyboard_zoom_step.max(1.01);
    let factor = match request {
        KeyboardZoomRequest::In => step,
        KeyboardZoomRequest::Out => 1.0 / step,
        KeyboardZoomRequest::Reset => 1.0,
    };

    let zoom_min = app
        .workspace
        .views
        .get(&view_id)
        .map(|v| v.camera.zoom_min)
        .unwrap_or(app.workspace.camera.zoom_min);
    let zoom_max = app
        .workspace
        .views
        .get(&view_id)
        .map(|v| v.camera.zoom_max)
        .unwrap_or(app.workspace.camera.zoom_max);

    let graph_rect = ui.max_rect();
    let local_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, graph_rect.size());
    let local_center = local_rect.center().to_vec2();
    let mut updated_zoom = None;
    let mut missing_metadata = false;

    ui.ctx().data_mut(|data| {
        if let Some(mut meta) = data.get_persisted::<MetadataFrame>(metadata_id) {
            let graph_center_pos = (local_center - meta.pan) / meta.zoom;
            let target = if matches!(request, KeyboardZoomRequest::Reset) {
                factor
            } else {
                meta.zoom * factor
            };
            let new_zoom = target.clamp(zoom_min, zoom_max);
            let pan_delta = graph_center_pos * meta.zoom - graph_center_pos * new_zoom;
            meta.pan += pan_delta;
            meta.zoom = new_zoom;
            data.insert_persisted(metadata_id, meta);
            updated_zoom = Some(new_zoom);
        } else {
            missing_metadata = true;
        }
    });

    if missing_metadata {
        emit_event(DiagnosticEvent::MessageReceived {
            channel_id: "runtime.ui.graph.keyboard_zoom_blocked_no_metadata",
            latency_us: 0,
        });
        app.restore_pending_keyboard_zoom_request(view_id, request);
    }

    // Keep zoom in sync on the appropriate camera.
    if let Some(new_zoom) = updated_zoom {
        if let Some(view) = app.workspace.views.get_mut(&view_id) {
            view.camera.current_zoom = new_zoom;
        }
    }

    updated_zoom
}

fn apply_pending_camera_command(
    ui: &Ui,
    app: &mut GraphBrowserApp,
    metadata_id: egui::Id,
    view_id: crate::app::GraphViewId,
    canvas_profile: &crate::registries::domain::layout::canvas::CanvasSurfaceProfile,
) -> Option<f32> {
    let Some(command) = app.pending_camera_command() else {
        return None;
    };
    if let Some(target_view) = app.pending_camera_command_target()
        && target_view != view_id
    {
        return None;
    }

    let zoom_min = app
        .workspace
        .views
        .get(&view_id)
        .map(|v| v.camera.zoom_min)
        .unwrap_or(app.workspace.camera.zoom_min);
    let zoom_max = app
        .workspace
        .views
        .get(&view_id)
        .map(|v| v.camera.zoom_max)
        .unwrap_or(app.workspace.camera.zoom_max);

    match command {
        CameraCommand::SetZoom(target_zoom) => {
            let mut updated_zoom = None;
            let mut missing_metadata = false;
            ui.ctx().data_mut(|data| {
                if let Some(mut meta) = data.get_persisted::<MetadataFrame>(metadata_id) {
                    let new_zoom = target_zoom.clamp(zoom_min, zoom_max);
                    meta.zoom = new_zoom;
                    data.insert_persisted(metadata_id, meta);
                    updated_zoom = Some(new_zoom);
                } else {
                    missing_metadata = true;
                }
            });
            if missing_metadata {
                emit_event(DiagnosticEvent::MessageReceived {
                    channel_id: "runtime.ui.graph.camera_zoom_deferred_no_metadata",
                    latency_us: 0,
                });
            }
            if let Some(new_zoom) = updated_zoom {
                if let Some(view) = app.workspace.views.get_mut(&view_id) {
                    view.camera.current_zoom = new_zoom;
                }
                app.clear_pending_camera_command();
            }
            updated_zoom
        }
        CameraCommand::Fit | CameraCommand::StartupFit | CameraCommand::FitSelection => {
            let graph_rect = ui.max_rect();
            let view_size = graph_rect.size();
            if view_size.x <= f32::EPSILON || view_size.y <= f32::EPSILON {
                emit_event(DiagnosticEvent::MessageReceived {
                    channel_id: "runtime.ui.graph.camera_fit_blocked_zero_view",
                    latency_us: 0,
                });
                return None;
            }

            let bounds = if matches!(command, CameraCommand::FitSelection) {
                node_bounds_for_selection(app)
            } else {
                node_bounds_for_all(app)
            };

            let Some((min_x, max_x, min_y, max_y)) = bounds else {
                if matches!(command, CameraCommand::FitSelection) {
                    emit_event(DiagnosticEvent::MessageReceived {
                        channel_id: "runtime.ui.graph.fit_selection_fallback_to_fit",
                        latency_us: 0,
                    });
                    app.request_camera_command_for_view(Some(view_id), CameraCommand::Fit);
                } else {
                    emit_event(DiagnosticEvent::MessageReceived {
                        channel_id: "runtime.ui.graph.camera_fit_blocked_no_bounds",
                        latency_us: 0,
                    });
                    app.clear_pending_camera_command();
                }
                return None;
            };

            let width = (max_x - min_x).abs().max(1.0);
            let height = (max_y - min_y).abs().max(1.0);
            let padding = if matches!(command, CameraCommand::FitSelection) {
                canvas_profile.navigation.camera_focus_selection_padding
            } else {
                canvas_profile.navigation.camera_fit_padding
            };
            let padded_width = width * padding;
            let padded_height = height * padding;
            let fit_zoom = (view_size.x / padded_width).min(view_size.y / padded_height);
            let raw_target = if matches!(command, CameraCommand::FitSelection) {
                fit_zoom
            } else {
                fit_zoom * canvas_profile.navigation.camera_fit_relax
            };
            let target_zoom = raw_target.clamp(zoom_min, zoom_max);

            let center = egui::pos2((min_x + max_x) * 0.5, (min_y + max_y) * 0.5);
            let viewport_center = egui::Rect::from_min_size(egui::Pos2::ZERO, graph_rect.size())
                .center()
                .to_vec2();
            let target_pan = viewport_center - center.to_vec2() * target_zoom;

            let mut updated_zoom = None;
            let mut missing_metadata = false;
            ui.ctx().data_mut(|data| {
                if let Some(mut meta) = data.get_persisted::<MetadataFrame>(metadata_id) {
                    meta.zoom = target_zoom;
                    meta.pan = target_pan;
                    data.insert_persisted(metadata_id, meta);
                    updated_zoom = Some(target_zoom);
                } else {
                    missing_metadata = true;
                }
            });

            if missing_metadata {
                emit_event(DiagnosticEvent::MessageReceived {
                    channel_id: "runtime.ui.graph.camera_fit_deferred_no_metadata",
                    latency_us: 0,
                });
            }

            if let Some(new_zoom) = updated_zoom {
                if let Some(view) = app.workspace.views.get_mut(&view_id) {
                    view.camera.current_zoom = new_zoom;
                }
                app.clear_pending_camera_command();
            }
            updated_zoom
        }
    }
}

fn apply_pending_wheel_zoom(
    ui: &Ui,
    response: &egui::Response,
    metadata_id: egui::Id,
    app: &mut GraphBrowserApp,
    view_id: crate::app::GraphViewId,
    navigation_policy: &crate::registries::domain::layout::canvas::CanvasNavigationPolicy,
) -> Option<f32> {
    let scroll_delta = app.pending_wheel_zoom_delta(view_id);
    if scroll_delta.abs() <= f32::EPSILON {
        return None;
    }

    let zoom_min = app
        .workspace
        .views
        .get(&view_id)
        .map(|v| v.camera.zoom_min)
        .unwrap_or(app.workspace.camera.zoom_min);
    let zoom_max = app
        .workspace
        .views
        .get(&view_id)
        .map(|v| v.camera.zoom_max)
        .unwrap_or(app.workspace.camera.zoom_max);

    let velocity_id = metadata_id.with("scroll_zoom_velocity");
    let mut velocity = ui
        .ctx()
        .data_mut(|d| d.get_persisted::<f32>(velocity_id))
        .unwrap_or(0.0);

    let impulse = navigation_policy.wheel_zoom_impulse_scale * (scroll_delta / 60.0).clamp(-1.0, 1.0);
    velocity += impulse;

    let mut updated_zoom = None;
    if velocity.abs() >= navigation_policy.wheel_zoom_inertia_min_abs {
        let factor = 1.0 + velocity;
        if factor > 0.0 {
            let graph_rect = response.rect;
            let local_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, graph_rect.size());
            let pointer_pos = ui.input(|i| i.pointer.latest_pos());
            let local_center = pointer_pos
                .map(|p| egui::pos2(p.x - graph_rect.min.x, p.y - graph_rect.min.y))
                .unwrap_or(local_rect.center())
                .to_vec2();

            ui.ctx().data_mut(|data| {
                if let Some(mut meta) = data.get_persisted::<MetadataFrame>(metadata_id) {
                    let graph_center_pos = (local_center - meta.pan) / meta.zoom;
                    let new_zoom = (meta.zoom * factor).clamp(zoom_min, zoom_max);
                    let pan_delta = graph_center_pos * meta.zoom - graph_center_pos * new_zoom;
                    meta.pan += pan_delta;
                    meta.zoom = new_zoom;
                    data.insert_persisted(metadata_id, meta);
                    updated_zoom = Some(new_zoom);
                }
            });
            if let Some(new_zoom) = updated_zoom
                && let Some(view) = app.workspace.views.get_mut(&view_id)
            {
                view.camera.current_zoom = new_zoom;
            }
        }
    }

    if updated_zoom.is_some() {
        app.clear_pending_wheel_zoom_delta();
    }

    velocity *= navigation_policy.wheel_zoom_inertia_damping;
    if velocity.abs() < navigation_policy.wheel_zoom_inertia_min_abs {
        velocity = 0.0;
    }
    ui.ctx()
        .data_mut(|d| d.insert_persisted(velocity_id, velocity));
    if velocity != 0.0 {
        ui.ctx().request_repaint_after(Duration::from_millis(16));
    }

    updated_zoom
}

fn node_bounds_for_all(app: &GraphBrowserApp) -> Option<(f32, f32, f32, f32)> {
    let state = app.workspace.egui_state.as_ref()?;
    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    let mut has_nodes = false;

    for (_, node) in state.graph.nodes_iter() {
        let pos = node.location();
        min_x = min_x.min(pos.x);
        max_x = max_x.max(pos.x);
        min_y = min_y.min(pos.y);
        max_y = max_y.max(pos.y);
        has_nodes = true;
    }

    if !has_nodes
        || !min_x.is_finite()
        || !max_x.is_finite()
        || !min_y.is_finite()
        || !max_y.is_finite()
    {
        return None;
    }

    Some((min_x, max_x, min_y, max_y))
}

fn node_bounds_for_selection(app: &GraphBrowserApp) -> Option<(f32, f32, f32, f32)> {
    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_y = f32::NEG_INFINITY;

    for key in app.workspace.selected_nodes.iter().copied() {
        if let Some(node) = app.workspace.graph.get_node(key) {
            min_x = min_x.min(node.position.x);
            max_x = max_x.max(node.position.x);
            min_y = min_y.min(node.position.y);
            max_y = max_y.max(node.position.y);
        }
    }

    if !min_x.is_finite() || !max_x.is_finite() || !min_y.is_finite() || !max_y.is_finite() {
        return None;
    }

    Some((min_x, max_x, min_y, max_y))
}

struct LassoGestureResult {
    action: Option<GraphAction>,
    suppress_context_menu: bool,
}

fn collect_lasso_action(
    ui: &Ui,
    app: &GraphBrowserApp,
    enabled: bool,
    metadata_id: egui::Id,
    lasso_binding: CanvasLassoBinding,
) -> LassoGestureResult {
    let start_id = metadata_id.with("lasso_start_screen");
    let moved_id = metadata_id.with("lasso_moved");
    let threshold_px = 6.0_f32;
    if !enabled {
        ui.ctx().data_mut(|d| {
            d.remove::<egui::Pos2>(start_id);
            d.remove::<bool>(moved_id);
        });
        return LassoGestureResult {
            action: None,
            suppress_context_menu: false,
        };
    }

    let graph_rect = ui.max_rect();
    let (pointer_pos, pressed, down, released, ctrl, shift, alt) = ui.input(|i| {
        let (pressed, down, released) = match lasso_binding {
            CanvasLassoBinding::RightDrag => (
                i.pointer.secondary_pressed(),
                i.pointer.secondary_down(),
                i.pointer.secondary_released(),
            ),
            CanvasLassoBinding::ShiftLeftDrag => (
                i.pointer.primary_pressed() && i.modifiers.shift,
                i.pointer.primary_down() && i.modifiers.shift,
                i.pointer.primary_released(),
            ),
        };
        (
            i.pointer.latest_pos(),
            pressed,
            down,
            released,
            i.modifiers.ctrl,
            i.modifiers.shift,
            i.modifiers.alt,
        )
    });

    if pressed
        && let Some(pos) = pointer_pos
        && graph_rect.contains(pos)
    {
        ui.ctx().data_mut(|d| {
            d.insert_persisted(start_id, pos);
            d.insert_persisted(moved_id, false);
        });
    }

    let start = ui
        .ctx()
        .data_mut(|d| d.get_persisted::<egui::Pos2>(start_id));
    let mut moved = ui
        .ctx()
        .data_mut(|d| d.get_persisted::<bool>(moved_id))
        .unwrap_or(false);
    if down && let (Some(a), Some(b)) = (start, pointer_pos) {
        if !moved && (b - a).length() >= threshold_px {
            moved = true;
            ui.ctx().data_mut(|d| d.insert_persisted(moved_id, true));
        }
        if moved {
            let rect = egui::Rect::from_two_pos(a, b);
            ui.painter().rect_stroke(
                rect,
                0.0,
                Stroke::new(1.5, Color32::from_rgb(90, 220, 170)),
                egui::epaint::StrokeKind::Outside,
            );
            ui.painter()
                .rect_filled(rect, 0.0, Color32::from_rgba_unmultiplied(90, 220, 170, 28));
        }
    }

    if !released {
        return LassoGestureResult {
            action: None,
            suppress_context_menu: false,
        };
    }
    ui.ctx().data_mut(|d| {
        d.remove::<egui::Pos2>(start_id);
        d.remove::<bool>(moved_id);
    });

    let (Some(a), Some(b)) = (start, pointer_pos) else {
        return LassoGestureResult {
            action: None,
            suppress_context_menu: false,
        };
    };
    if !moved {
        return LassoGestureResult {
            action: None,
            suppress_context_menu: false,
        };
    }

    let rect = egui::Rect::from_two_pos(a, b);
    let add_mode = ctrl || (matches!(lasso_binding, CanvasLassoBinding::RightDrag) && shift);
    let mode = if alt {
        SelectionUpdateMode::Toggle
    } else if add_mode {
        SelectionUpdateMode::Add
    } else {
        SelectionUpdateMode::Replace
    };
    let meta = ui
        .ctx()
        .data_mut(|d| d.get_persisted::<MetadataFrame>(metadata_id))
        .unwrap_or_default();
    let Some(state) = app.workspace.egui_state.as_ref() else {
        emit_event(DiagnosticEvent::MessageReceived {
            channel_id: "runtime.ui.graph.lasso_blocked_no_state",
            latency_us: 0,
        });
        return LassoGestureResult {
            action: None,
            suppress_context_menu: matches!(lasso_binding, CanvasLassoBinding::RightDrag),
        };
    };
    // Build an R*-tree index in canvas (world) space and query with the
    // lasso rect inverted from screen space.  This avoids a full O(n) scan
    // and keeps hit-testing in a stable coordinate system.
    let index = NodeSpatialIndex::build(
        state
            .graph
            .nodes_iter()
            .map(|(key, node)| (key, node.location(), node.display().radius())),
    );
    let canvas_min = meta.screen_to_canvas_pos(rect.min);
    let canvas_max = meta.screen_to_canvas_pos(rect.max);
    let canvas_rect = egui::Rect::from_min_max(canvas_min, canvas_max);
    let keys = index.nodes_with_center_in_canvas_rect(canvas_rect);

    LassoGestureResult {
        action: Some(GraphAction::LassoSelect { keys, mode }),
        suppress_context_menu: matches!(lasso_binding, CanvasLassoBinding::RightDrag) && moved,
    }
}

/// Convert egui_graphs events to resolved GraphActions and apply them.
fn collect_graph_actions(
    app: &GraphBrowserApp,
    events: &Rc<RefCell<Vec<Event>>>,
    split_open_modifier: bool,
    multi_select_modifier: bool,
) -> Vec<GraphAction> {
    let mut actions = Vec::new();

    for event in events.borrow_mut().drain(..) {
        match event {
            Event::NodeDoubleClick(p) => {
                if let Some(state) = app.workspace.egui_state.as_ref() {
                    let idx = NodeIndex::new(p.id);
                    if let Some(key) = state.get_key(idx) {
                        if split_open_modifier {
                            actions.push(GraphAction::FocusNodeSplit(key));
                        } else {
                            actions.push(GraphAction::FocusNode(key));
                        }
                    }
                } else {
                    emit_event(DiagnosticEvent::MessageReceived {
                        channel_id: "runtime.ui.graph.event_blocked_no_state",
                        latency_us: 0,
                    });
                }
            }
            Event::NodeDragStart(_) => {
                actions.push(GraphAction::DragStart);
            }
            Event::NodeDragEnd(p) => {
                // Resolve final position from egui_state
                let idx = NodeIndex::new(p.id);
                if let Some(state) = app.workspace.egui_state.as_ref() {
                    if let Some(key) = state.get_key(idx) {
                        let pos = state
                            .graph
                            .node(idx)
                            .map(|n| Point2D::new(n.location().x, n.location().y))
                            .unwrap_or_default();
                        actions.push(GraphAction::DragEnd(key, pos));
                    }
                } else {
                    emit_event(DiagnosticEvent::MessageReceived {
                        channel_id: "runtime.ui.graph.event_blocked_no_state",
                        latency_us: 0,
                    });
                }
            }
            Event::NodeMove(p) => {
                let idx = NodeIndex::new(p.id);
                if let Some(state) = app.workspace.egui_state.as_ref() {
                    if let Some(key) = state.get_key(idx) {
                        actions.push(GraphAction::MoveNode(
                            key,
                            Point2D::new(p.new_pos[0], p.new_pos[1]),
                        ));
                    }
                } else {
                    emit_event(DiagnosticEvent::MessageReceived {
                        channel_id: "runtime.ui.graph.event_blocked_no_state",
                        latency_us: 0,
                    });
                }
            }
            Event::NodeSelect(p) => {
                if let Some(state) = app.workspace.egui_state.as_ref() {
                    let idx = NodeIndex::new(p.id);
                    if let Some(key) = state.get_key(idx) {
                        actions.push(GraphAction::SelectNode {
                            key,
                            multi_select: multi_select_modifier,
                        });
                    }
                } else {
                    emit_event(DiagnosticEvent::MessageReceived {
                        channel_id: "runtime.ui.graph.event_blocked_no_state",
                        latency_us: 0,
                    });
                }
            }
            Event::NodeDeselect(p) => {
                // When Ctrl is held, a NodeDeselect means the user Ctrl+Clicked an
                // already-selected node to toggle it out of the multi-selection.
                if multi_select_modifier {
                    if let Some(state) = app.workspace.egui_state.as_ref() {
                        let idx = NodeIndex::new(p.id);
                        if let Some(key) = state.get_key(idx) {
                            actions.push(GraphAction::SelectNode {
                                key,
                                multi_select: true,
                            });
                        }
                    } else {
                        emit_event(DiagnosticEvent::MessageReceived {
                            channel_id: "runtime.ui.graph.event_blocked_no_state",
                            latency_us: 0,
                        });
                    }
                }
                // Without modifier: selection clearing is handled by the next SelectNode action.
            }
            Event::Zoom(p) => {
                actions.push(GraphAction::Zoom(p.new_zoom));
            }
            _ => {}
        }
    }

    actions
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
                intents.push(GraphIntent::SetNodePosition { key, position: pos });
            }
            GraphAction::MoveNode(key, pos) => {
                intents.push(GraphIntent::SetNodePosition { key, position: pos });
            }
            GraphAction::SelectNode { key, multi_select } => {
                intents.push(GraphIntent::SelectNode { key, multi_select });
            }
            GraphAction::LassoSelect { keys, mode } => {
                intents.push(GraphIntent::UpdateSelection { keys, mode });
            }
            GraphAction::ClearSelection => {
                intents.push(GraphIntent::UpdateSelection {
                    keys: Vec::new(),
                    mode: SelectionUpdateMode::Replace,
                });
            }
            GraphAction::Zoom(new_zoom) => {
                intents.push(GraphIntent::SetZoom { zoom: new_zoom });
            }
        }
    }
    intents
}

/// Sync node positions from egui_graphs layout state back into app graph state.
///
/// Pinned nodes keep their app-authored positions; their visual positions are
/// restored after layout so FR simulation does not move them.
///
/// **Group drag**: when the user is actively dragging (`is_interacting`) with
/// 2+ nodes selected, the dragged node's per-frame delta is detected by comparing
/// its egui_graphs position to its last-known `app.workspace.graph` position.  That same
/// delta is then applied to every other selected (non-pinned) node in both
/// `egui_state` and `app.workspace.graph`, keeping the group moving together without any
/// changes to `GraphAction` or `GraphIntent`.
pub(crate) fn sync_graph_positions_from_layout(app: &mut GraphBrowserApp) {
    let Some(state) = app.workspace.egui_state.as_ref() else {
        if app.workspace.is_interacting {
            emit_event(DiagnosticEvent::MessageReceived {
                channel_id: "runtime.ui.graph.layout_sync_blocked_no_state",
                latency_us: 0,
            });
        }
        return;
    };

    let layout_positions: Vec<(NodeKey, Point2D<f32>)> = app
        .workspace
        .graph
        .nodes()
        .filter_map(|(key, _)| {
            state
                .graph
                .node(key)
                .map(|n| (key, Point2D::new(n.location().x, n.location().y)))
        })
        .collect();

    // Detect group drag: during active interaction with 2+ selected nodes, find
    // the node whose egui_graphs position diverged from app.workspace.graph this frame.
    // This is the node the user is physically dragging.
    let group_drag_delta: Option<(NodeKey, egui::Vec2)> =
        if app.workspace.is_interacting && app.workspace.selected_nodes.len() > 1 {
            layout_positions.iter().find_map(|(key, egui_pos)| {
                if !app.workspace.selected_nodes.contains(key) {
                    return None;
                }
                let app_pos = app.workspace.graph.get_node(*key)?.position;
                let delta = egui::Vec2::new(egui_pos.x - app_pos.x, egui_pos.y - app_pos.y);
                // Only consider it a drag if it actually moved (filter float noise).
                if delta.length() > 0.01 {
                    Some((*key, delta))
                } else {
                    None
                }
            })
        } else {
            None
        };

    let mut pinned_positions = Vec::new();
    for (key, pos) in layout_positions {
        if let Some(node_mut) = app.workspace.graph.get_node_mut(key) {
            if node_mut.is_pinned {
                pinned_positions.push((key, node_mut.position));
            } else {
                node_mut.position = pos;
            }
        }
    }

    // Propagate the drag delta to secondary selected nodes.
    if let Some((dragged_key, delta)) = group_drag_delta {
        let secondary_keys: Vec<NodeKey> = app
            .workspace
            .selected_nodes
            .iter()
            .filter(|&&k| k != dragged_key)
            .copied()
            .collect();

        let mut secondary_updates: Vec<(NodeKey, egui::Pos2)> = Vec::new();
        for other_key in secondary_keys {
            if let Some(node) = app.workspace.graph.get_node_mut(other_key) {
                if !node.is_pinned {
                    node.position.x += delta.x;
                    node.position.y += delta.y;
                    secondary_updates
                        .push((other_key, egui::Pos2::new(node.position.x, node.position.y)));
                }
            }
        }
        if let Some(state_mut) = app.workspace.egui_state.as_mut() {
            for (key, pos) in secondary_updates {
                if let Some(egui_node) = state_mut.graph.node_mut(key) {
                    egui_node.set_location(pos);
                }
            }
        }
    }

    if let Some(state_mut) = app.workspace.egui_state.as_mut() {
        for (key, pos) in pinned_positions {
            if let Some(egui_node) = state_mut.graph.node_mut(key) {
                egui_node.set_location(egui::Pos2::new(pos.x, pos.y));
            }
        }
    }
}

/// Apply semantic clustering forces based on UDC tag similarity (Phase 2).
/// Nodes with similar UDC codes attract each other, creating "library shelves"
/// where content clusters by subject (math, physics, history, etc.).
///
/// Force model: F = k * similarity * (pos_b - pos_a)
/// where similarity = 1.0 - distance(code_a, code_b)
fn apply_semantic_clustering_forces(
    app: &mut GraphBrowserApp,
    semantic_config: Option<(bool, f32)>,
) {
    // Check if semantic clustering is enabled
    let (enabled, strength) = if let Some((enabled, strength)) = semantic_config {
        (enabled, strength)
    } else {
        // No lens active, semantic clustering is disabled by default
        // TODO: Add global semantic clustering setting when settings panel is implemented
        (false, 0.05)
    };

    if !enabled || strength < 1e-6 {
        return;
    }

    if !app.workspace.physics.base.is_running {
        return;
    }

    if app.workspace.semantic_index.is_empty() {
        return;
    }

    // Collect nodes with semantic tags
    let tagged_nodes: Vec<(
        crate::graph::NodeKey,
        crate::registries::atomic::knowledge::CompactCode,
    )> = app
        .workspace
        .semantic_index
        .iter()
        .map(|(&key, code)| (key, code.clone()))
        .collect();

    if tagged_nodes.len() < 2 {
        return; // Need at least 2 nodes to cluster
    }

    // Calculate pairwise semantic attractions
    // For efficiency, we use O(n²) for now; could optimize with clustering/sampling later
    let mut position_deltas: HashMap<crate::graph::NodeKey, egui::Vec2> = HashMap::new();

    for i in 0..tagged_nodes.len() {
        for j in (i + 1)..tagged_nodes.len() {
            let (key_a, code_a) = &tagged_nodes[i];
            let (key_b, code_b) = &tagged_nodes[j];

            // Calculate semantic distance (0.0 = identical, 1.0 = completely different)
            let distance = code_a.distance(code_b);
            let similarity = 1.0 - distance;

            // Skip weak similarities to reduce noise
            if similarity < 0.1 {
                continue;
            }

            // Get node positions
            let pos_a = app.workspace.graph.get_node(*key_a).map(|n| n.position);
            let pos_b = app.workspace.graph.get_node(*key_b).map(|n| n.position);

            if let (Some(pa), Some(pb)) = (pos_a, pos_b) {
                // Calculate attraction vector
                let delta = egui::Vec2::new(pb.x - pa.x, pb.y - pa.y);
                let force = delta * similarity * strength;

                // Apply force to both nodes (Newton's 3rd law)
                *position_deltas.entry(*key_a).or_insert(egui::Vec2::ZERO) += force;
                *position_deltas.entry(*key_b).or_insert(egui::Vec2::ZERO) -= force;
            }
        }
    }

    // Apply position deltas to app.workspace.graph and sync to egui_state
    for (key, delta) in &position_deltas {
        if let Some(node) = app.workspace.graph.get_node_mut(*key) {
            if !node.is_pinned {
                node.position.x += delta.x;
                node.position.y += delta.y;
            }
        }
    }

    // Sync updated positions back to egui_state
    if let Some(state_mut) = app.workspace.egui_state.as_mut() {
        for (key, _delta) in position_deltas {
            if let Some(node) = app.workspace.graph.get_node(key) {
                if !node.is_pinned {
                    if let Some(egui_node) = state_mut.graph.node_mut(key) {
                        egui_node.set_location(egui::Pos2::new(node.position.x, node.position.y));
                    }
                }
            }
        }
    }
}

/// Draw graph information overlay
fn draw_graph_info(ui: &mut egui::Ui, app: &GraphBrowserApp) {
    let info_text = format!(
        "Nodes: {} | Edges: {} | Physics: {} | Zoom: {:.1}x",
        app.workspace.graph.node_count(),
        app.workspace.graph.edge_count(),
        if app.workspace.physics.base.is_running {
            "Running"
        } else {
            "Paused"
        },
        app.workspace.camera.current_zoom
    );

    ui.painter().text(
        ui.available_rect_before_wrap().left_top() + Vec2::new(10.0, 10.0),
        egui::Align2::LEFT_TOP,
        info_text,
        egui::FontId::monospace(12.0),
        Color32::from_rgb(200, 200, 200),
    );

    // Draw controls hint
    let layout_domain = LayoutDomainRegistry::default();
    let layout_profile = layout_domain.resolve_profile(
        CANVAS_PROFILE_DEFAULT,
        WORKBENCH_SURFACE_DEFAULT,
        VIEWER_SURFACE_DEFAULT,
    );
    let lasso_hint =
        canvas_lasso_binding_label(layout_profile.canvas.profile.interaction.lasso_binding);
    let command_hint = match app.workspace.command_palette_shortcut {
        crate::app::CommandPaletteShortcut::F2 => "F2 Commands",
        crate::app::CommandPaletteShortcut::CtrlK => "Ctrl+K Commands",
    };
    let radial_hint = match app.workspace.radial_menu_shortcut {
        crate::app::RadialMenuShortcut::F3 => "F3 Radial",
        crate::app::RadialMenuShortcut::R => "R Radial",
    };
    let help_hint = match app.workspace.help_panel_shortcut {
        crate::app::HelpPanelShortcut::F1OrQuestion => "F1/? Help",
        crate::app::HelpPanelShortcut::H => "H Help",
    };
    let controls_text = format!(
        "Shortcuts: Ctrl+Click Multi-select | {lasso_hint} | Double-click Open | Drag tab out to split | N New Node | Del Remove | T Physics | R Reheat | +/-/0 Zoom | Z Smart Fit | L Toggle Pin | Ctrl+F Search | G Edge Ops | {command_hint} | {radial_hint} | Ctrl+Z/Y Undo/Redo | {help_hint}"
    );
    ui.painter().text(
        ui.available_rect_before_wrap().left_bottom() + Vec2::new(10.0, -10.0),
        egui::Align2::LEFT_BOTTOM,
        controls_text,
        egui::FontId::proportional(10.0),
        Color32::from_rgb(150, 150, 150),
    );
}

fn render_physics_settings_in_ui(ui: &mut Ui, app: &mut GraphBrowserApp) {
    let mut config = app.workspace.physics.clone();
    let mut config_changed = false;

    ui.label("Repulsion (c_repulse):");
    if ui
        .add(egui::Slider::new(&mut config.base.c_repulse, 0.0..=10.0))
        .changed()
    {
        config_changed = true;
    }

    ui.label("Attraction (c_attract):");
    if ui
        .add(egui::Slider::new(&mut config.base.c_attract, 0.0..=10.0))
        .changed()
    {
        config_changed = true;
    }

    ui.label("Ideal Distance Scale (k_scale):");
    if ui
        .add(egui::Slider::new(&mut config.base.k_scale, 0.1..=5.0))
        .changed()
    {
        config_changed = true;
    }

    ui.label("Center Gravity:");
    if ui
        .add(egui::Slider::new(&mut config.extras.0.params.c, 0.0..=1.0))
        .changed()
    {
        config_changed = true;
    }

    ui.label("Max Step:");
    if ui
        .add(egui::Slider::new(&mut config.base.max_step, 0.1..=100.0))
        .changed()
    {
        config_changed = true;
    }

    ui.separator();
    ui.label("Damping & Convergence");
    ui.label("Damping:");
    if ui
        .add(egui::Slider::new(&mut config.base.damping, 0.01..=1.0))
        .changed()
    {
        config_changed = true;
    }

    ui.label("Time Step (dt):");
    if ui
        .add(egui::Slider::new(&mut config.base.dt, 0.001..=1.0).logarithmic(true))
        .changed()
    {
        config_changed = true;
    }

    ui.label("Epsilon:");
    if ui
        .add(egui::Slider::new(&mut config.base.epsilon, 1e-6..=0.1).logarithmic(true))
        .changed()
    {
        config_changed = true;
    }

    ui.horizontal(|ui| {
        if ui.button("Reset to Defaults").clicked() {
            let running = config.base.is_running;
            config = GraphBrowserApp::default_physics_state();
            config.base.is_running = running;
            config_changed = true;
        }

        ui.small(if app.workspace.physics.base.is_running {
            "Status: Running"
        } else {
            "Status: Paused"
        });
    });

    if let Some(last_avg) = app.workspace.physics.base.last_avg_displacement {
        ui.small(format!("Last avg displacement: {:.4}", last_avg));
    }
    ui.small(format!("Step count: {}", app.workspace.physics.base.step_count));

    if config_changed {
        app.update_physics_config(config);
    }
}

/// Render keyboard shortcut help panel
pub fn render_help_panel(ctx: &egui::Context, app: &mut GraphBrowserApp) {
    if !app.workspace.show_help_panel {
        return;
    }

    let layout_domain = LayoutDomainRegistry::default();
    let layout_profile = layout_domain.resolve_profile(
        CANVAS_PROFILE_DEFAULT,
        WORKBENCH_SURFACE_DEFAULT,
        VIEWER_SURFACE_DEFAULT,
    );

    let mut open = app.workspace.show_help_panel;
    Window::new("Keyboard Shortcuts")
        .open(&mut open)
        .default_width(350.0)
        .resizable(false)
        .show(ctx, |ui| {
            egui::Grid::new("shortcut_grid")
                .num_columns(2)
                .spacing([20.0, 6.0])
                .show(ui, |ui| {
                    let lasso_binding = layout_profile.canvas.profile.interaction.lasso_binding;
                    let lasso_base = match lasso_binding {
                        CanvasLassoBinding::RightDrag => "Right+Drag",
                        CanvasLassoBinding::ShiftLeftDrag => "Shift+LeftDrag",
                    };
                    let lasso_add = match lasso_binding {
                        CanvasLassoBinding::RightDrag => "Right+Shift/Ctrl+Drag",
                        CanvasLassoBinding::ShiftLeftDrag => "Shift+Ctrl+LeftDrag",
                    };
                    let lasso_toggle = match lasso_binding {
                        CanvasLassoBinding::RightDrag => "Right+Alt+Drag",
                        CanvasLassoBinding::ShiftLeftDrag => "Shift+Alt+LeftDrag",
                    };
                    let command_palette_key = match app.workspace.command_palette_shortcut {
                        crate::app::CommandPaletteShortcut::F2 => "F2",
                        crate::app::CommandPaletteShortcut::CtrlK => "Ctrl+K",
                    };
                    let radial_key = match app.workspace.radial_menu_shortcut {
                        crate::app::RadialMenuShortcut::F3 => "F3",
                        crate::app::RadialMenuShortcut::R => "R",
                    };
                    let help_key = match app.workspace.help_panel_shortcut {
                        crate::app::HelpPanelShortcut::F1OrQuestion => "F1 / ?",
                        crate::app::HelpPanelShortcut::H => "H",
                    };
                    let shortcuts = [
                        ("Home / Esc", "Toggle Graph / Detail view"),
                        ("N", "Create new node"),
                        ("Delete", "Remove selected nodes"),
                        ("Ctrl+Shift+Delete", "Clear entire graph"),
                        ("T", "Toggle physics simulation"),
                        ("R", "Reheat physics simulation"),
                        ("+ / - / 0", "Zoom in / out / reset"),
                        (
                            "Z",
                            "Smart fit (2+ selected: fit selection; else fit graph)",
                        ),
                        ("P", "Physics settings panel"),
                        ("Ctrl+H", "History Manager panel"),
                        ("Ctrl+F", "Show graph search"),
                        (command_palette_key, "Toggle edge command palette"),
                        (radial_key, "Toggle radial command menu"),
                        ("Ctrl+Z / Ctrl+Y", "Undo / Redo"),
                        ("G", "Connect selected pair"),
                        ("Shift+G", "Connect both directions"),
                        ("Alt+G", "Remove user edge"),
                        ("I / U", "Pin / Unpin selected node(s)"),
                        ("L", "Toggle pin on primary selected node"),
                        (lasso_base, "Lasso select (replace)"),
                        (lasso_add, "Lasso add to selection"),
                        (lasso_toggle, "Lasso toggle selection"),
                        ("Search Up/Down", "Cycle graph matches"),
                        ("Search Enter", "Select active search match"),
                        (help_key, "This help panel"),
                        ("Ctrl+L / Alt+D", "Focus address bar"),
                        ("Double-click node", "Open node via workspace routing"),
                        ("Drag tab out", "Detach tab into split pane"),
                        ("Shift + Double-click node", "Fallback split-open gesture"),
                        ("Click + drag", "Move a node"),
                        ("Scroll wheel", "Zoom in / out"),
                    ];

                    for (key, desc) in shortcuts {
                        ui.strong(key);
                        ui.label(desc);
                        ui.end_row();
                    }
                });
        });
    app.workspace.show_help_panel = open;
}

/// Render History Manager panel with Timeline and Dissolved tabs.
pub fn render_history_manager_in_ui(
    ui: &mut Ui,
    app: &mut GraphBrowserApp,
) -> Vec<GraphIntent> {
    let mut intents = Vec::new();
    let (timeline_total, dissolved_total) = app.history_manager_archive_counts();

    ui.horizontal(|ui| {
        if ui.button("Settings").clicked() {
            intents.push(GraphIntent::OpenSettingsUrl {
                url: "graphshell://settings/general".to_string(),
            });
        }
        if ui.button("Done").clicked() {
            intents.push(GraphIntent::CloseToolPane {
                kind: crate::shell::desktop::workbench::pane_model::ToolPaneState::HistoryManager,
                restore_previous_focus: true,
            });
        }
    });
    ui.add_space(4.0);

    ui.horizontal(|ui| {
        ui.selectable_value(
            &mut app.workspace.history_manager_tab,
            HistoryManagerTab::Timeline,
            "Timeline",
        );
        ui.selectable_value(
            &mut app.workspace.history_manager_tab,
            HistoryManagerTab::Dissolved,
            "Dissolved",
        );
    });
    ui.add_space(8.0);

    match app.workspace.history_manager_tab {
        HistoryManagerTab::Timeline => {
            ui.horizontal(|ui| {
                ui.label(format!("Archived traversal entries: {timeline_total}"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Export").clicked() {
                        intents.push(GraphIntent::ExportHistoryTimeline);
                    }
                    if ui.button("Clear").clicked() {
                        intents.push(GraphIntent::ClearHistoryTimeline);
                    }
                });
            });
            let entries = app.history_manager_timeline_entries(history_manager_entry_limit());
            render_history_manager_rows(ui, app, &entries, &mut intents);
        }
        HistoryManagerTab::Dissolved => {
            ui.horizontal(|ui| {
                ui.label(format!("Archived dissolved entries: {dissolved_total}"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Export").clicked() {
                        intents.push(GraphIntent::ExportHistoryDissolved);
                    }
                    if ui.button("Clear").clicked() {
                        intents.push(GraphIntent::ClearHistoryDissolved);
                    }
                });
            });
            let entries = app.history_manager_dissolved_entries(history_manager_entry_limit());
            render_history_manager_rows(ui, app, &entries, &mut intents);
        }
    }

    intents
}

pub fn render_settings_tool_pane_in_ui_with_control_panel(
    ui: &mut Ui,
    app: &mut GraphBrowserApp,
    mut control_panel: Option<&mut crate::shell::desktop::runtime::control_panel::ControlPanel>,
) -> Vec<GraphIntent> {
    let mut intents: Vec<GraphIntent> = Vec::new();
    ui.heading("Settings");
    ui.separator();

    ui.horizontal(|ui| {
        if ui.button("History").clicked() {
            intents.push(GraphIntent::OpenSettingsUrl {
                url: "graphshell://settings/history".to_string(),
            });
        }
        if ui.button("Done").clicked() {
            intents.push(GraphIntent::CloseToolPane {
                kind: crate::shell::desktop::workbench::pane_model::ToolPaneState::Settings,
                restore_previous_focus: true,
            });
        }
    });
    ui.add_space(4.0);

    ui.horizontal_wrapped(|ui| {
        ui.label("Category:");
        ui.selectable_value(
            &mut app.workspace.settings_tool_page,
            crate::app::SettingsToolPage::General,
            "General",
        );
        ui.selectable_value(
            &mut app.workspace.settings_tool_page,
            crate::app::SettingsToolPage::Persistence,
            "Persistence",
        );
        ui.selectable_value(
            &mut app.workspace.settings_tool_page,
            crate::app::SettingsToolPage::Physics,
            "Physics",
        );
        ui.selectable_value(
            &mut app.workspace.settings_tool_page,
            crate::app::SettingsToolPage::Sync,
            "Sync",
        );
        ui.selectable_value(
            &mut app.workspace.settings_tool_page,
            crate::app::SettingsToolPage::Appearance,
            "Appearance",
        );
    });
    ui.separator();

    match app.workspace.settings_tool_page {
        crate::app::SettingsToolPage::General => {
            ui.label("Settings are page-backed app surfaces in this pane.");
            ui.label("Use categories to edit persistence, physics, sync, and appearance.");
            ui.add_space(8.0);
            if ui.button("Open History Surface").clicked() {
                intents.push(GraphIntent::OpenSettingsUrl {
                    url: "graphshell://settings/history".to_string(),
                });
            }
        }

        crate::app::SettingsToolPage::Persistence => {
            ui.label("Storage");
            ui.horizontal(|ui| {
                ui.label("Data directory:");
                let data_dir_input_id = ui.make_persistent_id("settings_tool_data_dir_input");
                let mut data_dir_input = ui
                    .data_mut(|d| d.get_persisted::<String>(data_dir_input_id))
                    .unwrap_or_default();
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut data_dir_input)
                            .desired_width(220.0)
                            .hint_text("C:\\path\\to\\graph_data"),
                    )
                    .changed()
                {
                    ui.data_mut(|d| d.insert_persisted(data_dir_input_id, data_dir_input.clone()));
                }
                if ui.button("Switch").clicked() {
                    let trimmed = data_dir_input.trim();
                    if !trimmed.is_empty() {
                        app.request_switch_data_dir(trimmed);
                    }
                }
            });

            ui.horizontal(|ui| {
                ui.label("Snapshot interval (sec):");
                let interval_input_id = ui.make_persistent_id("settings_tool_snapshot_interval_input");
                let mut interval_input = ui
                    .data_mut(|d| d.get_persisted::<String>(interval_input_id))
                    .unwrap_or_else(|| {
                        app.snapshot_interval_secs()
                            .unwrap_or(crate::services::persistence::DEFAULT_SNAPSHOT_INTERVAL_SECS)
                            .to_string()
                    });
                if ui
                    .add(egui::TextEdit::singleline(&mut interval_input).desired_width(80.0))
                    .changed()
                {
                    ui.data_mut(|d| d.insert_persisted(interval_input_id, interval_input.clone()));
                }
                if ui.button("Apply").clicked()
                    && let Ok(secs) = interval_input.trim().parse::<u64>()
                {
                    let _ = app.set_snapshot_interval_secs(secs);
                }
            });

            ui.separator();
            ui.label("Frames");
            if ui.button("Save Current Frame").clicked() {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                app.request_save_frame_snapshot_named(format!("workspace:toolpane-{now}"));
            }
            if ui.button("Prune Empty Named Frames").clicked() {
                app.request_prune_empty_frames();
            }

            ui.separator();
            ui.label("Graphs");
            if ui.button("Save Graph Snapshot").clicked() {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                app.request_save_graph_snapshot_named(format!("toolpane-graph-{now}"));
            }
            if ui.button("Restore Latest Graph").clicked() {
                app.request_restore_graph_snapshot_latest();
            }
        }

        crate::app::SettingsToolPage::Physics => {
            ui.label("Physics");
            render_physics_settings_in_ui(ui, app);
        }

        crate::app::SettingsToolPage::Sync => {
            ui.label("Sync");
            if let Some(control_panel) = control_panel.as_mut() {
                render_sync_settings_in_ui(ui, app, control_panel);
            } else {
                ui.small("Sync controls unavailable in this surface.");
            }
        }

        crate::app::SettingsToolPage::Appearance => {
            ui.label("Theme Mode");
            let current_dark = matches!(
                app.default_registry_theme_id(),
                Some(crate::registries::atomic::theme::THEME_ID_DARK)
            );
            let mut dark_mode = current_dark;
            ui.horizontal(|ui| {
                ui.radio_value(&mut dark_mode, false, "Light");
                ui.radio_value(&mut dark_mode, true, "Dark");
            });
            if dark_mode != current_dark {
                if dark_mode {
                    app.set_default_registry_theme_id(Some(crate::registries::atomic::theme::THEME_ID_DARK));
                } else {
                    app.set_default_registry_theme_id(Some(crate::registries::atomic::theme::THEME_ID_DEFAULT));
                }
            }
            ui.small("Theme mode is persisted through the workspace settings model.");
        }
    }

    intents
}

pub fn render_sync_settings_in_ui(
    ui: &mut Ui,
    app: &mut GraphBrowserApp,
    control_panel: &mut crate::shell::desktop::runtime::control_panel::ControlPanel,
) {
    let ctx = ui.ctx().clone();
    let pairing_code_id = egui::Id::new("verse_pairing_code");
    let pairing_code_input_id = egui::Id::new("verse_pairing_code_input");
    let discovery_results_id = egui::Id::new("verse_discovery_results");
    let sync_status_id = egui::Id::new("verse_sync_status");

    if let Some(discovery_result) = control_panel.take_discovery_results() {
        match discovery_result {
            Ok(peers) => {
                let discovered_count = peers.len();
                ctx.data_mut(|d| {
                    d.insert_temp(discovery_results_id, peers);
                    d.insert_temp(
                        sync_status_id,
                        format!("Discovery complete: {discovered_count} peer(s) found"),
                    );
                });
            }
            Err(error) => {
                ctx.data_mut(|d| {
                    d.insert_temp(sync_status_id, format!("Discovery failed: {error}"))
                });
            }
        }
    }

    let verse_initialized = crate::mods::native::verse::is_initialized();
    ui.label(egui::RichText::new("Trusted Devices").strong());
    ui.separator();

    if !verse_initialized {
        ui.label("Verse is initializing. Device list will appear shortly.");
    } else {
        ui.horizontal(|ui| {
            if ui.button("Show Pairing Code").clicked() {
                match crate::mods::native::verse::generate_pairing_code() {
                    Ok(code) => {
                        ctx.data_mut(|d| d.insert_temp(pairing_code_id, code));
                    }
                    Err(error) => {
                        ctx.data_mut(|d| {
                            d.insert_temp(
                                sync_status_id,
                                format!("Pairing code unavailable: {error}"),
                            )
                        });
                    }
                }
            }
            if ui.button("Discover Nearby").clicked() {
                match control_panel.request_discover_nearby_peers(2) {
                    Ok(()) => {
                        ctx.data_mut(|d| {
                            d.insert_temp(sync_status_id, "Discovering nearby peers...".to_string())
                        });
                    }
                    Err(error) => {
                        ctx.data_mut(|d| {
                            d.insert_temp(sync_status_id, format!("Discovery unavailable: {error}"))
                        });
                    }
                }
            }
            if ui.button("Sync Now").clicked() {
                let intents =
                    crate::shell::desktop::runtime::registries::phase5_execute_verse_sync_now_action(
                        app,
                    );
                if intents.is_empty() {
                    app.apply_intents([crate::app::GraphIntent::SyncNow]);
                } else {
                    app.apply_intents(intents);
                }
                ctx.data_mut(|d| {
                    d.insert_temp(sync_status_id, "Manual sync requested".to_string())
                });
            }
            if ui.button("Share Session Workspace").clicked() {
                let intents =
                    crate::shell::desktop::runtime::registries::phase5_execute_verse_share_workspace_action(
                        app,
                        crate::app::GraphBrowserApp::SESSION_WORKSPACE_LAYOUT_NAME,
                    );
                if !intents.is_empty() {
                    app.apply_intents(intents);
                }
                ctx.data_mut(|d| {
                    d.insert_temp(
                        sync_status_id,
                        "Shared session workspace with paired peers".to_string(),
                    )
                });
            }
        });

        if let Some(code) = ctx
            .data_mut(|d| d.get_temp::<crate::mods::native::verse::PairingCode>(pairing_code_id))
        {
            ui.group(|ui| {
                ui.label(egui::RichText::new("Pairing Code").strong());
                ui.monospace(code.phrase);
            });
        }

        let mut pairing_code_input = ctx
            .data_mut(|d| d.get_temp::<String>(pairing_code_input_id))
            .unwrap_or_default();
        ui.group(|ui| {
            ui.label(egui::RichText::new("Pair by Code").strong());
            ui.small("Format: word-word-word-word-word-word:<node-id>");
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut pairing_code_input)
                        .desired_width(340.0)
                        .hint_text("word-word-word-word-word-word:<node-id>"),
                );
                if ui.button("Pair").clicked() {
                    let code = pairing_code_input.trim().to_string();
                    if code.is_empty() {
                        ctx.data_mut(|d| {
                            d.insert_temp(sync_status_id, "Enter a pairing code first".to_string())
                        });
                    } else {
                        let before = crate::mods::native::verse::get_trusted_peers().len();
                        let intents =
                            crate::shell::desktop::runtime::registries::phase5_execute_verse_pair_code_action(
                                app, &code,
                            );
                        if !intents.is_empty() {
                            app.apply_intents(intents);
                        }
                        let after = crate::mods::native::verse::get_trusted_peers().len();
                        let status = if after > before {
                            "Pairing succeeded".to_string()
                        } else {
                            "Pairing not completed (verify code and try again)".to_string()
                        };
                        ctx.data_mut(|d| d.insert_temp(sync_status_id, status));
                    }
                }
            });
        });
        ctx.data_mut(|d| d.insert_temp(pairing_code_input_id, pairing_code_input));

        if let Some(peers) = ctx
            .data_mut(|d| d.get_temp::<Vec<crate::mods::native::verse::DiscoveredPeer>>(discovery_results_id))
            && !peers.is_empty()
        {
            ui.group(|ui| {
                ui.label(egui::RichText::new("Nearby Devices").strong());
                for peer in peers {
                    ui.horizontal(|ui| {
                        ui.label(format!("{} ({})", peer.device_name, peer.node_id.to_string()));
                        if ui.button("Pair").clicked() {
                            let intents =
                                crate::shell::desktop::runtime::registries::phase5_execute_verse_pair_local_peer_action(
                                    app,
                                    &peer.node_id.to_string(),
                                );
                            if !intents.is_empty() {
                                app.apply_intents(intents);
                            }
                            ctx.data_mut(|d| {
                                d.insert_temp(
                                    sync_status_id,
                                    format!("Paired with {}", peer.node_id.to_string()),
                                )
                            });
                        }
                    });
                }
            });
        }

        let peers = crate::mods::native::verse::get_trusted_peers();
        if peers.is_empty() {
            ui.label("No paired devices yet.");
        } else {
            for peer in &peers {
                ui.group(|ui| {
                    let peer_display = format!(
                        "{} ({})",
                        peer.display_name,
                        peer.node_id.to_string()[..8].to_uppercase()
                    );
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(peer_display).strong());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("Forget").clicked() {
                                let intents =
                                    crate::shell::desktop::runtime::registries::phase5_execute_verse_forget_device_action(
                                        app,
                                        &peer.node_id.to_string(),
                                    );
                                app.apply_intents(intents);
                            }
                        });
                    });

                    if peer.workspace_grants.is_empty() {
                        ui.small("No workspace grants");
                    } else {
                        for grant in &peer.workspace_grants {
                            ui.horizontal(|ui| {
                                let access_str = match grant.access {
                                    crate::mods::native::verse::AccessLevel::ReadOnly => {
                                        "read-only"
                                    }
                                    crate::mods::native::verse::AccessLevel::ReadWrite => {
                                        "read-write"
                                    }
                                };
                                ui.small(format!("{}: {}", grant.workspace_id, access_str));
                                if ui.small_button("Revoke").clicked() {
                                    let intent = crate::app::GraphIntent::RevokeWorkspaceAccess {
                                        peer_id: peer.node_id.to_string(),
                                        workspace_id: grant.workspace_id.clone(),
                                    };
                                    app.apply_intents(vec![intent]);
                                }
                            });
                        }
                    }
                });
            }
        }
    }

    ui.separator();
    ui.label(egui::RichText::new("Sync Status").strong());
    if !verse_initialized {
        ui.label("Initializing Verse networking...");
    } else {
        let peers = crate::mods::native::verse::get_trusted_peers();
        ui.label(format!("Connected peers: {}", peers.len()));
    }

    if let Some(message) = ctx.data_mut(|d| d.get_temp::<String>(sync_status_id)) {
        ui.separator();
        ui.small(message);
    }
}

fn history_manager_entry_limit() -> usize {
    static LIMIT: OnceLock<usize> = OnceLock::new();
    *LIMIT.get_or_init(|| {
        if let Ok(value) = env::var("GRAPHSHELL_HISTORY_MANAGER_LIMIT") {
            let trimmed = value.trim();
            if let Ok(parsed) = trimmed.parse::<usize>()
                && parsed > 0
            {
                emit_event(DiagnosticEvent::MessageSent {
                    channel_id: CHANNEL_UI_HISTORY_MANAGER_LIMIT,
                    byte_len: trimmed.len(),
                });
                return parsed;
            }
        }
        250
    })
}

#[cfg(test)]
pub(crate) fn history_manager_entry_limit_for_tests() -> usize {
    history_manager_entry_limit()
}

fn render_history_manager_rows(
    ui: &mut Ui,
    app: &GraphBrowserApp,
    entries: &[crate::services::persistence::types::LogEntry],
    intents: &mut Vec<GraphIntent>,
) {
    if entries.is_empty() {
        ui.label("No history entries yet.");
        return;
    }

    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for entry in entries {
                let crate::services::persistence::types::LogEntry::AppendTraversal {
                    from_node_id,
                    to_node_id,
                    timestamp_ms,
                    trigger,
                } = entry
                else {
                    continue;
                };

                let from_key = Uuid::parse_str(from_node_id)
                    .ok()
                    .and_then(|id| app.workspace.graph.get_node_key_by_id(id));
                let to_key = Uuid::parse_str(to_node_id)
                    .ok()
                    .and_then(|id| app.workspace.graph.get_node_key_by_id(id));

                let from_label = from_key
                    .and_then(|k| app.workspace.graph.get_node(k))
                    .map(|n| {
                        if n.title.is_empty() {
                            n.url.as_str()
                        } else {
                            n.title.as_str()
                        }
                    })
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| {
                        format!("<missing:{}>", &from_node_id[..from_node_id.len().min(8)])
                    });
                let to_label = to_key
                    .and_then(|k| app.workspace.graph.get_node(k))
                    .map(|n| {
                        if n.title.is_empty() {
                            n.url.as_str()
                        } else {
                            n.title.as_str()
                        }
                    })
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| {
                        format!("<missing:{}>", &to_node_id[..to_node_id.len().min(8)])
                    });

                let elapsed_ms = now_ms.saturating_sub(*timestamp_ms);
                let time_label = if elapsed_ms < 1_000 {
                    "just now".to_string()
                } else if elapsed_ms < 60_000 {
                    format!("{}s ago", elapsed_ms / 1_000)
                } else if elapsed_ms < 3_600_000 {
                    format!("{}m ago", elapsed_ms / 60_000)
                } else if elapsed_ms < 86_400_000 {
                    format!("{}h ago", elapsed_ms / 3_600_000)
                } else {
                    format!("{}d ago", elapsed_ms / 86_400_000)
                };

                let trigger_label = match trigger {
                    crate::services::persistence::types::PersistedNavigationTrigger::Back => {
                        "⬅ Back"
                    }
                    crate::services::persistence::types::PersistedNavigationTrigger::Forward => {
                        "➡ Forward"
                    }
                    crate::services::persistence::types::PersistedNavigationTrigger::Unknown => "↔",
                };

                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(time_label).weak().small());
                    ui.label(trigger_label);
                    let response =
                        ui.selectable_label(false, format!("{} → {}", from_label, to_label));
                    if response.clicked()
                        && let Some(key) = from_key
                    {
                        intents.push(GraphIntent::SelectNode {
                            key,
                            multi_select: false,
                        });
                        intents.push(GraphIntent::RequestZoomToSelected);
                    }
                });
                ui.add_space(2.0);
            }
        });
}

fn apply_ui_intents_with_checkpoint(app: &mut GraphBrowserApp, intents: Vec<GraphIntent>) {
    if intents.is_empty() {
        return;
    }
    if intents.iter().any(is_user_undoable_intent) {
        let layout = app
            .last_session_workspace_layout_json()
            .map(str::to_string)
            .or_else(|| {
                app.load_workspace_layout_json(GraphBrowserApp::SESSION_WORKSPACE_LAYOUT_NAME)
            });
        app.capture_undo_checkpoint(layout);
    }
    app.apply_intents(intents);
}

fn is_user_undoable_intent(intent: &GraphIntent) -> bool {
    matches!(
        intent,
        GraphIntent::CreateNodeNearCenter
            | GraphIntent::CreateNodeNearCenterAndOpen { .. }
            | GraphIntent::CreateNodeAtUrl { .. }
            | GraphIntent::CreateNodeAtUrlAndOpen { .. }
            | GraphIntent::RemoveSelectedNodes
            | GraphIntent::ClearGraph
            | GraphIntent::SetNodePosition { .. }
            | GraphIntent::SetNodeUrl { .. }
            | GraphIntent::CreateUserGroupedEdge { .. }
            | GraphIntent::RemoveEdge { .. }
            | GraphIntent::ExecuteEdgeCommand { .. }
            | GraphIntent::SetNodePinned { .. }
            | GraphIntent::TogglePrimaryNodePin
            | GraphIntent::PromoteNodeToActive { .. }
            | GraphIntent::DemoteNodeToWarm { .. }
            | GraphIntent::DemoteNodeToCold { .. }
    )
}

pub fn render_choose_frame_picker(ctx: &egui::Context, app: &mut GraphBrowserApp) -> bool {
    let mut open_settings_tool_pane = false;
    let Some(request) = app.choose_frame_picker_request() else {
        return false;
    };
    let target = request.node;
    if app.workspace.graph.get_node(target).is_none() {
        app.clear_choose_frame_picker();
        return false;
    }
    let mut selected_frame: Option<String> = None;
    let mut close = false;
    let mut memberships = match request.mode {
        ChooseFramePickerMode::OpenNodeInFrame => {
            app.sorted_frames_for_node_key(target)
        }
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
        .graph
        .get_node(target)
        .map(|node| format!("Choose Frame: {}", node.title))
        .unwrap_or_else(|| "Choose Frame".to_string());
    Window::new(title)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .default_width(300.0)
        .show(ctx, |ui| {
            if memberships.is_empty() {
                let msg = match request.mode {
                    ChooseFramePickerMode::OpenNodeInFrame => "No frame memberships for this node.",
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
                ui.small(msg);
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
                ui.small(header);
                for name in &memberships {
                    if ui.button(name).clicked() {
                        selected_frame = Some(name.clone());
                        close = true;
                    }
                }
            }
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Open Persistence Hub").clicked() {
                    open_settings_tool_pane = true;
                }
                if ui.button("Close").clicked() {
                    close = true;
                }
            });
        });
    if let Some(name) = selected_frame {
        match request.mode {
            ChooseFramePickerMode::OpenNodeInFrame => {
                app.apply_intents([GraphIntent::OpenNodeFrameRouted {
                    key: target,
                    prefer_frame: Some(name),
                }]);
            }
            ChooseFramePickerMode::AddNodeToFrame => {
                app.request_add_node_to_frame(target, name);
            }
            ChooseFramePickerMode::AddConnectedSelectionToFrame => {
                let mut seed_nodes: Vec<NodeKey> = if app.workspace.selected_nodes.is_empty() {
                    vec![target]
                } else {
                    app.workspace.selected_nodes.iter().copied().collect()
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
                nodes.retain(|key| app.workspace.graph.get_node(*key).is_some());
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
    let mut open_settings_tool_pane = false;
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
                },
            }
            ui.separator();
            if ui.button("Open Persistence Hub").clicked() {
                open_settings_tool_pane = true;
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
    if let Some((from, to)) = app.workspace.selected_nodes.ordered_pair() {
        return Some((from, to));
    }

    if app.workspace.selected_nodes.len() == 1 {
        let from = app.workspace.selected_nodes.primary()?;
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
        .or(app.workspace.selected_nodes.primary())
        .or(hovered_node)
        .or(focused_pane_node)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::SearchDisplayMode;
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
        app.apply_intents(intents);

        assert!(app.workspace.selected_nodes.contains(&key));
    }

    #[test]
    fn test_drag_start_sets_interacting() {
        let mut app = test_app();
        assert!(!app.workspace.is_interacting);

        let intents = intents_from_graph_actions(vec![GraphAction::DragStart]);
        app.apply_intents(intents);

        assert!(app.workspace.is_interacting);
    }

    #[test]
    fn test_drag_end_clears_interacting_and_updates_position() {
        let mut app = test_app();
        let key = app.add_node_and_sync("https://example.com".into(), Point2D::new(0.0, 0.0));
        app.set_interacting(true);

        let intents =
            intents_from_graph_actions(vec![GraphAction::DragEnd(key, Point2D::new(150.0, 250.0))]);
        app.apply_intents(intents);

        assert!(!app.workspace.is_interacting);
        let node = app.workspace.graph.get_node(key).unwrap();
        assert_eq!(node.position, Point2D::new(150.0, 250.0));
    }

    #[test]
    fn test_move_node_updates_position() {
        let mut app = test_app();
        let key = app.add_node_and_sync("https://example.com".into(), Point2D::new(0.0, 0.0));

        let intents =
            intents_from_graph_actions(vec![GraphAction::MoveNode(key, Point2D::new(42.0, 84.0))]);
        app.apply_intents(intents);

        let node = app.workspace.graph.get_node(key).unwrap();
        assert_eq!(node.position, Point2D::new(42.0, 84.0));
    }

    #[test]
    fn test_select_node_action() {
        let mut app = test_app();
        let key = app.add_node_and_sync("https://example.com".into(), Point2D::new(0.0, 0.0));

        let intents = intents_from_graph_actions(vec![GraphAction::SelectNode {
            key,
            multi_select: false,
        }]);
        app.apply_intents(intents);

        assert!(app.workspace.selected_nodes.contains(&key));
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
        app.apply_intents(intents);

        // Ctrl+Click adds b without deselecting a.
        let intents = intents_from_graph_actions(vec![GraphAction::SelectNode {
            key: b,
            multi_select: true,
        }]);
        app.apply_intents(intents);

        assert!(app.workspace.selected_nodes.contains(&a));
        assert!(app.workspace.selected_nodes.contains(&b));
        assert_eq!(app.workspace.selected_nodes.len(), 2);
    }

    #[test]
    fn test_select_node_action_ctrl_click_toggles_off_selected() {
        let mut app = test_app();
        let a = app.add_node_and_sync("a".into(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("b".into(), Point2D::new(100.0, 0.0));

        // Select both nodes.
        app.apply_intents(intents_from_graph_actions(vec![
            GraphAction::SelectNode {
                key: a,
                multi_select: false,
            },
            GraphAction::SelectNode {
                key: b,
                multi_select: true,
            },
        ]));
        assert_eq!(app.workspace.selected_nodes.len(), 2);

        // Ctrl+Click a again → toggles a out of the selection.
        app.apply_intents(intents_from_graph_actions(vec![GraphAction::SelectNode {
            key: a,
            multi_select: true,
        }]));

        assert!(!app.workspace.selected_nodes.contains(&a));
        assert!(app.workspace.selected_nodes.contains(&b));
        assert_eq!(app.workspace.selected_nodes.len(), 1);
    }

    #[test]
    fn test_select_node_single_click_does_not_affect_multi_selection() {
        let mut app = test_app();
        let a = app.add_node_and_sync("a".into(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("b".into(), Point2D::new(100.0, 0.0));
        let c = app.add_node_and_sync("c".into(), Point2D::new(200.0, 0.0));

        // Select a and b via multi-select.
        app.apply_intents(intents_from_graph_actions(vec![
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
        app.apply_intents(intents_from_graph_actions(vec![GraphAction::SelectNode {
            key: c,
            multi_select: false,
        }]));

        assert!(!app.workspace.selected_nodes.contains(&a));
        assert!(!app.workspace.selected_nodes.contains(&b));
        assert!(app.workspace.selected_nodes.contains(&c));
        assert_eq!(app.workspace.selected_nodes.len(), 1);
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
        app.apply_intents(intents);

        assert_eq!(app.workspace.selected_nodes.len(), 2);
        assert!(app.workspace.selected_nodes.contains(&a));
        assert!(app.workspace.selected_nodes.contains(&b));
        assert_eq!(app.workspace.selected_nodes.primary(), Some(b));
    }

    #[test]
    fn test_clear_selection_action_maps_to_intent() {
        let mut app = test_app();
        let a = app.add_node_and_sync("a".into(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("b".into(), Point2D::new(10.0, 0.0));

        app.apply_intents(intents_from_graph_actions(vec![GraphAction::LassoSelect {
            keys: vec![a, b],
            mode: SelectionUpdateMode::Replace,
        }]));
        assert_eq!(app.workspace.selected_nodes.len(), 2);

        app.apply_intents(intents_from_graph_actions(vec![
            GraphAction::ClearSelection,
        ]));
        assert!(app.workspace.selected_nodes.is_empty());
        assert_eq!(app.workspace.selected_nodes.primary(), None);
    }

    #[test]
    fn test_primary_click_handler_detects_selection_action() {
        let actions = vec![GraphAction::SelectNode {
            key: NodeIndex::new(0),
            multi_select: false,
        }];

        assert!(actions.iter().any(action_handles_primary_click));
    }

    #[test]
    fn test_primary_click_handler_ignores_zoom_only_actions() {
        let actions = vec![GraphAction::Zoom(1.2)];

        assert!(!actions.iter().any(action_handles_primary_click));
    }

    #[test]
    fn test_background_click_clear_selection_requires_no_modifiers() {
        let modifiers = egui::Modifiers::CTRL;

        assert!(!should_clear_selection_on_background_click(
            true,
            modifiers,
            None,
            false,
            false,
            false,
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
    fn test_zoom_action_clamps() {
        let mut app = test_app();

        let intents = intents_from_graph_actions(vec![GraphAction::Zoom(0.01)]);
        app.apply_intents(intents);

        // Should be clamped to min zoom
        assert!(app.workspace.camera.current_zoom >= app.workspace.camera.zoom_min);
    }

    #[test]
    fn test_multiple_actions_sequence() {
        let mut app = test_app();
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
        app.apply_intents(intents);

        assert!(app.workspace.selected_nodes.contains(&k1));
        assert_eq!(
            app.workspace.graph.get_node(k2).unwrap().position,
            Point2D::new(200.0, 300.0)
        );
        assert!((app.workspace.camera.current_zoom - 1.5).abs() < 0.01);
    }

    #[test]
    fn test_empty_actions_is_noop() {
        let mut app = test_app();
        let key = app.add_node_and_sync("a".into(), Point2D::new(50.0, 60.0));
        let pos_before = app.workspace.graph.get_node(key).unwrap().position;

        let intents = intents_from_graph_actions(vec![]);
        app.apply_intents(intents);

        assert_eq!(
            app.workspace.graph.get_node(key).unwrap().position,
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
        app.workspace.selected_nodes.clear();
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
    fn test_format_elapsed_ago_units() {
        assert_eq!(format_elapsed_ago(Duration::from_secs(2)), "just now");
        assert_eq!(format_elapsed_ago(Duration::from_secs(12)), "12s ago");
        assert_eq!(format_elapsed_ago(Duration::from_secs(120)), "2m ago");
        assert_eq!(
            format_elapsed_ago(Duration::from_secs(60 * 60 * 3)),
            "3h ago"
        );
        assert_eq!(
            format_elapsed_ago(Duration::from_secs(60 * 60 * 24 * 2)),
            "2d ago"
        );
    }

    #[test]
    fn test_format_last_visited_with_future_timestamp_is_just_now() {
        let now = SystemTime::now();
        let future = now + Duration::from_secs(10);
        assert_eq!(format_last_visited_with_now(future, now), "just now");
    }

    #[test]
    fn test_neighbor_set_computation() {
        let mut app = test_app();
        let a = app.add_node_and_sync("a".into(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("b".into(), Point2D::new(10.0, 0.0));
        let c = app.add_node_and_sync("c".into(), Point2D::new(20.0, 0.0));
        let _ = app.add_edge_and_sync(a, b, crate::graph::EdgeType::Hyperlink);
        let _ = app.add_edge_and_sync(c, a, crate::graph::EdgeType::Hyperlink);

        let set = hovered_adjacency_set(&app, Some(a));
        assert!(set.contains(&a));
        assert!(set.contains(&b));
        assert!(set.contains(&c));
    }

    #[test]
    fn test_search_highlight_mode_dims_not_hides() {
        let mut app = test_app();
        let a = app.add_node_and_sync("alpha".into(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("beta".into(), Point2D::new(10.0, 0.0));
        app.workspace.search_display_mode = SearchDisplayMode::Highlight;
        app.workspace.egui_state = Some(EguiGraphState::from_graph_with_visual_state(
            &app.workspace.graph,
            &app.workspace.selected_nodes,
            app.workspace.selected_nodes.primary(),
            &HashSet::new(),
        ));
        let matches = HashSet::from([a]);
        apply_search_node_visuals(&mut app, &matches, Some(a), true);

        let state = app.workspace.egui_state.as_ref().unwrap();
        assert!(state.graph.node(a).is_some());
        assert!(state.graph.node(b).is_some());
        let b_color = state.graph.node(b).unwrap().color().unwrap();
        assert!(b_color != lifecycle_color(NodeLifecycle::Cold));
    }

    #[test]
    fn test_search_filter_mode_hides_nodes() {
        let mut app = test_app();
        let a = app.add_node_and_sync("alpha".into(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("beta".into(), Point2D::new(10.0, 0.0));
        app.workspace.search_display_mode = SearchDisplayMode::Filter;
        let matches = HashSet::from([a]);
        let filtered = filtered_graph_for_search(&app, &matches);
        assert!(filtered.get_node(a).is_some());
        assert!(filtered.get_node(b).is_none());
    }

    /// Simulate the sync conditions for group drag:
    /// Build egui_state from the graph, move the dragged node in egui_state,
    /// then run sync and assert secondary selected nodes follow.
    fn setup_group_drag_sync(app: &mut GraphBrowserApp, dragged_key: NodeKey, delta: egui::Vec2) {
        use crate::graph::egui_adapter::EguiGraphState;
        // Build egui_state seeded from current app.workspace.graph positions.
        app.workspace.egui_state = Some(EguiGraphState::from_graph(
            &app.workspace.graph,
            &std::collections::HashSet::new(),
        ));
        // Simulate egui_graphs moving the dragged node by delta.
        if let Some(state_mut) = app.workspace.egui_state.as_mut() {
            if let Some(node) = state_mut.graph.node_mut(dragged_key) {
                let old = node.location();
                node.set_location(egui::Pos2::new(old.x + delta.x, old.y + delta.y));
            }
        }
        app.workspace.is_interacting = true;
    }

    #[test]
    fn test_group_drag_moves_secondary_selected_nodes() {
        let mut app = test_app();
        let a = app.add_node_and_sync("a".into(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("b".into(), Point2D::new(100.0, 0.0));
        let c = app.add_node_and_sync("c".into(), Point2D::new(200.0, 0.0));

        // Select A (primary) and B (secondary); C is unselected.
        app.select_node(a, false);
        app.select_node(b, true);

        let delta = egui::Vec2::new(10.0, 20.0);
        setup_group_drag_sync(&mut app, a, delta);
        sync_graph_positions_from_layout(&mut app);

        // A moved to its dragged position.
        let a_pos = app.workspace.graph.get_node(a).unwrap().position;
        assert!((a_pos.x - 10.0).abs() < 0.1, "a.x={}", a_pos.x);
        assert!((a_pos.y - 20.0).abs() < 0.1, "a.y={}", a_pos.y);

        // B followed by the same delta.
        let b_pos = app.workspace.graph.get_node(b).unwrap().position;
        assert!((b_pos.x - 110.0).abs() < 0.1, "b.x={}", b_pos.x);
        assert!((b_pos.y - 20.0).abs() < 0.1, "b.y={}", b_pos.y);

        // C was not selected — stays put.
        let c_pos = app.workspace.graph.get_node(c).unwrap().position;
        assert!((c_pos.x - 200.0).abs() < 0.1, "c.x={}", c_pos.x);
        assert!((c_pos.y - 0.0).abs() < 0.1, "c.y={}", c_pos.y);
    }

    #[test]
    fn test_group_drag_no_propagation_when_single_selection() {
        let mut app = test_app();
        let a = app.add_node_and_sync("a".into(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("b".into(), Point2D::new(100.0, 0.0));

        // Only A selected.
        app.select_node(a, false);

        let delta = egui::Vec2::new(10.0, 20.0);
        setup_group_drag_sync(&mut app, a, delta);
        sync_graph_positions_from_layout(&mut app);

        // B must not move (single selection — no group drag).
        let b_pos = app.workspace.graph.get_node(b).unwrap().position;
        assert!((b_pos.x - 100.0).abs() < 0.1, "b.x={}", b_pos.x);
        assert!((b_pos.y - 0.0).abs() < 0.1, "b.y={}", b_pos.y);
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
            let _ = app.add_edge_and_sync(pair[0], pair[1], crate::graph::EdgeType::Hyperlink);
        }

        let canvas_rect = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(300.0, 300.0));
        let metrics = viewport_culling_metrics_for_canvas_rect(&app.workspace.graph, canvas_rect)
            .expect("culling metrics should be available for dense graph viewport");

        assert!(metrics.visible_nodes < metrics.total_nodes);
        assert!(metrics.submitted_nodes < metrics.total_nodes);
        assert!(metrics.removed_nodes > 0);
        assert!(metrics.culled_submission_units < metrics.full_submission_units);
    }

    #[test]
    fn viewport_culling_benchmark_reports_lower_prep_time_than_full_rebuild() {
        let mut app = test_app();
        let mut keys = Vec::new();
        for row in 0..30 {
            for col in 0..30 {
                let key = app.add_node_and_sync(
                    format!("https://bench.example/{row}/{col}"),
                    Point2D::new(col as f32 * 30.0, row as f32 * 30.0),
                );
                keys.push(key);
            }
        }

        for pair in keys.windows(2) {
            let _ = app.add_edge_and_sync(pair[0], pair[1], crate::graph::EdgeType::Hyperlink);
        }

        let canvas_rect = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(260.0, 260.0));
        let culled_graph = viewport_culled_graph_for_canvas_rect(&app.workspace.graph, canvas_rect)
            .expect("expected culled graph for benchmark viewport");

        let full_start = Instant::now();
        for _ in 0..12 {
            let state = EguiGraphState::from_graph_with_visual_state(
                &app.workspace.graph,
                &app.workspace.selected_nodes,
                app.workspace.selected_nodes.primary(),
                &HashSet::new(),
            );
            black_box(state.graph.node_count());
        }
        let full_elapsed = full_start.elapsed();

        let culled_start = Instant::now();
        for _ in 0..12 {
            let state = EguiGraphState::from_graph_with_visual_state(
                &culled_graph,
                &app.workspace.selected_nodes,
                app.workspace.selected_nodes.primary(),
                &HashSet::new(),
            );
            black_box(state.graph.node_count());
        }
        let culled_elapsed = culled_start.elapsed();

        assert!(
            culled_elapsed < full_elapsed,
            "expected culled prep to be faster; full={:?}, culled={:?}",
            full_elapsed,
            culled_elapsed
        );
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
    fn background_pan_updates_metadata_and_focus_for_non_zero_delta() {
        let ctx = egui::Context::default();
        let metadata_id = egui::Id::new("test-background-pan");
        let view_id = crate::app::GraphViewId::default();
        let mut app = test_app();
        app.workspace
            .views
            .insert(view_id, crate::app::GraphViewState::new("Pan Test"));

        ctx.data_mut(|data| {
            let mut frame = MetadataFrame::default();
            frame.zoom = 1.0;
            frame.pan = egui::vec2(10.0, 20.0);
            data.insert_persisted(
                metadata_id,
                frame,
            );
        });

        let changed = apply_background_pan(
            &ctx,
            metadata_id,
            &mut app,
            view_id,
            egui::vec2(15.0, -5.0),
        );

        assert!(changed);
        assert_eq!(app.workspace.focused_view, Some(view_id));
        ctx.data_mut(|data| {
            let meta = data
                .get_persisted::<MetadataFrame>(metadata_id)
                .expect("background pan should keep metadata persisted");
            assert!((meta.pan.x - 25.0).abs() < 0.001);
            assert!((meta.pan.y - 15.0).abs() < 0.001);
        });
    }

    #[test]
    fn background_pan_is_noop_for_zero_delta() {
        let ctx = egui::Context::default();
        let metadata_id = egui::Id::new("test-background-pan-zero");
        let view_id = crate::app::GraphViewId::default();
        let mut app = test_app();
        app.workspace
            .views
            .insert(view_id, crate::app::GraphViewState::new("Zero Pan Test"));

        ctx.data_mut(|data| {
            let mut frame = MetadataFrame::default();
            frame.zoom = 1.0;
            frame.pan = egui::vec2(10.0, 20.0);
            data.insert_persisted(
                metadata_id,
                frame,
            );
        });

        let changed = apply_background_pan(&ctx, metadata_id, &mut app, view_id, Vec2::ZERO);

        assert!(!changed);
        assert_eq!(app.workspace.focused_view, None);
        ctx.data_mut(|data| {
            let meta = data
                .get_persisted::<MetadataFrame>(metadata_id)
                .expect("zero-delta pan should leave metadata intact");
            assert!((meta.pan.x - 10.0).abs() < 0.001);
            assert!((meta.pan.y - 20.0).abs() < 0.001);
        });
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
        let selection_a = viewport_culling_selection_for_canvas_rect(&app.workspace.graph, rect_a)
            .expect("expected culling selection for first view frame");
        let selection_b = viewport_culling_selection_for_canvas_rect(&app.workspace.graph, rect_b)
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
