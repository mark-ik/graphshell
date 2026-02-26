use std::collections::HashMap;

use super::{
    AccessibilityCapabilities, HistoryCapabilities, SecurityCapabilities, StorageCapabilities,
};

pub(crate) const WORKBENCH_SURFACE_DEFAULT: &str = "workbench_surface:default";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct WorkbenchLayoutPolicy {
    pub(crate) all_panes_must_have_tabs: bool,
    pub(crate) split_horizontal_default: bool,
    pub(crate) tab_wrapping_enabled: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct WorkbenchInteractionPolicy {
    pub(crate) tab_detach_enabled: bool,
    pub(crate) tab_detach_band_margin: f32,
    pub(crate) title_truncation_chars: usize,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct WorkbenchSurfaceProfile {
    pub(crate) profile_id: String,
    pub(crate) layout: WorkbenchLayoutPolicy,
    pub(crate) interaction: WorkbenchInteractionPolicy,
    pub(crate) split_horizontal_label: String,
    pub(crate) split_vertical_label: String,
    pub(crate) tab_group_label: String,
    pub(crate) grid_label: String,
    /// Accessibility conformance declaration for this workbench surface profile.
    pub(crate) accessibility: AccessibilityCapabilities,
    /// Security conformance declaration for this workbench surface profile.
    pub(crate) security: SecurityCapabilities,
    /// Storage conformance declaration for this workbench surface profile.
    pub(crate) storage: StorageCapabilities,
    /// History conformance declaration for this workbench surface profile.
    pub(crate) history: HistoryCapabilities,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct WorkbenchSurfaceResolution {
    pub(crate) requested_id: String,
    pub(crate) resolved_id: String,
    pub(crate) matched: bool,
    pub(crate) fallback_used: bool,
    pub(crate) profile: WorkbenchSurfaceProfile,
}

pub(crate) struct WorkbenchSurfaceRegistry {
    profiles: HashMap<String, WorkbenchSurfaceProfile>,
    fallback_id: String,
}

impl WorkbenchSurfaceRegistry {
    pub(crate) fn register(&mut self, profile_id: &str, profile: WorkbenchSurfaceProfile) {
        self.profiles
            .insert(profile_id.to_ascii_lowercase(), profile);
    }

    pub(crate) fn resolve(&self, profile_id: &str) -> WorkbenchSurfaceResolution {
        let requested = profile_id.trim().to_ascii_lowercase();
        let fallback = self
            .profiles
            .get(&self.fallback_id)
            .cloned()
            .expect("workbench surface fallback profile must exist");

        if requested.is_empty() {
            return WorkbenchSurfaceResolution {
                requested_id: requested,
                resolved_id: self.fallback_id.clone(),
                matched: false,
                fallback_used: true,
                profile: fallback,
            };
        }

        if let Some(profile) = self.profiles.get(&requested).cloned() {
            return WorkbenchSurfaceResolution {
                requested_id: requested.clone(),
                resolved_id: requested,
                matched: true,
                fallback_used: false,
                profile,
            };
        }

        WorkbenchSurfaceResolution {
            requested_id: requested,
            resolved_id: self.fallback_id.clone(),
            matched: false,
            fallback_used: true,
            profile: fallback,
        }
    }
}

impl Default for WorkbenchSurfaceRegistry {
    fn default() -> Self {
        let mut registry = Self {
            profiles: HashMap::new(),
            fallback_id: WORKBENCH_SURFACE_DEFAULT.to_string(),
        };
        registry.register(
            WORKBENCH_SURFACE_DEFAULT,
            WorkbenchSurfaceProfile {
                profile_id: WORKBENCH_SURFACE_DEFAULT.to_string(),
                layout: WorkbenchLayoutPolicy {
                    all_panes_must_have_tabs: true,
                    split_horizontal_default: true,
                    tab_wrapping_enabled: false,
                },
                interaction: WorkbenchInteractionPolicy {
                    tab_detach_enabled: true,
                    tab_detach_band_margin: 12.0,
                    title_truncation_chars: 26,
                },
                split_horizontal_label: "Split ↔".to_string(),
                split_vertical_label: "Split ↕".to_string(),
                tab_group_label: "Tab Group".to_string(),
                grid_label: "Grid".to_string(),
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
    fn workbench_surface_registry_resolves_default() {
        let registry = WorkbenchSurfaceRegistry::default();
        let resolution = registry.resolve(WORKBENCH_SURFACE_DEFAULT);
        assert!(resolution.matched);
        assert_eq!(resolution.profile.tab_group_label, "Tab Group");
        assert!(resolution.profile.layout.all_panes_must_have_tabs);
        assert!(resolution.profile.interaction.tab_detach_enabled);
        assert_eq!(resolution.profile.interaction.title_truncation_chars, 26);
    }

    #[test]
    fn workbench_surface_resolution_round_trips_via_json() {
        let registry = WorkbenchSurfaceRegistry::default();
        let resolution = registry.resolve(WORKBENCH_SURFACE_DEFAULT);

        let json = serde_json::to_string(&resolution).expect("resolution should serialize");
        let restored: WorkbenchSurfaceResolution =
            serde_json::from_str(&json).expect("resolution should deserialize");

        assert_eq!(restored.resolved_id, WORKBENCH_SURFACE_DEFAULT);
        assert_eq!(
            restored.profile.accessibility.level,
            super::super::ConformanceLevel::Full
        );
    }
}
