/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Semantic tag UI: chips, status labels, suggestions, search-origin badge,
//! tag panel window, and graph search scope/history helpers.

use crate::app::{GraphBrowserApp, GraphSearchHistoryEntry, GraphSearchOrigin};
use crate::graph::badge::{BadgeVisual, badge_visuals, badges_for_node};
use crate::graph::NodeKey;
use egui::Window;
use std::collections::{HashMap, HashSet};

use super::reducer_bridge::apply_reducer_graph_intents_hardened;
use crate::app::GraphIntent;

// ── Structs ───────────────────────────────────────────────────────────────────

pub(super) struct SelectedNodeEnrichmentSummary {
    pub(super) title: String,
    pub(super) url: String,
    pub(super) show_url: bool,
    pub(super) lifecycle: &'static str,
    pub(super) import_records: Vec<crate::graph::NodeImportRecordSummary>,
    pub(super) workspace_memberships: Vec<String>,
    pub(super) display_tags: Vec<SemanticTagStatusChip>,
    pub(super) hidden_tag_count: usize,
    pub(super) suggested_tags: Vec<SemanticSuggestionChip>,
    pub(super) placement_anchor: Option<PlacementAnchorSummary>,
    pub(super) semantic_lens_available: bool,
    pub(super) semantic_lens_active: bool,
}

pub(super) struct PlacementAnchorSummary {
    pub(super) key: NodeKey,
    pub(super) label: String,
    pub(super) slice_tag: Option<String>,
}

pub(super) struct SemanticTagChip {
    pub(super) query: String,
    pub(super) label: String,
}

pub(super) struct SemanticTagStatusChip {
    pub(super) chip: SemanticTagChip,
    pub(super) status: String,
}

pub(super) struct SemanticSuggestionChip {
    pub(super) chip: SemanticTagChip,
    pub(super) reason: String,
}

// ── Badge helpers ─────────────────────────────────────────────────────────────

pub(super) fn semantic_badges_by_key(
    app: &GraphBrowserApp,
    graph: &crate::graph::Graph,
) -> HashMap<NodeKey, Vec<BadgeVisual>> {
    use crate::graph::badge::Badge;
    graph
        .nodes()
        .filter_map(|(key, node)| {
            let badges = badges_for_node(
                node,
                app.membership_for_node(node.id).len(),
                app.crash_blocked_node_keys().any(|crashed| crashed == key),
            )
            .into_iter()
            .filter(|badge| !matches!(badge, Badge::WorkspaceCount(_)))
            .collect::<Vec<_>>();
            let badge_visuals = badge_visuals(&badges);
            (!badge_visuals.is_empty()).then_some((key, badge_visuals))
        })
        .collect()
}

// ── Tag display / status labels ───────────────────────────────────────────────

pub(super) fn semantic_tag_display_label(tag: &str) -> String {
    if let Some(code) = tag.strip_prefix("udc:") {
        return match crate::shell::desktop::runtime::registries::phase3_validate_knowledge_tag(code)
        {
            crate::shell::desktop::runtime::registries::knowledge::TagValidationResult::Valid {
                canonical_code,
                display_label,
            } => format!("{display_label} (udc:{canonical_code})"),
            crate::shell::desktop::runtime::registries::knowledge::TagValidationResult::Unknown { .. }
            | crate::shell::desktop::runtime::registries::knowledge::TagValidationResult::Malformed { .. } => {
                format!("udc:{code}")
            }
        };
    }
    tag.to_string()
}

fn semantic_tag_status_label(tag: &str) -> String {
    if let Some(code) = tag.strip_prefix("udc:") {
        return match crate::shell::desktop::runtime::registries::phase3_validate_knowledge_tag(code)
        {
            crate::shell::desktop::runtime::registries::knowledge::TagValidationResult::Valid {
                canonical_code,
                ..
            } => format!("KnowledgeRegistry canonical: udc:{canonical_code}"),
            crate::shell::desktop::runtime::registries::knowledge::TagValidationResult::Unknown { .. } => {
                "KnowledgeRegistry descendant/unknown code".to_string()
            }
            crate::shell::desktop::runtime::registries::knowledge::TagValidationResult::Malformed { .. } => {
                "Unrecognized semantic code".to_string()
            }
        };
    }
    if tag.starts_with('#') {
        "User tag".to_string()
    } else {
        "Node tag".to_string()
    }
}

fn semantic_tag_chip(tag: String) -> SemanticTagChip {
    SemanticTagChip {
        label: semantic_tag_display_label(&tag),
        query: tag,
    }
}

pub(super) fn semantic_tag_status_chip(tag: String) -> SemanticTagStatusChip {
    let status = semantic_tag_status_label(&tag);
    SemanticTagStatusChip {
        chip: semantic_tag_chip(tag),
        status,
    }
}

pub(super) fn semantic_suggestion_chip(
    app: &GraphBrowserApp,
    selected_key: NodeKey,
    tag: String,
) -> SemanticSuggestionChip {
    let reason = explain_semantic_suggestion(app, selected_key, &tag);
    SemanticSuggestionChip {
        chip: semantic_tag_chip(tag),
        reason,
    }
}

fn explain_semantic_suggestion(app: &GraphBrowserApp, selected_key: NodeKey, tag: &str) -> String {
    let Some(node) = app.domain_graph().get_node(selected_key) else {
        return "Suggested by the tag suggester agent.".to_string();
    };

    let code = tag.strip_prefix("udc:").unwrap_or(tag);
    let registry =
        crate::shell::desktop::runtime::registries::knowledge::KnowledgeRegistry::default();
    let title = node.title.trim();
    if !title.is_empty()
        && registry
            .search(title)
            .into_iter()
            .take(3)
            .any(|entry| entry.code == code)
    {
        return format!("Suggested by the tag suggester agent from the node title '{title}'.");
    }

    let url_text = node.url.replace([':', '/', '.', '-', '_'], " ");
    if registry
        .search(&url_text)
        .into_iter()
        .take(3)
        .any(|entry| entry.code == code)
    {
        return "Suggested by the tag suggester agent from URL/domain tokens.".to_string();
    }

    if let Some(anchor_key) =
        crate::shell::desktop::runtime::registries::phase3_suggest_semantic_placement_anchor(
            app,
            selected_key,
        )
    {
        let anchor_tags =
            crate::shell::desktop::runtime::registries::knowledge::tags_for_node(app, &anchor_key);
        if anchor_tags.iter().any(|anchor_tag| {
            anchor_tag == tag
                || (anchor_tag.starts_with("udc:")
                    && tag.starts_with(anchor_tag)
                    && tag.len() > anchor_tag.len())
        }) {
            return "Suggested because the node sits near a matching semantic anchor slice."
                .to_string();
        }
    }

    "Suggested by the tag suggester agent from title/URL semantic matches.".to_string()
}

// ── Tag utilities ─────────────────────────────────────────────────────────────

fn icon_picker_presets() -> Vec<crate::graph::badge::BadgeIcon> {
    use crate::graph::badge::BadgeIcon;
    vec![
        BadgeIcon::None,
        BadgeIcon::Emoji("🔖".to_string()),
        BadgeIcon::Emoji("📚".to_string()),
        BadgeIcon::Emoji("🔬".to_string()),
        BadgeIcon::Emoji("🧠".to_string()),
        BadgeIcon::Emoji("📝".to_string()),
        BadgeIcon::Emoji("🌟".to_string()),
        BadgeIcon::Emoji("📎".to_string()),
    ]
}

fn badge_icon_label(icon: &crate::graph::badge::BadgeIcon) -> String {
    match icon {
        crate::graph::badge::BadgeIcon::Emoji(value) => value.clone(),
        crate::graph::badge::BadgeIcon::Lucide(value) => value.clone(),
        crate::graph::badge::BadgeIcon::None => "None".to_string(),
    }
}

fn normalize_tag_entry_input(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(if trimmed.starts_with('#') {
        trimmed.to_ascii_lowercase()
    } else {
        trimmed.to_string()
    })
}

fn is_reserved_system_tag(tag: &str) -> bool {
    matches!(
        tag,
        GraphBrowserApp::TAG_PIN
            | GraphBrowserApp::TAG_STARRED
            | GraphBrowserApp::TAG_ARCHIVE
            | GraphBrowserApp::TAG_RESIDENT
            | GraphBrowserApp::TAG_PRIVATE
            | GraphBrowserApp::TAG_NOHISTORY
            | GraphBrowserApp::TAG_MONITOR
            | GraphBrowserApp::TAG_UNREAD
            | GraphBrowserApp::TAG_FOCUS
            | GraphBrowserApp::TAG_CLIP
    )
}

pub(super) fn reserved_tag_warning(query: &str) -> Option<String> {
    let normalized = normalize_tag_entry_input(query)?;
    if normalized.starts_with('#') && !is_reserved_system_tag(&normalized) {
        return Some("Unknown #tag will be accepted as user-defined.".to_string());
    }
    None
}

fn default_tag_suggestion_candidates() -> Vec<String> {
    [
        GraphBrowserApp::TAG_PIN,
        GraphBrowserApp::TAG_STARRED,
        GraphBrowserApp::TAG_UNREAD,
        GraphBrowserApp::TAG_FOCUS,
        GraphBrowserApp::TAG_MONITOR,
        GraphBrowserApp::TAG_PRIVATE,
        GraphBrowserApp::TAG_ARCHIVE,
        GraphBrowserApp::TAG_RESIDENT,
        GraphBrowserApp::TAG_NOHISTORY,
        GraphBrowserApp::TAG_CLIP,
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

pub(super) fn ranked_tag_suggestions(
    app: &GraphBrowserApp,
    selected_key: NodeKey,
    query: &str,
) -> Vec<SemanticTagChip> {
    let current_tags =
        crate::shell::desktop::runtime::registries::knowledge::tags_for_node(app, &selected_key)
            .into_iter()
            .collect::<HashSet<_>>();
    let mut candidates = default_tag_suggestion_candidates();
    candidates.extend(
        app.domain_graph()
            .nodes()
            .flat_map(|(_, node)| node.tags.iter().cloned()),
    );
    candidates.extend(app.suggested_semantic_tags_for_node(selected_key));
    candidates.sort();
    candidates.dedup();

    let mut ranked = Vec::new();
    let mut seen = HashSet::new();
    let mut push_candidate = |tag: String, ranked: &mut Vec<SemanticTagChip>| {
        if current_tags.contains(&tag) || !seen.insert(tag.clone()) {
            return;
        }
        ranked.push(semantic_tag_chip(tag));
    };

    if let Some(normalized_query) = normalize_tag_entry_input(query) {
        push_candidate(normalized_query.clone(), &mut ranked);

        for matched in crate::services::search::fuzzy_match_items(candidates, &normalized_query) {
            push_candidate(matched, &mut ranked);
            if ranked.len() >= 5 {
                return ranked;
            }
        }

        let knowledge_registry =
            crate::shell::desktop::runtime::registries::knowledge::KnowledgeRegistry::default();
        for entry in knowledge_registry.search(&normalized_query) {
            push_candidate(format!("udc:{}", entry.code), &mut ranked);
            if ranked.len() >= 5 {
                return ranked;
            }
        }
    } else {
        for candidate in candidates {
            push_candidate(candidate, &mut ranked);
            if ranked.len() >= 5 {
                break;
            }
        }
    }

    ranked
}

// ── Render tag panel ──────────────────────────────────────────────────────────

pub(super) fn render_selected_node_tag_panel(
    ctx: &egui::Context,
    app: &mut GraphBrowserApp,
    selected_key: NodeKey,
) {
    let Some((panel_node_key, panel_text_input)) = app
        .workspace
        .graph_runtime
        .tag_panel_state
        .as_ref()
        .map(|state| (state.node_key, state.text_input.clone()))
    else {
        return;
    };
    if panel_node_key != selected_key {
        app.workspace.graph_runtime.tag_panel_state = None;
        return;
    }
    let Some(node) = app.domain_graph().get_node(selected_key) else {
        app.workspace.graph_runtime.tag_panel_state = None;
        return;
    };

    let title = if node.title.is_empty() {
        node.url.clone()
    } else {
        node.title.clone()
    };
    let current_tags =
        crate::shell::desktop::runtime::registries::knowledge::tags_for_node(app, &selected_key);
    let mut text_input = panel_text_input;
    let mut open = true;
    let mut close_requested = false;
    let mut pending_intents = Vec::new();
    let mut pending_icon_write: Option<(String, Option<crate::graph::badge::BadgeIcon>)> = None;
    let warning = reserved_tag_warning(&text_input);
    let suggestions = ranked_tag_suggestions(app, selected_key, &text_input);

    Window::new(format!("Tags for {}", title))
        .id(egui::Id::new((
            "graph_node_tag_panel",
            selected_key.index(),
        )))
        .open(&mut open)
        .default_width(360.0)
        .show(ctx, |ui| {
            ui.small("Current tags");
            if current_tags.is_empty() {
                ui.small("No tags yet.");
            } else {
                ui.horizontal_wrapped(|ui| {
                    for tag in &current_tags {
                        let label = semantic_tag_display_label(tag);
                        if ui.small_button(format!("{label} ×")).clicked() {
                            pending_intents.push(GraphIntent::UntagNode {
                                key: selected_key,
                                tag: tag.clone(),
                            });
                        }
                    }
                });
            }

            ui.separator();
            ui.small("Add tag");
            let response = ui.add(
                egui::TextEdit::singleline(&mut text_input).hint_text("Add tag or semantic code…"),
            );
            let submit =
                response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
            ui.horizontal(|ui| {
                let picker_label = app
                    .workspace
                    .graph_runtime
                    .tag_panel_state
                    .as_ref()
                    .and_then(|state| state.pending_icon_override.as_ref())
                    .map(badge_icon_label)
                    .unwrap_or_else(|| "⊞".to_string());
                if ui.small_button(picker_label).clicked()
                    && let Some(panel_state) = app.workspace.graph_runtime.tag_panel_state.as_mut()
                {
                    panel_state.icon_picker_open = !panel_state.icon_picker_open;
                }
                if ui.small_button("Add").clicked() || submit {
                    if let Some(tag) = normalize_tag_entry_input(&text_input) {
                        pending_intents.push(GraphIntent::TagNode {
                            key: selected_key,
                            tag,
                        });
                        text_input.clear();
                    }
                }
                if ui.small_button("Close").clicked() {
                    close_requested = true;
                }
            });
            if app
                .workspace
                .graph_runtime
                .tag_panel_state
                .as_ref()
                .is_some_and(|state| state.icon_picker_open)
            {
                ui.small("Icon picker");
                ui.horizontal_wrapped(|ui| {
                    for icon in icon_picker_presets() {
                        let label = badge_icon_label(&icon);
                        if ui.small_button(label).clicked()
                            && let Some(panel_state) = app.workspace.graph_runtime.tag_panel_state.as_mut()
                        {
                            panel_state.pending_icon_override =
                                (!matches!(icon, crate::graph::badge::BadgeIcon::None))
                                    .then_some(icon.clone());
                            panel_state.icon_picker_open = false;
                        }
                    }
                });
            }
            if let Some(warning) = warning.as_ref() {
                ui.small(warning);
            }

            ui.separator();
            ui.small("Suggestions");
            if suggestions.is_empty() {
                ui.small("No suggestions yet.");
            } else {
                ui.horizontal_wrapped(|ui| {
                    for chip in &suggestions {
                        let button = egui::Button::new(egui::RichText::new(&chip.label).small());
                        if ui.add(button).clicked() {
                            pending_intents.push(GraphIntent::TagNode {
                                key: selected_key,
                                tag: chip.query.clone(),
                            });
                            text_input.clear();
                        }
                    }
                });
            }
        });

    if close_requested || !open {
        app.workspace.graph_runtime.tag_panel_state = None;
    } else if let Some(panel_state) = app.workspace.graph_runtime.tag_panel_state.as_mut() {
        panel_state.text_input = text_input;
    }

    if !pending_intents.is_empty() {
        let tag_for_icon_write = pending_intents.iter().find_map(|intent| match intent {
            GraphIntent::TagNode { tag, .. } => Some(tag.clone()),
            _ => None,
        });
        apply_reducer_graph_intents_hardened(app, pending_intents);
        if let Some(tag) = tag_for_icon_write
            && !is_reserved_system_tag(&tag)
            && !tag.starts_with("udc:")
        {
            let icon = app
                .workspace
                .graph_runtime
                .tag_panel_state
                .as_ref()
                .and_then(|state| state.pending_icon_override.clone());
            let _ = app.set_node_tag_icon_override(selected_key, &tag, icon.clone());
            pending_icon_write = Some((tag, icon));
        }
    }

    if let Some((_tag, _icon)) = pending_icon_write
        && let Some(panel_state) = app.workspace.graph_runtime.tag_panel_state.as_mut()
    {
        panel_state.pending_icon_override = None;
    }
}

// ── Tag button renderers ──────────────────────────────────────────────────────

pub(super) fn render_semantic_tag_buttons(
    ui: &mut egui::Ui,
    app: &mut GraphBrowserApp,
    chips: &[SemanticTagChip],
) {
    ui.horizontal_wrapped(|ui| {
        for chip in chips {
            let button = egui::Button::new(egui::RichText::new(&chip.label).small());
            if ui.add(button).clicked() {
                app.request_graph_search_with_options(
                    chip.query.clone(),
                    true,
                    GraphSearchOrigin::SemanticTag,
                    None,
                    1,
                    true,
                    Some(
                        crate::shell::desktop::ui::gui_orchestration::graph_search_toast_message(
                            GraphSearchOrigin::SemanticTag,
                            &chip.query,
                            0,
                        ),
                    ),
                );
            }
        }
    });
}

pub(super) fn render_semantic_tag_status_buttons(
    ui: &mut egui::Ui,
    app: &mut GraphBrowserApp,
    chips: &[SemanticTagStatusChip],
) {
    ui.horizontal_wrapped(|ui| {
        for entry in chips {
            ui.vertical(|ui| {
                let button = egui::Button::new(egui::RichText::new(&entry.chip.label).small());
                if ui.add(button).clicked() {
                    app.request_graph_search_with_options(
                        entry.chip.query.clone(),
                        true,
                        GraphSearchOrigin::SemanticTag,
                        None,
                        1,
                        true,
                        Some(
                            crate::shell::desktop::ui::gui_orchestration::graph_search_toast_message(
                                GraphSearchOrigin::SemanticTag,
                                &entry.chip.query,
                                0,
                            ),
                        ),
                    );
                }
                ui.small(&entry.status);
            });
        }
    });
}

pub(super) fn render_semantic_suggestion_buttons(
    ui: &mut egui::Ui,
    app: &mut GraphBrowserApp,
    chips: &[SemanticSuggestionChip],
) {
    ui.horizontal_wrapped(|ui| {
        for entry in chips {
            ui.vertical(|ui| {
                let button = egui::Button::new(egui::RichText::new(&entry.chip.label).small());
                if ui.add(button).clicked() {
                    app.request_graph_search_with_options(
                        entry.chip.query.clone(),
                        true,
                        GraphSearchOrigin::SemanticTag,
                        None,
                        1,
                        true,
                        Some(
                            crate::shell::desktop::ui::gui_orchestration::graph_search_toast_message(
                                GraphSearchOrigin::SemanticTag,
                                &entry.chip.query,
                                0,
                            ),
                        ),
                    );
                }
                ui.small(&entry.reason);
            });
        }
    });
}

// ── Search history / scope helpers ────────────────────────────────────────────

pub(super) fn graph_search_history_label(entry: &GraphSearchHistoryEntry) -> String {
    let origin = match entry.origin {
        GraphSearchOrigin::Manual => "manual",
        GraphSearchOrigin::SemanticTag => "tag",
        GraphSearchOrigin::AnchorSlice => "anchor",
    };
    let scope_suffix = if entry.neighborhood_anchor.is_some() && entry.neighborhood_depth > 1 {
        " + 2-hop"
    } else if entry.neighborhood_anchor.is_some() {
        " + nbr"
    } else {
        ""
    };
    format!("{origin}{scope_suffix}: {}", entry.query)
}

pub(super) fn request_graph_search_entry(
    app: &mut GraphBrowserApp,
    entry: GraphSearchHistoryEntry,
    record_history: bool,
    toast_message: Option<String>,
) {
    app.request_graph_search_with_options(
        entry.query,
        entry.filter_mode,
        entry.origin,
        entry.neighborhood_anchor,
        entry.neighborhood_depth,
        record_history,
        toast_message,
    );
}

pub(super) fn graph_search_scope_label(
    neighborhood_anchor: Option<NodeKey>,
    neighborhood_depth: u8,
) -> String {
    if neighborhood_anchor.is_none() {
        return String::new();
    }
    if neighborhood_depth > 1 {
        return format!(" | {}-hop neighborhood", neighborhood_depth);
    }
    " | 1-hop neighborhood".to_string()
}

pub(super) fn render_graph_search_origin_badge(ui: &mut egui::Ui, origin: &GraphSearchOrigin) {
    let (label, color) = match origin {
        GraphSearchOrigin::Manual => ("manual", egui::Color32::from_rgb(120, 170, 255)),
        GraphSearchOrigin::SemanticTag => ("semantic", egui::Color32::from_rgb(76, 175, 80)),
        GraphSearchOrigin::AnchorSlice => ("anchor", egui::Color32::from_rgb(255, 167, 38)),
    };
    ui.label(egui::RichText::new("●").small().color(color));
    ui.small(label);
}
