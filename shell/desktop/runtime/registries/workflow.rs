use std::collections::HashMap;

use crate::app::{GraphBrowserApp, GraphIntent};
use crate::registries::atomic::lens::{
    LENS_ID_DEFAULT, PHYSICS_ID_DEFAULT, PHYSICS_ID_GAS, PHYSICS_ID_SOLID, THEME_ID_DARK,
    THEME_ID_DEFAULT,
};
use crate::registries::domain::layout::canvas::CANVAS_PROFILE_DEFAULT;
use crate::shell::desktop::runtime::registries::workbench_surface::{
    WORKBENCH_PROFILE_COMPARE, WORKBENCH_PROFILE_DEFAULT, WORKBENCH_PROFILE_FOCUS,
};

pub(crate) const WORKFLOW_DEFAULT: &str = "workflow:default";
pub(crate) const WORKFLOW_RESEARCH: &str = "workflow:research";
pub(crate) const WORKFLOW_READING: &str = "workflow:reading";
pub(crate) const WORKFLOW_HISTORY: &str = "workflow:history";
pub(crate) const WORKFLOW_PRESENCE: &str = "workflow:presence";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorkflowDescriptor {
    pub(crate) id: String,
    pub(crate) display_name: String,
    pub(crate) lens_profile: String,
    pub(crate) workbench_profile: String,
    pub(crate) canvas_profile: String,
    pub(crate) physics_profile: String,
    pub(crate) theme_profile: Option<String>,
    pub(crate) implemented: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorkflowResolution {
    pub(crate) requested_id: String,
    pub(crate) resolved_id: String,
    pub(crate) matched: bool,
    pub(crate) fallback_used: bool,
    pub(crate) descriptor: WorkflowDescriptor,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorkflowCapability {
    pub(crate) requested_id: String,
    pub(crate) resolved_id: String,
    pub(crate) matched: bool,
    pub(crate) fallback_used: bool,
    pub(crate) display_name: String,
    pub(crate) implemented: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct WorkflowActivation {
    pub(crate) workflow_id: String,
    pub(crate) workbench_profile_id: String,
    pub(crate) canvas_profile_id: String,
    pub(crate) physics_profile_id: String,
    pub(crate) theme_profile_id: Option<String>,
    pub(crate) wal_intent: GraphIntent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WorkflowActivationError {
    NotImplemented { workflow_id: String },
}

pub(crate) struct WorkflowRegistry {
    workflows: HashMap<String, WorkflowDescriptor>,
    active: Option<String>,
    fallback_id: String,
}

impl Default for WorkflowRegistry {
    fn default() -> Self {
        let mut registry = Self {
            workflows: HashMap::new(),
            active: None,
            fallback_id: WORKFLOW_DEFAULT.to_string(),
        };
        for descriptor in builtin_workflows() {
            registry.register(descriptor);
        }
        registry
    }
}

impl WorkflowRegistry {
    pub(crate) fn register(&mut self, descriptor: WorkflowDescriptor) {
        self.workflows
            .insert(descriptor.id.to_ascii_lowercase(), descriptor);
    }

    pub(crate) fn active_workflow_id(&self) -> Option<&str> {
        self.active.as_deref()
    }

    pub(crate) fn resolve_workflow(&self, workflow_id: Option<&str>) -> WorkflowResolution {
        let requested = workflow_id
            .unwrap_or(self.fallback_id.as_str())
            .trim()
            .to_ascii_lowercase();
        let fallback = self
            .workflows
            .get(&self.fallback_id)
            .cloned()
            .unwrap_or_else(|| panic!("workflow fallback must exist"));

        if requested.is_empty() {
            return WorkflowResolution {
                requested_id: requested,
                resolved_id: self.fallback_id.clone(),
                matched: false,
                fallback_used: true,
                descriptor: fallback,
            };
        }

        if let Some(descriptor) = self.workflows.get(&requested).cloned() {
            return WorkflowResolution {
                requested_id: requested.clone(),
                resolved_id: requested,
                matched: true,
                fallback_used: false,
                descriptor,
            };
        }

        WorkflowResolution {
            requested_id: requested,
            resolved_id: self.fallback_id.clone(),
            matched: false,
            fallback_used: true,
            descriptor: fallback,
        }
    }

    pub(crate) fn describe_workflow(&self, workflow_id: Option<&str>) -> WorkflowCapability {
        let resolution = self.resolve_workflow(workflow_id);
        WorkflowCapability {
            requested_id: resolution.requested_id,
            resolved_id: resolution.resolved_id,
            matched: resolution.matched,
            fallback_used: resolution.fallback_used,
            display_name: resolution.descriptor.display_name,
            implemented: resolution.descriptor.implemented,
        }
    }

    pub(crate) fn activate(
        &mut self,
        graph_app: &mut GraphBrowserApp,
        workbench_profile_id: String,
        resolution: WorkflowResolution,
    ) -> Result<WorkflowActivation, WorkflowActivationError> {
        if !resolution.descriptor.implemented {
            return Err(WorkflowActivationError::NotImplemented {
                workflow_id: resolution.resolved_id,
            });
        }

        let descriptor = resolution.descriptor;
        graph_app.set_default_registry_lens_id(Some(&descriptor.lens_profile));
        graph_app.set_default_registry_physics_id(Some(&descriptor.physics_profile));
        graph_app.set_default_registry_theme_id(descriptor.theme_profile.as_deref());
        graph_app.save_workspace_layout_json(
            GraphBrowserApp::SETTINGS_CANVAS_PROFILE_ID_NAME,
            &descriptor.canvas_profile,
        );
        graph_app.save_workspace_layout_json(
            GraphBrowserApp::SETTINGS_WORKBENCH_SURFACE_PROFILE_ID_NAME,
            &workbench_profile_id,
        );
        graph_app.save_workspace_layout_json(
            GraphBrowserApp::SETTINGS_ACTIVE_WORKFLOW_ID_NAME,
            &descriptor.id,
        );

        self.active = Some(descriptor.id.clone());

        Ok(WorkflowActivation {
            workflow_id: descriptor.id.clone(),
            workbench_profile_id,
            canvas_profile_id: descriptor.canvas_profile,
            physics_profile_id: descriptor.physics_profile,
            theme_profile_id: descriptor.theme_profile,
            wal_intent: GraphIntent::WorkflowActivated {
                workflow_id: descriptor.id,
            },
        })
    }
}

fn builtin_workflows() -> [WorkflowDescriptor; 5] {
    [
        WorkflowDescriptor {
            id: WORKFLOW_DEFAULT.to_string(),
            display_name: "Default".to_string(),
            lens_profile: LENS_ID_DEFAULT.to_string(),
            workbench_profile: WORKBENCH_PROFILE_DEFAULT.to_string(),
            canvas_profile: CANVAS_PROFILE_DEFAULT.to_string(),
            physics_profile: PHYSICS_ID_DEFAULT.to_string(),
            theme_profile: Some(THEME_ID_DEFAULT.to_string()),
            implemented: true,
        },
        WorkflowDescriptor {
            id: WORKFLOW_RESEARCH.to_string(),
            display_name: "Research".to_string(),
            lens_profile: LENS_ID_DEFAULT.to_string(),
            workbench_profile: WORKBENCH_PROFILE_COMPARE.to_string(),
            canvas_profile: CANVAS_PROFILE_DEFAULT.to_string(),
            physics_profile: PHYSICS_ID_GAS.to_string(),
            theme_profile: Some(THEME_ID_DARK.to_string()),
            implemented: true,
        },
        WorkflowDescriptor {
            id: WORKFLOW_READING.to_string(),
            display_name: "Reading".to_string(),
            lens_profile: LENS_ID_DEFAULT.to_string(),
            workbench_profile: WORKBENCH_PROFILE_FOCUS.to_string(),
            canvas_profile: CANVAS_PROFILE_DEFAULT.to_string(),
            physics_profile: PHYSICS_ID_SOLID.to_string(),
            theme_profile: Some(THEME_ID_DEFAULT.to_string()),
            implemented: true,
        },
        WorkflowDescriptor {
            id: WORKFLOW_HISTORY.to_string(),
            display_name: "History".to_string(),
            lens_profile: LENS_ID_DEFAULT.to_string(),
            workbench_profile: WORKBENCH_PROFILE_FOCUS.to_string(),
            canvas_profile: CANVAS_PROFILE_DEFAULT.to_string(),
            physics_profile: PHYSICS_ID_SOLID.to_string(),
            theme_profile: Some(THEME_ID_DARK.to_string()),
            implemented: false,
        },
        WorkflowDescriptor {
            id: WORKFLOW_PRESENCE.to_string(),
            display_name: "Presence".to_string(),
            lens_profile: LENS_ID_DEFAULT.to_string(),
            workbench_profile: WORKBENCH_PROFILE_COMPARE.to_string(),
            canvas_profile: CANVAS_PROFILE_DEFAULT.to_string(),
            physics_profile: PHYSICS_ID_GAS.to_string(),
            theme_profile: Some(THEME_ID_DARK.to_string()),
            implemented: false,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_resolves_default_and_fallback_workflows() {
        let registry = WorkflowRegistry::default();

        let default = registry.resolve_workflow(Some(WORKFLOW_DEFAULT));
        assert!(default.matched);
        assert_eq!(default.resolved_id, WORKFLOW_DEFAULT);

        let fallback = registry.resolve_workflow(Some("workflow:missing"));
        assert!(fallback.fallback_used);
        assert_eq!(fallback.resolved_id, WORKFLOW_DEFAULT);
    }

    #[test]
    fn registry_describes_stub_workflow_as_not_implemented() {
        let registry = WorkflowRegistry::default();

        let capability = registry.describe_workflow(Some(WORKFLOW_HISTORY));
        assert_eq!(capability.display_name, "History");
        assert!(!capability.implemented);
    }

    #[test]
    fn activate_updates_app_defaults_and_returns_wal_intent() {
        let mut registry = WorkflowRegistry::default();
        let mut app = GraphBrowserApp::new_for_testing();
        let resolution = registry.resolve_workflow(Some(WORKFLOW_RESEARCH));

        let activation = registry
            .activate(
                &mut app,
                resolution.descriptor.workbench_profile.clone(),
                resolution,
            )
            .expect("workflow should activate");

        assert_eq!(activation.workflow_id, WORKFLOW_RESEARCH);
        assert_eq!(app.default_registry_lens_id(), Some(LENS_ID_DEFAULT));
        assert_eq!(app.default_registry_physics_id(), Some(PHYSICS_ID_GAS));
        assert_eq!(app.default_registry_theme_id(), Some(THEME_ID_DARK));
        assert!(matches!(
            activation.wal_intent,
            GraphIntent::WorkflowActivated { ref workflow_id } if workflow_id == WORKFLOW_RESEARCH
        ));
    }

    #[test]
    fn activate_rejects_stub_workflows() {
        let mut registry = WorkflowRegistry::default();
        let mut app = GraphBrowserApp::new_for_testing();
        let resolution = registry.resolve_workflow(Some(WORKFLOW_HISTORY));

        let err = registry
            .activate(
                &mut app,
                resolution.descriptor.workbench_profile.clone(),
                resolution,
            )
            .expect_err("stub workflow should reject activation");

        assert!(matches!(
            err,
            WorkflowActivationError::NotImplemented { workflow_id } if workflow_id == WORKFLOW_HISTORY
        ));
    }
}
