/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Canvas overlay passes: frame-affinity backdrops, highlighted-edge overlay,
//! hovered-edge tooltip, and hovered-node tooltip.

use crate::app::{GraphBrowserApp, GraphViewId, SimulateBehaviorPreset};
use crate::graph::scene_runtime::{
    SceneRegionEffect, SceneRegionId, SceneRegionResizeHandle, SceneRegionRuntime, SceneRegionShape,
};
use crate::graph::{
    EdgeFamily, EdgePayload, FrameLayoutHint, NodeLifecycle, RelationSelector, SemanticSubKind,
    SplitOrientation,
};
use crate::shell::desktop::runtime::registries::phase3_resolve_active_theme;
use crate::util::VersoAddress;
use egui::{Stroke, Ui, Vec2};
use egui_graphs::MetadataFrame;
use std::collections::BTreeSet;
use std::time::{Duration, UNIX_EPOCH};

use super::graph_info::{
    active_graph_search_node_keys, filtered_view_node_keys, gather_node_keys_into_scene_region,
};

// ── Edge helpers ──────────────────────────────────────────────────────────────

fn edge_family_rows(payload: &EdgePayload) -> Vec<String> {
    let mut rows = Vec::new();
    if payload.has_relation(RelationSelector::Semantic(SemanticSubKind::Hyperlink)) {
        rows.push("hyperlink | durable | graph.link_extraction".to_string());
    }
    if payload.has_relation(RelationSelector::Family(EdgeFamily::Traversal)) {
        rows.push("history | durable | runtime.navigation_log".to_string());
    }
    if payload.has_relation(RelationSelector::Semantic(SemanticSubKind::UserGrouped)) {
        rows.push("user-grouped | durable | user.explicit_grouping".to_string());
    }
    if let Some(arrangement) = payload.arrangement_data() {
        for sub_kind in &arrangement.sub_kinds {
            rows.push(format!(
                "arrangement/{} | {} | {}",
                sub_kind.as_tag(),
                sub_kind.durability().as_tag(),
                sub_kind.provenance()
            ));
        }
    }
    if rows.is_empty() {
        rows.push("unknown | session | runtime.edge_probe".to_string());
    }
    rows
}

pub(super) fn edge_endpoints_at_pointer(
    ui: &Ui,
    app: &GraphBrowserApp,
    metadata_id: egui::Id,
) -> Option<(crate::graph::NodeKey, crate::graph::NodeKey)> {
    let pointer = ui.input(|i| i.pointer.latest_pos())?;
    let state = app.workspace.graph_runtime.egui_state.as_ref()?;
    let meta = ui
        .ctx()
        .data_mut(|d| d.get_persisted::<MetadataFrame>(metadata_id))?;
    let edge_id = state.graph.edge_by_screen_pos(&meta, pointer)?;
    state.graph.edge_endpoints(edge_id)
}

// ── Overlay draw calls ────────────────────────────────────────────────────────

/// Render semi-transparent backdrop rectangles for all active frame-affinity
/// regions, positioned below graph nodes.
///
/// Uses the previous-frame [`MetadataFrame`] for canvas→screen coordinate
/// conversion.  Falls back to raw canvas coordinates if no metadata is
/// available yet (first frame after session start).
///
/// Spec: `layout_behaviors_and_physics_spec.md §4.6`
pub(super) fn draw_frame_affinity_backdrops(
    ui: &mut Ui,
    app: &GraphBrowserApp,
    metadata_id: egui::Id,
) {
    let regions = crate::graph::frame_affinity::derive_frame_affinity_regions(app.domain_graph());
    if regions.is_empty() {
        return;
    }

    let meta = ui
        .ctx()
        .data_mut(|d| d.get_persisted::<MetadataFrame>(metadata_id));

    let Some(_egui_state) = app.workspace.graph_runtime.egui_state.as_ref() else {
        return;
    };

    let painter = ui.painter().clone().with_layer_id(egui::LayerId::new(
        egui::Order::Middle,
        egui::Id::new("frame_affinity_backdrops"),
    ));

    for region in &regions {
        let Some(backdrop_rect) = frame_affinity_backdrop_rect(app, region, meta.as_ref()) else {
            continue;
        };

        let fill = egui::Color32::from_rgba_unmultiplied(
            region.color.r(),
            region.color.g(),
            region.color.b(),
            if frame_anchor_is_selected_or_current(app, region.frame_anchor) {
                42
            } else {
                30
            },
        );
        let stroke_color = egui::Color32::from_rgba_unmultiplied(
            region.color.r(),
            region.color.g(),
            region.color.b(),
            if frame_anchor_is_selected_or_current(app, region.frame_anchor) {
                170
            } else {
                80
            },
        );
        let stroke_width = if frame_anchor_is_selected_or_current(app, region.frame_anchor) {
            2.5
        } else {
            1.5
        };

        painter.rect(
            backdrop_rect,
            egui::CornerRadius::same(8),
            fill,
            egui::Stroke::new(stroke_width, stroke_color),
            egui::StrokeKind::Outside,
        );

        // Frame label — rendered at top-left of the backdrop rect.
        if let Some(label) = frame_anchor_label(app, region.frame_anchor) {
            let label_pos = backdrop_rect.left_top() + egui::Vec2::new(6.0, 4.0);
            painter.text(
                label_pos,
                egui::Align2::LEFT_TOP,
                label,
                egui::FontId::proportional(11.0),
                egui::Color32::from_rgba_unmultiplied(
                    region.color.r(),
                    region.color.g(),
                    region.color.b(),
                    180,
                ),
            );
        }

        if let Some(indicator) = frame_anchor_split_indicator(app, region.frame_anchor) {
            let indicator_padding = egui::vec2(6.0, 3.0);
            let indicator_galley = painter.layout_no_wrap(
                indicator,
                egui::FontId::proportional(10.0),
                egui::Color32::from_rgba_unmultiplied(255, 255, 255, 220),
            );
            let indicator_size = indicator_galley.size() + indicator_padding * 2.0;
            let indicator_rect = egui::Rect::from_min_size(
                egui::pos2(
                    backdrop_rect.right() - indicator_size.x - 6.0,
                    backdrop_rect.top() + 4.0,
                ),
                indicator_size,
            );
            painter.rect(
                indicator_rect,
                egui::CornerRadius::same(6),
                egui::Color32::from_rgba_unmultiplied(
                    region.color.r(),
                    region.color.g(),
                    region.color.b(),
                    110,
                ),
                egui::Stroke::new(
                    1.0,
                    egui::Color32::from_rgba_unmultiplied(
                        region.color.r(),
                        region.color.g(),
                        region.color.b(),
                        180,
                    ),
                ),
                egui::StrokeKind::Outside,
            );
            painter.galley(
                indicator_rect.center() - indicator_galley.size() * 0.5,
                indicator_galley,
                egui::Color32::from_rgba_unmultiplied(255, 255, 255, 220),
            );
        }
    }
}

pub(super) fn draw_scene_runtime_backdrops(
    ui: &mut Ui,
    app: &GraphBrowserApp,
    view_id: GraphViewId,
    metadata_id: egui::Id,
) {
    let Some(runtime) = app.graph_view_scene_runtime(view_id) else {
        return;
    };

    if runtime.regions.is_empty() && runtime.bounds_override.is_none() {
        return;
    }

    let meta = ui
        .ctx()
        .data_mut(|d| d.get_persisted::<MetadataFrame>(metadata_id));
    let painter = ui.painter().clone().with_layer_id(egui::LayerId::new(
        egui::Order::Middle,
        egui::Id::new(("scene_runtime_backdrops", view_id)),
    ));
    let arrange_mode = app.graph_view_scene_mode(view_id) == crate::app::SceneMode::Arrange;
    let simulate_mode = app.graph_view_scene_mode(view_id) == crate::app::SceneMode::Simulate;
    let simulate_visuals = simulate_mode
        .then(|| simulate_visual_profile(app.graph_view_simulate_behavior_preset(view_id)));
    let pointer = ui.input(|i| i.pointer.latest_pos());
    let selected_region = arrange_mode
        .then(|| app.graph_view_selected_scene_region(view_id))
        .flatten();
    let hovered_region = arrange_mode
        .then(|| {
            app.workspace
                .graph_runtime
                .hovered_scene_region
                .filter(|(hovered_view_id, _)| *hovered_view_id == view_id)
                .map(|(_, region_id)| region_id)
        })
        .flatten();

    if let Some(bounds) = runtime.bounds_override {
        let screen_rect = screen_rect_from_canvas_rect(bounds, meta.as_ref());
        let stroke = simulate_visuals
            .map(|visuals| {
                egui::Stroke::new(visuals.boundary_stroke_width, visuals.boundary_stroke)
            })
            .unwrap_or_else(|| {
                egui::Stroke::new(
                    1.5,
                    egui::Color32::from_rgba_unmultiplied(220, 220, 235, 96),
                )
            });
        let fill = simulate_visuals
            .map(|visuals| visuals.boundary_fill)
            .unwrap_or(egui::Color32::TRANSPARENT);
        painter.rect(
            screen_rect,
            egui::CornerRadius::same(10),
            fill,
            stroke,
            egui::StrokeKind::Outside,
        );
        painter.text(
            screen_rect.left_top() + egui::vec2(8.0, 6.0),
            egui::Align2::LEFT_TOP,
            "Scene Bounds",
            egui::FontId::proportional(10.0),
            simulate_visuals
                .map(|visuals| visuals.boundary_label)
                .unwrap_or_else(|| egui::Color32::from_rgba_unmultiplied(235, 235, 245, 140)),
        );
    }

    for region in &runtime.regions {
        if !region.visible {
            continue;
        }

        let colors = scene_region_colors(region.effect);
        let is_selected = selected_region == Some(region.id);
        let is_hovered = hovered_region == Some(region.id);
        let stroke_width = if is_selected {
            2.5
        } else if is_hovered {
            2.0
        } else {
            1.5
        };
        let fill = if is_selected {
            colors.fill.gamma_multiply(1.35)
        } else if is_hovered {
            colors.fill.gamma_multiply(1.15)
        } else if simulate_mode && matches!(region.effect, SceneRegionEffect::Wall) {
            colors.fill.gamma_multiply(1.1)
        } else {
            colors.fill
        };
        let stroke = if is_selected {
            colors.stroke.gamma_multiply(1.25)
        } else if is_hovered {
            colors.stroke.gamma_multiply(1.1)
        } else if simulate_mode && matches!(region.effect, SceneRegionEffect::Wall) {
            simulate_visuals
                .map(|visuals| visuals.boundary_stroke)
                .unwrap_or(colors.stroke.gamma_multiply(1.05))
        } else {
            colors.stroke
        };
        match region_screen_shape(region, meta.as_ref()) {
            Some(SceneBackdropScreenShape::Rect(rect)) => {
                painter.rect(
                    rect,
                    egui::CornerRadius::same(12),
                    fill,
                    egui::Stroke::new(stroke_width, stroke),
                    egui::StrokeKind::Outside,
                );
                if let Some(label) = region.label.as_deref() {
                    painter.text(
                        rect.left_top() + egui::vec2(8.0, 6.0),
                        egui::Align2::LEFT_TOP,
                        label,
                        egui::FontId::proportional(11.0),
                        colors.label,
                    );
                }
            }
            Some(SceneBackdropScreenShape::Circle { center, radius }) => {
                painter.circle_filled(center, radius, fill);
                painter.circle_stroke(center, radius, egui::Stroke::new(stroke_width, stroke));
                if let Some(label) = region.label.as_deref() {
                    painter.text(
                        center + egui::vec2(0.0, -(radius + 8.0)),
                        egui::Align2::CENTER_BOTTOM,
                        label,
                        egui::FontId::proportional(11.0),
                        colors.label,
                    );
                }
            }
            None => {}
        }
        if arrange_mode && is_selected {
            let hovered_handle = pointer
                .and_then(|pointer| scene_region_resize_handle_hit(region, pointer, meta.as_ref()));
            for handle in scene_region_resize_handles(region, meta.as_ref()) {
                let handle_fill = if hovered_handle.is_some_and(|hit| hit.handle == handle.handle) {
                    colors.stroke
                } else {
                    colors.label
                };
                painter.circle_filled(handle.center, SCENE_REGION_HANDLE_RADIUS, handle_fill);
                painter.circle_stroke(
                    handle.center,
                    SCENE_REGION_HANDLE_RADIUS,
                    egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(18, 18, 24, 220)),
                );
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct SceneRegionActionOverlayResult {
    pub(super) pointer_over: bool,
    pub(super) action_invoked: bool,
}

pub(super) fn draw_scene_region_action_overlay(
    ui: &mut Ui,
    app: &mut GraphBrowserApp,
    view_id: GraphViewId,
    metadata_id: egui::Id,
) -> SceneRegionActionOverlayResult {
    if app.graph_view_scene_mode(view_id) != crate::app::SceneMode::Arrange {
        return SceneRegionActionOverlayResult::default();
    }

    let Some(region_id) = app.graph_view_selected_scene_region(view_id) else {
        return SceneRegionActionOverlayResult::default();
    };
    let Some(region) = app.graph_view_scene_region(view_id, region_id).cloned() else {
        return SceneRegionActionOverlayResult::default();
    };

    let meta = ui
        .ctx()
        .data_mut(|d| d.get_persisted::<MetadataFrame>(metadata_id));
    let Some(anchor) = region_screen_shape(&region, meta.as_ref())
        .map(|shape| scene_region_action_anchor(shape, ui.max_rect()))
    else {
        return SceneRegionActionOverlayResult::default();
    };

    let view_selection = app.selection_for_view(view_id).clone();
    let selected_nodes: Vec<_> = view_selection.iter().copied().collect();
    let graphlet_nodes = if selected_nodes.is_empty() {
        Vec::new()
    } else {
        app.graphlet_members_for_nodes_in_view(&selected_nodes, Some(view_id))
    };
    let search_result_nodes = active_graph_search_node_keys(app);
    let filtered_view_nodes = filtered_view_node_keys(app, view_id);

    egui::Area::new(egui::Id::new(("scene_region_actions", view_id, region_id)))
        .order(egui::Order::Foreground)
        .fixed_pos(anchor)
        .show(ui.ctx(), |ui| {
            let frame = egui::Frame::window(ui.style())
                .corner_radius(egui::CornerRadius::same(10))
                .inner_margin(egui::Margin::symmetric(8, 6))
                .show(ui, |ui| {
                    let mut action_invoked = false;
                    ui.horizontal_wrapped(|ui| {
                        ui.label(egui::RichText::new("Gather").small().strong());
                        if ui
                            .add_enabled(
                                !selected_nodes.is_empty(),
                                egui::Button::new(format!("Selection {}", selected_nodes.len())),
                            )
                            .clicked()
                        {
                            gather_node_keys_into_scene_region(
                                app,
                                &region,
                                selected_nodes.clone(),
                            );
                            action_invoked = true;
                        }
                        if ui
                            .add_enabled(
                                !graphlet_nodes.is_empty(),
                                egui::Button::new(format!("Graphlet {}", graphlet_nodes.len())),
                            )
                            .clicked()
                        {
                            gather_node_keys_into_scene_region(
                                app,
                                &region,
                                graphlet_nodes.clone(),
                            );
                            action_invoked = true;
                        }
                        if ui
                            .add_enabled(
                                !search_result_nodes.is_empty(),
                                egui::Button::new(format!("Search {}", search_result_nodes.len())),
                            )
                            .clicked()
                        {
                            gather_node_keys_into_scene_region(
                                app,
                                &region,
                                search_result_nodes.clone(),
                            );
                            action_invoked = true;
                        }
                        if ui
                            .add_enabled(
                                !filtered_view_nodes.is_empty(),
                                egui::Button::new(format!(
                                    "Filtered {}",
                                    filtered_view_nodes.len()
                                )),
                            )
                            .clicked()
                        {
                            gather_node_keys_into_scene_region(
                                app,
                                &region,
                                filtered_view_nodes.clone(),
                            );
                            action_invoked = true;
                        }
                        if ui.small_button("Panel").clicked() {
                            app.open_scene_overlay(Some(view_id));
                            action_invoked = true;
                        }
                    });
                    action_invoked
                });
            SceneRegionActionOverlayResult {
                pointer_over: frame.response.hovered(),
                action_invoked: frame.inner,
            }
        })
        .inner
}

pub(super) fn draw_scene_simulate_overlays(
    ui: &mut Ui,
    app: &GraphBrowserApp,
    view_id: GraphViewId,
    metadata_id: egui::Id,
) {
    if app.graph_view_scene_mode(view_id) != crate::app::SceneMode::Simulate {
        return;
    }

    let reveal_nodes = app.graph_view_scene_reveal_nodes(view_id);
    let relation_xray = app.graph_view_scene_relation_xray(view_id);
    if !reveal_nodes && !relation_xray {
        return;
    }

    let Some(state) = app.workspace.graph_runtime.egui_state.as_ref() else {
        return;
    };
    let meta = ui
        .ctx()
        .data_mut(|d| d.get_persisted::<MetadataFrame>(metadata_id));
    let theme_tokens = phase3_resolve_active_theme(app.default_registry_theme_id()).tokens;
    let painter = ui.painter().clone().with_layer_id(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new(("scene_simulate_overlay", view_id)),
    ));
    let simulate_visuals =
        simulate_visual_profile(app.graph_view_simulate_behavior_preset(view_id));

    if let Some(impulses) = app
        .workspace
        .graph_runtime
        .simulate_release_impulses
        .get(&view_id)
    {
        for (key, impulse) in impulses {
            let magnitude = impulse.length();
            if magnitude <= 0.05 {
                continue;
            }
            let Some(node) = state.graph.node(*key) else {
                continue;
            };
            let (screen_pos, screen_radius) = if let Some(meta) = meta.as_ref() {
                (
                    meta.canvas_to_screen_pos(node.location()),
                    meta.canvas_to_screen_size(node.display().radius()),
                )
            } else {
                (node.location(), node.display().radius())
            };
            let settle_alpha = ((magnitude / 10.0).clamp(0.12, 1.0) * 255.0) as u8;
            let settle_color = egui::Color32::from_rgba_unmultiplied(
                simulate_visuals.settle_glow.r(),
                simulate_visuals.settle_glow.g(),
                simulate_visuals.settle_glow.b(),
                settle_alpha.min(simulate_visuals.settle_glow.a()),
            );
            painter.circle_stroke(
                screen_pos,
                screen_radius + 13.0,
                egui::Stroke::new(1.75, settle_color),
            );
        }
    }

    if reveal_nodes {
        let node_count = state.graph.nodes_iter().count();
        let show_labels = node_count <= 40;
        for (key, node) in state.graph.nodes_iter() {
            let (screen_pos, screen_radius) = if let Some(meta) = meta.as_ref() {
                (
                    meta.canvas_to_screen_pos(node.location()),
                    meta.canvas_to_screen_size(node.display().radius()),
                )
            } else {
                (node.location(), node.display().radius())
            };
            painter.circle_stroke(
                screen_pos,
                screen_radius + 8.0,
                egui::Stroke::new(
                    2.0,
                    egui::Color32::from_rgba_unmultiplied(
                        theme_tokens.graph_node_focus_ring.r(),
                        theme_tokens.graph_node_focus_ring.g(),
                        theme_tokens.graph_node_focus_ring.b(),
                        170,
                    ),
                ),
            );
            if show_labels && let Some(source) = app.domain_graph().get_node(key) {
                let label = compact_scene_node_label(source);
                if !label.is_empty() {
                    painter.text(
                        screen_pos + egui::vec2(0.0, -(screen_radius + 10.0)),
                        egui::Align2::CENTER_BOTTOM,
                        label,
                        egui::FontId::proportional(11.0),
                        egui::Color32::from_rgba_unmultiplied(245, 245, 250, 210),
                    );
                }
            }
        }
    }

    if relation_xray
        && let Some(focus_key) = scene_relation_xray_focus_node(app, view_id)
        && let Some(focus_node) = state.graph.node(focus_key)
    {
        let focus_pos = if let Some(meta) = meta.as_ref() {
            meta.canvas_to_screen_pos(focus_node.location())
        } else {
            focus_node.location()
        };
        let xray_color = theme_tokens.edge_tokens.selection.foreground_color;
        let mut seen = BTreeSet::new();
        for neighbor in app
            .domain_graph()
            .out_neighbors(focus_key)
            .chain(app.domain_graph().in_neighbors(focus_key))
        {
            if !seen.insert(neighbor.index()) {
                continue;
            }
            let Some(neighbor_node) = state.graph.node(neighbor) else {
                continue;
            };
            let neighbor_pos = if let Some(meta) = meta.as_ref() {
                meta.canvas_to_screen_pos(neighbor_node.location())
            } else {
                neighbor_node.location()
            };
            painter.line_segment(
                [focus_pos, neighbor_pos],
                Stroke::new(
                    3.0,
                    egui::Color32::from_rgba_unmultiplied(
                        xray_color.r(),
                        xray_color.g(),
                        xray_color.b(),
                        190,
                    ),
                ),
            );
            painter.circle_filled(
                neighbor_pos,
                4.0,
                egui::Color32::from_rgba_unmultiplied(
                    xray_color.r(),
                    xray_color.g(),
                    xray_color.b(),
                    220,
                ),
            );
        }
        painter.circle_stroke(
            focus_pos,
            if let Some(meta) = meta.as_ref() {
                meta.canvas_to_screen_size(focus_node.display().radius()) + 10.0
            } else {
                focus_node.display().radius() + 10.0
            },
            Stroke::new(
                2.5,
                egui::Color32::from_rgba_unmultiplied(
                    xray_color.r(),
                    xray_color.g(),
                    xray_color.b(),
                    230,
                ),
            ),
        );
    }
}

pub(super) fn scene_region_at_pointer(
    ui: &Ui,
    app: &GraphBrowserApp,
    view_id: GraphViewId,
    metadata_id: egui::Id,
) -> Option<SceneRegionId> {
    let pointer = ui.input(|i| i.pointer.latest_pos())?;
    let runtime = app.graph_view_scene_runtime(view_id)?;
    let meta = ui
        .ctx()
        .data_mut(|d| d.get_persisted::<MetadataFrame>(metadata_id));
    scene_region_at_screen_pos(runtime.regions.as_slice(), pointer, meta.as_ref())
        .map(|region| region.id)
}

pub(super) fn scene_region_resize_handle_at_pointer(
    ui: &Ui,
    app: &GraphBrowserApp,
    view_id: GraphViewId,
    metadata_id: egui::Id,
) -> Option<SceneRegionResizeHandleHit> {
    let selected_region_id = app.graph_view_selected_scene_region(view_id)?;
    let pointer = ui.input(|i| i.pointer.latest_pos())?;
    let runtime = app.graph_view_scene_runtime(view_id)?;
    let region = runtime
        .regions
        .iter()
        .find(|region| region.id == selected_region_id)?;
    let meta = ui
        .ctx()
        .data_mut(|d| d.get_persisted::<MetadataFrame>(metadata_id));
    scene_region_resize_handle_hit(region, pointer, meta.as_ref())
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum SceneBackdropScreenShape {
    Rect(egui::Rect),
    Circle { center: egui::Pos2, radius: f32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SceneRegionResizeHandleHit {
    pub(super) region_id: SceneRegionId,
    pub(super) handle: SceneRegionResizeHandle,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct SceneRegionResizeHandleScreen {
    handle: SceneRegionResizeHandle,
    center: egui::Pos2,
}

const SCENE_REGION_HANDLE_RADIUS: f32 = 7.0;

fn scene_region_action_anchor(
    shape: SceneBackdropScreenShape,
    canvas_rect: egui::Rect,
) -> egui::Pos2 {
    let anchor = match shape {
        SceneBackdropScreenShape::Rect(rect) => rect.right_top() + egui::vec2(12.0, -8.0),
        SceneBackdropScreenShape::Circle { center, radius } => {
            center + egui::vec2(radius + 12.0, -(radius + 12.0))
        }
    };
    egui::pos2(
        anchor
            .x
            .clamp(canvas_rect.left() + 8.0, canvas_rect.right() - 220.0),
        anchor
            .y
            .clamp(canvas_rect.top() + 8.0, canvas_rect.bottom() - 36.0),
    )
}

fn scene_relation_xray_focus_node(
    app: &GraphBrowserApp,
    view_id: GraphViewId,
) -> Option<crate::graph::NodeKey> {
    app.workspace
        .graph_runtime
        .hovered_graph_node
        .or_else(|| app.selection_for_view(view_id).primary())
}

fn compact_scene_node_label(node: &crate::graph::Node) -> String {
    let raw = if node.title.trim().is_empty() {
        node.cached_host
            .clone()
            .unwrap_or_else(|| node.url().trim().to_string())
    } else {
        node.title.trim().to_string()
    };
    if raw.chars().count() <= 28 {
        raw
    } else {
        let shortened: String = raw.chars().take(27).collect();
        format!("{shortened}…")
    }
}

fn scene_region_at_screen_pos<'a>(
    regions: &'a [SceneRegionRuntime],
    pointer: egui::Pos2,
    meta: Option<&MetadataFrame>,
) -> Option<&'a SceneRegionRuntime> {
    regions
        .iter()
        .filter(|region| region.visible)
        .filter_map(|region| {
            let shape = region_screen_shape(region, meta)?;
            screen_shape_contains(shape, pointer).then_some((region, screen_shape_area(shape)))
        })
        .min_by(|(_, left_area), (_, right_area)| {
            left_area
                .partial_cmp(right_area)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(region, _)| region)
}

fn scene_region_resize_handle_hit(
    region: &SceneRegionRuntime,
    pointer: egui::Pos2,
    meta: Option<&MetadataFrame>,
) -> Option<SceneRegionResizeHandleHit> {
    scene_region_resize_handles(region, meta)
        .into_iter()
        .find(|handle| {
            (pointer - handle.center).length_sq()
                <= SCENE_REGION_HANDLE_RADIUS * SCENE_REGION_HANDLE_RADIUS
        })
        .map(|handle| SceneRegionResizeHandleHit {
            region_id: region.id,
            handle: handle.handle,
        })
}

fn scene_region_resize_handles(
    region: &SceneRegionRuntime,
    meta: Option<&MetadataFrame>,
) -> Vec<SceneRegionResizeHandleScreen> {
    match region_screen_shape(region, meta) {
        Some(SceneBackdropScreenShape::Circle { center, radius }) => {
            vec![SceneRegionResizeHandleScreen {
                handle: SceneRegionResizeHandle::CircleRadius,
                center: center + egui::vec2(radius, 0.0),
            }]
        }
        Some(SceneBackdropScreenShape::Rect(rect)) => vec![
            SceneRegionResizeHandleScreen {
                handle: SceneRegionResizeHandle::RectTopLeft,
                center: rect.left_top(),
            },
            SceneRegionResizeHandleScreen {
                handle: SceneRegionResizeHandle::RectTopRight,
                center: rect.right_top(),
            },
            SceneRegionResizeHandleScreen {
                handle: SceneRegionResizeHandle::RectBottomLeft,
                center: rect.left_bottom(),
            },
            SceneRegionResizeHandleScreen {
                handle: SceneRegionResizeHandle::RectBottomRight,
                center: rect.right_bottom(),
            },
        ],
        None => Vec::new(),
    }
}

fn screen_shape_contains(shape: SceneBackdropScreenShape, pointer: egui::Pos2) -> bool {
    match shape {
        SceneBackdropScreenShape::Rect(rect) => rect.contains(pointer),
        SceneBackdropScreenShape::Circle { center, radius } => {
            (pointer - center).length_sq() <= radius * radius
        }
    }
}

fn screen_shape_area(shape: SceneBackdropScreenShape) -> f32 {
    match shape {
        SceneBackdropScreenShape::Rect(rect) => rect.width().max(0.0) * rect.height().max(0.0),
        SceneBackdropScreenShape::Circle { radius, .. } => std::f32::consts::PI * radius * radius,
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct SceneBackdropColors {
    fill: egui::Color32,
    stroke: egui::Color32,
    label: egui::Color32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct SimulateVisualProfile {
    boundary_stroke_width: f32,
    boundary_stroke: egui::Color32,
    boundary_fill: egui::Color32,
    boundary_label: egui::Color32,
    settle_glow: egui::Color32,
}

fn simulate_visual_profile(preset: SimulateBehaviorPreset) -> SimulateVisualProfile {
    match preset {
        SimulateBehaviorPreset::Float => SimulateVisualProfile {
            boundary_stroke_width: 1.25,
            boundary_stroke: egui::Color32::from_rgba_unmultiplied(176, 208, 236, 84),
            boundary_fill: egui::Color32::from_rgba_unmultiplied(128, 168, 212, 12),
            boundary_label: egui::Color32::from_rgba_unmultiplied(224, 236, 248, 132),
            settle_glow: egui::Color32::from_rgba_unmultiplied(188, 220, 245, 110),
        },
        SimulateBehaviorPreset::Packed => SimulateVisualProfile {
            boundary_stroke_width: 2.2,
            boundary_stroke: egui::Color32::from_rgba_unmultiplied(236, 212, 164, 132),
            boundary_fill: egui::Color32::from_rgba_unmultiplied(196, 172, 120, 20),
            boundary_label: egui::Color32::from_rgba_unmultiplied(246, 236, 210, 164),
            settle_glow: egui::Color32::from_rgba_unmultiplied(245, 224, 176, 126),
        },
        SimulateBehaviorPreset::Magnetic => SimulateVisualProfile {
            boundary_stroke_width: 1.75,
            boundary_stroke: egui::Color32::from_rgba_unmultiplied(168, 224, 204, 112),
            boundary_fill: egui::Color32::from_rgba_unmultiplied(112, 180, 160, 16),
            boundary_label: egui::Color32::from_rgba_unmultiplied(220, 246, 236, 152),
            settle_glow: egui::Color32::from_rgba_unmultiplied(178, 236, 214, 120),
        },
    }
}

fn scene_region_colors(effect: SceneRegionEffect) -> SceneBackdropColors {
    match effect {
        SceneRegionEffect::Attractor { .. } => SceneBackdropColors {
            fill: egui::Color32::from_rgba_unmultiplied(56, 122, 196, 28),
            stroke: egui::Color32::from_rgba_unmultiplied(96, 168, 245, 110),
            label: egui::Color32::from_rgba_unmultiplied(176, 216, 255, 180),
        },
        SceneRegionEffect::Repulsor { .. } => SceneBackdropColors {
            fill: egui::Color32::from_rgba_unmultiplied(186, 82, 82, 26),
            stroke: egui::Color32::from_rgba_unmultiplied(232, 124, 124, 110),
            label: egui::Color32::from_rgba_unmultiplied(255, 210, 210, 180),
        },
        SceneRegionEffect::Dampener { .. } => SceneBackdropColors {
            fill: egui::Color32::from_rgba_unmultiplied(110, 124, 132, 24),
            stroke: egui::Color32::from_rgba_unmultiplied(168, 182, 190, 96),
            label: egui::Color32::from_rgba_unmultiplied(220, 228, 235, 164),
        },
        SceneRegionEffect::Wall => SceneBackdropColors {
            fill: egui::Color32::from_rgba_unmultiplied(188, 164, 94, 20),
            stroke: egui::Color32::from_rgba_unmultiplied(232, 208, 128, 120),
            label: egui::Color32::from_rgba_unmultiplied(248, 236, 188, 180),
        },
    }
}

fn region_screen_shape(
    region: &SceneRegionRuntime,
    meta: Option<&MetadataFrame>,
) -> Option<SceneBackdropScreenShape> {
    match region.shape {
        SceneRegionShape::Circle { center, radius } => Some(SceneBackdropScreenShape::Circle {
            center: meta
                .map(|m| m.canvas_to_screen_pos(center))
                .unwrap_or(center),
            radius: meta
                .map(|m| m.canvas_to_screen_size(radius))
                .unwrap_or(radius)
                .max(1.0),
        }),
        SceneRegionShape::Rect { rect } => Some(SceneBackdropScreenShape::Rect(
            screen_rect_from_canvas_rect(rect, meta),
        )),
    }
}

fn screen_rect_from_canvas_rect(rect: egui::Rect, meta: Option<&MetadataFrame>) -> egui::Rect {
    if let Some(meta) = meta {
        egui::Rect::from_min_max(
            meta.canvas_to_screen_pos(rect.min),
            meta.canvas_to_screen_pos(rect.max),
        )
    } else {
        rect
    }
}

fn frame_affinity_backdrop_rect(
    app: &GraphBrowserApp,
    region: &crate::graph::frame_affinity::FrameAffinityRegion,
    meta: Option<&MetadataFrame>,
) -> Option<egui::Rect> {
    let egui_state = app.workspace.graph_runtime.egui_state.as_ref()?;
    let positions: Vec<egui::Pos2> = region
        .members
        .iter()
        .filter_map(|&key| {
            let node = egui_state.graph.node(key)?;
            let canvas_pos = node.location();
            let screen_pos = meta
                .map(|m| m.canvas_to_screen_pos(canvas_pos))
                .unwrap_or(canvas_pos);
            Some(screen_pos)
        })
        .collect();

    if positions.len() < 2 {
        return None;
    }

    let (min_x, min_y, max_x, max_y) = positions.iter().fold(
        (f32::MAX, f32::MAX, f32::MIN, f32::MIN),
        |(min_x, min_y, max_x, max_y), p| {
            (
                min_x.min(p.x),
                min_y.min(p.y),
                max_x.max(p.x),
                max_y.max(p.y),
            )
        },
    );

    let padding = meta.map(|m| m.canvas_to_screen_size(40.0)).unwrap_or(40.0);

    Some(egui::Rect::from_min_max(
        egui::Pos2::new(min_x - padding, min_y - padding),
        egui::Pos2::new(max_x + padding, max_y + padding),
    ))
}

/// Return the display label for a frame anchor node.
///
/// Prefers the node's title; falls back to the URL host segment.
fn frame_anchor_label(app: &GraphBrowserApp, anchor: crate::graph::NodeKey) -> Option<String> {
    let node = app.domain_graph().get_node(anchor)?;
    if !node.title.is_empty() && node.title != node.url() {
        return Some(node.title.clone());
    }
    // Fall back to URL host segment
    servo::ServoUrl::parse(node.url()).ok().and_then(|u| {
        u.host_str()
            .map(|h| h.trim_start_matches("www.").to_string())
    })
}

fn frame_layout_hint_indicator(hints: &[FrameLayoutHint]) -> Option<String> {
    match hints {
        [] => None,
        [FrameLayoutHint::SplitHalf { orientation, .. }] => Some(match orientation {
            SplitOrientation::Vertical => "||".to_string(),
            SplitOrientation::Horizontal => "=".to_string(),
        }),
        [FrameLayoutHint::SplitPamphlet { orientation, .. }] => Some(match orientation {
            SplitOrientation::Vertical => "|||".to_string(),
            SplitOrientation::Horizontal => "===".to_string(),
        }),
        [FrameLayoutHint::SplitTriptych { .. }] => Some("T".to_string()),
        [FrameLayoutHint::SplitQuartered { .. }] => Some("2x2".to_string()),
        _ => Some(format!("{} splits", hints.len())),
    }
}

fn frame_anchor_split_indicator(
    app: &GraphBrowserApp,
    anchor: crate::graph::NodeKey,
) -> Option<String> {
    let hints = app.domain_graph().frame_layout_hints(anchor)?;
    frame_layout_hint_indicator(hints)
}

fn frame_anchor_is_current_frame(app: &GraphBrowserApp, anchor: crate::graph::NodeKey) -> bool {
    let Some(frame_name) = app.current_frame_name() else {
        return false;
    };
    let frame_url = VersoAddress::frame(frame_name.to_string()).to_string();
    app.domain_graph()
        .get_node_by_url(&frame_url)
        .is_some_and(|(frame_key, _)| frame_key == anchor)
}

fn frame_anchor_is_selected_frame(app: &GraphBrowserApp, anchor: crate::graph::NodeKey) -> bool {
    let Some(frame_name) = app.selected_frame_name() else {
        return false;
    };
    let frame_url = VersoAddress::frame(frame_name.to_string()).to_string();
    app.domain_graph()
        .get_node_by_url(&frame_url)
        .is_some_and(|(frame_key, _)| frame_key == anchor)
}

fn frame_anchor_is_selected_or_current(
    app: &GraphBrowserApp,
    anchor: crate::graph::NodeKey,
) -> bool {
    frame_anchor_is_selected_frame(app, anchor) || frame_anchor_is_current_frame(app, anchor)
}

pub(super) fn frame_anchor_at_pointer(
    ui: &Ui,
    app: &GraphBrowserApp,
    metadata_id: egui::Id,
) -> Option<crate::graph::NodeKey> {
    let pointer = ui.input(|i| i.pointer.latest_pos())?;
    let regions = crate::graph::frame_affinity::derive_frame_affinity_regions(app.domain_graph());
    let meta = ui
        .ctx()
        .data_mut(|d| d.get_persisted::<MetadataFrame>(metadata_id));

    regions
        .into_iter()
        .filter_map(|region| {
            let rect = frame_affinity_backdrop_rect(app, &region, meta.as_ref())?;
            rect.contains(pointer)
                .then_some((region.frame_anchor, rect.width() * rect.height()))
        })
        .min_by(|(_, left_area), (_, right_area)| {
            left_area
                .partial_cmp(right_area)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(anchor, _)| anchor)
}

pub(super) fn draw_highlighted_edge_overlay(
    ui: &mut Ui,
    app: &GraphBrowserApp,
    _widget_id: egui::Id,
    metadata_id: egui::Id,
) {
    let theme_resolution = phase3_resolve_active_theme(app.default_registry_theme_id());
    let selection = theme_resolution.tokens.edge_tokens.selection;
    let Some((from, to)) = app.workspace.graph_runtime.highlighted_graph_edge else {
        return;
    };
    let Some(state) = app.workspace.graph_runtime.egui_state.as_ref() else {
        return;
    };
    let Some(from_node) = state.graph.node(from) else {
        return;
    };
    let Some(to_node) = state.graph.node(to) else {
        return;
    };
    let (from_screen, to_screen) = if let Some(meta) = ui
        .ctx()
        .data_mut(|d| d.get_persisted::<MetadataFrame>(metadata_id))
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
        Stroke::new(6.0, selection.halo_color),
    );
    ui.painter().line_segment(
        [from_screen, to_screen],
        Stroke::new(5.0 + selection.width_delta, selection.foreground_color),
    );
    ui.painter().circle_filled(
        from_screen,
        6.0 + selection.width_delta,
        selection.foreground_color,
    );
    ui.painter().circle_filled(
        to_screen,
        6.0 + selection.width_delta,
        selection.foreground_color,
    );
}

pub(super) fn draw_hovered_edge_tooltip(
    ui: &Ui,
    app: &GraphBrowserApp,
    widget_id: egui::Id,
    metadata_id: egui::Id,
) {
    if app.workspace.graph_runtime.hovered_graph_node.is_some() {
        return;
    }
    let Some(pointer) = ui.input(|i| i.pointer.hover_pos()) else {
        return;
    };
    let Some((from, to)) = edge_endpoints_at_pointer(ui, app, metadata_id) else {
        return;
    };

    let ab_payload = app
        .domain_graph()
        .find_edge_key(from, to)
        .and_then(|k| app.domain_graph().get_edge(k));
    let ba_payload = app
        .domain_graph()
        .find_edge_key(to, from)
        .and_then(|k| app.domain_graph().get_edge(k));

    let ab_count = ab_payload.map(|p| p.traversals().len()).unwrap_or(0);
    let ba_count = ba_payload.map(|p| p.traversals().len()).unwrap_or(0);
    let total = ab_count + ba_count;
    if ab_payload.is_none() && ba_payload.is_none() {
        return;
    }

    let latest_ts = ab_payload
        .into_iter()
        .flat_map(|p| p.traversals().iter().map(|t| t.timestamp_ms))
        .chain(
            ba_payload
                .into_iter()
                .flat_map(|p| p.traversals().iter().map(|t| t.timestamp_ms)),
        )
        .max();

    let from_label = app
        .domain_graph()
        .get_node(from)
        .map(|n| n.title.as_str())
        .filter(|t| !t.is_empty())
        .or_else(|| app.domain_graph().get_node(from).map(|n| n.url()))
        .unwrap_or("unknown");
    let to_label = app
        .domain_graph()
        .get_node(to)
        .map(|n| n.title.as_str())
        .filter(|t| !t.is_empty())
        .or_else(|| app.domain_graph().get_node(to).map(|n| n.url()))
        .unwrap_or("unknown");

    let latest_text = latest_ts
        .and_then(|ms| {
            UNIX_EPOCH
                .checked_add(Duration::from_millis(ms))
                .and_then(|ts| ts.duration_since(UNIX_EPOCH).ok())
                .map(|d| format!("{}s", d.as_secs()))
        })
        .unwrap_or_else(|| "unknown".to_string());

    let mut family_rows: BTreeSet<String> = BTreeSet::new();
    for payload in [ab_payload, ba_payload].into_iter().flatten() {
        for row in edge_family_rows(payload) {
            family_rows.insert(row);
        }
    }

    egui::Area::new(widget_id.with("edge_hover_tooltip"))
        .order(egui::Order::Tooltip)
        .fixed_pos(pointer + Vec2::new(14.0, 14.0))
        .show(ui.ctx(), |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_max_width(320.0);
                ui.label(egui::RichText::new("Edge Inspection").strong());
                ui.label(format!("{from_label} <-> {to_label}"));
                ui.separator();
                if total > 0 {
                    ui.label(format!("{from_label} -> {to_label}: {ab_count}"));
                    ui.label(format!("{to_label} -> {from_label}: {ba_count}"));
                    ui.label(format!("Total traversals: {total}"));
                    ui.label(format!("Latest traversal: {latest_text}"));
                    ui.separator();
                }
                ui.label(egui::RichText::new("Family | Durability | Provenance").small());
                for row in family_rows {
                    ui.label(egui::RichText::new(row).small());
                }
            });
        });
}

pub(super) fn draw_hovered_node_tooltip(
    ui: &Ui,
    app: &GraphBrowserApp,
    widget_id: egui::Id,
    metadata_id: egui::Id,
) {
    fn compact_hover_node_label(node: &crate::graph::Node) -> String {
        let raw = if node.title.trim().is_empty() {
            node.url().trim()
        } else {
            node.title.trim()
        };
        if raw.is_empty() {
            return "Untitled node".to_string();
        }
        if raw.chars().count() <= 72 {
            return raw.to_string();
        }
        let shortened: String = raw.chars().take(71).collect();
        format!("{shortened}…")
    }

    let Some(key) = app.workspace.graph_runtime.hovered_graph_node else {
        return;
    };
    let Some(node) = app.domain_graph().get_node(key) else {
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
            NodeLifecycle::Tombstone => "Tombstone".to_string(),
        }
    };
    let last_visited_text = format_last_visited(node.last_visited);
    let workspace_memberships: Vec<String> =
        app.membership_for_node(node.id).iter().cloned().collect();
    let anchor = pointer_pos
        .or_else(|| {
            app.workspace
                .graph_runtime
                .egui_state
                .as_ref()
                .and_then(|state| {
                    state.graph.node(key).map(|n| {
                        if let Some(meta) = ui
                            .ctx()
                            .data_mut(|d| d.get_persisted::<MetadataFrame>(metadata_id))
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
            let theme_tokens = phase3_resolve_active_theme(app.default_registry_theme_id()).tokens;
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_min_width(240.0);
                ui.strong(compact_hover_node_label(node));
                ui.label(
                    egui::RichText::new(format!("Last visited: {last_visited_text}"))
                        .small()
                        .color(theme_tokens.radial_chrome_text),
                );
                ui.label(
                    egui::RichText::new(format!("Lifecycle: {lifecycle_text}"))
                        .small()
                        .color(theme_tokens.radial_chrome_text),
                );
                if !workspace_memberships.is_empty() {
                    ui.separator();
                    ui.label(
                        egui::RichText::new(format!(
                            "Workspaces ({})",
                            workspace_memberships.len()
                        ))
                        .small()
                        .color(theme_tokens.command_notice),
                    );
                    for workspace in &workspace_memberships {
                        ui.label(
                            egui::RichText::new(format!("- {workspace}"))
                                .small()
                                .color(theme_tokens.radial_chrome_text),
                        );
                    }
                }
            });
        });
}

// ── Time formatting ───────────────────────────────────────────────────────────

pub(super) fn format_last_visited(last_visited: std::time::SystemTime) -> String {
    let now = std::time::SystemTime::now();
    format_last_visited_with_now(last_visited, now)
}

pub(super) fn format_last_visited_with_now(
    last_visited: std::time::SystemTime,
    now: std::time::SystemTime,
) -> String {
    let Ok(elapsed) = now.duration_since(last_visited) else {
        return "just now".to_string();
    };
    format_elapsed_ago(elapsed)
}

pub(super) fn format_elapsed_ago(elapsed: Duration) -> String {
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

#[cfg(test)]
mod tests {
    use super::{
        SceneBackdropScreenShape, frame_anchor_is_current_frame, frame_layout_hint_indicator,
        region_screen_shape, scene_region_at_screen_pos, scene_region_resize_handle_hit,
        screen_rect_from_canvas_rect, simulate_visual_profile,
    };
    use crate::app::{GraphBrowserApp, SimulateBehaviorPreset};
    use crate::graph::scene_runtime::SceneRegionResizeHandle;
    use crate::graph::scene_runtime::{SceneRegionEffect, SceneRegionRuntime};
    use crate::graph::{DominantEdge, FrameLayoutHint, SplitOrientation};
    use euclid::default::Point2D;

    #[test]
    fn frame_layout_hint_indicator_returns_none_for_empty_hint_list() {
        assert_eq!(frame_layout_hint_indicator(&[]), None);
    }

    #[test]
    fn frame_layout_hint_indicator_returns_triptych_token_for_single_triptych_hint() {
        let hints = vec![FrameLayoutHint::SplitTriptych {
            dominant: "dominant".to_string(),
            dominant_edge: DominantEdge::Left,
            wings: ["wing-a".to_string(), "wing-b".to_string()],
        }];

        assert_eq!(frame_layout_hint_indicator(&hints), Some("T".to_string()));
    }

    #[test]
    fn frame_layout_hint_indicator_returns_count_when_multiple_hints_exist() {
        let hints = vec![
            FrameLayoutHint::SplitHalf {
                first: "a".to_string(),
                second: "b".to_string(),
                orientation: SplitOrientation::Horizontal,
            },
            FrameLayoutHint::SplitQuartered {
                top_left: "a".to_string(),
                top_right: "b".to_string(),
                bottom_left: "c".to_string(),
                bottom_right: "d".to_string(),
            },
        ];

        assert_eq!(
            frame_layout_hint_indicator(&hints),
            Some("2 splits".to_string())
        );
    }

    #[test]
    fn frame_anchor_is_current_frame_matches_active_frame_anchor() {
        let mut app = GraphBrowserApp::new_for_testing();
        let node = app.add_node_and_sync(
            "https://active-frame.example".to_string(),
            Point2D::new(0.0, 0.0),
        );
        let mut tiles = egui_tiles::Tiles::default();
        let node_tile = tiles.insert_pane(
            crate::shell::desktop::workbench::tile_kind::TileKind::Node(node.into()),
        );
        let root = tiles.insert_tab_tile(vec![node_tile]);
        let tree = egui_tiles::Tree::new("active_frame_anchor", root, tiles);
        app.sync_named_workbench_frame_graph_representation("alpha", &tree);
        app.note_frame_activated("alpha", [node]);

        let frame_url = crate::util::VersoAddress::frame("alpha").to_string();
        let (frame_key, _) = app
            .domain_graph()
            .get_node_by_url(&frame_url)
            .expect("frame anchor should exist");

        assert!(frame_anchor_is_current_frame(&app, frame_key));
    }

    #[test]
    fn screen_rect_from_canvas_rect_returns_identity_without_metadata() {
        let rect = egui::Rect::from_min_max(egui::pos2(10.0, 20.0), egui::pos2(40.0, 60.0));

        assert_eq!(screen_rect_from_canvas_rect(rect, None), rect);
    }

    #[test]
    fn region_screen_shape_returns_rect_identity_without_metadata() {
        let rect = egui::Rect::from_min_max(egui::pos2(10.0, 20.0), egui::pos2(40.0, 60.0));
        let region = SceneRegionRuntime::rect(rect, SceneRegionEffect::Wall);

        assert_eq!(
            region_screen_shape(&region, None),
            Some(SceneBackdropScreenShape::Rect(rect))
        );
    }

    #[test]
    fn region_screen_shape_returns_circle_identity_without_metadata() {
        let center = egui::pos2(16.0, 24.0);
        let region = SceneRegionRuntime::circle(
            center,
            18.0,
            SceneRegionEffect::Attractor { strength: 0.2 },
        );

        assert_eq!(
            region_screen_shape(&region, None),
            Some(SceneBackdropScreenShape::Circle {
                center,
                radius: 18.0,
            })
        );
    }

    #[test]
    fn simulate_visual_profiles_have_distinct_boundary_weights() {
        let float = simulate_visual_profile(SimulateBehaviorPreset::Float);
        let packed = simulate_visual_profile(SimulateBehaviorPreset::Packed);
        let magnetic = simulate_visual_profile(SimulateBehaviorPreset::Magnetic);

        assert!(packed.boundary_stroke_width > magnetic.boundary_stroke_width);
        assert!(magnetic.boundary_stroke_width > float.boundary_stroke_width);
    }

    #[test]
    fn scene_region_hit_prefers_smallest_overlapping_region() {
        let outer = SceneRegionRuntime::rect(
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(100.0, 100.0)),
            SceneRegionEffect::Wall,
        );
        let inner = SceneRegionRuntime::rect(
            egui::Rect::from_min_max(egui::pos2(40.0, 40.0), egui::pos2(60.0, 60.0)),
            SceneRegionEffect::Attractor { strength: 0.2 },
        );
        let regions = vec![outer.clone(), inner.clone()];

        let hit = scene_region_at_screen_pos(&regions, egui::pos2(50.0, 50.0), None)
            .expect("pointer should hit overlapping regions");

        assert_eq!(hit.id, inner.id);
    }

    #[test]
    fn scene_region_hit_ignores_invisible_regions() {
        let visible = SceneRegionRuntime::rect(
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(100.0, 100.0)),
            SceneRegionEffect::Wall,
        );
        let hidden = SceneRegionRuntime::rect(
            egui::Rect::from_min_max(egui::pos2(40.0, 40.0), egui::pos2(60.0, 60.0)),
            SceneRegionEffect::Attractor { strength: 0.2 },
        )
        .with_visibility(false);
        let regions = vec![visible.clone(), hidden];

        let hit = scene_region_at_screen_pos(&regions, egui::pos2(50.0, 50.0), None)
            .expect("pointer should hit visible region");

        assert_eq!(hit.id, visible.id);
    }

    #[test]
    fn scene_region_resize_handle_hit_detects_circle_radius_handle() {
        let region = SceneRegionRuntime::circle(
            egui::pos2(20.0, 20.0),
            30.0,
            SceneRegionEffect::Attractor { strength: 0.2 },
        );

        let hit = scene_region_resize_handle_hit(&region, egui::pos2(50.0, 20.0), None)
            .expect("pointer should hit the circle radius handle");

        assert_eq!(hit.region_id, region.id);
        assert_eq!(hit.handle, SceneRegionResizeHandle::CircleRadius);
    }

    #[test]
    fn scene_region_resize_handle_hit_detects_rect_corner_handle() {
        let region = SceneRegionRuntime::rect(
            egui::Rect::from_min_max(egui::pos2(10.0, 20.0), egui::pos2(60.0, 80.0)),
            SceneRegionEffect::Wall,
        );

        let hit = scene_region_resize_handle_hit(&region, egui::pos2(10.0, 20.0), None)
            .expect("pointer should hit the rect top-left handle");

        assert_eq!(hit.region_id, region.id);
        assert_eq!(hit.handle, SceneRegionResizeHandle::RectTopLeft);
    }
}

