/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Wry composited-frame source skeleton.
//!
//! This module does not implement platform frame capture yet. It establishes
//! the runtime state shape that a future Windows/macOS capture bridge can
//! populate without overloading the overlay manager API.

use std::time::{Duration, Instant};

use super::wry_types::WryFrameCaptureBackend;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WryFrameAvailability {
    Unsupported,
    Pending,
    Ready,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WryFrameMetadata {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WryFrameState {
    pub(crate) availability: WryFrameAvailability,
    pub(crate) backend: WryFrameCaptureBackend,
    pub(crate) metadata: Option<WryFrameMetadata>,
    pub(crate) message: String,
    pub(crate) last_refresh_at: Option<Instant>,
}

impl WryFrameState {
    pub(crate) fn unsupported<M: Into<String>>(
        backend: WryFrameCaptureBackend,
        message: M,
    ) -> Self {
        Self {
            availability: WryFrameAvailability::Unsupported,
            backend,
            metadata: None,
            message: message.into(),
            last_refresh_at: None,
        }
    }

    pub(crate) fn pending<M: Into<String>>(backend: WryFrameCaptureBackend, message: M) -> Self {
        Self {
            availability: WryFrameAvailability::Pending,
            backend,
            metadata: None,
            message: message.into(),
            last_refresh_at: None,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn ready(backend: WryFrameCaptureBackend, metadata: WryFrameMetadata) -> Self {
        Self {
            availability: WryFrameAvailability::Ready,
            backend,
            metadata: Some(metadata),
            message: "Frame available.".to_string(),
            last_refresh_at: None,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn failed<M: Into<String>>(backend: WryFrameCaptureBackend, message: M) -> Self {
        Self {
            availability: WryFrameAvailability::Failed,
            backend,
            metadata: None,
            message: message.into(),
            last_refresh_at: None,
        }
    }

    pub(crate) fn mark_refreshed(&mut self, refreshed_at: Instant) {
        self.last_refresh_at = Some(refreshed_at);
    }

    pub(crate) fn should_refresh(&self, now: Instant, min_interval: Duration) -> bool {
        self.last_refresh_at
            .map(|last| now.saturating_duration_since(last) >= min_interval)
            .unwrap_or(true)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WryFrameSource {
    state_by_node: std::collections::HashMap<u64, WryFrameState>,
    png_bytes_by_node: std::collections::HashMap<u64, Vec<u8>>,
}

impl WryFrameSource {
    pub(crate) fn new() -> Self {
        Self {
            state_by_node: std::collections::HashMap::new(),
            png_bytes_by_node: std::collections::HashMap::new(),
        }
    }

    pub(crate) fn register_node(&mut self, node_id: u64, initial_state: WryFrameState) {
        self.state_by_node.insert(node_id, initial_state);
    }

    pub(crate) fn unregister_node(&mut self, node_id: u64) {
        self.state_by_node.remove(&node_id);
        self.png_bytes_by_node.remove(&node_id);
    }

    pub(crate) fn state_for_node(&self, node_id: u64) -> Option<&WryFrameState> {
        self.state_by_node.get(&node_id)
    }

    pub(crate) fn set_state_for_node(&mut self, node_id: u64, state: WryFrameState) {
        if let Some(slot) = self.state_by_node.get_mut(&node_id) {
            *slot = state;
        }
    }

    pub(crate) fn png_bytes_for_node(&self, node_id: u64) -> Option<&[u8]> {
        self.png_bytes_by_node.get(&node_id).map(Vec::as_slice)
    }

    pub(crate) fn set_png_bytes_for_node(&mut self, node_id: u64, png_bytes: Vec<u8>) {
        if self.state_by_node.contains_key(&node_id) {
            self.png_bytes_by_node.insert(node_id, png_bytes);
        }
    }

    pub(crate) fn clear_png_bytes_for_node(&mut self, node_id: u64) {
        self.png_bytes_by_node.remove(&node_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_source_registers_and_unregisters_nodes() {
        let mut source = WryFrameSource::new();
        source.register_node(
            7,
            WryFrameState::pending(
                WryFrameCaptureBackend::WebView2VisualCapture,
                "waiting for first frame",
            ),
        );
        assert!(source.state_for_node(7).is_some());
        source.unregister_node(7);
        assert!(source.state_for_node(7).is_none());
    }

    #[test]
    fn frame_source_tracks_png_bytes_per_node() {
        let mut source = WryFrameSource::new();
        source.register_node(
            9,
            WryFrameState::pending(
                WryFrameCaptureBackend::WebView2VisualCapture,
                "waiting for first frame",
            ),
        );
        source.set_png_bytes_for_node(9, vec![1, 2, 3, 4]);
        assert_eq!(source.png_bytes_for_node(9), Some(&[1, 2, 3, 4][..]));
        source.clear_png_bytes_for_node(9);
        assert!(source.png_bytes_for_node(9).is_none());
    }

    #[test]
    fn frame_state_refresh_policy_respects_min_interval() {
        let mut state = WryFrameState::pending(
            WryFrameCaptureBackend::WebView2VisualCapture,
            "waiting for first frame",
        );
        let base = Instant::now();
        assert!(state.should_refresh(base, Duration::from_secs(30)));
        state.mark_refreshed(base);
        assert!(!state.should_refresh(base + Duration::from_secs(5), Duration::from_secs(30)));
        assert!(state.should_refresh(base + Duration::from_secs(31), Duration::from_secs(30)));
    }
}

