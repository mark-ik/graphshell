/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use crate::app::{GraphBrowserApp, GraphSearchOrigin, SearchDisplayMode};
use crate::graph::NodeKey;
use crate::shell::desktop::ui::gui_state::LocalFocusTarget;

pub(crate) struct GraphSearchUiArgs<'a> {
    pub ctx: &'a egui::Context,
    pub graph_app: &'a mut GraphBrowserApp,
    pub graph_search_query: &'a mut String,
    pub graph_search_filter_mode: &'a mut bool,
    pub graph_search_matches: &'a mut Vec<NodeKey>,
    pub graph_search_active_match_index: &'a mut Option<usize>,
    pub local_widget_focus: &'a mut Option<LocalFocusTarget>,
    pub focus_graph_search_field: &'a mut bool,
}

pub(crate) fn render_graph_search_window<F>(
    args: GraphSearchUiArgs<'_>,
    mut refresh_graph_search_matches: F,
) where
    F: FnMut(&mut GraphBrowserApp, &str, &mut Vec<NodeKey>, &mut Option<usize>),
{
    let GraphSearchUiArgs {
        ctx,
        graph_app,
        graph_search_query,
        graph_search_filter_mode,
        graph_search_matches,
        graph_search_active_match_index,
        local_widget_focus,
        focus_graph_search_field,
    } = args;

    egui::Window::new("Graph Search")
        .id(egui::Id::new("graph_search_window"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::RIGHT_TOP, [-16.0, 52.0])
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                let search_id = egui::Id::new("graph_search_input");
                let search_field = ui.add(
                    egui::TextEdit::singleline(graph_search_query)
                        .id(search_id)
                        .desired_width(280.0)
                        .hint_text("Find title, URL, #tag, or UDC code"),
                );
                if *focus_graph_search_field {
                    search_field.request_focus();
                    *focus_graph_search_field = false;
                    *local_widget_focus = Some(LocalFocusTarget::GraphSearch);
                }
                if search_field.gained_focus() || search_field.has_focus() {
                    *local_widget_focus = Some(LocalFocusTarget::GraphSearch);
                }
                if search_field.lost_focus()
                    && matches!(*local_widget_focus, Some(LocalFocusTarget::GraphSearch))
                {
                    *local_widget_focus = None;
                }
                if search_field.changed() {
                    graph_app.workspace.graph_runtime.active_graph_search_origin =
                        GraphSearchOrigin::Manual;
                    graph_app
                        .workspace
                        .graph_runtime
                        .active_graph_search_neighborhood_anchor = None;
                    graph_app
                        .workspace
                        .graph_runtime
                        .active_graph_search_neighborhood_depth = 1;
                    refresh_graph_search_matches(
                        graph_app,
                        graph_search_query,
                        graph_search_matches,
                        graph_search_active_match_index,
                    );
                    graph_app.workspace.graph_runtime.egui_state_dirty = true;
                }
                let mut mode_changed = false;
                ui.horizontal(|ui| {
                    if ui
                        .selectable_label(!*graph_search_filter_mode, "Highlight")
                        .clicked()
                    {
                        *graph_search_filter_mode = false;
                        graph_app.workspace.graph_runtime.search_display_mode =
                            SearchDisplayMode::Highlight;
                        mode_changed = true;
                    }
                    if ui
                        .selectable_label(*graph_search_filter_mode, "Filter")
                        .clicked()
                    {
                        *graph_search_filter_mode = true;
                        graph_app.workspace.graph_runtime.search_display_mode =
                            SearchDisplayMode::Filter;
                        mode_changed = true;
                    }
                });
                if mode_changed {
                    graph_app.workspace.graph_runtime.egui_state_dirty = true;
                }
                if ui.button("Clear").clicked() {
                    graph_app.workspace.graph_runtime.active_graph_search_origin =
                        GraphSearchOrigin::Manual;
                    graph_app
                        .workspace
                        .graph_runtime
                        .active_graph_search_neighborhood_anchor = None;
                    graph_app
                        .workspace
                        .graph_runtime
                        .active_graph_search_neighborhood_depth = 1;
                    graph_search_query.clear();
                    refresh_graph_search_matches(
                        graph_app,
                        graph_search_query,
                        graph_search_matches,
                        graph_search_active_match_index,
                    );
                    graph_app.workspace.graph_runtime.egui_state_dirty = true;
                }
            });
            let active_display = graph_search_active_match_index
                .map(|idx| idx + 1)
                .unwrap_or(0);
            ui.label(format!(
                "{} matches | active {}",
                graph_search_matches.len(),
                active_display
            ));
        });
}

