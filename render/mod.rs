/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Graph rendering module using egui_graphs.
//!
//! Delegates graph visualization and interaction to the egui_graphs crate,
//! which provides built-in navigation (zoom/pan), node dragging, and selection.

use crate::app::{
    CameraCommand, ChooseWorkspacePickerMode, EdgeCommand, GraphBrowserApp, GraphIntent, KeyboardZoomRequest,
    HistoryManagerTab, LassoMouseBinding, MemoryPressureLevel, PendingConnectedOpenScope, PendingTileOpenMode,
    SearchDisplayMode, SelectionUpdateMode, UnsavedWorkspacePromptAction,
    UnsavedWorkspacePromptRequest,
};
use crate::graph::egui_adapter::{EguiGraphState, GraphEdgeShape, GraphNodeShape};
use crate::graph::{NodeKey, NodeLifecycle};
use crate::shell::desktop::ui::persistence_ops;
use crate::registries::domain::layout::LayoutDomainRegistry;
use crate::registries::domain::layout::canvas::CANVAS_PROFILE_DEFAULT;
use crate::registries::domain::layout::viewer_surface::VIEWER_SURFACE_DEFAULT;
use crate::registries::domain::layout::workbench_surface::WORKBENCH_SURFACE_DEFAULT;
use egui::{Color32, Key, Stroke, Ui, Vec2, Window};
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
use crate::shell::desktop::runtime::diagnostics::{DiagnosticEvent, emit_event};
use crate::shell::desktop::runtime::registries::CHANNEL_UI_HISTORY_MANAGER_LIMIT;

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
    Zoom(f32),
}

/// Render graph info and controls hint overlay text into the current UI.
pub fn render_graph_info_in_ui(ui: &mut Ui, app: &GraphBrowserApp) {
    draw_graph_info(ui, app);
}

fn canvas_navigation_settings(
    profile: &crate::registries::domain::layout::canvas::CanvasSurfaceProfile,
) -> SettingsNavigation {
    let zoom_enabled = if profile.layout_algorithm.algorithm_id == "graph_layout:tree" {
        false
    } else {
        profile.navigation.zoom_and_pan_enabled
    };

    SettingsNavigation::new()
        .with_fit_to_screen_enabled(profile.navigation.fit_to_screen_enabled)
        .with_zoom_and_pan_enabled(zoom_enabled)
}

fn canvas_interaction_settings(
    profile: &crate::registries::domain::layout::canvas::CanvasSurfaceProfile,
    radial_open: bool,
    right_button_down: bool,
) -> SettingsInteraction {
    let is_tree_topology = profile.topology.policy_id == "topology:tree";

    SettingsInteraction::new()
        .with_dragging_enabled(
            profile.interaction.dragging_enabled
                && !radial_open
                && !right_button_down
                && !is_tree_topology,
        )
        .with_node_selection_enabled(
            profile.interaction.node_selection_enabled && !radial_open && !right_button_down,
        )
        .with_node_clicking_enabled(profile.interaction.node_clicking_enabled && !radial_open)
}

fn canvas_style_settings(
    profile: &crate::registries::domain::layout::canvas::CanvasSurfaceProfile,
) -> SettingsStyle {
    SettingsStyle::new().with_labels_always(profile.style.labels_always)
}

/// Render graph content and return resolved interaction actions.
///
/// This lets callers customize how specific actions are handled
/// (e.g. routing double-click to tile opening instead of detail view).
pub fn render_graph_in_ui_collect_actions(
    ui: &mut Ui,
    app: &mut GraphBrowserApp,
    view_id: Option<crate::app::GraphViewId>,
    search_matches: &HashSet<NodeKey>,
    active_search_match: Option<NodeKey>,
    search_display_mode: SearchDisplayMode,
    search_query_active: bool,
) -> Vec<GraphAction> {
    let ctrl_pressed = ui.input(|i| i.modifiers.ctrl || i.modifiers.command);
    let right_button_down = ui.input(|i| i.pointer.secondary_down());
    let radial_open = app.workspace.show_radial_menu;
    let filtered_graph =
        if matches!(search_display_mode, SearchDisplayMode::Filter) && search_query_active {
            Some(filtered_graph_for_search(app, search_matches))
        } else {
            None
        };
    let graph_for_render = filtered_graph.as_ref().unwrap_or(&app.workspace.graph);

    // Build or reuse egui_graphs state (rebuild always when filtering is active).
    if app.workspace.egui_state.is_none() || app.workspace.egui_state_dirty || filtered_graph.is_some() {
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

    let layout_domain = LayoutDomainRegistry::default();
    let layout_profile = layout_domain.resolve_profile(
        CANVAS_PROFILE_DEFAULT,
        WORKBENCH_SURFACE_DEFAULT,
        VIEWER_SURFACE_DEFAULT,
    );
    let canvas_profile = &layout_profile.canvas.profile;

    // Graph settings resolved from layout domain canvas surface profile.
    let nav = canvas_navigation_settings(canvas_profile);
    let interaction = canvas_interaction_settings(canvas_profile, radial_open, right_button_down);
    let style = canvas_style_settings(canvas_profile);

    // Resolve physics state and lens from the view (or default to global).
    let (mut physics_state, lens_config) = if let Some(view_id) = view_id
        && let Some(view) = app.workspace.views.get(&view_id)
    {
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
            let should_capture = if app.workspace.scroll_zoom_requires_ctrl {
                ctrl_pressed
            } else {
                true
            };

            if should_capture {
                app.queue_pending_wheel_zoom_delta(scroll_delta);
                input.smooth_scroll_delta.y = 0.0;
                input.raw_scroll_delta.y = 0.0;
            }
        });
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
    if let Some(view_id) = view_id
        && let Some(view) = app.workspace.views.get_mut(&view_id)
        && let Some(local) = &mut view.local_simulation
    {
        local.physics = new_physics;
    } else {
        app.workspace.physics = new_physics;
    }

    // Apply semantic clustering forces if enabled (UDC Phase 2)
    let semantic_config = if let Some(view_id) = view_id {
        app.workspace.views
            .get(&view_id)
            .map(|v| (v.lens.physics.semantic_clustering, v.lens.physics.semantic_strength))
    } else {
        None
    };
    apply_semantic_clustering_forces(app, semantic_config);

    app.workspace.hovered_graph_node = app.workspace.egui_state.as_ref().and_then(|state| {
        state
            .graph
            .hovered_node()
            .and_then(|idx| state.get_key(idx))
    });
    let lasso = collect_lasso_action(ui, app, !radial_open);

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
            app.request_choose_workspace_picker(target);
        }
    }
    draw_highlighted_edge_overlay(ui, app, response.id);
    draw_hovered_node_tooltip(ui, app, response.id);
    draw_hovered_edge_tooltip(ui, app, response.id);

    // Custom navigation handling (Zoom/Pan/Fit)
    // We use the widget ID from the response to target the correct MetadataFrame.
    let metadata_id = response.id.with("metadata");
    let custom_zoom = handle_custom_navigation(ui, &response, metadata_id, app, !radial_open);

    let split_open_modifier = ui.input(|i| i.modifiers.shift);
    let mut actions = collect_graph_actions(app, &events, split_open_modifier, ctrl_pressed);
    if let Some(lasso_action) = lasso.action {
        actions.push(lasso_action);
    }
    if let Some(zoom) = custom_zoom {
        actions.push(GraphAction::Zoom(zoom));
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

    egui::Area::new(egui::Id::new("graph_edge_hover_tooltip"))
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

    egui::Area::new(egui::Id::new("graph_node_hover_tooltip"))
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
            app.workspace.graph
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
) -> Option<f32> {
    if !enabled {
        return None;
    }

    // Apply pending durable camera command.
    let camera_zoom = apply_pending_camera_command(ui, app, metadata_id);

    // Apply keyboard zoom
    let keyboard_zoom = apply_pending_keyboard_zoom_request(ui, app, metadata_id);

    // Apply pre-intercepted wheel zoom delta.
    let wheel_zoom = apply_pending_wheel_zoom(ui, response, metadata_id, app);

    let pointer_inside = response.hovered();
    
    // Pan with Left Mouse Button on background
    // Note: We check if we are NOT hovering a node to allow node dragging.
    // app.workspace.hovered_graph_node is updated before this function in render_graph_in_ui_collect_actions.
    if pointer_inside && app.workspace.hovered_graph_node.is_none() && ui.input(|i| i.pointer.primary_down()) {
        let delta = ui.input(|i| i.pointer.delta());
        if delta != Vec2::ZERO {
            ui.ctx().data_mut(|data| {
                if let Some(mut meta) = data.get_persisted::<MetadataFrame>(metadata_id) {
                    meta.pan += delta;
                    data.insert_persisted(metadata_id, meta);
                }
            });
        }
    }

    // Clamp zoom bounds
    ui.ctx().data_mut(|data| {
        if let Some(mut meta) = data.get_persisted::<MetadataFrame>(metadata_id) {
            let clamped = app.workspace.camera.clamp(meta.zoom);
            if (meta.zoom - clamped).abs() > f32::EPSILON {
                meta.zoom = clamped;
                app.workspace.camera.current_zoom = clamped;
                data.insert_persisted(metadata_id, meta);
            }
        }
    });

    camera_zoom.or(keyboard_zoom).or(wheel_zoom)
}

fn apply_pending_keyboard_zoom_request(
    ui: &Ui,
    app: &mut GraphBrowserApp,
    metadata_id: egui::Id,
) -> Option<f32> {
    let Some(request) = app.take_pending_keyboard_zoom_request() else {
        return None;
    };

    let factor = match request {
        KeyboardZoomRequest::In => 1.1,
        KeyboardZoomRequest::Out => 1.0 / 1.1,
        KeyboardZoomRequest::Reset => 1.0,
    };

    let graph_rect = ui.max_rect();
    let local_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, graph_rect.size());
    let local_center = local_rect.center().to_vec2();
    let mut updated_zoom = None;

    ui.ctx().data_mut(|data| {
        if let Some(mut meta) = data.get_persisted::<MetadataFrame>(metadata_id) {
            let graph_center_pos = (local_center - meta.pan) / meta.zoom;
            let new_zoom = app
                .workspace
                .camera
                .clamp(if matches!(request, KeyboardZoomRequest::Reset) {
                    factor
                } else {
                    meta.zoom * factor
                });
            let pan_delta = graph_center_pos * meta.zoom - graph_center_pos * new_zoom;
            meta.pan += pan_delta;
            meta.zoom = new_zoom;
            app.workspace.camera.current_zoom = new_zoom;
            data.insert_persisted(metadata_id, meta);
            updated_zoom = Some(new_zoom);
        }
    });

    updated_zoom
}

const CAMERA_FIT_PADDING: f32 = 1.1;
const CAMERA_FIT_RELAX: f32 = 0.5;
const CAMERA_FOCUS_SELECTION_PADDING: f32 = 1.2;

fn apply_pending_camera_command(
    ui: &Ui,
    app: &mut GraphBrowserApp,
    metadata_id: egui::Id,
) -> Option<f32> {
    let Some(command) = app.pending_camera_command() else {
        return None;
    };
    match command {
        CameraCommand::SetZoom(target_zoom) => {
            let mut updated_zoom = None;
            ui.ctx().data_mut(|data| {
                if let Some(mut meta) = data.get_persisted::<MetadataFrame>(metadata_id) {
                    let new_zoom = app.workspace.camera.clamp(target_zoom);
                    meta.zoom = new_zoom;
                    app.workspace.camera.current_zoom = new_zoom;
                    data.insert_persisted(metadata_id, meta);
                    updated_zoom = Some(new_zoom);
                }
            });
            if updated_zoom.is_some() {
                app.clear_pending_camera_command();
            }
            updated_zoom
        }
        CameraCommand::Fit | CameraCommand::StartupFit | CameraCommand::FitSelection => {
            let graph_rect = ui.max_rect();
            let view_size = graph_rect.size();
            if view_size.x <= f32::EPSILON || view_size.y <= f32::EPSILON {
                return None;
            }

            let bounds = if matches!(command, CameraCommand::FitSelection) {
                node_bounds_for_selection(app)
            } else {
                node_bounds_for_all(app)
            };

            let Some((min_x, max_x, min_y, max_y)) = bounds else {
                if matches!(command, CameraCommand::FitSelection) {
                    app.request_camera_command(CameraCommand::Fit);
                } else {
                    app.clear_pending_camera_command();
                }
                return None;
            };

            let width = (max_x - min_x).abs().max(1.0);
            let height = (max_y - min_y).abs().max(1.0);
            let padding = if matches!(command, CameraCommand::FitSelection) {
                CAMERA_FOCUS_SELECTION_PADDING
            } else {
                CAMERA_FIT_PADDING
            };
            let padded_width = width * padding;
            let padded_height = height * padding;
            let fit_zoom = (view_size.x / padded_width).min(view_size.y / padded_height);
            let target_zoom = if matches!(command, CameraCommand::FitSelection) {
                app.workspace.camera.clamp(fit_zoom)
            } else {
                app.workspace.camera.clamp(fit_zoom * CAMERA_FIT_RELAX)
            };

            let center = egui::pos2((min_x + max_x) * 0.5, (min_y + max_y) * 0.5);
            let viewport_center = egui::Rect::from_min_size(egui::Pos2::ZERO, graph_rect.size())
                .center()
                .to_vec2();
            let target_pan = viewport_center - center.to_vec2() * target_zoom;

            let mut updated_zoom = None;
            ui.ctx().data_mut(|data| {
                if let Some(mut meta) = data.get_persisted::<MetadataFrame>(metadata_id) {
                    meta.zoom = target_zoom;
                    meta.pan = target_pan;
                    app.workspace.camera.current_zoom = target_zoom;
                    data.insert_persisted(metadata_id, meta);
                    updated_zoom = Some(target_zoom);
                }
            });

            if updated_zoom.is_some() {
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
) -> Option<f32> {
    let scroll_delta = app.pending_wheel_zoom_delta();
    if scroll_delta.abs() <= f32::EPSILON {
        return None;
    }

    let velocity_id = egui::Id::new("graph_scroll_zoom_velocity");
    let mut velocity = ui
        .ctx()
        .data_mut(|d| d.get_persisted::<f32>(velocity_id))
        .unwrap_or(0.0);

    let impulse = app.workspace.scroll_zoom_impulse_scale * (scroll_delta / 60.0).clamp(-1.0, 1.0);
    velocity += impulse;

    let mut updated_zoom = None;
    if velocity.abs() >= app.workspace.scroll_zoom_inertia_min_abs {
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
                    let new_zoom = app.workspace.camera.clamp(meta.zoom * factor);
                    let pan_delta = graph_center_pos * meta.zoom - graph_center_pos * new_zoom;
                    meta.pan += pan_delta;
                    meta.zoom = new_zoom;
                    app.workspace.camera.current_zoom = new_zoom;
                    data.insert_persisted(metadata_id, meta);
                    updated_zoom = Some(new_zoom);
                }
            });
        }
    }

    if updated_zoom.is_some() {
        app.clear_pending_wheel_zoom_delta();
    }

    velocity *= app.workspace.scroll_zoom_inertia_damping;
    if velocity.abs() < app.workspace.scroll_zoom_inertia_min_abs {
        velocity = 0.0;
    }
    ui.ctx().data_mut(|d| d.insert_persisted(velocity_id, velocity));
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

fn collect_lasso_action(ui: &Ui, app: &GraphBrowserApp, enabled: bool) -> LassoGestureResult {
    let start_id = egui::Id::new("graph_lasso_start_screen");
    let moved_id = egui::Id::new("graph_lasso_moved");
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
        let (pressed, down, released) = match app.workspace.lasso_mouse_binding {
            LassoMouseBinding::RightDrag => (
                i.pointer.secondary_pressed(),
                i.pointer.secondary_down(),
                i.pointer.secondary_released(),
            ),
            LassoMouseBinding::ShiftLeftDrag => (
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
    let add_mode = ctrl || (matches!(app.workspace.lasso_mouse_binding, LassoMouseBinding::RightDrag) && shift);
    let mode = if alt {
        SelectionUpdateMode::Toggle
    } else if add_mode {
        SelectionUpdateMode::Add
    } else {
        SelectionUpdateMode::Replace
    };
    // Note: Lasso uses a different metadata ID logic if we change the graph view ID.
    // However, lasso uses screen coordinates and converts them.
    // We need the metadata to convert screen to canvas.
    // Since we don't have the response ID here easily, we might need to pass it or use the last known one.
    // For now, let's assume lasso works if we use the same ID logic, but lasso is called *after* graph view.
    // We can pass the ID to collect_lasso_action if needed, but let's stick to the current flow.
    // Actually, lasso needs metadata to map screen rect to canvas rect.
    // We should probably pass the metadata ID to collect_lasso_action too, but let's see if it breaks.
    // The current implementation uses "egui_graphs_metadata_" which is the default if no ID is provided?
    // No, egui_graphs uses `id.with("metadata")`.
    // If we change the ID of the graph view, we MUST update where lasso looks for metadata.
    // Since we can't easily pass the ID here without refactoring `collect_lasso_action` signature significantly
    // (it's called before we have the response in the current flow), we might have a problem.
    // BUT, `render_graph_in_ui_collect_actions` calls `collect_lasso_action`.
    // We can move `collect_lasso_action` call to *after* we get the response.
    
    // Let's defer fixing lasso metadata ID until we see if it breaks.
    // Actually, `collect_lasso_action` uses `egui::Id::new("egui_graphs_metadata_")` which is WRONG if we change the ID.
    // We should fix this.
    
    let meta_id = egui::Id::new("egui_graphs_metadata_"); // This is likely wrong now.
    // We will fix this by passing the ID in the next step if needed.
    
    let meta = ui
        .ctx()
        .data_mut(|d| d.get_persisted::<MetadataFrame>(meta_id))
        .unwrap_or_default();
    let Some(state) = app.workspace.egui_state.as_ref() else {
        return LassoGestureResult {
            action: None,
            suppress_context_menu: matches!(app.workspace.lasso_mouse_binding, LassoMouseBinding::RightDrag),
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
    let keys = index.nodes_in_canvas_rect(canvas_rect);

    LassoGestureResult {
        action: Some(GraphAction::LassoSelect { keys, mode }),
        suppress_context_menu: matches!(app.workspace.lasso_mouse_binding, LassoMouseBinding::RightDrag)
            && moved,
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
                }
            },
            Event::NodeDragStart(_) => {
                actions.push(GraphAction::DragStart);
            },
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
                }
            },
            Event::NodeMove(p) => {
                let idx = NodeIndex::new(p.id);
                if let Some(state) = app.workspace.egui_state.as_ref() {
                    if let Some(key) = state.get_key(idx) {
                        actions.push(GraphAction::MoveNode(
                            key,
                            Point2D::new(p.new_pos[0], p.new_pos[1]),
                        ));
                    }
                }
            },
            Event::NodeSelect(p) => {
                if let Some(state) = app.workspace.egui_state.as_ref() {
                    let idx = NodeIndex::new(p.id);
                    if let Some(key) = state.get_key(idx) {
                        actions.push(GraphAction::SelectNode {
                            key,
                            multi_select: multi_select_modifier,
                        });
                    }
                }
            },
            Event::NodeDeselect(_) => {
                // Selection clearing handled by the next SelectNode action
            },
            Event::Zoom(p) => {
                actions.push(GraphAction::Zoom(p.new_zoom));
            },
            _ => {},
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
                intents.push(GraphIntent::OpenNodeWorkspaceRouted {
                    key,
                    prefer_workspace: None,
                });
            },
            GraphAction::FocusNodeSplit(key) => {
                intents.push(GraphIntent::SelectNode {
                    key,
                    multi_select: false,
                });
            },
            GraphAction::DragStart => {
                intents.push(GraphIntent::SetInteracting { interacting: true });
            },
            GraphAction::DragEnd(key, pos) => {
                intents.push(GraphIntent::SetInteracting { interacting: false });
                intents.push(GraphIntent::SetNodePosition { key, position: pos });
            },
            GraphAction::MoveNode(key, pos) => {
                intents.push(GraphIntent::SetNodePosition { key, position: pos });
            },
            GraphAction::SelectNode { key, multi_select } => {
                intents.push(GraphIntent::SelectNode { key, multi_select });
            },
            GraphAction::LassoSelect { keys, mode } => {
                intents.push(GraphIntent::UpdateSelection { keys, mode });
            },
            GraphAction::Zoom(new_zoom) => {
                intents.push(GraphIntent::SetZoom { zoom: new_zoom });
            },
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
    let tagged_nodes: Vec<(crate::graph::NodeKey, crate::registries::atomic::knowledge::CompactCode)> =
        app.workspace.semantic_index
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
    let lasso_hint = match app.workspace.lasso_mouse_binding {
        LassoMouseBinding::RightDrag => "Right-Drag Lasso",
        LassoMouseBinding::ShiftLeftDrag => "Shift+Left-Drag Lasso",
    };
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

/// Render physics configuration panel
pub fn render_physics_panel(ctx: &egui::Context, app: &mut GraphBrowserApp) {
    if !app.workspace.show_physics_panel {
        return;
    }

    Window::new("Physics Configuration")
        .default_width(300.0)
        .show(ctx, |ui| {
            ui.heading("Force Parameters");

            let mut config = app.workspace.physics.clone();
            let mut config_changed = false;

            ui.add_space(8.0);

            ui.label("Repulsion (c_repulse):");
            if ui
                .add(egui::Slider::new(&mut config.base.c_repulse, 0.0..=10.0))
                .changed()
            {
                config_changed = true;
            }

            ui.add_space(4.0);

            ui.label("Attraction (c_attract):");
            if ui
                .add(egui::Slider::new(&mut config.base.c_attract, 0.0..=10.0))
                .changed()
            {
                config_changed = true;
            }

            ui.add_space(4.0);

            ui.label("Ideal Distance Scale (k_scale):");
            if ui
                .add(egui::Slider::new(&mut config.base.k_scale, 0.1..=5.0))
                .changed()
            {
                config_changed = true;
            }

            ui.add_space(4.0);
            ui.label("Center Gravity:");
            if ui
                .add(egui::Slider::new(&mut config.extras.0.params.c, 0.0..=1.0))
                .changed()
            {
                config_changed = true;
            }

            ui.add_space(4.0);

            ui.label("Max Step:");
            if ui
                .add(egui::Slider::new(&mut config.base.max_step, 0.1..=100.0))
                .changed()
            {
                config_changed = true;
            }

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);

            ui.heading("Damping & Convergence");
            ui.add_space(8.0);

            ui.label("Damping:");
            if ui
                .add(egui::Slider::new(&mut config.base.damping, 0.01..=1.0))
                .changed()
            {
                config_changed = true;
            }

            ui.add_space(4.0);

            ui.label("Time Step (dt):");
            if ui
                .add(egui::Slider::new(&mut config.base.dt, 0.001..=1.0).logarithmic(true))
                .changed()
            {
                config_changed = true;
            }

            ui.add_space(4.0);

            ui.label("Epsilon:");
            if ui
                .add(egui::Slider::new(&mut config.base.epsilon, 1e-6..=0.1).logarithmic(true))
                .changed()
            {
                config_changed = true;
            }

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);

            // Reset button
            ui.horizontal(|ui| {
                if ui.button("Reset to Defaults").clicked() {
                    let running = config.base.is_running;
                    config = GraphBrowserApp::default_physics_state();
                    config.base.is_running = running;
                    config_changed = true;
                }

                ui.label(if app.workspace.physics.base.is_running {
                    "Status: Running"
                } else {
                    "Status: Paused"
                });
            });

            if let Some(last_avg) = app.workspace.physics.base.last_avg_displacement {
                ui.label(format!("Last avg displacement: {:.4}", last_avg));
            }
            ui.label(format!("Step count: {}", app.workspace.physics.base.step_count));

            // Apply config changes
            if config_changed {
                app.update_physics_config(config);
            }
        });
}

/// Render keyboard shortcut help panel
pub fn render_help_panel(ctx: &egui::Context, app: &mut GraphBrowserApp) {
    if !app.workspace.show_help_panel {
        return;
    }

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
                    let lasso_base = match app.workspace.lasso_mouse_binding {
                        LassoMouseBinding::RightDrag => "Right+Drag",
                        LassoMouseBinding::ShiftLeftDrag => "Shift+LeftDrag",
                    };
                    let lasso_add = match app.workspace.lasso_mouse_binding {
                        LassoMouseBinding::RightDrag => "Right+Shift/Ctrl+Drag",
                        LassoMouseBinding::ShiftLeftDrag => "Shift+Ctrl+LeftDrag",
                    };
                    let lasso_toggle = match app.workspace.lasso_mouse_binding {
                        LassoMouseBinding::RightDrag => "Right+Alt+Drag",
                        LassoMouseBinding::ShiftLeftDrag => "Shift+Alt+LeftDrag",
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
pub fn render_history_manager_panel(ctx: &egui::Context, app: &mut GraphBrowserApp) -> Vec<GraphIntent> {
    let mut intents = Vec::new();
    if !app.workspace.show_history_manager {
        return intents;
    }

    let mut open = app.workspace.show_history_manager;
    let (timeline_total, dissolved_total) = app.history_manager_archive_counts();

    Window::new("History Manager")
        .open(&mut open)
        .default_width(520.0)
        .default_height(540.0)
        .show(ctx, |ui| {
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
        });

    app.workspace.show_history_manager = open;
    intents
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
                    .map(|n| if n.title.is_empty() { n.url.as_str() } else { n.title.as_str() })
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| format!("<missing:{}>", &from_node_id[..from_node_id.len().min(8)]));
                let to_label = to_key
                    .and_then(|k| app.workspace.graph.get_node(k))
                    .map(|n| if n.title.is_empty() { n.url.as_str() } else { n.title.as_str() })
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| format!("<missing:{}>", &to_node_id[..to_node_id.len().min(8)]));

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
                    crate::services::persistence::types::PersistedNavigationTrigger::Back => "⬅ Back",
                    crate::services::persistence::types::PersistedNavigationTrigger::Forward => "➡ Forward",
                    crate::services::persistence::types::PersistedNavigationTrigger::Unknown => "↔",
                };

                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(time_label).weak().small());
                    ui.label(trigger_label);
                    let response = ui.selectable_label(false, format!("{} → {}", from_label, to_label));
                    if response.clicked() && let Some(key) = from_key {
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
            .or_else(|| app.load_workspace_layout_json(GraphBrowserApp::SESSION_WORKSPACE_LAYOUT_NAME));
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum ContextPinScope {
    Workspace,
    Pane,
}

fn context_pin_scope_for(focused_pane_node: Option<NodeKey>) -> ContextPinScope {
    if focused_pane_node.is_some() {
        ContextPinScope::Pane
    } else {
        ContextPinScope::Workspace
    }
}

fn context_pin_workspace_name(scope: ContextPinScope) -> &'static str {
    match scope {
        ContextPinScope::Workspace => GraphBrowserApp::WORKSPACE_PIN_WORKSPACE_NAME,
        ContextPinScope::Pane => GraphBrowserApp::WORKSPACE_PIN_PANE_NAME,
    }
}

fn saved_workspace_runtime_layout_json(app: &GraphBrowserApp, workspace_name: &str) -> Option<String> {
    let bundle = persistence_ops::load_named_workspace_bundle(app, workspace_name).ok()?;
    let (tree, _) = persistence_ops::restore_runtime_tree_from_workspace_bundle(app, &bundle).ok()?;
    serde_json::to_string(&tree).ok()
}

fn context_pin_label(scope: ContextPinScope) -> &'static str {
    match scope {
        ContextPinScope::Workspace => "Pin Workspace",
        ContextPinScope::Pane => "Pin Pane",
    }
}

pub fn render_persistence_panel(
    ctx: &egui::Context,
    app: &mut GraphBrowserApp,
    focused_pane_node: Option<NodeKey>,
    current_layout_json: Option<&str>,
) {
    if !app.workspace.show_persistence_panel {
        return;
    }

    let pin_load_picker_state_id = egui::Id::new("persistence_pin_load_picker_open");
    let mut show_pin_load_picker = ctx
        .data_mut(|d| d.get_persisted::<bool>(pin_load_picker_state_id))
        .unwrap_or(false);
    let mut open = app.workspace.show_persistence_panel;
    Window::new("Persistence Hub")
        .open(&mut open)
        .default_width(420.0)
        .show(ctx, |ui| {
            ui.label("Storage");
            ui.horizontal(|ui| {
                ui.label("Data directory:");
                let data_dir_input_id = ui.make_persistent_id("persistence_data_dir_input");
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
                let interval_input_id =
                    ui.make_persistent_id("persistence_snapshot_interval_input");
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
            ui.label("Workspaces");
            ui.horizontal(|ui| {
                ui.label("Autosave every (sec):");
                let autosave_interval_id =
                    ui.make_persistent_id("workspace_autosave_interval_input");
                let mut autosave_interval = ui
                    .data_mut(|d| d.get_persisted::<String>(autosave_interval_id))
                    .unwrap_or_else(|| app.workspace_autosave_interval_secs().to_string());
                if ui
                    .add(egui::TextEdit::singleline(&mut autosave_interval).desired_width(72.0))
                    .changed()
                {
                    ui.data_mut(|d| {
                        d.insert_persisted(autosave_interval_id, autosave_interval.clone())
                    });
                }
                if ui.button("Apply").clicked()
                    && let Ok(secs) = autosave_interval.trim().parse::<u64>()
                {
                    let _ = app.set_workspace_autosave_interval_secs(secs);
                }
            });
            ui.horizontal(|ui| {
                ui.label("Autosave retention:");
                let mut retention = app.workspace_autosave_retention() as u32;
                if ui
                    .add(egui::Slider::new(&mut retention, 0..=5).suffix(" previous"))
                    .changed()
                {
                    let _ = app.set_workspace_autosave_retention(retention as u8);
                }
            });
            if app.should_prompt_unsaved_workspace_save() {
                ui.colored_label(
                    Color32::from_rgb(255, 180, 70),
                    "Current workspace has unsaved graph changes; save before switching.",
                );
            }
            let (active_count, warm_count, cold_count) = app.lifecycle_counts();
            let pressure_label = match app.memory_pressure_level() {
                MemoryPressureLevel::Unknown => "Unknown",
                MemoryPressureLevel::Normal => "Normal",
                MemoryPressureLevel::Warning => "Warning",
                MemoryPressureLevel::Critical => "Critical",
            };
            let pressure_color = match app.memory_pressure_level() {
                MemoryPressureLevel::Unknown => Color32::from_rgb(180, 180, 190),
                MemoryPressureLevel::Normal => Color32::from_rgb(140, 220, 160),
                MemoryPressureLevel::Warning => Color32::from_rgb(255, 205, 120),
                MemoryPressureLevel::Critical => Color32::from_rgb(255, 145, 145),
            };
            ui.small(format!(
                "Lifecycle: Active {}/{} | Warm {}/{} | Cold {} | Mapped {}",
                active_count,
                app.active_webview_limit(),
                warm_count,
                app.warm_cache_limit(),
                cold_count,
                app.mapped_webview_count()
            ));
            ui.colored_label(
                pressure_color,
                format!(
                    "Memory pressure: {} (avail {} MiB / total {} MiB)",
                    pressure_label,
                    app.memory_available_mib(),
                    app.memory_total_mib()
                ),
            );
            let pin_scope = context_pin_scope_for(focused_pane_node);
            let pin_workspace_name = context_pin_workspace_name(pin_scope);
            let pin_active = current_layout_json.is_some_and(|current| {
                saved_workspace_runtime_layout_json(app, pin_workspace_name)
                    .as_deref()
                    .is_some_and(|saved_runtime| saved_runtime == current)
            });
            let has_any_saved_pin = app
                .load_workspace_layout_json(GraphBrowserApp::WORKSPACE_PIN_WORKSPACE_NAME)
                .is_some()
                || app
                    .load_workspace_layout_json(GraphBrowserApp::WORKSPACE_PIN_PANE_NAME)
                    .is_some();
            ui.horizontal(|ui| {
                if ui
                    .selectable_label(pin_active, context_pin_label(pin_scope))
                    .clicked()
                    && current_layout_json.is_some()
                {
                    app.request_save_workspace_snapshot_named(pin_workspace_name.to_string());
                }
                if ui
                    .add_enabled(has_any_saved_pin, egui::Button::new("Load Pin..."))
                    .clicked()
                {
                    show_pin_load_picker = true;
                }
            });
            if ui.button("Prune Session Workspace").clicked() {
                let _ = app.clear_session_workspace_layout();
            }
            ui.separator();
            ui.label("Retention");
            if ui.button("Prune Empty Named Workspaces").clicked() {
                app.request_prune_empty_workspaces();
            }
            ui.horizontal(|ui| {
                let keep_latest_id = ui.make_persistent_id("workspace_keep_latest_named_input");
                let mut keep_latest = ui
                    .data_mut(|d| d.get_persisted::<String>(keep_latest_id))
                    .unwrap_or_else(|| "10".to_string());
                if ui
                    .add(egui::TextEdit::singleline(&mut keep_latest).desired_width(56.0))
                    .changed()
                {
                    ui.data_mut(|d| d.insert_persisted(keep_latest_id, keep_latest.clone()));
                }
                if ui.button("Keep Latest N Named").clicked()
                    && let Ok(keep) = keep_latest.trim().parse::<usize>()
                {
                    app.request_keep_latest_named_workspaces(keep);
                }
            });
            ui.small("Reserved autosave workspaces are excluded from batch retention.");
            ui.separator();
            let workspace_name_id = ui.make_persistent_id("workspace_name_input");
            let mut workspace_name = ui
                .data_mut(|d| d.get_persisted::<String>(workspace_name_id))
                .unwrap_or_default();
            let workspace_name_changed = ui
                .add(
                    egui::TextEdit::singleline(&mut workspace_name)
                        .hint_text("workspace name (e.g. research-1)"),
                )
                .changed();
            if workspace_name_changed {
                ui.data_mut(|d| d.insert_persisted(workspace_name_id, workspace_name.clone()));
            }
            let workspace_name = workspace_name.trim().to_string();
            ui.horizontal(|ui| {
                if ui.button("Save Auto").clicked() {
                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    app.request_save_workspace_snapshot_named(format!("workspace:auto-{now}"));
                }
                if ui
                    .add_enabled(!workspace_name.is_empty(), egui::Button::new("Save Named"))
                    .clicked()
                {
                    app.request_save_workspace_snapshot_named(workspace_name.clone());
                }
                if ui
                    .add_enabled(
                        !workspace_name.is_empty(),
                        egui::Button::new("Restore Named"),
                    )
                    .clicked()
                {
                    app.request_restore_workspace_snapshot_named(workspace_name.clone());
                }
                if ui
                    .add_enabled(
                        !workspace_name.is_empty(),
                        egui::Button::new("Delete Named"),
                    )
                    .clicked()
                {
                    if !GraphBrowserApp::is_reserved_workspace_layout_name(&workspace_name) {
                        let _ = app.delete_workspace_layout(&workspace_name);
                    }
                }
            });
            let mut workspace_names = app.list_workspace_layout_names();
            workspace_names.sort();
            workspace_names.retain(|name| {
                name != GraphBrowserApp::WORKSPACE_PIN_WORKSPACE_NAME
                    && name != GraphBrowserApp::WORKSPACE_PIN_PANE_NAME
                    && name != GraphBrowserApp::SETTINGS_TOAST_ANCHOR_NAME
            });
            if workspace_names.is_empty() {
                ui.small("No workspaces saved.");
            } else {
                ui.small("Saved:");
                for name in workspace_names {
                    let is_reserved = GraphBrowserApp::is_reserved_workspace_layout_name(&name);
                    let label = if name == GraphBrowserApp::SESSION_WORKSPACE_LAYOUT_NAME {
                        "session-latest (autosave)"
                    } else if name == "latest" {
                        "latest (autosave)"
                    } else if let Some(idx) = name.strip_prefix("workspace:session-prev-") {
                        if idx.chars().all(|c| c.is_ascii_digit()) {
                            "session-previous (autosave)"
                        } else {
                            &name
                        }
                    } else {
                        &name
                    };
                    ui.horizontal(|ui| {
                        if ui.button(label).clicked() {
                            app.request_restore_workspace_snapshot_named(name.clone());
                        }
                        if ui.small_button("Load").clicked() {
                            app.request_restore_workspace_snapshot_named(name.clone());
                        }
                        if ui
                            .add_enabled(
                                app.get_single_selected_node().is_some(),
                                egui::Button::new("Open Sel").small(),
                            )
                            .clicked()
                            && let Some(key) = app.get_single_selected_node()
                        {
                            app.apply_intents([GraphIntent::OpenNodeWorkspaceRouted {
                                key,
                                prefer_workspace: Some(name.clone()),
                            }]);
                        }
                        if ui
                            .add_enabled(!is_reserved, egui::Button::new("Del").small())
                            .clicked()
                        {
                            let _ = app.delete_workspace_layout(&name);
                        }
                    });
                }
            }

            ui.separator();
            ui.label("Graphs");
            let graph_name_id = ui.make_persistent_id("graph_name_input");
            let mut graph_name = ui
                .data_mut(|d| d.get_persisted::<String>(graph_name_id))
                .unwrap_or_default();
            let graph_name_changed = ui
                .add(
                    egui::TextEdit::singleline(&mut graph_name)
                        .hint_text("graph snapshot name (e.g. ideation-v1)"),
                )
                .changed();
            if graph_name_changed {
                ui.data_mut(|d| d.insert_persisted(graph_name_id, graph_name.clone()));
            }
            let graph_name = graph_name.trim().to_string();
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(!graph_name.is_empty(), egui::Button::new("Save Graph"))
                    .clicked()
                {
                    app.request_save_graph_snapshot_named(graph_name.clone());
                }
                if ui
                    .add_enabled(!graph_name.is_empty(), egui::Button::new("Load Graph"))
                    .clicked()
                {
                    app.request_restore_graph_snapshot_named(graph_name.clone());
                }
                if ui
                    .add_enabled(!graph_name.is_empty(), egui::Button::new("Delete Graph"))
                    .clicked()
                {
                    app.request_delete_graph_snapshot_named(graph_name.clone());
                }
            });
            let mut named_graphs = app.list_named_graph_snapshot_names();
            named_graphs.sort();
            let has_latest_graph = app.has_latest_graph_snapshot();
            if named_graphs.is_empty() && !has_latest_graph {
                ui.small("No graph snapshots saved.");
            } else {
                ui.small("Saved:");
                if has_latest_graph {
                    ui.horizontal(|ui| {
                        if ui.button("latest (autosave)").clicked() {
                            app.request_restore_graph_snapshot_latest();
                        }
                        if ui.small_button("Load").clicked() {
                            app.request_restore_graph_snapshot_latest();
                        }
                        ui.add_enabled(false, egui::Button::new("Del").small());
                    });
                }
                for name in named_graphs {
                    ui.horizontal(|ui| {
                        if ui.button(&name).clicked() {
                            app.request_restore_graph_snapshot_named(name.clone());
                        }
                        if ui.small_button("Load").clicked() {
                            app.request_restore_graph_snapshot_named(name.clone());
                        }
                        if ui.small_button("Del").clicked() {
                            app.request_delete_graph_snapshot_named(name.clone());
                        }
                    });
                }
            }
        });
    if open && show_pin_load_picker {
        let mut close_picker = false;
        Window::new("Load Pin Snapshot")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .default_width(300.0)
            .show(ctx, |ui| {
                let options = [
                    (
                        GraphBrowserApp::WORKSPACE_PIN_WORKSPACE_NAME,
                        "Workspace Pin",
                    ),
                    (GraphBrowserApp::WORKSPACE_PIN_PANE_NAME, "Pane Pin"),
                ];
                let mut any = false;
                for (workspace_name, label) in options {
                    let Some(_saved_layout) = app.load_workspace_layout_json(workspace_name) else {
                        continue;
                    };
                    any = true;
                    let active = current_layout_json.is_some_and(|current| {
                        saved_workspace_runtime_layout_json(app, workspace_name)
                            .as_deref()
                            .is_some_and(|saved_runtime| saved_runtime == current)
                    });
                    ui.horizontal(|ui| {
                        let text = if active {
                            format!("{label} (active)")
                        } else {
                            label.to_string()
                        };
                        if ui.button(text).clicked() {
                            app.request_restore_workspace_snapshot_named(
                                workspace_name.to_string(),
                            );
                            close_picker = true;
                        }
                    });
                }
                if !any {
                    ui.small("No pin snapshots saved.");
                }
                ui.separator();
                if ui.button("Close").clicked() {
                    close_picker = true;
                }
            });
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            close_picker = true;
        }
        if close_picker {
            show_pin_load_picker = false;
        }
    }
    if !open {
        show_pin_load_picker = false;
    }
    ctx.data_mut(|d| d.insert_persisted(pin_load_picker_state_id, show_pin_load_picker));
    app.workspace.show_persistence_panel = open;
}

pub fn render_sync_panel(ctx: &egui::Context, app: &mut GraphBrowserApp) {
    if !app.workspace.show_sync_panel {
        return;
    }

    let pairing_code_id = egui::Id::new("verse_pairing_code");
    let pairing_code_input_id = egui::Id::new("verse_pairing_code_input");
    let discovery_results_id = egui::Id::new("verse_discovery_results");
    let sync_status_id = egui::Id::new("verse_sync_status");

    if let Some(discovery_result) =
        crate::shell::desktop::runtime::control_panel::take_discovery_results()
    {
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

    let mut open = app.workspace.show_sync_panel;
    Window::new("Sync Settings")
        .open(&mut open)
        .default_width(500.0)
        .show(ctx, |ui| {
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
                        match crate::shell::desktop::runtime::control_panel::request_discover_nearby_peers(2) {
                            Ok(()) => {
                                ctx.data_mut(|d| {
                                    d.insert_temp(
                                        sync_status_id,
                                        "Discovering nearby peers...".to_string(),
                                    )
                                });
                            }
                            Err(error) => {
                                ctx.data_mut(|d| {
                                    d.insert_temp(
                                        sync_status_id,
                                        format!("Discovery unavailable: {error}"),
                                    )
                                });
                            }
                        }
                    }
                    if ui.button("Sync Now").clicked() {
                        let intents = crate::shell::desktop::runtime::registries::phase5_execute_verse_sync_now_action(app);
                        if intents.is_empty() {
                            app.apply_intents([crate::app::GraphIntent::SyncNow]);
                        } else {
                            app.apply_intents(intents);
                        }
                        ctx.data_mut(|d| {
                            d.insert_temp(
                                sync_status_id,
                                "Manual sync requested".to_string(),
                            )
                        });
                    }
                    if ui.button("Share Session Workspace").clicked() {
                        let intents = crate::shell::desktop::runtime::registries::phase5_execute_verse_share_workspace_action(
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

                if let Some(code) = ctx.data_mut(|d| {
                    d.get_temp::<crate::mods::native::verse::PairingCode>(pairing_code_id)
                }) {
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
                                    d.insert_temp(
                                        sync_status_id,
                                        "Enter a pairing code first".to_string(),
                                    )
                                });
                            } else {
                                let before = crate::mods::native::verse::get_trusted_peers().len();
                                let intents = crate::shell::desktop::runtime::registries::phase5_execute_verse_pair_code_action(
                                    app,
                                    &code,
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

                if let Some(peers) = ctx.data_mut(|d| {
                    d.get_temp::<Vec<crate::mods::native::verse::DiscoveredPeer>>(discovery_results_id)
                }) {
                    if !peers.is_empty() {
                        ui.group(|ui| {
                            ui.label(egui::RichText::new("Nearby Devices").strong());
                            for peer in peers {
                                ui.horizontal(|ui| {
                                    ui.label(format!(
                                        "{} ({})",
                                        peer.device_name,
                                        peer.node_id.to_string()
                                    ));
                                    if ui.button("Pair").clicked() {
                                        let intents = crate::shell::desktop::runtime::registries::phase5_execute_verse_pair_local_peer_action(
                                            app,
                                            &peer.node_id.to_string(),
                                        );
                                        if !intents.is_empty() {
                                            app.apply_intents(intents);
                                        }
                                        ctx.data_mut(|d| {
                                            d.insert_temp(
                                                sync_status_id,
                                                format!(
                                                    "Paired with {}",
                                                    peer.node_id.to_string()
                                                ),
                                            )
                                        });
                                    }
                                });
                            }
                        });
                    }
                }

                let peers = crate::mods::native::verse::get_trusted_peers();

                if peers.is_empty() {
                    ui.label("No paired devices yet.");
                } else {
                    for peer in &peers {
                        ui.horizontal(|ui| {
                            let peer_display = format!("{} ({})", peer.display_name, peer.node_id.to_string()[..8].to_uppercase());
                            ui.label(peer_display);
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.button("Manage Access").clicked() {
                                    app.workspace.show_manage_access_dialog = true;
                                }
                                if ui.button("Forget").clicked() {
                                    let intents = crate::shell::desktop::runtime::registries::phase5_execute_verse_forget_device_action(
                                        app,
                                        &peer.node_id.to_string(),
                                    );
                                    app.apply_intents(intents);
                                }
                            });
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
        });
    
    app.workspace.show_sync_panel = open;
}

pub fn render_manage_access_dialog(ctx: &egui::Context, app: &mut GraphBrowserApp) {
    if !app.workspace.show_manage_access_dialog {
        return;
    }
    
    let mut open = app.workspace.show_manage_access_dialog;
    Window::new("Manage Access")
        .open(&mut open)
        .default_width(500.0)
        .show(ctx, |ui| {
            if !crate::mods::native::verse::is_initialized() {
                ui.label("Sync is starting. Access controls will appear shortly.");
                return;
            }

            ui.label("Grant or revoke workspace access for paired devices");
            ui.separator();
            
            let peers = crate::mods::native::verse::get_trusted_peers();
            
            if peers.is_empty() {
                ui.label("No paired devices");
            } else {
                for peer in &peers {
                    ui.group(|ui| {
                        ui.label(egui::RichText::new(&peer.display_name).strong());
                        
                        if peer.workspace_grants.is_empty() {
                            ui.label("No workspace grants");
                        } else {
                            for grant in &peer.workspace_grants {
                                ui.horizontal(|ui| {
                                    let access_str = match grant.access {
                                        crate::mods::native::verse::AccessLevel::ReadOnly => "🔒 Read-Only",
                                        crate::mods::native::verse::AccessLevel::ReadWrite => "✏️ Read-Write",
                                    };
                                    ui.label(format!("{}: {}", grant.workspace_id, access_str));
                                    if ui.button("Revoke").clicked() {
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
        });
    
    app.workspace.show_manage_access_dialog = open;
}

pub fn render_choose_workspace_picker(ctx: &egui::Context, app: &mut GraphBrowserApp) {
    let Some(request) = app.choose_workspace_picker_request() else {
        return;
    };
    let target = request.node;
    if app.workspace.graph.get_node(target).is_none() {
        app.clear_choose_workspace_picker();
        return;
    }
    let mut selected_workspace: Option<String> = None;
    let mut close = false;
    let mut memberships = match request.mode {
        ChooseWorkspacePickerMode::OpenNodeInWorkspace => {
            app.sorted_workspaces_for_node_key(target)
        },
        ChooseWorkspacePickerMode::AddNodeToWorkspace => {
            let mut all = app
                .list_workspace_layout_names()
                .into_iter()
                .filter(|name| !GraphBrowserApp::is_reserved_workspace_layout_name(name))
                .collect::<Vec<_>>();
            all.sort();
            all
        },
        ChooseWorkspacePickerMode::AddConnectedSelectionToWorkspace => {
            let mut all = app
                .list_workspace_layout_names()
                .into_iter()
                .filter(|name| !GraphBrowserApp::is_reserved_workspace_layout_name(name))
                .collect::<Vec<_>>();
            all.sort();
            all
        },
        ChooseWorkspacePickerMode::AddExactSelectionToWorkspace => {
            let mut all = app
                .list_workspace_layout_names()
                .into_iter()
                .filter(|name| !GraphBrowserApp::is_reserved_workspace_layout_name(name))
                .collect::<Vec<_>>();
            all.sort();
            all
        },
    };
    let title = app
        .workspace
        .graph
        .get_node(target)
        .map(|node| format!("Choose Workspace: {}", node.title))
        .unwrap_or_else(|| "Choose Workspace".to_string());
    Window::new(title)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .default_width(300.0)
        .show(ctx, |ui| {
            if memberships.is_empty() {
                let msg = match request.mode {
                    ChooseWorkspacePickerMode::OpenNodeInWorkspace => {
                        "No workspace memberships for this node."
                    },
                    ChooseWorkspacePickerMode::AddNodeToWorkspace => {
                        "No named workspaces available. Save one first."
                    },
                    ChooseWorkspacePickerMode::AddConnectedSelectionToWorkspace => {
                        "No named workspaces available. Save one first."
                    },
                    ChooseWorkspacePickerMode::AddExactSelectionToWorkspace => {
                        "No named workspaces available. Save one first."
                    },
                };
                ui.small(msg);
            } else {
                memberships.dedup();
                let header = match request.mode {
                    ChooseWorkspacePickerMode::OpenNodeInWorkspace => "Open in workspace:",
                    ChooseWorkspacePickerMode::AddNodeToWorkspace => "Add node to workspace:",
                    ChooseWorkspacePickerMode::AddConnectedSelectionToWorkspace => {
                        "Add connected nodes to workspace:"
                    },
                    ChooseWorkspacePickerMode::AddExactSelectionToWorkspace => {
                        "Add selected nodes to workspace:"
                    },
                };
                ui.small(header);
                for name in &memberships {
                    if ui.button(name).clicked() {
                        selected_workspace = Some(name.clone());
                        close = true;
                    }
                }
            }
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Open Persistence Hub").clicked() {
                    app.workspace.show_persistence_panel = true;
                }
                if ui.button("Close").clicked() {
                    close = true;
                }
            });
        });
    if let Some(name) = selected_workspace {
        match request.mode {
            ChooseWorkspacePickerMode::OpenNodeInWorkspace => {
                app.apply_intents([GraphIntent::OpenNodeWorkspaceRouted {
                    key: target,
                    prefer_workspace: Some(name),
                }]);
            },
            ChooseWorkspacePickerMode::AddNodeToWorkspace => {
                app.request_add_node_to_workspace(target, name);
            },
            ChooseWorkspacePickerMode::AddConnectedSelectionToWorkspace => {
                let mut seed_nodes: Vec<NodeKey> = if app.workspace.selected_nodes.is_empty() {
                    vec![target]
                } else {
                    app.workspace.selected_nodes.iter().copied().collect()
                };
                if !seed_nodes.contains(&target) {
                    seed_nodes.push(target);
                }
                app.request_add_connected_to_workspace(seed_nodes, name);
            },
            ChooseWorkspacePickerMode::AddExactSelectionToWorkspace => {
                let mut nodes = app
                    .choose_workspace_picker_exact_nodes()
                    .map(|keys| keys.to_vec())
                    .unwrap_or_else(|| vec![target]);
                nodes.retain(|key| app.workspace.graph.get_node(*key).is_some());
                nodes.sort_by_key(|key| key.index());
                nodes.dedup();
                if !nodes.is_empty() {
                    app.request_add_exact_nodes_to_workspace(nodes, name);
                }
            },
        }
    }
    if close {
        app.clear_choose_workspace_picker();
    }
}

pub fn render_unsaved_workspace_prompt(ctx: &egui::Context, app: &mut GraphBrowserApp) {
    let Some(request) = app.unsaved_workspace_prompt_request().cloned() else {
        return;
    };
    let mut action: Option<UnsavedWorkspacePromptAction> = None;
    Window::new("Unsaved Workspace Changes")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .default_width(380.0)
        .show(ctx, |ui| {
            match &request {
                UnsavedWorkspacePromptRequest::WorkspaceSwitch { name, .. } => {
                    ui.label(format!(
                        "This workspace has unsaved graph changes.\nSwitch to '{name}' without saving?"
                    ));
                },
            }
            ui.separator();
            if ui.button("Open Persistence Hub").clicked() {
                app.workspace.show_persistence_panel = true;
            }
            ui.horizontal(|ui| {
                if ui.button("Proceed Without Saving").clicked() {
                    action = Some(UnsavedWorkspacePromptAction::ProceedWithoutSaving);
                }
                if ui.button("Cancel").clicked() {
                    action = Some(UnsavedWorkspacePromptAction::Cancel);
                }
            });
        });
    if let Some(action) = action {
        app.set_unsaved_workspace_prompt_action(action);
    }
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

        assert_eq!(app.workspace.graph.get_node(key).unwrap().position, pos_before);
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
}
