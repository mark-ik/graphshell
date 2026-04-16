use super::*;

impl<'a> GraphshellTileBehavior<'a> {
    #[cfg(feature = "diagnostics")]
    pub(super) fn render_tool_pane(
        &mut self,
        ui: &mut egui::Ui,
        tool: &crate::shell::desktop::workbench::pane_model::ToolPaneRef,
    ) {
        use crate::shell::desktop::workbench::pane_model::ToolPaneState;

        match &tool.kind {
            ToolPaneState::Diagnostics => {
                let signal_trace =
                    crate::shell::desktop::runtime::registries::phase3_signal_trace_snapshot();
                self.diagnostics_state.render_in_pane(
                    ui,
                    self.graph_app,
                    self.runtime_focus_inspector.as_ref(),
                    &signal_trace,
                );
            }
            ToolPaneState::HistoryManager => {
                let intents = render::render_history_manager_in_ui(ui, self.graph_app);
                self.extend_post_render_intents(intents);
            }
            ToolPaneState::AccessibilityInspector => {
                Self::render_accessibility_inspector_scaffold(ui, self.graph_app);
            }
            ToolPaneState::FileTree => {
                let intents = render::render_navigator_tool_pane_in_ui(ui, self.graph_app);
                self.extend_post_render_intents(intents);
            }
            ToolPaneState::Settings => {
                let intents = render::render_settings_tool_pane_in_ui_with_control_panel(
                    ui,
                    self.graph_app,
                    Some(self.control_panel),
                );
                self.extend_post_render_intents(intents);
            }
        }
    }
}
