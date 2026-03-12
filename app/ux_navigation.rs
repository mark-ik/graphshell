use super::*;
use crate::shell::desktop::runtime::registries::{
    CHANNEL_UX_FOCUS_CAPTURE_ENTER, CHANNEL_UX_FOCUS_CAPTURE_EXIT,
};

impl GraphBrowserApp {
    pub(crate) fn emit_ux_navigation_transition(&self) {
        emit_event(DiagnosticEvent::MessageReceived {
            channel_id: CHANNEL_UX_NAVIGATION_TRANSITION,
            latency_us: 0,
        });
    }

    fn emit_focus_capture_enter(&self) {
        emit_event(DiagnosticEvent::MessageReceived {
            channel_id: CHANNEL_UX_FOCUS_CAPTURE_ENTER,
            latency_us: 0,
        });
    }

    fn emit_focus_capture_exit(&self) {
        emit_event(DiagnosticEvent::MessageReceived {
            channel_id: CHANNEL_UX_FOCUS_CAPTURE_EXIT,
            latency_us: 0,
        });
    }

    pub fn toggle_help_panel(&mut self) {
        if self.workspace.show_help_panel {
            self.close_help_panel();
        } else {
            self.open_help_panel();
        }
    }

    pub fn open_help_panel(&mut self) {
        let was_open = self.workspace.show_help_panel;
        if self.pending_transient_surface_return_target().is_none() {
            self.set_pending_transient_surface_return_target(
                self.workspace
                    .focused_view
                    .map(ToolSurfaceReturnTarget::Graph),
            );
        }
        self.workspace.show_help_panel = true;
        self.workspace.show_command_palette = false;
        self.workspace.command_palette_contextual_mode = false;
        self.workspace.show_radial_menu = false;
        self.set_pending_node_context_target(None);
        if !was_open {
            self.emit_focus_capture_enter();
            self.emit_ux_navigation_transition();
        }
    }

    pub fn close_help_panel(&mut self) {
        if !self.workspace.show_help_panel {
            return;
        }
        self.workspace.show_help_panel = false;
        if !self.workspace.show_command_palette && !self.workspace.show_radial_menu {
            self.request_restore_transient_surface_focus();
        }
        self.emit_focus_capture_exit();
        self.emit_ux_navigation_transition();
    }

    pub fn open_command_palette(&mut self) {
        self.workspace.command_palette_contextual_mode = false;
        self.set_pending_node_context_target(None);
        self.set_command_palette_visibility(true);
    }

    pub fn open_context_palette(&mut self) {
        self.workspace.command_palette_contextual_mode = true;
        self.set_command_palette_visibility(true);
    }

    pub fn close_command_palette(&mut self) {
        self.set_command_palette_visibility(false);
    }

    pub fn toggle_command_palette(&mut self) {
        if self.workspace.show_command_palette {
            self.set_command_palette_visibility(false);
        } else {
            self.workspace.command_palette_contextual_mode = false;
            self.set_pending_node_context_target(None);
            self.set_command_palette_visibility(true);
        }
    }

    pub fn toggle_radial_menu(&mut self) {
        if self.workspace.show_radial_menu {
            self.close_radial_menu();
        } else {
            self.open_radial_menu();
        }
    }

    pub fn open_radial_menu(&mut self) {
        let was_open = self.workspace.show_radial_menu;
        self.workspace.show_help_panel = false;
        self.workspace.show_command_palette = false;
        self.workspace.command_palette_contextual_mode = false;
        self.workspace.show_radial_menu = true;
        if !was_open {
            self.emit_focus_capture_enter();
            self.emit_ux_navigation_transition();
        }
    }

    pub fn close_radial_menu(&mut self) {
        if !self.workspace.show_radial_menu {
            return;
        }
        self.workspace.show_radial_menu = false;
        self.set_pending_node_context_target(None);
        if !self.workspace.show_command_palette && !self.workspace.show_help_panel {
            self.request_restore_transient_surface_focus();
        }
        self.emit_focus_capture_exit();
        self.emit_ux_navigation_transition();
    }

    fn set_command_palette_visibility(&mut self, open: bool) {
        if self.workspace.show_command_palette == open {
            return;
        }

        self.workspace.show_command_palette = open;
        if open {
            self.workspace.show_help_panel = false;
            self.workspace.show_radial_menu = false;
            self.emit_focus_capture_enter();
        } else {
            self.set_pending_node_context_target(None);
            self.workspace.command_palette_contextual_mode = false;
            self.emit_focus_capture_exit();
        }
        self.emit_ux_navigation_transition();
    }
}
