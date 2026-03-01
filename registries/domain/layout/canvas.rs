use std::collections::HashMap;

use super::SurfaceSubsystemCapabilities;

pub(crate) const CANVAS_PROFILE_DEFAULT: &str = "canvas:default";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct CanvasTopologyPolicy {
    pub(crate) policy_id: String,
    pub(crate) directed: bool,
    pub(crate) cycles_allowed: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct CanvasLayoutAlgorithmPolicy {
    pub(crate) algorithm_id: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct CanvasNavigationPolicy {
    pub(crate) fit_to_screen_enabled: bool,
    pub(crate) zoom_and_pan_enabled: bool,
    pub(crate) wheel_zoom_requires_ctrl: bool,
    #[serde(default = "default_keyboard_zoom_step")]
    pub(crate) keyboard_zoom_step: f32,
    #[serde(default = "default_wheel_zoom_impulse_scale")]
    pub(crate) wheel_zoom_impulse_scale: f32,
    #[serde(default = "default_wheel_zoom_inertia_damping")]
    pub(crate) wheel_zoom_inertia_damping: f32,
    #[serde(default = "default_wheel_zoom_inertia_min_abs")]
    pub(crate) wheel_zoom_inertia_min_abs: f32,
    #[serde(default = "default_camera_fit_padding")]
    pub(crate) camera_fit_padding: f32,
    #[serde(default = "default_camera_fit_relax")]
    pub(crate) camera_fit_relax: f32,
    #[serde(default = "default_camera_focus_selection_padding")]
    pub(crate) camera_focus_selection_padding: f32,
}

fn default_keyboard_zoom_step() -> f32 {
    1.1
}

fn default_camera_fit_padding() -> f32 {
    1.1
}

fn default_camera_fit_relax() -> f32 {
    0.5
}

fn default_camera_focus_selection_padding() -> f32 {
    1.2
}

fn default_wheel_zoom_impulse_scale() -> f32 {
    0.012
}

fn default_wheel_zoom_inertia_damping() -> f32 {
    0.86
}

fn default_wheel_zoom_inertia_min_abs() -> f32 {
    0.00035
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum CanvasLassoBinding {
    RightDrag,
    ShiftLeftDrag,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct CanvasInteractionPolicy {
    pub(crate) dragging_enabled: bool,
    pub(crate) node_selection_enabled: bool,
    pub(crate) node_clicking_enabled: bool,
    pub(crate) lasso_binding: CanvasLassoBinding,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct CanvasStylePolicy {
    pub(crate) labels_always: bool,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) enum EdgeLodPolicy {
    Full,
    SkipLabels,
    Hidden,
}

/// Rendering performance and quality policy controls.
///
/// These toggles gate Phase 1 performance optimizations so behavior remains
/// policy-driven rather than hardcoded in render callsites.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct CanvasPerformancePolicy {
    /// When true, only nodes within the visible viewport are submitted to the
    /// graph renderer each frame.
    pub(crate) viewport_culling_enabled: bool,
    pub(crate) label_culling_enabled: bool,
    pub(crate) edge_lod: EdgeLodPolicy,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct CanvasSurfaceProfile {
    pub(crate) profile_id: String,
    pub(crate) topology: CanvasTopologyPolicy,
    pub(crate) layout_algorithm: CanvasLayoutAlgorithmPolicy,
    pub(crate) navigation: CanvasNavigationPolicy,
    pub(crate) interaction: CanvasInteractionPolicy,
    pub(crate) style: CanvasStylePolicy,
    pub(crate) performance: CanvasPerformancePolicy,
    /// Folded subsystem conformance declarations for this canvas surface.
    #[serde(flatten)]
    pub(crate) subsystems: SurfaceSubsystemCapabilities,
}

impl CanvasSurfaceProfile {
    pub(crate) fn should_capture_wheel_zoom(&self, ctrl_pressed: bool) -> bool {
        !self.navigation.wheel_zoom_requires_ctrl || ctrl_pressed
    }

    pub(crate) fn allows_background_pan(
        &self,
        no_hovered_node: bool,
        pointer_inside: bool,
        primary_down: bool,
        lasso_primary_drag_active: bool,
        _radial_open: bool,
        right_button_down: bool,
    ) -> bool {
        let is_tree_topology = self.topology.policy_id == "topology:tree";
        pointer_inside
            && no_hovered_node
            && primary_down
            && !lasso_primary_drag_active
            && self.interaction.dragging_enabled
            && !right_button_down
            && !is_tree_topology
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
                    fit_to_screen_enabled: true,
                    zoom_and_pan_enabled: true,
                    wheel_zoom_requires_ctrl: false,
                    keyboard_zoom_step: default_keyboard_zoom_step(),
                    camera_fit_padding: default_camera_fit_padding(),
                    wheel_zoom_impulse_scale: default_wheel_zoom_impulse_scale(),
                    wheel_zoom_inertia_damping: default_wheel_zoom_inertia_damping(),
                    wheel_zoom_inertia_min_abs: default_wheel_zoom_inertia_min_abs(),
                    camera_fit_relax: default_camera_fit_relax(),
                    camera_focus_selection_padding: default_camera_focus_selection_padding(),
                },
                interaction: CanvasInteractionPolicy {
                    dragging_enabled: true,
                    node_selection_enabled: true,
                    node_clicking_enabled: true,
                    lasso_binding: CanvasLassoBinding::RightDrag,
                },
                style: CanvasStylePolicy {
                    labels_always: true,
                },
                performance: CanvasPerformancePolicy {
                    viewport_culling_enabled: true,
                    label_culling_enabled: false,
                    edge_lod: EdgeLodPolicy::Full,
                },
                subsystems: SurfaceSubsystemCapabilities::full(),
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
        assert!(resolution.profile.navigation.zoom_and_pan_enabled);
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
                    wheel_zoom_requires_ctrl: false,
                    keyboard_zoom_step: default_keyboard_zoom_step(),
                    camera_fit_padding: default_camera_fit_padding(),
                    wheel_zoom_impulse_scale: default_wheel_zoom_impulse_scale(),
                    wheel_zoom_inertia_damping: default_wheel_zoom_inertia_damping(),
                    wheel_zoom_inertia_min_abs: default_wheel_zoom_inertia_min_abs(),
                    camera_fit_relax: default_camera_fit_relax(),
                    camera_focus_selection_padding: default_camera_focus_selection_padding(),
                },
                interaction: CanvasInteractionPolicy {
                    dragging_enabled: true,
                    node_selection_enabled: true,
                    node_clicking_enabled: true,
                    lasso_binding: CanvasLassoBinding::RightDrag,
                },
                style: CanvasStylePolicy {
                    labels_always: false,
                },
                performance: CanvasPerformancePolicy {
                    viewport_culling_enabled: true,
                    label_culling_enabled: true,
                    edge_lod: EdgeLodPolicy::SkipLabels,
                },
                subsystems: SurfaceSubsystemCapabilities::full(),
            },
        );
        let resolution = registry.resolve("canvas:perf");
        assert!(resolution.matched);
        let perf = &resolution.profile.performance;
        assert!(perf.viewport_culling_enabled);
        assert!(perf.label_culling_enabled);
        assert_eq!(perf.edge_lod, EdgeLodPolicy::SkipLabels);
    }

    #[test]
    fn canvas_surface_profile_allows_background_pan_when_conditions_met() {
        let registry = CanvasRegistry::default();
        let resolution = registry.resolve(CANVAS_PROFILE_DEFAULT);
        assert!(resolution.profile.allows_background_pan(
            true, true, true, false, false, false,
        ));
    }

    #[test]
    fn canvas_surface_profile_blocks_background_pan_for_active_lasso_drag() {
        let registry = CanvasRegistry::default();
        let resolution = registry.resolve(CANVAS_PROFILE_DEFAULT);
        assert!(!resolution.profile.allows_background_pan(
            true, true, true, true, false, false,
        ));
    }

    #[test]
    fn canvas_surface_profile_allows_background_pan_when_radial_menu_open() {
        let registry = CanvasRegistry::default();
        let resolution = registry.resolve(CANVAS_PROFILE_DEFAULT);
        assert!(resolution.profile.allows_background_pan(
            true, true, true, false, true, false,
        ));
    }

    #[test]
    fn canvas_surface_profile_wheel_zoom_capture_without_ctrl_requirement() {
        let registry = CanvasRegistry::default();
        let resolution = registry.resolve(CANVAS_PROFILE_DEFAULT);
        assert!(resolution.profile.should_capture_wheel_zoom(false));
        assert!(resolution.profile.should_capture_wheel_zoom(true));
        assert!(resolution.profile.navigation.keyboard_zoom_step > 1.0);
        assert!(resolution.profile.navigation.camera_fit_padding > 1.0);
        assert!(resolution.profile.navigation.camera_fit_relax > 0.0);
        assert!(resolution.profile.navigation.camera_focus_selection_padding > 1.0);
        assert!(resolution.profile.navigation.wheel_zoom_impulse_scale > 0.0);
        assert!(resolution.profile.navigation.wheel_zoom_inertia_damping > 0.0);
        assert!(resolution.profile.navigation.wheel_zoom_inertia_min_abs > 0.0);
    }

    #[test]
    fn canvas_surface_profile_wheel_zoom_capture_with_ctrl_requirement() {
        let mut registry = CanvasRegistry::default();
        registry.register(
            "canvas:ctrl-wheel",
            CanvasSurfaceProfile {
                profile_id: "canvas:ctrl-wheel".to_string(),
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
                    wheel_zoom_requires_ctrl: true,
                    keyboard_zoom_step: default_keyboard_zoom_step(),
                    camera_fit_padding: default_camera_fit_padding(),
                    wheel_zoom_impulse_scale: default_wheel_zoom_impulse_scale(),
                    wheel_zoom_inertia_damping: default_wheel_zoom_inertia_damping(),
                    wheel_zoom_inertia_min_abs: default_wheel_zoom_inertia_min_abs(),
                    camera_fit_relax: default_camera_fit_relax(),
                    camera_focus_selection_padding: default_camera_focus_selection_padding(),
                },
                interaction: CanvasInteractionPolicy {
                    dragging_enabled: true,
                    node_selection_enabled: true,
                    node_clicking_enabled: true,
                    lasso_binding: CanvasLassoBinding::RightDrag,
                },
                style: CanvasStylePolicy {
                    labels_always: true,
                },
                performance: CanvasPerformancePolicy {
                    viewport_culling_enabled: true,
                    label_culling_enabled: false,
                    edge_lod: EdgeLodPolicy::Full,
                },
                subsystems: SurfaceSubsystemCapabilities::full(),
            },
        );
        let resolution = registry.resolve("canvas:ctrl-wheel");
        assert!(!resolution.profile.should_capture_wheel_zoom(false));
        assert!(resolution.profile.should_capture_wheel_zoom(true));
    }
}
