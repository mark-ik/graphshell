use crate::graph::physics::{
    GraphPhysicsExtensionConfig, GraphPhysicsState, GraphPhysicsTuning, apply_graph_physics_tuning,
};

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
    pub(crate) fn graph_physics_tuning(&self) -> GraphPhysicsTuning {
        GraphPhysicsTuning {
            repulsion_strength: self.repulsion_strength,
            attraction_strength: self.attraction_strength,
            gravity_strength: self.gravity_strength,
            damping: self.damping,
        }
    }

    /// Build the physics extension config, gating frame-affinity by the
    /// canvas-level `zones_enabled` flag (spec §4.3).
    ///
    /// `zones_enabled` comes from `CanvasSurfaceProfile::zones_enabled()`,
    /// resolved via `phase3_resolve_active_canvas_profile()` at the render callsite.
    pub(crate) fn graph_physics_extensions(&self, zones_enabled: bool) -> GraphPhysicsExtensionConfig {
        GraphPhysicsExtensionConfig {
            degree_repulsion: self.degree_repulsion,
            domain_clustering: self.domain_clustering,
            semantic_clustering: self.semantic_clustering,
            semantic_strength: self.semantic_strength,
            frame_affinity: zones_enabled,
        }
    }

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

    pub fn apply_to_state(&self, state: &mut GraphPhysicsState) {
        apply_graph_physics_tuning(state, self.graph_physics_tuning());
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

    #[test]
    fn physics_profile_applies_tuning_via_graph_physics_adapter() {
        let mut state = GraphPhysicsState::default();
        let profile = PhysicsProfile {
            name: "Custom".to_string(),
            repulsion_strength: 0.61,
            attraction_strength: 0.19,
            gravity_strength: 0.27,
            damping: 0.48,
            degree_repulsion: true,
            domain_clustering: false,
            semantic_clustering: false,
            semantic_strength: 0.05,
            auto_pause: true,
        };

        profile.apply_to_state(&mut state);

        assert_eq!(state.base.c_repulse, 0.61);
        assert_eq!(state.base.c_attract, 0.19);
        assert_eq!(state.base.damping, 0.48);
        assert_eq!(state.extras.0.params.c, 0.27);
    }

    #[test]
    fn physics_profile_exposes_graph_physics_extensions() {
        let profile = PhysicsProfile {
            name: "Custom".to_string(),
            repulsion_strength: 0.61,
            attraction_strength: 0.19,
            gravity_strength: 0.27,
            damping: 0.48,
            degree_repulsion: false,
            domain_clustering: true,
            semantic_clustering: true,
            semantic_strength: 0.23,
            auto_pause: true,
        };

        let extensions = profile.graph_physics_extensions(false);

        assert!(!extensions.degree_repulsion);
        assert!(extensions.domain_clustering);
        assert!(extensions.semantic_clustering);
        assert_eq!(extensions.semantic_strength, 0.23);
        assert!(!extensions.frame_affinity);

        let extensions_zones = profile.graph_physics_extensions(true);
        assert!(extensions_zones.frame_affinity);
    }
}
