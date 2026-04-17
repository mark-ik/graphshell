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
//! ## Residual egui leaks
//!
//! `egui::Rect`, `egui::Pos2`, and `egui::Vec2` appear in several fields
//! because the data they describe (tile rects, pointer position, viewport
//! size) currently originates from egui primitives. The M3.5 doc flags this
//! as a "cosmetic leak" to be cleaned up in a follow-on pass; iced could
//! implement these ports against the same types via simple conversions, so
//! the leak does not block a second host.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::graph::NodeKey;
use crate::shell::desktop::workbench::compositor_adapter::OverlayStrokePass;
use crate::shell::desktop::workbench::pane_model::PaneId;
use crate::shell::desktop::workbench::ux_replay::{HostEvent, ModifiersState};
use graph_tree::{OwnedTreeRow, SplitBoundary, TabEntry};
use servo::LoadStatus;

// ---------------------------------------------------------------------------
// Runtime output: FrameViewModel
// ---------------------------------------------------------------------------

/// Per-frame snapshot produced by `GraphshellRuntime` for the host to paint.
///
/// All fields are read-only from the host's perspective. The host may
/// rasterize, lay out, or cache derived quantities, but must not mutate the
/// model — any feedback flows back through `HostPorts` / [`FrameHostInput`].
///
/// (No `Debug` derive because `OverlayStrokePass` contains non-Debug types
/// like `egui::Stroke` today. Once the overlay descriptors are cleaned up of
/// residual egui types, this can gain `Debug`.)
#[derive(Clone, Default)]
pub(crate) struct FrameViewModel {
    /// Visible panes with their screen rects, in stable iteration order.
    pub(crate) active_pane_rects: Vec<(PaneId, NodeKey, egui::Rect)>,

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
}

/// Aggregate focus state exposed to the host.
#[derive(Debug, Clone, Default)]
pub(crate) struct FocusViewModel {
    /// Currently focused node (for node-pane focus; None for graph-surface
    /// focus or no focus).
    pub(crate) focused_node: Option<NodeKey>,

    /// Whether the graph canvas has focus (as opposed to a node pane or chrome).
    pub(crate) graph_surface_focused: bool,

    /// Active focus-ring animation, if any.
    pub(crate) focus_ring: Option<FocusRingSpec>,
}

/// Focus-ring animation state the host renders over a node pane.
#[derive(Debug, Clone, Copy)]
pub(crate) struct FocusRingSpec {
    pub(crate) node_key: NodeKey,
    pub(crate) started_at: Instant,
    pub(crate) duration: Duration,
}

/// Toolbar / location-bar projection for the host.
#[derive(Debug, Clone, Default)]
pub(crate) struct ToolbarViewModel {
    pub(crate) location: String,
    pub(crate) location_dirty: bool,
    pub(crate) location_submitted: bool,
    pub(crate) load_status: Option<LoadStatus>,
    pub(crate) status_text: Option<String>,
    pub(crate) can_go_back: bool,
    pub(crate) can_go_forward: bool,
    /// Per-pane location-bar drafts. The host uses these to populate the
    /// input widget when the active pane changes.
    pub(crate) per_pane_drafts: HashMap<PaneId, ToolbarDraftSnapshot>,
}

/// Immutable snapshot of a per-pane toolbar draft, safe to share with the host.
#[derive(Debug, Clone, Default)]
pub(crate) struct ToolbarDraftSnapshot {
    pub(crate) location: String,
    pub(crate) location_dirty: bool,
    pub(crate) location_submitted: bool,
}

/// Which dialogs / overlays are open. Flags for booleans; detailed state for
/// dialogs with content.
#[derive(Debug, Clone, Default)]
pub(crate) struct DialogsViewModel {
    pub(crate) bookmark_import_open: bool,
    pub(crate) command_palette_toggle_requested: bool,
    pub(crate) show_command_palette: bool,
    pub(crate) show_context_palette: bool,
    pub(crate) show_help_panel: bool,
    pub(crate) show_radial_menu: bool,
    pub(crate) show_settings_overlay: bool,
    pub(crate) show_clip_inspector: bool,
    pub(crate) show_scene_overlay: bool,
}

/// Host-neutral toast spec. The host maps this onto its notification system
/// (`egui_notify::Toasts`, iced's toast widget, etc.).
#[derive(Debug, Clone)]
pub(crate) struct ToastSpec {
    pub(crate) severity: ToastSeverity,
    pub(crate) message: String,
    pub(crate) duration: Option<Duration>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToastSeverity {
    Info,
    Success,
    Warning,
    Error,
}

/// Host-neutral degraded-receipt spec. Mirrors the private
/// `DegradedReceipt` inside `tile_compositor`; the view-model exposes it so
/// any host can render the receipts without reaching into compositor internals.
#[derive(Debug, Clone)]
pub(crate) struct DegradedReceiptSpec {
    pub(crate) tile_rect: egui::Rect,
    pub(crate) message: String,
}

// ---------------------------------------------------------------------------
// Host input: FrameHostInput
// ---------------------------------------------------------------------------

/// Per-frame input bundle flowing from the host into the runtime.
///
/// The runtime consumes this, advances state, and produces a
/// [`FrameViewModel`]. Any host-specific side effects (webview creation
/// requests, clipboard writes, focus handoffs) travel back through
/// `HostPorts` rather than through the view model.
#[derive(Debug, Clone, Default)]
pub(crate) struct FrameHostInput {
    /// Host-neutral events translated from native input (keyboard, pointer,
    /// scroll, focus, resize, synthesized command-surface actions).
    pub(crate) events: Vec<HostEvent>,

    /// Current pointer hover position in screen coordinates (if any).
    pub(crate) pointer_hover: Option<egui::Pos2>,

    /// Current viewport size.
    pub(crate) viewport_size: egui::Vec2,

    /// Whether a host-owned widget currently wants keyboard input
    /// (affects whether the runtime routes keyboard events to content).
    pub(crate) wants_keyboard: bool,

    /// Whether a host-owned widget currently wants pointer input.
    pub(crate) wants_pointer: bool,

    /// Active keyboard modifier state this frame.
    pub(crate) modifiers: ModifiersState,

    /// True when the host observed at least one native input event this
    /// frame. Used by the runtime to mark a user gesture for idle-watchdog
    /// timing. Populated even when `events` is still empty during the
    /// partial event-translation migration.
    pub(crate) had_input_events: bool,
}
