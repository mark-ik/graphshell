use std::collections::HashMap;

pub(crate) const THEME_ID_DEFAULT: &str = "theme:default";
pub(crate) const THEME_ID_DARK: &str = "theme:dark";

#[derive(Debug, Clone)]
pub(crate) struct ThemeResolution {
    pub(crate) requested_id: String,
    pub(crate) resolved_id: String,
    pub(crate) matched: bool,
    pub(crate) fallback_used: bool,
    pub(crate) theme_id: String,
}

pub(crate) struct ThemeRegistry {
    themes: HashMap<String, String>,
    fallback_id: String,
}

impl ThemeRegistry {
    pub(crate) fn register(&mut self, theme_id: &str) {
        self.themes
            .insert(theme_id.to_ascii_lowercase(), theme_id.to_ascii_lowercase());
    }

    pub(crate) fn resolve(&self, theme_id: &str) -> ThemeResolution {
        let requested = theme_id.trim().to_ascii_lowercase();
        let fallback_theme = self
            .themes
            .get(&self.fallback_id)
            .cloned()
            .unwrap_or_else(|| THEME_ID_DEFAULT.to_string());

        if requested.is_empty() {
            return ThemeResolution {
                requested_id: requested,
                resolved_id: self.fallback_id.clone(),
                matched: false,
                fallback_used: true,
                theme_id: fallback_theme,
            };
        }

        if let Some(theme) = self.themes.get(&requested).cloned() {
            return ThemeResolution {
                requested_id: requested.clone(),
                resolved_id: requested,
                matched: true,
                fallback_used: false,
                theme_id: theme,
            };
        }

        ThemeResolution {
            requested_id: requested,
            resolved_id: self.fallback_id.clone(),
            matched: false,
            fallback_used: true,
            theme_id: fallback_theme,
        }
    }
}

impl Default for ThemeRegistry {
    fn default() -> Self {
        let mut registry = Self {
            themes: HashMap::new(),
            fallback_id: THEME_ID_DEFAULT.to_string(),
        };
        registry.register(THEME_ID_DEFAULT);
        registry.register(THEME_ID_DARK);
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
    }

    #[test]
    fn theme_registry_falls_back_for_unknown_id() {
        let registry = ThemeRegistry::default();
        let resolution = registry.resolve("theme:unknown");

        assert!(!resolution.matched);
        assert!(resolution.fallback_used);
        assert_eq!(resolution.resolved_id, THEME_ID_DEFAULT);
        assert_eq!(resolution.theme_id, THEME_ID_DEFAULT);
    }
}
