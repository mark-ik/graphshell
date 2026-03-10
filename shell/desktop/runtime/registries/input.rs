use std::collections::{HashMap, hash_map::Entry};

pub(crate) const INPUT_BINDING_TOOLBAR_SUBMIT: &str = "input.toolbar.submit";
pub(crate) const ACTION_TOOLBAR_SUBMIT: &str = "action.toolbar.submit";
pub(crate) const INPUT_BINDING_TOOLBAR_NAV_BACK: &str = "input.toolbar.nav.back";
pub(crate) const INPUT_BINDING_TOOLBAR_NAV_FORWARD: &str = "input.toolbar.nav.forward";
pub(crate) const INPUT_BINDING_TOOLBAR_NAV_RELOAD: &str = "input.toolbar.nav.reload";
pub(crate) const ACTION_TOOLBAR_NAV_BACK: &str = "action.toolbar.nav.back";
pub(crate) const ACTION_TOOLBAR_NAV_FORWARD: &str = "action.toolbar.nav.forward";
pub(crate) const ACTION_TOOLBAR_NAV_RELOAD: &str = "action.toolbar.nav.reload";
pub(crate) const ACTION_GRAPH_VIEW_CONFIRM: &str = "action.graph_view.confirm";
pub(crate) const ACTION_GRAPH_CYCLE_FOCUS_REGION: &str = "action.graph.cycle_focus_region";
pub(crate) const ACTION_GRAPH_COMMAND_PALETTE_OPEN: &str = "action.graph.command_palette_open";
pub(crate) const ACTION_GRAPH_RADIAL_MENU_OPEN: &str = "action.graph.radial_menu_open";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct ModifierMask(u8);

impl ModifierMask {
    pub(crate) const NONE: Self = Self(0);
    pub(crate) const ALT: Self = Self(1 << 0);

    fn label(self) -> &'static str {
        match self.0 {
            0 => "none",
            value if value == Self::ALT.0 => "alt",
            _ => "custom",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum NamedKey {
    Enter,
    ArrowLeft,
    ArrowRight,
    F5,
}

impl NamedKey {
    fn label(self) -> &'static str {
        match self {
            Self::Enter => "enter",
            Self::ArrowLeft => "arrow_left",
            Self::ArrowRight => "arrow_right",
            Self::F5 => "f5",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Keycode {
    Named(NamedKey),
    Char(char),
}

impl Keycode {
    fn label(self) -> String {
        match self {
            Self::Named(named) => named.label().to_string(),
            Self::Char(ch) => format!("char:{}", ch.to_ascii_lowercase()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum GamepadButton {
    South,
    DPadUp,
    DPadDown,
    DPadLeft,
    DPadRight,
    LeftBumper,
    RightBumper,
    LeftStickPress,
    East,
    Start,
}

impl GamepadButton {
    fn label(self) -> &'static str {
        match self {
            Self::South => "south",
            Self::DPadUp => "dpad_up",
            Self::DPadDown => "dpad_down",
            Self::DPadLeft => "dpad_left",
            Self::DPadRight => "dpad_right",
            Self::LeftBumper => "left_bumper",
            Self::RightBumper => "right_bumper",
            Self::LeftStickPress => "left_stick_press",
            Self::East => "east",
            Self::Start => "start",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum InputBinding {
    Key {
        modifiers: ModifierMask,
        keycode: Keycode,
    },
    Gamepad {
        button: GamepadButton,
        modifier: Option<GamepadButton>,
    },
    Chord(Vec<InputBinding>),
}

impl InputBinding {
    fn label(&self) -> String {
        match self {
            Self::Key { modifiers, keycode } => {
                format!("key:{}:{}", modifiers.label(), keycode.label())
            }
            Self::Gamepad { button, modifier } => match modifier {
                Some(modifier) => {
                    format!("gamepad:{}+{}", modifier.label(), button.label())
                }
                None => format!("gamepad:{}", button.label()),
            },
            Self::Chord(sequence) => {
                let parts = sequence.iter().map(Self::label).collect::<Vec<_>>();
                format!("chord:{}", parts.join(">"))
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum InputContext {
    GraphView,
    DetailView,
    OmnibarOpen,
    RadialMenuOpen,
    DialogOpen,
}

impl InputContext {
    fn label(self) -> &'static str {
        match self {
            Self::GraphView => "graph_view",
            Self::DetailView => "detail_view",
            Self::OmnibarOpen => "omnibar_open",
            Self::RadialMenuOpen => "radial_menu_open",
            Self::DialogOpen => "dialog_open",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum BindingSlot {
    Routed(String),
    Conflict(Vec<String>),
}

fn toolbar_submit_binding() -> InputBinding {
    InputBinding::Key {
        modifiers: ModifierMask::NONE,
        keycode: Keycode::Named(NamedKey::Enter),
    }
}

fn graph_view_confirm_binding() -> InputBinding {
    InputBinding::Key {
        modifiers: ModifierMask::NONE,
        keycode: Keycode::Named(NamedKey::Enter),
    }
}

fn toolbar_nav_back_binding() -> InputBinding {
    InputBinding::Key {
        modifiers: ModifierMask::ALT,
        keycode: Keycode::Named(NamedKey::ArrowLeft),
    }
}

fn toolbar_nav_forward_binding() -> InputBinding {
    InputBinding::Key {
        modifiers: ModifierMask::ALT,
        keycode: Keycode::Named(NamedKey::ArrowRight),
    }
}

fn toolbar_nav_reload_binding() -> InputBinding {
    InputBinding::Key {
        modifiers: ModifierMask::NONE,
        keycode: Keycode::Named(NamedKey::F5),
    }
}

fn gamepad_command_palette_binding() -> InputBinding {
    InputBinding::Gamepad {
        button: GamepadButton::Start,
        modifier: None,
    }
}

fn gamepad_radial_menu_binding() -> InputBinding {
    InputBinding::Gamepad {
        button: GamepadButton::South,
        modifier: None,
    }
}

fn gamepad_cycle_focus_binding(button: GamepadButton) -> InputBinding {
    InputBinding::Gamepad {
        button,
        modifier: None,
    }
}

fn gamepad_nav_back_binding() -> InputBinding {
    InputBinding::Gamepad {
        button: GamepadButton::LeftBumper,
        modifier: None,
    }
}

fn gamepad_nav_forward_binding() -> InputBinding {
    InputBinding::Gamepad {
        button: GamepadButton::RightBumper,
        modifier: None,
    }
}

fn binding_label(binding: &InputBinding, context: InputContext) -> String {
    format!("{}@{}", binding.label(), context.label())
}

fn legacy_binding(binding_id: &str) -> Option<(InputBinding, InputContext)> {
    match binding_id.to_ascii_lowercase().as_str() {
        INPUT_BINDING_TOOLBAR_SUBMIT => Some((toolbar_submit_binding(), InputContext::OmnibarOpen)),
        INPUT_BINDING_TOOLBAR_NAV_BACK => Some((toolbar_nav_back_binding(), InputContext::DetailView)),
        INPUT_BINDING_TOOLBAR_NAV_FORWARD => {
            Some((toolbar_nav_forward_binding(), InputContext::DetailView))
        }
        INPUT_BINDING_TOOLBAR_NAV_RELOAD => {
            Some((toolbar_nav_reload_binding(), InputContext::DetailView))
        }
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InputBindingResolution {
    pub(crate) binding_label: String,
    pub(crate) context: InputContext,
    pub(crate) action_id: Option<String>,
    pub(crate) matched: bool,
    pub(crate) conflicted: bool,
}

pub(crate) struct InputRegistry {
    bindings: HashMap<(InputContext, InputBinding), BindingSlot>,
}

impl InputRegistry {
    pub(crate) fn register_binding(
        &mut self,
        binding: InputBinding,
        action_id: &str,
        context: InputContext,
    ) {
        let normalized_action_id = action_id.to_ascii_lowercase();
        match self.bindings.entry((context, binding)) {
            Entry::Vacant(entry) => {
                entry.insert(BindingSlot::Routed(normalized_action_id));
            }
            Entry::Occupied(mut entry) => match entry.get_mut() {
                BindingSlot::Routed(existing) if *existing == normalized_action_id => {}
                BindingSlot::Routed(existing) => {
                    let actions = vec![existing.clone(), normalized_action_id];
                    entry.insert(BindingSlot::Conflict(actions));
                }
                BindingSlot::Conflict(actions) => {
                    if !actions.contains(&normalized_action_id) {
                        actions.push(normalized_action_id);
                    }
                }
            },
        }
    }

    pub(crate) fn resolve(
        &self,
        binding: &InputBinding,
        context: InputContext,
    ) -> InputBindingResolution {
        let label = binding_label(binding, context);
        match self.bindings.get(&(context, binding.clone())) {
            Some(BindingSlot::Routed(action_id)) => InputBindingResolution {
                binding_label: label,
                context,
                matched: true,
                conflicted: false,
                action_id: Some(action_id.clone()),
            },
            Some(BindingSlot::Conflict(_)) => InputBindingResolution {
                binding_label: label,
                context,
                matched: false,
                conflicted: true,
                action_id: None,
            },
            None => InputBindingResolution {
                binding_label: label,
                context,
                matched: false,
                conflicted: false,
                action_id: None,
            },
        }
    }

    pub(crate) fn resolve_binding_id(&self, binding_id: &str) -> InputBindingResolution {
        if let Some((binding, context)) = legacy_binding(binding_id) {
            return self.resolve(&binding, context);
        }

        InputBindingResolution {
            binding_label: binding_id.to_ascii_lowercase(),
            context: InputContext::DialogOpen,
            action_id: None,
            matched: false,
            conflicted: false,
        }
    }
}

impl Default for InputRegistry {
    fn default() -> Self {
        let mut registry = Self {
            bindings: HashMap::new(),
        };
        registry.register_binding(
            toolbar_submit_binding(),
            ACTION_TOOLBAR_SUBMIT,
            InputContext::OmnibarOpen,
        );
        registry.register_binding(
            graph_view_confirm_binding(),
            ACTION_GRAPH_VIEW_CONFIRM,
            InputContext::GraphView,
        );
        registry.register_binding(
            toolbar_nav_back_binding(),
            ACTION_TOOLBAR_NAV_BACK,
            InputContext::DetailView,
        );
        registry.register_binding(
            toolbar_nav_forward_binding(),
            ACTION_TOOLBAR_NAV_FORWARD,
            InputContext::DetailView,
        );
        registry.register_binding(
            toolbar_nav_reload_binding(),
            ACTION_TOOLBAR_NAV_RELOAD,
            InputContext::DetailView,
        );
        registry.register_binding(
            gamepad_command_palette_binding(),
            ACTION_GRAPH_COMMAND_PALETTE_OPEN,
            InputContext::GraphView,
        );
        registry.register_binding(
            gamepad_radial_menu_binding(),
            ACTION_GRAPH_RADIAL_MENU_OPEN,
            InputContext::GraphView,
        );
        registry.register_binding(
            gamepad_cycle_focus_binding(GamepadButton::DPadUp),
            ACTION_GRAPH_CYCLE_FOCUS_REGION,
            InputContext::GraphView,
        );
        registry.register_binding(
            gamepad_cycle_focus_binding(GamepadButton::DPadDown),
            ACTION_GRAPH_CYCLE_FOCUS_REGION,
            InputContext::GraphView,
        );
        registry.register_binding(
            gamepad_cycle_focus_binding(GamepadButton::DPadLeft),
            ACTION_GRAPH_CYCLE_FOCUS_REGION,
            InputContext::GraphView,
        );
        registry.register_binding(
            gamepad_cycle_focus_binding(GamepadButton::DPadRight),
            ACTION_GRAPH_CYCLE_FOCUS_REGION,
            InputContext::GraphView,
        );
        registry.register_binding(
            gamepad_nav_back_binding(),
            ACTION_TOOLBAR_NAV_BACK,
            InputContext::DetailView,
        );
        registry.register_binding(
            gamepad_nav_forward_binding(),
            ACTION_TOOLBAR_NAV_FORWARD,
            InputContext::DetailView,
        );
        registry
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_registry_resolves_toolbar_submit_binding() {
        let registry = InputRegistry::default();
        let resolution = registry.resolve(&toolbar_submit_binding(), InputContext::OmnibarOpen);

        assert!(resolution.matched);
        assert_eq!(resolution.action_id.as_deref(), Some(ACTION_TOOLBAR_SUBMIT));
    }

    #[test]
    fn input_registry_reports_missing_binding() {
        let registry = InputRegistry::default();
        let resolution = registry.resolve_binding_id("input.unknown.binding");

        assert!(!resolution.matched);
        assert!(!resolution.conflicted);
        assert_eq!(resolution.action_id, None);
    }

    #[test]
    fn input_registry_resolves_toolbar_nav_bindings() {
        let registry = InputRegistry::default();

        let back = registry.resolve(&toolbar_nav_back_binding(), InputContext::DetailView);
        assert!(back.matched);
        assert_eq!(back.action_id.as_deref(), Some(ACTION_TOOLBAR_NAV_BACK));

        let forward = registry.resolve(&toolbar_nav_forward_binding(), InputContext::DetailView);
        assert!(forward.matched);
        assert_eq!(
            forward.action_id.as_deref(),
            Some(ACTION_TOOLBAR_NAV_FORWARD)
        );

        let reload = registry.resolve(&toolbar_nav_reload_binding(), InputContext::DetailView);
        assert!(reload.matched);
        assert_eq!(reload.action_id.as_deref(), Some(ACTION_TOOLBAR_NAV_RELOAD));
    }

    #[test]
    fn input_registry_resolves_enter_differently_by_context() {
        let registry = InputRegistry::default();

        let omnibar = registry.resolve(&toolbar_submit_binding(), InputContext::OmnibarOpen);
        assert_eq!(omnibar.action_id.as_deref(), Some(ACTION_TOOLBAR_SUBMIT));

        let graph_view = registry.resolve(&graph_view_confirm_binding(), InputContext::GraphView);
        assert_eq!(
            graph_view.action_id.as_deref(),
            Some(ACTION_GRAPH_VIEW_CONFIRM)
        );
    }

    #[test]
    fn input_registry_detects_same_binding_conflict_within_context() {
        let mut registry = InputRegistry {
            bindings: HashMap::new(),
        };

        registry.register_binding(
            toolbar_submit_binding(),
            ACTION_TOOLBAR_SUBMIT,
            InputContext::OmnibarOpen,
        );
        registry.register_binding(
            toolbar_submit_binding(),
            ACTION_GRAPH_VIEW_CONFIRM,
            InputContext::OmnibarOpen,
        );

        let resolution = registry.resolve(&toolbar_submit_binding(), InputContext::OmnibarOpen);
        assert!(!resolution.matched);
        assert!(resolution.conflicted);
        assert_eq!(resolution.action_id, None);
    }

    #[test]
    fn input_registry_legacy_binding_ids_resolve_through_typed_map() {
        let registry = InputRegistry::default();

        let resolution = registry.resolve_binding_id(INPUT_BINDING_TOOLBAR_NAV_RELOAD);
        assert!(resolution.matched);
        assert_eq!(resolution.context, InputContext::DetailView);
        assert_eq!(resolution.action_id.as_deref(), Some(ACTION_TOOLBAR_NAV_RELOAD));
    }

    #[test]
    fn input_registry_resolves_graph_view_gamepad_bindings() {
        let registry = InputRegistry::default();

        let command_palette = registry.resolve(
            &gamepad_command_palette_binding(),
            InputContext::GraphView,
        );
        assert_eq!(
            command_palette.action_id.as_deref(),
            Some(ACTION_GRAPH_COMMAND_PALETTE_OPEN)
        );

        let radial_menu = registry.resolve(&gamepad_radial_menu_binding(), InputContext::GraphView);
        assert_eq!(
            radial_menu.action_id.as_deref(),
            Some(ACTION_GRAPH_RADIAL_MENU_OPEN)
        );

        let focus_cycle = registry.resolve(
            &gamepad_cycle_focus_binding(GamepadButton::DPadLeft),
            InputContext::GraphView,
        );
        assert_eq!(
            focus_cycle.action_id.as_deref(),
            Some(ACTION_GRAPH_CYCLE_FOCUS_REGION)
        );
    }

    #[test]
    fn input_registry_resolves_detail_view_gamepad_nav_bindings() {
        let registry = InputRegistry::default();

        let back = registry.resolve(&gamepad_nav_back_binding(), InputContext::DetailView);
        assert_eq!(back.action_id.as_deref(), Some(ACTION_TOOLBAR_NAV_BACK));

        let forward = registry.resolve(&gamepad_nav_forward_binding(), InputContext::DetailView);
        assert_eq!(forward.action_id.as_deref(), Some(ACTION_TOOLBAR_NAV_FORWARD));
    }
}
