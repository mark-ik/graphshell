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
//! **Scope (Slice 26 / observability complete)**: Slices 23-26
//! shipped the full host-neutral observability foundation:
//!
//! - **Slice 23**: `graphshell_core::ux_observability` —
//!   `UxEvent` taxonomy, `UxObserver` trait, `UxObservers` registry,
//!   built-in `CountingObserver` and `RecordingObserver`. Iced
//!   emits at every chrome-surface transition and intent dispatch.
//! - **Slice 24**: `graphshell_core::accessibility` —
//!   `AccessibilityDescriptor` keyed by `SurfaceId`, using
//!   `accesskit::Role` directly. The lookup is locked-in shape;
//!   iced's vendored fork has no AccessKit hook today, but a
//!   future host or a future iced version consumes the same lookup.
//! - **Slice 25**: `graphshell_core::ux_probes` —
//!   `MutualExclusionProbe` and `OpenDismissBalanceProbe` assert
//!   §4.10 invariants against the event stream. The iced host's
//!   "dismiss-before-open" supersession sequencing satisfies both
//!   under a real message-driven trace.
//! - **Slice 26**: `graphshell_core::ux_diagnostics` —
//!   `UxChannelObserver` adapter forwards each `UxEvent` to a
//!   pluggable `DiagnosticsChannelSink`, routing through canonical
//!   `"ux.<surface>.<event>"` channel ids. Iced runtime registers a
//!   `NoopChannelSink` by default; a future host swap to a real
//!   registry sink is one line.
//!
//! All four modules are portable: `graphshell-core` only. Future
//! hosts (egui, Stage-G/H) plug in identically.
//!
//! **Slice 27**: the Navigator's Activity Log bucket — the third
//! and last Presentation Bucket — is now wired to live data via a
//! bounded `RecordingObserver` (capacity 100). Every `UxEvent` the
//! host emits flows in; the bucket renders most-recent-first as
//! one line per event. Three Navigator buckets now ship with real
//! data: Tree Spine (Slice 20, GraphTree members), Activity Log
//! (this slice, UxEvent stream); Swatches remains a stub pending
//! the canvas-instance multi-render-profile work.
//!
//! **Slice 28-32**: per-action handler dispatch table grew from 7 to
//! 28 `ActionId`s across structural-context unlocks:
//!
//! - **Slice 28**: 7 added via `apply_reducer_intents` /
//!   `enqueue_app_command` / direct methods. Plus `host_routed_action`
//!   intercept layer.
//! - **Slice 29**: Tile graphlet projection — finishes deferred
//!   per-tile NodeKey threading; tile pane shows real tabs from
//!   `runtime.graph_tree.members()`.
//! - **Slice 30**: 6 settings/hub/history actions wired via
//!   `verso://...` URL routing through `add_node_and_sync`.
//! - **Slice 31**: Frame switcher — multi-Frame composition with
//!   inactive_frames Vec; FrameOpen/FrameDelete/FrameSelect host-
//!   routed.
//! - **Slice 32**: NodeCreate URL-input modal — NodeNew /
//!   NodeNewAsTab host-routed; new SurfaceId::NodeCreate variant
//!   propagated through accessibility / ux_diagnostics / ux_probes.
//!
//! 28 ActionIds wired (was 7). Remaining unhandled categories
//! enumerated in `dispatch_action`'s wildcard arm.
//!
//! **Slice 33**: Swatches bucket — the third Navigator Presentation
//! Bucket — now renders a real grid of compact canvas instances per
//! built-in recipe (FullGraph, RecentlyActive, FocusedNeighborhood).
//! All three Navigator buckets (Tree Spine / Swatches / Activity
//! Log) now ship with real data; the placeholder-stub state is
//! gone. Per-recipe scene scoping (filtered nodes, lens overrides)
//! is the next iteration; the rendering pipeline doesn't change
//! shape when that lands.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use euclid::default::Vector2D;
use graph_canvas::camera::CanvasCamera;
use iced::time;
use iced::widget::{button, canvas, column, container, mouse_area, pane_grid, row, rule, scrollable, text, text_input};
use iced::{Element, Length, Point, Subscription, Task};
use graphshell_iced_widgets::{ContextMenu, ContextMenuEntry, Modal, TileTab, TileTabs, animation, tokens};

mod view;
use view::*;
mod state;
use state::*;

/// Frame interval for the runtime tick `Subscription`. ~60 Hz. Per
/// [`iced_composition_skeleton_spec.md` §1.5](
/// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_composition_skeleton_spec.md):
/// the runtime tick must run from a `time::every` Subscription, not
/// poll inside `view`, so time-based runtime state (focus-ring fades,
/// recipe-result drains, lifecycle transitions) advances even without
/// user input. Stage A done condition.
const RUNTIME_TICK_INTERVAL: Duration = Duration::from_millis(16);


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
    /// Right-click context-menu state. Per
    /// [`iced_command_palette_spec.md` §7.3](
    /// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_command_palette_spec.md).
    pub(crate) context_menu: ContextMenuState,
    /// Confirmation modal that gates destructive intents. Per
    /// [`iced_command_palette_spec.md` §5](
    /// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_command_palette_spec.md).
    pub(crate) confirm_dialog: ConfirmDialogState,
    /// URL-input modal opened by `NodeNew` / `NodeNewAsTab` actions
    /// from non-omnibar surfaces (palette, context menu). Slice 32.
    pub(crate) node_create: NodeCreateState,
    /// Label-input modal opened by the `FrameRename` action. Slice 34.
    pub(crate) frame_rename: FrameRenameState,
    /// Bounded recorder backing the Navigator's Activity Log
    /// bucket. Registered as a `UxObserver` on the runtime; every
    /// `UxEvent` the host emits flows in. Slice 27.
    pub(crate) activity_log_recorder:
        std::sync::Arc<graphshell_core::ux_observability::RecordingObserver>,
    /// Active Frame's identity. The active Frame's split-tree state
    /// lives in `frame` (the Slice 3 field). Slice 31.
    pub(crate) frame_id: FrameId,
    /// Active Frame's display label (rendered in the Frame switcher).
    pub(crate) frame_label: String,
    /// Backgrounded Frames. Switching frames swaps the active
    /// `FrameState` with one of these via `std::mem::swap`.
    pub(crate) inactive_frames: Vec<NamedFrame>,
    /// Stable reference instant for animation phase clocks. Slice 38.
    /// All pulse-shaped animations read from this so their phases
    /// stay coherent (no per-pulse drift).
    pub(crate) startup_instant: std::time::Instant,
    /// Set whenever any modal-like surface (palette / finder /
    /// confirm / node_create / frame_rename) opens; cleared when
    /// it closes. Slice 42: modal renderers compute `opacity =
    /// ease_out_cubic(progress)` from this and pass to
    /// `gs::Modal::opacity` for a fade-in. ContextMenu doesn't use
    /// gs::Modal, so it doesn't participate today.
    pub(crate) modal_opened_at: Option<std::time::Instant>,
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
    /// `update` writes the new values into the runtime's per-view
    /// camera map (legacy path) AND into the active Frame's
    /// per-pane cache (Slice 35) keyed by `pane_id`. The base layer
    /// passes `pane_id = None`.
    CameraChanged {
        pane_id: Option<PaneId>,
        pan: Vector2D<f32>,
        zoom: f32,
    },
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

    // --- Tree Spine messages — Slice 20 ---

    /// User clicked a node row in the Tree Spine bucket. Dispatches
    /// `HostIntent::OpenNode { node_key }` so the runtime promotes
    /// it to focused state — same wiring as Node Finder selection.
    TreeSpineNodeClicked(graphshell_core::graph::NodeKey),

    // --- Swatches bucket messages — Slice 33 ---

    /// User clicked a swatch card. Slice 33 stub: surface via toast;
    /// downstream slice promotes the swatch to a canvas Pane via the
    /// uphill rule (per iced_composition_skeleton_spec.md §6.2).
    SwatchClicked(SwatchRecipe),

    // --- NodeCreate modal messages — Slice 32 ---

    /// Open the NodeCreate URL-input modal. Triggered by
    /// `NodeNew` / `NodeNewAsTab` actions from palette / context menu.
    NodeCreateOpen,
    /// Text edited in the URL field.
    NodeCreateInput(String),
    /// User pressed Enter / clicked Create. If the URL is non-empty,
    /// dispatches `HostIntent::CreateNodeAtUrl` and closes the modal.
    NodeCreateSubmit,
    /// Cancel button, click-outside, or Escape — drops the draft.
    NodeCreateCancel,

    // --- FrameRename modal messages — Slice 34 ---

    /// Open the Frame rename modal pre-seeded with the active
    /// Frame's current label. Triggered by `FrameRename` action.
    FrameRenameOpen,
    /// Text edited in the label field.
    FrameRenameInput(String),
    /// User submitted the new label — applies it to the active
    /// Frame and closes the modal. Empty submissions are no-ops.
    FrameRenameSubmit,
    /// Cancel button, click-outside, or Escape — drops the draft.
    FrameRenameCancel,

    // --- Frame composition messages — Slice 31 ---

    /// Create a new Frame with a fresh `FrameState` and switch to it
    /// (the previous active Frame moves into `inactive_frames`).
    NewFrame,
    /// Switch the active Frame. `index` addresses
    /// `inactive_frames[index]`; the current active Frame moves into
    /// that slot via mem::swap.
    SwitchFrame(usize),
    /// Close the current active Frame and switch to
    /// `inactive_frames[0]`. No-op when there's only one Frame.
    CloseCurrentFrame,

    // --- Settings pane messages — Slice 39 ---
    // Settings live as tile content rendered inside any tile pane
    // whose active tile's URL starts with "verso://settings". The
    // pane delegates to render_settings_pane(app); these messages
    // are the toggle-style controls it surfaces.

    /// Toggle whether the left Navigator host is visible.
    SettingsToggleNavigatorLeft,
    /// Toggle whether the right Navigator host is visible.
    SettingsToggleNavigatorRight,
    /// Toggle whether the top Navigator host is visible.
    SettingsToggleNavigatorTop,
    /// Toggle whether the bottom Navigator host is visible.
    SettingsToggleNavigatorBottom,

    // --- Tile Tabs messages — Slice 29 ---

    /// User clicked a tile tab. Dispatches `HostIntent::OpenNode`
    /// so the runtime promotes the node to focused state. Per-
    /// graphlet presentation-state (Active/Inactive) wiring lands
    /// once the graphlet authority surfaces a transition method.
    TileTabSelected {
        pane_id: PaneId,
        node_key: graphshell_core::graph::NodeKey,
    },
    /// User clicked a tile tab's close `×`. Slice 29 stub: surfaces
    /// the deactivate intent via toast; the real
    /// `LifecycleIntent::ToggleTilePresentationState { node_key,
    /// graphlet_id }` path lands when the graphlet authority is
    /// wired (per iced jump-ship plan §4.4).
    TileTabClosed {
        pane_id: PaneId,
        node_key: graphshell_core::graph::NodeKey,
    },

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
    /// ArrowDown pressed while the palette is open — advance the
    /// focused row (wraps).
    PaletteFocusDown,
    /// ArrowUp pressed while the palette is open — retreat the
    /// focused row (wraps).
    PaletteFocusUp,
    /// Enter pressed inside the palette text input — fires the
    /// currently-focused row, or row 0 if no row is focused.
    PaletteSubmitFocused,

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
    /// ArrowDown pressed while the finder is open — advance the
    /// focused row (wraps).
    NodeFinderFocusDown,
    /// ArrowUp pressed while the finder is open — retreat the focused
    /// row (wraps).
    NodeFinderFocusUp,
    /// Enter pressed inside the finder text input — fires the
    /// currently-focused row, or row 0 if no row is focused.
    NodeFinderSubmitFocused,

    // --- Context Menu messages — Slice 8 ---

    /// Right-click occurred on a context-menu target. The anchor is
    /// read from `IcedHost.cursor_position` at handle time (set by the
    /// CursorMoved cache). Mutually exclusive with palette/finder —
    /// opening dismisses any active modal.
    ContextMenuOpen { target: ContextMenuTarget },
    /// User clicked an entry at the given index. Slice 8: stub-acks
    /// via toast; downstream slice routes via the uphill rule
    /// (e.g. `LifecycleIntent::Tombstone`).
    ContextMenuEntrySelected(usize),
    /// Click outside the menu, or Escape, dismisses without acting.
    ContextMenuDismiss,

    // --- Confirm Dialog messages — Slice 14 ---

    /// User confirmed the pending destructive intent. The handler
    /// pushes the saved intent onto `pending_host_intents`, drives
    /// a tick, and closes the dialog.
    ConfirmDialogConfirm,
    /// User cancelled (Cancel button, click-outside via Modal::on_blur,
    /// or Escape). The pending intent is dropped without dispatch.
    ConfirmDialogCancel,
}

impl IcedApp {
    /// Construct an app whose `IcedHost` wraps the supplied runtime.
    pub(crate) fn with_runtime(runtime: GraphshellRuntime) -> Self {
        let activity_log_recorder = std::sync::Arc::new(
            graphshell_core::ux_observability::RecordingObserver::with_capacity(
                ACTIVITY_LOG_CAPACITY,
            ),
        );
        let mut app = Self {
            host: IcedHost::with_runtime(runtime),
            last_view_model: None,
            omnibar: OmnibarSession::default(),
            frame: FrameState::new(),
            navigator: NavigatorState::default(),
            command_palette: CommandPaletteState::default(),
            node_finder: NodeFinderState::default(),
            context_menu: ContextMenuState::default(),
            confirm_dialog: ConfirmDialogState::default(),
            node_create: NodeCreateState::default(),
            frame_rename: FrameRenameState::default(),
            activity_log_recorder: std::sync::Arc::clone(&activity_log_recorder),
            frame_id: FrameId::next(),
            frame_label: "Frame 1".to_string(),
            inactive_frames: Vec::new(),
            startup_instant: std::time::Instant::now(),
            modal_opened_at: None,
        };
        // Slice 26: register a UxChannelObserver with a NoopSink so
        // every UxEvent passes through the canonical channel-mapping
        // path even though no host-side registry is wired yet. When
        // the diagnostics feature lights up, replacing the sink is a
        // one-line change.
        let channel_observer =
            graphshell_core::ux_diagnostics::UxChannelObserver::new(
                graphshell_core::ux_diagnostics::NoopChannelSink,
            );
        app.host
            .runtime
            .ux_observers
            .register(Box::new(channel_observer));
        // Slice 27: register the Activity Log recorder. The Arc clone
        // gives the host a snapshot handle while the registry owns
        // its observer-side adapter.
        app.host.runtime.ux_observers.register(Box::new(
            ActivityLogRecorderProxy(activity_log_recorder),
        ));
        app
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
                // Order: confirm_dialog → node_create → context_menu
                // → palette → node_finder → omnibar (modals on top
                // dismiss first; backgrounded modals next).
                if is_escape_key(&event) {
                    if self.confirm_dialog.is_open {
                        return Task::done(Message::ConfirmDialogCancel);
                    }
                    if self.frame_rename.is_open {
                        return Task::done(Message::FrameRenameCancel);
                    }
                    if self.node_create.is_open {
                        return Task::done(Message::NodeCreateCancel);
                    }
                    if self.context_menu.is_open {
                        return Task::done(Message::ContextMenuDismiss);
                    }
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
                // Arrow-key navigation when a modal is open. The arrows
                // also move the text_input cursor harmlessly within the
                // typed query — proper key consumption requires a
                // text_input on_key wiring deferred to a later slice.
                if self.command_palette.is_open {
                    if is_arrow_down_key(&event) {
                        return Task::done(Message::PaletteFocusDown);
                    }
                    if is_arrow_up_key(&event) {
                        return Task::done(Message::PaletteFocusUp);
                    }
                }
                if self.node_finder.is_open {
                    if is_arrow_down_key(&event) {
                        return Task::done(Message::NodeFinderFocusDown);
                    }
                    if is_arrow_up_key(&event) {
                        return Task::done(Message::NodeFinderFocusUp);
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
            Message::CameraChanged { pane_id, pan, zoom } => {
                // Legacy view-keyed entry — preserved so fit-to-screen
                // and cross-host paths keep observing camera changes.
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
                // Slice 35: per-pane cache on the active Frame.
                if let Some(pane_id) = pane_id {
                    let cached = self
                        .frame
                        .pane_cameras
                        .entry(pane_id)
                        .or_insert_with(CanvasCamera::default);
                    cached.pan = pan;
                    cached.zoom = zoom;
                    cached.pan_velocity = Vector2D::zero();
                }
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
                // Open the Node Finder pre-seeded with the routed query
                // and refreshed result list from the live graph.
                self.node_finder.all_results =
                    build_finder_results(&self.host.runtime.graph_app);
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
                    pane_grid::DragEvent::Picked { .. } => {
                        // Slice 36: surface drag-in-progress so the
                        // drop-zone hint banner renders.
                        self.frame.drag_in_progress = true;
                    }
                    pane_grid::DragEvent::Dropped { pane, target } => {
                        // `State::drop` handles all target variants
                        // (edge-of-grid, center-of-pane, edge-of-pane)
                        // with the correct split axis derived from the
                        // drop region (per pane_grid §3.1 defaults).
                        self.frame.split_state.drop(pane, target);
                        self.frame.drag_in_progress = false;
                    }
                    pane_grid::DragEvent::Canceled { .. } => {
                        self.frame.drag_in_progress = false;
                    }
                }
                Task::none()
            }
            Message::PaneGridResized(event) => {
                self.frame.split_state.resize(event.split, event.ratio);
                Task::none()
            }
            Message::ClosePane(pane) => {
                // Slice 35: drop the closing pane's per-pane camera
                // cache before the pane handle goes away.
                if let Some(meta) = self.frame.split_state.get(pane) {
                    self.frame.pane_cameras.remove(&meta.pane_id);
                }
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

            // Modal fade-in pattern (Slice 42, generalized in Slice 47):
            // every gs::Modal-backed surface sets `modal_opened_at` on
            // open, clears it on close, and reads `modal_fade_opacity`
            // in its renderer. Mutually exclusive overlays share the
            // same clock — opening one modal overwrites the previous
            // timestamp so the new surface always fades from scrim.
            Message::PaletteOpen { origin } => {
                // Opening the palette closes the node finder (mutually
                // exclusive overlays per the canonical specs).
                let was_finder_open = self.node_finder.is_open;
                self.node_finder.is_open = false;
                self.command_palette.is_open = true;
                self.command_palette.origin = origin;
                self.command_palette.query.clear();
                self.command_palette.focused_index = None;
                self.modal_opened_at = Some(std::time::Instant::now());
                if was_finder_open {
                    emit_ux_event(
                        self,
                        graphshell_core::ux_observability::UxEvent::SurfaceDismissed {
                            surface: graphshell_core::ux_observability::SurfaceId::NodeFinder,
                            reason:
                                graphshell_core::ux_observability::DismissReason::Superseded,
                        },
                    );
                }
                emit_ux_event(
                    self,
                    graphshell_core::ux_observability::UxEvent::SurfaceOpened {
                        surface: graphshell_core::ux_observability::SurfaceId::CommandPalette,
                    },
                );
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
                self.modal_opened_at = None;
                emit_ux_event(
                    self,
                    graphshell_core::ux_observability::UxEvent::SurfaceDismissed {
                        surface: graphshell_core::ux_observability::SurfaceId::CommandPalette,
                        reason: graphshell_core::ux_observability::DismissReason::Cancelled,
                    },
                );
                Task::none()
            }
            Message::PaletteActionSelected(idx) => {
                // Slice 10/28: resolve the visible-list slot to a
                // canonical ActionId. Slice 28 adds the host-side
                // intercept: ActionIds whose effect is opening
                // another iced overlay (palette / finder / settings
                // pane) are handled here, *before* pushing
                // HostIntent::Action — those can't be runtime-side
                // because the host owns the overlay state.
                //
                // Other actions push the intent and tick the runtime
                // so apply_host_intents records the dispatch in
                // last_dispatched_action / dispatched_action_count
                // and runs any per-action handler that has landed.
                let visible = visible_palette_actions(&self.command_palette);
                let acked = visible
                    .get(idx)
                    .filter(|a| a.is_available)
                    .map(|a| (a.label.clone(), a.action_id));
                if let Some((label, action_id)) = acked {
                    let key = action_id.key();
                    self.command_palette.is_open = false;
                    self.command_palette.query.clear();
                    self.command_palette.focused_index = None;
                    self.modal_opened_at = None;
                    self.host.toast_queue.push(graphshell_runtime::ToastSpec {
                        severity: ToastSeverity::Info,
                        message: format!("action: {label} [{key}]"),
                        duration: None,
                    });
                    // Host-side intercepts: turn the ActionId into
                    // an iced Message rather than a runtime intent.
                    if let Some(host_msg) = host_routed_action(action_id) {
                        return Task::done(host_msg);
                    }
                    // Runtime path.
                    self.host.pending_host_intents.push(
                        graphshell_core::shell_state::host_intent::HostIntent::Action {
                            action_id,
                        },
                    );
                    self.tick_with_events(Vec::new());
                }
                Task::none()
            }
            Message::PaletteFocusDown => {
                let visible_count = visible_palette_actions(&self.command_palette).len();
                if visible_count > 0 {
                    let next = self
                        .command_palette
                        .focused_index
                        .map(|i| (i + 1) % visible_count)
                        .unwrap_or(0);
                    self.command_palette.focused_index = Some(next);
                }
                Task::none()
            }
            Message::PaletteFocusUp => {
                let visible_count = visible_palette_actions(&self.command_palette).len();
                if visible_count > 0 {
                    let prev = self
                        .command_palette
                        .focused_index
                        .map(|i| if i == 0 { visible_count - 1 } else { i - 1 })
                        .unwrap_or(visible_count - 1);
                    self.command_palette.focused_index = Some(prev);
                }
                Task::none()
            }
            Message::PaletteSubmitFocused => {
                let idx = self.command_palette.focused_index.unwrap_or(0);
                Task::done(Message::PaletteActionSelected(idx))
            }

            // --- Node Finder handlers ---

            Message::NodeFinderOpen { origin } => {
                // Mutually exclusive with the command palette. Refresh
                // the result list from the live graph so the finder
                // always reflects current truth.
                let was_palette_open = self.command_palette.is_open;
                self.command_palette.is_open = false;
                // Slice 47: every gs::Modal-backed surface fades in
                // through the same clock. The previous palette's
                // timestamp is overwritten here so the finder fades
                // from the scrim cleanly.
                self.modal_opened_at = Some(std::time::Instant::now());
                self.node_finder.all_results =
                    build_finder_results(&self.host.runtime.graph_app);
                self.node_finder.is_open = true;
                self.node_finder.origin = origin;
                self.node_finder.query.clear();
                self.node_finder.focused_index = None;
                if was_palette_open {
                    emit_ux_event(
                        self,
                        graphshell_core::ux_observability::UxEvent::SurfaceDismissed {
                            surface:
                                graphshell_core::ux_observability::SurfaceId::CommandPalette,
                            reason:
                                graphshell_core::ux_observability::DismissReason::Superseded,
                        },
                    );
                }
                emit_ux_event(
                    self,
                    graphshell_core::ux_observability::UxEvent::SurfaceOpened {
                        surface: graphshell_core::ux_observability::SurfaceId::NodeFinder,
                    },
                );
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
                self.modal_opened_at = None;
                emit_ux_event(
                    self,
                    graphshell_core::ux_observability::UxEvent::SurfaceDismissed {
                        surface: graphshell_core::ux_observability::SurfaceId::NodeFinder,
                        reason: graphshell_core::ux_observability::DismissReason::Cancelled,
                    },
                );
                Task::none()
            }
            Message::NodeFinderResultSelected(idx) => {
                // Slice 12: resolve the visible-list slot to a real
                // NodeKey and push HostIntent::OpenNode. The runtime's
                // apply_host_intents promotes the node to
                // focused_node_hint and bumps opened_node_count;
                // pane-routing per WorkbenchProfile is downstream
                // domain work.
                let visible = visible_finder_results(&self.node_finder);
                let acked = visible.get(idx).map(|r| {
                    (r.node_key, r.title.clone(), r.address.clone())
                });
                if let Some((node_key, title, address)) = acked {
                    self.host.pending_host_intents.push(
                        graphshell_core::shell_state::host_intent::HostIntent::OpenNode {
                            node_key,
                        },
                    );
                    self.host.toast_queue.push(graphshell_runtime::ToastSpec {
                        severity: ToastSeverity::Info,
                        message: format!("open: {title} ({address})"),
                        duration: None,
                    });
                    self.node_finder.is_open = false;
                    self.node_finder.query.clear();
                    self.node_finder.focused_index = None;
                    self.modal_opened_at = None;
                    // Drive a tick so the runtime drains the intent
                    // immediately and observers see the focus change.
                    self.tick_with_events(Vec::new());
                }
                Task::none()
            }
            Message::NodeFinderFocusDown => {
                let visible_count = visible_finder_results(&self.node_finder).len();
                if visible_count > 0 {
                    let next = self
                        .node_finder
                        .focused_index
                        .map(|i| (i + 1) % visible_count)
                        .unwrap_or(0);
                    self.node_finder.focused_index = Some(next);
                }
                Task::none()
            }
            Message::NodeFinderFocusUp => {
                let visible_count = visible_finder_results(&self.node_finder).len();
                if visible_count > 0 {
                    let prev = self
                        .node_finder
                        .focused_index
                        .map(|i| if i == 0 { visible_count - 1 } else { i - 1 })
                        .unwrap_or(visible_count - 1);
                    self.node_finder.focused_index = Some(prev);
                }
                Task::none()
            }
            Message::NodeFinderSubmitFocused => {
                let idx = self.node_finder.focused_index.unwrap_or(0);
                Task::done(Message::NodeFinderResultSelected(idx))
            }

            // --- Context Menu handlers ---

            Message::ContextMenuOpen { target } => {
                // Mutually exclusive with palette / node-finder.
                let was_palette = self.command_palette.is_open;
                let was_finder = self.node_finder.is_open;
                self.command_palette.is_open = false;
                self.node_finder.is_open = false;
                // Slice 42: ContextMenu doesn't use gs::Modal —
                // clear the fade-in clock if a modal was open.
                self.modal_opened_at = None;
                self.context_menu.is_open = true;
                self.context_menu.target = target;
                // The cursor cache is fed by every CursorMoved
                // event; a missing entry only happens at startup
                // before the user has moved the pointer, which is
                // essentially impossible to reach via right-click
                // in practice. Falling back to Point::ORIGIN keeps
                // tests and pathological reentries deterministic.
                self.context_menu.anchor =
                    self.host.cursor_position.unwrap_or(Point::ORIGIN);
                self.context_menu.items = items_for_target(target);
                if was_palette {
                    emit_ux_event(
                        self,
                        graphshell_core::ux_observability::UxEvent::SurfaceDismissed {
                            surface:
                                graphshell_core::ux_observability::SurfaceId::CommandPalette,
                            reason:
                                graphshell_core::ux_observability::DismissReason::Superseded,
                        },
                    );
                }
                if was_finder {
                    emit_ux_event(
                        self,
                        graphshell_core::ux_observability::UxEvent::SurfaceDismissed {
                            surface: graphshell_core::ux_observability::SurfaceId::NodeFinder,
                            reason:
                                graphshell_core::ux_observability::DismissReason::Superseded,
                        },
                    );
                }
                emit_ux_event(
                    self,
                    graphshell_core::ux_observability::UxEvent::SurfaceOpened {
                        surface: graphshell_core::ux_observability::SurfaceId::ContextMenu,
                    },
                );
                Task::none()
            }
            Message::ContextMenuEntrySelected(idx) => {
                // Slice 13: resolve the row to an optional HostIntent
                // and push it onto pending_host_intents (same path the
                // palette and node finder use). Disabled rows are
                // no-ops; rows with `intent = None` toast only.
                //
                // Slice 14: destructive rows do NOT push immediately —
                // the intent is parked in `confirm_dialog.pending_intent`
                // and the user must confirm via the gated modal first.
                let acked = self.context_menu.items.get(idx).filter(|item| {
                    item.entry.disabled_reason.is_none()
                });
                let acked = acked.map(|item| {
                    (
                        item.entry.label.clone(),
                        item.entry.destructive,
                        item.intent.clone(),
                    )
                });
                if let Some((label, destructive, intent)) = acked {
                    self.context_menu.is_open = false;
                    self.context_menu.items.clear();
                    emit_ux_event(
                        self,
                        graphshell_core::ux_observability::UxEvent::SurfaceDismissed {
                            surface:
                                graphshell_core::ux_observability::SurfaceId::ContextMenu,
                            reason:
                                graphshell_core::ux_observability::DismissReason::Confirmed,
                        },
                    );

                    if destructive && intent.is_some() {
                        // Park the intent in the confirm dialog gate.
                        self.confirm_dialog.is_open = true;
                        self.confirm_dialog.action_label = label;
                        self.confirm_dialog.pending_intent = intent;
                        // Confirm dialog is gs::Modal-backed; fade in
                        // through the shared clock.
                        self.modal_opened_at = Some(std::time::Instant::now());
                        emit_ux_event(
                            self,
                            graphshell_core::ux_observability::UxEvent::SurfaceOpened {
                                surface:
                                    graphshell_core::ux_observability::SurfaceId::ConfirmDialog,
                            },
                        );
                        return Task::none();
                    }

                    let dispatched = if let Some(intent) = intent {
                        self.host.pending_host_intents.push(intent);
                        true
                    } else {
                        false
                    };
                    let suffix = if dispatched { "" } else { " (stub)" };
                    self.host.toast_queue.push(graphshell_runtime::ToastSpec {
                        severity: ToastSeverity::Info,
                        message: format!("context: {label}{suffix}"),
                        duration: None,
                    });
                    if dispatched {
                        self.tick_with_events(Vec::new());
                    }
                }
                Task::none()
            }
            Message::ContextMenuDismiss => {
                self.context_menu.is_open = false;
                self.context_menu.items.clear();
                emit_ux_event(
                    self,
                    graphshell_core::ux_observability::UxEvent::SurfaceDismissed {
                        surface: graphshell_core::ux_observability::SurfaceId::ContextMenu,
                        reason: graphshell_core::ux_observability::DismissReason::Cancelled,
                    },
                );
                Task::none()
            }

            // --- Confirm Dialog handlers ---

            Message::ConfirmDialogConfirm => {
                let label = std::mem::take(&mut self.confirm_dialog.action_label);
                let intent = self.confirm_dialog.pending_intent.take();
                self.confirm_dialog.is_open = false;
                self.modal_opened_at = None;
                emit_ux_event(
                    self,
                    graphshell_core::ux_observability::UxEvent::SurfaceDismissed {
                        surface: graphshell_core::ux_observability::SurfaceId::ConfirmDialog,
                        reason: graphshell_core::ux_observability::DismissReason::Confirmed,
                    },
                );
                if let Some(intent) = intent {
                    self.host.pending_host_intents.push(intent);
                    self.host.toast_queue.push(graphshell_runtime::ToastSpec {
                        severity: ToastSeverity::Info,
                        message: format!("confirmed: {label}"),
                        duration: None,
                    });
                    self.tick_with_events(Vec::new());
                }
                Task::none()
            }
            Message::ConfirmDialogCancel => {
                let _ = self.confirm_dialog.pending_intent.take();
                self.confirm_dialog.action_label.clear();
                self.confirm_dialog.is_open = false;
                self.modal_opened_at = None;
                emit_ux_event(
                    self,
                    graphshell_core::ux_observability::UxEvent::SurfaceDismissed {
                        surface: graphshell_core::ux_observability::SurfaceId::ConfirmDialog,
                        reason: graphshell_core::ux_observability::DismissReason::Cancelled,
                    },
                );
                Task::none()
            }

            // --- Tree Spine handlers ---

            Message::TreeSpineNodeClicked(node_key) => {
                self.host.pending_host_intents.push(
                    graphshell_core::shell_state::host_intent::HostIntent::OpenNode {
                        node_key,
                    },
                );
                self.tick_with_events(Vec::new());
                Task::none()
            }

            // --- Swatches handlers ---

            Message::SwatchClicked(recipe) => {
                self.host.toast_queue.push(graphshell_runtime::ToastSpec {
                    severity: ToastSeverity::Info,
                    message: format!("swatch: {} (stub)", recipe.label()),
                    duration: None,
                });
                Task::none()
            }

            // --- NodeCreate modal handlers ---

            Message::NodeCreateOpen => {
                self.node_create.is_open = true;
                self.node_create.url_draft.clear();
                self.modal_opened_at = Some(std::time::Instant::now());
                emit_ux_event(
                    self,
                    graphshell_core::ux_observability::UxEvent::SurfaceOpened {
                        surface: graphshell_core::ux_observability::SurfaceId::NodeCreate,
                    },
                );
                iced::widget::operation::focus(iced::widget::Id::new(
                    NODE_CREATE_INPUT_ID,
                ))
            }
            Message::NodeCreateInput(value) => {
                self.node_create.url_draft = value;
                Task::none()
            }
            Message::NodeCreateSubmit => {
                let draft = std::mem::take(&mut self.node_create.url_draft);
                self.node_create.is_open = false;
                self.modal_opened_at = None;
                emit_ux_event(
                    self,
                    graphshell_core::ux_observability::UxEvent::SurfaceDismissed {
                        surface: graphshell_core::ux_observability::SurfaceId::NodeCreate,
                        reason: graphshell_core::ux_observability::DismissReason::Confirmed,
                    },
                );
                if !draft.is_empty() {
                    self.queue_create_node_at_url(draft.clone());
                    self.host.toast_queue.push(graphshell_runtime::ToastSpec {
                        severity: ToastSeverity::Success,
                        message: format!("created: {draft}"),
                        duration: None,
                    });
                }
                Task::none()
            }
            Message::NodeCreateCancel => {
                self.node_create.url_draft.clear();
                self.node_create.is_open = false;
                self.modal_opened_at = None;
                emit_ux_event(
                    self,
                    graphshell_core::ux_observability::UxEvent::SurfaceDismissed {
                        surface: graphshell_core::ux_observability::SurfaceId::NodeCreate,
                        reason: graphshell_core::ux_observability::DismissReason::Cancelled,
                    },
                );
                Task::none()
            }

            // --- FrameRename modal handlers ---

            Message::FrameRenameOpen => {
                self.frame_rename.is_open = true;
                self.frame_rename.label_draft = self.frame_label.clone();
                self.modal_opened_at = Some(std::time::Instant::now());
                emit_ux_event(
                    self,
                    graphshell_core::ux_observability::UxEvent::SurfaceOpened {
                        surface: graphshell_core::ux_observability::SurfaceId::FrameRename,
                    },
                );
                iced::widget::operation::focus(iced::widget::Id::new(
                    FRAME_RENAME_INPUT_ID,
                ))
            }
            Message::FrameRenameInput(value) => {
                self.frame_rename.label_draft = value;
                Task::none()
            }
            Message::FrameRenameSubmit => {
                let draft = std::mem::take(&mut self.frame_rename.label_draft);
                let trimmed = draft.trim();
                if !trimmed.is_empty() {
                    self.frame_label = trimmed.to_string();
                }
                self.frame_rename.is_open = false;
                self.modal_opened_at = None;
                emit_ux_event(
                    self,
                    graphshell_core::ux_observability::UxEvent::SurfaceDismissed {
                        surface: graphshell_core::ux_observability::SurfaceId::FrameRename,
                        reason: graphshell_core::ux_observability::DismissReason::Confirmed,
                    },
                );
                Task::none()
            }
            Message::FrameRenameCancel => {
                self.frame_rename.label_draft.clear();
                self.frame_rename.is_open = false;
                self.modal_opened_at = None;
                emit_ux_event(
                    self,
                    graphshell_core::ux_observability::UxEvent::SurfaceDismissed {
                        surface: graphshell_core::ux_observability::SurfaceId::FrameRename,
                        reason: graphshell_core::ux_observability::DismissReason::Cancelled,
                    },
                );
                Task::none()
            }

            // --- Frame composition handlers ---

            Message::NewFrame => {
                let new_id = FrameId::next();
                let total = self.inactive_frames.len() + 2;
                let new_label = format!("Frame {total}");
                let mut new_state = FrameState::new();
                // Swap: new_state becomes the active frame; the
                // previously-active frame moves into inactive_frames.
                std::mem::swap(&mut self.frame, &mut new_state);
                let prev = NamedFrame {
                    id: self.frame_id,
                    label: std::mem::replace(&mut self.frame_label, new_label),
                    state: new_state,
                };
                self.inactive_frames.push(prev);
                self.frame_id = new_id;
                Task::none()
            }
            Message::SwitchFrame(index) => {
                if let Some(target) = self.inactive_frames.get_mut(index) {
                    std::mem::swap(&mut self.frame, &mut target.state);
                    std::mem::swap(&mut self.frame_id, &mut target.id);
                    std::mem::swap(&mut self.frame_label, &mut target.label);
                } else {
                    // Slice 41: previously a silent no-op when no
                    // inactive Frame existed at `index`. Surface so
                    // the user sees why FrameSelect from a single-
                    // Frame palette didn't switch.
                    self.host.toast_queue.push(graphshell_runtime::ToastSpec {
                        severity: ToastSeverity::Info,
                        message: "No other Frames open — open one with FrameOpen first.".into(),
                        duration: None,
                    });
                }
                Task::none()
            }
            Message::CloseCurrentFrame => {
                // Pull the first inactive frame in as the new active
                // frame; the closed frame is dropped. No-op when
                // there's only one frame open.
                if let Some(restored) = self.inactive_frames.pop() {
                    self.frame = restored.state;
                    self.frame_id = restored.id;
                    self.frame_label = restored.label;
                }
                Task::none()
            }

            // --- Settings pane handlers ---

            Message::SettingsToggleNavigatorLeft => {
                self.navigator.show_left = !self.navigator.show_left;
                Task::none()
            }
            Message::SettingsToggleNavigatorRight => {
                self.navigator.show_right = !self.navigator.show_right;
                Task::none()
            }
            Message::SettingsToggleNavigatorTop => {
                self.navigator.show_top = !self.navigator.show_top;
                Task::none()
            }
            Message::SettingsToggleNavigatorBottom => {
                self.navigator.show_bottom = !self.navigator.show_bottom;
                Task::none()
            }

            // --- Tile Tabs handlers ---

            Message::TileTabSelected { pane_id: _, node_key } => {
                self.host.pending_host_intents.push(
                    graphshell_core::shell_state::host_intent::HostIntent::OpenNode {
                        node_key,
                    },
                );
                self.tick_with_events(Vec::new());
                Task::none()
            }
            Message::TileTabClosed { pane_id: _, node_key } => {
                // Slice 29 stub: presentation-state transition lands
                // when the graphlet authority exposes
                // ToggleTilePresentationState. For now, surface the
                // intent so the user knows close was received.
                self.host.toast_queue.push(graphshell_runtime::ToastSpec {
                    severity: ToastSeverity::Info,
                    message: format!("close tile: n{} (stub)", node_key.index()),
                    duration: None,
                });
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
        // Slice 31: Frame switcher visible whenever there's more
        // than one Frame open. Single-Frame sessions skip the chrome
        // entirely so the trivial case stays uncluttered.
        if !self.inactive_frames.is_empty() {
            body_children.push(render_frame_switcher(self));
        }
        // Slice 36: drop-zone hint banner appears only while a pane
        // drag is in progress. Pane_grid handles the actual drop
        // logic; the banner is a visible cue so the user knows
        // dropping on an edge splits and dropping on the center
        // swaps panes.
        // Slice 38: banner alpha pulses with a 1200ms sine period to
        // signal "active" without being aggressive.
        if self.frame.drag_in_progress {
            let pulse = animation::pulse(
                std::time::Instant::now(),
                self.startup_instant,
                1200,
            );
            body_children.push(render_drop_zone_hint(pulse));
        }
        if self.navigator.show_top {
            body_children.push(render_navigator_host(self, NavigatorAnchor::Top));
        }
        body_children.push(main_row.into());
        if self.navigator.show_bottom {
            body_children.push(render_navigator_host(self, NavigatorAnchor::Bottom));
        }
        body_children.push(toast_stack.into());
        body_children.push(render_status_bar(self));

        let body: Element<'_, Message> = container(column(body_children).spacing(0))
            .padding(0)
            .width(Length::Fill)
            .height(Length::Fill)
            .into();

        // Layer overlays on top of the body. State-level mutual
        // exclusion means at most one of palette/finder/context_menu
        // should be open at a time, but the view tolerates concurrent
        // flags — last-pushed wins visually.
        let mut layered: Vec<Element<'_, Message>> = vec![body];
        if self.command_palette.is_open {
            layered.push(render_command_palette(self));
        }
        if self.node_finder.is_open {
            layered.push(render_node_finder(self));
        }
        if self.context_menu.is_open {
            layered.push(render_context_menu(self));
        }
        if self.node_create.is_open {
            layered.push(render_node_create_modal(self));
        }
        if self.frame_rename.is_open {
            layered.push(render_frame_rename_modal(self));
        }
        if self.confirm_dialog.is_open {
            layered.push(render_confirm_dialog(self));
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
mod tests;
