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
//! **Scope (Slice 19 / Stage A+E)**: Slice 18 wired more per-action
//! handlers. Slice 19 brings up the StatusBar slot from the
//! composition skeleton spec §2 — a 20px row at the bottom of the
//! Application column showing live runtime indicators: a green
//! "ready" dot, the cumulative `dispatched_action_count`, the
//! current `pending_host_intents` queue depth, and the
//! `focused_node_hint` (or "—" when no node is focused). All four
//! values read directly from runtime state — no new state shape
//! introduced. The bar is the bottom edge of the Elm triad's view;
//! per-target chrome (background-task indicators, sync status)
//! lands when those subsystems expose runtime view-model data.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use euclid::default::Vector2D;
use graph_canvas::camera::CanvasCamera;
use iced::time;
use iced::widget::{button, canvas, column, container, mouse_area, pane_grid, row, rule, scrollable, text, text_input};
use iced::{Element, Length, Point, Subscription, Task};
use graphshell_iced_widgets::{ContextMenu, ContextMenuEntry, Modal, TileTab, TileTabs};

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

/// One ranked action in the Command Palette. Host-side mirror of
/// [`iced_command_palette_spec.md` §2.3](
/// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_command_palette_spec.md)'s
/// `RankedAction`.
///
/// Slice 9: `action_id` carries the canonical `graphshell_core::actions::ActionId`,
/// `label` comes from `ActionId::label()`, and the master list comes
/// from `all_action_ids()`. Availability / disabled-reason / fuzzy
/// ranking are still placeholders pending an `ActionRegistryViewModel`
/// in `graphshell-runtime`.
#[derive(Debug, Clone)]
pub(crate) struct RankedAction {
    /// Canonical action identity from `graphshell_core::actions`.
    pub(crate) action_id: graphshell_core::actions::ActionId,
    /// Verb-target label (e.g. "Open Settings"), per the canonical
    /// command-surface wording rules.
    pub(crate) label: String,
    /// Optional secondary description rendered dimmer beneath the
    /// label. Slice 9 derives this from the action's category badge.
    pub(crate) description: Option<String>,
    /// Right-aligned shortcut hint (e.g. "Ctrl+,").
    pub(crate) keybinding: Option<String>,
    /// `false` greys the row out; selection is suppressed.
    pub(crate) is_available: bool,
    /// Tooltip-style explanation when `is_available == false`.
    pub(crate) disabled_reason: Option<String>,
}

/// Widget-local state for the Command Palette modal. Per
/// [`iced_command_palette_spec.md` §2.3](
/// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_command_palette_spec.md).
///
/// `all_actions` is the master list (placeholder data in Slice 7,
/// runtime-sourced once the `ActionRegistry` is wired). The visible
/// list is computed inline by [`visible_palette_actions`] so the
/// query filter is always consistent with the typed input. The
/// `focused_index` is an index into the *visible* slice, not the
/// master list.
#[derive(Debug, Clone)]
pub(crate) struct CommandPaletteState {
    pub(crate) is_open: bool,
    pub(crate) origin: PaletteOrigin,
    pub(crate) query: String,
    pub(crate) focused_index: Option<usize>,
    pub(crate) all_actions: Vec<RankedAction>,
}

impl Default for CommandPaletteState {
    fn default() -> Self {
        Self {
            is_open: false,
            origin: PaletteOrigin::KeyboardShortcut,
            query: String::new(),
            focused_index: None,
            all_actions: registry_actions(),
        }
    }
}

/// Build the palette's master action list from the canonical
/// `graphshell_core::actions::all_action_ids()` registry. Each entry
/// uses `ActionId::label()` as the row label and the category's label
/// as a description badge.
///
/// Availability is conservatively `true` for every action — the real
/// `is_available` predicate (selection-set awareness, capability
/// gating per [`command_surface_interaction_spec.md` §4.1](../../../design_docs/graphshell_docs/implementation_strategy/aspect_command/command_surface_interaction_spec.md))
/// requires an `ActionRegistryViewModel` from `graphshell-runtime`.
/// That swap doesn't change this function's caller — it just passes
/// the predicate result here instead of the literal `true`.
fn registry_actions() -> Vec<RankedAction> {
    graphshell_core::actions::all_action_ids()
        .iter()
        .copied()
        .map(|id| RankedAction {
            action_id: id,
            label: id.label().to_string(),
            description: Some(id.category().label().to_string()),
            keybinding: None,
            is_available: true,
            disabled_reason: None,
        })
        .collect()
}

/// Filter the palette's master list by query. Empty query → all rows.
/// Substring match (case-insensitive) on the label; the runtime swap
/// will replace this with `ActionRegistryViewModel::rank_for_query`.
fn visible_palette_actions(state: &CommandPaletteState) -> Vec<&RankedAction> {
    if state.query.is_empty() {
        state.all_actions.iter().collect()
    } else {
        let q = state.query.to_lowercase();
        state
            .all_actions
            .iter()
            .filter(|a| a.label.to_lowercase().contains(&q))
            .collect()
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

/// Where in a node the user's query matched. Per
/// [`iced_node_finder_spec.md` §4](
/// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_node_finder_spec.md).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MatchSource {
    Title,
    Address,
    Tag,
    Recency,
}

/// One result row in the Node Finder. Host-side mirror of
/// [`iced_node_finder_spec.md` §4](
/// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_node_finder_spec.md)'s
/// `NodeFinderResult`.
///
/// Slice 11: `node_key` carries the canonical
/// `graphshell_core::graph::NodeKey`; results are read live from
/// `runtime.graph_app.domain_graph()` whenever the finder opens.
/// Selection toasts the resolved title + URL; downstream slice will
/// route to a real `OpenNode` host intent.
#[derive(Debug, Clone)]
pub(crate) struct NodeFinderResult {
    /// Canonical node identity. Selection routing reads this directly.
    pub(crate) node_key: graphshell_core::graph::NodeKey,
    /// Node title (or address fallback when the title is empty).
    pub(crate) title: String,
    /// Canonical address (URL or `verso://...`).
    pub(crate) address: String,
    /// Short type chip (Web / Internal / Other) derived from the
    /// address scheme.
    pub(crate) node_type: String,
    /// Where the query matched (Title / Address / Recency).
    pub(crate) match_source: MatchSource,
}

/// Widget-local state for the Node Finder modal. Per
/// [`iced_node_finder_spec.md` §5](
/// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_node_finder_spec.md).
///
/// `all_results` is the master list. Empty-query means recency-ranked
/// (the placeholder list approximates this for Slice 7); a typed query
/// filters by substring on title or address.
#[derive(Debug, Clone)]
pub(crate) struct NodeFinderState {
    pub(crate) is_open: bool,
    pub(crate) origin: NodeFinderOrigin,
    pub(crate) query: String,
    pub(crate) focused_index: Option<usize>,
    pub(crate) all_results: Vec<NodeFinderResult>,
}

impl Default for NodeFinderState {
    fn default() -> Self {
        Self {
            is_open: false,
            origin: NodeFinderOrigin::KeyboardShortcut,
            query: String::new(),
            focused_index: None,
            all_results: Vec::new(),
        }
    }
}

/// Build the finder's result list from the runtime's current graph
/// state. Called when the finder opens (or when the omnibar routes
/// non-URL input here) so the result set always reflects current
/// truth — the spec's "recency-ranked" empty-query default is
/// approximated by graph-iteration order pending a real recency
/// aggregate in `SUBSYSTEM_HISTORY`.
fn build_finder_results(
    graph_app: &crate::app::GraphBrowserApp,
) -> Vec<NodeFinderResult> {
    graph_app
        .domain_graph()
        .nodes()
        .map(|(node_key, node)| {
            let address = node.url().to_string();
            let title = if node.title.is_empty() {
                address.clone()
            } else {
                node.title.clone()
            };
            let node_type = if address.starts_with("verso:") {
                "Internal".to_string()
            } else if address.starts_with("http://") || address.starts_with("https://") {
                "Web".to_string()
            } else {
                "Other".to_string()
            };
            NodeFinderResult {
                node_key,
                title,
                address,
                node_type,
                match_source: MatchSource::Recency,
            }
        })
        .collect()
}

/// Filter the finder's master list by query. Empty query → all rows
/// in recency order. Substring match on title or address; runtime
/// swap replaces this with fuzzy-match ranking when
/// `NodeFinderViewModel` lands.
fn visible_finder_results(state: &NodeFinderState) -> Vec<&NodeFinderResult> {
    if state.query.is_empty() {
        state.all_results.iter().collect()
    } else {
        let q = state.query.to_lowercase();
        state
            .all_results
            .iter()
            .filter(|r| {
                r.title.to_lowercase().contains(&q)
                    || r.address.to_lowercase().contains(&q)
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Confirm Dialog — Slice 14 (gate destructive intents)
// ---------------------------------------------------------------------------

/// Widget-local state for the confirmation modal that gates
/// destructive `HostIntent`s. Per
/// [`iced_command_palette_spec.md` §5](
/// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_command_palette_spec.md):
/// any command-surface entry marked `destructive` must route through
/// this gate before pushing its intent — the user must explicitly
/// confirm before the runtime sees the dispatch.
///
/// Slice 14 wires this for the ContextMenu's destructive entries
/// (today: Tombstone). Palette destructive routing lands once
/// `RankedAction` carries an `is_destructive` flag (the
/// `graphshell_core::actions::ActionId` registry doesn't expose
/// destructive metadata yet).
#[derive(Debug, Clone, Default)]
pub(crate) struct ConfirmDialogState {
    pub(crate) is_open: bool,
    /// Verb-target description rendered in the dialog body
    /// (e.g., "Tombstone the focused node").
    pub(crate) action_label: String,
    /// Intent to push on confirm. `None` while the dialog is closed.
    /// On Cancel / dismiss the intent is dropped without dispatch.
    pub(crate) pending_intent:
        Option<graphshell_core::shell_state::host_intent::HostIntent>,
}

// ---------------------------------------------------------------------------
// Context Menu — Slice 8 (right-click overlay on panes / base layer)
// ---------------------------------------------------------------------------

/// What was right-clicked. The target drives entry selection per
/// [`iced_command_palette_spec.md` §7.3](
/// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_command_palette_spec.md).
///
/// Slice 8 wired three targets: tile panes, canvas panes, and the
/// canvas base layer (empty Frame). Slice 16 added an optional
/// `node_key` to the pane variants so a future hit-test pass (canvas
/// node lookup, tile-tab right-click) can route actions to the
/// specific node under the cursor. The pane right-click handlers
/// today still pass `node_key: None` — hit-test wiring lands later.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ContextMenuTarget {
    TilePane {
        pane_id: PaneId,
        node_key: Option<graphshell_core::graph::NodeKey>,
    },
    CanvasPane {
        pane_id: PaneId,
        node_key: Option<graphshell_core::graph::NodeKey>,
    },
    BaseLayer,
}

impl ContextMenuTarget {
    /// The node the action applies to, if the target identifies one.
    /// `None` for `BaseLayer` and for pane variants whose hit-test
    /// hasn't been wired yet.
    fn node_key(self) -> Option<graphshell_core::graph::NodeKey> {
        match self {
            ContextMenuTarget::TilePane { node_key, .. } => node_key,
            ContextMenuTarget::CanvasPane { node_key, .. } => node_key,
            ContextMenuTarget::BaseLayer => None,
        }
    }
}

/// One row in the context menu, pairing the display entry with an
/// optional dispatch payload. `intent = None` means "stub-only" —
/// selection logs a toast but emits no host intent. `intent = Some(_)`
/// pushes the intent through the same `pending_host_intents` queue
/// that the palette and node finder use, so selection closes the
/// dispatch loop end-to-end.
#[derive(Debug, Clone)]
pub(crate) struct ContextMenuItem {
    pub(crate) entry: ContextMenuEntry,
    pub(crate) intent: Option<graphshell_core::shell_state::host_intent::HostIntent>,
}

impl ContextMenuItem {
    fn stub(entry: ContextMenuEntry) -> Self {
        Self { entry, intent: None }
    }

    /// Build an action item. If `target_node` is `Some`, the dispatch
    /// routes via `HostIntent::ActionOnNode` so the runtime
    /// pre-positions focus before running the per-action handler;
    /// otherwise it routes via `HostIntent::Action` and the handler
    /// operates on whatever the runtime considers focused.
    fn action(
        entry: ContextMenuEntry,
        action_id: graphshell_core::actions::ActionId,
        target_node: Option<graphshell_core::graph::NodeKey>,
    ) -> Self {
        let intent = match target_node {
            Some(node_key) => {
                graphshell_core::shell_state::host_intent::HostIntent::ActionOnNode {
                    action_id,
                    node_key,
                }
            }
            None => graphshell_core::shell_state::host_intent::HostIntent::Action {
                action_id,
            },
        };
        Self { entry, intent: Some(intent) }
    }
}

/// Widget-local state for the context-menu overlay. Mutually
/// exclusive with the modal overlays — opening the context menu
/// closes any open palette/finder.
#[derive(Debug, Clone)]
pub(crate) struct ContextMenuState {
    pub(crate) is_open: bool,
    pub(crate) anchor: Point,
    pub(crate) target: ContextMenuTarget,
    pub(crate) items: Vec<ContextMenuItem>,
}

impl Default for ContextMenuState {
    fn default() -> Self {
        Self {
            is_open: false,
            anchor: Point::ORIGIN,
            target: ContextMenuTarget::BaseLayer,
            items: Vec::new(),
        }
    }
}

/// Slice 13 entry sets per target. Each row pairs a display entry
/// with an optional `HostIntent`; selection pushes the intent
/// through the runtime if present. Replaced by the runtime
/// `ActionRegistry::available_for(target, view_model)` once that
/// surface lands.
///
/// Wired entries (selection actually dispatches):
/// - "Activate" → `NodeWarmSelect`
/// - "Pin" → `NodePinToggle`
/// - "Remove from graphlet" → `NodeRemoveFromGraphlet`
/// - "Tombstone" → `NodeMarkTombstone` (destructive — future slice
///   routes through ConfirmDialog before push)
/// - CanvasPane "Inspect" → `GraphCommandPalette` (placeholder
///   routing — a real "Inspect" action lands later)
///
/// Stub-only entries (toast only, no intent yet — need data the
/// current targets don't carry):
/// - CanvasPane "Open in Pane" — needs a destination Pane id
/// - BaseLayer "Open Pane" — needs a node target
/// - BaseLayer "Switch graphlet" — disabled (no graphlets yet)
fn items_for_target(target: ContextMenuTarget) -> Vec<ContextMenuItem> {
    use graphshell_core::actions::ActionId;
    let node = target.node_key();
    match target {
        ContextMenuTarget::TilePane { .. } => vec![
            ContextMenuItem::action(
                ContextMenuEntry::new("Activate"),
                ActionId::NodeWarmSelect,
                node,
            ),
            ContextMenuItem::action(ContextMenuEntry::new("Pin"), ActionId::NodePinToggle, node),
            ContextMenuItem::action(
                ContextMenuEntry::new("Remove from graphlet"),
                ActionId::NodeRemoveFromGraphlet,
                node,
            ),
            ContextMenuItem::action(
                ContextMenuEntry::new("Tombstone").destructive(),
                ActionId::NodeMarkTombstone,
                node,
            ),
        ],
        ContextMenuTarget::CanvasPane { .. } => vec![
            ContextMenuItem::stub(ContextMenuEntry::new("Open in Pane")),
            ContextMenuItem::action(ContextMenuEntry::new("Pin"), ActionId::NodePinToggle, node),
            ContextMenuItem::action(
                ContextMenuEntry::new("Inspect"),
                ActionId::GraphCommandPalette,
                node,
            ),
            ContextMenuItem::action(
                ContextMenuEntry::new("Remove from graphlet"),
                ActionId::NodeRemoveFromGraphlet,
                node,
            ),
            ContextMenuItem::action(
                ContextMenuEntry::new("Tombstone").destructive(),
                ActionId::NodeMarkTombstone,
                node,
            ),
        ],
        ContextMenuTarget::BaseLayer => vec![
            ContextMenuItem::stub(ContextMenuEntry::new("Open Pane")),
            ContextMenuItem::stub(
                ContextMenuEntry::new("Switch graphlet").disabled("No graphlets defined yet"),
            ),
        ],
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
    /// Right-click context-menu state. Per
    /// [`iced_command_palette_spec.md` §7.3](
    /// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_command_palette_spec.md).
    pub(crate) context_menu: ContextMenuState,
    /// Confirmation modal that gates destructive intents. Per
    /// [`iced_command_palette_spec.md` §5](
    /// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_command_palette_spec.md).
    pub(crate) confirm_dialog: ConfirmDialogState,
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

    // --- Tree Spine messages — Slice 20 ---

    /// User clicked a node row in the Tree Spine bucket. Dispatches
    /// `HostIntent::OpenNode { node_key }` so the runtime promotes
    /// it to focused state — same wiring as Node Finder selection.
    TreeSpineNodeClicked(graphshell_core::graph::NodeKey),

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
        Self {
            host: IcedHost::with_runtime(runtime),
            last_view_model: None,
            omnibar: OmnibarSession::default(),
            frame: FrameState::new(),
            navigator: NavigatorState::default(),
            command_palette: CommandPaletteState::default(),
            node_finder: NodeFinderState::default(),
            context_menu: ContextMenuState::default(),
            confirm_dialog: ConfirmDialogState::default(),
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
                // Order: confirm_dialog → context_menu → palette →
                // node_finder → omnibar (the dialog sits on top of
                // everything else when it's open).
                if is_escape_key(&event) {
                    if self.confirm_dialog.is_open {
                        return Task::done(Message::ConfirmDialogCancel);
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
                // Slice 10: resolve the visible-list slot to a canonical
                // ActionId and push HostIntent::Action onto the runtime's
                // pending-intents queue. The runtime's
                // `apply_host_intents` records the dispatch in
                // `last_dispatched_action` / `dispatched_action_count`
                // and tick-drives any per-action handlers that have
                // landed. The toast continues to surface the resolved
                // label + namespace:name key so the dispatch path is
                // user-visible. Disabled / out-of-range selections are
                // no-ops.
                let visible = visible_palette_actions(&self.command_palette);
                let acked = visible
                    .get(idx)
                    .filter(|a| a.is_available)
                    .map(|a| (a.label.clone(), a.action_id));
                if let Some((label, action_id)) = acked {
                    let key = action_id.key();
                    self.host.pending_host_intents.push(
                        graphshell_core::shell_state::host_intent::HostIntent::Action {
                            action_id,
                        },
                    );
                    self.host.toast_queue.push(graphshell_runtime::ToastSpec {
                        severity: ToastSeverity::Info,
                        message: format!("action: {label} [{key}]"),
                        duration: None,
                    });
                    self.command_palette.is_open = false;
                    self.command_palette.query.clear();
                    self.command_palette.focused_index = None;
                    // Drive a tick so the runtime drains the intent
                    // immediately and observers see the dispatch.
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
                self.command_palette.is_open = false;
                self.node_finder.all_results =
                    build_finder_results(&self.host.runtime.graph_app);
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
                self.command_palette.is_open = false;
                self.node_finder.is_open = false;
                self.context_menu.is_open = true;
                self.context_menu.target = target;
                self.context_menu.anchor =
                    self.host.cursor_position.unwrap_or(Point::ORIGIN);
                self.context_menu.items = items_for_target(target);
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

                    if destructive && intent.is_some() {
                        // Park the intent in the confirm dialog gate.
                        self.confirm_dialog.is_open = true;
                        self.confirm_dialog.action_label = label;
                        self.confirm_dialog.pending_intent = intent;
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
                Task::none()
            }

            // --- Confirm Dialog handlers ---

            Message::ConfirmDialogConfirm => {
                let label = std::mem::take(&mut self.confirm_dialog.action_label);
                let intent = self.confirm_dialog.pending_intent.take();
                self.confirm_dialog.is_open = false;
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
/// program; Tile panes show a tile-tab bar over a placeholder body.
///
/// The body is wrapped in a `mouse_area` whose `on_right_press` opens
/// the context menu against the appropriate `ContextMenuTarget`. The
/// anchor (cursor position) is read in the message handler from
/// `IcedHost.cursor_position`.
fn render_pane_body<'a>(app: &'a IcedApp, meta: &PaneMeta) -> Element<'a, Message> {
    let inner: Element<'a, Message> = match meta.pane_type {
        PaneType::Canvas => {
            let program =
                graph_canvas_from_app(&app.host.runtime.graph_app, app.host.view_id);
            let _: &GraphCanvasProgram = &program;
            let graph: Element<'_, super::iced_graph_canvas::GraphCanvasMessage> =
                canvas(program).width(Length::Fill).height(Length::Fill).into();
            // Capture the pane id so RightClicked can target this pane.
            let pane_id = meta.pane_id;
            graph.map(move |gcm| match gcm {
                super::iced_graph_canvas::GraphCanvasMessage::CameraChanged { pan, zoom } => {
                    Message::CameraChanged { pan, zoom }
                }
                super::iced_graph_canvas::GraphCanvasMessage::RightClicked { hit_node } => {
                    Message::ContextMenuOpen {
                        target: ContextMenuTarget::CanvasPane {
                            pane_id,
                            node_key: hit_node,
                        },
                    }
                }
            })
        }
        PaneType::Tile => {
            // Tile pane: TileTabs bar over the tile body.
            // Tile list and selection state come from the Navigator domain
            // layer (S5); this slice uses placeholder tabs so the widget is
            // exercised in the live layout.
            //
            // Slice 21 wires per-tab right-click: each tab dispatches
            // ContextMenuOpen with TilePane { pane_id, node_key }. Tab
            // → NodeKey lookup currently passes None — the structural
            // wiring closes; future slices populate node_keys when
            // real tile data is wired.
            let pane_id_for_tabs = meta.pane_id;
            let tabs = TileTabs::new()
                .push(TileTab::new("Tab A"))
                .push(TileTab::new("Tab B"))
                .selected(Some(0))
                .on_select(|_i| Message::Tick)
                .on_close(|_i| Message::Tick)
                .on_right_click(move |_i| Message::ContextMenuOpen {
                    target: ContextMenuTarget::TilePane {
                        pane_id: pane_id_for_tabs,
                        node_key: None,
                    },
                });
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
    };

    // Slice 17: canvas panes handle right-click natively in the
    // canvas Program (hit-test populates node_key). Tile panes still
    // route right-click via the outer mouse_area since they don't
    // have an inner Program; tile-tab right-click hit-test lands
    // when the tile bar exposes per-tab targets.
    match meta.pane_type {
        PaneType::Canvas => inner,
        PaneType::Tile => mouse_area(inner)
            .on_right_press(Message::ContextMenuOpen {
                target: ContextMenuTarget::TilePane {
                    pane_id: meta.pane_id,
                    node_key: None,
                },
            })
            .into(),
    }
}

/// Canvas base layer — rendered when the Frame has zero Panes.
///
/// This is the same `GraphCanvasProgram` used inside Canvas Panes;
/// per spec §2.3 the base layer is a distinct code branch, not a
/// degenerate Pane. Wrapped in a `mouse_area` so right-click opens the
/// `ContextMenuTarget::BaseLayer` menu.
fn render_canvas_base_layer(app: &IcedApp) -> Element<'_, Message> {
    let program = graph_canvas_from_app(&app.host.runtime.graph_app, app.host.view_id);
    let _: &GraphCanvasProgram = &program;
    let graph: Element<'_, super::iced_graph_canvas::GraphCanvasMessage> =
        canvas(program).width(Length::Fill).height(Length::Fill).into();
    // Slice 17: the canvas program now handles right-click natively
    // and runs hit-test. Empty-space right-click still falls through
    // to BaseLayer; node-on right-click would currently surface
    // CanvasPane semantics, but the base layer has no pane id so we
    // route every right-click to BaseLayer for now. A later slice
    // can introduce a `BaseLayerWithNode { node_key }` target.
    graph
        .map(|gcm| match gcm {
            super::iced_graph_canvas::GraphCanvasMessage::CameraChanged { pan, zoom } => {
                Message::CameraChanged { pan, zoom }
            }
            super::iced_graph_canvas::GraphCanvasMessage::RightClicked { .. } => {
                Message::ContextMenuOpen {
                    target: ContextMenuTarget::BaseLayer,
                }
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
    // Tree Spine bucket — Slice 20 reads from the runtime's GraphTree
    // and renders one row per member. Each row is a button that
    // dispatches `Message::TreeSpineNodeClicked(node_key)` → the
    // runtime promotes the node to focused via HostIntent::OpenNode.
    let tree_spine: Element<'_, Message> = render_tree_spine_bucket(app);

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
/// Slice 7 renders real result rows from the (placeholder) action list,
/// with focused-state highlighting and click handlers per row. Disabled
/// rows render dimmed and accept no clicks (`on_press_maybe(None)`).
/// Arrow-key navigation routes through `PaletteFocusUp/Down`; Enter
/// fires the focused row via `PaletteSubmitFocused`.
fn render_command_palette(app: &IcedApp) -> Element<'_, Message> {
    let input = text_input("Type a command or search…", &app.command_palette.query)
        .id(iced::widget::Id::new(PALETTE_INPUT_ID))
        .on_input(Message::PaletteQuery)
        .on_submit(Message::PaletteSubmitFocused)
        .size(14)
        .padding(6)
        .width(Length::Fill);

    let divider = rule::horizontal(1.0);

    let visible = visible_palette_actions(&app.command_palette);
    let focused = app.command_palette.focused_index;

    let results: Element<'_, Message> = if visible.is_empty() {
        let empty_label = if app.command_palette.query.is_empty() {
            "No actions available."
        } else {
            "No matching actions."
        };
        container(text(empty_label).size(12))
            .padding(12)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    } else {
        let rows: Vec<Element<'_, Message>> = visible
            .iter()
            .enumerate()
            .map(|(i, action)| render_palette_row(i, action, focused == Some(i)))
            .collect();
        scrollable(column(rows).spacing(2).padding(4))
            .height(Length::Fill)
            .into()
    };

    let footer = text("Esc to dismiss · ↑/↓ to navigate · Enter to run").size(11);

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

/// One row in the Command Palette ranked-action list.
///
/// Layout: label (filling, bold-ish via size) on the left, optional
/// keybinding right-aligned. Description (if present) appears beneath
/// the label at smaller size. Disabled rows pass `None` to
/// `on_press_maybe`; focused rows get a brighter background.
fn render_palette_row<'a>(
    idx: usize,
    action: &'a RankedAction,
    is_focused: bool,
) -> Element<'a, Message> {
    // Header line: label + optional keybinding.
    let label_el: Element<'a, Message> = text(action.label.as_str()).size(13).width(Length::Fill).into();
    let header: Element<'a, Message> = if let Some(kb) = action.keybinding.as_deref() {
        row![label_el, text(kb).size(11)]
            .spacing(8)
            .align_y(iced::Alignment::Center)
            .into()
    } else {
        label_el
    };

    // Optional description line.
    let body: Element<'a, Message> = match action.description.as_deref() {
        Some(desc) if !desc.is_empty() => column![header, text(desc).size(11)]
            .spacing(2)
            .into(),
        _ => header,
    };

    let msg: Option<Message> = if action.is_available {
        Some(Message::PaletteActionSelected(idx))
    } else {
        None
    };

    let is_disabled = !action.is_available;

    button(body)
        .on_press_maybe(msg)
        .padding([6, 10])
        .width(Length::Fill)
        .style(move |theme: &iced::Theme, status| {
            let pal = theme.palette();
            let hovered = matches!(
                status,
                iced::widget::button::Status::Hovered
                    | iced::widget::button::Status::Pressed
            );
            let bg = if is_focused {
                Some(pal.primary.weak.color.into())
            } else if hovered && !is_disabled {
                Some(iced::Color::from_rgba(1.0, 1.0, 1.0, 0.05).into())
            } else {
                None
            };
            let text_color = if is_disabled {
                iced::Color {
                    a: pal.background.base.text.a * 0.4,
                    ..pal.background.base.text
                }
            } else if is_focused {
                pal.primary.weak.text
            } else {
                pal.background.base.text
            };
            iced::widget::button::Style {
                background: bg,
                text_color,
                border: iced::Border {
                    radius: 3.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        })
        .into()
}

/// Render the Node Finder modal. Per
/// [`iced_node_finder_spec.md` §3](
/// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_node_finder_spec.md).
///
/// Slice 7 renders real result rows from the (placeholder) result list
/// with click handlers and focused-state highlighting. Arrow-key nav
/// routes through `NodeFinderFocusUp/Down`; Enter fires the focused row
/// via `NodeFinderSubmitFocused`.
fn render_node_finder(app: &IcedApp) -> Element<'_, Message> {
    let input = text_input(
        "Search nodes by title, tag, URL, or content…",
        &app.node_finder.query,
    )
    .id(iced::widget::Id::new(NODE_FINDER_INPUT_ID))
    .on_input(Message::NodeFinderQuery)
    .on_submit(Message::NodeFinderSubmitFocused)
    .size(14)
    .padding(6)
    .width(Length::Fill);

    let divider = rule::horizontal(1.0);

    let visible = visible_finder_results(&app.node_finder);
    let focused = app.node_finder.focused_index;

    let results: Element<'_, Message> = if visible.is_empty() {
        let empty_label = if app.node_finder.query.is_empty() {
            "No recently-active nodes yet."
        } else {
            "No matching nodes."
        };
        container(text(empty_label).size(12))
            .padding(12)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    } else {
        let rows: Vec<Element<'_, Message>> = visible
            .iter()
            .enumerate()
            .map(|(i, result)| render_finder_row(i, result, focused == Some(i)))
            .collect();
        scrollable(column(rows).spacing(2).padding(4))
            .height(Length::Fill)
            .into()
    };

    let footer = text("Esc to dismiss · ↑/↓ to navigate · Enter to open").size(11);

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

/// One row in the Node Finder result list.
///
/// Layout: title (filling) + node-type chip on the right; address
/// (smaller, dimmer) beneath the title. The match-source badge from
/// the spec is folded into the type chip until styling tokens land.
fn render_finder_row<'a>(
    idx: usize,
    result: &'a NodeFinderResult,
    is_focused: bool,
) -> Element<'a, Message> {
    let title_el: Element<'a, Message> = text(result.title.as_str()).size(13).width(Length::Fill).into();
    let header: Element<'a, Message> = row![
        title_el,
        text(result.node_type.as_str()).size(10),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center)
    .into();

    let body = column![header, text(result.address.as_str()).size(11)]
        .spacing(2);

    button(body)
        .on_press(Message::NodeFinderResultSelected(idx))
        .padding([6, 10])
        .width(Length::Fill)
        .style(move |theme: &iced::Theme, status| {
            let pal = theme.palette();
            let hovered = matches!(
                status,
                iced::widget::button::Status::Hovered
                    | iced::widget::button::Status::Pressed
            );
            let bg = if is_focused {
                Some(pal.primary.weak.color.into())
            } else if hovered {
                Some(iced::Color::from_rgba(1.0, 1.0, 1.0, 0.05).into())
            } else {
                None
            };
            let text_color = if is_focused {
                pal.primary.weak.text
            } else {
                pal.background.base.text
            };
            iced::widget::button::Style {
                background: bg,
                text_color,
                border: iced::Border {
                    radius: 3.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        })
        .into()
}

/// Render the confirmation modal that gates destructive intents.
/// Shown when `ConfirmDialogState::is_open` is `true`; click-outside
/// (`Modal::on_blur`) and Escape both fire `ConfirmDialogCancel`. Per
/// [`iced_command_palette_spec.md` §5](
/// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_command_palette_spec.md).
fn render_confirm_dialog(app: &IcedApp) -> Element<'_, Message> {
    let title = text("Confirm destructive action").size(15);
    let body = text(format!(
        "{} cannot be undone. Continue?",
        app.confirm_dialog.action_label
    ))
    .size(13);

    let cancel = button(text("Cancel").size(13))
        .on_press(Message::ConfirmDialogCancel)
        .padding([6, 14]);
    let confirm = button(text("Confirm").size(13))
        .on_press(Message::ConfirmDialogConfirm)
        .padding([6, 14])
        .style(|theme: &iced::Theme, status| {
            let pal = theme.palette();
            let hovered = matches!(
                status,
                iced::widget::button::Status::Hovered
                    | iced::widget::button::Status::Pressed
            );
            iced::widget::button::Style {
                background: Some(if hovered {
                    pal.danger.strong.color.into()
                } else {
                    pal.danger.base.color.into()
                }),
                text_color: pal.danger.strong.text,
                border: iced::Border {
                    radius: 3.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        });

    let buttons = iced::widget::row![
        iced::widget::Space::new().width(Length::Fill),
        cancel,
        confirm,
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center);

    let inner = column![title, body, buttons]
        .spacing(12)
        .padding(4)
        .width(Length::Fill);

    Modal::new(inner)
        .on_blur(Message::ConfirmDialogCancel)
        .max_width(420.0)
        .into()
}

/// Render the right-click context menu using `gs::ContextMenu`. The
/// widget itself does the positioning (via `pin` at the recorded
/// anchor) and the overlay layering (full-viewport dismiss area
/// behind an opaque menu panel). The host-side `ContextMenuItem`
/// pairs the display entry with an optional dispatch payload; only
/// the entry half is handed to the widget.
fn render_context_menu(app: &IcedApp) -> Element<'_, Message> {
    let mut menu = ContextMenu::new(app.context_menu.anchor)
        .on_select(Message::ContextMenuEntrySelected)
        .on_dismiss(Message::ContextMenuDismiss);
    for item in &app.context_menu.items {
        menu = menu.push(item.entry.clone());
    }
    menu.into()
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

/// Is this iced event an ArrowDown keypress?
fn is_arrow_down_key(event: &iced::Event) -> bool {
    matches!(
        event,
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowDown),
            ..
        })
    )
}

/// Is this iced event an ArrowUp keypress?
fn is_arrow_up_key(event: &iced::Event) -> bool {
    matches!(
        event,
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowUp),
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

/// Render the Tree Spine bucket — Navigator's left-rail "structural
/// list" of nodes in the workbench's GraphTree. Per
/// [`iced_composition_skeleton_spec.md` §6.1](
/// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_composition_skeleton_spec.md).
///
/// Slice 20 wiring: read from `runtime.graph_tree.members()` and emit
/// one button per member with the resolved title (from the domain
/// graph). Click → `Message::TreeSpineNodeClicked(node_key)` → push
/// `HostIntent::OpenNode { node_key }`. Lifecycle / Active+Inactive
/// toggles, indentation by topology depth, and `lazy` virtualization
/// are subsequent slices once their domain hooks are wired.
fn render_tree_spine_bucket(app: &IcedApp) -> Element<'_, Message> {
    let runtime = &app.host.runtime;
    let header: Element<'_, Message> = text("Tree Spine")
        .size(11)
        .width(Length::Fill)
        .into();

    let member_count = runtime.graph_tree.member_count();
    if member_count == 0 {
        return scrollable(
            column![header, text("  ○ no members yet").size(11)].spacing(4),
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

    let mut spine = column![header];
    for row in rows {
        spine = spine.push(row);
    }

    scrollable(spine.spacing(2).padding([2, 0]))
        .height(Length::FillPortion(2))
        .into()
}

/// One row in the Tree Spine list. Click dispatches an OpenNode
/// intent against the row's NodeKey.
fn tree_spine_row<'a>(
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
                    Some(iced::Color::from_rgba(1.0, 1.0, 1.0, 0.05).into())
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

/// Render the StatusBar slot. Per
/// [`iced_composition_skeleton_spec.md` §2](
/// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_composition_skeleton_spec.md):
/// ambient system status, process indicators, background task count.
///
/// Slice 19 wires four indicators sourced from runtime state:
/// - **status dot** — green "ready" pulse (will animate on activity
///   in a later slice with `cosmic-time`)
/// - **actions** — `runtime.dispatched_action_count` (cumulative
///   `HostIntent::Action` dispatches since runtime start)
/// - **pending** — `host.pending_host_intents.len()` (queued intents
///   awaiting the next tick drain)
/// - **focused** — `runtime.focused_node_hint` (rendered as the
///   underlying NodeKey index, or "—" when no node is focused)
fn render_status_bar(app: &IcedApp) -> Element<'_, Message> {
    let dispatched = app.host.runtime.dispatched_action_count;
    let pending = app.host.pending_host_intents.len();
    let focused_label = app
        .host
        .runtime
        .focused_node_hint
        .map(|k| format!("n{}", k.index()))
        .unwrap_or_else(|| "—".to_string());

    let dot = text("●").size(11).style(|theme: &iced::Theme| {
        let pal = theme.palette();
        iced::widget::text::Style {
            color: Some(pal.success.base.color),
        }
    });
    let ready = text("ready").size(11);
    let actions = text(format!("actions: {dispatched}")).size(11);
    let pending_text = text(format!("pending: {pending}")).size(11);
    let focused = text(format!("focused: {focused_label}")).size(11);

    container(
        iced::widget::row![
            dot,
            ready,
            iced::widget::Space::new().width(Length::Fixed(8.0)),
            actions,
            iced::widget::Space::new().width(Length::Fixed(8.0)),
            pending_text,
            iced::widget::Space::new().width(Length::Fixed(8.0)),
            focused,
            iced::widget::Space::new().width(Length::Fill),
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center),
    )
    .padding([3, 8])
    .width(Length::Fill)
    .height(Length::Fixed(20.0))
    .style(|theme: &iced::Theme| {
        let pal = theme.palette();
        container::Style {
            background: Some(
                iced::Color {
                    a: 0.05,
                    ..pal.background.base.text
                }
                .into(),
            ),
            ..Default::default()
        }
    })
    .into()
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

        // Slice 9: row 0 is whatever the canonical registry's first
        // action is — capture its label so the assertion stays stable
        // as the registry evolves.
        let expected_label = app.command_palette.all_actions[0].label.clone();
        let _ = app.update(Message::PaletteActionSelected(0));

        assert!(!app.command_palette.is_open);
        assert_eq!(app.host.toast_queue.len(), 1);
        assert!(
            app.host.toast_queue[0].message.contains(&expected_label),
            "expected resolved label in toast, got: {}",
            app.host.toast_queue[0].message,
        );
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

    // --- Modal data + nav tests (Slice 7) ---

    #[test]
    fn palette_action_select_dispatches_host_intent() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        let _ = app.update(Message::PaletteOpen {
            origin: PaletteOrigin::KeyboardShortcut,
        });

        let visible = visible_palette_actions(&app.command_palette);
        let row0_id = visible[0].action_id;
        assert_eq!(
            app.host.runtime.dispatched_action_count, 0,
            "no dispatch yet"
        );
        assert!(app.host.runtime.last_dispatched_action.is_none());

        let _ = app.update(Message::PaletteActionSelected(0));

        // The intent landed on pending_host_intents AND was drained
        // by the inline tick that PaletteActionSelected triggers.
        assert!(
            app.host.pending_host_intents.is_empty(),
            "intent queue drained by post-select tick",
        );
        assert_eq!(
            app.host.runtime.dispatched_action_count, 1,
            "runtime observed exactly one HostIntent::Action",
        );
        assert_eq!(
            app.host.runtime.last_dispatched_action,
            Some(row0_id),
            "runtime recorded the resolved ActionId",
        );
    }

    #[test]
    fn toggle_physics_action_actually_toggles_runtime_flag() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let initial_running = app
            .host
            .runtime
            .graph_app
            .workspace
            .graph_runtime
            .physics
            .is_running;

        // Push HostIntent::Action(GraphTogglePhysics) directly via the
        // queue so we don't have to find the right palette index.
        app.host.pending_host_intents.push(
            graphshell_core::shell_state::host_intent::HostIntent::Action {
                action_id: graphshell_core::actions::ActionId::GraphTogglePhysics,
            },
        );
        app.tick_with_events(Vec::new());

        let after_running = app
            .host
            .runtime
            .graph_app
            .workspace
            .graph_runtime
            .physics
            .is_running;

        assert_ne!(
            initial_running, after_running,
            "GraphTogglePhysics should flip physics.is_running",
        );
        assert_eq!(app.host.runtime.dispatched_action_count, 1);

        // A second dispatch flips it back.
        app.host.pending_host_intents.push(
            graphshell_core::shell_state::host_intent::HostIntent::Action {
                action_id: graphshell_core::actions::ActionId::GraphTogglePhysics,
            },
        );
        app.tick_with_events(Vec::new());
        let twice_toggled = app
            .host
            .runtime
            .graph_app
            .workspace
            .graph_runtime
            .physics
            .is_running;
        assert_eq!(twice_toggled, initial_running, "second toggle restores");
    }

    #[test]
    fn action_on_node_pre_focuses_target_then_dispatches() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        // Seed two nodes so we have real NodeKeys to target.
        seed_test_nodes(&mut app, 2);
        let target_key = app
            .host
            .runtime
            .graph_app
            .domain_graph()
            .nodes()
            .nth(1)
            .map(|(k, _)| k)
            .expect("seeded ≥2 nodes");

        // Dispatch an action targeting the second node directly.
        app.host.pending_host_intents.push(
            graphshell_core::shell_state::host_intent::HostIntent::ActionOnNode {
                action_id: graphshell_core::actions::ActionId::NodePinToggle,
                node_key: target_key,
            },
        );
        app.tick_with_events(Vec::new());

        assert_eq!(
            app.host.runtime.focused_node_hint,
            Some(target_key),
            "ActionOnNode pre-focuses the target before running the handler",
        );
        assert_eq!(app.host.runtime.dispatched_action_count, 1);
        assert_eq!(
            app.host.runtime.last_dispatched_action,
            Some(graphshell_core::actions::ActionId::NodePinToggle),
        );
    }

    #[test]
    fn context_menu_with_target_node_dispatches_action_on_node() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        seed_test_nodes(&mut app, 1);
        let target_key = app
            .host
            .runtime
            .graph_app
            .domain_graph()
            .nodes()
            .next()
            .map(|(k, _)| k)
            .unwrap();

        // Manually open a context menu against a target carrying a
        // real NodeKey — Slice 16 ships the type but the right-click
        // handlers don't hit-test yet, so this simulates a future
        // canvas-hit-test path.
        let _ = app.update(Message::ContextMenuOpen {
            target: ContextMenuTarget::CanvasPane {
                pane_id: PaneId(99),
                node_key: Some(target_key),
            },
        });

        // Pick the "Pin" entry — wired to NodePinToggle.
        let pin_idx = app
            .context_menu
            .items
            .iter()
            .position(|i| i.entry.label == "Pin")
            .expect("CanvasPane menu carries a Pin entry");
        let pin_intent = app.context_menu.items[pin_idx].intent.clone();

        // The intent should be ActionOnNode (target carries a node_key).
        assert!(matches!(
            pin_intent,
            Some(graphshell_core::shell_state::host_intent::HostIntent::ActionOnNode { .. })
        ));

        let _ = app.update(Message::ContextMenuEntrySelected(pin_idx));

        assert_eq!(app.host.runtime.focused_node_hint, Some(target_key));
        assert_eq!(app.host.runtime.dispatched_action_count, 1);
    }

    #[test]
    fn context_menu_without_target_node_dispatches_plain_action() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        if let Some((_, meta)) = app.frame.split_state.iter_mut().next() {
            meta.pane_type = PaneType::Tile;
        }
        let pane_id = app
            .frame
            .split_state
            .iter()
            .next()
            .map(|(_, m)| m.pane_id)
            .unwrap();

        // Right-click the pane body — current handler passes node_key: None.
        let _ = app.update(Message::ContextMenuOpen {
            target: ContextMenuTarget::TilePane {
                pane_id,
                node_key: None,
            },
        });

        let pin_idx = app
            .context_menu
            .items
            .iter()
            .position(|i| i.entry.label == "Pin")
            .unwrap();
        let pin_intent = app.context_menu.items[pin_idx].intent.clone();

        // Without a target node, the intent is plain Action.
        assert!(matches!(
            pin_intent,
            Some(graphshell_core::shell_state::host_intent::HostIntent::Action { .. })
        ));
    }

    #[test]
    fn tree_spine_click_dispatches_open_node_intent() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        seed_test_nodes(&mut app, 1);

        // Pull a NodeKey out of the graph (the GraphTree may be empty
        // until incremental_sync runs, but the dispatch path is what
        // we're testing).
        let node_key = app
            .host
            .runtime
            .graph_app
            .domain_graph()
            .nodes()
            .next()
            .map(|(k, _)| k)
            .expect("seeded a node");

        assert_eq!(app.host.runtime.opened_node_count, 0);

        let _ = app.update(Message::TreeSpineNodeClicked(node_key));

        assert_eq!(
            app.host.runtime.opened_node_count, 1,
            "tree-spine click dispatches HostIntent::OpenNode",
        );
        assert_eq!(
            app.host.runtime.focused_node_hint,
            Some(node_key),
            "runtime promoted the resolved NodeKey to focused",
        );
    }

    #[test]
    fn view_renders_status_bar_with_runtime_counters() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        // Smoke test: status bar renders with default state. Drop
        // the borrow before the next mutation.
        {
            let _element = app.view();
        }

        // Dispatch an action to bump the actions counter; render again.
        app.host.pending_host_intents.push(
            graphshell_core::shell_state::host_intent::HostIntent::Action {
                action_id: graphshell_core::actions::ActionId::GraphTogglePhysics,
            },
        );
        app.tick_with_events(Vec::new());
        assert_eq!(app.host.runtime.dispatched_action_count, 1);

        let _ = app.update(Message::Tick);
        let _element = app.view();
    }

    #[test]
    fn graph_fit_action_dispatches_without_panicking() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        // Seed a node so there's a focused view available.
        seed_test_nodes(&mut app, 1);

        app.host.pending_host_intents.push(
            graphshell_core::shell_state::host_intent::HostIntent::Action {
                action_id: graphshell_core::actions::ActionId::GraphFit,
            },
        );
        app.tick_with_events(Vec::new());

        // The fit request lands on the focused view's camera-command
        // queue, drained by the next render frame. The test confirms
        // the routing path closed without panicking.
        assert_eq!(app.host.runtime.dispatched_action_count, 1);
        assert_eq!(
            app.host.runtime.last_dispatched_action,
            Some(graphshell_core::actions::ActionId::GraphFit),
        );
    }

    #[test]
    fn persist_undo_and_redo_actions_are_dispatched() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        // Both undo and redo are no-ops without a checkpoint history,
        // but the dispatch path must still run without panicking.
        for action_id in [
            graphshell_core::actions::ActionId::PersistUndo,
            graphshell_core::actions::ActionId::PersistRedo,
        ] {
            app.host.pending_host_intents.push(
                graphshell_core::shell_state::host_intent::HostIntent::Action { action_id },
            );
            app.tick_with_events(Vec::new());
        }

        assert_eq!(app.host.runtime.dispatched_action_count, 2);
    }

    #[test]
    fn unhandled_action_still_records_dispatch() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        // ActionId::FrameRename has no handler today — Slice 15 is
        // incremental. The dispatch counter still bumps so the
        // routing path is observable.
        app.host.pending_host_intents.push(
            graphshell_core::shell_state::host_intent::HostIntent::Action {
                action_id: graphshell_core::actions::ActionId::FrameRename,
            },
        );
        app.tick_with_events(Vec::new());

        assert_eq!(app.host.runtime.dispatched_action_count, 1);
        assert_eq!(
            app.host.runtime.last_dispatched_action,
            Some(graphshell_core::actions::ActionId::FrameRename),
        );
    }

    #[test]
    fn palette_disabled_action_does_not_dispatch() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        let _ = app.update(Message::PaletteOpen {
            origin: PaletteOrigin::KeyboardShortcut,
        });

        // Synthesize a disabled row at index 0.
        app.command_palette.all_actions[0].is_available = false;
        app.command_palette.all_actions[0].disabled_reason = Some("test".into());

        let _ = app.update(Message::PaletteActionSelected(0));

        assert_eq!(
            app.host.runtime.dispatched_action_count, 0,
            "disabled selection must not dispatch",
        );
        assert!(app.host.runtime.last_dispatched_action.is_none());
        assert!(app.host.pending_host_intents.is_empty());
    }

    #[test]
    fn palette_action_select_toast_carries_canonical_key() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        let _ = app.update(Message::PaletteOpen {
            origin: PaletteOrigin::KeyboardShortcut,
        });

        let visible = visible_palette_actions(&app.command_palette);
        let row0_id = visible[0].action_id;
        let expected_key = row0_id.key();

        let _ = app.update(Message::PaletteActionSelected(0));

        assert_eq!(app.host.toast_queue.len(), 1);
        let msg = &app.host.toast_queue[0].message;
        assert!(
            msg.contains(expected_key),
            "toast should embed canonical ActionId::key() ({expected_key}); got: {msg}",
        );
    }

    #[test]
    fn palette_seeded_from_action_registry() {
        let runtime = GraphshellRuntime::for_testing();
        let app = IcedApp::with_runtime(runtime);

        // Every ActionId in the canonical registry becomes one RankedAction.
        let registry_count = graphshell_core::actions::all_action_ids().len();
        assert_eq!(
            app.command_palette.all_actions.len(),
            registry_count,
            "palette mirrors graphshell_core::actions::all_action_ids()",
        );
        assert!(
            app.command_palette
                .all_actions
                .iter()
                .any(|a| a.label == "Open Settings Pane"),
            "expected canonical ActionId::label(); got labels: {:?}",
            app.command_palette
                .all_actions
                .iter()
                .map(|a| a.label.as_str())
                .take(5)
                .collect::<Vec<_>>(),
        );
    }

    #[test]
    fn palette_query_filters_visible_actions() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let total = app.command_palette.all_actions.len();

        // No query → all actions visible.
        assert_eq!(visible_palette_actions(&app.command_palette).len(), total);

        // Substring match (case-insensitive).
        let _ = app.update(Message::PaletteQuery("settings".into()));
        let visible = visible_palette_actions(&app.command_palette);
        assert!(visible.iter().all(|a| a.label.to_lowercase().contains("settings")));
        assert!(!visible.is_empty(), "Settings is in the placeholder list");

        // Reset query → all visible again.
        let _ = app.update(Message::PaletteQuery(String::new()));
        assert_eq!(visible_palette_actions(&app.command_palette).len(), total);
    }

    #[test]
    fn palette_focus_down_advances_and_wraps() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        let _ = app.update(Message::PaletteOpen {
            origin: PaletteOrigin::KeyboardShortcut,
        });

        let total = visible_palette_actions(&app.command_palette).len();
        assert!(total > 1, "need ≥2 placeholder rows for wrap test");

        let _ = app.update(Message::PaletteFocusDown);
        assert_eq!(app.command_palette.focused_index, Some(0));

        for expected in 1..total {
            let _ = app.update(Message::PaletteFocusDown);
            assert_eq!(app.command_palette.focused_index, Some(expected));
        }

        // Wrap around.
        let _ = app.update(Message::PaletteFocusDown);
        assert_eq!(app.command_palette.focused_index, Some(0));
    }

    #[test]
    fn palette_focus_up_from_none_wraps_to_last() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        let _ = app.update(Message::PaletteOpen {
            origin: PaletteOrigin::KeyboardShortcut,
        });

        let total = visible_palette_actions(&app.command_palette).len();
        assert!(total > 0);

        let _ = app.update(Message::PaletteFocusUp);
        assert_eq!(app.command_palette.focused_index, Some(total - 1));
    }

    #[test]
    fn palette_submit_focused_fires_focused_action() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        let _ = app.update(Message::PaletteOpen {
            origin: PaletteOrigin::KeyboardShortcut,
        });

        // Focus the second row.
        let _ = app.update(Message::PaletteFocusDown);
        let _ = app.update(Message::PaletteFocusDown);
        assert_eq!(app.command_palette.focused_index, Some(1));

        // Resolve PaletteSubmitFocused → PaletteActionSelected(1).
        let _ = app.update(Message::PaletteSubmitFocused);
        let _ = app.update(Message::PaletteActionSelected(1));

        assert!(!app.command_palette.is_open, "selecting closes the palette");
        assert_eq!(app.host.toast_queue.len(), 1);
    }

    #[test]
    fn palette_disabled_action_select_is_noop() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        let _ = app.update(Message::PaletteOpen {
            origin: PaletteOrigin::KeyboardShortcut,
        });

        // Slice 9: every registry action seeds with `is_available =
        // true`. Synthesize a disabled row to exercise the no-op path
        // — runtime swap will drive availability via
        // ActionRegistryViewModel.
        app.command_palette.all_actions[0].is_available = false;
        app.command_palette.all_actions[0].disabled_reason =
            Some("synthetic disabled state".into());

        let _ = app.update(Message::PaletteActionSelected(0));

        assert!(
            app.command_palette.is_open,
            "disabled selection must not close the palette",
        );
        assert!(app.host.toast_queue.is_empty());
    }

    #[test]
    fn palette_query_reset_clears_focus() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);
        let _ = app.update(Message::PaletteOpen {
            origin: PaletteOrigin::KeyboardShortcut,
        });
        let _ = app.update(Message::PaletteFocusDown);
        assert_eq!(app.command_palette.focused_index, Some(0));

        let _ = app.update(Message::PaletteQuery("newquery".into()));
        assert!(
            app.command_palette.focused_index.is_none(),
            "query change must reset focus index — visible list shape changed",
        );
    }

    /// Seed the runtime with `count` nodes via the same OmnibarSubmit
    /// path the real UI uses, returning the URL strings so the test
    /// can assert on them.
    fn seed_test_nodes(app: &mut IcedApp, count: usize) -> Vec<String> {
        let mut urls = Vec::with_capacity(count);
        for i in 0..count {
            let url = format!("https://example{i}.test/");
            let _ = app.update(Message::OmnibarInput(url.clone()));
            let _ = app.update(Message::OmnibarSubmit);
            urls.push(url);
        }
        urls
    }

    #[test]
    fn finder_focus_down_advances_and_wraps() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        // Seed at least 2 nodes so wrap behaviour is observable.
        seed_test_nodes(&mut app, 3);

        let _ = app.update(Message::NodeFinderOpen {
            origin: NodeFinderOrigin::KeyboardShortcut,
        });

        let total = visible_finder_results(&app.node_finder).len();
        assert!(total > 1, "seeded ≥3 nodes; finder should reflect them");

        let _ = app.update(Message::NodeFinderFocusDown);
        assert_eq!(app.node_finder.focused_index, Some(0));

        for expected in 1..total {
            let _ = app.update(Message::NodeFinderFocusDown);
            assert_eq!(app.node_finder.focused_index, Some(expected));
        }

        let _ = app.update(Message::NodeFinderFocusDown);
        assert_eq!(app.node_finder.focused_index, Some(0), "wrap to first row");
    }

    #[test]
    fn finder_query_filters_by_title_or_address() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        seed_test_nodes(&mut app, 3); // example0 / example1 / example2

        let _ = app.update(Message::NodeFinderOpen {
            origin: NodeFinderOrigin::KeyboardShortcut,
        });
        let _ = app.update(Message::NodeFinderQuery("example1".into()));

        let visible = visible_finder_results(&app.node_finder);
        assert!(!visible.is_empty(), "exactly one URL contains 'example1'");
        assert!(
            visible.iter().all(|r| {
                r.title.to_lowercase().contains("example1")
                    || r.address.to_lowercase().contains("example1")
            }),
            "filtered set must satisfy the query",
        );
    }

    #[test]
    fn finder_result_selected_toasts_resolved_url() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let urls = seed_test_nodes(&mut app, 1);
        // OmnibarSubmit pushes its own success toast — drain so this
        // test only observes the finder's toast.
        app.host.toast_queue.clear();

        let _ = app.update(Message::NodeFinderOpen {
            origin: NodeFinderOrigin::KeyboardShortcut,
        });
        let _ = app.update(Message::NodeFinderResultSelected(0));

        assert!(!app.node_finder.is_open);
        assert_eq!(app.host.toast_queue.len(), 1);
        let msg = &app.host.toast_queue[0].message;
        assert!(
            msg.contains(&urls[0]),
            "toast should carry the resolved URL ({}); got: {msg}",
            urls[0],
        );
    }

    #[test]
    fn finder_seeded_from_runtime_graph_on_open() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        // Finder default is empty until opened — Slice 11 reads from
        // the live graph at open time rather than caching placeholders.
        assert!(app.node_finder.all_results.is_empty());

        seed_test_nodes(&mut app, 2);

        let _ = app.update(Message::NodeFinderOpen {
            origin: NodeFinderOrigin::KeyboardShortcut,
        });

        let nodes_in_graph = app.host.runtime.graph_app.domain_graph().nodes().count();
        assert_eq!(
            app.node_finder.all_results.len(),
            nodes_in_graph,
            "every node in the graph maps to one finder row",
        );
    }

    #[test]
    fn finder_selection_dispatches_open_node_intent() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        seed_test_nodes(&mut app, 1);

        let _ = app.update(Message::NodeFinderOpen {
            origin: NodeFinderOrigin::KeyboardShortcut,
        });

        // Capture the resolved NodeKey before selection.
        let row0_node_key = app.node_finder.all_results[0].node_key;
        assert_eq!(app.host.runtime.opened_node_count, 0);
        assert!(app.host.runtime.focused_node_hint.is_none());

        let _ = app.update(Message::NodeFinderResultSelected(0));

        assert!(
            app.host.pending_host_intents.is_empty(),
            "intent queue drained by post-select tick",
        );
        assert_eq!(
            app.host.runtime.opened_node_count, 1,
            "runtime observed exactly one HostIntent::OpenNode",
        );
        assert_eq!(
            app.host.runtime.focused_node_hint,
            Some(row0_node_key),
            "runtime promoted the resolved NodeKey to focused_node_hint",
        );
    }

    #[test]
    fn finder_out_of_range_selection_does_not_dispatch() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let _ = app.update(Message::NodeFinderOpen {
            origin: NodeFinderOrigin::KeyboardShortcut,
        });

        // Empty graph → empty result list → idx 0 is out of range.
        let _ = app.update(Message::NodeFinderResultSelected(0));

        assert_eq!(
            app.host.runtime.opened_node_count, 0,
            "out-of-range selection must not dispatch",
        );
        assert!(app.host.runtime.focused_node_hint.is_none());
    }

    #[test]
    fn omnibar_route_to_finder_seeds_real_results() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        seed_test_nodes(&mut app, 2);

        // Non-URL omnibar submit routes the query to the Node Finder
        // and populates results from the live graph.
        let _ = app.update(Message::OmnibarRouteToNodeFinder("ex".into()));

        assert!(app.node_finder.is_open);
        assert_eq!(app.node_finder.query, "ex");
        let visible = visible_finder_results(&app.node_finder);
        assert!(!visible.is_empty(), "seeded URLs match 'ex' substring");
    }

    // --- Context menu tests (Slice 8) ---

    #[test]
    fn context_menu_open_seeds_entries_and_anchor() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        // Cache a cursor position via the regular CursorMoved path so
        // the menu's anchor reads from the canonical source.
        let _ = app.update(Message::IcedEvent(iced::Event::Mouse(
            iced::mouse::Event::CursorMoved {
                position: iced::Point::new(120.0, 80.0),
            },
        )));

        // Need a Tile pane to test that target. Replace the seeded
        // Canvas pane via direct mutation since there's no public
        // "convert pane" message yet.
        if let Some((_, meta)) = app.frame.split_state.iter_mut().next() {
            meta.pane_type = PaneType::Tile;
        }

        let pane_id = app
            .frame
            .split_state
            .iter()
            .next()
            .map(|(_, m)| m.pane_id)
            .expect("pane present");

        let _ = app.update(Message::ContextMenuOpen {
            target: ContextMenuTarget::TilePane { pane_id, node_key: None },
        });

        assert!(app.context_menu.is_open);
        assert_eq!(app.context_menu.target, ContextMenuTarget::TilePane { pane_id, node_key: None });
        assert_eq!(app.context_menu.anchor, iced::Point::new(120.0, 80.0));
        assert!(
            app.context_menu
                .items
                .iter()
                .any(|i| i.entry.label == "Activate"),
            "TilePane menu should include Activate",
        );
        assert!(
            app.context_menu.items.iter().any(|i| i.entry.destructive),
            "TilePane menu should include a destructive Tombstone entry",
        );
    }

    #[test]
    fn context_menu_target_drives_entry_set() {
        // Distinct targets surface distinct entry sets.
        let canvas = items_for_target(ContextMenuTarget::CanvasPane { pane_id: PaneId(1), node_key: None });
        let tile = items_for_target(ContextMenuTarget::TilePane { pane_id: PaneId(1), node_key: None });
        let base = items_for_target(ContextMenuTarget::BaseLayer);

        assert!(canvas.iter().any(|i| i.entry.label == "Inspect"));
        assert!(!tile.iter().any(|i| i.entry.label == "Inspect"));
        assert!(base.iter().any(|i| i.entry.label == "Open Pane"));
    }

    #[test]
    fn context_menu_open_dismisses_modals() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let _ = app.update(Message::PaletteOpen {
            origin: PaletteOrigin::KeyboardShortcut,
        });
        let _ = app.update(Message::ContextMenuOpen {
            target: ContextMenuTarget::BaseLayer,
        });

        assert!(!app.command_palette.is_open, "context menu closes palette");
        assert!(app.context_menu.is_open);
    }

    #[test]
    fn context_menu_entry_selected_acks_and_closes() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let _ = app.update(Message::ContextMenuOpen {
            target: ContextMenuTarget::BaseLayer,
        });
        // BaseLayer entry 0 is "Open Pane" (enabled, intent = None — stub-only).
        let _ = app.update(Message::ContextMenuEntrySelected(0));

        assert!(!app.context_menu.is_open);
        assert!(app.context_menu.items.is_empty(), "items cleared");
        assert_eq!(app.host.toast_queue.len(), 1);
        let msg = &app.host.toast_queue[0].message;
        assert!(msg.contains("Open Pane"), "got: {msg}");
        assert!(
            msg.contains("(stub)"),
            "BaseLayer 'Open Pane' has no intent yet — toast should mark it stub",
        );
    }

    #[test]
    fn context_menu_disabled_entry_select_is_noop() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let _ = app.update(Message::ContextMenuOpen {
            target: ContextMenuTarget::BaseLayer,
        });
        // BaseLayer entry 1 is "Switch graphlet" (disabled — no graphlets).
        let disabled_idx = app
            .context_menu
            .items
            .iter()
            .position(|i| i.entry.disabled_reason.is_some())
            .expect("BaseLayer has a disabled entry");

        let _ = app.update(Message::ContextMenuEntrySelected(disabled_idx));

        assert!(
            app.context_menu.is_open,
            "disabled select must not close the menu",
        );
        assert!(app.host.toast_queue.is_empty());
    }

    #[test]
    fn context_menu_dismiss_closes_without_acting() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let _ = app.update(Message::ContextMenuOpen {
            target: ContextMenuTarget::BaseLayer,
        });
        let _ = app.update(Message::ContextMenuDismiss);

        assert!(!app.context_menu.is_open);
        assert!(app.host.toast_queue.is_empty());
    }

    #[test]
    fn escape_closes_context_menu_first() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        // Both context menu and palette could be open simultaneously
        // even though the state-level wiring should prevent it; verify
        // Escape's precedence in the resolution order regardless.
        let _ = app.update(Message::PaletteOpen {
            origin: PaletteOrigin::KeyboardShortcut,
        });
        // Force-open context menu over the palette by direct dispatch
        // (skips the state-level mutual-exclusion path).
        let _ = app.update(Message::ContextMenuOpen {
            target: ContextMenuTarget::BaseLayer,
        });
        assert!(app.context_menu.is_open);
        assert!(!app.command_palette.is_open, "ContextMenuOpen closed palette");

        // Now palette is already closed, so Escape should close the
        // context menu.
        let _ = app.update(Message::ContextMenuDismiss);
        assert!(!app.context_menu.is_open);
    }

    #[test]
    fn context_menu_action_entry_dispatches_host_intent() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        // Convert the seeded Canvas pane to a Tile pane so the
        // wired-action entries are available.
        if let Some((_, meta)) = app.frame.split_state.iter_mut().next() {
            meta.pane_type = PaneType::Tile;
        }
        let pane_id = app
            .frame
            .split_state
            .iter()
            .next()
            .map(|(_, m)| m.pane_id)
            .expect("pane present");

        let _ = app.update(Message::ContextMenuOpen {
            target: ContextMenuTarget::TilePane { pane_id, node_key: None },
        });
        // Find the "Pin" entry — it's wired to ActionId::NodePinToggle.
        let pin_idx = app
            .context_menu
            .items
            .iter()
            .position(|i| i.entry.label == "Pin")
            .expect("TilePane menu carries a Pin entry");
        assert_eq!(app.host.runtime.dispatched_action_count, 0);

        let _ = app.update(Message::ContextMenuEntrySelected(pin_idx));

        assert!(!app.context_menu.is_open);
        assert!(
            app.host.pending_host_intents.is_empty(),
            "post-select tick drained the intent",
        );
        assert_eq!(
            app.host.runtime.dispatched_action_count, 1,
            "runtime observed exactly one HostIntent::Action",
        );
        assert_eq!(
            app.host.runtime.last_dispatched_action,
            Some(graphshell_core::actions::ActionId::NodePinToggle),
            "context-menu selection routed the wired ActionId",
        );
        // Toast should NOT carry the (stub) suffix since dispatch closed.
        let msg = &app.host.toast_queue[0].message;
        assert!(msg.contains("Pin") && !msg.contains("(stub)"), "got: {msg}");
    }

    #[test]
    fn context_menu_destructive_entry_routes_through_confirm_dialog() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        if let Some((_, meta)) = app.frame.split_state.iter_mut().next() {
            meta.pane_type = PaneType::Tile;
        }
        let pane_id = app
            .frame
            .split_state
            .iter()
            .next()
            .map(|(_, m)| m.pane_id)
            .expect("pane present");

        let _ = app.update(Message::ContextMenuOpen {
            target: ContextMenuTarget::TilePane { pane_id, node_key: None },
        });

        let tombstone_idx = app
            .context_menu
            .items
            .iter()
            .position(|i| i.entry.destructive)
            .expect("TilePane menu carries a destructive Tombstone entry");

        // Slice 14: destructive selection parks the intent in the
        // confirm dialog instead of dispatching immediately.
        let _ = app.update(Message::ContextMenuEntrySelected(tombstone_idx));

        assert!(
            app.confirm_dialog.is_open,
            "destructive selection opens the confirm dialog gate",
        );
        assert!(app.confirm_dialog.pending_intent.is_some());
        assert!(
            !app.context_menu.is_open,
            "context menu closed when the dialog opened",
        );
        assert_eq!(
            app.host.runtime.dispatched_action_count, 0,
            "no dispatch yet — awaiting confirmation",
        );

        // User confirms.
        let _ = app.update(Message::ConfirmDialogConfirm);

        assert!(!app.confirm_dialog.is_open);
        assert!(app.confirm_dialog.pending_intent.is_none());
        assert_eq!(
            app.host.runtime.last_dispatched_action,
            Some(graphshell_core::actions::ActionId::NodeMarkTombstone),
            "confirm dispatched the parked intent",
        );
    }

    #[test]
    fn confirm_dialog_cancel_drops_pending_intent() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        if let Some((_, meta)) = app.frame.split_state.iter_mut().next() {
            meta.pane_type = PaneType::Tile;
        }
        let pane_id = app
            .frame
            .split_state
            .iter()
            .next()
            .map(|(_, m)| m.pane_id)
            .unwrap();

        let _ = app.update(Message::ContextMenuOpen {
            target: ContextMenuTarget::TilePane { pane_id, node_key: None },
        });
        let tombstone_idx = app
            .context_menu
            .items
            .iter()
            .position(|i| i.entry.destructive)
            .unwrap();
        let _ = app.update(Message::ContextMenuEntrySelected(tombstone_idx));
        assert!(app.confirm_dialog.is_open);

        let _ = app.update(Message::ConfirmDialogCancel);

        assert!(!app.confirm_dialog.is_open);
        assert!(app.confirm_dialog.pending_intent.is_none());
        assert_eq!(
            app.host.runtime.dispatched_action_count, 0,
            "cancel must drop the parked intent without dispatching",
        );
    }

    #[test]
    fn confirm_dialog_escape_cancels_first() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        // Open both context menu and (hypothetically) confirm dialog
        // — but since destructive selection closes the menu before
        // opening the dialog, the natural state is just the dialog.
        if let Some((_, meta)) = app.frame.split_state.iter_mut().next() {
            meta.pane_type = PaneType::Tile;
        }
        let pane_id = app
            .frame
            .split_state
            .iter()
            .next()
            .map(|(_, m)| m.pane_id)
            .unwrap();
        let _ = app.update(Message::ContextMenuOpen {
            target: ContextMenuTarget::TilePane { pane_id, node_key: None },
        });
        let tombstone_idx = app
            .context_menu
            .items
            .iter()
            .position(|i| i.entry.destructive)
            .unwrap();
        let _ = app.update(Message::ContextMenuEntrySelected(tombstone_idx));
        assert!(app.confirm_dialog.is_open);

        let _ = app.update(Message::ConfirmDialogCancel);

        assert!(!app.confirm_dialog.is_open);
    }

    #[test]
    fn non_destructive_action_skips_confirm_dialog() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        if let Some((_, meta)) = app.frame.split_state.iter_mut().next() {
            meta.pane_type = PaneType::Tile;
        }
        let pane_id = app
            .frame
            .split_state
            .iter()
            .next()
            .map(|(_, m)| m.pane_id)
            .unwrap();

        let _ = app.update(Message::ContextMenuOpen {
            target: ContextMenuTarget::TilePane { pane_id, node_key: None },
        });
        let pin_idx = app
            .context_menu
            .items
            .iter()
            .position(|i| i.entry.label == "Pin")
            .unwrap();

        let _ = app.update(Message::ContextMenuEntrySelected(pin_idx));

        assert!(
            !app.confirm_dialog.is_open,
            "non-destructive entries do not open the confirm dialog",
        );
        assert_eq!(
            app.host.runtime.dispatched_action_count, 1,
            "non-destructive entries dispatch immediately",
        );
    }

    #[test]
    fn view_renders_with_confirm_dialog_open() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        if let Some((_, meta)) = app.frame.split_state.iter_mut().next() {
            meta.pane_type = PaneType::Tile;
        }
        let pane_id = app
            .frame
            .split_state
            .iter()
            .next()
            .map(|(_, m)| m.pane_id)
            .unwrap();
        let _ = app.update(Message::ContextMenuOpen {
            target: ContextMenuTarget::TilePane { pane_id, node_key: None },
        });
        let tombstone_idx = app
            .context_menu
            .items
            .iter()
            .position(|i| i.entry.destructive)
            .unwrap();
        let _ = app.update(Message::ContextMenuEntrySelected(tombstone_idx));
        let _ = app.update(Message::Tick);

        let _element = app.view();
    }

    #[test]
    fn context_menu_stub_entry_does_not_dispatch() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let _ = app.update(Message::ContextMenuOpen {
            target: ContextMenuTarget::BaseLayer,
        });
        // BaseLayer "Open Pane" is intent=None (stub-only).
        let _ = app.update(Message::ContextMenuEntrySelected(0));

        assert_eq!(
            app.host.runtime.dispatched_action_count, 0,
            "stub entries must not dispatch",
        );
        assert!(app.host.pending_host_intents.is_empty());
    }

    #[test]
    fn view_renders_with_context_menu_open() {
        let runtime = GraphshellRuntime::for_testing();
        let mut app = IcedApp::with_runtime(runtime);

        let _ = app.update(Message::ContextMenuOpen {
            target: ContextMenuTarget::BaseLayer,
        });
        let _ = app.update(Message::Tick);

        // Render-time smoke test — the gs::ContextMenu overlay must
        // not panic when stacked on top of the body.
        let _element = app.view();
    }
}
