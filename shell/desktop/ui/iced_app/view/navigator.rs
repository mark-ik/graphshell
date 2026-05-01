//! Navigator-bucket render functions extracted from `view/mod.rs`
//! — Slice 45 / Phase D continuation. Covers the three Presentation
//! Bucket renders (Tree Spine / Swatches / Activity Log), their row
//! and card helpers, and `format_ux_event` (used by the Activity
//! Log bucket and by tests). The orchestrator
//! `render_navigator_host` stays in `view/mod.rs`.

use super::*;

pub(crate) fn render_tree_spine_bucket(app: &IcedApp) -> Element<'_, Message> {
    let runtime = &app.host.runtime;
    let header: Element<'_, Message> = text("Tree Spine")
        .size(11)
        .width(Length::Fill)
        .into();

    let member_count = runtime.graph_tree.member_count();
    if member_count == 0 {
        return scrollable(
            iced::widget::column![header, text("— No nodes in this workbench").size(11)].spacing(4),
        )
        .height(Length::FillPortion(2))
        .into();
    }

    // Collect (NodeKey, label) pairs so the borrow on graph_tree is
    // dropped before the column builder consumes the strings.
    let members: Vec<(graphshell_core::graph::NodeKey, String)> = runtime
        .graph_tree
        .members()
        .map(|(node_key, _entry)| {
            let label = runtime
                .graph_app
                .domain_graph()
                .get_node(*node_key)
                .map(|n| {
                    if n.title.is_empty() {
                        n.url().to_string()
                    } else {
                        n.title.clone()
                    }
                })
                .unwrap_or_else(|| format!("n{}", node_key.index()));
            (*node_key, label)
        })
        .collect();

    let rows: Vec<Element<'_, Message>> = members
        .into_iter()
        .map(|(node_key, label)| tree_spine_row(node_key, label))
        .collect();

    let mut spine = iced::widget::column![header];
    for row in rows {
        spine = spine.push(row);
    }

    scrollable(spine.spacing(2).padding([2, 0]))
        .height(Length::FillPortion(2))
        .into()
}

/// Render the Swatches bucket — Navigator's middle-row "live
/// projections at a glance" surface. Per
/// [`iced_composition_skeleton_spec.md` §6.2](
/// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_composition_skeleton_spec.md):
/// virtualized grid of compact canvas instances, one per recipe.
///
/// Slice 33 ships a vertical stack of three built-in recipe cards
/// (FullGraph / RecentlyActive / FocusedNeighborhood). Each card is
/// a `gs::*`-clean composition: a small canvas widget showing the
/// graph at compact size, a label below, and a click-area that
/// dispatches `SwatchClicked(recipe)`.
///
/// Layout deviates from the spec's wrap_horizontally because vertical
/// stacking matches how the bucket lives inside a sidebar Navigator
/// host (180px wide). When a Navigator host opens in the wider
/// Top/Bottom toolbar configuration, the layout adapts via
/// FillPortion height — the card-internals don't change shape.
///
/// Real per-recipe scene scoping (filtered nodes, lens overrides)
/// lands when the projection-recipe authority is wired; the canvas
/// rendering path below stays.
pub(crate) fn render_swatches_bucket(app: &IcedApp) -> Element<'_, Message> {
    let header: Element<'_, Message> =
        text("Swatches").size(11).width(Length::Fill).into();

    let nodes_count = app.host.runtime.graph_app.domain_graph().nodes().count();
    if nodes_count == 0 {
        return scrollable(
            iced::widget::column![header, text("— No recipes yet (graph is empty)").size(11)]
                .spacing(4),
        )
        .height(Length::FillPortion(1))
        .into();
    }

    let mut col = iced::widget::column![header];
    for recipe in SwatchRecipe::builtin_set() {
        col = col.push(render_swatch_card(app, *recipe));
    }

    scrollable(col.spacing(6).padding([2, 0]))
        .height(Length::FillPortion(1))
        .into()
}

/// Render one swatch card. Layout: a 60px-tall canvas frame on top
/// (showing the graph in miniature), then a click-button under it
/// labeled with the recipe name.
pub(crate) fn render_swatch_card<'a>(app: &'a IcedApp, recipe: SwatchRecipe) -> Element<'a, Message> {
    // Slice 33: every recipe currently shares the same scene input
    // (full graph). When per-recipe scoping lands, this swap to
    // `recipe.scene_input_for(graph_app, view_id)` — single call site
    // change.
    let program = graph_canvas_from_app(&app.host.runtime.graph_app, app.host.view_id);
    let _: &GraphCanvasProgram = &program;
    let canvas_widget: Element<'_, crate::shell::desktop::ui::iced_graph_canvas::GraphCanvasMessage> =
        canvas(program)
            .width(Length::Fill)
            .height(Length::Fixed(60.0))
            .into();
    // Swatch canvases swallow camera-changed and right-clicked events
    // for now — they don't have their own pane id and right-click
    // would compete with the swatch click handler. Map to Tick (no-op).
    let canvas_inner: Element<'_, Message> = canvas_widget.map(|_| Message::Tick);

    let label = button(text(recipe.label()).size(11).width(Length::Fill))
        .on_press(Message::SwatchClicked(recipe))
        .padding([2, 6])
        .width(Length::Fill)
        .style(|theme: &iced::Theme, status| {
            let pal = theme.palette();
            let hovered = matches!(
                status,
                iced::widget::button::Status::Hovered
                    | iced::widget::button::Status::Pressed
            );
            iced::widget::button::Style {
                background: if hovered {
                    Some(tokens::HOVER_OVERLAY_SUBTLE.into())
                } else {
                    None
                },
                text_color: pal.background.base.text,
                border: iced::Border {
                    radius: 2.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        });

    container(iced::widget::column![canvas_inner, label].spacing(2))
        .padding(3)
        .width(Length::Fill)
        .style(|theme: &iced::Theme| {
            let pal = theme.palette();
            container::Style {
                background: Some(
                    tokens::chrome_band(
                        pal.background.base.text,
                        tokens::CHROME_BAND_FAINT,
                    )
                    .into(),
                ),
                border: iced::Border {
                    radius: tokens::RADIUS_BUTTON.into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        })
        .into()
}

/// Render the Activity Log bucket — Navigator's right-rail
/// "what just happened" stream. Per
/// [`iced_composition_skeleton_spec.md` §6.3](
/// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_composition_skeleton_spec.md).
///
/// Slice 27: snapshots the host's bounded `RecordingObserver`
/// (capacity `ACTIVITY_LOG_CAPACITY = 100`) and renders one row per
/// recorded event in most-recent-first order. Empty buffer shows a
/// "no activity yet" placeholder. Subsequent slices can add
/// click-to-navigate (e.g., a row for `OpenNodeDispatched` could
/// re-focus that node) and visual filtering.
pub(crate) fn render_activity_log_bucket(app: &IcedApp) -> Element<'_, Message> {
    let header: Element<'_, Message> =
        text("Activity Log").size(11).width(Length::Fill).into();

    let events = app.activity_log_recorder.snapshot();
    if events.is_empty() {
        return scrollable(
            iced::widget::column![header, text("— No activity yet").size(11)].spacing(4),
        )
        .height(Length::FillPortion(1))
        .into();
    }

    // Render most-recent first. The recorder appends in observation
    // order, so reverse for display.
    let rows: Vec<Element<'_, Message>> = events
        .into_iter()
        .rev()
        .map(activity_log_row)
        .collect();

    let mut col = iced::widget::column![header];
    for row in rows {
        col = col.push(row);
    }

    scrollable(col.spacing(2).padding([2, 0]))
        .height(Length::FillPortion(1))
        .into()
}

/// One row in the Activity Log. Renders a single line of text
/// describing the event; click handlers land in a future slice.
pub(crate) fn activity_log_row<'a>(
    event: graphshell_core::ux_observability::UxEvent,
) -> Element<'a, Message> {
    text(format_ux_event(&event))
        .size(11)
        .width(Length::Fill)
        .into()
}

/// Convert a `UxEvent` into a concise human-readable summary line
/// for the Activity Log. Surface variants render as
/// `"opened: Command Palette"` / `"dismissed: Node Finder (cancelled)"`;
/// dispatches render as `"action: graph:toggle_physics"` /
/// `"open node: n7"`.
pub(crate) fn format_ux_event(event: &graphshell_core::ux_observability::UxEvent) -> String {
    use graphshell_core::ux_observability::{DismissReason, SurfaceId, UxEvent};
    fn surface_label(s: SurfaceId) -> &'static str {
        match s {
            SurfaceId::Omnibar => "Omnibar",
            SurfaceId::CommandPalette => "Command Palette",
            SurfaceId::NodeFinder => "Node Finder",
            SurfaceId::ContextMenu => "Context Menu",
            SurfaceId::ConfirmDialog => "Confirm Dialog",
            SurfaceId::NodeCreate => "Create Node",
            SurfaceId::FrameRename => "Rename Frame",
            SurfaceId::StatusBar => "Status Bar",
            SurfaceId::TreeSpine => "Tree Spine",
            SurfaceId::NavigatorHost => "Navigator",
            SurfaceId::TilePane => "Tile Pane",
            SurfaceId::CanvasPane => "Canvas Pane",
            SurfaceId::BaseLayer => "Base Layer",
        }
    }
    fn reason_label(r: DismissReason) -> &'static str {
        match r {
            DismissReason::Confirmed => "confirmed",
            DismissReason::Cancelled => "cancelled",
            DismissReason::Superseded => "superseded",
            DismissReason::Programmatic => "programmatic",
        }
    }
    match event {
        UxEvent::SurfaceOpened { surface } => {
            format!("opened: {}", surface_label(*surface))
        }
        UxEvent::SurfaceDismissed { surface, reason } => format!(
            "dismissed: {} ({})",
            surface_label(*surface),
            reason_label(*reason)
        ),
        UxEvent::ActionDispatched { action_id, target } => match target {
            Some(node_key) => {
                format!("action: {} → n{}", action_id.key(), node_key.index())
            }
            None => format!("action: {}", action_id.key()),
        },
        UxEvent::OpenNodeDispatched { node_key } => {
            format!("open node: n{}", node_key.index())
        }
    }
}

/// One row in the Tree Spine list. Click dispatches an OpenNode
/// intent against the row's NodeKey.
pub(crate) fn tree_spine_row<'a>(
    node_key: graphshell_core::graph::NodeKey,
    label: String,
) -> Element<'a, Message> {
    button(text(label).size(11).width(Length::Fill))
        .on_press(Message::TreeSpineNodeClicked(node_key))
        .padding([2, 6])
        .width(Length::Fill)
        .style(|theme: &iced::Theme, status| {
            let pal = theme.palette();
            let hovered = matches!(
                status,
                iced::widget::button::Status::Hovered
                    | iced::widget::button::Status::Pressed
            );
            iced::widget::button::Style {
                background: if hovered {
                    Some(tokens::HOVER_OVERLAY_SUBTLE.into())
                } else {
                    None
                },
                text_color: pal.background.base.text,
                border: iced::Border {
                    radius: 2.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        })
        .into()
}
