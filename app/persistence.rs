fn open_store_for_startup(data_dir: PathBuf) -> Result<GraphStore, String> {
    #[cfg(test)]
    {
        return GraphStore::open(data_dir).map_err(|e| e.to_string());
    }

    #[cfg(not(test))]
    {
        let start = Instant::now();
        emit_event(DiagnosticEvent::MessageSent {
            channel_id: CHANNEL_STARTUP_PERSISTENCE_OPEN_STARTED,
            byte_len: data_dir.to_string_lossy().len(),
        });
        let timeout_ms = Self::startup_persistence_timeout_ms();
        let (tx, rx) = mpsc::channel();

        std::thread::Builder::new()
            .name("graphstore-open".to_string())
            .spawn(move || {
                let _ = tx.send(GraphStore::open(data_dir));
            })
            .map_err(|e| format!("failed to spawn persistence-open thread: {e}"))?;

        if timeout_ms == 0 {
            let result = rx.recv().map_err(|_| {
                "persistence-open worker disconnected before sending result".to_string()
            })?;

            match &result {
                Ok(_) => emit_event(DiagnosticEvent::MessageReceived {
                    channel_id: CHANNEL_STARTUP_PERSISTENCE_OPEN_SUCCEEDED,
                    latency_us: start.elapsed().as_micros() as u64,
                }),
                Err(_) => emit_event(DiagnosticEvent::MessageReceived {
                    channel_id: CHANNEL_STARTUP_PERSISTENCE_OPEN_FAILED,
                    latency_us: start.elapsed().as_micros() as u64,
                }),
            }

            return result.map_err(|e| e.to_string());
        }

        match rx.recv_timeout(Duration::from_millis(timeout_ms)) {
            Ok(result) => {
                match &result {
                    Ok(_) => emit_event(DiagnosticEvent::MessageReceived {
                        channel_id: CHANNEL_STARTUP_PERSISTENCE_OPEN_SUCCEEDED,
                        latency_us: start.elapsed().as_micros() as u64,
                    }),
                    Err(_) => emit_event(DiagnosticEvent::MessageReceived {
                        channel_id: CHANNEL_STARTUP_PERSISTENCE_OPEN_FAILED,
                        latency_us: start.elapsed().as_micros() as u64,
                    }),
                }
                result.map_err(|e| e.to_string())
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                emit_event(DiagnosticEvent::MessageReceived {
                    channel_id: CHANNEL_STARTUP_PERSISTENCE_OPEN_TIMEOUT,
                    latency_us: start.elapsed().as_micros() as u64,
                });
                Err(format!(
                    "startup persistence open timed out after {}ms; continuing without persistence",
                    timeout_ms
                ))
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                Err("persistence-open worker disconnected before sending result".to_string())
            }
        }
    }
}

/// Configure periodic persistence snapshot interval in seconds.
pub fn set_snapshot_interval_secs(&mut self, secs: u64) -> Result<(), String> {
    let store = self
        .services
        .persistence
        .as_mut()
        .ok_or_else(|| "Persistence is not available".to_string())?;
    store
        .set_snapshot_interval_secs(secs)
        .map_err(|e| e.to_string())
}

/// Current periodic persistence snapshot interval in seconds, if persistence is enabled.
pub fn snapshot_interval_secs(&self) -> Option<u64> {
    self.services
        .persistence
        .as_ref()
        .map(|store| store.snapshot_interval_secs())
}

/// Take an immediate snapshot (e.g., on shutdown)
pub fn take_snapshot(&mut self) {
    if let Some(store) = &mut self.services.persistence {
        store.take_snapshot(&self.workspace.domain.graph);
    }
}

/// Persist serialized tile layout JSON.
pub fn save_tile_layout_json(&mut self, layout_json: &str) {
    if let Some(store) = &mut self.services.persistence
        && let Err(e) = store.save_tile_layout_json(layout_json)
    {
        warn!("Failed to save tile layout: {e}");
    }
}

pub fn set_sync_command_tx(
    &mut self,
    tx: Option<tokio_mpsc::Sender<crate::mods::native::verse::SyncCommand>>,
) {
    self.services.sync_command_tx = tx;
}

pub fn request_sync_all_trusted_peers(&self, workspace_id: &str) -> Result<usize, String> {
    let Some(tx) = self.services.sync_command_tx.clone() else {
        return Err("sync worker command channel unavailable".to_string());
    };
    let peers = crate::mods::native::verse::get_trusted_peers();
    let mut enqueued = 0usize;
    for peer in peers {
        if tx
            .try_send(crate::mods::native::verse::SyncCommand::SyncWorkspace {
                peer: peer.node_id,
                workspace_id: workspace_id.to_string(),
            })
            .is_ok()
        {
            enqueued += 1;
        }
    }
    Ok(enqueued)
}

/// Load serialized tile layout JSON from persistence.
pub fn load_tile_layout_json(&self) -> Option<String> {
    self.services
        .persistence
        .as_ref()
        .and_then(|store| store.load_tile_layout_json())
}

/// Persist serialized tile layout JSON under a workspace name.
pub fn save_workspace_layout_json(&mut self, name: &str, layout_json: &str) {
    if let Some(store) = &mut self.services.persistence
        && let Err(e) = store.save_workspace_layout_json(name, layout_json)
    {
        warn!("Failed to save frame layout '{name}': {e}");
    }
    if !Self::is_reserved_workspace_layout_name(name) {
        self.workspace.workbench_session.current_workspace_is_synthesized = false;
        self.workspace.workbench_session.workspace_has_unsaved_changes = false;
        self.workspace.workbench_session.unsaved_workspace_prompt_warned = false;
    }
}

fn layout_json_hash(layout_json: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    layout_json.hash(&mut hasher);
    hasher.finish()
}

fn session_workspace_history_key(index: u8) -> String {
    format!("{}{index}", Self::SESSION_WORKSPACE_PREV_PREFIX)
}

fn rotate_session_workspace_history(&mut self, latest_layout_before_overwrite: &str) {
    let retention = self.workspace.workbench_session.workspace_autosave_retention;
    if retention == 0 {
        return;
    }

    for idx in (1..retention).rev() {
        let from_key = Self::session_workspace_history_key(idx);
        let to_key = Self::session_workspace_history_key(idx + 1);
        if let Some(layout) = self.load_workspace_layout_json(&from_key) {
            self.save_workspace_layout_json(&to_key, &layout);
        }
    }
    let first_key = Self::session_workspace_history_key(1);
    self.save_workspace_layout_json(&first_key, latest_layout_before_overwrite);
}

/// Persist reserved session frame layout only when the live runtime layout changes.
///
/// The persisted payload for `SESSION_WORKSPACE_LAYOUT_NAME` is the canonical
/// runtime `egui_tiles::Tree<TileKind>` JSON.
pub fn save_session_workspace_layout_json_if_changed(&mut self, layout_json: &str) {
    let next_hash = Self::layout_json_hash(layout_json);
    if self.workspace.workbench_session.last_session_workspace_layout_hash == Some(next_hash) {
        return;
    }
    if let Some(last_at) = self.workspace.workbench_session.last_workspace_autosave_at
        && last_at.elapsed() < self.workspace.workbench_session.workspace_autosave_interval
    {
        return;
    }
    let previous_latest = self.load_workspace_layout_json(Self::SESSION_WORKSPACE_LAYOUT_NAME);
    self.save_workspace_layout_json(Self::SESSION_WORKSPACE_LAYOUT_NAME, layout_json);
    if let Some(previous_latest) = previous_latest {
        self.rotate_session_workspace_history(&previous_latest);
    }
    self.workspace.workbench_session.last_session_workspace_layout_hash = Some(next_hash);
    self.workspace.workbench_session.last_session_workspace_layout_json = Some(layout_json.to_string());
    self.workspace.workbench_session.last_workspace_autosave_at = Some(Instant::now());
}

/// Mark currently loaded layout as session baseline to suppress redundant writes.
pub fn mark_session_workspace_layout_json(&mut self, layout_json: &str) {
    self.workspace.workbench_session.last_session_workspace_layout_hash = Some(Self::layout_json_hash(layout_json));
    self.workspace.workbench_session.last_session_workspace_layout_json = Some(layout_json.to_string());
    self.workspace.workbench_session.last_workspace_autosave_at = Some(Instant::now());
}

/// Mark currently loaded layout as session baseline to suppress redundant writes.
pub fn mark_session_frame_layout_json(&mut self, layout_json: &str) {
    self.mark_session_workspace_layout_json(layout_json);
}

pub fn last_session_workspace_layout_json(&self) -> Option<&str> {
    self.workspace.workbench_session.last_session_workspace_layout_json.as_deref()
}

/// Load serialized tile layout JSON by workspace name.
pub fn load_workspace_layout_json(&self, name: &str) -> Option<String> {
    self.services
        .persistence
        .as_ref()
        .and_then(|store| store.load_workspace_layout_json(name))
}

/// List persisted frame layout names in stable order.
pub fn list_workspace_layout_names(&self) -> Vec<String> {
    self.services
        .persistence
        .as_ref()
        .map(|store| store.list_workspace_layout_names())
        .unwrap_or_default()
}

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
        || name == Self::SETTINGS_OMNIBAR_PREFERRED_SCOPE_NAME
        || name == Self::SETTINGS_OMNIBAR_NON_AT_ORDER_NAME
        || name == Self::SETTINGS_WRY_ENABLED_NAME
        || name == Self::SETTINGS_GRAPH_VIEW_LAYOUT_MANAGER_NAME
        || name == Self::SETTINGS_REGISTRY_LENS_ID_NAME
        || name == Self::SETTINGS_REGISTRY_PHYSICS_ID_NAME
        || name == Self::SETTINGS_REGISTRY_THEME_ID_NAME
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
        &self.workspace.chrome_ui.command_palette_shortcut.to_string(),
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
    self.workspace.chrome_ui.lasso_binding_preference
}

pub fn set_lasso_binding_preference(&mut self, binding: CanvasLassoBinding) {
    self.workspace.chrome_ui.lasso_binding_preference = binding;
    self.save_lasso_binding_preference();
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
        &self.workspace.chrome_ui.context_command_surface_preference.to_string(),
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
        &self.workspace.chrome_ui.lasso_binding_preference.to_string(),
    );
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

pub fn set_default_registry_lens_id(&mut self, lens_id: Option<&str>) {
    let normalized = Self::normalize_optional_registry_id(lens_id.map(str::to_owned));
    self.workspace.chrome_ui.default_registry_lens_id = normalized.clone();
    self.save_workspace_layout_json(
        Self::SETTINGS_REGISTRY_LENS_ID_NAME,
        normalized.as_deref().unwrap_or(""),
    );
}

pub fn set_default_registry_physics_id(&mut self, physics_id: Option<&str>) {
    let normalized = Self::normalize_optional_registry_id(physics_id.map(str::to_owned));
    self.workspace.chrome_ui.default_registry_physics_id = normalized.clone();
    self.save_workspace_layout_json(
        Self::SETTINGS_REGISTRY_PHYSICS_ID_NAME,
        normalized.as_deref().unwrap_or(""),
    );
}

pub fn set_default_registry_theme_id(&mut self, theme_id: Option<&str>) {
    let normalized = Self::normalize_optional_registry_id(theme_id.map(str::to_owned));
    let persisted = normalized.as_deref().map(|requested| {
        crate::shell::desktop::runtime::registries::phase3_set_active_theme(requested).resolved_id
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
    self.workspace.chrome_ui.default_registry_physics_id.as_deref()
}

pub fn default_registry_theme_id(&self) -> Option<&str> {
    self.workspace.chrome_ui.default_registry_theme_id.as_deref()
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

fn load_persisted_ui_settings(&mut self) {
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
    if let Some(raw) = self.load_workspace_layout_json(Self::SETTINGS_COMMAND_PALETTE_SHORTCUT_NAME)
    {
        if let Ok(shortcut) = raw.parse::<CommandPaletteShortcut>() {
            self.workspace.chrome_ui.command_palette_shortcut = shortcut;
        } else {
            warn!("Ignoring invalid persisted command-palette shortcut: '{raw}'");
        }
    }
    if let Some(raw) = self.load_workspace_layout_json(Self::SETTINGS_HELP_PANEL_SHORTCUT_NAME) {
        if let Ok(shortcut) = raw.parse::<HelpPanelShortcut>() {
            self.workspace.chrome_ui.help_panel_shortcut = shortcut;
        } else {
            warn!("Ignoring invalid persisted help-panel shortcut: '{raw}'");
        }
    }
    if let Some(raw) = self.load_workspace_layout_json(Self::SETTINGS_RADIAL_MENU_SHORTCUT_NAME) {
        if let Ok(shortcut) = raw.parse::<RadialMenuShortcut>() {
            self.workspace.chrome_ui.radial_menu_shortcut = shortcut;
        } else {
            warn!("Ignoring invalid persisted radial-menu shortcut: '{raw}'");
        }
    }
    if let Some(raw) = self.load_workspace_layout_json(Self::SETTINGS_CONTEXT_COMMAND_SURFACE_NAME)
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
    if let Some(raw) = self.load_workspace_layout_json(Self::SETTINGS_KEYBOARD_PAN_INPUT_MODE_NAME)
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
            "true" | "1" | "yes" | "on" => self.workspace.chrome_ui.camera_pan_inertia_enabled = true,
            "false" | "0" | "no" | "off" => self.workspace.chrome_ui.camera_pan_inertia_enabled = false,
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
    if let Some(raw) = self.load_workspace_layout_json(Self::SETTINGS_OMNIBAR_PREFERRED_SCOPE_NAME)
    {
        if let Ok(scope) = raw.parse::<OmnibarPreferredScope>() {
            self.workspace.chrome_ui.omnibar_preferred_scope = scope;
        } else {
            warn!("Ignoring invalid persisted omnibar preferred scope: '{raw}'");
        }
    }
    if let Some(raw) = self.load_workspace_layout_json(Self::SETTINGS_OMNIBAR_NON_AT_ORDER_NAME) {
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
    if let Some(theme_id) = self.workspace.chrome_ui.default_registry_theme_id.as_deref() {
        let resolution = crate::shell::desktop::runtime::registries::phase3_set_active_theme(theme_id);
        self.workspace.chrome_ui.default_registry_theme_id = Some(resolution.resolved_id);
    }
    self.load_graph_view_layout_manager_state();

    crate::registries::atomic::diagnostics::apply_persisted_channel_configs(
        self.diagnostics_channel_configs(),
    );
}

fn normalize_optional_registry_id(raw: Option<String>) -> Option<String> {
    raw.and_then(|value| {
        let normalized = value.trim().to_ascii_lowercase();
        (!normalized.is_empty()).then_some(normalized)
    })
}

fn with_registry_lens_defaults(&self, mut lens: LensConfig) -> LensConfig {
    if lens.lens_id.is_none() {
        lens.lens_id = self.workspace.chrome_ui.default_registry_lens_id.clone();
    }
    lens
}

/// Delete a persisted frame layout by name.
pub fn delete_workspace_layout(&mut self, name: &str) -> Result<(), String> {
    if Self::is_reserved_workspace_layout_name(name) {
        return Err(format!("Cannot delete reserved workspace '{name}'"));
    }
    self.services
        .persistence
        .as_mut()
        .ok_or_else(|| "Persistence is not enabled".to_string())?
        .delete_workspace_layout(name)
        .map_err(|e| e.to_string())?;
    self.remove_named_workbench_frame_graph_representation(name);
    self.workspace
        .node_last_active_workspace
        .retain(|_, (_, workspace_name)| workspace_name != name);
    for memberships in self.workspace.workbench_session.node_workspace_membership.values_mut() {
        memberships.remove(name);
    }
    self.workspace
        .node_workspace_membership
        .retain(|_, memberships| !memberships.is_empty());
    self.workspace.graph_runtime.egui_state_dirty = true;
    self.emit_ux_navigation_transition();
    Ok(())
}

/// Delete the reserved session frame snapshot and reset hash baseline.
pub fn clear_session_workspace_layout(&mut self) -> Result<(), String> {
    let mut names_to_delete = vec![Self::SESSION_WORKSPACE_LAYOUT_NAME.to_string()];
    for idx in 1..=5 {
        names_to_delete.push(Self::session_workspace_history_key(idx));
    }
    let store = self
        .services
        .persistence
        .as_mut()
        .ok_or_else(|| "Persistence is not enabled".to_string())?;
    for name in names_to_delete {
        let _ = store.delete_workspace_layout(&name);
    }
    self.workspace.workbench_session.last_session_workspace_layout_hash = None;
    self.workspace.workbench_session.last_session_workspace_layout_json = None;
    self.workspace.workbench_session.last_workspace_autosave_at = None;
    Ok(())
}

pub fn workspace_autosave_interval_secs(&self) -> u64 {
    self.workspace.workbench_session.workspace_autosave_interval.as_secs()
}

pub fn set_workspace_autosave_interval_secs(&mut self, secs: u64) -> Result<(), String> {
    if secs == 0 {
        return Err("Workspace autosave interval must be greater than zero".to_string());
    }
    self.workspace.workbench_session.workspace_autosave_interval = Duration::from_secs(secs);
    Ok(())
}

pub fn workspace_autosave_retention(&self) -> u8 {
    self.workspace.workbench_session.workspace_autosave_retention
}

pub fn set_workspace_autosave_retention(&mut self, count: u8) -> Result<(), String> {
    if count > 5 {
        return Err("Workspace autosave retention must be between 0 and 5".to_string());
    }
    if count < self.workspace.workbench_session.workspace_autosave_retention
        && let Some(store) = self.services.persistence.as_mut()
    {
        for idx in (count + 1)..=5 {
            let _ = store.delete_workspace_layout(&Self::session_workspace_history_key(idx));
        }
    }
    self.workspace.workbench_session.workspace_autosave_retention = count;
    Ok(())
}
