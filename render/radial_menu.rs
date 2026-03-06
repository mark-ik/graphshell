/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Radial command menu — directional, `ActionRegistry`-backed.
//!
//! Content is populated via [`super::action_registry::list_radial_actions_for_category`]
//! rather than the old hardcoded `RadialCommand` / `RadialDomain` enums.
//! Action dispatch is handled by [`super::command_palette::execute_action`],
//! which is shared with the command palette so both surfaces use a single
//! execution path.

use crate::app::{GraphBrowserApp, GraphIntent, SelectionUpdateMode};
use crate::graph::NodeKey;
use crate::render::action_registry::{
    ActionCategory, ActionContext, ActionEntry, ActionId, InputMode,
    list_radial_actions_for_category,
};
use crate::shell::desktop::runtime::diagnostics::{DiagnosticEvent, emit_event};
use crate::shell::desktop::runtime::registries::{
    CHANNEL_UX_NAVIGATION_TRANSITION, CHANNEL_UX_RADIAL_LABEL_COLLISION, CHANNEL_UX_RADIAL_LAYOUT,
    CHANNEL_UX_RADIAL_MODE_FALLBACK, CHANNEL_UX_RADIAL_OVERFLOW,
};
use egui::{Color32, Key, Stroke, Window};

const MAX_VISIBLE_ACTIONS_PER_RING: usize = 8;
const COMMAND_RING_RADIUS: f32 = 165.0;
const COMMAND_BUTTON_RADIUS: f32 = 22.0;
const MIN_COMMAND_CENTER_SPACING: f32 = (COMMAND_BUTTON_RADIUS * 2.0) + 4.0;
const HOVER_LABEL_MAX_CHARS: usize = 22;
const HOVER_LABEL_OFFSET: f32 = 34.0;
const RADIAL_DISABLED_TEXT_COLOR: Color32 = Color32::from_rgb(165, 172, 178);
const RADIAL_FALLBACK_NOTICE_KEY: &str = "radial_mode_fallback_notice";
const RAIL_OFFSET_STEP_RAD: f32 = 0.08;

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

/// Render the radial command menu.
///
/// Content is driven by [`list_radial_actions_for_category`]; no hardcoded
/// `RadialCommand` enum exists in this module.
pub fn render_radial_command_menu(
    ctx: &egui::Context,
    app: &mut GraphBrowserApp,
    hovered_node: Option<NodeKey>,
    focused_pane_node: Option<NodeKey>,
) {
    let was_open = app.workspace.show_radial_menu;
    if !was_open {
        return;
    }

    let pair_context = super::resolve_pair_command_context(app, hovered_node, focused_pane_node);
    let source_context = super::resolve_source_node_context(app, hovered_node, focused_pane_node);
    let any_selected = !app.focused_selection().is_empty();
    let mut intents = Vec::new();
    let mut should_close = false;

    let action_context = ActionContext {
        target_node: source_context,
        pair_context,
        any_selected,
        focused_pane_available: focused_pane_node.is_some(),
        undo_available: app.undo_stack_len() > 0,
        redo_available: app.redo_stack_len() > 0,
        input_mode: InputMode::Gamepad,
        view_id: app
            .workspace
            .focused_view
            .unwrap_or_else(crate::app::GraphViewId::new),
        wry_override_allowed: cfg!(feature = "wry")
            && app.wry_enabled()
            && crate::registries::infrastructure::mod_loader::runtime_has_capability("viewer:wry"),
    };

    let center_id = egui::Id::new("radial_menu_center");
    let node_context_group_state_id = egui::Id::new("node_context_kbd_group");
    let node_context_command_state_id = egui::Id::new("node_context_kbd_command");
    let pointer = ctx.input(|i| i.pointer.latest_pos());
    let center = ctx
        .data_mut(|d| d.get_persisted::<egui::Pos2>(center_id))
        .or(pointer)
        .unwrap_or(egui::pos2(320.0, 220.0));
    ctx.data_mut(|d| d.insert_persisted(center_id, center));

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
        if ctx.input(|i| i.key_pressed(Key::Enter)) {
            if let Some(entry) = keyboard_commands.get(command_idx)
                && entry.enabled
            {
                super::command_palette::execute_action(
                    app,
                    entry.id,
                    pair_context,
                    source_context,
                    &mut intents,
                    focused_pane_node,
                );
                should_close = true;
            }
        }
        ctx.data_mut(|d| {
            d.insert_persisted(node_context_group_state_id, group_idx);
            d.insert_persisted(node_context_command_state_id, command_idx);
        });

        let window_response = Window::new("Node Context")
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .fixed_pos(center + egui::vec2(12.0, 12.0))
            .default_width(260.0)
            .show(ctx, |ui| {
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
                                    super::command_palette::execute_action(
                                        app,
                                        entry.id,
                                        pair_context,
                                        source_context,
                                        &mut intents,
                                        focused_pane_node,
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
                        super::command_palette::execute_action(
                            app,
                            entry.id,
                            pair_context,
                            source_context,
                            &mut intents,
                            focused_pane_node,
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
                intents.push(GraphIntent::UpdateSelection {
                    keys: Vec::new(),
                    mode: SelectionUpdateMode::Replace,
                });
                should_close = true;
            }
        }
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            should_close = true;
        }
    } else {
        // Circular radial mode: hover by angle, click to confirm.
        let mut domain_offsets = [0.0f32; 4];
        let mut command_offsets = [0.0f32; 4];
        for domain in RadialDomain::ALL {
            domain_offsets[domain.index()] = ctx
                .data_mut(|d| d.get_persisted::<f32>(domain_offset_id(domain)))
                .unwrap_or(0.0);
            command_offsets[domain.index()] = ctx
                .data_mut(|d| d.get_persisted::<f32>(command_offset_id(domain)))
                .unwrap_or(0.0);
        }

        let mut hovered_domain = None;
        let mut hovered_entry: Option<ActionEntry> = None;
        let mut fallback_to_command_palette = false;
        if let Some(pos) = pointer {
            let delta = pos - center;
            let r = delta.length();
            if r > 40.0 {
                let angle = delta.y.atan2(delta.x);
                hovered_domain = Some(domain_from_angle_with_offsets(angle, &domain_offsets));
                if r > 120.0
                    && let Some(domain) = hovered_domain
                {
                    let cmds = list_radial_actions_for_category(&action_context, domain.category());
                    let page_state_id = egui::Id::new("radial_menu_page").with(domain.label());
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
                    hovered_entry = nearest_entry_for_pointer(
                        domain,
                        center,
                        pos,
                        visible_cmds,
                        domain_offsets[domain.index()],
                        command_offsets[domain.index()],
                    );

                    emit_event(DiagnosticEvent::MessageSent {
                        channel_id: CHANNEL_UX_RADIAL_LAYOUT,
                        byte_len: visible_cmds.len() + page + (page_count * 10),
                    });
                }
            }
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
        }

        for domain in RadialDomain::ALL {
            ctx.data_mut(|d| {
                d.insert_persisted(domain_offset_id(domain), domain_offsets[domain.index()]);
                d.insert_persisted(command_offset_id(domain), command_offsets[domain.index()]);
            });
        }

        let mut clicked_entry: Option<ActionEntry> = None;
        if ctx.input(|i| i.pointer.button_released(egui::PointerButton::Primary)) {
            clicked_entry = hovered_entry.clone();
            should_close = true;
        }
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            should_close = true;
        }

        egui::Area::new("radial_command_menu".into())
            .fixed_pos(center - egui::vec2(220.0, 220.0))
            .interactable(false)
            .show(ctx, |ui| {
                ui.set_min_size(egui::vec2(440.0, 440.0));
                let painter = ui.painter();
                painter.circle_filled(center, 36.0, Color32::from_rgb(28, 32, 36));
                painter.circle_stroke(
                    center,
                    36.0,
                    Stroke::new(2.0, Color32::from_rgb(90, 110, 125)),
                );
                let hub_label = hovered_domain
                    .map(|d| d.label().to_string())
                    .unwrap_or_else(|| "Cmd".to_string());
                painter.text(
                    center,
                    egui::Align2::CENTER_CENTER,
                    hub_label,
                    egui::FontId::proportional(16.0),
                    Color32::from_rgb(210, 230, 245),
                );

                for domain in RadialDomain::ALL {
                    let base = domain_anchor_with_offsets(
                        center,
                        domain,
                        92.0,
                        domain_offsets[domain.index()],
                    );
                    let color = if Some(domain) == hovered_domain {
                        Color32::from_rgb(70, 130, 170)
                    } else {
                        Color32::from_rgb(50, 66, 80)
                    };
                    painter.circle_filled(base, 26.0, color);
                    painter.text(
                        base,
                        egui::Align2::CENTER_CENTER,
                        domain.label(),
                        egui::FontId::proportional(12.0),
                        Color32::WHITE,
                    );
                }

                if let Some(domain) = hovered_domain {
                    let cmds = list_radial_actions_for_category(&action_context, domain.category());
                    let page_state_id = egui::Id::new("radial_menu_page").with(domain.label());
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
                    if cmds.len() > visible_cmds.len() {
                        emit_event(DiagnosticEvent::MessageSent {
                            channel_id: CHANNEL_UX_RADIAL_OVERFLOW,
                            byte_len: cmds.len() - visible_cmds.len(),
                        });
                    }
                    ctx.data_mut(|d| d.insert_persisted(page_state_id, page));

                    let label_layout = compute_label_layout_metrics(center, domain, visible_cmds);
                    let packed_collisions = (label_layout.pre_collisions << 16)
                        .saturating_add(label_layout.post_collisions);
                    emit_event(DiagnosticEvent::MessageSent {
                        channel_id: CHANNEL_UX_RADIAL_LABEL_COLLISION,
                        byte_len: packed_collisions,
                    });
                    if label_layout.post_collisions > 0 {
                        fallback_to_command_palette = true;
                        emit_event(DiagnosticEvent::MessageReceived {
                            channel_id: CHANNEL_UX_RADIAL_MODE_FALLBACK,
                            latency_us: label_layout.post_collisions as u64,
                        });
                    }

                    for (idx, entry) in visible_cmds.iter().enumerate() {
                        let anchor = command_anchor_with_offsets(
                            center,
                            domain,
                            idx,
                            visible_cmds.len(),
                            domain_offsets[domain.index()],
                            command_offsets[domain.index()],
                        );
                        let is_hovered = hovered_entry.as_ref().is_some_and(|h| h.id == entry.id);
                        let color = if is_hovered {
                            Color32::from_rgb(80, 170, 215)
                        } else if entry.enabled {
                            Color32::from_rgb(64, 82, 98)
                        } else {
                            Color32::from_rgb(42, 48, 54)
                        };
                        painter.circle_filled(anchor, 22.0, color);
                        painter.text(
                            anchor,
                            egui::Align2::CENTER_CENTER,
                            entry.id.short_label(),
                            egui::FontId::proportional(10.0),
                            if entry.enabled {
                                Color32::from_rgb(230, 240, 248)
                            } else {
                                RADIAL_DISABLED_TEXT_COLOR
                            },
                        );

                        if is_hovered {
                            let label_text =
                                bounded_hover_label(entry.id.label(), HOVER_LABEL_MAX_CHARS);
                            let label_pos = radial_label_anchor(anchor, center, HOVER_LABEL_OFFSET);
                            draw_radial_hover_label(painter, label_pos, &label_text);
                        }
                    }
                }

                if let Some(domain) = hovered_domain {
                    let page_state_id = egui::Id::new("radial_menu_page").with(domain.label());
                    let page = ctx
                        .data_mut(|d| d.get_persisted::<usize>(page_state_id))
                        .unwrap_or(0);
                    let cmds = list_radial_actions_for_category(&action_context, domain.category());
                    let page_count = ring_page_count(cmds.len(), MAX_VISIBLE_ACTIONS_PER_RING);
                    if page_count > 1 {
                        painter.text(
                            center + egui::vec2(0.0, 52.0),
                            egui::Align2::CENTER_CENTER,
                            format!("Page {}/{}", page + 1, page_count),
                            egui::FontId::proportional(11.0),
                            Color32::from_rgb(170, 190, 205),
                        );
                    }
                    painter.text(
                        center + egui::vec2(0.0, 94.0),
                        egui::Align2::CENTER_CENTER,
                        "Arrow Left/Right: Tier1 rail | Shift+Arrow: Tier2 rail",
                        egui::FontId::proportional(10.0),
                        Color32::from_rgb(170, 190, 205),
                    );
                }

                if fallback_to_command_palette {
                    painter.text(
                        center + egui::vec2(0.0, 76.0),
                        egui::Align2::CENTER_CENTER,
                        "Radial layout constrained. Switching to command palette.",
                        egui::FontId::proportional(11.0),
                        Color32::from_rgb(234, 200, 145),
                    );
                }
            });

        if fallback_to_command_palette {
            if app.pending_node_context_target().is_none() {
                app.set_pending_node_context_target(source_context);
            }
            app.workspace.show_command_palette = true;
            ctx.data_mut(|d| d.insert_persisted(egui::Id::new(RADIAL_FALLBACK_NOTICE_KEY), true));
            should_close = true;
        }

        if !fallback_to_command_palette
            && let Some(entry) = clicked_entry
        {
            if entry.enabled {
                super::command_palette::execute_action(
                    app,
                    entry.id,
                    pair_context,
                    source_context,
                    &mut intents,
                    focused_pane_node,
                );
            }
        }
    }

    app.workspace.show_radial_menu = !should_close;
    if app.workspace.show_radial_menu != was_open {
        emit_event(DiagnosticEvent::MessageReceived {
            channel_id: CHANNEL_UX_NAVIGATION_TRANSITION,
            latency_us: 0,
        });
    }
    if !app.workspace.show_radial_menu {
        app.set_pending_node_context_target(None);
        ctx.data_mut(|d| {
            d.remove::<egui::Pos2>(center_id);
            d.remove::<usize>(node_context_group_state_id);
            d.remove::<usize>(node_context_command_state_id);
        });
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
    let base = domain_angle_with_offsets(domain, domain_offset) + command_offset;
    let spread = command_spread_for_len(len, COMMAND_RING_RADIUS, MIN_COMMAND_CENTER_SPACING);
    let t = if len <= 1 {
        0.0
    } else {
        idx as f32 / (len.saturating_sub(1) as f32) - 0.5
    };
    let angle = base + t * spread;
    center
        + egui::vec2(
            angle.cos() * COMMAND_RING_RADIUS,
            angle.sin() * COMMAND_RING_RADIUS,
        )
}

fn domain_offset_id(domain: RadialDomain) -> egui::Id {
    egui::Id::new("radial_domain_rail_offset").with(domain.label())
}

fn command_offset_id(domain: RadialDomain) -> egui::Id {
    egui::Id::new("radial_command_rail_offset").with(domain.label())
}

fn command_spread_for_len(len: usize, radius: f32, min_center_spacing: f32) -> f32 {
    if len <= 1 {
        return 0.0;
    }

    let required_min_spread = ((len.saturating_sub(1)) as f32) * (min_center_spacing / radius);
    required_min_spread.max(0.8).min(2.6)
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
    let mut best: Option<(f32, ActionEntry)> = None;
    for (idx, entry) in cmds.iter().enumerate() {
        if !entry.enabled {
            continue;
        }
        let anchor = command_anchor_with_offsets(
            center,
            domain,
            idx,
            cmds.len(),
            domain_offset,
            command_offset,
        );
        let d = (pointer - anchor).length_sq();
        match best {
            Some((best_d, _)) if d >= best_d => {}
            _ => best = Some((d, entry.clone())),
        }
    }
    best.map(|(_, entry)| entry)
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

fn draw_radial_hover_label(painter: &egui::Painter, pos: egui::Pos2, text: &str) {
    let font = egui::FontId::proportional(12.0);
    let approx_width = (text.chars().count() as f32) * 7.2 + 14.0;
    let size = egui::vec2(approx_width.max(44.0), 22.0);
    let rect = egui::Rect::from_center_size(pos, size);
    painter.rect_filled(rect, 6.0, Color32::from_rgba_unmultiplied(22, 28, 34, 235));
    painter.rect_stroke(
        rect,
        6.0,
        Stroke::new(1.0, Color32::from_rgb(88, 110, 126)),
        egui::StrokeKind::Middle,
    );
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        font,
        Color32::from_rgb(220, 236, 248),
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
        let probe = domain_angle_with_offsets(RadialDomain::Node, offsets[RadialDomain::Node.index()]);
        assert_eq!(domain_from_angle_with_offsets(probe, &offsets), RadialDomain::Node);
    }

    #[test]
    fn command_anchor_with_offsets_moves_anchor_position() {
        let center = egui::pos2(0.0, 0.0);
        let base = command_anchor(center, RadialDomain::Node, 0, 4);
        let shifted = command_anchor_with_offsets(center, RadialDomain::Node, 0, 4, 0.2, 0.1);
        assert_ne!(base, shifted);
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

    #[test]
    fn radial_disabled_text_contrast_meets_wcag_minimum_for_text() {
        let disabled_button_fill = Color32::from_rgb(42, 48, 54);
        let ratio = contrast_ratio(RADIAL_DISABLED_TEXT_COLOR, disabled_button_fill);
        assert!(
            ratio >= 4.5,
            "expected disabled text contrast >= 4.5:1, got {ratio:.2}:1"
        );
    }
}
