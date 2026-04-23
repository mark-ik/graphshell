/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! App-facing runtime seam.
//!
//! The graph-app layer depends on these imports as portable runtime
//! vocabulary. Today they re-export the desktop shell implementation;
//! follow-on extraction can redirect this module to a verso-host crate or
//! another host implementation without reopening `graph_app.rs`.

#[allow(unused_imports)]
pub(crate) use crate::shell::desktop::runtime::{caches, control_panel, diagnostics, registries};
pub(crate) use crate::shell::desktop::runtime::caches::{CachePolicy, RuntimeCaches};
pub(crate) use crate::shell::desktop::runtime::diagnostics::{DiagnosticEvent, emit_event};
pub(crate) use crate::shell::desktop::runtime::registries::input::{
    InputBinding, InputBindingRemap, InputConflict as InputRemapConflict, InputContext,
};
pub(crate) use crate::shell::desktop::runtime::registries::{
    CHANNEL_HISTORY_ARCHIVE_CLEAR_FAILED, CHANNEL_HISTORY_ARCHIVE_DISSOLVED_APPENDED,
    CHANNEL_HISTORY_ARCHIVE_EXPORT_FAILED, CHANNEL_HISTORY_TIMELINE_PREVIEW_ENTERED,
    CHANNEL_HISTORY_TIMELINE_PREVIEW_EXITED, CHANNEL_HISTORY_TIMELINE_PREVIEW_ISOLATION_VIOLATION,
    CHANNEL_HISTORY_TIMELINE_REPLAY_FAILED, CHANNEL_HISTORY_TIMELINE_REPLAY_STARTED,
    CHANNEL_HISTORY_TIMELINE_REPLAY_SUCCEEDED, CHANNEL_HISTORY_TIMELINE_RETURN_TO_PRESENT_FAILED,
    CHANNEL_HISTORY_TRAVERSAL_RECORD_FAILED, CHANNEL_HISTORY_TRAVERSAL_RECORDED,
    CHANNEL_PERSISTENCE_RECOVER_FAILED, CHANNEL_PERSISTENCE_RECOVER_SUCCEEDED,
    CHANNEL_STARTUP_PERSISTENCE_OPEN_FAILED, CHANNEL_STARTUP_PERSISTENCE_OPEN_STARTED,
    CHANNEL_STARTUP_PERSISTENCE_OPEN_SUCCEEDED, CHANNEL_STARTUP_PERSISTENCE_OPEN_TIMEOUT,
    CHANNEL_UI_GRAPH_CAMERA_COMMAND_BLOCKED_MISSING_TARGET_VIEW,
    CHANNEL_UI_GRAPH_CAMERA_REQUEST_BLOCKED, CHANNEL_UI_GRAPH_KEYBOARD_ZOOM_BLOCKED,
    CHANNEL_UX_NAVIGATION_TRANSITION, phase2_apply_input_binding_remaps,
    phase2_describe_input_bindings, phase2_reset_input_binding_remaps,
};