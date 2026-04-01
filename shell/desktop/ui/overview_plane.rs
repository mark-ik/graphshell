/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use egui::{Color32, Context, Key, Pos2, RichText, Sense, Stroke, StrokeKind, Ui, Vec2, Window};

use crate::app::{
    GraphBrowserApp, GraphIntent, GraphViewId, GraphViewLayoutDirection, PendingTileOpenMode,
};

const OVERVIEW_CELL_SIZE: Vec2 = Vec2::new(156.0, 92.0);
const OVERVIEW_CELL_GAP: f32 = 16.0;
const OVERVIEW_SWATCH_GAP: f32 = 8.0;
const NAVIGATOR_OVERVIEW_SWATCH_MIN_WIDTH: f32 = 272.0;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OverviewSlotSnapshot {
    pub(crate) view_id: GraphViewId,
    pub(crate) name: String,
    pub(crate) row: i32,
    pub(crate) col: i32,
    pub(crate) archived: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OverviewSurfaceAction {
    FocusView(GraphViewId),
    OpenView(GraphViewId),
    ToggleOverviewPlane,
}

pub(crate) fn render_overview_plane(ctx: &Context, app: &mut GraphBrowserApp) {
    if !app.graph_view_layout_manager_active() {
        return;
    }

    let slots = sorted_slot_snapshots(app);
    let active_slots: Vec<_> = slots.iter().filter(|slot| !slot.archived).cloned().collect();
    let archived_slots: Vec<_> = slots.iter().filter(|slot| slot.archived).cloned().collect();
    let selected_view_id = selected_overview_view_id(app, &slots);
    let selected_slot = slots.iter().find(|slot| Some(slot.view_id) == selected_view_id);
    let mut open = true;
    let mut close_requested = false;
    let mut pending_intents = Vec::new();

    let response = Window::new("Overview Plane")
        .id(egui::Id::new("graphshell_overview_plane"))
        .default_pos(overview_window_pos(app))
        .default_width(880.0)
        .default_height(560.0)
        .resizable(true)
        .open(&mut open)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Graph-owned graph-view management")
                        .small()
                        .italics(),
                );
                ui.separator();
                if ui.button("Create View").clicked() {
                    pending_intents.push(GraphIntent::CreateGraphViewSlot {
                        anchor_view: selected_view_id,
                        direction: GraphViewLayoutDirection::Right,
                        open_mode: Some(PendingTileOpenMode::Tab),
                    });
                }
                if ui.button("Exit").clicked() {
                    close_requested = true;
                }
            });
            ui.small(
                "Click to focus a view, double-click to open it in the workbench, or drag a card to move its slot.",
            );
            ui.separator();

            ui.columns(2, |columns| {
                render_overview_grid(
                    &mut columns[0],
                    &active_slots,
                    selected_view_id,
                    &mut pending_intents,
                );
                render_overview_details(
                    &mut columns[1],
                    app,
                    ctx,
                    selected_slot,
                    &archived_slots,
                    &mut pending_intents,
                );
            });
        });

    if let Some(rect) = response.as_ref().map(|inner| inner.response.rect)
        && ctx.input(|input| input.pointer.primary_clicked())
        && let Some(pointer) = ctx.input(|input| input.pointer.interact_pos())
        && !rect.contains(pointer)
    {
        close_requested = true;
    }

    if close_requested || !open {
        pending_intents.push(GraphIntent::ExitGraphViewLayoutManager);
    }

    if !pending_intents.is_empty() {
        app.apply_reducer_intents(pending_intents);
    }
}

pub(crate) fn sorted_slot_snapshots(app: &GraphBrowserApp) -> Vec<OverviewSlotSnapshot> {
    let mut slots: Vec<_> = app
        .workspace
        .graph_runtime
        .graph_view_layout_manager
        .slots
        .values()
        .map(|slot| OverviewSlotSnapshot {
            view_id: slot.view_id,
            name: slot.name.clone(),
            row: slot.row,
            col: slot.col,
            archived: slot.archived,
        })
        .collect();
    slots.sort_by(|left, right| {
        left.archived
            .cmp(&right.archived)
            .then(left.row.cmp(&right.row))
            .then(left.col.cmp(&right.col))
            .then(left.name.cmp(&right.name))
    });
    slots
}

pub(crate) fn selected_overview_view_id(
    app: &GraphBrowserApp,
    slots: &[OverviewSlotSnapshot],
) -> Option<GraphViewId> {
    app.workspace
        .graph_runtime
        .focused_view
        .filter(|view_id| slots.iter().any(|slot| slot.view_id == *view_id))
        .or_else(|| slots.iter().find(|slot| !slot.archived).map(|slot| slot.view_id))
        .or_else(|| slots.first().map(|slot| slot.view_id))
}

fn overview_window_pos(app: &GraphBrowserApp) -> Pos2 {
    if let Some(view_id) = app.workspace.graph_runtime.focused_view
        && let Some(rect) = app.workspace.graph_runtime.graph_view_canvas_rects.get(&view_id)
    {
        return Pos2::new(rect.left() + 24.0, rect.top() + 24.0);
    }

    app.workspace
        .graph_runtime
        .graph_view_canvas_rects
        .values()
        .next()
        .map(|rect| Pos2::new(rect.left() + 24.0, rect.top() + 24.0))
        .unwrap_or_else(|| Pos2::new(48.0, 96.0))
}

pub(crate) fn render_navigator_overview_swatch(
    ui: &mut Ui,
    app: &GraphBrowserApp,
) -> Vec<OverviewSurfaceAction> {
    let slots = sorted_slot_snapshots(app);
    let active_slots: Vec<_> = slots.iter().filter(|slot| !slot.archived).cloned().collect();
    let archived_count = slots.iter().filter(|slot| slot.archived).count();
    let selected_view_id = selected_overview_view_id(app, &slots);
    let mut actions = Vec::new();
    let show_archived_id = egui::Id::new("navigator_overview_show_archived");
    let mut show_archived = ui
        .ctx()
        .data_mut(|data| data.get_persisted::<bool>(show_archived_id))
        .unwrap_or(false);
    let swatch_enabled = navigator_overview_swatch_enabled(ui.available_width());

    ui.horizontal(|ui| {
        ui.label(RichText::new("Views").small().strong());
        ui.separator();
        ui.label(
            RichText::new(format!(
                "{} active · {} archived",
                active_slots.len(),
                archived_count
            ))
            .small()
            .weak(),
        );
        if archived_count > 0 {
            ui.separator();
            let archived_label = if show_archived {
                "Hide archived"
            } else {
                "Show archived"
            };
            if ui
                .small_button(archived_label)
                .on_hover_text("Toggle archived graph views in the Navigator list")
                .clicked()
            {
                show_archived = !show_archived;
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let manage_label = if app.graph_view_layout_manager_active() {
                "Manage*"
            } else {
                "Manage"
            };
            if ui
                .small_button(manage_label)
                .on_hover_text("Open the full Overview Plane")
                .clicked()
            {
                actions.push(OverviewSurfaceAction::ToggleOverviewPlane);
            }
        });
    });
    ui.ctx()
        .data_mut(|data| data.insert_persisted(show_archived_id, show_archived));

    if active_slots.is_empty() {
        ui.small("No active graph views yet.");
        return actions;
    }

    let list_slots: Vec<_> = if show_archived {
        slots.iter().collect()
    } else {
        active_slots.iter().collect()
    };
    ui.vertical(|ui| {
        for slot in list_slots {
            ui.horizontal_wrapped(|ui| {
                let label = if Some(slot.view_id) == selected_view_id {
                    RichText::new(&slot.name).small().strong()
                } else {
                    RichText::new(&slot.name).small()
                };
                if ui.selectable_label(Some(slot.view_id) == selected_view_id, label).clicked() {
                    actions.push(OverviewSurfaceAction::FocusView(slot.view_id));
                }
                if ui.small_button("Open").clicked() {
                    actions.push(OverviewSurfaceAction::OpenView(slot.view_id));
                }

                let mut hints = vec![format!("r{} · c{}", slot.row, slot.col)];
                if slot.archived {
                    hints.push("archived".to_string());
                }
                if let Some(node_count) = app.graph_view_owned_node_count(slot.view_id) {
                    hints.push(format!("{node_count} nodes"));
                }
                let external_links = app.graph_view_external_link_count(slot.view_id);
                if external_links > 0 {
                    hints.push(format!("{external_links} x-links"));
                }
                ui.label(RichText::new(hints.join(" · ")).small().weak());
            });
        }
    });

    if swatch_enabled {
        ui.add_space(6.0);
        render_compact_overview_grid(ui, &active_slots, selected_view_id, &mut actions);
    }

    if let Some(selected_slot) = slots
        .iter()
        .find(|slot| Some(slot.view_id) == selected_view_id)
    {
        ui.horizontal_wrapped(|ui| {
            let mut summary = vec![format!(
                "Focused: {} (r{} · c{})",
                selected_slot.name, selected_slot.row, selected_slot.col
            )];
            if let Some(node_count) = app.graph_view_owned_node_count(selected_slot.view_id) {
                summary.push(format!("{node_count} nodes"));
            }
            ui.label(
                RichText::new(summary.join(" · ")).small().weak(),
            );
            if ui.small_button("Open").clicked() {
                actions.push(OverviewSurfaceAction::OpenView(selected_slot.view_id));
            }
        });
    }

    actions
}

fn render_compact_overview_grid(
    ui: &mut Ui,
    slots: &[OverviewSlotSnapshot],
    selected_view_id: Option<GraphViewId>,
    actions: &mut Vec<OverviewSurfaceAction>,
) {
    let Some((min_row, max_row, min_col, max_col)) = overview_grid_bounds(slots) else {
        return;
    };

    let rows = (max_row - min_row + 1).max(1) as f32;
    let cols = (max_col - min_col + 1).max(1) as f32;
    let available_width = ui.available_width().max(140.0);
    let cell_width = ((available_width - (cols - 1.0) * OVERVIEW_SWATCH_GAP) / cols)
        .clamp(42.0, 76.0);
    let cell_height = (cell_width * 0.58).clamp(24.0, 44.0);
    let grid_size = Vec2::new(
        cols * cell_width + (cols - 1.0) * OVERVIEW_SWATCH_GAP,
        rows * cell_height + (rows - 1.0) * OVERVIEW_SWATCH_GAP,
    );
    let (grid_rect, _) = ui.allocate_exact_size(grid_size, Sense::hover());
    let painter = ui.painter();

    for slot in slots {
        let cell_rect = compact_slot_rect_for_coords(
            slot.row,
            slot.col,
            min_row,
            min_col,
            grid_rect.min,
            Vec2::new(cell_width, cell_height),
        );
        let response = ui.interact(
            cell_rect,
            egui::Id::new(("navigator_overview_slot", slot.view_id.as_uuid())),
            Sense::click(),
        );
        let is_selected = Some(slot.view_id) == selected_view_id;
        let fill = if is_selected {
            Color32::from_rgb(66, 88, 120)
        } else {
            Color32::from_rgb(40, 45, 54)
        };
        let stroke = if is_selected {
            Stroke::new(2.0, Color32::from_rgb(180, 210, 255))
        } else {
            Stroke::new(1.0, Color32::from_gray(100))
        };
        painter.rect_filled(cell_rect, 6.0, fill);
        painter.rect_stroke(cell_rect, 6.0, stroke, StrokeKind::Outside);
        painter.text(
            cell_rect.center(),
            egui::Align2::CENTER_CENTER,
            compact_overview_label(&slot.name, if cell_width >= 60.0 { 12 } else { 6 }),
            egui::TextStyle::Small.resolve(ui.style()),
            Color32::WHITE,
        );

        if response.clicked() {
            actions.push(OverviewSurfaceAction::FocusView(slot.view_id));
        }
        if response.double_clicked() {
            actions.push(OverviewSurfaceAction::OpenView(slot.view_id));
        }
    }
}

fn render_overview_grid(
    ui: &mut Ui,
    slots: &[OverviewSlotSnapshot],
    selected_view_id: Option<GraphViewId>,
    pending_intents: &mut Vec<GraphIntent>,
) {
    ui.label(RichText::new("View regions").strong());
    if slots.is_empty() {
        ui.small("No active graph views yet.");
        return;
    }

    let Some((min_row, max_row, min_col, max_col)) = overview_grid_bounds(slots) else {
        ui.small("No active graph views yet.");
        return;
    };

    let rows = (max_row - min_row + 1).max(1) as f32;
    let cols = (max_col - min_col + 1).max(1) as f32;
    let grid_size = Vec2::new(
        cols * OVERVIEW_CELL_SIZE.x + (cols - 1.0) * OVERVIEW_CELL_GAP,
        rows * OVERVIEW_CELL_SIZE.y + (rows - 1.0) * OVERVIEW_CELL_GAP,
    );
    let (grid_rect, _) = ui.allocate_exact_size(grid_size, Sense::hover());
    let painter = ui.painter();

    for slot in slots {
        let cell_rect = slot_rect(slot, min_row, min_col, grid_rect.min);
        let response = ui.interact(
            cell_rect,
            egui::Id::new(("overview_plane_slot", slot.view_id.as_uuid())),
            Sense::click_and_drag(),
        );
        let is_selected = Some(slot.view_id) == selected_view_id;
        let fill = if is_selected {
            Color32::from_rgb(66, 88, 120)
        } else {
            Color32::from_rgb(42, 48, 60)
        };
        let stroke = if is_selected {
            Stroke::new(2.0, Color32::from_rgb(180, 210, 255))
        } else {
            Stroke::new(1.0, Color32::from_gray(110))
        };
        painter.rect_filled(cell_rect, 8.0, fill);
        painter.rect_stroke(cell_rect, 8.0, stroke, StrokeKind::Outside);
        painter.text(
            cell_rect.left_top() + Vec2::new(10.0, 10.0),
            egui::Align2::LEFT_TOP,
            &slot.name,
            egui::TextStyle::Button.resolve(ui.style()),
            Color32::WHITE,
        );
        painter.text(
            cell_rect.left_bottom() + Vec2::new(10.0, -10.0),
            egui::Align2::LEFT_BOTTOM,
            format!("r{} · c{}", slot.row, slot.col),
            egui::TextStyle::Small.resolve(ui.style()),
            Color32::from_gray(220),
        );

        if response.clicked() {
            pending_intents.push(GraphIntent::FocusGraphView {
                view_id: slot.view_id,
            });
        }
        if response.double_clicked() {
            pending_intents.push(GraphIntent::RouteGraphViewToWorkbench {
                view_id: slot.view_id,
                mode: PendingTileOpenMode::Tab,
            });
        }
        if response.drag_stopped() {
            let (target_row, target_col) = drag_target_slot_position(slot, response.drag_delta());
            if target_row != slot.row || target_col != slot.col {
                pending_intents.push(GraphIntent::MoveGraphViewSlot {
                    view_id: slot.view_id,
                    row: target_row,
                    col: target_col,
                });
            }
        }
        if response.dragged() {
            let (target_row, target_col) = drag_target_slot_position(slot, response.drag_delta());
            if target_row != slot.row || target_col != slot.col {
                let preview_rect =
                    slot_rect_for_coords(target_row, target_col, min_row, min_col, grid_rect.min);
                painter.rect_stroke(
                    preview_rect.expand(2.0),
                    10.0,
                    Stroke::new(2.0, Color32::from_rgb(120, 210, 180)),
                    StrokeKind::Outside,
                );
            }
        }
    }
}

fn render_overview_details(
    ui: &mut Ui,
    app: &GraphBrowserApp,
    ctx: &Context,
    selected_slot: Option<&OverviewSlotSnapshot>,
    archived_slots: &[OverviewSlotSnapshot],
    pending_intents: &mut Vec<GraphIntent>,
) {
    ui.label(RichText::new("Details").strong());
    let Some(slot) = selected_slot else {
        ui.small("Select a graph view region to inspect it.");
        return;
    };

    let rename_id = egui::Id::new(("overview_plane_rename", slot.view_id.as_uuid()));
    let mut rename_draft = ctx
        .data_mut(|data| data.get_persisted::<String>(rename_id))
        .unwrap_or_else(|| slot.name.clone());

    ui.label(format!("Selected view: {}", slot.name));
    ui.small(format!("Slot position: row {}, col {}", slot.row, slot.col));
    if let Some(node_count) = app.graph_view_owned_node_count(slot.view_id) {
        ui.small(format!("Owned nodes: {node_count}"));
    }
    ui.add_space(6.0);

    let rename_response = ui.text_edit_singleline(&mut rename_draft);
    ctx.data_mut(|data| data.insert_persisted(rename_id, rename_draft.clone()));
    if (rename_response.lost_focus() && ui.input(|input| input.key_pressed(Key::Enter)))
        || ui.button("Rename").clicked()
    {
        let trimmed = rename_draft.trim();
        if !trimmed.is_empty() && trimmed != slot.name {
            pending_intents.push(GraphIntent::RenameGraphViewSlot {
                view_id: slot.view_id,
                name: trimmed.to_string(),
            });
        }
    }

    ui.horizontal(|ui| {
        if ui.button("Open").clicked() {
            pending_intents.push(GraphIntent::RouteGraphViewToWorkbench {
                view_id: slot.view_id,
                mode: PendingTileOpenMode::Tab,
            });
        }
        if ui.button("Focus").clicked() {
            pending_intents.push(GraphIntent::FocusGraphView {
                view_id: slot.view_id,
            });
        }
        if ui.button("Archive").clicked() {
            pending_intents.push(GraphIntent::ArchiveGraphViewSlot {
                view_id: slot.view_id,
            });
        }
    });

    ui.separator();
    ui.small("Transfer focused selection");
    let focused_view = app.workspace.graph_runtime.focused_view;
    let focused_selection_count = app.focused_selection().len();
    let transfer_enabled = focused_view.is_some()
        && focused_selection_count > 0
        && focused_view != Some(slot.view_id);
    let transfer_reason = if focused_view.is_none() {
        "Focus a source graph view first."
    } else if focused_selection_count == 0 {
        "Select one or more nodes in the focused graph view first."
    } else {
        "Selected view already owns the focused selection."
    };
    let move_button = ui.add_enabled(
        transfer_enabled,
        egui::Button::new(format!("Move {focused_selection_count} selected node(s) here")),
    );
    let move_button = if transfer_enabled {
        move_button.on_hover_text("Transfer the focused selection into this graph view")
    } else {
        move_button.on_disabled_hover_text(transfer_reason)
    };
    if move_button.clicked()
        && let Some(source_view) = focused_view
    {
        pending_intents.push(GraphIntent::TransferSelectedNodesToGraphView {
            source_view,
            destination_view: slot.view_id,
        });
    }

    ui.separator();
    ui.small("Move slot");
    ui.horizontal(|ui| {
        directional_button(
            ui,
            "Left",
            GraphViewLayoutDirection::Left,
            slot,
            pending_intents,
            false,
        );
        directional_button(
            ui,
            "Right",
            GraphViewLayoutDirection::Right,
            slot,
            pending_intents,
            false,
        );
    });
    ui.horizontal(|ui| {
        directional_button(
            ui,
            "Up",
            GraphViewLayoutDirection::Up,
            slot,
            pending_intents,
            false,
        );
        directional_button(
            ui,
            "Down",
            GraphViewLayoutDirection::Down,
            slot,
            pending_intents,
            false,
        );
    });

    ui.separator();
    ui.small("Create adjacent view");
    ui.horizontal(|ui| {
        directional_button(
            ui,
            "+ Left",
            GraphViewLayoutDirection::Left,
            slot,
            pending_intents,
            true,
        );
        directional_button(
            ui,
            "+ Right",
            GraphViewLayoutDirection::Right,
            slot,
            pending_intents,
            true,
        );
    });
    ui.horizontal(|ui| {
        directional_button(
            ui,
            "+ Up",
            GraphViewLayoutDirection::Up,
            slot,
            pending_intents,
            true,
        );
        directional_button(
            ui,
            "+ Down",
            GraphViewLayoutDirection::Down,
            slot,
            pending_intents,
            true,
        );
    });

    if !archived_slots.is_empty() {
        ui.separator();
        ui.collapsing("Archived views", |ui| {
            for archived in archived_slots {
                ui.horizontal(|ui| {
                    ui.label(&archived.name);
                    if ui.button("Restore").clicked() {
                        pending_intents.push(GraphIntent::RestoreGraphViewSlot {
                            view_id: archived.view_id,
                            row: archived.row,
                            col: archived.col,
                        });
                    }
                });
            }
        });
    }
}

fn directional_button(
    ui: &mut Ui,
    label: &str,
    direction: GraphViewLayoutDirection,
    slot: &OverviewSlotSnapshot,
    pending_intents: &mut Vec<GraphIntent>,
    create: bool,
) {
    if ui.button(label).clicked() {
        if create {
            pending_intents.push(GraphIntent::CreateGraphViewSlot {
                anchor_view: Some(slot.view_id),
                direction,
                open_mode: Some(PendingTileOpenMode::Tab),
            });
        } else {
            let (row, col) = shifted_slot_position(slot.row, slot.col, direction);
            pending_intents.push(GraphIntent::MoveGraphViewSlot {
                view_id: slot.view_id,
                row,
                col,
            });
        }
    }
}

fn overview_grid_bounds(slots: &[OverviewSlotSnapshot]) -> Option<(i32, i32, i32, i32)> {
    let mut iter = slots.iter();
    let first = iter.next()?;
    let mut min_row = first.row;
    let mut max_row = first.row;
    let mut min_col = first.col;
    let mut max_col = first.col;
    for slot in iter {
        min_row = min_row.min(slot.row);
        max_row = max_row.max(slot.row);
        min_col = min_col.min(slot.col);
        max_col = max_col.max(slot.col);
    }
    Some((min_row, max_row, min_col, max_col))
}

fn slot_rect(
    slot: &OverviewSlotSnapshot,
    min_row: i32,
    min_col: i32,
    origin: Pos2,
) -> egui::Rect {
    slot_rect_for_coords(slot.row, slot.col, min_row, min_col, origin)
}

fn slot_rect_for_coords(
    row: i32,
    col: i32,
    min_row: i32,
    min_col: i32,
    origin: Pos2,
) -> egui::Rect {
    let x = origin.x + (col - min_col) as f32 * (OVERVIEW_CELL_SIZE.x + OVERVIEW_CELL_GAP);
    let y = origin.y + (row - min_row) as f32 * (OVERVIEW_CELL_SIZE.y + OVERVIEW_CELL_GAP);
    egui::Rect::from_min_size(Pos2::new(x, y), OVERVIEW_CELL_SIZE)
}

fn compact_slot_rect_for_coords(
    row: i32,
    col: i32,
    min_row: i32,
    min_col: i32,
    origin: Pos2,
    cell_size: Vec2,
) -> egui::Rect {
    let x = origin.x + (col - min_col) as f32 * (cell_size.x + OVERVIEW_SWATCH_GAP);
    let y = origin.y + (row - min_row) as f32 * (cell_size.y + OVERVIEW_SWATCH_GAP);
    egui::Rect::from_min_size(Pos2::new(x, y), cell_size)
}

fn compact_overview_label(name: &str, max_chars: usize) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return "View".to_string();
    }
    if trimmed.chars().count() <= max_chars {
        return trimmed.to_string();
    }
    let mut compact: String = trimmed.chars().take(max_chars.saturating_sub(1)).collect();
    compact.push('…');
    compact
}

fn navigator_overview_swatch_enabled(available_width: f32) -> bool {
    available_width >= NAVIGATOR_OVERVIEW_SWATCH_MIN_WIDTH
}

fn drag_target_slot_position(slot: &OverviewSlotSnapshot, drag_delta: Vec2) -> (i32, i32) {
    let col_delta = (drag_delta.x / (OVERVIEW_CELL_SIZE.x + OVERVIEW_CELL_GAP)).round() as i32;
    let row_delta = (drag_delta.y / (OVERVIEW_CELL_SIZE.y + OVERVIEW_CELL_GAP)).round() as i32;
    (slot.row + row_delta, slot.col + col_delta)
}

fn shifted_slot_position(row: i32, col: i32, direction: GraphViewLayoutDirection) -> (i32, i32) {
    match direction {
        GraphViewLayoutDirection::Up => (row - 1, col),
        GraphViewLayoutDirection::Down => (row + 1, col),
        GraphViewLayoutDirection::Left => (row, col - 1),
        GraphViewLayoutDirection::Right => (row, col + 1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drag_target_slot_position_rounds_to_nearest_grid_cell() {
        let slot = OverviewSlotSnapshot {
            view_id: GraphViewId::new(),
            name: "View".to_string(),
            row: 4,
            col: 7,
            archived: false,
        };

        let horizontal_step = OVERVIEW_CELL_SIZE.x + OVERVIEW_CELL_GAP;
        let vertical_step = OVERVIEW_CELL_SIZE.y + OVERVIEW_CELL_GAP;

        assert_eq!(
            drag_target_slot_position(
                &slot,
                Vec2::new(horizontal_step * 1.1, -vertical_step * 0.9)
            ),
            (3, 8)
        );
    }

    #[test]
    fn selected_overview_view_id_prefers_focused_view() {
        let view_a = GraphViewId::new();
        let view_b = GraphViewId::new();
        let mut app = GraphBrowserApp::new_for_testing();
        app.ensure_graph_view_registered(view_a);
        app.ensure_graph_view_registered(view_b);
        app.workspace.graph_runtime.focused_view = Some(view_b);

        let slots = sorted_slot_snapshots(&app);
        assert_eq!(selected_overview_view_id(&app, &slots), Some(view_b));
    }

    #[test]
    fn navigator_overview_swatch_enabled_requires_sidebar_width() {
        assert!(!navigator_overview_swatch_enabled(240.0));
        assert!(navigator_overview_swatch_enabled(NAVIGATOR_OVERVIEW_SWATCH_MIN_WIDTH));
    }

    #[test]
    fn sorted_slot_snapshots_lists_active_before_archived() {
        let active = GraphViewId::new();
        let archived = GraphViewId::new();
        let mut app = GraphBrowserApp::new_for_testing();
        app.ensure_graph_view_registered(active);
        app.ensure_graph_view_registered(archived);
        app.archive_graph_view_slot(archived);

        let slots = sorted_slot_snapshots(&app);

        assert_eq!(slots.first().map(|slot| slot.view_id), Some(active));
        assert_eq!(slots.last().map(|slot| slot.view_id), Some(archived));
    }
}