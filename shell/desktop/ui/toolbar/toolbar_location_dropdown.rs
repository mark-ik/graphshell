use egui::{Context, Key, Modifiers, Response, Ui, Vec2};

use super::*;
use crate::shell::desktop::ui::toolbar_routing;

/// Render dropdown overlay UI for omnibar search session with keyboard navigation,
/// multi-select support, bulk operations, and scope prefix shortcuts.
pub fn render_omnibar_dropdown(
    ctx: &Context,
    ui: &mut Ui,
    location_field: &Response,
    location: &mut String,
    location_dirty: &mut bool,
    omnibar_search_session: &mut Option<OmnibarSearchSession>,
    graph_app: &mut GraphBrowserApp,
    tiles_tree: &Tree<TileKind>,
    is_graph_view: bool,
    focused_toolbar_node: Option<NodeKey>,
    window: &EmbedderWindow,
    has_webview_tiles: bool,
    frame_intents: &mut Vec<GraphIntent>,
    open_selected_mode_after_submit: &mut Option<ToolbarOpenMode>,
) {
    // Keyboard navigation within dropdown
    if let Some(session) = omnibar_search_session.as_mut()
        && location_field.has_focus()
        && session.query == location.trim()
        && !session.matches.is_empty()
    {
        if ui.input(|i| i.key_pressed(Key::ArrowDown)) {
            session.active_index = (session.active_index + 1) % session.matches.len();
        }
        if ui.input(|i| i.key_pressed(Key::ArrowUp)) {
            session.active_index = if session.active_index == 0 {
                session.matches.len() - 1
            } else {
                session.active_index - 1
            };
        }
    }

    // Render active match overlay (counter + signifier)
    if let Some(session) = omnibar_search_session.as_ref()
        && location_field.has_focus()
        && session.query == location.trim()
        && !session.matches.is_empty()
        && let Some(active_match) = session.matches.get(session.active_index).cloned()
    {
        let counter = format!("{}/{}", session.active_index + 1, session.matches.len());
        let pos = location_field.rect.right_top() + Vec2::new(-8.0, 4.0);
        ui.painter().text(
            pos,
            egui::Align2::RIGHT_TOP,
            counter,
            egui::FontId::proportional(11.0),
            egui::Color32::GRAY,
        );
        let tag = omnibar_match_signifier(graph_app, tiles_tree, &active_match);
        let tag_pos = pos + Vec2::new(0.0, 12.0);
        ui.painter().text(
            tag_pos,
            egui::Align2::RIGHT_TOP,
            tag,
            egui::FontId::proportional(10.0),
            egui::Color32::from_gray(150),
        );
    }

    let mut clicked_omnibar_match: Option<OmnibarMatch> = None;
    let mut clicked_omnibar_index_with_modifiers: Option<(usize, Modifiers)> = None;
    let mut bulk_open_selected = false;
    let mut bulk_add_selected_to_workspace = false;
    let mut clicked_scope_prefix: Option<&'static str> = None;

    // Render dropdown area with match list, bulk actions, and scope shortcuts
    if let Some(session) = omnibar_search_session.as_mut()
        && location_field.has_focus()
        && session.query == location.trim()
    {
        session.selected_indices.retain(|idx| *idx < session.matches.len());
        if session.anchor_index.is_some_and(|idx| idx >= session.matches.len()) {
            session.anchor_index = None;
        }
        let dropdown_pos = location_field.rect.left_bottom() + Vec2::new(0.0, 2.0);
        egui::Area::new(egui::Id::new("omnibar_dropdown"))
            .order(egui::Order::Foreground)
            .fixed_pos(dropdown_pos)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_min_width(location_field.rect.width());
                    let row_count = session.matches.len().min(OMNIBAR_DROPDOWN_MAX_ROWS);
                    for idx in 0..row_count {
                        let active = idx == session.active_index;
                        let selected = session.selected_indices.contains(&idx);
                        let m = session.matches[idx].clone();
                        let label = omnibar_match_label(graph_app, &m);
                        let signifier = omnibar_match_signifier(graph_app, tiles_tree, &m);
                        let row = ui.horizontal(|ui| {
                            let selected_label = ui.selectable_label(active || selected, label);
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.small(signifier);
                                },
                            );
                            selected_label
                        });
                        let response = row.inner;
                        if response.hovered() {
                            session.active_index = idx;
                        }
                        if response.clicked() {
                            let modifiers = ui.input(|i| i.modifiers);
                            if modifiers.ctrl || modifiers.shift {
                                clicked_omnibar_index_with_modifiers = Some((idx, modifiers));
                            } else {
                                clicked_omnibar_match = Some(m);
                            }
                        }
                    }
                    if !session.selected_indices.is_empty() {
                        ui.separator();
                        ui.horizontal_wrapped(|ui| {
                            ui.small(format!("{} selected", session.selected_indices.len()));
                            if ui.small_button("Open Selected").clicked() {
                                bulk_open_selected = true;
                            }
                            if ui.small_button("Add Selected To Workspace...").clicked() {
                                bulk_add_selected_to_workspace = true;
                            }
                        });
                    }
                    if row_count > 0 {
                        ui.separator();
                    }
                    if let Some(status) = provider_status_label(session.provider_status) {
                        ui.small(status);
                    }
                    ui.horizontal_wrapped(|ui| {
                        for (label, prefix) in [
                            ("@n", "@n "),
                            ("@N", "@N "),
                            ("@t", "@t "),
                            ("@T", "@T "),
                            ("@g", "@g "),
                            ("@b", "@b "),
                            ("@d", "@d "),
                        ] {
                            if ui.small_button(label).clicked() {
                                clicked_scope_prefix = Some(prefix);
                            }
                        }
                    });
                });
            });
    }

    // Handle multi-select with Shift/Ctrl modifiers
    if let Some((idx, modifiers)) = clicked_omnibar_index_with_modifiers
        && let Some(session) = omnibar_search_session.as_mut()
    {
        if modifiers.shift {
            let anchor = session.anchor_index.unwrap_or(idx);
            if !modifiers.ctrl {
                session.selected_indices.clear();
            }
            if let Some(range) = inclusive_index_range(anchor, idx, session.matches.len()) {
                for selected_idx in range {
                    session.selected_indices.insert(selected_idx);
                }
            }
            session.anchor_index = Some(anchor);
        } else if modifiers.ctrl {
            if !session.selected_indices.insert(idx) {
                session.selected_indices.remove(&idx);
            }
            session.anchor_index = Some(idx);
        }
        session.active_index = idx;
    }

    // Bulk open selected matches
    if bulk_open_selected && let Some(session) = omnibar_search_session.as_ref() {
        let mut ordered: Vec<usize> = session.selected_indices.iter().copied().collect();
        ordered.sort_unstable();
        for idx in ordered {
            if let Some(item) = session.matches.get(idx).cloned() {
                apply_omnibar_match(
                    graph_app,
                    item,
                    has_webview_tiles,
                    false,
                    frame_intents,
                    open_selected_mode_after_submit,
                );
            }
        }
        *location_dirty = true;
    }

    // Bulk add selected node matches to workspace picker
    if bulk_add_selected_to_workspace && let Some(session) = omnibar_search_session.as_ref() {
        let mut node_keys = Vec::new();
        let mut ordered: Vec<usize> = session.selected_indices.iter().copied().collect();
        ordered.sort_unstable();
        for idx in ordered {
            if let Some(OmnibarMatch::Node(key)) = session.matches.get(idx) {
                node_keys.push(*key);
            }
        }
        node_keys.sort_by_key(|key| key.index());
        node_keys.dedup();
        if !node_keys.is_empty() {
            graph_app.request_add_exact_selection_to_workspace_picker(node_keys);
        }
    }

    // Handle scope prefix shortcut clicks
    if let Some(prefix) = clicked_scope_prefix {
        *location = prefix.to_string();
        *location_dirty = true;
        *omnibar_search_session = None;
    }

    // Handle clicked match (either SearchQuery or direct match)
    if let Some(active_match) = clicked_omnibar_match {
        match active_match {
            OmnibarMatch::SearchQuery { query, provider } => {
                *location = query;
                *omnibar_search_session = None;
                let split_open_requested = ui.input(|i| i.modifiers.shift);
                let provider_searchpage = searchpage_template_for_provider(provider);
                let submit_result = toolbar_routing::submit_address_bar_intents(
                    graph_app,
                    location,
                    is_graph_view,
                    focused_toolbar_node,
                    split_open_requested,
                    window,
                    provider_searchpage,
                );
                frame_intents.extend(submit_result.intents);
                if submit_result.mark_clean {
                    *location_dirty = false;
                    *open_selected_mode_after_submit = submit_result.open_mode;
                }
            },
            other => {
                let shift_override_original = ui.input(|i| i.modifiers.shift);
                apply_omnibar_match(
                    graph_app,
                    other,
                    has_webview_tiles,
                    shift_override_original,
                    frame_intents,
                    open_selected_mode_after_submit,
                );
                *location_dirty = true;
            },
        }
    }
}

/// Calculate inclusive range between two indices within array bounds.
fn inclusive_index_range(start: usize, end: usize, len: usize) -> Option<std::ops::RangeInclusive<usize>> {
    if start >= len || end >= len {
        return None;
    }
    if start <= end {
        Some(start..=end)
    } else {
        Some(end..=start)
    }
}
