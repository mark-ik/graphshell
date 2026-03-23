use super::*;

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

impl GraphBrowserApp {
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

    pub fn workbench_host_pinned(&self) -> bool {
        self.workspace.chrome_ui.workbench_host_pinned
    }

    pub fn set_workbench_host_pinned(&mut self, pinned: bool) {
        self.workspace.chrome_ui.workbench_host_pinned = pinned;
        self.save_workbench_host_pinned();
    }

    pub fn chrome_overlay_active(&self) -> bool {
        self.workspace.chrome_ui.show_settings_overlay
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
        self.workspace.chrome_ui.default_registry_theme_id = self
            .load_workspace_layout_json(Self::SETTINGS_REGISTRY_THEME_ID_NAME)
            .map(|raw| Self::normalize_optional_registry_id(Some(raw)))
            .unwrap_or(None);
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

    pub(crate) fn with_registry_lens_defaults(&self, mut lens: LensConfig) -> LensConfig {
        if lens.lens_id.is_none() {
            lens.lens_id = self.workspace.chrome_ui.default_registry_lens_id.clone();
        }
        lens
    }
}
