/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::app::{GraphViewId, ToolSurfaceReturnTarget};
use crate::graph::NodeKey;
use crate::shell::desktop::ui::toolbar::toolbar_ui::OmnibarSearchSession;
use crate::shell::desktop::workbench::pane_model::PaneId;
use servo::{LoadStatus, WebViewId};

pub(super) struct ToolbarState {
    pub(super) location: String,
    pub(super) location_dirty: bool,
    pub(super) location_submitted: bool,
    pub(super) show_clear_data_confirm: bool,
    pub(super) load_status: LoadStatus,
    pub(super) status_text: Option<String>,
    pub(super) can_go_back: bool,
    pub(super) can_go_forward: bool,
}

#[derive(Clone)]
pub(super) struct ToolbarDraft {
    pub(super) location: String,
    pub(super) location_dirty: bool,
    pub(super) location_submitted: bool,
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
    pub(super) semantic_region: Option<SemanticRegionFocus>,
    pub(super) local_widget_focus: Option<LocalFocusTarget>,
    pub(super) embedded_content_focus: Option<EmbeddedContentTarget>,
    pub(super) tool_surface_return_target: Option<ToolSurfaceReturnTarget>,
    pub(super) command_surface_return_target: Option<ToolSurfaceReturnTarget>,
    pub(super) transient_surface_return_target: Option<ToolSurfaceReturnTarget>,
    pub(crate) capture_stack: Vec<FocusCaptureEntry>,
    pub(crate) realized_focus_state: Option<RuntimeFocusState>,
}

pub(super) struct GuiRuntimeState {
    pub(super) graph_search_open: bool,
    pub(super) graph_search_query: String,
    pub(super) graph_search_filter_mode: bool,
    pub(super) graph_search_matches: Vec<NodeKey>,
    pub(super) graph_search_active_match_index: Option<usize>,
    pub(super) focused_node_hint: Option<NodeKey>,
    pub(super) graph_surface_focused: bool,
    pub(super) focus_ring_node_key: Option<NodeKey>,
    pub(super) focus_ring_started_at: Option<Instant>,
    pub(super) focus_ring_duration: Duration,
    pub(super) omnibar_search_session: Option<OmnibarSearchSession>,
    pub(super) focus_authority: RuntimeFocusAuthorityState,
    pub(super) toolbar_drafts: HashMap<PaneId, ToolbarDraft>,
    pub(super) command_palette_toggle_requested: bool,
    pub(super) pending_webview_context_surface_requests: Vec<PendingWebviewContextSurfaceRequest>,
    pub(super) deferred_open_child_webviews: Vec<WebViewId>,
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
    pub(crate) show_settings_overlay: bool,
    pub(crate) show_radial_menu: bool,
    pub(crate) show_clip_inspector: bool,
    pub(crate) show_clear_data_confirm: bool,
    pub(crate) command_surface_return_target: Option<ToolSurfaceReturnTarget>,
    pub(crate) transient_surface_return_target: Option<ToolSurfaceReturnTarget>,
}
