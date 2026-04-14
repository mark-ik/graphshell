/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Wry viewer scaffold.
//!
//! This adapter is intentionally minimal in the first slice and focuses on
//! overlay synchronization contracts with the manager.

use super::wry_manager::{OverlayRect, WryManager};
use super::wry_types::WryRenderMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WryViewer {
    pub node_id: u64,
    pub render_mode: WryRenderMode,
}

impl WryViewer {
    pub(crate) fn new(node_id: u64, render_mode: WryRenderMode) -> Self {
        Self {
            node_id,
            render_mode,
        }
    }

    pub(crate) fn is_overlay_mode(&self) -> bool {
        matches!(self.render_mode, WryRenderMode::NativeOverlay)
    }

    pub(crate) fn sync_overlay(&self, manager: &mut WryManager, rect: OverlayRect, visible: bool) {
        manager.sync_overlay(self.node_id, rect, visible);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mods::native::verso::wry_types::WryRenderMode;

    #[test]
    fn native_overlay_mode_reports_true() {
        let viewer = WryViewer::new(1, WryRenderMode::NativeOverlay);
        assert!(viewer.is_overlay_mode());
    }

    #[test]
    fn viewer_sync_updates_manager_state() {
        let mut manager = WryManager::new();

        let viewer = WryViewer::new(42, WryRenderMode::NativeOverlay);
        viewer.sync_overlay(
            &mut manager,
            OverlayRect {
                x: 5.0,
                y: 15.0,
                width: 100.0,
                height: 50.0,
            },
            false,
        );

        let state = manager
            .last_sync_state(42)
            .expect("expected sync state after viewer update");
        assert!(!state.visible);
        assert_eq!(state.rect.x, 5.0);
    }
}

