/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use egui::{Context, Key, Pos2, TextEdit, Window};
use egui_tiles::Tree;

use crate::app::{GraphBrowserApp, GraphIntent, TagPanelState};
use crate::graph::NodeKey;
use crate::graph::badge::BadgeIcon;
use crate::render::semantic_tags::{
    badge_icon_label, is_reserved_system_tag, normalize_tag_entry_input, ranked_tag_suggestions,
    reserved_tag_warning, semantic_tag_display_label,
};
use crate::shell::desktop::runtime::registries::knowledge::tags_for_node;
use crate::shell::desktop::runtime::registries::phase3_resolve_active_theme;
use crate::shell::desktop::workbench::tile_kind::TileKind;

#[derive(Clone, Copy)]
struct EmojiIconEntry {
    emoji: &'static str,
    keywords: &'static [&'static str],
}

const EMOJI_ICON_CATALOG: &[EmojiIconEntry] = &[
    EmojiIconEntry {
        emoji: "🔖",
        keywords: &["bookmark", "label", "tag"],
    },
    EmojiIconEntry {
        emoji: "📚",
        keywords: &["book", "library", "reading"],
    },
    EmojiIconEntry {
        emoji: "🔬",
        keywords: &["research", "science", "study"],
    },
    EmojiIconEntry {
        emoji: "🧠",
        keywords: &["brain", "idea", "knowledge", "mind"],
    },
    EmojiIconEntry {
        emoji: "📝",
        keywords: &["draft", "note", "writing"],
    },
    EmojiIconEntry {
        emoji: "🌟",
        keywords: &["favorite", "important", "star"],
    },
    EmojiIconEntry {
        emoji: "📎",
        keywords: &["attach", "clip", "reference"],
    },
    EmojiIconEntry {
        emoji: "🗂",
        keywords: &["archive", "folder", "organize"],
    },
    EmojiIconEntry {
        emoji: "🧪",
        keywords: &["experiment", "lab", "test"],
    },
    EmojiIconEntry {
        emoji: "🗺",
        keywords: &["map", "route", "travel"],
    },
    EmojiIconEntry {
        emoji: "🎯",
        keywords: &["focus", "goal", "target"],
    },
    EmojiIconEntry {
        emoji: "🧭",
        keywords: &["direction", "explore", "navigate"],
    },
    EmojiIconEntry {
        emoji: "⚙",
        keywords: &["gear", "settings", "system"],
    },
    EmojiIconEntry {
        emoji: "💡",
        keywords: &["idea", "insight", "light"],
    },
    EmojiIconEntry {
        emoji: "📌",
        keywords: &["pin", "priority", "save"],
    },
    EmojiIconEntry {
        emoji: "📰",
        keywords: &["article", "news", "press"],
    },
    EmojiIconEntry {
        emoji: "🎬",
        keywords: &["film", "media", "video"],
    },
    EmojiIconEntry {
        emoji: "🎵",
        keywords: &["audio", "music", "sound"],
    },
];

pub(crate) fn open_node_tag_panel(
    app: &mut GraphBrowserApp,
    node_key: NodeKey,
    prefer_pane_anchor: bool,
) {
    app.workspace.graph_runtime.tag_panel_state = Some(TagPanelState {
        node_key,
        text_input: String::new(),
        icon_picker_open: false,
        icon_search_input: String::new(),
        prefer_pane_anchor,
        pending_icon_override: None,
    });
}

pub(crate) fn open_tag_panel_for_current_focus(
    app: &mut GraphBrowserApp,
    tiles_tree: &Tree<TileKind>,
    graph_surface_focused: bool,
    focused_hint: Option<NodeKey>,
) -> bool {
    if graph_surface_focused {
        if let Some(node_key) = app.get_single_selected_node() {
            open_node_tag_panel(app, node_key, false);
            return true;
        }
        return false;
    }

    if let Some(node_key) = app
        .workspace
        .graph_runtime
        .active_pane_rects
        .first()
        .map(|(_, nk, _)| *nk)
    {
        open_node_tag_panel(app, node_key, true);
        return true;
    }

    if let Some(node_key) = app.get_single_selected_node() {
        open_node_tag_panel(app, node_key, false);
        return true;
    }

    false
}

pub(crate) fn render_tag_panel(
    ctx: &Context,
    app: &mut GraphBrowserApp,
    tiles_tree: &Tree<TileKind>,
    graph_surface_focused: bool,
    focused_hint: Option<NodeKey>,
) {
    let Some(snapshot) = app.workspace.graph_runtime.tag_panel_state.clone() else {
        return;
    };

    if should_close_tag_panel(
        app,
        tiles_tree,
        graph_surface_focused,
        focused_hint,
        &snapshot,
    ) {
        app.workspace.graph_runtime.tag_panel_state = None;
        return;
    }

    let Some(node) = app.domain_graph().get_node(snapshot.node_key) else {
        app.workspace.graph_runtime.tag_panel_state = None;
        return;
    };

    let title = app
        .user_visible_node_title(snapshot.node_key)
        .unwrap_or_else(|| {
            if node.title.is_empty() {
                node.url().to_string()
            } else {
                node.title.clone()
            }
        });
    let current_tags = tags_for_node(app, &snapshot.node_key);
    let suggestions = ranked_tag_suggestions(app, snapshot.node_key, &snapshot.text_input);
    let warning = reserved_tag_warning(&snapshot.text_input);
    let theme_tokens = phase3_resolve_active_theme(app.default_registry_theme_id()).tokens;
    let mut open = true;
    let mut close_requested = ctx.input(|input| input.key_pressed(Key::Escape));
    let mut text_input = snapshot.text_input.clone();
    let mut icon_picker_open = snapshot.icon_picker_open;
    let mut icon_search_input = snapshot.icon_search_input.clone();
    let mut pending_icon_override = snapshot.pending_icon_override.clone();
    let mut pending_intents = Vec::new();
    let mut tag_for_icon_write: Option<String> = None;
    let window_pos = tag_panel_window_pos(app, tiles_tree, &snapshot);

    let response = Window::new(format!("Tags for {title}"))
        .id(egui::Id::new((
            "graphshell_tag_panel",
            snapshot.node_key.index(),
        )))
        .collapsible(false)
        .default_width(360.0)
        .default_pos(window_pos)
        .open(&mut open)
        .show(ctx, |ui| {
            ui.small("Current tags");
            if current_tags.is_empty() {
                ui.small("No tags yet.");
            } else {
                ui.horizontal_wrapped(|ui| {
                    for tag in &current_tags {
                        let label = semantic_tag_display_label(tag);
                        if ui.small_button(format!("{label} ×")).clicked() {
                            pending_intents.push(GraphIntent::UntagNode {
                                key: snapshot.node_key,
                                tag: tag.clone(),
                            });
                        }
                    }
                });
            }

            ui.separator();
            ui.small("Add tag");
            let response = ui.add(
                TextEdit::singleline(&mut text_input).hint_text("Add tag or semantic code..."),
            );
            let submit =
                response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
            ui.horizontal(|ui| {
                let picker_label = pending_icon_override
                    .as_ref()
                    .map(badge_icon_label)
                    .unwrap_or_else(|| "⊞".to_string());
                if ui.small_button(picker_label).clicked() {
                    icon_picker_open = !icon_picker_open;
                }
                if ui.small_button("Add").clicked() || submit {
                    if let Some(tag) = normalize_tag_entry_input(&text_input) {
                        tag_for_icon_write = Some(tag.clone());
                        pending_intents.push(GraphIntent::TagNode {
                            key: snapshot.node_key,
                            tag,
                        });
                        text_input.clear();
                    }
                }
                if ui.small_button("Close").clicked() {
                    close_requested = true;
                }
            });

            if icon_picker_open {
                ui.separator();
                ui.small("Icon picker");
                ui.add(
                    TextEdit::singleline(&mut icon_search_input).hint_text("Search emoji icons..."),
                );
                ui.horizontal_wrapped(|ui| {
                    if ui.small_button("None").clicked() {
                        pending_icon_override = None;
                        icon_picker_open = false;
                    }
                    for icon in matching_emoji_icons(&icon_search_input) {
                        let label = badge_icon_label(&icon);
                        if ui.small_button(label).clicked() {
                            pending_icon_override = Some(icon);
                            icon_picker_open = false;
                        }
                    }
                });
            }

            if let Some(warning) = warning.as_ref() {
                ui.label(
                    egui::RichText::new(warning)
                        .small()
                        .color(theme_tokens.command_notice),
                );
            }

            ui.separator();
            ui.small("Suggestions");
            if suggestions.is_empty() {
                ui.small("No suggestions yet.");
            } else {
                ui.horizontal_wrapped(|ui| {
                    for chip in &suggestions {
                        let button = egui::Button::new(egui::RichText::new(&chip.label).small());
                        if ui.add(button).clicked() {
                            tag_for_icon_write = Some(chip.query.clone());
                            pending_intents.push(GraphIntent::TagNode {
                                key: snapshot.node_key,
                                tag: chip.query.clone(),
                            });
                            text_input.clear();
                        }
                    }
                });
            }
        });

    let window_rect: Option<egui::Rect> = response.as_ref().map(|inner| inner.response.rect);
    if let Some(rect) = window_rect
        && ctx.input(|input| input.pointer.primary_clicked())
        && let Some(pointer) = ctx.input(|input| input.pointer.interact_pos())
        && !rect.contains(pointer)
    {
        close_requested = true;
    }

    if close_requested || !open {
        app.workspace.graph_runtime.tag_panel_state = None;
    } else if let Some(state) = app.workspace.graph_runtime.tag_panel_state.as_mut() {
        state.text_input = text_input;
        state.icon_picker_open = icon_picker_open;
        state.icon_search_input = icon_search_input;
        state.pending_icon_override = pending_icon_override.clone();
    }

    if !pending_intents.is_empty() {
        app.apply_reducer_intents(pending_intents);
        if let Some(tag) = tag_for_icon_write
            && !is_reserved_system_tag(&tag)
            && !tag.starts_with("udc:")
        {
            let _ = app.set_node_tag_icon_override(snapshot.node_key, &tag, pending_icon_override);
            if let Some(state) = app.workspace.graph_runtime.tag_panel_state.as_mut() {
                state.pending_icon_override = None;
                state.icon_search_input.clear();
            }
        }
    }
}

fn should_close_tag_panel(
    app: &GraphBrowserApp,
    tiles_tree: &Tree<TileKind>,
    graph_surface_focused: bool,
    focused_hint: Option<NodeKey>,
    state: &TagPanelState,
) -> bool {
    if app.domain_graph().get_node(state.node_key).is_none() {
        return true;
    }
    if graph_surface_focused {
        return app.get_single_selected_node() != Some(state.node_key);
    }
    if state.prefer_pane_anchor {
        return app
            .workspace
            .graph_runtime
            .active_pane_rects
            .first()
            .map(|(_, nk, _)| *nk)
            != Some(state.node_key);
    }
    false
}

fn tag_panel_window_pos(
    app: &GraphBrowserApp,
    _tiles_tree: &Tree<TileKind>,
    state: &TagPanelState,
) -> Pos2 {
    if state.prefer_pane_anchor
        && let Some((_, _, rect)) = app
            .workspace
            .graph_runtime
            .active_pane_rects
            .iter()
            .find(|(_, node_key, _)| *node_key == state.node_key)
    {
        return Pos2::new(rect.right() + 12.0, rect.top());
    }

    if let Some(view_id) = app.workspace.graph_runtime.focused_view
        && let Some(rect) = app
            .workspace
            .graph_runtime
            .graph_view_canvas_rects
            .get(&view_id)
    {
        return Pos2::new(
            (rect.right() - 372.0).max(rect.left() + 12.0),
            rect.top() + 12.0,
        );
    }

    Pos2::new(24.0, 96.0)
}

fn matching_emoji_icons(query: &str) -> Vec<BadgeIcon> {
    let normalized = query.trim().to_ascii_lowercase();
    EMOJI_ICON_CATALOG
        .iter()
        .filter(|entry| {
            normalized.is_empty()
                || entry.emoji.contains(&normalized)
                || entry
                    .keywords
                    .iter()
                    .any(|keyword| keyword.contains(&normalized))
        })
        .map(|entry| BadgeIcon::Emoji(entry.emoji.to_string()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{GraphBrowserApp, GraphViewId, SelectionUpdateMode};
    use crate::shell::desktop::workbench::pane_model::{GraphPaneRef, NodePaneState};
    use crate::shell::desktop::workbench::tile_kind::TileKind;
    use crate::shell::desktop::workbench::tile_view_ops::focus_pane;
    use egui::{Event, RawInput};
    use egui_tiles::{Tiles, Tree};
    use euclid::default::Point2D;

    fn node_tree(states: Vec<NodePaneState>) -> Tree<TileKind> {
        let mut tiles = Tiles::default();
        let children: Vec<_> = states
            .into_iter()
            .map(|state| tiles.insert_pane(TileKind::Node(state)))
            .collect();
        let root = tiles.insert_tab_tile(children);
        Tree::new("tag_panel_nodes", root, tiles)
    }

    /// Seed `graph_runtime.active_pane_rects` from the active tiles in the tree.
    /// Compositor and chrome callers read from this cache rather than scanning the
    /// tile tree, so tests must populate it explicitly.
    fn seed_active_pane_rects_from_tree(app: &mut GraphBrowserApp, tree: &Tree<TileKind>) {
        app.workspace.graph_runtime.active_pane_rects = tree
            .active_tiles()
            .into_iter()
            .filter_map(|tile_id| match tree.tiles.get(tile_id) {
                Some(egui_tiles::Tile::Pane(TileKind::Node(state))) => {
                    Some((state.pane_id, state.node, egui::Rect::ZERO))
                }
                _ => None,
            })
            .collect();
    }

    fn graph_tree() -> Tree<TileKind> {
        let mut tiles = Tiles::default();
        let graph = tiles.insert_pane(TileKind::Graph(GraphPaneRef::new(GraphViewId::new())));
        let root = tiles.insert_tab_tile(vec![graph]);
        Tree::new("tag_panel_graph", root, tiles)
    }

    #[test]
    fn open_node_tag_panel_resets_transient_inputs() {
        let mut app = GraphBrowserApp::new_for_testing();
        let node =
            app.add_node_and_sync("https://tags.example".to_string(), Point2D::new(0.0, 0.0));

        open_node_tag_panel(&mut app, node, true);

        let state = app
            .workspace
            .graph_runtime
            .tag_panel_state
            .as_ref()
            .expect("tag panel should open");
        assert_eq!(state.node_key, node);
        assert!(state.text_input.is_empty());
        assert!(state.icon_search_input.is_empty());
        assert!(state.prefer_pane_anchor);
        assert!(state.pending_icon_override.is_none());
    }

    #[test]
    fn matching_emoji_icons_filters_by_keyword() {
        let matches = matching_emoji_icons("science");
        assert!(
            matches
                .iter()
                .any(|icon| matches!(icon, BadgeIcon::Emoji(value) if value == "🔬"))
        );
    }

    #[test]
    fn open_tag_panel_for_current_focus_prefers_active_node_pane() {
        let mut app = GraphBrowserApp::new_for_testing();
        let node =
            app.add_node_and_sync("https://pane.example".to_string(), Point2D::new(0.0, 0.0));
        let tree = node_tree(vec![NodePaneState::for_node(node)]);
        seed_active_pane_rects_from_tree(&mut app, &tree);

        assert!(open_tag_panel_for_current_focus(
            &mut app, &tree, false, None
        ));

        let state = app
            .workspace
            .graph_runtime
            .tag_panel_state
            .as_ref()
            .expect("tag panel should open for active node pane");
        assert_eq!(state.node_key, node);
        assert!(state.prefer_pane_anchor);
    }

    #[test]
    fn open_tag_panel_for_current_focus_falls_back_to_selected_node() {
        let mut app = GraphBrowserApp::new_for_testing();
        let node = app.add_node_and_sync(
            "https://selected.example".to_string(),
            Point2D::new(0.0, 0.0),
        );
        app.update_focused_selection(vec![node], SelectionUpdateMode::Replace);
        let tree = graph_tree();

        assert!(open_tag_panel_for_current_focus(
            &mut app, &tree, false, None
        ));

        let state = app
            .workspace
            .graph_runtime
            .tag_panel_state
            .as_ref()
            .expect("tag panel should open for selected node");
        assert_eq!(state.node_key, node);
        assert!(!state.prefer_pane_anchor);
    }

    #[test]
    fn should_close_tag_panel_when_graph_selection_changes() {
        let mut app = GraphBrowserApp::new_for_testing();
        let node_a = app.add_node_and_sync(
            "https://selection-a.example".to_string(),
            Point2D::new(0.0, 0.0),
        );
        let node_b = app.add_node_and_sync(
            "https://selection-b.example".to_string(),
            Point2D::new(40.0, 0.0),
        );
        app.update_focused_selection(vec![node_a], SelectionUpdateMode::Replace);
        open_node_tag_panel(&mut app, node_a, false);
        let state = app
            .workspace
            .graph_runtime
            .tag_panel_state
            .clone()
            .expect("tag panel should open");
        let tree = graph_tree();

        assert!(!should_close_tag_panel(&app, &tree, true, None, &state));

        app.update_focused_selection(vec![node_b], SelectionUpdateMode::Replace);
        assert!(should_close_tag_panel(&app, &tree, true, None, &state));
    }

    #[test]
    fn should_close_tag_panel_when_pane_focus_moves() {
        let mut app = GraphBrowserApp::new_for_testing();
        let node_a =
            app.add_node_and_sync("https://pane-a.example".to_string(), Point2D::new(0.0, 0.0));
        let node_b = app.add_node_and_sync(
            "https://pane-b.example".to_string(),
            Point2D::new(40.0, 0.0),
        );
        let pane_a = NodePaneState::for_node(node_a);
        let pane_b = NodePaneState::for_node(node_b);
        let pane_a_id = pane_a.pane_id;
        let pane_b_id = pane_b.pane_id;
        let mut tree = node_tree(vec![pane_a, pane_b]);
        assert!(focus_pane(&mut tree, pane_a_id));
        seed_active_pane_rects_from_tree(&mut app, &tree);

        open_node_tag_panel(&mut app, node_a, true);
        let state = app
            .workspace
            .graph_runtime
            .tag_panel_state
            .clone()
            .expect("tag panel should open");

        assert!(!should_close_tag_panel(&app, &tree, false, None, &state));

        assert!(focus_pane(&mut tree, pane_b_id));
        seed_active_pane_rects_from_tree(&mut app, &tree);
        assert!(should_close_tag_panel(&app, &tree, false, None, &state));
    }

    #[test]
    fn render_tag_panel_closes_on_escape() {
        let mut app = GraphBrowserApp::new_for_testing();
        let node =
            app.add_node_and_sync("https://escape.example".to_string(), Point2D::new(0.0, 0.0));
        app.update_focused_selection(vec![node], SelectionUpdateMode::Replace);
        open_node_tag_panel(&mut app, node, false);
        let tree = graph_tree();
        let ctx = egui::Context::default();
        let mut raw = RawInput::default();
        raw.events.push(Event::Key {
            key: Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        });

        let _ = ctx.run(raw, |ctx| {
            render_tag_panel(ctx, &mut app, &tree, true, None);
        });

        assert!(app.workspace.graph_runtime.tag_panel_state.is_none());
    }
}
