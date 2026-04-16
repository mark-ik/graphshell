/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Graph info overlay: stats bar, search status, enrichment panel, controls
//! hint, layout algorithm selection, and semantic depth badge.

use crate::app::{
    GraphBrowserApp, GraphIntent, GraphSearchHistoryEntry, GraphSearchOrigin, SceneMode,
    SearchDisplayMode, SimulateBehaviorPreset, ThreeDMode, ViewAction, ViewDimension, ZSource,
};
use crate::graph::NodeKey;
use crate::graph::format_imported_at_secs;
use crate::graph::scene_runtime::{SceneRegionEffect, SceneRegionRuntime, SceneRegionShape};
use crate::model::graph::ClassificationStatus;
use crate::util::CoordBridge;
use egui::Vec2;
use euclid::default::Point2D;
use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

use super::canvas_visuals::{
    active_presentation_profile, active_view_filter_expr, evaluate_active_view_filter,
    visible_nodes_for_view_filters,
};
use super::reducer_bridge::apply_ui_intents_with_checkpoint;
use super::semantic_tags::{
    PlacementAnchorSummary, SelectedNodeEnrichmentSummary, graph_search_history_label,
    graph_search_scope_label, render_classification_chips, render_graph_search_origin_badge,
    render_semantic_suggestion_buttons, render_semantic_tag_status_buttons,
    request_graph_search_entry, semantic_suggestion_chip, semantic_tag_status_chip,
};

// ── Public API ────────────────────────────────────────────────────────────────

pub(super) fn draw_graph_info(
    ui: &mut egui::Ui,
    app: &mut GraphBrowserApp,
    view_id: crate::app::GraphViewId,
) {
    let presentation = active_presentation_profile(app);
    let info_text = format!(
        "Nodes: {} | Edges: {} | Physics: {} | Zoom: {:.1}x",
        app.domain_graph().node_count(),
        app.domain_graph().edge_count(),
        if app.workspace.graph_runtime.physics.base.is_running {
            "Running"
        } else {
            "Paused"
        },
        app.workspace.graph_runtime.camera.current_zoom
    );

    ui.painter().text(
        ui.available_rect_before_wrap().left_top() + Vec2::new(10.0, 10.0),
        egui::Align2::LEFT_TOP,
        info_text,
        egui::FontId::monospace(12.0),
        presentation.info_text.to_color32(),
    );

    let mut top_left_overlay_y = 28.0;
    if !app
        .workspace
        .graph_runtime
        .active_graph_search_query
        .is_empty()
    {
        let query = app
            .workspace
            .graph_runtime
            .active_graph_search_query
            .clone();
        let filter_mode = matches!(
            app.workspace.graph_runtime.search_display_mode,
            SearchDisplayMode::Filter
        );
        let match_count = app.workspace.graph_runtime.active_graph_search_match_count;
        let current_entry = GraphSearchHistoryEntry {
            query: query.clone(),
            filter_mode,
            origin: app
                .workspace
                .graph_runtime
                .active_graph_search_origin
                .clone(),
            neighborhood_anchor: app
                .workspace
                .graph_runtime
                .active_graph_search_neighborhood_anchor,
            neighborhood_depth: app
                .workspace
                .graph_runtime
                .active_graph_search_neighborhood_depth,
        };
        let pinned_entry = app.workspace.graph_runtime.pinned_graph_search.clone();
        let is_pinned = pinned_entry.as_ref() == Some(&current_entry);
        let area_rect = ui.available_rect_before_wrap();
        egui::Area::new(egui::Id::new("graph_active_search_status"))
            .order(egui::Order::Foreground)
            .fixed_pos(area_rect.left_top() + Vec2::new(10.0, 26.0))
            .show(ui.ctx(), |ui| {
                egui::Frame::window(ui.style())
                    .inner_margin(egui::Margin::same(8))
                    .show(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            render_graph_search_origin_badge(
                                app,
                                ui,
                                &app.workspace.graph_runtime.active_graph_search_origin,
                            );
                            let scope_label = graph_search_scope_label(
                                app.workspace
                                    .graph_runtime
                                    .active_graph_search_neighborhood_anchor,
                                app.workspace
                                    .graph_runtime
                                    .active_graph_search_neighborhood_depth,
                            );
                            ui.small(format!(
                                "Search: {query} | {match_count} matches{scope_label}"
                            ));
                            if !app.workspace.graph_runtime.graph_search_history.is_empty()
                                && ui.small_button("Back").clicked()
                            {
                                if let Some(entry) =
                                    app.workspace.graph_runtime.graph_search_history.pop()
                                {
                                    request_graph_search_entry(app, entry, false, None);
                                }
                            }
                            if ui
                                .small_button(if is_pinned { "Unpin" } else { "Pin" })
                                .clicked()
                            {
                                if is_pinned {
                                    app.workspace.graph_runtime.pinned_graph_search = None;
                                } else {
                                    app.workspace.graph_runtime.pinned_graph_search =
                                        Some(current_entry.clone());
                                }
                                app.workspace.graph_runtime.egui_state_dirty = true;
                            }
                            if ui.selectable_label(!filter_mode, "Highlight").clicked() {
                                app.request_graph_search_with_options(
                                    query.clone(),
                                    false,
                                    app.workspace
                                        .graph_runtime
                                        .active_graph_search_origin
                                        .clone(),
                                    app.workspace
                                        .graph_runtime
                                        .active_graph_search_neighborhood_anchor,
                                    app.workspace
                                        .graph_runtime
                                        .active_graph_search_neighborhood_depth,
                                    true,
                                    None,
                                );
                            }
                            if ui.selectable_label(filter_mode, "Filter").clicked() {
                                app.request_graph_search_with_options(
                                    query.clone(),
                                    true,
                                    app.workspace
                                        .graph_runtime
                                        .active_graph_search_origin
                                        .clone(),
                                    app.workspace
                                        .graph_runtime
                                        .active_graph_search_neighborhood_anchor,
                                    app.workspace
                                        .graph_runtime
                                        .active_graph_search_neighborhood_depth,
                                    true,
                                    None,
                                );
                            }
                            if app
                                .workspace
                                .graph_runtime
                                .active_graph_search_neighborhood_anchor
                                .is_some()
                            {
                                let depth = app
                                    .workspace
                                    .graph_runtime
                                    .active_graph_search_neighborhood_depth;
                                if ui.selectable_label(depth == 1, "1-hop").clicked() {
                                    app.request_graph_search_with_options(
                                        query.clone(),
                                        filter_mode,
                                        app.workspace
                                            .graph_runtime
                                            .active_graph_search_origin
                                            .clone(),
                                        app.workspace
                                            .graph_runtime
                                            .active_graph_search_neighborhood_anchor,
                                        1,
                                        false,
                                        None,
                                    );
                                }
                                if ui.selectable_label(depth == 2, "2-hop").clicked() {
                                    app.request_graph_search_with_options(
                                        query.clone(),
                                        filter_mode,
                                        app.workspace
                                            .graph_runtime
                                            .active_graph_search_origin
                                            .clone(),
                                        app.workspace
                                            .graph_runtime
                                            .active_graph_search_neighborhood_anchor,
                                        2,
                                        false,
                                        None,
                                    );
                                }
                            }
                            if ui.small_button("Clear").clicked() {
                                app.request_graph_search(String::new(), false);
                            }
                        });
                        if let Some(entry) = pinned_entry.filter(|entry| entry != &current_entry) {
                            ui.separator();
                            ui.horizontal_wrapped(|ui| {
                                ui.small("Pinned:");
                                if ui
                                    .small_button(graph_search_history_label(&entry))
                                    .clicked()
                                {
                                    request_graph_search_entry(app, entry, false, None);
                                }
                            });
                        }
                        if !app.workspace.graph_runtime.graph_search_history.is_empty() {
                            ui.separator();
                            ui.horizontal_wrapped(|ui| {
                                ui.small("Recent:");
                                let recent_entries = app
                                    .workspace
                                    .graph_runtime
                                    .graph_search_history
                                    .iter()
                                    .rev()
                                    .take(3)
                                    .cloned()
                                    .collect::<Vec<_>>();
                                for entry in recent_entries {
                                    let label = graph_search_history_label(&entry);
                                    if ui.small_button(label).clicked() {
                                        request_graph_search_entry(app, entry, false, None);
                                    }
                                }
                            });
                        }
                    });
            });
        top_left_overlay_y = 58.0;
    } else if let Some(entry) = app.workspace.graph_runtime.pinned_graph_search.clone() {
        let area_rect = ui.available_rect_before_wrap();
        egui::Area::new(egui::Id::new("graph_pinned_search_status"))
            .order(egui::Order::Foreground)
            .fixed_pos(area_rect.left_top() + Vec2::new(10.0, 26.0))
            .show(ui.ctx(), |ui| {
                egui::Frame::window(ui.style())
                    .inner_margin(egui::Margin::same(6))
                    .show(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(egui::RichText::new("PIN").small().strong());
                            render_graph_search_origin_badge(app, ui, &entry.origin);
                            ui.small(graph_search_history_label(&entry));
                            if ui.small_button("Restore").clicked() {
                                request_graph_search_entry(app, entry.clone(), false, None);
                            }
                            if ui.small_button("X").clicked() {
                                app.workspace.graph_runtime.pinned_graph_search = None;
                                app.workspace.graph_runtime.egui_state_dirty = true;
                            }
                        });
                    });
            });
        top_left_overlay_y = 54.0;
    }

    if let Some(expr) = active_view_filter_expr(app, view_id).cloned() {
        let summary = evaluate_active_view_filter(app, view_id);
        let match_count = summary
            .as_ref()
            .map(|summary| summary.result.matched_nodes.len())
            .unwrap_or(0);
        let warning_count = summary
            .as_ref()
            .map(|summary| summary.warnings.len())
            .unwrap_or(0);
        let area_rect = ui.available_rect_before_wrap();
        egui::Area::new(egui::Id::new(("graph_active_facet_filter", view_id)))
            .order(egui::Order::Foreground)
            .fixed_pos(area_rect.left_top() + Vec2::new(10.0, top_left_overlay_y))
            .show(ui.ctx(), |ui| {
                egui::Frame::window(ui.style())
                    .inner_margin(egui::Margin::same(6))
                    .show(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(egui::RichText::new("Facet Filter").small().strong());
                            ui.small(format!(
                                "{} | {} visible",
                                expr.display_label(),
                                match_count
                            ));
                            if warning_count > 0 {
                                ui.small(format!("{warning_count} warning(s)"));
                            }
                            if ui.small_button("Clear").clicked() {
                                apply_ui_intents_with_checkpoint(
                                    app,
                                    vec![GraphIntent::ClearViewFilter { view_id }],
                                );
                            }
                        });
                    });
            });
        top_left_overlay_y += 30.0;
    }

    if let Some((label, tooltip)) = graph_view_semantic_depth_status_badge(app, view_id) {
        let area_rect = ui.available_rect_before_wrap();
        egui::Area::new(egui::Id::new(("graph_semantic_depth_status", view_id)))
            .order(egui::Order::Foreground)
            .fixed_pos(area_rect.left_top() + Vec2::new(10.0, top_left_overlay_y))
            .show(ui.ctx(), |ui| {
                egui::Frame::window(ui.style())
                    .inner_margin(egui::Margin::same(6))
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new(label).small().strong())
                            .on_hover_text(tooltip);
                    });
            });
        top_left_overlay_y += 28.0;
    }

    render_scene_quick_actions(ui, app, view_id, top_left_overlay_y);

    if let Some(selected_key) = app.get_single_selected_node() {
        let suggestions = app.suggested_semantic_tags_for_node(selected_key);
        if !suggestions.is_empty() {
            ui.painter().text(
                ui.available_rect_before_wrap().left_top() + Vec2::new(10.0, top_left_overlay_y),
                egui::Align2::LEFT_TOP,
                format!("Suggested tags: {}", suggestions.join(", ")),
                egui::FontId::proportional(11.0),
                presentation.controls_text.to_color32(),
            );
        }

        if let Some(summary) = selected_node_enrichment_summary(app, selected_key) {
            let area_rect = ui.available_rect_before_wrap();
            egui::Area::new(egui::Id::new("graph_selected_node_enrichment"))
                .order(egui::Order::Foreground)
                .fixed_pos(area_rect.right_top() + Vec2::new(-288.0, 12.0))
                .show(ui.ctx(), |ui| {
                    egui::Frame::window(ui.style())
                        .inner_margin(egui::Margin::same(10))
                        .show(ui, |ui| {
                            ui.set_width(276.0);
                            ui.horizontal(|ui| {
                                ui.heading("Selected Node");
                                if ui.small_button("Edit Tags").clicked() {
                                    crate::shell::desktop::ui::tag_panel::open_node_tag_panel(
                                        app,
                                        selected_key,
                                        false,
                                    );
                                }
                            });
                            ui.small(&summary.title);
                            if summary.show_url {
                                ui.small(&summary.url);
                            }
                            ui.separator();
                            ui.small(format!("Lifecycle: {}", summary.lifecycle));
                            if let Some(anchor) = summary.placement_anchor.as_ref() {
                                ui.horizontal_wrapped(|ui| {
                                    ui.small(format!("Semantic anchor: {}", anchor.label));
                                    if ui.small_button("Jump").clicked() {
                                        apply_ui_intents_with_checkpoint(
                                            app,
                                            vec![
                                                GraphIntent::SelectNode {
                                                    key: anchor.key,
                                                    multi_select: false,
                                                },
                                                ViewAction::RequestZoomToSelected.into(),
                                            ],
                                        );
                                    }
                                    if let Some(tag) = anchor.slice_tag.as_ref()
                                        && ui.small_button("Anchor Slice").clicked()
                                    {
                                        app.request_graph_search_with_options(
                                            tag.clone(),
                                            true,
                                            GraphSearchOrigin::AnchorSlice,
                                            None,
                                            1,
                                            true,
                                            Some(
                                                crate::shell::desktop::ui::gui_orchestration::graph_search_toast_message(
                                                    GraphSearchOrigin::AnchorSlice,
                                                    tag,
                                                    0,
                                                ),
                                            ),
                                        );
                                    }
                                    if let Some(tag) = anchor.slice_tag.as_ref()
                                        && ui.small_button("Slice + Neighborhood").clicked()
                                    {
                                        app.request_graph_search_with_options(
                                            tag.clone(),
                                            true,
                                            GraphSearchOrigin::AnchorSlice,
                                            Some(anchor.key),
                                            1,
                                            true,
                                            Some(
                                                crate::shell::desktop::ui::gui_orchestration::graph_search_toast_message(
                                                    GraphSearchOrigin::AnchorSlice,
                                                    tag,
                                                    1,
                                                ),
                                            ),
                                        );
                                    }
                                });
                            }
                            if summary.semantic_lens_available {
                                ui.horizontal_wrapped(|ui| {
                                    ui.small("Semantic view");
                                    if ui
                                        .small_button(if summary.semantic_lens_active {
                                            "Restore View"
                                        } else {
                                            "UDC Depth View"
                                        })
                                        .clicked()
                                        && let Some(target_view_id) =
                                            app.workspace.graph_runtime.focused_view
                                    {
                                        apply_ui_intents_with_checkpoint(
                                            app,
                                            vec![GraphIntent::ToggleSemanticDepthView {
                                                view_id: target_view_id,
                                            }],
                                        );
                                    }
                                });
                                ui.small(if summary.semantic_lens_active {
                                    "Restore the previous Graph View dimension and leave semantic depth mode."
                                } else {
                                    "Visible payoff: the current Graph View lifts nodes into UDC depth layers using semantic class ancestry."
                                });
                            }
                            if !summary.workspace_memberships.is_empty() {
                                ui.small(format!(
                                    "Frames: {}",
                                    summary.workspace_memberships.join(", ")
                                ));
                            }
                            if !summary.import_records.is_empty() {
                                ui.separator();
                                ui.small("Imported");
                                for record in &summary.import_records {
                                    ui.horizontal_wrapped(|ui| {
                                        ui.small(format!(
                                            "{}  {}  {}",
                                            record.source_label,
                                            format_imported_at_secs(record.imported_at_secs),
                                            record.record_id,
                                        ));
                                        if ui.small_button("Promote Group").clicked() {
                                            apply_ui_intents_with_checkpoint(
                                                app,
                                                vec![
                                                    GraphIntent::PromoteImportRecordToUserGroup {
                                                        record_id: record.record_id.clone(),
                                                        anchor: selected_key,
                                                    },
                                                ],
                                            );
                                        }
                                        if ui.small_button("Hide Here").clicked() {
                                            apply_ui_intents_with_checkpoint(
                                                app,
                                                vec![
                                                    GraphIntent::SuppressImportRecordMembership {
                                                        record_id: record.record_id.clone(),
                                                        key: selected_key,
                                                    },
                                                ],
                                            );
                                        }
                                        if ui.small_button("Delete Record").clicked() {
                                            apply_ui_intents_with_checkpoint(
                                                app,
                                                vec![GraphIntent::DeleteImportRecord {
                                                    record_id: record.record_id.clone(),
                                                }],
                                            );
                                        }
                                    });
                                }
                            }
                            ui.separator();
                            ui.small("Semantic tags");
                            if summary.display_tags.is_empty() {
                                ui.small("No semantic tags on this node yet.");
                            } else {
                                render_semantic_tag_status_buttons(
                                    ui,
                                    app,
                                    &summary.display_tags,
                                );
                                if summary.hidden_tag_count > 0 {
                                    ui.small(format!("+{} more", summary.hidden_tag_count));
                                }
                            }
                            if !summary.suggested_tags.is_empty() {
                                ui.separator();
                                ui.small("Suggestions");
                                render_semantic_suggestion_buttons(
                                    ui,
                                    app,
                                    &summary.suggested_tags,
                                );
                            }
                            if !summary.display_classifications.is_empty() {
                                ui.separator();
                                ui.small("Classifications");
                                render_classification_chips(
                                    ui,
                                    app,
                                    &summary.display_classifications,
                                );
                            }
                            ui.separator();
                            ui.small("Click a tag to filter the graph by that semantic slice. Status shows whether the tag is canonical knowledge state or a looser node-local tag; suggestion text explains why the tag suggester surfaced it.");
                        });
                });
        }
    }

    // Draw controls hint
    let lasso_hint = super::canvas_lasso_binding_label(app.lasso_binding_preference());
    let command_hint =
        crate::shell::desktop::runtime::registries::phase2_binding_display_labels_for_action(
            crate::shell::desktop::runtime::registries::input::action_id::graph::COMMAND_PALETTE_OPEN,
        )
        .join(" / ");
    let radial_hint =
        crate::shell::desktop::runtime::registries::phase2_binding_display_labels_for_action(
            crate::shell::desktop::runtime::registries::input::action_id::graph::RADIAL_MENU_OPEN,
        )
        .join(" / ");
    let help_hint =
        crate::shell::desktop::runtime::registries::phase2_binding_display_labels_for_action(
            crate::shell::desktop::runtime::registries::input::action_id::workbench::HELP_OPEN,
        )
        .join(" / ");
    let tags_hint =
        crate::shell::desktop::runtime::registries::phase2_binding_display_labels_for_action(
            crate::shell::desktop::runtime::registries::input::action_id::graph::NODE_EDIT_TAGS,
        )
        .join(" / ");
    let controls_text = format!(
        "Shortcuts: Ctrl+Click Multi-select | {lasso_hint} | Double-click Open | Drag tab out to split | N New Node | Del Remove | T Physics | {tags_hint} Tags | R Reheat | +/-/0 Zoom | C Position-Lock | Z Zoom-Lock | WASD/Arrows Pan | F9 Camera Controls | L Toggle Pin | Ctrl+F Search | G Edge Ops | {command_hint} Commands | {radial_hint} Radial | Ctrl+Z/Y Undo/Redo | {help_hint} Help"
    );
    ui.painter().text(
        ui.available_rect_before_wrap().left_bottom() + Vec2::new(10.0, -10.0),
        egui::Align2::LEFT_BOTTOM,
        controls_text,
        egui::FontId::proportional(10.0),
        presentation.controls_text.to_color32(),
    );
}

fn render_scene_quick_actions(
    ui: &mut egui::Ui,
    app: &mut GraphBrowserApp,
    view_id: crate::app::GraphViewId,
    top_left_overlay_y: f32,
) {
    let area_rect = ui.available_rect_before_wrap();
    egui::Area::new(egui::Id::new(("graph_scene_quick_actions", view_id)))
        .order(egui::Order::Foreground)
        .fixed_pos(area_rect.left_top() + Vec2::new(10.0, top_left_overlay_y))
        .show(ui.ctx(), |ui| {
            egui::Frame::window(ui.style())
                .inner_margin(egui::Margin::same(6))
                .show(ui, |ui| {
                    render_scene_mode_selector(ui, app, view_id, true);
                    match app.graph_view_scene_mode(view_id) {
                        SceneMode::Arrange => {
                            render_scene_authoring_toolbar(ui, app, view_id, true);
                            ui.small(
                                "Arrange mode foregrounds runtime scene authoring. Drag or resize regions directly on-canvas.",
                            );
                            if let Some(region_id) = app.graph_view_selected_scene_region(view_id)
                                && let Some(region) = app.graph_view_scene_region(view_id, region_id)
                            {
                                let label = region.label.as_deref().unwrap_or("Unlabeled Region");
                                ui.small(format!(
                                    "Selected: {label}. Drag or resize it on-canvas, or edit details in the Scene panel."
                                ));
                            }
                        }
                        SceneMode::Browse => {
                            ui.horizontal_wrapped(|ui| {
                                ui.label(egui::RichText::new("Scene").small().strong());
                                if ui.small_button("Open Panel").clicked() {
                                    app.open_scene_overlay(Some(view_id));
                                }
                            });
                            ui.small(
                                "Browse keeps scene affordances quiet. Switch to Arrange to create or edit regions.",
                            );
                        }
                        SceneMode::Simulate => {
                            render_scene_simulate_toolbar(ui, app, view_id, true);
                            ui.small(
                                "Simulate now foregrounds object legibility: reveal all node-objects and turn on scoped relation x-ray without leaving the graph. Use Arrange for region authoring.",
                            );
                        }
                    }
                });
        });
}

pub(crate) fn resolve_scene_surface_view_id(
    app: &GraphBrowserApp,
) -> Option<crate::app::GraphViewId> {
    app.workspace
        .chrome_ui
        .scene_overlay_view
        .filter(|view_id| app.workspace.graph_runtime.views.contains_key(view_id))
        .or_else(|| {
            app.workspace
                .graph_runtime
                .focused_view
                .filter(|view_id| app.workspace.graph_runtime.views.contains_key(view_id))
        })
        .or_else(|| {
            (app.workspace.graph_runtime.views.len() == 1)
                .then(|| app.workspace.graph_runtime.views.keys().next().copied())
                .flatten()
        })
}

pub(crate) fn render_scene_surface_in_ui(ui: &mut egui::Ui, app: &mut GraphBrowserApp) {
    let mut view_ids: Vec<_> = app.workspace.graph_runtime.views.keys().copied().collect();
    view_ids.sort_by_key(|view_id| view_id.as_uuid());

    if view_ids.is_empty() {
        ui.label("Scene tools need an active graph view.");
        ui.small("Open or focus a graph surface to author runtime scene regions.");
        return;
    }

    let mut view_id = resolve_scene_surface_view_id(app).unwrap_or(view_ids[0]);
    app.workspace.chrome_ui.scene_overlay_view = Some(view_id);

    if view_ids.len() > 1 {
        egui::ComboBox::from_label("Graph View")
            .selected_text(graph_view_scene_label(view_id))
            .show_ui(ui, |ui| {
                for candidate in view_ids.iter().copied() {
                    ui.selectable_value(&mut view_id, candidate, graph_view_scene_label(candidate));
                }
            });
        if app.workspace.chrome_ui.scene_overlay_view != Some(view_id) {
            app.workspace.chrome_ui.scene_overlay_view = Some(view_id);
        }
    } else {
        ui.label(
            egui::RichText::new(graph_view_scene_label(view_id))
                .small()
                .strong(),
        );
    }

    render_scene_mode_selector(ui, app, view_id, false);
    ui.add_space(4.0);

    let region_count = app
        .graph_view_scene_runtime(view_id)
        .map_or(0, |runtime| runtime.regions.len());
    let selected_label = app
        .graph_view_selected_scene_region(view_id)
        .and_then(|region_id| app.graph_view_scene_region(view_id, region_id))
        .and_then(|region| region.label.clone())
        .unwrap_or_else(|| "None".to_string());
    let bounds_status = if app
        .graph_view_scene_runtime(view_id)
        .and_then(|runtime| runtime.bounds_override)
        .is_some()
    {
        "View bounds active"
    } else {
        "No bounds override"
    };

    ui.small(format!(
        "{region_count} regions | Selected: {selected_label} | {bounds_status}"
    ));
    ui.separator();
    match app.graph_view_scene_mode(view_id) {
        SceneMode::Arrange => {
            render_scene_authoring_toolbar(ui, app, view_id, false);
            ui.small(
                "Scene regions are runtime-only for now. Drag or resize them on-canvas to shape the current graph view.",
            );
            render_selected_scene_region_inspector(ui, app, view_id);
        }
        SceneMode::Simulate => {
            render_scene_simulate_toolbar(ui, app, view_id, false);
            ui.small(
                "Simulate keeps authoring quieter and foregrounds scene legibility. Reveal Nodes halos every node-object; Relation X-Ray exposes the hovered or primary selected node's local graph structure.",
            );
        }
        SceneMode::Browse => {
            ui.small(
                "Switch this graph view to Arrange to foreground scene authoring, or Simulate to foreground object-world legibility.",
            );
        }
    }
}

fn graph_view_scene_label(view_id: crate::app::GraphViewId) -> String {
    let id = view_id.as_uuid().to_string();
    format!("Graph {}", &id[..8])
}

fn render_scene_authoring_toolbar(
    ui: &mut egui::Ui,
    app: &mut GraphBrowserApp,
    view_id: crate::app::GraphViewId,
    compact: bool,
) {
    ui.horizontal_wrapped(|ui| {
        ui.label(egui::RichText::new("Scene").small().strong());
        let panel_label = if app.workspace.chrome_ui.show_scene_overlay
            && resolve_scene_surface_view_id(app) == Some(view_id)
        {
            "Hide Panel"
        } else {
            "Open Panel"
        };
        if ui.small_button(panel_label).clicked() {
            if app.workspace.chrome_ui.show_scene_overlay
                && resolve_scene_surface_view_id(app) == Some(view_id)
            {
                app.close_scene_overlay();
            } else {
                app.open_scene_overlay(Some(view_id));
            }
        }

        let has_selection = app.get_single_selected_node_for_view(view_id).is_some();
        if ui
            .add_enabled(has_selection, egui::Button::new("Attractor"))
            .clicked()
        {
            if let Some(region) = scene_region_around_selected_node(
                app,
                view_id,
                "Attractor",
                SceneRegionEffect::Attractor { strength: 0.12 },
            ) {
                app.add_graph_view_scene_region(view_id, region);
            }
        }
        if ui
            .add_enabled(has_selection, egui::Button::new("Repulsor"))
            .clicked()
        {
            if let Some(region) = scene_region_around_selected_node(
                app,
                view_id,
                "Repulsor",
                SceneRegionEffect::Repulsor { strength: 12.0 },
            ) {
                app.add_graph_view_scene_region(view_id, region);
            }
        }
        if ui
            .add_enabled(has_selection, egui::Button::new("Dampener"))
            .clicked()
        {
            if let Some(region) = scene_region_around_selected_node(
                app,
                view_id,
                "Dampener",
                SceneRegionEffect::Dampener { factor: 0.5 },
            ) {
                app.add_graph_view_scene_region(view_id, region);
            }
        }
        if ui
            .add_enabled(has_selection, egui::Button::new("Wall Box"))
            .clicked()
        {
            if let Some(region) =
                scene_rect_around_selected_node(app, view_id, "Wall", SceneRegionEffect::Wall)
            {
                app.add_graph_view_scene_region(view_id, region);
            }
        }
        if ui.small_button("Use View Bounds").clicked() {
            if let Some(bounds) = app
                .workspace
                .graph_runtime
                .graph_view_canvas_rects
                .get(&view_id)
                .copied()
            {
                app.set_graph_view_scene_bounds_override(view_id, Some(bounds));
            }
        }
        if ui.small_button("Clear").clicked() {
            app.clear_graph_view_scene_runtime(view_id);
        }
    });

    if !compact {
        ui.add_space(4.0);
    }
}

fn render_scene_mode_selector(
    ui: &mut egui::Ui,
    app: &mut GraphBrowserApp,
    view_id: crate::app::GraphViewId,
    compact: bool,
) {
    let mut mode = app.graph_view_scene_mode(view_id);
    ui.horizontal_wrapped(|ui| {
        ui.label(egui::RichText::new("Mode").small().strong());
        ui.selectable_value(&mut mode, SceneMode::Browse, "Browse");
        ui.selectable_value(&mut mode, SceneMode::Arrange, "Arrange");
        ui.selectable_value(&mut mode, SceneMode::Simulate, "Simulate");
    });
    if mode != app.graph_view_scene_mode(view_id) {
        app.set_graph_view_scene_mode(view_id, mode);
        if mode == SceneMode::Arrange {
            app.workspace.chrome_ui.scene_overlay_view = Some(view_id);
        }
    }
    if !compact {
        ui.small(
            "Browse keeps the graph calm, Arrange foregrounds spatial authoring, and Simulate reserves the graph for richer scene behavior later.",
        );
    }
}

fn render_scene_simulate_toolbar(
    ui: &mut egui::Ui,
    app: &mut GraphBrowserApp,
    view_id: crate::app::GraphViewId,
    compact: bool,
) {
    let mut reveal_nodes = app.graph_view_scene_reveal_nodes(view_id);
    let mut relation_xray = app.graph_view_scene_relation_xray(view_id);
    let mut preset = app.graph_view_simulate_behavior_preset(view_id);
    ui.horizontal_wrapped(|ui| {
        ui.label(egui::RichText::new("Simulate").small().strong());
        let panel_label = if app.workspace.chrome_ui.show_scene_overlay
            && resolve_scene_surface_view_id(app) == Some(view_id)
        {
            "Hide Panel"
        } else {
            "Open Panel"
        };
        if ui.small_button(panel_label).clicked() {
            if app.workspace.chrome_ui.show_scene_overlay
                && resolve_scene_surface_view_id(app) == Some(view_id)
            {
                app.close_scene_overlay();
            } else {
                app.open_scene_overlay(Some(view_id));
            }
        }
        if ui.toggle_value(&mut reveal_nodes, "Reveal Nodes").changed() {
            app.set_graph_view_scene_reveal_nodes(view_id, reveal_nodes);
        }
        if ui
            .toggle_value(&mut relation_xray, "Relation X-Ray")
            .changed()
        {
            app.set_graph_view_scene_relation_xray(view_id, relation_xray);
        }
    });
    ui.horizontal_wrapped(|ui| {
        ui.label("Preset");
        ui.selectable_value(&mut preset, SimulateBehaviorPreset::Float, "Float");
        ui.selectable_value(&mut preset, SimulateBehaviorPreset::Packed, "Packed");
        ui.selectable_value(&mut preset, SimulateBehaviorPreset::Magnetic, "Magnetic");
    });
    if preset != app.graph_view_simulate_behavior_preset(view_id) {
        app.set_graph_view_simulate_behavior_preset(view_id, preset);
    }
    if compact {
        render_simulate_preset_compact_legend(ui, preset);
    } else {
        render_simulate_preset_preview(ui, preset);
    }
    if !compact {
        ui.small(simulate_preset_description(preset));
    }
}

fn simulate_preset_description(preset: SimulateBehaviorPreset) -> &'static str {
    match preset {
        SimulateBehaviorPreset::Float => {
            "Float keeps the scene gentle and roomy: light separation, contained drift, and soft region influence."
        }
        SimulateBehaviorPreset::Packed => {
            "Packed makes node-objects settle with stronger personal space and firmer containment, closer to a solid tabletop."
        }
        SimulateBehaviorPreset::Magnetic => {
            "Magnetic keeps objects contained but makes scene regions exert much stronger pull and push, so zones feel decisively active."
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct SimulatePresetVisualCue {
    accent: egui::Color32,
    cue: &'static str,
}

fn simulate_preset_visual_cue(preset: SimulateBehaviorPreset) -> SimulatePresetVisualCue {
    match preset {
        SimulateBehaviorPreset::Float => SimulatePresetVisualCue {
            accent: egui::Color32::from_rgb(176, 208, 236),
            cue: "Airy bounds, long glide",
        },
        SimulateBehaviorPreset::Packed => SimulatePresetVisualCue {
            accent: egui::Color32::from_rgb(236, 212, 164),
            cue: "Firm bounds, quick settle",
        },
        SimulateBehaviorPreset::Magnetic => SimulatePresetVisualCue {
            accent: egui::Color32::from_rgb(168, 224, 204),
            cue: "Strong zones, medium coast",
        },
    }
}

fn render_simulate_preset_compact_legend(ui: &mut egui::Ui, preset: SimulateBehaviorPreset) {
    let cue = simulate_preset_visual_cue(preset);
    ui.horizontal_wrapped(|ui| {
        ui.colored_label(cue.accent, "●");
        ui.small(cue.cue);
    });
}

fn render_simulate_preset_preview(ui: &mut egui::Ui, selected: SimulateBehaviorPreset) {
    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        for preset in [
            SimulateBehaviorPreset::Float,
            SimulateBehaviorPreset::Packed,
            SimulateBehaviorPreset::Magnetic,
        ] {
            let cue = simulate_preset_visual_cue(preset);
            let selected_preset = preset == selected;
            egui::Frame::group(ui.style())
                .fill(if selected_preset {
                    cue.accent.linear_multiply(0.16)
                } else {
                    egui::Color32::from_rgba_unmultiplied(255, 255, 255, 8)
                })
                .stroke(egui::Stroke::new(
                    if selected_preset { 1.5 } else { 1.0 },
                    if selected_preset {
                        cue.accent
                    } else {
                        egui::Color32::from_rgba_unmultiplied(255, 255, 255, 24)
                    },
                ))
                .corner_radius(egui::CornerRadius::same(8))
                .inner_margin(egui::Margin::symmetric(8, 6))
                .show(ui, |ui| {
                    ui.set_min_width(116.0);
                    ui.horizontal(|ui| {
                        ui.colored_label(cue.accent, "●");
                        ui.label(match preset {
                            SimulateBehaviorPreset::Float => "Float",
                            SimulateBehaviorPreset::Packed => "Packed",
                            SimulateBehaviorPreset::Magnetic => "Magnetic",
                        });
                    });
                    ui.small(cue.cue);
                });
        }
    });
}

fn render_selected_scene_region_inspector(
    ui: &mut egui::Ui,
    app: &mut GraphBrowserApp,
    view_id: crate::app::GraphViewId,
) {
    let Some(region_id) = app.graph_view_selected_scene_region(view_id) else {
        return;
    };
    let Some(region) = app.graph_view_scene_region(view_id, region_id).cloned() else {
        return;
    };

    ui.separator();
    ui.label(egui::RichText::new("Selected Region").small().strong());

    let mut label = region.label.clone().unwrap_or_default();
    ui.horizontal(|ui| {
        ui.label("Label");
        if ui.text_edit_singleline(&mut label).changed() {
            let mut updated = region.clone();
            let trimmed = label.trim();
            updated.label = (!trimmed.is_empty()).then(|| trimmed.to_string());
            let _ = app.replace_graph_view_scene_region(view_id, updated);
        }
    });

    let mut visible = region.visible;
    if ui.checkbox(&mut visible, "Visible").changed() {
        let mut updated = region.clone();
        updated.visible = visible;
        let _ = app.replace_graph_view_scene_region(view_id, updated);
    }

    ui.horizontal_wrapped(|ui| {
        ui.label("Effect");
        for (name, effect) in selected_scene_region_effect_options(region.effect) {
            let is_selected =
                scene_region_effect_kind(region.effect) == scene_region_effect_kind(effect);
            if ui.selectable_label(is_selected, name).clicked() && !is_selected {
                let mut updated = region.clone();
                updated.effect = effect;
                let _ = app.replace_graph_view_scene_region(view_id, updated);
            }
        }
    });

    match region.effect {
        SceneRegionEffect::Attractor { strength } => {
            let mut next_strength = strength;
            ui.horizontal(|ui| {
                ui.label("Strength");
                if ui
                    .add(egui::Slider::new(&mut next_strength, 0.02..=0.6).logarithmic(true))
                    .changed()
                {
                    let mut updated = region.clone();
                    updated.effect = SceneRegionEffect::Attractor {
                        strength: next_strength,
                    };
                    let _ = app.replace_graph_view_scene_region(view_id, updated);
                }
            });
        }
        SceneRegionEffect::Repulsor { strength } => {
            let mut next_strength = strength;
            ui.horizontal(|ui| {
                ui.label("Strength");
                if ui
                    .add(egui::Slider::new(&mut next_strength, 1.0..=32.0))
                    .changed()
                {
                    let mut updated = region.clone();
                    updated.effect = SceneRegionEffect::Repulsor {
                        strength: next_strength,
                    };
                    let _ = app.replace_graph_view_scene_region(view_id, updated);
                }
            });
        }
        SceneRegionEffect::Dampener { factor } => {
            let mut next_factor = factor;
            ui.horizontal(|ui| {
                ui.label("Factor");
                if ui
                    .add(egui::Slider::new(&mut next_factor, 0.05..=1.0))
                    .changed()
                {
                    let mut updated = region.clone();
                    updated.effect = SceneRegionEffect::Dampener {
                        factor: next_factor,
                    };
                    let _ = app.replace_graph_view_scene_region(view_id, updated);
                }
            });
        }
        SceneRegionEffect::Wall => {
            ui.small("Wall regions only constrain space; they do not apply a strength curve.");
        }
    }

    let view_selection = app.selection_for_view(view_id).clone();
    let selected_nodes: Vec<NodeKey> = view_selection.iter().copied().collect();
    let graphlet_nodes = if selected_nodes.is_empty() {
        Vec::new()
    } else {
        app.graphlet_members_for_nodes_in_view(&selected_nodes, Some(view_id))
    };
    let classification_candidates = gather_classification_candidates(app, &selected_nodes);
    let tag_candidates = gather_tag_candidates(app, &selected_nodes);
    let domain_candidates = gather_domain_candidates(app, &selected_nodes);
    let frame_candidates = gather_frame_candidates(app, &selected_nodes);
    let search_result_nodes = active_graph_search_node_keys(app);
    let filtered_view_nodes = filtered_view_node_keys(app, view_id);

    ui.separator();
    ui.label(egui::RichText::new("Gather Here").small().strong());
    ui.horizontal_wrapped(|ui| {
        if ui
            .add_enabled(
                !selected_nodes.is_empty(),
                egui::Button::new(format!("Selection ({})", selected_nodes.len())),
            )
            .clicked()
        {
            gather_node_keys_into_scene_region(app, &region, selected_nodes.clone());
        }
        if ui
            .add_enabled(
                !graphlet_nodes.is_empty(),
                egui::Button::new(format!("Graphlet ({})", graphlet_nodes.len())),
            )
            .clicked()
        {
            gather_node_keys_into_scene_region(app, &region, graphlet_nodes.clone());
        }
        if ui
            .add_enabled(
                !search_result_nodes.is_empty(),
                egui::Button::new(format!("Search Results ({})", search_result_nodes.len())),
            )
            .clicked()
        {
            gather_node_keys_into_scene_region(app, &region, search_result_nodes.clone());
        }
        if ui
            .add_enabled(
                !filtered_view_nodes.is_empty(),
                egui::Button::new(format!("Filtered View ({})", filtered_view_nodes.len())),
            )
            .clicked()
        {
            gather_node_keys_into_scene_region(app, &region, filtered_view_nodes.clone());
        }
    });
    render_scene_gather_candidate_picker(
        ui,
        "Class",
        ui.make_persistent_id(("scene_gather_class", view_id, format!("{region_id:?}"))),
        &classification_candidates,
        |classification| {
            gather_node_keys_into_scene_region(
                app,
                &region,
                node_keys_for_classification(app, classification),
            )
        },
    );
    render_scene_gather_candidate_picker(
        ui,
        "Tag",
        ui.make_persistent_id(("scene_gather_tag", view_id, format!("{region_id:?}"))),
        &tag_candidates,
        |tag| gather_node_keys_into_scene_region(app, &region, node_keys_for_tag(app, tag)),
    );
    render_scene_gather_candidate_picker(
        ui,
        "Domain",
        ui.make_persistent_id(("scene_gather_domain", view_id, format!("{region_id:?}"))),
        &domain_candidates,
        |domain| {
            gather_node_keys_into_scene_region(app, &region, node_keys_for_domain(app, domain))
        },
    );
    render_scene_gather_candidate_picker(
        ui,
        "Frame",
        ui.make_persistent_id(("scene_gather_frame", view_id, format!("{region_id:?}"))),
        &frame_candidates,
        |frame| gather_node_keys_into_scene_region(app, &region, node_keys_for_frame(app, frame)),
    );
    ui.small(
        "Gather packs the chosen nodes into this region using a stable layout. Pinned nodes stay put. Class/Tag/Domain/Frame candidates are derived from the current view selection, while Search Results and Filtered View reuse the active graph search and view filter state.",
    );

    if ui.small_button("Delete Region").clicked() {
        let _ = app.remove_graph_view_scene_region(view_id, region_id);
    }
}

fn scene_region_effect_kind(effect: SceneRegionEffect) -> &'static str {
    match effect {
        SceneRegionEffect::Attractor { .. } => "Attractor",
        SceneRegionEffect::Repulsor { .. } => "Repulsor",
        SceneRegionEffect::Dampener { .. } => "Dampener",
        SceneRegionEffect::Wall => "Wall",
    }
}

fn selected_scene_region_effect_options(
    effect: SceneRegionEffect,
) -> [(&'static str, SceneRegionEffect); 4] {
    let attractor_strength = match effect {
        SceneRegionEffect::Attractor { strength } => strength,
        _ => 0.12,
    };
    let repulsor_strength = match effect {
        SceneRegionEffect::Repulsor { strength } => strength,
        _ => 12.0,
    };
    let dampener_factor = match effect {
        SceneRegionEffect::Dampener { factor } => factor,
        _ => 0.5,
    };
    [
        (
            "Attractor",
            SceneRegionEffect::Attractor {
                strength: attractor_strength,
            },
        ),
        (
            "Repulsor",
            SceneRegionEffect::Repulsor {
                strength: repulsor_strength,
            },
        ),
        (
            "Dampener",
            SceneRegionEffect::Dampener {
                factor: dampener_factor,
            },
        ),
        ("Wall", SceneRegionEffect::Wall),
    ]
}

fn scene_region_around_selected_node(
    app: &GraphBrowserApp,
    view_id: crate::app::GraphViewId,
    label: &str,
    effect: SceneRegionEffect,
) -> Option<SceneRegionRuntime> {
    let key = app.get_single_selected_node_for_view(view_id)?;
    let center = app.domain_graph().node_projected_position(key)?.to_pos2();
    Some(SceneRegionRuntime::circle(center, 120.0, effect).with_label(format!("{label} Region")))
}

fn scene_rect_around_selected_node(
    app: &GraphBrowserApp,
    view_id: crate::app::GraphViewId,
    label: &str,
    effect: SceneRegionEffect,
) -> Option<SceneRegionRuntime> {
    let key = app.get_single_selected_node_for_view(view_id)?;
    let center = app.domain_graph().node_projected_position(key)?.to_pos2();
    let rect = egui::Rect::from_center_size(center, egui::vec2(220.0, 160.0));
    Some(SceneRegionRuntime::rect(rect, effect).with_label(format!("{label} Region")))
}

pub(crate) fn gather_node_keys_into_scene_region(
    app: &mut GraphBrowserApp,
    region: &SceneRegionRuntime,
    keys: Vec<NodeKey>,
) {
    let mut movable: Vec<NodeKey> = keys
        .into_iter()
        .filter(|key| {
            app.domain_graph()
                .get_node(*key)
                .is_some_and(|node| !node.is_pinned)
        })
        .collect();
    movable.sort_by_key(|key| key.index());
    movable.dedup();
    if movable.is_empty() {
        return;
    }

    let placements = scene_region_gather_positions(region, movable.len());
    for (key, position) in movable.into_iter().zip(placements.into_iter()) {
        let next = Point2D::new(position.x, position.y);
        let _ = app
            .domain_graph_mut()
            .set_node_projected_position(key, next);
        if let Some(state_mut) = app.workspace.graph_runtime.egui_state.as_mut()
            && let Some(node) = state_mut.graph.node_mut(key)
        {
            node.set_location(position);
        }
    }
    app.workspace.graph_runtime.egui_state_dirty = true;
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SceneGatherCandidate {
    id: String,
    label: String,
}

impl SceneGatherCandidate {
    fn plain(value: impl Into<String>) -> Self {
        let value = value.into();
        Self {
            id: value.clone(),
            label: value,
        }
    }
}

fn render_scene_gather_candidate_picker(
    ui: &mut egui::Ui,
    label: &str,
    id: egui::Id,
    candidates: &[SceneGatherCandidate],
    mut on_gather: impl FnMut(&str),
) {
    ui.horizontal_wrapped(|ui| {
        ui.label(label);
        let mut selected_id = ui.ctx().data_mut(|data| {
            data.get_persisted::<String>(id)
                .filter(|value| candidates.iter().any(|candidate| candidate.id == *value))
                .or_else(|| candidates.first().map(|candidate| candidate.id.clone()))
        });
        let enabled = !candidates.is_empty();
        let selected_text = selected_id
            .as_deref()
            .and_then(|selected| {
                candidates
                    .iter()
                    .find(|candidate| candidate.id == selected)
                    .map(|candidate| candidate.label.as_str())
            })
            .unwrap_or("None");
        egui::ComboBox::from_id_salt(id)
            .selected_text(selected_text)
            .width(180.0)
            .show_ui(ui, |ui| {
                for candidate in candidates {
                    ui.selectable_value(
                        &mut selected_id,
                        Some(candidate.id.clone()),
                        candidate.label.as_str(),
                    );
                }
            });
        ui.ctx().data_mut(|data| {
            if let Some(selected) = selected_id.clone() {
                data.insert_persisted(id, selected);
            } else {
                data.remove::<String>(id);
            }
        });
        if ui
            .add_enabled(enabled, egui::Button::new(format!("Gather {label}")))
            .clicked()
            && let Some(selected) = selected_id.as_deref()
        {
            on_gather(selected);
        }
    });
}

fn node_keys_for_tag(app: &GraphBrowserApp, tag: &str) -> Vec<NodeKey> {
    let mut keys: Vec<NodeKey> = app
        .domain_graph()
        .nodes()
        .filter_map(|(key, _)| {
            app.domain_graph()
                .node_tags(key)
                .is_some_and(|tags| tags.contains(tag))
                .then_some(key)
        })
        .collect();
    keys.sort_by_key(|key| key.index());
    keys
}

fn gather_classification_candidates(
    app: &GraphBrowserApp,
    selected_nodes: &[NodeKey],
) -> Vec<SceneGatherCandidate> {
    let mut candidates: Vec<SceneGatherCandidate> = selected_nodes
        .iter()
        .filter_map(|&key| app.domain_graph().node_classifications(key))
        .flat_map(|classifications| classifications.iter())
        .filter(|classification| classification.status != ClassificationStatus::Rejected)
        .map(|classification| {
            let label = classification
                .label
                .as_ref()
                .filter(|label| !label.trim().is_empty())
                .map_or_else(
                    || format!("{:?}: {}", classification.scheme, classification.value),
                    |label| {
                        format!(
                            "{:?}: {} ({})",
                            classification.scheme, label, classification.value
                        )
                    },
                );
            SceneGatherCandidate {
                id: classification_key(classification.scheme.clone(), &classification.value),
                label,
            }
        })
        .collect();
    candidates.sort_by(|a, b| a.label.cmp(&b.label).then_with(|| a.id.cmp(&b.id)));
    candidates.dedup_by(|a, b| a.id == b.id);
    candidates
}

fn gather_tag_candidates(
    app: &GraphBrowserApp,
    selected_nodes: &[NodeKey],
) -> Vec<SceneGatherCandidate> {
    let mut tags: Vec<String> = selected_nodes
        .iter()
        .filter_map(|&key| app.domain_graph().node_tags(key))
        .flat_map(|tags| tags.iter().cloned())
        .collect();
    tags.sort();
    tags.dedup();
    tags.into_iter().map(SceneGatherCandidate::plain).collect()
}

fn gather_domain_candidates(
    app: &GraphBrowserApp,
    selected_nodes: &[NodeKey],
) -> Vec<SceneGatherCandidate> {
    let mut domains: Vec<String> = selected_nodes
        .iter()
        .filter_map(|&key| app.domain_graph().get_node(key))
        .filter_map(|node| registrable_domain_key(node.url()))
        .collect();
    domains.sort();
    domains.dedup();
    domains
        .into_iter()
        .map(SceneGatherCandidate::plain)
        .collect()
}

fn gather_frame_candidates(
    app: &GraphBrowserApp,
    selected_nodes: &[NodeKey],
) -> Vec<SceneGatherCandidate> {
    let mut frames: Vec<String> = selected_nodes
        .iter()
        .flat_map(|&key| app.frames_for_node_key(key).iter().cloned())
        .collect();
    frames.sort();
    frames.dedup();
    frames
        .into_iter()
        .map(SceneGatherCandidate::plain)
        .collect()
}

fn classification_key(scheme: crate::model::graph::ClassificationScheme, value: &str) -> String {
    format!("{scheme:?}|{value}")
}

fn node_keys_for_classification(app: &GraphBrowserApp, classification: &str) -> Vec<NodeKey> {
    let mut keys: Vec<NodeKey> = app
        .domain_graph()
        .nodes()
        .filter_map(|(key, node)| {
            node.classifications
                .iter()
                .any(|candidate| {
                    candidate.status != ClassificationStatus::Rejected
                        && classification_key(candidate.scheme.clone(), &candidate.value)
                            == classification
                })
                .then_some(key)
        })
        .collect();
    keys.sort_by_key(|key| key.index());
    keys
}

pub(crate) fn active_graph_search_node_keys(app: &GraphBrowserApp) -> Vec<NodeKey> {
    let query = app.workspace.graph_runtime.active_graph_search_query.trim();
    if query.is_empty() {
        return Vec::new();
    }

    let mut ranked =
        crate::shell::desktop::runtime::registries::phase3_index_search(app, query, 64)
            .into_iter()
            .filter_map(|result| {
                match result.kind {
            crate::shell::desktop::runtime::registries::index::SearchResultKind::Node(key) => {
                Some(key)
            }
            crate::shell::desktop::runtime::registries::index::SearchResultKind::HistoryUrl(_)
            | crate::shell::desktop::runtime::registries::index::SearchResultKind::KnowledgeTag {
                ..
            } => None,
        }
            })
            .collect::<Vec<_>>();
    ranked.dedup();

    let Some(anchor) = app
        .workspace
        .graph_runtime
        .active_graph_search_neighborhood_anchor
    else {
        return ranked;
    };
    if app.domain_graph().get_node(anchor).is_none() {
        return ranked;
    }

    let mut seen: HashSet<NodeKey> = ranked.iter().copied().collect();
    let depth = app
        .workspace
        .graph_runtime
        .active_graph_search_neighborhood_depth
        .clamp(1, 2);
    for neighbor in std::iter::once(anchor).chain(
        app.domain_graph()
            .connected_candidates_with_depth(anchor, depth)
            .into_iter()
            .map(|(neighbor, _)| neighbor),
    ) {
        if seen.insert(neighbor) {
            ranked.push(neighbor);
        }
    }
    ranked
}

pub(crate) fn filtered_view_node_keys(
    app: &GraphBrowserApp,
    view_id: crate::app::GraphViewId,
) -> Vec<NodeKey> {
    let search_matches: HashSet<NodeKey> = active_graph_search_node_keys(app).into_iter().collect();
    let query_active = !app
        .workspace
        .graph_runtime
        .active_graph_search_query
        .trim()
        .is_empty();
    let Some(visible) = visible_nodes_for_view_filters(
        app,
        view_id,
        &search_matches,
        app.workspace.graph_runtime.search_display_mode,
        query_active,
    ) else {
        return Vec::new();
    };

    let mut keys: Vec<NodeKey> = visible.into_iter().collect();
    keys.sort_by_key(|key| key.index());
    keys
}

fn node_keys_for_domain(app: &GraphBrowserApp, domain: &str) -> Vec<NodeKey> {
    let mut keys: Vec<NodeKey> = app
        .domain_graph()
        .nodes()
        .filter_map(|(key, node)| {
            registrable_domain_key(node.url())
                .as_deref()
                .is_some_and(|candidate| candidate == domain)
                .then_some(key)
        })
        .collect();
    keys.sort_by_key(|key| key.index());
    keys
}

fn node_keys_for_frame(app: &GraphBrowserApp, frame: &str) -> Vec<NodeKey> {
    let mut keys: Vec<NodeKey> = app
        .domain_graph()
        .nodes()
        .filter_map(|(key, _)| app.frames_for_node_key(key).contains(frame).then_some(key))
        .collect();
    keys.sort_by_key(|key| key.index());
    keys
}

fn scene_region_gather_positions(region: &SceneRegionRuntime, count: usize) -> Vec<egui::Pos2> {
    match region.shape {
        SceneRegionShape::Rect { rect } => rect_gather_positions(rect, count),
        SceneRegionShape::Circle { center, radius } => {
            circle_gather_positions(center, radius, count)
        }
    }
}

fn rect_gather_positions(rect: egui::Rect, count: usize) -> Vec<egui::Pos2> {
    let inset = 18.0;
    let stride = 54.0;
    let usable = egui::Rect::from_min_max(
        rect.min + egui::vec2(inset, inset),
        rect.max - egui::vec2(inset, inset),
    );
    let cols = ((usable.width().max(stride) / stride).floor() as usize).max(1);

    (0..count)
        .map(|index| {
            let col = index % cols;
            let row = index / cols;
            let x = (usable.left() + stride * 0.5 + col as f32 * stride).min(usable.right() - 12.0);
            let y = (usable.top() + stride * 0.5 + row as f32 * stride).min(usable.bottom() - 12.0);
            egui::pos2(x, y)
        })
        .collect()
}

fn circle_gather_positions(center: egui::Pos2, radius: f32, count: usize) -> Vec<egui::Pos2> {
    if count == 0 {
        return Vec::new();
    }
    if count == 1 {
        return vec![center];
    }

    let max_radius = (radius - 18.0).max(12.0);
    let radial_step = 42.0;
    (0..count)
        .map(|index| {
            if index == 0 {
                return center;
            }
            let ring = 1 + ((index - 1) / 6) as i32;
            let ring_index = ((index - 1) % 6) as f32;
            let angle = ring_index / 6.0 * std::f32::consts::TAU + ring as f32 * 0.35;
            let distance = (ring as f32 * radial_step).min(max_radius);
            center + egui::vec2(angle.cos() * distance, angle.sin() * distance)
        })
        .collect()
}

fn registrable_domain_key(url: &str) -> Option<String> {
    let parsed = url::Url::parse(url).ok()?;
    let host = parsed
        .host_str()?
        .trim_start_matches("www.")
        .to_ascii_lowercase();
    if host.parse::<std::net::IpAddr>().is_ok() {
        return Some(host);
    }

    let labels: Vec<&str> = host
        .split('.')
        .filter(|segment| !segment.is_empty())
        .collect();
    if labels.len() <= 2 {
        return Some(host);
    }

    let common_country_slds = ["ac", "co", "com", "edu", "gov", "net", "org"];
    let tail_len = if labels.last().is_some_and(|tld| tld.len() == 2)
        && labels
            .get(labels.len().saturating_sub(2))
            .is_some_and(|sld| common_country_slds.contains(sld))
        && labels.len() >= 3
    {
        3
    } else {
        2
    };

    Some(labels[labels.len() - tail_len..].join("."))
}

// ── Layout algorithm helpers ───────────────────────────────────────────────────

pub(super) fn requested_layout_algorithm_id(
    app: &GraphBrowserApp,
    view_id: crate::app::GraphViewId,
    canvas_profile: &crate::registries::domain::layout::canvas::CanvasSurfaceProfile,
) -> String {
    use crate::app::graph_layout::layout_algorithm_id_for_mode;
    app.workspace
        .graph_runtime
        .views
        .get(&view_id)
        .map(|view| match view.resolved_layout_mode() {
            crate::registries::atomic::lens::LayoutMode::Free => {
                view.resolved_layout_algorithm_id().to_string()
            }
            other => layout_algorithm_id_for_mode(other).to_string(),
        })
        .unwrap_or_else(|| canvas_profile.layout_algorithm.algorithm_id.clone())
}

pub(super) fn should_apply_layout_algorithm(
    app: &GraphBrowserApp,
    view_id: crate::app::GraphViewId,
    resolved_layout_id: &str,
) -> bool {
    let layout_changed = app
        .workspace
        .graph_runtime
        .views
        .get(&view_id)
        .and_then(|view| view.last_layout_algorithm_id.as_deref())
        != Some(resolved_layout_id);

    app.workspace.graph_runtime.egui_state.is_none()
        || app.workspace.graph_runtime.egui_state_dirty
        || layout_changed
}

// ── Semantic depth badge ──────────────────────────────────────────────────────

pub(super) fn graph_view_semantic_depth_status_badge(
    app: &GraphBrowserApp,
    view_id: crate::app::GraphViewId,
) -> Option<(&'static str, &'static str)> {
    app.workspace.graph_runtime.views.get(&view_id).and_then(|view| {
        matches!(
            view.dimension,
            ViewDimension::ThreeD {
                mode: ThreeDMode::TwoPointFive,
                z_source: ZSource::UdcLevel { .. }
            }
        )
        .then_some((
            "UDC Depth",
            "Semantic depth view is active for this Graph View. Nodes are lifted into UDC depth layers until you restore the previous view.",
        ))
    })
}

// ── Enrichment summary ────────────────────────────────────────────────────────

pub(super) fn selected_node_enrichment_summary(
    app: &GraphBrowserApp,
    selected_key: NodeKey,
) -> Option<SelectedNodeEnrichmentSummary> {
    use crate::graph::NodeLifecycle;
    let node = app.domain_graph().get_node(selected_key)?;
    let mut display_tags =
        crate::shell::desktop::runtime::registries::knowledge::tags_for_node(app, &selected_key)
            .into_iter()
            .map(semantic_tag_status_chip)
            .collect::<Vec<_>>();
    let hidden_tag_count = display_tags.len().saturating_sub(4);
    display_tags.truncate(4);

    let suggested_tags = app
        .suggested_semantic_tags_for_node(selected_key)
        .into_iter()
        .map(|tag| semantic_suggestion_chip(app, selected_key, tag))
        .collect::<Vec<_>>();

    let placement_anchor =
        crate::shell::desktop::runtime::registries::phase3_suggest_semantic_placement_anchor(
            app,
            selected_key,
        )
        .and_then(|anchor_key| {
            app.domain_graph().get_node(anchor_key).map(|anchor| {
                let label = if anchor.title.is_empty() {
                    anchor.url().to_string()
                } else {
                    anchor.title.clone()
                };
                let slice_tag =
                    crate::shell::desktop::runtime::registries::knowledge::tags_for_node(
                        app,
                        &anchor_key,
                    )
                    .into_iter()
                    .next();
                PlacementAnchorSummary {
                    key: anchor_key,
                    label,
                    slice_tag,
                }
            })
        });

    use super::semantic_tags::ClassificationChip;
    use crate::graph::{ClassificationProvenance, ClassificationStatus};
    let resolution_audits = latest_identity_resolution_audits(app, selected_key);
    let display_classifications: Vec<ClassificationChip> = app
        .domain_graph()
        .node_classifications(selected_key)
        .map(|classifications| {
            classifications
                .iter()
                .map(|c| {
                    let scheme = format!("{:?}", c.scheme);
                    let resolution_detail = parse_resolution_chip_metadata(
                        &c.scheme,
                        &c.value,
                        resolution_audits.get(&scheme),
                    );
                    ClassificationChip {
                        value: c.value.clone(),
                        label: resolution_detail
                            .as_ref()
                            .map(|detail| detail.label.clone())
                            .or_else(|| c.label.clone())
                            .unwrap_or_else(|| c.value.clone()),
                        provenance: match c.provenance {
                            ClassificationProvenance::UserAuthored => "User",
                            ClassificationProvenance::Imported => "Imported",
                            ClassificationProvenance::InheritedFromSource => "Inherited",
                            ClassificationProvenance::RegistryDerived => "Registry",
                            ClassificationProvenance::AgentSuggested => "Agent",
                            ClassificationProvenance::CommunitySynced => "Community",
                        },
                        status: match c.status {
                            ClassificationStatus::Accepted => "Accepted",
                            ClassificationStatus::Suggested => "Suggested",
                            ClassificationStatus::Rejected => "Rejected",
                            ClassificationStatus::Verified => "Verified",
                            ClassificationStatus::Imported => "Imported",
                        },
                        confidence: c.confidence,
                        primary: c.primary,
                        scheme: scheme.clone(),
                        metadata: resolution_detail
                            .as_ref()
                            .map(|detail| detail.metadata.clone()),
                        hover_detail: resolution_detail
                            .as_ref()
                            .map(|detail| detail.hover.clone()),
                        node_key: selected_key,
                        classification_value: c.value.clone(),
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    Some(SelectedNodeEnrichmentSummary {
        title: if node.title.is_empty() {
            node.url().to_string()
        } else {
            node.title.clone()
        },
        url: node.url().to_string(),
        show_url: !node.title.is_empty() && node.title != node.url(),
        lifecycle: match node.lifecycle {
            NodeLifecycle::Active => "Active",
            NodeLifecycle::Warm => "Warm",
            NodeLifecycle::Cold => "Cold",
            NodeLifecycle::Tombstone => "Ghost Node",
        },
        import_records: app
            .domain_graph()
            .import_record_summaries_for_node(selected_key),
        workspace_memberships: app.membership_for_node(node.id).iter().cloned().collect(),
        display_tags,
        hidden_tag_count,
        suggested_tags,
        placement_anchor,
        display_classifications,
        semantic_lens_available: app
            .workspace
            .graph_runtime
            .semantic_index
            .contains_key(&selected_key),
        semantic_lens_active: app
            .workspace
            .graph_runtime
            .focused_view
            .and_then(|view_id| app.workspace.graph_runtime.views.get(&view_id))
            .is_some_and(|view| {
                matches!(
                    view.dimension,
                    ViewDimension::ThreeD {
                        mode: ThreeDMode::TwoPointFive,
                        z_source: ZSource::UdcLevel { .. }
                    }
                )
            }),
    })
}

struct ResolutionChipMetadata {
    label: String,
    metadata: String,
    hover: String,
}

fn latest_identity_resolution_audits(
    app: &GraphBrowserApp,
    selected_key: NodeKey,
) -> HashMap<String, graphshell_comms::identity::IdentityResolutionAuditRecord> {
    let Some(node) = app.domain_graph().get_node(selected_key) else {
        return HashMap::new();
    };
    app.node_audit_history_entries(node.id, 32)
        .into_iter()
        .filter_map(|entry| match entry {
            crate::services::persistence::types::LogEntry::AppendNodeAuditEvent {
                event, ..
            } => match event {
                crate::services::persistence::types::NodeAuditEventKind::ActionRecorded {
                    action,
                    detail,
                } => graphshell_comms::identity::parse_identity_resolution_audit_event(
                    &action, &detail,
                ),
                _ => None,
            },
            _ => None,
        })
        .fold(HashMap::new(), |mut acc, record| {
            acc.entry(format!("Custom(\"resolution:{}\")", record.protocol.key()))
                .or_insert(record);
            acc
        })
}

fn parse_resolution_chip_metadata(
    scheme: &crate::model::graph::ClassificationScheme,
    value: &str,
    audit: Option<&graphshell_comms::identity::IdentityResolutionAuditRecord>,
) -> Option<ResolutionChipMetadata> {
    let protocol = match scheme {
        crate::model::graph::ClassificationScheme::Custom(custom) => custom
            .strip_prefix("resolution:")
            .and_then(graphshell_comms::capabilities::MiddlenetProtocol::from_key)?,
        _ => return None,
    };
    let descriptor = graphshell_comms::capabilities::descriptor(protocol);
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let (freshness, cache_state, age_label, sources, changed_label) = if let Some(audit) = audit {
        let age_ms = now_ms.saturating_sub(audit.resolved_at_ms);
        let age_label = if age_ms < 60_000 {
            format!("{}s ago", age_ms / 1_000)
        } else if age_ms < 3_600_000 {
            format!("{}m ago", age_ms / 60_000)
        } else if age_ms < 86_400_000 {
            format!("{}h ago", age_ms / 3_600_000)
        } else {
            format!("{}d ago", age_ms / 86_400_000)
        };
        (
            audit.freshness.label().to_string(),
            match audit.cache_state {
                graphshell_comms::identity::IdentityResolutionCacheState::Hit => "Cache hit",
                graphshell_comms::identity::IdentityResolutionCacheState::Miss => "Cache miss",
            }
            .to_string(),
            age_label,
            if audit.source_endpoints.is_empty() {
                "No source endpoints recorded".to_string()
            } else {
                format!("Sources: {}", audit.source_endpoints.join(", "))
            },
            audit
                .changed
                .map(|changed| if changed { "Changed" } else { "Unchanged" }.to_string()),
        )
    } else {
        (
            "No audit".to_string(),
            "Cache unknown".to_string(),
            "age unknown".to_string(),
            "No source endpoints recorded".to_string(),
            None,
        )
    };
    let metadata = if let Some(changed) = changed_label {
        format!(
            "resolution:{} · {} · {} · {} · {}",
            protocol.key(),
            freshness,
            cache_state,
            age_label,
            changed
        )
    } else {
        format!(
            "resolution:{} · {} · {} · {}",
            protocol.key(),
            freshness,
            cache_state,
            age_label
        )
    };
    Some(ResolutionChipMetadata {
        label: format!("Resolved via {}", descriptor.display_name),
        metadata,
        hover: format!("Query: {}\n{}", value, sources),
    })
}

#[cfg(test)]
mod tests {
    use super::{
        SceneGatherCandidate, filtered_view_node_keys, gather_classification_candidates,
        gather_domain_candidates, gather_frame_candidates, gather_node_keys_into_scene_region,
        gather_tag_candidates, node_keys_for_classification, node_keys_for_domain,
        node_keys_for_frame, node_keys_for_tag, resolve_scene_surface_view_id,
        scene_rect_around_selected_node, scene_region_around_selected_node,
    };
    use crate::app::{GraphBrowserApp, GraphViewId, GraphViewState};
    use crate::graph::scene_runtime::{SceneRegionEffect, SceneRegionShape};
    use crate::model::graph::filter::{FacetExpr, FacetOperand, FacetOperator, FacetPredicate};
    use crate::model::graph::{
        ClassificationProvenance, ClassificationScheme, ClassificationStatus, NodeClassification,
    };
    use crate::util::CoordBridge;
    use euclid::default::Point2D;

    fn plain_candidates(values: &[&str]) -> Vec<SceneGatherCandidate> {
        values
            .iter()
            .map(|value| SceneGatherCandidate::plain((*value).to_string()))
            .collect()
    }

    #[test]
    fn scene_region_around_selected_node_builds_labeled_circle() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_id = GraphViewId::new();
        app.workspace
            .graph_runtime
            .views
            .insert(view_id, GraphViewState::new_with_id(view_id, "Scene"));
        app.workspace.graph_runtime.focused_view = Some(view_id);
        let key = app.add_node_and_sync("https://example.com".into(), Point2D::new(40.0, 60.0));
        app.select_node(key, false);

        let region = scene_region_around_selected_node(
            &app,
            view_id,
            "Attractor",
            SceneRegionEffect::Attractor { strength: 0.12 },
        )
        .expect("selected node should produce a scene region");

        assert_eq!(region.label.as_deref(), Some("Attractor Region"));
        assert_eq!(
            region.shape,
            SceneRegionShape::Circle {
                center: egui::pos2(40.0, 60.0),
                radius: 120.0,
            }
        );
    }

    #[test]
    fn scene_rect_around_selected_node_builds_labeled_rect() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_id = GraphViewId::new();
        app.workspace
            .graph_runtime
            .views
            .insert(view_id, GraphViewState::new_with_id(view_id, "Scene"));
        app.workspace.graph_runtime.focused_view = Some(view_id);
        let key = app.add_node_and_sync("https://example.com".into(), Point2D::new(10.0, 20.0));
        app.select_node(key, false);

        let region =
            scene_rect_around_selected_node(&app, view_id, "Wall", SceneRegionEffect::Wall)
                .expect("selected node should produce a wall box region");

        assert_eq!(region.label.as_deref(), Some("Wall Region"));
        assert_eq!(
            region.shape,
            SceneRegionShape::Rect {
                rect: egui::Rect::from_center_size(
                    egui::pos2(10.0, 20.0),
                    egui::vec2(220.0, 160.0),
                ),
            }
        );
    }

    #[test]
    fn scene_region_around_selected_node_uses_view_scoped_selection() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_a = GraphViewId::new();
        let view_b = GraphViewId::new();
        app.workspace
            .graph_runtime
            .views
            .insert(view_a, GraphViewState::new_with_id(view_a, "A"));
        app.workspace
            .graph_runtime
            .views
            .insert(view_b, GraphViewState::new_with_id(view_b, "B"));

        let a = app.add_node_and_sync("https://a.example".into(), Point2D::new(10.0, 20.0));
        let b = app.add_node_and_sync("https://b.example".into(), Point2D::new(80.0, 90.0));

        app.workspace.graph_runtime.focused_view = Some(view_a);
        app.select_node(a, false);
        app.workspace.graph_runtime.focused_view = Some(view_b);
        app.select_node(b, false);

        let region = scene_region_around_selected_node(
            &app,
            view_a,
            "Attractor",
            SceneRegionEffect::Attractor { strength: 0.12 },
        )
        .expect("view-scoped selection should resolve for requested view");

        assert_eq!(
            region.shape,
            SceneRegionShape::Circle {
                center: egui::pos2(10.0, 20.0),
                radius: 120.0,
            }
        );
    }

    #[test]
    fn resolve_scene_surface_view_prefers_valid_overlay_target() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_a = GraphViewId::new();
        let view_b = GraphViewId::new();
        app.workspace
            .graph_runtime
            .views
            .insert(view_a, GraphViewState::new_with_id(view_a, "A"));
        app.workspace
            .graph_runtime
            .views
            .insert(view_b, GraphViewState::new_with_id(view_b, "B"));
        app.workspace.graph_runtime.focused_view = Some(view_a);
        app.workspace.chrome_ui.scene_overlay_view = Some(view_b);

        assert_eq!(resolve_scene_surface_view_id(&app), Some(view_b));
    }

    #[test]
    fn resolve_scene_surface_view_falls_back_to_focused_view_when_target_missing() {
        let mut app = GraphBrowserApp::new_for_testing();
        let missing = GraphViewId::new();
        let focused = GraphViewId::new();
        app.workspace
            .graph_runtime
            .views
            .insert(focused, GraphViewState::new_with_id(focused, "Focused"));
        app.workspace.graph_runtime.focused_view = Some(focused);
        app.workspace.chrome_ui.scene_overlay_view = Some(missing);

        assert_eq!(resolve_scene_surface_view_id(&app), Some(focused));
    }

    #[test]
    fn gather_node_keys_into_rect_region_repositions_nodes_inside_region() {
        let mut app = GraphBrowserApp::new_for_testing();
        let a = app.add_node_and_sync("https://a.example".into(), Point2D::new(-200.0, -200.0));
        let b = app.add_node_and_sync("https://b.example".into(), Point2D::new(300.0, 250.0));
        let region = crate::graph::scene_runtime::SceneRegionRuntime::rect(
            egui::Rect::from_center_size(egui::pos2(100.0, 120.0), egui::vec2(220.0, 160.0)),
            SceneRegionEffect::Attractor { strength: 0.12 },
        );

        gather_node_keys_into_scene_region(&mut app, &region, vec![a, b]);

        let after_a = app.domain_graph().node_projected_position(a).unwrap();
        let after_b = app.domain_graph().node_projected_position(b).unwrap();
        match region.shape {
            SceneRegionShape::Rect { rect } => {
                assert!(rect.contains(after_a.to_pos2()));
                assert!(rect.contains(after_b.to_pos2()));
            }
            _ => panic!("expected rect region"),
        }
    }

    #[test]
    fn gather_node_keys_into_region_skips_pinned_nodes() {
        let mut app = GraphBrowserApp::new_for_testing();
        let pinned =
            app.add_node_and_sync("https://pinned.example".into(), Point2D::new(10.0, 15.0));
        let moved =
            app.add_node_and_sync("https://moved.example".into(), Point2D::new(300.0, 250.0));
        app.domain_graph_mut()
            .get_node_mut(pinned)
            .expect("pinned node exists")
            .is_pinned = true;
        let before_pinned = app.domain_graph().node_projected_position(pinned).unwrap();
        let region = crate::graph::scene_runtime::SceneRegionRuntime::circle(
            egui::pos2(80.0, 90.0),
            120.0,
            SceneRegionEffect::Repulsor { strength: 6.0 },
        );

        gather_node_keys_into_scene_region(&mut app, &region, vec![pinned, moved]);

        let after_pinned = app.domain_graph().node_projected_position(pinned).unwrap();
        let after_moved = app.domain_graph().node_projected_position(moved).unwrap();
        assert_eq!(after_pinned, before_pinned);
        match region.shape {
            SceneRegionShape::Circle { center, radius } => {
                assert!((after_moved.to_pos2() - center).length() <= radius);
            }
            _ => panic!("expected circle region"),
        }
    }

    #[test]
    fn node_keys_for_tag_returns_matching_nodes() {
        let mut app = GraphBrowserApp::new_for_testing();
        let tagged = app.add_node_and_sync("https://tagged.example".into(), Point2D::new(0.0, 0.0));
        let other = app.add_node_and_sync("https://other.example".into(), Point2D::new(10.0, 0.0));
        let _ = app
            .workspace
            .domain
            .graph
            .insert_node_tag(tagged, "focus".to_string());
        let _ = app
            .workspace
            .domain
            .graph
            .insert_node_tag(other, "archive".to_string());

        assert_eq!(node_keys_for_tag(&app, "focus"), vec![tagged]);
    }

    #[test]
    fn node_keys_for_domain_groups_by_registrable_domain() {
        let mut app = GraphBrowserApp::new_for_testing();
        let a = app.add_node_and_sync(
            "https://www.docs.example.co.uk/a".into(),
            Point2D::new(0.0, 0.0),
        );
        let b = app.add_node_and_sync(
            "https://blog.example.co.uk/b".into(),
            Point2D::new(10.0, 0.0),
        );
        let c = app.add_node_and_sync("https://other.test/c".into(), Point2D::new(20.0, 0.0));

        assert_eq!(node_keys_for_domain(&app, "example.co.uk"), vec![a, b]);
        assert_eq!(node_keys_for_domain(&app, "other.test"), vec![c]);
    }

    #[test]
    fn node_keys_for_frame_returns_membership_matches() {
        let mut app = GraphBrowserApp::new_for_testing();
        let a = app.add_node_and_sync("https://a.example".into(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("https://b.example".into(), Point2D::new(10.0, 0.0));
        let frame_name = "Research".to_string();
        let a_uuid = app.domain_graph().get_node(a).unwrap().id;
        let b_uuid = app.domain_graph().get_node(b).unwrap().id;
        app.workspace
            .workbench_session
            .node_workspace_membership
            .entry(a_uuid)
            .or_default()
            .insert(frame_name.clone());
        app.workspace
            .workbench_session
            .node_workspace_membership
            .entry(b_uuid)
            .or_default()
            .insert("Archive".to_string());

        assert_eq!(node_keys_for_frame(&app, &frame_name), vec![a]);
    }

    #[test]
    fn gather_classification_candidates_dedup_and_skip_rejected_records() {
        let mut app = GraphBrowserApp::new_for_testing();
        let a = app.add_node_and_sync("https://a.example".into(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("https://b.example".into(), Point2D::new(10.0, 0.0));
        app.workspace.domain.graph.add_node_classification(
            a,
            NodeClassification {
                scheme: ClassificationScheme::Udc,
                value: "udc:51".to_string(),
                label: Some("Mathematics".to_string()),
                confidence: 0.9,
                provenance: ClassificationProvenance::RegistryDerived,
                status: ClassificationStatus::Accepted,
                primary: true,
            },
        );
        app.workspace.domain.graph.add_node_classification(
            b,
            NodeClassification {
                scheme: ClassificationScheme::Udc,
                value: "udc:51".to_string(),
                label: Some("Mathematics".to_string()),
                confidence: 0.7,
                provenance: ClassificationProvenance::AgentSuggested,
                status: ClassificationStatus::Suggested,
                primary: false,
            },
        );
        app.workspace.domain.graph.add_node_classification(
            b,
            NodeClassification {
                scheme: ClassificationScheme::ContentKind,
                value: "article".to_string(),
                label: Some("Article".to_string()),
                confidence: 0.3,
                provenance: ClassificationProvenance::AgentSuggested,
                status: ClassificationStatus::Rejected,
                primary: false,
            },
        );

        assert_eq!(
            gather_classification_candidates(&app, &[a, b]),
            vec![SceneGatherCandidate {
                id: "Udc|udc:51".to_string(),
                label: "Udc: Mathematics (udc:51)".to_string(),
            }]
        );
    }

    #[test]
    fn node_keys_for_classification_returns_matching_non_rejected_nodes() {
        let mut app = GraphBrowserApp::new_for_testing();
        let a = app.add_node_and_sync("https://a.example".into(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("https://b.example".into(), Point2D::new(10.0, 0.0));
        app.workspace.domain.graph.add_node_classification(
            a,
            NodeClassification {
                scheme: ClassificationScheme::Udc,
                value: "udc:51".to_string(),
                label: None,
                confidence: 1.0,
                provenance: ClassificationProvenance::UserAuthored,
                status: ClassificationStatus::Accepted,
                primary: true,
            },
        );
        app.workspace.domain.graph.add_node_classification(
            b,
            NodeClassification {
                scheme: ClassificationScheme::Udc,
                value: "udc:51".to_string(),
                label: None,
                confidence: 0.4,
                provenance: ClassificationProvenance::AgentSuggested,
                status: ClassificationStatus::Rejected,
                primary: false,
            },
        );

        assert_eq!(node_keys_for_classification(&app, "Udc|udc:51"), vec![a]);
    }

    #[test]
    fn gather_tag_candidates_dedup_and_sort_selection_tags() {
        let mut app = GraphBrowserApp::new_for_testing();
        let a = app.add_node_and_sync("https://a.example".into(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("https://b.example".into(), Point2D::new(10.0, 0.0));
        let _ = app
            .workspace
            .domain
            .graph
            .insert_node_tag(a, "focus".to_string());
        let _ = app
            .workspace
            .domain
            .graph
            .insert_node_tag(a, "work".to_string());
        let _ = app
            .workspace
            .domain
            .graph
            .insert_node_tag(b, "focus".to_string());

        assert_eq!(
            gather_tag_candidates(&app, &[a, b]),
            plain_candidates(&["focus", "work"])
        );
    }

    #[test]
    fn gather_domain_candidates_dedup_and_sort_selection_domains() {
        let mut app = GraphBrowserApp::new_for_testing();
        let a = app.add_node_and_sync(
            "https://www.docs.example.com/a".into(),
            Point2D::new(0.0, 0.0),
        );
        let b = app.add_node_and_sync("https://blog.example.com/b".into(), Point2D::new(10.0, 0.0));
        let c = app.add_node_and_sync("https://other.test/c".into(), Point2D::new(20.0, 0.0));

        assert_eq!(
            gather_domain_candidates(&app, &[a, b, c]),
            plain_candidates(&["example.com", "other.test"])
        );
    }

    #[test]
    fn gather_frame_candidates_dedup_and_sort_selection_frames() {
        let mut app = GraphBrowserApp::new_for_testing();
        let a = app.add_node_and_sync("https://a.example".into(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("https://b.example".into(), Point2D::new(10.0, 0.0));
        let a_uuid = app.domain_graph().get_node(a).unwrap().id;
        let b_uuid = app.domain_graph().get_node(b).unwrap().id;
        app.workspace
            .workbench_session
            .node_workspace_membership
            .entry(a_uuid)
            .or_default()
            .extend(["Research".to_string(), "Archive".to_string()]);
        app.workspace
            .workbench_session
            .node_workspace_membership
            .entry(b_uuid)
            .or_default()
            .insert("Research".to_string());

        assert_eq!(
            gather_frame_candidates(&app, &[a, b]),
            plain_candidates(&["Archive", "Research"])
        );
    }

    #[test]
    fn filtered_view_node_keys_returns_current_view_filter_matches() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_id = GraphViewId::new();
        let mut view = GraphViewState::new_with_id(view_id, "Scene");
        view.apply_filter_override(Some(FacetExpr::Predicate(FacetPredicate {
            facet_key: "domain".to_string(),
            operator: FacetOperator::Eq,
            operand: FacetOperand::Scalar(crate::model::graph::filter::FacetScalar::Text(
                "example.com".to_string(),
            )),
        })));
        app.workspace.graph_runtime.views.insert(view_id, view);

        let a = app.add_node_and_sync("https://example.com/a".into(), Point2D::new(0.0, 0.0));
        let b = app.add_node_and_sync("https://other.test/b".into(), Point2D::new(10.0, 0.0));

        assert_eq!(filtered_view_node_keys(&app, view_id), vec![a]);
        assert_ne!(filtered_view_node_keys(&app, view_id), vec![b]);
    }

    #[test]
    fn search_result_gather_reuses_active_graph_search_query() {
        let mut app = GraphBrowserApp::new_for_testing();
        let a = app.add_node_and_sync("https://example.com/alpha".into(), Point2D::new(0.0, 0.0));
        let _ = app.add_node_and_sync("https://other.test/beta".into(), Point2D::new(10.0, 0.0));
        app.workspace.graph_runtime.active_graph_search_query = "alpha".to_string();

        let matches = super::active_graph_search_node_keys(&app);
        assert!(matches.contains(&a));
        assert_eq!(matches.len(), 1);
    }

    #[test]
    fn search_result_gather_extends_with_active_anchor_neighborhood() {
        let mut app = GraphBrowserApp::new_for_testing();
        let anchor =
            app.add_node_and_sync("https://example.com/anchor".into(), Point2D::new(0.0, 0.0));
        let neighbor = app.add_node_and_sync(
            "https://other.test/neighbor".into(),
            Point2D::new(10.0, 0.0),
        );
        let _ = app.domain_graph_mut().add_edge(
            anchor,
            neighbor,
            crate::graph::EdgeType::Hyperlink,
            None,
        );
        app.workspace.graph_runtime.active_graph_search_query = "anchor".to_string();
        app.workspace
            .graph_runtime
            .active_graph_search_neighborhood_anchor = Some(anchor);
        app.workspace
            .graph_runtime
            .active_graph_search_neighborhood_depth = 1;

        let matches = super::active_graph_search_node_keys(&app);
        assert!(matches.contains(&anchor));
        assert!(matches.contains(&neighbor));
        assert_eq!(matches.len(), 2);
    }

    #[test]
    fn filtered_view_node_keys_intersects_search_filter_and_view_filter() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_id = GraphViewId::new();
        let mut view = GraphViewState::new_with_id(view_id, "Scene");
        view.apply_filter_override(Some(FacetExpr::Predicate(FacetPredicate {
            facet_key: "domain".to_string(),
            operator: FacetOperator::Eq,
            operand: FacetOperand::Scalar(crate::model::graph::filter::FacetScalar::Text(
                "example.com".to_string(),
            )),
        })));
        app.workspace.graph_runtime.views.insert(view_id, view);
        let alpha =
            app.add_node_and_sync("https://example.com/alpha".into(), Point2D::new(0.0, 0.0));
        let _beta =
            app.add_node_and_sync("https://example.com/beta".into(), Point2D::new(10.0, 0.0));
        let _other =
            app.add_node_and_sync("https://other.test/alpha".into(), Point2D::new(20.0, 0.0));
        app.workspace.graph_runtime.active_graph_search_query = "alpha".to_string();
        app.workspace.graph_runtime.search_display_mode = crate::app::SearchDisplayMode::Filter;

        assert_eq!(filtered_view_node_keys(&app, view_id), vec![alpha]);
    }

    #[test]
    fn filtered_view_node_keys_respects_owned_node_mask_even_without_filter_expr() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_id = GraphViewId::new();
        let mut view = GraphViewState::new_with_id(view_id, "Scene");
        let a = app.add_node_and_sync("https://example.com/a".into(), Point2D::new(0.0, 0.0));
        let _b = app.add_node_and_sync("https://example.com/b".into(), Point2D::new(10.0, 0.0));
        view.owned_node_mask = Some([a].into_iter().collect());
        app.workspace.graph_runtime.views.insert(view_id, view);

        assert_eq!(filtered_view_node_keys(&app, view_id), vec![a]);
    }

    #[test]
    fn filtered_view_node_keys_respects_graphlet_mask_even_without_filter_expr() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_id = GraphViewId::new();
        let mut view = GraphViewState::new_with_id(view_id, "Scene");
        let a = app.add_node_and_sync("https://example.com/a".into(), Point2D::new(0.0, 0.0));
        let _b = app.add_node_and_sync("https://example.com/b".into(), Point2D::new(10.0, 0.0));
        view.graphlet_node_mask = Some([a].into_iter().collect());
        app.workspace.graph_runtime.views.insert(view_id, view);

        assert_eq!(filtered_view_node_keys(&app, view_id), vec![a]);
    }

    #[test]
    fn filtered_view_node_keys_respects_tombstone_visibility() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_id = GraphViewId::new();
        let view = GraphViewState::new_with_id(view_id, "Scene");
        app.workspace.graph_runtime.views.insert(view_id, view);
        let hidden =
            app.add_node_and_sync("https://example.com/hidden".into(), Point2D::new(0.0, 0.0));
        app.domain_graph_mut()
            .get_node_mut(hidden)
            .expect("node exists")
            .lifecycle = crate::graph::NodeLifecycle::Tombstone;

        assert!(filtered_view_node_keys(&app, view_id).contains(&hidden) == false);
    }

    #[test]
    fn filtered_view_node_keys_handles_missing_view() {
        let app = GraphBrowserApp::new_for_testing();
        assert!(filtered_view_node_keys(&app, GraphViewId::new()).is_empty());
    }

    #[test]
    fn search_result_gather_returns_empty_for_blank_query() {
        let app = GraphBrowserApp::new_for_testing();
        assert!(super::active_graph_search_node_keys(&app).is_empty());
    }

    #[test]
    fn search_result_gather_dedups_anchor_and_result_overlap() {
        let mut app = GraphBrowserApp::new_for_testing();
        let anchor =
            app.add_node_and_sync("https://example.com/anchor".into(), Point2D::new(0.0, 0.0));
        app.workspace.graph_runtime.active_graph_search_query = "anchor".to_string();
        app.workspace
            .graph_runtime
            .active_graph_search_neighborhood_anchor = Some(anchor);
        app.workspace
            .graph_runtime
            .active_graph_search_neighborhood_depth = 1;

        let matches = super::active_graph_search_node_keys(&app);
        assert_eq!(matches, vec![anchor]);
    }
}
