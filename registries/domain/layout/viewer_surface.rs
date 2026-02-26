use std::collections::HashMap;

use super::{AccessibilityCapabilities, SecurityCapabilities};

pub(crate) const VIEWER_SURFACE_DEFAULT: &str = "viewer_surface:default";

#[derive(Debug, Clone)]
pub(crate) struct ViewerSurfaceProfile {
    pub(crate) profile_id: String,
    pub(crate) reader_mode_default: bool,
    pub(crate) smooth_scroll_enabled: bool,
    pub(crate) zoom_step: f32,
    /// Accessibility conformance declaration for this viewer surface profile.
    /// Defaults to `Full` for built-in profiles; degraded-path registrations
    /// should use `Partial` or `None` with a descriptive reason.
    pub(crate) accessibility: AccessibilityCapabilities,
    /// Security conformance declaration for this viewer surface profile.
    /// Reflects content isolation / sandbox guarantees for the viewer backend.
    pub(crate) security: SecurityCapabilities,
}

#[derive(Debug, Clone)]
pub(crate) struct ViewerSurfaceResolution {
    pub(crate) requested_id: String,
    pub(crate) resolved_id: String,
    pub(crate) matched: bool,
    pub(crate) fallback_used: bool,
    pub(crate) profile: ViewerSurfaceProfile,
}

pub(crate) struct ViewerSurfaceRegistry {
    profiles: HashMap<String, ViewerSurfaceProfile>,
    fallback_id: String,
}

impl ViewerSurfaceRegistry {
    pub(crate) fn register(&mut self, profile_id: &str, profile: ViewerSurfaceProfile) {
        self.profiles
            .insert(profile_id.to_ascii_lowercase(), profile);
    }

    pub(crate) fn resolve(&self, profile_id: &str) -> ViewerSurfaceResolution {
        let requested = profile_id.trim().to_ascii_lowercase();
        let fallback = self
            .profiles
            .get(&self.fallback_id)
            .cloned()
            .expect("viewer surface fallback profile must exist");

        if requested.is_empty() {
            return ViewerSurfaceResolution {
                requested_id: requested,
                resolved_id: self.fallback_id.clone(),
                matched: false,
                fallback_used: true,
                profile: fallback,
            };
        }

        if let Some(profile) = self.profiles.get(&requested).cloned() {
            return ViewerSurfaceResolution {
                requested_id: requested.clone(),
                resolved_id: requested,
                matched: true,
                fallback_used: false,
                profile,
            };
        }

        ViewerSurfaceResolution {
            requested_id: requested,
            resolved_id: self.fallback_id.clone(),
            matched: false,
            fallback_used: true,
            profile: fallback,
        }
    }
}

impl Default for ViewerSurfaceRegistry {
    fn default() -> Self {
        let mut registry = Self {
            profiles: HashMap::new(),
            fallback_id: VIEWER_SURFACE_DEFAULT.to_string(),
        };
        registry.register(
            VIEWER_SURFACE_DEFAULT,
            ViewerSurfaceProfile {
                profile_id: VIEWER_SURFACE_DEFAULT.to_string(),
                reader_mode_default: false,
                smooth_scroll_enabled: true,
                zoom_step: 1.1,
                accessibility: AccessibilityCapabilities::full(),
                security: SecurityCapabilities::full(),
            },
        );
        registry
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewer_surface_registry_falls_back_for_unknown_profile() {
        let registry = ViewerSurfaceRegistry::default();
        let resolution = registry.resolve("viewer_surface:unknown");
        assert!(!resolution.matched);
        assert!(resolution.fallback_used);
        assert_eq!(resolution.resolved_id, VIEWER_SURFACE_DEFAULT);
    }
}
