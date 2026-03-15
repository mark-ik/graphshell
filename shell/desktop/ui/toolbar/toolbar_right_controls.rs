use crate::app::{GraphBrowserApp, GraphIntent, WorkbenchIntent};
use crate::shell::desktop::host::running_app_state::RunningAppState;
use crate::shell::desktop::host::window::EmbedderWindow;
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

    let command_button = ui
        .add(super::toolbar_button("Cmd"))
        .on_hover_text("Open command palette (F2)");
    if command_button.clicked() {
        graph_app.enqueue_workbench_intent(WorkbenchIntent::ToggleCommandPalette);
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

    let clear_data_button = ui
        .add(super::toolbar_button("Clr"))
        .on_hover_text("Clear graph and saved data");
    clear_data_button.widget_info(|| {
        let mut info = WidgetInfo::new(WidgetType::Button);
        info.label = Some("Clear graph and saved data".into());
        info
    });
    if clear_data_button.clicked() {
        *show_clear_data_confirm = true;
    }
}

/// Render a simple sync status indicator showing Verse P2P status
fn render_sync_status_indicator(ui: &mut egui::Ui) {
    use crate::mods::verse;

    // Check if Verse is available
    let (status_char, status_color, tooltip) = if !verse::is_initialized() {
        // Verse not available - show gray dot
        (
            "○",
            egui::Color32::from_rgb(128, 128, 128),
            "Sync: Not available".to_string(),
        )
    } else {
        let peers = crate::shell::desktop::runtime::registries::phase3_trusted_peers();
        if !peers.is_empty() {
            // Has peers - show green dot
            (
                "●",
                egui::Color32::from_rgb(0, 200, 0),
                format!(
                    "Sync: Connected ({} peer{})",
                    peers.len(),
                    if peers.len() == 1 { "" } else { "s" }
                ),
            )
        } else {
            // No peers - show yellow dot
            (
                "○",
                egui::Color32::from_rgb(200, 200, 0),
                "Sync: Ready (no peers)".to_string(),
            )
        }
    };

    ui.label(egui::RichText::new(status_char).color(status_color))
        .on_hover_text(tooltip);
}
