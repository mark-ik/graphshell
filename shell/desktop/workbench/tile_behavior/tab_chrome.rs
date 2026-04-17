use super::*;
use crate::graph::badge::{Badge, badges_for_node, tab_badge_token};
use crate::shell::desktop::ui::toolbar::toolbar_ui::CommandBarFocusTarget;
use crate::shell::desktop::ui::toolbar_routing;

impl<'a> GraphshellTileBehavior<'a> {
    pub(super) fn tab_title_for_tile(&mut self, pane: &TileKind) -> WidgetText {
        match pane {
            TileKind::Pane(crate::shell::desktop::workbench::pane_model::PaneViewState::Graph(
                view_ref,
            )) => self
                .graph_app
                .workspace
                .graph_runtime
                .views
                .get(&view_ref.graph_view_id)
                .map(|v| v.name.clone().into())
                .unwrap_or_else(|| "Graph".into()),
            TileKind::Pane(crate::shell::desktop::workbench::pane_model::PaneViewState::Node(
                state,
            )) => self
                .graph_app
                .domain_graph()
                .get_node(state.node)
                .map(|n| n.title.clone().into())
                .unwrap_or_else(|| format!("Node {:?}", state.node).into()),
            #[cfg(feature = "diagnostics")]
            TileKind::Pane(crate::shell::desktop::workbench::pane_model::PaneViewState::Tool(
                tool,
            )) => tool.title().into(),
            TileKind::Graph(view_ref) => self
                .graph_app
                .workspace
                .graph_runtime
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
    let chip_spacing = 6.0;
    let x_margin = behavior.tab_title_spacing(ui.visuals());
    let workbench_surface = registries::phase3_resolve_active_workbench_surface_profile();
    let node_key_for_tab = match tiles.get(tile_id) {
        Some(Tile::Pane(TileKind::Pane(
            crate::shell::desktop::workbench::pane_model::PaneViewState::Node(state),
        ))) => Some(state.node),
        Some(Tile::Pane(TileKind::Node(state))) => Some(state.node),
        _ => None,
    };
    let frame_chip_label = node_key_for_tab
        .and_then(|node_key| primary_frame_name_for_node(behavior.graph_app, node_key));
    let split_offer = node_key_for_tab.and_then(|node_key| {
        node_frame_split_offer_candidate(behavior.graph_app, node_key)
            .filter(|candidate| frame_chip_label.as_deref() != Some(candidate.frame_name.as_str()))
    });

    let (title_text, favicon_texture) = match tiles.get(tile_id) {
        Some(Tile::Pane(TileKind::Pane(
            crate::shell::desktop::workbench::pane_model::PaneViewState::Graph(view_ref),
        ))) => {
            let name = behavior
                .graph_app
                .workspace
                .graph_runtime
                .views
                .get(&view_ref.graph_view_id)
                .map(|v| v.name.clone())
                .unwrap_or_else(|| "Graph".to_string());
            (name, None)
        }
        Some(Tile::Pane(TileKind::Pane(
            crate::shell::desktop::workbench::pane_model::PaneViewState::Node(state),
        ))) => {
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
        Some(Tile::Pane(TileKind::Pane(
            crate::shell::desktop::workbench::pane_model::PaneViewState::Tool(tool),
        ))) => (tool.title().to_string(), None),
        Some(Tile::Pane(TileKind::Graph(view_ref))) => {
            let name = behavior
                .graph_app
                .workspace
                .graph_runtime
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
            let label = crate::shell::desktop::ui::persistence_ops::frame_hint_tab_info(
                behavior.graph_app,
                tile_id,
            )
            .map(|(_, _, hint)| {
                crate::shell::desktop::ui::persistence_ops::frame_layout_hint_summary(&hint)
            })
            .unwrap_or_else(|| match linear.dir {
                egui_tiles::LinearDir::Horizontal => {
                    workbench_surface.profile.split_horizontal_label.clone()
                }
                egui_tiles::LinearDir::Vertical => {
                    workbench_surface.profile.split_vertical_label.clone()
                }
            });
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
    let chip_font_id = egui::FontId::proportional(10.0);
    let chip_galley = frame_chip_label.as_ref().map(|frame_name| {
        ui.painter().layout_no_wrap(
            format!("Frame {frame_name}"),
            chip_font_id.clone(),
            ui.visuals().selection.stroke.color,
        )
    });
    let offer_galley = split_offer.as_ref().map(|candidate| {
        ui.painter().layout_no_wrap(
            if candidate.hint_count == 1 {
                "Split".to_string()
            } else {
                format!("Split {}", candidate.hint_count)
            },
            chip_font_id.clone(),
            egui::Color32::from_rgb(210, 225, 240),
        )
    });

    let icon_width = if favicon_texture.is_some() {
        icon_size + icon_spacing
    } else {
        0.0
    };
    let chip_width = chip_galley
        .as_ref()
        .map(|galley| galley.size().x + 10.0 + chip_spacing)
        .unwrap_or(0.0);
    let offer_width = offer_galley
        .as_ref()
        .map(|galley| galley.size().x + 10.0 + chip_spacing)
        .unwrap_or(0.0);
    let button_width = galley.size().x
        + 2.0 * x_margin
        + icon_width
        + chip_width
        + offer_width
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
        if let Some(Tile::Pane(kind)) = tiles.get(tile_id) {
            behavior
                .graph_app
                .enqueue_workbench_intent(WorkbenchIntent::UpdatePaneSelection {
                    pane_id: kind.pane_id(),
                    mode: tile_selection_mode,
                });
        }

        if let Some(Tile::Pane(TileKind::Pane(
            crate::shell::desktop::workbench::pane_model::PaneViewState::Node(state),
        ))) = tiles.get(tile_id)
        {
            let node_key = state.node;
            if modifiers.shift {
                let ordered_nodes =
                    crate::shell::desktop::workbench::semantic_tabs::semantic_tab_node_order_for_tile_in_tiles(
                        tiles,
                        behavior.graph_app,
                        tile_id,
                    )
                    .unwrap_or_else(|| vec![node_key]);
                let target_index = ordered_nodes
                    .iter()
                    .position(|key| *key == node_key)
                    .unwrap_or(0);
                let anchor_key = behavior
                    .graph_app
                    .workspace
                    .graph_runtime
                    .tab_selection_anchor
                    .unwrap_or(node_key);
                let anchor_index = ordered_nodes
                    .iter()
                    .position(|key| *key == anchor_key)
                    .unwrap_or(target_index);
                if !modifiers.ctrl {
                    behavior
                        .graph_app
                        .workspace
                        .graph_runtime
                        .selected_tab_nodes
                        .clear();
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
                    crate::shell::desktop::workbench::semantic_tabs::semantic_tab_node_order_for_tile_in_tiles(
                        tiles,
                        behavior.graph_app,
                        tile_id,
                    )
                    .unwrap_or_else(|| vec![node_key]);
                let target_index = ordered_nodes
                    .iter()
                    .position(|key| *key == node_key)
                    .unwrap_or(0);
                let anchor_key = behavior
                    .graph_app
                    .workspace
                    .graph_runtime
                    .tab_selection_anchor
                    .unwrap_or(node_key);
                let anchor_index = ordered_nodes
                    .iter()
                    .position(|key| *key == anchor_key)
                    .unwrap_or(target_index);
                if !modifiers.ctrl {
                    behavior
                        .graph_app
                        .workspace
                        .graph_runtime
                        .selected_tab_nodes
                        .clear();
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
            Some(Tile::Pane(TileKind::Pane(
                crate::shell::desktop::workbench::pane_model::PaneViewState::Node(state),
            ))) => Some(state.node),
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
                if behavior.graph_app.workspace.graph_runtime.selected_tab_nodes.contains(&state.node)
        ) || matches!(
            tiles.get(tile_id),
            Some(Tile::Pane(TileKind::Node(state)))
                if behavior.graph_app.workspace.graph_runtime.selected_tab_nodes.contains(&state.node)
        );
        if tab_multi_selected && !state.active {
            bg_color = bg_color.linear_multiply(1.08);
            stroke = Stroke::new(stroke.width.max(1.5), Color32::from_rgb(95, 170, 255));
        }
        let tile_selected = tiles
            .get(tile_id)
            .and_then(|tile| match tile {
                Tile::Pane(kind) => Some(
                    behavior
                        .graph_app
                        .workbench_tile_selection()
                        .selected_pane_ids
                        .contains(&kind.pane_id()),
                ),
                _ => None,
            })
            .unwrap_or(false);
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
        if let Some(frame_name) = frame_chip_label.as_ref()
            && let Some(chip_galley) = chip_galley.as_ref()
        {
            let chip_size = chip_galley.size() + egui::vec2(10.0, 4.0);
            let chip_rect = egui::Rect::from_min_size(
                egui::pos2(text_rect.min.x, tab_rect.center().y - chip_size.y * 0.5),
                chip_size,
            );
            let chip_id = ui.auto_id_with(("frame_chip", tile_id));
            let chip_response = ui
                .interact(chip_rect, chip_id, Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            ui.painter().rect(
                chip_rect,
                egui::CornerRadius::same(6),
                egui::Color32::from_rgba_unmultiplied(70, 110, 150, 40),
                Stroke::new(1.0, ui.visuals().selection.stroke.color),
                egui::StrokeKind::Inside,
            );
            ui.painter().galley(
                chip_rect.center() - chip_galley.size() * 0.5,
                chip_galley.clone(),
                ui.visuals().selection.stroke.color,
            );
            if chip_response.clicked() {
                behavior.queue_post_render_intent(ViewAction::SetSelectedFrame {
                    frame_name: Some(frame_name.clone()),
                });
            }
            if chip_response.secondary_clicked() {
                behavior.graph_app.set_pending_node_context_target(None);
                behavior
                    .graph_app
                    .set_pending_frame_context_target(Some(frame_name.clone()));
                match behavior.graph_app.context_command_surface_preference() {
                    crate::app::ContextCommandSurfacePreference::RadialPalette => {
                        if let Some(pointer) = ui.input(|i| i.pointer.latest_pos()) {
                            ui.ctx().data_mut(|d| {
                                d.insert_persisted(egui::Id::new("radial_menu_center"), pointer);
                            });
                        }
                        if !behavior.graph_app.workspace.chrome_ui.show_radial_menu {
                            let _ = toolbar_routing::request_radial_menu_toggle(
                                behavior.graph_app,
                                CommandBarFocusTarget::new(None, node_key_for_tab),
                            );
                        }
                    }
                    crate::app::ContextCommandSurfacePreference::ContextPalette => {
                        behavior.graph_app.set_context_palette_anchor(
                            ui.input(|i| i.pointer.latest_pos().map(|pos| [pos.x, pos.y])),
                        );
                        behavior.graph_app.open_context_palette();
                    }
                }
            }
            text_rect.min.x = chip_rect.max.x + chip_spacing;
        }
        if let Some(candidate) = split_offer.as_ref()
            && let Some(offer_galley) = offer_galley.as_ref()
            && let Some(node_key) = node_key_for_tab
        {
            let offer_size = offer_galley.size() + egui::vec2(10.0, 4.0);
            let offer_rect = egui::Rect::from_min_size(
                egui::pos2(text_rect.min.x, tab_rect.center().y - offer_size.y * 0.5),
                offer_size,
            );
            let offer_id = ui.auto_id_with(("frame_split_offer", tile_id));
            let offer_response = ui
                .interact(offer_rect, offer_id, Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            ui.painter().rect(
                offer_rect,
                egui::CornerRadius::same(6),
                egui::Color32::from_rgba_unmultiplied(40, 90, 120, 55),
                Stroke::new(1.0, egui::Color32::from_rgb(120, 200, 255)),
                egui::StrokeKind::Inside,
            );
            ui.painter().galley(
                offer_rect.center() - offer_galley.size() * 0.5,
                offer_galley.clone(),
                egui::Color32::from_rgb(210, 225, 240),
            );
            if offer_response.clicked() {
                behavior.queue_post_render_intent(GraphIntent::OpenNodeFrameRouted {
                    key: node_key,
                    prefer_frame: Some(candidate.frame_name.clone()),
                });
            }
            text_rect.min.x = offer_rect.max.x + chip_spacing;
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
