use std::collections::HashMap;

use super::{
    AccessibilityCapabilities, HistoryCapabilities, SecurityCapabilities, StorageCapabilities,
};

pub(crate) const CANVAS_PROFILE_DEFAULT: &str = "canvas:default";

#[derive(Debug, Clone)]
pub(crate) struct CanvasTopologyPolicy {
    pub(crate) policy_id: String,
    pub(crate) directed: bool,
    pub(crate) cycles_allowed: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct CanvasLayoutAlgorithmPolicy {
    pub(crate) algorithm_id: String,
}

#[derive(Debug, Clone)]
pub(crate) struct CanvasNavigationPolicy {
    pub(crate) fit_to_screen_enabled: bool,
    pub(crate) zoom_and_pan_enabled: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct CanvasInteractionPolicy {
    pub(crate) dragging_enabled: bool,
    pub(crate) node_selection_enabled: bool,
    pub(crate) node_clicking_enabled: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct CanvasStylePolicy {
    pub(crate) labels_always: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum EdgeLodPolicy {
    Full,
    SkipLabels,
    Hidden,
}

/// Rendering performance and quality policy controls.
///
/// These toggles gate Phase 1 performance optimizations so behavior remains
/// policy-driven rather than hardcoded in render callsites.
#[derive(Debug, Clone)]
pub(crate) struct CanvasPerformancePolicy {
    /// When true, only nodes within the visible viewport are submitted to the
    /// graph renderer each frame.
    pub(crate) viewport_culling_enabled: bool,
    pub(crate) label_culling_enabled: bool,
    pub(crate) edge_lod: EdgeLodPolicy,
}

#[derive(Debug, Clone)]
pub(crate) struct CanvasSurfaceProfile {
    pub(crate) profile_id: String,
    pub(crate) topology: CanvasTopologyPolicy,
    pub(crate) layout_algorithm: CanvasLayoutAlgorithmPolicy,
    pub(crate) navigation: CanvasNavigationPolicy,
    pub(crate) interaction: CanvasInteractionPolicy,
    pub(crate) style: CanvasStylePolicy,
    pub(crate) performance: CanvasPerformancePolicy,
    /// Accessibility conformance declaration for this canvas surface profile.
    pub(crate) accessibility: AccessibilityCapabilities,
    /// Security conformance declaration for this canvas surface profile.
    pub(crate) security: SecurityCapabilities,
    /// Storage conformance declaration for this canvas surface profile.
    pub(crate) storage: StorageCapabilities,
    /// History conformance declaration for this canvas surface profile.
    pub(crate) history: HistoryCapabilities,
}

#[derive(Debug, Clone)]
pub(crate) struct CanvasSurfaceResolution {
    pub(crate) requested_id: String,
    pub(crate) resolved_id: String,
    pub(crate) matched: bool,
    pub(crate) fallback_used: bool,
    pub(crate) profile: CanvasSurfaceProfile,
}

pub(crate) struct CanvasRegistry {
    profiles: HashMap<String, CanvasSurfaceProfile>,
    fallback_id: String,
}

impl CanvasRegistry {
    pub(crate) fn register(&mut self, profile_id: &str, profile: CanvasSurfaceProfile) {
        self.profiles
            .insert(profile_id.to_ascii_lowercase(), profile);
    }

    pub(crate) fn resolve(&self, profile_id: &str) -> CanvasSurfaceResolution {
        let requested = profile_id.trim().to_ascii_lowercase();
        let fallback = self
            .profiles
            .get(&self.fallback_id)
            .cloned()
            .expect("canvas fallback profile must exist");

        if requested.is_empty() {
            return CanvasSurfaceResolution {
                requested_id: requested,
                resolved_id: self.fallback_id.clone(),
                matched: false,
                fallback_used: true,
                profile: fallback,
            };
        }

        if let Some(profile) = self.profiles.get(&requested).cloned() {
            return CanvasSurfaceResolution {
                requested_id: requested.clone(),
                resolved_id: requested,
                matched: true,
                fallback_used: false,
                profile,
            };
        }

        CanvasSurfaceResolution {
            requested_id: requested,
            resolved_id: self.fallback_id.clone(),
            matched: false,
            fallback_used: true,
            profile: fallback,
        }
    }
}

impl Default for CanvasRegistry {
    fn default() -> Self {
        let mut registry = Self {
            profiles: HashMap::new(),
            fallback_id: CANVAS_PROFILE_DEFAULT.to_string(),
        };
        registry.register(
            CANVAS_PROFILE_DEFAULT,
            CanvasSurfaceProfile {
                profile_id: CANVAS_PROFILE_DEFAULT.to_string(),
                topology: CanvasTopologyPolicy {
                    policy_id: "topology:free".to_string(),
                    directed: false,
                    cycles_allowed: true,
                },
                layout_algorithm: CanvasLayoutAlgorithmPolicy {
                    algorithm_id: "graph_layout:force_directed".to_string(),
                },
                navigation: CanvasNavigationPolicy {
                    fit_to_screen_enabled: false,
                    zoom_and_pan_enabled: false,
                },
                interaction: CanvasInteractionPolicy {
                    dragging_enabled: true,
                    node_selection_enabled: true,
                    node_clicking_enabled: true,
                },
                style: CanvasStylePolicy {
                    labels_always: true,
                },
                performance: CanvasPerformancePolicy {
                    viewport_culling_enabled: true,
                    label_culling_enabled: false,
                    edge_lod: EdgeLodPolicy::Full,
                },
                accessibility: AccessibilityCapabilities::full(),
                security: SecurityCapabilities::full(),
                storage: StorageCapabilities::full(),
                history: HistoryCapabilities::full(),
            },
        );
        registry
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canvas_registry_resolves_default_profile() {
        let registry = CanvasRegistry::default();
        let resolution = registry.resolve(CANVAS_PROFILE_DEFAULT);
        assert!(resolution.matched);
        assert!(!resolution.fallback_used);
        assert_eq!(resolution.resolved_id, CANVAS_PROFILE_DEFAULT);
        assert_eq!(resolution.profile.topology.policy_id, "topology:free");
        assert_eq!(
            resolution.profile.layout_algorithm.algorithm_id,
            "graph_layout:force_directed"
        );
        assert!(!resolution.profile.navigation.zoom_and_pan_enabled);
        assert!(resolution.profile.performance.viewport_culling_enabled);
    }

    #[test]
    fn canvas_registry_falls_back_for_unknown_profile() {
        let registry = CanvasRegistry::default();
        let resolution = registry.resolve("canvas:unknown");
        assert!(!resolution.matched);
        assert!(resolution.fallback_used);
        assert_eq!(resolution.resolved_id, CANVAS_PROFILE_DEFAULT);
    }

    #[test]
    fn canvas_registry_default_profile_has_performance_policy() {
        let registry = CanvasRegistry::default();
        let resolution = registry.resolve(CANVAS_PROFILE_DEFAULT);
        let perf = &resolution.profile.performance;
        assert!(perf.viewport_culling_enabled);
        assert!(!perf.label_culling_enabled);
        assert_eq!(perf.edge_lod, EdgeLodPolicy::Full);
    }

    #[test]
    fn canvas_registry_custom_profile_with_culling_enabled() {
        let mut registry = CanvasRegistry::default();
        registry.register(
            "canvas:perf",
            CanvasSurfaceProfile {
                profile_id: "canvas:perf".to_string(),
                topology: CanvasTopologyPolicy {
                    policy_id: "topology:free".to_string(),
                    directed: false,
                    cycles_allowed: true,
                },
                layout_algorithm: CanvasLayoutAlgorithmPolicy {
                    algorithm_id: "graph_layout:force_directed".to_string(),
                },
                navigation: CanvasNavigationPolicy {
                    fit_to_screen_enabled: false,
                    zoom_and_pan_enabled: false,
                },
                interaction: CanvasInteractionPolicy {
                    dragging_enabled: true,
                    node_selection_enabled: true,
                    node_clicking_enabled: true,
                },
                style: CanvasStylePolicy {
                    labels_always: false,
                },
                performance: CanvasPerformancePolicy {
                    viewport_culling_enabled: true,
                    label_culling_enabled: true,
                    edge_lod: EdgeLodPolicy::SkipLabels,
                },
                accessibility: AccessibilityCapabilities::full(),
                security: SecurityCapabilities::full(),
                storage: StorageCapabilities::full(),
                history: HistoryCapabilities::full(),
            },
        );
        let resolution = registry.resolve("canvas:perf");
        assert!(resolution.matched);
        let perf = &resolution.profile.performance;
        assert!(perf.viewport_culling_enabled);
        assert!(perf.label_culling_enabled);
        assert_eq!(perf.edge_lod, EdgeLodPolicy::SkipLabels);
    }
}
