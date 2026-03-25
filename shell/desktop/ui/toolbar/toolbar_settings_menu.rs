use crate::app::{
    ContextCommandSurfacePreference, GraphBrowserApp, GraphIntent, SettingsToolPage, ThemeMode,
    WorkbenchIntent,
};
use crate::shell::desktop::host::running_app_state::RunningAppState;
use crate::shell::desktop::host::window::EmbedderWindow;
use crate::shell::desktop::runtime::registries::phase3_resolve_active_theme;
use crate::shell::desktop::workbench::pane_model::ToolPaneState;
use crate::util::{GraphshellSettingsPath, VersoAddress};

fn open_settings_page(
    _graph_app: &mut GraphBrowserApp,
    page: SettingsToolPage,
    prefer_overlay: bool,
) {
    let path = match page {
        SettingsToolPage::General => GraphshellSettingsPath::General,
        SettingsToolPage::Persistence => GraphshellSettingsPath::Persistence,
        SettingsToolPage::Physics => GraphshellSettingsPath::Physics,
        SettingsToolPage::Sync => GraphshellSettingsPath::Sync,
        SettingsToolPage::Appearance => GraphshellSettingsPath::Appearance,
        SettingsToolPage::Keybindings => GraphshellSettingsPath::Keybindings,
        SettingsToolPage::Advanced => GraphshellSettingsPath::Advanced,
    };

    crate::shell::desktop::runtime::registries::phase3_publish_settings_route_requested(
        &VersoAddress::settings(path).to_string(),
        prefer_overlay,
    );
}

fn theme_mode_toggle_label(mode: ThemeMode) -> &'static str {
    match mode {
        ThemeMode::System => "Theme: System (follows OS)",
        ThemeMode::Light => "Theme: Light",
        ThemeMode::Dark => "Theme: Dark",
    }
}

fn theme_mode_next(mode: ThemeMode) -> ThemeMode {
    match mode {
        ThemeMode::System => ThemeMode::Light,
        ThemeMode::Light => ThemeMode::Dark,
        ThemeMode::Dark => ThemeMode::System,
    }
}

fn context_command_surface_label(preference: ContextCommandSurfacePreference) -> &'static str {
    match preference {
        ContextCommandSurfacePreference::RadialPalette => "Radial Palette",
        ContextCommandSurfacePreference::ContextPalette => "Context Palette",
    }
}

pub(super) fn render_settings_menu(
    ui: &mut egui::Ui,
    graph_app: &mut GraphBrowserApp,
    state: &RunningAppState,
    prefer_overlay: bool,
    frame_intents: &mut Vec<GraphIntent>,
    location_dirty: &mut bool,
    _window: &EmbedderWindow,
    #[cfg(feature = "diagnostics")]
    diagnostics_state: &mut crate::shell::desktop::runtime::diagnostics::DiagnosticsState,
) {
    let max_menu_height = (ui.ctx().input(|i| i.content_rect().height()) - 120.0).max(180.0);
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .max_height(max_menu_height)
        .show(ui, |ui| {
            let theme_tokens =
                phase3_resolve_active_theme(graph_app.default_registry_theme_id()).tokens;
            ui.label(
                egui::RichText::new(if prefer_overlay {
                    "Graph scope: settings pages open as overlays."
                } else {
                    "Workbench scope: settings pages open in the hosted settings pane."
                })
                .small()
                .weak(),
            );

            ui.separator();
            ui.label("Settings Pages");
            if ui.button("Open Settings").clicked() {
                open_settings_page(graph_app, SettingsToolPage::General, prefer_overlay);
                ui.close();
            }
            if ui.button("Open Persistence").clicked() {
                open_settings_page(graph_app, SettingsToolPage::Persistence, prefer_overlay);
                ui.close();
            }
            if ui.button("Open Appearance & Viewer").clicked() {
                open_settings_page(graph_app, SettingsToolPage::Appearance, prefer_overlay);
                ui.close();
            }
            if ui.button("Open Input & Commands").clicked() {
                open_settings_page(graph_app, SettingsToolPage::Keybindings, prefer_overlay);
                ui.close();
            }
            if ui.button("Open Physics").clicked() {
                open_settings_page(graph_app, SettingsToolPage::Physics, prefer_overlay);
                ui.close();
            }
            if ui.button("Open Sync").clicked() {
                open_settings_page(graph_app, SettingsToolPage::Sync, prefer_overlay);
                ui.close();
            }
            if ui.button("Open Advanced").clicked() {
                open_settings_page(graph_app, SettingsToolPage::Advanced, prefer_overlay);
                ui.close();
            }

            ui.separator();
            ui.label("Appearance");
            let current_mode = graph_app.theme_mode();
            ui.label(
                egui::RichText::new(theme_mode_toggle_label(current_mode))
                    .small()
                    .color(theme_tokens.radial_chrome_text),
            );
            let next_mode = theme_mode_next(current_mode);
            if ui
                .button(format!("Switch to: {}", theme_mode_toggle_label(next_mode)))
                .clicked()
            {
                graph_app.set_theme_mode(next_mode);
                ui.close();
            }

            ui.separator();
            ui.label("Command Surfaces");
            ui.label(
                egui::RichText::new(format!(
                    "Right-click surface: {}",
                    context_command_surface_label(graph_app.context_command_surface_preference())
                ))
                .small()
                .color(theme_tokens.radial_chrome_text),
            );
            for preference in [
                ContextCommandSurfacePreference::RadialPalette,
                ContextCommandSurfacePreference::ContextPalette,
            ] {
                if ui
                    .selectable_label(
                        graph_app.context_command_surface_preference() == preference,
                        context_command_surface_label(preference),
                    )
                    .clicked()
                {
                    graph_app.set_context_command_surface_preference(preference);
                }
            }

            ui.separator();
            ui.label("Related Surfaces");
            if ui.button("Open History Manager").clicked() {
                graph_app.enqueue_workbench_intent(WorkbenchIntent::OpenToolPane {
                    kind: ToolPaneState::HistoryManager,
                });
                ui.close();
            }
            if ui
                .button(if graph_app.workspace.chrome_ui.show_help_panel {
                    "Hide Help Panel"
                } else {
                    "Show Help Panel"
                })
                .clicked()
            {
                frame_intents.push(GraphIntent::ToggleHelpPanel);
                ui.close();
            }
            #[cfg(feature = "diagnostics")]
            if ui.button("Open Diagnostics Pane").clicked() {
                graph_app.enqueue_workbench_intent(WorkbenchIntent::OpenToolPane {
                    kind: ToolPaneState::Diagnostics,
                });
                ui.close();
            }

            ui.separator();
            ui.label("Browser");
            if ui.button("Open Preferences Page").clicked() {
                super::request_open_settings_page(graph_app, frame_intents, "servo:preferences");
                ui.close();
            }
            if ui.button("Open Experimental Preferences").clicked() {
                super::request_open_settings_page(
                    graph_app,
                    frame_intents,
                    "servo:experimental-preferences",
                );
                ui.close();
            }
            let mut experimental_preferences_enabled = state.experimental_preferences_enabled();
            let prefs_toggle = ui
                .toggle_value(
                    &mut experimental_preferences_enabled,
                    "Experimental Preferences",
                )
                .on_hover_text("Enable experimental prefs");
            if prefs_toggle.clicked() {
                state.set_experimental_preferences_enabled(experimental_preferences_enabled);
                *location_dirty = false;
                graph_app.request_reload_all();
            }

            #[cfg(feature = "diagnostics")]
            {
                ui.separator();
                ui.label("Diagnostics Export");
                if ui.button("Export Diagnostic Snapshot (JSON)").clicked() {
                    diagnostics_state.sync_history_health_snapshot_from_app(graph_app);
                    match diagnostics_state.export_snapshot_json() {
                        Ok(path) => log::info!("Diagnostics JSON exported: {}", path.display()),
                        Err(err) => log::warn!("Diagnostics JSON export failed: {err}"),
                    }
                }
                if ui.button("Export Diagnostic Snapshot (SVG)").clicked() {
                    diagnostics_state.sync_history_health_snapshot_from_app(graph_app);
                    match diagnostics_state.export_snapshot_svg() {
                        Ok(path) => log::info!("Diagnostics SVG exported: {}", path.display()),
                        Err(err) => log::warn!("Diagnostics SVG export failed: {err}"),
                    }
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use super::{context_command_surface_label, theme_mode_next, theme_mode_toggle_label};
    use crate::app::{ContextCommandSurfacePreference, ThemeMode};

    #[test]
    fn theme_mode_cycles_system_to_light_to_dark_and_back() {
        assert_eq!(theme_mode_next(ThemeMode::System), ThemeMode::Light);
        assert_eq!(theme_mode_next(ThemeMode::Light), ThemeMode::Dark);
        assert_eq!(theme_mode_next(ThemeMode::Dark), ThemeMode::System);
    }

    #[test]
    fn theme_mode_toggle_labels_are_non_empty() {
        for mode in [ThemeMode::System, ThemeMode::Light, ThemeMode::Dark] {
            assert!(!theme_mode_toggle_label(mode).is_empty());
        }
    }

    #[test]
    fn context_command_surface_labels_match_palette_names() {
        assert_eq!(
            context_command_surface_label(ContextCommandSurfacePreference::RadialPalette),
            "Radial Palette"
        );
        assert_eq!(
            context_command_surface_label(ContextCommandSurfacePreference::ContextPalette),
            "Context Palette"
        );
    }
}
