//! View-layer free functions extracted from `iced_app/mod.rs`. The
//! orchestration (IcedApp, Message, update, view, run_application)
//! lives in `iced_app/mod.rs`; surface-specific render helpers, modal
//! widgets, navigator buckets, chrome surfaces, and host helpers each
//! live in their own submodule and are re-exported here.

use super::*;

mod chrome;
mod helpers;
mod modals;
mod navigator;
mod panes;

pub(crate) use chrome::*;
pub(crate) use helpers::*;
pub(crate) use modals::*;
pub(crate) use navigator::*;
pub(crate) use panes::*;


// ---------------------------------------------------------------------------
// Navigator host rendering — Slice 4 (structural layout)
// ---------------------------------------------------------------------------

/// Which edge of the workbench a Navigator host is anchored to.
///
/// Left / Right → sidebar form factor (vertical column, fixed width).
/// Top / Bottom → toolbar form factor (horizontal row, fixed height).
/// Per [`iced_composition_skeleton_spec.md` §2](
/// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_composition_skeleton_spec.md).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum NavigatorAnchor {
    Left,
    Right,
    Top,
    Bottom,
}

/// Render one Navigator host slot with stub Presentation Buckets.
///
/// Per spec §6: each host renders the three canonical buckets — Tree
/// Spine, Swatches, Activity Log — in a layout appropriate for its
/// form factor. This slice renders structural stubs; real bucket content
/// (lazy+scrollable trees, canvas swatch grid, event stream) lands once
/// the Navigator domain layer is wired (S5).
pub(crate) fn render_navigator_host(app: &IcedApp, anchor: NavigatorAnchor) -> Element<'_, Message> {
    // Tree Spine bucket — Slice 20 reads from the runtime's GraphTree
    // and renders one row per member. Each row is a button that
    // dispatches `Message::TreeSpineNodeClicked(node_key)` → the
    // runtime promotes the node to focused via HostIntent::OpenNode.
    let tree_spine: Element<'_, Message> = render_tree_spine_bucket(app);

    // Swatches bucket — Slice 33 renders one compact canvas card per
    // built-in projection recipe. Per
    // [`iced_composition_skeleton_spec.md` §6.2](
    // ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_composition_skeleton_spec.md):
    // virtualized grid of `canvas::Program` instances at the
    // `RenderProfile::Swatch` profile.
    let swatches: Element<'_, Message> = render_swatches_bucket(app);

    // Activity Log bucket — Slice 27 reads from the host's bounded
    // RecordingObserver and renders one row per UxEvent in
    // most-recent-first order. Per
    // [`iced_composition_skeleton_spec.md` §6.3](
    // ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_composition_skeleton_spec.md):
    // event-stream view of recent runtime activity.
    let activity_log: Element<'_, Message> = render_activity_log_bucket(app);

    match anchor {
        NavigatorAnchor::Left | NavigatorAnchor::Right => {
            // Sidebar form factor: vertical column, fixed width.
            container(
                iced::widget::column![tree_spine, swatches, activity_log]
                    .spacing(4)
                    .height(Length::Fill),
            )
            .width(Length::Fixed(180.0))
            .height(Length::Fill)
            .padding(6)
            .into()
        }
        NavigatorAnchor::Top | NavigatorAnchor::Bottom => {
            // Toolbar form factor: horizontal row, fixed height.
            container(
                iced::widget::row![tree_spine, swatches, activity_log]
                    .spacing(4)
                    .width(Length::Fill),
            )
            .width(Length::Fill)
            .height(Length::Fixed(120.0))
            .padding(6)
            .into()
        }
    }
}

