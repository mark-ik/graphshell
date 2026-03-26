/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Canvas overlay passes: frame-affinity backdrops, highlighted-edge overlay,
//! hovered-edge tooltip, and hovered-node tooltip.

use crate::app::GraphBrowserApp;
use crate::graph::{
    EdgeFamily, EdgePayload, FrameLayoutHint, NodeLifecycle, RelationSelector, SemanticSubKind,
    SplitOrientation,
};
use crate::shell::desktop::runtime::registries::phase3_resolve_active_theme;
use crate::util::VersoAddress;
use egui::{Stroke, Ui, Vec2};
use egui_graphs::MetadataFrame;
use std::collections::BTreeSet;
use std::time::{Duration, UNIX_EPOCH};

// ── Edge helpers ──────────────────────────────────────────────────────────────

fn edge_family_rows(payload: &EdgePayload) -> Vec<String> {
    let mut rows = Vec::new();
    if payload.has_relation(RelationSelector::Semantic(SemanticSubKind::Hyperlink)) {
        rows.push("hyperlink | durable | graph.link_extraction".to_string());
    }
    if payload.has_relation(RelationSelector::Family(EdgeFamily::Traversal)) {
        rows.push("history | durable | runtime.navigation_log".to_string());
    }
    if payload.has_relation(RelationSelector::Semantic(SemanticSubKind::UserGrouped)) {
        rows.push("user-grouped | durable | user.explicit_grouping".to_string());
    }
    if let Some(arrangement) = payload.arrangement_data() {
        for sub_kind in &arrangement.sub_kinds {
            rows.push(format!(
                "arrangement/{} | {} | {}",
                sub_kind.as_tag(),
                sub_kind.durability().as_tag(),
                sub_kind.provenance()
            ));
        }
    }
    if rows.is_empty() {
        rows.push("unknown | session | runtime.edge_probe".to_string());
    }
    rows
}

pub(super) fn edge_endpoints_at_pointer(
    ui: &Ui,
    app: &GraphBrowserApp,
    metadata_id: egui::Id,
) -> Option<(crate::graph::NodeKey, crate::graph::NodeKey)> {
    let pointer = ui.input(|i| i.pointer.latest_pos())?;
    let state = app.workspace.graph_runtime.egui_state.as_ref()?;
    let meta = ui
        .ctx()
        .data_mut(|d| d.get_persisted::<MetadataFrame>(metadata_id))?;
    let edge_id = state.graph.edge_by_screen_pos(&meta, pointer)?;
    state.graph.edge_endpoints(edge_id)
}

// ── Overlay draw calls ────────────────────────────────────────────────────────

/// Render semi-transparent backdrop rectangles for all active frame-affinity
/// regions, positioned below graph nodes.
///
/// Uses the previous-frame [`MetadataFrame`] for canvas→screen coordinate
/// conversion.  Falls back to raw canvas coordinates if no metadata is
/// available yet (first frame after session start).
///
/// Spec: `layout_behaviors_and_physics_spec.md §4.6`
pub(super) fn draw_frame_affinity_backdrops(
    ui: &mut Ui,
    app: &GraphBrowserApp,
    metadata_id: egui::Id,
) {
    let regions = crate::graph::frame_affinity::derive_frame_affinity_regions(app.domain_graph());
    if regions.is_empty() {
        return;
    }

    let meta = ui
        .ctx()
        .data_mut(|d| d.get_persisted::<MetadataFrame>(metadata_id));

    let Some(_egui_state) = app.workspace.graph_runtime.egui_state.as_ref() else {
        return;
    };

    let painter = ui.painter().clone().with_layer_id(egui::LayerId::new(
        egui::Order::Middle,
        egui::Id::new("frame_affinity_backdrops"),
    ));

    for region in &regions {
        let Some(backdrop_rect) = frame_affinity_backdrop_rect(app, region, meta.as_ref()) else {
            continue;
        };

        let fill = egui::Color32::from_rgba_unmultiplied(
            region.color.r(),
            region.color.g(),
            region.color.b(),
            if frame_anchor_is_selected_or_current(app, region.frame_anchor) {
                42
            } else {
                30
            },
        );
        let stroke_color = egui::Color32::from_rgba_unmultiplied(
            region.color.r(),
            region.color.g(),
            region.color.b(),
            if frame_anchor_is_selected_or_current(app, region.frame_anchor) {
                170
            } else {
                80
            },
        );
        let stroke_width = if frame_anchor_is_selected_or_current(app, region.frame_anchor) {
            2.5
        } else {
            1.5
        };

        painter.rect(
            backdrop_rect,
            egui::CornerRadius::same(8),
            fill,
            egui::Stroke::new(stroke_width, stroke_color),
            egui::StrokeKind::Outside,
        );

        // Frame label — rendered at top-left of the backdrop rect.
        if let Some(label) = frame_anchor_label(app, region.frame_anchor) {
            let label_pos = backdrop_rect.left_top() + egui::Vec2::new(6.0, 4.0);
            painter.text(
                label_pos,
                egui::Align2::LEFT_TOP,
                label,
                egui::FontId::proportional(11.0),
                egui::Color32::from_rgba_unmultiplied(
                    region.color.r(),
                    region.color.g(),
                    region.color.b(),
                    180,
                ),
            );
        }

        if let Some(indicator) = frame_anchor_split_indicator(app, region.frame_anchor) {
            let indicator_padding = egui::vec2(6.0, 3.0);
            let indicator_galley = painter.layout_no_wrap(
                indicator,
                egui::FontId::proportional(10.0),
                egui::Color32::from_rgba_unmultiplied(255, 255, 255, 220),
            );
            let indicator_size = indicator_galley.size() + indicator_padding * 2.0;
            let indicator_rect = egui::Rect::from_min_size(
                egui::pos2(
                    backdrop_rect.right() - indicator_size.x - 6.0,
                    backdrop_rect.top() + 4.0,
                ),
                indicator_size,
            );
            painter.rect(
                indicator_rect,
                egui::CornerRadius::same(6),
                egui::Color32::from_rgba_unmultiplied(
                    region.color.r(),
                    region.color.g(),
                    region.color.b(),
                    110,
                ),
                egui::Stroke::new(
                    1.0,
                    egui::Color32::from_rgba_unmultiplied(
                        region.color.r(),
                        region.color.g(),
                        region.color.b(),
                        180,
                    ),
                ),
                egui::StrokeKind::Outside,
            );
            painter.galley(
                indicator_rect.center() - indicator_galley.size() * 0.5,
                indicator_galley,
                egui::Color32::from_rgba_unmultiplied(255, 255, 255, 220),
            );
        }
    }
}

fn frame_affinity_backdrop_rect(
    app: &GraphBrowserApp,
    region: &crate::graph::frame_affinity::FrameAffinityRegion,
    meta: Option<&MetadataFrame>,
) -> Option<egui::Rect> {
    let egui_state = app.workspace.graph_runtime.egui_state.as_ref()?;
    let positions: Vec<egui::Pos2> = region
        .members
        .iter()
        .filter_map(|&key| {
            let node = egui_state.graph.node(key)?;
            let canvas_pos = node.location();
            let screen_pos = meta
                .map(|m| m.canvas_to_screen_pos(canvas_pos))
                .unwrap_or(canvas_pos);
            Some(screen_pos)
        })
        .collect();

    if positions.len() < 2 {
        return None;
    }

    let (min_x, min_y, max_x, max_y) = positions.iter().fold(
        (f32::MAX, f32::MAX, f32::MIN, f32::MIN),
        |(min_x, min_y, max_x, max_y), p| {
            (
                min_x.min(p.x),
                min_y.min(p.y),
                max_x.max(p.x),
                max_y.max(p.y),
            )
        },
    );

    let padding = meta.map(|m| m.canvas_to_screen_size(40.0)).unwrap_or(40.0);

    Some(egui::Rect::from_min_max(
        egui::Pos2::new(min_x - padding, min_y - padding),
        egui::Pos2::new(max_x + padding, max_y + padding),
    ))
}

/// Return the display label for a frame anchor node.
///
/// Prefers the node's title; falls back to the URL host segment.
fn frame_anchor_label(app: &GraphBrowserApp, anchor: crate::graph::NodeKey) -> Option<String> {
    let node = app.domain_graph().get_node(anchor)?;
    if !node.title.is_empty() && node.title != node.url {
        return Some(node.title.clone());
    }
    // Fall back to URL host segment
    servo::ServoUrl::parse(&node.url).ok().and_then(|u| {
        u.host_str()
            .map(|h| h.trim_start_matches("www.").to_string())
    })
}

fn frame_layout_hint_indicator(hints: &[FrameLayoutHint]) -> Option<String> {
    match hints {
        [] => None,
        [FrameLayoutHint::SplitHalf { orientation, .. }] => Some(match orientation {
            SplitOrientation::Vertical => "||".to_string(),
            SplitOrientation::Horizontal => "=".to_string(),
        }),
        [FrameLayoutHint::SplitPamphlet { orientation, .. }] => Some(match orientation {
            SplitOrientation::Vertical => "|||".to_string(),
            SplitOrientation::Horizontal => "===".to_string(),
        }),
        [FrameLayoutHint::SplitTriptych { .. }] => Some("T".to_string()),
        [FrameLayoutHint::SplitQuartered { .. }] => Some("2x2".to_string()),
        _ => Some(format!("{} splits", hints.len())),
    }
}

fn frame_anchor_split_indicator(
    app: &GraphBrowserApp,
    anchor: crate::graph::NodeKey,
) -> Option<String> {
    let hints = app.domain_graph().frame_layout_hints(anchor)?;
    frame_layout_hint_indicator(hints)
}

fn frame_anchor_is_current_frame(app: &GraphBrowserApp, anchor: crate::graph::NodeKey) -> bool {
    let Some(frame_name) = app.current_frame_name() else {
        return false;
    };
    let frame_url = VersoAddress::frame(frame_name.to_string()).to_string();
    app.domain_graph()
        .get_node_by_url(&frame_url)
        .is_some_and(|(frame_key, _)| frame_key == anchor)
}

fn frame_anchor_is_selected_frame(app: &GraphBrowserApp, anchor: crate::graph::NodeKey) -> bool {
    let Some(frame_name) = app.selected_frame_name() else {
        return false;
    };
    let frame_url = VersoAddress::frame(frame_name.to_string()).to_string();
    app.domain_graph()
        .get_node_by_url(&frame_url)
        .is_some_and(|(frame_key, _)| frame_key == anchor)
}

fn frame_anchor_is_selected_or_current(
    app: &GraphBrowserApp,
    anchor: crate::graph::NodeKey,
) -> bool {
    frame_anchor_is_selected_frame(app, anchor) || frame_anchor_is_current_frame(app, anchor)
}

pub(super) fn frame_anchor_at_pointer(
    ui: &Ui,
    app: &GraphBrowserApp,
    metadata_id: egui::Id,
) -> Option<crate::graph::NodeKey> {
    let pointer = ui.input(|i| i.pointer.latest_pos())?;
    let regions = crate::graph::frame_affinity::derive_frame_affinity_regions(app.domain_graph());
    let meta = ui
        .ctx()
        .data_mut(|d| d.get_persisted::<MetadataFrame>(metadata_id));

    regions
        .into_iter()
        .filter_map(|region| {
            let rect = frame_affinity_backdrop_rect(app, &region, meta.as_ref())?;
            rect.contains(pointer)
                .then_some((region.frame_anchor, rect.width() * rect.height()))
        })
        .min_by(|(_, left_area), (_, right_area)| {
            left_area
                .partial_cmp(right_area)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(anchor, _)| anchor)
}

pub(super) fn draw_highlighted_edge_overlay(
    ui: &mut Ui,
    app: &GraphBrowserApp,
    _widget_id: egui::Id,
    metadata_id: egui::Id,
) {
    let theme_resolution = phase3_resolve_active_theme(app.default_registry_theme_id());
    let selection = theme_resolution.tokens.edge_tokens.selection;
    let Some((from, to)) = app.workspace.graph_runtime.highlighted_graph_edge else {
        return;
    };
    let Some(state) = app.workspace.graph_runtime.egui_state.as_ref() else {
        return;
    };
    let Some(from_node) = state.graph.node(from) else {
        return;
    };
    let Some(to_node) = state.graph.node(to) else {
        return;
    };
    let (from_screen, to_screen) = if let Some(meta) = ui
        .ctx()
        .data_mut(|d| d.get_persisted::<MetadataFrame>(metadata_id))
    {
        (
            meta.canvas_to_screen_pos(from_node.location()),
            meta.canvas_to_screen_pos(to_node.location()),
        )
    } else {
        (from_node.location(), to_node.location())
    };
    ui.painter().line_segment(
        [from_screen, to_screen],
        Stroke::new(6.0, selection.halo_color),
    );
    ui.painter().line_segment(
        [from_screen, to_screen],
        Stroke::new(5.0 + selection.width_delta, selection.foreground_color),
    );
    ui.painter().circle_filled(
        from_screen,
        6.0 + selection.width_delta,
        selection.foreground_color,
    );
    ui.painter().circle_filled(
        to_screen,
        6.0 + selection.width_delta,
        selection.foreground_color,
    );
}

pub(super) fn draw_hovered_edge_tooltip(
    ui: &Ui,
    app: &GraphBrowserApp,
    widget_id: egui::Id,
    metadata_id: egui::Id,
) {
    if app.workspace.graph_runtime.hovered_graph_node.is_some() {
        return;
    }
    let Some(pointer) = ui.input(|i| i.pointer.hover_pos()) else {
        return;
    };
    let Some((from, to)) = edge_endpoints_at_pointer(ui, app, metadata_id) else {
        return;
    };

    let ab_payload = app
        .domain_graph()
        .find_edge_key(from, to)
        .and_then(|k| app.domain_graph().get_edge(k));
    let ba_payload = app
        .domain_graph()
        .find_edge_key(to, from)
        .and_then(|k| app.domain_graph().get_edge(k));

    let ab_count = ab_payload.map(|p| p.traversals().len()).unwrap_or(0);
    let ba_count = ba_payload.map(|p| p.traversals().len()).unwrap_or(0);
    let total = ab_count + ba_count;
    if ab_payload.is_none() && ba_payload.is_none() {
        return;
    }

    let latest_ts = ab_payload
        .into_iter()
        .flat_map(|p| p.traversals().iter().map(|t| t.timestamp_ms))
        .chain(
            ba_payload
                .into_iter()
                .flat_map(|p| p.traversals().iter().map(|t| t.timestamp_ms)),
        )
        .max();

    let from_label = app
        .domain_graph()
        .get_node(from)
        .map(|n| n.title.as_str())
        .filter(|t| !t.is_empty())
        .or_else(|| app.domain_graph().get_node(from).map(|n| n.url.as_str()))
        .unwrap_or("unknown");
    let to_label = app
        .domain_graph()
        .get_node(to)
        .map(|n| n.title.as_str())
        .filter(|t| !t.is_empty())
        .or_else(|| app.domain_graph().get_node(to).map(|n| n.url.as_str()))
        .unwrap_or("unknown");

    let latest_text = latest_ts
        .and_then(|ms| {
            UNIX_EPOCH
                .checked_add(Duration::from_millis(ms))
                .and_then(|ts| ts.duration_since(UNIX_EPOCH).ok())
                .map(|d| format!("{}s", d.as_secs()))
        })
        .unwrap_or_else(|| "unknown".to_string());

    let mut family_rows: BTreeSet<String> = BTreeSet::new();
    for payload in [ab_payload, ba_payload].into_iter().flatten() {
        for row in edge_family_rows(payload) {
            family_rows.insert(row);
        }
    }

    egui::Area::new(widget_id.with("edge_hover_tooltip"))
        .order(egui::Order::Tooltip)
        .fixed_pos(pointer + Vec2::new(14.0, 14.0))
        .show(ui.ctx(), |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_max_width(320.0);
                ui.label(egui::RichText::new("Edge Inspection").strong());
                ui.label(format!("{from_label} <-> {to_label}"));
                ui.separator();
                if total > 0 {
                    ui.label(format!("{from_label} -> {to_label}: {ab_count}"));
                    ui.label(format!("{to_label} -> {from_label}: {ba_count}"));
                    ui.label(format!("Total traversals: {total}"));
                    ui.label(format!("Latest traversal: {latest_text}"));
                    ui.separator();
                }
                ui.label(egui::RichText::new("Family | Durability | Provenance").small());
                for row in family_rows {
                    ui.label(egui::RichText::new(row).small());
                }
            });
        });
}

pub(super) fn draw_hovered_node_tooltip(
    ui: &Ui,
    app: &GraphBrowserApp,
    widget_id: egui::Id,
    metadata_id: egui::Id,
) {
    fn compact_hover_node_label(node: &crate::graph::Node) -> String {
        let raw = if node.title.trim().is_empty() {
            node.url.trim()
        } else {
            node.title.trim()
        };
        if raw.is_empty() {
            return "Untitled node".to_string();
        }
        if raw.chars().count() <= 72 {
            return raw.to_string();
        }
        let shortened: String = raw.chars().take(71).collect();
        format!("{shortened}…")
    }

    let Some(key) = app.workspace.graph_runtime.hovered_graph_node else {
        return;
    };
    let Some(node) = app.domain_graph().get_node(key) else {
        return;
    };
    let pointer_pos = ui.input(|i| i.pointer.latest_pos());

    let lifecycle_text = if app.is_crash_blocked(key) {
        "Crashed".to_string()
    } else {
        match node.lifecycle {
            NodeLifecycle::Active => "Active".to_string(),
            NodeLifecycle::Warm => "Warm".to_string(),
            NodeLifecycle::Cold => "Cold".to_string(),
            NodeLifecycle::Tombstone => "Tombstone".to_string(),
        }
    };
    let last_visited_text = format_last_visited(node.last_visited);
    let workspace_memberships: Vec<String> =
        app.membership_for_node(node.id).iter().cloned().collect();
    let anchor = pointer_pos
        .or_else(|| {
            app.workspace
                .graph_runtime
                .egui_state
                .as_ref()
                .and_then(|state| {
                    state.graph.node(key).map(|n| {
                        if let Some(meta) = ui
                            .ctx()
                            .data_mut(|d| d.get_persisted::<MetadataFrame>(metadata_id))
                        {
                            meta.canvas_to_screen_pos(n.location())
                        } else {
                            n.location()
                        }
                    })
                })
        })
        .unwrap_or_else(|| ui.max_rect().center());

    egui::Area::new(widget_id.with("node_hover_tooltip"))
        .order(egui::Order::Tooltip)
        .fixed_pos(anchor + egui::vec2(14.0, 14.0))
        .interactable(false)
        .show(ui.ctx(), |ui| {
            let theme_tokens = phase3_resolve_active_theme(app.default_registry_theme_id()).tokens;
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_min_width(240.0);
                ui.strong(compact_hover_node_label(node));
                ui.label(
                    egui::RichText::new(format!("Last visited: {last_visited_text}"))
                        .small()
                        .color(theme_tokens.radial_chrome_text),
                );
                ui.label(
                    egui::RichText::new(format!("Lifecycle: {lifecycle_text}"))
                        .small()
                        .color(theme_tokens.radial_chrome_text),
                );
                if !workspace_memberships.is_empty() {
                    ui.separator();
                    ui.label(
                        egui::RichText::new(format!(
                            "Workspaces ({})",
                            workspace_memberships.len()
                        ))
                        .small()
                        .color(theme_tokens.command_notice),
                    );
                    for workspace in &workspace_memberships {
                        ui.label(
                            egui::RichText::new(format!("- {workspace}"))
                                .small()
                                .color(theme_tokens.radial_chrome_text),
                        );
                    }
                }
            });
        });
}

// ── Time formatting ───────────────────────────────────────────────────────────

pub(super) fn format_last_visited(last_visited: std::time::SystemTime) -> String {
    let now = std::time::SystemTime::now();
    format_last_visited_with_now(last_visited, now)
}

pub(super) fn format_last_visited_with_now(
    last_visited: std::time::SystemTime,
    now: std::time::SystemTime,
) -> String {
    let Ok(elapsed) = now.duration_since(last_visited) else {
        return "just now".to_string();
    };
    format_elapsed_ago(elapsed)
}

pub(super) fn format_elapsed_ago(elapsed: Duration) -> String {
    let secs = elapsed.as_secs();
    if secs < 5 {
        return "just now".to_string();
    }
    if secs < 60 {
        return format!("{secs}s ago");
    }
    if secs < 60 * 60 {
        return format!("{}m ago", secs / 60);
    }
    if secs < 60 * 60 * 24 {
        return format!("{}h ago", secs / (60 * 60));
    }
    if secs < 60 * 60 * 24 * 7 {
        return format!("{}d ago", secs / (60 * 60 * 24));
    }
    format!("{}w ago", secs / (60 * 60 * 24 * 7))
}

#[cfg(test)]
mod tests {
    use super::{frame_anchor_is_current_frame, frame_layout_hint_indicator};
    use crate::app::GraphBrowserApp;
    use crate::graph::{DominantEdge, FrameLayoutHint, SplitOrientation};
    use euclid::default::Point2D;

    #[test]
    fn frame_layout_hint_indicator_returns_none_for_empty_hint_list() {
        assert_eq!(frame_layout_hint_indicator(&[]), None);
    }

    #[test]
    fn frame_layout_hint_indicator_returns_triptych_token_for_single_triptych_hint() {
        let hints = vec![FrameLayoutHint::SplitTriptych {
            dominant: "dominant".to_string(),
            dominant_edge: DominantEdge::Left,
            wings: ["wing-a".to_string(), "wing-b".to_string()],
        }];

        assert_eq!(frame_layout_hint_indicator(&hints), Some("T".to_string()));
    }

    #[test]
    fn frame_layout_hint_indicator_returns_count_when_multiple_hints_exist() {
        let hints = vec![
            FrameLayoutHint::SplitHalf {
                first: "a".to_string(),
                second: "b".to_string(),
                orientation: SplitOrientation::Horizontal,
            },
            FrameLayoutHint::SplitQuartered {
                top_left: "a".to_string(),
                top_right: "b".to_string(),
                bottom_left: "c".to_string(),
                bottom_right: "d".to_string(),
            },
        ];

        assert_eq!(
            frame_layout_hint_indicator(&hints),
            Some("2 splits".to_string())
        );
    }

    #[test]
    fn frame_anchor_is_current_frame_matches_active_frame_anchor() {
        let mut app = GraphBrowserApp::new_for_testing();
        let node = app.add_node_and_sync(
            "https://active-frame.example".to_string(),
            Point2D::new(0.0, 0.0),
        );
        let mut tiles = egui_tiles::Tiles::default();
        let node_tile = tiles.insert_pane(
            crate::shell::desktop::workbench::tile_kind::TileKind::Node(node.into()),
        );
        let root = tiles.insert_tab_tile(vec![node_tile]);
        let tree = egui_tiles::Tree::new("active_frame_anchor", root, tiles);
        app.sync_named_workbench_frame_graph_representation("alpha", &tree);
        app.note_frame_activated("alpha", [node]);

        let frame_url = crate::util::VersoAddress::frame("alpha").to_string();
        let (frame_key, _) = app
            .domain_graph()
            .get_node_by_url(&frame_url)
            .expect("frame anchor should exist");

        assert!(frame_anchor_is_current_frame(&app, frame_key));
    }
}
