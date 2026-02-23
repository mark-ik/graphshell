use std::collections::HashMap;

use crate::app::PhysicsProfile;

pub(crate) const PHYSICS_ID_DEFAULT: &str = "physics:default";
pub(crate) const PHYSICS_ID_GAS: &str = "physics:gas";

#[derive(Debug, Clone)]
pub(crate) struct PhysicsResolution {
    pub(crate) requested_id: String,
    pub(crate) resolved_id: String,
    pub(crate) matched: bool,
    pub(crate) fallback_used: bool,
    pub(crate) profile: PhysicsProfile,
}

pub(crate) struct PhysicsRegistry {
    profiles: HashMap<String, PhysicsProfile>,
    fallback_id: String,
}

impl PhysicsRegistry {
    pub(crate) fn register(&mut self, physics_id: &str, profile: PhysicsProfile) {
        self.profiles
            .insert(physics_id.to_ascii_lowercase(), profile);
    }

    pub(crate) fn resolve(&self, physics_id: &str) -> PhysicsResolution {
        let requested = physics_id.trim().to_ascii_lowercase();
        let fallback_profile = self
            .profiles
            .get(&self.fallback_id)
            .cloned()
            .unwrap_or_default();

        if requested.is_empty() {
            return PhysicsResolution {
                requested_id: requested,
                resolved_id: self.fallback_id.clone(),
                matched: false,
                fallback_used: true,
                profile: fallback_profile,
            };
        }

        if let Some(profile) = self.profiles.get(&requested).cloned() {
            return PhysicsResolution {
                requested_id: requested.clone(),
                resolved_id: requested,
                matched: true,
                fallback_used: false,
                profile,
            };
        }

        PhysicsResolution {
            requested_id: requested,
            resolved_id: self.fallback_id.clone(),
            matched: false,
            fallback_used: true,
            profile: fallback_profile,
        }
    }
}

impl Default for PhysicsRegistry {
    fn default() -> Self {
        let mut registry = Self {
            profiles: HashMap::new(),
            fallback_id: PHYSICS_ID_DEFAULT.to_string(),
        };
        registry.register(PHYSICS_ID_DEFAULT, PhysicsProfile::default());
        registry.register(PHYSICS_ID_GAS, PhysicsProfile::gas());
        registry
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn physics_registry_resolves_default() {
        let registry = PhysicsRegistry::default();
        let resolution = registry.resolve(PHYSICS_ID_DEFAULT);

        assert!(resolution.matched);
        assert!(!resolution.fallback_used);
        assert_eq!(resolution.resolved_id, PHYSICS_ID_DEFAULT);
        assert_eq!(resolution.profile.name, "Liquid");
    }

    #[test]
    fn physics_registry_falls_back_for_unknown_id() {
        let registry = PhysicsRegistry::default();
        let resolution = registry.resolve("physics:unknown");

        assert!(!resolution.matched);
        assert!(resolution.fallback_used);
        assert_eq!(resolution.resolved_id, PHYSICS_ID_DEFAULT);
        assert_eq!(resolution.profile.name, "Liquid");
    }
}
