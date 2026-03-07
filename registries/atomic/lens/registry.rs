use std::collections::HashMap;

use super::{LayoutMode, PhysicsProfile, THEME_ID_DEFAULT, ThemeData, resolve_theme_data};

pub(crate) const LENS_ID_DEFAULT: &str = "lens:default";

#[derive(Debug, Clone)]
pub(crate) struct LensDefinition {
    pub(crate) display_name: String,
    pub(crate) physics: PhysicsProfile,
    pub(crate) layout: LayoutMode,
    pub(crate) theme: Option<ThemeData>,
    pub(crate) filters: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct LensResolution {
    pub(crate) requested_id: String,
    pub(crate) resolved_id: String,
    pub(crate) matched: bool,
    pub(crate) fallback_used: bool,
    pub(crate) definition: LensDefinition,
}

pub(crate) struct LensRegistry {
    lenses: HashMap<String, LensDefinition>,
    fallback_id: String,
}

impl LensRegistry {
    pub(crate) fn register(&mut self, lens_id: &str, definition: LensDefinition) {
        self.lenses.insert(lens_id.to_ascii_lowercase(), definition);
    }

    pub(crate) fn resolve(&self, lens_id: &str) -> LensResolution {
        let requested = lens_id.trim().to_ascii_lowercase();
        let fallback_lens = self
            .lenses
            .get(&self.fallback_id)
            .cloned()
            .unwrap_or_else(default_lens_definition);

        if requested.is_empty() {
            return LensResolution {
                requested_id: requested,
                resolved_id: self.fallback_id.clone(),
                matched: false,
                fallback_used: true,
                definition: fallback_lens,
            };
        }

        if let Some(lens) = self.lenses.get(&requested).cloned() {
            return LensResolution {
                requested_id: requested.clone(),
                resolved_id: requested,
                matched: true,
                fallback_used: false,
                definition: lens,
            };
        }

        LensResolution {
            requested_id: requested,
            resolved_id: self.fallback_id.clone(),
            matched: false,
            fallback_used: true,
            definition: fallback_lens,
        }
    }
}

impl Default for LensRegistry {
    fn default() -> Self {
        let mut registry = Self {
            lenses: HashMap::new(),
            fallback_id: LENS_ID_DEFAULT.to_string(),
        };
        registry.register(LENS_ID_DEFAULT, default_lens_definition());
        registry
    }
}

fn default_lens_definition() -> LensDefinition {
    LensDefinition {
        display_name: "Default".to_string(),
        physics: PhysicsProfile::default(),
        layout: LayoutMode::Free,
        theme: Some(resolve_theme_data(THEME_ID_DEFAULT).theme),
        filters: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lens_registry_resolves_default_lens() {
        let registry = LensRegistry::default();
        let resolution = registry.resolve(LENS_ID_DEFAULT);

        assert!(resolution.matched);
        assert!(!resolution.fallback_used);
        assert_eq!(resolution.resolved_id, LENS_ID_DEFAULT);
        assert_eq!(resolution.definition.display_name, "Default");
    }

    #[test]
    fn lens_registry_falls_back_for_unknown_lens() {
        let registry = LensRegistry::default();
        let resolution = registry.resolve("lens:unknown");

        assert!(!resolution.matched);
        assert!(resolution.fallback_used);
        assert_eq!(resolution.resolved_id, LENS_ID_DEFAULT);
        assert_eq!(resolution.definition.display_name, "Default");
    }
}
