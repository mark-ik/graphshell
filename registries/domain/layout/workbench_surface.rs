use super::SurfaceSubsystemCapabilities;
use super::profile_registry::{ProfileRegistry, ProfileResolution};

pub(crate) const WORKBENCH_SURFACE_DEFAULT: &str = "workbench_surface:default";
pub(crate) const WORKBENCH_SURFACE_FOCUS: &str = "workbench_surface:focus";
pub(crate) const WORKBENCH_SURFACE_COMPARE: &str = "workbench_surface:compare";

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum SplitDirection {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum TabStripPosition {
    Top,
    Bottom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum InitialLayout {
    Single,
    TwoPane,
    Grid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum FocusCycle {
    Tabs,
    Panes,
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum FocusTrigger {
    Click,
    KeyboardNav,
    Auto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum WorkbenchLock {
    None,
    PreventSplit,
    PreventClose,
    FullLock,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct FocusHandoffPolicy {
    pub(crate) canvas_to_pane_trigger: FocusTrigger,
    pub(crate) pane_to_canvas_trigger: FocusTrigger,
    pub(crate) focus_ring: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct WorkbenchLayoutPolicy {
    pub(crate) all_panes_must_have_tabs: bool,
    pub(crate) split_horizontal_default: bool,
    pub(crate) tab_wrapping_enabled: bool,
    pub(crate) default_split_direction: SplitDirection,
    pub(crate) min_pane_size: [f32; 2],
    pub(crate) tab_strip_visible: bool,
    pub(crate) tab_strip_position: TabStripPosition,
    pub(crate) resize_handles_visible: bool,
    pub(crate) initial_layout: InitialLayout,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct WorkbenchInteractionPolicy {
    pub(crate) tab_detach_enabled: bool,
    pub(crate) tab_detach_band_margin: f32,
    pub(crate) title_truncation_chars: usize,
    pub(crate) drag_to_split: bool,
    pub(crate) double_click_to_expand: bool,
    pub(crate) keyboard_focus_cycle: FocusCycle,
    pub(crate) close_empty_panes: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct WorkbenchSurfaceProfile {
    pub(crate) profile_id: String,
    pub(crate) display_name: String,
    pub(crate) layout: WorkbenchLayoutPolicy,
    pub(crate) interaction: WorkbenchInteractionPolicy,
    pub(crate) focus_handoff: FocusHandoffPolicy,
    pub(crate) lock: WorkbenchLock,
    pub(crate) split_horizontal_label: String,
    pub(crate) split_vertical_label: String,
    pub(crate) tab_group_label: String,
    pub(crate) grid_label: String,
    /// Folded subsystem conformance declarations for this workbench surface.
    #[serde(flatten)]
    pub(crate) subsystems: SurfaceSubsystemCapabilities,
}

pub(crate) type WorkbenchSurfaceResolution = ProfileResolution<WorkbenchSurfaceProfile>;

pub(crate) struct WorkbenchSurfaceRegistry {
    profiles: ProfileRegistry<WorkbenchSurfaceProfile>,
}

impl WorkbenchSurfaceRegistry {
    pub(crate) fn register(&mut self, profile_id: &str, profile: WorkbenchSurfaceProfile) {
        self.profiles.register(profile_id, profile);
    }

    pub(crate) fn resolve(&self, profile_id: &str) -> WorkbenchSurfaceResolution {
        self.profiles.resolve(profile_id, "workbench surface")
    }
}

impl Default for WorkbenchSurfaceRegistry {
    fn default() -> Self {
        let mut registry = Self {
            profiles: ProfileRegistry::new(WORKBENCH_SURFACE_DEFAULT),
        };
        registry.register(
            WORKBENCH_SURFACE_DEFAULT,
            WorkbenchSurfaceProfile {
                profile_id: WORKBENCH_SURFACE_DEFAULT.to_string(),
                display_name: "Default".to_string(),
                layout: WorkbenchLayoutPolicy {
                    all_panes_must_have_tabs: true,
                    split_horizontal_default: true,
                    tab_wrapping_enabled: false,
                    default_split_direction: SplitDirection::Horizontal,
                    min_pane_size: [180.0, 140.0],
                    tab_strip_visible: true,
                    tab_strip_position: TabStripPosition::Top,
                    resize_handles_visible: true,
                    initial_layout: InitialLayout::Single,
                },
                interaction: WorkbenchInteractionPolicy {
                    tab_detach_enabled: true,
                    tab_detach_band_margin: 12.0,
                    title_truncation_chars: 26,
                    drag_to_split: true,
                    double_click_to_expand: false,
                    keyboard_focus_cycle: FocusCycle::Both,
                    close_empty_panes: true,
                },
                focus_handoff: FocusHandoffPolicy {
                    canvas_to_pane_trigger: FocusTrigger::Click,
                    pane_to_canvas_trigger: FocusTrigger::KeyboardNav,
                    focus_ring: "standard".to_string(),
                },
                lock: WorkbenchLock::None,
                split_horizontal_label: "Split ↔".to_string(),
                split_vertical_label: "Split ↕".to_string(),
                tab_group_label: "Tab Group".to_string(),
                grid_label: "Grid".to_string(),
                subsystems: SurfaceSubsystemCapabilities::full(),
            },
        );
        registry.register(
            WORKBENCH_SURFACE_FOCUS,
            WorkbenchSurfaceProfile {
                profile_id: WORKBENCH_SURFACE_FOCUS.to_string(),
                display_name: "Focus".to_string(),
                layout: WorkbenchLayoutPolicy {
                    all_panes_must_have_tabs: true,
                    split_horizontal_default: true,
                    tab_wrapping_enabled: false,
                    default_split_direction: SplitDirection::Horizontal,
                    min_pane_size: [240.0, 160.0],
                    tab_strip_visible: true,
                    tab_strip_position: TabStripPosition::Top,
                    resize_handles_visible: false,
                    initial_layout: InitialLayout::Single,
                },
                interaction: WorkbenchInteractionPolicy {
                    tab_detach_enabled: false,
                    tab_detach_band_margin: 0.0,
                    title_truncation_chars: 32,
                    drag_to_split: false,
                    double_click_to_expand: true,
                    keyboard_focus_cycle: FocusCycle::Tabs,
                    close_empty_panes: true,
                },
                focus_handoff: FocusHandoffPolicy {
                    canvas_to_pane_trigger: FocusTrigger::Click,
                    pane_to_canvas_trigger: FocusTrigger::Auto,
                    focus_ring: "focus".to_string(),
                },
                lock: WorkbenchLock::PreventSplit,
                split_horizontal_label: "Focus Split ↔".to_string(),
                split_vertical_label: "Focus Split ↕".to_string(),
                tab_group_label: "Focus Tabs".to_string(),
                grid_label: "Focus Grid".to_string(),
                subsystems: SurfaceSubsystemCapabilities::full(),
            },
        );
        registry.register(
            WORKBENCH_SURFACE_COMPARE,
            WorkbenchSurfaceProfile {
                profile_id: WORKBENCH_SURFACE_COMPARE.to_string(),
                display_name: "Compare".to_string(),
                layout: WorkbenchLayoutPolicy {
                    all_panes_must_have_tabs: true,
                    split_horizontal_default: true,
                    tab_wrapping_enabled: false,
                    default_split_direction: SplitDirection::Horizontal,
                    min_pane_size: [200.0, 150.0],
                    tab_strip_visible: true,
                    tab_strip_position: TabStripPosition::Top,
                    resize_handles_visible: true,
                    initial_layout: InitialLayout::TwoPane,
                },
                interaction: WorkbenchInteractionPolicy {
                    tab_detach_enabled: true,
                    tab_detach_band_margin: 12.0,
                    title_truncation_chars: 24,
                    drag_to_split: true,
                    double_click_to_expand: false,
                    keyboard_focus_cycle: FocusCycle::Panes,
                    close_empty_panes: true,
                },
                focus_handoff: FocusHandoffPolicy {
                    canvas_to_pane_trigger: FocusTrigger::KeyboardNav,
                    pane_to_canvas_trigger: FocusTrigger::KeyboardNav,
                    focus_ring: "compare".to_string(),
                },
                lock: WorkbenchLock::None,
                split_horizontal_label: "Compare ↔".to_string(),
                split_vertical_label: "Compare ↕".to_string(),
                tab_group_label: "Compare Tabs".to_string(),
                grid_label: "Compare Grid".to_string(),
                subsystems: SurfaceSubsystemCapabilities::full(),
            },
        );
        registry
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registries::domain::layout::ConformanceLevel;

    #[test]
    fn workbench_surface_registry_resolves_default() {
        let registry = WorkbenchSurfaceRegistry::default();
        let resolution = registry.resolve(WORKBENCH_SURFACE_DEFAULT);
        assert!(resolution.matched);
        assert_eq!(resolution.profile.tab_group_label, "Tab Group");
        assert_eq!(resolution.profile.display_name, "Default");
        assert!(resolution.profile.layout.all_panes_must_have_tabs);
        assert!(resolution.profile.interaction.tab_detach_enabled);
        assert_eq!(resolution.profile.interaction.title_truncation_chars, 26);
        assert_eq!(
            resolution.profile.layout.default_split_direction,
            SplitDirection::Horizontal
        );
        assert_eq!(
            resolution.profile.interaction.keyboard_focus_cycle,
            FocusCycle::Both
        );
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
            restored.profile.subsystems.accessibility.level,
            ConformanceLevel::Full
        );
    }

    #[test]
    fn workbench_surface_registry_resolves_focus_and_compare_profiles() {
        let registry = WorkbenchSurfaceRegistry::default();

        let focus = registry.resolve(WORKBENCH_SURFACE_FOCUS);
        assert!(focus.matched);
        assert_eq!(focus.profile.display_name, "Focus");
        assert_eq!(focus.profile.lock, WorkbenchLock::PreventSplit);
        assert_eq!(focus.profile.layout.initial_layout, InitialLayout::Single);

        let compare = registry.resolve(WORKBENCH_SURFACE_COMPARE);
        assert!(compare.matched);
        assert_eq!(compare.profile.display_name, "Compare");
        assert_eq!(compare.profile.layout.initial_layout, InitialLayout::TwoPane);
        assert_eq!(
            compare.profile.interaction.keyboard_focus_cycle,
            FocusCycle::Panes
        );
    }
}
