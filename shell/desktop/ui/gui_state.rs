/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::app::{GraphBrowserApp, GraphViewId, ToolSurfaceReturnTarget};
use crate::graph::NodeKey;
use crate::shell::desktop::lifecycle::webview_backpressure::WebviewCreationBackpressureState;
use crate::shell::desktop::runtime::control_panel::ControlPanel;
use crate::shell::desktop::runtime::registries::RegistryRuntime;
use crate::shell::desktop::ui::frame_model::{
    DialogsViewModel, FocusRingSpec, FocusViewModel, FrameHostInput, FrameViewModel,
    ToolbarDraftSnapshot, ToolbarViewModel,
};
use crate::shell::desktop::ui::gui::frame_inbox::GuiFrameInbox;
use crate::shell::desktop::ui::host_ports::HostPorts;
use crate::shell::desktop::ui::toolbar::toolbar_ui::OmnibarSearchSession;
use crate::shell::desktop::workbench::compositor_adapter::ViewerSurfaceRegistry;
use crate::shell::desktop::workbench::pane_model::PaneId;
use egui_file_dialog::{DialogState, FileDialog as EguiFileDialog, Filter};
use servo::{LoadStatus, WebViewId};

pub(crate) struct ToolbarState {
    pub(crate) location: String,
    pub(crate) location_dirty: bool,
    pub(crate) location_submitted: bool,
    pub(crate) show_clear_data_confirm: bool,
    pub(crate) load_status: LoadStatus,
    pub(crate) status_text: Option<String>,
    pub(crate) can_go_back: bool,
    pub(crate) can_go_forward: bool,
}

#[derive(Clone)]
pub(crate) struct ToolbarDraft {
    pub(crate) location: String,
    pub(crate) location_dirty: bool,
    pub(crate) location_submitted: bool,
}

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

impl ToolbarDraft {
    pub(super) fn from_toolbar_state(toolbar_state: &ToolbarState) -> Self {
        Self {
            location: toolbar_state.location.clone(),
            location_dirty: toolbar_state.location_dirty,
            location_submitted: toolbar_state.location_submitted,
        }
    }

    pub(super) fn apply_to_toolbar_state(&self, toolbar_state: &mut ToolbarState) {
        toolbar_state.location = self.location.clone();
        toolbar_state.location_dirty = self.location_dirty;
        toolbar_state.location_submitted = self.location_submitted;
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

    /// Registry runtime for semantic services.
    pub(crate) registry_runtime: Arc<RegistryRuntime>,

    /// Tokio runtime for async background workers.
    pub(crate) tokio_runtime: tokio::runtime::Runtime,

    /// Phase D unified viewer surface registry keyed by NodeKey. Single
    /// authority for per-node content surface state.
    pub(crate) viewer_surfaces: ViewerSurfaceRegistry,

    /// Runtime backpressure state for tile-driven viewer creation retries.
    pub(crate) webview_creation_backpressure: HashMap<NodeKey, WebviewCreationBackpressureState>,

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
    pub(crate) focus_ring_started_at: Option<Instant>,
    pub(crate) focus_ring_duration: Duration,
    pub(crate) omnibar_search_session: Option<OmnibarSearchSession>,
    pub(crate) focus_authority: RuntimeFocusAuthorityState,
    pub(crate) toolbar_drafts: HashMap<PaneId, ToolbarDraft>,
    pub(crate) command_palette_toggle_requested: bool,
    pub(crate) pending_webview_context_surface_requests: Vec<PendingWebviewContextSurfaceRequest>,
    pub(crate) deferred_open_child_webviews: Vec<WebViewId>,
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
    pub(crate) fn tick<H: HostPorts>(
        &mut self,
        input: &FrameHostInput,
        _ports: &mut H,
    ) -> FrameViewModel {
        self.ingest_frame_input(input);
        self.project_view_model()
    }

    /// Ingest host-supplied frame input.
    ///
    /// Currently runs the runtime-owned per-frame housekeeping that has
    /// migrated off the host-side phase pipeline. Event-to-intent
    /// translation still flows through the existing `handle_keyboard_phase`
    /// / `pending_webview_context_surface_requests` mechanisms; future
    /// expansions will route those here too.
    pub(crate) fn ingest_frame_input(&mut self, _input: &FrameHostInput) {
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
            MemoryPressureLevel::Warning => {
                (prefetch_target.is_some(), Duration::from_secs(20))
            }
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
        let focus_ring = self.focus_ring_node_key.map(|node_key| FocusRingSpec {
            node_key,
            started_at: self.focus_ring_started_at.unwrap_or_else(Instant::now),
            duration: self.focus_ring_duration,
        });

        FrameViewModel {
            active_pane_rects: self
                .graph_app
                .workspace
                .graph_runtime
                .active_pane_rects
                .clone(),
            tree_rows: self
                .graph_app
                .workspace
                .graph_runtime
                .cached_tree_rows
                .clone(),
            tab_order: self
                .graph_app
                .workspace
                .graph_runtime
                .cached_tab_order
                .clone(),
            split_boundaries: self
                .graph_app
                .workspace
                .graph_runtime
                .cached_split_boundaries
                .clone(),
            active_pane: self
                .graph_app
                .workspace
                .graph_runtime
                .active_pane_rects
                .first()
                .map(|(_, node_key, _)| *node_key),
            focus: FocusViewModel {
                focused_node: self.focused_node_hint,
                graph_surface_focused: self.graph_surface_focused,
                focus_ring,
            },
            toolbar: ToolbarViewModel {
                location: self.toolbar_state.location.clone(),
                location_dirty: self.toolbar_state.location_dirty,
                location_submitted: self.toolbar_state.location_submitted,
                load_status: Some(self.toolbar_state.load_status),
                status_text: self.toolbar_state.status_text.clone(),
                can_go_back: self.toolbar_state.can_go_back,
                can_go_forward: self.toolbar_state.can_go_forward,
                per_pane_drafts: self
                    .toolbar_drafts
                    .iter()
                    .map(|(pane_id, draft)| {
                        (
                            *pane_id,
                            ToolbarDraftSnapshot {
                                location: draft.location.clone(),
                                location_dirty: draft.location_dirty,
                                location_submitted: draft.location_submitted,
                            },
                        )
                    })
                    .collect(),
            },
            overlays: Vec::new(),
            dialogs: DialogsViewModel {
                bookmark_import_open: self.bookmark_import_dialog.is_some(),
                command_palette_toggle_requested: self.command_palette_toggle_requested,
                show_command_palette: chrome_ui.show_command_palette,
                show_context_palette: chrome_ui.show_context_palette,
                show_help_panel: chrome_ui.show_help_panel,
                show_radial_menu: chrome_ui.show_radial_menu,
                show_settings_overlay: chrome_ui.show_settings_overlay,
                show_clip_inspector: chrome_ui.show_clip_inspector,
                show_scene_overlay: chrome_ui.show_scene_overlay,
            },
            toasts: Vec::new(),
            surfaces_to_present: Vec::new(),
            degraded_receipts: Vec::new(),
        }
    }
}

#[cfg(test)]
impl GraphshellRuntime {
    /// Build a minimal runtime suitable for focus-state / session-state unit
    /// tests. The infrastructure fields (control_panel, registry, tokio_runtime,
    /// etc.) are initialized to sensible defaults; tests can mutate whichever
    /// session fields they need to exercise.
    pub(crate) fn for_testing() -> Self {
        let tokio_runtime = tokio::runtime::Runtime::new()
            .expect("failed to create tokio runtime for test GraphshellRuntime");
        let mut control_panel =
            ControlPanel::new_with_runtime(None, tokio_runtime.handle().clone());
        let frame_inbox = GuiFrameInbox::spawn(&mut control_panel);
        Self {
            graph_app: GraphBrowserApp::new_for_testing(),
            graph_tree: graph_tree::GraphTree::new(
                graph_tree::LayoutMode::TreeStyleTabs,
                graph_tree::ProjectionLens::Traversal,
            ),
            workbench_view_id: GraphViewId::new(),
            toolbar_state: ToolbarState {
                location: String::new(),
                location_dirty: false,
                location_submitted: false,
                show_clear_data_confirm: false,
                load_status: servo::LoadStatus::Complete,
                status_text: None,
                can_go_back: false,
                can_go_forward: false,
            },
            bookmark_import_dialog: None,
            control_panel,
            registry_runtime: Arc::new(RegistryRuntime::default()),
            tokio_runtime,
            viewer_surfaces: ViewerSurfaceRegistry::new(),
            webview_creation_backpressure: HashMap::new(),
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
            focus_ring_duration: Duration::from_millis(500),
            omnibar_search_session: None,
            focus_authority: RuntimeFocusAuthorityState::default(),
            toolbar_drafts: HashMap::new(),
            command_palette_toggle_requested: false,
            pending_webview_context_surface_requests: Vec::new(),
            deferred_open_child_webviews: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PendingWebviewContextSurfaceRequest {
    pub(crate) webview_id: WebViewId,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EmbeddedContentTarget {
    WebView {
        renderer_id: WebViewId,
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

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FocusedContentStatus {
    pub(crate) node_key: Option<NodeKey>,
    pub(crate) renderer_id: Option<WebViewId>,
    pub(crate) current_url: Option<String>,
    pub(crate) load_status: LoadStatus,
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
    pub(crate) fn unavailable(node_key: Option<NodeKey>, renderer_id: Option<WebViewId>) -> Self {
        Self {
            node_key,
            renderer_id,
            current_url: None,
            load_status: LoadStatus::Complete,
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
    pub(crate) embedded_content_focus_webview: Option<WebViewId>,
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
