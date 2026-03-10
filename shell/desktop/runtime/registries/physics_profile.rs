use crate::registries::atomic::lens::{
    PHYSICS_ID_DEFAULT, PHYSICS_ID_GAS, PHYSICS_ID_SOLID, PhysicsProfileResolution,
    resolve_physics_profile,
};

pub(crate) const PHYSICS_PROFILE_LIQUID: &str = PHYSICS_ID_DEFAULT;
pub(crate) const PHYSICS_PROFILE_GAS: &str = PHYSICS_ID_GAS;
pub(crate) const PHYSICS_PROFILE_SOLID: &str = PHYSICS_ID_SOLID;

pub(crate) struct PhysicsProfileRegistry {
    active_profile_id: String,
}

impl Default for PhysicsProfileRegistry {
    fn default() -> Self {
        Self {
            active_profile_id: PHYSICS_PROFILE_LIQUID.to_string(),
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

    #[test]
    fn physics_profile_registry_defaults_to_liquid() {
        let registry = PhysicsProfileRegistry::default();
        let resolution = registry.active_profile();

        assert_eq!(registry.active_profile_id(), PHYSICS_PROFILE_LIQUID);
        assert!(resolution.matched);
        assert_eq!(resolution.resolved_id, PHYSICS_PROFILE_LIQUID);
    }

    #[test]
    fn physics_profile_registry_switches_and_falls_back() {
        let mut registry = PhysicsProfileRegistry::default();

        let gas = registry.set_active_profile(PHYSICS_PROFILE_GAS);
        assert_eq!(gas.resolved_id, PHYSICS_PROFILE_GAS);
        assert_eq!(registry.active_profile_id(), PHYSICS_PROFILE_GAS);

        let solid = registry.set_active_profile(PHYSICS_PROFILE_SOLID);
        assert_eq!(solid.resolved_id, PHYSICS_PROFILE_SOLID);
        assert_eq!(registry.active_profile_id(), PHYSICS_PROFILE_SOLID);

        let fallback = registry.set_active_profile("physics:missing");
        assert!(fallback.fallback_used);
        assert_eq!(fallback.resolved_id, PHYSICS_PROFILE_LIQUID);
        assert_eq!(registry.active_profile_id(), PHYSICS_PROFILE_LIQUID);
    }

    #[test]
    fn physics_profile_registry_presets_have_distinct_parameters() {
        let liquid = resolve_physics_profile(PHYSICS_PROFILE_LIQUID).profile;
        let gas = resolve_physics_profile(PHYSICS_PROFILE_GAS).profile;
        let solid = resolve_physics_profile(PHYSICS_PROFILE_SOLID).profile;

        assert_ne!(liquid.repulsion_strength, gas.repulsion_strength);
        assert_ne!(gas.attraction_strength, solid.attraction_strength);
        assert_ne!(liquid.gravity_strength, solid.gravity_strength);
    }
}
