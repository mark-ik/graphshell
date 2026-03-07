use crate::registries::atomic::lens::{
    PhysicsProfileResolution, ThemeResolution, resolve_physics_profile, resolve_theme_data,
};

#[derive(Debug, Clone)]
pub(crate) struct PresentationDomainProfileResolution {
    pub(crate) physics: PhysicsProfileResolution,
    pub(crate) theme: ThemeResolution,
}

pub(crate) struct PresentationDomainRegistry {}

impl Default for PresentationDomainRegistry {
    fn default() -> Self {
        Self {}
    }
}

impl PresentationDomainRegistry {
    pub(crate) fn resolve_profile(
        &self,
        physics_id: &str,
        theme_id: &str,
    ) -> PresentationDomainProfileResolution {
        PresentationDomainProfileResolution {
            physics: resolve_physics_profile(physics_id),
            theme: resolve_theme_data(theme_id),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registries::atomic::lens::{PHYSICS_ID_DEFAULT, THEME_ID_DEFAULT};

    #[test]
    fn presentation_domain_resolves_default_profile() {
        let domain = PresentationDomainRegistry::default();
        let resolution = domain.resolve_profile(PHYSICS_ID_DEFAULT, THEME_ID_DEFAULT);

        assert!(resolution.physics.matched);
        assert!(resolution.theme.matched);
        assert!(!resolution.physics.fallback_used);
        assert!(!resolution.theme.fallback_used);
    }

    #[test]
    fn presentation_domain_falls_back_independently() {
        let domain = PresentationDomainRegistry::default();
        let resolution = domain.resolve_profile("physics:unknown", "theme:unknown");

        assert!(resolution.physics.fallback_used);
        assert!(resolution.theme.fallback_used);
    }
}
