/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Egui adapter for `graph_tree::GraphTree<NodeKey>`.
//!
//! Implements `GraphTreeRenderer` to render the tree-style tab sidebar,
//! flat tab bar, and split pane chrome using egui. This module runs
//! **alongside** the existing `egui_tiles::Tree<TileKind>` during the
//! parallel introduction phase; it will progressively take over layout
//! authority as the migration proceeds.
//!
//! The adapter is intentionally thin: it reads `GraphTree` state and
//! paints egui widgets. All mutations flow back through `NavAction`.

use std::collections::HashMap;

use egui::{Color32, RichText, Sense, Stroke, TextStyle, Ui, Vec2};

use graph_tree::{
    GraphTree, GraphTreeRenderer, Lifecycle, OwnedTreeRow, TabEntry,
};

use crate::graph::NodeKey;

/// Egui rendering context passed through `GraphTreeRenderer::Ctx`.
pub(crate) struct EguiTreeCtx<'a> {
    pub(crate) ui: &'a mut Ui,
    /// Callback to resolve a human-readable label for a member node.
    /// Returns the node title if available, otherwise a debug repr.
    pub(crate) label_fn: &'a dyn Fn(NodeKey) -> String,
}

/// Output from a single `GraphTreeRenderer` call.
pub(crate) struct EguiTreeOutput {
    /// Nav actions requested by user interaction (clicks, expand toggles, etc.).
    pub(crate) actions: Vec<graph_tree::NavAction<NodeKey>>,
}

impl EguiTreeOutput {
    fn empty() -> Self {
        Self {
            actions: Vec::new(),
        }
    }
}

/// The egui-side renderer for `GraphTree<NodeKey>`.
///
/// Stateless between frames — all persistent state lives in the `GraphTree`
/// itself. Created fresh each frame with references to app state needed for
/// label resolution and styling.
pub(crate) struct EguiGraphTreeRenderer;

impl GraphTreeRenderer<NodeKey> for EguiGraphTreeRenderer {
    type Ctx = EguiTreeCtx<'static>;
    type Out = EguiTreeOutput;

    fn render_tree_tabs(
        &mut self,
        tree: &GraphTree<NodeKey>,
        rows: &[OwnedTreeRow<NodeKey>],
        ctx: &mut Self::Ctx,
    ) -> Self::Out {
        render_tree_tabs_impl(tree, rows, ctx)
    }

    fn render_flat_tabs(
        &mut self,
        tree: &GraphTree<NodeKey>,
        tabs: &[TabEntry<NodeKey>],
        ctx: &mut Self::Ctx,
    ) -> Self::Out {
        render_flat_tabs_impl(tree, tabs, ctx)
    }

    fn render_pane_chrome(
        &mut self,
        tree: &GraphTree<NodeKey>,
        rects: &HashMap<NodeKey, graph_tree::Rect>,
        ctx: &mut Self::Ctx,
    ) -> Self::Out {
        render_pane_chrome_impl(tree, rects, ctx)
    }
}

// ---------------------------------------------------------------------------
// Tree-style tabs sidebar
// ---------------------------------------------------------------------------

fn render_tree_tabs_impl(
    tree: &GraphTree<NodeKey>,
    rows: &[OwnedTreeRow<NodeKey>],
    ctx: &mut EguiTreeCtx<'_>,
) -> EguiTreeOutput {
    let mut actions = Vec::new();
    let active = tree.active().cloned();
    let indent_px = 16.0;

    for row in rows {
        let is_active = active.as_ref() == Some(&row.member);
        let label_text = (ctx.label_fn)(row.member);

        // Indent based on tree depth.
        let indent = row.depth as f32 * indent_px;
        let lifecycle = tree
            .get(&row.member)
            .map(|e| e.lifecycle)
            .unwrap_or(Lifecycle::Cold);

        ctx.ui.horizontal(|ui| {
            ui.add_space(indent);

            // Expand/collapse toggle for parent nodes.
            if row.has_children {
                let arrow = if row.is_expanded { "▼" } else { "▶" };
                if ui
                    .add(egui::Button::new(arrow).frame(false))
                    .clicked()
                {
                    actions.push(graph_tree::NavAction::ToggleExpand(row.member));
                }
            } else {
                // Leaf spacer — keep alignment consistent.
                ui.add_space(ui.spacing().icon_width);
            }

            // Lifecycle badge dot.
            let badge_color = lifecycle_color(lifecycle);
            let (badge_rect, _) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
            ui.painter().circle_filled(badge_rect.center(), 3.5, badge_color);

            // Row label — clickable to activate.
            let text = if is_active {
                RichText::new(&label_text).strong()
            } else {
                RichText::new(&label_text)
            };

            let response = ui.selectable_label(is_active, text);
            if response.clicked() {
                actions.push(graph_tree::NavAction::Activate(row.member));
            }
            if response.secondary_clicked() {
                actions.push(graph_tree::NavAction::Select(row.member));
            }
        });
    }

    EguiTreeOutput { actions }
}

// ---------------------------------------------------------------------------
// Flat tab bar
// ---------------------------------------------------------------------------

fn render_flat_tabs_impl(
    tree: &GraphTree<NodeKey>,
    tabs: &[TabEntry<NodeKey>],
    ctx: &mut EguiTreeCtx<'_>,
) -> EguiTreeOutput {
    let mut actions = Vec::new();
    let active = tree.active().cloned();

    ctx.ui.horizontal(|ui| {
        for tab in tabs {
            let is_active = active.as_ref() == Some(&tab.member);
            let label = (ctx.label_fn)(tab.member);

            let text = if is_active {
                RichText::new(&label).strong()
            } else {
                RichText::new(&label)
            };

            let response = ui.selectable_label(is_active, text);
            if response.clicked() {
                actions.push(graph_tree::NavAction::Activate(tab.member));
            }

            // Close button for non-anchor tabs.
            if !tab.is_anchor {
                if ui
                    .add(egui::Button::new("×").frame(false).small())
                    .clicked()
                {
                    actions.push(graph_tree::NavAction::Dismiss(tab.member));
                }
            }
        }
    });

    EguiTreeOutput { actions }
}

// ---------------------------------------------------------------------------
// Split pane chrome (borders and labels)
// ---------------------------------------------------------------------------

fn render_pane_chrome_impl(
    tree: &GraphTree<NodeKey>,
    rects: &HashMap<NodeKey, graph_tree::Rect>,
    ctx: &mut EguiTreeCtx<'_>,
) -> EguiTreeOutput {
    let actions = Vec::new();
    let active = tree.active().cloned();
    let painter = ctx.ui.painter();

    for (member, rect) in rects {
        let egui_rect = to_egui_rect(rect);
        let is_active = active.as_ref() == Some(member);

        // Border.
        let stroke = if is_active {
            Stroke::new(2.0, Color32::from_rgb(100, 160, 255))
        } else {
            Stroke::new(1.0, Color32::from_gray(80))
        };
        painter.rect_stroke(egui_rect, 0.0, stroke, egui::StrokeKind::Inside);

        // Small label in top-left corner.
        let label = (ctx.label_fn)(*member);
        painter.text(
            egui_rect.left_top() + Vec2::new(4.0, 2.0),
            egui::Align2::LEFT_TOP,
            label,
            TextStyle::Small.resolve(ctx.ui.style()),
            if is_active {
                Color32::WHITE
            } else {
                Color32::from_gray(180)
            },
        );
    }

    EguiTreeOutput { actions }
}

// ---------------------------------------------------------------------------
// Render pass integration
// ---------------------------------------------------------------------------

/// Run the GraphTree renderer for the current layout mode and return collected
/// NavActions. This bypasses the `GraphTreeRenderer` trait (which requires
/// `'static` Ctx) and calls the impl functions directly with the real lifetimes.
pub(crate) fn render_graph_tree_chrome(
    graph_tree: &graph_tree::GraphTree<NodeKey>,
    tree_rows: &[graph_tree::OwnedTreeRow<NodeKey>],
    tab_order: &[graph_tree::TabEntry<NodeKey>],
    raw_pane_rects: &HashMap<NodeKey, graph_tree::Rect>,
    ctx: &mut EguiTreeCtx<'_>,
) -> Vec<graph_tree::NavAction<NodeKey>> {
    let mut all_actions = Vec::new();
    let mode = graph_tree.layout_mode();

    match mode {
        graph_tree::LayoutMode::TreeStyleTabs => {
            let out = render_tree_tabs_impl(graph_tree, tree_rows, ctx);
            all_actions.extend(out.actions);
        }
        graph_tree::LayoutMode::FlatTabs => {
            let out = render_flat_tabs_impl(graph_tree, tab_order, ctx);
            all_actions.extend(out.actions);
        }
        graph_tree::LayoutMode::SplitPanes => {
            // In split mode, render both the pane chrome and the tree sidebar.
            let chrome_out = render_pane_chrome_impl(graph_tree, raw_pane_rects, ctx);
            all_actions.extend(chrome_out.actions);
            let tree_out = render_tree_tabs_impl(graph_tree, tree_rows, ctx);
            all_actions.extend(tree_out.actions);
        }
    }

    all_actions
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn lifecycle_color(lifecycle: Lifecycle) -> Color32 {
    match lifecycle {
        Lifecycle::Active => Color32::from_rgb(80, 200, 120),  // green
        Lifecycle::Warm => Color32::from_rgb(255, 180, 60),    // amber
        Lifecycle::Cold => Color32::from_gray(120),             // gray
    }
}

fn to_egui_rect(r: &graph_tree::Rect) -> egui::Rect {
    egui::Rect::from_min_size(
        egui::pos2(r.x, r.y),
        egui::vec2(r.w, r.h),
    )
}

/// Convert an egui `Rect` to a graph-tree `Rect`.
pub(crate) fn from_egui_rect(r: egui::Rect) -> graph_tree::Rect {
    graph_tree::Rect::new(r.min.x, r.min.y, r.width(), r.height())
}

