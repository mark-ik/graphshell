/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Canvas input handling: lasso gesture, keyboard traversal, and egui_graphs
//! event → GraphAction conversion.

use crate::app::{GraphBrowserApp, SelectionUpdateMode, VisibleNavigationRegionSet};
use crate::graph::NodeKey;
use crate::registries::domain::layout::canvas::CanvasLassoBinding;
use crate::render::GraphAction;
use crate::shell::desktop::runtime::diagnostics::{DiagnosticEvent, emit_event};
use crate::shell::desktop::runtime::registries::CHANNEL_UI_GRAPH_EVENT_BLOCKED_NO_STATE;
use crate::shell::desktop::runtime::registries::CHANNEL_UI_GRAPH_LASSO_BLOCKED_NO_STATE;
use egui::{Stroke, Ui};
use egui_graphs::{MetadataFrame, events::Event};
use euclid::default::Point2D;
use petgraph::stable_graph::NodeIndex;
use std::cell::RefCell;
use std::rc::Rc;

use super::canvas_visuals::active_presentation_profile;
use super::node_key_or_emit_ambiguous_hit;
use super::spatial_index::NodeSpatialIndex;

// ── Lasso gesture ─────────────────────────────────────────────────────────────

pub(super) struct LassoGestureResult {
    pub(super) action: Option<GraphAction>,
    pub(super) suppress_context_menu: bool,
}

pub(super) fn resolve_lasso_selection_mode(
    lasso_binding: CanvasLassoBinding,
    ctrl: bool,
    shift: bool,
    alt: bool,
) -> SelectionUpdateMode {
    let add_mode = ctrl || (matches!(lasso_binding, CanvasLassoBinding::RightDrag) && shift);
    if alt {
        SelectionUpdateMode::Toggle
    } else if add_mode {
        SelectionUpdateMode::Add
    } else {
        SelectionUpdateMode::Replace
    }
}

pub(super) fn normalize_lasso_keys(mut keys: Vec<NodeKey>) -> Vec<NodeKey> {
    keys.sort_by_key(|key| key.index());
    keys.dedup_by_key(|key| key.index());
    keys
}

pub(super) fn collect_lasso_action(
    ui: &Ui,
    app: &GraphBrowserApp,
    enabled: bool,
    metadata_id: egui::Id,
    lasso_binding: CanvasLassoBinding,
    visible_graph_regions: &VisibleNavigationRegionSet,
) -> LassoGestureResult {
    let presentation = active_presentation_profile(app);
    let (start_id, moved_id) = lasso_state_ids(metadata_id);
    let threshold_px = 6.0_f32;
    if !enabled {
        ui.ctx().data_mut(|d| {
            d.remove::<egui::Pos2>(start_id);
            d.remove::<bool>(moved_id);
        });
        return LassoGestureResult {
            action: None,
            suppress_context_menu: false,
        };
    }

    let (pointer_pos, pressed, down, released, ctrl, shift, alt) = ui.input(|i| {
        let (pressed, down, released) = match lasso_binding {
            CanvasLassoBinding::RightDrag => (
                i.pointer.secondary_pressed(),
                i.pointer.secondary_down(),
                i.pointer.secondary_released(),
            ),
            CanvasLassoBinding::ShiftLeftDrag => (
                i.pointer.primary_pressed() && i.modifiers.shift,
                i.pointer.primary_down() && i.modifiers.shift,
                i.pointer.primary_released(),
            ),
        };
        (
            i.pointer.latest_pos(),
            pressed,
            down,
            released,
            i.modifiers.ctrl,
            i.modifiers.shift,
            i.modifiers.alt,
        )
    });

    if pressed
        && let Some(pos) = pointer_pos
        && visible_graph_regions.contains_point(pos)
    {
        ui.ctx().data_mut(|d| {
            d.insert_persisted(start_id, pos);
            d.insert_persisted(moved_id, false);
        });
    }

    let start = ui
        .ctx()
        .data_mut(|d| d.get_persisted::<egui::Pos2>(start_id));
    let mut moved = ui
        .ctx()
        .data_mut(|d| d.get_persisted::<bool>(moved_id))
        .unwrap_or(false);
    if down && let (Some(a), Some(b)) = (start, pointer_pos) {
        if !moved && (b - a).length() >= threshold_px {
            moved = true;
            ui.ctx().data_mut(|d| d.insert_persisted(moved_id, true));
        }
        if moved {
            let rect = egui::Rect::from_two_pos(a, b);
            ui.painter().rect_stroke(
                rect,
                0.0,
                Stroke::new(1.5, presentation.lasso_stroke.to_color32()),
                egui::epaint::StrokeKind::Outside,
            );
            ui.painter()
                .rect_filled(rect, 0.0, presentation.lasso_fill.to_color32());
        }
    }

    if !released {
        return LassoGestureResult {
            action: None,
            suppress_context_menu: false,
        };
    }
    ui.ctx().data_mut(|d| {
        d.remove::<egui::Pos2>(start_id);
        d.remove::<bool>(moved_id);
    });

    let (Some(a), Some(b)) = (start, pointer_pos) else {
        return LassoGestureResult {
            action: None,
            suppress_context_menu: false,
        };
    };
    if !moved {
        return LassoGestureResult {
            action: None,
            suppress_context_menu: false,
        };
    }

    let rect = egui::Rect::from_two_pos(a, b);
    let mode = resolve_lasso_selection_mode(lasso_binding, ctrl, shift, alt);
    let meta = ui
        .ctx()
        .data_mut(|d| d.get_persisted::<MetadataFrame>(metadata_id))
        .unwrap_or_default();
    let Some(state) = app.workspace.graph_runtime.egui_state.as_ref() else {
        emit_event(DiagnosticEvent::MessageReceived {
            channel_id: CHANNEL_UI_GRAPH_LASSO_BLOCKED_NO_STATE,
            latency_us: 0,
        });
        return LassoGestureResult {
            action: None,
            suppress_context_menu: matches!(lasso_binding, CanvasLassoBinding::RightDrag),
        };
    };
    // Build an R*-tree index in canvas (world) space and query with the
    // lasso rect inverted from screen space.  This avoids a full O(n) scan
    // and keeps hit-testing in a stable coordinate system.
    let index = NodeSpatialIndex::build(
        state
            .graph
            .nodes_iter()
            .map(|(key, node)| (key, node.location(), node.display().radius())),
    );
    let canvas_min = meta.screen_to_canvas_pos(rect.min);
    let canvas_max = meta.screen_to_canvas_pos(rect.max);
    let canvas_rect = egui::Rect::from_min_max(canvas_min, canvas_max);
    let keys = normalize_lasso_keys(index.nodes_with_center_in_canvas_rect(canvas_rect));

    LassoGestureResult {
        action: Some(GraphAction::LassoSelect { keys, mode }),
        suppress_context_menu: matches!(lasso_binding, CanvasLassoBinding::RightDrag) && moved,
    }
}

pub(super) fn lasso_state_ids(metadata_id: egui::Id) -> (egui::Id, egui::Id) {
    (
        metadata_id.with("lasso_start_screen"),
        metadata_id.with("lasso_moved"),
    )
}

// ── Keyboard traversal ────────────────────────────────────────────────────────

fn node_key_traversal_order(app: &GraphBrowserApp) -> Vec<NodeKey> {
    let mut keys: Vec<NodeKey> = app.domain_graph().nodes().map(|(key, _)| key).collect();
    keys.sort_by_key(|key| key.index());
    keys
}

pub(super) fn next_keyboard_traversal_node(
    app: &GraphBrowserApp,
    selection: &crate::app::SelectionState,
    reverse: bool,
) -> Option<NodeKey> {
    let ordered_keys = node_key_traversal_order(app);
    if ordered_keys.is_empty() {
        return None;
    }

    let current = selection.primary();
    let len = ordered_keys.len();
    let next_index = current
        .and_then(|key| ordered_keys.iter().position(|candidate| *candidate == key))
        .map(|idx| {
            if reverse {
                (idx + len - 1) % len
            } else {
                (idx + 1) % len
            }
        })
        .unwrap_or_else(|| if reverse { len - 1 } else { 0 });

    ordered_keys.get(next_index).copied()
}

pub(super) fn collect_graph_keyboard_traversal_action(
    ui: &Ui,
    response: &egui::Response,
    app: &GraphBrowserApp,
    selection: &crate::app::SelectionState,
    radial_open: bool,
    lasso_active: bool,
) -> Option<GraphAction> {
    if radial_open || lasso_active || !response.has_focus() {
        return None;
    }

    let cycle_backward = ui.input(|i| {
        i.clone()
            .consume_key(egui::Modifiers::SHIFT, egui::Key::Tab)
    });
    let cycle_forward = if cycle_backward {
        false
    } else {
        ui.input(|i| i.clone().consume_key(egui::Modifiers::NONE, egui::Key::Tab))
    };

    if !(cycle_backward || cycle_forward) {
        return None;
    }

    let reverse = cycle_backward;
    let key = next_keyboard_traversal_node(app, selection, reverse)?;
    Some(GraphAction::SelectNode {
        key,
        multi_select: false,
    })
}

// ── Accessibility helpers ─────────────────────────────────────────────────────

pub(super) fn graph_node_accessibility_name(node: &crate::graph::Node) -> String {
    if !node.title.trim().is_empty() {
        node.title.clone()
    } else if !node.url.trim().is_empty() {
        node.url.clone()
    } else {
        "Untitled node".to_string()
    }
}

pub(super) fn graph_canvas_accessibility_label(
    app: &GraphBrowserApp,
    selection: &crate::app::SelectionState,
) -> String {
    if let Some(primary) = selection.primary()
        && let Some(node) = app.domain_graph().get_node(primary)
    {
        return format!(
            "Graph canvas. Focused node: {}. Press Tab or Shift+Tab to move between nodes.",
            graph_node_accessibility_name(node)
        );
    }

    "Graph canvas. No node focused. Press Tab to focus the first node.".to_string()
}

// ── Event → GraphAction conversion ───────────────────────────────────────────

/// Convert egui_graphs events to resolved GraphActions.
pub(super) fn collect_graph_actions(
    app: &GraphBrowserApp,
    events: &Rc<RefCell<Vec<Event>>>,
    split_open_modifier: bool,
    multi_select_modifier: bool,
) -> Vec<GraphAction> {
    let mut actions = Vec::new();

    for event in events.borrow_mut().drain(..) {
        match event {
            Event::NodeDoubleClick(p) => {
                if let Some(state) = app.workspace.graph_runtime.egui_state.as_ref() {
                    let idx = NodeIndex::new(p.id);
                    if let Some(key) = state.get_key(idx) {
                        if split_open_modifier {
                            actions.push(GraphAction::FocusNodeSplit(key));
                        } else {
                            actions.push(GraphAction::FocusNode(key));
                        }
                    }
                } else {
                    emit_event(DiagnosticEvent::MessageReceived {
                        channel_id: CHANNEL_UI_GRAPH_EVENT_BLOCKED_NO_STATE,
                        latency_us: 0,
                    });
                }
            }
            Event::NodeDragStart(_) => {
                actions.push(GraphAction::DragStart);
            }
            Event::NodeDragEnd(p) => {
                let idx = NodeIndex::new(p.id);
                if let Some(state) = app.workspace.graph_runtime.egui_state.as_ref() {
                    if let Some(key) = state.get_key(idx) {
                        let pos = state
                            .graph
                            .node(idx)
                            .map(|n| Point2D::new(n.location().x, n.location().y))
                            .unwrap_or_default();
                        actions.push(GraphAction::DragEnd(key, pos));
                    }
                } else {
                    emit_event(DiagnosticEvent::MessageReceived {
                        channel_id: CHANNEL_UI_GRAPH_EVENT_BLOCKED_NO_STATE,
                        latency_us: 0,
                    });
                }
            }
            Event::NodeMove(p) => {
                let idx = NodeIndex::new(p.id);
                if let Some(state) = app.workspace.graph_runtime.egui_state.as_ref() {
                    if let Some(key) = state.get_key(idx) {
                        actions.push(GraphAction::MoveNode(
                            key,
                            Point2D::new(p.new_pos[0], p.new_pos[1]),
                        ));
                    }
                } else {
                    emit_event(DiagnosticEvent::MessageReceived {
                        channel_id: CHANNEL_UI_GRAPH_EVENT_BLOCKED_NO_STATE,
                        latency_us: 0,
                    });
                }
            }
            Event::NodeSelect(p) => {
                if let Some(state) = app.workspace.graph_runtime.egui_state.as_ref() {
                    let idx = NodeIndex::new(p.id);
                    if let Some(key) = node_key_or_emit_ambiguous_hit(state.get_key(idx)) {
                        actions.push(GraphAction::SelectNode {
                            key,
                            multi_select: multi_select_modifier,
                        });
                    }
                } else {
                    emit_event(DiagnosticEvent::MessageReceived {
                        channel_id: CHANNEL_UI_GRAPH_EVENT_BLOCKED_NO_STATE,
                        latency_us: 0,
                    });
                }
            }
            Event::NodeDeselect(p) => {
                // When Ctrl is held, a NodeDeselect means the user Ctrl+Clicked an
                // already-selected node to toggle it out of the multi-selection.
                if multi_select_modifier {
                    if let Some(state) = app.workspace.graph_runtime.egui_state.as_ref() {
                        let idx = NodeIndex::new(p.id);
                        if let Some(key) = node_key_or_emit_ambiguous_hit(state.get_key(idx)) {
                            actions.push(GraphAction::SelectNode {
                                key,
                                multi_select: true,
                            });
                        }
                    } else {
                        emit_event(DiagnosticEvent::MessageReceived {
                            channel_id: CHANNEL_UI_GRAPH_EVENT_BLOCKED_NO_STATE,
                            latency_us: 0,
                        });
                    }
                }
                // Without modifier: selection clearing is handled by the next SelectNode action.
            }
            Event::Zoom(p) => {
                actions.push(GraphAction::Zoom(p.new_zoom));
            }
            _ => {}
        }
    }

    actions
}
