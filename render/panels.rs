use std::env;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use egui::{Ui, Window};
use uuid::Uuid;

use crate::app::{
    GraphBrowserApp, GraphIntent, HistoryCaptureStatus, HistoryManagerTab, KeyboardPanInputMode,
    WorkbenchIntent,
};
use crate::registries::domain::layout::canvas::CanvasLassoBinding;
use crate::shell::desktop::runtime::diagnostics::{DiagnosticEvent, emit_event};
use crate::shell::desktop::runtime::registries::{
    CHANNEL_UI_HISTORY_MANAGER_LIMIT, CHANNEL_UX_NAVIGATION_TRANSITION,
};
use crate::util::{GraphshellSettingsPath, VersoAddress};

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
    if let Some(view_id) = camera_settings_target_view_id(app)
        && let Some(view) = app.workspace.views.get(&view_id)
    {
        return crate::registries::atomic::lens::physics_profile_id(&view.lens.physics).to_string();
    }

    app.default_registry_physics_id()
        .unwrap_or(crate::registries::atomic::lens::PHYSICS_ID_DEFAULT)
        .to_string()
}

fn apply_node_dynamics_profile_selection(app: &mut GraphBrowserApp, physics_id: &str) {
    app.set_default_registry_physics_id(Some(physics_id));

    let profile = crate::registries::atomic::lens::resolve_physics_profile(physics_id).profile;
    let mut resolved_profile = None;
    if let Some(view_id) = camera_settings_target_view_id(app) {
        let updated_lens = app.workspace.views.get(&view_id).map(|view| {
            let mut lens = view.lens.clone();
            lens.physics = profile.clone();
            lens
        });

        if let Some(updated_lens) = updated_lens {
            let profile = updated_lens.physics.clone();
            if let Some(view) = app.workspace.views.get_mut(&view_id) {
                view.lens = updated_lens;
            }
            resolved_profile = Some(profile);
        }
    }

    let profile = resolved_profile.unwrap_or(profile);

    let mut config = app.workspace.physics.clone();
    profile.apply_to_state(&mut config);
    app.update_physics_config(config);
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
                            let command_palette_key = match app.workspace.command_palette_shortcut {
                                crate::app::CommandPaletteShortcut::F2 => "F2",
                                crate::app::CommandPaletteShortcut::CtrlK => "Ctrl+K",
                            };
                            let radial_key = match app.workspace.radial_menu_shortcut {
                                crate::app::RadialMenuShortcut::F3 => "F3",
                                crate::app::RadialMenuShortcut::R => "R",
                            };
                            let help_key = match app.workspace.help_panel_shortcut {
                                crate::app::HelpPanelShortcut::F1OrQuestion => "F1 / ?",
                                crate::app::HelpPanelShortcut::H => "H",
                            };
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
                                (command_palette_key, "Toggle command palette"),
                                (radial_key, "Toggle radial palette mode"),
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
                                (help_key, "This help panel"),
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
