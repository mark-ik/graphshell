use crate::app::{GraphBrowserApp, GraphIntent};
use crate::shell::desktop::host::running_app_state::RunningAppState;
use crate::shell::desktop::host::window::EmbedderWindow;
use crate::shell::desktop::ui::toolbar::toolbar_ui::CommandBarFocusTarget;
use egui::{WidgetInfo, WidgetType};

pub(super) fn render_toolbar_right_controls(
    ui: &mut egui::Ui,
    state: &RunningAppState,
    graph_app: &mut GraphBrowserApp,
    window: &EmbedderWindow,
    command_bar_focus_target: CommandBarFocusTarget,
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
            command_bar_focus_target,
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
