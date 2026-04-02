use std::path::Path;

use super::*;

/// User preference for how the application theme is selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeMode {
    /// Follow the OS dark/light preference (default).
    #[default]
    System,
    /// Always use the light theme.
    Light,
    /// Always use the dark theme.
    Dark,
}

impl_display_from_str!(ThemeMode {
    ThemeMode::System => "system",
    ThemeMode::Light => "light",
    ThemeMode::Dark => "dark",
});

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DefaultWebViewerBackend {
    #[default]
    Servo,
    Wry,
}

impl_display_from_str!(DefaultWebViewerBackend {
    DefaultWebViewerBackend::Servo => "viewer:webview",
    DefaultWebViewerBackend::Wry => "viewer:wry",
});

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WryRenderModePreference {
    #[default]
    Auto,
    ForceOverlay,
    ForceTexture,
}

impl_display_from_str!(WryRenderModePreference {
    WryRenderModePreference::Auto => "auto",
    WryRenderModePreference::ForceOverlay => "force_overlay",
    WryRenderModePreference::ForceTexture => "force_texture",
});

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SettingsToolPage {
    #[default]
    General,
    Persistence,
    Physics,
    Sync,
    Appearance,
    Keybindings,
    Advanced,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct WorkspaceUserStylesheetSetting {
    pub path: String,
    pub enabled: bool,
}

impl GraphBrowserApp {
    pub(crate) const SETTINGS_DEFAULT_WEB_VIEWER_BACKEND_NAME: &str =
        "settings.default_web_viewer_backend";
    pub(crate) const SETTINGS_WRY_RENDER_MODE_PREFERENCE_NAME: &str =
        "settings.wry_render_mode_preference";
    pub(crate) const SETTINGS_WORKSPACE_USER_STYLESHEETS_NAME: &str =
        "settings.workspace_user_stylesheets";
    pub fn is_reserved_workspace_layout_name(name: &str) -> bool {
        name == "latest"
            || name == Self::SESSION_WORKSPACE_LAYOUT_NAME
            || name == Self::WORKSPACE_PIN_WORKSPACE_NAME
            || name == Self::WORKSPACE_PIN_PANE_NAME
            || name == Self::SETTINGS_TOAST_ANCHOR_NAME
            || name == Self::SETTINGS_COMMAND_PALETTE_SHORTCUT_NAME
            || name == Self::SETTINGS_HELP_PANEL_SHORTCUT_NAME
            || name == Self::SETTINGS_RADIAL_MENU_SHORTCUT_NAME
            || name == Self::SETTINGS_CONTEXT_COMMAND_SURFACE_NAME
            || name == Self::SETTINGS_KEYBOARD_PAN_STEP_NAME
            || name == Self::SETTINGS_KEYBOARD_PAN_INPUT_MODE_NAME
            || name == Self::SETTINGS_CAMERA_PAN_INERTIA_ENABLED_NAME
            || name == Self::SETTINGS_CAMERA_PAN_INERTIA_DAMPING_NAME
            || name == Self::SETTINGS_LASSO_BINDING_NAME
            || name == Self::SETTINGS_INPUT_BINDING_REMAPS_NAME
            || name == Self::SETTINGS_OMNIBAR_PREFERRED_SCOPE_NAME
            || name == Self::SETTINGS_OMNIBAR_NON_AT_ORDER_NAME
            || name == Self::SETTINGS_WRY_ENABLED_NAME
            || name == Self::SETTINGS_DEFAULT_WEB_VIEWER_BACKEND_NAME
            || name == Self::SETTINGS_WRY_RENDER_MODE_PREFERENCE_NAME
            || name == Self::SETTINGS_WORKSPACE_USER_STYLESHEETS_NAME
            || name == Self::SETTINGS_WEBVIEW_PREVIEW_ACTIVE_REFRESH_SECS_NAME
            || name == Self::SETTINGS_WEBVIEW_PREVIEW_WARM_REFRESH_SECS_NAME
            || name == Self::SETTINGS_WORKBENCH_HOST_PINNED_NAME
            || name == Self::SETTINGS_WORKBENCH_PROFILE_STATE_NAME
            || name == Self::SETTINGS_WORKBENCH_SURFACE_PROFILE_ID_NAME
            || name == Self::SETTINGS_CANVAS_PROFILE_ID_NAME
            || name == Self::SETTINGS_ACTIVE_WORKFLOW_ID_NAME
            || name == Self::SETTINGS_NOSTR_SIGNER_SETTINGS_NAME
            || name == Self::SETTINGS_NOSTR_NIP07_PERMISSIONS_NAME
            || name == Self::SETTINGS_NOSTR_SUBSCRIPTIONS_NAME
            || name == Self::SETTINGS_GRAPH_VIEW_LAYOUT_MANAGER_NAME
            || name.starts_with(Self::SETTINGS_DIAGNOSTICS_CHANNEL_CONFIG_PREFIX)
            || name.starts_with(Self::SESSION_WORKSPACE_PREV_PREFIX)
    }

    pub fn set_toast_anchor_preference(&mut self, preference: ToastAnchorPreference) {
        self.workspace.chrome_ui.toast_anchor_preference = preference;
        self.save_toast_anchor_preference();
    }

    fn save_toast_anchor_preference(&mut self) {
        self.save_workspace_layout_json(
            Self::SETTINGS_TOAST_ANCHOR_NAME,
            &self.workspace.chrome_ui.toast_anchor_preference.to_string(),
        );
    }

    pub fn set_command_palette_shortcut(&mut self, shortcut: CommandPaletteShortcut) {
        self.workspace.chrome_ui.command_palette_shortcut = shortcut;
        self.save_command_palette_shortcut();
    }

    fn save_command_palette_shortcut(&mut self) {
        self.save_workspace_layout_json(
            Self::SETTINGS_COMMAND_PALETTE_SHORTCUT_NAME,
            &self
                .workspace
                .chrome_ui
                .command_palette_shortcut
                .to_string(),
        );
    }

    pub fn set_help_panel_shortcut(&mut self, shortcut: HelpPanelShortcut) {
        self.workspace.chrome_ui.help_panel_shortcut = shortcut;
        self.save_help_panel_shortcut();
    }

    fn save_help_panel_shortcut(&mut self) {
        self.save_workspace_layout_json(
            Self::SETTINGS_HELP_PANEL_SHORTCUT_NAME,
            &self.workspace.chrome_ui.help_panel_shortcut.to_string(),
        );
    }

    pub fn set_radial_menu_shortcut(&mut self, shortcut: RadialMenuShortcut) {
        self.workspace.chrome_ui.radial_menu_shortcut = shortcut;
        self.save_radial_menu_shortcut();
    }

    pub fn context_command_surface_preference(&self) -> ContextCommandSurfacePreference {
        self.workspace.chrome_ui.context_command_surface_preference
    }

    pub fn set_context_command_surface_preference(
        &mut self,
        preference: ContextCommandSurfacePreference,
    ) {
        self.workspace.chrome_ui.context_command_surface_preference = preference;
        self.save_context_command_surface_preference();
    }

    pub fn keyboard_pan_step(&self) -> f32 {
        self.workspace.chrome_ui.keyboard_pan_step
    }

    pub fn set_keyboard_pan_step(&mut self, step: f32) {
        let normalized = step.clamp(1.0, 200.0);
        self.workspace.chrome_ui.keyboard_pan_step = normalized;
        crate::shell::desktop::runtime::registries::phase3_set_active_canvas_keyboard_pan_step(
            normalized,
        );
        self.save_keyboard_pan_step();
    }

    pub fn keyboard_pan_input_mode(&self) -> KeyboardPanInputMode {
        self.workspace.chrome_ui.keyboard_pan_input_mode
    }

    pub fn set_keyboard_pan_input_mode(&mut self, mode: KeyboardPanInputMode) {
        self.workspace.chrome_ui.keyboard_pan_input_mode = mode;
        self.save_keyboard_pan_input_mode();
    }

    pub fn camera_pan_inertia_enabled(&self) -> bool {
        self.workspace.chrome_ui.camera_pan_inertia_enabled
    }

    pub fn set_camera_pan_inertia_enabled(&mut self, enabled: bool) {
        self.workspace.chrome_ui.camera_pan_inertia_enabled = enabled;
        self.save_camera_pan_inertia_enabled();
    }

    pub fn camera_pan_inertia_damping(&self) -> f32 {
        self.workspace.chrome_ui.camera_pan_inertia_damping
    }

    pub fn set_camera_pan_inertia_damping(&mut self, damping: f32) {
        let normalized = damping.clamp(0.70, 0.99);
        self.workspace.chrome_ui.camera_pan_inertia_damping = normalized;
        self.save_camera_pan_inertia_damping();
    }

    pub fn lasso_binding_preference(&self) -> CanvasLassoBinding {
        crate::shell::desktop::runtime::registries::phase3_resolve_active_canvas_profile()
            .profile
            .interaction
            .lasso_binding
    }

    pub fn set_lasso_binding_preference(&mut self, binding: CanvasLassoBinding) {
        self.workspace.chrome_ui.lasso_binding_preference = binding;
        crate::shell::desktop::runtime::registries::phase3_set_active_canvas_lasso_binding(binding);
        self.save_lasso_binding_preference();
    }

    pub fn set_input_binding_remaps(
        &mut self,
        remaps: &[InputBindingRemap],
    ) -> Result<(), InputRemapConflict> {
        phase2_apply_input_binding_remaps(remaps)?;
        self.save_input_binding_remaps(remaps);
        Ok(())
    }

    pub fn input_binding_remaps(&self) -> Vec<InputBindingRemap> {
        self.load_workspace_layout_json(Self::SETTINGS_INPUT_BINDING_REMAPS_NAME)
            .and_then(|raw| Self::decode_input_binding_remaps(&raw).ok())
            .unwrap_or_default()
    }

    pub fn set_input_binding_for_action(
        &mut self,
        action_id: &str,
        context: InputContext,
        binding: InputBinding,
    ) -> Result<(), InputRemapConflict> {
        let mut remaps = self.input_binding_remaps();
        remaps.retain(|remap| {
            let descriptor = phase2_describe_input_bindings()
                .into_iter()
                .find(|entry| entry.action_id == action_id && entry.context == context);
            descriptor.as_ref().is_none_or(|entry| {
                entry
                    .default_binding
                    .as_ref()
                    .is_none_or(|default_binding| {
                        !(remap.context == context && remap.old == *default_binding)
                    })
            })
        });

        if let Some(descriptor) = phase2_describe_input_bindings()
            .into_iter()
            .find(|entry| entry.action_id == action_id && entry.context == context)
            && let Some(default_binding) = descriptor.default_binding
            && binding != default_binding
        {
            remaps.push(InputBindingRemap {
                old: default_binding,
                new: binding,
                context,
            });
        }

        self.set_input_binding_remaps(&remaps)
    }

    pub fn reset_input_binding_for_action(&mut self, action_id: &str, context: InputContext) {
        let descriptors = phase2_describe_input_bindings();
        let Some(default_binding) = descriptors
            .iter()
            .find(|entry| entry.action_id == action_id && entry.context == context)
            .and_then(|entry| entry.default_binding.clone())
        else {
            return;
        };

        let mut remaps = self.input_binding_remaps();
        remaps.retain(|remap| !(remap.context == context && remap.old == default_binding));
        if let Err(error) = self.set_input_binding_remaps(&remaps) {
            warn!("failed to reset input binding for action '{action_id}': {error:?}");
        }
    }

    fn save_radial_menu_shortcut(&mut self) {
        self.save_workspace_layout_json(
            Self::SETTINGS_RADIAL_MENU_SHORTCUT_NAME,
            &self.workspace.chrome_ui.radial_menu_shortcut.to_string(),
        );
    }

    fn save_context_command_surface_preference(&mut self) {
        self.save_workspace_layout_json(
            Self::SETTINGS_CONTEXT_COMMAND_SURFACE_NAME,
            &self
                .workspace
                .chrome_ui
                .context_command_surface_preference
                .to_string(),
        );
    }

    fn save_keyboard_pan_step(&mut self) {
        self.save_workspace_layout_json(
            Self::SETTINGS_KEYBOARD_PAN_STEP_NAME,
            &format!("{:.3}", self.workspace.chrome_ui.keyboard_pan_step),
        );
    }

    fn save_keyboard_pan_input_mode(&mut self) {
        self.save_workspace_layout_json(
            Self::SETTINGS_KEYBOARD_PAN_INPUT_MODE_NAME,
            &self.workspace.chrome_ui.keyboard_pan_input_mode.to_string(),
        );
    }

    fn save_camera_pan_inertia_enabled(&mut self) {
        self.save_workspace_layout_json(
            Self::SETTINGS_CAMERA_PAN_INERTIA_ENABLED_NAME,
            if self.workspace.chrome_ui.camera_pan_inertia_enabled {
                "true"
            } else {
                "false"
            },
        );
    }

    fn save_camera_pan_inertia_damping(&mut self) {
        self.save_workspace_layout_json(
            Self::SETTINGS_CAMERA_PAN_INERTIA_DAMPING_NAME,
            &format!("{:.3}", self.workspace.chrome_ui.camera_pan_inertia_damping),
        );
    }

    fn save_lasso_binding_preference(&mut self) {
        self.save_workspace_layout_json(
            Self::SETTINGS_LASSO_BINDING_NAME,
            &self
                .workspace
                .chrome_ui
                .lasso_binding_preference
                .to_string(),
        );
    }

    fn save_input_binding_remaps(&mut self, remaps: &[InputBindingRemap]) {
        let encoded = remaps
            .iter()
            .map(InputBindingRemap::encode)
            .collect::<Vec<_>>()
            .join("\n");
        self.save_workspace_layout_json(Self::SETTINGS_INPUT_BINDING_REMAPS_NAME, &encoded);
    }

    pub fn save_persisted_nostr_signer_settings(&mut self) {
        let encoded = serde_json::to_string(
            &crate::shell::desktop::runtime::registries::phase3_nostr_persisted_signer_settings(),
        )
        .unwrap_or_else(|_| "{\"backend\":\"local_host_key\"}".to_string());
        self.save_workspace_layout_json(Self::SETTINGS_NOSTR_SIGNER_SETTINGS_NAME, &encoded);
    }

    pub fn save_persisted_nostr_nip07_permissions(&mut self) {
        let encoded = serde_json::to_string(
            &crate::shell::desktop::runtime::registries::phase3_nostr_persisted_nip07_permissions(),
        )
        .unwrap_or_else(|_| "[]".to_string());
        self.save_workspace_layout_json(Self::SETTINGS_NOSTR_NIP07_PERMISSIONS_NAME, &encoded);
    }

    fn load_persisted_nostr_signer_settings(&mut self) {
        let Some(raw) = self.load_workspace_layout_json(Self::SETTINGS_NOSTR_SIGNER_SETTINGS_NAME)
        else {
            return;
        };
        let Some(settings) = serde_json::from_str::<
            crate::shell::desktop::runtime::registries::PersistedNostrSignerSettings,
        >(&raw)
        .map_err(|error| {
            warn!("Ignoring invalid persisted nostr signer settings: {error}");
            error
        })
        .ok() else {
            return;
        };
        if let Err(error) =
            crate::shell::desktop::runtime::registries::phase3_nostr_apply_persisted_signer_settings(
                &settings,
            )
        {
            warn!("Ignoring persisted nostr signer settings restore failure: {error:?}");
        }
    }

    fn load_persisted_nostr_nip07_permissions(&mut self) {
        let permissions = self
            .load_workspace_layout_json(Self::SETTINGS_NOSTR_NIP07_PERMISSIONS_NAME)
            .and_then(|raw| {
                serde_json::from_str::<
                    Vec<crate::shell::desktop::runtime::registries::Nip07PermissionGrant>,
                >(&raw)
                .map_err(|error| {
                    warn!("Ignoring invalid persisted nostr nip07 permissions: {error}");
                    error
                })
                .ok()
            })
            .unwrap_or_default();

        if let Err(error) =
        crate::shell::desktop::runtime::registries::phase3_nostr_apply_persisted_nip07_permissions(
            &permissions,
        )
    {
        warn!("Ignoring persisted nostr nip07 permissions restore failure: {error:?}");
    }
    }

    pub fn save_persisted_nostr_subscriptions(&mut self) {
        let encoded = serde_json::to_string(
            &crate::shell::desktop::runtime::registries::phase3_nostr_persisted_subscriptions(),
        )
        .unwrap_or_else(|_| "[]".to_string());
        self.save_workspace_layout_json(Self::SETTINGS_NOSTR_SUBSCRIPTIONS_NAME, &encoded);
    }

    fn load_persisted_nostr_subscriptions(&mut self) {
        let subscriptions = self
            .load_workspace_layout_json(Self::SETTINGS_NOSTR_SUBSCRIPTIONS_NAME)
            .and_then(|raw| {
                serde_json::from_str::<
                    Vec<crate::shell::desktop::runtime::registries::PersistedNostrSubscription>,
                >(&raw)
                .map_err(|error| {
                    warn!("Ignoring invalid persisted nostr subscriptions: {error}");
                    error
                })
                .ok()
            })
            .unwrap_or_default();

        if let Err(error) =
            crate::shell::desktop::runtime::registries::phase3_restore_nostr_subscriptions(
                &subscriptions,
            )
        {
            warn!("Ignoring persisted nostr subscriptions restore failure: {error:?}");
        }
    }

    pub fn set_omnibar_preferred_scope(&mut self, scope: OmnibarPreferredScope) {
        self.workspace.chrome_ui.omnibar_preferred_scope = scope;
        self.save_omnibar_preferred_scope();
    }

    fn save_omnibar_preferred_scope(&mut self) {
        self.save_workspace_layout_json(
            Self::SETTINGS_OMNIBAR_PREFERRED_SCOPE_NAME,
            &self.workspace.chrome_ui.omnibar_preferred_scope.to_string(),
        );
    }

    pub fn set_omnibar_non_at_order(&mut self, order: OmnibarNonAtOrderPreset) {
        self.workspace.chrome_ui.omnibar_non_at_order = order;
        self.save_omnibar_non_at_order();
    }

    fn save_omnibar_non_at_order(&mut self) {
        self.save_workspace_layout_json(
            Self::SETTINGS_OMNIBAR_NON_AT_ORDER_NAME,
            &self.workspace.chrome_ui.omnibar_non_at_order.to_string(),
        );
    }

    pub fn wry_enabled(&self) -> bool {
        self.workspace.chrome_ui.wry_enabled
    }

    pub fn set_wry_enabled(&mut self, enabled: bool) {
        self.workspace.chrome_ui.wry_enabled = enabled;
        self.save_wry_enabled();
    }

    pub fn default_web_viewer_backend(&self) -> DefaultWebViewerBackend {
        self.workspace.chrome_ui.default_web_viewer_backend
    }

    pub fn set_default_web_viewer_backend(&mut self, backend: DefaultWebViewerBackend) {
        self.workspace.chrome_ui.default_web_viewer_backend = backend;
        self.save_default_web_viewer_backend();
    }

    pub fn wry_render_mode_preference(&self) -> WryRenderModePreference {
        self.workspace.chrome_ui.wry_render_mode_preference
    }

    pub fn set_wry_render_mode_preference(&mut self, preference: WryRenderModePreference) {
        self.workspace.chrome_ui.wry_render_mode_preference = preference;
        self.save_wry_render_mode_preference();
    }

    pub fn workspace_user_stylesheets(&self) -> &[WorkspaceUserStylesheetSetting] {
        &self.workspace.chrome_ui.workspace_user_stylesheets
    }

    pub fn add_workspace_user_stylesheet(&mut self, path: &str) -> Result<(), String> {
        let trimmed = path.trim();
        if trimmed.is_empty() {
            return Err("Enter a stylesheet path first.".to_string());
        }

        let (resolved, _) = crate::prefs::read_user_stylesheet_source(Path::new(trimmed))
            .map_err(|error| format!("Failed to load stylesheet '{trimmed}': {error}"))?;
        let normalized_path = resolved.to_string_lossy().into_owned();

        if let Some(entry) = self
            .workspace
            .chrome_ui
            .workspace_user_stylesheets
            .iter_mut()
            .find(|entry| entry.path == normalized_path)
        {
            entry.enabled = true;
        } else {
            self.workspace.chrome_ui.workspace_user_stylesheets.push(
                WorkspaceUserStylesheetSetting {
                    path: normalized_path,
                    enabled: true,
                },
            );
        }

        self.workspace
            .chrome_ui
            .workspace_user_stylesheets_initialized = true;
        self.queue_workspace_user_stylesheet_runtime_apply(true);
        Ok(())
    }

    pub fn set_workspace_user_stylesheet_enabled(&mut self, index: usize, enabled: bool) {
        let Some(entry) = self
            .workspace
            .chrome_ui
            .workspace_user_stylesheets
            .get_mut(index)
        else {
            return;
        };

        if entry.enabled == enabled {
            return;
        }

        entry.enabled = enabled;
        self.workspace
            .chrome_ui
            .workspace_user_stylesheets_initialized = true;
        self.queue_workspace_user_stylesheet_runtime_apply(true);
    }

    pub fn remove_workspace_user_stylesheet(&mut self, index: usize) {
        if index >= self.workspace.chrome_ui.workspace_user_stylesheets.len() {
            return;
        }

        self.workspace
            .chrome_ui
            .workspace_user_stylesheets
            .remove(index);
        self.workspace
            .chrome_ui
            .workspace_user_stylesheets_initialized = true;
        self.queue_workspace_user_stylesheet_runtime_apply(true);
    }

    pub fn reload_workspace_user_stylesheets(&mut self) {
        self.workspace
            .chrome_ui
            .workspace_user_stylesheets_initialized = true;
        self.queue_workspace_user_stylesheet_runtime_apply(true);
    }

    pub(crate) fn reconcile_workspace_user_stylesheets_with_runtime(
        &mut self,
        runtime_snapshot: Vec<WorkspaceUserStylesheetSetting>,
    ) {
        if !self
            .workspace
            .chrome_ui
            .workspace_user_stylesheets_initialized
        {
            self.workspace.chrome_ui.workspace_user_stylesheets = runtime_snapshot;
            self.workspace
                .chrome_ui
                .workspace_user_stylesheets_initialized = true;
            self.workspace
                .chrome_ui
                .workspace_user_stylesheets_runtime_synced = true;
            self.workspace
                .chrome_ui
                .workspace_user_stylesheet_status_message = None;
            return;
        }

        if self
            .workspace
            .chrome_ui
            .workspace_user_stylesheets_runtime_synced
        {
            return;
        }

        if self.enabled_workspace_user_stylesheets() == runtime_snapshot {
            self.workspace
                .chrome_ui
                .workspace_user_stylesheets_runtime_synced = true;
            self.workspace
                .chrome_ui
                .workspace_user_stylesheet_status_message = None;
            return;
        }

        self.queue_workspace_user_stylesheet_runtime_apply(true);
    }

    pub fn webview_preview_active_refresh_secs(&self) -> u64 {
        self.workspace.chrome_ui.webview_preview_active_refresh_secs
    }

    pub fn set_webview_preview_active_refresh_secs(&mut self, secs: u64) {
        self.workspace.chrome_ui.webview_preview_active_refresh_secs = secs.clamp(1, 300);
        self.save_webview_preview_active_refresh_secs();
    }

    pub fn webview_preview_warm_refresh_secs(&self) -> u64 {
        self.workspace.chrome_ui.webview_preview_warm_refresh_secs
    }

    pub fn set_webview_preview_warm_refresh_secs(&mut self, secs: u64) {
        self.workspace.chrome_ui.webview_preview_warm_refresh_secs = secs.clamp(5, 3600);
        self.save_webview_preview_warm_refresh_secs();
    }

    pub fn workbench_host_pinned(&self) -> bool {
        self.workspace.chrome_ui.workbench_host_pinned
    }

    pub fn set_workbench_host_pinned(&mut self, pinned: bool) {
        self.workspace.chrome_ui.workbench_host_pinned = pinned;
        self.save_workbench_host_pinned();
    }

    pub fn chrome_overlay_active(&self) -> bool {
        self.workspace.chrome_ui.show_settings_overlay
            || self.workspace.chrome_ui.show_scene_overlay
            || self.workspace.chrome_ui.show_help_panel
            || self.workspace.chrome_ui.show_command_palette
            || self.workspace.chrome_ui.show_context_palette
            || self.workspace.chrome_ui.show_radial_menu
            || self.workspace.chrome_ui.show_clip_inspector
    }

    fn save_wry_enabled(&mut self) {
        self.save_workspace_layout_json(
            Self::SETTINGS_WRY_ENABLED_NAME,
            if self.workspace.chrome_ui.wry_enabled {
                "true"
            } else {
                "false"
            },
        );
    }

    fn save_default_web_viewer_backend(&mut self) {
        self.save_workspace_layout_json(
            Self::SETTINGS_DEFAULT_WEB_VIEWER_BACKEND_NAME,
            &self
                .workspace
                .chrome_ui
                .default_web_viewer_backend
                .to_string(),
        );
    }

    fn save_wry_render_mode_preference(&mut self) {
        self.save_workspace_layout_json(
            Self::SETTINGS_WRY_RENDER_MODE_PREFERENCE_NAME,
            &self
                .workspace
                .chrome_ui
                .wry_render_mode_preference
                .to_string(),
        );
    }

    fn save_workspace_user_stylesheets(&mut self) {
        let Ok(encoded) =
            serde_json::to_string(&self.workspace.chrome_ui.workspace_user_stylesheets)
        else {
            warn!("Failed to serialize workspace user stylesheet settings");
            return;
        };

        self.save_workspace_layout_json(Self::SETTINGS_WORKSPACE_USER_STYLESHEETS_NAME, &encoded);
    }

    fn save_webview_preview_active_refresh_secs(&mut self) {
        self.save_workspace_layout_json(
            Self::SETTINGS_WEBVIEW_PREVIEW_ACTIVE_REFRESH_SECS_NAME,
            &self
                .workspace
                .chrome_ui
                .webview_preview_active_refresh_secs
                .to_string(),
        );
    }

    fn save_webview_preview_warm_refresh_secs(&mut self) {
        self.save_workspace_layout_json(
            Self::SETTINGS_WEBVIEW_PREVIEW_WARM_REFRESH_SECS_NAME,
            &self
                .workspace
                .chrome_ui
                .webview_preview_warm_refresh_secs
                .to_string(),
        );
    }

    fn save_workbench_host_pinned(&mut self) {
        self.save_workspace_layout_json(
            Self::SETTINGS_WORKBENCH_HOST_PINNED_NAME,
            if self.workspace.chrome_ui.workbench_host_pinned {
                "true"
            } else {
                "false"
            },
        );
    }

    pub(crate) fn save_workbench_profile_state(&mut self) {
        let Ok(encoded) =
            serde_json::to_string(&self.workspace.workbench_session.workbench_profile)
        else {
            warn!("Failed to serialize persisted workbench profile state");
            return;
        };
        self.save_workspace_layout_json(Self::SETTINGS_WORKBENCH_PROFILE_STATE_NAME, &encoded);
    }

    pub fn set_default_registry_lens_id(&mut self, lens_id: Option<&str>) {
        let normalized = Self::normalize_optional_registry_id(lens_id.map(str::to_owned));
        self.workspace.chrome_ui.default_registry_lens_id = normalized.clone();
        crate::shell::desktop::runtime::registries::phase3_publish_lens_changed(
            normalized.as_deref(),
        );
        self.save_workspace_layout_json(
            Self::SETTINGS_REGISTRY_LENS_ID_NAME,
            normalized.as_deref().unwrap_or(""),
        );
    }

    pub fn set_default_registry_physics_id(&mut self, physics_id: Option<&str>) {
        let normalized = Self::normalize_optional_registry_id(physics_id.map(str::to_owned));
        self.workspace.chrome_ui.default_registry_physics_id = normalized.clone();
        let resolution =
            crate::shell::desktop::runtime::registries::phase3_set_active_physics_profile(
                normalized
                    .as_deref()
                    .unwrap_or(crate::registries::atomic::lens::PHYSICS_ID_DEFAULT),
            );
        self.apply_physics_profile(&resolution.profile);
        self.save_workspace_layout_json(
            Self::SETTINGS_REGISTRY_PHYSICS_ID_NAME,
            normalized.as_deref().unwrap_or(""),
        );
    }

    pub fn set_default_registry_theme_id(&mut self, theme_id: Option<&str>) {
        let normalized = Self::normalize_optional_registry_id(theme_id.map(str::to_owned));
        let persisted = normalized.as_deref().map(|requested| {
            crate::shell::desktop::runtime::registries::phase3_set_active_theme(requested)
                .resolved_id
        });
        self.workspace.chrome_ui.default_registry_theme_id = persisted.clone();
        self.save_workspace_layout_json(
            Self::SETTINGS_REGISTRY_THEME_ID_NAME,
            persisted.as_deref().unwrap_or(""),
        );
    }

    /// Set the theme mode preference and apply it immediately.
    ///
    /// - `System`: clears the explicit theme ID and lets `WindowEvent::ThemeChanged` drive
    ///   the active theme. The runtime theme is not changed here — the next OS event will
    ///   apply the correct theme. If the OS preference is already known the caller should
    ///   call `apply_system_theme_preference` directly.
    /// - `Light` / `Dark`: sets the explicit theme ID and applies it now.
    pub fn set_theme_mode(&mut self, mode: ThemeMode) {
        self.workspace.chrome_ui.theme_mode = mode;
        self.save_workspace_layout_json(Self::SETTINGS_THEME_MODE_NAME, &mode.to_string());
        let follows_system = mode == ThemeMode::System;
        crate::shell::desktop::runtime::registries::phase3_set_theme_follows_system(follows_system);
        match mode {
            ThemeMode::System => {
                // Clear explicit override — runtime will follow OS events.
                self.workspace.chrome_ui.default_registry_theme_id = None;
                self.save_workspace_layout_json(Self::SETTINGS_REGISTRY_THEME_ID_NAME, "");
            }
            ThemeMode::Light => {
                self.set_default_registry_theme_id(Some(
                    crate::shell::desktop::runtime::registries::theme::THEME_ID_LIGHT,
                ));
            }
            ThemeMode::Dark => {
                self.set_default_registry_theme_id(Some(
                    crate::shell::desktop::runtime::registries::theme::THEME_ID_DARK,
                ));
            }
        }
    }

    pub fn theme_mode(&self) -> ThemeMode {
        self.workspace.chrome_ui.theme_mode
    }

    pub fn default_registry_lens_id(&self) -> Option<&str> {
        self.workspace.chrome_ui.default_registry_lens_id.as_deref()
    }

    pub fn default_registry_physics_id(&self) -> Option<&str> {
        self.workspace
            .chrome_ui
            .default_registry_physics_id
            .as_deref()
    }

    pub fn default_registry_theme_id(&self) -> Option<&str> {
        self.workspace
            .chrome_ui
            .default_registry_theme_id
            .as_deref()
    }

    pub fn set_diagnostics_channel_config(&mut self, channel_id: &str, config: &ChannelConfig) {
        let normalized = channel_id.trim().to_ascii_lowercase();
        if normalized.is_empty() {
            return;
        }
        let key = format!(
            "{}{}",
            Self::SETTINGS_DIAGNOSTICS_CHANNEL_CONFIG_PREFIX,
            normalized
        );
        self.save_workspace_layout_json(
            &key,
            &format!(
                "{}|{}|{}",
                if config.enabled { "1" } else { "0" },
                config.sample_rate,
                config.retention_count
            ),
        );
    }

    pub fn diagnostics_channel_configs(&self) -> Vec<(String, ChannelConfig)> {
        self.list_workspace_layout_names()
            .into_iter()
            .filter_map(|key| {
                let channel_id = key
                    .strip_prefix(Self::SETTINGS_DIAGNOSTICS_CHANNEL_CONFIG_PREFIX)?
                    .to_string();
                let raw = self.load_workspace_layout_json(&key)?;
                parse_diagnostics_channel_config(&raw).map(|config| (channel_id, config))
            })
            .collect()
    }

    pub(crate) fn load_persisted_ui_settings(&mut self) {
        let Some(raw) = self.load_workspace_layout_json(Self::SETTINGS_TOAST_ANCHOR_NAME) else {
            return self.load_additional_persisted_ui_settings();
        };
        if let Ok(preference) = raw.parse::<ToastAnchorPreference>() {
            self.workspace.chrome_ui.toast_anchor_preference = preference;
        } else {
            warn!("Ignoring invalid persisted toast anchor preference: '{raw}'");
        }
        self.load_additional_persisted_ui_settings();
    }

    fn load_additional_persisted_ui_settings(&mut self) {
        if let Some(raw) =
            self.load_workspace_layout_json(Self::SETTINGS_COMMAND_PALETTE_SHORTCUT_NAME)
        {
            if let Ok(shortcut) = raw.parse::<CommandPaletteShortcut>() {
                self.workspace.chrome_ui.command_palette_shortcut = shortcut;
            } else {
                warn!("Ignoring invalid persisted command-palette shortcut: '{raw}'");
            }
        }
        if let Some(raw) = self.load_workspace_layout_json(Self::SETTINGS_HELP_PANEL_SHORTCUT_NAME)
        {
            if let Ok(shortcut) = raw.parse::<HelpPanelShortcut>() {
                self.workspace.chrome_ui.help_panel_shortcut = shortcut;
            } else {
                warn!("Ignoring invalid persisted help-panel shortcut: '{raw}'");
            }
        }
        if let Some(raw) = self.load_workspace_layout_json(Self::SETTINGS_RADIAL_MENU_SHORTCUT_NAME)
        {
            if let Ok(shortcut) = raw.parse::<RadialMenuShortcut>() {
                self.workspace.chrome_ui.radial_menu_shortcut = shortcut;
            } else {
                warn!("Ignoring invalid persisted radial-menu shortcut: '{raw}'");
            }
        }
        if let Some(raw) =
            self.load_workspace_layout_json(Self::SETTINGS_CONTEXT_COMMAND_SURFACE_NAME)
        {
            if let Ok(preference) = raw.parse::<ContextCommandSurfacePreference>() {
                self.workspace.chrome_ui.context_command_surface_preference = preference;
            } else {
                warn!("Ignoring invalid persisted context-command surface preference: '{raw}'");
            }
        }
        if let Some(raw) = self.load_workspace_layout_json(Self::SETTINGS_KEYBOARD_PAN_STEP_NAME) {
            if let Ok(step) = raw.trim().parse::<f32>() {
                self.workspace.chrome_ui.keyboard_pan_step = step.clamp(1.0, 200.0);
            } else {
                warn!("Ignoring invalid persisted keyboard pan step: '{raw}'");
            }
        }
        if let Some(raw) =
            self.load_workspace_layout_json(Self::SETTINGS_KEYBOARD_PAN_INPUT_MODE_NAME)
        {
            if let Ok(mode) = raw.parse::<KeyboardPanInputMode>() {
                self.workspace.chrome_ui.keyboard_pan_input_mode = mode;
            } else {
                warn!("Ignoring invalid persisted keyboard pan input mode: '{raw}'");
            }
        }
        if let Some(raw) =
            self.load_workspace_layout_json(Self::SETTINGS_CAMERA_PAN_INERTIA_ENABLED_NAME)
        {
            match raw.trim().to_ascii_lowercase().as_str() {
                "true" | "1" | "yes" | "on" => {
                    self.workspace.chrome_ui.camera_pan_inertia_enabled = true
                }
                "false" | "0" | "no" | "off" => {
                    self.workspace.chrome_ui.camera_pan_inertia_enabled = false
                }
                _ => warn!("Ignoring invalid persisted camera pan inertia enabled flag: '{raw}'"),
            }
        }
        if let Some(raw) =
            self.load_workspace_layout_json(Self::SETTINGS_CAMERA_PAN_INERTIA_DAMPING_NAME)
        {
            if let Ok(damping) = raw.trim().parse::<f32>() {
                self.workspace.chrome_ui.camera_pan_inertia_damping = damping.clamp(0.70, 0.99);
            } else {
                warn!("Ignoring invalid persisted camera pan inertia damping: '{raw}'");
            }
        }
        if let Some(raw) = self.load_workspace_layout_json(Self::SETTINGS_LASSO_BINDING_NAME) {
            if let Ok(binding) = raw.parse::<CanvasLassoBinding>() {
                self.workspace.chrome_ui.lasso_binding_preference = binding;
            } else {
                warn!("Ignoring invalid persisted lasso binding preference: '{raw}'");
            }
        }
        self.load_persisted_input_binding_remaps();
        if let Some(raw) =
            self.load_workspace_layout_json(Self::SETTINGS_OMNIBAR_PREFERRED_SCOPE_NAME)
        {
            if let Ok(scope) = raw.parse::<OmnibarPreferredScope>() {
                self.workspace.chrome_ui.omnibar_preferred_scope = scope;
            } else {
                warn!("Ignoring invalid persisted omnibar preferred scope: '{raw}'");
            }
        }
        if let Some(raw) = self.load_workspace_layout_json(Self::SETTINGS_OMNIBAR_NON_AT_ORDER_NAME)
        {
            if let Ok(order) = raw.parse::<OmnibarNonAtOrderPreset>() {
                self.workspace.chrome_ui.omnibar_non_at_order = order;
            } else {
                warn!("Ignoring invalid persisted omnibar non-@ order preset: '{raw}'");
            }
        }
        if let Some(raw) = self.load_workspace_layout_json(Self::SETTINGS_WRY_ENABLED_NAME) {
            match raw.trim().to_ascii_lowercase().as_str() {
                "true" | "1" | "yes" | "on" => self.workspace.chrome_ui.wry_enabled = true,
                "false" | "0" | "no" | "off" => self.workspace.chrome_ui.wry_enabled = false,
                _ => warn!("Ignoring invalid persisted wry enabled flag: '{raw}'"),
            }
        }
        if let Some(raw) =
            self.load_workspace_layout_json(Self::SETTINGS_WORKSPACE_USER_STYLESHEETS_NAME)
        {
            match serde_json::from_str::<Vec<WorkspaceUserStylesheetSetting>>(&raw) {
                Ok(entries) => {
                    self.workspace.chrome_ui.workspace_user_stylesheets = entries;
                    self.workspace
                        .chrome_ui
                        .workspace_user_stylesheets_initialized = true;
                    self.workspace
                        .chrome_ui
                        .workspace_user_stylesheets_runtime_synced = false;
                    self.workspace
                        .chrome_ui
                        .workspace_user_stylesheet_status_message = None;
                }
                Err(error) => {
                    warn!("Ignoring invalid persisted workspace user stylesheet settings: {error}")
                }
            }
        }
        self.workspace.chrome_ui.default_web_viewer_backend = self
            .load_workspace_layout_json(Self::SETTINGS_DEFAULT_WEB_VIEWER_BACKEND_NAME)
            .and_then(|raw| raw.parse::<DefaultWebViewerBackend>().ok())
            .unwrap_or_default();
        self.workspace.chrome_ui.wry_render_mode_preference = self
            .load_workspace_layout_json(Self::SETTINGS_WRY_RENDER_MODE_PREFERENCE_NAME)
            .and_then(|raw| raw.parse::<WryRenderModePreference>().ok())
            .unwrap_or_default();
        self.workspace.chrome_ui.webview_preview_active_refresh_secs = self
            .load_workspace_layout_json(Self::SETTINGS_WEBVIEW_PREVIEW_ACTIVE_REFRESH_SECS_NAME)
            .and_then(|raw| raw.trim().parse::<u64>().ok())
            .map(|secs| secs.clamp(1, 300))
            .unwrap_or(Self::DEFAULT_WEBVIEW_PREVIEW_ACTIVE_REFRESH_SECS);
        self.workspace.chrome_ui.webview_preview_warm_refresh_secs = self
            .load_workspace_layout_json(Self::SETTINGS_WEBVIEW_PREVIEW_WARM_REFRESH_SECS_NAME)
            .and_then(|raw| raw.trim().parse::<u64>().ok())
            .map(|secs| secs.clamp(5, 3600))
            .unwrap_or(Self::DEFAULT_WEBVIEW_PREVIEW_WARM_REFRESH_SECS);
        if let Some(raw) =
            self.load_workspace_layout_json(Self::SETTINGS_WORKBENCH_HOST_PINNED_NAME)
        {
            match raw.trim().to_ascii_lowercase().as_str() {
                "true" | "1" | "yes" | "on" => {
                    self.workspace.chrome_ui.workbench_host_pinned = true;
                }
                "false" | "0" | "no" | "off" => {
                    self.workspace.chrome_ui.workbench_host_pinned = false;
                }
                _ => warn!("Ignoring invalid persisted workbench host pinned flag: '{raw}'"),
            }
        }
        if let Some(raw) =
            self.load_workspace_layout_json(Self::SETTINGS_WORKBENCH_PROFILE_STATE_NAME)
        {
            match serde_json::from_str::<WorkbenchProfile>(&raw) {
                Ok(profile) => self.restore_workbench_profile(profile),
                Err(error) => {
                    warn!("Ignoring invalid persisted workbench profile state: {error}");
                }
            }
        }
        self.workspace.chrome_ui.default_registry_lens_id = self
            .load_workspace_layout_json(Self::SETTINGS_REGISTRY_LENS_ID_NAME)
            .map(|raw| Self::normalize_optional_registry_id(Some(raw)))
            .unwrap_or(None);
        self.workspace.chrome_ui.default_registry_physics_id = self
            .load_workspace_layout_json(Self::SETTINGS_REGISTRY_PHYSICS_ID_NAME)
            .map(|raw| Self::normalize_optional_registry_id(Some(raw)))
            .unwrap_or(None);
        // Load theme mode first; it governs how the explicit theme id is used.
        let loaded_theme_mode = self
            .load_workspace_layout_json(Self::SETTINGS_THEME_MODE_NAME)
            .and_then(|raw| raw.parse::<ThemeMode>().ok())
            .unwrap_or(ThemeMode::System);
        self.workspace.chrome_ui.theme_mode = loaded_theme_mode;
        crate::shell::desktop::runtime::registries::phase3_set_theme_follows_system(
            loaded_theme_mode == ThemeMode::System,
        );

        self.workspace.chrome_ui.default_registry_theme_id = self
            .load_workspace_layout_json(Self::SETTINGS_REGISTRY_THEME_ID_NAME)
            .map(|raw| Self::normalize_optional_registry_id(Some(raw)))
            .unwrap_or(None);
        // Only apply the persisted explicit theme when mode is not System.
        // System mode relies on WindowEvent::ThemeChanged to set the active theme.
        if loaded_theme_mode != ThemeMode::System {
            if let Some(theme_id) = self
                .workspace
                .chrome_ui
                .default_registry_theme_id
                .as_deref()
            {
                let resolution =
                    crate::shell::desktop::runtime::registries::phase3_set_active_theme(theme_id);
                self.workspace.chrome_ui.default_registry_theme_id = Some(resolution.resolved_id);
            }
        }
        let canvas_profile_id = self
            .load_workspace_layout_json(Self::SETTINGS_CANVAS_PROFILE_ID_NAME)
            .map(|raw| raw.trim().to_ascii_lowercase())
            .filter(|raw| !raw.is_empty());
        let workbench_surface_profile_id = self
            .load_workspace_layout_json(Self::SETTINGS_WORKBENCH_SURFACE_PROFILE_ID_NAME)
            .map(|raw| raw.trim().to_ascii_lowercase())
            .filter(|raw| !raw.is_empty());
        let active_workflow_id = self
            .load_workspace_layout_json(Self::SETTINGS_ACTIVE_WORKFLOW_ID_NAME)
            .map(|raw| raw.trim().to_ascii_lowercase())
            .filter(|raw| !raw.is_empty());
        if let Some(physics_id) = self
            .workspace
            .chrome_ui
            .default_registry_physics_id
            .as_deref()
        {
            let resolution =
                crate::shell::desktop::runtime::registries::phase3_set_active_physics_profile(
                    physics_id,
                );
            self.apply_physics_profile(&resolution.profile);
        } else {
            let resolution =
                crate::shell::desktop::runtime::registries::phase3_set_active_physics_profile(
                    crate::registries::atomic::lens::PHYSICS_ID_DEFAULT,
                );
            self.apply_physics_profile(&resolution.profile);
        }
        if let Some(profile_id) = canvas_profile_id.as_deref() {
            crate::shell::desktop::runtime::registries::phase3_set_active_canvas_profile(
                profile_id,
            );
        } else {
            crate::shell::desktop::runtime::registries::phase3_set_active_canvas_profile(
                crate::registries::domain::layout::canvas::CANVAS_PROFILE_DEFAULT,
            );
        }
        crate::shell::desktop::runtime::registries::phase3_set_active_canvas_keyboard_pan_step(
            self.workspace.chrome_ui.keyboard_pan_step,
        );
        crate::shell::desktop::runtime::registries::phase3_set_active_canvas_lasso_binding(
            self.workspace.chrome_ui.lasso_binding_preference,
        );
        if let Some(profile_id) = workbench_surface_profile_id.as_deref() {
            crate::shell::desktop::runtime::registries::phase3_set_active_workbench_surface_profile(
                profile_id,
            );
        }
        if let Some(workflow_id) = active_workflow_id.as_deref()
            && let Err(error) = crate::shell::desktop::runtime::registries::phase3_activate_workflow(
                self,
                workflow_id,
            )
        {
            warn!("Ignoring invalid persisted workflow activation '{workflow_id}': {error:?}");
        }
        crate::shell::desktop::runtime::registries::phase3_set_active_canvas_keyboard_pan_step(
            self.workspace.chrome_ui.keyboard_pan_step,
        );
        crate::shell::desktop::runtime::registries::phase3_set_active_canvas_lasso_binding(
            self.workspace.chrome_ui.lasso_binding_preference,
        );
        self.load_persisted_nostr_signer_settings();
        self.load_persisted_nostr_nip07_permissions();
        self.load_persisted_nostr_subscriptions();
        self.load_graph_view_layout_manager_state();

        crate::registries::atomic::diagnostics::apply_persisted_channel_configs(
            self.diagnostics_channel_configs(),
        );
    }

    fn load_persisted_input_binding_remaps(&mut self) {
        let Some(raw) = self.load_workspace_layout_json(Self::SETTINGS_INPUT_BINDING_REMAPS_NAME)
        else {
            phase2_reset_input_binding_remaps();
            return;
        };

        let remaps = match Self::decode_input_binding_remaps(&raw) {
            Ok(remaps) => remaps,
            Err(_) => {
                warn!("Ignoring invalid persisted input binding remaps");
                phase2_reset_input_binding_remaps();
                return;
            }
        };

        if phase2_apply_input_binding_remaps(&remaps).is_err() {
            warn!("Ignoring persisted input binding remaps that conflict with defaults");
            phase2_reset_input_binding_remaps();
        }
    }

    fn decode_input_binding_remaps(raw: &str) -> Result<Vec<InputBindingRemap>, ()> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Ok(Vec::new());
        }

        trimmed
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(InputBindingRemap::decode)
            .collect()
    }

    fn normalize_optional_registry_id(raw: Option<String>) -> Option<String> {
        raw.and_then(|value| {
            let normalized = value.trim().to_ascii_lowercase();
            (!normalized.is_empty()).then_some(normalized)
        })
    }

    fn enabled_workspace_user_stylesheets(&self) -> Vec<WorkspaceUserStylesheetSetting> {
        self.workspace
            .chrome_ui
            .workspace_user_stylesheets
            .iter()
            .filter(|entry| entry.enabled)
            .cloned()
            .collect()
    }

    fn build_runtime_user_stylesheet_specs(
        entries: &[WorkspaceUserStylesheetSetting],
    ) -> (Vec<RuntimeUserStylesheetSpec>, Vec<String>) {
        let mut stylesheets = Vec::new();
        let mut failures = Vec::new();

        for entry in entries.iter().filter(|entry| entry.enabled) {
            match crate::prefs::read_user_stylesheet_source(Path::new(&entry.path)) {
                Ok((path, source)) => stylesheets.push(RuntimeUserStylesheetSpec { path, source }),
                Err(error) => failures.push(format!("{} ({error})", entry.path)),
            }
        }

        (stylesheets, failures)
    }

    fn queue_workspace_user_stylesheet_runtime_apply(&mut self, reload: bool) {
        let (stylesheets, failures) = Self::build_runtime_user_stylesheet_specs(
            &self.workspace.chrome_ui.workspace_user_stylesheets,
        );
        self.workspace
            .chrome_ui
            .workspace_user_stylesheets_runtime_synced = true;
        self.workspace
            .chrome_ui
            .workspace_user_stylesheet_status_message = if failures.is_empty() {
            None
        } else {
            Some(format!(
                "Skipped unreadable stylesheet entries: {}",
                failures.join("; ")
            ))
        };
        self.save_workspace_user_stylesheets();
        self.set_pending_apply_user_stylesheets(stylesheets, reload);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adding_workspace_user_stylesheet_queues_runtime_apply() {
        let temp_dir = tempfile::tempdir().expect("temp dir should exist");
        let stylesheet_path = temp_dir.path().join("user.css");
        std::fs::write(&stylesheet_path, "body { color: rgb(4, 5, 6); }")
            .expect("stylesheet should be writable");

        let mut app = GraphBrowserApp::new_for_testing();
        app.add_workspace_user_stylesheet(stylesheet_path.to_str().unwrap())
            .expect("stylesheet should be accepted");

        let (stylesheets, reload) = app
            .take_pending_apply_user_stylesheets()
            .expect("runtime apply command should be queued");
        assert!(reload);
        assert_eq!(app.workspace_user_stylesheets().len(), 1);
        assert_eq!(stylesheets.len(), 1);
        assert_eq!(stylesheets[0].source, "body { color: rgb(4, 5, 6); }");
        assert_eq!(stylesheets[0].path, stylesheet_path);
    }

    #[test]
    fn disabling_workspace_user_stylesheet_clears_runtime_apply_list() {
        let temp_dir = tempfile::tempdir().expect("temp dir should exist");
        let stylesheet_path = temp_dir.path().join("user.css");
        std::fs::write(&stylesheet_path, "body { color: rgb(7, 8, 9); }")
            .expect("stylesheet should be writable");

        let mut app = GraphBrowserApp::new_for_testing();
        app.add_workspace_user_stylesheet(stylesheet_path.to_str().unwrap())
            .expect("stylesheet should be accepted");
        let _ = app.take_pending_apply_user_stylesheets();

        app.set_workspace_user_stylesheet_enabled(0, false);
        let (stylesheets, reload) = app
            .take_pending_apply_user_stylesheets()
            .expect("runtime apply command should be queued");

        assert!(reload);
        assert!(stylesheets.is_empty());
    }

    #[test]
    fn runtime_bootstrap_populates_workspace_user_stylesheets_once() {
        let mut app = GraphBrowserApp::new_for_testing();
        app.reconcile_workspace_user_stylesheets_with_runtime(vec![
            WorkspaceUserStylesheetSetting {
                path: "C:/styles/one.css".to_string(),
                enabled: true,
            },
        ]);

        assert_eq!(app.workspace_user_stylesheets().len(), 1);
        assert!(
            app.workspace
                .chrome_ui
                .workspace_user_stylesheets_runtime_synced
        );
    }
}
