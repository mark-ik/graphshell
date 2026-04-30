/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! iced `Program` surface around [`IcedHost`] — M5.2 + M5.3.
//!
//! Third layer in the iced host stack:
//!
//! 1. [`GraphshellRuntime`] — host-neutral state, shared with `EguiHost`.
//! 2. [`super::iced_host::IcedHost`] — iced-side adapter around the runtime.
//! 3. [`IcedApp`] *(this module)* — iced `Program`-shaped type iced's event
//!    loop actually drives.
//!
//! **Scope (Slice 6 / Stage A+E)**: Slices 3-4 wired the Frame split
//! tree, Navigator host structural layout, and the `gs::TileTabs`
//! widget; Slice 5 added real `gs::Modal` and `gs::ContextMenu`
//! widgets in `graphshell-iced-widgets`; Slice 6 wires the Command
//! Palette (`Ctrl+Shift+P`) and Node Finder (`Ctrl+P`) modals into
//! `IcedApp` using `gs::Modal`. Both modals carry widget-local state
//! (open flag, query, focused index) and stub-empty result lists —
//! the `ActionRegistry` / `NodeFinderViewModel` data sources are
//! wired in a downstream slice. Mutual exclusion (opening one closes
//! the other) and Escape / click-outside dismissal are wired today.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use euclid::default::Vector2D;
use graph_canvas::camera::CanvasCamera;
use iced::time;
use iced::widget::{button, canvas, column, container, pane_grid, row, rule, scrollable, text, text_input};
use iced::{Element, Length, Subscription, Task};
use graphshell_iced_widgets::{Modal, TileTab, TileTabs};

/// Frame interval for the runtime tick `Subscription`. ~60 Hz. Per
/// [`iced_composition_skeleton_spec.md` §1.5](
/// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_composition_skeleton_spec.md):
/// the runtime tick must run from a `time::every` Subscription, not
/// poll inside `view`, so time-based runtime state (focus-ring fades,
/// recipe-result drains, lifecycle transitions) advances even without
/// user input. Stage A done condition.
const RUNTIME_TICK_INTERVAL: Duration = Duration::from_millis(16);

/// Stable widget id for the omnibar text input. Addressed by the
/// `OmnibarFocus` handler via `iced::widget::operation::focus`. Any
/// future iced widget that wants programmatic focus gets a similar
/// named id so the id is portable across `view` rebuilds.
const OMNIBAR_INPUT_ID: &str = "graphshell:omnibar_input";

// ---------------------------------------------------------------------------
// Omnibar (CommandBar slot) state — Slice 2
// ---------------------------------------------------------------------------

/// Rendering mode for the [`OmnibarSession`] (CommandBar slot).
///
/// - **Display**: the Navigator-projected breadcrumb/address is shown
///   read-only. Transitions to `Input` on `Ctrl+L` or click.
/// - **Input**: an active `text_input` accepts URL or search text.
///   Transitions back to `Display` on submit or Escape.
///
/// Per [`iced_omnibar_spec.md` §2](
/// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_omnibar_spec.md).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum OmnibarMode {
    Display,
    Input,
}

/// Widget-local state for the omnibar (CommandBar slot).
///
/// `draft` and `mode` live here, never in the runtime — the omnibar
/// must never write to graph truth (coherence guarantee §10 of the
/// omnibar spec). The runtime's `FrameViewModel.toolbar.location` is
/// the read-only address projected for Display mode.
#[derive(Debug, Clone)]
pub(crate) struct OmnibarSession {
    pub(crate) mode: OmnibarMode,
    /// Current input text. Populated from `FrameViewModel.toolbar.location`
    /// when transitioning to Input mode (if empty), then updated by
    /// `OmnibarInput`. Cleared on submit or Escape.
    pub(crate) draft: String,
    /// Widget id held before the omnibar captured focus; restored on
    /// dismiss, submit, or Escape. Unpopulated in Slice 2 — full
    /// focus-restore lands when iced exposes a focus-query operation.
    pub(crate) focus_token: Option<iced::widget::Id>,
}

impl Default for OmnibarSession {
    fn default() -> Self {
        Self {
            mode: OmnibarMode::Display,
            draft: String::new(),
            focus_token: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Frame split-tree state — Slice 3 (Stage B)
// ---------------------------------------------------------------------------

/// Stable application-level identity for a Pane.
///
/// Distinct from iced's opaque `pane_grid::Pane` handle — `PaneId` is
/// a durable identifier that survives pane rearrangement and can be
/// round-tripped through `HostIntent`. The iced handle is used for
/// layout operations; `PaneId` is used for domain semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct PaneId(u64);

impl PaneId {
    fn next() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        Self(NEXT.fetch_add(1, Ordering::Relaxed))
    }
}

/// Whether a Pane renders a tile-tab bar over active tiles (Tile) or
/// a canvas instance scoped to the Pane's graphlet (Canvas).
///
/// Per [`iced_composition_skeleton_spec.md` §4](
/// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_composition_skeleton_spec.md).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PaneType {
    /// Renders `gs::TileTabs` over active tiles + tile body. S4 sub-
    /// slice materializes the TileTabs widget; Slice 3 renders a stub.
    Tile,
    /// Renders a `canvas::Canvas` instance scoped to the Pane's graphlet
    /// (using the existing `GraphCanvasProgram`).
    Canvas,
}

/// Metadata stored inside `pane_grid::State<PaneMeta>` per Pane.
///
/// iced's `pane_grid::Pane` is an opaque layout handle; `PaneMeta` is
/// the application-level payload. The split-tree topology lives in
/// iced's `State`; domain semantics live here.
#[derive(Debug, Clone)]
pub(crate) struct PaneMeta {
    pub pane_id: PaneId,
    pub pane_type: PaneType,
}

/// The Frame's split-tree authority. Wraps `pane_grid::State<PaneMeta>`
/// and the currently-focused pane handle.
///
/// Per [`iced_composition_skeleton_spec.md` §3](
/// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_composition_skeleton_spec.md):
/// `pane_grid::State<PaneMeta>` *is* the split-tree authority. No
/// side-structure mirrors it.
///
/// `base_layer_active` tracks whether all user-facing Panes have been
/// closed. It is necessary because `pane_grid::State::close` cannot
/// reduce the state to zero panes — the last `close` call is a no-op
/// from iced's perspective, leaving one inert Pane inside the state.
/// When `base_layer_active` is `true`, the render path skips the
/// `pane_grid` widget and shows the canvas base layer instead.
pub(crate) struct FrameState {
    pub split_state: pane_grid::State<PaneMeta>,
    /// `true` iff all user-visible Panes have been closed and the
    /// canvas base layer is the active render path.
    pub base_layer_active: bool,
    pub focused_pane: Option<pane_grid::Pane>,
}

impl FrameState {
    /// Initialize with one Canvas pane — the default launch state.
    /// An empty Frame (zero Panes) would show only the canvas base
    /// layer; pre-seeding with one Canvas pane makes the pane_grid
    /// visible immediately for Slice 3 verification.
    fn new() -> Self {
        let first = PaneMeta {
            pane_id: PaneId::next(),
            pane_type: PaneType::Canvas,
        };
        let (split_state, _handle) = pane_grid::State::new(first);
        Self {
            split_state,
            base_layer_active: false,
            focused_pane: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Navigator host state — Slice 4 (structural layout)
// ---------------------------------------------------------------------------

/// Tracks which Navigator host slots are currently visible.
///
/// Per [`iced_composition_skeleton_spec.md` §2](
/// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_composition_skeleton_spec.md)
/// and [`NAVIGATOR.md` §11](
/// ../../../design_docs/graphshell_docs/implementation_strategy/navigator/NAVIGATOR.md):
/// each slot is conditional. The four anchors are Left (sidebar),
/// Right (sidebar), Top (toolbar), Bottom (toolbar). Presentation Bucket
/// content inside each host is a stub in this slice; full data-driven
/// buckets land once the Navigator domain layer is wired.
pub(crate) struct NavigatorState {
    pub show_left: bool,
    pub show_right: bool,
    pub show_top: bool,
    pub show_bottom: bool,
}

impl Default for NavigatorState {
    fn default() -> Self {
        Self {
            show_left: true,
            show_right: false,
            show_top: false,
            show_bottom: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Command Palette + Node Finder — Slice 5/6 (Modal-backed overlays)
// ---------------------------------------------------------------------------

/// Stable widget id for the command palette text input. Addressed by
/// `iced::widget::operation::focus` on `PaletteOpen`.
const PALETTE_INPUT_ID: &str = "graphshell:command_palette_input";

/// Stable widget id for the node finder text input.
const NODE_FINDER_INPUT_ID: &str = "graphshell:node_finder_input";

/// Why the Command Palette was opened. Recorded for diagnostics
/// provenance per
/// [`iced_command_palette_spec.md` §2.1](
/// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_command_palette_spec.md).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PaletteOrigin {
    KeyboardShortcut,
    TriggerButton,
    ContextFallback,
    Programmatic,
}

/// Widget-local state for the Command Palette modal. Per
/// [`iced_command_palette_spec.md` §2.3](
/// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_command_palette_spec.md).
///
/// Slice 6 carries the open-state, query, and focused-row index. The
/// `ranked_actions` list is empty until the `ActionRegistry` is wired
/// in a downstream slice; the Modal renders a "no actions" footer when
/// the list is empty rather than crashing.
#[derive(Debug, Clone)]
pub(crate) struct CommandPaletteState {
    pub(crate) is_open: bool,
    pub(crate) origin: PaletteOrigin,
    pub(crate) query: String,
    pub(crate) focused_index: Option<usize>,
}

impl Default for CommandPaletteState {
    fn default() -> Self {
        Self {
            is_open: false,
            origin: PaletteOrigin::KeyboardShortcut,
            query: String::new(),
            focused_index: None,
        }
    }
}

/// Why the Node Finder was opened. Per
/// [`iced_node_finder_spec.md` §5](
/// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_node_finder_spec.md).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NodeFinderOrigin {
    KeyboardShortcut,
    OmnibarRoute(String),
    TreeSpineRow,
    Programmatic,
}

/// Widget-local state for the Node Finder modal. Per
/// [`iced_node_finder_spec.md` §5](
/// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_node_finder_spec.md).
///
/// Slice 6 carries the open-state and query. The `results` list is
/// empty until the runtime exposes a `NodeFinderViewModel`; downstream
/// slices wire fuzzy-match ranking against graph truth.
#[derive(Debug, Clone)]
pub(crate) struct NodeFinderState {
    pub(crate) is_open: bool,
    pub(crate) origin: NodeFinderOrigin,
    pub(crate) query: String,
    pub(crate) focused_index: Option<usize>,
}

impl Default for NodeFinderState {
    fn default() -> Self {
        Self {
            is_open: false,
            origin: NodeFinderOrigin::KeyboardShortcut,
            query: String::new(),
            focused_index: None,
        }
    }
}

// ---------------------------------------------------------------------------
// IcedApp
// ---------------------------------------------------------------------------

use crate::shell::desktop::ui::gui_state::GraphshellRuntime;
use crate::shell::desktop::ui::iced_graph_canvas::{
    GraphCanvasProgram, from_graph_app as graph_canvas_from_app,
};
use crate::shell::desktop::ui::iced_host::IcedHost;
use graphshell_core::host_event::HostEvent;
use graphshell_runtime::{FrameHostInput, FrameViewModel, ToastSeverity};

/// App-level state held across iced frames.
///
/// Owns the `IcedHost` adapter (which in turn owns the shared runtime)
/// plus the most recent `FrameViewModel` the runtime produced — cached
/// so the next `view` call can read projected state (focus, toolbar,
/// etc.) without re-running `tick`.
pub(crate) struct IcedApp {
    pub(crate) host: IcedHost,
    /// Last `FrameViewModel` produced by `runtime.tick`. `None` before
    /// the first tick; populated lazily after the first real input or
    /// explicit `Tick` message.
    pub(crate) last_view_model: Option<FrameViewModel>,
    /// CommandBar-slot omnibar state. Per
    /// [`iced_omnibar_spec.md` §4](
    /// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_omnibar_spec.md):
    /// draft and mode live here; never written to graph truth.
    pub(crate) omnibar: OmnibarSession,
    /// Frame split-tree authority. Per
    /// [`iced_composition_skeleton_spec.md` §3](
    /// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_composition_skeleton_spec.md):
    /// `pane_grid::State<PaneMeta>` is the sole source of truth for
    /// split topology; no sidecar mirrors it.
    pub(crate) frame: FrameState,
    /// Which Navigator host slots are currently visible. Drives the
    /// conditional slot layout in `view()` per spec §2.
    pub(crate) navigator: NavigatorState,
    /// Command Palette modal state (Ctrl+Shift+P). Per
    /// [`iced_command_palette_spec.md`](
    /// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_command_palette_spec.md).
    pub(crate) command_palette: CommandPaletteState,
    /// Node Finder modal state (Ctrl+P). Per
    /// [`iced_node_finder_spec.md`](
    /// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_node_finder_spec.md).
    pub(crate) node_finder: NodeFinderState,
}

/// Messages iced pushes into `IcedApp::update`.
#[derive(Debug, Clone)]
pub(crate) enum Message {
    /// Frame pulse with no input — drives `tick` against an empty
    /// `FrameHostInput` so the runtime can advance time-based state
    /// (focus-ring fades, etc.) even in the absence of user input.
    Tick,
    /// Raw iced event from the event subscription. Translated into a
    /// `HostEvent` via `iced_events::from_iced_event` and ticked
    /// through the runtime immediately. Also caches cursor-position and
    /// modifier state onto `IcedHost` so the `HostInputPort` getters
    /// surface live values at tick time.
    IcedEvent(iced::Event),
    /// Camera state mutated in the graph canvas. Published by
    /// `GraphCanvasProgram::update` after wheel-zoom or drag-pan.
    /// `update` persists the new values into the runtime's per-view
    /// camera map so other surfaces see the same camera state.
    CameraChanged { pan: Vector2D<f32>, zoom: f32 },
    /// User clicked a link inside a middlenet-rendered document.
    /// Routes through `HostIntent::CreateNodeAtUrl`; spatial-browsing
    /// semantics (links open as new nodes, not navigate-in-place).
    LinkActivated(middlenet_engine::document::LinkTarget),

    // --- Omnibar (CommandBar slot) messages — Slice 2 ---

    /// Display → Input mode. Triggered by `Ctrl+L` or a click into
    /// the omnibar area. Captures keyboard focus on `OMNIBAR_INPUT_ID`.
    OmnibarFocus,
    /// Input → Display mode, no submit. Fired when the text input
    /// loses focus to another widget (e.g. clicking outside). Slice 2:
    /// not yet wired from `text_input` (vendored iced lacks `on_blur`);
    /// wired in S4 when available or via a focus-change event.
    OmnibarBlur,
    /// Text edited in the omnibar text input. Updates `omnibar.draft`
    /// only — no tick, no runtime mutation.
    OmnibarInput(String),
    /// Enter pressed in the omnibar text input.
    /// URL-shaped draft → `HostIntent::CreateNodeAtUrl`.
    /// Non-URL-shaped draft → `OmnibarRouteToNodeFinder`.
    OmnibarSubmit,
    /// Escape pressed while the omnibar is in Input mode. Clears the
    /// draft and returns to Display mode.
    OmnibarKeyEscape,
    /// Non-URL-shaped query routed from `OmnibarSubmit`. Clears the
    /// omnibar and returns to Display mode. Node Finder activation is
    /// a stub in Slice 2; the full surface lands in S4.
    OmnibarRouteToNodeFinder(String),

    // --- Frame split-tree (pane_grid) messages — Slice 3 ---
    // Per [`iced_composition_skeleton_spec.md` §3](
    // ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_composition_skeleton_spec.md).

    /// A Pane was clicked — record it as the focused Pane.
    PaneFocused(pane_grid::Pane),
    /// A drag-and-drop event from `pane_grid`. On `Dropped`, the state
    /// is updated via `pane_grid::State::drop`. Picked / Canceled are
    /// acknowledged silently (layout state is inside iced's State).
    PaneGridDragged(pane_grid::DragEvent),
    /// A resize event from `pane_grid`. Applies the new split ratio via
    /// `pane_grid::State::resize`.
    PaneGridResized(pane_grid::ResizeEvent),
    /// Close the given Pane. If it was the last Pane in the Frame, the
    /// split tree becomes empty and the canvas base layer is shown.
    ClosePane(pane_grid::Pane),

    // --- Command Palette messages — Slice 6 ---
    // Per [`iced_command_palette_spec.md`](
    // ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_command_palette_spec.md).

    /// Open the Command Palette modal with the given origin. Captures
    /// keyboard focus on the palette text input.
    PaletteOpen { origin: PaletteOrigin },
    /// Query text edited in the palette text input.
    PaletteQuery(String),
    /// Close the palette and discard query/focus state. Click-outside
    /// (via `Modal::on_blur`) and Escape both fire this.
    PaletteCloseAndRestoreFocus,
    /// User picked the entry at the given index in the ranked-action
    /// list. Slice 6: stub-acks via toast; downstream slice routes the
    /// selected `ActionId` to `HostIntent::Action`.
    PaletteActionSelected(usize),

    // --- Node Finder messages — Slice 6 ---
    // Per [`iced_node_finder_spec.md`](
    // ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_node_finder_spec.md).

    /// Open the Node Finder modal with the given origin. Captures
    /// keyboard focus on the finder text input.
    NodeFinderOpen { origin: NodeFinderOrigin },
    /// Query text edited in the finder text input.
    NodeFinderQuery(String),
    /// Close the finder and discard query/focus state.
    NodeFinderCloseAndRestoreFocus,
    /// User picked the result at the given index. Slice 6: stub-acks
    /// via toast; downstream slice routes to `WorkbenchIntent::OpenNode`.
    NodeFinderResultSelected(usize),
}

impl IcedApp {
    /// Construct an app whose `IcedHost` wraps the supplied runtime.
    pub(crate) fn with_runtime(runtime: GraphshellRuntime) -> Self {
        Self {
            host: IcedHost::with_runtime(runtime),
            last_view_model: None,
            omnibar: OmnibarSession::default(),
            frame: FrameState::new(),
            navigator: NavigatorState::default(),
            command_palette: CommandPaletteState::default(),
            node_finder: NodeFinderState::default(),
        }
    }

    fn title(&self) -> String {
        "Graphshell — iced host (M5)".to_string()
    }

    /// Drive one tick of the runtime with the supplied host-neutral
    /// events. Extracted so both `Message::Tick` and
    /// `Message::IcedEvent` converge on the same tick path.
    fn tick_with_events(&mut self, events: Vec<HostEvent>) {
        let had_input_events = !events.is_empty();
        let input = FrameHostInput {
            events,
            had_input_events,
            ..FrameHostInput::default()
        };
        let view_model = self.host.tick_with_input(&input);
        self.last_view_model = Some(view_model);
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Tick => {
                self.tick_with_events(Vec::new());
                Task::none()
            }
            Message::IcedEvent(event) => {
                // Cache cursor + modifier state on the host before
                // ticking so `HostInputPort::pointer_hover_position`
                // and `HostInputPort::modifiers` surface live values
                // inside this tick.
                self.cache_host_input_state(&event);

                // App-level hotkeys intercepted before runtime translation.
                // Order matters: more-specific modifier combos first so
                // Ctrl+Shift+P doesn't fall through to Ctrl+P.
                if is_command_palette_hotkey(&event) {
                    return Task::done(Message::PaletteOpen {
                        origin: PaletteOrigin::KeyboardShortcut,
                    });
                }
                if is_node_finder_hotkey(&event) {
                    return Task::done(Message::NodeFinderOpen {
                        origin: NodeFinderOrigin::KeyboardShortcut,
                    });
                }
                if is_omnibar_focus_hotkey(&event) {
                    return Task::done(Message::OmnibarFocus);
                }
                // Escape closes whichever overlay is currently open.
                // Order: palette → node_finder → omnibar.
                if is_escape_key(&event) {
                    if self.command_palette.is_open {
                        return Task::done(Message::PaletteCloseAndRestoreFocus);
                    }
                    if self.node_finder.is_open {
                        return Task::done(Message::NodeFinderCloseAndRestoreFocus);
                    }
                    if self.omnibar.mode == OmnibarMode::Input {
                        return Task::done(Message::OmnibarKeyEscape);
                    }
                }

                // Translate; drop events with no host-neutral equivalent.
                // Only tick if something translated — avoids spurious
                // empty-input ticks per iced event.
                let events: Vec<HostEvent> = super::iced_events::from_iced_event(&event)
                    .into_iter()
                    .collect();
                if !events.is_empty() {
                    self.tick_with_events(events);
                }
                Task::none()
            }
            Message::CameraChanged { pan, zoom } => {
                let view_id = self.host.view_id;
                let entry = self
                    .host
                    .runtime
                    .graph_app
                    .workspace
                    .graph_runtime
                    .canvas_cameras
                    .entry(view_id)
                    .or_insert_with(CanvasCamera::default);
                entry.pan = pan;
                entry.zoom = zoom;
                entry.pan_velocity = Vector2D::zero();
                Task::none()
            }
            Message::LinkActivated(target) => {
                let href = target.href.clone();
                if !href.is_empty() {
                    self.queue_create_node_at_url(href.clone());
                    self.host.toast_queue.push(graphshell_runtime::ToastSpec {
                        severity: ToastSeverity::Info,
                        message: format!("link → {href}"),
                        duration: None,
                    });
                }
                Task::none()
            }

            // --- Omnibar handlers ---

            Message::OmnibarFocus => {
                self.omnibar.mode = OmnibarMode::Input;
                if self.omnibar.draft.is_empty() {
                    if let Some(vm) = self.last_view_model.as_ref() {
                        self.omnibar.draft = vm.toolbar.location.clone();
                    }
                }
                iced::widget::operation::focus(iced::widget::Id::new(OMNIBAR_INPUT_ID))
            }
            Message::OmnibarBlur => {
                self.omnibar.mode = OmnibarMode::Display;
                Task::none()
            }
            Message::OmnibarInput(draft) => {
                self.omnibar.draft = draft;
                Task::none()
            }
            Message::OmnibarSubmit => {
                let draft = std::mem::take(&mut self.omnibar.draft);
                self.omnibar.mode = OmnibarMode::Display;
                if draft.is_empty() {
                    return Task::none();
                }
                if is_url_shaped(&draft) {
                    self.queue_create_node_at_url(draft.clone());
                    self.host.toast_queue.push(graphshell_runtime::ToastSpec {
                        severity: ToastSeverity::Success,
                        message: format!("opened: {draft}"),
                        duration: None,
                    });
                    Task::none()
                } else {
                    Task::done(Message::OmnibarRouteToNodeFinder(draft))
                }
            }
            Message::OmnibarKeyEscape => {
                self.omnibar.mode = OmnibarMode::Display;
                self.omnibar.draft.clear();
                Task::none()
            }
            Message::OmnibarRouteToNodeFinder(query) => {
                self.omnibar.draft.clear();
                self.omnibar.mode = OmnibarMode::Display;
                // Open the Node Finder pre-seeded with the routed query.
                self.node_finder.is_open = true;
                self.node_finder.origin = NodeFinderOrigin::OmnibarRoute(query.clone());
                self.node_finder.query = query;
                self.node_finder.focused_index = None;
                iced::widget::operation::focus(iced::widget::Id::new(NODE_FINDER_INPUT_ID))
            }

            // --- Frame split-tree handlers ---

            Message::PaneFocused(pane) => {
                self.frame.focused_pane = Some(pane);
                Task::none()
            }
            Message::PaneGridDragged(event) => {
                match event {
                    pane_grid::DragEvent::Picked { .. } => {}
                    pane_grid::DragEvent::Dropped { pane, target } => {
                        // `State::drop` handles all target variants
                        // (edge-of-grid, center-of-pane, edge-of-pane)
                        // with the correct split axis derived from the
                        // drop region (per pane_grid §3.1 defaults).
                        self.frame.split_state.drop(pane, target);
                    }
                    pane_grid::DragEvent::Canceled { .. } => {}
                }
                Task::none()
            }
            Message::PaneGridResized(event) => {
                self.frame.split_state.resize(event.split, event.ratio);
                Task::none()
            }
            Message::ClosePane(pane) => {
                if self.frame.focused_pane == Some(pane) {
                    self.frame.focused_pane = None;
                }
                if !self.frame.base_layer_active {
                    if self.frame.split_state.len() <= 1 {
                        // `pane_grid::State::close` is a no-op on the last
                        // pane — it can't reduce the state to zero. Set the
                        // flag instead so the render path shows the canvas
                        // base layer.
                        self.frame.base_layer_active = true;
                    } else {
                        let _ = self.frame.split_state.close(pane);
                    }
                }
                Task::none()
            }

            // --- Command Palette handlers ---

            Message::PaletteOpen { origin } => {
                // Opening the palette closes the node finder (mutually
                // exclusive overlays per the canonical specs).
                self.node_finder.is_open = false;
                self.command_palette.is_open = true;
                self.command_palette.origin = origin;
                self.command_palette.query.clear();
                self.command_palette.focused_index = None;
                iced::widget::operation::focus(iced::widget::Id::new(PALETTE_INPUT_ID))
            }
            Message::PaletteQuery(query) => {
                self.command_palette.query = query;
                // Slice 6 stub: real ranking happens once ActionRegistry
                // is wired. For now, focus simply resets to None.
                self.command_palette.focused_index = None;
                Task::none()
            }
            Message::PaletteCloseAndRestoreFocus => {
                self.command_palette.is_open = false;
                self.command_palette.query.clear();
                self.command_palette.focused_index = None;
                Task::none()
            }
            Message::PaletteActionSelected(idx) => {
                // Slice 6 stub: ack via toast. Downstream slice will look
                // up the RankedAction at `idx` and emit HostIntent::Action.
                self.host.toast_queue.push(graphshell_runtime::ToastSpec {
                    severity: ToastSeverity::Info,
                    message: format!("palette action #{idx} (stub)"),
                    duration: None,
                });
                self.command_palette.is_open = false;
                self.command_palette.query.clear();
                self.command_palette.focused_index = None;
                Task::none()
            }

            // --- Node Finder handlers ---

            Message::NodeFinderOpen { origin } => {
                // Mutually exclusive with the command palette.
                self.command_palette.is_open = false;
                self.node_finder.is_open = true;
                self.node_finder.origin = origin;
                self.node_finder.query.clear();
                self.node_finder.focused_index = None;
                iced::widget::operation::focus(iced::widget::Id::new(NODE_FINDER_INPUT_ID))
            }
            Message::NodeFinderQuery(query) => {
                self.node_finder.query = query;
                self.node_finder.focused_index = None;
                Task::none()
            }
            Message::NodeFinderCloseAndRestoreFocus => {
                self.node_finder.is_open = false;
                self.node_finder.query.clear();
                self.node_finder.focused_index = None;
                Task::none()
            }
            Message::NodeFinderResultSelected(idx) => {
                // Slice 6 stub: ack via toast. Downstream slice will
                // look up the NodeKey at `idx` and emit
                // WorkbenchIntent::OpenNode.
                self.host.toast_queue.push(graphshell_runtime::ToastSpec {
                    severity: ToastSeverity::Info,
                    message: format!("node finder result #{idx} (stub)"),
                    duration: None,
                });
                self.node_finder.is_open = false;
                self.node_finder.query.clear();
                self.node_finder.focused_index = None;
                Task::none()
            }
        }
    }

    /// Queue a `HostIntent::CreateNodeAtUrl` for the next tick and
    /// drive it. Shared between `OmnibarSubmit` and `LinkActivated`
    /// so both routes flow through the same sanctioned-writes path
    /// per §12.17 of the iced jump-ship plan.
    fn queue_create_node_at_url(&mut self, url: String) {
        self.host.pending_host_intents.push(
            graphshell_core::shell_state::host_intent::HostIntent::CreateNodeAtUrl {
                url,
                position: graphshell_core::geometry::PortablePoint::new(0.0, 0.0),
            },
        );
        self.tick_with_events(Vec::new());
    }

    /// Update `IcedHost.cursor_position` / `IcedHost.modifiers` from
    /// an incoming iced event so the `HostInputPort` getters surface
    /// live values on the next tick.
    fn cache_host_input_state(&mut self, event: &iced::Event) {
        match event {
            iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
                self.host.cursor_position = Some(*position);
            }
            iced::Event::Mouse(iced::mouse::Event::CursorLeft) => {
                self.host.cursor_position = None;
            }
            iced::Event::Keyboard(iced::keyboard::Event::ModifiersChanged(mods)) => {
                self.host.modifiers = *mods;
            }
            iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { modifiers, .. })
            | iced::Event::Keyboard(iced::keyboard::Event::KeyReleased { modifiers, .. }) => {
                self.host.modifiers = *modifiers;
            }
            _ => {}
        }
    }

    /// Subscribe to iced's native event stream and a 60 Hz runtime
    /// tick. The timer subscription drives `Message::Tick` at
    /// `RUNTIME_TICK_INTERVAL` so the runtime advances time-based
    /// state even without user input — Stage A done condition per
    /// [`iced_composition_skeleton_spec.md` §1.5](
    /// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_composition_skeleton_spec.md).
    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            iced::event::listen().map(Message::IcedEvent),
            time::every(RUNTIME_TICK_INTERVAL).map(|_instant| Message::Tick),
        ])
    }

    fn view(&self) -> Element<'_, Message> {
        let command_bar = render_command_bar(self);
        let toast_stack = render_toast_stack(&self.host.toast_queue);

        // FrameSplitTree slot: pane_grid when Panes exist; canvas base
        // layer when the Frame is empty (per spec §2.3).
        let frame_area = render_frame_split_tree(self);

        // Main content row: optional left Navigator | Frame | optional right Navigator.
        // Per [`iced_composition_skeleton_spec.md` §2](spec).
        let mut main_row_children: Vec<Element<'_, Message>> = Vec::new();
        if self.navigator.show_left {
            main_row_children.push(render_navigator_host(self, NavigatorAnchor::Left));
        }
        main_row_children.push(frame_area);
        if self.navigator.show_right {
            main_row_children.push(render_navigator_host(self, NavigatorAnchor::Right));
        }
        let main_row = row(main_row_children)
            .spacing(0)
            .height(Length::Fill);

        // Full-height column: optional top | main row | optional bottom | toasts.
        let mut body_children: Vec<Element<'_, Message>> = Vec::new();
        body_children.push(command_bar);
        if self.navigator.show_top {
            body_children.push(render_navigator_host(self, NavigatorAnchor::Top));
        }
        body_children.push(main_row.into());
        if self.navigator.show_bottom {
            body_children.push(render_navigator_host(self, NavigatorAnchor::Bottom));
        }
        body_children.push(toast_stack.into());

        let body: Element<'_, Message> = container(column(body_children).spacing(0))
            .padding(0)
            .width(Length::Fill)
            .height(Length::Fill)
            .into();

        // Layer modal overlays on top of the body. Both modals are
        // mutually exclusive at the state level (opening one closes the
        // other in `update`), but the view tolerates both flags being
        // set — last-pushed wins visually.
        let mut layered: Vec<Element<'_, Message>> = vec![body];
        if self.command_palette.is_open {
            layered.push(render_command_palette(self));
        }
        if self.node_finder.is_open {
            layered.push(render_node_finder(self));
        }

        if layered.len() == 1 {
            layered.into_iter().next().unwrap()
        } else {
            iced::widget::stack(layered)
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        }
    }
}

// ---------------------------------------------------------------------------
// View helpers
// ---------------------------------------------------------------------------

/// Render the FrameSplitTree slot. If the Frame has Panes, renders the
/// `pane_grid`; otherwise renders the canvas base layer fallback.
///
/// Per [`iced_composition_skeleton_spec.md` §2.3](
/// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_composition_skeleton_spec.md):
/// the base layer is the empty-Frame path, not a degenerate Pane.
fn render_frame_split_tree(app: &IcedApp) -> Element<'_, Message> {
    if app.frame.base_layer_active {
        render_canvas_base_layer(app)
    } else {
        pane_grid(&app.frame.split_state, |pane_handle, meta, _is_maximized| {
            let pane_label = match meta.pane_type {
                PaneType::Canvas => "Canvas",
                PaneType::Tile => "Tile pane",
            };

            // Title bar: pane label + close button.
            let title_row: Element<'_, Message> = iced::widget::row![
                text(pane_label).size(12).width(Length::Fill),
                button(text("×").size(10)).on_press(Message::ClosePane(pane_handle)),
            ]
            .align_y(iced::Alignment::Center)
            .spacing(4)
            .into();

            let body = render_pane_body(app, meta);
            pane_grid::Content::new(body).title_bar(pane_grid::TitleBar::new(title_row))
        })
        .on_click(Message::PaneFocused)
        .on_drag(Message::PaneGridDragged)
        .on_resize(10, Message::PaneGridResized)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }
}

/// Render the body of one Pane. Canvas panes show the graph canvas
/// program; Tile panes show a stub pending S4 (when `gs::TileTabs` and
/// the viewer surfaces are wired).
fn render_pane_body<'a>(app: &'a IcedApp, meta: &PaneMeta) -> Element<'a, Message> {
    match meta.pane_type {
        PaneType::Canvas => {
            let program =
                graph_canvas_from_app(&app.host.runtime.graph_app, app.host.view_id);
            let _: &GraphCanvasProgram = &program;
            let graph: Element<'_, super::iced_graph_canvas::GraphCanvasMessage> =
                canvas(program).width(Length::Fill).height(Length::Fill).into();
            graph.map(|gcm| match gcm {
                super::iced_graph_canvas::GraphCanvasMessage::CameraChanged { pan, zoom } => {
                    Message::CameraChanged { pan, zoom }
                }
            })
        }
        PaneType::Tile => {
            // Tile pane: TileTabs bar over the tile body.
            // Tile list and selection state come from the Navigator domain
            // layer (S5); this slice uses placeholder tabs so the widget is
            // exercised in the live layout.
            let tabs = TileTabs::new()
                .push(TileTab::new("Tab A"))
                .push(TileTab::new("Tab B"))
                .selected(Some(0))
                .on_select(|_i| Message::Tick)
                .on_close(|_i| Message::Tick);
            let tile_body = container(text("Tile body — Navigator wires content in S5").size(12))
                .center(Length::Fill);
            column![
                Element::from(tabs),
                tile_body.height(Length::Fill).width(Length::Fill),
            ]
            .spacing(0)
            .height(Length::Fill)
            .into()
        }
    }
}

/// Canvas base layer — rendered when the Frame has zero Panes.
///
/// This is the same `GraphCanvasProgram` used inside Canvas Panes;
/// per spec §2.3 the base layer is a distinct code branch, not a
/// degenerate Pane.
fn render_canvas_base_layer(app: &IcedApp) -> Element<'_, Message> {
    let program = graph_canvas_from_app(&app.host.runtime.graph_app, app.host.view_id);
    let _: &GraphCanvasProgram = &program;
    let graph: Element<'_, super::iced_graph_canvas::GraphCanvasMessage> =
        canvas(program).width(Length::Fill).height(Length::Fill).into();
    graph.map(|gcm| match gcm {
        super::iced_graph_canvas::GraphCanvasMessage::CameraChanged { pan, zoom } => {
            Message::CameraChanged { pan, zoom }
        }
    })
}

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
enum NavigatorAnchor {
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
fn render_navigator_host(app: &IcedApp, anchor: NavigatorAnchor) -> Element<'_, Message> {
    let _ = app; // referenced once full bucket data is wired

    // Tree Spine bucket — lazy+scrollable tree stub.
    // Real: `scrollable(lazy(generation, |_| column(rows)))` per spec §6.1.
    let tree_spine: Element<'_, Message> = scrollable(
        column![
            text("Tree Spine").size(11),
            text("  ▼ Graphlet: (none)").size(11),
            text("      ○ —").size(11),
        ]
        .spacing(2),
    )
    .height(Length::FillPortion(2))
    .into();

    // Swatches bucket — virtualized canvas grid stub.
    // Real: `scrollable(wrap_horizontally(swatch_cards))` per spec §6.2.
    let swatches: Element<'_, Message> = container(text("Swatches — (stub)").size(11))
        .height(Length::FillPortion(1))
        .width(Length::Fill)
        .into();

    // Activity Log bucket — lazy+scrollable event stream stub.
    // Real: `scrollable(lazy(generation, |_| column(event_rows)))` per spec §6.3.
    let activity_log: Element<'_, Message> = scrollable(
        column![
            text("Activity Log").size(11),
            text("  — (no events)").size(11),
        ]
        .spacing(2),
    )
    .height(Length::FillPortion(1))
    .into();

    match anchor {
        NavigatorAnchor::Left | NavigatorAnchor::Right => {
            // Sidebar form factor: vertical column, fixed width.
            container(
                column![tree_spine, swatches, activity_log]
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

/// Render the CommandBar slot omnibar. Per
/// [`iced_omnibar_spec.md` §3](
/// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_omnibar_spec.md).
///
/// Slice 2: structural layout with placeholder sub-widgets. Real
/// Navigator projections (scope badge content, graphlet chip, settings
/// button routing, sync status) land in S4 when those surfaces exist.
fn render_command_bar(app: &IcedApp) -> Element<'_, Message> {
    let scope_badge = text("–").size(12);

    let center: Element<'_, Message> = match app.omnibar.mode {
        OmnibarMode::Display => {
            let location = app
                .last_view_model
                .as_ref()
                .map(|vm| vm.toolbar.location.as_str())
                .unwrap_or("—");
            text(location).size(14).width(Length::Fill).into()
        }
        OmnibarMode::Input => text_input("Enter URL or search…", &app.omnibar.draft)
            .id(iced::widget::Id::new(OMNIBAR_INPUT_ID))
            .on_input(Message::OmnibarInput)
            .on_submit(Message::OmnibarSubmit)
            .size(14)
            .padding(4)
            .width(Length::Fill)
            .into(),
    };

    let settings_stub = text("⚙").size(14);
    let sync_stub = text("◉").size(12);

    iced::widget::row![scope_badge, center, settings_stub, sync_stub,]
        .spacing(8)
        .align_y(iced::Alignment::Center)
        .into()
}

/// Render the Command Palette modal. Per
/// [`iced_command_palette_spec.md` §2.2](
/// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_command_palette_spec.md).
///
/// Slice 6 renders the canonical layout (text input, divider,
/// scrollable result list, footer). The result list is empty until the
/// `ActionRegistry` is wired in a downstream slice; the footer surfaces
/// the empty-state hint so the modal is recognisable.
fn render_command_palette(app: &IcedApp) -> Element<'_, Message> {
    let input = text_input("Type a command or search…", &app.command_palette.query)
        .id(iced::widget::Id::new(PALETTE_INPUT_ID))
        .on_input(Message::PaletteQuery)
        .size(14)
        .padding(6)
        .width(Length::Fill);

    let divider = rule::horizontal(1.0);

    // Slice 6: empty results list. Real ranking lands once the
    // ActionRegistry is wired into the runtime view-model.
    let results: Element<'_, Message> = scrollable(
        column![text("No actions registered yet (Slice 6 stub).")
            .size(12)
            .width(Length::Fill)]
        .spacing(2)
        .padding(8),
    )
    .height(Length::Fill)
    .into();

    let footer = text("Esc to dismiss · Enter to run").size(11);

    let body = column![
        text("Command Palette").size(13),
        input,
        divider,
        results,
        footer,
    ]
    .spacing(8)
    .padding(12)
    .width(Length::Fill)
    .height(Length::Fill);

    Modal::new(body)
        .on_blur(Message::PaletteCloseAndRestoreFocus)
        .max_width(640.0)
        .max_height(480.0)
        .into()
}

/// Render the Node Finder modal. Per
/// [`iced_node_finder_spec.md` §3](
/// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_node_finder_spec.md).
///
/// Slice 6 renders the canonical layout. The result list is empty
/// until the runtime exposes a `NodeFinderViewModel`.
fn render_node_finder(app: &IcedApp) -> Element<'_, Message> {
    let input = text_input(
        "Search nodes by title, tag, URL, or content…",
        &app.node_finder.query,
    )
    .id(iced::widget::Id::new(NODE_FINDER_INPUT_ID))
    .on_input(Message::NodeFinderQuery)
    .size(14)
    .padding(6)
    .width(Length::Fill);

    let divider = rule::horizontal(1.0);

    // Slice 6: empty result list. Real fuzzy-match ranking lands once
    // the runtime exposes NodeFinderViewModel.
    let empty_label = if app.node_finder.query.is_empty() {
        "No recently-active nodes yet (Slice 6 stub)."
    } else {
        "No matching nodes (Slice 6 stub)."
    };
    let results: Element<'_, Message> = scrollable(
        column![text(empty_label).size(12).width(Length::Fill)]
            .spacing(2)
            .padding(8),
    )
    .height(Length::Fill)
    .into();

    let footer = text("Esc to dismiss · Enter to open").size(11);

    let body = column![
        text("Node Finder").size(13),
        input,
        divider,
        results,
        footer,
    ]
    .spacing(8)
    .padding(12)
    .width(Length::Fill)
    .height(Length::Fill);

    Modal::new(body)
        .on_blur(Message::NodeFinderCloseAndRestoreFocus)
        .max_width(640.0)
        .max_height(480.0)
        .into()
}

/// Is this iced event the "focus the omnibar" hotkey?
/// Ctrl+L (Cmd+L on macOS via `Modifiers::command()`). Consumed at
/// the app level — never reaches the runtime's `HostEvent` translation.
fn is_omnibar_focus_hotkey(event: &iced::Event) -> bool {
    match event {
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: iced::keyboard::Key::Character(c),
            modifiers,
            ..
        }) => c.as_ref().eq_ignore_ascii_case("l") && modifiers.command(),
        _ => false,
    }
}

/// Is this iced event the "open Command Palette" hotkey?
/// Ctrl+Shift+P (Zed/VSCode-shaped). Per
/// [`iced_command_palette_spec.md` §2.1](
/// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_command_palette_spec.md).
fn is_command_palette_hotkey(event: &iced::Event) -> bool {
    match event {
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: iced::keyboard::Key::Character(c),
            modifiers,
            ..
        }) => {
            c.as_ref().eq_ignore_ascii_case("p")
                && modifiers.command()
                && modifiers.shift()
        }
        _ => false,
    }
}

/// Is this iced event the "open Node Finder" hotkey?
/// Ctrl+P **without** Shift (Zed/VSCode-shaped). Per
/// [`iced_node_finder_spec.md` §2](
/// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_node_finder_spec.md).
fn is_node_finder_hotkey(event: &iced::Event) -> bool {
    match event {
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: iced::keyboard::Key::Character(c),
            modifiers,
            ..
        }) => {
            c.as_ref().eq_ignore_ascii_case("p")
                && modifiers.command()
                && !modifiers.shift()
        }
        _ => false,
    }
}

/// Is this iced event an Escape keypress?
fn is_escape_key(event: &iced::Event) -> bool {
    matches!(
        event,
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
            ..
        })
    )
}

/// Does `s` look like a URL or bare hostname?
///
/// Per [`iced_omnibar_spec.md` §6.1](
/// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_omnibar_spec.md):
/// explicit scheme (`://`) → URL; no spaces + contains `.` → bare
/// host. Everything else → non-URL-shaped → route to Node Finder.
fn is_url_shaped(s: &str) -> bool {
    let s = s.trim();
    if s.is_empty() {
        return false;
    }
    if s.contains("://") {
        return true;
    }
    !s.contains(' ') && s.contains('.')
}

/// Render the host's toast queue as a stack of severity-prefixed rows.
fn render_toast_stack(
    toasts: &[graphshell_runtime::ToastSpec],
) -> iced::widget::Column<'_, Message> {
    if toasts.is_empty() {
        return iced::widget::column![];
    }
    let mut col = iced::widget::column![].spacing(4);
    for toast in toasts {
        let severity_tag = match toast.severity {
            ToastSeverity::Info => "ℹ",
            ToastSeverity::Success => "✓",
            ToastSeverity::Warning => "⚠",
            ToastSeverity::Error => "✗",
        };
        col = col.push(text(format!("{severity_tag} {}", toast.message)).size(12));
    }
    col
}

/// Wire up an `iced::Application` around `IcedApp`.
///
/// Invoked from `cli::main` when `--iced` / `GRAPHSHELL_ICED=1` is set.
pub(crate) fn run_application(runtime: GraphshellRuntime) -> iced::Result {
    let runtime_slot = std::cell::RefCell::new(Some(runtime));
    iced::application(
        move || {
            let runtime = runtime_slot
                .borrow_mut()
                .take()
                .expect("iced application boot closure called more than once");
            (IcedApp::with_runtime(runtime), Task::none())
        },
        IcedApp::update,
        IcedApp::view,
    )
    .title(IcedApp::title)
    .subscription(IcedApp::subscription)
    .run()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iced_app_tick_drives_runtime() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        assert!(app.last_view_model.is_none(), "view-model cache starts empty");

        let _task = app.update(Message::Tick);

        assert!(app.last_view_model.is_some(), "Tick populates view-model");
        let _element = app.view();
    }

    #[test]
    fn iced_event_drives_runtime_tick_via_update() {
        use iced::mouse;
        use iced::Point;

        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let event = iced::Event::Mouse(mouse::Event::CursorMoved {
            position: Point { x: 42.0, y: 24.0 },
        });
        let _task = app.update(Message::IcedEvent(event));

        assert!(
            app.last_view_model.is_some(),
            "translated iced event should have driven a runtime tick",
        );
    }

    #[test]
    fn untranslatable_iced_event_does_not_tick() {
        use iced::mouse;

        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let event = iced::Event::Mouse(mouse::Event::CursorEntered);
        let _task = app.update(Message::IcedEvent(event));

        assert!(
            app.last_view_model.is_none(),
            "untranslatable event must be dropped without ticking",
        );
    }

    #[test]
    fn camera_changed_persists_to_runtime_canvas_cameras() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        let view_id = app.host.view_id;

        let pan = Vector2D::new(42.0, -17.0);
        let zoom = 1.75;
        let _task = app.update(Message::CameraChanged { pan, zoom });

        let camera = app
            .host
            .runtime
            .graph_app
            .workspace
            .graph_runtime
            .canvas_cameras
            .get(&view_id)
            .expect("camera should be persisted under host view_id");
        assert_eq!(camera.pan, pan);
        assert_eq!(camera.zoom, zoom);
        assert_eq!(camera.pan_velocity, Vector2D::zero());
    }

    // --- Omnibar tests (Slice 2) ---

    #[test]
    fn omnibar_input_updates_draft_without_ticking() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        assert!(app.omnibar.draft.is_empty(), "draft starts empty");
        assert!(app.last_view_model.is_none(), "no tick has run");

        let _task = app.update(Message::OmnibarInput("https://exa".into()));
        assert_eq!(app.omnibar.draft, "https://exa");
        assert!(
            app.last_view_model.is_none(),
            "typing must not tick the runtime",
        );

        let _task = app.update(Message::OmnibarInput("https://example.com".into()));
        assert_eq!(app.omnibar.draft, "https://example.com");
    }

    #[test]
    fn omnibar_submit_url_creates_node_and_returns_to_display() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let nodes_before = app.host.runtime.graph_app.domain_graph().nodes().count();

        let _ = app.update(Message::OmnibarFocus);
        let _ = app.update(Message::OmnibarInput("https://submit.test/".into()));
        let _ = app.update(Message::OmnibarSubmit);

        assert!(app.omnibar.draft.is_empty(), "submit clears draft");
        assert_eq!(app.omnibar.mode, OmnibarMode::Display);
        assert_eq!(app.host.toast_queue.len(), 1, "submit enqueues ack toast");
        assert!(app.host.toast_queue[0].message.contains("https://submit.test/"));

        let nodes_after = app.host.runtime.graph_app.domain_graph().nodes().count();
        assert_eq!(nodes_after, nodes_before + 1, "exactly one node added");
        assert!(app.host.pending_host_intents.is_empty(), "intent queue drained");
    }

    #[test]
    fn omnibar_submit_non_url_routes_to_node_finder() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let nodes_before = app.host.runtime.graph_app.domain_graph().nodes().count();

        let _ = app.update(Message::OmnibarInput("graphql tutorial".into()));
        let _ = app.update(Message::OmnibarSubmit);

        assert!(app.omnibar.draft.is_empty());
        assert_eq!(app.omnibar.mode, OmnibarMode::Display);
        assert_eq!(
            app.host.runtime.graph_app.domain_graph().nodes().count(),
            nodes_before,
            "non-URL submit must not create a graph node",
        );
        assert!(
            app.host.toast_queue.is_empty(),
            "OmnibarSubmit alone does not toast — routing happens via Task::done",
        );

        // Simulate iced driving the Task::done message — Slice 6 wiring
        // opens the Node Finder pre-seeded with the query (no toast).
        let _ = app.update(Message::OmnibarRouteToNodeFinder("graphql tutorial".into()));
        assert!(app.node_finder.is_open, "non-URL submit opens the Node Finder");
        assert_eq!(app.node_finder.query, "graphql tutorial");
        assert!(
            app.host.toast_queue.is_empty(),
            "Slice 6 routing does not toast — the modal itself is the affordance",
        );
    }

    #[test]
    fn omnibar_submit_empty_is_noop() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let _ = app.update(Message::OmnibarSubmit);
        assert!(app.host.toast_queue.is_empty());
        assert!(app.omnibar.draft.is_empty());
    }

    #[test]
    fn ctrl_l_transitions_omnibar_to_input_mode() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        assert_eq!(app.omnibar.mode, OmnibarMode::Display);

        let ctrl_l = iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: iced::keyboard::Key::Character("l".into()),
            modified_key: iced::keyboard::Key::Character("l".into()),
            physical_key: iced::keyboard::key::Physical::Unidentified(
                iced::keyboard::key::NativeCode::Unidentified,
            ),
            location: iced::keyboard::Location::Standard,
            modifiers: iced::keyboard::Modifiers::CTRL,
            text: None,
            repeat: false,
        });
        let _task = app.update(Message::IcedEvent(ctrl_l));
        assert!(
            app.last_view_model.is_none(),
            "Ctrl+L must not tick the runtime",
        );
        let _task = app.update(Message::OmnibarFocus);
        assert_eq!(app.omnibar.mode, OmnibarMode::Input);
    }

    #[test]
    fn escape_dismisses_omnibar() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let _ = app.update(Message::OmnibarFocus);
        let _ = app.update(Message::OmnibarInput("partial".into()));
        let _ = app.update(Message::OmnibarKeyEscape);

        assert_eq!(app.omnibar.mode, OmnibarMode::Display);
        assert!(app.omnibar.draft.is_empty(), "escape clears draft");
    }

    #[test]
    fn omnibar_blur_returns_to_display_preserving_draft() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let _ = app.update(Message::OmnibarFocus);
        let _ = app.update(Message::OmnibarInput("partial".into()));
        let _ = app.update(Message::OmnibarBlur);

        assert_eq!(app.omnibar.mode, OmnibarMode::Display);
        assert_eq!(app.omnibar.draft, "partial", "blur preserves draft");
    }

    #[test]
    fn bare_l_keypress_is_not_a_hotkey() {
        let event = iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: iced::keyboard::Key::Character("l".into()),
            modified_key: iced::keyboard::Key::Character("l".into()),
            physical_key: iced::keyboard::key::Physical::Unidentified(
                iced::keyboard::key::NativeCode::Unidentified,
            ),
            location: iced::keyboard::Location::Standard,
            modifiers: iced::keyboard::Modifiers::empty(),
            text: None,
            repeat: false,
        });
        assert!(!super::is_omnibar_focus_hotkey(&event));
    }

    #[test]
    fn url_shape_detection() {
        assert!(is_url_shaped("https://example.com"));
        assert!(is_url_shaped("verso://settings"));
        assert!(is_url_shaped("http://localhost:8080/path"));
        assert!(is_url_shaped("example.com"));
        assert!(is_url_shaped("sub.example.co.uk"));
        assert!(!is_url_shaped("graphql tutorial"));
        assert!(!is_url_shaped("find nodes"));
        assert!(!is_url_shaped(""));
        assert!(!is_url_shaped("   "));
    }

    #[test]
    fn cursor_cache_syncs_from_iced_events() {
        use iced::mouse;
        use iced::Point;

        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        assert!(app.host.cursor_position.is_none(), "starts uncached");
        let _task = app.update(Message::IcedEvent(iced::Event::Mouse(
            mouse::Event::CursorMoved {
                position: Point { x: 12.5, y: 34.5 },
            },
        )));
        assert_eq!(app.host.cursor_position, Some(iced::Point::new(12.5, 34.5)));

        let _task = app.update(Message::IcedEvent(iced::Event::Mouse(
            mouse::Event::CursorLeft,
        )));
        assert!(app.host.cursor_position.is_none(), "CursorLeft clears cache");
    }

    // --- Frame split-tree tests (Slice 3) ---

    /// `IcedApp` starts with exactly one Canvas pane pre-seeded in the
    /// Frame (the default launch state for Slice 3 verification).
    #[test]
    fn frame_starts_with_one_canvas_pane() {
        let runtime = GraphshellRuntime::for_testing();
        let app = IcedApp::with_runtime(runtime);

        assert!(!app.frame.base_layer_active, "pane_grid is active at launch");
        assert_eq!(app.frame.split_state.len(), 1, "exactly one pane at launch");

        let (_, meta) = app
            .frame
            .split_state
            .iter()
            .next()
            .expect("should have one pane");
        assert_eq!(meta.pane_type, PaneType::Canvas, "initial pane is Canvas");
    }

    /// `PaneFocused` records the iced pane handle as the focused pane.
    #[test]
    fn pane_focused_records_handle() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        assert!(app.frame.focused_pane.is_none(), "no focused pane at start");

        let (pane_handle, _) = app
            .frame
            .split_state
            .iter()
            .next()
            .expect("should have one pane");
        let handle = *pane_handle;

        let _ = app.update(Message::PaneFocused(handle));
        assert_eq!(app.frame.focused_pane, Some(handle));
    }

    /// `ClosePane` on the only Pane activates the canvas base layer.
    ///
    /// Note: `pane_grid::State::close` is a no-op on the last pane (iced
    /// cannot reduce the state to zero panes). `FrameState::base_layer_active`
    /// is the flag that switches the render path to the canvas base layer.
    #[test]
    fn close_last_pane_activates_base_layer() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        assert_eq!(app.frame.split_state.len(), 1);
        assert!(!app.frame.base_layer_active);

        let (pane_handle, _) = app
            .frame
            .split_state
            .iter()
            .next()
            .expect("should have a pane");
        let handle = *pane_handle;

        let _ = app.update(Message::PaneFocused(handle));
        let _ = app.update(Message::ClosePane(handle));

        assert!(
            app.frame.base_layer_active,
            "base_layer_active should be set after closing the last pane",
        );
        assert_eq!(
            app.frame.focused_pane, None,
            "focused pane cleared when it is closed",
        );
    }

    /// Closing a Pane that is not the focused Pane leaves `focused_pane`
    /// intact.
    #[test]
    fn close_non_focused_pane_preserves_focus() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        // Split so there are two panes.
        let (first_handle, _) = app
            .frame
            .split_state
            .iter()
            .next()
            .expect("should have a pane");
        let first = *first_handle;

        let second_meta = PaneMeta {
            pane_id: PaneId::next(),
            pane_type: PaneType::Tile,
        };
        let (second, _split) = app
            .frame
            .split_state
            .split(pane_grid::Axis::Vertical, first, second_meta)
            .expect("split should succeed");

        // Focus the first pane; close the second.
        let _ = app.update(Message::PaneFocused(first));
        let _ = app.update(Message::ClosePane(second));

        assert_eq!(app.frame.split_state.len(), 1, "one pane remains");
        assert_eq!(
            app.frame.focused_pane,
            Some(first),
            "focused_pane unchanged when a non-focused pane is closed",
        );
    }

    /// `view()` produces an element without panicking for the default
    /// (one-pane) frame state.
    #[test]
    fn view_renders_pane_grid_without_panic() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        let _ = app.update(Message::Tick);
        let _element = app.view();
    }

    /// After closing the last pane, `view()` falls back to the canvas
    /// base layer (`base_layer_active`) without panicking.
    #[test]
    fn view_renders_base_layer_when_last_pane_closed() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let (handle, _) = app
            .frame
            .split_state
            .iter()
            .next()
            .expect("initial pane");
        let handle = *handle;
        let _ = app.update(Message::ClosePane(handle));
        assert!(app.frame.base_layer_active);

        let _ = app.update(Message::Tick);
        let _element = app.view();
    }

    // --- Command Palette + Node Finder tests (Slice 6) ---

    fn key_press(c: &str, modifiers: iced::keyboard::Modifiers) -> iced::Event {
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: iced::keyboard::Key::Character(c.into()),
            modified_key: iced::keyboard::Key::Character(c.into()),
            physical_key: iced::keyboard::key::Physical::Unidentified(
                iced::keyboard::key::NativeCode::Unidentified,
            ),
            location: iced::keyboard::Location::Standard,
            modifiers,
            text: None,
            repeat: false,
        })
    }

    #[test]
    fn ctrl_shift_p_opens_command_palette() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        assert!(!app.command_palette.is_open);

        let event = key_press(
            "p",
            iced::keyboard::Modifiers::CTRL | iced::keyboard::Modifiers::SHIFT,
        );
        // The IcedEvent path returns Task::done(PaletteOpen{...}); simulate
        // the runtime delivering that message back to update().
        let _ = app.update(Message::IcedEvent(event));
        let _ = app.update(Message::PaletteOpen {
            origin: PaletteOrigin::KeyboardShortcut,
        });

        assert!(app.command_palette.is_open);
        assert_eq!(app.command_palette.origin, PaletteOrigin::KeyboardShortcut);
    }

    #[test]
    fn ctrl_p_opens_node_finder_not_palette() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let _ = app.update(Message::NodeFinderOpen {
            origin: NodeFinderOrigin::KeyboardShortcut,
        });

        assert!(app.node_finder.is_open);
        assert!(!app.command_palette.is_open);
    }

    #[test]
    fn palette_and_finder_hotkeys_are_distinct() {
        let ctrl_p = key_press("p", iced::keyboard::Modifiers::CTRL);
        let ctrl_shift_p = key_press(
            "p",
            iced::keyboard::Modifiers::CTRL | iced::keyboard::Modifiers::SHIFT,
        );

        assert!(super::is_node_finder_hotkey(&ctrl_p));
        assert!(!super::is_command_palette_hotkey(&ctrl_p));
        assert!(super::is_command_palette_hotkey(&ctrl_shift_p));
        assert!(!super::is_node_finder_hotkey(&ctrl_shift_p));
    }

    #[test]
    fn palette_and_finder_are_mutually_exclusive() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let _ = app.update(Message::PaletteOpen {
            origin: PaletteOrigin::KeyboardShortcut,
        });
        assert!(app.command_palette.is_open);

        // Opening node finder closes the palette.
        let _ = app.update(Message::NodeFinderOpen {
            origin: NodeFinderOrigin::KeyboardShortcut,
        });
        assert!(!app.command_palette.is_open);
        assert!(app.node_finder.is_open);

        // Opening palette closes the finder.
        let _ = app.update(Message::PaletteOpen {
            origin: PaletteOrigin::KeyboardShortcut,
        });
        assert!(app.command_palette.is_open);
        assert!(!app.node_finder.is_open);
    }

    #[test]
    fn palette_query_updates_state_without_ticking() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let _ = app.update(Message::PaletteOpen {
            origin: PaletteOrigin::KeyboardShortcut,
        });
        let _ = app.update(Message::PaletteQuery("toggl".into()));

        assert_eq!(app.command_palette.query, "toggl");
        assert!(
            app.last_view_model.is_none(),
            "palette typing must not tick the runtime",
        );
    }

    #[test]
    fn palette_close_clears_state() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let _ = app.update(Message::PaletteOpen {
            origin: PaletteOrigin::KeyboardShortcut,
        });
        let _ = app.update(Message::PaletteQuery("partial".into()));
        let _ = app.update(Message::PaletteCloseAndRestoreFocus);

        assert!(!app.command_palette.is_open);
        assert!(app.command_palette.query.is_empty());
        assert!(app.command_palette.focused_index.is_none());
    }

    #[test]
    fn omnibar_route_to_node_finder_actually_opens_finder() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let _ = app.update(Message::OmnibarRouteToNodeFinder("graph theory".into()));

        assert!(app.node_finder.is_open, "non-URL omnibar submit opens node finder");
        assert_eq!(app.node_finder.query, "graph theory", "query is pre-seeded");
        assert_eq!(
            app.node_finder.origin,
            NodeFinderOrigin::OmnibarRoute("graph theory".into()),
            "origin records the routed query",
        );
        assert_eq!(app.omnibar.mode, OmnibarMode::Display, "omnibar returned to Display");
    }

    #[test]
    fn escape_closes_palette_when_open() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let _ = app.update(Message::PaletteOpen {
            origin: PaletteOrigin::KeyboardShortcut,
        });
        assert!(app.command_palette.is_open);

        let _ = app.update(Message::PaletteCloseAndRestoreFocus);
        assert!(!app.command_palette.is_open);
    }

    #[test]
    fn palette_action_selected_closes_and_acks() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let _ = app.update(Message::PaletteOpen {
            origin: PaletteOrigin::KeyboardShortcut,
        });
        assert_eq!(app.host.toast_queue.len(), 0);

        let _ = app.update(Message::PaletteActionSelected(0));

        assert!(!app.command_palette.is_open);
        assert_eq!(app.host.toast_queue.len(), 1);
        assert!(app.host.toast_queue[0].message.contains("palette action"));
    }

    #[test]
    fn view_renders_with_palette_open() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let _ = app.update(Message::PaletteOpen {
            origin: PaletteOrigin::KeyboardShortcut,
        });
        let _ = app.update(Message::Tick);

        // Render-time smoke test: must not panic with a modal stacked
        // on top of the body.
        let _element = app.view();
    }

    #[test]
    fn view_renders_with_node_finder_open() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let _ = app.update(Message::NodeFinderOpen {
            origin: NodeFinderOrigin::KeyboardShortcut,
        });
        let _ = app.update(Message::Tick);

        let _element = app.view();
    }
}
