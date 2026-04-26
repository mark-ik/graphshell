/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use crate::app::{GraphBrowserApp, GraphViewId, ToolSurfaceReturnTarget};
use crate::graph::NodeKey;
use crate::shell::desktop::lifecycle::webview_backpressure::WebviewCreationBackpressureState;
use crate::shell::desktop::runtime::control_panel::ControlPanel;
use crate::shell::desktop::runtime::registries::RegistryRuntime;
#[cfg(any(test, feature = "iced-host"))]
use crate::shell::desktop::runtime::registry_signal_router::RegistrySignalRouter;
use crate::shell::desktop::ui::command_palette_state::CommandPaletteSession;
use crate::shell::desktop::ui::finalize_actions::drain_pending_runtime_finalize_actions;
use crate::shell::desktop::ui::gui::frame_inbox::GuiFrameInbox;
#[cfg(any(test, feature = "iced-host"))]
use crate::shell::desktop::ui::gui::frame_inbox::spawn_gui_frame_inbox;
use crate::shell::desktop::ui::omnibar_state::{
    OmnibarSearchSession, OmnibarSessionKind, ProviderSuggestionError, ProviderSuggestionStatus,
};
use crate::shell::desktop::workbench::compositor_adapter::ViewerSurfaceRegistry;
use crate::shell::desktop::workbench::pane_model::PaneId;
use egui_file_dialog::{DialogState, FileDialog as EguiFileDialog, Filter};
use graphshell_core::async_host::AsyncSpawner;
use graphshell_core::content::{ContentLoadState, ViewerInstanceId};
use graphshell_core::signal_router::SignalRouter;
use graphshell_core::viewer_host::ViewerSurfaceHost;
use graphshell_runtime::frame_projection::{
    GraphRuntimeLayoutProjectionInput, project_graph_runtime_layout_view_model,
};
use graphshell_runtime::ports::RuntimeTickPorts;
use graphshell_runtime::{
    AccessibilityProjectionInput, CommandPaletteProjectionInput, CommandPaletteScopeView,
    DialogsProjectionInput, FocusProjectionInput, FocusRingSettingsView, FrameHostInput,
    FrameViewModel, GraphSearchProjectionInput, OmnibarProjectionInput, OmnibarProviderStatusView,
    OmnibarSessionKindView, SettingsProjectionInput, ThumbnailAspectView, ThumbnailFilterView,
    ThumbnailFormatView, ThumbnailSettingsView, ToolbarProjectionInput,
    TransientFrameOutputsProjectionInput, project_accessibility_view_model,
    project_command_palette_view_model, project_dialogs_view_model, project_focus_view_model,
    project_graph_search_view_model, project_omnibar_view_model, project_settings_view_model,
    project_toolbar_view_model, project_transient_frame_outputs,
};
#[cfg(any(test, feature = "iced-host"))]
use verso_host::{NoopViewerSurfaceHost, TokioAsyncSpawner};

// Toolbar session types moved to `graphshell_core::shell_state::toolbar`
// in M4 slice 2 (2026-04-22). Re-exported here so existing call sites
// (`ToolbarState { … }`, `ToolbarEditable { … }`, `ToolbarDraft` type
// refs) resolve unchanged. Cold-startup constructions should prefer
// `ToolbarState::with_initial_location(...)` over field-by-field
// construction — reduces the chance that a new `ToolbarState` field
// gets default-zeroed at a construction site and forgotten.
pub(crate) use graphshell_core::shell_state::toolbar::{
    ToolbarDraft, ToolbarEditable, ToolbarState,
};

pub(super) enum BookmarkImportDialogEvent {
    Continue,
    Picked(PathBuf),
    Cancelled,
}

pub(crate) struct BookmarkImportDialogState {
    dialog: EguiFileDialog,
}

impl BookmarkImportDialogState {
    pub(super) fn new() -> Self {
        let bookmark_file_filter = Filter::new(|path: &std::path::Path| {
            path.extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| {
                    matches!(ext.to_ascii_lowercase().as_str(), "html" | "htm" | "json")
                })
        });

        let dialog = EguiFileDialog::new()
            .add_file_filter("Bookmark Files", bookmark_file_filter)
            .default_file_filter("Bookmark Files");

        Self { dialog }
    }

    pub(super) fn update(&mut self, ctx: &egui::Context) -> BookmarkImportDialogEvent {
        if *self.dialog.state() == DialogState::Closed {
            self.dialog.pick_file();
        }

        match self.dialog.update(ctx).state() {
            DialogState::Open => BookmarkImportDialogEvent::Continue,
            DialogState::Picked(path) => BookmarkImportDialogEvent::Picked(path.clone()),
            DialogState::PickedMultiple(paths) => paths
                .first()
                .cloned()
                .map(BookmarkImportDialogEvent::Picked)
                .unwrap_or(BookmarkImportDialogEvent::Cancelled),
            DialogState::Cancelled | DialogState::Closed => BookmarkImportDialogEvent::Cancelled,
        }
    }
}

pub(super) fn toolbar_location_input_id(active_toolbar_pane: Option<PaneId>) -> egui::Id {
    egui::Id::new((
        "location_input",
        active_toolbar_pane.map(|pane_id| pane_id.to_string()),
    ))
}

#[derive(Clone, Default)]
pub(crate) struct RuntimeFocusAuthorityState {
    pub(super) pane_activation: Option<PaneId>,
    pub(super) last_non_graph_pane_activation: Option<PaneId>,
    pub(super) semantic_region: Option<SemanticRegionFocus>,
    pub(super) local_widget_focus: Option<LocalFocusTarget>,
    pub(super) embedded_content_focus: Option<EmbeddedContentTarget>,
    pub(super) tool_surface_return_target: Option<ToolSurfaceReturnTarget>,
    pub(super) command_surface_return_target: Option<ToolSurfaceReturnTarget>,
    pub(super) transient_surface_return_target: Option<ToolSurfaceReturnTarget>,
    pub(crate) capture_stack: Vec<FocusCaptureEntry>,
    pub(crate) realized_focus_state: Option<RuntimeFocusState>,
}

// `FocusAuthorityMut` moved to
// `graphshell_core::shell_state::authorities` in M4 slice 10
// (2026-04-22). Re-exported at the original path so existing
// `crate::shell::desktop::ui::gui_state::FocusAuthorityMut`
// imports resolve unchanged. `latch_ring` now takes a `now:
// PortableInstant` parameter (the portable version doesn't reach
// for `portable_now()` internally); callers supply it via
// `shell::desktop::ui::portable_time::portable_now()`.
pub(crate) use graphshell_core::shell_state::authorities::FocusAuthorityMut;

/// Host-facing mutation handle for toolbar / omnibar session state.
///
/// Per the M4 runtime extraction (§3.3 Toolbar / omnibar session state),
/// toolbar and omnibar state live on `GraphshellRuntime` and are host-
/// neutral. The widget receives this bundle in place of the five
/// individual `&mut` fields (`location`, `location_dirty`,
/// `location_submitted`, `show_clear_data_confirm`, `omnibar_search_session`)
/// that the `Input` surface used to carry, and calls named methods
/// (`set_location`, `mark_dirty`, `clear_omnibar_session`, …) instead of
/// poking raw fields.
///
/// Follows the `FocusAuthorityMut` pattern: callers destructure
/// `GraphshellRuntime` at the host boundary, assemble the bundle, and
/// pass it down. Fields are `pub(crate)` so deep call stacks that still
/// expect raw refs (egui TextEdit callbacks wanting `&mut String`) can
/// reach them without forcing a method signature rewrite.
pub(crate) struct ToolbarAuthorityMut<'a> {
    pub(crate) editable: &'a mut ToolbarEditable,
    pub(crate) show_clear_data_confirm: &'a mut bool,
    pub(crate) omnibar_search_session: &'a mut Option<OmnibarSearchSession>,
    /// Non-portable companion to `omnibar_search_session.provider_mailbox`:
    /// the host-side crossbeam receiver + generation tag that
    /// `drive_provider_suggestion_bridge` drains into the portable
    /// `AsyncRequestState`. Bundled alongside the session because the
    /// two are always mutated together in the toolbar frame.
    pub(crate) omnibar_provider_suggestion_driver: &'a mut Option<
        crate::shell::desktop::ui::toolbar::toolbar_provider_driver::ProviderSuggestionDriver,
    >,
}

impl<'a> ToolbarAuthorityMut<'a> {
    /// Reborrow the bundle with a shorter lifetime so it can be threaded
    /// through sub-functions without moving out of the outer bundle.
    pub(crate) fn reborrow(&mut self) -> ToolbarAuthorityMut<'_> {
        ToolbarAuthorityMut {
            editable: &mut *self.editable,
            show_clear_data_confirm: &mut *self.show_clear_data_confirm,
            omnibar_search_session: &mut *self.omnibar_search_session,
            omnibar_provider_suggestion_driver: &mut *self.omnibar_provider_suggestion_driver,
        }
    }

    pub(crate) fn location(&self) -> &str {
        &self.editable.location
    }

    pub(crate) fn location_mut(&mut self) -> &mut String {
        &mut self.editable.location
    }

    pub(crate) fn set_location(&mut self, value: impl Into<String>) {
        self.editable.location = value.into();
    }

    pub(crate) fn is_dirty(&self) -> bool {
        self.editable.location_dirty
    }

    pub(crate) fn set_dirty(&mut self, value: bool) {
        self.editable.location_dirty = value;
    }

    pub(crate) fn mark_dirty(&mut self) {
        self.editable.location_dirty = true;
    }

    pub(crate) fn clear_dirty(&mut self) {
        self.editable.location_dirty = false;
    }

    pub(crate) fn submitted(&self) -> bool {
        self.editable.location_submitted
    }

    pub(crate) fn set_submitted(&mut self, value: bool) {
        self.editable.location_submitted = value;
    }

    pub(crate) fn mark_submitted(&mut self) {
        self.editable.location_submitted = true;
    }

    pub(crate) fn clear_submitted(&mut self) {
        self.editable.location_submitted = false;
    }

    pub(crate) fn clear_data_confirm_armed(&self) -> bool {
        *self.show_clear_data_confirm
    }

    pub(crate) fn set_clear_data_confirm(&mut self, value: bool) {
        *self.show_clear_data_confirm = value;
    }

    pub(crate) fn omnibar_session(&self) -> Option<&OmnibarSearchSession> {
        self.omnibar_search_session.as_ref()
    }

    pub(crate) fn omnibar_session_mut(&mut self) -> Option<&mut OmnibarSearchSession> {
        self.omnibar_search_session.as_mut()
    }

    pub(crate) fn set_omnibar_session(&mut self, session: Option<OmnibarSearchSession>) {
        *self.omnibar_search_session = session;
    }

    pub(crate) fn clear_omnibar_session(&mut self) -> Option<OmnibarSearchSession> {
        self.omnibar_search_session.take()
    }
}

// `GraphSearchAuthorityMut` moved to `graphshell_core::shell_state::authorities`
// in M4 slice 9 (2026-04-22). The compatibility re-export at this path was
// removed in the 2026-04-23 Lane B' phase-args collapse — all consumers now
// import from the kernel directly.

// `CommandAuthorityMut` moved to
// `graphshell_core::shell_state::authorities` in M4 slice 9 (2026-04-22).
// Re-exported at this path so existing call sites resolve unchanged.
pub(crate) use graphshell_core::shell_state::authorities::CommandAuthorityMut;

/// Host-neutral runtime state for the Graphshell shell.
///
/// Per the M3.5 runtime boundary design
/// (`design_docs/graphshell_docs/implementation_strategy/shell/2026-04-16_runtime_boundary_design.md`),
/// this owns all Category A (durable runtime) fields that survive a host
/// migration from egui to iced. The host adapter (`EguiHost` today, a future
/// `IcedHost` eventually) holds only Category B/C/D fields.
pub(crate) struct GraphshellRuntime {
    // --- Core model & services ---
    /// Graph browser application state (graph, selection, intents).
    pub(crate) graph_app: GraphBrowserApp,

    /// Workbench membership + layout authority.
    pub(crate) graph_tree: graph_tree::GraphTree<NodeKey>,

    /// Stable UUID identifying this workbench's `GraphTree` slot in persistence.
    pub(crate) workbench_view_id: GraphViewId,

    /// Toolbar session state (location text, load status, nav capability).
    pub(crate) toolbar_state: ToolbarState,

    /// Graphshell-owned bookmark import file dialog state.
    pub(crate) bookmark_import_dialog: Option<BookmarkImportDialogState>,

    /// Async worker supervision and intent queue.
    pub(crate) control_panel: ControlPanel,

    /// Host-provided async task spawner.
    pub(crate) async_spawner: Arc<dyn AsyncSpawner>,

    /// Registry runtime for semantic services.
    pub(crate) registry_runtime: Arc<RegistryRuntime>,

    /// Host-provided signal router for frame-bound inbox subscriptions.
    pub(crate) signal_router: Arc<dyn SignalRouter>,

    /// Tokio runtime for async background workers.
    pub(crate) tokio_runtime: tokio::runtime::Runtime,

    /// Phase D unified viewer surface registry keyed by NodeKey. Single
    /// authority for per-node content surface state.
    pub(crate) viewer_surfaces: ViewerSurfaceRegistry,

    /// Host-owned viewer surface allocator/retirer.
    pub(crate) viewer_surface_host: Box<dyn ViewerSurfaceHost<ViewerSurfaceRegistry>>,

    /// Runtime backpressure state for tile-driven viewer creation retries.
    pub(crate) webview_creation_backpressure: HashMap<NodeKey, WebviewCreationBackpressureState>,

    /// Runtime viewers with an in-flight thumbnail capture request. This
    /// is pure runtime tracking (per M3.5 design §3.5, "thumbnail
    /// request tracking → Runtime") — the paired `tx`/`rx` channels
    /// stay on the host adapter until the render-backend boundary is
    /// formalized, but the set of pending WebViewIds lives here so
    /// iced will inherit it for free.
    pub(crate) thumbnail_capture_in_flight: std::collections::HashSet<ViewerInstanceId>,

    /// Typed frame-bound relay set for Shell-facing async signal bridges.
    pub(crate) frame_inbox: GuiFrameInbox,

    // --- Session state (formerly GuiRuntimeState) ---
    pub(crate) graph_search_open: bool,
    pub(crate) graph_search_query: String,
    pub(crate) graph_search_filter_mode: bool,
    pub(crate) graph_search_matches: Vec<NodeKey>,
    pub(crate) graph_search_active_match_index: Option<usize>,
    pub(crate) focused_node_hint: Option<NodeKey>,
    pub(crate) graph_surface_focused: bool,
    pub(crate) focus_ring_node_key: Option<NodeKey>,
    pub(crate) focus_ring_started_at: Option<graphshell_core::time::PortableInstant>,
    pub(crate) omnibar_search_session: Option<OmnibarSearchSession>,
    /// Host-side driver (crossbeam receiver + generation tag) for the
    /// omnibar provider-suggestion async request. Non-portable
    /// companion to `omnibar_search_session.provider_mailbox.result`
    /// (which is `AsyncRequestState<ProviderSuggestionFetchOutcome>`).
    /// The toolbar frame bridges this into the portable mailbox state
    /// at the top of each render. Introduced M4 slice 5 (2026-04-22).
    pub(crate) omnibar_provider_suggestion_driver: Option<
        crate::shell::desktop::ui::toolbar::toolbar_provider_driver::ProviderSuggestionDriver,
    >,
    pub(crate) focus_authority: RuntimeFocusAuthorityState,
    pub(crate) toolbar_drafts: HashMap<PaneId, ToolbarDraft>,
    pub(crate) command_palette_toggle_requested: bool,
    /// Command-palette session state (search query, scope filter,
    /// selection cursor, focus-on-open flag). Previously stashed in
    /// `egui::Context::data_mut(...)` persistent storage inside
    /// `render::command_palette`; moved here in M4 session 3 so it
    /// survives host migration.
    pub(crate) command_palette_session: CommandPaletteSession,
    pub(crate) pending_webview_context_surface_requests: Vec<PendingWebviewContextSurfaceRequest>,
    /// Two-step "clear graph and saved data" confirm deadline (unix
    /// seconds). `None` when not armed. Previously lived inside egui's
    /// `ctx::data_mut` temp storage; moved onto runtime state in M6
    /// §4.2 so iced can read/write it the same way.
    pub(crate) clear_data_confirm_deadline_secs: Option<f64>,
    /// Command-surface telemetry sink (latest published snapshot +
    /// event-sequence counters). Previously a crate-global
    /// `OnceLock<CommandSurfaceTelemetry>`; moved onto runtime state in
    /// M4 slice 6 (2026-04-22) so iced and egui hosts get their own
    /// sink and the static storage goes away.
    pub(crate) command_surface_telemetry:
        crate::shell::desktop::ui::command_surface_telemetry::CommandSurfaceTelemetry,

    /// Diagnostics state (channel registry snapshots, latency
    /// percentiles, edge metric counters, etc.). Lifted onto the
    /// runtime in §12.16 (2026-04-24) so iced inherits it for free —
    /// previously lived as a field on `EguiHost`. The diagnostics pane
    /// renderer (still egui-specific in `runtime/diagnostics/pane_ui.rs`)
    /// reads from this field; a future slice will lift the rendering
    /// shape to a host-neutral view-model so iced can render the same
    /// data through its own widget set.
    #[cfg(feature = "diagnostics")]
    pub(crate) diagnostics_state: crate::shell::desktop::runtime::diagnostics::DiagnosticsState,
}

impl GraphshellRuntime {
    /// Per-frame runtime tick.
    ///
    /// Conceptually this is the entry point described in the M3.5 runtime
    /// boundary design: the host supplies input, the runtime advances state
    /// and returns a read-only view-model the host renders.
    ///
    /// **Today's state (M4.5b early):** the tick is partially wired. It
    /// ingests the supplied input and projects a view-model from current
    /// runtime state, but does not yet subsume the full frame pipeline
    /// (toolbar rendering, compositor passes, phase orchestration) — those
    /// still run on the host-side path. Work will migrate into `tick` phase
    /// by phase; each migrated phase stops mutating shell state outside the
    /// runtime and starts writing through the supplied `ports` instead.
    ///
    /// The `ports` parameter is accepted generically so that iced can
    /// eventually provide its own port bundle. For now only the input port
    /// is consulted; other ports are held for forward compatibility.
    pub(crate) fn tick<H>(&mut self, input: &FrameHostInput, ports: &mut H) -> FrameViewModel
    where
        H: RuntimeTickPorts,
    {
        self.ingest_frame_input(input);
        self.apply_host_intents(&input.host_intents);
        self.drain_pending_finalize_actions(ports);
        self.project_view_model()
    }

    /// Translate portable `HostIntent`s (coming from chrome surfaces —
    /// iced toolbar, command palette, omnibar) into runtime-internal
    /// reducer actions and apply them. Runs before `project_view_model`
    /// so the tick's view-model output reflects the new state.
    ///
    /// §12.17: this is the sanctioned path for hosts to mutate graph
    /// state. Host adapter files push `HostIntent`s onto
    /// `FrameHostInput.host_intents`; the runtime owns the translation.
    pub(crate) fn apply_host_intents(
        &mut self,
        intents: &[graphshell_core::shell_state::host_intent::HostIntent],
    ) {
        use graphshell_core::shell_state::host_intent::HostIntent;
        for intent in intents {
            match intent {
                HostIntent::CreateNodeAtUrl { url, position } => {
                    // `add_node_and_sync` is the same entrypoint the
                    // egui toolbar submit flow uses today. Translating
                    // to it preserves protocol-probe triggering and
                    // physics re-heat behavior for free.
                    let pos = euclid::default::Point2D::new(position.x, position.y);
                    let _ = self.graph_app.add_node_and_sync(url.clone(), pos);
                }
            }
        }
    }

    /// Drain pending node-status notices and clipboard-copy requests
    /// through the supplied ports. Runs every tick after
    /// `ingest_frame_input` so any host (egui today, iced tomorrow)
    /// gets these user-visible side effects for free.
    fn drain_pending_finalize_actions<P>(&mut self, ports: &mut P)
    where
        P: RuntimeTickPorts,
    {
        drain_pending_runtime_finalize_actions(&mut self.graph_app, ports);
    }

    /// Ingest host-supplied frame input.
    ///
    /// Currently runs the runtime-owned per-frame housekeeping that has
    /// migrated off the host-side phase pipeline. Event-to-intent
    /// translation still flows through the existing `handle_keyboard_phase`
    /// / `pending_webview_context_surface_requests` mechanisms; future
    /// expansions will route those here too.
    pub(crate) fn ingest_frame_input(&mut self, input: &FrameHostInput) {
        // Advance frame-local physics housekeeping (drag-release inertia
        // decay). Previously ran at the top of `run_update_frame_prelude`;
        // migrated here in M4.5b Step 4 because it only touches runtime
        // state.
        self.graph_app.tick_frame();

        // Update the prefetch lifecycle policy based on current memory
        // pressure and selection. Previously ran inside
        // `initialize_frame_intents` during the PreFrameInit phase;
        // migrated here in M4.5b Step 5 because both inputs
        // (`graph_app`, `control_panel`) live on the runtime.
        self.update_prefetch_lifecycle_policy();

        // Drain frame-inbox signals whose consumers only touch runtime
        // state. Settings-route and profile-invalidation drains remain
        // host-side because their consumers reach into `tiles_tree` and
        // the egui `Context` respectively.
        self.apply_pending_runtime_frame_inbox_signals();

        // Record a user-gesture timestamp and advance the idle watchdog for
        // Tier 1 worker suspension. Both previously called directly from
        // `execute_update_frame`; both inputs (`control_panel`,
        // `registry_runtime`) live on the runtime, so the pair runs here.
        // Order matters: record the gesture before checking the threshold
        // so this frame's activity is visible to this frame's watchdog tick.
        if input.had_input_events {
            self.control_panel.notify_user_gesture();
        }
        self.control_panel
            .tick_idle_watchdog(&self.registry_runtime);
    }

    /// Drain the subset of frame-inbox signals whose consumers depend only
    /// on runtime state. Ran per-tick so any host inherits them for free.
    fn apply_pending_runtime_frame_inbox_signals(&mut self) {
        if self.frame_inbox.take_semantic_index_refresh() {
            self.graph_app.refresh_registry_backed_view_lenses();
        }
        if self.frame_inbox.take_workbench_projection_refresh() {
            let _ = crate::shell::desktop::ui::persistence_ops::refresh_workbench_projection_from_manifests(
                &mut self.graph_app,
            );
        }
    }

    /// Refresh the prefetch lifecycle policy on `control_panel` from the
    /// current memory-pressure level and single-selection state on
    /// `graph_app`. Runs every tick via `ingest_frame_input`.
    fn update_prefetch_lifecycle_policy(&self) {
        use crate::app::MemoryPressureLevel;
        use crate::shell::desktop::runtime::control_panel::LifecyclePolicy;

        let memory_pressure_level = self.graph_app.memory_pressure_level();
        let prefetch_target = self.graph_app.get_single_selected_node();
        let (prefetch_enabled, prefetch_interval) = match memory_pressure_level {
            MemoryPressureLevel::Critical => (false, Duration::from_secs(30)),
            MemoryPressureLevel::Warning => (prefetch_target.is_some(), Duration::from_secs(20)),
            MemoryPressureLevel::Normal => (prefetch_target.is_some(), Duration::from_secs(8)),
            MemoryPressureLevel::Unknown => (prefetch_target.is_some(), Duration::from_secs(12)),
        };

        self.control_panel.update_lifecycle_policy(LifecyclePolicy {
            prefetch_enabled,
            prefetch_interval,
            prefetch_target,
            memory_pressure_level,
        });
    }

    /// Project a read-only view-model from current runtime state.
    ///
    /// Populates fields that are directly readable from `GraphshellRuntime`
    /// and `self.graph_app` today, including the per-frame GraphTree layout
    /// outputs (tree rows, tab order, split boundaries) cached onto
    /// `graph_runtime` by `tile_render_pass`. Overlay descriptors, the toast
    /// queue, degraded receipts, and surface-presentation requests are still
    /// left empty — those originate inside the compositor / pipeline
    /// phases that have not yet migrated onto the tick path.
    pub(crate) fn project_view_model(&self) -> FrameViewModel {
        let chrome_ui = &self.graph_app.workspace.chrome_ui;
        let focus_ring_settings = chrome_ui.focus_ring_settings;
        let thumbnail_settings = chrome_ui.thumbnail_settings;
        // Derive the active pane's focused node by correlating the
        // focus-authority's tracked pane activation with the rect
        // roster. Falls back to the first rendered pane when no pane
        // activation is set — maintains the pre-M4 behavior for
        // startup frames and when the user clicks the graph canvas
        // without targeting a pane.
        //
        // Previously this was `active_pane_rects.first()` which ignored
        // focus authority entirely, so the "active" pane reported in
        // the view-model could lag the user's actual selection by one
        // frame when pane activation changed via keyboard.
        let pane_rects = &self.graph_app.workspace.graph_runtime.active_pane_rects;
        let pane_node_order: Vec<_> = pane_rects
            .iter()
            .map(|(pane_id, node_key, _)| (*pane_id, *node_key))
            .collect();
        let focus_projection = project_focus_view_model(FocusProjectionInput {
            graph_surface_focused: self.graph_surface_focused,
            focus_ring_node_key: self.focus_ring_node_key,
            focus_ring_started_at: self.focus_ring_started_at,
            focus_ring_settings: FocusRingSettingsView {
                enabled: focus_ring_settings.enabled,
                duration_ms: focus_ring_settings.duration_ms,
                curve: focus_ring_settings.curve,
                color_override: focus_ring_settings.color_override,
            },
            pane_activation: self.focus_authority.pane_activation,
            pane_node_order: &pane_node_order,
            now: crate::shell::desktop::ui::portable_time::portable_now(),
        });
        let transient_outputs =
            project_transient_frame_outputs(TransientFrameOutputsProjectionInput {
                captures_in_flight: self.thumbnail_capture_in_flight.len(),
            });

        // 2026-04-25 graphshell-runtime extraction Slice 2: shape the
        // per-frame layout-cache block through `frame_projection::
        // project_graph_runtime_layout_view_model`. The shell-side
        // egui::Rect → PortableRect conversion stays here; the runtime
        // helper consumes the already-portable form.
        let layout_active_pane_rects: Vec<_> = self
            .graph_app
            .workspace
            .graph_runtime
            .active_pane_rects
            .iter()
            .map(|(pane_id, node_key, rect)| {
                (
                    *pane_id,
                    *node_key,
                    crate::shell::desktop::workbench::compositor_adapter::portable_rect_from_egui(
                        *rect,
                    ),
                )
            })
            .collect();
        let layout_projection =
            project_graph_runtime_layout_view_model(GraphRuntimeLayoutProjectionInput {
                active_pane_rects: &layout_active_pane_rects,
                pane_render_modes: &self.graph_app.workspace.graph_runtime.pane_render_modes,
                pane_viewer_ids: &self.graph_app.workspace.graph_runtime.pane_viewer_ids,
                tree_rows: &self.graph_app.workspace.graph_runtime.cached_tree_rows,
                tab_order: &self.graph_app.workspace.graph_runtime.cached_tab_order,
                split_boundaries: &self
                    .graph_app
                    .workspace
                    .graph_runtime
                    .cached_split_boundaries,
            });

        FrameViewModel {
            active_pane_rects: layout_projection.active_pane_rects,
            pane_render_modes: layout_projection.pane_render_modes,
            pane_viewer_ids: layout_projection.pane_viewer_ids,
            tree_rows: layout_projection.tree_rows,
            tab_order: layout_projection.tab_order,
            split_boundaries: layout_projection.split_boundaries,
            active_pane: focus_projection.active_pane,
            focus: focus_projection.focus,
            toolbar: project_toolbar_view_model(ToolbarProjectionInput {
                location: self.toolbar_state.editable.location.clone(),
                location_dirty: self.toolbar_state.editable.location_dirty,
                location_submitted: self.toolbar_state.editable.location_submitted,
                load_status: Some(self.toolbar_state.load_status),
                status_text: self.toolbar_state.status_text.clone(),
                can_go_back: self.toolbar_state.can_go_back,
                can_go_forward: self.toolbar_state.can_go_forward,
                active_pane_draft: self.focus_authority.pane_activation.and_then(|pane| {
                    self.toolbar_drafts
                        .get(&pane)
                        .map(|draft| (pane, draft.clone()))
                }),
            }),
            omnibar: self.omnibar_search_session.as_ref().map(project_omnibar),
            graph_search: project_graph_search_view_model(GraphSearchProjectionInput {
                open: self.graph_search_open,
                query: self.graph_search_query.clone(),
                filter_mode: self.graph_search_filter_mode,
                match_count: self.graph_search_matches.len(),
                active_match_index: self.graph_search_active_match_index,
            }),
            command_palette: project_command_palette_view_model(CommandPaletteProjectionInput {
                open: chrome_ui.show_command_palette,
                contextual_mode: chrome_ui.command_palette_contextual_mode,
                query: self.command_palette_session.query.clone(),
                scope: project_palette_scope(self.command_palette_session.scope),
                selected_index: self.command_palette_session.selected_index,
                toggle_requested: self.command_palette_toggle_requested,
            }),
            overlays: transient_outputs.overlays,
            dialogs: project_dialogs_view_model(DialogsProjectionInput {
                bookmark_import_open: self.bookmark_import_dialog.is_some(),
                command_palette_toggle_requested: self.command_palette_toggle_requested,
                show_command_palette: chrome_ui.show_command_palette,
                show_context_palette: chrome_ui.show_context_palette,
                show_help_panel: chrome_ui.show_help_panel,
                show_radial_menu: chrome_ui.show_radial_menu,
                show_settings_overlay: chrome_ui.show_settings_overlay,
                show_clip_inspector: chrome_ui.show_clip_inspector,
                show_scene_overlay: chrome_ui.show_scene_overlay,
                show_clear_data_confirm: self.toolbar_state.show_clear_data_confirm,
                clear_data_confirm_deadline_secs: self.clear_data_confirm_deadline_secs,
            }),
            toasts: transient_outputs.toasts,
            surfaces_to_present: transient_outputs.surfaces_to_present,
            degraded_receipts: transient_outputs.degraded_receipts,
            captures_in_flight: transient_outputs.captures_in_flight,
            settings: project_settings_view_model(SettingsProjectionInput {
                focus_ring: FocusRingSettingsView {
                    enabled: focus_ring_settings.enabled,
                    duration_ms: focus_ring_settings.duration_ms,
                    curve: focus_ring_settings.curve,
                    color_override: focus_ring_settings.color_override,
                },
                thumbnail: ThumbnailSettingsView {
                    enabled: thumbnail_settings.enabled,
                    width: thumbnail_settings.width,
                    height: thumbnail_settings.height,
                    filter: match thumbnail_settings.filter {
                        crate::app::ThumbnailFilter::Nearest => ThumbnailFilterView::Nearest,
                        crate::app::ThumbnailFilter::Triangle => ThumbnailFilterView::Triangle,
                        crate::app::ThumbnailFilter::CatmullRom => ThumbnailFilterView::CatmullRom,
                        crate::app::ThumbnailFilter::Gaussian => ThumbnailFilterView::Gaussian,
                        crate::app::ThumbnailFilter::Lanczos3 => ThumbnailFilterView::Lanczos3,
                    },
                    format: match thumbnail_settings.format {
                        crate::app::ThumbnailFormat::Png => ThumbnailFormatView::Png,
                        crate::app::ThumbnailFormat::Jpeg => ThumbnailFormatView::Jpeg,
                        crate::app::ThumbnailFormat::WebP => ThumbnailFormatView::WebP,
                    },
                    jpeg_quality: thumbnail_settings.jpeg_quality,
                    aspect: match thumbnail_settings.aspect {
                        crate::app::ThumbnailAspect::Fixed => ThumbnailAspectView::Fixed,
                        crate::app::ThumbnailAspect::MatchSource => {
                            ThumbnailAspectView::MatchSource
                        }
                        crate::app::ThumbnailAspect::Square => ThumbnailAspectView::Square,
                    },
                },
            }),
            accessibility: {
                // §12.15 (2026-04-24): summarize AT state from the
                // shell-side UxTreeSnapshot. Hosts use this to decide
                // whether to refresh their AccessKit-side tree this
                // frame; the full snapshot is fetched separately via
                // `ux_tree::latest_snapshot()`.
                let snapshot = crate::shell::desktop::workbench::ux_tree::latest_snapshot();
                project_accessibility_view_model(AccessibilityProjectionInput {
                    focused_node: self.focused_node_hint,
                    snapshot_version: snapshot.as_ref().map(|s| s.semantic_version).unwrap_or(0),
                    snapshot_published: snapshot.is_some(),
                })
            },
            // 2026-04-25 graphshell-runtime extraction Slice 2:
            // is_graph_view derivation moved into the layout helper.
            // True iff `active_pane_rects` is empty, i.e. the graph
            // canvas is the only visible surface.
            is_graph_view: layout_projection.is_graph_view,
        }
    }
}

/// Project an active omnibar session onto its host-neutral view-model.
/// Project the runtime's `SearchPaletteScope` onto the host-neutral
/// view-model enum.
fn project_palette_scope(
    scope: crate::shell::desktop::ui::command_palette_state::SearchPaletteScope,
) -> CommandPaletteScopeView {
    use crate::shell::desktop::ui::command_palette_state::SearchPaletteScope;
    match scope {
        SearchPaletteScope::CurrentTarget => CommandPaletteScopeView::CurrentTarget,
        SearchPaletteScope::ActivePane => CommandPaletteScopeView::ActivePane,
        SearchPaletteScope::ActiveGraph => CommandPaletteScopeView::ActiveGraph,
        SearchPaletteScope::Workbench => CommandPaletteScopeView::Workbench,
    }
}

fn project_omnibar(session: &OmnibarSearchSession) -> graphshell_runtime::OmnibarViewModel {
    project_omnibar_view_model(OmnibarProjectionInput {
        kind: match session.kind {
            OmnibarSessionKind::Graph(_) => OmnibarSessionKindView::Graph,
            OmnibarSessionKind::SearchProvider(_) => OmnibarSessionKindView::SearchProvider,
        },
        query: session.query.clone(),
        match_count: session.matches.len(),
        active_match_index: session.active_index,
        selected_index_count: session.selected_indices.len(),
        provider_status: match session.provider_mailbox.status {
            ProviderSuggestionStatus::Idle => OmnibarProviderStatusView::Idle,
            ProviderSuggestionStatus::Loading => OmnibarProviderStatusView::Loading,
            ProviderSuggestionStatus::Ready => OmnibarProviderStatusView::Ready,
            ProviderSuggestionStatus::Failed(ProviderSuggestionError::Network) => {
                OmnibarProviderStatusView::FailedNetwork
            }
            ProviderSuggestionStatus::Failed(ProviderSuggestionError::HttpStatus(code)) => {
                OmnibarProviderStatusView::FailedHttp(code)
            }
            ProviderSuggestionStatus::Failed(ProviderSuggestionError::Parse) => {
                OmnibarProviderStatusView::FailedParse
            }
        },
    })
}

#[cfg(any(test, feature = "iced-host"))]
impl GraphshellRuntime {
    /// Build a minimal runtime with default infrastructure: a fresh tokio
    /// runtime, a stub `ControlPanel`, placeholder graph state, and empty
    /// session fields. Used by unit tests (via the `for_testing` alias) and
    /// by the M5 iced bring-up path (no servo webviews, no persistence
    /// restore). Full production init still flows through
    /// `EguiHost::new(...)`; the iced host will grow a parallel production
    /// builder once webview + persistence integration moves onto the
    /// runtime boundary.
    pub(crate) fn new_minimal() -> Self {
        let tokio_runtime = tokio::runtime::Runtime::new()
            .expect("failed to create tokio runtime for test GraphshellRuntime");
        let async_spawner: Arc<dyn AsyncSpawner> =
            Arc::new(TokioAsyncSpawner::new(Some(tokio_runtime.handle().clone())));
        let signal_router: Arc<dyn SignalRouter> = Arc::new(RegistrySignalRouter);
        let mut control_panel =
            ControlPanel::new_with_async_spawner(None, Arc::clone(&async_spawner));
        let frame_inbox = spawn_gui_frame_inbox(&mut control_panel, Arc::clone(&signal_router));
        Self {
            graph_app: GraphBrowserApp::new_for_testing(),
            graph_tree: graph_tree::GraphTree::new(
                graph_tree::LayoutMode::TreeStyleTabs,
                graph_tree::ProjectionLens::Traversal,
            ),
            workbench_view_id: GraphViewId::new(),
            toolbar_state: ToolbarState::with_initial_location(""),
            bookmark_import_dialog: None,
            control_panel,
            async_spawner,
            registry_runtime: Arc::new(RegistryRuntime::default()),
            signal_router,
            tokio_runtime,
            viewer_surfaces: ViewerSurfaceRegistry::new(),
            viewer_surface_host: Box::new(NoopViewerSurfaceHost),
            webview_creation_backpressure: HashMap::new(),
            thumbnail_capture_in_flight: std::collections::HashSet::new(),
            frame_inbox,
            graph_search_open: false,
            graph_search_query: String::new(),
            graph_search_filter_mode: false,
            graph_search_matches: Vec::new(),
            graph_search_active_match_index: None,
            focused_node_hint: None,
            graph_surface_focused: false,
            focus_ring_node_key: None,
            focus_ring_started_at: None,
            omnibar_search_session: None,
            omnibar_provider_suggestion_driver: None,
            focus_authority: RuntimeFocusAuthorityState::default(),
            toolbar_drafts: HashMap::new(),
            command_palette_toggle_requested: false,
            command_palette_session: CommandPaletteSession::default(),
            pending_webview_context_surface_requests: Vec::new(),
            clear_data_confirm_deadline_secs: None,
            command_surface_telemetry:
                crate::shell::desktop::ui::command_surface_telemetry::CommandSurfaceTelemetry::new(),
            #[cfg(feature = "diagnostics")]
            diagnostics_state: crate::shell::desktop::runtime::diagnostics::DiagnosticsState::new(),
        }
    }

    /// Test/bring-up alias for [`GraphshellRuntime::new_minimal`].
    /// Retained for callers that pre-date the rename.
    pub(crate) fn for_testing() -> Self {
        Self::new_minimal()
    }
}

/// Portable context-menu request. Completed M4 slice 4 (2026-04-22):
/// `webview_id` is a `ViewerInstanceId`; boundary sites convert from
/// `servo::WebViewId` via `viewer_instance_id_from_servo(...)` and
/// consumers unwrap via `servo_webview_id_from_viewer_instance(...)`.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PendingWebviewContextSurfaceRequest {
    pub(crate) webview_id: ViewerInstanceId,
    pub(crate) anchor: [f32; 2],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PaneRegionHint {
    GraphSurface,
    NodePane,
    ToolPane,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SemanticRegionFocus {
    ModalDialog,
    CommandPalette,
    ContextPalette,
    RadialPalette,
    ClipInspector,
    HelpPanel,
    SceneOverlay,
    SettingsOverlay,
    Toolbar,
    GraphSurface {
        view_id: Option<GraphViewId>,
    },
    NodePane {
        pane_id: Option<PaneId>,
        node_key: Option<NodeKey>,
    },
    ToolPane {
        pane_id: Option<PaneId>,
    },
    Unspecified,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LocalFocusTarget {
    ToolbarLocation { pane_id: Option<PaneId> },
    GraphSearch,
}

/// Focus target for embedded content inside a pane. Completed M4 slice
/// 4 (2026-04-22): `renderer_id` is a `ViewerInstanceId`. The `WebView`
/// variant name describes the target kind, not the provider; a future
/// Wry or MiddleNet direct-viewer target would be a sibling variant,
/// distinguished at the `ViewerInstanceId` level.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EmbeddedContentTarget {
    WebView {
        renderer_id: ViewerInstanceId,
        node_key: Option<NodeKey>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FocusedContentFeatureSupport {
    Unsupported,
    Available,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FocusedContentMediaState {
    Unsupported,
    Silent,
    Playing,
    Muted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FocusedContentDownloadState {
    Unsupported,
    Idle,
    Active,
    Recent,
}

/// Snapshot of the focused viewer's user-visible status. Completed M4
/// slice 4 (2026-04-22): `renderer_id` is a portable `ViewerInstanceId`.
/// The `unavailable` constructor accepts the portable type; callers in
/// `webview_status_sync.rs` wrap servo's `WebViewId` at the boundary.
///
/// `Eq` is not derivable because `content_zoom_level: Option<f32>` carries
/// a float that doesn't implement `Eq` (NaN semantics); tests rely on
/// `PartialEq` only.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FocusedContentStatus {
    pub(crate) node_key: Option<NodeKey>,
    pub(crate) renderer_id: Option<ViewerInstanceId>,
    pub(crate) current_url: Option<String>,
    pub(crate) load_status: ContentLoadState,
    pub(crate) status_text: Option<String>,
    pub(crate) can_go_back: bool,
    pub(crate) can_go_forward: bool,
    pub(crate) can_stop_load: bool,
    pub(crate) find_in_page: FocusedContentFeatureSupport,
    pub(crate) content_zoom_level: Option<f32>,
    pub(crate) media_state: FocusedContentMediaState,
    pub(crate) download_state: FocusedContentDownloadState,
}

impl FocusedContentStatus {
    pub(crate) fn unavailable(
        node_key: Option<NodeKey>,
        renderer_id: Option<ViewerInstanceId>,
    ) -> Self {
        Self {
            node_key,
            renderer_id,
            current_url: None,
            load_status: ContentLoadState::Complete,
            status_text: None,
            can_go_back: false,
            can_go_forward: false,
            can_stop_load: false,
            find_in_page: FocusedContentFeatureSupport::Unsupported,
            content_zoom_level: None,
            media_state: FocusedContentMediaState::Unsupported,
            download_state: FocusedContentDownloadState::Unsupported,
        }
    }

    pub(crate) fn live_content_active(&self) -> bool {
        self.renderer_id.is_some()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FocusCaptureSurface {
    ModalDialog,
    CommandPalette,
    ContextPalette,
    RadialPalette,
    ClipInspector,
    HelpPanel,
    SceneOverlay,
    SettingsOverlay,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ReturnAnchor {
    ToolSurface(ToolSurfaceReturnTarget),
    GraphView(GraphViewId),
    Pane(PaneId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FocusCaptureEntry {
    pub(crate) surface: FocusCaptureSurface,
    pub(crate) return_anchor: Option<ReturnAnchor>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FocusCommand {
    EnterCommandPalette {
        contextual_mode: bool,
        return_target: Option<ToolSurfaceReturnTarget>,
    },
    ExitCommandPalette,
    EnterTransientSurface {
        surface: FocusCaptureSurface,
        return_target: Option<ToolSurfaceReturnTarget>,
    },
    ExitTransientSurface {
        surface: FocusCaptureSurface,
        restore_target: Option<ToolSurfaceReturnTarget>,
    },
    SetEmbeddedContentFocus {
        target: Option<EmbeddedContentTarget>,
    },
    EnterToolPane {
        return_target: Option<ToolSurfaceReturnTarget>,
    },
    ExitToolPane {
        restore_target: Option<ToolSurfaceReturnTarget>,
    },
    SetSemanticRegion {
        region: SemanticRegionFocus,
    },
    Capture {
        surface: FocusCaptureSurface,
        return_anchor: Option<ReturnAnchor>,
    },
    RestoreCapturedFocus {
        surface: FocusCaptureSurface,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeFocusState {
    pub(crate) semantic_region: SemanticRegionFocus,
    pub(crate) pane_activation: Option<PaneId>,
    pub(crate) graph_view_focus: Option<GraphViewId>,
    pub(crate) local_widget_focus: Option<LocalFocusTarget>,
    pub(crate) embedded_content_focus: Option<EmbeddedContentTarget>,
    pub(crate) capture_stack: Vec<FocusCaptureEntry>,
}

impl RuntimeFocusState {
    pub(crate) fn overlay_active(&self) -> bool {
        !self.capture_stack.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeFocusInspector {
    pub(crate) desired: RuntimeFocusState,
    pub(crate) realized: RuntimeFocusState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeFocusInputs {
    pub(crate) semantic_region_override: Option<SemanticRegionFocus>,
    pub(crate) pane_activation: Option<PaneId>,
    pub(crate) pane_region_hint: Option<PaneRegionHint>,
    pub(crate) focused_view: Option<GraphViewId>,
    pub(crate) focused_node_hint: Option<NodeKey>,
    pub(crate) graph_surface_focused: bool,
    pub(crate) local_widget_focus: Option<LocalFocusTarget>,
    /// Completed M4 slice 4 (2026-04-22): portable viewer identity.
    /// Call sites wrap via `viewer_instance_id_from_servo(...)` when
    /// coming from servo-sourced focus events.
    pub(crate) embedded_content_focus_webview: Option<ViewerInstanceId>,
    pub(crate) embedded_content_focus_node: Option<NodeKey>,
    pub(crate) show_command_palette: bool,
    pub(crate) show_context_palette: bool,
    pub(crate) command_palette_contextual_mode: bool,
    pub(crate) show_help_panel: bool,
    pub(crate) show_scene_overlay: bool,
    pub(crate) show_settings_overlay: bool,
    pub(crate) show_radial_menu: bool,
    pub(crate) show_clip_inspector: bool,
    pub(crate) show_clear_data_confirm: bool,
    pub(crate) command_surface_return_target: Option<ToolSurfaceReturnTarget>,
    pub(crate) transient_surface_return_target: Option<ToolSurfaceReturnTarget>,
}
