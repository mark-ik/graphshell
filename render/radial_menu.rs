/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Radial palette — directional, `ActionRegistry`-backed.
//!
//! Content is populated via [`super::action_registry::list_radial_actions_for_category`]
//! rather than the old hardcoded `RadialCommand` / `RadialDomain` enums.
//! Action dispatch is handled by [`super::command_palette::execute_action`],
//! which is shared with the command/context palette so both surfaces use a single
//! execution path.

use crate::app::{GraphBrowserApp, SelectionUpdateMode, SurfaceHostId, UxConfigMode, ViewAction};
use crate::graph::NodeKey;
use crate::render::action_registry::{
    ActionCategory, ActionContext, ActionEntry, ActionId, InputMode, category_persisted_name,
    default_category_order, list_radial_actions_for_category, rank_categories_for_context,
};
use crate::render::command_profile::{
    load_category_recency, load_pinned_categories, record_recent_category,
};
use crate::shell::desktop::runtime::diagnostics::{DiagnosticEvent, emit_event};
use crate::shell::desktop::runtime::registries::{
    CHANNEL_UX_RADIAL_LABEL_COLLISION, CHANNEL_UX_RADIAL_LAYOUT, CHANNEL_UX_RADIAL_MODE_FALLBACK,
    CHANNEL_UX_RADIAL_OVERFLOW,
};
use crate::shell::desktop::ui::toolbar::toolbar_ui::CommandBarFocusTarget;
use crate::shell::desktop::ui::toolbar_routing;
use egui::{Color32, Key, Stroke, Window};
use std::sync::{Mutex, OnceLock};

const MAX_VISIBLE_ACTIONS_PER_RING: usize = 8;
const COMMAND_RING_RADIUS: f32 = 112.0;
const HUB_RADIUS_DEFAULT: f32 = 24.0;
const TIER1_RING_RADIUS_DEFAULT: f32 = 64.0;
const TIER2_RING_RADIUS_DEFAULT: f32 = COMMAND_RING_RADIUS;
const HUB_RADIUS_MIN: f32 = 24.0;
const HUB_RADIUS_MAX: f32 = 56.0;
const TIER1_RING_RADIUS_MIN: f32 = 56.0;
const TIER1_RING_RADIUS_MAX: f32 = 144.0;
const TIER2_RING_RADIUS_MIN: f32 = 96.0;
const TIER2_RING_RADIUS_MAX: f32 = 224.0;
const COMMAND_BUTTON_RADIUS: f32 = 18.0;
const MIN_COMMAND_CENTER_SPACING: f32 = (COMMAND_BUTTON_RADIUS * 2.0) + 4.0;
const HOVER_LABEL_MAX_CHARS: usize = 18;
const HOVER_LABEL_OFFSET: f32 = 28.0;
const RADIAL_FALLBACK_NOTICE_KEY: &str = "radial_mode_fallback_notice";
const RADIAL_GAMEPAD_INPUTS_KEY: &str = "radial_menu_gamepad_inputs";
const RADIAL_SELECTED_DOMAIN_KEY: &str = "radial_menu_selected_domain";
const RADIAL_LAYOUT_HOST_KEY: &str = "radial_menu_layout_surface_host";
const RAIL_OFFSET_STEP_RAD: f32 = 0.08;
const RING_COLLISION_EPSILON: f32 = 2.0;
const HUB_RADIUS_KEY: &str = "radial_hub_radius";
const TIER1_RING_RADIUS_KEY: &str = "radial_tier1_ring_radius";
const TIER2_RING_RADIUS_KEY: &str = "radial_tier2_ring_radius";
// Current radial UI does not enlarge button radius on hover yet.
// Keep this explicit so the pre-check gate can be tightened when hover-size growth lands.
const EFFECTIVE_HOVER_BUTTON_RADIUS: f32 = COMMAND_BUTTON_RADIUS;

fn active_theme_tokens(
    app: &GraphBrowserApp,
) -> crate::shell::desktop::runtime::registries::theme::ThemeTokenSet {
    crate::shell::desktop::runtime::registries::phase3_resolve_active_theme(
        app.default_registry_theme_id(),
    )
    .tokens
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RadialGamepadInput {
    NavigateUp,
    NavigateDown,
    NavigateLeft,
    NavigateRight,
    Confirm,
    Cancel,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RadialSectorSemanticMetadata {
    pub(crate) tier: u8,
    pub(crate) domain_label: String,
    pub(crate) action_id: String,
    pub(crate) enabled: bool,
    pub(crate) page: usize,
    pub(crate) rail_position: f32,
    pub(crate) angle_rad: f32,
    pub(crate) hover_scale: f32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct RadialPaletteSemanticSnapshot {
    pub(crate) sectors: Vec<RadialSectorSemanticMetadata>,
    pub(crate) summary: RadialPaletteSemanticSummary,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct RadialPaletteSemanticSummary {
    pub(crate) tier1_visible_count: usize,
    pub(crate) tier2_visible_count: usize,
    pub(crate) tier2_page: usize,
    pub(crate) tier2_page_count: usize,
    pub(crate) overflow_hidden_entries: usize,
    pub(crate) label_pre_collisions: usize,
    pub(crate) label_post_collisions: usize,
    pub(crate) fallback_to_palette: bool,
    pub(crate) fallback_reason: Option<String>,
}

static LATEST_RADIAL_SEMANTIC_SNAPSHOT: OnceLock<Mutex<Option<RadialPaletteSemanticSnapshot>>> =
    OnceLock::new();

fn radial_snapshot_cache() -> &'static Mutex<Option<RadialPaletteSemanticSnapshot>> {
    LATEST_RADIAL_SEMANTIC_SNAPSHOT.get_or_init(|| Mutex::new(None))
}

pub(crate) fn publish_semantic_snapshot(snapshot: RadialPaletteSemanticSnapshot) {
    if let Ok(mut slot) = radial_snapshot_cache().lock() {
        *slot = Some(snapshot);
    }
}

pub(crate) fn latest_semantic_snapshot() -> Option<RadialPaletteSemanticSnapshot> {
    radial_snapshot_cache()
        .lock()
        .ok()
        .and_then(|slot| slot.clone())
}

pub(crate) fn clear_semantic_snapshot() {
    if let Ok(mut slot) = radial_snapshot_cache().lock() {
        *slot = None;
    }
}

/// Serialization lock for tests that publish/clear the radial-palette
/// semantic snapshot process-global.
///
/// The snapshot is a `OnceLock<Mutex<Option<...>>>`; parallel tests that
/// run `publish → build/read → clear` against it race each other when
/// run under the default `cargo test --lib` parallelism. Any test that
/// walks that sequence must bind the returned guard to a local for the
/// whole dance:
///
/// ```ignore
/// let _guard = lock_radial_palette_snapshot_tests();
/// publish_semantic_snapshot(...);
/// // ... read / build_snapshot ...
/// clear_semantic_snapshot();
/// ```
///
/// Matches the existing `lock_command_surface_snapshot_tests` pattern
/// for the sibling command-surface snapshot global. The lock lives
/// in release builds too but is never acquired outside tests, so
/// there is no runtime cost in production.
pub(crate) fn lock_radial_palette_snapshot_tests()
-> std::sync::MutexGuard<'static, ()> {
    static TEST_LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    match TEST_LOCK.get_or_init(|| std::sync::Mutex::new(())).lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn radial_category_label(category: ActionCategory) -> &'static str {
    match category {
        ActionCategory::Persistence => "Persist",
        _ => category.label(),
    }
}

fn ordered_radial_categories(
    ctx: &egui::Context,
    action_context: &ActionContext,
) -> [ActionCategory; 4] {
    let categories_present: Vec<ActionCategory> = default_category_order()
        .into_iter()
        .filter(|category| !list_radial_actions_for_category(action_context, *category).is_empty())
        .collect();
    let mut ordered = rank_categories_for_context(
        &categories_present,
        action_context,
        &load_category_recency(ctx),
        &load_pinned_categories(ctx),
    );
    for category in default_category_order() {
        if !ordered.contains(&category) {
            ordered.push(category);
        }
    }
    [ordered[0], ordered[1], ordered[2], ordered[3]]
}

fn load_radial_geometry(ctx: &egui::Context) -> (f32, f32, f32) {
    let hub = ctx
        .data_mut(|d| d.get_persisted::<f32>(egui::Id::new(HUB_RADIUS_KEY)))
        .unwrap_or(HUB_RADIUS_DEFAULT)
        .clamp(HUB_RADIUS_MIN, HUB_RADIUS_MAX);
    let tier1 = ctx
        .data_mut(|d| d.get_persisted::<f32>(egui::Id::new(TIER1_RING_RADIUS_KEY)))
        .unwrap_or(TIER1_RING_RADIUS_DEFAULT)
        .clamp(TIER1_RING_RADIUS_MIN, TIER1_RING_RADIUS_MAX);
    let tier2 = ctx
        .data_mut(|d| d.get_persisted::<f32>(egui::Id::new(TIER2_RING_RADIUS_KEY)))
        .unwrap_or(TIER2_RING_RADIUS_DEFAULT)
        .clamp(TIER2_RING_RADIUS_MIN, TIER2_RING_RADIUS_MAX);
    (hub, tier1, tier2.max(tier1 + 24.0))
}

fn persist_radial_geometry(
    ctx: &egui::Context,
    hub_radius: f32,
    tier1_radius: f32,
    tier2_radius: f32,
) {
    ctx.data_mut(|d| {
        d.insert_persisted(egui::Id::new(HUB_RADIUS_KEY), hub_radius);
        d.insert_persisted(egui::Id::new(TIER1_RING_RADIUS_KEY), tier1_radius);
        d.insert_persisted(egui::Id::new(TIER2_RING_RADIUS_KEY), tier2_radius);
    });
}

pub(crate) fn queue_gamepad_input(ctx: &egui::Context, input: RadialGamepadInput) {
    let queue_id = egui::Id::new(RADIAL_GAMEPAD_INPUTS_KEY);
    ctx.data_mut(|d| {
        let mut pending = d
            .get_temp::<Vec<RadialGamepadInput>>(queue_id)
            .unwrap_or_default();
        pending.push(input);
        d.insert_temp(queue_id, pending);
    });
}

fn take_gamepad_inputs(ctx: &egui::Context) -> Vec<RadialGamepadInput> {
    ctx.data_mut(|d| {
        let queue_id = egui::Id::new(RADIAL_GAMEPAD_INPUTS_KEY);
        let pending = d
            .get_temp::<Vec<RadialGamepadInput>>(queue_id)
            .unwrap_or_default();
        d.remove::<Vec<RadialGamepadInput>>(queue_id);
        pending
    })
}

fn radial_command_selection_state_id(category: ActionCategory) -> egui::Id {
    egui::Id::new("radial_menu_selected_command").with(category_persisted_name(category))
}

fn radial_layout_surface_host_label(surface_host: &SurfaceHostId) -> &'static str {
    match surface_host {
        SurfaceHostId::Navigator(crate::app::workbench_layout_policy::NavigatorHostId::Top) => {
            "Top"
        }
        SurfaceHostId::Navigator(crate::app::workbench_layout_policy::NavigatorHostId::Bottom) => {
            "Bottom"
        }
        SurfaceHostId::Navigator(crate::app::workbench_layout_policy::NavigatorHostId::Left) => {
            "Left"
        }
        SurfaceHostId::Navigator(crate::app::workbench_layout_policy::NavigatorHostId::Right) => {
            "Right"
        }
        SurfaceHostId::Role(_) => "Workbench",
    }
}

fn cycle_layout_surface_host(
    visible_hosts: &[SurfaceHostId],
    selected_host: &Option<SurfaceHostId>,
    step: isize,
) -> Option<SurfaceHostId> {
    if visible_hosts.is_empty() {
        return None;
    }

    let current_index = selected_host
        .as_ref()
        .and_then(|surface_host| visible_hosts.iter().position(|host| host == surface_host))
        .unwrap_or(0);
    let len = visible_hosts.len() as isize;
    let next_index = (current_index as isize + step).rem_euclid(len) as usize;
    Some(visible_hosts[next_index].clone())
}

fn layout_action_requires_explicit_host(entry: Option<&ActionEntry>) -> bool {
    entry.is_some_and(|entry| {
        matches!(
            entry.id,
            ActionId::WorkbenchUnlockSurfaceLayout
                | ActionId::WorkbenchLockSurfaceLayout
                | ActionId::WorkbenchRememberLayoutPreference
        )
    })
}

fn build_radial_action_context(
    app: &GraphBrowserApp,
    source_context: Option<NodeKey>,
    pair_context: Option<(NodeKey, NodeKey)>,
    any_selected: bool,
    focused_pane_node: Option<NodeKey>,
    selected_layout_surface_host: Option<SurfaceHostId>,
) -> ActionContext {
    let frame_context = app.pending_frame_context_target().map(str::to_string);
    ActionContext {
        scope: app
            .workspace
            .chrome_ui
            .surface_state
            .scope()
            .cloned()
            .unwrap_or_default(),
        target_node: source_context,
        target_frame_member: frame_context.as_deref().and_then(|frame_name| {
            app.arrangement_projection_groups()
                .into_iter()
                .find(|group| {
                    group.sub_kind == crate::graph::ArrangementSubKind::FrameMember
                        && group.id == frame_name
                })
                .and_then(|group| group.member_keys.into_iter().next())
        }),
        target_frame_split_offer_suppressed: frame_context.as_deref().is_some_and(|frame_name| {
            let frame_url = crate::util::VersoAddress::frame(frame_name.to_string()).to_string();
            app.domain_graph()
                .get_node_by_url(&frame_url)
                .and_then(|(frame_key, _)| {
                    app.domain_graph().frame_split_offer_suppressed(frame_key)
                })
                .unwrap_or(false)
        }),
        target_frame_name: frame_context,
        pair_context,
        any_selected,
        focused_pane_available: focused_pane_node.is_some(),
        undo_available: app.undo_stack_len() > 0,
        redo_available: app.redo_stack_len() > 0,
        input_mode: InputMode::Gamepad,
        view_id: app
            .workspace
            .graph_runtime
            .focused_view
            .unwrap_or_else(crate::app::GraphViewId::new),
        wry_override_allowed: cfg!(feature = "wry")
            && app.wry_enabled()
            && crate::registries::infrastructure::mod_loader::runtime_has_capability("viewer:wry"),
        layout_surface_host_available: selected_layout_surface_host.is_some(),
        layout_surface_target_host: selected_layout_surface_host.clone(),
        layout_surface_target_ambiguous: app.has_ambiguous_navigator_surface_host_target()
            && selected_layout_surface_host.is_none(),
        layout_surface_configuring: matches!(
            app.workspace.workbench_session.ux_config_mode,
            UxConfigMode::Configuring { .. }
        ),
        layout_surface_has_draft: selected_layout_surface_host.as_ref().is_some_and(
            |surface_host| {
                app.workbench_layout_constraint_draft_for_host(surface_host)
                    .is_some()
            },
        ),
    }
}

/// Radial domain maps to `ActionCategory` for registry-backed content.
///
/// Kept as an internal UI type for angular layout calculations only.
/// Action *content* is now driven by `ActionRegistry`, not this enum.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum RadialDomain {
    Node,
    Edge,
    Graph,
    Persistence,
}

impl RadialDomain {
    const ALL: [Self; 4] = [Self::Node, Self::Edge, Self::Graph, Self::Persistence];

    fn label(self) -> &'static str {
        match self {
            Self::Node => "Node",
            Self::Edge => "Edge",
            Self::Graph => "Graph",
            Self::Persistence => "Persist",
        }
    }

    fn category(self) -> ActionCategory {
        match self {
            Self::Node => ActionCategory::Node,
            Self::Edge => ActionCategory::Edge,
            Self::Graph => ActionCategory::Graph,
            Self::Persistence => ActionCategory::Persistence,
        }
    }

    fn index(self) -> usize {
        match self {
            Self::Node => 0,
            Self::Edge => 1,
            Self::Graph => 2,
            Self::Persistence => 3,
        }
    }
}

/// Keyboard navigation groups for the node-context (right-click) mode.
///
/// Maps to `ActionCategory` for registry-backed content.
#[derive(Clone, Copy, PartialEq, Eq)]
enum NodeContextGroup {
    Frame,
    Edge,
    Node,
}

impl NodeContextGroup {
    const ALL: [Self; 3] = [Self::Frame, Self::Edge, Self::Node];

    fn label(self) -> &'static str {
        match self {
            Self::Frame => "Frame",
            Self::Edge => "Edge",
            Self::Node => "Node",
        }
    }

    /// Return registry-backed commands for this keyboard group.
    fn actions(self, context: &ActionContext) -> Vec<ActionEntry> {
        use ActionId::*;
        let all = list_radial_actions_for_category(context, self.category());
        match self {
            // Frame group: subset of Node actions focused on frame/open operations.
            Self::Frame => all
                .into_iter()
                .filter(|e| {
                    matches!(
                        e.id,
                        NodeOpenFrame
                            | NodeChooseFrame
                            | NodeAddToFrame
                            | NodeAddConnectedToFrame
                            | NodeOpenNeighbors
                            | NodeOpenConnected
                    )
                })
                .collect(),
            Self::Edge => all,
            // Node group: pin, delete, split, copy.
            Self::Node => all
                .into_iter()
                .filter(|e| {
                    matches!(
                        e.id,
                        NodeOpenSplit | NodePinToggle | NodeDelete | NodeCopyUrl | NodeCopyTitle
                    )
                })
                .collect(),
        }
    }

    fn category(self) -> ActionCategory {
        match self {
            Self::Frame | Self::Node => ActionCategory::Node,
            Self::Edge => ActionCategory::Edge,
        }
    }
}

/// Render the radial palette.
///
/// Content is driven by [`list_radial_actions_for_category`]; no hardcoded
/// `RadialCommand` enum exists in this module.
pub fn render_radial_command_menu(
    ctx: &egui::Context,
    app: &mut GraphBrowserApp,
    hovered_node: Option<NodeKey>,
    focused_pane_node: Option<NodeKey>,
    focused_pane_id: Option<crate::shell::desktop::workbench::pane_model::PaneId>,
) {
    let was_open = app.workspace.chrome_ui.show_radial_menu;
    if !was_open {
        return;
    }

    let pair_context = super::resolve_pair_command_context(app, hovered_node, focused_pane_node);
    let source_context = super::resolve_source_node_context(app, hovered_node, focused_pane_node);
    let any_selected = !app.focused_selection().is_empty();
    let mut intents = Vec::new();
    let mut should_close = false;
    let theme_tokens = active_theme_tokens(app);

    let center_id = egui::Id::new("radial_menu_center");
    let node_context_group_state_id = egui::Id::new("node_context_kbd_group");
    let node_context_command_state_id = egui::Id::new("node_context_kbd_command");
    let layout_host_state_id = egui::Id::new(RADIAL_LAYOUT_HOST_KEY);
    let pointer = ctx.input(|i| i.pointer.latest_pos());
    let center = ctx
        .data_mut(|d| d.get_persisted::<egui::Pos2>(center_id))
        .or(pointer)
        .unwrap_or(egui::pos2(320.0, 220.0));
    ctx.data_mut(|d| d.insert_persisted(center_id, center));
    let pending_gamepad_inputs = take_gamepad_inputs(ctx);
    let visible_layout_surface_hosts = app.visible_navigator_surface_hosts();
    let configuring_layout_surface_host = match &app.workspace.workbench_session.ux_config_mode {
        UxConfigMode::Configuring { surface_host } => Some(surface_host.clone()),
        UxConfigMode::Locked => None,
    };
    let mut selected_layout_surface_host =
        if let Some(surface_host) = configuring_layout_surface_host.clone() {
            Some(surface_host)
        } else {
            ctx.data_mut(|d| d.get_persisted::<String>(layout_host_state_id))
                .and_then(|raw| raw.parse::<SurfaceHostId>().ok())
                .filter(|surface_host| visible_layout_surface_hosts.contains(surface_host))
                .or_else(|| app.targetable_navigator_surface_host())
                .or_else(|| visible_layout_surface_hosts.first().cloned())
        };

    let mut action_context = build_radial_action_context(
        app,
        source_context,
        pair_context,
        any_selected,
        focused_pane_node,
        selected_layout_surface_host.clone(),
    );

    if app.pending_node_context_target().is_some() {
        let mut group_idx = ctx
            .data_mut(|d| d.get_persisted::<usize>(node_context_group_state_id))
            .unwrap_or(0);
        let mut command_idx = ctx
            .data_mut(|d| d.get_persisted::<usize>(node_context_command_state_id))
            .unwrap_or(0);
        group_idx %= NodeContextGroup::ALL.len();

        let mut group_changed = false;
        if ctx.input(|i| i.key_pressed(Key::ArrowLeft)) {
            group_idx = (group_idx + NodeContextGroup::ALL.len() - 1) % NodeContextGroup::ALL.len();
            group_changed = true;
        }
        if ctx.input(|i| i.key_pressed(Key::ArrowRight)) {
            group_idx = (group_idx + 1) % NodeContextGroup::ALL.len();
            group_changed = true;
        }

        let keyboard_group = NodeContextGroup::ALL[group_idx];
        let keyboard_commands = keyboard_group.actions(&action_context);
        let close_idx = keyboard_commands.len();
        if group_changed {
            command_idx = 0;
        }
        if command_idx >= close_idx {
            command_idx = 0;
        }
        let keyboard_slot_count = keyboard_commands.len();
        if keyboard_slot_count > 0 && ctx.input(|i| i.key_pressed(Key::ArrowUp)) {
            command_idx = (command_idx + keyboard_slot_count - 1) % keyboard_slot_count;
        }
        if keyboard_slot_count > 0 && ctx.input(|i| i.key_pressed(Key::ArrowDown)) {
            command_idx = (command_idx + 1) % keyboard_slot_count;
        }
        for input in pending_gamepad_inputs.iter().copied() {
            match input {
                RadialGamepadInput::NavigateLeft => {
                    group_idx =
                        (group_idx + NodeContextGroup::ALL.len() - 1) % NodeContextGroup::ALL.len();
                    command_idx = 0;
                }
                RadialGamepadInput::NavigateRight => {
                    group_idx = (group_idx + 1) % NodeContextGroup::ALL.len();
                    command_idx = 0;
                }
                RadialGamepadInput::NavigateUp => {
                    let keyboard_commands =
                        NodeContextGroup::ALL[group_idx].actions(&action_context);
                    if !keyboard_commands.is_empty() {
                        command_idx =
                            (command_idx + keyboard_commands.len() - 1) % keyboard_commands.len();
                    }
                }
                RadialGamepadInput::NavigateDown => {
                    let keyboard_commands =
                        NodeContextGroup::ALL[group_idx].actions(&action_context);
                    if !keyboard_commands.is_empty() {
                        command_idx = (command_idx + 1) % keyboard_commands.len();
                    }
                }
                RadialGamepadInput::Confirm => {
                    let keyboard_commands =
                        NodeContextGroup::ALL[group_idx].actions(&action_context);
                    if let Some(entry) = keyboard_commands.get(command_idx)
                        && entry.enabled
                    {
                        record_recent_category(ctx, entry.id.category());
                        super::command_palette::execute_action_with_layout_target(
                            app,
                            entry.id,
                            selected_layout_surface_host.clone(),
                            pair_context,
                            source_context,
                            &mut intents,
                            focused_pane_node,
                            focused_pane_id,
                        );
                        should_close = true;
                    }
                }
                RadialGamepadInput::Cancel => {
                    should_close = true;
                }
            }
            if should_close {
                break;
            }
        }
        if ctx.input(|i| i.key_pressed(Key::Enter)) {
            if let Some(entry) = keyboard_commands.get(command_idx)
                && entry.enabled
            {
                record_recent_category(ctx, entry.id.category());
                super::command_palette::execute_action_with_layout_target(
                    app,
                    entry.id,
                    selected_layout_surface_host.clone(),
                    pair_context,
                    source_context,
                    &mut intents,
                    focused_pane_node,
                    focused_pane_id,
                );
                should_close = true;
            }
        }
        ctx.data_mut(|d| {
            d.insert_persisted(node_context_group_state_id, group_idx);
            d.insert_persisted(node_context_command_state_id, command_idx);
        });

        let window_response = Window::new("Context Palette")
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .fixed_pos(center + egui::vec2(12.0, 12.0))
            .default_width(260.0)
            .show(ctx, |ui| {
                if visible_layout_surface_hosts.len() > 1 {
                    ui.horizontal(|ui| {
                        ui.label("Host:");
                        if ui.small_button("<").clicked() {
                            selected_layout_surface_host = cycle_layout_surface_host(
                                &visible_layout_surface_hosts,
                                &selected_layout_surface_host,
                                -1,
                            );
                            action_context = build_radial_action_context(
                                app,
                                source_context,
                                pair_context,
                                any_selected,
                                focused_pane_node,
                                selected_layout_surface_host.clone(),
                            );
                        }
                        ui.label(
                            selected_layout_surface_host
                                .as_ref()
                                .map(radial_layout_surface_host_label)
                                .unwrap_or("Select Host"),
                        );
                        if ui.small_button(">").clicked() {
                            selected_layout_surface_host = cycle_layout_surface_host(
                                &visible_layout_surface_hosts,
                                &selected_layout_surface_host,
                                1,
                            );
                            action_context = build_radial_action_context(
                                app,
                                source_context,
                                pair_context,
                                any_selected,
                                focused_pane_node,
                                selected_layout_surface_host.clone(),
                            );
                        }
                    });
                    ui.small("Layout actions apply to the selected Navigator host.");
                    ui.separator();
                }
                ui.horizontal(|ui| {
                    for (idx, group) in NodeContextGroup::ALL.iter().enumerate() {
                        let heading = if idx == group_idx {
                            format!("[{}]", group.label())
                        } else {
                            group.label().to_string()
                        };
                        ui.menu_button(heading, |ui| {
                            for entry in group.actions(&action_context) {
                                if ui
                                    .add_enabled(entry.enabled, egui::Button::new(entry.id.label()))
                                    .clicked()
                                {
                                    record_recent_category(ctx, entry.id.category());
                                    super::command_palette::execute_action_with_layout_target(
                                        app,
                                        entry.id,
                                        selected_layout_surface_host.clone(),
                                        pair_context,
                                        source_context,
                                        &mut intents,
                                        focused_pane_node,
                                        focused_pane_id,
                                    );
                                    should_close = true;
                                    ui.close();
                                }
                            }
                        });
                    }
                });
                ui.separator();
                ui.small("Keyboard: <- -> groups, Up/Down actions, Enter run");
                ui.small("Esc or click outside to close");
                let keyboard_group = NodeContextGroup::ALL[group_idx];
                let keyboard_commands = keyboard_group.actions(&action_context);
                if let Some(current) = keyboard_commands.get(command_idx) {
                    ui.small(format!(
                        "Focus: {} / {}",
                        keyboard_group.label(),
                        current.id.label()
                    ));
                }
                for (idx, entry) in keyboard_commands.iter().enumerate() {
                    let label = if idx == command_idx {
                        format!("> {}", entry.id.label())
                    } else {
                        entry.id.label().to_string()
                    };
                    if ui
                        .add_enabled(entry.enabled, egui::Button::new(label))
                        .clicked()
                    {
                        command_idx = idx;
                        record_recent_category(ctx, entry.id.category());
                        super::command_palette::execute_action_with_layout_target(
                            app,
                            entry.id,
                            selected_layout_surface_host.clone(),
                            pair_context,
                            source_context,
                            &mut intents,
                            focused_pane_node,
                            focused_pane_id,
                        );
                        should_close = true;
                    }
                }
                ctx.data_mut(|d| d.insert_persisted(node_context_command_state_id, command_idx));
            });
        if let Some(response) = window_response {
            let clicked_outside = ctx.input(|i| {
                i.pointer.primary_clicked()
                    && i.pointer
                        .latest_pos()
                        .is_some_and(|pos| !response.response.rect.contains(pos))
            });
            if clicked_outside {
                intents.push(
                    ViewAction::UpdateSelection {
                        keys: Vec::new(),
                        mode: SelectionUpdateMode::Replace,
                    }
                    .into(),
                );
                should_close = true;
            }
        }
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            should_close = true;
        }
    } else {
        // Circular radial mode: hover by angle, click to confirm.
        let ordered_categories = ordered_radial_categories(ctx, &action_context);
        let mut domain_offsets = [0.0f32; 4];
        let mut command_offsets = [0.0f32; 4];
        let mut semantic_snapshot = RadialPaletteSemanticSnapshot::default();
        let (mut hub_radius, mut tier1_ring_radius, mut tier2_ring_radius) =
            load_radial_geometry(ctx);
        for domain in RadialDomain::ALL {
            let category = ordered_categories[domain.index()];
            domain_offsets[domain.index()] = ctx
                .data_mut(|d| d.get_persisted::<f32>(domain_offset_id(category)))
                .unwrap_or(0.0);
            command_offsets[domain.index()] = ctx
                .data_mut(|d| d.get_persisted::<f32>(command_offset_id(category)))
                .unwrap_or(0.0);
        }

        let mut hovered_domain = None;
        let mut hovered_entry: Option<ActionEntry> = None;
        let mut clicked_entry: Option<ActionEntry> = None;
        let mut fallback_to_command_palette = false;
        let mut fallback_reason: Option<&'static str> = None;
        let selected_domain_state_id = egui::Id::new(RADIAL_SELECTED_DOMAIN_KEY);
        let mut selected_domain_idx = ctx
            .data_mut(|d| d.get_persisted::<usize>(selected_domain_state_id))
            .unwrap_or(0)
            % RadialDomain::ALL.len();
        if let Some(pos) = pointer {
            let delta = pos - center;
            let r = delta.length();
            if r > 40.0 {
                let angle = delta.y.atan2(delta.x);
                hovered_domain = Some(domain_from_angle_with_offsets(angle, &domain_offsets));
                if r > 120.0
                    && let Some(domain) = hovered_domain
                {
                    let category = ordered_categories[domain.index()];
                    let cmds = list_radial_actions_for_category(&action_context, category);
                    let page_state_id =
                        egui::Id::new("radial_menu_page").with(category_persisted_name(category));
                    let page_count = ring_page_count(cmds.len(), MAX_VISIBLE_ACTIONS_PER_RING);
                    let mut page = ctx
                        .data_mut(|d| d.get_persisted::<usize>(page_state_id))
                        .unwrap_or(0);
                    if page_count > 0 {
                        page %= page_count;
                    } else {
                        page = 0;
                    }
                    let visible_cmds =
                        paged_ring_entries(&cmds, page, MAX_VISIBLE_ACTIONS_PER_RING);
                    if cmds.len() > visible_cmds.len() {
                        emit_event(DiagnosticEvent::MessageSent {
                            channel_id: CHANNEL_UX_RADIAL_OVERFLOW,
                            byte_len: cmds.len() - visible_cmds.len(),
                        });
                    }
                    let hover_layout_ok = ring_layout_supports_hover_non_overlap_with_ring_radius(
                        center,
                        domain,
                        visible_cmds.len(),
                        domain_offsets[domain.index()],
                        command_offsets[domain.index()],
                        tier2_ring_radius,
                    );
                    if !hover_layout_ok {
                        if !fallback_to_command_palette {
                            emit_event(DiagnosticEvent::MessageReceived {
                                channel_id: CHANNEL_UX_RADIAL_MODE_FALLBACK,
                                latency_us: visible_cmds.len() as u64,
                            });
                        }
                        fallback_to_command_palette = true;
                    } else {
                        hovered_entry = nearest_entry_for_pointer_with_radius(
                            domain,
                            center,
                            pos,
                            visible_cmds,
                            domain_offsets[domain.index()],
                            command_offsets[domain.index()],
                            tier2_ring_radius,
                        );
                    }

                    emit_event(DiagnosticEvent::MessageSent {
                        channel_id: CHANNEL_UX_RADIAL_LAYOUT,
                        byte_len: visible_cmds.len() + page + (page_count * 10),
                    });
                }
            }
        }
        if let Some(domain) = hovered_domain {
            selected_domain_idx = domain.index();
        }

        for input in pending_gamepad_inputs.iter().copied() {
            match input {
                RadialGamepadInput::NavigateLeft => {
                    let current_category = ordered_categories[selected_domain_idx];
                    let current_command_state_id =
                        radial_command_selection_state_id(current_category);
                    let current_commands =
                        list_radial_actions_for_category(&action_context, current_category);
                    let current_command_idx = ctx
                        .data_mut(|d| d.get_persisted::<usize>(current_command_state_id))
                        .unwrap_or(0);
                    let current_entry = if current_commands.is_empty() {
                        None
                    } else {
                        current_commands.get(current_command_idx % current_commands.len())
                    };
                    if visible_layout_surface_hosts.len() > 1
                        && layout_action_requires_explicit_host(current_entry)
                    {
                        selected_layout_surface_host = cycle_layout_surface_host(
                            &visible_layout_surface_hosts,
                            &selected_layout_surface_host,
                            -1,
                        );
                        action_context = build_radial_action_context(
                            app,
                            source_context,
                            pair_context,
                            any_selected,
                            focused_pane_node,
                            selected_layout_surface_host.clone(),
                        );
                    } else {
                        selected_domain_idx = (selected_domain_idx + RadialDomain::ALL.len() - 1)
                            % RadialDomain::ALL.len();
                    }
                }
                RadialGamepadInput::NavigateRight => {
                    let current_category = ordered_categories[selected_domain_idx];
                    let current_command_state_id =
                        radial_command_selection_state_id(current_category);
                    let current_commands =
                        list_radial_actions_for_category(&action_context, current_category);
                    let current_command_idx = ctx
                        .data_mut(|d| d.get_persisted::<usize>(current_command_state_id))
                        .unwrap_or(0);
                    let current_entry = if current_commands.is_empty() {
                        None
                    } else {
                        current_commands.get(current_command_idx % current_commands.len())
                    };
                    if visible_layout_surface_hosts.len() > 1
                        && layout_action_requires_explicit_host(current_entry)
                    {
                        selected_layout_surface_host = cycle_layout_surface_host(
                            &visible_layout_surface_hosts,
                            &selected_layout_surface_host,
                            1,
                        );
                        action_context = build_radial_action_context(
                            app,
                            source_context,
                            pair_context,
                            any_selected,
                            focused_pane_node,
                            selected_layout_surface_host.clone(),
                        );
                    } else {
                        selected_domain_idx = (selected_domain_idx + 1) % RadialDomain::ALL.len();
                    }
                }
                RadialGamepadInput::NavigateUp
                | RadialGamepadInput::NavigateDown
                | RadialGamepadInput::Confirm => {
                    let category = ordered_categories[selected_domain_idx];
                    let command_state_id = radial_command_selection_state_id(category);
                    let commands = list_radial_actions_for_category(&action_context, category);
                    let mut selected_command_idx = ctx
                        .data_mut(|d| d.get_persisted::<usize>(command_state_id))
                        .unwrap_or(0);
                    if !commands.is_empty() {
                        selected_command_idx %= commands.len();
                        match input {
                            RadialGamepadInput::NavigateUp => {
                                selected_command_idx =
                                    (selected_command_idx + commands.len() - 1) % commands.len();
                            }
                            RadialGamepadInput::NavigateDown => {
                                selected_command_idx = (selected_command_idx + 1) % commands.len();
                            }
                            RadialGamepadInput::Confirm => {
                                if let Some(entry) = commands.get(selected_command_idx).cloned()
                                    && entry.enabled
                                {
                                    clicked_entry = Some(entry);
                                    should_close = true;
                                }
                            }
                            _ => {}
                        }
                    } else {
                        selected_command_idx = 0;
                    }
                    ctx.data_mut(|d| d.insert_persisted(command_state_id, selected_command_idx));
                }
                RadialGamepadInput::Cancel => {
                    should_close = true;
                }
            }
            if should_close {
                break;
            }
        }
        ctx.data_mut(|d| d.insert_persisted(selected_domain_state_id, selected_domain_idx));

        let selected_domain = RadialDomain::ALL[selected_domain_idx];
        let selected_category = ordered_categories[selected_domain.index()];
        let selected_command_state_id = radial_command_selection_state_id(selected_category);
        let selected_commands =
            list_radial_actions_for_category(&action_context, selected_category);
        let mut selected_command_idx = ctx
            .data_mut(|d| d.get_persisted::<usize>(selected_command_state_id))
            .unwrap_or(0);
        if !selected_commands.is_empty() {
            selected_command_idx %= selected_commands.len();
        } else {
            selected_command_idx = 0;
        }
        ctx.data_mut(|d| d.insert_persisted(selected_command_state_id, selected_command_idx));
        let selected_page = if selected_commands.is_empty() {
            0
        } else {
            selected_command_idx / MAX_VISIBLE_ACTIONS_PER_RING
        };
        ctx.data_mut(|d| {
            d.insert_persisted(
                egui::Id::new("radial_menu_page").with(category_persisted_name(selected_category)),
                selected_page,
            )
        });
        let selected_visible_commands = paged_ring_entries(
            &selected_commands,
            selected_page,
            MAX_VISIBLE_ACTIONS_PER_RING,
        );
        let selected_visible_idx = if selected_visible_commands.is_empty() {
            0
        } else {
            selected_command_idx % MAX_VISIBLE_ACTIONS_PER_RING
        };
        if !pending_gamepad_inputs.is_empty() || hovered_domain.is_none() {
            hovered_domain = Some(selected_domain);
            hovered_entry = selected_visible_commands.get(selected_visible_idx).cloned();
        }

        if let Some(domain) = hovered_domain {
            if ctx.input(|i| i.key_pressed(Key::ArrowLeft)) {
                let idx = domain.index();
                if ctx.input(|i| i.modifiers.shift) {
                    command_offsets[idx] -= RAIL_OFFSET_STEP_RAD;
                } else {
                    domain_offsets[idx] -= RAIL_OFFSET_STEP_RAD;
                }
            }
            if ctx.input(|i| i.key_pressed(Key::ArrowRight)) {
                let idx = domain.index();
                if ctx.input(|i| i.modifiers.shift) {
                    command_offsets[idx] += RAIL_OFFSET_STEP_RAD;
                } else {
                    domain_offsets[idx] += RAIL_OFFSET_STEP_RAD;
                }
            }
            if ctx.input(|i| i.modifiers.alt && i.key_pressed(Key::ArrowUp)) {
                if ctx.input(|i| i.modifiers.ctrl) {
                    hub_radius = (hub_radius + 2.0).clamp(HUB_RADIUS_MIN, HUB_RADIUS_MAX);
                } else if ctx.input(|i| i.modifiers.shift) {
                    tier2_ring_radius = (tier2_ring_radius + 4.0)
                        .clamp(TIER2_RING_RADIUS_MIN, TIER2_RING_RADIUS_MAX);
                } else {
                    tier1_ring_radius = (tier1_ring_radius + 4.0)
                        .clamp(TIER1_RING_RADIUS_MIN, TIER1_RING_RADIUS_MAX);
                }
            }
            if ctx.input(|i| i.modifiers.alt && i.key_pressed(Key::ArrowDown)) {
                if ctx.input(|i| i.modifiers.ctrl) {
                    hub_radius = (hub_radius - 2.0).clamp(HUB_RADIUS_MIN, HUB_RADIUS_MAX);
                } else if ctx.input(|i| i.modifiers.shift) {
                    tier2_ring_radius = (tier2_ring_radius - 4.0)
                        .clamp(TIER2_RING_RADIUS_MIN, TIER2_RING_RADIUS_MAX);
                } else {
                    tier1_ring_radius = (tier1_ring_radius - 4.0)
                        .clamp(TIER1_RING_RADIUS_MIN, TIER1_RING_RADIUS_MAX);
                }
            }
        }
        tier2_ring_radius = tier2_ring_radius.max(tier1_ring_radius + 24.0);

        for domain in RadialDomain::ALL {
            let category = ordered_categories[domain.index()];
            ctx.data_mut(|d| {
                d.insert_persisted(domain_offset_id(category), domain_offsets[domain.index()]);
                d.insert_persisted(command_offset_id(category), command_offsets[domain.index()]);
            });
        }
        persist_radial_geometry(ctx, hub_radius, tier1_ring_radius, tier2_ring_radius);

        if ctx.input(|i| i.pointer.button_released(egui::PointerButton::Primary)) {
            clicked_entry = hovered_entry.clone();
            should_close = true;
        }
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            should_close = true;
        }

        let radial_surface_extent =
            tier2_ring_radius + HOVER_LABEL_OFFSET + COMMAND_BUTTON_RADIUS + 36.0;
        egui::Area::new("radial_command_menu".into())
            .fixed_pos(center - egui::vec2(radial_surface_extent, radial_surface_extent))
            .interactable(false)
            .show(ctx, |ui| {
                ui.set_min_size(egui::vec2(
                    radial_surface_extent * 2.0,
                    radial_surface_extent * 2.0,
                ));
                let painter = ui.painter();
                painter.circle_filled(center, hub_radius, theme_tokens.radial_hub_fill);
                painter.circle_stroke(
                    center,
                    hub_radius,
                    Stroke::new(2.0, theme_tokens.radial_hub_stroke),
                );
                let hub_label = hovered_domain
                    .map(|d| radial_category_label(ordered_categories[d.index()]).to_string())
                    .unwrap_or_else(|| "Cmd".to_string());
                painter.text(
                    center,
                    egui::Align2::CENTER_CENTER,
                    hub_label,
                    egui::FontId::proportional(14.0),
                    theme_tokens.radial_hub_text,
                );

                for domain in RadialDomain::ALL {
                    let category = ordered_categories[domain.index()];
                    let base = domain_anchor_with_offsets(
                        center,
                        domain,
                        tier1_ring_radius,
                        domain_offsets[domain.index()],
                    );
                    semantic_snapshot.sectors.push(RadialSectorSemanticMetadata {
                        tier: 1,
                        domain_label: radial_category_label(category).to_string(),
                        action_id: format!("category:{}", category_persisted_name(category)),
                        enabled: true,
                        page: 0,
                        rail_position: domain_offsets[domain.index()],
                        angle_rad: domain_angle_with_offsets(
                            domain,
                            domain_offsets[domain.index()],
                        ),
                        hover_scale: if Some(domain) == hovered_domain { 1.5 } else { 1.0 },
                    });
                    semantic_snapshot.summary.tier1_visible_count += 1;
                    let color = if Some(domain) == hovered_domain {
                        theme_tokens.radial_domain_active_fill
                    } else {
                        theme_tokens.radial_domain_idle_fill
                    };
                    painter.circle_filled(base, 22.0, color);
                    painter.text(
                        base,
                        egui::Align2::CENTER_CENTER,
                        radial_category_label(category),
                        egui::FontId::proportional(12.0),
                        Color32::WHITE,
                    );
                }

                if let Some(domain) = hovered_domain {
                    let category = ordered_categories[domain.index()];
                    let cmds = list_radial_actions_for_category(&action_context, category);
                    let page_state_id = egui::Id::new("radial_menu_page")
                        .with(category_persisted_name(category));
                    let page_count = ring_page_count(cmds.len(), MAX_VISIBLE_ACTIONS_PER_RING);
                    let mut page = ctx
                        .data_mut(|d| d.get_persisted::<usize>(page_state_id))
                        .unwrap_or(0);
                    if page_count > 0 {
                        page %= page_count;
                    } else {
                        page = 0;
                    }

                    if page_count > 1 {
                        if ctx.input(|i| i.key_pressed(Key::PageDown)) {
                            page = (page + 1) % page_count;
                        }
                        if ctx.input(|i| i.key_pressed(Key::PageUp)) {
                            page = (page + page_count - 1) % page_count;
                        }
                    }

                    let visible_cmds =
                        paged_ring_entries(&cmds, page, MAX_VISIBLE_ACTIONS_PER_RING);
                    semantic_snapshot.summary.tier2_visible_count = visible_cmds.len();
                    semantic_snapshot.summary.tier2_page = page;
                    semantic_snapshot.summary.tier2_page_count = page_count;
                    if cmds.len() > visible_cmds.len() {
                        semantic_snapshot.summary.overflow_hidden_entries =
                            cmds.len() - visible_cmds.len();
                        emit_event(DiagnosticEvent::MessageSent {
                            channel_id: CHANNEL_UX_RADIAL_OVERFLOW,
                            byte_len: cmds.len() - visible_cmds.len(),
                        });
                    }
                    ctx.data_mut(|d| d.insert_persisted(page_state_id, page));

                    let hover_layout_ok = ring_layout_supports_hover_non_overlap_with_ring_radius(
                        center,
                        domain,
                        visible_cmds.len(),
                        domain_offsets[domain.index()],
                        command_offsets[domain.index()],
                        tier2_ring_radius,
                    );
                    if !hover_layout_ok {
                        if !fallback_to_command_palette {
                            emit_event(DiagnosticEvent::MessageReceived {
                                channel_id: CHANNEL_UX_RADIAL_MODE_FALLBACK,
                                latency_us: visible_cmds.len() as u64,
                            });
                        }
                        fallback_to_command_palette = true;
                        fallback_reason.get_or_insert("hover_non_overlap_precheck_failed");
                    }

                    let label_layout = compute_label_layout_metrics_with_radius(
                        center,
                        domain,
                        visible_cmds,
                        tier2_ring_radius,
                    );
                    let packed_collisions = (label_layout.pre_collisions << 16)
                        .saturating_add(label_layout.post_collisions);
                    semantic_snapshot.summary.label_pre_collisions = label_layout.pre_collisions;
                    semantic_snapshot.summary.label_post_collisions = label_layout.post_collisions;
                    emit_event(DiagnosticEvent::MessageSent {
                        channel_id: CHANNEL_UX_RADIAL_LABEL_COLLISION,
                        byte_len: packed_collisions,
                    });
                    if label_layout.post_collisions > 0 {
                        fallback_to_command_palette = true;
                        fallback_reason.get_or_insert("label_collision_resolver_failed");
                        emit_event(DiagnosticEvent::MessageReceived {
                            channel_id: CHANNEL_UX_RADIAL_MODE_FALLBACK,
                            latency_us: label_layout.post_collisions as u64,
                        });
                    }

                    for (idx, entry) in visible_cmds.iter().enumerate() {
                        let angle = command_angle_with_offsets_at_radius(
                            domain,
                            idx,
                            visible_cmds.len(),
                            domain_offsets[domain.index()],
                            command_offsets[domain.index()],
                            tier2_ring_radius,
                        );
                        let is_hovered = hovered_entry.as_ref().is_some_and(|h| h.id == entry.id);
                        semantic_snapshot.sectors.push(RadialSectorSemanticMetadata {
                            tier: 2,
                            domain_label: radial_category_label(category).to_string(),
                            action_id: entry.id.key().to_string(),
                            enabled: entry.enabled,
                            page,
                            rail_position: command_offsets[domain.index()],
                            angle_rad: angle,
                            hover_scale: if is_hovered { 1.5 } else { 1.0 },
                        });
                        let anchor = command_anchor_with_offsets_at_radius(
                            center,
                            domain,
                            idx,
                            visible_cmds.len(),
                            domain_offsets[domain.index()],
                            command_offsets[domain.index()],
                            tier2_ring_radius,
                        );
                        let color = if is_hovered {
                            theme_tokens.radial_command_active_fill
                        } else if entry.enabled {
                            theme_tokens.radial_command_hover_fill
                        } else {
                            theme_tokens.radial_command_disabled_fill
                        };
                        painter.circle_filled(anchor, COMMAND_BUTTON_RADIUS, color);
                        painter.text(
                            anchor,
                            egui::Align2::CENTER_CENTER,
                            entry.id.short_label(),
                            egui::FontId::proportional(10.0),
                            if entry.enabled {
                                theme_tokens.radial_command_text
                            } else {
                                theme_tokens.radial_disabled_text
                            },
                        );

                        if is_hovered {
                            let label_text =
                                bounded_hover_label(entry.id.label(), HOVER_LABEL_MAX_CHARS);
                            let label_pos = radial_label_anchor(anchor, center, HOVER_LABEL_OFFSET);
                            draw_radial_hover_label(painter, label_pos, &label_text, &theme_tokens);
                        }
                    }
                }

                if let Some(domain) = hovered_domain {
                    let category = ordered_categories[domain.index()];
                    let page_state_id = egui::Id::new("radial_menu_page")
                        .with(category_persisted_name(category));
                    let page = ctx
                        .data_mut(|d| d.get_persisted::<usize>(page_state_id))
                        .unwrap_or(0);
                    let cmds = list_radial_actions_for_category(&action_context, category);
                    let page_count = ring_page_count(cmds.len(), MAX_VISIBLE_ACTIONS_PER_RING);
                    if page_count > 1 {
                        painter.text(
                            center + egui::vec2(0.0, 52.0),
                            egui::Align2::CENTER_CENTER,
                            format!("Page {}/{}", page + 1, page_count),
                            egui::FontId::proportional(11.0),
                            theme_tokens.radial_chrome_text,
                        );
                    }
                    painter.text(
                        center + egui::vec2(0.0, 94.0),
                        egui::Align2::CENTER_CENTER,
                        "Arrow Left/Right: Tier1 rail | Shift+Arrow: Tier2 rail | Alt+Up/Down radius",
                        egui::FontId::proportional(10.0),
                        theme_tokens.radial_chrome_text,
                    );
                    if visible_layout_surface_hosts.len() > 1 {
                        painter.text(
                            center + egui::vec2(0.0, 68.0),
                            egui::Align2::CENTER_CENTER,
                            format!(
                                "Host: {}",
                                selected_layout_surface_host
                                    .as_ref()
                                    .map(radial_layout_surface_host_label)
                                    .unwrap_or("Select Host")
                            ),
                            egui::FontId::proportional(11.0),
                            theme_tokens.radial_chrome_text,
                        );
                    }
                }

                semantic_snapshot.summary.fallback_to_palette = fallback_to_command_palette;
                semantic_snapshot.summary.fallback_reason = fallback_reason.map(str::to_string);

                if fallback_to_command_palette {
                    painter.text(
                        center + egui::vec2(0.0, 76.0),
                        egui::Align2::CENTER_CENTER,
                        "Radial palette constrained. Switching to context palette.",
                        egui::FontId::proportional(11.0),
                        theme_tokens.radial_warning_text,
                    );
                }
            });

        if fallback_to_command_palette {
            clear_semantic_snapshot();
        } else {
            publish_semantic_snapshot(semantic_snapshot);
        }

        if fallback_to_command_palette {
            app.set_pending_node_context_target(source_context);
            if app.pending_command_surface_return_target().is_none() {
                app.set_pending_command_surface_return_target(Some(
                    crate::app::ToolSurfaceReturnTarget::Graph(action_context.view_id),
                ));
            }
            app.open_context_palette();
            ctx.data_mut(|d| d.insert_persisted(egui::Id::new(RADIAL_FALLBACK_NOTICE_KEY), true));
            should_close = true;
        }

        if !fallback_to_command_palette && let Some(entry) = clicked_entry {
            if entry.enabled {
                record_recent_category(ctx, entry.id.category());
                super::command_palette::execute_action_with_layout_target(
                    app,
                    entry.id,
                    selected_layout_surface_host.clone(),
                    pair_context,
                    source_context,
                    &mut intents,
                    focused_pane_node,
                    focused_pane_id,
                );
            }
        }
    }

    ctx.data_mut(|d| {
        if let Some(surface_host) = &selected_layout_surface_host {
            d.insert_persisted(layout_host_state_id, surface_host.to_string());
        }
    });

    if should_close {
        let _ = toolbar_routing::request_radial_menu_close(
            app,
            CommandBarFocusTarget::new(focused_pane_id, source_context.or(focused_pane_node)),
        );
    } else {
        app.workspace.chrome_ui.show_radial_menu = true;
    }
    if !app.workspace.chrome_ui.show_radial_menu {
        clear_semantic_snapshot();
        app.set_pending_node_context_target(None);
        ctx.data_mut(|d| {
            d.remove::<egui::Pos2>(center_id);
            d.remove::<usize>(node_context_group_state_id);
            d.remove::<usize>(node_context_command_state_id);
            d.remove::<String>(layout_host_state_id);
            d.remove::<usize>(egui::Id::new(RADIAL_SELECTED_DOMAIN_KEY));
            for category in default_category_order() {
                d.remove::<usize>(radial_command_selection_state_id(category));
            }
        });
    } else if app.pending_node_context_target().is_some() {
        // Context-palette mode has no radial sector geometry to publish.
        clear_semantic_snapshot();
    }
    super::apply_ui_intents_with_checkpoint(app, intents);
}

// --- Radial layout helpers ---------------------------------------------------

fn domain_from_angle(angle: f32) -> RadialDomain {
    let mut best = RadialDomain::Node;
    let mut best_dist = f32::MAX;
    for domain in RadialDomain::ALL {
        let target = domain_angle(domain);
        let mut d = (angle - target).abs();
        if d > std::f32::consts::PI {
            d = 2.0 * std::f32::consts::PI - d;
        }
        if d < best_dist {
            best_dist = d;
            best = domain;
        }
    }
    best
}

fn domain_from_angle_with_offsets(angle: f32, domain_offsets: &[f32; 4]) -> RadialDomain {
    let mut best = RadialDomain::Node;
    let mut best_dist = f32::MAX;
    for domain in RadialDomain::ALL {
        let target = domain_angle_with_offsets(domain, domain_offsets[domain.index()]);
        let mut d = (angle - target).abs();
        if d > std::f32::consts::PI {
            d = 2.0 * std::f32::consts::PI - d;
        }
        if d < best_dist {
            best_dist = d;
            best = domain;
        }
    }
    best
}

fn domain_angle(domain: RadialDomain) -> f32 {
    match domain {
        RadialDomain::Node => -std::f32::consts::FRAC_PI_2,
        RadialDomain::Edge => -0.25,
        RadialDomain::Graph => 1.45,
        RadialDomain::Persistence => 2.7,
    }
}

fn domain_anchor(center: egui::Pos2, domain: RadialDomain, radius: f32) -> egui::Pos2 {
    domain_anchor_with_offsets(center, domain, radius, 0.0)
}

fn domain_anchor_with_offsets(
    center: egui::Pos2,
    domain: RadialDomain,
    radius: f32,
    domain_offset: f32,
) -> egui::Pos2 {
    let a = domain_angle_with_offsets(domain, domain_offset);
    center + egui::vec2(a.cos() * radius, a.sin() * radius)
}

fn domain_angle_with_offsets(domain: RadialDomain, domain_offset: f32) -> f32 {
    domain_angle(domain) + domain_offset
}

fn command_anchor(center: egui::Pos2, domain: RadialDomain, idx: usize, len: usize) -> egui::Pos2 {
    command_anchor_with_offsets(center, domain, idx, len, 0.0, 0.0)
}

fn command_anchor_with_offsets(
    center: egui::Pos2,
    domain: RadialDomain,
    idx: usize,
    len: usize,
    domain_offset: f32,
    command_offset: f32,
) -> egui::Pos2 {
    command_anchor_with_offsets_at_radius(
        center,
        domain,
        idx,
        len,
        domain_offset,
        command_offset,
        COMMAND_RING_RADIUS,
    )
}

fn command_anchor_with_offsets_at_radius(
    center: egui::Pos2,
    domain: RadialDomain,
    idx: usize,
    len: usize,
    domain_offset: f32,
    command_offset: f32,
    command_ring_radius: f32,
) -> egui::Pos2 {
    let angle = command_angle_with_offsets_at_radius(
        domain,
        idx,
        len,
        domain_offset,
        command_offset,
        command_ring_radius,
    );
    center
        + egui::vec2(
            angle.cos() * command_ring_radius,
            angle.sin() * command_ring_radius,
        )
}

fn command_angle_with_offsets_at_radius(
    domain: RadialDomain,
    idx: usize,
    len: usize,
    domain_offset: f32,
    command_offset: f32,
    command_ring_radius: f32,
) -> f32 {
    let base = domain_angle_with_offsets(domain, domain_offset) + command_offset;
    let spread = command_spread_for_len(len, command_ring_radius, MIN_COMMAND_CENTER_SPACING);
    let t = if len <= 1 {
        0.0
    } else {
        idx as f32 / (len.saturating_sub(1) as f32) - 0.5
    };
    base + t * spread
}

fn command_angle_with_offsets(
    domain: RadialDomain,
    idx: usize,
    len: usize,
    domain_offset: f32,
    command_offset: f32,
) -> f32 {
    let base = domain_angle_with_offsets(domain, domain_offset) + command_offset;
    let spread = command_spread_for_len(len, COMMAND_RING_RADIUS, MIN_COMMAND_CENTER_SPACING);
    let t = if len <= 1 {
        0.0
    } else {
        idx as f32 / (len.saturating_sub(1) as f32) - 0.5
    };
    base + t * spread
}

fn domain_offset_id(category: ActionCategory) -> egui::Id {
    egui::Id::new("radial_domain_rail_offset").with(category_persisted_name(category))
}

fn command_offset_id(category: ActionCategory) -> egui::Id {
    egui::Id::new("radial_command_rail_offset").with(category_persisted_name(category))
}

fn command_spread_for_len(len: usize, radius: f32, min_center_spacing: f32) -> f32 {
    if len <= 1 {
        return 0.0;
    }

    let required_min_spread = ((len.saturating_sub(1)) as f32) * (min_center_spacing / radius);
    required_min_spread.max(0.8).min(2.6)
}

fn ring_layout_supports_hover_non_overlap(
    center: egui::Pos2,
    domain: RadialDomain,
    len: usize,
    domain_offset: f32,
    command_offset: f32,
) -> bool {
    ring_layout_supports_hover_non_overlap_with_ring_radius(
        center,
        domain,
        len,
        domain_offset,
        command_offset,
        COMMAND_RING_RADIUS,
    )
}

fn ring_layout_supports_hover_non_overlap_with_ring_radius(
    center: egui::Pos2,
    domain: RadialDomain,
    len: usize,
    domain_offset: f32,
    command_offset: f32,
    ring_radius: f32,
) -> bool {
    ring_layout_supports_hover_non_overlap_with_radius(
        center,
        domain,
        len,
        domain_offset,
        command_offset,
        EFFECTIVE_HOVER_BUTTON_RADIUS,
        ring_radius,
    )
}

fn ring_layout_supports_hover_non_overlap_with_radius(
    center: egui::Pos2,
    domain: RadialDomain,
    len: usize,
    domain_offset: f32,
    command_offset: f32,
    hovered_radius: f32,
    ring_radius: f32,
) -> bool {
    if len <= 1 {
        return true;
    }

    let anchors: Vec<egui::Pos2> = (0..len)
        .map(|idx| {
            command_anchor_with_offsets_at_radius(
                center,
                domain,
                idx,
                len,
                domain_offset,
                command_offset,
                ring_radius,
            )
        })
        .collect();

    for hovered_idx in 0..anchors.len() {
        for other_idx in 0..anchors.len() {
            if hovered_idx == other_idx {
                continue;
            }
            let distance = (anchors[hovered_idx] - anchors[other_idx]).length();
            if distance < (hovered_radius + COMMAND_BUTTON_RADIUS + RING_COLLISION_EPSILON) {
                return false;
            }
        }
    }
    true
}

fn nearest_entry_for_pointer_with_radius(
    domain: RadialDomain,
    center: egui::Pos2,
    pointer: egui::Pos2,
    cmds: &[ActionEntry],
    domain_offset: f32,
    command_offset: f32,
    ring_radius: f32,
) -> Option<ActionEntry> {
    let mut best: Option<(f32, ActionEntry)> = None;
    for (idx, entry) in cmds.iter().enumerate() {
        if !entry.enabled {
            continue;
        }
        let anchor = command_anchor_with_offsets_at_radius(
            center,
            domain,
            idx,
            cmds.len(),
            domain_offset,
            command_offset,
            ring_radius,
        );
        let d = (pointer - anchor).length_sq();
        match best {
            Some((best_d, _)) if d >= best_d => {}
            _ => best = Some((d, entry.clone())),
        }
    }
    best.map(|(_, entry)| entry)
}

fn visible_ring_entries(cmds: &[ActionEntry]) -> &[ActionEntry] {
    &cmds[..cmds.len().min(MAX_VISIBLE_ACTIONS_PER_RING)]
}

fn ring_page_count(total: usize, page_size: usize) -> usize {
    if total == 0 || page_size == 0 {
        return 0;
    }
    total.div_ceil(page_size)
}

fn paged_ring_entries(cmds: &[ActionEntry], page: usize, page_size: usize) -> &[ActionEntry] {
    if cmds.is_empty() || page_size == 0 {
        return &cmds[0..0];
    }
    let page_count = ring_page_count(cmds.len(), page_size);
    let normalized_page = page.min(page_count.saturating_sub(1));
    let start = normalized_page * page_size;
    let end = (start + page_size).min(cmds.len());
    &cmds[start..end]
}

fn nearest_entry_for_pointer(
    domain: RadialDomain,
    center: egui::Pos2,
    pointer: egui::Pos2,
    cmds: &[ActionEntry],
    domain_offset: f32,
    command_offset: f32,
) -> Option<ActionEntry> {
    nearest_entry_for_pointer_with_radius(
        domain,
        center,
        pointer,
        cmds,
        domain_offset,
        command_offset,
        COMMAND_RING_RADIUS,
    )
}

fn compute_label_layout_metrics_with_radius(
    center: egui::Pos2,
    domain: RadialDomain,
    entries: &[ActionEntry],
    ring_radius: f32,
) -> LabelLayoutMetrics {
    if entries.len() <= 1 {
        return LabelLayoutMetrics {
            pre_collisions: 0,
            post_collisions: 0,
        };
    }

    let base_rects: Vec<egui::Rect> = entries
        .iter()
        .enumerate()
        .map(|(idx, entry)| {
            let anchor = command_anchor_with_offsets_at_radius(
                center,
                domain,
                idx,
                entries.len(),
                0.0,
                0.0,
                ring_radius,
            );
            let label_pos = radial_label_anchor(anchor, center, HOVER_LABEL_OFFSET);
            hover_label_rect(label_pos, entry.id.label())
        })
        .collect();

    let pre_collisions = count_rect_collisions(&base_rects);

    let resolved_rects = resolve_label_rect_collisions(base_rects, center);
    let post_collisions = count_rect_collisions(&resolved_rects);

    LabelLayoutMetrics {
        pre_collisions,
        post_collisions,
    }
}

fn bounded_hover_label(label: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }

    let count = label.chars().count();
    if count <= max_chars {
        return label.to_string();
    }

    if max_chars <= 1 {
        return "…".to_string();
    }

    let keep = max_chars - 1;
    let mut out = label.chars().take(keep).collect::<String>();
    out.push('…');
    out
}

fn radial_label_anchor(anchor: egui::Pos2, center: egui::Pos2, outward: f32) -> egui::Pos2 {
    let delta = anchor - center;
    let len = delta.length();
    if len <= f32::EPSILON {
        return anchor + egui::vec2(outward, 0.0);
    }
    anchor + (delta / len) * outward
}

fn draw_radial_hover_label(
    painter: &egui::Painter,
    pos: egui::Pos2,
    text: &str,
    tokens: &crate::shell::desktop::runtime::registries::theme::ThemeTokenSet,
) {
    let font = egui::FontId::proportional(12.0);
    let approx_width = (text.chars().count() as f32) * 7.2 + 14.0;
    let size = egui::vec2(approx_width.max(44.0), 22.0);
    let rect = egui::Rect::from_center_size(pos, size);
    painter.rect_filled(rect, 6.0, tokens.hover_label_background);
    painter.rect_stroke(
        rect,
        6.0,
        Stroke::new(1.0, tokens.hover_label_stroke),
        egui::StrokeKind::Middle,
    );
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        font,
        tokens.hover_label_text,
    );
}

struct LabelLayoutMetrics {
    pre_collisions: usize,
    post_collisions: usize,
}

fn compute_label_layout_metrics(
    center: egui::Pos2,
    domain: RadialDomain,
    entries: &[ActionEntry],
) -> LabelLayoutMetrics {
    if entries.len() <= 1 {
        return LabelLayoutMetrics {
            pre_collisions: 0,
            post_collisions: 0,
        };
    }

    let base_rects: Vec<egui::Rect> = entries
        .iter()
        .enumerate()
        .map(|(idx, entry)| {
            let anchor = command_anchor(center, domain, idx, entries.len());
            let label_pos = radial_label_anchor(anchor, center, HOVER_LABEL_OFFSET);
            hover_label_rect(label_pos, entry.id.label())
        })
        .collect();

    let pre_collisions = count_rect_collisions(&base_rects);

    let resolved_rects = resolve_label_rect_collisions(base_rects, center);
    let post_collisions = count_rect_collisions(&resolved_rects);

    LabelLayoutMetrics {
        pre_collisions,
        post_collisions,
    }
}

fn hover_label_rect(pos: egui::Pos2, label: &str) -> egui::Rect {
    let text = bounded_hover_label(label, HOVER_LABEL_MAX_CHARS);
    let approx_width = (text.chars().count() as f32) * 7.2 + 14.0;
    let size = egui::vec2(approx_width.max(44.0), 22.0);
    egui::Rect::from_center_size(pos, size)
}

fn count_rect_collisions(rects: &[egui::Rect]) -> usize {
    let mut collisions = 0usize;
    for left in 0..rects.len() {
        for right in (left + 1)..rects.len() {
            if rects[left].intersects(rects[right]) {
                collisions = collisions.saturating_add(1);
            }
        }
    }
    collisions
}

fn resolve_label_rect_collisions(
    mut rects: Vec<egui::Rect>,
    center: egui::Pos2,
) -> Vec<egui::Rect> {
    if rects.len() <= 1 {
        return rects;
    }

    const MAX_PASSES: usize = 6;
    const EXTRA_STEP: f32 = 10.0;

    for _ in 0..MAX_PASSES {
        let mut changed = false;
        for idx in 0..rects.len() {
            let mut overlaps = false;
            for other in 0..rects.len() {
                if idx == other {
                    continue;
                }
                if rects[idx].intersects(rects[other]) {
                    overlaps = true;
                    break;
                }
            }
            if overlaps {
                let offset = rects[idx].center() - center;
                let length = offset.length();
                let direction = if length <= f32::EPSILON {
                    egui::vec2(1.0, 0.0)
                } else {
                    offset / length
                };
                rects[idx] = rects[idx].translate(direction * EXTRA_STEP);
                changed = true;
            }
        }

        if !changed || count_rect_collisions(&rects) == 0 {
            break;
        }
    }

    rects
}

#[cfg(test)]
mod tests {
    use super::*;

    fn to_linear_component(component: u8) -> f32 {
        let value = component as f32 / 255.0;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    }

    fn relative_luminance(color: Color32) -> f32 {
        0.2126 * to_linear_component(color.r())
            + 0.7152 * to_linear_component(color.g())
            + 0.0722 * to_linear_component(color.b())
    }

    fn contrast_ratio(foreground: Color32, background: Color32) -> f32 {
        let mut l1 = relative_luminance(foreground);
        let mut l2 = relative_luminance(background);
        if l2 > l1 {
            std::mem::swap(&mut l1, &mut l2);
        }
        (l1 + 0.05) / (l2 + 0.05)
    }

    fn sample_entries() -> Vec<ActionEntry> {
        use crate::render::action_registry::ActionId;
        vec![
            ActionEntry {
                id: ActionId::NodeNew,
                enabled: true,
            },
            ActionEntry {
                id: ActionId::NodeOpenFrame,
                enabled: true,
            },
            ActionEntry {
                id: ActionId::NodeOpenNeighbors,
                enabled: true,
            },
            ActionEntry {
                id: ActionId::NodeOpenConnected,
                enabled: true,
            },
            ActionEntry {
                id: ActionId::NodeOpenSplit,
                enabled: true,
            },
            ActionEntry {
                id: ActionId::NodeCopyUrl,
                enabled: true,
            },
            ActionEntry {
                id: ActionId::NodeCopyTitle,
                enabled: true,
            },
            ActionEntry {
                id: ActionId::NodePinToggle,
                enabled: true,
            },
            ActionEntry {
                id: ActionId::GraphFit,
                enabled: true,
            },
            ActionEntry {
                id: ActionId::GraphTogglePhysics,
                enabled: true,
            },
            ActionEntry {
                id: ActionId::PersistUndo,
                enabled: true,
            },
            ActionEntry {
                id: ActionId::PersistRedo,
                enabled: true,
            },
        ]
    }

    #[test]
    fn visible_ring_entries_caps_at_eight_stably() {
        let entries = sample_entries();
        let visible = visible_ring_entries(&entries);
        assert_eq!(visible.len(), MAX_VISIBLE_ACTIONS_PER_RING);
        for idx in 0..MAX_VISIBLE_ACTIONS_PER_RING {
            assert_eq!(visible[idx].id, entries[idx].id);
        }
    }

    #[test]
    fn command_anchor_spacing_avoids_overlap_at_max_visible() {
        let center = egui::pos2(0.0, 0.0);
        let len = MAX_VISIBLE_ACTIONS_PER_RING;
        let anchors: Vec<egui::Pos2> = (0..len)
            .map(|idx| command_anchor(center, RadialDomain::Node, idx, len))
            .collect();

        for idx in 1..anchors.len() {
            let distance = (anchors[idx] - anchors[idx - 1]).length();
            assert!(
                distance >= MIN_COMMAND_CENTER_SPACING - 0.5,
                "adjacent command anchors overlap: distance={distance}"
            );
        }
    }

    #[test]
    fn domain_from_angle_with_offsets_tracks_rotated_domain_position() {
        let mut offsets = [0.0f32; 4];
        offsets[RadialDomain::Node.index()] = 0.4;
        let probe =
            domain_angle_with_offsets(RadialDomain::Node, offsets[RadialDomain::Node.index()]);
        assert_eq!(
            domain_from_angle_with_offsets(probe, &offsets),
            RadialDomain::Node
        );
    }

    #[test]
    fn command_anchor_with_offsets_moves_anchor_position() {
        let center = egui::pos2(0.0, 0.0);
        let base = command_anchor(center, RadialDomain::Node, 0, 4);
        let shifted = command_anchor_with_offsets(center, RadialDomain::Node, 0, 4, 0.2, 0.1);
        assert_ne!(base, shifted);
    }

    #[test]
    fn ring_layout_hover_non_overlap_precheck_accepts_current_geometry() {
        let center = egui::pos2(0.0, 0.0);
        assert!(ring_layout_supports_hover_non_overlap(
            center,
            RadialDomain::Node,
            MAX_VISIBLE_ACTIONS_PER_RING,
            0.0,
            0.0
        ));
    }

    #[test]
    fn ring_layout_hover_non_overlap_precheck_detects_expanded_radius_conflict() {
        let center = egui::pos2(0.0, 0.0);
        assert!(!ring_layout_supports_hover_non_overlap_with_radius(
            center,
            RadialDomain::Node,
            MAX_VISIBLE_ACTIONS_PER_RING,
            0.0,
            0.0,
            COMMAND_BUTTON_RADIUS * 1.5,
            COMMAND_RING_RADIUS,
        ));
    }

    #[test]
    fn paged_ring_entries_windows_are_deterministic() {
        let entries = sample_entries();
        let page0 = paged_ring_entries(&entries, 0, MAX_VISIBLE_ACTIONS_PER_RING);
        let page1 = paged_ring_entries(&entries, 1, MAX_VISIBLE_ACTIONS_PER_RING);

        assert_eq!(page0.len(), MAX_VISIBLE_ACTIONS_PER_RING);
        assert_eq!(page1.len(), entries.len() - MAX_VISIBLE_ACTIONS_PER_RING);
        assert_eq!(page0[0].id, entries[0].id);
        assert_eq!(page1[0].id, entries[MAX_VISIBLE_ACTIONS_PER_RING].id);
    }

    #[test]
    fn bounded_hover_label_truncates_with_ellipsis() {
        let text = "Open with Connected Nodes";
        let bounded = bounded_hover_label(text, 12);
        assert_eq!(bounded.chars().count(), 12);
        assert!(bounded.ends_with('…'));
    }

    #[test]
    fn radial_label_anchor_offsets_outward_from_center() {
        let center = egui::pos2(0.0, 0.0);
        let anchor = egui::pos2(50.0, 0.0);
        let label = radial_label_anchor(anchor, center, 20.0);
        assert!(label.x > anchor.x);
        assert!((label.y - anchor.y).abs() < 0.001);
    }

    #[test]
    fn resolve_label_rect_collisions_reduces_or_preserves_collision_count() {
        let rects = vec![
            egui::Rect::from_center_size(egui::pos2(0.0, 0.0), egui::vec2(120.0, 24.0)),
            egui::Rect::from_center_size(egui::pos2(10.0, 0.0), egui::vec2(120.0, 24.0)),
            egui::Rect::from_center_size(egui::pos2(20.0, 0.0), egui::vec2(120.0, 24.0)),
        ];

        let pre = count_rect_collisions(&rects);
        let resolved = resolve_label_rect_collisions(rects, egui::pos2(-200.0, 0.0));
        let post = count_rect_collisions(&resolved);
        assert!(post <= pre);
    }

    // ── §6 scenario contract tests (radial_menu_geometry_and_overflow_spec.md §6) ──────

    /// §6 scenario 1: 1–8 category contexts render all Tier-1 categories on one ring page.
    /// With exactly 4 registered categories, all fit on page 0 — no overflow page is needed.
    #[test]
    fn scenario_1_to_8_categories_fit_on_single_tier1_page() {
        let categories = default_category_order();
        assert!(
            categories.len() <= MAX_VISIBLE_ACTIONS_PER_RING,
            "all registered Tier-1 categories must fit on a single ring page (≤{}); \
             got {}",
            MAX_VISIBLE_ACTIONS_PER_RING,
            categories.len()
        );
        let page_count = ring_page_count(categories.len(), MAX_VISIBLE_ACTIONS_PER_RING);
        assert_eq!(
            page_count, 1,
            "exactly 1 Tier-1 page expected when category count ≤ max-visible"
        );
    }

    /// §6 scenario 2: Tier-1 category selection drives the Tier-2 option ring for that
    /// category. Each registered category must produce a non-empty Tier-2 action list.
    #[test]
    fn scenario_tier1_selection_drives_tier2_option_ring() {
        let ctx = ActionContext {
            scope: crate::app::ActionScope::default(),
            target_node: None,
            target_frame_name: None,
            target_frame_member: None,
            target_frame_split_offer_suppressed: false,
            pair_context: None,
            any_selected: false,
            focused_pane_available: false,
            undo_available: true,
            redo_available: false,
            input_mode: InputMode::MouseKeyboard,
            view_id: crate::app::GraphViewId::new(),
            wry_override_allowed: false,
            layout_surface_host_available: false,
            layout_surface_target_host: None,
            layout_surface_target_ambiguous: false,
            layout_surface_configuring: false,
            layout_surface_has_draft: false,
        };
        for category in default_category_order() {
            let actions = list_radial_actions_for_category(&ctx, category);
            assert!(
                !actions.is_empty(),
                "Tier-1 category {:?} must produce at least one Tier-2 action for undo-available context",
                category
            );
        }
    }

    #[test]
    fn cycle_layout_surface_host_wraps_between_visible_hosts() {
        let hosts = vec![
            SurfaceHostId::Navigator(crate::app::workbench_layout_policy::NavigatorHostId::Top),
            SurfaceHostId::Navigator(crate::app::workbench_layout_policy::NavigatorHostId::Bottom),
        ];

        assert_eq!(
            cycle_layout_surface_host(&hosts, &None, 1),
            Some(hosts[1].clone())
        );
        assert_eq!(
            cycle_layout_surface_host(&hosts, &Some(hosts[1].clone()), 1),
            Some(hosts[0].clone())
        );
        assert_eq!(
            cycle_layout_surface_host(&hosts, &Some(hosts[0].clone()), -1),
            Some(hosts[1].clone())
        );
    }

    #[test]
    fn build_radial_action_context_uses_selected_host_for_layout_draft_state() {
        let mut app = GraphBrowserApp::new_for_testing();
        let top =
            SurfaceHostId::Navigator(crate::app::workbench_layout_policy::NavigatorHostId::Top);
        let bottom =
            SurfaceHostId::Navigator(crate::app::workbench_layout_policy::NavigatorHostId::Bottom);
        app.set_workbench_layout_constraint(
            top.clone(),
            crate::app::WorkbenchLayoutConstraint::anchored_split(
                top.clone(),
                crate::app::workbench_layout_policy::AnchorEdge::Top,
                0.15,
            ),
        );
        app.set_workbench_layout_constraint(
            bottom.clone(),
            crate::app::WorkbenchLayoutConstraint::anchored_split(
                bottom.clone(),
                crate::app::workbench_layout_policy::AnchorEdge::Bottom,
                0.15,
            ),
        );
        app.set_workbench_layout_constraint_draft(
            bottom.clone(),
            crate::app::WorkbenchLayoutConstraint::anchored_split(
                bottom.clone(),
                crate::app::workbench_layout_policy::AnchorEdge::Bottom,
                0.21,
            ),
        );

        let top_context = build_radial_action_context(&app, None, None, false, None, Some(top));
        assert!(!top_context.layout_surface_has_draft);

        let bottom_context =
            build_radial_action_context(&app, None, None, false, None, Some(bottom));
        assert!(bottom_context.layout_surface_has_draft);
    }

    /// §6 scenario 3: >8 categories/options trigger deterministic paging.
    /// Build a synthetic entry list of 9 and verify page 0 has exactly 8, page 1 has 1.
    #[test]
    fn scenario_overflow_beyond_8_triggers_deterministic_paging() {
        let entries: Vec<ActionEntry> = (0..9)
            .map(|i| ActionEntry {
                id: if i % 2 == 0 {
                    ActionId::NodeNew
                } else {
                    ActionId::GraphFit
                },
                enabled: true,
            })
            .collect();

        let page_count = ring_page_count(entries.len(), MAX_VISIBLE_ACTIONS_PER_RING);
        assert_eq!(page_count, 2, "9 entries should produce 2 pages");

        let page0 = paged_ring_entries(&entries, 0, MAX_VISIBLE_ACTIONS_PER_RING);
        let page1 = paged_ring_entries(&entries, 1, MAX_VISIBLE_ACTIONS_PER_RING);
        assert_eq!(page0.len(), MAX_VISIBLE_ACTIONS_PER_RING);
        assert_eq!(page1.len(), 1);

        // Determinism: same entries, same pages, same order.
        let page0_again = paged_ring_entries(&entries, 0, MAX_VISIBLE_ACTIONS_PER_RING);
        assert!(
            page0
                .iter()
                .zip(page0_again.iter())
                .all(|(a, b)| a.id == b.id),
            "Tier-2 paging must be deterministic across calls"
        );
    }

    /// §6 scenario 4: Tier-1/Tier-2 rings never produce lane/radius overlap under
    /// hover scaling. The pre-check must pass for the default geometry at max visible count.
    #[test]
    fn scenario_rings_do_not_overlap_under_hover_scaling_at_max_visible() {
        let center = egui::pos2(0.0, 0.0);
        for domain in RadialDomain::ALL {
            assert!(
                ring_layout_supports_hover_non_overlap(
                    center,
                    domain,
                    MAX_VISIBLE_ACTIONS_PER_RING,
                    0.0,
                    0.0,
                ),
                "Tier-2 ring with domain {:?} at max visible count must not produce \
                 lane overlap under hover scaling (spec §4.1, §6 scenario 4)",
                domain
            );
        }
    }

    /// §6 scenario 5: Label collision resolver reduces collisions at the default ring radius.
    ///
    /// Full zero-collision convergence is not guaranteed by the resolver alone — the spec
    /// contract (§4.1) defines a three-stage fallback: radial offset → in-field truncation/scroll
    /// → pagination. The unit-testable invariant is: post-collision count ≤ pre-collision count,
    /// and the `ux:radial_label_collision` Warn channel fires when post > 0 (diagnostics contract
    /// in §5.2 covers the observable fallback signal).
    #[test]
    fn scenario_label_collision_resolver_reduces_collisions_at_default_radius() {
        let center = egui::pos2(0.0, 0.0);
        let entries = sample_entries(); // 8 entries, evenly distributed
        let metrics = compute_label_layout_metrics_with_radius(
            center,
            RadialDomain::Node,
            &entries,
            COMMAND_RING_RADIUS,
        );
        assert!(
            metrics.post_collisions <= metrics.pre_collisions,
            "label collision resolver must not increase collision count (spec §4.1, §6 scenario 5); \
             pre={}, post={}",
            metrics.pre_collisions,
            metrics.post_collisions
        );
        // With only 2 entries the anchors are far enough apart that the resolver must fully converge.
        let two_entry_metrics = compute_label_layout_metrics_with_radius(
            center,
            RadialDomain::Node,
            &entries[..2],
            COMMAND_RING_RADIUS,
        );
        assert_eq!(
            two_entry_metrics.post_collisions, 0,
            "label collision resolver must converge to 0 overlaps for 2-entry ring \
             (anchors are 180° apart — no tangential crowding) (spec §4.1, §6 scenario 5)"
        );
    }

    /// §6 scenario 6 (partial): Keyboard/gamepad angular selection resolves to the
    /// same entry as pointer-based nearest-entry resolution when both aim at the
    /// same angular position. Full dispatch parity requires egui context (integration test).
    #[test]
    fn scenario_keyboard_angular_selection_matches_pointer_nearest_entry() {
        let center = egui::pos2(0.0, 0.0);
        let entries = sample_entries();
        let len = entries.len().min(MAX_VISIBLE_ACTIONS_PER_RING);

        // For each anchor, aim the pointer directly at it — nearest_entry must pick that entry.
        for idx in 0..len {
            let anchor = command_anchor(center, RadialDomain::Node, idx, len);
            let nearest = nearest_entry_for_pointer(
                RadialDomain::Node,
                center,
                anchor,
                &entries[..len],
                0.0,
                0.0,
            );
            assert!(
                nearest.is_some(),
                "pointer aimed at anchor {idx} must resolve a nearest entry (spec §4.3, §6 scenario 6)"
            );
            assert_eq!(
                nearest.unwrap().id,
                entries[idx].id,
                "pointer aimed directly at anchor {idx} must resolve the entry at that index"
            );
        }
    }

    #[test]
    fn radial_disabled_text_contrast_meets_wcag_minimum_for_text() {
        let tokens = crate::shell::desktop::runtime::registries::theme::ThemeRegistry::default()
            .active_theme()
            .tokens;
        let ratio = contrast_ratio(
            tokens.radial_disabled_text,
            tokens.radial_command_disabled_fill,
        );
        assert!(
            ratio >= 4.5,
            "expected disabled text contrast >= 4.5:1, got {ratio:.2}:1"
        );
    }
}
