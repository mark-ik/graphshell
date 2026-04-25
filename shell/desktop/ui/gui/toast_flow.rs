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

use graphshell_runtime::NodeStatusNotice;
use graphshell_runtime::RuntimeNodeStatusNoticeState;
use graphshell_runtime::ToastSeverity;
use graphshell_runtime::ToastSpec;
use graphshell_runtime::drain_pending_node_status_notices;
use graphshell_runtime::emit_node_status_toast;
use graphshell_runtime::port_error;

use crate::app::{GraphBrowserApp, NodeStatusNoticeRequest};
use crate::graph::NodeKey;
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

impl RuntimeNodeStatusNoticeState for GraphBrowserApp {
    type AuditEvent = crate::services::persistence::types::NodeAuditEventKind;

    fn take_pending_node_status_notice(
        &mut self,
    ) -> Option<(NodeStatusNotice, Option<Self::AuditEvent>)> {
        GraphBrowserApp::take_pending_node_status_notice(self).map(
            |NodeStatusNoticeRequest {
                 notice,
                 audit_event,
             }| { (notice, audit_event) },
        )
    }

    fn log_node_status_audit_event(&mut self, key: NodeKey, event: Self::AuditEvent) {
        GraphBrowserApp::log_node_audit_event(self, key, event);
    }
}

pub(crate) fn handle_pending_node_status_notices<P>(graph_app: &mut GraphBrowserApp, port: &mut P)
where
    P: HostToastPort + ?Sized,
{
    drain_pending_node_status_notices(graph_app, port);
}
