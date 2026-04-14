use crate::registries::atomic::lens::{
    PHYSICS_ID_DEFAULT, PHYSICS_ID_DRIFT, PHYSICS_ID_SCATTER, PHYSICS_ID_SETTLE,
    PhysicsProfileResolution, resolve_physics_profile,
};

pub(crate) const PHYSICS_PROFILE_DEFAULT: &str = PHYSICS_ID_DEFAULT;
pub(crate) const PHYSICS_PROFILE_DRIFT: &str = PHYSICS_ID_DRIFT;
pub(crate) const PHYSICS_PROFILE_SCATTER: &str = PHYSICS_ID_SCATTER;
pub(crate) const PHYSICS_PROFILE_SETTLE: &str = PHYSICS_ID_SETTLE;

pub(crate) struct PhysicsProfileRegistry {
    active_profile_id: String,
}

impl Default for PhysicsProfileRegistry {
    fn default() -> Self {
        Self {
            active_profile_id: PHYSICS_PROFILE_DEFAULT.to_string(),
        }
    }
}

impl PhysicsProfileRegistry {
    pub(crate) fn active_profile_id(&self) -> &str {
        &self.active_profile_id
    }

    pub(crate) fn set_active_profile(&mut self, profile_id: &str) -> PhysicsProfileResolution {
        let resolution = resolve_physics_profile(profile_id);
        self.active_profile_id = resolution.resolved_id.clone();
        resolution
    }

    pub(crate) fn active_profile(&self) -> PhysicsProfileResolution {
        resolve_physics_profile(&self.active_profile_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registries::atomic::lens::physics_profile_descriptors;

    #[test]
    fn physics_profile_registry_defaults_to_drift() {
        let registry = PhysicsProfileRegistry::default();
        let resolution = registry.active_profile();

        assert_eq!(registry.active_profile_id(), PHYSICS_PROFILE_DEFAULT);
        assert!(resolution.matched);
        assert_eq!(resolution.resolved_id, PHYSICS_PROFILE_DEFAULT);
        assert_eq!(resolution.display_name, "Drift");
    }

    #[test]
    fn physics_profile_registry_switches_and_falls_back() {
        let mut registry = PhysicsProfileRegistry::default();

        let scatter = registry.set_active_profile(PHYSICS_PROFILE_SCATTER);
        assert_eq!(scatter.resolved_id, PHYSICS_PROFILE_SCATTER);
        assert_eq!(registry.active_profile_id(), PHYSICS_PROFILE_SCATTER);

        let settle = registry.set_active_profile(PHYSICS_PROFILE_SETTLE);
        assert_eq!(settle.resolved_id, PHYSICS_PROFILE_SETTLE);
        assert_eq!(registry.active_profile_id(), PHYSICS_PROFILE_SETTLE);

        let fallback = registry.set_active_profile("physics:missing");
        assert!(fallback.fallback_used);
        assert_eq!(fallback.resolved_id, PHYSICS_PROFILE_DEFAULT);
        assert_eq!(registry.active_profile_id(), PHYSICS_PROFILE_DEFAULT);
    }

    #[test]
    fn physics_profile_registry_seed_catalog_contains_helper_era_portfolio() {
        let ids: Vec<_> = physics_profile_descriptors()
            .iter()
            .map(|descriptor| descriptor.id.as_str())
            .collect();

        assert_eq!(
            ids,
            vec![
                "physics:drift",
                "physics:scatter",
                "physics:settle",
                "physics:archipelago",
                "physics:resonance",
                "physics:constellation",
            ]
        );
    }
}

