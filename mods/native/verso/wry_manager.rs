/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Wry manager — owns real `wry::WebView` instances for NativeOverlay panes.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use raw_window_handle::RawWindowHandle;

#[cfg(target_os = "windows")]
use super::wry_frame_source::WryFrameMetadata;
use super::wry_frame_source::{WryFrameSource, WryFrameState};
use super::wry_types::{WryCompositedTextureSupport, WryPlatform, WryRenderMode};
#[cfg(target_os = "windows")]
use image::ImageFormat;
#[cfg(target_os = "windows")]
use webview2_com::{
    CapturePreviewCompletedHandler,
    Microsoft::Web::WebView2::Win32::COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG,
};
#[cfg(target_os = "windows")]
use windows::Win32::{
    Foundation::HGLOBAL,
    System::{
        Com::STREAM_SEEK_SET,
        Com::StructuredStorage::{CreateStreamOnHGlobal, GetHGlobalFromStream},
        Memory::{GlobalLock, GlobalSize, GlobalUnlock},
    },
};
#[cfg(target_os = "windows")]
use wry::WebViewExtWindows;

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
    /// The URL most recently sent to the webview (initial or via navigate).
    last_url: String,
}

pub(crate) struct WryManager {
    platform: WryPlatform,
    default_mode: WryRenderMode,
    webviews: HashMap<u64, WebviewSlot>,
    frame_source: WryFrameSource,
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
            frame_source: WryFrameSource::new(),
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

    pub(crate) fn composited_texture_support(&self) -> WryCompositedTextureSupport {
        WryCompositedTextureSupport::for_platform(self.platform)
    }

    pub(crate) fn frame_state_for_node(&self, node_id: u64) -> Option<&WryFrameState> {
        self.frame_source.state_for_node(node_id)
    }

    pub(crate) fn frame_png_bytes_for_node(&self, node_id: u64) -> Option<&[u8]> {
        self.frame_source.png_bytes_for_node(node_id)
    }

    pub(crate) fn refresh_frame_state_for_node(&mut self, node_id: u64) {
        self.refresh_frame_state_for_node_impl(node_id, None);
    }

    pub(crate) fn refresh_frame_state_for_node_if_stale(
        &mut self,
        node_id: u64,
        min_interval: Duration,
    ) -> bool {
        self.refresh_frame_state_for_node_impl(node_id, Some(min_interval))
    }

    fn refresh_frame_state_for_node_impl(
        &mut self,
        node_id: u64,
        min_interval: Option<Duration>,
    ) -> bool {
        let now = Instant::now();
        if let (Some(existing), Some(min_interval)) =
            (self.frame_source.state_for_node(node_id), min_interval)
            && !existing.should_refresh(now, min_interval)
        {
            return false;
        }

        let support = self.composited_texture_support();
        let mut state = if support.supported {
            #[cfg(target_os = "windows")]
            {
                if let Some(slot) = self.webviews.get(&node_id) {
                    match capture_webview_preview_png(slot) {
                        Ok((png_bytes, metadata)) => {
                            self.frame_source.set_png_bytes_for_node(node_id, png_bytes);
                            WryFrameState::ready(support.preferred_backend, metadata)
                        }
                        Err(error) => {
                            self.frame_source.clear_png_bytes_for_node(node_id);
                            WryFrameState::failed(
                                support.preferred_backend,
                                format!("CapturePreview probe failed: {error}"),
                            )
                        }
                    }
                } else {
                    self.frame_source.clear_png_bytes_for_node(node_id);
                    WryFrameState::pending(
                        support.preferred_backend,
                        "Frame capture backend is available; waiting for a live WebView instance.",
                    )
                }
            }
            #[cfg(not(target_os = "windows"))]
            {
                WryFrameState::pending(
                    support.preferred_backend,
                    "Frame capture backend declared but this platform probe is not wired yet.",
                )
            }
        } else {
            self.frame_source.clear_png_bytes_for_node(node_id);
            WryFrameState::unsupported(support.preferred_backend, support.reason)
        };
        state.mark_refreshed(now);
        self.frame_source.set_state_for_node(node_id, state);
        true
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
                        last_url: url.to_string(),
                    },
                );
                self.frame_source.register_node(
                    node_id,
                    WryFrameState::pending(
                        self.composited_texture_support().preferred_backend,
                        "WebView created; composited frame capture not wired yet.",
                    ),
                );
                self.refresh_frame_state_for_node(node_id);
            }
            Err(e) => {
                log::warn!("wry: failed to create WebView for node {node_id}: {e}");
            }
        }
    }

    pub(crate) fn destroy_webview(&mut self, node_id: u64) {
        self.webviews.remove(&node_id);
        self.frame_source.unregister_node(node_id);
    }

    pub(crate) fn has_webview(&self, node_id: u64) -> bool {
        self.webviews.contains_key(&node_id)
    }

    /// Returns the URL most recently loaded (or navigated to) in the webview,
    /// or `None` if no webview exists for this node.
    pub(crate) fn last_url(&self, node_id: u64) -> Option<&str> {
        self.webviews
            .get(&node_id)
            .map(|slot| slot.last_url.as_str())
    }

    /// Navigate an existing wry WebView to a new URL.
    ///
    /// No-ops if no webview exists for `node_id` (caller should call
    /// `ensure_wry_overlay_for_node` first).
    pub(crate) fn navigate_webview(&mut self, node_id: u64, url: &str) {
        let Some(slot) = self.webviews.get_mut(&node_id) else {
            return;
        };
        if slot.last_url == url {
            return;
        }
        if let Err(e) = slot.webview.load_url(url) {
            log::warn!("wry: navigate failed for node {node_id}: {e}");
            return;
        }
        slot.last_url = url.to_string();
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

#[cfg(target_os = "windows")]
fn capture_webview_preview_png(slot: &WebviewSlot) -> Result<(Vec<u8>, WryFrameMetadata), String> {
    let controller = slot.webview.controller();
    let webview = unsafe { controller.CoreWebView2() }.map_err(|error| error.to_string())?;
    let stream = unsafe { CreateStreamOnHGlobal(HGLOBAL::default(), true) }
        .map_err(|error| error.to_string())?;

    CapturePreviewCompletedHandler::wait_for_async_operation(
        Box::new({
            let stream = stream.clone();
            move |handler| unsafe {
                webview
                    .CapturePreview(
                        COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG,
                        &stream,
                        &handler,
                    )
                    .map_err(webview2_com::Error::WindowsError)
            }
        }),
        Box::new(|result| {
            result?;
            Ok(())
        }),
    )
    .map_err(|error| error.to_string())?;

    let png_bytes = read_stream_png_bytes(&stream)?;
    let metadata = decode_png_metadata(&png_bytes)?;
    Ok((png_bytes, metadata))
}

#[cfg(target_os = "windows")]
fn read_stream_png_bytes(stream: &windows::Win32::System::Com::IStream) -> Result<Vec<u8>, String> {
    unsafe {
        stream
            .Seek(0, STREAM_SEEK_SET, None)
            .map_err(|error| error.to_string())?;
    }
    let hglobal = unsafe { GetHGlobalFromStream(stream) }.map_err(|error| error.to_string())?;
    let size = unsafe { GlobalSize(hglobal) };
    if size == 0 {
        return Err("CapturePreview completed with an empty stream.".to_string());
    }
    let data = unsafe { GlobalLock(hglobal) };
    if data.is_null() {
        return Err("GlobalLock returned null for captured preview stream.".to_string());
    }
    let bytes = unsafe { std::slice::from_raw_parts(data as *const u8, size).to_vec() };
    unsafe {
        let _ = GlobalUnlock(hglobal);
    }
    Ok(bytes)
}

#[cfg(target_os = "windows")]
fn decode_png_metadata(png_bytes: &[u8]) -> Result<WryFrameMetadata, String> {
    let image = image::load_from_memory_with_format(png_bytes, ImageFormat::Png)
        .map_err(|error| error.to_string())?;
    Ok(WryFrameMetadata {
        width: image.width(),
        height: image.height(),
        revision: 1,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mods::native::verso::wry_types::WryPlatform;

    /// Smoke test: WryManager is constructable and reports no webviews.
    #[test]
    fn new_manager_has_no_webviews() {
        let manager = WryManager::new();
        assert!(!manager.has_webview(0));
    }

    #[test]
    fn composited_texture_support_reports_current_platform_state() {
        let manager = WryManager::new();
        let support = manager.composited_texture_support();
        assert!(!support.reason.is_empty());
        if manager.platform() == WryPlatform::Windows {
            assert!(support.supported);
        }
    }

    #[test]
    fn frame_state_is_absent_for_unknown_node() {
        let manager = WryManager::new();
        assert!(manager.frame_state_for_node(77).is_none());
    }

    #[test]
    fn refresh_frame_state_populates_unsupported_state() {
        let mut manager = WryManager::new();
        manager.frame_source.register_node(
            8,
            WryFrameState::pending(
                manager.composited_texture_support().preferred_backend,
                "placeholder",
            ),
        );
        manager.refresh_frame_state_for_node(8);
        let state = manager
            .frame_state_for_node(8)
            .expect("frame state should exist after refresh");
        assert!(!state.message.is_empty());
    }

    #[test]
    fn frame_png_bytes_are_absent_for_unknown_node() {
        let manager = WryManager::new();
        assert!(manager.frame_png_bytes_for_node(77).is_none());
    }
}
