/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Host capability ports — the service interfaces `GraphshellRuntime` uses to
//! drive whatever host (egui, iced) is presenting its output.
//!
//! 2026-04-25 servo-into-verso S3a moved the trait definitions into the
//! `graphshell-runtime` crate so the iced launch path can implement them
//! without depending on Servo-coupled modules. This file is now a thin
//! shell-side re-export shim that preserves the existing `host_ports::*`
//! import paths used throughout the egui-host pipeline, plus a handful of
//! Servo-specific compatibility helpers (e.g., `HostAccessibilityPort`
//! injection wired through `servo::WebViewId`).
//!
//! See the trait docs in `graphshell_runtime::ports` for the canonical
//! definitions.

// Trait surface lives in graphshell-runtime per the 2026-04-25 S3a
// extraction. Shell-side imports use the legacy aliases so existing
// `host_ports::*` call sites keep working with no churn. New call
// sites should prefer importing from `graphshell_runtime::ports`
// directly.
pub(crate) use graphshell_runtime::ports::{
    HostAccessibilityPort, HostInputPort, HostPaintPort, HostSurfacePort, HostTexturePort,
    RuntimeClipboardPort as HostClipboardPort, RuntimeToastPort as HostToastPort,
};
#[allow(unused_imports)]
pub(crate) use graphshell_runtime::ports::{BackendViewportInPixels, HostPorts, ViewerSurfaceId};

/// Servo-specific extension trait for accesskit tree-update
/// injection.
///
/// Lives in graphshell main (not graphshell-runtime) because it's
/// keyed on `servo::WebViewId` directly — the egui-host's accesskit
/// anchor derivation has been WebViewId-shaped since M3, and
/// switching it to a host-neutral key is future architectural work.
/// For now, gating the injection trait behind `servo-engine` keeps
/// the runtime crate Servo-free while preserving the existing egui
/// pipeline.
#[cfg(feature = "servo-engine")]
pub(crate) trait ServoAccessibilityInjectionPort {
    /// Inject an accessibility tree update received from Servo's
    /// accesskit stream for a particular webview.
    fn inject_tree_update(
        &mut self,
        webview_id: verso::servo_engine::WebViewId,
        update: verso::servo_engine::accesskit::TreeUpdate,
    );
}
