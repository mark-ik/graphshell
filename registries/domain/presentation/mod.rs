use crate::registries::atomic::physics_profile::PhysicsProfileRegistry;
use crate::registries::atomic::theme::ThemeRegistry;

#[derive(Debug, Clone)]
pub(crate) struct PresentationDomainProfileResolution {
    pub(crate) physics: crate::registries::atomic::physics_profile::PhysicsProfileResolution,
    pub(crate) theme: crate::registries::atomic::theme::ThemeResolution,
}

#[derive(Default)]
pub(crate) struct PresentationDomainRegistry {
    physics_profile: PhysicsProfileRegistry,
    theme: ThemeRegistry,
}

impl PresentationDomainRegistry {
    pub(crate) fn resolve_profile(
        &self,
        physics_id: &str,
        theme_id: &str,
    ) -> PresentationDomainProfileResolution {
        PresentationDomainProfileResolution {
            physics: self.physics_profile.resolve(physics_id),
            theme: self.theme.resolve(theme_id),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registries::atomic::physics_profile::PHYSICS_ID_DEFAULT;
    use crate::registries::atomic::theme::THEME_ID_DEFAULT;

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
