/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Toast emission and host-neutral notification helpers.
//!
//! Split out of `gui_orchestration.rs` as part of M6 §4.1. Owns:
//!
//! - [`ToastsAdapter`] — bridges a raw `egui_notify::Toasts` into
//!   `HostToastPort` for tests that don't want to build the full
//!   `EguiHostPorts` bundle.
//! - [`handle_pending_node_status_notices`] — drains pending
//!   `NodeStatusNoticeRequest`s from `graph_app` through any
//!   `HostToastPort`.
//! - Shared helpers `emit_node_status_toast` and `port_error` used by
//!   the sibling [`super::clipboard_flow`] module.
//!
//! Re-exported from `gui_orchestration` so existing callers see the
//! same public API surface.

use crate::app::{GraphBrowserApp, NodeStatusNoticeRequest, UiNotificationLevel};
use crate::shell::desktop::ui::frame_model::{ToastSeverity, ToastSpec};
use crate::shell::desktop::ui::host_ports::HostToastPort;

/// Adapter that bridges a raw `&mut egui_notify::Toasts` into
/// `HostToastPort`. Used by tests that want the port-taking drain
/// functions without building the full `EguiHostPorts` bundle.
pub(crate) struct ToastsAdapter<'a> {
    pub(crate) toasts: &'a mut egui_notify::Toasts,
}

impl<'a> HostToastPort for ToastsAdapter<'a> {
    fn enqueue(&mut self, toast: ToastSpec) {
        let entry = match toast.severity {
            ToastSeverity::Info => self.toasts.info(toast.message),
            ToastSeverity::Success => self.toasts.success(toast.message),
            ToastSeverity::Warning => self.toasts.warning(toast.message),
            ToastSeverity::Error => self.toasts.error(toast.message),
        };
        if let Some(duration) = toast.duration {
            entry.duration(Some(duration));
        }
    }
}

pub(crate) fn handle_pending_node_status_notices<P>(graph_app: &mut GraphBrowserApp, port: &mut P)
where
    P: HostToastPort + ?Sized,
{
    while let Some(NodeStatusNoticeRequest {
        key,
        level,
        message,
        audit_event,
    }) = graph_app.take_pending_node_status_notice()
    {
        emit_node_status_toast(port, level, &message);

        let Some(event) = audit_event else {
            continue;
        };
        graph_app.log_node_audit_event(key, event);
    }
}

/// Emit a single error-severity toast through the port. Shared by the
/// clipboard flow for failure reporting.
pub(crate) fn port_error<P>(port: &mut P, message: impl Into<String>)
where
    P: HostToastPort + ?Sized,
{
    port.enqueue_message(ToastSeverity::Error, message, None);
}

/// Emit a node-status notice at the requested severity. Shared by the
/// clipboard flow for success/warning paths.
pub(crate) fn emit_node_status_toast<P>(port: &mut P, level: UiNotificationLevel, message: &str)
where
    P: HostToastPort + ?Sized,
{
    let severity = match level {
        UiNotificationLevel::Success => ToastSeverity::Success,
        UiNotificationLevel::Warning => ToastSeverity::Warning,
        UiNotificationLevel::Error => ToastSeverity::Error,
    };
    port.enqueue_message(severity, message.to_string(), None);
}
