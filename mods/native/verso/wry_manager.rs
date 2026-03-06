/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Wry manager scaffold.
//!
//! First slice responsibilities:
//! - centralize ownership over Wry lifecycle records,
//! - provide a compile-safe API for compositor/lifecycle wiring,
//! - avoid direct `wry::WebView` plumbing until the next milestone.

use std::collections::HashMap;

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

#[derive(Debug, Default, Clone, PartialEq)]
struct WebviewSlot {
    last_sync: Option<OverlaySyncState>,
}

#[derive(Debug)]
pub(crate) struct WryManager {
    platform: WryPlatform,
    default_mode: WryRenderMode,
    webviews: HashMap<u64, WebviewSlot>,
}

impl WryManager {
    pub(crate) fn new() -> Self {
        let platform = WryPlatform::detect();
        Self {
            default_mode: WryRenderMode::for_platform(platform),
            platform,
            webviews: HashMap::new(),
        }
    }

    pub(crate) fn platform(&self) -> WryPlatform {
        self.platform
    }

    pub(crate) fn default_mode(&self) -> WryRenderMode {
        self.default_mode
    }

    pub(crate) fn create_webview(&mut self, node_id: u64) {
        self.webviews.entry(node_id).or_default();
    }

    pub(crate) fn destroy_webview(&mut self, node_id: u64) {
        self.webviews.remove(&node_id);
    }

    pub(crate) fn has_webview(&self, node_id: u64) -> bool {
        self.webviews.contains_key(&node_id)
    }

    pub(crate) fn sync_overlay(&mut self, node_id: u64, rect: OverlayRect, visible: bool) {
        if let Some(slot) = self.webviews.get_mut(&node_id) {
            slot.last_sync = Some(OverlaySyncState { rect, visible });
        }
    }

    pub(crate) fn last_sync_state(&self, node_id: u64) -> Option<OverlaySyncState> {
        self.webviews.get(&node_id).and_then(|slot| slot.last_sync)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_and_destroy_webview_slot() {
        let mut manager = WryManager::new();
        manager.create_webview(7);
        assert!(manager.has_webview(7));
        manager.destroy_webview(7);
        assert!(!manager.has_webview(7));
    }

    #[test]
    fn sync_overlay_records_last_state() {
        let mut manager = WryManager::new();
        manager.create_webview(11);
        manager.sync_overlay(
            11,
            OverlayRect {
                x: 10.0,
                y: 20.0,
                width: 300.0,
                height: 200.0,
            },
            true,
        );

        let state = manager
            .last_sync_state(11)
            .expect("expected recorded sync state");
        assert!(state.visible);
        assert_eq!(state.rect.width, 300.0);
        assert_eq!(state.rect.height, 200.0);
    }
}
