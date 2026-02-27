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
use egui::{Color32, Key, Stroke, Window};

/// Radial domain maps to `ActionCategory` for registry-backed content.
///
/// Kept as an internal UI type for angular layout calculations only.
/// Action *content* is now driven by `ActionRegistry`, not this enum.
#[derive(Clone, Copy, PartialEq, Eq)]
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
    if !app.workspace.show_radial_menu {
        return;
    }

    let pair_context = super::resolve_pair_command_context(app, hovered_node, focused_pane_node);
    let source_context = super::resolve_source_node_context(app, hovered_node, focused_pane_node);
    let any_selected = !app.workspace.selected_nodes.is_empty();
    let mut intents = Vec::new();
    let mut should_close = false;

    let action_context = ActionContext {
        target_node: source_context,
        pair_context,
        any_selected,
        focused_pane_available: focused_pane_node.is_some(),
        input_mode: InputMode::Gamepad,
        view_id: crate::app::GraphViewId::new(),
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
        let mut hovered_domain = None;
        let mut hovered_entry: Option<ActionEntry> = None;
        if let Some(pos) = pointer {
            let delta = pos - center;
            let r = delta.length();
            if r > 40.0 {
                let angle = delta.y.atan2(delta.x);
                hovered_domain = Some(domain_from_angle(angle));
                if r > 120.0
                    && let Some(domain) = hovered_domain
                {
                    let cmds = list_radial_actions_for_category(&action_context, domain.category());
                    hovered_entry = nearest_entry_for_pointer(domain, center, pos, &cmds);
                }
            }
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
                painter.text(
                    center,
                    egui::Align2::CENTER_CENTER,
                    "Cmd",
                    egui::FontId::proportional(16.0),
                    Color32::from_rgb(210, 230, 245),
                );

                for domain in RadialDomain::ALL {
                    let base = domain_anchor(center, domain, 92.0);
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
                    for (idx, entry) in cmds.iter().enumerate() {
                        let anchor = command_anchor(center, domain, idx, cmds.len());
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
                                Color32::from_rgb(120, 125, 130)
                            },
                        );
                    }
                }
            });

        if let Some(entry) = clicked_entry {
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

fn domain_angle(domain: RadialDomain) -> f32 {
    match domain {
        RadialDomain::Node => -std::f32::consts::FRAC_PI_2,
        RadialDomain::Edge => -0.25,
        RadialDomain::Graph => 1.45,
        RadialDomain::Persistence => 2.7,
    }
}

fn domain_anchor(center: egui::Pos2, domain: RadialDomain, radius: f32) -> egui::Pos2 {
    let a = domain_angle(domain);
    center + egui::vec2(a.cos() * radius, a.sin() * radius)
}

fn command_anchor(center: egui::Pos2, domain: RadialDomain, idx: usize, len: usize) -> egui::Pos2 {
    let base = domain_angle(domain);
    let spread = 0.8_f32;
    let t = if len <= 1 {
        0.0
    } else {
        idx as f32 / (len.saturating_sub(1) as f32) - 0.5
    };
    let angle = base + t * spread;
    center + egui::vec2(angle.cos() * 165.0, angle.sin() * 165.0)
}

fn nearest_entry_for_pointer(
    domain: RadialDomain,
    center: egui::Pos2,
    pointer: egui::Pos2,
    cmds: &[ActionEntry],
) -> Option<ActionEntry> {
    let mut best: Option<(f32, ActionEntry)> = None;
    for (idx, entry) in cmds.iter().enumerate() {
        if !entry.enabled {
            continue;
        }
        let anchor = command_anchor(center, domain, idx, cmds.len());
        let d = (pointer - anchor).length_sq();
        match best {
            Some((best_d, _)) if d >= best_d => {}
            _ => best = Some((d, entry.clone())),
        }
    }
    best.map(|(_, entry)| entry)
}
