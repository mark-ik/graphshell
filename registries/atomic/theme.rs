use std::collections::HashMap;

pub(crate) const THEME_ID_DEFAULT: &str = "theme:default";
pub(crate) const THEME_ID_DARK: &str = "theme:dark";

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ThemeData {
    pub(crate) background_rgb: (u8, u8, u8),
    pub(crate) accent_rgb: (u8, u8, u8),
    pub(crate) font_scale: f32,
    pub(crate) stroke_width: f32,
}

#[derive(Debug, Clone)]
pub(crate) struct ThemeResolution {
    pub(crate) requested_id: String,
    pub(crate) resolved_id: String,
    pub(crate) matched: bool,
    pub(crate) fallback_used: bool,
    pub(crate) theme_id: String,
    pub(crate) theme: ThemeData,
}

pub(crate) struct ThemeRegistry {
    themes: HashMap<String, ThemeData>,
    fallback_id: String,
}

impl ThemeRegistry {
    pub(crate) fn register(&mut self, theme_id: &str, theme: ThemeData) {
        self.themes.insert(theme_id.to_ascii_lowercase(), theme);
    }

    pub(crate) fn register_core_seed_defaults(&mut self) {
        self.register(
            THEME_ID_DEFAULT,
            ThemeData {
                background_rgb: (20, 20, 25),
                accent_rgb: (80, 220, 255),
                font_scale: 1.0,
                stroke_width: 1.0,
            },
        );
        self.register(
            THEME_ID_DARK,
            ThemeData {
                background_rgb: (14, 14, 18),
                accent_rgb: (110, 170, 255),
                font_scale: 1.0,
                stroke_width: 1.0,
            },
        );
    }

    pub(crate) fn resolve(&self, theme_id: &str) -> ThemeResolution {
        let requested = theme_id.trim().to_ascii_lowercase();
        let fallback_theme = self
            .themes
            .get(&self.fallback_id)
            .cloned()
            .unwrap_or(ThemeData {
                background_rgb: (20, 20, 25),
                accent_rgb: (80, 220, 255),
                font_scale: 1.0,
                stroke_width: 1.0,
            });

        if requested.is_empty() {
            return ThemeResolution {
                requested_id: requested,
                resolved_id: self.fallback_id.clone(),
                matched: false,
                fallback_used: true,
                theme_id: self.fallback_id.clone(),
                theme: fallback_theme,
            };
        }

        if let Some(theme) = self.themes.get(&requested).cloned() {
            return ThemeResolution {
                requested_id: requested.clone(),
                resolved_id: requested,
                matched: true,
                fallback_used: false,
                theme_id: theme_id.trim().to_ascii_lowercase(),
                theme,
            };
        }

        ThemeResolution {
            requested_id: requested,
            resolved_id: self.fallback_id.clone(),
            matched: false,
            fallback_used: true,
            theme_id: self.fallback_id.clone(),
            theme: fallback_theme,
        }
    }
}

impl Default for ThemeRegistry {
    fn default() -> Self {
        let mut registry = Self {
            themes: HashMap::new(),
            fallback_id: THEME_ID_DEFAULT.to_string(),
        };
        registry.register_core_seed_defaults();
        registry
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_registry_resolves_default() {
        let registry = ThemeRegistry::default();
        let resolution = registry.resolve(THEME_ID_DEFAULT);

        assert!(resolution.matched);
        assert!(!resolution.fallback_used);
        assert_eq!(resolution.resolved_id, THEME_ID_DEFAULT);
        assert_eq!(resolution.theme_id, THEME_ID_DEFAULT);
        assert_eq!(resolution.theme.background_rgb, (20, 20, 25));
    }

    #[test]
    fn theme_registry_falls_back_for_unknown_id() {
        let registry = ThemeRegistry::default();
        let resolution = registry.resolve("theme:unknown");

        assert!(!resolution.matched);
        assert!(resolution.fallback_used);
        assert_eq!(resolution.resolved_id, THEME_ID_DEFAULT);
        assert_eq!(resolution.theme_id, THEME_ID_DEFAULT);
        assert_eq!(resolution.theme.accent_rgb, (80, 220, 255));
    }
}
