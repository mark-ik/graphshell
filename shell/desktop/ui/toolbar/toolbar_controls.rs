use crate::app::GraphBrowserApp;
use crate::graph::NodeKey;
use crate::shell::desktop::host::window::EmbedderWindow;
use egui::{WidgetInfo, WidgetType};
use std::collections::HashSet;

use crate::shell::desktop::ui::toolbar_routing::{self, ToolbarNavAction};

pub(super) fn render_workspace_pin_controls(
    ui: &mut egui::Ui,
    graph_app: &mut GraphBrowserApp,
    has_node_panes: bool,
    focused_pane_pin_name: Option<&str>,
    persisted_workspace_names: &HashSet<String>,
) {
    if !has_node_panes {
        return;
    }

    if let Some(pane_pin_name) = focused_pane_pin_name {
        let pane_is_pinned = persisted_workspace_names.contains(pane_pin_name);
        let pane_pin_label = if pane_is_pinned { "P-" } else { "P+" };
        let pane_pin_button = ui.add(super::toolbar_button(pane_pin_label)).on_hover_text(
            if pane_is_pinned {
                "Unpin focused pane workspace snapshot"
            } else {
                "Pin focused pane workspace snapshot"
            },
        );
        if pane_pin_button.clicked() {
            if pane_is_pinned {
                if let Err(e) = graph_app.delete_workspace_layout(pane_pin_name) {
                    log::warn!(
                        "Failed to unpin focused pane workspace '{pane_pin_name}': {e}"
                    );
                }
            } else {
                graph_app.request_save_workspace_snapshot_named(pane_pin_name.to_string());
            }
        }

        let pane_recall_button = ui
            .add_enabled(pane_is_pinned, super::toolbar_button("PR"))
            .on_hover_text("Recall focused pane pinned workspace");
        if pane_recall_button.clicked() {
            graph_app.request_restore_workspace_snapshot_named(pane_pin_name.to_string());
        }
    }

    let space_is_pinned = persisted_workspace_names.contains(super::WORKSPACE_PIN_NAME);
    let space_pin_label = if space_is_pinned { "W-" } else { "W+" };
    let space_pin_button = ui.add(super::toolbar_button(space_pin_label)).on_hover_text(
        if space_is_pinned {
            "Unpin current workspace snapshot"
        } else {
            "Pin current workspace snapshot"
        },
    );
    if space_pin_button.clicked() {
        if space_is_pinned {
            if let Err(e) = graph_app.delete_workspace_layout(super::WORKSPACE_PIN_NAME) {
                log::warn!(
                    "Failed to unpin workspace snapshot '{}': {e}",
                    super::WORKSPACE_PIN_NAME
                );
            }
        } else {
            graph_app.request_save_workspace_snapshot_named(super::WORKSPACE_PIN_NAME.to_string());
        }
    }

    let space_recall_button = ui
        .add_enabled(space_is_pinned, super::toolbar_button("WR"))
        .on_hover_text("Recall pinned workspace snapshot");
    if space_recall_button.clicked() {
        graph_app.request_restore_workspace_snapshot_named(super::WORKSPACE_PIN_NAME.to_string());
    }
}

pub(super) fn render_navigation_buttons(
    ui: &mut egui::Ui,
    graph_app: &mut GraphBrowserApp,
    window: &EmbedderWindow,
    focused_toolbar_node: Option<NodeKey>,
    can_go_back: bool,
    can_go_forward: bool,
    location_dirty: &mut bool,
) {
    let back_button = ui.add_enabled(can_go_back, super::toolbar_button("<"));
    back_button.widget_info(|| {
        let mut info = WidgetInfo::new(WidgetType::Button);
        info.label = Some("Back".into());
        info
    });
    if back_button.clicked() {
        *location_dirty = false;
        let _ = toolbar_routing::run_nav_action(
            graph_app,
            window,
            focused_toolbar_node,
            ToolbarNavAction::Back,
        );
    }

    let forward_button = ui.add_enabled(can_go_forward, super::toolbar_button(">"));
    forward_button.widget_info(|| {
        let mut info = WidgetInfo::new(WidgetType::Button);
        info.label = Some("Forward".into());
        info
    });
    if forward_button.clicked() {
        *location_dirty = false;
        let _ = toolbar_routing::run_nav_action(
            graph_app,
            window,
            focused_toolbar_node,
            ToolbarNavAction::Forward,
        );
    }

    let reload_button = ui.add(super::toolbar_button("R"));
    reload_button.widget_info(|| {
        let mut info = WidgetInfo::new(WidgetType::Button);
        info.label = Some("Reload".into());
        info
    });
    if reload_button.clicked() {
        *location_dirty = false;
        let _ = toolbar_routing::run_nav_action(
            graph_app,
            window,
            focused_toolbar_node,
            ToolbarNavAction::Reload,
        );
    }
}
