use std::env;
use std::str::FromStr;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use egui::{Ui, Window};
use uuid::Uuid;

use crate::app::{
    ClipInspectorFilter, ContextCommandSurfacePreference, GraphBrowserApp, GraphIntent,
    HistoryCaptureStatus, HistoryManagerTab, KeyboardPanInputMode, OmnibarNonAtOrderPreset,
    OmnibarPreferredScope, SettingsToolPage, ToastAnchorPreference, ViewAction, WorkbenchIntent,
    clip_capture_matches_filter, clip_capture_matches_query,
};
use crate::registries::domain::layout::canvas::CanvasLassoBinding;
use crate::shell::desktop::runtime::diagnostics::{DiagnosticEvent, emit_event};
use crate::shell::desktop::runtime::registries::input::{
    GamepadButton, InputBinding, InputBindingRemap, InputBindingSection, InputContext, action_id,
};
use crate::shell::desktop::runtime::registries::{
    CHANNEL_UI_HISTORY_MANAGER_LIMIT, phase2_describe_input_bindings,
};
use crate::shell::desktop::workbench::tile_compositor::CompositorFrameActivitySummary;
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
    crate::shell::desktop::runtime::registries::phase3_resolve_active_physics_profile().resolved_id
}

fn apply_node_dynamics_profile_selection(app: &mut GraphBrowserApp, physics_id: &str) {
    apply_reducer_graph_intents_hardened(
        app,
        vec![GraphIntent::SetPhysicsProfile {
            profile_id: physics_id.to_string(),
        }],
    );
}

fn selected_dynamic_layout_algorithm_id(app: &GraphBrowserApp) -> String {
    if let Some(view_id) = camera_settings_target_view_id(app)
        && let Some(view) = app.workspace.views.get(&view_id)
    {
        return view.lens.layout_algorithm_id.clone();
    }

    crate::shell::desktop::runtime::registries::phase3_resolve_active_canvas_profile()
        .profile
        .layout_algorithm
        .algorithm_id
}

fn apply_dynamic_layout_algorithm_selection(app: &mut GraphBrowserApp, algorithm_id: &str) {
    let Some(view_id) = camera_settings_target_view_id(app) else {
        return;
    };
    let Some(view) = app.workspace.views.get(&view_id) else {
        return;
    };

    let mut lens = view.lens.clone();
    lens.layout_algorithm_id = algorithm_id.to_string();
    lens.layout = crate::registries::atomic::lens::LayoutMode::Free;
    apply_reducer_graph_intents_hardened(app, vec![GraphIntent::SetViewLens { view_id, lens }]);
}

fn semantic_depth_view_toggle_label(app: &GraphBrowserApp) -> Option<&'static str> {
    let view_id = camera_settings_target_view_id(app)?;
    let view = app.workspace.views.get(&view_id)?;
    Some(
        if matches!(
            view.dimension,
            crate::app::ViewDimension::ThreeD {
                mode: crate::app::ThreeDMode::TwoPointFive,
                z_source: crate::app::ZSource::UdcLevel { .. }
            }
        ) {
            "Restore View"
        } else {
            "UDC Depth View"
        },
    )
}

fn apply_semantic_depth_view_toggle(app: &mut GraphBrowserApp) {
    let Some(view_id) = camera_settings_target_view_id(app) else {
        return;
    };

    apply_reducer_graph_intents_hardened(
        app,
        vec![GraphIntent::ToggleSemanticDepthView { view_id }],
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

    let mut layout_algorithm_id = selected_dynamic_layout_algorithm_id(app);
    let previous_layout_algorithm_id = layout_algorithm_id.clone();
    ui.label("Dynamic Layout Algorithm");
    ui.small("Choose the active force-directed engine for the focused graph view.");
    ui.horizontal_wrapped(|ui| {
        ui.radio_value(
            &mut layout_algorithm_id,
            crate::app::graph_layout::GRAPH_LAYOUT_FORCE_DIRECTED.to_string(),
            "Force Directed",
        );
        ui.radio_value(
            &mut layout_algorithm_id,
            crate::app::graph_layout::GRAPH_LAYOUT_FORCE_DIRECTED_BARNES_HUT.to_string(),
            "Barnes-Hut",
        );
    });
    if camera_settings_target_view_id(app).is_none() {
        ui.small("Select or focus a graph view to persist a layout-engine override.");
    }
    if layout_algorithm_id != previous_layout_algorithm_id {
        apply_dynamic_layout_algorithm_selection(app, &layout_algorithm_id);
    }

    ui.separator();
    ui.label("Semantic View");
    ui.small("Apply or restore the focused Graph View's reversible UDC depth layering.");
    if let Some(button_label) = semantic_depth_view_toggle_label(app) {
        if ui.button(button_label).clicked() {
            apply_semantic_depth_view_toggle(app);
        }
        ui.small(if button_label == "Restore View" {
            "The focused Graph View is currently using semantic depth layers; restore the prior dimension when finished."
        } else {
            "Lift nodes into UDC depth layers without changing layout or lens settings."
        });
    } else {
        ui.small("Select or focus a graph view to toggle the semantic depth view.");
    }

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
    if was_open && !open {
        app.enqueue_workbench_intent(WorkbenchIntent::ToggleHelpPanel);
    } else {
        app.workspace.show_help_panel = open;
    }
}

pub fn render_clip_inspector_panel(ctx: &egui::Context, app: &mut GraphBrowserApp) {
    if !app.workspace.show_clip_inspector {
        return;
    }

    enum InspectorAction {
        ClipSelected(crate::app::ClipCaptureData),
        ClipFiltered(Vec<crate::app::ClipCaptureData>),
        StepStack(isize),
    }

    let mut open = app.workspace.show_clip_inspector;
    let mut close_requested = false;
    let mut action = None;
    if let Some(state) = app.workspace.clip_inspector_state.as_ref()
        && !state.pointer_stack.is_empty()
    {
        egui::Area::new(egui::Id::new("clip_inspector_overlay"))
            .order(egui::Order::Foreground)
            .fixed_pos(egui::pos2(18.0, 72.0))
            .show(ctx, |ui| {
                egui::Frame::window(ui.style())
                    .inner_margin(egui::Margin::same(8))
                    .show(ui, |ui| {
                        let selected = &state.pointer_stack[state.pointer_stack_index];
                        ui.horizontal(|ui| {
                            ui.strong("Inspector");
                            ui.small(format!(
                                "{} / {}",
                                state.pointer_stack_index + 1,
                                state.pointer_stack.len()
                            ));
                        });
                        ui.small(format!("{} [{}]", selected.clip_title, selected.tag_name));
                        ui.horizontal(|ui| {
                            if ui.button("Up Stack").clicked() {
                                action = Some(InspectorAction::StepStack(1));
                            }
                            if ui.button("Down Stack").clicked() {
                                action = Some(InspectorAction::StepStack(-1));
                            }
                            if ui.button("Clip Hovered").clicked() {
                                action = Some(InspectorAction::ClipSelected(selected.clone()));
                            }
                        });
                    });
            });
    }
    Window::new("Web Inspector")
        .open(&mut open)
        .default_width(640.0)
        .default_height(520.0)
        .resizable(true)
        .show(ctx, |ui| {
            let Some(state) = app.workspace.clip_inspector_state.as_mut() else {
                close_requested = true;
                return;
            };

            ui.horizontal(|ui| {
                ui.heading(state.page_title.as_deref().unwrap_or("Page Inspector"));
                ui.separator();
                ui.small(&state.source_url);
            });
            ui.add_space(6.0);

            ui.horizontal(|ui| {
                ui.label("Search");
                ui.text_edit_singleline(&mut state.search_query);
            });
            ui.add_space(4.0);

            ui.horizontal_wrapped(|ui| {
                for (label, filter) in [
                    ("All", ClipInspectorFilter::All),
                    ("Text", ClipInspectorFilter::Text),
                    ("Link", ClipInspectorFilter::Link),
                    ("Image", ClipInspectorFilter::Image),
                    ("Structure", ClipInspectorFilter::Structure),
                    ("Media", ClipInspectorFilter::Media),
                ] {
                    ui.selectable_value(&mut state.filter, filter, label);
                }
            });
            ui.separator();

            let filtered_indices = state
                .captures
                .iter()
                .enumerate()
                .filter_map(|(index, capture)| {
                    (clip_capture_matches_filter(capture, state.filter)
                        && clip_capture_matches_query(capture, &state.search_query))
                    .then_some(index)
                })
                .collect::<Vec<_>>();

            if filtered_indices.is_empty() {
                ui.label("No page elements match the current filter.");
            } else {
                if state.selected_index >= filtered_indices.len() {
                    state.selected_index = 0;
                }
                ui.horizontal(|ui| {
                    if ui.button("Prev").clicked() && state.selected_index > 0 {
                        state.selected_index -= 1;
                    }
                    ui.label(format!(
                        "{} of {}",
                        state.selected_index + 1,
                        filtered_indices.len()
                    ));
                    if ui.button("Next").clicked()
                        && state.selected_index + 1 < filtered_indices.len()
                    {
                        state.selected_index += 1;
                    }
                    if ui.button("Clip Selected").clicked() {
                        action = Some(InspectorAction::ClipSelected(
                            state.captures[filtered_indices[state.selected_index]].clone(),
                        ));
                    }
                    if ui
                        .button(format!("Clip Filtered ({})", filtered_indices.len()))
                        .clicked()
                    {
                        action = Some(InspectorAction::ClipFiltered(
                            filtered_indices
                                .iter()
                                .map(|index| state.captures[*index].clone())
                                .collect::<Vec<_>>(),
                        ));
                    }
                    if ui.button("Close").clicked() {
                        close_requested = true;
                    }
                });
                ui.add_space(8.0);

                ui.columns(2, |columns| {
                    columns[0].vertical(|ui| {
                        ui.label("Candidates");
                        egui::ScrollArea::vertical()
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                for (visible_index, capture_index) in filtered_indices.iter().enumerate()
                                {
                                    let capture = &state.captures[*capture_index];
                                    let selected = visible_index == state.selected_index;
                                    let label = format!(
                                        "{} [{}]",
                                        capture.clip_title,
                                        capture.tag_name
                                    );
                                    if ui.selectable_label(selected, label).clicked() {
                                        state.selected_index = visible_index;
                                    }
                                    if !capture.text_excerpt.is_empty() {
                                        ui.small(ellipsis(&capture.text_excerpt, 88));
                                    }
                                    ui.add_space(4.0);
                                }
                            });
                    });

                    columns[1].vertical(|ui| {
                        let selected = &state.captures[filtered_indices[state.selected_index]];
                        ui.label("Selected Element");
                        ui.separator();
                        ui.strong(&selected.clip_title);
                        ui.small(format!("Tag: <{}>", selected.tag_name));
                        if let Some(href) = selected.href.as_ref() {
                            ui.small(format!("Link: {href}"));
                        }
                        if let Some(image_url) = selected.image_url.as_ref() {
                            ui.small(format!("Media: {image_url}"));
                        }
                        ui.add_space(6.0);
                        ui.label(ellipsis(
                            if selected.text_excerpt.is_empty() {
                                &selected.outer_html
                            } else {
                                &selected.text_excerpt
                            },
                            420,
                        ));
                        ui.add_space(8.0);
                        ui.small(
                            "Inspector mode is selection-first: inspect the page's temporary element graph here, then choose what to materialize into durable nodes.",
                        );
                    });
                });
            }
        });

    if !open || close_requested {
        app.close_clip_inspector();
    } else {
        app.workspace.show_clip_inspector = open;
    }

    match action {
        Some(InspectorAction::ClipSelected(capture)) => {
            let _ = app.create_clip_node_from_capture(&capture);
        }
        Some(InspectorAction::ClipFiltered(captures)) => {
            let _ = app.create_clip_nodes_from_captures(&captures);
        }
        Some(InspectorAction::StepStack(delta)) => {
            app.clip_inspector_step_stack(delta);
        }
        None => {}
    }
}

fn ellipsis(value: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for (index, ch) in value.chars().enumerate() {
        if index >= max_chars {
            out.push_str("...");
            break;
        }
        out.push(ch);
    }
    out
}

pub fn render_history_manager_in_ui(ui: &mut Ui, app: &mut GraphBrowserApp) -> Vec<GraphIntent> {
    let mut intents = Vec::new();
    let (timeline_total, dissolved_total) = app.history_manager_archive_counts();
    let health = app.history_health_summary();
    let auto_curate_keep = history_manager_auto_curate_keep_latest();
    let compositor_activity = history_manager_compositor_activity_snapshot();

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
    if let Some(activity_label) = history_manager_activity_summary_label(&compositor_activity) {
        ui.label(egui::RichText::new(activity_label).small().weak());
    }
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
            render_history_manager_rows(ui, app, &entries, &compositor_activity, &mut intents);
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
            render_history_manager_rows(ui, app, &entries, &compositor_activity, &mut intents);
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
    compositor_activity: &HistoryManagerCompositorActivity,
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
                if let Some(activity_chip) = history_manager_activity_chip(
                    compositor_activity,
                    from_key,
                    to_key,
                ) {
                    ui.label(egui::RichText::new(activity_chip).small().color(egui::Color32::from_rgb(90, 200, 120)));
                }
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

const HISTORY_MANAGER_ACTIVITY_FRAME_WINDOW: usize = 8;

#[derive(Default, Clone)]
struct HistoryManagerCompositorActivity {
    active_tile_keys: std::collections::HashSet<crate::graph::NodeKey>,
    latest_frame_index: Option<u64>,
    frame_sample_count: usize,
}

fn history_manager_compositor_activity_snapshot() -> HistoryManagerCompositorActivity {
    let summaries =
        crate::shell::desktop::workbench::tile_compositor::compositor_activity_summaries_snapshot();
    compositor_activity_for_history_manager(&summaries)
}

fn compositor_activity_for_history_manager(
    summaries: &[CompositorFrameActivitySummary],
) -> HistoryManagerCompositorActivity {
    let mut activity = HistoryManagerCompositorActivity::default();

    for summary in summaries
        .iter()
        .rev()
        .take(HISTORY_MANAGER_ACTIVITY_FRAME_WINDOW)
    {
        activity.latest_frame_index = activity.latest_frame_index.max(Some(summary.frame_index));
        activity.frame_sample_count += 1;
        activity
            .active_tile_keys
            .extend(summary.active_tile_keys.iter().copied());
    }

    activity
}

fn history_manager_activity_summary_label(
    activity: &HistoryManagerCompositorActivity,
) -> Option<String> {
    if activity.frame_sample_count == 0 {
        return None;
    }

    Some(format!(
        "Recent compositor activity: {} active tiles across {} sampled frames{}.",
        activity.active_tile_keys.len(),
        activity.frame_sample_count,
        activity
            .latest_frame_index
            .map(|frame| format!(" (latest frame #{frame})"))
            .unwrap_or_default()
    ))
}

fn history_manager_activity_chip(
    activity: &HistoryManagerCompositorActivity,
    from_key: Option<crate::graph::NodeKey>,
    to_key: Option<crate::graph::NodeKey>,
) -> Option<&'static str> {
    let from_active = from_key.is_some_and(|key| activity.active_tile_keys.contains(&key));
    let to_active = to_key.is_some_and(|key| activity.active_tile_keys.contains(&key));

    match (from_active, to_active) {
        (true, true) => Some("live path"),
        (true, false) => Some("from live"),
        (false, true) => Some("to live"),
        (false, false) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compositor_activity_for_history_manager_limits_recent_frames() {
        let summaries = vec![
            CompositorFrameActivitySummary {
                active_tile_keys: vec![crate::graph::NodeKey::new(1)],
                idle_tile_keys: Vec::new(),
                frame_index: 1,
            },
            CompositorFrameActivitySummary {
                active_tile_keys: vec![crate::graph::NodeKey::new(2)],
                idle_tile_keys: Vec::new(),
                frame_index: 2,
            },
        ];

        let activity = compositor_activity_for_history_manager(&summaries);

        assert_eq!(activity.frame_sample_count, 2);
        assert_eq!(activity.latest_frame_index, Some(2));
        assert!(
            activity
                .active_tile_keys
                .contains(&crate::graph::NodeKey::new(1))
        );
        assert!(
            activity
                .active_tile_keys
                .contains(&crate::graph::NodeKey::new(2))
        );
    }

    #[test]
    fn history_manager_activity_chip_marks_live_endpoints() {
        let activity = HistoryManagerCompositorActivity {
            active_tile_keys: [crate::graph::NodeKey::new(5)].into_iter().collect(),
            latest_frame_index: Some(7),
            frame_sample_count: 1,
        };

        assert_eq!(
            history_manager_activity_chip(
                &activity,
                Some(crate::graph::NodeKey::new(5)),
                Some(crate::graph::NodeKey::new(9)),
            ),
            Some("from live")
        );
        assert_eq!(
            history_manager_activity_chip(
                &activity,
                Some(crate::graph::NodeKey::new(9)),
                Some(crate::graph::NodeKey::new(5)),
            ),
            Some("to live")
        );
        assert_eq!(
            history_manager_activity_chip(
                &activity,
                Some(crate::graph::NodeKey::new(5)),
                Some(crate::graph::NodeKey::new(5)),
            ),
            Some("live path")
        );
        assert_eq!(
            history_manager_activity_chip(
                &activity,
                Some(crate::graph::NodeKey::new(1)),
                Some(crate::graph::NodeKey::new(2)),
            ),
            None
        );
    }

    #[test]
    fn semantic_depth_view_toggle_label_reflects_focused_view_state() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_id = crate::app::GraphViewId::new();
        app.workspace.views.insert(
            view_id,
            crate::app::GraphViewState::new_with_id(view_id, "Focused"),
        );
        app.workspace.focused_view = Some(view_id);

        assert_eq!(
            semantic_depth_view_toggle_label(&app),
            Some("UDC Depth View")
        );

        app.apply_reducer_intents([GraphIntent::ToggleSemanticDepthView { view_id }]);

        assert_eq!(semantic_depth_view_toggle_label(&app), Some("Restore View"));
    }

    #[test]
    fn apply_semantic_depth_view_toggle_targets_focused_graph_view() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_id = crate::app::GraphViewId::new();
        let mut view = crate::app::GraphViewState::new_with_id(view_id, "Focused");
        view.dimension = crate::app::ViewDimension::ThreeD {
            mode: crate::app::ThreeDMode::Isometric,
            z_source: crate::app::ZSource::BfsDepth { scale: 7.0 },
        };
        app.workspace.views.insert(view_id, view);
        app.workspace.focused_view = Some(view_id);

        apply_semantic_depth_view_toggle(&mut app);
        assert_eq!(semantic_depth_view_toggle_label(&app), Some("Restore View"));

        apply_semantic_depth_view_toggle(&mut app);
        assert_eq!(
            semantic_depth_view_toggle_label(&app),
            Some("UDC Depth View")
        );
        assert!(matches!(
            app.workspace.views.get(&view_id).unwrap().dimension,
            crate::app::ViewDimension::ThreeD {
                mode: crate::app::ThreeDMode::Isometric,
                z_source: crate::app::ZSource::BfsDepth { scale: 7.0 }
            }
        ));
    }
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SettingsSurfaceMode {
    ToolPane,
    Overlay,
}

fn settings_page_label(page: SettingsToolPage) -> &'static str {
    match page {
        SettingsToolPage::General => "Overview",
        SettingsToolPage::Persistence => "Persistence",
        SettingsToolPage::Physics => "Physics",
        SettingsToolPage::Sync => "Sync",
        SettingsToolPage::Appearance => "Appearance & Viewer",
        SettingsToolPage::Keybindings => "Input & Commands",
        SettingsToolPage::Advanced => "Advanced",
    }
}

fn settings_page_summary(page: SettingsToolPage) -> &'static str {
    match page {
        SettingsToolPage::General => "How settings surfaces and related tools are organized.",
        SettingsToolPage::Persistence => "Storage paths, snapshots, and graph persistence.",
        SettingsToolPage::Physics => "Simulation, camera, and layout behavior.",
        SettingsToolPage::Sync => "Verse and peer-facing sync controls.",
        SettingsToolPage::Appearance => "Theme, toasts, and viewer/backend preferences.",
        SettingsToolPage::Keybindings => "Input behavior, omnibar defaults, and keybindings.",
        SettingsToolPage::Advanced => "Registry-level defaults and diagnostic launchers.",
    }
}

fn toast_anchor_label(anchor: ToastAnchorPreference) -> &'static str {
    match anchor {
        ToastAnchorPreference::TopRight => "Top Right",
        ToastAnchorPreference::TopLeft => "Top Left",
        ToastAnchorPreference::BottomRight => "Bottom Right (Default)",
        ToastAnchorPreference::BottomLeft => "Bottom Left",
    }
}

fn lasso_binding_label(binding: CanvasLassoBinding) -> &'static str {
    match binding {
        CanvasLassoBinding::RightDrag => "Right Drag (Default)",
        CanvasLassoBinding::ShiftLeftDrag => "Shift + Left Drag",
    }
}

fn omnibar_preferred_scope_label(scope: OmnibarPreferredScope) -> &'static str {
    match scope {
        OmnibarPreferredScope::Auto => "Auto (Contextual)",
        OmnibarPreferredScope::LocalTabs => "Local Tabs First",
        OmnibarPreferredScope::ConnectedNodes => "Connected Nodes First",
        OmnibarPreferredScope::ProviderDefault => "Provider First",
        OmnibarPreferredScope::GlobalNodes => "Global Nodes First",
        OmnibarPreferredScope::GlobalTabs => "Global Tabs First",
    }
}

fn omnibar_non_at_order_label(order: OmnibarNonAtOrderPreset) -> &'static str {
    match order {
        OmnibarNonAtOrderPreset::ContextualThenProviderThenGlobal => {
            "Contextual -> Provider -> Global (Default)"
        }
        OmnibarNonAtOrderPreset::ProviderThenContextualThenGlobal => {
            "Provider -> Contextual -> Global"
        }
    }
}

fn render_settings_nav(ui: &mut Ui, current_page: &mut SettingsToolPage) {
    ui.set_min_width(172.0);
    ui.label(egui::RichText::new("Pages").small().strong());
    ui.add_space(4.0);

    for page in [
        SettingsToolPage::General,
        SettingsToolPage::Persistence,
        SettingsToolPage::Appearance,
        SettingsToolPage::Keybindings,
        SettingsToolPage::Physics,
        SettingsToolPage::Sync,
        SettingsToolPage::Advanced,
    ] {
        let selected = *current_page == page;
        let response = ui.selectable_label(selected, settings_page_label(page));
        if response.clicked() {
            *current_page = page;
        }
        ui.add(
            egui::Label::new(
                egui::RichText::new(settings_page_summary(page))
                    .small()
                    .weak(),
            )
            .wrap(),
        );
        ui.add_space(6.0);
    }
}

fn render_settings_surface_in_ui_with_control_panel(
    ui: &mut Ui,
    app: &mut GraphBrowserApp,
    mut control_panel: Option<&mut crate::shell::desktop::runtime::control_panel::ControlPanel>,
    surface_mode: SettingsSurfaceMode,
) -> Vec<GraphIntent> {
    let mut intents: Vec<GraphIntent> = Vec::new();
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.heading("Settings");
            ui.label(
                egui::RichText::new(match surface_mode {
                    SettingsSurfaceMode::ToolPane => "Workbench-hosted pane",
                    SettingsSurfaceMode::Overlay => "Transient graph overlay",
                })
                .small()
                .weak(),
            );
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Done").clicked() {
                match surface_mode {
                    SettingsSurfaceMode::ToolPane => {
                        app.enqueue_workbench_intent(WorkbenchIntent::CloseToolPane {
                            kind: crate::shell::desktop::workbench::pane_model::ToolPaneState::Settings,
                            restore_previous_focus: true,
                        });
                    }
                    SettingsSurfaceMode::Overlay => app.close_settings_overlay(),
                }
            }
            if matches!(surface_mode, SettingsSurfaceMode::Overlay)
                && ui.button("Tile This Page").clicked()
            {
                app.enqueue_workbench_intent(WorkbenchIntent::OpenToolPane {
                    kind: crate::shell::desktop::workbench::pane_model::ToolPaneState::Settings,
                });
                app.close_settings_overlay();
            }
        });
    });
    ui.separator();

    ui.horizontal_top(|ui| {
        ui.vertical(|ui| render_settings_nav(ui, &mut app.workspace.settings_tool_page));
        ui.separator();
        ui.vertical(|ui| {
            ui.heading(settings_page_label(app.workspace.settings_tool_page));
            ui.label(
                egui::RichText::new(settings_page_summary(app.workspace.settings_tool_page))
                    .small()
                    .weak(),
            );
            ui.add_space(8.0);
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    match app.workspace.settings_tool_page {
                SettingsToolPage::General => {
                    ui.label(
                        "Settings surfaces configure graph, view, and workbench behavior without becoming the semantic owner of those domains.",
                    );
                    ui.small(
                        "Use the page list on the left for editable categories. Related tool surfaces stay separate so settings does not turn into a generic utilities drawer.",
                    );

                    ui.separator();
                    ui.label("Current Surface");
                    ui.small(match surface_mode {
                        SettingsSurfaceMode::ToolPane => {
                            "This page is hosted in the workbench and participates in normal pane focus, split, and restore behavior."
                        }
                        SettingsSurfaceMode::Overlay => {
                            "This page is currently a graph overlay. Use 'Tile This Page' to promote it into the workbench without changing the route."
                        }
                    });

                    ui.separator();
                    ui.label("Related Surfaces");
                    if ui.button("Open History Manager").clicked() {
                        app.enqueue_workbench_intent(WorkbenchIntent::OpenToolPane {
                            kind: crate::shell::desktop::workbench::pane_model::ToolPaneState::HistoryManager,
                        });
                    }
                    if ui
                        .button(if app.workspace.show_help_panel {
                            "Hide Help Panel"
                        } else {
                            "Show Help Panel"
                        })
                        .clicked()
                    {
                        intents.push(GraphIntent::ToggleHelpPanel);
                    }
                    #[cfg(feature = "diagnostics")]
                    if ui.button("Open Diagnostics Pane").clicked() {
                        app.enqueue_workbench_intent(WorkbenchIntent::OpenToolPane {
                            kind: crate::shell::desktop::workbench::pane_model::ToolPaneState::Diagnostics,
                        });
                    }
                }
                SettingsToolPage::Persistence => {
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
                    ui.label("Graph Snapshots");
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
                    ui.small(
                        "Frame save, restore, and pruning now live with the active frame in workbench chrome rather than inside persistence settings.",
                    );
                }
                SettingsToolPage::Physics => {
                    render_physics_settings_in_ui(ui, app);
                }
                SettingsToolPage::Sync => {
                    if let Some(control_panel) = control_panel.as_mut() {
                        render_sync_settings_in_ui(ui, app, control_panel);
                    } else {
                        ui.small("Sync controls unavailable in this surface.");
                    }
                }
                SettingsToolPage::Appearance => {
                    ui.label("Theme");
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
                    ui.label("Notifications");
                    ui.label(format!(
                        "Toast anchor: {}",
                        toast_anchor_label(app.workspace.toast_anchor_preference)
                    ));
                    for anchor in [
                        ToastAnchorPreference::BottomRight,
                        ToastAnchorPreference::BottomLeft,
                        ToastAnchorPreference::TopRight,
                        ToastAnchorPreference::TopLeft,
                    ] {
                        if ui
                            .selectable_label(
                                app.workspace.toast_anchor_preference == anchor,
                                toast_anchor_label(anchor),
                            )
                            .clicked()
                        {
                            app.set_toast_anchor_preference(anchor);
                        }
                    }

                    ui.separator();
                    ui.label("Viewer Backends");
                    let wry_compiled = cfg!(feature = "wry");
                    let wry_capability_available = wry_compiled
                        && crate::registries::infrastructure::mod_loader::runtime_has_capability(
                            "viewer:wry",
                        );
                    let wry_disabled_reason = if !wry_compiled {
                        Some("Wry backend is not compiled in this build.")
                    } else if !wry_capability_available {
                        Some("Runtime capability 'viewer:wry' is unavailable.")
                    } else {
                        None
                    };
                    let mut wry_enabled = app.wry_enabled();
                    let wry_toggle_response = ui.add_enabled(
                        wry_disabled_reason.is_none(),
                        egui::Checkbox::new(&mut wry_enabled, "Enable Wry backend"),
                    );
                    if wry_toggle_response.changed() {
                        app.set_wry_enabled(wry_enabled);
                    }
                    if let Some(reason) = wry_disabled_reason {
                        wry_toggle_response.on_hover_text(reason);
                        ui.small(reason);
                    }
                }
                SettingsToolPage::Keybindings => {
                    ui.label("Graph Input");
                    ui.label(format!(
                        "Lasso: {}",
                        lasso_binding_label(app.lasso_binding_preference())
                    ));
                    for binding in [
                        CanvasLassoBinding::RightDrag,
                        CanvasLassoBinding::ShiftLeftDrag,
                    ] {
                        if ui
                            .selectable_label(
                                app.lasso_binding_preference() == binding,
                                lasso_binding_label(binding),
                            )
                            .clicked()
                        {
                            app.set_lasso_binding_preference(binding);
                        }
                    }

                    ui.separator();
                    ui.label("Command Surfaces");
                    ui.label(format!(
                        "Right-click surface: {}",
                        match app.context_command_surface_preference() {
                            ContextCommandSurfacePreference::RadialPalette => "Radial Palette",
                            ContextCommandSurfacePreference::ContextPalette => "Context Palette",
                        }
                    ));
                    for preference in [
                        ContextCommandSurfacePreference::RadialPalette,
                        ContextCommandSurfacePreference::ContextPalette,
                    ] {
                        let label = match preference {
                            ContextCommandSurfacePreference::RadialPalette => "Radial Palette",
                            ContextCommandSurfacePreference::ContextPalette => "Context Palette",
                        };
                        if ui
                            .selectable_label(
                                app.context_command_surface_preference() == preference,
                                label,
                            )
                            .clicked()
                        {
                            app.set_context_command_surface_preference(preference);
                        }
                    }

                    let radial_open_east_preset = InputBindingRemap {
                        old: InputBinding::Gamepad {
                            button: GamepadButton::South,
                            modifier: None,
                        },
                        new: InputBinding::Gamepad {
                            button: GamepadButton::East,
                            modifier: None,
                        },
                        context: InputContext::GraphView,
                    };
                    let active_remaps = app.input_binding_remaps();
                    let radial_profile_label = if active_remaps.is_empty() {
                        "South / A (Default)"
                    } else if active_remaps.len() == 1 && active_remaps[0] == radial_open_east_preset
                    {
                        "East / B"
                    } else {
                        "Custom"
                    };
                    ui.label(format!("Gamepad radial palette open: {radial_profile_label}"));
                    if ui
                        .selectable_label(active_remaps.is_empty(), "South / A (Default)")
                        .clicked()
                        && let Err(error) = app.set_input_binding_remaps(&[])
                    {
                        log::warn!("failed to restore default input remaps: {error:?}");
                    }
                    if ui
                        .selectable_label(
                            active_remaps.len() == 1 && active_remaps[0] == radial_open_east_preset,
                            "East / B",
                        )
                        .clicked()
                        && let Err(error) =
                            app.set_input_binding_remaps(&[radial_open_east_preset.clone()])
                    {
                        log::warn!("failed to apply radial-open remap preset: {error:?}");
                    }
                    if !active_remaps.is_empty()
                        && !(active_remaps.len() == 1 && active_remaps[0] == radial_open_east_preset)
                    {
                        ui.small("Stored remaps include custom bindings outside these presets.");
                    }

                    ui.separator();
                    ui.label("Omnibar");
                    ui.label(format!(
                        "Preferred scope: {}",
                        omnibar_preferred_scope_label(app.workspace.omnibar_preferred_scope)
                    ));
                    for scope in [
                        OmnibarPreferredScope::Auto,
                        OmnibarPreferredScope::LocalTabs,
                        OmnibarPreferredScope::ConnectedNodes,
                        OmnibarPreferredScope::ProviderDefault,
                        OmnibarPreferredScope::GlobalNodes,
                        OmnibarPreferredScope::GlobalTabs,
                    ] {
                        if ui
                            .selectable_label(
                                app.workspace.omnibar_preferred_scope == scope,
                                omnibar_preferred_scope_label(scope),
                            )
                            .clicked()
                        {
                            app.set_omnibar_preferred_scope(scope);
                        }
                    }
                    ui.label(format!(
                        "Non-@ order: {}",
                        omnibar_non_at_order_label(app.workspace.omnibar_non_at_order)
                    ));
                    for order in [
                        OmnibarNonAtOrderPreset::ContextualThenProviderThenGlobal,
                        OmnibarNonAtOrderPreset::ProviderThenContextualThenGlobal,
                    ] {
                        if ui
                            .selectable_label(
                                app.workspace.omnibar_non_at_order == order,
                                omnibar_non_at_order_label(order),
                            )
                            .clicked()
                        {
                            app.set_omnibar_non_at_order(order);
                        }
                    }

                    ui.separator();
                    ui.label("Keybindings");
                    render_keybindings_settings_in_ui(ui, app);
                }
                SettingsToolPage::Advanced => {
                    ui.label("Registry Defaults");

                    let mut lens_id = app
                        .default_registry_lens_id()
                        .unwrap_or_default()
                        .to_string();
                    if ui
                        .horizontal(|ui| {
                            ui.label("Lens ID");
                            ui.text_edit_singleline(&mut lens_id)
                        })
                        .inner
                        .changed()
                    {
                        let value = lens_id.trim();
                        app.set_default_registry_lens_id((!value.is_empty()).then_some(value));
                    }

                    let mut physics_id = app
                        .default_registry_physics_id()
                        .unwrap_or_default()
                        .to_string();
                    if ui
                        .horizontal(|ui| {
                            ui.label("Physics ID");
                            ui.text_edit_singleline(&mut physics_id)
                        })
                        .inner
                        .changed()
                    {
                        let value = physics_id.trim();
                        app.set_default_registry_physics_id((!value.is_empty()).then_some(value));
                    }

                    let mut theme_id = app
                        .default_registry_theme_id()
                        .unwrap_or_default()
                        .to_string();
                    if ui
                        .horizontal(|ui| {
                            ui.label("Theme ID");
                            ui.text_edit_singleline(&mut theme_id)
                        })
                        .inner
                        .changed()
                    {
                        let value = theme_id.trim();
                        app.set_default_registry_theme_id((!value.is_empty()).then_some(value));
                    }

                    ui.separator();
                    ui.label("Related Control Surfaces");
                    if ui.button("Open History Manager").clicked() {
                        app.enqueue_workbench_intent(WorkbenchIntent::OpenToolPane {
                            kind: crate::shell::desktop::workbench::pane_model::ToolPaneState::HistoryManager,
                        });
                    }
                    #[cfg(feature = "diagnostics")]
                    if ui.button("Open Diagnostics Pane").clicked() {
                        app.enqueue_workbench_intent(WorkbenchIntent::OpenToolPane {
                            kind: crate::shell::desktop::workbench::pane_model::ToolPaneState::Diagnostics,
                        });
                    }
                }
                    }
                });
        });
    });

    intents
}

pub fn render_settings_tool_pane_in_ui_with_control_panel(
    ui: &mut Ui,
    app: &mut GraphBrowserApp,
    control_panel: Option<&mut crate::shell::desktop::runtime::control_panel::ControlPanel>,
) -> Vec<GraphIntent> {
    render_settings_surface_in_ui_with_control_panel(
        ui,
        app,
        control_panel,
        SettingsSurfaceMode::ToolPane,
    )
}

pub fn render_settings_overlay_panel(
    ctx: &egui::Context,
    app: &mut GraphBrowserApp,
    control_panel: Option<&mut crate::shell::desktop::runtime::control_panel::ControlPanel>,
) {
    if !app.workspace.show_settings_overlay {
        return;
    }

    let mut open = app.workspace.show_settings_overlay;
    Window::new("Settings")
        .open(&mut open)
        .default_width(520.0)
        .default_height(560.0)
        .resizable(true)
        .show(ctx, |ui| {
            let _ = render_settings_surface_in_ui_with_control_panel(
                ui,
                app,
                control_panel,
                SettingsSurfaceMode::Overlay,
            );
        });

    if app.workspace.show_settings_overlay && !open {
        app.close_settings_overlay();
    } else {
        app.workspace.show_settings_overlay = open;
    }
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

    if let (Some(action_id), Some(context_raw)) =
        (capture_action.as_deref(), capture_context.as_deref())
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
                            .button(if is_capturing {
                                "Press Key..."
                            } else {
                                "Rebind"
                            })
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
                        let before =
                            crate::shell::desktop::runtime::registries::phase3_trusted_peers()
                                .len();
                        let intents = crate::shell::desktop::runtime::registries::phase5_execute_verse_pair_code_action(
                            app,
                            &code,
                        );
                        if !intents.is_empty() {
                            apply_reducer_graph_intents_hardened(app, intents);
                        }
                        let after =
                            crate::shell::desktop::runtime::registries::phase3_trusted_peers()
                                .len();
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

        let peers = crate::shell::desktop::runtime::registries::phase3_trusted_peers();
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
        let peers = crate::shell::desktop::runtime::registries::phase3_trusted_peers();
        ui.label(format!("Connected peers: {}", peers.len()));
    }

    render_nostr_sync_settings_in_ui(ui, app);

    if let Some(message) = ctx.data_mut(|d| d.get_temp::<String>(sync_status_id)) {
        ui.separator();
        ui.small(message);
    }
}

fn render_nostr_sync_settings_in_ui(ui: &mut Ui, app: &mut GraphBrowserApp) {
    let ctx = ui.ctx().clone();
    let bunker_uri_input_id = egui::Id::new("nostr_bunker_uri_input");
    let nostr_status_id = egui::Id::new("nostr_signer_status");

    ui.separator();
    ui.label(egui::RichText::new("Nostr Signer").strong());

    if ui.button("Use Local Signer").clicked() {
        crate::shell::desktop::runtime::registries::phase3_nostr_use_local_signer();
        app.save_persisted_nostr_signer_settings();
        ctx.data_mut(|d| {
            d.insert_temp(
                nostr_status_id,
                "Using local secp256k1 user signer".to_string(),
            )
        });
    }

    let mut bunker_uri_input = ctx
        .data_mut(|d| d.get_temp::<String>(bunker_uri_input_id))
        .unwrap_or_default();
    ui.group(|ui| {
        ui.label(egui::RichText::new("Remote Signer (NIP-46)").strong());
        ui.small(
            "Paste a bunker URI to configure a delegated signer. Shared secrets are applied for the current session only and are not persisted.",
        );
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut bunker_uri_input)
                    .desired_width(420.0)
                    .password(true)
                    .hint_text("bunker://<pubkey>?relay=wss://...&secret=...&perms=sign_event"),
            );
            if ui.button("Apply Bunker URI").clicked() {
                match crate::shell::desktop::runtime::registries::phase3_nostr_use_nip46_bunker_uri(
                    bunker_uri_input.trim(),
                ) {
                    Ok(parsed) => {
                        app.save_persisted_nostr_signer_settings();
                        bunker_uri_input.clear();
                        ctx.data_mut(|d| {
                            d.insert_temp(
                                nostr_status_id,
                                format!(
                                    "Configured NIP-46 signer {} on {} relay(s)",
                                    &parsed.signer_pubkey[..parsed.signer_pubkey.len().min(16)],
                                    parsed.relay_urls.len()
                                ),
                            )
                        });
                    }
                    Err(error) => {
                        ctx.data_mut(|d| {
                            d.insert_temp(
                                nostr_status_id,
                                format!("Bunker URI rejected: {error}"),
                            )
                        });
                    }
                }
            }
        });
    });
    ctx.data_mut(|d| d.insert_temp(bunker_uri_input_id, bunker_uri_input));

    match crate::shell::desktop::runtime::registries::phase3_nostr_signer_backend_snapshot() {
        crate::shell::desktop::runtime::registries::NostrSignerBackendSnapshot::LocalHostKey => {
            ui.small("Current backend: local secp256k1 user signer.");
        }
        crate::shell::desktop::runtime::registries::NostrSignerBackendSnapshot::Nip46Delegated {
            relay_urls,
            signer_pubkey,
            has_ephemeral_secret,
            requested_permissions,
            permission_grants,
            signer_user_pubkey,
            connected,
        } => {
            ui.small(format!(
                "Current backend: NIP-46 delegated signer {}",
                &signer_pubkey[..signer_pubkey.len().min(16)]
            ));
            ui.small(format!("Relays: {}", relay_urls.join(", ")));
            ui.small(if connected {
                "Connection status: connected"
            } else {
                "Connection status: pending connect on first use"
            });
            ui.small(if has_ephemeral_secret {
                "Session secret: loaded for this run only"
            } else {
                "Session secret: none loaded"
            });
            if let Some(user_pubkey) = signer_user_pubkey {
                ui.small(format!("Signer user pubkey: {user_pubkey}"));
            }

            let mut grants = permission_grants;
            if grants.is_empty() {
                grants.push(
                    crate::shell::desktop::runtime::registries::nostr_core::Nip46PermissionGrant {
                        permission: "sign_event".to_string(),
                        decision: crate::shell::desktop::runtime::registries::nostr_core::Nip46PermissionDecision::Pending,
                    },
                );
            }

            ui.separator();
            ui.label("Local permission memory");
            if requested_permissions.is_empty() {
                ui.small("No permissions were declared in the bunker URI. Local approval still gates delegated signing.");
            } else {
                ui.small(format!(
                    "Bunker-declared permissions: {}",
                    requested_permissions.join(", ")
                ));
            }
            for grant in grants {
                ui.horizontal(|ui| {
                    ui.label(&grant.permission);
                    ui.small(match grant.decision {
                        crate::shell::desktop::runtime::registries::nostr_core::Nip46PermissionDecision::Pending => "pending",
                        crate::shell::desktop::runtime::registries::nostr_core::Nip46PermissionDecision::Allow => "allowed",
                        crate::shell::desktop::runtime::registries::nostr_core::Nip46PermissionDecision::Deny => "denied",
                    });
                    if ui.small_button("Allow").clicked()
                        && crate::shell::desktop::runtime::registries::phase3_nostr_set_nip46_permission(
                            &grant.permission,
                            crate::shell::desktop::runtime::registries::nostr_core::Nip46PermissionDecision::Allow,
                        )
                        .is_ok()
                    {
                        app.save_persisted_nostr_signer_settings();
                    }
                    if ui.small_button("Deny").clicked()
                        && crate::shell::desktop::runtime::registries::phase3_nostr_set_nip46_permission(
                            &grant.permission,
                            crate::shell::desktop::runtime::registries::nostr_core::Nip46PermissionDecision::Deny,
                        )
                        .is_ok()
                    {
                        app.save_persisted_nostr_signer_settings();
                    }
                    if ui.small_button("Reset").clicked()
                        && crate::shell::desktop::runtime::registries::phase3_nostr_set_nip46_permission(
                            &grant.permission,
                            crate::shell::desktop::runtime::registries::nostr_core::Nip46PermissionDecision::Pending,
                        )
                        .is_ok()
                    {
                        app.save_persisted_nostr_signer_settings();
                    }
                });
            }
        }
    }

    ui.separator();
    ui.label(egui::RichText::new("NIP-07 Web Origins").strong());
    ui.small(
        "Graphshell injects a host-owned window.nostr bridge. Sensitive methods are gated per origin.",
    );
    let nip07_grants =
        crate::shell::desktop::runtime::registries::phase3_nostr_nip07_permission_grants();
    if nip07_grants.is_empty() {
        ui.small("No web origins have requested NIP-07 access yet.");
    } else {
        for grant in nip07_grants {
            ui.horizontal_wrapped(|ui| {
                ui.monospace(&grant.origin);
                ui.label(&grant.method);
                ui.small(match grant.decision {
                    crate::shell::desktop::runtime::registries::Nip07PermissionDecision::Pending => {
                        "pending"
                    }
                    crate::shell::desktop::runtime::registries::Nip07PermissionDecision::Allow => {
                        "allowed"
                    }
                    crate::shell::desktop::runtime::registries::Nip07PermissionDecision::Deny => {
                        "denied"
                    }
                });
                if ui.small_button("Allow").clicked()
                    && crate::shell::desktop::runtime::registries::phase3_nostr_set_nip07_permission(
                        &grant.origin,
                        &grant.method,
                        crate::shell::desktop::runtime::registries::Nip07PermissionDecision::Allow,
                    )
                    .is_ok()
                {
                    app.save_persisted_nostr_nip07_permissions();
                    ctx.data_mut(|d| {
                        d.insert_temp(
                            nostr_status_id,
                            format!("Allowed {} for {}", grant.method, grant.origin),
                        )
                    });
                }
                if ui.small_button("Deny").clicked()
                    && crate::shell::desktop::runtime::registries::phase3_nostr_set_nip07_permission(
                        &grant.origin,
                        &grant.method,
                        crate::shell::desktop::runtime::registries::Nip07PermissionDecision::Deny,
                    )
                    .is_ok()
                {
                    app.save_persisted_nostr_nip07_permissions();
                    ctx.data_mut(|d| {
                        d.insert_temp(
                            nostr_status_id,
                            format!("Denied {} for {}", grant.method, grant.origin),
                        )
                    });
                }
                if ui.small_button("Reset").clicked()
                    && crate::shell::desktop::runtime::registries::phase3_nostr_set_nip07_permission(
                        &grant.origin,
                        &grant.method,
                        crate::shell::desktop::runtime::registries::Nip07PermissionDecision::Pending,
                    )
                    .is_ok()
                {
                    app.save_persisted_nostr_nip07_permissions();
                    ctx.data_mut(|d| {
                        d.insert_temp(
                            nostr_status_id,
                            format!("Reset {} for {}", grant.method, grant.origin),
                        )
                    });
                }
            });
        }
    }

    if let Some(message) = ctx.data_mut(|d| d.get_temp::<String>(nostr_status_id)) {
        ui.small(message);
    }
}
