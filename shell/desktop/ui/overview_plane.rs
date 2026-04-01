/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use egui::{Color32, Context, Key, Pos2, RichText, Sense, Stroke, StrokeKind, Ui, Vec2, Window};

use crate::app::{
    GraphBrowserApp, GraphIntent, GraphViewId, GraphViewLayoutDirection, PendingTileOpenMode,
};

const OVERVIEW_CELL_SIZE: Vec2 = Vec2::new(156.0, 92.0);
const OVERVIEW_CELL_GAP: f32 = 16.0;

#[derive(Clone)]
struct OverviewSlotSnapshot {
    view_id: GraphViewId,
    name: String,
    row: i32,
    col: i32,
    archived: bool,
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

fn sorted_slot_snapshots(app: &GraphBrowserApp) -> Vec<OverviewSlotSnapshot> {
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

fn selected_overview_view_id(
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

    ui.label(format!("Focused view: {}", slot.name));
    ui.small(format!("Slot position: row {}, col {}", slot.row, slot.col));
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
}