/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Wry runtime type scaffolds for the verso wry engine.
//!
//! These types intentionally stay lightweight in the first
//! implementation slice. Runtime ownership and compositor contracts
//! are defined in
//! `design_docs/graphshell_docs/implementation_strategy/viewer/wry_integration_spec.md`.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WryPlatform {
    Windows,
    MacOS,
    Linux,
    Other,
}

impl WryPlatform {
    pub fn detect() -> Self {
        if cfg!(target_os = "windows") {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::MacOS
        } else if cfg!(target_os = "linux") {
            Self::Linux
        } else {
            Self::Other
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WryRenderMode {
    NativeOverlay,
    CompositedTexture,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WryFrameCaptureBackend {
    None,
    WebView2VisualCapture,
    WkSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WryCompositedTextureSupport {
    pub preferred_backend: WryFrameCaptureBackend,
    pub supported: bool,
    pub reason: &'static str,
}

impl WryRenderMode {
    pub fn for_platform(platform: WryPlatform) -> Self {
        match platform {
            // Linux is currently NativeOverlay-only in the spec.
            WryPlatform::Linux => Self::NativeOverlay,
            // Windows/macOS default to NativeOverlay in initial implementation.
            WryPlatform::Windows | WryPlatform::MacOS | WryPlatform::Other => Self::NativeOverlay,
        }
    }
}

impl WryCompositedTextureSupport {
    pub fn for_platform(platform: WryPlatform) -> Self {
        match platform {
            WryPlatform::Windows => Self {
                preferred_backend: WryFrameCaptureBackend::WebView2VisualCapture,
                supported: true,
                reason: "Windows experimental preview capture is wired through WebView2 CapturePreview; compositor texture upload is still pending.",
            },
            WryPlatform::MacOS => Self {
                preferred_backend: WryFrameCaptureBackend::WkSnapshot,
                supported: false,
                reason: "macOS path not yet wired: no WKWebView snapshot-to-texture bridge is implemented.",
            },
            WryPlatform::Linux => Self {
                preferred_backend: WryFrameCaptureBackend::None,
                supported: false,
                reason: "Linux remains NativeOverlay-only in the current Wry integration contract.",
            },
            WryPlatform::Other => Self {
                preferred_backend: WryFrameCaptureBackend::None,
                supported: false,
                reason: "No composited Wry capture backend is defined for this platform.",
            },
        }
    }
}
