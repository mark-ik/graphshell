/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use egui::{Color32, Pos2, RichText, Sense, Stroke, StrokeKind, Ui, Vec2};
use std::collections::{HashMap, HashSet};

use crate::app::ViewGraphletPartition;
use crate::graph::NodeKey;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SwatchSourceScope {
    Graphlet,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SwatchLayoutProfile {
    LocalNeighborhood,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SwatchSizeClass {
    Compact,
    Regular,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SwatchDensityPolicy {
    pub preview_node_limit: usize,
    pub show_counts: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SwatchInteractionProfile {
    PreviewOnly,
    SelectAndOpenDetail,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SwatchHostOptions {
    pub size_class: SwatchSizeClass,
    pub card_size: Vec2,
    pub preview_height: f32,
}

#[derive(Debug, Clone)]
pub(crate) struct GraphSwatchSpec<'a> {
    pub source_scope: SwatchSourceScope,
    pub layout_profile: SwatchLayoutProfile,
    pub density_policy: SwatchDensityPolicy,
    pub interaction_profile: SwatchInteractionProfile,
    pub host_options: SwatchHostOptions,
    pub graphlet: &'a ViewGraphletPartition,
    pub title: String,
    pub summary: String,
    pub badge: Option<&'a str>,
    pub footer: String,
    pub emphasized: bool,
    pub hover_text: Option<&'a str>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GraphSwatchInteraction {
    SelectSource,
    OpenDetail,
}

pub(crate) fn render_graph_swatch_card(
    ui: &mut Ui,
    spec: &GraphSwatchSpec<'_>,
) -> Option<GraphSwatchInteraction> {
    let (card_rect, response) = ui.allocate_exact_size(spec.host_options.card_size, Sense::click());
    let response = match spec.interaction_profile {
        SwatchInteractionProfile::PreviewOnly => response,
        SwatchInteractionProfile::SelectAndOpenDetail => {
            response.on_hover_cursor(egui::CursorIcon::PointingHand)
        }
    };
    let response = if let Some(hover_text) = spec.hover_text {
        response.on_hover_text(hover_text)
    } else {
        response
    };

    if ui.is_rect_visible(card_rect) {
        let corner_radius = match spec.host_options.size_class {
            SwatchSizeClass::Compact => 6.0,
            SwatchSizeClass::Regular => 10.0,
        };
        let preview_min_width = match spec.host_options.size_class {
            SwatchSizeClass::Compact => 120.0,
            SwatchSizeClass::Regular => 160.0,
        };
        let fill = if spec.emphasized {
            Color32::from_rgb(46, 57, 73)
        } else {
            Color32::from_rgb(32, 37, 45)
        };
        let stroke = if spec.emphasized {
            Stroke::new(1.5, Color32::from_rgb(160, 205, 255))
        } else if response.hovered() {
            Stroke::new(1.25, Color32::from_gray(126))
        } else {
            Stroke::new(1.0, Color32::from_gray(90))
        };
        let painter = ui.painter();
        painter.rect_filled(card_rect, corner_radius, fill);
        painter.rect_stroke(card_rect, corner_radius, stroke, StrokeKind::Outside);

        let content_rect = card_rect.shrink2(Vec2::new(8.0, 8.0));
        ui.scope_builder(egui::UiBuilder::new().max_rect(content_rect), |ui| {
            ui.set_min_width(content_rect.width());
            ui.horizontal(|ui| {
                ui.label(RichText::new(&spec.title).small().strong());
                if let Some(badge) = spec.badge {
                    ui.separator();
                    ui.label(RichText::new(badge).small().weak());
                }
            });
            if spec.density_policy.show_counts {
                ui.label(RichText::new(&spec.summary).small().weak());
            }
            ui.add_space(4.0);

            let preview_size = Vec2::new(
                ui.available_width().max(preview_min_width),
                spec.host_options.preview_height,
            );
            let (preview_rect, _) = ui.allocate_exact_size(preview_size, Sense::hover());
            paint_graphlet_preview(
                ui,
                preview_rect,
                spec.graphlet,
                spec.layout_profile,
                spec.density_policy.preview_node_limit,
            );

            ui.label(RichText::new(&spec.footer).small().weak());
        });
    }

    match spec.interaction_profile {
        SwatchInteractionProfile::PreviewOnly => None,
        SwatchInteractionProfile::SelectAndOpenDetail => {
            if response.double_clicked() {
                Some(GraphSwatchInteraction::OpenDetail)
            } else if response.clicked() {
                Some(GraphSwatchInteraction::SelectSource)
            } else {
                None
            }
        }
    }
}

fn paint_graphlet_preview(
    ui: &Ui,
    rect: egui::Rect,
    graphlet: &ViewGraphletPartition,
    layout_profile: SwatchLayoutProfile,
    preview_node_limit: usize,
) {
    let painter = ui.painter();
    painter.rect_filled(rect, 6.0, Color32::from_rgb(24, 28, 34));
    painter.rect_stroke(
        rect,
        6.0,
        Stroke::new(1.0, Color32::from_gray(72)),
        StrokeKind::Outside,
    );

    let preview_nodes = graphlet_preview_node_keys(graphlet, preview_node_limit);
    let positions = graphlet_preview_positions(
        graphlet,
        &preview_nodes,
        rect.shrink2(Vec2::new(8.0, 8.0)),
        layout_profile,
    );
    if positions.is_empty() {
        return;
    }

    for (from, to) in &graphlet.internal_edges {
        let Some(from_pos) = positions.get(from) else {
            continue;
        };
        let Some(to_pos) = positions.get(to) else {
            continue;
        };
        painter.line_segment(
            [*from_pos, *to_pos],
            Stroke::new(1.0, Color32::from_rgba_unmultiplied(150, 176, 214, 92)),
        );
    }

    for node in &preview_nodes {
        let Some(position) = positions.get(node) else {
            continue;
        };
        let is_anchor = *node == graphlet.anchor;
        painter.circle_filled(
            *position,
            if is_anchor { 4.8 } else { 3.5 },
            if is_anchor {
                Color32::from_rgb(242, 199, 94)
            } else {
                Color32::from_rgb(176, 210, 255)
            },
        );
        painter.circle_stroke(
            *position,
            if is_anchor { 4.8 } else { 3.5 },
            Stroke::new(1.0, Color32::from_rgba_unmultiplied(18, 20, 22, 180)),
        );
    }
}

pub(crate) fn graphlet_preview_node_keys(
    graphlet: &ViewGraphletPartition,
    preview_node_limit: usize,
) -> Vec<NodeKey> {
    let mut nodes = vec![graphlet.anchor];
    nodes.extend(
        graphlet
            .members
            .iter()
            .copied()
            .filter(|node| *node != graphlet.anchor)
            .take(preview_node_limit.saturating_sub(1)),
    );
    nodes
}

fn graphlet_preview_positions(
    graphlet: &ViewGraphletPartition,
    preview_nodes: &[NodeKey],
    rect: egui::Rect,
    layout_profile: SwatchLayoutProfile,
) -> HashMap<NodeKey, Pos2> {
    match layout_profile {
        SwatchLayoutProfile::LocalNeighborhood => {
            local_neighborhood_positions(graphlet, preview_nodes, rect)
        }
    }
}

fn local_neighborhood_positions(
    graphlet: &ViewGraphletPartition,
    preview_nodes: &[NodeKey],
    rect: egui::Rect,
) -> HashMap<NodeKey, Pos2> {
    if preview_nodes.is_empty() {
        return HashMap::new();
    }

    let mut out = HashMap::new();
    let center = rect.center();
    out.insert(graphlet.anchor, center);

    let anchor_neighbors: HashSet<_> = graphlet
        .internal_edges
        .iter()
        .filter_map(|(from, to)| {
            if *from == graphlet.anchor {
                Some(*to)
            } else if *to == graphlet.anchor {
                Some(*from)
            } else {
                None
            }
        })
        .collect();

    let mut inner_ring = Vec::new();
    let mut outer_ring = Vec::new();
    for node in preview_nodes
        .iter()
        .copied()
        .filter(|node| *node != graphlet.anchor)
    {
        if anchor_neighbors.contains(&node) {
            inner_ring.push(node);
        } else {
            outer_ring.push(node);
        }
    }

    out.extend(ring_positions(
        &inner_ring,
        center,
        rect.width() * 0.24,
        rect.height() * 0.22,
        -std::f32::consts::FRAC_PI_2,
    ));
    out.extend(ring_positions(
        &outer_ring,
        center,
        rect.width() * 0.38,
        rect.height() * 0.34,
        -std::f32::consts::FRAC_PI_2 + 0.35,
    ));
    out
}

fn ring_positions(
    nodes: &[NodeKey],
    center: Pos2,
    radius_x: f32,
    radius_y: f32,
    angle_offset: f32,
) -> Vec<(NodeKey, Pos2)> {
    if nodes.is_empty() {
        return Vec::new();
    }

    let step = std::f32::consts::TAU / nodes.len() as f32;
    nodes
        .iter()
        .enumerate()
        .map(|(index, node)| {
            let angle = angle_offset + step * index as f32;
            (
                *node,
                Pos2::new(
                    center.x + radius_x * angle.cos(),
                    center.y + radius_y * angle.sin(),
                ),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graphlet_preview_node_keys_keep_anchor_visible_when_truncated() {
        let partition = ViewGraphletPartition {
            anchor: NodeKey::new(99),
            members: (0..24).map(NodeKey::new).collect(),
            internal_edges: vec![],
        };

        let preview_nodes = graphlet_preview_node_keys(&partition, 18);

        assert_eq!(preview_nodes.first().copied(), Some(partition.anchor));
        assert!(preview_nodes.contains(&partition.anchor));
        assert_eq!(preview_nodes.len(), 18);
    }
}
