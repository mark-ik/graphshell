use std::collections::HashMap;

use crate::registries::atomic::lens::{
    THEME_ID_DARK as LEGACY_THEME_ID_DARK, THEME_ID_DEFAULT as LEGACY_THEME_ID_DEFAULT, ThemeData,
};

pub(crate) const THEME_ID_DEFAULT: &str = LEGACY_THEME_ID_DEFAULT;
pub(crate) const THEME_ID_LIGHT: &str = "theme:light";
pub(crate) const THEME_ID_DARK: &str = LEGACY_THEME_ID_DARK;
pub(crate) const THEME_ID_HIGH_CONTRAST: &str = "theme:high_contrast";

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct ThemeTokenSet {
    pub(crate) theme_id: String,
    pub(crate) display_name: String,
    pub(crate) theme_data: ThemeData,
    pub(crate) command_notice: egui::Color32,
    pub(crate) radial_disabled_text: egui::Color32,
    pub(crate) radial_hub_fill: egui::Color32,
    pub(crate) radial_hub_stroke: egui::Color32,
    pub(crate) radial_hub_text: egui::Color32,
    pub(crate) radial_domain_active_fill: egui::Color32,
    pub(crate) radial_domain_idle_fill: egui::Color32,
    pub(crate) radial_command_active_fill: egui::Color32,
    pub(crate) radial_command_hover_fill: egui::Color32,
    pub(crate) radial_command_disabled_fill: egui::Color32,
    pub(crate) radial_command_text: egui::Color32,
    pub(crate) radial_chrome_text: egui::Color32,
    pub(crate) radial_warning_text: egui::Color32,
    pub(crate) hover_label_background: egui::Color32,
    pub(crate) hover_label_stroke: egui::Color32,
    pub(crate) hover_label_text: egui::Color32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ThemeCapability {
    pub(crate) requested_id: String,
    pub(crate) resolved_id: String,
    pub(crate) matched: bool,
    pub(crate) fallback_used: bool,
    pub(crate) display_name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ThemeResolution {
    pub(crate) requested_id: String,
    pub(crate) resolved_id: String,
    pub(crate) matched: bool,
    pub(crate) fallback_used: bool,
    pub(crate) tokens: ThemeTokenSet,
}

pub(crate) struct ThemeRegistry {
    themes: HashMap<String, ThemeTokenSet>,
    active: String,
    fallback_id: String,
}

impl Default for ThemeRegistry {
    fn default() -> Self {
        let mut registry = Self {
            themes: HashMap::new(),
            active: THEME_ID_DEFAULT.to_string(),
            fallback_id: THEME_ID_DEFAULT.to_string(),
        };
        registry
            .register_theme(default_theme_tokens())
            .expect("default theme must be valid");
        registry
            .register_theme(light_theme_tokens())
            .expect("light theme must be valid");
        registry
            .register_theme(dark_theme_tokens())
            .expect("dark theme must be valid");
        registry
            .register_theme(high_contrast_theme_tokens())
            .expect("high-contrast theme must be valid");
        registry
    }
}

impl ThemeRegistry {
    pub(crate) fn register_theme(&mut self, tokens: ThemeTokenSet) -> Result<(), String> {
        validate_theme_tokens(&tokens)?;
        self.themes
            .insert(tokens.theme_id.to_ascii_lowercase(), tokens);
        Ok(())
    }

    pub(crate) fn unregister_theme(&mut self, theme_id: &str) -> bool {
        let normalized = theme_id.trim().to_ascii_lowercase();
        if normalized == self.fallback_id {
            return false;
        }
        self.themes.remove(&normalized).is_some()
    }

    pub(crate) fn resolve_theme(&self, theme_id: Option<&str>) -> ThemeResolution {
        let requested = theme_id
            .unwrap_or(self.active.as_str())
            .trim()
            .to_ascii_lowercase();
        let fallback = self
            .themes
            .get(&self.fallback_id)
            .cloned()
            .unwrap_or_else(default_theme_tokens);

        if requested.is_empty() {
            return ThemeResolution {
                requested_id: requested,
                resolved_id: self.fallback_id.clone(),
                matched: false,
                fallback_used: true,
                tokens: fallback,
            };
        }

        if let Some(tokens) = self.themes.get(&requested).cloned() {
            return ThemeResolution {
                requested_id: requested.clone(),
                resolved_id: requested,
                matched: true,
                fallback_used: false,
                tokens,
            };
        }

        ThemeResolution {
            requested_id: requested,
            resolved_id: self.fallback_id.clone(),
            matched: false,
            fallback_used: true,
            tokens: fallback,
        }
    }

    pub(crate) fn describe_theme(&self, theme_id: Option<&str>) -> ThemeCapability {
        let resolution = self.resolve_theme(theme_id);
        ThemeCapability {
            requested_id: resolution.requested_id,
            resolved_id: resolution.resolved_id,
            matched: resolution.matched,
            fallback_used: resolution.fallback_used,
            display_name: resolution.tokens.display_name,
        }
    }

    pub(crate) fn set_active_theme(&mut self, theme_id: &str) -> ThemeResolution {
        let resolution = self.resolve_theme(Some(theme_id));
        self.active = resolution.resolved_id.clone();
        resolution
    }

    pub(crate) fn active_theme(&self) -> ThemeResolution {
        self.resolve_theme(None)
    }
}

fn validate_theme_tokens(tokens: &ThemeTokenSet) -> Result<(), String> {
    let minimum_ratio = if tokens.theme_id == THEME_ID_HIGH_CONTRAST {
        7.0
    } else {
        4.5
    };

    for (label, foreground, background) in [
        (
            "radial disabled text",
            tokens.radial_disabled_text,
            tokens.radial_command_disabled_fill,
        ),
        (
            "radial hub text",
            tokens.radial_hub_text,
            tokens.radial_hub_fill,
        ),
        (
            "hover label text",
            tokens.hover_label_text,
            tokens.hover_label_background,
        ),
        (
            "command notice",
            tokens.command_notice,
            tokens.hover_label_background,
        ),
    ] {
        let ratio = contrast_ratio(foreground, background);
        if ratio < minimum_ratio {
            return Err(format!(
                "{label} contrast {ratio:.2} below minimum {minimum_ratio:.2}"
            ));
        }
    }

    Ok(())
}

fn contrast_ratio(foreground: egui::Color32, background: egui::Color32) -> f32 {
    let mut l1 = relative_luminance(foreground);
    let mut l2 = relative_luminance(background);
    if l2 > l1 {
        std::mem::swap(&mut l1, &mut l2);
    }
    (l1 + 0.05) / (l2 + 0.05)
}

fn relative_luminance(color: egui::Color32) -> f32 {
    0.2126 * to_linear_component(color.r())
        + 0.7152 * to_linear_component(color.g())
        + 0.0722 * to_linear_component(color.b())
}

fn to_linear_component(component: u8) -> f32 {
    let value = component as f32 / 255.0;
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

fn default_theme_tokens() -> ThemeTokenSet {
    ThemeTokenSet {
        theme_id: THEME_ID_DEFAULT.to_string(),
        display_name: "Default".to_string(),
        theme_data: ThemeData {
            background_rgb: (20, 20, 25),
            accent_rgb: (80, 220, 255),
            font_scale: 1.0,
            stroke_width: 1.0,
        },
        command_notice: egui::Color32::from_rgb(234, 200, 145),
        radial_disabled_text: egui::Color32::from_rgb(165, 172, 178),
        radial_hub_fill: egui::Color32::from_rgb(28, 32, 36),
        radial_hub_stroke: egui::Color32::from_rgb(90, 110, 125),
        radial_hub_text: egui::Color32::from_rgb(210, 230, 245),
        radial_domain_active_fill: egui::Color32::from_rgb(70, 130, 170),
        radial_domain_idle_fill: egui::Color32::from_rgb(50, 66, 80),
        radial_command_active_fill: egui::Color32::from_rgb(80, 170, 215),
        radial_command_hover_fill: egui::Color32::from_rgb(64, 82, 98),
        radial_command_disabled_fill: egui::Color32::from_rgb(42, 48, 54),
        radial_command_text: egui::Color32::from_rgb(230, 240, 248),
        radial_chrome_text: egui::Color32::from_rgb(170, 190, 205),
        radial_warning_text: egui::Color32::from_rgb(234, 200, 145),
        hover_label_background: egui::Color32::from_rgba_unmultiplied(22, 28, 34, 235),
        hover_label_stroke: egui::Color32::from_rgb(88, 110, 126),
        hover_label_text: egui::Color32::from_rgb(220, 236, 248),
    }
}

fn light_theme_tokens() -> ThemeTokenSet {
    ThemeTokenSet {
        theme_id: THEME_ID_LIGHT.to_string(),
        display_name: "Light".to_string(),
        theme_data: ThemeData {
            background_rgb: (20, 20, 25),
            accent_rgb: (80, 220, 255),
            font_scale: 1.0,
            stroke_width: 1.0,
        },
        ..default_theme_tokens()
    }
}

fn dark_theme_tokens() -> ThemeTokenSet {
    ThemeTokenSet {
        theme_id: THEME_ID_DARK.to_string(),
        display_name: "Dark".to_string(),
        theme_data: ThemeData {
            background_rgb: (14, 14, 18),
            accent_rgb: (110, 170, 255),
            font_scale: 1.0,
            stroke_width: 1.0,
        },
        command_notice: egui::Color32::from_rgb(240, 214, 164),
        radial_disabled_text: egui::Color32::from_rgb(176, 182, 190),
        radial_hub_fill: egui::Color32::from_rgb(20, 24, 30),
        radial_hub_stroke: egui::Color32::from_rgb(92, 116, 138),
        radial_hub_text: egui::Color32::from_rgb(220, 234, 250),
        radial_domain_active_fill: egui::Color32::from_rgb(86, 140, 186),
        radial_domain_idle_fill: egui::Color32::from_rgb(44, 56, 72),
        radial_command_active_fill: egui::Color32::from_rgb(94, 166, 224),
        radial_command_hover_fill: egui::Color32::from_rgb(58, 74, 92),
        radial_command_disabled_fill: egui::Color32::from_rgb(34, 40, 48),
        radial_command_text: egui::Color32::from_rgb(232, 240, 248),
        radial_chrome_text: egui::Color32::from_rgb(184, 198, 214),
        radial_warning_text: egui::Color32::from_rgb(240, 214, 164),
        hover_label_background: egui::Color32::from_rgba_unmultiplied(16, 20, 28, 240),
        hover_label_stroke: egui::Color32::from_rgb(86, 110, 136),
        hover_label_text: egui::Color32::from_rgb(226, 236, 248),
    }
}

fn high_contrast_theme_tokens() -> ThemeTokenSet {
    ThemeTokenSet {
        theme_id: THEME_ID_HIGH_CONTRAST.to_string(),
        display_name: "High Contrast".to_string(),
        theme_data: ThemeData {
            background_rgb: (0, 0, 0),
            accent_rgb: (255, 230, 0),
            font_scale: 1.1,
            stroke_width: 1.5,
        },
        command_notice: egui::Color32::from_rgb(255, 230, 0),
        radial_disabled_text: egui::Color32::from_rgb(255, 255, 255),
        radial_hub_fill: egui::Color32::from_rgb(0, 0, 0),
        radial_hub_stroke: egui::Color32::from_rgb(255, 255, 255),
        radial_hub_text: egui::Color32::from_rgb(255, 255, 255),
        radial_domain_active_fill: egui::Color32::from_rgb(255, 230, 0),
        radial_domain_idle_fill: egui::Color32::from_rgb(0, 0, 0),
        radial_command_active_fill: egui::Color32::from_rgb(255, 230, 0),
        radial_command_hover_fill: egui::Color32::from_rgb(40, 40, 40),
        radial_command_disabled_fill: egui::Color32::from_rgb(0, 0, 0),
        radial_command_text: egui::Color32::from_rgb(255, 255, 255),
        radial_chrome_text: egui::Color32::from_rgb(255, 255, 255),
        radial_warning_text: egui::Color32::from_rgb(255, 230, 0),
        hover_label_background: egui::Color32::from_rgba_unmultiplied(0, 0, 0, 255),
        hover_label_stroke: egui::Color32::from_rgb(255, 255, 255),
        hover_label_text: egui::Color32::from_rgb(255, 255, 255),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_registry_resolves_builtin_themes_and_fallbacks() {
        let registry = ThemeRegistry::default();
        let dark = registry.resolve_theme(Some(THEME_ID_DARK));
        assert!(dark.matched);
        assert_eq!(dark.resolved_id, THEME_ID_DARK);

        let fallback = registry.resolve_theme(Some("theme:missing"));
        assert!(fallback.fallback_used);
        assert_eq!(fallback.resolved_id, THEME_ID_DEFAULT);
    }

    #[test]
    fn high_contrast_theme_passes_wcag_validation() {
        validate_theme_tokens(&high_contrast_theme_tokens())
            .expect("high contrast theme should satisfy validation");
    }
}
