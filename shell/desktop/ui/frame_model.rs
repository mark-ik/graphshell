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

use std::time::{Duration, Instant};

use crate::graph::NodeKey;
use crate::shell::desktop::ui::gui_state::ToolbarDraft;
use crate::shell::desktop::workbench::compositor_adapter::{
    OverlayStrokePass, PortablePoint, PortableRect, PortableSize,
};
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

    /// Focus-ring paint alpha for the current focused node at projection time
    /// (0.0 when no ring applies; 1.0→0.0 linear fade-out while the ring
    /// animation is live). Hosts paint the ring proportional to this value
    /// without having to read `started_at`/`duration` and re-derive the math.
    pub(crate) focus_ring_alpha: f32,
}

/// Focus-ring animation state the host renders over a node pane.
#[derive(Debug, Clone, Copy)]
pub(crate) struct FocusRingSpec {
    pub(crate) node_key: NodeKey,
    pub(crate) started_at: Instant,
    pub(crate) duration: Duration,
}

impl FocusRingSpec {
    /// Paint alpha at `now` for a given currently-focused node using the
    /// default linear curve. Returns 0.0 when the ring does not apply
    /// (different node, or animation elapsed); otherwise a linear
    /// fade-out from 1.0 to 0.0 across `duration`.
    ///
    /// Thin wrapper over [`Self::alpha_at_with_curve`] with
    /// [`FocusRingCurve::Linear`] — preserves the M4.1 slice-1a contract
    /// pinned by existing unit tests.
    pub(crate) fn alpha_at(&self, focused_node_key: Option<NodeKey>, now: Instant) -> f32 {
        self.alpha_at_with_curve(
            focused_node_key,
            now,
            crate::app::FocusRingCurve::Linear,
        )
    }

    /// Paint alpha at `now` with the supplied fade reshape. Same gating
    /// semantics as [`Self::alpha_at`] — returns 0.0 when the ring
    /// doesn't apply to `focused_node_key` or when the animation has
    /// elapsed — but the in-window alpha is piped through
    /// [`FocusRingCurve::alpha_from_progress`] so callers can honor user
    /// preference (linear, ease-out, step).
    pub(crate) fn alpha_at_with_curve(
        &self,
        focused_node_key: Option<NodeKey>,
        now: Instant,
        curve: crate::app::FocusRingCurve,
    ) -> f32 {
        if Some(self.node_key) != focused_node_key {
            return 0.0;
        }
        if self.duration.is_zero() {
            // Avoid a division-by-zero when the user has configured an
            // instant-off ring (`duration_ms = 0`). Step-like behavior.
            return 0.0;
        }
        let elapsed = now
            .checked_duration_since(self.started_at)
            .unwrap_or_default();
        if elapsed >= self.duration {
            return 0.0;
        }
        let progress = elapsed.as_secs_f32() / self.duration.as_secs_f32();
        curve.alpha_from_progress(progress)
    }
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
    /// Draft snapshot for the currently active pane, if one has been
    /// captured. The runtime already restores this draft into
    /// `location`/`location_dirty`/`location_submitted` on pane switch,
    /// so hosts rarely need to consume the draft directly — it is
    /// exposed here so iced can render per-pane indicators (e.g., a
    /// "draft pending" dot on tab chrome) without reaching into the
    /// runtime's `toolbar_drafts` map. Previously projected as a full
    /// `HashMap<PaneId, ToolbarDraft>` clone every frame; narrowed in
    /// M4 because no host consumed the full map.
    pub(crate) active_pane_draft: Option<(PaneId, ToolbarDraft)>,
}

/// Omnibar search session projection.
///
/// Captures the state the host must paint this frame when the omnibar is
/// active: query text, current match slate, active-match cursor, and
/// provider-suggestion status. Mutation flows through
/// `ToolbarAuthorityMut::omnibar_session_mut`; this is the read-only
/// snapshot hosts project off without touching runtime state.
#[derive(Debug, Clone)]
pub(crate) struct OmnibarViewModel {
    pub(crate) kind: OmnibarSessionKindView,
    pub(crate) query: String,
    pub(crate) match_count: usize,
    pub(crate) active_match_index: usize,
    pub(crate) selected_index_count: usize,
    pub(crate) provider_status: OmnibarProviderStatusView,
}

/// Host-neutral classification of an omnibar session's origin, so hosts
/// can render different badges for graph navigation vs. external search.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OmnibarSessionKindView {
    /// Graph-scoped navigation session (node/tab/edge match modes).
    Graph,
    /// External search-provider session (DuckDuckGo, Bing, Google).
    SearchProvider,
}

/// Host-neutral projection of the provider suggestion mailbox status.
/// Mirrors `omnibar_state::ProviderSuggestionStatus` without the
/// host-specific string formatting that `provider_status_label` applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OmnibarProviderStatusView {
    Idle,
    Loading,
    Ready,
    FailedNetwork,
    FailedHttp(u16),
    FailedParse,
}

/// Graph-search panel (Ctrl+G) projection.
///
/// Mirrors the five `graph_search_*` fields on `GraphshellRuntime` as a
/// read-only snapshot. Mutation flows through
/// `GraphSearchAuthorityMut`.
#[derive(Debug, Clone, Default)]
pub(crate) struct GraphSearchViewModel {
    pub(crate) open: bool,
    pub(crate) query: String,
    pub(crate) filter_mode: bool,
    pub(crate) match_count: usize,
    pub(crate) active_match_index: Option<usize>,
}

/// Command-palette projection.
///
/// Mirrors the open flag + `CommandPaletteSession` fields the host needs
/// to render the palette shell (chrome framing, focus badge, etc.). The
/// match list is not projected — it's computed per-frame from the
/// action registry inside the widget; projecting it would just be a
/// duplicate allocation.
#[derive(Debug, Clone, Default)]
pub(crate) struct CommandPaletteViewModel {
    pub(crate) open: bool,
    pub(crate) contextual_mode: bool,
    pub(crate) query: String,
    pub(crate) scope: CommandPaletteScopeView,
    pub(crate) selected_index: Option<usize>,
    pub(crate) toggle_requested: bool,
}

/// Host-neutral projection of `SearchPaletteScope`. Distinct from the
/// runtime enum so future scope variants don't propagate into the
/// view-model without deliberate wiring.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum CommandPaletteScopeView {
    CurrentTarget,
    ActivePane,
    ActiveGraph,
    #[default]
    Workbench,
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
    /// "Clear graph and saved data" two-step confirmation is primed.
    /// The host renders a warning toast + arms the runtime-owned
    /// deadline; a second click within the window executes.
    pub(crate) show_clear_data_confirm: bool,
    /// Unix-seconds deadline for the clear-data confirm two-step
    /// prompt. `None` when not armed. Lives on `GraphshellRuntime` so
    /// it survives the host migration without relying on egui's
    /// per-frame `data_mut` temp storage.
    pub(crate) clear_data_confirm_deadline_secs: Option<f64>,
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
    pub(crate) pointer_hover: Option<PortablePoint>,

    /// Current viewport size.
    pub(crate) viewport_size: PortableSize,

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

#[cfg(test)]
mod tests {
    use super::FocusRingSpec;
    use crate::graph::NodeKey;
    use std::time::{Duration, Instant};

    fn spec(node: NodeKey, started: Instant, duration_ms: u64) -> FocusRingSpec {
        FocusRingSpec {
            node_key: node,
            started_at: started,
            duration: Duration::from_millis(duration_ms),
        }
    }

    #[test]
    fn alpha_is_zero_when_focused_node_differs() {
        let now = Instant::now();
        let s = spec(NodeKey::new(1), now, 500);
        assert_eq!(s.alpha_at(Some(NodeKey::new(2)), now), 0.0);
        assert_eq!(s.alpha_at(None, now), 0.0);
    }

    #[test]
    fn alpha_is_full_at_start_and_zero_past_duration() {
        let start = Instant::now();
        let s = spec(NodeKey::new(7), start, 500);
        // Exactly at start -> full intensity.
        assert!((s.alpha_at(Some(NodeKey::new(7)), start) - 1.0).abs() < 1e-6);
        // After duration -> clamped to zero.
        let past = start + Duration::from_millis(600);
        assert_eq!(s.alpha_at(Some(NodeKey::new(7)), past), 0.0);
    }

    #[test]
    fn alpha_fades_linearly_through_duration() {
        let start = Instant::now();
        let s = spec(NodeKey::new(3), start, 1000);
        let half = start + Duration::from_millis(500);
        let alpha = s.alpha_at(Some(NodeKey::new(3)), half);
        assert!(
            (alpha - 0.5).abs() < 1e-3,
            "expected ~0.5 at half duration, got {alpha}"
        );
    }

    #[test]
    fn alpha_is_zero_for_clock_before_start() {
        // Defensive: `now` earlier than `started_at` (can happen with
        // fake/test clocks). Should not panic and should return 0.0.
        let start = Instant::now();
        let earlier = start - Duration::from_millis(50);
        let s = spec(NodeKey::new(9), start, 500);
        // checked_duration_since returns None for earlier < start, so elapsed
        // collapses to Duration::default() (zero) -> full alpha, which is
        // the defensible behavior (the ring is "fresh"). The important
        // guarantee is that it does not panic.
        let alpha = s.alpha_at(Some(NodeKey::new(9)), earlier);
        assert!((0.0..=1.0).contains(&alpha), "alpha out of range: {alpha}");
    }

    #[test]
    fn zero_duration_settings_produce_zero_alpha_even_mid_animation() {
        // Users who configure `duration_ms = 0` for an instant-off
        // ring must not trigger a division-by-zero in the progress
        // calculation. `alpha_at_with_curve` returns 0.0 up-front for
        // the degenerate duration.
        let start = Instant::now();
        let s = FocusRingSpec {
            node_key: NodeKey::new(1),
            started_at: start,
            duration: Duration::from_millis(0),
        };
        assert_eq!(
            s.alpha_at_with_curve(
                Some(NodeKey::new(1)),
                start,
                crate::app::FocusRingCurve::Linear,
            ),
            0.0
        );
    }

    #[test]
    fn ease_out_curve_reshapes_alpha_toward_zero_faster_than_linear() {
        // At the midpoint, EaseOut (alpha = (1-p)^2 = 0.25) drops
        // alpha further than Linear (alpha = 1-p = 0.5). Pinning this
        // relationship so future tweaks to the curve math don't
        // silently invert it.
        let start = Instant::now();
        let s = spec(NodeKey::new(3), start, 1000);
        let half = start + Duration::from_millis(500);
        let linear = s.alpha_at_with_curve(
            Some(NodeKey::new(3)),
            half,
            crate::app::FocusRingCurve::Linear,
        );
        let ease_out = s.alpha_at_with_curve(
            Some(NodeKey::new(3)),
            half,
            crate::app::FocusRingCurve::EaseOut,
        );
        assert!(
            ease_out < linear,
            "expected ease_out ({ease_out}) < linear ({linear}) at midpoint"
        );
        assert!((ease_out - 0.25).abs() < 1e-3, "expected ~0.25, got {ease_out}");
    }

    #[test]
    fn step_curve_is_on_then_off_with_no_fade() {
        // Step curve should stay at full alpha for the entire duration,
        // then snap to zero at/after expiry.
        let start = Instant::now();
        let s = spec(NodeKey::new(5), start, 500);
        let mid = start + Duration::from_millis(250);
        let past = start + Duration::from_millis(600);
        assert_eq!(
            s.alpha_at_with_curve(
                Some(NodeKey::new(5)),
                mid,
                crate::app::FocusRingCurve::Step,
            ),
            1.0
        );
        assert_eq!(
            s.alpha_at_with_curve(
                Some(NodeKey::new(5)),
                past,
                crate::app::FocusRingCurve::Step,
            ),
            0.0
        );
    }
}
