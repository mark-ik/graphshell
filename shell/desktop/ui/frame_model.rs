/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Host ↔ Runtime frame protocol.
//!
//! Per the M3.5 runtime boundary design
//! (`design_docs/graphshell_docs/implementation_strategy/shell/2026-04-16_runtime_boundary_design.md`
//! §5), this module defines the shapes that flow between a `GraphshellRuntime`
//! and whatever host (egui today, iced later) is driving it.
//!
//! - [`FrameHostInput`] flows **into** the runtime each tick (events, pointer,
//!   viewport, host-widget focus flags).
//! - [`FrameViewModel`] is the runtime's **output** each tick — a read-only
//!   snapshot the host paints.
//!
//! The types are defined in M4.3 as scaffolding; population at the runtime
//! boundary and consumption at the host boundary land in M4.4 / M4.5 once
//! `HostPorts` are in place. Today the runtime still mutates shell state
//! directly; these types are the migration target.
//!
//! Post-M3.6, the boundary vocabulary uses portable types (`PortableRect`,
//! `PortablePoint`, `PortableSize` from `compositor_adapter`) rather than
//! `egui::*`. Egui hosts convert at population sites
//! (`gui.rs::build_frame_host_input`, `gui_state.rs::project_view_model`);
//! iced hosts consume portable types directly.

use crate::graph::NodeKey;
use crate::shell::desktop::workbench::compositor_adapter::{OverlayStrokePass, PortableRect};
use crate::shell::desktop::workbench::pane_model::PaneId;
use graph_tree::{OwnedTreeRow, SplitBoundary, TabEntry};

// Sub-view-model types + `FocusRingCurve` / `FocusRingSpec` / `FrameHostInput`
// moved to `graphshell_core::shell_state::frame_model` in M4 slice 8
// (2026-04-22). Re-exported here so callers resolve unchanged.
// `FocusRingSpec.started_at` changed from `std::time::Instant` to
// `graphshell_core::time::PortableInstant`; the shell populates via
// `crate::shell::desktop::ui::portable_time::portable_now()`.
pub(crate) use graphshell_core::shell_state::frame_model::{
    CommandPaletteScopeView, CommandPaletteViewModel, DegradedReceiptSpec, DialogsViewModel,
    FocusRingCurve, FocusRingSpec, FocusViewModel, FrameHostInput, GraphSearchViewModel,
    OmnibarProviderStatusView, OmnibarSessionKindView, OmnibarViewModel, ToastSeverity, ToastSpec,
    ToolbarViewModel,
};

// ---------------------------------------------------------------------------
// Runtime output: FrameViewModel
// ---------------------------------------------------------------------------

/// Per-frame snapshot produced by `GraphshellRuntime` for the host to paint.
///
/// All fields are read-only from the host's perspective. The host may
/// rasterize, lay out, or cache derived quantities, but must not mutate the
/// model — any feedback flows back through `HostPorts` / [`FrameHostInput`].
///
/// (No `Debug` derive because `OverlayStrokePass` transitively contains
/// non-Debug types — e.g., `OverlayAffordanceStyle` which derives only
/// `Clone, Copy`. Can be revisited independently of the M3.6 type cleanup.)
#[derive(Clone, Default)]
pub(crate) struct FrameViewModel {
    /// Visible panes with their screen rects (portable units), in stable
    /// iteration order.
    pub(crate) active_pane_rects: Vec<(PaneId, NodeKey, PortableRect)>,

    /// PaneId → TileRenderMode mapping, refreshed per frame alongside
    /// `active_pane_rects`. Mirrors `graph_runtime.pane_render_modes`
    /// for hosts that can't read `graph_app.workspace.graph_runtime`
    /// directly (iced). Previously host code reached through the
    /// compositor which scraped tiles directly; this projection makes
    /// the map a proper view-model citizen.
    pub(crate) pane_render_modes: std::collections::HashMap<
        PaneId,
        crate::shell::desktop::workbench::pane_model::TileRenderMode,
    >,

    /// PaneId → viewer-ID mapping, refreshed per frame alongside
    /// `active_pane_rects`. Resolves to the string identifier of the
    /// viewer implementation a pane is currently hosting (e.g.,
    /// "servo", "wry:…"). Consumed by compositor semantic-input
    /// resolution.
    pub(crate) pane_viewer_ids: std::collections::HashMap<PaneId, String>,

    /// GraphTree rows for sidebar / navigator rendering.
    pub(crate) tree_rows: Vec<OwnedTreeRow<NodeKey>>,

    /// Flat tab ordering for a tab-bar view.
    pub(crate) tab_order: Vec<TabEntry<NodeKey>>,

    /// Split boundaries (draggable gutter handles between panes).
    pub(crate) split_boundaries: Vec<SplitBoundary<NodeKey>>,

    /// Currently active member (the pane that owns keyboard focus among panes).
    pub(crate) active_pane: Option<NodeKey>,

    /// Aggregate focus state (which surface is focused, focus ring animation).
    pub(crate) focus: FocusViewModel,

    /// Toolbar / location bar state.
    pub(crate) toolbar: ToolbarViewModel,

    /// Omnibar search session projection. `None` when no session is
    /// active (user is not in an `@`-prefixed or search-provider query).
    pub(crate) omnibar: Option<OmnibarViewModel>,

    /// Graph-search (Ctrl+G) panel state projection.
    pub(crate) graph_search: GraphSearchViewModel,

    /// Command-palette (F2 / Ctrl+K) session projection.
    pub(crate) command_palette: CommandPaletteViewModel,

    /// Overlay descriptors the host must paint this frame
    /// (focus rings, selection strokes, lens glyphs, etc.).
    pub(crate) overlays: Vec<OverlayStrokePass>,

    /// Which dialogs / overlays are open.
    pub(crate) dialogs: DialogsViewModel,

    /// Toasts queued this frame — the host drains and displays them.
    pub(crate) toasts: Vec<ToastSpec>,

    /// Content surfaces (webviews etc.) whose content changed this frame and
    /// should be presented. The host consults the `ViewerSurfaceRegistry`
    /// (via `HostSurfacePort`) to resolve each key to a concrete handle.
    pub(crate) surfaces_to_present: Vec<NodeKey>,

    /// UX-visible degraded-mode receipts the host should render as chrome
    /// (e.g., "content viewer is in degraded mode").
    pub(crate) degraded_receipts: Vec<DegradedReceiptSpec>,

    /// Number of webview thumbnail captures currently pending async
    /// completion. Hosts can gate a "capture in progress" spinner /
    /// dim overlay on `captures_in_flight > 0`. The set of
    /// `WebViewId`s itself is not projected — consumers only need the
    /// count; the set stays on `GraphshellRuntime` as the mutation
    /// target.
    pub(crate) captures_in_flight: usize,
}
