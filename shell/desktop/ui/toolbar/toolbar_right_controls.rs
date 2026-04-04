use crate::app::{GraphBrowserApp, GraphIntent};
use crate::shell::desktop::host::running_app_state::RunningAppState;
use crate::shell::desktop::host::window::EmbedderWindow;
use crate::shell::desktop::runtime::registries::phase3_resolve_active_theme;
use egui::{WidgetInfo, WidgetType};

pub(super) struct SyncStatusSummary {
    pub(super) label: String,
    pub(super) tooltip: String,
    pub(super) color: egui::Color32,
}

pub(super) fn render_toolbar_right_controls(
    ui: &mut egui::Ui,
    state: &RunningAppState,
    graph_app: &mut GraphBrowserApp,
    window: &EmbedderWindow,
    is_graph_view: bool,
    location_dirty: &mut bool,
    show_clear_data_confirm: &mut bool,
    frame_intents: &mut Vec<GraphIntent>,
    #[cfg(feature = "diagnostics")]
    diagnostics_state: &mut crate::shell::desktop::runtime::diagnostics::DiagnosticsState,
) {
    let fit_button = ui
        .add(super::toolbar_button("Fit"))
        .on_hover_text("Fit graph to screen");
    if fit_button.clicked() {
        frame_intents.push(GraphIntent::RequestFitToScreen);
    }

    ui.menu_button("Settings", |ui| {
        super::render_settings_menu(
            ui,
            graph_app,
            state,
            is_graph_view,
            frame_intents,
            location_dirty,
            window,
            #[cfg(feature = "diagnostics")]
            diagnostics_state,
        );
    });

    ui.menu_button("More", |ui| {
        let clear_data_button = ui.button("Clear graph and saved data");
        clear_data_button.widget_info(|| {
            let mut info = WidgetInfo::new(WidgetType::Button);
            info.label = Some("Clear graph and saved data".into());
            info
        });
        if clear_data_button.clicked() {
            *show_clear_data_confirm = true;
            ui.close();
        }
    });
}

pub(super) fn sync_status_summary() -> SyncStatusSummary {
    use crate::mods::verse;

    let theme_tokens = phase3_resolve_active_theme(None).tokens;
    let (label, color, tooltip) = if !verse::is_initialized() {
        (
            "Sync: unavailable".to_string(),
            theme_tokens.radial_chrome_text,
            "Sync: Not available".to_string(),
        )
    } else {
        let peers = crate::shell::desktop::runtime::registries::phase3_trusted_peers();
        if !peers.is_empty() {
            (
                format!(
                    "Sync: connected ({} peer{})",
                    peers.len(),
                    if peers.len() == 1 { "" } else { "s" }
                ),
                theme_tokens.status_success,
                format!(
                    "Sync: Connected ({} peer{})",
                    peers.len(),
                    if peers.len() == 1 { "" } else { "s" }
                ),
            )
        } else {
            (
                "Sync: ready (no peers)".to_string(),
                theme_tokens.command_notice,
                "Sync: Ready (no peers)".to_string(),
            )
        }
    };

    SyncStatusSummary {
        label,
        tooltip,
        color,
    }
}
