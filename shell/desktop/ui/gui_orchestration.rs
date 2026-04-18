/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Shell-side orchestration surface.
//!
//! Post-M6 §4.1 split: this file is now mostly module declarations +
//! re-exports. The phase logic lives in per-concern submodules under
//! `gui/` (clipboard, toast, pre-frame, toolbar-phase, pending-open,
//! semantic-lifecycle, workbench-dispatch, graph-search-flow-phase).
//! Each re-export below names the public entry points the rest of the
//! shell (and tests) reach `gui_orchestration::*`.
//!
//! The imports that follow look dead by Rust's analysis but several
//! submodules rely on them through `use super::*;` — notably
//! `focus_realizer` and `workbench_intent_interceptor`. Silencing the
//! unused-import warnings here is simpler than enumerating every
//! transitive name. A follow-on cleanup can dissolve `use super::*`
//! in those two submodules and remove the allow.

#![allow(unused_imports)]

use std::collections::{HashMap, HashSet};

use crate::app::{
    GraphBrowserApp, GraphIntent, GraphSearchHistoryEntry, GraphSearchOrigin, GraphSearchRequest,
    LifecycleCause, PendingTileOpenMode, SearchDisplayMode, UndoBoundaryReason, WorkbenchIntent,
};
use crate::graph::NodeKey;
use crate::shell::desktop::host::running_app_state::RunningAppState;
use crate::shell::desktop::host::window::EmbedderWindow;
use crate::shell::desktop::lifecycle::lifecycle_intents;
use crate::shell::desktop::lifecycle::webview_backpressure::WebviewCreationBackpressureState;
use crate::shell::desktop::runtime::control_panel::ControlPanel;
#[cfg(feature = "diagnostics")]
use crate::shell::desktop::runtime::diagnostics;
use crate::shell::desktop::runtime::diagnostics::{DiagnosticEvent, emit_event};
use crate::shell::desktop::runtime::registries::{
    CHANNEL_UX_CONTRACT_WARNING, CHANNEL_UX_DISPATCH_CONSUMED,
    CHANNEL_UX_DISPATCH_DEFAULT_PREVENTED, CHANNEL_UX_DISPATCH_PHASE, CHANNEL_UX_DISPATCH_STARTED,
    CHANNEL_UX_FOCUS_REALIZATION_MISMATCH, CHANNEL_UX_FOCUS_RETURN_FALLBACK,
    CHANNEL_UX_NAVIGATION_TRANSITION, CHANNEL_UX_NAVIGATION_VIOLATION,
};
use crate::shell::desktop::ui::graph_search_flow::{self, GraphSearchFlowArgs};
use crate::shell::desktop::ui::graph_search_ui::{self, GraphSearchUiArgs};
use crate::shell::desktop::ui::gui_frame::ToolbarDialogPhaseArgs;
use crate::shell::desktop::ui::gui_frame::{self};
use crate::shell::desktop::ui::gui_state::{
    LocalFocusTarget, RuntimeFocusAuthorityState, ToolbarState,
};
use crate::shell::desktop::ui::nav_targeting;
use crate::shell::desktop::ui::toolbar::toolbar_ui::OmnibarSearchSession;
use crate::shell::desktop::ui::toolbar_routing::{self, ToolbarOpenMode};
use crate::shell::desktop::workbench::pane_model::{PaneViewState, ToolPaneState};
use crate::shell::desktop::workbench::tile_kind::TileKind;
use crate::shell::desktop::workbench::tile_view_ops::{TileOpenMode, ToggleTileViewArgs};
use egui_tiles::{Tile, Tree};
use servo::WebViewId;
use servo::{OffscreenRenderingContext, WindowRenderingContext};
use std::rc::Rc;
use winit::window::Window;

#[path = "gui/clipboard_flow.rs"]
mod clipboard_flow;
#[path = "gui/focus_realizer.rs"]
mod focus_realizer;
#[path = "gui/graph_search_flow_phase.rs"]
mod graph_search_flow_phase;
#[path = "gui/pending_open_flow.rs"]
mod pending_open_flow;
#[path = "gui/pre_frame_flow.rs"]
mod pre_frame_flow;
#[path = "gui/semantic_lifecycle_flow.rs"]
mod semantic_lifecycle_flow;
#[path = "gui/toast_flow.rs"]
mod toast_flow;
#[path = "gui/toolbar_draft.rs"]
mod toolbar_draft;
#[path = "gui/toolbar_phase_flow.rs"]
mod toolbar_phase_flow;
#[path = "gui/workbench_dispatch_flow.rs"]
mod workbench_dispatch_flow;
#[path = "gui/workbench_intent_interceptor.rs"]
mod workbench_intent_interceptor;

pub(crate) use clipboard_flow::{
    CLIPBOARD_STATUS_EMPTY_TEXT, CLIPBOARD_STATUS_FAILURE_PREFIX,
    CLIPBOARD_STATUS_SUCCESS_TITLE_TEXT, CLIPBOARD_STATUS_SUCCESS_URL_TEXT,
    CLIPBOARD_STATUS_UNAVAILABLE_TEXT, ClipboardAdapter, clipboard_copy_failure_text,
    clipboard_copy_missing_node_failure_text, clipboard_copy_success_text,
    handle_pending_clipboard_copy_requests,
};
pub(crate) use pending_open_flow::{
    handle_pending_open_clip_after_intents, handle_pending_open_node_after_intents,
    handle_pending_open_note_after_intents,
};
pub(crate) use graph_search_flow_phase::{
    active_graph_search_match, graph_search_toast_message, run_graph_search_phase,
    run_graph_search_window_phase,
};
pub(crate) use pre_frame_flow::{PreFramePhaseOutput, run_pre_frame_phase};
pub(crate) use semantic_lifecycle_flow::run_semantic_lifecycle_phase;
pub(crate) use toast_flow::{ToastsAdapter, handle_pending_node_status_notices};
pub(crate) use toolbar_draft::{persist_active_toolbar_draft, sync_active_toolbar_draft};
pub(crate) use toolbar_phase_flow::{run_keyboard_phase, run_toolbar_phase};
pub(crate) use workbench_dispatch_flow::handle_tool_pane_intents;

use focus_realizer::FocusRealizer;



#[cfg(test)]
#[path = "gui_orchestration_tests.rs"]
mod gui_orchestration_tests;
