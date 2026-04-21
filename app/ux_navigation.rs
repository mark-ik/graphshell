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

    pub fn toggle_workbench_overlay(&mut self) {
        if self.workbench_overlay_visible() {
            self.close_workbench_overlay();
        } else {
            self.open_workbench_overlay();
        }
    }

    pub fn open_workbench_overlay(&mut self) {
        if self.workbench_overlay_visible()
            || matches!(
                self.workbench_display_mode(),
                WorkbenchDisplayMode::Dedicated
            )
        {
            return;
        }

        self.set_workbench_overlay_visible(true);
        self.emit_ux_navigation_transition();
    }

    pub fn close_workbench_overlay(&mut self) {
        if !self.workbench_overlay_visible() {
            return;
        }

        self.set_workbench_overlay_visible(false);
        self.emit_ux_navigation_transition();
    }

    pub fn toggle_scene_overlay(&mut self, preferred_view: Option<GraphViewId>) {
        if self.workspace.chrome_ui.show_scene_overlay {
            self.close_scene_overlay();
        } else {
            self.open_scene_overlay(preferred_view);
        }
    }

    pub fn open_scene_overlay(&mut self, preferred_view: Option<GraphViewId>) {
        let was_open = self.workspace.chrome_ui.show_scene_overlay;
        let resolved_view = preferred_view
            .filter(|view_id| self.workspace.graph_runtime.views.contains_key(view_id))
            .or_else(|| {
                self.workspace
                    .graph_runtime
                    .focused_view
                    .filter(|view_id| self.workspace.graph_runtime.views.contains_key(view_id))
            })
            .or_else(|| {
                (self.workspace.graph_runtime.views.len() == 1)
                    .then(|| self.workspace.graph_runtime.views.keys().next().copied())
                    .flatten()
            });
        self.workspace.chrome_ui.scene_overlay_view = resolved_view;
        if self.pending_transient_surface_return_target().is_none() {
            self.set_pending_transient_surface_return_target(
                resolved_view
                    .or(self.workspace.graph_runtime.focused_view)
                    .map(ToolSurfaceReturnTarget::Graph),
            );
        }
        self.workspace.chrome_ui.show_scene_overlay = true;
        self.workspace.chrome_ui.show_help_panel = false;
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
        }
    }

    pub fn close_scene_overlay(&mut self) {
        if !self.workspace.chrome_ui.show_scene_overlay {
            return;
        }
        self.workspace.chrome_ui.show_scene_overlay = false;
        if !self.workspace.chrome_ui.show_command_palette
            && !self.workspace.chrome_ui.show_context_palette
            && !self.workspace.chrome_ui.show_help_panel
            && !self.workspace.chrome_ui.show_settings_overlay
            && !self.workspace.chrome_ui.show_radial_menu
        {
            self.request_restore_transient_surface_focus();
        }
        self.emit_focus_capture_exit();
        self.emit_ux_navigation_transition();
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
        self.workspace.chrome_ui.show_scene_overlay = false;
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
            && !self.workspace.chrome_ui.show_scene_overlay
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
        self.workspace.chrome_ui.show_scene_overlay = false;
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
            && !self.workspace.chrome_ui.show_scene_overlay
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
        self.set_pending_frame_context_target(None);
        self.workspace.chrome_ui.surface_state = ActionSurfaceState::PaletteGlobal;
        self.set_command_surface_visibility(true, false);
    }

    pub fn open_context_palette(&mut self) {
        let scope = self.derive_scope_from_pending_targets();
        let anchor = self
            .workspace
            .chrome_ui
            .context_palette_anchor
            .map(Anchor::viewport_point)
            .unwrap_or(Anchor::ScreenCenter);
        self.workspace.chrome_ui.surface_state =
            ActionSurfaceState::PaletteContextual { scope, anchor };
        self.set_command_surface_visibility(false, true);
    }

    /// Open the contextual palette with an explicit scope and anchor.
    /// Preferred entry point for right-click handlers post-redesign;
    /// legacy `open_context_palette()` remains as a delegator.
    ///
    /// When `anchor` is a target variant, the legacy `[f32; 2]`
    /// anchor is left untouched so callers can supply a cursor
    /// fallback via `set_context_palette_anchor` for the render-site
    /// read that has not yet been migrated.
    pub fn open_palette_contextual(&mut self, scope: ActionScope, anchor: Anchor) {
        if let Some(point) = anchor.resolved_screen_point() {
            self.workspace.chrome_ui.context_palette_anchor = Some(point);
        }
        self.workspace.chrome_ui.surface_state =
            ActionSurfaceState::PaletteContextual { scope, anchor };
        self.set_command_surface_visibility(false, true);
    }

    /// Open the radial menu with an explicit scope and anchor. Preferred
    /// entry point for right-click/radial-trigger handlers.
    pub fn open_radial(&mut self, scope: ActionScope, anchor: Anchor) {
        self.workspace.chrome_ui.surface_state =
            ActionSurfaceState::Radial { scope, anchor };
        self.open_radial_menu();
    }

    /// Open the global palette (Ctrl+K). Preferred entry point.
    pub fn open_palette_global(&mut self) {
        self.open_command_palette();
    }

    /// Close whichever action surface is open. Idempotent.
    pub fn close_action_surface(&mut self) {
        match self.workspace.chrome_ui.surface_state.clone() {
            ActionSurfaceState::Closed => {}
            ActionSurfaceState::PaletteGlobal | ActionSurfaceState::PaletteContextual { .. } => {
                self.close_command_palette();
            }
            ActionSurfaceState::Radial { .. } => {
                self.close_radial_menu();
            }
        }
    }

    /// Derive `ActionScope` from the currently set
    /// `pending_*_context_target` values. Used by the legacy
    /// `open_context_palette` entry to populate `surface_state`
    /// without changing call-site signatures.
    fn derive_scope_from_pending_targets(&self) -> ActionScope {
        let target = if let Some(node) = self.pending_node_context_target() {
            ScopeTarget::Node(node)
        } else if let Some(frame) = self.pending_frame_context_target() {
            ScopeTarget::Frame(frame.to_string())
        } else {
            ScopeTarget::None
        };
        match self.workspace.graph_runtime.focused_view {
            Some(view_id) => ActionScope::Graph { view_id, target },
            None => ActionScope::Global,
        }
    }

    pub fn set_context_palette_anchor(&mut self, anchor: Option<[f32; 2]>) {
        self.workspace.chrome_ui.context_palette_anchor = anchor;
    }

    pub fn close_command_palette(&mut self) {
        self.workspace.chrome_ui.context_palette_anchor = None;
        self.workspace.chrome_ui.surface_state = ActionSurfaceState::Closed;
        self.set_command_surface_visibility(false, false);
    }

    pub fn toggle_command_palette(&mut self) {
        if self.workspace.chrome_ui.show_command_palette {
            self.workspace.chrome_ui.context_palette_anchor = None;
            self.workspace.chrome_ui.surface_state = ActionSurfaceState::Closed;
            self.set_command_surface_visibility(false, false);
        } else {
            self.workspace.chrome_ui.context_palette_anchor = None;
            self.set_pending_node_context_target(None);
            self.set_pending_frame_context_target(None);
            self.workspace.chrome_ui.surface_state = ActionSurfaceState::PaletteGlobal;
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
        self.workspace.chrome_ui.show_scene_overlay = false;
        self.workspace.chrome_ui.show_settings_overlay = false;
        self.workspace.chrome_ui.show_command_palette = false;
        self.workspace.chrome_ui.show_context_palette = false;
        self.workspace.chrome_ui.command_palette_contextual_mode = false;
        self.workspace.chrome_ui.context_palette_anchor = None;
        self.workspace.chrome_ui.show_radial_menu = true;
        // If `open_radial(scope, anchor)` already populated the
        // enum, preserve it; otherwise fall back to a scopeless
        // radial (legacy F3 toggle with no explicit target).
        if !matches!(
            self.workspace.chrome_ui.surface_state,
            ActionSurfaceState::Radial { .. }
        ) {
            self.workspace.chrome_ui.surface_state = ActionSurfaceState::Radial {
                scope: self.derive_scope_from_pending_targets(),
                anchor: Anchor::ScreenCenter,
            };
        }
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
        self.workspace.chrome_ui.surface_state = ActionSurfaceState::Closed;
        self.set_pending_node_context_target(None);
        self.set_pending_frame_context_target(None);
        if !self.workspace.chrome_ui.show_command_palette
            && !self.workspace.chrome_ui.show_context_palette
            && !self.workspace.chrome_ui.show_scene_overlay
            && !self.workspace.chrome_ui.show_help_panel
            && !self.workspace.chrome_ui.show_settings_overlay
        {
            self.request_restore_transient_surface_focus();
        }
        self.emit_focus_capture_exit();
        self.emit_ux_navigation_transition();
    }

    /// Close an action surface whose stored scope targets `removed`.
    /// Called from node-deletion hooks so a palette/radial opened on
    /// a now-gone node doesn't linger.
    pub fn close_action_surface_if_targets_node(&mut self, removed: NodeKey) {
        if self.workspace.chrome_ui.surface_state.targets_node(removed) {
            self.close_action_surface();
        }
    }

    /// Close any graph-scoped action surface. Called from `clear_graph`.
    pub fn close_action_surface_if_graph_scoped(&mut self) {
        if self.workspace.chrome_ui.surface_state.is_graph_scoped() {
            self.close_action_surface();
        }
    }

    /// Close an action surface whose scope belongs to a different
    /// graph view than `current`. Called from focus-change hooks.
    pub fn close_action_surface_if_in_other_view(&mut self, current: GraphViewId) {
        if self
            .workspace
            .chrome_ui
            .surface_state
            .is_in_other_view(current)
        {
            self.close_action_surface();
        }
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
            self.workspace.chrome_ui.show_scene_overlay = false;
            self.workspace.chrome_ui.show_settings_overlay = false;
            self.workspace.chrome_ui.show_radial_menu = false;
            self.close_clip_inspector();
            self.emit_focus_capture_enter();
        } else {
            self.set_pending_node_context_target(None);
            self.set_pending_frame_context_target(None);
            self.workspace.chrome_ui.command_palette_contextual_mode = false;
            self.emit_focus_capture_exit();
        }
        self.emit_ux_navigation_transition();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::GraphViewState;

    #[test]
    fn open_scene_overlay_targets_requested_graph_view() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_id = GraphViewId::new();
        app.workspace
            .graph_runtime
            .views
            .insert(view_id, GraphViewState::new_with_id(view_id, "Scene"));
        app.workspace.graph_runtime.focused_view = Some(view_id);

        app.open_scene_overlay(Some(view_id));

        assert!(app.workspace.chrome_ui.show_scene_overlay);
        assert_eq!(app.workspace.chrome_ui.scene_overlay_view, Some(view_id));
        assert_eq!(
            app.pending_transient_surface_return_target(),
            Some(ToolSurfaceReturnTarget::Graph(view_id))
        );
    }

    #[test]
    fn open_scene_overlay_closes_other_transient_surfaces() {
        let mut app = GraphBrowserApp::new_for_testing();
        let view_id = GraphViewId::new();
        app.workspace
            .graph_runtime
            .views
            .insert(view_id, GraphViewState::new_with_id(view_id, "Scene"));
        app.workspace.graph_runtime.focused_view = Some(view_id);
        app.workspace.chrome_ui.show_help_panel = true;
        app.workspace.chrome_ui.show_settings_overlay = true;
        app.workspace.chrome_ui.show_radial_menu = true;

        app.open_scene_overlay(Some(view_id));

        assert!(app.workspace.chrome_ui.show_scene_overlay);
        assert!(!app.workspace.chrome_ui.show_help_panel);
        assert!(!app.workspace.chrome_ui.show_settings_overlay);
        assert!(!app.workspace.chrome_ui.show_radial_menu);
    }

    #[test]
    fn toggle_workbench_overlay_round_trips_visibility() {
        let mut app = GraphBrowserApp::new_for_testing();

        app.toggle_workbench_overlay();
        assert!(app.workbench_overlay_visible());

        app.toggle_workbench_overlay();
        assert!(!app.workbench_overlay_visible());
    }

    #[test]
    fn dedicated_workbench_mode_blocks_overlay_open() {
        let mut app = GraphBrowserApp::new_for_testing();
        app.set_workbench_display_mode(WorkbenchDisplayMode::Dedicated);

        app.open_workbench_overlay();

        assert!(!app.workbench_overlay_visible());
    }
}
