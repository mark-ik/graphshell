use crate::app::{GraphBrowserApp, WorkbenchIntent};
use egui::{WidgetInfo, WidgetType};
use std::collections::HashSet;

/// **Authority: Graph** — graph history manipulation. Called from the Graph
/// authority pass in the left column.
pub(crate) fn render_graph_history_buttons(
    ui: &mut egui::Ui,
    frame_intents: &mut Vec<crate::app::GraphIntent>,
) {
    let undo_button = ui.add(super::toolbar_button("Undo"));
    undo_button.widget_info(|| {
        let mut info = WidgetInfo::new(WidgetType::Button);
        info.label = Some("Undo".into());
        info
    });
    if undo_button.clicked() {
        frame_intents.push(crate::app::GraphIntent::Undo);
    }

    let redo_button = ui.add(super::toolbar_button("Redo"));
    redo_button.widget_info(|| {
        let mut info = WidgetInfo::new(WidgetType::Button);
        info.label = Some("Redo".into());
        info
    });
    if redo_button.clicked() {
        frame_intents.push(crate::app::GraphIntent::Redo);
    }
}

pub(super) fn render_frame_pin_controls(
    ui: &mut egui::Ui,
    graph_app: &mut GraphBrowserApp,
    has_hosted_panes: bool,
    focused_pane_pin_name: Option<&str>,
    persisted_frame_names: &HashSet<String>,
) {
    if !has_hosted_panes {
        return;
    }

    if let Some(pane_pin_name) = focused_pane_pin_name {
        let pane_is_pinned = persisted_frame_names.contains(pane_pin_name);
        let pane_pin_label = if pane_is_pinned { "P-" } else { "P+" };
        let pane_pin_button =
            ui.add(super::toolbar_button(pane_pin_label))
                .on_hover_text(if pane_is_pinned {
                    "Unpin focused pane frame snapshot"
                } else {
                    "Pin focused pane frame snapshot"
                });
        if pane_pin_button.clicked() {
            if pane_is_pinned {
                graph_app.enqueue_workbench_intent(WorkbenchIntent::DeleteFrame {
                    frame_name: pane_pin_name.to_string(),
                });
            } else {
                graph_app.enqueue_workbench_intent(WorkbenchIntent::SaveFrameSnapshotNamed {
                    name: pane_pin_name.to_string(),
                });
            }
        }

        let pane_recall_button = ui
            .add_enabled(pane_is_pinned, super::toolbar_button("PR"))
            .on_hover_text("Recall focused pane pinned frame");
        if pane_recall_button.clicked() {
            graph_app.enqueue_workbench_intent(WorkbenchIntent::RestoreFrame {
                name: pane_pin_name.to_string(),
            });
        }
    }

    let space_is_pinned = persisted_frame_names.contains(super::WORKSPACE_PIN_NAME);
    let space_pin_label = if space_is_pinned { "W-" } else { "W+" };
    let space_pin_button = ui
        .add(super::toolbar_button(space_pin_label))
        .on_hover_text(if space_is_pinned {
            "Unpin current frame snapshot"
        } else {
            "Pin current frame snapshot"
        });
    if space_pin_button.clicked() {
        if space_is_pinned {
            graph_app.enqueue_workbench_intent(WorkbenchIntent::DeleteFrame {
                frame_name: super::WORKSPACE_PIN_NAME.to_string(),
            });
        } else {
            graph_app.enqueue_workbench_intent(WorkbenchIntent::SaveFrameSnapshotNamed {
                name: super::WORKSPACE_PIN_NAME.to_string(),
            });
        }
    }

    let space_recall_button = ui
        .add_enabled(space_is_pinned, super::toolbar_button("WR"))
        .on_hover_text("Recall pinned frame snapshot");
    if space_recall_button.clicked() {
        graph_app.enqueue_workbench_intent(WorkbenchIntent::RestoreFrame {
            name: super::WORKSPACE_PIN_NAME.to_string(),
        });
    }
}

