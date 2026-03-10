use crate::registries::domain::layout::canvas::{
    CANVAS_PROFILE_DEFAULT, CanvasLassoBinding, CanvasRegistry as DomainCanvasRegistry,
    CanvasSurfaceProfile, CanvasSurfaceResolution,
};

pub(crate) struct CanvasRegistry {
    profiles: DomainCanvasRegistry,
    active_profile_id: String,
}

impl Default for CanvasRegistry {
    fn default() -> Self {
        Self {
            profiles: DomainCanvasRegistry::default(),
            active_profile_id: CANVAS_PROFILE_DEFAULT.to_string(),
        }
    }
}

impl CanvasRegistry {
    pub(crate) fn active_profile_id(&self) -> &str {
        &self.active_profile_id
    }

    pub(crate) fn set_active_profile(&mut self, profile_id: &str) -> CanvasSurfaceResolution {
        let resolution = self.profiles.resolve(profile_id);
        self.active_profile_id = resolution.resolved_id.clone();
        resolution
    }

    pub(crate) fn active_profile(&self) -> CanvasSurfaceResolution {
        self.profiles.resolve(&self.active_profile_id)
    }

    pub(crate) fn active_profile_snapshot(&self) -> CanvasSurfaceProfile {
        self.active_profile().profile
    }

    fn update_active_profile(
        &mut self,
        mutator: impl FnOnce(&mut CanvasSurfaceProfile),
    ) -> CanvasSurfaceResolution {
        let mut profile = self.active_profile_snapshot();
        mutator(&mut profile);
        let profile_id = profile.profile_id.clone();
        self.profiles.register(&profile_id, profile);
        self.active_profile_id = profile_id;
        self.active_profile()
    }

    pub(crate) fn set_active_lasso_binding(
        &mut self,
        binding: CanvasLassoBinding,
    ) -> CanvasSurfaceResolution {
        self.update_active_profile(|profile| {
            profile.interaction.lasso_binding = binding;
        })
    }

    pub(crate) fn set_active_keyboard_pan_step(&mut self, step: f32) -> CanvasSurfaceResolution {
        self.update_active_profile(|profile| {
            profile.navigation.keyboard_pan_step = step.clamp(1.0, 200.0);
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canvas_registry_defaults_to_default_profile() {
        let registry = CanvasRegistry::default();
        let resolution = registry.active_profile();

        assert_eq!(registry.active_profile_id(), CANVAS_PROFILE_DEFAULT);
        assert!(resolution.matched);
        assert_eq!(resolution.resolved_id, CANVAS_PROFILE_DEFAULT);
    }

    #[test]
    fn canvas_registry_updates_active_profile_preferences() {
        let mut registry = CanvasRegistry::default();

        let updated = registry.set_active_keyboard_pan_step(42.0);
        assert_eq!(updated.profile.navigation.keyboard_pan_step, 42.0);

        let updated = registry.set_active_lasso_binding(CanvasLassoBinding::ShiftLeftDrag);
        assert_eq!(
            updated.profile.interaction.lasso_binding,
            CanvasLassoBinding::ShiftLeftDrag
        );
        assert_eq!(registry.active_profile_id(), CANVAS_PROFILE_DEFAULT);
    }
}
