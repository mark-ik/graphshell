use egui_graphs::FruchtermanReingoldWithCenterGravityState;

pub(crate) const PHYSICS_ID_DEFAULT: &str = "physics:liquid";
pub(crate) const PHYSICS_ID_GAS: &str = "physics:gas";
pub(crate) const PHYSICS_ID_SOLID: &str = "physics:solid";
const PHYSICS_ID_LEGACY_DEFAULT: &str = "physics:default";

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PhysicsProfile {
    pub name: String,
    pub repulsion_strength: f32,
    pub attraction_strength: f32,
    pub gravity_strength: f32,
    pub damping: f32,
    pub degree_repulsion: bool,
    pub domain_clustering: bool,
    pub semantic_clustering: bool,
    pub semantic_strength: f32,
    pub auto_pause: bool,
}

impl Default for PhysicsProfile {
    fn default() -> Self {
        Self::liquid()
    }
}

impl PhysicsProfile {
    pub fn liquid() -> Self {
        Self {
            name: "Liquid".to_string(),
            repulsion_strength: 0.28,
            attraction_strength: 0.22,
            gravity_strength: 0.18,
            damping: 0.55,
            degree_repulsion: true,
            domain_clustering: false,
            semantic_clustering: false,
            semantic_strength: 0.05,
            auto_pause: true,
        }
    }

    pub fn gas() -> Self {
        Self {
            name: "Gas".to_string(),
            repulsion_strength: 0.8,
            attraction_strength: 0.05,
            gravity_strength: 0.0,
            damping: 0.8,
            degree_repulsion: false,
            domain_clustering: false,
            semantic_clustering: false,
            semantic_strength: 0.05,
            auto_pause: false,
        }
    }

    pub fn solid() -> Self {
        Self {
            name: "Solid".to_string(),
            repulsion_strength: 0.12,
            attraction_strength: 0.42,
            gravity_strength: 0.24,
            damping: 0.4,
            degree_repulsion: true,
            domain_clustering: true,
            semantic_clustering: false,
            semantic_strength: 0.05,
            auto_pause: true,
        }
    }

    pub fn apply_to_state(&self, state: &mut FruchtermanReingoldWithCenterGravityState) {
        state.base.c_repulse = self.repulsion_strength;
        state.base.c_attract = self.attraction_strength;
        state.base.damping = self.damping;
        state.extras.0.params.c = self.gravity_strength;
    }
}

#[derive(Debug, Clone)]
pub(crate) struct PhysicsProfileResolution {
    pub(crate) requested_id: String,
    pub(crate) resolved_id: String,
    pub(crate) matched: bool,
    pub(crate) fallback_used: bool,
    pub(crate) profile: PhysicsProfile,
}

pub(crate) fn resolve_physics_profile(physics_id: &str) -> PhysicsProfileResolution {
    let requested = physics_id.trim().to_ascii_lowercase();
    let canonical_requested = if requested == PHYSICS_ID_LEGACY_DEFAULT {
        PHYSICS_ID_DEFAULT.to_string()
    } else {
        requested.clone()
    };
    let fallback_profile = PhysicsProfile::default();

    if requested.is_empty() {
        return PhysicsProfileResolution {
            requested_id: requested,
            resolved_id: PHYSICS_ID_DEFAULT.to_string(),
            matched: false,
            fallback_used: true,
            profile: fallback_profile,
        };
    }

    let profile = match canonical_requested.as_str() {
        PHYSICS_ID_DEFAULT => Some(PhysicsProfile::default()),
        PHYSICS_ID_GAS => Some(PhysicsProfile::gas()),
        PHYSICS_ID_SOLID => Some(PhysicsProfile::solid()),
        _ => None,
    };

    if let Some(profile) = profile {
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
        resolved_id: PHYSICS_ID_DEFAULT.to_string(),
        matched: false,
        fallback_used: true,
        profile: fallback_profile,
    }
}

pub(crate) fn physics_profile_id(profile: &PhysicsProfile) -> &'static str {
    if *profile == PhysicsProfile::gas() {
        PHYSICS_ID_GAS
    } else if *profile == PhysicsProfile::solid() {
        PHYSICS_ID_SOLID
    } else {
        PHYSICS_ID_DEFAULT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn physics_profile_lookup_maps_legacy_default_to_liquid_id() {
        let resolution = resolve_physics_profile(PHYSICS_ID_LEGACY_DEFAULT);

        assert!(resolution.matched);
        assert_eq!(resolution.requested_id, PHYSICS_ID_LEGACY_DEFAULT);
        assert_eq!(resolution.resolved_id, PHYSICS_ID_DEFAULT);
        assert_eq!(resolution.profile.name, "Liquid");
    }
}
