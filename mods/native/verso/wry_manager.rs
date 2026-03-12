/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Wry manager — owns real `wry::WebView` instances for NativeOverlay panes.

use std::collections::HashMap;

use raw_window_handle::RawWindowHandle;

use super::wry_types::{WryPlatform, WryRenderMode};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct OverlayRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct OverlaySyncState {
    pub rect: OverlayRect,
    pub visible: bool,
}

struct WebviewSlot {
    webview: wry::WebView,
    last_sync: Option<OverlaySyncState>,
}

pub(crate) struct WryManager {
    platform: WryPlatform,
    default_mode: WryRenderMode,
    webviews: HashMap<u64, WebviewSlot>,
    #[cfg(test)]
    test_sync_states: HashMap<u64, OverlaySyncState>,
}

impl WryManager {
    pub(crate) fn new() -> Self {
        let platform = WryPlatform::detect();
        Self {
            default_mode: WryRenderMode::for_platform(platform),
            platform,
            webviews: HashMap::new(),
            #[cfg(test)]
            test_sync_states: HashMap::new(),
        }
    }

    pub(crate) fn platform(&self) -> WryPlatform {
        self.platform
    }

    pub(crate) fn default_mode(&self) -> WryRenderMode {
        self.default_mode
    }

    /// Create a wry child WebView for the given node at the given URL.
    ///
    /// `parent_handle` is the OS window handle from
    /// `EmbedderWindow::raw_window_handle_for_child`.  The caller guarantees
    /// the underlying OS handle remains valid for the lifetime of this `WryManager`.
    ///
    /// The created WebView starts hidden; call `sync_overlay` to position and show it.
    pub(crate) fn create_webview(
        &mut self,
        node_id: u64,
        url: &str,
        parent_handle: RawWindowHandle,
    ) {
        if self.webviews.contains_key(&node_id) {
            return;
        }

        // SAFETY: The caller holds the OS window alive for the duration of this
        // WryManager (the winit window owns the HWND and lives for the app lifetime).
        let borrowed = unsafe { raw_window_handle::WindowHandle::borrow_raw(parent_handle) };

        let result = wry::WebViewBuilder::new()
            .with_url(url)
            .with_visible(false)
            .build_as_child(&borrowed);

        match result {
            Ok(webview) => {
                self.webviews.insert(
                    node_id,
                    WebviewSlot {
                        webview,
                        last_sync: None,
                    },
                );
            }
            Err(e) => {
                log::warn!("wry: failed to create WebView for node {node_id}: {e}");
            }
        }
    }

    pub(crate) fn destroy_webview(&mut self, node_id: u64) {
        self.webviews.remove(&node_id);
    }

    pub(crate) fn has_webview(&self, node_id: u64) -> bool {
        self.webviews.contains_key(&node_id)
    }

    pub(crate) fn sync_overlay(&mut self, node_id: u64, rect: OverlayRect, visible: bool) {
        let Some(slot) = self.webviews.get_mut(&node_id) else {
            #[cfg(test)]
            {
                self.test_sync_states
                    .insert(node_id, OverlaySyncState { rect, visible });
            }
            return;
        };
        slot.last_sync = Some(OverlaySyncState { rect, visible });

        let bounds = wry::Rect {
            position: wry::dpi::LogicalPosition::new(rect.x as f64, rect.y as f64).into(),
            size: wry::dpi::LogicalSize::new(rect.width as f64, rect.height as f64).into(),
        };
        if let Err(e) = slot.webview.set_bounds(bounds) {
            log::warn!("wry: set_bounds failed for node {node_id}: {e}");
        }
        if let Err(e) = slot.webview.set_visible(visible) {
            log::warn!("wry: set_visible({visible}) failed for node {node_id}: {e}");
        }
    }

    pub(crate) fn last_sync_state(&self, node_id: u64) -> Option<OverlaySyncState> {
        self.webviews
            .get(&node_id)
            .and_then(|slot| slot.last_sync)
            .or_else(|| {
                #[cfg(test)]
                {
                    return self.test_sync_states.get(&node_id).copied();
                }
                #[allow(unreachable_code)]
                None
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Smoke test: WryManager is constructable and reports no webviews.
    #[test]
    fn new_manager_has_no_webviews() {
        let manager = WryManager::new();
        assert!(!manager.has_webview(0));
    }
}
