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

use crate::app::{ClipboardCopyKind, ClipboardCopyRequest, GraphBrowserApp};
use crate::graph::NodeKey;
use crate::shell::desktop::runtime::diagnostics::{DiagnosticEvent, emit_event};
use crate::shell::desktop::runtime::registries::CHANNEL_UI_CLIPBOARD_COPY_FAILED;
use crate::shell::desktop::ui::frame_model::ToastSeverity;
use crate::shell::desktop::ui::host_ports::{HostClipboardPort, HostToastPort};

use super::toast_flow::port_error;

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

pub(crate) fn handle_pending_clipboard_copy_requests<P>(
    graph_app: &mut GraphBrowserApp,
    ports: &mut P,
) where
    P: HostToastPort + HostClipboardPort,
{
    while let Some(ClipboardCopyRequest { key, kind }) = graph_app.take_pending_clipboard_copy() {
        handle_pending_clipboard_copy_request(graph_app, ports, key, kind);
    }
}

fn handle_pending_clipboard_copy_request<P>(
    graph_app: &GraphBrowserApp,
    ports: &mut P,
    key: NodeKey,
    kind: ClipboardCopyKind,
) where
    P: HostToastPort + HostClipboardPort,
{
    let Some(value) = clipboard_copy_value_for_node(graph_app, key, kind, ports) else {
        return;
    };

    match ports.set_text(&value) {
        Ok(()) => emit_clipboard_copy_success_toast(ports, kind),
        Err(e) => {
            emit_clipboard_copy_failure(e.len());
            let failure_text = if e == "clipboard unavailable" {
                CLIPBOARD_STATUS_UNAVAILABLE_TEXT.to_string()
            } else {
                clipboard_copy_failure_text(&e)
            };
            port_error(ports, failure_text);
        }
    }
}

pub(crate) const CLIPBOARD_STATUS_SUCCESS_URL_TEXT: &str = "Copied URL";
pub(crate) const CLIPBOARD_STATUS_SUCCESS_TITLE_TEXT: &str = "Copied title";
pub(crate) const CLIPBOARD_STATUS_UNAVAILABLE_TEXT: &str = "Clipboard unavailable";
pub(crate) const CLIPBOARD_STATUS_EMPTY_TEXT: &str = "Nothing to copy";
pub(crate) const CLIPBOARD_STATUS_FAILURE_PREFIX: &str = "Copy failed";
pub(crate) const CLIPBOARD_STATUS_MISSING_NODE_SUGGESTION_TEXT: &str =
    "select a node and try again";

fn clipboard_copy_value_for_node<P>(
    graph_app: &GraphBrowserApp,
    key: NodeKey,
    kind: ClipboardCopyKind,
    port: &mut P,
) -> Option<String>
where
    P: HostToastPort + ?Sized,
{
    let Some(node) = graph_app.domain_graph().get_node(key) else {
        port.enqueue_message(
            ToastSeverity::Error,
            clipboard_copy_missing_node_failure_text(),
            None,
        );
        return None;
    };

    let visible_url = graph_app
        .user_visible_node_url(key)
        .unwrap_or_else(|| node.url().to_string());
    let visible_title = graph_app
        .user_visible_node_title(key)
        .unwrap_or_else(|| node.title.clone());

    let value = match kind {
        ClipboardCopyKind::Url => visible_url,
        ClipboardCopyKind::Title => {
            clipboard_title_or_url(visible_title.as_str(), visible_url.as_str())
        }
    };

    if value.trim().is_empty() {
        port.enqueue_message(ToastSeverity::Warning, CLIPBOARD_STATUS_EMPTY_TEXT, None);
        return None;
    }

    Some(value)
}

fn clipboard_title_or_url(title: &str, url: &str) -> String {
    if title.is_empty() {
        url.to_owned()
    } else {
        title.to_owned()
    }
}

fn emit_clipboard_copy_failure(byte_len: usize) {
    emit_event(DiagnosticEvent::MessageSent {
        channel_id: CHANNEL_UI_CLIPBOARD_COPY_FAILED,
        byte_len,
    });
}

fn emit_clipboard_copy_success_toast<P>(port: &mut P, kind: ClipboardCopyKind)
where
    P: HostToastPort + ?Sized,
{
    port.enqueue_message(
        ToastSeverity::Success,
        clipboard_copy_success_text(kind).to_string(),
        None,
    );
}

pub(crate) fn clipboard_copy_success_text(kind: ClipboardCopyKind) -> &'static str {
    match kind {
        ClipboardCopyKind::Url => CLIPBOARD_STATUS_SUCCESS_URL_TEXT,
        ClipboardCopyKind::Title => CLIPBOARD_STATUS_SUCCESS_TITLE_TEXT,
    }
}

pub(crate) fn clipboard_copy_failure_text(detail: &str) -> String {
    format!("{CLIPBOARD_STATUS_FAILURE_PREFIX}: {detail}")
}

pub(crate) fn clipboard_copy_missing_node_failure_text() -> String {
    clipboard_copy_failure_text(
        format!("node no longer exists; {CLIPBOARD_STATUS_MISSING_NODE_SUGGESTION_TEXT}").as_str(),
    )
}
