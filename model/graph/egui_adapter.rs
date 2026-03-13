/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Adapter layer between GraphShell's Graph and egui_graphs for visualization.
//!
//! Converts the Graph's StableGraph to an egui_graphs::Graph each frame,
//! and reads back user interactions (drag, selection, double-click).

use super::apply::{GraphDelta, GraphDeltaResult};
use super::{
    EdgeKey, EdgeKind, EdgePayload, Graph, GraphDirection, GraphIndex, Node, NodeKey, NodeLifecycle,
};
use egui::epaint::{CircleShape, CubicBezierShape, TextShape};
use egui::{
    Color32, FontFamily, FontId, Pos2, Rect, Shape, Stroke, TextureHandle, TextureId, Vec2,
};
use egui_graphs::DrawContext;
use egui_graphs::{
    DefaultEdgeShape, DisplayEdge, DisplayNode, EdgeProps, NodeProps, to_graph_custom,
};
use image::load_from_memory;
use petgraph::visit::{EdgeRef, IntoEdgeReferences};
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use uuid::Uuid;

use crate::util::CoordBridge;

/// Type alias for the egui_graphs graph with our node/edge types
pub type EguiGraph = egui_graphs::Graph<
    Node,
    EdgePayload,
    GraphDirection,
    GraphIndex,
    GraphNodeShape,
    GraphEdgeShape,
>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
enum SelectionVisualRole {
    #[default]
    None,
    Primary,
    Secondary,
}

/// Node shape that renders favicon textures when available.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct GraphNodeShape {
    pos: Pos2,
    selected: bool,
    dragged: bool,
    hovered: bool,
    color: Option<Color32>,
    label_text: String,
    title_text: String,
    url_text: String,
    cached_host: Option<String>,
    radius: f32,
    thumbnail_png: Option<Vec<u8>>,
    thumbnail_width: u32,
    thumbnail_height: u32,
    thumbnail_hash: u64,
    #[serde(skip, default)]
    thumbnail_handle: Option<TextureHandle>,
    favicon_rgba: Option<Vec<u8>>,
    favicon_width: u32,
    favicon_height: u32,
    favicon_hash: u64,
    #[serde(skip, default)]
    favicon_handle: Option<TextureHandle>,
    #[serde(default)]
    workspace_membership_count: usize,
    #[serde(default)]
    workspace_membership_names: Vec<String>,
    #[serde(default)]
    semantic_badges: Vec<String>,
    #[serde(default)]
    semantic_badge_overflow: usize,
    #[serde(default)]
    is_archived: bool,
    #[serde(default)]
    is_pinned: bool,
    #[serde(default)]
    is_crashed: bool,
    #[serde(default)]
    selection_role: SelectionVisualRole,
}

impl From<NodeProps<Node>> for GraphNodeShape {
    fn from(node_props: NodeProps<Node>) -> Self {
        let mut shape = Self {
            pos: node_props.location(),
            selected: node_props.selected,
            dragged: node_props.dragged,
            hovered: node_props.hovered,
            color: node_props.color(),
            label_text: node_props.label.to_string(),
            title_text: node_props.payload.title.clone(),
            url_text: node_props.payload.url.clone(),
            cached_host: node_props.payload.cached_host.clone(),
            radius: 5.0,
            thumbnail_png: node_props.payload.thumbnail_png.clone(),
            thumbnail_width: node_props.payload.thumbnail_width,
            thumbnail_height: node_props.payload.thumbnail_height,
            thumbnail_hash: 0,
            thumbnail_handle: None,
            favicon_rgba: node_props.payload.favicon_rgba.clone(),
            favicon_width: node_props.payload.favicon_width,
            favicon_height: node_props.payload.favicon_height,
            favicon_hash: 0,
            favicon_handle: None,
            workspace_membership_count: 0,
            workspace_membership_names: Vec::new(),
            semantic_badges: Vec::new(),
            semantic_badge_overflow: 0,
            is_archived: false,
            is_pinned: node_props.payload.is_pinned,
            is_crashed: false,
            selection_role: if node_props.selected {
                SelectionVisualRole::Primary
            } else {
                SelectionVisualRole::None
            },
        };
        shape.thumbnail_hash = Self::hash_bytes(&shape.thumbnail_png);
        shape.favicon_hash = Self::hash_favicon(&shape.favicon_rgba);
        shape
    }
}

impl DisplayNode<Node, EdgePayload, GraphDirection, GraphIndex> for GraphNodeShape {
    fn is_inside(&self, pos: Pos2) -> bool {
        (pos - self.pos).length() <= self.radius
    }

    fn closest_boundary_point(&self, dir: Vec2) -> Pos2 {
        self.pos + dir.normalized() * self.radius
    }

    fn shapes(&mut self, ctx: &DrawContext) -> Vec<Shape> {
        let mut res = Vec::with_capacity(4);
        let circle_center = ctx.meta.canvas_to_screen_pos(self.pos);
        let circle_radius = ctx.meta.canvas_to_screen_size(self.radius);
        let color = self.effective_color(ctx);
        let stroke = self.effective_stroke(ctx);

        res.push(
            CircleShape {
                center: circle_center,
                radius: circle_radius,
                fill: color,
                stroke,
            }
            .into(),
        );

        if let Some(texture_id) = self.ensure_favicon_texture(ctx) {
            let size = Vec2::splat(circle_radius * 1.5);
            let rect = Rect::from_center_size(circle_center, size);
            let uv = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0));
            res.push(Shape::image(texture_id, rect, uv, Color32::WHITE));
        }
        if (self.selected || self.dragged || self.hovered)
            && let Some(texture_id) = self.ensure_thumbnail_texture(ctx)
        {
            let size = Vec2::new(circle_radius * 2.4, circle_radius * 1.8);
            let rect = Rect::from_center_size(circle_center, size);
            let uv = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0));
            res.push(Shape::image(texture_id, rect, uv, Color32::WHITE));
        }
        self.push_workspace_membership_badge(ctx, circle_center, circle_radius, &mut res);
        self.push_pinned_indicator(circle_center, circle_radius, &mut res);
        self.push_semantic_badges(ctx, circle_center, circle_radius, &mut res);
        self.push_secondary_selection_halo(circle_center, circle_radius, &mut res);

        let Some(label_text) = self.label_text_for_zoom(ctx.meta.zoom) else {
            return res;
        };

        let galley = self.label_galley(ctx, circle_radius, color, label_text);
        let label_pos = Pos2::new(
            center_x(galley.size().x, circle_center.x),
            circle_center.y - circle_radius * 2.0,
        );
        res.push(TextShape::new(label_pos, galley, color).into());
        res
    }

    fn update(&mut self, state: &NodeProps<Node>) {
        self.pos = state.location();
        self.selected = state.selected;
        self.dragged = state.dragged;
        self.hovered = state.hovered;
        self.label_text = state.label.to_string();
        self.title_text = state.payload.title.clone();
        self.url_text = state.payload.url.clone();
        self.cached_host = state.payload.cached_host.clone();
        self.color = state.color();
        self.is_pinned = state.payload.is_pinned;
        if !self.selected {
            self.selection_role = SelectionVisualRole::None;
        }

        let new_thumbnail = state.payload.thumbnail_png.clone();
        let new_thumbnail_hash = Self::hash_bytes(&new_thumbnail);
        if new_thumbnail_hash != self.thumbnail_hash
            || self.thumbnail_width != state.payload.thumbnail_width
            || self.thumbnail_height != state.payload.thumbnail_height
        {
            self.thumbnail_png = new_thumbnail;
            self.thumbnail_width = state.payload.thumbnail_width;
            self.thumbnail_height = state.payload.thumbnail_height;
            self.thumbnail_hash = new_thumbnail_hash;
            self.thumbnail_handle = None;
        }

        let new_rgba = state.payload.favicon_rgba.clone();
        let new_hash = Self::hash_favicon(&new_rgba);
        if new_hash != self.favicon_hash
            || self.favicon_width != state.payload.favicon_width
            || self.favicon_height != state.payload.favicon_height
        {
            self.favicon_rgba = new_rgba;
            self.favicon_width = state.payload.favicon_width;
            self.favicon_height = state.payload.favicon_height;
            self.favicon_hash = new_hash;
            self.favicon_handle = None;
        }
    }
}

impl GraphNodeShape {
    pub fn radius(&self) -> f32 {
        self.radius
    }

    pub fn workspace_membership_count(&self) -> usize {
        self.workspace_membership_count
    }

    pub fn workspace_badge_hit_rect_screen(
        &self,
        circle_center_screen: Pos2,
        circle_radius_screen: f32,
    ) -> Option<Rect> {
        if self.workspace_membership_count == 0 {
            return None;
        }
        let scale = (circle_radius_screen / 15.0).clamp(0.7, 1.8);
        let text = self.workspace_membership_count.to_string();
        let text_width = text.chars().count() as f32 * (6.0 * scale);
        let badge_size = Vec2::new(text_width + 8.0 * scale, 14.0 * scale);
        let badge_center = Pos2::new(
            circle_center_screen.x + circle_radius_screen * 0.95,
            circle_center_screen.y - circle_radius_screen * 0.95,
        );
        Some(Rect::from_center_size(badge_center, badge_size))
    }

    fn set_workspace_memberships(&mut self, names: Vec<String>) {
        self.workspace_membership_count = names.len();
        self.workspace_membership_names = names;
    }

    fn set_semantic_badges(&mut self, badges: Vec<String>) {
        const BADGE_SLOTS: usize = 3;

        if badges.len() > BADGE_SLOTS {
            self.semantic_badge_overflow = badges.len().saturating_sub(BADGE_SLOTS - 1);
            self.semantic_badges = badges.into_iter().take(BADGE_SLOTS - 1).collect();
        } else {
            self.semantic_badge_overflow = 0;
            self.semantic_badges = badges;
        }
    }

    fn set_archived(&mut self, archived: bool) {
        self.is_archived = archived;
    }

    fn set_selection_role(&mut self, role: SelectionVisualRole) {
        self.selection_role = role;
    }

    fn set_crashed(&mut self, crashed: bool) {
        self.is_crashed = crashed;
    }

    fn push_workspace_membership_badge(
        &self,
        ctx: &DrawContext,
        circle_center: Pos2,
        circle_radius: f32,
        shapes: &mut Vec<Shape>,
    ) {
        if self.workspace_membership_count == 0 {
            return;
        }

        let scale = (circle_radius / 15.0).clamp(0.7, 1.8);
        let badge_text = self.workspace_membership_count.to_string();
        let badge_font = FontId::new((9.5 * scale).clamp(8.0, 18.0), FontFamily::Monospace);
        let badge_galley = ctx
            .ctx
            .fonts_mut(|f| f.layout_no_wrap(badge_text, badge_font, Color32::from_gray(245)));
        let padding = Vec2::new(4.0 * scale, 2.0 * scale);
        let badge_size = badge_galley.size() + padding * 2.0;
        // Top-right keeps clear of top-center pin affordances.
        let badge_center = Pos2::new(
            circle_center.x + circle_radius * 0.95,
            circle_center.y - circle_radius * 0.95,
        );
        let badge_rect = Rect::from_center_size(badge_center, badge_size);
        shapes.push(Shape::rect_filled(
            badge_rect,
            4.0 * scale,
            Color32::from_rgba_unmultiplied(20, 30, 46, 224),
        ));
        let badge_pos = Pos2::new(badge_rect.min.x + padding.x, badge_rect.min.y + padding.y);
        shapes.push(TextShape::new(badge_pos, badge_galley, Color32::from_gray(245)).into());
    }

    fn push_pinned_indicator(
        &self,
        circle_center: Pos2,
        circle_radius: f32,
        shapes: &mut Vec<Shape>,
    ) {
        if !self.is_pinned {
            return;
        }
        let marker_center = Pos2::new(circle_center.x, circle_center.y - circle_radius * 0.9);
        let marker_radius = circle_radius.clamp(2.0, 5.0);
        shapes.push(
            CircleShape {
                center: marker_center,
                radius: marker_radius,
                fill: Color32::WHITE,
                stroke: Stroke::new(1.0, Color32::from_gray(40)),
            }
            .into(),
        );
    }

    fn push_semantic_badges(
        &self,
        ctx: &DrawContext,
        circle_center: Pos2,
        circle_radius: f32,
        shapes: &mut Vec<Shape>,
    ) {
        if self.semantic_badges.is_empty() && self.semantic_badge_overflow == 0 {
            return;
        }

        let scale = (circle_radius / 15.0).clamp(0.7, 1.8);
        let font = FontId::new((8.5 * scale).clamp(8.0, 16.0), FontFamily::Monospace);
        let mut next_left = if self.workspace_membership_count > 0 {
            circle_center.x + circle_radius * 0.35
        } else {
            circle_center.x + circle_radius * 1.1
        };
        let badge_center_y = circle_center.y - circle_radius * 0.95;

        for badge in &self.semantic_badges {
            let galley = ctx.ctx.fonts_mut(|f| {
                f.layout_no_wrap(badge.clone(), font.clone(), Color32::from_gray(245))
            });
            let padding = Vec2::new(4.0 * scale, 2.0 * scale);
            let badge_size = galley.size() + padding * 2.0;
            let badge_rect = Rect::from_min_size(
                Pos2::new(
                    next_left - badge_size.x,
                    badge_center_y - badge_size.y * 0.5,
                ),
                badge_size,
            );
            shapes.push(Shape::rect_filled(
                badge_rect,
                4.0 * scale,
                Color32::from_rgba_unmultiplied(34, 44, 64, 224),
            ));
            let badge_pos = Pos2::new(badge_rect.min.x + padding.x, badge_rect.min.y + padding.y);
            shapes.push(TextShape::new(badge_pos, galley, Color32::from_gray(245)).into());
            next_left = badge_rect.min.x - 4.0 * scale;
        }

        if self.semantic_badge_overflow > 0 {
            let overflow_text = format!("+{}", self.semantic_badge_overflow);
            let galley = ctx
                .ctx
                .fonts_mut(|f| f.layout_no_wrap(overflow_text, font, Color32::from_gray(245)));
            let padding = Vec2::new(4.0 * scale, 2.0 * scale);
            let badge_size = galley.size() + padding * 2.0;
            let badge_rect = Rect::from_min_size(
                Pos2::new(
                    next_left - badge_size.x,
                    badge_center_y - badge_size.y * 0.5,
                ),
                badge_size,
            );
            shapes.push(Shape::rect_filled(
                badge_rect,
                4.0 * scale,
                Color32::from_rgba_unmultiplied(24, 24, 24, 216),
            ));
            let badge_pos = Pos2::new(badge_rect.min.x + padding.x, badge_rect.min.y + padding.y);
            shapes.push(TextShape::new(badge_pos, galley, Color32::from_gray(245)).into());
        }
    }

    fn push_secondary_selection_halo(
        &self,
        circle_center: Pos2,
        circle_radius: f32,
        shapes: &mut Vec<Shape>,
    ) {
        if self.selection_role != SelectionVisualRole::Secondary || self.hovered || self.dragged {
            return;
        }
        shapes.push(
            CircleShape {
                center: circle_center,
                radius: circle_radius + 2.0,
                fill: Color32::TRANSPARENT,
                stroke: Stroke::new(2.0, Color32::from_rgb(255, 200, 100)),
            }
            .into(),
        );
    }

    fn ensure_thumbnail_texture(&mut self, ctx: &DrawContext) -> Option<TextureId> {
        if self.thumbnail_handle.is_none() {
            let thumbnail_png = self.thumbnail_png.as_ref()?;
            let image = load_from_memory(thumbnail_png).ok()?.to_rgba8();
            let width = image.width() as usize;
            let height = image.height() as usize;
            if width == 0 || height == 0 {
                return None;
            }
            if self.thumbnail_width > 0
                && self.thumbnail_height > 0
                && (self.thumbnail_width != width as u32 || self.thumbnail_height != height as u32)
            {
                return None;
            }
            let image = egui::ColorImage::from_rgba_unmultiplied([width, height], &image);
            let handle = ctx.ctx.load_texture(
                format!("graph-node-thumbnail-{}", self.thumbnail_hash),
                image,
                Default::default(),
            );
            self.thumbnail_handle = Some(handle);
        }
        self.thumbnail_handle.as_ref().map(|h| h.id())
    }

    fn effective_color(&self, ctx: &DrawContext) -> Color32 {
        if let Some(c) = self.projected_color() {
            return c;
        }
        let style = if self.selected || self.dragged || self.hovered {
            ctx.ctx.style().visuals.widgets.active
        } else {
            ctx.ctx.style().visuals.widgets.inactive
        };
        style.fg_stroke.color
    }

    fn projected_color(&self) -> Option<Color32> {
        if self.selection_role == SelectionVisualRole::Primary {
            return Some(self.apply_archive_tint(Color32::from_rgb(255, 200, 100)));
        }
        if self.is_crashed {
            return Some(self.apply_archive_tint(Color32::from_rgb(205, 112, 82)));
        }
        self.color.map(|color| self.apply_archive_tint(color))
    }

    fn effective_stroke(&self, ctx: &DrawContext) -> Stroke {
        let _ = ctx;
        if self.dragged {
            return Stroke::new(2.5, Color32::from_rgb(255, 220, 120));
        }
        if self.hovered {
            return Stroke::new(2.0, Color32::from_rgb(255, 170, 90));
        }
        if self.selection_role == SelectionVisualRole::Primary {
            return Stroke::new(1.8, Color32::from_rgb(255, 200, 120));
        }
        Stroke::new(1.0, Color32::from_gray(90))
    }

    fn apply_archive_tint(&self, color: Color32) -> Color32 {
        if self.is_archived {
            color.linear_multiply(0.45)
        } else {
            color
        }
    }

    fn label_galley(
        &self,
        ctx: &DrawContext,
        radius: f32,
        color: Color32,
        label_text: String,
    ) -> std::sync::Arc<egui::Galley> {
        // Guard against pathological zoom/scale values that can request enormous glyph atlases.
        let font_size = if radius.is_finite() {
            radius.clamp(6.0, 96.0)
        } else {
            12.0
        };
        ctx.ctx.fonts_mut(|f| {
            f.layout_no_wrap(
                label_text,
                FontId::new(font_size, FontFamily::Monospace),
                color,
            )
        })
    }

    fn label_text_for_zoom(&self, zoom: f32) -> Option<String> {
        Self::label_text_for_zoom_value(
            &self.title_text,
            &self.url_text,
            self.cached_host.as_deref(),
            &self.label_text,
            zoom,
        )
    }

    fn label_text_for_zoom_value(
        title: &str,
        url: &str,
        cached_host: Option<&str>,
        fallback: &str,
        zoom: f32,
    ) -> Option<String> {
        if zoom < 0.6 {
            return None;
        }
        if zoom <= 1.5 {
            if let Some(host) = cached_host
                && !host.is_empty()
            {
                return Some(host.to_string());
            }
            let candidate = if title.is_empty() { fallback } else { title };
            return Some(crate::util::truncate_with_ellipsis(candidate, 20));
        }

        if !title.is_empty() && title != url {
            Some(title.to_string())
        } else if !url.is_empty() {
            Some(url.to_string())
        } else if fallback.is_empty() {
            None
        } else {
            Some(fallback.to_string())
        }
    }

    fn ensure_favicon_texture(&mut self, ctx: &DrawContext) -> Option<TextureId> {
        if self.favicon_handle.is_none() {
            let rgba = self.favicon_rgba.as_ref()?;
            if self.favicon_width == 0 || self.favicon_height == 0 {
                return None;
            }

            let expected_len = self.favicon_width as usize * self.favicon_height as usize * 4;
            if rgba.len() != expected_len {
                return None;
            }

            let image = egui::ColorImage::from_rgba_unmultiplied(
                [self.favicon_width as usize, self.favicon_height as usize],
                rgba,
            );
            let handle = ctx.ctx.load_texture(
                format!("graph-node-favicon-{}", self.favicon_hash),
                image,
                Default::default(),
            );
            self.favicon_handle = Some(handle);
        }
        self.favicon_handle.as_ref().map(|h| h.id())
    }

    fn hash_favicon(data: &Option<Vec<u8>>) -> u64 {
        Self::hash_bytes(data)
    }

    fn hash_bytes(data: &Option<Vec<u8>>) -> u64 {
        let Some(bytes) = data else {
            return 0;
        };
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        bytes.hash(&mut hasher);
        hasher.finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum GraphEdgeVisualStyle {
    Hyperlink,
    History,
    UserGrouped,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum DominantDirectionCue {
    None,
    AlongEdge,
    AgainstEdge,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LogicalPairTraversalAggregate {
    total_count: usize,
    forward_count: usize,
    backward_count: usize,
    dominant_cue: DominantDirectionCue,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct GraphEdgeShape {
    default_impl: DefaultEdgeShape,
    style: GraphEdgeVisualStyle,
    dimmed: bool,
    hidden: bool,
    traversal_total_count: usize,
    dominant_direction_cue: DominantDirectionCue,
}

impl From<EdgeProps<EdgePayload>> for GraphEdgeShape {
    fn from(edge: EdgeProps<EdgePayload>) -> Self {
        let style = Self::style_from_payload(&edge.payload);
        Self {
            default_impl: DefaultEdgeShape::from(edge),
            style,
            dimmed: false,
            hidden: false,
            traversal_total_count: 0,
            dominant_direction_cue: DominantDirectionCue::None,
        }
    }
}

impl<
    N: Clone,
    Ty: petgraph::EdgeType,
    Ix: petgraph::stable_graph::IndexType,
    D: DisplayNode<N, EdgePayload, Ty, Ix>,
> DisplayEdge<N, EdgePayload, Ty, Ix, D> for GraphEdgeShape
{
    fn shapes(
        &mut self,
        start: &egui_graphs::Node<N, EdgePayload, Ty, Ix, D>,
        end: &egui_graphs::Node<N, EdgePayload, Ty, Ix, D>,
        ctx: &DrawContext,
    ) -> Vec<Shape> {
        if self.hidden {
            return Vec::new();
        }
        let (base_color, width) = self.style_stroke();
        let color = if self.dimmed {
            base_color.gamma_multiply(0.35)
        } else {
            base_color
        };
        if self.style == GraphEdgeVisualStyle::History {
            return self.dashed_shapes(start, end, ctx, color, width);
        }

        let mut shapes = self
            .default_impl
            .shapes(start, end, ctx)
            .into_iter()
            .map(|shape| restyle_edge_shape(shape, color, width))
            .collect::<Vec<_>>();
        self.append_direction_cue(&mut shapes, start, end, color, width);
        shapes
    }

    fn update(&mut self, state: &EdgeProps<EdgePayload>) {
        <DefaultEdgeShape as DisplayEdge<N, EdgePayload, Ty, Ix, D>>::update(
            &mut self.default_impl,
            state,
        );
        self.style = Self::style_from_payload(&state.payload);
    }

    fn is_inside(
        &self,
        start: &egui_graphs::Node<N, EdgePayload, Ty, Ix, D>,
        end: &egui_graphs::Node<N, EdgePayload, Ty, Ix, D>,
        pos: Pos2,
    ) -> bool {
        if self.hidden {
            return false;
        }
        self.default_impl.is_inside(start, end, pos)
    }
}

impl GraphEdgeShape {
    pub(crate) fn set_dimmed(&mut self, dimmed: bool) {
        self.dimmed = dimmed;
    }

    fn set_hidden(&mut self, hidden: bool) {
        self.hidden = hidden;
    }

    fn configure_logical_pair(
        &mut self,
        style: GraphEdgeVisualStyle,
        aggregate: LogicalPairTraversalAggregate,
    ) {
        self.style = style;
        self.traversal_total_count = aggregate.total_count;
        self.dominant_direction_cue = aggregate.dominant_cue;
    }

    fn style_stroke(&self) -> (Color32, f32) {
        let traversal_bonus = if self.style == GraphEdgeVisualStyle::History {
            Self::traversal_width_bonus(self.traversal_total_count)
        } else {
            0.0
        };
        match self.style {
            GraphEdgeVisualStyle::Hyperlink => (Color32::from_gray(160), 1.4 + traversal_bonus),
            GraphEdgeVisualStyle::History => {
                (Color32::from_rgb(120, 180, 210), 1.8 + traversal_bonus)
            }
            GraphEdgeVisualStyle::UserGrouped => (Color32::from_rgb(236, 171, 64), 3.0),
        }
    }

    fn style_from_payload(payload: &EdgePayload) -> GraphEdgeVisualStyle {
        if payload.has_kind(EdgeKind::UserGrouped) {
            GraphEdgeVisualStyle::UserGrouped
        } else if payload.has_kind(EdgeKind::TraversalDerived) {
            GraphEdgeVisualStyle::History
        } else {
            GraphEdgeVisualStyle::Hyperlink
        }
    }

    fn traversal_width_bonus(total_count: usize) -> f32 {
        if total_count == 0 {
            0.0
        } else {
            ((total_count as f32).sqrt() * 0.35).min(2.5)
        }
    }

    fn dominant_direction_from_counts(
        forward_count: usize,
        backward_count: usize,
        threshold_ratio: f32,
    ) -> DominantDirectionCue {
        let total = forward_count + backward_count;
        if total == 0 {
            return DominantDirectionCue::None;
        }
        let forward_ratio = forward_count as f32 / total as f32;
        let backward_ratio = backward_count as f32 / total as f32;
        if forward_ratio > threshold_ratio {
            DominantDirectionCue::AlongEdge
        } else if backward_ratio > threshold_ratio {
            DominantDirectionCue::AgainstEdge
        } else {
            DominantDirectionCue::None
        }
    }

    fn append_direction_cue<
        N: Clone,
        Ty: petgraph::EdgeType,
        Ix: petgraph::stable_graph::IndexType,
        D: DisplayNode<N, EdgePayload, Ty, Ix>,
    >(
        &self,
        shapes: &mut Vec<Shape>,
        start: &egui_graphs::Node<N, EdgePayload, Ty, Ix, D>,
        end: &egui_graphs::Node<N, EdgePayload, Ty, Ix, D>,
        color: Color32,
        width: f32,
    ) {
        if self.dominant_direction_cue == DominantDirectionCue::None {
            return;
        }
        if start.id() == end.id() {
            return;
        }
        let (arrow_from, arrow_to) = match self.dominant_direction_cue {
            DominantDirectionCue::AlongEdge => (start.location(), end.location()),
            DominantDirectionCue::AgainstEdge => (end.location(), start.location()),
            DominantDirectionCue::None => return,
        };
        let vec = arrow_to - arrow_from;
        let len = vec.length();
        if len <= f32::EPSILON {
            return;
        }
        let dir = vec / len;
        let tip = arrow_to - dir * (8.0 + width * 1.5);
        let perp = egui::vec2(-dir.y, dir.x);
        let head_len = 7.0 + width;
        let head_half = 4.0 + width * 0.4;
        let left = tip - dir * head_len + perp * head_half;
        let right = tip - dir * head_len - perp * head_half;
        let stroke = Stroke::new(width.max(1.2), color);
        shapes.push(Shape::line_segment([tip, left], stroke));
        shapes.push(Shape::line_segment([tip, right], stroke));
    }

    fn dashed_shapes<
        N: Clone,
        Ty: petgraph::EdgeType,
        Ix: petgraph::stable_graph::IndexType,
        D: DisplayNode<N, EdgePayload, Ty, Ix>,
    >(
        &self,
        start: &egui_graphs::Node<N, EdgePayload, Ty, Ix, D>,
        end: &egui_graphs::Node<N, EdgePayload, Ty, Ix, D>,
        ctx: &DrawContext,
        color: Color32,
        width: f32,
    ) -> Vec<Shape> {
        if start.id() == end.id() {
            return self
                .default_impl
                .clone()
                .shapes(start, end, ctx)
                .into_iter()
                .map(|shape| restyle_edge_shape(shape, color, width))
                .collect();
        }
        let dir = (end.location() - start.location()).normalized();
        let start_connector = start.display().closest_boundary_point(dir);
        let end_connector = end.display().closest_boundary_point(-dir);
        let screen_start = ctx.meta.canvas_to_screen_pos(start_connector);
        let screen_end = ctx.meta.canvas_to_screen_pos(end_connector);
        let vec = screen_end - screen_start;
        let total = vec.length();
        if total <= f32::EPSILON {
            return Vec::new();
        }
        let unit = vec / total;
        let dash = 8.0_f32;
        let gap = 6.0_f32;
        let mut shapes = Vec::new();
        let mut traveled = 0.0_f32;
        let stroke = Stroke::new(width, color);
        while traveled < total {
            let seg_start = screen_start + unit * traveled;
            let seg_end = screen_start + unit * (traveled + dash).min(total);
            shapes.push(Shape::line_segment([seg_start, seg_end], stroke));
            traveled += dash + gap;
        }
        shapes
    }
}

#[cfg(test)]
impl GraphEdgeShape {
    fn hidden(&self) -> bool {
        self.hidden
    }
}

fn logical_pair_key(a: NodeKey, b: NodeKey) -> (NodeKey, NodeKey) {
    if a.index() <= b.index() {
        (a, b)
    } else {
        (b, a)
    }
}

fn aggregate_logical_pair_traversals(
    graph: &Graph,
    a: NodeKey,
    b: NodeKey,
) -> (GraphEdgeVisualStyle, LogicalPairTraversalAggregate) {
    let ab_key = graph.find_edge_key(a, b);
    let ba_key = graph.find_edge_key(b, a);
    let ab_payload = ab_key.and_then(|k| graph.get_edge(k));
    let ba_payload = ba_key.and_then(|k| graph.get_edge(k));

    let ab_total = ab_payload
        .map(|p| p.metrics().total_navigations as usize)
        .unwrap_or(0);
    let ba_total = ba_payload
        .map(|p| p.metrics().total_navigations as usize)
        .unwrap_or(0);
    let total_count = ab_total + ba_total;
    let dominant_source = ab_payload.or(ba_payload);
    let forward_count = dominant_source
        .map(|p| p.metrics().forward_navigations as usize)
        .unwrap_or(0);
    let backward_count = dominant_source
        .map(|p| p.metrics().backward_navigations as usize)
        .unwrap_or(0);
    let style = if ab_payload.is_some_and(|p| p.has_kind(EdgeKind::UserGrouped))
        || ba_payload.is_some_and(|p| p.has_kind(EdgeKind::UserGrouped))
    {
        GraphEdgeVisualStyle::UserGrouped
    } else if ab_payload.is_some_and(|p| p.has_kind(EdgeKind::TraversalDerived))
        || ba_payload.is_some_and(|p| p.has_kind(EdgeKind::TraversalDerived))
    {
        GraphEdgeVisualStyle::History
    } else {
        GraphEdgeVisualStyle::Hyperlink
    };
    let aggregate = LogicalPairTraversalAggregate {
        total_count,
        forward_count,
        backward_count,
        dominant_cue: GraphEdgeShape::dominant_direction_from_counts(
            forward_count,
            backward_count,
            0.60,
        ),
    };
    (style, aggregate)
}

fn restyle_edge_shape(shape: Shape, color: Color32, width: f32) -> Shape {
    match shape {
        Shape::LineSegment { points, stroke: _ } => {
            Shape::line_segment(points, Stroke::new(width, color))
        }
        Shape::CubicBezier(cubic) => Shape::CubicBezier(CubicBezierShape {
            stroke: Stroke::new(width, color).into(),
            fill: Color32::TRANSPARENT,
            ..cubic
        }),
        other => other,
    }
}

fn center_x(width: f32, center_x: f32) -> f32 {
    center_x - width / 2.0
}

/// Converted egui_graphs representation.
pub struct EguiGraphState {
    /// The egui_graphs graph ready for rendering
    pub graph: EguiGraph,
    /// True when this retained graph mirrors the full domain graph rather than
    /// a filtered or culled projection.
    pub represents_full_graph: bool,
}

impl EguiGraphState {
    /// Build an egui_graphs::Graph directly from our Graph's StableGraph.
    ///
    /// Sets node positions, labels, colors, and selection state
    /// based on current graph data.
    pub fn from_graph(graph: &Graph, selected_nodes: &HashSet<NodeKey>) -> Self {
        Self::from_graph_with_visual_state(graph, selected_nodes, None, &HashSet::new())
    }

    pub fn from_graph_with_visual_state(
        graph: &Graph,
        selected_nodes: &HashSet<NodeKey>,
        primary_selected: Option<NodeKey>,
        crashed_nodes: &HashSet<NodeKey>,
    ) -> Self {
        Self::from_graph_with_visual_state_projection(
            graph,
            selected_nodes,
            primary_selected,
            crashed_nodes,
            true,
        )
    }

    pub fn from_graph_with_visual_state_projection(
        graph: &Graph,
        selected_nodes: &HashSet<NodeKey>,
        primary_selected: Option<NodeKey>,
        crashed_nodes: &HashSet<NodeKey>,
        represents_full_graph: bool,
    ) -> Self {
        let mut egui_graph: EguiGraph = to_graph_custom(
            &graph.inner,
            |node: &mut egui_graphs::Node<
                Node,
                EdgePayload,
                GraphDirection,
                GraphIndex,
                GraphNodeShape,
            >| {
                // Extract all data from payload before any mutations
                let position = node.payload().projected_position();
                let title = node.payload().title.clone();
                let lifecycle = node.payload().lifecycle;

                // Seed position from app graph state
                node.set_location(position.to_pos2());

                // Keep full label source; zoom tiers are handled in GraphNodeShape.
                let label = if title.is_empty() {
                    node.payload().url.clone()
                } else {
                    title.clone()
                };
                node.set_label(label);

                // Set color based on lifecycle.
                let color = match lifecycle {
                    NodeLifecycle::Active => Color32::from_rgb(100, 200, 255),
                    NodeLifecycle::Warm => Color32::from_rgb(120, 170, 205),
                    NodeLifecycle::Cold => Color32::from_rgb(140, 140, 165),
                    NodeLifecycle::Tombstone => Color32::from_rgb(96, 96, 96),
                };
                node.set_color(color);

                // Set radius based on lifecycle
                let radius = match lifecycle {
                    NodeLifecycle::Active => 18.0,
                    NodeLifecycle::Warm => 16.5,
                    NodeLifecycle::Cold => 15.0,
                    NodeLifecycle::Tombstone => 14.0,
                };
                node.display_mut().radius = radius;

                // Selection is projected from app state after graph conversion.
                node.set_selected(false);
            },
            |edge| {
                edge.set_label(String::new());
            },
        );

        // Project app selection onto egui nodes.
        for key in selected_nodes {
            if let Some(node) = egui_graph.node_mut(*key) {
                node.set_selected(true);
                let is_primary = primary_selected == Some(*key);
                node.display_mut().set_selection_role(if is_primary {
                    SelectionVisualRole::Primary
                } else {
                    SelectionVisualRole::Secondary
                });
                if is_primary {
                    node.set_color(Color32::from_rgb(255, 200, 100));
                }
            }
        }

        for key in crashed_nodes {
            if let Some(node) = egui_graph.node_mut(*key) {
                node.display_mut().set_crashed(true);
            }
        }

        refresh_logical_pair_edge_display(graph, &mut egui_graph);

        Self {
            graph: egui_graph,
            represents_full_graph,
        }
    }

    /// Build graph adapter state with optional workspace membership metadata.
    pub fn from_graph_with_memberships(
        graph: &Graph,
        selected_nodes: &HashSet<NodeKey>,
        primary_selected: Option<NodeKey>,
        crashed_nodes: &HashSet<NodeKey>,
        memberships_by_uuid: &HashMap<Uuid, Vec<String>>,
        semantic_badges_by_key: &HashMap<NodeKey, Vec<String>>,
        archived_nodes: &HashSet<NodeKey>,
    ) -> Self {
        Self::from_graph_with_memberships_projection(
            graph,
            selected_nodes,
            primary_selected,
            crashed_nodes,
            memberships_by_uuid,
            semantic_badges_by_key,
            archived_nodes,
            true,
        )
    }

    pub fn from_graph_with_memberships_projection(
        graph: &Graph,
        selected_nodes: &HashSet<NodeKey>,
        primary_selected: Option<NodeKey>,
        crashed_nodes: &HashSet<NodeKey>,
        memberships_by_uuid: &HashMap<Uuid, Vec<String>>,
        semantic_badges_by_key: &HashMap<NodeKey, Vec<String>>,
        archived_nodes: &HashSet<NodeKey>,
        represents_full_graph: bool,
    ) -> Self {
        let mut state = Self::from_graph_with_visual_state_projection(
            graph,
            selected_nodes,
            primary_selected,
            crashed_nodes,
            represents_full_graph,
        );
        for (key, node) in graph.nodes() {
            if let Some(egui_node) = state.graph.node_mut(key) {
                egui_node.display_mut().set_workspace_memberships(
                    memberships_by_uuid
                        .get(&node.id)
                        .cloned()
                        .unwrap_or_default(),
                );
                egui_node.display_mut().set_semantic_badges(
                    semantic_badges_by_key
                        .get(&key)
                        .cloned()
                        .unwrap_or_default(),
                );
                egui_node
                    .display_mut()
                    .set_archived(archived_nodes.contains(&key));
            }
        }
        state
    }

    pub fn sync_from_delta(
        &mut self,
        graph: &Graph,
        delta: &GraphDelta,
        result: &GraphDeltaResult,
    ) -> bool {
        if !self.represents_full_graph {
            return false;
        }

        match (delta, result) {
            (GraphDelta::AddNode { .. }, GraphDeltaResult::NodeAdded(_))
            | (
                GraphDelta::ReplayAddNodeWithIdIfMissing { .. },
                GraphDeltaResult::NodeMaybeAdded(_),
            )
            | (GraphDelta::SetNodeTitle { .. }, GraphDeltaResult::NodeMetadataUpdated(_))
            | (GraphDelta::SetNodeUrl { .. }, GraphDeltaResult::NodeUrlUpdated(_))
            | (GraphDelta::SetNodeThumbnail { .. }, GraphDeltaResult::NodeMetadataUpdated(_))
            | (GraphDelta::SetNodeFavicon { .. }, GraphDeltaResult::NodeMetadataUpdated(_))
            | (GraphDelta::SetNodeMimeHint { .. }, GraphDeltaResult::NodeMetadataUpdated(_))
            | (GraphDelta::SetNodeAddressKind { .. }, GraphDeltaResult::NodeMetadataUpdated(_))
            | (GraphDelta::SetNodePinned { .. }, GraphDeltaResult::NodeMetadataUpdated(_)) => {
                self.sync_nodes_from_graph(graph);
            }
            (GraphDelta::RemoveNode { .. }, GraphDeltaResult::NodeRemoved(_))
            | (GraphDelta::ReplayRemoveNodeById { .. }, GraphDeltaResult::NodeRemoved(_)) => {
                self.sync_nodes_from_graph(graph);
                self.sync_edges_from_graph(graph);
            }
            (GraphDelta::AddEdge { .. }, GraphDeltaResult::EdgeAdded(_))
            | (GraphDelta::ReplayAddEdgeByIds { .. }, GraphDeltaResult::EdgeAdded(_))
            | (GraphDelta::ReplayRemoveEdgesByIds { .. }, GraphDeltaResult::EdgesRemoved(_))
            | (GraphDelta::RemoveEdges { .. }, GraphDeltaResult::EdgesRemoved(_))
            | (GraphDelta::AppendTraversal { .. }, GraphDeltaResult::TraversalAppended(_)) => {
                self.sync_edges_from_graph(graph);
            }
            _ => return false,
        }

        true
    }

    fn sync_nodes_from_graph(&mut self, graph: &Graph) {
        let stale_keys: Vec<_> = self
            .graph
            .nodes_iter()
            .map(|(key, _)| key)
            .filter(|key| graph.get_node(*key).is_none())
            .collect();
        for key in stale_keys {
            let _ = self.graph.remove_node(key);
        }

        for (key, node) in graph.nodes() {
            sync_egui_node_from_domain(&mut self.graph, key, node);
        }
    }

    fn sync_edges_from_graph(&mut self, graph: &Graph) {
        let stale_keys: Vec<_> = self
            .graph
            .edges_iter()
            .map(|(key, _)| key)
            .filter(|key| graph.get_edge(*key).is_none())
            .collect();
        for key in stale_keys {
            let _ = self.graph.remove_edge(key);
        }

        for edge in graph.inner.edge_references() {
            sync_egui_edge_from_domain(
                &mut self.graph,
                edge.id(),
                edge.source(),
                edge.target(),
                edge.weight(),
            );
        }

        refresh_logical_pair_edge_display(graph, &mut self.graph);
    }

    /// Validate that a graph key exists in the retained egui graph.
    pub fn get_key(&self, idx: NodeKey) -> Option<NodeKey> {
        self.graph.node(idx).map(|_| idx)
    }
}

#[cfg(test)]
impl EguiGraphState {
    /// Test helper: validate that a node key exists in the retained egui graph.
    fn get_index(&self, key: NodeKey) -> Option<NodeKey> {
        self.graph.node(key).map(|_| key)
    }
}

fn sync_egui_node_from_domain(egui_graph: &mut EguiGraph, key: NodeKey, node: &Node) {
    let position = node.projected_position();
    let label = if node.title.is_empty() {
        node.url.clone()
    } else {
        node.title.clone()
    };
    let color = match node.lifecycle {
        NodeLifecycle::Active => Color32::from_rgb(100, 200, 255),
        NodeLifecycle::Warm => Color32::from_rgb(120, 170, 205),
        NodeLifecycle::Cold => Color32::from_rgb(140, 140, 165),
        NodeLifecycle::Tombstone => Color32::from_rgb(96, 96, 96),
    };
    let radius = match node.lifecycle {
        NodeLifecycle::Active => 18.0,
        NodeLifecycle::Warm => 16.5,
        NodeLifecycle::Cold => 15.0,
        NodeLifecycle::Tombstone => 14.0,
    };

    if let Some(egui_node) = egui_graph.node_mut(key) {
        *egui_node.payload_mut() = node.clone();
        egui_node.set_location(position.to_pos2());
        egui_node.set_label(label);
        egui_node.set_color(color);
        egui_node.display_mut().radius = radius;
        return;
    }

    let created = egui_graph.add_node_custom(node.clone(), |egui_node| {
        egui_node.set_location(position.to_pos2());
        egui_node.set_label(label);
        egui_node.set_color(color);
        egui_node.display_mut().radius = radius;
        egui_node.set_selected(false);
    });
    debug_assert_eq!(created, key);
}

fn sync_egui_edge_from_domain(
    egui_graph: &mut EguiGraph,
    edge_key: EdgeKey,
    from: NodeKey,
    to: NodeKey,
    payload: &EdgePayload,
) {
    if let Some(edge) = egui_graph.edge_mut(edge_key) {
        *edge.payload_mut() = payload.clone();
        edge.set_label(String::new());
        return;
    }

    let created = egui_graph.add_edge_custom(from, to, payload.clone(), |edge| {
        edge.set_label(String::new());
    });
    debug_assert_eq!(created, edge_key);
}

fn refresh_logical_pair_edge_display(graph: &Graph, egui_graph: &mut EguiGraph) {
    let edge_ids: Vec<EdgeKey> = graph
        .inner
        .edge_references()
        .map(|edge| edge.id())
        .collect();
    let mut processed_pairs = HashSet::new();
    for edge_id in edge_ids {
        let Some((from, to)) = graph.inner.edge_endpoints(edge_id) else {
            continue;
        };
        if from == to {
            continue;
        }
        let pair = logical_pair_key(from, to);
        if !processed_pairs.insert(pair) {
            if let Some(edge) = egui_graph.edge_mut(edge_id) {
                edge.display_mut().set_hidden(true);
            }
            continue;
        }

        let (a, b) = pair;
        let (style, aggregate) = aggregate_logical_pair_traversals(graph, a, b);

        if (from, to) != pair
            && let Some(edge) = egui_graph.edge_mut(edge_id)
        {
            edge.display_mut().set_hidden(true);
        }
        let canonical_edge_id = if (from, to) == pair {
            edge_id
        } else if let Some(id) = graph.find_edge_key(a, b) {
            id
        } else {
            edge_id
        };
        if let Some(edge) = egui_graph.edge_mut(canonical_edge_id) {
            edge.display_mut().set_hidden(false);
            edge.display_mut().configure_logical_pair(style, aggregate);
        }

        if let Some(reverse_edge_id) = graph.find_edge_key(b, a)
            && reverse_edge_id != canonical_edge_id
            && let Some(edge) = egui_graph.edge_mut(reverse_edge_id)
        {
            edge.display_mut().set_hidden(true);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::EdgeType;
    use egui::Color32;
    use euclid::default::Point2D;

    #[test]
    fn test_egui_adapter_empty_graph() {
        let graph = Graph::new();
        let selected_nodes = HashSet::new();
        let state = EguiGraphState::from_graph(&graph, &selected_nodes);

        assert_eq!(state.graph.node_count(), 0);
        assert_eq!(state.graph.edge_count(), 0);
    }

    #[test]
    fn test_egui_adapter_nodes_with_positions() {
        let mut graph = Graph::new();
        let key = graph.add_node(
            "https://example.com".to_string(),
            Point2D::new(100.0, 200.0),
        );
        let selected_nodes = HashSet::new();
        let state = EguiGraphState::from_graph(&graph, &selected_nodes);

        assert_eq!(state.graph.node_count(), 1);

        let idx = state.get_index(key).unwrap();
        let node = state.graph.node(idx).unwrap();
        assert_eq!(node.location(), Pos2::new(100.0, 200.0));
    }

    #[test]
    fn test_egui_adapter_roundtrip_key_mapping() {
        let mut graph = Graph::new();
        let key1 = graph.add_node("a".to_string(), Point2D::new(0.0, 0.0));
        let key2 = graph.add_node("b".to_string(), Point2D::new(100.0, 100.0));
        graph.add_edge(key1, key2, EdgeType::Hyperlink, None);
        let selected_nodes = HashSet::new();
        let state = EguiGraphState::from_graph(&graph, &selected_nodes);

        let idx1 = state.get_index(key1).unwrap();
        let idx2 = state.get_index(key2).unwrap();
        assert_eq!(state.get_key(idx1), Some(key1));
        assert_eq!(state.get_key(idx2), Some(key2));

        assert_eq!(state.graph.node_count(), 2);
        assert_eq!(state.graph.edge_count(), 1);
    }

    #[test]
    fn test_egui_adapter_selection_state() {
        let mut graph = Graph::new();
        let key = graph.add_node("test".to_string(), Point2D::new(0.0, 0.0));
        let mut selected_nodes = HashSet::new();
        selected_nodes.insert(key);

        let state = EguiGraphState::from_graph(&graph, &selected_nodes);
        let idx = state.get_index(key).unwrap();
        let node = state.graph.node(idx).unwrap();

        assert!(node.selected());
    }

    #[test]
    fn test_egui_adapter_lifecycle_colors() {
        let mut graph = Graph::new();
        let key_active = graph.add_node("active".to_string(), Point2D::new(0.0, 0.0));
        let key_warm = graph.add_node("warm".to_string(), Point2D::new(50.0, 0.0));
        let key_cold = graph.add_node("cold".to_string(), Point2D::new(100.0, 0.0));

        graph.get_node_mut(key_active).unwrap().lifecycle = NodeLifecycle::Active;
        graph.get_node_mut(key_warm).unwrap().lifecycle = NodeLifecycle::Warm;
        let selected_nodes = HashSet::new();
        let state = EguiGraphState::from_graph(&graph, &selected_nodes);

        let idx_active = state.get_index(key_active).unwrap();
        let idx_warm = state.get_index(key_warm).unwrap();
        let idx_cold = state.get_index(key_cold).unwrap();

        let active_node = state.graph.node(idx_active).unwrap();
        let warm_node = state.graph.node(idx_warm).unwrap();
        let cold_node = state.graph.node(idx_cold).unwrap();

        assert_eq!(active_node.color(), Some(Color32::from_rgb(100, 200, 255)));
        assert_eq!(warm_node.color(), Some(Color32::from_rgb(120, 170, 205)));
        assert_eq!(cold_node.color(), Some(Color32::from_rgb(140, 140, 165)));
    }

    #[test]
    fn test_truncate_label() {
        use crate::util::truncate_with_ellipsis;
        assert_eq!(truncate_with_ellipsis("short", 20), "short");
        let result =
            truncate_with_ellipsis("this is a very long title that should be truncated", 20);
        assert_eq!(result.chars().count(), 20);
        assert!(result.ends_with('\u{2026}'));
    }

    #[test]
    fn test_membership_badge_metadata_injected_by_uuid() {
        let mut graph = Graph::new();
        let key = graph.add_node(
            "https://example.com".to_string(),
            Point2D::new(100.0, 200.0),
        );
        let node_id = graph.get_node(key).unwrap().id;
        let selected_nodes = HashSet::new();
        let memberships = HashMap::from([(
            node_id,
            vec!["alpha".to_string(), "beta".to_string(), "gamma".to_string()],
        )]);

        let state = EguiGraphState::from_graph_with_memberships(
            &graph,
            &selected_nodes,
            None,
            &HashSet::new(),
            &memberships,
            &HashMap::new(),
            &HashSet::new(),
        );
        let node = state.graph.node(key).unwrap();
        let shape = node.display();

        assert_eq!(shape.workspace_membership_count, 3);
        assert_eq!(
            shape.workspace_membership_names,
            vec!["alpha".to_string(), "beta".to_string(), "gamma".to_string()]
        );
    }

    #[test]
    fn test_membership_badge_metadata_empty_without_mapping() {
        let mut graph = Graph::new();
        let key = graph.add_node(
            "https://example.com".to_string(),
            Point2D::new(100.0, 200.0),
        );
        let selected_nodes = HashSet::new();
        let memberships: HashMap<Uuid, Vec<String>> = HashMap::new();

        let state = EguiGraphState::from_graph_with_memberships(
            &graph,
            &selected_nodes,
            None,
            &HashSet::new(),
            &memberships,
            &HashMap::new(),
            &HashSet::new(),
        );
        let node = state.graph.node(key).unwrap();
        let shape = node.display();

        assert_eq!(shape.workspace_membership_count, 0);
        assert!(shape.workspace_membership_names.is_empty());
    }

    #[test]
    fn test_pinned_flag_copied_from_graph_node() {
        let mut graph = Graph::new();
        let key = graph.add_node("https://example.com".to_string(), Point2D::new(0.0, 0.0));
        graph.get_node_mut(key).unwrap().is_pinned = true;

        let state = EguiGraphState::from_graph(&graph, &HashSet::new());
        let shape = state.graph.node(key).unwrap().display();
        assert!(shape.is_pinned);
    }

    #[test]
    fn semantic_badges_fill_all_three_slots_before_overflow() {
        let mut graph = Graph::new();
        let key = graph.add_node("https://example.com".to_string(), Point2D::new(0.0, 0.0));

        let state = EguiGraphState::from_graph_with_memberships(
            &graph,
            &HashSet::new(),
            None,
            &HashSet::new(),
            &HashMap::new(),
            &HashMap::from([(
                key,
                vec!["51".to_string(), "#pin".to_string(), "#focus".to_string()],
            )]),
            &HashSet::new(),
        );

        let shape = state.graph.node(key).unwrap().display();
        assert_eq!(
            shape.semantic_badges,
            vec!["51".to_string(), "#pin".to_string(), "#focus".to_string()]
        );
        assert_eq!(shape.semantic_badge_overflow, 0);
    }

    #[test]
    fn test_label_tier_full() {
        let label = GraphNodeShape::label_text_for_zoom_value(
            "https://example.com/full/path",
            "https://example.com/full/path",
            Some("example.com"),
            "fallback",
            2.0,
        );
        assert_eq!(label.as_deref(), Some("https://example.com/full/path"));
    }

    #[test]
    fn test_label_tier_domain() {
        let label = GraphNodeShape::label_text_for_zoom_value(
            "Very Long Title Name",
            "https://docs.example.com/some/path?q=1",
            Some("docs.example.com"),
            "fallback",
            1.0,
        );
        assert_eq!(label.as_deref(), Some("docs.example.com"));
    }

    #[test]
    fn test_label_tier_none() {
        let label = GraphNodeShape::label_text_for_zoom_value(
            "Title",
            "https://example.com",
            Some("example.com"),
            "fallback",
            0.4,
        );
        assert!(label.is_none());
    }

    #[test]
    fn test_label_tier_boundary_lower_hide() {
        // zoom just below 0.6 must hide label
        let label = GraphNodeShape::label_text_for_zoom_value(
            "Title",
            "https://example.com",
            Some("example.com"),
            "fallback",
            0.59,
        );
        assert!(label.is_none());
    }

    #[test]
    fn test_label_tier_boundary_lower_domain() {
        // zoom exactly 0.6 must show domain tier
        let label = GraphNodeShape::label_text_for_zoom_value(
            "Title",
            "https://example.com",
            Some("example.com"),
            "fallback",
            0.6,
        );
        assert_eq!(label.as_deref(), Some("example.com"));
    }

    #[test]
    fn test_label_tier_boundary_upper_domain() {
        // zoom exactly 1.5 must still show domain tier
        let label = GraphNodeShape::label_text_for_zoom_value(
            "Title",
            "https://example.com",
            Some("example.com"),
            "fallback",
            1.5,
        );
        assert_eq!(label.as_deref(), Some("example.com"));
    }

    #[test]
    fn test_label_tier_boundary_upper_full() {
        // zoom just above 1.5 must show full tier
        let label = GraphNodeShape::label_text_for_zoom_value(
            "My Page Title",
            "https://example.com",
            Some("example.com"),
            "fallback",
            1.51,
        );
        assert_eq!(label.as_deref(), Some("My Page Title"));
    }

    #[test]
    fn test_label_tier_domain_non_url_fallback_to_title() {
        // non-parseable URL in domain tier falls back to truncated title
        let label = GraphNodeShape::label_text_for_zoom_value(
            "My Title",
            "not-a-valid-url",
            None,
            "fallback",
            1.0,
        );
        assert_eq!(label.as_deref(), Some("My Title"));
    }

    #[test]
    fn test_label_tier_domain_empty_title_uses_fallback() {
        // empty title in domain tier falls back to fallback text
        let label =
            GraphNodeShape::label_text_for_zoom_value("", "not-a-valid-url", None, "fb", 1.0);
        assert_eq!(label.as_deref(), Some("fb"));
    }

    #[test]
    fn test_label_tier_full_prefers_title_over_url() {
        // when title differs from URL, full tier returns title
        let label = GraphNodeShape::label_text_for_zoom_value(
            "Page Title",
            "https://example.com/some/path",
            Some("example.com"),
            "fallback",
            2.0,
        );
        assert_eq!(label.as_deref(), Some("Page Title"));
    }

    #[test]
    fn test_label_tier_full_empty_title_uses_url() {
        // empty title in full tier falls back to URL
        let label = GraphNodeShape::label_text_for_zoom_value(
            "",
            "https://example.com",
            Some("example.com"),
            "fallback",
            2.0,
        );
        assert_eq!(label.as_deref(), Some("https://example.com"));
    }

    #[test]
    fn test_label_tier_full_all_empty_returns_none() {
        // all fields empty at full zoom returns None
        let label = GraphNodeShape::label_text_for_zoom_value("", "", None, "", 2.0);
        assert!(label.is_none());
    }

    #[test]
    fn test_edge_shape_selection() {
        let history = GraphEdgeShape::from(EdgeProps {
            payload: EdgePayload::from_edge_type(EdgeType::History, None),
            order: 0,
            selected: false,
            label: String::new(),
        });
        let grouped = GraphEdgeShape::from(EdgeProps {
            payload: EdgePayload::from_edge_type(EdgeType::UserGrouped, None),
            order: 0,
            selected: false,
            label: String::new(),
        });

        let (history_color, _) = history.style_stroke();
        let (grouped_color, grouped_width) = grouped.style_stroke();
        assert_eq!(history.style, GraphEdgeVisualStyle::History);
        assert_eq!(grouped.style, GraphEdgeVisualStyle::UserGrouped);
        assert_eq!(history_color, Color32::from_rgb(120, 180, 210));
        assert_eq!(grouped_color, Color32::from_rgb(236, 171, 64));
        assert!(grouped_width > 2.0);
    }

    #[test]
    fn test_traversal_count_drives_stroke_width() {
        let mut edge = GraphEdgeShape::from(EdgeProps {
            payload: EdgePayload::from_edge_type(EdgeType::History, None),
            order: 0,
            selected: false,
            label: String::new(),
        });
        edge.configure_logical_pair(
            GraphEdgeVisualStyle::History,
            LogicalPairTraversalAggregate {
                total_count: 1,
                forward_count: 1,
                backward_count: 0,
                dominant_cue: DominantDirectionCue::None,
            },
        );
        let (_, w1) = edge.style_stroke();
        edge.configure_logical_pair(
            GraphEdgeVisualStyle::History,
            LogicalPairTraversalAggregate {
                total_count: 9,
                forward_count: 9,
                backward_count: 0,
                dominant_cue: DominantDirectionCue::None,
            },
        );
        let (_, w9) = edge.style_stroke();
        assert!(w9 > w1);
    }

    #[test]
    fn test_dominant_direction_above_threshold() {
        let cue = GraphEdgeShape::dominant_direction_from_counts(7, 3, 0.60);
        assert_eq!(cue, DominantDirectionCue::AlongEdge);
    }

    #[test]
    fn test_dominant_direction_below_threshold() {
        let cue = GraphEdgeShape::dominant_direction_from_counts(6, 4, 0.60);
        assert_eq!(cue, DominantDirectionCue::None);
    }

    #[test]
    fn test_display_dedup_skips_reverse_pair() {
        let mut graph = Graph::new();
        let a = graph.add_node("https://a.example".into(), Point2D::new(0.0, 0.0));
        let b = graph.add_node("https://b.example".into(), Point2D::new(10.0, 0.0));
        let _ = graph.add_edge(a, b, EdgeType::Hyperlink, None);
        let _ = graph.add_edge(b, a, EdgeType::History, None);
        let selected = HashSet::new();
        let state = EguiGraphState::from_graph(&graph, &selected);

        let visible_edges = state
            .graph
            .edges_iter()
            .filter(|(_, edge)| !edge.display().hidden())
            .count();
        assert_eq!(visible_edges, 1);
    }

    #[test]
    fn test_secondary_selected_visual_differs_from_primary() {
        let mut graph = Graph::new();
        let primary = graph.add_node("https://a.example".into(), Point2D::new(0.0, 0.0));
        let secondary = graph.add_node("https://b.example".into(), Point2D::new(10.0, 0.0));
        let mut selected = HashSet::new();
        selected.insert(primary);
        selected.insert(secondary);

        let state = EguiGraphState::from_graph_with_visual_state(
            &graph,
            &selected,
            Some(primary),
            &HashSet::new(),
        );
        let p = state.graph.node(primary).unwrap().display();
        let s = state.graph.node(secondary).unwrap().display();
        assert_eq!(p.selection_role, SelectionVisualRole::Primary);
        assert_eq!(s.selection_role, SelectionVisualRole::Secondary);
    }

    #[test]
    fn test_crashed_node_color_differs_from_cold() {
        let mut graph = Graph::new();
        let cold = graph.add_node("https://cold.example".into(), Point2D::new(0.0, 0.0));
        let crashed = graph.add_node("https://crashed.example".into(), Point2D::new(10.0, 0.0));
        let crashed_nodes = HashSet::from([crashed]);
        let state = EguiGraphState::from_graph_with_visual_state(
            &graph,
            &HashSet::new(),
            None,
            &crashed_nodes,
        );
        let cold_color = state.graph.node(cold).unwrap().display().projected_color();
        let crashed_color = state
            .graph
            .node(crashed)
            .unwrap()
            .display()
            .projected_color();
        assert_ne!(cold_color, crashed_color);
    }
}
