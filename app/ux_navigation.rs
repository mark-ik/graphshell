use super::*;

impl GraphBrowserApp {
    pub(crate) fn emit_ux_navigation_transition(&self) {
        emit_event(DiagnosticEvent::MessageReceived {
            channel_id: CHANNEL_UX_NAVIGATION_TRANSITION,
            latency_us: 0,
        });
    }

    pub fn toggle_help_panel(&mut self) {
        self.workspace.show_help_panel = !self.workspace.show_help_panel;
        if self.workspace.show_help_panel {
            self.workspace.show_command_palette = false;
            self.workspace.show_radial_menu = false;
            self.set_pending_node_context_target(None);
        }
        self.emit_ux_navigation_transition();
    }

    pub fn toggle_command_palette(&mut self) {
        self.workspace.show_command_palette = !self.workspace.show_command_palette;
        if self.workspace.show_command_palette {
            self.workspace.show_help_panel = false;
            self.workspace.show_radial_menu = false;
            self.set_pending_node_context_target(None);
        }
        self.emit_ux_navigation_transition();
    }

    pub fn toggle_radial_menu(&mut self) {
        self.workspace.show_radial_menu = !self.workspace.show_radial_menu;
        if self.workspace.show_radial_menu {
            self.workspace.show_help_panel = false;
            self.workspace.show_command_palette = false;
        } else {
            self.set_pending_node_context_target(None);
        }
        self.emit_ux_navigation_transition();
    }
}