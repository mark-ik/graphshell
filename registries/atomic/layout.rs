use std::collections::HashMap;

use crate::app::LayoutMode;

pub(crate) const LAYOUT_ID_DEFAULT: &str = "layout:default";
pub(crate) const LAYOUT_ID_GRID: &str = "layout:grid";

#[derive(Debug, Clone)]
pub(crate) struct LayoutResolution {
    pub(crate) requested_id: String,
    pub(crate) resolved_id: String,
    pub(crate) matched: bool,
    pub(crate) fallback_used: bool,
    pub(crate) layout: LayoutMode,
}

pub(crate) struct LayoutRegistry {
    layouts: HashMap<String, LayoutMode>,
    fallback_id: String,
}

impl LayoutRegistry {
    pub(crate) fn register(&mut self, layout_id: &str, layout: LayoutMode) {
        self.layouts.insert(layout_id.to_ascii_lowercase(), layout);
    }

    pub(crate) fn resolve(&self, layout_id: &str) -> LayoutResolution {
        let requested = layout_id.trim().to_ascii_lowercase();
        let fallback_layout = self
            .layouts
            .get(&self.fallback_id)
            .cloned()
            .unwrap_or(LayoutMode::Free);

        if requested.is_empty() {
            return LayoutResolution {
                requested_id: requested,
                resolved_id: self.fallback_id.clone(),
                matched: false,
                fallback_used: true,
                layout: fallback_layout,
            };
        }

        if let Some(layout) = self.layouts.get(&requested).cloned() {
            return LayoutResolution {
                requested_id: requested.clone(),
                resolved_id: requested,
                matched: true,
                fallback_used: false,
                layout,
            };
        }

        LayoutResolution {
            requested_id: requested,
            resolved_id: self.fallback_id.clone(),
            matched: false,
            fallback_used: true,
            layout: fallback_layout,
        }
    }
}

impl Default for LayoutRegistry {
    fn default() -> Self {
        let mut registry = Self {
            layouts: HashMap::new(),
            fallback_id: LAYOUT_ID_DEFAULT.to_string(),
        };
        registry.register(LAYOUT_ID_DEFAULT, LayoutMode::Free);
        registry.register(LAYOUT_ID_GRID, LayoutMode::Grid { gap: 48.0 });
        registry
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_registry_resolves_default() {
        let registry = LayoutRegistry::default();
        let resolution = registry.resolve(LAYOUT_ID_DEFAULT);

        assert!(resolution.matched);
        assert!(!resolution.fallback_used);
        assert_eq!(resolution.resolved_id, LAYOUT_ID_DEFAULT);
        assert!(matches!(resolution.layout, LayoutMode::Free));
    }

    #[test]
    fn layout_registry_falls_back_for_unknown_id() {
        let registry = LayoutRegistry::default();
        let resolution = registry.resolve("layout:unknown");

        assert!(!resolution.matched);
        assert!(resolution.fallback_used);
        assert_eq!(resolution.resolved_id, LAYOUT_ID_DEFAULT);
        assert!(matches!(resolution.layout, LayoutMode::Free));
    }
}
