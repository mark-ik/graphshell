/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Graph info overlay: stats bar, search status, enrichment panel, controls
//! hint, layout algorithm selection, and semantic depth badge.

use crate::app::{
    GraphBrowserApp, GraphIntent, GraphSearchHistoryEntry, GraphSearchOrigin, SearchDisplayMode,
    TagPanelState, ThreeDMode, ViewAction, ViewDimension, ZSource,
};
use crate::graph::NodeKey;
use crate::graph::format_imported_at_secs;
use egui::Vec2;

use super::canvas_visuals::{
    active_presentation_profile, active_view_filter_expr, evaluate_active_view_filter,
};
use super::reducer_bridge::apply_ui_intents_with_checkpoint;
use super::semantic_tags::{
    PlacementAnchorSummary, SelectedNodeEnrichmentSummary, graph_search_history_label,
    graph_search_scope_label, render_classification_chips, render_graph_search_origin_badge,
    render_selected_node_tag_panel, render_semantic_suggestion_buttons,
    render_semantic_tag_status_buttons, request_graph_search_entry, semantic_suggestion_chip,
    semantic_tag_status_chip,
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
                                    app.workspace.graph_runtime.tag_panel_state =
                                        Some(TagPanelState {
                                            node_key: selected_key,
                                            text_input: String::new(),
                                            icon_picker_open: false,
                                            pending_icon_override: None,
                                        });
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

        render_selected_node_tag_panel(ui.ctx(), app, selected_key);
    } else if app.workspace.graph_runtime.tag_panel_state.is_some() {
        app.workspace.graph_runtime.tag_panel_state = None;
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
    let controls_text = format!(
        "Shortcuts: Ctrl+Click Multi-select | {lasso_hint} | Double-click Open | Drag tab out to split | N New Node | Del Remove | T Physics | R Reheat | +/-/0 Zoom | C Position-Lock | Z Zoom-Lock | WASD/Arrows Pan | F9 Camera Controls | L Toggle Pin | Ctrl+F Search | G Edge Ops | {command_hint} Commands | {radial_hint} Radial | Ctrl+Z/Y Undo/Redo | {help_hint} Help"
    );
    ui.painter().text(
        ui.available_rect_before_wrap().left_bottom() + Vec2::new(10.0, -10.0),
        egui::Align2::LEFT_BOTTOM,
        controls_text,
        egui::FontId::proportional(10.0),
        presentation.controls_text.to_color32(),
    );
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
    let display_classifications: Vec<ClassificationChip> = app
        .domain_graph()
        .node_classifications(selected_key)
        .map(|classifications| {
            classifications
                .iter()
                .map(|c| ClassificationChip {
                    value: c.value.clone(),
                    label: c.label.clone().unwrap_or_else(|| c.value.clone()),
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
                    scheme: format!("{:?}", c.scheme),
                    node_key: selected_key,
                    classification_value: c.value.clone(),
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
