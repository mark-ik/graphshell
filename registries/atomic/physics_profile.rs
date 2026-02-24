use std::collections::HashMap;

use crate::app::PhysicsProfile;

pub(crate) const PHYSICS_ID_DEFAULT: &str = "physics:liquid";
pub(crate) const PHYSICS_ID_GAS: &str = "physics:gas";
pub(crate) const PHYSICS_ID_SOLID: &str = "physics:solid";
const PHYSICS_ID_LEGACY_DEFAULT: &str = "physics:default";

#[derive(Debug, Clone)]
pub(crate) struct PhysicsProfileResolution {
    pub(crate) requested_id: String,
    pub(crate) resolved_id: String,
    pub(crate) matched: bool,
    pub(crate) fallback_used: bool,
    pub(crate) profile: PhysicsProfile,
}

pub(crate) struct PhysicsProfileRegistry {
    profiles: HashMap<String, PhysicsProfile>,
    fallback_id: String,
}

impl PhysicsProfileRegistry {
    pub(crate) fn register(&mut self, physics_id: &str, profile: PhysicsProfile) {
        self.profiles
            .insert(physics_id.to_ascii_lowercase(), profile);
    }

    pub(crate) fn register_core_seed_defaults(&mut self) {
        self.register(PHYSICS_ID_DEFAULT, PhysicsProfile::default());
        self.register(PHYSICS_ID_LEGACY_DEFAULT, PhysicsProfile::default());
        self.register(PHYSICS_ID_GAS, PhysicsProfile::gas());
        self.register(PHYSICS_ID_SOLID, PhysicsProfile::solid());
    }

    pub(crate) fn resolve(&self, physics_id: &str) -> PhysicsProfileResolution {
        let requested = physics_id.trim().to_ascii_lowercase();
        let canonical_requested = if requested == PHYSICS_ID_LEGACY_DEFAULT {
            PHYSICS_ID_DEFAULT.to_string()
        } else {
            requested.clone()
        };
        let fallback_profile = self
            .profiles
            .get(&self.fallback_id)
            .cloned()
            .unwrap_or_default();

        if requested.is_empty() {
            return PhysicsProfileResolution {
                requested_id: requested,
                resolved_id: self.fallback_id.clone(),
                matched: false,
                fallback_used: true,
                profile: fallback_profile,
            };
        }

        if let Some(profile) = self.profiles.get(&canonical_requested).cloned() {
            return PhysicsProfileResolution {
                requested_id: requested,
                resolved_id: canonical_requested,
                matched: true,
                fallback_used: false,
                profile,
            };
        }

        PhysicsProfileResolution {
            requested_id: requested,
            resolved_id: self.fallback_id.clone(),
            matched: false,
            fallback_used: true,
            profile: fallback_profile,
        }
    }
}

impl Default for PhysicsProfileRegistry {
    fn default() -> Self {
        let mut registry = Self {
            profiles: HashMap::new(),
            fallback_id: PHYSICS_ID_DEFAULT.to_string(),
        };
        registry.register_core_seed_defaults();
        registry
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn physics_profile_registry_resolves_default() {
        let registry = PhysicsProfileRegistry::default();
        let resolution = registry.resolve(PHYSICS_ID_DEFAULT);

        assert!(resolution.matched);
        assert!(!resolution.fallback_used);
        assert_eq!(resolution.resolved_id, PHYSICS_ID_DEFAULT);
        assert_eq!(resolution.profile.name, "Liquid");
    }

    #[test]
    fn physics_profile_registry_has_solid_core_seed() {
        let registry = PhysicsProfileRegistry::default();
        let resolution = registry.resolve(PHYSICS_ID_SOLID);

        assert!(resolution.matched);
        assert_eq!(resolution.profile.name, "Solid");
    }

    #[test]
    fn physics_profile_registry_maps_legacy_default_to_liquid_id() {
        let registry = PhysicsProfileRegistry::default();
        let resolution = registry.resolve("physics:default");

        assert!(resolution.matched);
        assert_eq!(resolution.requested_id, "physics:default");
        assert_eq!(resolution.resolved_id, PHYSICS_ID_DEFAULT);
        assert_eq!(resolution.profile.name, "Liquid");
    }
}
