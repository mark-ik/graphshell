/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Portable host services consumed directly by `GraphshellRuntime::tick`.
//!
//! This crate does not yet own the full host boundary. It starts with the
//! smallest real seam: finalize-action side effects (`ToastSpec` delivery and
//! clipboard writes) that already operate on portable types.

use std::time::Duration;

use graphshell_core::shell_state::frame_model::{ToastSeverity, ToastSpec};

/// Clipboard get/set for runtime-owned finalize actions.
pub trait RuntimeClipboardPort {
    /// Read current clipboard text. Returns `None` if unavailable or empty.
    fn get_text(&mut self) -> Option<String>;

    /// Write text to the clipboard.
    fn set_text(&mut self, text: &str) -> Result<(), String>;
}

/// Transient notification delivery for runtime-owned finalize actions.
pub trait RuntimeToastPort {
    /// Enqueue a toast for display.
    fn enqueue(&mut self, toast: ToastSpec);

    /// Convenience helper for constructing a `ToastSpec` inline.
    fn enqueue_message(
        &mut self,
        severity: ToastSeverity,
        message: impl Into<String>,
        duration: Option<Duration>,
    ) {
        self.enqueue(ToastSpec {
            severity,
            message: message.into(),
            duration,
        });
    }
}

/// Composite bound for the portable port subset `GraphshellRuntime::tick`
/// actually uses today.
pub trait RuntimeTickPorts: RuntimeClipboardPort + RuntimeToastPort {}

impl<T> RuntimeTickPorts for T where T: RuntimeClipboardPort + RuntimeToastPort {}
