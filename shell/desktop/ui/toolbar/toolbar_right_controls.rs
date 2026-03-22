use crate::app::{GraphBrowserApp, GraphIntent};
use crate::shell::desktop::host::running_app_state::RunningAppState;
use crate::shell::desktop::host::window::EmbedderWindow;
use crate::shell::desktop::runtime::registries::phase3_resolve_active_theme;
use egui::{WidgetInfo, WidgetType};

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
    // Sync status indicator
    render_sync_status_indicator(ui);

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

/// Render a simple sync status indicator showing Verse P2P status
fn render_sync_status_indicator(ui: &mut egui::Ui) {
    use crate::mods::verse;

    let theme_tokens = phase3_resolve_active_theme(None).tokens;
    // Check if Verse is available
    let (status_char, status_color, tooltip) = if !verse::is_initialized() {
        // Verse not available - show neutral dot
        (
            "○",
            theme_tokens.radial_chrome_text,
            "Sync: Not available".to_string(),
        )
    } else {
        let peers = crate::shell::desktop::runtime::registries::phase3_trusted_peers();
        if !peers.is_empty() {
            // Has peers - show success dot
            (
                "●",
                theme_tokens.status_success,
                format!(
                    "Sync: Connected ({} peer{})",
                    peers.len(),
                    if peers.len() == 1 { "" } else { "s" }
                ),
            )
        } else {
            // No peers yet - show notice dot
            (
                "○",
                theme_tokens.command_notice,
                "Sync: Ready (no peers)".to_string(),
            )
        }
    };

    ui.label(egui::RichText::new(status_char).color(status_color))
        .on_hover_text(tooltip);
}
