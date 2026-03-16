use super::*;
use crate::graph::badge::{Badge, badges_for_node, tab_badge_token};

impl<'a> GraphshellTileBehavior<'a> {
    pub(super) fn tab_title_for_tile(&mut self, pane: &TileKind) -> WidgetText {
        match pane {
            TileKind::Pane(crate::shell::desktop::workbench::pane_model::PaneViewState::Graph(view_ref)) => self
                .graph_app
                .workspace
                .views
                .get(&view_ref.graph_view_id)
                .map(|v| v.name.clone().into())
                .unwrap_or_else(|| "Graph".into()),
            TileKind::Pane(crate::shell::desktop::workbench::pane_model::PaneViewState::Node(state)) => self
                .graph_app
                .domain_graph()
                .get_node(state.node)
                .map(|n| n.title.clone().into())
                .unwrap_or_else(|| format!("Node {:?}", state.node).into()),
            #[cfg(feature = "diagnostics")]
            TileKind::Pane(crate::shell::desktop::workbench::pane_model::PaneViewState::Tool(tool)) => tool.title().into(),
            TileKind::Graph(view_ref) => self
                .graph_app
                .workspace
                .views
                .get(&view_ref.graph_view_id)
                .map(|v| v.name.clone().into())
                .unwrap_or_else(|| "Graph".into()),
            TileKind::Node(state) => self
                .graph_app
                .domain_graph()
                .get_node(state.node)
                .map(|n| n.title.clone().into())
                .unwrap_or_else(|| format!("Node {:?}", state.node).into()),
            #[cfg(feature = "diagnostics")]
            TileKind::Tool(tool) => tool.title().into(),
        }
    }

    pub(super) fn render_tab_ui(
        &mut self,
        tiles: &mut Tiles<TileKind>,
        ui: &mut Ui,
        id: Id,
        tile_id: TileId,
        state: &TabState,
    ) -> Response {
        render_tab_ui_impl(self, tiles, ui, id, tile_id, state)
    }
}

fn render_tab_ui_impl(
    behavior: &mut GraphshellTileBehavior<'_>,
    tiles: &mut Tiles<TileKind>,
    ui: &mut Ui,
    id: Id,
    tile_id: TileId,
    state: &TabState,
) -> Response {
    let close_btn_size = Vec2::splat(behavior.close_button_outer_size());
    let close_btn_left_padding = 4.0;
    let icon_size = 16.0;
    let icon_spacing = 6.0;
    let x_margin = behavior.tab_title_spacing(ui.visuals());
    let workbench_surface = registries::phase3_resolve_active_workbench_surface_profile();

    let (title_text, favicon_texture) = match tiles.get(tile_id) {
        Some(Tile::Pane(TileKind::Pane(crate::shell::desktop::workbench::pane_model::PaneViewState::Graph(view_ref)))) => {
            let name = behavior
                .graph_app
                .workspace
                .views
                .get(&view_ref.graph_view_id)
                .map(|v| v.name.clone())
                .unwrap_or_else(|| "Graph".to_string());
            (name, None)
        }
        Some(Tile::Pane(TileKind::Pane(crate::shell::desktop::workbench::pane_model::PaneViewState::Node(state)))) => {
            let title = behavior
                .graph_app
                .domain_graph()
                .get_node(state.node)
                .map(|n| n.title.clone())
                .unwrap_or_else(|| format!("Node {:?}", state.node));
            let title = truncate_with_ellipsis(
                &title,
                workbench_surface.profile.interaction.title_truncation_chars,
            );
            let favicon = behavior.favicon_texture_id(ui, state.node);
            (title, favicon)
        }
        #[cfg(feature = "diagnostics")]
        Some(Tile::Pane(TileKind::Pane(crate::shell::desktop::workbench::pane_model::PaneViewState::Tool(tool)))) => (tool.title().to_string(), None),
        Some(Tile::Pane(TileKind::Graph(view_ref))) => {
            let name = behavior
                .graph_app
                .workspace
                .views
                .get(&view_ref.graph_view_id)
                .map(|v| v.name.clone())
                .unwrap_or_else(|| "Graph".to_string());
            (name, None)
        }
        Some(Tile::Pane(TileKind::Node(state))) => {
            let title = behavior
                .graph_app
                .domain_graph()
                .get_node(state.node)
                .map(|n| n.title.clone())
                .unwrap_or_else(|| format!("Node {:?}", state.node));
            let badge_suffix = behavior
                .graph_app
                .domain_graph()
                .get_node(state.node)
                .map(|node| {
                    badges_for_node(
                        node,
                        behavior.graph_app.membership_for_node(node.id).len(),
                        behavior
                            .graph_app
                            .crash_blocked_node_keys()
                            .any(|crashed| crashed == state.node),
                    )
                })
                .unwrap_or_default()
                .into_iter()
                .filter(|badge| !matches!(badge, Badge::WorkspaceCount(_)))
                .filter_map(|badge| tab_badge_token(&badge))
                .take(2)
                .collect::<Vec<_>>();
            let title = if badge_suffix.is_empty() {
                title
            } else {
                format!("{title} {}", badge_suffix.join(" "))
            };
            let title = truncate_with_ellipsis(
                &title,
                workbench_surface.profile.interaction.title_truncation_chars,
            );
            let favicon = behavior.favicon_texture_id(ui, state.node);
            (title, favicon)
        }
        #[cfg(feature = "diagnostics")]
        Some(Tile::Pane(TileKind::Tool(tool))) => (tool.title().to_string(), None),
        Some(Tile::Container(Container::Linear(linear))) => {
            let label = match linear.dir {
                egui_tiles::LinearDir::Horizontal => {
                    workbench_surface.profile.split_horizontal_label.clone()
                }
                egui_tiles::LinearDir::Vertical => {
                    workbench_surface.profile.split_vertical_label.clone()
                }
            };
            (label, None)
        }
        Some(Tile::Container(Container::Tabs(_))) => {
            (workbench_surface.profile.tab_group_label.clone(), None)
        }
        Some(Tile::Container(Container::Grid(_))) => {
            (workbench_surface.profile.grid_label.clone(), None)
        }
        None => ("MISSING TILE".to_string(), None),
    };

    let font_id = TextStyle::Button.resolve(ui.style());
    let galley = WidgetText::from(title_text).into_galley(
        ui,
        Some(egui::TextWrapMode::Extend),
        f32::INFINITY,
        font_id,
    );

    let icon_width = if favicon_texture.is_some() {
        icon_size + icon_spacing
    } else {
        0.0
    };
    let button_width = galley.size().x
        + 2.0 * x_margin
        + icon_width
        + f32::from(state.closable) * (close_btn_left_padding + close_btn_size.x);
    let (_, tab_rect) = ui.allocate_space(vec2(button_width, ui.available_height()));

    let tab_response = ui
        .interact(tab_rect, id, Sense::click_and_drag())
        .on_hover_cursor(behavior.tab_hover_cursor_icon());

    if tab_response.clicked() {
        let modifiers = ui.input(|i| i.modifiers);
        let tile_selection_mode = if modifiers.ctrl {
            SelectionUpdateMode::Toggle
        } else {
            SelectionUpdateMode::Replace
        };
        behavior
            .graph_app
            .enqueue_workbench_intent(WorkbenchIntent::UpdateTileSelection {
                tile_id,
                mode: tile_selection_mode,
            });

        if let Some(Tile::Pane(TileKind::Pane(crate::shell::desktop::workbench::pane_model::PaneViewState::Node(state)))) = tiles.get(tile_id) {
            let node_key = state.node;
            if modifiers.shift {
                let ordered_nodes =
                    GraphshellTileBehavior::tab_group_node_order_for_tile(tiles, tile_id)
                        .unwrap_or_else(|| vec![node_key]);
                let target_index = ordered_nodes
                    .iter()
                    .position(|key| *key == node_key)
                    .unwrap_or(0);
                let anchor_key = behavior
                    .graph_app
                    .workspace
                    .tab_selection_anchor
                    .unwrap_or(node_key);
                let anchor_index = ordered_nodes
                    .iter()
                    .position(|key| *key == anchor_key)
                    .unwrap_or(target_index);
                if !modifiers.ctrl {
                    behavior.graph_app.workspace.selected_tab_nodes.clear();
                }
                if let Some(range) =
                    inclusive_index_range(anchor_index, target_index, ordered_nodes.len())
                {
                    behavior
                        .graph_app
                        .add_tab_selection_keys(range.map(|idx| ordered_nodes[idx]));
                }
            } else if modifiers.ctrl {
                behavior.graph_app.toggle_tab_selection(node_key);
            } else {
                behavior.graph_app.set_tab_selection_single(node_key);
                behavior.queue_post_render_intent(GraphIntent::SelectNode {
                    key: node_key,
                    multi_select: false,
                });
            }
        }

        if let Some(Tile::Pane(TileKind::Node(state))) = tiles.get(tile_id) {
            let node_key = state.node;
            if modifiers.shift {
                let ordered_nodes =
                    GraphshellTileBehavior::tab_group_node_order_for_tile(tiles, tile_id)
                        .unwrap_or_else(|| vec![node_key]);
                let target_index = ordered_nodes
                    .iter()
                    .position(|key| *key == node_key)
                    .unwrap_or(0);
                let anchor_key = behavior
                    .graph_app
                    .workspace
                    .tab_selection_anchor
                    .unwrap_or(node_key);
                let anchor_index = ordered_nodes
                    .iter()
                    .position(|key| *key == anchor_key)
                    .unwrap_or(target_index);
                if !modifiers.ctrl {
                    behavior.graph_app.workspace.selected_tab_nodes.clear();
                }
                if let Some(range) =
                    inclusive_index_range(anchor_index, target_index, ordered_nodes.len())
                {
                    behavior
                        .graph_app
                        .add_tab_selection_keys(range.map(|idx| ordered_nodes[idx]));
                }
            } else if modifiers.ctrl {
                behavior.graph_app.toggle_tab_selection(node_key);
            } else {
                behavior.graph_app.set_tab_selection_single(node_key);
                behavior.queue_post_render_intent(GraphIntent::SelectNode {
                    key: node_key,
                    multi_select: false,
                });
            }
        }
    }

    if tab_response.drag_stopped()
        && let Some(node_key) = match tiles.get(tile_id) {
            Some(Tile::Pane(TileKind::Pane(crate::shell::desktop::workbench::pane_model::PaneViewState::Node(state)))) => Some(state.node),
            Some(Tile::Pane(TileKind::Node(state))) => Some(state.node),
            _ => None,
        }
    {
        behavior.pending_tab_drag_stopped_nodes.insert(node_key);
        if workbench_surface.profile.interaction.tab_detach_enabled
            && GraphshellTileBehavior::should_detach_tab_on_drag_stop(
                ui,
                tab_rect,
                workbench_surface.profile.interaction.tab_detach_band_margin,
            )
        {
            behavior.graph_app.request_detach_node_to_split(node_key);
        }
    }

    if ui.is_rect_visible(tab_rect) && !state.is_being_dragged {
        let mut bg_color = behavior.tab_bg_color(ui.visuals(), tiles, tile_id, state);
        let mut stroke = behavior.tab_outline_stroke(ui.visuals(), tiles, tile_id, state);
        let tab_multi_selected = matches!(
            tiles.get(tile_id),
            Some(Tile::Pane(TileKind::Pane(crate::shell::desktop::workbench::pane_model::PaneViewState::Node(state))))
                if behavior.graph_app.workspace.selected_tab_nodes.contains(&state.node)
        ) || matches!(
            tiles.get(tile_id),
            Some(Tile::Pane(TileKind::Node(state)))
                if behavior.graph_app.workspace.selected_tab_nodes.contains(&state.node)
        );
        if tab_multi_selected && !state.active {
            bg_color = bg_color.linear_multiply(1.08);
            stroke = Stroke::new(stroke.width.max(1.5), Color32::from_rgb(95, 170, 255));
        }
        let tile_selected = behavior
            .graph_app
            .workbench_tile_selection()
            .selected_tile_ids
            .contains(&tile_id);
        if tile_selected {
            bg_color = bg_color.linear_multiply(if state.active { 1.12 } else { 1.06 });
            stroke = Stroke::new(
                stroke.width.max(if state.active { 2.0 } else { 1.75 }),
                if state.active {
                    Color32::from_rgb(255, 210, 90)
                } else {
                    Color32::from_rgb(120, 200, 255)
                },
            );
        }
        ui.painter().rect(
            tab_rect.shrink(0.5),
            0.0,
            bg_color,
            stroke,
            egui::StrokeKind::Inside,
        );

        if state.active {
            ui.painter().hline(
                tab_rect.x_range(),
                tab_rect.bottom(),
                Stroke::new(stroke.width + 1.0, bg_color),
            );
        }

        let mut text_rect = tab_rect.shrink(x_margin);
        if let Some(texture_id) = favicon_texture {
            let icon_rect = egui::Align2::LEFT_CENTER
                .align_size_within_rect(vec2(icon_size, icon_size), text_rect);
            ui.painter().image(
                texture_id,
                icon_rect,
                egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
            text_rect.min.x += icon_size + icon_spacing;
        }

        let text_color = behavior.tab_text_color(ui.visuals(), tiles, tile_id, state);
        let text_position = egui::Align2::LEFT_CENTER
            .align_size_within_rect(galley.size(), text_rect)
            .min;
        ui.painter().galley(text_position, galley, text_color);

        if state.closable {
            let close_btn_rect = egui::Align2::RIGHT_CENTER
                .align_size_within_rect(close_btn_size, tab_rect.shrink(x_margin));

            let close_btn_id = ui.auto_id_with("tab_close_btn");
            let close_btn_response = ui
                .interact(close_btn_rect, close_btn_id, Sense::click_and_drag())
                .on_hover_cursor(egui::CursorIcon::Default);

            let visuals = ui.style().interact(&close_btn_response);
            let rect = close_btn_rect
                .shrink(behavior.close_button_inner_margin())
                .expand(visuals.expansion);
            let stroke = visuals.fg_stroke;
            ui.painter()
                .line_segment([rect.left_top(), rect.right_bottom()], stroke);
            ui.painter()
                .line_segment([rect.right_top(), rect.left_bottom()], stroke);

            if close_btn_response.clicked() && behavior.on_tab_close(tiles, tile_id) {
                tiles.remove(tile_id);
            }
        }
    }

    behavior.on_tab_button(tiles, tile_id, tab_response)
}
