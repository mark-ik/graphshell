/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Verso-managed host routing vocabulary and host-owned adapters.
//!
//! This crate is the home for host-specific integration code that should not
//! stay in the portable app layer. It can own concrete dependencies such as
//! Servo and Tokio while still exposing narrower host seams back to the root
//! application crate.

pub mod async_spawner;
pub mod renderer;
pub mod viewer_surface_host;

use graphshell_core::content::ViewerInstanceId;
use serde::{Deserialize, Serialize};
use verso::{HostCapabilities, VersoResolvedRoute, ViewerHandle, WebEnginePreference};

pub use async_spawner::TokioAsyncSpawner;
pub use renderer::RendererId;
pub use viewer_surface_host::{
    NoopViewerSurfaceHost, ServoViewerSurfaceHost, ViewerSurfaceRegistryHost,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HostEngine {
    Middlenet,
    Servo,
    Wry,
}

impl HostEngine {
    pub const fn viewer_handle(self) -> ViewerHandle {
        match self {
            Self::Middlenet => ViewerHandle::Middlenet,
            Self::Servo => ViewerHandle::Webview,
            Self::Wry => ViewerHandle::Wry,
        }
    }

    pub const fn from_viewer_handle(handle: ViewerHandle) -> Self {
        match handle {
            ViewerHandle::Middlenet => Self::Middlenet,
            ViewerHandle::Webview => Self::Servo,
            ViewerHandle::Wry => Self::Wry,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostSurfaceBinding {
    pub renderer_id: ViewerInstanceId,
    pub route: VersoResolvedRoute,
}

impl HostSurfaceBinding {
    pub fn engine(&self) -> HostEngine {
        HostEngine::from_viewer_handle(self.route.handle())
    }
}

pub trait VersoHostIntegration {
    fn capabilities(&self) -> HostCapabilities;

    fn preferred_web_engine(&self) -> WebEnginePreference {
        if self.capabilities().supports_servo {
            WebEnginePreference::Servo
        } else {
            WebEnginePreference::Wry
        }
    }

    fn supports_engine(&self, engine: HostEngine) -> bool {
        let caps = self.capabilities();
        match engine {
            HostEngine::Middlenet => {
                caps.supports_middlenet_direct
                    || caps.supports_middlenet_html
                    || caps.supports_middlenet_faithful_source
            }
            HostEngine::Servo => caps.supports_servo,
            HostEngine::Wry => caps.supports_wry,
        }
    }

    fn supports_route(&self, route: &VersoResolvedRoute) -> bool {
        self.supports_engine(HostEngine::from_viewer_handle(route.handle()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use verso::{EngineChoice, VersoPaneOwner, VersoResolvedRoute, VersoRouteReason};

    struct StubHost {
        caps: HostCapabilities,
    }

    impl VersoHostIntegration for StubHost {
        fn capabilities(&self) -> HostCapabilities {
            self.caps.clone()
        }
    }

    fn route(handle: ViewerHandle, engine: EngineChoice) -> VersoResolvedRoute {
        VersoResolvedRoute {
            handle,
            engine,
            middlenet_lane: None,
            reason: VersoRouteReason::Unsupported,
            owner: VersoPaneOwner::Policy,
        }
    }

    #[test]
    fn host_engine_maps_viewer_handles() {
        assert_eq!(HostEngine::from_viewer_handle(ViewerHandle::Middlenet), HostEngine::Middlenet);
        assert_eq!(HostEngine::from_viewer_handle(ViewerHandle::Webview), HostEngine::Servo);
        assert_eq!(HostEngine::from_viewer_handle(ViewerHandle::Wry), HostEngine::Wry);
    }

    #[test]
    fn supports_route_checks_matching_backend_capability() {
        let host = StubHost {
            caps: HostCapabilities {
                supports_middlenet_direct: true,
                supports_middlenet_html: false,
                supports_middlenet_faithful_source: false,
                supports_servo: false,
                supports_wry: true,
            },
        };

        assert!(host.supports_route(&route(ViewerHandle::Middlenet, EngineChoice::Middlenet)));
        assert!(host.supports_route(&route(ViewerHandle::Wry, EngineChoice::Wry)));
        assert!(!host.supports_route(&route(ViewerHandle::Webview, EngineChoice::Servo)));
    }
}