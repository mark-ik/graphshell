/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Iced ↔ wry overlay bridge.
//!
//! Mounts `wry::WebView` instances as children of the iced window
//! per [VERSO_AS_PEER.md](../../../design_docs/verso_docs/technical_architecture/VERSO_AS_PEER.md)'s
//! `viewer:wry` overlay model. Owns the wry [`WryManager`] from
//! [`verso::wry_engine`] and provides:
//!
//! - **`request_window_handle(id) -> Task<...>`** — async helper
//!   that extracts a `RawWindowHandle` from an iced window id.
//!   Wraps `iced::runtime::window::run(id, |&dyn Window| ...)`
//!   so callers don't have to thread iced's runtime types
//!   through their app logic.
//! - **`WryHost`** — host-side state container holding the
//!   [`WryManager`] plus a remembered window handle. Methods:
//!   `mount`, `unmount`, `sync_overlay`, `navigate`, `last_url`.
//!
//! **Why this is a side-effect manager and not a `canvas::Program`
//! widget**: wry overlays are native OS WebView windows positioned
//! over the iced window — they are not rendered through iced's
//! widget tree. The iced application owns layout (where to put the
//! overlay rect) and tells wry directly where to paint via
//! [`WryHost::sync_overlay`]. Future slices can grow a custom iced
//! widget that exposes its laid-out rect to the host so positioning
//! is automatic.

use std::cell::OnceCell;

use iced::Task;
use iced::window::{self as iced_window, Window};
use raw_window_handle::RawWindowHandle;
// `HasWindowHandle` is brought into scope as a trait inside
// `request_window_handle` via the iced `Window` blanket impl, but
// we don't need it elsewhere — see that fn body for the call.

/// Send-asserting wrapper around [`RawWindowHandle`].
///
/// `RawWindowHandle` is `Copy` but not `Send` because some platform
/// variants embed raw pointers (`*mut c_void`-style) that the
/// raw-window-handle crate conservatively does not mark as Send.
///
/// **Why this is sound here**: iced's `Task` delivery always lands
/// on the main thread (the only thread that owns iced's
/// application + winit window). `wry::WebView` is `!Send` on
/// Windows/macOS, so callers will only ever construct WebViews on
/// the main thread anyway. The `RawWindowHandle` we ferry through
/// the Task channel is just a numeric handle to an OS-owned
/// window — moving the *bits* between threads is safe; what would
/// be unsafe is *operating on the window* off-thread, which we
/// don't.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SendableWindowHandle(pub RawWindowHandle);

// SAFETY: see the type-level docs above. The handle is OS-owned;
// transporting its bits across threads is safe. Operating on the
// associated window off-thread is not, and the consumers in this
// crate don't do that.
unsafe impl Send for SendableWindowHandle {}
unsafe impl Sync for SendableWindowHandle {}

pub use verso::wry_engine::manager::{OverlayRect, OverlaySyncState, WryManager};
pub use verso::wry_engine::types::{
    WryCompositedTextureSupport, WryFrameCaptureBackend, WryPlatform, WryRenderMode,
};

/// Result returned by [`request_window_handle`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowHandleOutcome {
    /// Successfully extracted the raw window handle. Wrapped in
    /// [`SendableWindowHandle`] so the value can flow through
    /// iced's `Task` channel (which requires `Send + 'static`).
    Got(SendableWindowHandle),
    /// iced couldn't supply a handle for this window id (window
    /// closed, never opened, or the underlying winit handle was
    /// unavailable). Caller may retry on next frame.
    Unavailable,
}

/// Build an iced [`Task`] that asks iced for the raw window handle
/// of `window_id`. The task resolves to a [`WindowHandleOutcome`];
/// the iced app's `update` function receives the result and can
/// pass it to [`WryHost::set_window_handle`] before mounting any
/// webviews.
///
/// Wraps [`iced::runtime::window::run`] (the public `Task<T>`
/// helper that lets a closure see the iced window's `&dyn Window`
/// implementor) and extracts a `RawWindowHandle` via
/// [`raw_window_handle::HasWindowHandle::window_handle`].
pub fn request_window_handle(window_id: iced::window::Id) -> Task<WindowHandleOutcome> {
    // `window.window_handle()` resolves through the `iced::window::Window`
    // trait's `HasWindowHandle` supertrait — no extra import needed.
    iced_window::run(window_id, |window: &dyn Window| {
        match window.window_handle() {
            Ok(handle) => WindowHandleOutcome::Got(SendableWindowHandle(handle.as_raw())),
            Err(_) => WindowHandleOutcome::Unavailable,
        }
    })
}

/// Host-side container the iced app owns to drive wry overlays.
///
/// Mirrors graphshell main's `mods/native/web_runtime` thread-local
/// `WRY_MANAGER` pattern, but reorganized for iced:
///
/// - The window handle is captured once via
///   [`set_window_handle`](Self::set_window_handle) (typically from
///   a `WindowHandleOutcome` task result) and reused for every
///   subsequent `mount` call until the window is destroyed.
/// - Method-style API (`mount`, `unmount`, `sync_overlay`) replaces
///   the per-node free functions in graphshell main since iced apps
///   own their `WryHost` instance directly rather than reaching
///   into a thread-local.
///
/// Not `Send` because `wry::WebView` is `!Send` on Windows/macOS
/// (COM/Obj-C constraints); iced apps drive this from the main
/// thread anyway.
pub struct WryHost {
    manager: WryManager,
    parent_handle: OnceCell<RawWindowHandle>,
}

impl WryHost {
    pub fn new() -> Self {
        Self {
            manager: WryManager::new(),
            parent_handle: OnceCell::new(),
        }
    }

    /// Capture the parent window handle that future `mount` calls
    /// will use as the WebView parent. Subsequent calls are
    /// silently ignored (the cell is set-once); restart the host
    /// to re-bind to a new window.
    pub fn set_window_handle(&self, handle: RawWindowHandle) {
        let _ = self.parent_handle.set(handle);
    }

    /// Whether a parent window handle has been installed.
    pub fn has_window_handle(&self) -> bool {
        self.parent_handle.get().is_some()
    }

    /// Apply a [`WindowHandleOutcome`] returned from a
    /// [`request_window_handle`] task. Returns `true` if the host
    /// now has a handle (either newly set or already present),
    /// `false` if the outcome was `Unavailable`.
    pub fn apply_window_handle_outcome(&self, outcome: WindowHandleOutcome) -> bool {
        match outcome {
            WindowHandleOutcome::Got(SendableWindowHandle(handle)) => {
                self.set_window_handle(handle);
                true
            }
            WindowHandleOutcome::Unavailable => self.has_window_handle(),
        }
    }

    /// Mount a wry WebView for `node_id` at `url`, positioned at
    /// `rect`. Requires the parent window handle to have been
    /// installed via [`set_window_handle`] first; returns `false`
    /// otherwise.
    pub fn mount(&mut self, node_id: u64, url: &str, rect: OverlayRect) -> bool {
        let Some(handle) = self.parent_handle.get().copied() else {
            log::warn!(
                "iced-wry-viewer: mount({node_id}, {url}) skipped — no parent window handle yet"
            );
            return false;
        };
        if !self.manager.has_webview(node_id) {
            self.manager.create_webview(node_id, url, handle);
        }
        self.manager.sync_overlay(node_id, rect, true);
        true
    }

    /// Update an existing overlay's bounds + visibility. No-op
    /// when the node has no webview.
    pub fn sync_overlay(&mut self, node_id: u64, rect: OverlayRect, visible: bool) {
        self.manager.sync_overlay(node_id, rect, visible);
    }

    /// Hide an overlay without destroying its WebView. The
    /// WebView keeps its loaded page; calling [`sync_overlay`]
    /// with `visible: true` makes it reappear.
    pub fn hide(&mut self, node_id: u64) -> bool {
        if !self.manager.has_webview(node_id) {
            return false;
        }
        if let Some(state) = self.manager.last_sync_state(node_id) {
            self.manager.sync_overlay(node_id, state.rect, false);
        }
        true
    }

    /// Destroy a webview entirely. The slot is removed from the
    /// underlying [`WryManager`]; a future [`mount`] for the same
    /// node id starts a fresh WebView.
    pub fn unmount(&mut self, node_id: u64) -> bool {
        if !self.manager.has_webview(node_id) {
            return false;
        }
        self.manager.destroy_webview(node_id);
        true
    }

    /// Navigate an existing overlay to a new URL. No-op when the
    /// node has no webview; caller should [`mount`] first.
    pub fn navigate(&mut self, node_id: u64, url: &str) {
        self.manager.navigate_webview(node_id, url);
    }

    /// URL most recently loaded in the overlay for `node_id`.
    pub fn last_url(&self, node_id: u64) -> Option<&str> {
        self.manager.last_url(node_id)
    }

    /// Last sync state (rect + visibility) recorded for `node_id`.
    pub fn last_sync_state(&self, node_id: u64) -> Option<OverlaySyncState> {
        self.manager.last_sync_state(node_id)
    }

    /// Whether a webview currently exists for `node_id`.
    pub fn has_webview(&self, node_id: u64) -> bool {
        self.manager.has_webview(node_id)
    }

    /// Direct access to the underlying [`WryManager`] for
    /// advanced use (frame capture, render mode probing). Most
    /// callers should use the per-node methods above.
    pub fn manager(&self) -> &WryManager {
        &self.manager
    }

    /// Mutable access. See [`manager`](Self::manager) for the
    /// shared-reference accessor.
    pub fn manager_mut(&mut self) -> &mut WryManager {
        &mut self.manager
    }
}

impl Default for WryHost {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_host_has_no_window_handle_or_webviews() {
        let host = WryHost::new();
        assert!(!host.has_window_handle());
        assert!(!host.has_webview(0));
    }

    #[test]
    fn mount_without_handle_logs_and_returns_false() {
        let mut host = WryHost::new();
        let ok = host.mount(
            7,
            "https://example.com/",
            OverlayRect {
                x: 10.0,
                y: 20.0,
                width: 100.0,
                height: 50.0,
            },
        );
        assert!(!ok, "mount should refuse without a parent handle");
        assert!(!host.has_webview(7));
    }

    #[test]
    fn apply_unavailable_outcome_keeps_handle_state() {
        let host = WryHost::new();
        assert!(!host.apply_window_handle_outcome(WindowHandleOutcome::Unavailable));
        assert!(!host.has_window_handle());
    }

    /// Sync-overlay against a node with no live webview is a no-op.
    /// The wry manager's test-only side table that captured these
    /// in graphshell main is `#[cfg(test)]` and only available
    /// inside verso's own test cycle; cross-crate tests assert the
    /// "no panic, no state recorded" shape instead.
    #[test]
    fn sync_overlay_without_webview_is_noop() {
        let mut host = WryHost::new();
        let rect = OverlayRect {
            x: 5.0,
            y: 15.0,
            width: 200.0,
            height: 100.0,
        };
        host.sync_overlay(11, rect, true);
        // No webview was mounted, so there's no slot to query.
        assert!(host.last_sync_state(11).is_none());
        assert!(!host.has_webview(11));
    }

    #[test]
    fn unmount_returns_false_when_node_has_no_webview() {
        let mut host = WryHost::new();
        assert!(!host.unmount(99));
    }

    #[test]
    fn hide_returns_false_when_node_has_no_webview() {
        let mut host = WryHost::new();
        assert!(!host.hide(99));
    }

    #[test]
    fn manager_accessors_expose_the_underlying_wry_manager() {
        let mut host = WryHost::new();
        assert!(!host.manager().has_webview(0));
        assert!(host.manager_mut().last_url(0).is_none());
    }
}
