//! State types and data-shape builders extracted from
//! `iced_app/mod.rs` — Phase A of the post-Slice-39 decomposition.
//! Holds: every host-side state struct (FrameState, OmnibarSession,
//! palette / finder / context_menu / confirm / node_create /
//! frame_rename modal states, NavigatorState), their `Default` /
//! constructor `impl`s, the per-modal stable widget ids, the
//! `RankedAction` / `NodeFinderResult` data shapes, and the data
//! builders (registry_actions / build_finder_results /
//! visible_*_actions / items_for_target). IcedApp + Message +
//! update / view / handlers stay in mod.rs.

use std::sync::atomic::{AtomicU64, Ordering};

use euclid::default::Vector2D;
use graph_canvas::camera::CanvasCamera;
use iced::widget::pane_grid;
use iced::Point;

use graphshell_iced_widgets::ContextMenuEntry;

/// Stable widget id for the omnibar text input. Addressed by the
/// `OmnibarFocus` handler via `iced::widget::operation::focus`. Any
/// future iced widget that wants programmatic focus gets a similar
/// named id so the id is portable across `view` rebuilds.
pub(super) const OMNIBAR_INPUT_ID: &str = "graphshell:omnibar_input";

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
pub(crate) struct PaneId(pub(super) u64);

impl PaneId {
    pub(super) fn next() -> Self {
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
    /// Per-pane camera state — Slice 35. iced's `Program::State`
    /// already gives a per-widget camera within a stable widget
    /// tree, but Frame switches (Slice 31) rebuild the tree, losing
    /// the iced-side state. This cache persists each pane's camera
    /// at the most recent `Message::CameraChanged` so a future
    /// canvas-program-initialization slice can seed `Program::State`
    /// from here on remount. Today the cache is observable + drained
    /// by the tests; live restoration awaits the program-side hook.
    pub pane_cameras: std::collections::HashMap<PaneId, CanvasCamera>,
    /// `true` while a pane drag is in progress (between Picked and
    /// Dropped/Canceled). Slice 36 — drives the drop-zone hint
    /// banner; pane_grid handles the actual drop logic.
    pub drag_in_progress: bool,
}

impl FrameState {
    /// Initialize with one Canvas pane — the default launch state.
    /// An empty Frame (zero Panes) would show only the canvas base
    /// layer; pre-seeding with one Canvas pane makes the pane_grid
    /// visible immediately for Slice 3 verification.
    pub(super) fn new() -> Self {
        let first = PaneMeta {
            pane_id: PaneId::next(),
            pane_type: PaneType::Canvas,
        };
        let (split_state, _handle) = pane_grid::State::new(first);
        Self {
            split_state,
            base_layer_active: false,
            focused_pane: None,
            pane_cameras: std::collections::HashMap::new(),
            drag_in_progress: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Swatches — Slice 33 (Navigator's third Presentation Bucket)
// ---------------------------------------------------------------------------

/// One projection recipe surfaced as a swatch card. Per
/// [`iced_composition_skeleton_spec.md` §6.2](
/// ../../../design_docs/graphshell_docs/implementation_strategy/shell/iced_composition_skeleton_spec.md):
/// each swatch is a live canvas instance running one recipe at the
/// `Swatch` render profile (compact, low-fidelity).
///
/// Slice 33 ships three placeholder recipes — `FullGraph`,
/// `RecentlyActive`, `FocusedNeighborhood` — all currently rendering
/// the same scene against the runtime's graph_app. Real per-recipe
/// scoping (filtered scene input, lens overrides) lands when the
/// projection-recipe authority surfaces a `recipe.derive_scene_input`
/// method; the rendering pipeline below doesn't change shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum SwatchRecipe {
    FullGraph,
    RecentlyActive,
    FocusedNeighborhood,
}

impl SwatchRecipe {
    pub(super) fn label(self) -> &'static str {
        match self {
            SwatchRecipe::FullGraph => "Full graph",
            SwatchRecipe::RecentlyActive => "Recently active",
            SwatchRecipe::FocusedNeighborhood => "Focused neighborhood",
        }
    }

    /// Built-in recipe set for Slice 33. When user-defined recipes
    /// land, the order comes from a runtime view-model field instead.
    pub(super) fn builtin_set() -> &'static [SwatchRecipe] {
        &[
            SwatchRecipe::FullGraph,
            SwatchRecipe::RecentlyActive,
            SwatchRecipe::FocusedNeighborhood,
        ]
    }
}

// ---------------------------------------------------------------------------
// Frame composition — Slice 31 (multi-Frame switcher)
// ---------------------------------------------------------------------------

/// Stable application-level identity for a Frame. Distinct from
/// `FrameState` (the per-Frame split-tree authority) — `FrameId` is
/// the durable handle by which the Frame switcher routes
/// activation messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct FrameId(u64);

impl FrameId {
    pub(super) fn next() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        Self(NEXT.fetch_add(1, Ordering::Relaxed))
    }
}

/// One backgrounded Frame held in `inactive_frames`. The active
/// Frame's state lives in `IcedApp::frame` (the existing field
/// from Slice 3); switching swaps the contents via `std::mem::swap`
/// so `app.frame.*` continues to address the active Frame's pane
/// grid without test churn.
pub(crate) struct NamedFrame {
    pub(crate) id: FrameId,
    pub(crate) label: String,
    pub(crate) state: FrameState,
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
pub(super) const PALETTE_INPUT_ID: &str = "graphshell:command_palette_input";

/// How many UX events the Activity Log bucket retains. Bounded so
/// long-lived sessions don't accumulate unbounded history. The
/// bucket renders the most-recent end of the buffer.
pub(super) const ACTIVITY_LOG_CAPACITY: usize = 100;

/// `UxObserver` adapter that forwards into the host's
/// `Arc<RecordingObserver>` so the Activity Log bucket can snapshot
/// the recorder without holding a mutable borrow on the registry.
pub(super) struct ActivityLogRecorderProxy(pub(super) 
    std::sync::Arc<graphshell_core::ux_observability::RecordingObserver>,
);

impl graphshell_core::ux_observability::UxObserver for ActivityLogRecorderProxy {
    fn observe(&self, event: &graphshell_core::ux_observability::UxEvent) {
        self.0.observe(event);
    }
}

/// Stable widget id for the node finder text input.
pub(super) const NODE_FINDER_INPUT_ID: &str = "graphshell:node_finder_input";

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
pub(super) fn registry_actions() -> Vec<RankedAction> {
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
pub(super) fn visible_palette_actions(state: &CommandPaletteState) -> Vec<&RankedAction> {
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
pub(super) fn build_finder_results(
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
pub(super) fn visible_finder_results(state: &NodeFinderState) -> Vec<&NodeFinderResult> {
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
// NodeCreate Modal — Slice 32 (URL input for NodeNew)
// ---------------------------------------------------------------------------

/// Stable widget id for the NodeCreate text input. Slice 32.
pub(super) const NODE_CREATE_INPUT_ID: &str = "graphshell:node_create_input";

/// Widget-local state for the URL-prompt modal. Opened by the
/// `NodeNew` / `NodeNewAsTab` actions; submission creates a node at
/// the entered URL via `HostIntent::CreateNodeAtUrl` and closes the
/// modal. Cancel / click-outside / Escape drops the draft.
#[derive(Debug, Clone, Default)]
pub(crate) struct NodeCreateState {
    pub(crate) is_open: bool,
    pub(crate) url_draft: String,
}

// ---------------------------------------------------------------------------
// FrameRename Modal — Slice 34
// ---------------------------------------------------------------------------

/// Stable widget id for the FrameRename text input.
pub(super) const FRAME_RENAME_INPUT_ID: &str = "graphshell:frame_rename_input";

/// Widget-local state for the Frame rename modal. Triggered by the
/// `FrameRename` action (host-routed from Slice 31's Frame
/// composition layer); applies the new label to the active Frame
/// on submit. Cancel / click-outside / Escape drops the draft.
#[derive(Debug, Clone, Default)]
pub(crate) struct FrameRenameState {
    pub(crate) is_open: bool,
    pub(crate) label_draft: String,
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
    pub(super) fn node_key(self) -> Option<graphshell_core::graph::NodeKey> {
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
    pub(super) fn stub(entry: ContextMenuEntry) -> Self {
        Self { entry, intent: None }
    }

    /// Build an action item. If `target_node` is `Some`, the dispatch
    /// routes via `HostIntent::ActionOnNode` so the runtime
    /// pre-positions focus before running the per-action handler;
    /// otherwise it routes via `HostIntent::Action` and the handler
    /// operates on whatever the runtime considers focused.
    pub(super) fn action(
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
pub(super) fn items_for_target(target: ContextMenuTarget) -> Vec<ContextMenuItem> {
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
