use std::path::Path;

use super::*;
use crate::app::runtime_ports::registries;

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

impl DefaultWebViewerBackend {
    /// Map the user-facing web-backend setting to the verso routing
    /// authority's preference type. Centralizes the conversion so
    /// call sites stop open-coding the match.
    pub fn web_engine_preference(self) -> ::verso::WebEnginePreference {
        match self {
            Self::Servo => ::verso::WebEnginePreference::Servo,
            Self::Wry => ::verso::WebEnginePreference::Wry,
        }
    }
}

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
pub enum NavigatorSidebarSidePreference {
    #[default]
    Left,
    Right,
}

impl_display_from_str!(NavigatorSidebarSidePreference {
    NavigatorSidebarSidePreference::Left => "left",
    NavigatorSidebarSidePreference::Right => "right",
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

// `FocusRingCurve` moved to `graphshell_core::shell_state::frame_model`
// in M4 slice 8 (2026-04-22). Re-exported here so existing
// `crate::app::FocusRingCurve` imports resolve unchanged. The
// portable version carries its own `Display`/`FromStr` impls with
// the same wire shape the old `impl_display_from_str!` macro
// produced ("linear", "ease_out", "step").
pub use graphshell_core::shell_state::frame_model::FocusRingCurve;

/// User-configurable focus-ring behavior. Lives on
/// `ChromeUiState::focus_ring_settings`; defaults match the historical
/// hardcoded behavior (500 ms linear fade, theme color).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FocusRingSettings {
    /// Whether to paint the focus ring at all. Set to `false` for
    /// reduced-motion preferences; render paths will treat
    /// `focus_ring_alpha` as `0.0`.
    #[serde(default = "FocusRingSettings::default_enabled")]
    pub enabled: bool,

    /// Duration of the fade-out animation in milliseconds. Clamped to
    /// [`FocusRingSettings::MIN_DURATION_MS`]..=[`FocusRingSettings::MAX_DURATION_MS`]
    /// by the setter.
    #[serde(default = "FocusRingSettings::default_duration_ms")]
    pub duration_ms: u32,

    /// Reshape applied to the linear fade-out progress. See
    /// [`FocusRingCurve`].
    #[serde(default)]
    pub curve: FocusRingCurve,

    /// Optional user-chosen ring color (RGB). `None` (default) means
    /// inherit the active presentation theme's `focus_ring` color.
    /// `Some([r, g, b])` overrides it — alpha is still modulated by
    /// the ring animation.
    #[serde(default)]
    pub color_override: Option<[u8; 3]>,
}

impl FocusRingSettings {
    pub const MIN_DURATION_MS: u32 = 0;
    pub const MAX_DURATION_MS: u32 = 5_000;
    pub const DEFAULT_DURATION_MS: u32 = 500;

    fn default_enabled() -> bool {
        true
    }

    fn default_duration_ms() -> u32 {
        Self::DEFAULT_DURATION_MS
    }

    pub fn duration(&self) -> std::time::Duration {
        std::time::Duration::from_millis(u64::from(self.duration_ms))
    }
}

impl Default for FocusRingSettings {
    fn default() -> Self {
        Self {
            enabled: Self::default_enabled(),
            duration_ms: Self::default_duration_ms(),
            curve: FocusRingCurve::Linear,
            color_override: None,
        }
    }
}

/// Resampling filter applied when the thumbnail pipeline scales a
/// captured screenshot down to the configured width / height. Maps 1:1
/// onto `image::imageops::FilterType`.
///
/// Quality/speed tradeoff:
/// - [`Nearest`](ThumbnailFilter::Nearest) — fastest, chunky (pixelated look); acceptable
///   for tiny thumbnails where speed dominates.
/// - [`Triangle`](ThumbnailFilter::Triangle) — default; reasonable quality at moderate
///   cost (linear interpolation).
/// - [`CatmullRom`](ThumbnailFilter::CatmullRom) — cubic, smoother edges than Triangle.
/// - [`Gaussian`](ThumbnailFilter::Gaussian) — soft, good for screenshots with fine text.
/// - [`Lanczos3`](ThumbnailFilter::Lanczos3) — highest quality, slowest; best for hi-DPI
///   display thumbnails where sharpness matters.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize,
)]
pub enum ThumbnailFilter {
    Nearest,
    #[default]
    Triangle,
    CatmullRom,
    Gaussian,
    Lanczos3,
}

impl_display_from_str!(ThumbnailFilter {
    ThumbnailFilter::Nearest => "nearest",
    ThumbnailFilter::Triangle => "triangle",
    ThumbnailFilter::CatmullRom => "catmull_rom",
    ThumbnailFilter::Gaussian => "gaussian",
    ThumbnailFilter::Lanczos3 => "lanczos3",
});

/// Encoded format for cached thumbnail bytes. PNG is lossless and
/// larger; JPEG is lossy and smaller, with a quality knob on
/// [`ThumbnailSettings::jpeg_quality`]. Downstream decoders use
/// `image::load_from_memory` (magic-byte detection) so caches mixed
/// across format toggles coexist cleanly — stale PNG bytes stay
/// decodable after the user switches to JPEG and vice versa; they
/// just get replaced at the next re-capture.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize,
)]
pub enum ThumbnailFormat {
    /// Lossless PNG (default). Bigger files, crisp at any zoom.
    #[default]
    Png,
    /// Lossy JPEG. Smaller files; alpha is composited to opaque before
    /// encoding because JPEG has no alpha channel.
    Jpeg,
    /// Lossless WebP. Typically ~20–30% smaller than PNG at the same
    /// visual fidelity on screenshot content. Preserves alpha.
    ///
    /// No quality knob is exposed because this variant is
    /// deliberately lossless. Pure-Rust lossy WebP encoding does not
    /// exist in the ecosystem as of this codebase's dependencies — the
    /// options are FFI-to-libwebp (native dependency, build-system
    /// cost, platform-specific fallout) or vendored C. At thumbnail
    /// scale (≤1024×1024) the filesize/quality win of lossy WebP over
    /// JPEG at matched quality is typically single-digit percent, so
    /// users who want small lossy thumbnails should pick `Jpeg` with a
    /// quality slider; those who want small + alpha pick this variant.
    /// If the tradeoff ever shifts (pure-Rust lossy WebP lands, or
    /// thumbnail sizes grow to where the compression delta matters),
    /// revisit — the encoder dispatch in
    /// `thumbnail_pipeline::encode_thumbnail` is the only site that
    /// needs to grow a fourth arm.
    WebP,
}

impl_display_from_str!(ThumbnailFormat {
    ThumbnailFormat::Png => "png",
    ThumbnailFormat::Jpeg => "jpeg",
    ThumbnailFormat::WebP => "webp",
});

/// Aspect-ratio policy applied when scaling a captured screenshot
/// down to the target thumbnail size. Changes which variant of the
/// `image::imageops::FilterType` resize path the pipeline picks
/// (`resize_to_fill` for crop-to-box, `resize` for preserve-aspect).
///
/// - [`Fixed`](ThumbnailAspect::Fixed) — historical behavior; use `width × height`
///   verbatim, crop the source to match (may letterbox-style lose
///   pixels when source aspect ≠ target aspect).
/// - [`MatchSource`](ThumbnailAspect::MatchSource) — preserve the source's aspect
///   ratio; the longer side is scaled to `max(width, height)`. Good
///   for mixed-aspect browsing (phones portrait, monitors landscape).
/// - [`Square`](ThumbnailAspect::Square) — force square output using `width`
///   for both dimensions. Matches tile grids that expect 1:1.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize,
)]
pub enum ThumbnailAspect {
    #[default]
    Fixed,
    MatchSource,
    Square,
}

impl_display_from_str!(ThumbnailAspect {
    ThumbnailAspect::Fixed => "fixed",
    ThumbnailAspect::MatchSource => "match_source",
    ThumbnailAspect::Square => "square",
});

/// User-configurable node-thumbnail capture behavior. Lives on
/// `ChromeUiState::thumbnail_settings`; defaults match the historical
/// hardcoded behavior (enabled, 256×192, Triangle filter, PNG format).
///
/// Cadence note: refresh intervals for active vs warm preview
/// thumbnails live on the sibling [`ChromeUiState`] fields
/// `webview_preview_active_refresh_secs` and
/// `webview_preview_warm_refresh_secs`. They're deliberately separate
/// from `ThumbnailSettings` because the capture pipeline doesn't read
/// them — the window's preview scheduler does. Present them together
/// in a single UI panel; keep them separate at the data model level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ThumbnailSettings {
    /// Master kill switch. When `false`, the thumbnail pipeline skips
    /// new captures entirely — useful for privacy (no page-content
    /// screenshots cached), low-memory devices, or slow GPUs.
    /// In-flight captures still complete and drain normally.
    #[serde(default = "ThumbnailSettings::default_enabled")]
    pub enabled: bool,

    /// Thumbnail target width in pixels. Clamped to
    /// [`ThumbnailSettings::MIN_DIMENSION`]..=[`ThumbnailSettings::MAX_DIMENSION`]
    /// by the setter. Aspect is preserved against `height`; capture uses
    /// `resize_to_fill` which crops to match.
    #[serde(default = "ThumbnailSettings::default_width")]
    pub width: u32,

    /// Thumbnail target height in pixels. See [`Self::width`].
    #[serde(default = "ThumbnailSettings::default_height")]
    pub height: u32,

    /// Resampling filter for the downscale pass. See [`ThumbnailFilter`]
    /// for quality/speed tradeoffs.
    #[serde(default)]
    pub filter: ThumbnailFilter,

    /// Encoded format. Default is [`ThumbnailFormat::Png`] (historical
    /// behavior). [`ThumbnailFormat::Jpeg`] uses the [`Self::jpeg_quality`]
    /// knob to trade filesize for quality.
    #[serde(default)]
    pub format: ThumbnailFormat,

    /// Encoder quality for JPEG output, on a 1..=100 scale. Clamped
    /// into range by [`Self::clamp_dimensions`]. Ignored when
    /// `format != ThumbnailFormat::Jpeg`. Default 85 is a common
    /// sweet-spot for UI thumbnails (near-visually-lossless at roughly
    /// a third the size of the PNG equivalent for typical screenshots).
    #[serde(default = "ThumbnailSettings::default_jpeg_quality")]
    pub jpeg_quality: u8,

    /// Aspect-ratio policy for the downscale pass. See
    /// [`ThumbnailAspect`]. Default [`ThumbnailAspect::Fixed`]
    /// preserves the pre-M4.1 behavior (crop-to-box at `width × height`).
    #[serde(default)]
    pub aspect: ThumbnailAspect,
}

impl ThumbnailSettings {
    pub const MIN_DIMENSION: u32 = 64;
    pub const MAX_DIMENSION: u32 = 1024;
    pub const DEFAULT_WIDTH: u32 = 256;
    pub const DEFAULT_HEIGHT: u32 = 192;
    pub const MIN_JPEG_QUALITY: u8 = 1;
    pub const MAX_JPEG_QUALITY: u8 = 100;
    pub const DEFAULT_JPEG_QUALITY: u8 = 85;

    fn default_enabled() -> bool {
        true
    }

    fn default_width() -> u32 {
        Self::DEFAULT_WIDTH
    }

    fn default_height() -> u32 {
        Self::DEFAULT_HEIGHT
    }

    fn default_jpeg_quality() -> u8 {
        Self::DEFAULT_JPEG_QUALITY
    }

    /// Normalize `width`/`height` and `jpeg_quality` into their
    /// supported ranges. Idempotent. Setter plumbing calls this so UI
    /// sliders can pass raw values without separately validating.
    pub fn clamp_dimensions(mut self) -> Self {
        self.width = self.width.clamp(Self::MIN_DIMENSION, Self::MAX_DIMENSION);
        self.height = self.height.clamp(Self::MIN_DIMENSION, Self::MAX_DIMENSION);
        self.jpeg_quality = self
            .jpeg_quality
            .clamp(Self::MIN_JPEG_QUALITY, Self::MAX_JPEG_QUALITY);
        self
    }
}

impl Default for ThumbnailSettings {
    fn default() -> Self {
        Self {
            enabled: Self::default_enabled(),
            width: Self::default_width(),
            height: Self::default_height(),
            filter: ThumbnailFilter::Triangle,
            format: ThumbnailFormat::Png,
            jpeg_quality: Self::default_jpeg_quality(),
            aspect: ThumbnailAspect::Fixed,
        }
    }
}

impl GraphBrowserApp {
    pub(crate) const SETTINGS_DEFAULT_WEB_VIEWER_BACKEND_NAME: &str =
        "settings.default_web_viewer_backend";
    pub(crate) const SETTINGS_WRY_RENDER_MODE_PREFERENCE_NAME: &str =
        "settings.wry_render_mode_preference";
    pub(crate) const SETTINGS_WORKSPACE_USER_STYLESHEETS_NAME: &str =
        "settings.workspace_user_stylesheets";
    pub(crate) const SETTINGS_FOCUS_RING_SETTINGS_NAME: &str = "settings.focus_ring_settings";
    pub(crate) const SETTINGS_THUMBNAIL_SETTINGS_NAME: &str = "settings.thumbnail_settings";
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
            || name == Self::SETTINGS_LASSO_BINDING_NAME
            || name == Self::SETTINGS_INPUT_BINDING_REMAPS_NAME
            || name == Self::SETTINGS_OMNIBAR_PREFERRED_SCOPE_NAME
            || name == Self::SETTINGS_OMNIBAR_NON_AT_ORDER_NAME
            || name == Self::SETTINGS_OMNIBAR_DROPDOWN_MAX_ROWS_NAME
            || name == Self::SETTINGS_TOOLBAR_HEIGHT_DP_NAME
            || name == Self::SETTINGS_OMNIBAR_PROVIDER_DEBOUNCE_MS_NAME
            || name == Self::SETTINGS_COMMAND_PALETTE_DEFAULT_SCOPE_NAME
            || name == Self::SETTINGS_COMMAND_PALETTE_MAX_PER_CATEGORY_NAME
            || name == Self::SETTINGS_COMMAND_PALETTE_RECENTS_DEPTH_NAME
            || name == Self::SETTINGS_COMMAND_PALETTE_RECENTS_NAME
            || name == Self::SETTINGS_COMMAND_PALETTE_TIER1_DEFAULT_CATEGORY_NAME
            || name == Self::SETTINGS_WRY_ENABLED_NAME
            || name == Self::SETTINGS_DEFAULT_WEB_VIEWER_BACKEND_NAME
            || name == Self::SETTINGS_WRY_RENDER_MODE_PREFERENCE_NAME
            || name == Self::SETTINGS_WORKSPACE_USER_STYLESHEETS_NAME
            || name == Self::SETTINGS_FOCUS_RING_SETTINGS_NAME
            || name == Self::SETTINGS_THUMBNAIL_SETTINGS_NAME
            || name == Self::SETTINGS_WEBVIEW_PREVIEW_ACTIVE_REFRESH_SECS_NAME
            || name == Self::SETTINGS_WEBVIEW_PREVIEW_WARM_REFRESH_SECS_NAME
            || name == Self::SETTINGS_NAVIGATOR_SIDEBAR_SIDE_NAME
            || name == Self::SETTINGS_WORKBENCH_DISPLAY_MODE_NAME
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

    pub fn focus_ring_settings(&self) -> FocusRingSettings {
        self.workspace.chrome_ui.focus_ring_settings
    }

    /// Update the focus-ring settings. Duration is clamped into the
    /// supported range; color override is accepted verbatim. The
    /// updated settings are persisted via
    /// [`Self::SETTINGS_FOCUS_RING_SETTINGS_NAME`].
    pub fn set_focus_ring_settings(&mut self, mut settings: FocusRingSettings) {
        settings.duration_ms = settings
            .duration_ms
            .clamp(FocusRingSettings::MIN_DURATION_MS, FocusRingSettings::MAX_DURATION_MS);
        self.workspace.chrome_ui.focus_ring_settings = settings;
        self.save_focus_ring_settings();
    }

    fn save_focus_ring_settings(&mut self) {
        let json = match serde_json::to_string(&self.workspace.chrome_ui.focus_ring_settings) {
            Ok(s) => s,
            Err(error) => {
                warn!("Failed to serialize focus ring settings for persistence: {error}");
                return;
            }
        };
        self.save_workspace_layout_json(Self::SETTINGS_FOCUS_RING_SETTINGS_NAME, &json);
    }

    pub fn thumbnail_settings(&self) -> ThumbnailSettings {
        self.workspace.chrome_ui.thumbnail_settings
    }

    /// Update thumbnail capture settings. Width/height are clamped into
    /// the supported range by [`ThumbnailSettings::clamp_dimensions`] so
    /// callers can pass raw slider values without separately validating.
    /// Persisted via [`Self::SETTINGS_THUMBNAIL_SETTINGS_NAME`].
    pub fn set_thumbnail_settings(&mut self, settings: ThumbnailSettings) {
        self.workspace.chrome_ui.thumbnail_settings = settings.clamp_dimensions();
        self.save_thumbnail_settings();
    }

    fn save_thumbnail_settings(&mut self) {
        let json = match serde_json::to_string(&self.workspace.chrome_ui.thumbnail_settings) {
            Ok(s) => s,
            Err(error) => {
                warn!("Failed to serialize thumbnail settings for persistence: {error}");
                return;
            }
        };
        self.save_workspace_layout_json(Self::SETTINGS_THUMBNAIL_SETTINGS_NAME, &json);
    }

    pub fn keyboard_pan_step(&self) -> f32 {
        self.workspace.chrome_ui.keyboard_pan_step
    }

    pub fn set_keyboard_pan_step(&mut self, step: f32) {
        let normalized = step.clamp(1.0, 200.0);
        self.workspace.chrome_ui.keyboard_pan_step = normalized;
        registries::phase3_set_active_canvas_keyboard_pan_step(normalized);
        self.save_keyboard_pan_step();
    }

    pub fn keyboard_pan_input_mode(&self) -> KeyboardPanInputMode {
        self.workspace.chrome_ui.keyboard_pan_input_mode
    }

    pub fn set_keyboard_pan_input_mode(&mut self, mode: KeyboardPanInputMode) {
        self.workspace.chrome_ui.keyboard_pan_input_mode = mode;
        self.save_keyboard_pan_input_mode();
    }

    // `camera_pan_inertia_*` used to live here as zombie workspace-global
    // preferences that persisted and displayed but never drove behavior.
    // Pan inertia on/off and damping now resolve through
    // `GraphBrowserApp::resolve_navigation_policy(view_id)` →
    // `NavigationPolicy::{pan_inertia_enabled, pan_damping_per_second}`
    // with per-view override + per-graph default. Removed 2026-04-20
    // per the Node Style configurability sweep follow-on.

    pub fn lasso_binding_preference(&self) -> CanvasLassoBinding {
        self.workspace.chrome_ui.lasso_binding_preference
    }

    pub fn set_lasso_binding_preference(&mut self, binding: CanvasLassoBinding) {
        self.workspace.chrome_ui.lasso_binding_preference = binding;
        registries::phase3_set_active_canvas_lasso_binding(binding);
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
            &registries::phase3_nostr_persisted_signer_settings(),
        )
        .unwrap_or_else(|_| "{\"backend\":\"local_host_key\"}".to_string());
        self.save_workspace_layout_json(Self::SETTINGS_NOSTR_SIGNER_SETTINGS_NAME, &encoded);
    }

    pub fn save_persisted_nostr_nip07_permissions(&mut self) {
        let encoded = serde_json::to_string(
            &registries::phase3_nostr_persisted_nip07_permissions(),
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
            registries::PersistedNostrSignerSettings,
        >(&raw)
        .map_err(|error| {
            warn!("Ignoring invalid persisted nostr signer settings: {error}");
            error
        })
        .ok() else {
            return;
        };
        if let Err(error) = registries::phase3_nostr_apply_persisted_signer_settings(&settings) {
            warn!("Ignoring persisted nostr signer settings restore failure: {error:?}");
        }
    }

    fn load_persisted_nostr_nip07_permissions(&mut self) {
        let permissions = self
            .load_workspace_layout_json(Self::SETTINGS_NOSTR_NIP07_PERMISSIONS_NAME)
            .and_then(|raw| {
                serde_json::from_str::<Vec<registries::Nip07PermissionGrant>>(&raw)
                .map_err(|error| {
                    warn!("Ignoring invalid persisted nostr nip07 permissions: {error}");
                    error
                })
                .ok()
            })
            .unwrap_or_default();

        if let Err(error) = registries::phase3_nostr_apply_persisted_nip07_permissions(&permissions)
        {
            warn!("Ignoring persisted nostr nip07 permissions restore failure: {error:?}");
        }
    }

    pub fn save_persisted_nostr_subscriptions(&mut self) {
        let encoded = serde_json::to_string(
            &registries::phase3_nostr_persisted_subscriptions(),
        )
        .unwrap_or_else(|_| "[]".to_string());
        self.save_workspace_layout_json(Self::SETTINGS_NOSTR_SUBSCRIPTIONS_NAME, &encoded);
    }

    fn load_persisted_nostr_subscriptions(&mut self) {
        let subscriptions = self
            .load_workspace_layout_json(Self::SETTINGS_NOSTR_SUBSCRIPTIONS_NAME)
            .and_then(|raw| {
                serde_json::from_str::<Vec<registries::PersistedNostrSubscription>>(&raw)
                .map_err(|error| {
                    warn!("Ignoring invalid persisted nostr subscriptions: {error}");
                    error
                })
                .ok()
            })
            .unwrap_or_default();

        if let Err(error) = registries::phase3_restore_nostr_subscriptions(&subscriptions) {
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

    pub fn omnibar_dropdown_max_rows(&self) -> usize {
        self.workspace.chrome_ui.omnibar_dropdown_max_rows
    }

    pub fn set_omnibar_dropdown_max_rows(&mut self, rows: usize) {
        self.workspace.chrome_ui.omnibar_dropdown_max_rows = rows.clamp(3, 24);
        self.save_omnibar_dropdown_max_rows();
    }

    fn save_omnibar_dropdown_max_rows(&mut self) {
        self.save_workspace_layout_json(
            Self::SETTINGS_OMNIBAR_DROPDOWN_MAX_ROWS_NAME,
            &self
                .workspace
                .chrome_ui
                .omnibar_dropdown_max_rows
                .to_string(),
        );
    }

    pub fn toolbar_height_dp(&self) -> f32 {
        self.workspace.chrome_ui.toolbar_height_dp
    }

    pub fn set_toolbar_height_dp(&mut self, height: f32) {
        self.workspace.chrome_ui.toolbar_height_dp = height.clamp(24.0, 96.0);
        self.save_toolbar_height_dp();
    }

    fn save_toolbar_height_dp(&mut self) {
        self.save_workspace_layout_json(
            Self::SETTINGS_TOOLBAR_HEIGHT_DP_NAME,
            &self.workspace.chrome_ui.toolbar_height_dp.to_string(),
        );
    }

    pub fn omnibar_provider_debounce_ms(&self) -> u64 {
        self.workspace.chrome_ui.omnibar_provider_debounce_ms
    }

    pub fn set_omnibar_provider_debounce_ms(&mut self, ms: u64) {
        self.workspace.chrome_ui.omnibar_provider_debounce_ms = ms.min(2000);
        self.save_omnibar_provider_debounce_ms();
    }

    fn save_omnibar_provider_debounce_ms(&mut self) {
        self.save_workspace_layout_json(
            Self::SETTINGS_OMNIBAR_PROVIDER_DEBOUNCE_MS_NAME,
            &self
                .workspace
                .chrome_ui
                .omnibar_provider_debounce_ms
                .to_string(),
        );
    }

    pub fn command_palette_default_scope(
        &self,
    ) -> crate::shell::desktop::ui::command_palette_state::SearchPaletteScope {
        self.workspace.chrome_ui.command_palette_default_scope
    }

    pub fn set_command_palette_default_scope(
        &mut self,
        scope: crate::shell::desktop::ui::command_palette_state::SearchPaletteScope,
    ) {
        self.workspace.chrome_ui.command_palette_default_scope = scope;
        self.save_command_palette_default_scope();
    }

    fn save_command_palette_default_scope(&mut self) {
        self.save_workspace_layout_json(
            Self::SETTINGS_COMMAND_PALETTE_DEFAULT_SCOPE_NAME,
            &self.workspace.chrome_ui.command_palette_default_scope.to_string(),
        );
    }

    pub fn command_palette_max_per_category(&self) -> usize {
        self.workspace.chrome_ui.command_palette_max_per_category
    }

    pub fn set_command_palette_max_per_category(&mut self, cap: usize) {
        self.workspace.chrome_ui.command_palette_max_per_category = cap.min(100);
        self.save_command_palette_max_per_category();
    }

    fn save_command_palette_max_per_category(&mut self) {
        self.save_workspace_layout_json(
            Self::SETTINGS_COMMAND_PALETTE_MAX_PER_CATEGORY_NAME,
            &self
                .workspace
                .chrome_ui
                .command_palette_max_per_category
                .to_string(),
        );
    }

    pub fn command_palette_recents_depth(&self) -> usize {
        self.workspace.chrome_ui.command_palette_recents_depth
    }

    pub fn set_command_palette_recents_depth(&mut self, depth: usize) {
        let clamped = depth.min(32);
        self.workspace.chrome_ui.command_palette_recents_depth = clamped;
        if self.workspace.chrome_ui.command_palette_recents.len() > clamped {
            self.workspace
                .chrome_ui
                .command_palette_recents
                .truncate(clamped);
            self.save_command_palette_recents();
        }
        self.save_command_palette_recents_depth();
    }

    fn save_command_palette_recents_depth(&mut self) {
        self.save_workspace_layout_json(
            Self::SETTINGS_COMMAND_PALETTE_RECENTS_DEPTH_NAME,
            &self
                .workspace
                .chrome_ui
                .command_palette_recents_depth
                .to_string(),
        );
    }

    /// Bump `action_id` to the head of the recents ring. De-duplicates
    /// (removing any prior occurrence), then truncates to the current
    /// `command_palette_recents_depth`. No-op when depth is `0`.
    pub fn record_command_palette_recent(
        &mut self,
        action_id: crate::render::action_registry::ActionId,
    ) {
        let depth = self.workspace.chrome_ui.command_palette_recents_depth;
        if depth == 0 {
            if !self.workspace.chrome_ui.command_palette_recents.is_empty() {
                self.workspace.chrome_ui.command_palette_recents.clear();
                self.save_command_palette_recents();
            }
            return;
        }
        let recents = &mut self.workspace.chrome_ui.command_palette_recents;
        recents.retain(|existing| *existing != action_id);
        recents.insert(0, action_id);
        if recents.len() > depth {
            recents.truncate(depth);
        }
        self.save_command_palette_recents();
    }

    /// Drop the recents ring entirely (e.g. "Forget recent commands").
    pub fn clear_command_palette_recents(&mut self) {
        if !self.workspace.chrome_ui.command_palette_recents.is_empty() {
            self.workspace.chrome_ui.command_palette_recents.clear();
            self.save_command_palette_recents();
        }
    }

    fn save_command_palette_recents(&mut self) {
        let encoded = match serde_json::to_string(
            &self.workspace.chrome_ui.command_palette_recents,
        ) {
            Ok(s) => s,
            Err(error) => {
                warn!("Failed to serialize command palette recents: {error}");
                return;
            }
        };
        self.save_workspace_layout_json(
            Self::SETTINGS_COMMAND_PALETTE_RECENTS_NAME,
            &encoded,
        );
    }

    pub fn command_palette_tier1_default_category(
        &self,
    ) -> Option<crate::render::action_registry::ActionCategory> {
        self.workspace.chrome_ui.command_palette_tier1_default_category
    }

    pub fn set_command_palette_tier1_default_category(
        &mut self,
        category: Option<crate::render::action_registry::ActionCategory>,
    ) {
        if self.workspace.chrome_ui.command_palette_tier1_default_category == category {
            return;
        }
        self.workspace.chrome_ui.command_palette_tier1_default_category = category;
        self.save_command_palette_tier1_default_category();
    }

    fn save_command_palette_tier1_default_category(&mut self) {
        let encoded = match self.workspace.chrome_ui.command_palette_tier1_default_category {
            Some(category) => {
                crate::render::action_registry::category_persisted_name(category).to_string()
            }
            None => String::new(),
        };
        self.save_workspace_layout_json(
            Self::SETTINGS_COMMAND_PALETTE_TIER1_DEFAULT_CATEGORY_NAME,
            &encoded,
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

    pub fn navigator_sidebar_side_preference(&self) -> NavigatorSidebarSidePreference {
        self.workspace.chrome_ui.navigator_sidebar_side_preference
    }

    pub fn set_navigator_sidebar_side_preference(
        &mut self,
        preference: NavigatorSidebarSidePreference,
    ) {
        self.workspace.chrome_ui.navigator_sidebar_side_preference = preference;
        self.save_navigator_sidebar_side_preference();
    }

    pub fn preferred_default_navigator_surface_host(&self) -> SurfaceHostId {
        crate::app::workbench_layout_policy::navigator_surface_host_for_sidebar_side(
            self.navigator_sidebar_side_preference(),
        )
    }

    pub fn workbench_display_mode(&self) -> WorkbenchDisplayMode {
        self.workspace.chrome_ui.workbench_display_mode
    }

    pub fn set_workbench_display_mode(&mut self, mode: WorkbenchDisplayMode) {
        self.workspace.chrome_ui.workbench_display_mode = mode;
        if matches!(mode, WorkbenchDisplayMode::Dedicated) {
            self.workspace.chrome_ui.show_workbench_overlay = false;
        }
        self.save_workbench_display_mode();
    }

    pub fn workbench_host_pinned(&self) -> bool {
        self.workspace.chrome_ui.workbench_host_pinned
    }

    pub fn set_workbench_host_pinned(&mut self, pinned: bool) {
        self.workspace.chrome_ui.workbench_host_pinned = pinned;
        self.save_workbench_host_pinned();
    }

    pub fn workbench_overlay_visible(&self) -> bool {
        self.workspace.chrome_ui.show_workbench_overlay
    }

    pub fn set_workbench_overlay_visible(&mut self, visible: bool) {
        if visible
            && matches!(
                self.workbench_display_mode(),
                WorkbenchDisplayMode::Dedicated
            )
        {
            return;
        }
        self.workspace.chrome_ui.show_workbench_overlay = visible;
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

    fn save_navigator_sidebar_side_preference(&mut self) {
        self.save_workspace_layout_json(
            Self::SETTINGS_NAVIGATOR_SIDEBAR_SIDE_NAME,
            &self
                .workspace
                .chrome_ui
                .navigator_sidebar_side_preference
                .to_string(),
        );
    }

    fn save_workbench_display_mode(&mut self) {
        self.save_workspace_layout_json(
            Self::SETTINGS_WORKBENCH_DISPLAY_MODE_NAME,
            &self.workspace.chrome_ui.workbench_display_mode.to_string(),
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
        registries::phase3_publish_lens_changed(normalized.as_deref());
        self.save_workspace_layout_json(
            Self::SETTINGS_REGISTRY_LENS_ID_NAME,
            normalized.as_deref().unwrap_or(""),
        );
    }

    pub fn set_default_registry_physics_id(&mut self, physics_id: Option<&str>) {
        let normalized = Self::normalize_optional_registry_id(physics_id.map(str::to_owned));
        let resolution = registries::phase3_set_active_physics_profile(
            normalized
                .as_deref()
                .unwrap_or(crate::registries::atomic::lens::PHYSICS_ID_DEFAULT),
        );
        let persisted = normalized.as_ref().map(|_| resolution.resolved_id.clone());
        self.workspace.chrome_ui.default_registry_physics_id = persisted.clone();
        self.apply_physics_profile(&resolution.resolved_id, &resolution.profile);
        self.save_workspace_layout_json(
            Self::SETTINGS_REGISTRY_PHYSICS_ID_NAME,
            persisted.as_deref().unwrap_or(""),
        );
    }

    pub fn set_default_registry_theme_id(&mut self, theme_id: Option<&str>) {
        let normalized = Self::normalize_optional_registry_id(theme_id.map(str::to_owned));
        let persisted = normalized
            .as_deref()
            .map(|requested| registries::phase3_set_active_theme(requested).resolved_id);
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
        registries::phase3_set_theme_follows_system(follows_system);
        match mode {
            ThemeMode::System => {
                // Clear explicit override — runtime will follow OS events.
                self.workspace.chrome_ui.default_registry_theme_id = None;
                self.save_workspace_layout_json(Self::SETTINGS_REGISTRY_THEME_ID_NAME, "");
            }
            ThemeMode::Light => {
                self.set_default_registry_theme_id(Some(registries::theme::THEME_ID_LIGHT));
            }
            ThemeMode::Dark => {
                self.set_default_registry_theme_id(Some(registries::theme::THEME_ID_DARK));
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
        // Legacy `settings.camera_pan_inertia_*` layout-json keys used
        // to load into `workspace.chrome_ui.camera_pan_inertia_*` here.
        // Those fields were zombies — persisted and displayed but never
        // consumed by the actual inertia tick — and were removed on
        // 2026-04-20. Any old workspace JSON carrying those keys is
        // silently ignored; pan inertia now lives on `NavigationPolicy`
        // (per-view override + per-graph default).
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
        self.workspace.chrome_ui.omnibar_dropdown_max_rows = self
            .load_workspace_layout_json(Self::SETTINGS_OMNIBAR_DROPDOWN_MAX_ROWS_NAME)
            .and_then(|raw| raw.trim().parse::<usize>().ok())
            .map(|rows| rows.clamp(3, 24))
            .unwrap_or(Self::DEFAULT_OMNIBAR_DROPDOWN_MAX_ROWS);
        self.workspace.chrome_ui.toolbar_height_dp = self
            .load_workspace_layout_json(Self::SETTINGS_TOOLBAR_HEIGHT_DP_NAME)
            .and_then(|raw| raw.trim().parse::<f32>().ok())
            .filter(|v| v.is_finite())
            .map(|v| v.clamp(24.0, 96.0))
            .unwrap_or(Self::DEFAULT_TOOLBAR_HEIGHT_DP);
        self.workspace.chrome_ui.omnibar_provider_debounce_ms = self
            .load_workspace_layout_json(Self::SETTINGS_OMNIBAR_PROVIDER_DEBOUNCE_MS_NAME)
            .and_then(|raw| raw.trim().parse::<u64>().ok())
            .map(|ms| ms.min(2000))
            .unwrap_or(Self::DEFAULT_OMNIBAR_PROVIDER_DEBOUNCE_MS);
        self.workspace.chrome_ui.command_palette_default_scope = self
            .load_workspace_layout_json(Self::SETTINGS_COMMAND_PALETTE_DEFAULT_SCOPE_NAME)
            .and_then(|raw| {
                raw.parse::<crate::shell::desktop::ui::command_palette_state::SearchPaletteScope>()
                    .ok()
            })
            .unwrap_or_default();
        self.workspace.chrome_ui.command_palette_max_per_category = self
            .load_workspace_layout_json(Self::SETTINGS_COMMAND_PALETTE_MAX_PER_CATEGORY_NAME)
            .and_then(|raw| raw.trim().parse::<usize>().ok())
            .map(|cap| cap.min(100))
            .unwrap_or(Self::DEFAULT_COMMAND_PALETTE_MAX_PER_CATEGORY);
        self.workspace.chrome_ui.command_palette_recents_depth = self
            .load_workspace_layout_json(Self::SETTINGS_COMMAND_PALETTE_RECENTS_DEPTH_NAME)
            .and_then(|raw| raw.trim().parse::<usize>().ok())
            .map(|depth| depth.min(32))
            .unwrap_or(Self::DEFAULT_COMMAND_PALETTE_RECENTS_DEPTH);
        self.workspace.chrome_ui.command_palette_recents = self
            .load_workspace_layout_json(Self::SETTINGS_COMMAND_PALETTE_RECENTS_NAME)
            .and_then(|raw| {
                serde_json::from_str::<
                    Vec<crate::render::action_registry::ActionId>,
                >(&raw)
                .map_err(|error| {
                    warn!(
                        "Ignoring invalid persisted command palette recents: {error}"
                    );
                    error
                })
                .ok()
            })
            .map(|mut recents| {
                let depth = self.workspace.chrome_ui.command_palette_recents_depth;
                if recents.len() > depth {
                    recents.truncate(depth);
                }
                recents
            })
            .unwrap_or_default();
        self.workspace.chrome_ui.command_palette_tier1_default_category = self
            .load_workspace_layout_json(
                Self::SETTINGS_COMMAND_PALETTE_TIER1_DEFAULT_CATEGORY_NAME,
            )
            .and_then(|raw| {
                let trimmed = raw.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    crate::render::action_registry::category_from_persisted_name(trimmed)
                }
            });
        if let Some(raw) = self.load_workspace_layout_json(Self::SETTINGS_WRY_ENABLED_NAME) {
            match raw.trim().to_ascii_lowercase().as_str() {
                "true" | "1" | "yes" | "on" => self.workspace.chrome_ui.wry_enabled = true,
                "false" | "0" | "no" | "off" => self.workspace.chrome_ui.wry_enabled = false,
                _ => warn!("Ignoring invalid persisted wry enabled flag: '{raw}'"),
            }
        }
        if let Some(raw) =
            self.load_workspace_layout_json(Self::SETTINGS_FOCUS_RING_SETTINGS_NAME)
        {
            match serde_json::from_str::<FocusRingSettings>(&raw) {
                Ok(mut settings) => {
                    settings.duration_ms = settings.duration_ms.clamp(
                        FocusRingSettings::MIN_DURATION_MS,
                        FocusRingSettings::MAX_DURATION_MS,
                    );
                    self.workspace.chrome_ui.focus_ring_settings = settings;
                }
                Err(error) => {
                    warn!("Ignoring invalid persisted focus ring settings: {error}")
                }
            }
        }
        if let Some(raw) =
            self.load_workspace_layout_json(Self::SETTINGS_THUMBNAIL_SETTINGS_NAME)
        {
            match serde_json::from_str::<ThumbnailSettings>(&raw) {
                Ok(settings) => {
                    self.workspace.chrome_ui.thumbnail_settings =
                        settings.clamp_dimensions();
                }
                Err(error) => {
                    warn!("Ignoring invalid persisted thumbnail settings: {error}")
                }
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
        self.workspace.chrome_ui.navigator_sidebar_side_preference = self
            .load_workspace_layout_json(Self::SETTINGS_NAVIGATOR_SIDEBAR_SIDE_NAME)
            .and_then(|raw| raw.parse::<NavigatorSidebarSidePreference>().ok())
            .unwrap_or_default();
        self.workspace.chrome_ui.workbench_display_mode = self
            .load_workspace_layout_json(Self::SETTINGS_WORKBENCH_DISPLAY_MODE_NAME)
            .and_then(|raw| raw.parse::<WorkbenchDisplayMode>().ok())
            .unwrap_or_default();
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
        registries::phase3_set_theme_follows_system(loaded_theme_mode == ThemeMode::System);

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
                let resolution = registries::phase3_set_active_theme(theme_id);
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
            let resolution = registries::phase3_set_active_physics_profile(physics_id);
            self.workspace.chrome_ui.default_registry_physics_id =
                Some(resolution.resolved_id.clone());
            self.apply_physics_profile(&resolution.resolved_id, &resolution.profile);
        } else {
            let resolution = registries::phase3_set_active_physics_profile(
                crate::registries::atomic::lens::PHYSICS_ID_DEFAULT,
            );
            self.apply_physics_profile(&resolution.resolved_id, &resolution.profile);
        }
        if let Some(profile_id) = canvas_profile_id.as_deref() {
            registries::phase3_set_active_canvas_profile(profile_id);
        } else {
            registries::phase3_set_active_canvas_profile(
                crate::registries::domain::layout::canvas::CANVAS_PROFILE_DEFAULT,
            );
        }
        registries::phase3_set_active_canvas_keyboard_pan_step(
            self.workspace.chrome_ui.keyboard_pan_step,
        );
        registries::phase3_set_active_canvas_lasso_binding(
            self.workspace.chrome_ui.lasso_binding_preference,
        );
        if let Some(profile_id) = workbench_surface_profile_id.as_deref() {
            registries::phase3_set_active_workbench_surface_profile(profile_id);
        }
        if let Some(workflow_id) = active_workflow_id.as_deref()
            && let Err(error) = registries::phase3_activate_workflow(self, workflow_id)
        {
            warn!("Ignoring invalid persisted workflow activation '{workflow_id}': {error:?}");
        }
        registries::phase3_set_active_canvas_keyboard_pan_step(
            self.workspace.chrome_ui.keyboard_pan_step,
        );
        registries::phase3_set_active_canvas_lasso_binding(
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

    // -------------------------------------------------------------------
    // ThumbnailSettings (M4.1 session 4 customization)
    //
    // Pin the user-configurable surface on `chrome_ui.thumbnail_settings`:
    // setter-side clamping, serde-roundtrip with defaults, and
    // FilterType enum roundtrip.
    // -------------------------------------------------------------------

    #[test]
    fn thumbnail_settings_setter_clamps_dimensions_and_quality() {
        let mut app = GraphBrowserApp::new_for_testing();
        app.set_thumbnail_settings(ThumbnailSettings {
            enabled: true,
            width: 10,        // below MIN_DIMENSION (64)
            height: 10_000,   // above MAX_DIMENSION (1024)
            filter: ThumbnailFilter::Lanczos3,
            format: ThumbnailFormat::Jpeg,
            jpeg_quality: 200, // above MAX_JPEG_QUALITY (100)
            aspect: ThumbnailAspect::MatchSource,
        });

        let stored = app.thumbnail_settings();
        assert_eq!(stored.width, ThumbnailSettings::MIN_DIMENSION);
        assert_eq!(stored.height, ThumbnailSettings::MAX_DIMENSION);
        assert_eq!(stored.filter, ThumbnailFilter::Lanczos3);
        assert_eq!(stored.format, ThumbnailFormat::Jpeg);
        assert_eq!(stored.jpeg_quality, ThumbnailSettings::MAX_JPEG_QUALITY);
        assert_eq!(stored.aspect, ThumbnailAspect::MatchSource);
        assert!(stored.enabled);
    }

    #[test]
    fn thumbnail_settings_setter_clamps_zero_quality_to_min() {
        // Quality of 0 is invalid JPEG; must clamp up to MIN (1).
        let mut app = GraphBrowserApp::new_for_testing();
        app.set_thumbnail_settings(ThumbnailSettings {
            jpeg_quality: 0,
            ..ThumbnailSettings::default()
        });
        assert_eq!(
            app.thumbnail_settings().jpeg_quality,
            ThumbnailSettings::MIN_JPEG_QUALITY
        );
    }

    #[test]
    fn thumbnail_format_display_from_str_roundtrip() {
        use std::str::FromStr;
        for variant in [
            ThumbnailFormat::Png,
            ThumbnailFormat::Jpeg,
            ThumbnailFormat::WebP,
        ] {
            let rendered = variant.to_string();
            let parsed = ThumbnailFormat::from_str(&rendered)
                .unwrap_or_else(|_| panic!("'{rendered}' should parse back"));
            assert_eq!(parsed, variant);
        }
    }

    #[test]
    fn thumbnail_aspect_display_from_str_roundtrip() {
        use std::str::FromStr;
        for variant in [
            ThumbnailAspect::Fixed,
            ThumbnailAspect::MatchSource,
            ThumbnailAspect::Square,
        ] {
            let rendered = variant.to_string();
            let parsed = ThumbnailAspect::from_str(&rendered)
                .unwrap_or_else(|_| panic!("'{rendered}' should parse back"));
            assert_eq!(parsed, variant);
        }
    }

    #[test]
    fn thumbnail_settings_serde_roundtrip_with_defaults() {
        // Empty JSON must deserialize cleanly via `#[serde(default)]`
        // so old workspaces without the blob keep working.
        let parsed: ThumbnailSettings =
            serde_json::from_str("{}").expect("defaults must cover empty JSON");
        assert_eq!(parsed, ThumbnailSettings::default());
    }

    #[test]
    fn thumbnail_filter_display_from_str_roundtrip() {
        use std::str::FromStr;

        for variant in [
            ThumbnailFilter::Nearest,
            ThumbnailFilter::Triangle,
            ThumbnailFilter::CatmullRom,
            ThumbnailFilter::Gaussian,
            ThumbnailFilter::Lanczos3,
        ] {
            let rendered = variant.to_string();
            let parsed = ThumbnailFilter::from_str(&rendered)
                .unwrap_or_else(|_| panic!("'{rendered}' should parse back"));
            assert_eq!(parsed, variant);
        }
    }

    #[test]
    fn disabled_thumbnail_settings_serde_roundtrip() {
        // Make sure an intentionally-disabled settings value survives
        // a full JSON roundtrip (privacy-preference use case).
        let original = ThumbnailSettings {
            enabled: false,
            width: 512,
            height: 512,
            filter: ThumbnailFilter::Lanczos3,
            format: ThumbnailFormat::Jpeg,
            jpeg_quality: 70,
            aspect: ThumbnailAspect::Square,
        };
        let json = serde_json::to_string(&original).expect("serialize");
        let restored: ThumbnailSettings =
            serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored, original);
    }

    #[test]
    fn thumbnail_settings_legacy_json_defaults_aspect_and_webp_not_required() {
        // Pre-backlog persisted blobs lack `aspect`; `#[serde(default)]`
        // must supply the historical Fixed mode. WebP format value
        // ("WebP") must also round-trip through a settings blob.
        let legacy_no_aspect = r#"{
            "enabled": true,
            "width": 256,
            "height": 192,
            "filter": "Triangle",
            "format": "Png",
            "jpeg_quality": 85
        }"#;
        let parsed: ThumbnailSettings =
            serde_json::from_str(legacy_no_aspect).expect("legacy blob deserializes");
        assert_eq!(parsed.aspect, ThumbnailAspect::default());

        // WebP settings must also survive a full roundtrip.
        let webp = ThumbnailSettings {
            format: ThumbnailFormat::WebP,
            ..ThumbnailSettings::default()
        };
        let json = serde_json::to_string(&webp).expect("serialize");
        let restored: ThumbnailSettings =
            serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.format, ThumbnailFormat::WebP);
    }

    #[test]
    fn thumbnail_settings_partial_json_inherits_defaults_for_new_fields() {
        // Existing persisted blobs (pre-session-4 follow-ons) lack
        // `format` and `jpeg_quality`. `#[serde(default)]` must cover
        // both so old workspaces deserialize cleanly.
        let legacy_json = r#"{
            "enabled": true,
            "width": 256,
            "height": 192,
            "filter": "Triangle"
        }"#;
        // Note: filter uses the default serde representation of the enum
        // (variant name), not the Display string, because
        // `#[derive(Deserialize)]` on an enum with no custom attrs uses
        // the variant name. We want to pin both the field-defaulting
        // and the fact that parsing works, so we supply a filter that
        // `Deserialize` will accept.
        let parsed: ThumbnailSettings =
            serde_json::from_str(legacy_json).expect("legacy blob deserializes");
        assert_eq!(parsed.format, ThumbnailFormat::default());
        assert_eq!(parsed.jpeg_quality, ThumbnailSettings::DEFAULT_JPEG_QUALITY);
    }
}
