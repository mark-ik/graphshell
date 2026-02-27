use std::collections::HashMap;

pub(crate) const INPUT_BINDING_TOOLBAR_SUBMIT: &str = "input.toolbar.submit";
pub(crate) const ACTION_TOOLBAR_SUBMIT: &str = "action.toolbar.submit";
pub(crate) const INPUT_BINDING_TOOLBAR_NAV_BACK: &str = "input.toolbar.nav.back";
pub(crate) const INPUT_BINDING_TOOLBAR_NAV_FORWARD: &str = "input.toolbar.nav.forward";
pub(crate) const INPUT_BINDING_TOOLBAR_NAV_RELOAD: &str = "input.toolbar.nav.reload";
pub(crate) const ACTION_TOOLBAR_NAV_BACK: &str = "action.toolbar.nav.back";
pub(crate) const ACTION_TOOLBAR_NAV_FORWARD: &str = "action.toolbar.nav.forward";
pub(crate) const ACTION_TOOLBAR_NAV_RELOAD: &str = "action.toolbar.nav.reload";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InputBindingResolution {
    pub(crate) binding_id: String,
    pub(crate) action_id: Option<String>,
    pub(crate) matched: bool,
}

pub(crate) struct InputRegistry {
    bindings: HashMap<String, String>,
}

impl InputRegistry {
    pub(crate) fn register_binding(&mut self, binding_id: &str, action_id: &str) {
        self.bindings.insert(
            binding_id.to_ascii_lowercase(),
            action_id.to_ascii_lowercase(),
        );
    }

    pub(crate) fn resolve(&self, binding_id: &str) -> InputBindingResolution {
        let normalized = binding_id.to_ascii_lowercase();
        let action_id = self.bindings.get(&normalized).cloned();
        InputBindingResolution {
            binding_id: normalized,
            matched: action_id.is_some(),
            action_id,
        }
    }
}

impl Default for InputRegistry {
    fn default() -> Self {
        let mut registry = Self {
            bindings: HashMap::new(),
        };
        registry.register_binding(INPUT_BINDING_TOOLBAR_SUBMIT, ACTION_TOOLBAR_SUBMIT);
        registry.register_binding(INPUT_BINDING_TOOLBAR_NAV_BACK, ACTION_TOOLBAR_NAV_BACK);
        registry.register_binding(
            INPUT_BINDING_TOOLBAR_NAV_FORWARD,
            ACTION_TOOLBAR_NAV_FORWARD,
        );
        registry.register_binding(INPUT_BINDING_TOOLBAR_NAV_RELOAD, ACTION_TOOLBAR_NAV_RELOAD);
        registry
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_registry_resolves_toolbar_submit_binding() {
        let registry = InputRegistry::default();
        let resolution = registry.resolve(INPUT_BINDING_TOOLBAR_SUBMIT);

        assert!(resolution.matched);
        assert_eq!(resolution.action_id.as_deref(), Some(ACTION_TOOLBAR_SUBMIT));
    }

    #[test]
    fn input_registry_reports_missing_binding() {
        let registry = InputRegistry::default();
        let resolution = registry.resolve("input.unknown.binding");

        assert!(!resolution.matched);
        assert_eq!(resolution.action_id, None);
    }

    #[test]
    fn input_registry_resolves_toolbar_nav_bindings() {
        let registry = InputRegistry::default();

        let back = registry.resolve(INPUT_BINDING_TOOLBAR_NAV_BACK);
        assert!(back.matched);
        assert_eq!(back.action_id.as_deref(), Some(ACTION_TOOLBAR_NAV_BACK));

        let forward = registry.resolve(INPUT_BINDING_TOOLBAR_NAV_FORWARD);
        assert!(forward.matched);
        assert_eq!(
            forward.action_id.as_deref(),
            Some(ACTION_TOOLBAR_NAV_FORWARD)
        );

        let reload = registry.resolve(INPUT_BINDING_TOOLBAR_NAV_RELOAD);
        assert!(reload.matched);
        assert_eq!(reload.action_id.as_deref(), Some(ACTION_TOOLBAR_NAV_RELOAD));
    }
}
