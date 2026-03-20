use super::*;
use crate::shell::desktop::runtime::registries::{
    CHANNEL_UX_ARRANGEMENT_DURABILITY_TRANSITION, CHANNEL_UX_ARRANGEMENT_MISSING_FAMILY_FALLBACK,
    CHANNEL_UX_ARRANGEMENT_PROJECTION_HEALTH, CHANNEL_UX_FOCUS_CAPTURE_ENTER,
    CHANNEL_UX_FOCUS_CAPTURE_EXIT,
};

impl GraphBrowserApp {
    pub(crate) fn emit_ux_navigation_transition(&self) {
        emit_event(DiagnosticEvent::MessageReceived {
            channel_id: CHANNEL_UX_NAVIGATION_TRANSITION,
            latency_us: 0,
        });
    }

    pub(crate) fn emit_arrangement_projection_health(&self) {
        emit_event(DiagnosticEvent::MessageReceived {
            channel_id: CHANNEL_UX_ARRANGEMENT_PROJECTION_HEALTH,
            latency_us: 0,
        });
    }

    pub(crate) fn emit_arrangement_missing_family_fallback(&self) {
        emit_event(DiagnosticEvent::MessageReceived {
            channel_id: CHANNEL_UX_ARRANGEMENT_MISSING_FAMILY_FALLBACK,
            latency_us: 0,
        });
    }

    pub(crate) fn emit_arrangement_durability_transition(&self) {
        emit_event(DiagnosticEvent::MessageReceived {
            channel_id: CHANNEL_UX_ARRANGEMENT_DURABILITY_TRANSITION,
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
        if self.workspace.chrome_ui.show_help_panel {
            self.close_help_panel();
        } else {
            self.open_help_panel();
        }
    }

    pub fn open_help_panel(&mut self) {
        let was_open = self.workspace.chrome_ui.show_help_panel;
        if self.pending_transient_surface_return_target().is_none() {
            self.set_pending_transient_surface_return_target(
                self.workspace
                    .graph_runtime
                    .focused_view
                    .map(ToolSurfaceReturnTarget::Graph),
            );
        }
        self.workspace.chrome_ui.show_help_panel = true;
        self.workspace.chrome_ui.show_settings_overlay = false;
        self.workspace.chrome_ui.show_command_palette = false;
        self.workspace.chrome_ui.show_context_palette = false;
        self.workspace.chrome_ui.command_palette_contextual_mode = false;
        self.workspace.chrome_ui.show_radial_menu = false;
        self.close_clip_inspector();
        self.set_pending_node_context_target(None);
        if !was_open {
            self.emit_focus_capture_enter();
            self.emit_ux_navigation_transition();
            crate::shell::desktop::runtime::registries::phase3_publish_workbench_projection_refresh_requested("settings_overlay_opened");
        }
    }

    pub fn close_help_panel(&mut self) {
        if !self.workspace.chrome_ui.show_help_panel {
            return;
        }
        self.workspace.chrome_ui.show_help_panel = false;
        if !self.workspace.chrome_ui.show_command_palette
            && !self.workspace.chrome_ui.show_context_palette
            && !self.workspace.chrome_ui.show_radial_menu
            && !self.workspace.chrome_ui.show_settings_overlay
        {
            self.request_restore_transient_surface_focus();
        }
        self.emit_focus_capture_exit();
        self.emit_ux_navigation_transition();
        crate::shell::desktop::runtime::registries::phase3_publish_workbench_projection_refresh_requested("settings_overlay_closed");
    }

    pub fn open_settings_overlay(&mut self, page: SettingsToolPage) {
        let was_open = self.workspace.chrome_ui.show_settings_overlay;
        self.workspace.chrome_ui.settings_tool_page = page;
        if self.pending_transient_surface_return_target().is_none() {
            self.set_pending_transient_surface_return_target(
                self.workspace
                    .graph_runtime
                    .focused_view
                    .map(ToolSurfaceReturnTarget::Graph),
            );
        }
        self.workspace.chrome_ui.show_settings_overlay = true;
        self.workspace.chrome_ui.show_help_panel = false;
        self.workspace.chrome_ui.show_command_palette = false;
        self.workspace.chrome_ui.show_context_palette = false;
        self.workspace.chrome_ui.command_palette_contextual_mode = false;
        self.workspace.chrome_ui.show_radial_menu = false;
        self.close_clip_inspector();
        self.set_pending_node_context_target(None);
        if !was_open {
            self.emit_focus_capture_enter();
            self.emit_ux_navigation_transition();
        }
    }

    pub fn close_settings_overlay(&mut self) {
        if !self.workspace.chrome_ui.show_settings_overlay {
            return;
        }
        self.workspace.chrome_ui.show_settings_overlay = false;
        if !self.workspace.chrome_ui.show_command_palette
            && !self.workspace.chrome_ui.show_context_palette
            && !self.workspace.chrome_ui.show_help_panel
            && !self.workspace.chrome_ui.show_radial_menu
        {
            self.request_restore_transient_surface_focus();
        }
        self.emit_focus_capture_exit();
        self.emit_ux_navigation_transition();
    }

    pub fn open_command_palette(&mut self) {
        self.workspace.chrome_ui.context_palette_anchor = None;
        self.set_pending_node_context_target(None);
        self.set_command_surface_visibility(true, false);
    }

    pub fn open_context_palette(&mut self) {
        self.set_command_surface_visibility(false, true);
    }

    pub fn set_context_palette_anchor(&mut self, anchor: Option<[f32; 2]>) {
        self.workspace.chrome_ui.context_palette_anchor = anchor;
    }

    pub fn close_command_palette(&mut self) {
        self.workspace.chrome_ui.context_palette_anchor = None;
        self.set_command_surface_visibility(false, false);
    }

    pub fn toggle_command_palette(&mut self) {
        if self.workspace.chrome_ui.show_command_palette {
            self.workspace.chrome_ui.context_palette_anchor = None;
            self.set_command_surface_visibility(false, false);
        } else {
            self.workspace.chrome_ui.context_palette_anchor = None;
            self.set_pending_node_context_target(None);
            self.set_command_surface_visibility(true, false);
        }
    }

    pub fn toggle_radial_menu(&mut self) {
        if self.workspace.chrome_ui.show_radial_menu {
            self.close_radial_menu();
        } else {
            self.open_radial_menu();
        }
    }

    pub fn open_radial_menu(&mut self) {
        let was_open = self.workspace.chrome_ui.show_radial_menu;
        self.workspace.chrome_ui.show_help_panel = false;
        self.workspace.chrome_ui.show_settings_overlay = false;
        self.workspace.chrome_ui.show_command_palette = false;
        self.workspace.chrome_ui.show_context_palette = false;
        self.workspace.chrome_ui.command_palette_contextual_mode = false;
        self.workspace.chrome_ui.context_palette_anchor = None;
        self.workspace.chrome_ui.show_radial_menu = true;
        self.close_clip_inspector();
        if !was_open {
            self.emit_focus_capture_enter();
            self.emit_ux_navigation_transition();
        }
    }

    pub fn close_radial_menu(&mut self) {
        if !self.workspace.chrome_ui.show_radial_menu {
            return;
        }
        self.workspace.chrome_ui.show_radial_menu = false;
        self.set_pending_node_context_target(None);
        if !self.workspace.chrome_ui.show_command_palette
            && !self.workspace.chrome_ui.show_context_palette
            && !self.workspace.chrome_ui.show_help_panel
            && !self.workspace.chrome_ui.show_settings_overlay
        {
            self.request_restore_transient_surface_focus();
        }
        self.emit_focus_capture_exit();
        self.emit_ux_navigation_transition();
    }

    fn set_command_surface_visibility(
        &mut self,
        show_command_palette: bool,
        show_context_palette: bool,
    ) {
        if self.workspace.chrome_ui.show_command_palette == show_command_palette
            && self.workspace.chrome_ui.show_context_palette == show_context_palette
        {
            return;
        }

        self.workspace.chrome_ui.show_command_palette = show_command_palette;
        self.workspace.chrome_ui.show_context_palette = show_context_palette;
        self.workspace.chrome_ui.command_palette_contextual_mode = show_context_palette;
        if show_command_palette || show_context_palette {
            self.workspace.chrome_ui.show_help_panel = false;
            self.workspace.chrome_ui.show_settings_overlay = false;
            self.workspace.chrome_ui.show_radial_menu = false;
            self.close_clip_inspector();
            self.emit_focus_capture_enter();
        } else {
            self.set_pending_node_context_target(None);
            self.workspace.chrome_ui.command_palette_contextual_mode = false;
            self.emit_focus_capture_exit();
        }
        self.emit_ux_navigation_transition();
    }
}
