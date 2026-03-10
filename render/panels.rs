use std::env;
use std::str::FromStr;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use egui::{Ui, Window};
use uuid::Uuid;

use crate::app::{
    GraphBrowserApp, GraphIntent, HistoryCaptureStatus, HistoryManagerTab, KeyboardPanInputMode,
    ViewAction, WorkbenchIntent,
};
use crate::registries::domain::layout::canvas::CanvasLassoBinding;
use crate::shell::desktop::runtime::diagnostics::{DiagnosticEvent, emit_event};
use crate::shell::desktop::runtime::registries::{
    CHANNEL_UI_HISTORY_MANAGER_LIMIT, CHANNEL_UX_NAVIGATION_TRANSITION,
    phase2_describe_input_bindings,
};
use crate::shell::desktop::runtime::registries::input::{
    InputBinding, InputBindingSection, InputContext, action_id,
};
use crate::util::{GraphshellSettingsPath, VersoAddress};

use super::reducer_bridge::apply_reducer_graph_intents_hardened;

fn camera_settings_target_view_id(app: &GraphBrowserApp) -> Option<crate::app::GraphViewId> {
    if let Some(view_id) = app.workspace.focused_view {
        Some(view_id)
    } else if app.workspace.views.len() == 1 {
        app.workspace.views.keys().next().copied()
    } else {
        None
    }
}

fn selected_node_dynamics_profile_id(app: &GraphBrowserApp) -> String {
    let _ = app;
    crate::shell::desktop::runtime::registries::phase3_resolve_active_physics_profile()
        .resolved_id
}

fn apply_node_dynamics_profile_selection(app: &mut GraphBrowserApp, physics_id: &str) {
    apply_reducer_graph_intents_hardened(
        app,
        vec![GraphIntent::SetPhysicsProfile {
            profile_id: physics_id.to_string(),
        }],
    );
}

pub(crate) fn render_physics_settings_in_ui(ui: &mut Ui, app: &mut GraphBrowserApp) {
    ui.label("Node Dynamics");
    ui.small("Liquid/Gas/Solid control node motion behavior. They do not control camera policy.");

    let mut dynamics_id = selected_node_dynamics_profile_id(app);
    let previous_dynamics_id = dynamics_id.clone();
    ui.horizontal_wrapped(|ui| {
        ui.radio_value(
            &mut dynamics_id,
            crate::registries::atomic::lens::PHYSICS_ID_DEFAULT.to_string(),
            "Liquid",
        );
        ui.radio_value(
            &mut dynamics_id,
            crate::registries::atomic::lens::PHYSICS_ID_GAS.to_string(),
            "Gas",
        );
        ui.radio_value(
            &mut dynamics_id,
            crate::registries::atomic::lens::PHYSICS_ID_SOLID.to_string(),
            "Solid",
        );
    });
    if dynamics_id != previous_dynamics_id {
        apply_node_dynamics_profile_selection(app, &dynamics_id);
    }
    let dynamics_summary = match dynamics_id.as_str() {
        crate::registries::atomic::lens::PHYSICS_ID_DEFAULT => {
            "Liquid: motile clustering with bounded drift."
        }
        crate::registries::atomic::lens::PHYSICS_ID_GAS => {
            "Gas: stronger mutual repulsion and broader spread."
        }
        crate::registries::atomic::lens::PHYSICS_ID_SOLID => {
            "Solid: heavily damped movement that settles quickly."
        }
        _ => "Custom profile ID: fallback resolves through registry defaults.",
    };
    ui.small(dynamics_summary);

    ui.separator();
    ui.label("Physics Engine Settings");
    ui.small("Fruchterman-Reingold + center-gravity coefficients for the active simulation.");

    let mut config = app.workspace.physics.clone();
    let mut config_changed = false;

    ui.label("Repulsion (c_repulse):");
    if ui
        .add(egui::Slider::new(&mut config.base.c_repulse, 0.0..=10.0))
        .changed()
    {
        config_changed = true;
    }

    ui.label("Attraction (c_attract):");
    if ui
        .add(egui::Slider::new(&mut config.base.c_attract, 0.0..=10.0))
        .changed()
    {
        config_changed = true;
    }

    ui.label("Ideal Distance Scale (k_scale):");
    if ui
        .add(egui::Slider::new(&mut config.base.k_scale, 0.1..=5.0))
        .changed()
    {
        config_changed = true;
    }

    ui.label("Center Gravity:");
    if ui
        .add(egui::Slider::new(&mut config.extras.0.params.c, 0.0..=1.0))
        .changed()
    {
        config_changed = true;
    }

    ui.label("Max Step:");
    if ui
        .add(egui::Slider::new(&mut config.base.max_step, 0.1..=100.0))
        .changed()
    {
        config_changed = true;
    }

    ui.separator();
    ui.label("Damping & Convergence");
    ui.label("Damping:");
    if ui
        .add(egui::Slider::new(&mut config.base.damping, 0.01..=1.0))
        .changed()
    {
        config_changed = true;
    }

    ui.label("Time Step (dt):");
    if ui
        .add(egui::Slider::new(&mut config.base.dt, 0.001..=1.0).logarithmic(true))
        .changed()
    {
        config_changed = true;
    }

    ui.label("Epsilon:");
    if ui
        .add(egui::Slider::new(&mut config.base.epsilon, 1e-6..=0.1).logarithmic(true))
        .changed()
    {
        config_changed = true;
    }

    ui.horizontal(|ui| {
        if ui.button("Reset to Defaults").clicked() {
            let running = config.base.is_running;
            config = GraphBrowserApp::default_physics_state();
            config.base.is_running = running;
            config_changed = true;
        }

        ui.small(if app.workspace.physics.base.is_running {
            "Status: Running"
        } else {
            "Status: Paused"
        });
    });

    if let Some(last_avg) = app.workspace.physics.base.last_avg_displacement {
        ui.small(format!("Last avg displacement: {:.4}", last_avg));
    }
    ui.small(format!(
        "Step count: {}",
        app.workspace.physics.base.step_count
    ));

    ui.separator();
    render_camera_controls_settings_in_ui(ui, app);

    if config_changed {
        app.update_physics_config(config);
    }
}

fn render_camera_controls_settings_in_ui(ui: &mut Ui, app: &mut GraphBrowserApp) {
    ui.label("Camera Policy");
    ui.small(
        "Default camera behavior is manual: no auto-fit or auto-zoom until a fit lock is enabled.",
    );
    let position_fit_locked = app.camera_position_fit_locked();
    let zoom_fit_locked = app.camera_zoom_fit_locked();
    ui.small(format!(
        "Status: Position {} · Zoom {}",
        if position_fit_locked { "ON" } else { "OFF" },
        if zoom_fit_locked { "ON" } else { "OFF" }
    ));

    let mut position_fit_lock_enabled = position_fit_locked;
    if ui
        .checkbox(
            &mut position_fit_lock_enabled,
            "Lock camera position to graph fit",
        )
        .changed()
    {
        app.set_camera_position_fit_locked(position_fit_lock_enabled);
    }
    let mut zoom_fit_lock_enabled = zoom_fit_locked;
    if ui
        .checkbox(&mut zoom_fit_lock_enabled, "Lock camera zoom to graph fit")
        .changed()
    {
        app.set_camera_zoom_fit_locked(zoom_fit_lock_enabled);
    }
    ui.small("`C` toggles position lock. `Z` toggles zoom lock. Locks are per active graph view.");
    ui.small("Position lock blocks manual pan; zoom lock blocks manual zoom.");

    ui.horizontal(|ui| {
        ui.label("Keyboard pan speed");
        let mut pan_step = app.keyboard_pan_step();
        if ui
            .add(
                egui::Slider::new(&mut pan_step, 1.0..=80.0)
                    .step_by(1.0)
                    .suffix(" px"),
            )
            .changed()
        {
            app.set_keyboard_pan_step(pan_step);
        }
    });

    ui.horizontal(|ui| {
        ui.label("Keyboard pan keys");
        let mut mode = app.keyboard_pan_input_mode();
        ui.radio_value(
            &mut mode,
            KeyboardPanInputMode::WasdAndArrows,
            "WASD + Arrows",
        );
        ui.radio_value(&mut mode, KeyboardPanInputMode::ArrowsOnly, "Arrows only");
        if mode != app.keyboard_pan_input_mode() {
            app.set_keyboard_pan_input_mode(mode);
        }
    });

    ui.separator();
    ui.label("Pan Inertia");
    let mut inertia_enabled = app.camera_pan_inertia_enabled();
    if ui
        .checkbox(
            &mut inertia_enabled,
            "Enable slight camera inertia after pan input",
        )
        .changed()
    {
        app.set_camera_pan_inertia_enabled(inertia_enabled);
    }
    if inertia_enabled {
        ui.horizontal(|ui| {
            ui.label("Inertia damping");
            let mut damping = app.camera_pan_inertia_damping();
            if ui
                .add(egui::Slider::new(&mut damping, 0.70..=0.99).fixed_decimals(2))
                .changed()
            {
                app.set_camera_pan_inertia_damping(damping);
            }
        });
    }
}

pub fn render_help_panel(ctx: &egui::Context, app: &mut GraphBrowserApp) {
    let was_open = app.workspace.show_help_panel;
    if !was_open {
        return;
    }

    let mut open = app.workspace.show_help_panel;
    Window::new("Keyboard Shortcuts")
        .open(&mut open)
        .default_width(350.0)
        .default_height(420.0)
        .resizable(true)
        .show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    egui::Grid::new("shortcut_grid")
                        .num_columns(2)
                        .spacing([20.0, 6.0])
                        .show(ui, |ui| {
                            let lasso_binding = app.lasso_binding_preference();
                            let lasso_base = match lasso_binding {
                                CanvasLassoBinding::RightDrag => "Right+Drag",
                                CanvasLassoBinding::ShiftLeftDrag => "Shift+LeftDrag",
                            };
                            let lasso_add = match lasso_binding {
                                CanvasLassoBinding::RightDrag => "Right+Shift/Ctrl+Drag",
                                CanvasLassoBinding::ShiftLeftDrag => "Shift+Ctrl+LeftDrag",
                            };
                            let lasso_toggle = match lasso_binding {
                                CanvasLassoBinding::RightDrag => "Right+Alt+Drag",
                                CanvasLassoBinding::ShiftLeftDrag => "Shift+Alt+LeftDrag",
                            };
                            let command_palette_key = crate::shell::desktop::runtime::registries::phase2_binding_display_labels_for_action(
                                action_id::graph::COMMAND_PALETTE_OPEN,
                            ).join(" / ");
                            let radial_key = crate::shell::desktop::runtime::registries::phase2_binding_display_labels_for_action(
                                action_id::graph::RADIAL_MENU_OPEN,
                            ).join(" / ");
                            let help_key = crate::shell::desktop::runtime::registries::phase2_binding_display_labels_for_action(
                                action_id::workbench::HELP_OPEN,
                            ).join(" / ");
                            let shortcuts = [
                                ("Home / Esc", "Toggle Graph / Detail view"),
                                ("N", "Create new node"),
                                ("Delete", "Remove selected nodes"),
                                ("Ctrl+Shift+Delete", "Clear entire graph"),
                                ("T", "Toggle physics simulation"),
                                ("R", "Reheat physics simulation"),
                                ("+ / - / 0", "Zoom in / out / reset"),
                                ("C", "Toggle camera position-fit lock"),
                                ("Z", "Toggle camera zoom-fit lock"),
                                ("P", "Physics settings panel"),
                                ("Ctrl+H", "History Manager panel"),
                                ("Ctrl+F", "Show graph search"),
                                (&command_palette_key, "Toggle command palette"),
                                (&radial_key, "Toggle radial palette mode"),
                                ("Ctrl+Z / Ctrl+Y", "Undo / Redo"),
                                ("G", "Connect selected pair"),
                                ("Shift+G", "Connect both directions"),
                                ("Alt+G", "Remove user edge"),
                                ("I / U", "Pin / Unpin selected node(s)"),
                                ("L", "Toggle pin on primary selected node"),
                                (lasso_base, "Lasso select (replace)"),
                                (lasso_add, "Lasso add to selection"),
                                (lasso_toggle, "Lasso toggle selection"),
                                ("Search Up/Down", "Cycle graph matches"),
                                ("Search Enter", "Select active search match"),
                                (&help_key, "This help panel"),
                                ("Ctrl+L / Alt+D", "Focus address bar"),
                                ("Double-click node", "Open node via workspace routing"),
                                ("Drag tab out", "Detach tab into split pane"),
                                ("Shift + Double-click node", "Fallback split-open gesture"),
                                ("Click + drag", "Move a node"),
                                ("Scroll wheel", "Zoom in / out"),
                            ];

                            for (key, desc) in shortcuts {
                                ui.strong(key);
                                ui.label(desc);
                                ui.end_row();
                            }
                        });
                });
        });
    app.workspace.show_help_panel = open;
    if app.workspace.show_help_panel != was_open {
        emit_event(DiagnosticEvent::MessageReceived {
            channel_id: CHANNEL_UX_NAVIGATION_TRANSITION,
            latency_us: 0,
        });
    }
}

pub fn render_history_manager_in_ui(ui: &mut Ui, app: &mut GraphBrowserApp) -> Vec<GraphIntent> {
    let mut intents = Vec::new();
    let (timeline_total, dissolved_total) = app.history_manager_archive_counts();
    let health = app.history_health_summary();
    let auto_curate_keep = history_manager_auto_curate_keep_latest();

    ui.horizontal(|ui| {
        if ui.button("Settings").clicked() {
            app.enqueue_workbench_intent(WorkbenchIntent::OpenSettingsUrl {
                url: VersoAddress::settings(GraphshellSettingsPath::General).to_string(),
            });
        }
        if ui.button("Done").clicked() {
            app.enqueue_workbench_intent(WorkbenchIntent::CloseToolPane {
                kind: crate::shell::desktop::workbench::pane_model::ToolPaneState::HistoryManager,
                restore_previous_focus: true,
            });
        }
    });
    ui.add_space(4.0);

    ui.horizontal(|ui| {
        ui.selectable_value(
            &mut app.workspace.history_manager_tab,
            HistoryManagerTab::Timeline,
            "Timeline",
        );
        ui.selectable_value(
            &mut app.workspace.history_manager_tab,
            HistoryManagerTab::Dissolved,
            "Dissolved",
        );
    });
    ui.add_space(8.0);

    let capture_label = match health.capture_status {
        HistoryCaptureStatus::Full => "active",
        HistoryCaptureStatus::DegradedCaptureOnly => "degraded",
    };
    let preview_label = if health.preview_mode_active {
        "active"
    } else {
        "off"
    };
    let last_violation = if health.last_preview_isolation_violation {
        "yes"
    } else {
        "none"
    };
    let last_event_label = if let Some(last_ms) = health.last_event_unix_ms {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(last_ms);
        let elapsed_ms = now_ms.saturating_sub(last_ms);
        if elapsed_ms < 1_000 {
            "just now".to_string()
        } else if elapsed_ms < 60_000 {
            format!("{}s ago", elapsed_ms / 1_000)
        } else if elapsed_ms < 3_600_000 {
            format!("{}m ago", elapsed_ms / 60_000)
        } else if elapsed_ms < 86_400_000 {
            format!("{}h ago", elapsed_ms / 3_600_000)
        } else {
            format!("{}d ago", elapsed_ms / 86_400_000)
        }
    } else {
        "none".to_string()
    };
    let reason_bucket = health
        .recent_failure_reason_bucket
        .as_deref()
        .unwrap_or("none");
    let last_error = health.last_error.as_deref().unwrap_or("none");
    let return_to_present = health
        .last_return_to_present_result
        .as_deref()
        .unwrap_or("none");
    let replay_label = if health.replay_in_progress {
        format!(
            "{}/{}",
            health
                .replay_cursor
                .map(|v| v.to_string())
                .unwrap_or_else(|| "?".to_string()),
            health
                .replay_total_steps
                .map(|v| v.to_string())
                .unwrap_or_else(|| "?".to_string())
        )
    } else {
        "idle".to_string()
    };

    ui.label(
        egui::RichText::new(format!(
            "Health: capture={capture_label} | failures={} | reason={reason_bucket} | archive=({}/{}) | preview={preview_label} | replay={replay_label} | last isolation violation={last_violation} | last return-to-present={return_to_present} | last event={last_event_label} | last error={last_error}",
            health.recent_traversal_append_failures,
            health.traversal_archive_count,
            health.dissolved_archive_count
        ))
        .small(),
    );
    ui.add_space(6.0);

    match app.workspace.history_manager_tab {
        HistoryManagerTab::Timeline => {
            ui.horizontal(|ui| {
                ui.label(format!("Archived traversal entries: {timeline_total}"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Export").clicked() {
                        intents.push(crate::app::RuntimeEvent::ExportHistoryTimeline.into());
                    }
                    if ui.button("Clear").clicked() {
                        intents.push(crate::app::RuntimeEvent::ClearHistoryTimeline.into());
                    }
                    if ui.button("Auto-Curate").clicked() {
                        intents.push(GraphIntent::AutoCurateHistoryTimeline {
                            keep_latest: auto_curate_keep,
                        });
                    }
                });
            });
            ui.small(format!(
                "Auto-curation keeps latest {} timeline entries.",
                auto_curate_keep
            ));
            if health.preview_mode_active {
                ui.horizontal_wrapped(|ui| {
                    ui.small("Replay controls:");
                    if ui.button("Reset").clicked() {
                        intents.push(GraphIntent::HistoryTimelineReplaySetTotal {
                            total_steps: timeline_total,
                        });
                        intents.push(GraphIntent::HistoryTimelineReplayReset);
                    }
                    if ui.button("+1").clicked() {
                        intents.push(GraphIntent::HistoryTimelineReplaySetTotal {
                            total_steps: timeline_total,
                        });
                        intents.push(GraphIntent::HistoryTimelineReplayAdvance { steps: 1 });
                    }
                    if ui.button("+10").clicked() {
                        intents.push(GraphIntent::HistoryTimelineReplaySetTotal {
                            total_steps: timeline_total,
                        });
                        intents.push(GraphIntent::HistoryTimelineReplayAdvance { steps: 10 });
                    }
                    if ui.button("Finish").clicked() {
                        intents.push(GraphIntent::HistoryTimelineReplaySetTotal {
                            total_steps: timeline_total,
                        });
                        intents.push(GraphIntent::HistoryTimelineReplayAdvance {
                            steps: timeline_total.max(1),
                        });
                    }
                });
            }
            let entries = app.history_manager_timeline_entries(history_manager_entry_limit());
            render_history_manager_rows(ui, app, &entries, &mut intents);
        }
        HistoryManagerTab::Dissolved => {
            ui.horizontal(|ui| {
                ui.label(format!("Archived dissolved entries: {dissolved_total}"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Export").clicked() {
                        intents.push(crate::app::RuntimeEvent::ExportHistoryDissolved.into());
                    }
                    if ui.button("Clear").clicked() {
                        intents.push(crate::app::RuntimeEvent::ClearHistoryDissolved.into());
                    }
                    if ui.button("Auto-Curate").clicked() {
                        intents.push(GraphIntent::AutoCurateHistoryDissolved {
                            keep_latest: auto_curate_keep,
                        });
                    }
                });
            });
            ui.small(format!(
                "Auto-curation keeps latest {} dissolved entries.",
                auto_curate_keep
            ));
            let entries = app.history_manager_dissolved_entries(history_manager_entry_limit());
            render_history_manager_rows(ui, app, &entries, &mut intents);
        }
    }

    intents
}

fn history_manager_entry_limit() -> usize {
    static LIMIT: OnceLock<usize> = OnceLock::new();
    *LIMIT.get_or_init(|| {
        if let Ok(value) = env::var("GRAPHSHELL_HISTORY_MANAGER_LIMIT") {
            let trimmed = value.trim();
            if let Ok(parsed) = trimmed.parse::<usize>()
                && parsed > 0
            {
                emit_event(DiagnosticEvent::MessageSent {
                    channel_id: CHANNEL_UI_HISTORY_MANAGER_LIMIT,
                    byte_len: trimmed.len(),
                });
                return parsed;
            }
        }
        250
    })
}

fn history_manager_auto_curate_keep_latest() -> usize {
    const DEFAULT_KEEP_LATEST: usize = 5_000;
    static KEEP_LATEST: OnceLock<usize> = OnceLock::new();

    *KEEP_LATEST.get_or_init(|| {
        if let Ok(value) = env::var("GRAPHSHELL_HISTORY_ARCHIVE_KEEP_LATEST") {
            value
                .parse::<usize>()
                .ok()
                .filter(|v| *v > 0)
                .unwrap_or(DEFAULT_KEEP_LATEST)
        } else {
            DEFAULT_KEEP_LATEST
        }
    })
}

#[cfg(test)]
pub(crate) fn history_manager_entry_limit_for_tests() -> usize {
    history_manager_entry_limit()
}

fn render_history_manager_rows(
    ui: &mut Ui,
    app: &GraphBrowserApp,
    entries: &[crate::services::persistence::types::LogEntry],
    intents: &mut Vec<GraphIntent>,
) {
    if entries.is_empty() {
        ui.label("No history entries yet.");
        return;
    }

    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        for entry in entries {
            let crate::services::persistence::types::LogEntry::AppendTraversal {
                from_node_id,
                to_node_id,
                timestamp_ms,
                trigger,
            } = entry
            else {
                continue;
            };

            let from_key = Uuid::parse_str(from_node_id)
                .ok()
                .and_then(|id| app.workspace.domain.graph.get_node_key_by_id(id));
            let to_key = Uuid::parse_str(to_node_id)
                .ok()
                .and_then(|id| app.workspace.domain.graph.get_node_key_by_id(id));

            let from_label = from_key
                .and_then(|k| app.workspace.domain.graph.get_node(k))
                .map(|n| if n.title.is_empty() { n.url.as_str() } else { n.title.as_str() })
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("<missing:{}>", &from_node_id[..from_node_id.len().min(8)]));
            let to_label = to_key
                .and_then(|k| app.workspace.domain.graph.get_node(k))
                .map(|n| if n.title.is_empty() { n.url.as_str() } else { n.title.as_str() })
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("<missing:{}>", &to_node_id[..to_node_id.len().min(8)]));

            let elapsed_ms = now_ms.saturating_sub(*timestamp_ms);
            let time_label = if elapsed_ms < 1_000 {
                "just now".to_string()
            } else if elapsed_ms < 60_000 {
                format!("{}s ago", elapsed_ms / 1_000)
            } else if elapsed_ms < 3_600_000 {
                format!("{}m ago", elapsed_ms / 60_000)
            } else if elapsed_ms < 86_400_000 {
                format!("{}h ago", elapsed_ms / 3_600_000)
            } else {
                format!("{}d ago", elapsed_ms / 86_400_000)
            };

            let trigger_label = match trigger {
                crate::services::persistence::types::PersistedNavigationTrigger::LinkClick => "🔗 Link",
                crate::services::persistence::types::PersistedNavigationTrigger::Back => "⬅ Back",
                crate::services::persistence::types::PersistedNavigationTrigger::Forward => "➡ Forward",
                crate::services::persistence::types::PersistedNavigationTrigger::AddressBarEntry => "⌨ Address",
                crate::services::persistence::types::PersistedNavigationTrigger::PanePromotion => "⬆ Promote",
                crate::services::persistence::types::PersistedNavigationTrigger::Programmatic => "⚙ Programmatic",
                crate::services::persistence::types::PersistedNavigationTrigger::Unknown => "↔",
            };

            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(time_label).weak().small());
                ui.label(trigger_label);
                let response = ui.selectable_label(false, format!("{} → {}", from_label, to_label));
                if response.clicked() && let Some(key) = from_key {
                    intents.push(GraphIntent::SelectNode {
                        key,
                        multi_select: false,
                    });
                    intents.push(GraphIntent::RequestZoomToSelected);
                }
            });
            ui.add_space(2.0);
        }
    });
}

pub fn render_file_tree_tool_pane_in_ui(
    ui: &mut Ui,
    app: &mut GraphBrowserApp,
) -> Vec<GraphIntent> {
    fn file_tree_row_label(row_key: &str) -> String {
        if let Some(rest) = row_key.strip_prefix("fs:") {
            let path = rest.split('#').next().unwrap_or(rest);
            let name = path.rsplit('/').next().unwrap_or(path);
            if !name.is_empty() && name != path {
                return format!("{name} ({path})");
            }
            return path.to_string();
        }

        if let Some(rest) = row_key.strip_prefix("node:") {
            return format!("Node {}", &rest.chars().take(8).collect::<String>());
        }

        if let Some(rest) = row_key.strip_prefix("view:") {
            return format!("Saved View {}", &rest.chars().take(8).collect::<String>());
        }

        row_key.to_string()
    }

    let mut intents = Vec::new();
    ui.heading("File Tree");
    ui.separator();

    ui.horizontal(|ui| {
        if ui.button("Done").clicked() {
            app.enqueue_workbench_intent(WorkbenchIntent::CloseToolPane {
                kind: crate::shell::desktop::workbench::pane_model::ToolPaneState::FileTree,
                restore_previous_focus: true,
            });
        }
        if ui.button("Refresh").clicked() {
            intents.push(ViewAction::RebuildFileTreeProjection.into());
        }
    });
    ui.add_space(4.0);

    ui.label("Graph-owned hierarchical projection (pane-hosted surface).");

    let mut relation_source = app.file_tree_projection_state().containment_relation_source;
    ui.horizontal(|ui| {
        ui.label("Containment source:");
        ui.selectable_value(
            &mut relation_source,
            crate::app::FileTreeContainmentRelationSource::GraphContainment,
            "Graph",
        );
        ui.selectable_value(
            &mut relation_source,
            crate::app::FileTreeContainmentRelationSource::SavedViewCollections,
            "Saved Views",
        );
        ui.selectable_value(
            &mut relation_source,
            crate::app::FileTreeContainmentRelationSource::ImportedFilesystemProjection,
            "Imported FS",
        );
    });
    if relation_source != app.file_tree_projection_state().containment_relation_source {
        intents.push(
            ViewAction::SetFileTreeContainmentRelationSource {
                source: relation_source,
            }
            .into(),
        );
        intents.push(ViewAction::RebuildFileTreeProjection.into());
    }

    let mut sort_mode = app.file_tree_projection_state().sort_mode;
    ui.horizontal(|ui| {
        ui.label("Sort:");
        ui.selectable_value(
            &mut sort_mode,
            crate::app::FileTreeSortMode::Manual,
            "Manual",
        );
        ui.selectable_value(
            &mut sort_mode,
            crate::app::FileTreeSortMode::NameAscending,
            "Name ↑",
        );
        ui.selectable_value(
            &mut sort_mode,
            crate::app::FileTreeSortMode::NameDescending,
            "Name ↓",
        );
    });
    if sort_mode != app.file_tree_projection_state().sort_mode {
        intents.push(ViewAction::SetFileTreeSortMode { sort_mode }.into());
    }

    ui.horizontal(|ui| {
        ui.label("Root filter:");
        let mut root_filter = app
            .file_tree_projection_state()
            .root_filter
            .clone()
            .unwrap_or_default();
        if ui
            .add(
                egui::TextEdit::singleline(&mut root_filter)
                    .desired_width(240.0)
                    .hint_text("optional projection root"),
            )
            .changed()
        {
            let trimmed = root_filter.trim().to_string();
            if trimmed.is_empty() {
                intents.push(ViewAction::SetFileTreeRootFilter { root_filter: None }.into());
            } else {
                intents.push(
                    ViewAction::SetFileTreeRootFilter {
                        root_filter: Some(trimmed),
                    }
                    .into(),
                );
            }
        }
    });

    ui.separator();
    ui.label(format!(
        "Rows: {} mapped, {} selected, {} expanded",
        app.file_tree_projection_state().row_targets.len(),
        app.file_tree_projection_state().selected_rows.len(),
        app.file_tree_projection_state().expanded_rows.len(),
    ));

    let mut row_targets: Vec<(String, crate::app::FileTreeProjectionTarget)> = app
        .file_tree_projection_state()
        .row_targets
        .iter()
        .map(|(row_key, target)| (row_key.clone(), *target))
        .collect();
    row_targets.sort_by(|(left, _), (right, _)| left.cmp(right));

    if row_targets.is_empty() {
        ui.small("No mapped rows yet.");
    } else {
        let selected_rows_current = app.file_tree_projection_state().selected_rows.clone();
        let mut expanded_rows_next = app.file_tree_projection_state().expanded_rows.clone();

        egui::ScrollArea::vertical()
            .max_height(180.0)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for (row_key, _) in &row_targets {
                    ui.horizontal(|ui| {
                        let is_expanded = expanded_rows_next.contains(row_key);
                        if ui
                            .small_button(if is_expanded { "▾" } else { "▸" })
                            .clicked()
                        {
                            if is_expanded {
                                expanded_rows_next.remove(row_key);
                            } else {
                                expanded_rows_next.insert(row_key.clone());
                            }
                        }

                        let is_selected = selected_rows_current.contains(row_key);
                        let response =
                            ui.selectable_label(is_selected, file_tree_row_label(row_key));
                        if response.clicked() {
                            intents.push(
                                ViewAction::SetFileTreeSelectedRows {
                                    rows: vec![row_key.clone()],
                                }
                                .into(),
                            );
                        }
                        response.on_hover_text(row_key);
                    });
                }
            });

        if expanded_rows_next != app.file_tree_projection_state().expanded_rows {
            let mut expanded_rows: Vec<String> = expanded_rows_next.into_iter().collect();
            expanded_rows.sort();
            intents.push(
                ViewAction::SetFileTreeExpandedRows {
                    rows: expanded_rows,
                }
                .into(),
            );
        }

        let selected_row = app
            .file_tree_projection_state()
            .selected_rows
            .iter()
            .next()
            .cloned();
        let selected_target = selected_row.as_ref().and_then(|row| {
            app.file_tree_projection_state()
                .row_targets
                .get(row)
                .copied()
        });
        if let Some(selected_row) = selected_row
            && let Some(target) = selected_target
        {
            ui.horizontal(|ui| {
                ui.label(format!("Selected: {selected_row}"));
                if ui.button("Open Target").clicked() {
                    match target {
                        crate::app::FileTreeProjectionTarget::Node(node_key) => {
                            intents.push(GraphIntent::OpenNodeFrameRouted {
                                key: node_key,
                                prefer_frame: None,
                            });
                        }
                        crate::app::FileTreeProjectionTarget::SavedView(view_id) => {
                            app.enqueue_workbench_intent(WorkbenchIntent::OpenViewUrl {
                                url: VersoAddress::view(view_id.as_uuid().to_string()).to_string(),
                            });
                        }
                    }
                }
            });
        }
    }

    intents
}

pub fn render_settings_tool_pane_in_ui_with_control_panel(
    ui: &mut Ui,
    app: &mut GraphBrowserApp,
    mut control_panel: Option<&mut crate::shell::desktop::runtime::control_panel::ControlPanel>,
) -> Vec<GraphIntent> {
    let intents: Vec<GraphIntent> = Vec::new();
    ui.heading("Settings");
    ui.separator();

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui.button("History").clicked() {
                    app.enqueue_workbench_intent(WorkbenchIntent::OpenSettingsUrl {
                        url: VersoAddress::settings(GraphshellSettingsPath::History).to_string(),
                    });
                }
                if ui.button("Done").clicked() {
                    app.enqueue_workbench_intent(WorkbenchIntent::CloseToolPane {
                        kind: crate::shell::desktop::workbench::pane_model::ToolPaneState::Settings,
                        restore_previous_focus: true,
                    });
                }
            });
            ui.add_space(4.0);

            ui.horizontal_wrapped(|ui| {
                ui.label("Category:");
                ui.selectable_value(
                    &mut app.workspace.settings_tool_page,
                    crate::app::SettingsToolPage::General,
                    "General",
                );
                ui.selectable_value(
                    &mut app.workspace.settings_tool_page,
                    crate::app::SettingsToolPage::Persistence,
                    "Persistence",
                );
                ui.selectable_value(
                    &mut app.workspace.settings_tool_page,
                    crate::app::SettingsToolPage::Physics,
                    "Physics",
                );
                ui.selectable_value(
                    &mut app.workspace.settings_tool_page,
                    crate::app::SettingsToolPage::Sync,
                    "Sync",
                );
                ui.selectable_value(
                    &mut app.workspace.settings_tool_page,
                    crate::app::SettingsToolPage::Appearance,
                    "Appearance",
                );
                ui.selectable_value(
                    &mut app.workspace.settings_tool_page,
                    crate::app::SettingsToolPage::Keybindings,
                    "Keybindings",
                );
            });
            ui.separator();

            match app.workspace.settings_tool_page {
                crate::app::SettingsToolPage::General => {
                    ui.label("Settings are page-backed app surfaces in this pane.");
                    ui.label(
                        "Use categories to edit persistence, physics, sync, and appearance.",
                    );
                    ui.add_space(8.0);
                    if ui.button("Open History Surface").clicked() {
                        app.enqueue_workbench_intent(WorkbenchIntent::OpenSettingsUrl {
                            url: VersoAddress::settings(GraphshellSettingsPath::History).to_string(),
                        });
                    }
                }
                crate::app::SettingsToolPage::Persistence => {
                    ui.label("Storage");
                    ui.horizontal(|ui| {
                        ui.label("Data directory:");
                        let data_dir_input_id = ui.make_persistent_id("settings_tool_data_dir_input");
                        let mut data_dir_input = ui
                            .data_mut(|d| d.get_persisted::<String>(data_dir_input_id))
                            .unwrap_or_default();
                        if ui
                            .add(
                                egui::TextEdit::singleline(&mut data_dir_input)
                                    .desired_width(220.0)
                                    .hint_text("C:\\path\\to\\graph_data"),
                            )
                            .changed()
                        {
                            ui.data_mut(|d| {
                                d.insert_persisted(data_dir_input_id, data_dir_input.clone())
                            });
                        }
                        if ui.button("Switch").clicked() {
                            let trimmed = data_dir_input.trim();
                            if !trimmed.is_empty() {
                                app.request_switch_data_dir(trimmed);
                            }
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label("Snapshot interval (sec):");
                        let interval_input_id =
                            ui.make_persistent_id("settings_tool_snapshot_interval_input");
                        let mut interval_input = ui
                            .data_mut(|d| d.get_persisted::<String>(interval_input_id))
                            .unwrap_or_else(|| {
                                app.snapshot_interval_secs()
                                    .unwrap_or(
                                        crate::services::persistence::DEFAULT_SNAPSHOT_INTERVAL_SECS,
                                    )
                                    .to_string()
                            });
                        if ui
                            .add(egui::TextEdit::singleline(&mut interval_input).desired_width(80.0))
                            .changed()
                        {
                            ui.data_mut(|d| {
                                d.insert_persisted(interval_input_id, interval_input.clone())
                            });
                        }
                        if ui.button("Apply").clicked()
                            && let Ok(secs) = interval_input.trim().parse::<u64>()
                        {
                            let _ = app.set_snapshot_interval_secs(secs);
                        }
                    });

                    ui.separator();
                    ui.label("Frames");
                    if ui.button("Save Current Frame").clicked() {
                        let now = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .map(|d| d.as_secs())
                            .unwrap_or(0);
                        app.request_save_frame_snapshot_named(format!("workspace:toolpane-{now}"));
                    }
                    if ui.button("Prune Empty Named Frames").clicked() {
                        app.request_prune_empty_frames();
                    }

                    ui.separator();
                    ui.label("Graphs");
                    if ui.button("Save Graph Snapshot").clicked() {
                        let now = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .map(|d| d.as_secs())
                            .unwrap_or(0);
                        app.request_save_graph_snapshot_named(format!("toolpane-graph-{now}"));
                    }
                    if ui.button("Restore Latest Graph").clicked() {
                        app.request_restore_graph_snapshot_latest();
                    }
                }
                crate::app::SettingsToolPage::Physics => {
                    ui.label("Physics");
                    render_physics_settings_in_ui(ui, app);
                }
                crate::app::SettingsToolPage::Sync => {
                    ui.label("Sync");
                    if let Some(control_panel) = control_panel.as_mut() {
                        render_sync_settings_in_ui(ui, app, control_panel);
                    } else {
                        ui.small("Sync controls unavailable in this surface.");
                    }
                }
                crate::app::SettingsToolPage::Appearance => {
                    ui.label("Theme Mode");
                    let current_dark = matches!(
                        app.default_registry_theme_id(),
                        Some(crate::shell::desktop::runtime::registries::theme::THEME_ID_DARK)
                    );
                    let mut dark_mode = current_dark;
                    ui.horizontal(|ui| {
                        ui.radio_value(&mut dark_mode, false, "Light");
                        ui.radio_value(&mut dark_mode, true, "Dark");
                    });
                    if dark_mode != current_dark {
                        if dark_mode {
                            app.set_default_registry_theme_id(Some(
                                crate::shell::desktop::runtime::registries::theme::THEME_ID_DARK,
                            ));
                        } else {
                            app.set_default_registry_theme_id(Some(
                                crate::shell::desktop::runtime::registries::theme::THEME_ID_LIGHT,
                            ));
                        }
                    }
                    ui.small("Theme mode is persisted through the workspace settings model.");

                    ui.separator();
                    ui.label("Graph Input");
                    ui.horizontal(|ui| {
                        ui.label("Lasso binding");
                        let mut binding = app.lasso_binding_preference();
                        ui.radio_value(&mut binding, CanvasLassoBinding::RightDrag, "Right Drag");
                        ui.radio_value(
                            &mut binding,
                            CanvasLassoBinding::ShiftLeftDrag,
                            "Shift + Left Drag",
                        );
                        if binding != app.lasso_binding_preference() {
                            app.set_lasso_binding_preference(binding);
                        }
                    });
                    ui.small("Press F9 to jump directly to Camera Controls in Physics settings.");
                }
                crate::app::SettingsToolPage::Keybindings => {
                    render_keybindings_settings_in_ui(ui, app);
                }
            }
        });

    intents
}

fn render_keybindings_settings_in_ui(ui: &mut Ui, app: &mut GraphBrowserApp) {
    let capture_action_id = ui.make_persistent_id("settings_keybindings_capture_action");
    let capture_context_id = ui.make_persistent_id("settings_keybindings_capture_context");
    let capture_error_id = ui.make_persistent_id("settings_keybindings_capture_error");

    let mut capture_action = ui
        .ctx()
        .data_mut(|data| data.get_persisted::<String>(capture_action_id));
    let mut capture_context = ui
        .ctx()
        .data_mut(|data| data.get_persisted::<String>(capture_context_id));
    let mut capture_error = ui
        .ctx()
        .data_mut(|data| data.get_persisted::<String>(capture_error_id));

    if let (Some(action_id), Some(context_raw)) = (capture_action.as_deref(), capture_context.as_deref())
        && let Ok(context) = InputContext::from_str(context_raw)
    {
        let captured = ui.ctx().input(|input| {
            input.events.iter().find_map(|event| match event {
                egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } => Some((*key, *modifiers)),
                _ => None,
            })
        });

        if let Some((key, modifiers)) = captured {
            if key == egui::Key::Escape {
                capture_action = None;
                capture_context = None;
                capture_error = None;
            } else if let Some(binding) = InputBinding::from_egui_key(key, &modifiers) {
                match app.set_input_binding_for_action(action_id, context, binding) {
                    Ok(()) => {
                        capture_action = None;
                        capture_context = None;
                        capture_error = None;
                    }
                    Err(error) => {
                        capture_error = Some(format!("{error:?}"));
                    }
                }
            }
        }
    }

    ui.label("Keyboard shortcuts are registry-backed and persisted per workspace.");
    ui.small("Click Rebind, then press a key or chord. Press Esc to cancel capture.");
    ui.add_space(8.0);

    if ui.button("Reset All To Defaults").clicked() {
        if let Err(error) = app.set_input_binding_remaps(&[]) {
            capture_error = Some(format!("{error:?}"));
        } else {
            capture_error = None;
        }
    }

    if let Some(error) = capture_error.as_deref() {
        ui.colored_label(egui::Color32::from_rgb(180, 60, 60), error);
    }

    let descriptors = phase2_describe_input_bindings();
    for section in [
        InputBindingSection::Graph,
        InputBindingSection::Workbench,
        InputBindingSection::Navigation,
    ] {
        let entries = descriptors
            .iter()
            .filter(|entry| entry.section == section)
            .collect::<Vec<_>>();
        if entries.is_empty() {
            continue;
        }
        ui.separator();
        ui.heading(section.label());
        egui::Grid::new(format!("keybinding_grid_{}", section.label()))
            .num_columns(5)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                ui.strong("Action");
                ui.strong("Current");
                ui.strong("Default");
                ui.strong("Context");
                ui.strong("Edit");
                ui.end_row();

                for entry in entries {
                    let is_capturing = capture_action.as_deref() == Some(entry.action_id.as_str())
                        && capture_context.as_deref() == Some(entry.context.label());
                    ui.label(entry.display_name);
                    ui.label(
                        entry
                            .current_binding
                            .as_ref()
                            .map(InputBinding::display_label)
                            .unwrap_or_else(|| "Unbound".to_string()),
                    );
                    ui.label(
                        entry
                            .default_binding
                            .as_ref()
                            .map(InputBinding::display_label)
                            .unwrap_or_else(|| "None".to_string()),
                    );
                    ui.small(entry.context.label());
                    ui.horizontal(|ui| {
                        if ui
                            .button(if is_capturing { "Press Key..." } else { "Rebind" })
                            .clicked()
                        {
                            capture_action = Some(entry.action_id.clone());
                            capture_context = Some(entry.context.label().to_string());
                            capture_error = None;
                        }
                        if ui.button("Reset").clicked() {
                            app.reset_input_binding_for_action(&entry.action_id, entry.context);
                            capture_error = None;
                        }
                    });
                    ui.end_row();
                }
            });
    }

    ui.ctx().data_mut(|data| {
        if let Some(value) = capture_action {
            data.insert_persisted(capture_action_id, value);
        } else {
            data.remove::<String>(capture_action_id);
        }
        if let Some(value) = capture_context {
            data.insert_persisted(capture_context_id, value);
        } else {
            data.remove::<String>(capture_context_id);
        }
        if let Some(value) = capture_error {
            data.insert_persisted(capture_error_id, value);
        } else {
            data.remove::<String>(capture_error_id);
        }
    });
}

pub fn render_sync_settings_in_ui(
    ui: &mut Ui,
    app: &mut GraphBrowserApp,
    control_panel: &mut crate::shell::desktop::runtime::control_panel::ControlPanel,
) {
    let ctx = ui.ctx().clone();
    let pairing_code_id = egui::Id::new("verse_pairing_code");
    let pairing_code_input_id = egui::Id::new("verse_pairing_code_input");
    let discovery_results_id = egui::Id::new("verse_discovery_results");
    let sync_status_id = egui::Id::new("verse_sync_status");

    if let Some(discovery_result) = control_panel.take_discovery_results() {
        match discovery_result {
            Ok(peers) => {
                let discovered_count = peers.len();
                ctx.data_mut(|d| {
                    d.insert_temp(discovery_results_id, peers);
                    d.insert_temp(
                        sync_status_id,
                        format!("Discovery complete: {discovered_count} peer(s) found"),
                    );
                });
            }
            Err(error) => {
                ctx.data_mut(|d| {
                    d.insert_temp(sync_status_id, format!("Discovery failed: {error}"))
                });
            }
        }
    }

    let verse_initialized = crate::mods::native::verse::is_initialized();
    ui.label(egui::RichText::new("Trusted Devices").strong());
    ui.separator();

    if !verse_initialized {
        ui.label("Verse is initializing. Device list will appear shortly.");
    } else {
        ui.horizontal(|ui| {
            if ui.button("Show Pairing Code").clicked() {
                match crate::mods::native::verse::generate_pairing_code() {
                    Ok(code) => {
                        ctx.data_mut(|d| d.insert_temp(pairing_code_id, code));
                    }
                    Err(error) => {
                        ctx.data_mut(|d| {
                            d.insert_temp(
                                sync_status_id,
                                format!("Pairing code unavailable: {error}"),
                            )
                        });
                    }
                }
            }
            if ui.button("Discover Nearby").clicked() {
                match control_panel.request_discover_nearby_peers(2) {
                    Ok(()) => {
                        ctx.data_mut(|d| {
                            d.insert_temp(sync_status_id, "Discovering nearby peers...".to_string())
                        });
                    }
                    Err(error) => {
                        ctx.data_mut(|d| {
                            d.insert_temp(sync_status_id, format!("Discovery unavailable: {error}"))
                        });
                    }
                }
            }
            if ui.button("Sync Now").clicked() {
                let intents =
                    crate::shell::desktop::runtime::registries::phase5_execute_verse_sync_now_action(
                        app,
                    );
                if intents.is_empty() {
                    apply_reducer_graph_intents_hardened(
                        app,
                        [crate::app::RuntimeEvent::SyncNow.into()],
                    );
                } else {
                    apply_reducer_graph_intents_hardened(app, intents);
                }
                ctx.data_mut(|d| d.insert_temp(sync_status_id, "Manual sync requested".to_string()));
            }
            if ui.button("Share Session Workspace").clicked() {
                let intents = crate::shell::desktop::runtime::registries::phase5_execute_verse_share_workspace_action(
                    app,
                    crate::app::GraphBrowserApp::SESSION_WORKSPACE_LAYOUT_NAME,
                );
                if !intents.is_empty() {
                    apply_reducer_graph_intents_hardened(app, intents);
                }
                ctx.data_mut(|d| {
                    d.insert_temp(
                        sync_status_id,
                        "Shared session workspace with paired peers".to_string(),
                    )
                });
            }
        });

        if let Some(code) =
            ctx.data_mut(|d| d.get_temp::<crate::mods::native::verse::PairingCode>(pairing_code_id))
        {
            ui.group(|ui| {
                ui.label(egui::RichText::new("Pairing Code").strong());
                ui.monospace(code.phrase);
            });
        }

        let mut pairing_code_input = ctx
            .data_mut(|d| d.get_temp::<String>(pairing_code_input_id))
            .unwrap_or_default();
        ui.group(|ui| {
            ui.label(egui::RichText::new("Pair by Code").strong());
            ui.small("Format: word-word-word-word-word-word:<node-id>");
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut pairing_code_input)
                        .desired_width(340.0)
                        .hint_text("word-word-word-word-word-word:<node-id>"),
                );
                if ui.button("Pair").clicked() {
                    let code = pairing_code_input.trim().to_string();
                    if code.is_empty() {
                        ctx.data_mut(|d| {
                            d.insert_temp(sync_status_id, "Enter a pairing code first".to_string())
                        });
                    } else {
                        let before = crate::mods::native::verse::get_trusted_peers().len();
                        let intents = crate::shell::desktop::runtime::registries::phase5_execute_verse_pair_code_action(
                            app,
                            &code,
                        );
                        if !intents.is_empty() {
                            apply_reducer_graph_intents_hardened(app, intents);
                        }
                        let after = crate::mods::native::verse::get_trusted_peers().len();
                        let status = if after > before {
                            "Pairing succeeded".to_string()
                        } else {
                            "Pairing not completed (verify code and try again)".to_string()
                        };
                        ctx.data_mut(|d| d.insert_temp(sync_status_id, status));
                    }
                }
            });
        });
        ctx.data_mut(|d| d.insert_temp(pairing_code_input_id, pairing_code_input));

        if let Some(peers) = ctx.data_mut(|d| {
            d.get_temp::<Vec<crate::mods::native::verse::DiscoveredPeer>>(discovery_results_id)
        }) && !peers.is_empty()
        {
            ui.group(|ui| {
                ui.label(egui::RichText::new("Nearby Devices").strong());
                for peer in peers {
                    ui.horizontal(|ui| {
                        ui.label(format!("{} ({})", peer.device_name, peer.node_id.to_string()));
                        if ui.button("Pair").clicked() {
                            let intents = crate::shell::desktop::runtime::registries::phase5_execute_verse_pair_local_peer_action(
                                app,
                                &peer.node_id.to_string(),
                            );
                            if !intents.is_empty() {
                                apply_reducer_graph_intents_hardened(app, intents);
                            }
                            ctx.data_mut(|d| {
                                d.insert_temp(
                                    sync_status_id,
                                    format!("Paired with {}", peer.node_id.to_string()),
                                )
                            });
                        }
                    });
                }
            });
        }

        let peers = crate::mods::native::verse::get_trusted_peers();
        if peers.is_empty() {
            ui.label("No paired devices yet.");
        } else {
            for peer in &peers {
                ui.group(|ui| {
                    let peer_display = format!(
                        "{} ({})",
                        peer.display_name,
                        peer.node_id.to_string()[..8].to_uppercase()
                    );
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(peer_display).strong());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("Forget").clicked() {
                                let intents = crate::shell::desktop::runtime::registries::phase5_execute_verse_forget_device_action(
                                    app,
                                    &peer.node_id.to_string(),
                                );
                                apply_reducer_graph_intents_hardened(app, intents);
                            }
                        });
                    });

                    if peer.workspace_grants.is_empty() {
                        ui.small("No workspace grants");
                    } else {
                        for grant in &peer.workspace_grants {
                            ui.horizontal(|ui| {
                                let access_str = match grant.access {
                                    crate::mods::native::verse::AccessLevel::ReadOnly => {
                                        "read-only"
                                    }
                                    crate::mods::native::verse::AccessLevel::ReadWrite => {
                                        "read-write"
                                    }
                                };
                                ui.small(format!("{}: {}", grant.workspace_id, access_str));
                                if ui.small_button("Revoke").clicked() {
                                    let intent = crate::app::GraphIntent::RevokeWorkspaceAccess {
                                        peer_id: peer.node_id.to_string(),
                                        workspace_id: grant.workspace_id.clone(),
                                    };
                                    apply_reducer_graph_intents_hardened(app, vec![intent]);
                                }
                            });
                        }
                    }
                });
            }
        }
    }

    ui.separator();
    ui.label(egui::RichText::new("Sync Status").strong());
    if !verse_initialized {
        ui.label("Initializing Verse networking...");
    } else {
        let peers = crate::mods::native::verse::get_trusted_peers();
        ui.label(format!("Connected peers: {}", peers.len()));
    }

    if let Some(message) = ctx.data_mut(|d| d.get_temp::<String>(sync_status_id)) {
        ui.separator();
        ui.small(message);
    }
}
