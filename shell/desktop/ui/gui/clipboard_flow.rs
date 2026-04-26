/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Clipboard copy flow — drain pending `ClipboardCopyRequest`s from
//! `graph_app` and write them through `HostClipboardPort`, surfacing
//! success / failure through `HostToastPort`.
//!
//! Split out of `gui_orchestration.rs` as part of M6 §4.1. Owns:
//!
//! - [`ClipboardAdapter`] — bridges a raw `Option<arboard::Clipboard>`
//!   into `HostClipboardPort` for test paths.
//! - [`handle_pending_clipboard_copy_requests`] — the public drain
//!   entry point called from `GraphshellRuntime::tick`.
//! - Private helpers for value derivation (url vs title) and
//!   success/failure message shaping.
//!
//! Uses toast helpers from [`super::toast_flow`].

use arboard::Clipboard;
pub(crate) use graphshell_runtime::CLIPBOARD_STATUS_EMPTY_TEXT;
pub(crate) use graphshell_runtime::CLIPBOARD_STATUS_FAILURE_PREFIX;
pub(crate) use graphshell_runtime::CLIPBOARD_STATUS_SUCCESS_TITLE_TEXT;
pub(crate) use graphshell_runtime::CLIPBOARD_STATUS_SUCCESS_URL_TEXT;
pub(crate) use graphshell_runtime::CLIPBOARD_STATUS_UNAVAILABLE_TEXT;
use graphshell_runtime::ClipboardCopyKind;
use graphshell_runtime::ClipboardCopyRequest;
use graphshell_runtime::ClipboardCopySource;
use graphshell_runtime::RuntimeClipboardCopyState;
pub(crate) use graphshell_runtime::clipboard_copy_failure_text;
pub(crate) use graphshell_runtime::clipboard_copy_missing_node_failure_text;
pub(crate) use graphshell_runtime::clipboard_copy_success_text;
use graphshell_runtime::drain_pending_clipboard_copy_requests;

use crate::app::GraphBrowserApp;
use crate::shell::desktop::runtime::diagnostics::{DiagnosticEvent, emit_event};
use crate::shell::desktop::runtime::registries::CHANNEL_UI_CLIPBOARD_COPY_FAILED;
use crate::shell::desktop::ui::host_ports::{HostClipboardPort, HostToastPort};

/// Adapter that bridges a raw `&mut Option<Clipboard>` into
/// `HostClipboardPort`. Mirrors the wiring in `EguiHostPorts::clipboard`
/// and is used by the same test-path entry points as `ToastsAdapter`.
pub(crate) struct ClipboardAdapter<'a> {
    pub(crate) clipboard: &'a mut Option<Clipboard>,
}

impl<'a> HostClipboardPort for ClipboardAdapter<'a> {
    fn get_text(&mut self) -> Option<String> {
        let cb = self.clipboard.as_mut()?;
        cb.get_text().ok()
    }

    fn set_text(&mut self, text: &str) -> Result<(), String> {
        if self.clipboard.is_none() {
            *self.clipboard = Clipboard::new().ok();
        }
        let Some(cb) = self.clipboard.as_mut() else {
            return Err("clipboard unavailable".to_string());
        };
        cb.set_text(text).map_err(|e| e.to_string())
    }
}

impl RuntimeClipboardCopyState for GraphBrowserApp {
    fn take_pending_clipboard_copy(&mut self) -> Option<ClipboardCopyRequest> {
        GraphBrowserApp::take_pending_clipboard_copy(self)
    }

    fn clipboard_copy_source(&self, key: crate::graph::NodeKey) -> Option<ClipboardCopySource> {
        let node = self.domain_graph().get_node(key)?;
        let visible_url = self
            .user_visible_node_url(key)
            .unwrap_or_else(|| node.url().to_string());
        let visible_title = self
            .user_visible_node_title(key)
            .unwrap_or_else(|| node.title.clone());

        Some(ClipboardCopySource {
            visible_url,
            visible_title,
        })
    }
}

pub(crate) fn handle_pending_clipboard_copy_requests<P>(
    graph_app: &mut GraphBrowserApp,
    ports: &mut P,
) where
    P: HostToastPort + HostClipboardPort,
{
    drain_pending_clipboard_copy_requests(graph_app, ports, emit_clipboard_copy_failure);
}

fn emit_clipboard_copy_failure(byte_len: usize) {
    emit_event(DiagnosticEvent::MessageSent {
        channel_id: CHANNEL_UI_CLIPBOARD_COPY_FAILED,
        byte_len,
    });
}
