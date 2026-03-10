use crate::app::WorkbenchIntent;
pub(crate) use crate::registries::domain::layout::workbench_surface::WorkbenchSurfaceResolution;
use crate::registries::domain::layout::workbench_surface::{
    FocusHandoffPolicy, WorkbenchInteractionPolicy, WorkbenchLayoutPolicy, WorkbenchLock,
    WorkbenchSurfaceProfile, WorkbenchSurfaceRegistry as DomainWorkbenchSurfaceRegistry,
    WORKBENCH_SURFACE_COMPARE, WORKBENCH_SURFACE_DEFAULT, WORKBENCH_SURFACE_FOCUS,
};

pub(crate) const WORKBENCH_PROFILE_DEFAULT: &str = WORKBENCH_SURFACE_DEFAULT;
pub(crate) const WORKBENCH_PROFILE_FOCUS: &str = WORKBENCH_SURFACE_FOCUS;
pub(crate) const WORKBENCH_PROFILE_COMPARE: &str = WORKBENCH_SURFACE_COMPARE;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorkbenchSurfaceDescription {
    pub(crate) requested_id: String,
    pub(crate) resolved_id: String,
    pub(crate) matched: bool,
    pub(crate) fallback_used: bool,
    pub(crate) display_name: String,
    pub(crate) lock: WorkbenchLock,
}

pub(crate) struct WorkbenchSurfaceRegistry {
    profiles: DomainWorkbenchSurfaceRegistry,
    active_profile_id: String,
}

impl Default for WorkbenchSurfaceRegistry {
    fn default() -> Self {
        Self {
            profiles: DomainWorkbenchSurfaceRegistry::default(),
            active_profile_id: WORKBENCH_PROFILE_DEFAULT.to_string(),
        }
    }
}

impl WorkbenchSurfaceRegistry {
    pub(crate) fn active_profile_id(&self) -> &str {
        &self.active_profile_id
    }

    pub(crate) fn set_active_profile(&mut self, profile_id: &str) -> WorkbenchSurfaceResolution {
        let resolution = self.profiles.resolve(profile_id);
        self.active_profile_id = resolution.resolved_id.clone();
        resolution
    }

    pub(crate) fn active_profile(&self) -> WorkbenchSurfaceResolution {
        self.profiles.resolve(&self.active_profile_id)
    }

    pub(crate) fn resolve_layout_policy(&self) -> WorkbenchLayoutPolicy {
        self.active_profile().profile.layout
    }

    pub(crate) fn resolve_interaction_policy(&self) -> WorkbenchInteractionPolicy {
        self.active_profile().profile.interaction
    }

    pub(crate) fn resolve_focus_handoff_policy(&self) -> FocusHandoffPolicy {
        self.active_profile().profile.focus_handoff
    }

    pub(crate) fn resolve_profile(&self, profile_id: Option<&str>) -> WorkbenchSurfaceResolution {
        match profile_id {
            Some(profile_id) => self.profiles.resolve(profile_id),
            None => self.active_profile(),
        }
    }

    pub(crate) fn describe_surface(
        &self,
        profile_id: Option<&str>,
    ) -> WorkbenchSurfaceDescription {
        let resolution = self.resolve_profile(profile_id);
        WorkbenchSurfaceDescription {
            requested_id: resolution.requested_id,
            resolved_id: resolution.resolved_id,
            matched: resolution.matched,
            fallback_used: resolution.fallback_used,
            display_name: resolution.profile.display_name,
            lock: resolution.profile.lock,
        }
    }

    pub(crate) fn active_lock(&self) -> WorkbenchLock {
        self.active_profile().profile.lock
    }

    pub(crate) fn active_profile_snapshot(&self) -> WorkbenchSurfaceProfile {
        self.active_profile().profile
    }

    pub(crate) fn can_mutate(lock: WorkbenchLock, intent: &WorkbenchIntent) -> bool {
        match lock {
            WorkbenchLock::None => true,
            WorkbenchLock::PreventSplit => !matches!(
                intent,
                WorkbenchIntent::SplitPane { .. } | WorkbenchIntent::DetachNodeToSplit { .. }
            ),
            WorkbenchLock::PreventClose => !matches!(
                intent,
                WorkbenchIntent::ClosePane { .. } | WorkbenchIntent::CloseToolPane { .. }
            ),
            WorkbenchLock::FullLock => !matches!(
                intent,
                WorkbenchIntent::SplitPane { .. }
                    | WorkbenchIntent::DetachNodeToSplit { .. }
                    | WorkbenchIntent::ClosePane { .. }
                    | WorkbenchIntent::CloseToolPane { .. }
                    | WorkbenchIntent::OpenToolPane { .. }
                    | WorkbenchIntent::SetPaneView { .. }
                    | WorkbenchIntent::OpenGraphViewPane { .. }
                    | WorkbenchIntent::OpenNodeInPane { .. }
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::desktop::workbench::pane_model::{PaneId, SplitDirection};

    #[test]
    fn registry_defaults_to_default_profile() {
        let registry = WorkbenchSurfaceRegistry::default();

        assert_eq!(registry.active_profile_id(), WORKBENCH_PROFILE_DEFAULT);
        assert_eq!(
            registry.resolve_layout_policy().default_split_direction,
            crate::registries::domain::layout::workbench_surface::SplitDirection::Horizontal
        );
    }

    #[test]
    fn registry_switches_profiles_with_fallback() {
        let mut registry = WorkbenchSurfaceRegistry::default();

        let compare = registry.set_active_profile(WORKBENCH_PROFILE_COMPARE);
        assert!(compare.matched);
        assert_eq!(registry.active_profile_id(), WORKBENCH_PROFILE_COMPARE);

        let fallback = registry.set_active_profile("workbench_surface:missing");
        assert!(fallback.fallback_used);
        assert_eq!(registry.active_profile_id(), WORKBENCH_PROFILE_DEFAULT);
    }

    #[test]
    fn describe_surface_reports_resolution_metadata() {
        let registry = WorkbenchSurfaceRegistry::default();

        let description = registry.describe_surface(Some(WORKBENCH_PROFILE_FOCUS));
        assert_eq!(description.display_name, "Focus");
        assert_eq!(description.lock, WorkbenchLock::PreventSplit);
        assert!(description.matched);
    }

    #[test]
    fn mutation_guard_respects_lock_modes() {
        let split = WorkbenchIntent::SplitPane {
            source_pane: PaneId::new(),
            direction: SplitDirection::Horizontal,
        };
        let close = WorkbenchIntent::ClosePane {
            pane: PaneId::new(),
            restore_previous_focus: true,
        };

        assert!(!WorkbenchSurfaceRegistry::can_mutate(
            WorkbenchLock::PreventSplit,
            &split
        ));
        assert!(WorkbenchSurfaceRegistry::can_mutate(
            WorkbenchLock::PreventSplit,
            &close
        ));
        assert!(!WorkbenchSurfaceRegistry::can_mutate(
            WorkbenchLock::PreventClose,
            &close
        ));
        assert!(!WorkbenchSurfaceRegistry::can_mutate(WorkbenchLock::FullLock, &split));
    }
}
