/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use middlenet_core::source::{MiddleNetContentKind, MiddleNetSource};
use middlenet_engine::engine::{
    HostCapabilities as MiddleNetHostCapabilities, LaneDecision, LaneOverride, PreparedDocument,
};
use serde::{Deserialize, Serialize};

/// System-WebView (wry) backend, gated behind the `wry-engine`
/// Cargo feature. Re-exports the upstream `wry` crate so downstream
/// consumers (notably `iced-wry-viewer` and the future migrated
/// `wry_manager`) depend on `verso` rather than `wry` directly,
/// keeping the `viewer:wry` capability owned by verso per
/// [VERSO_AS_PEER.md](../../../design_docs/verso_docs/technical_architecture/VERSO_AS_PEER.md).
#[cfg(feature = "wry-engine")]
pub mod wry_engine {
    pub use wry::*;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EngineChoice {
    Middlenet,
    Servo,
    Wry,
    Unsupported,
}

impl EngineChoice {
    pub fn label(self) -> &'static str {
        match self {
            Self::Middlenet => "Middlenet",
            Self::Servo => "Servo",
            Self::Wry => "Wry",
            Self::Unsupported => "Unsupported",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EngineOverride {
    Middlenet(LaneOverride),
    Servo,
    Wry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WebEnginePreference {
    Servo,
    Wry,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostCapabilities {
    pub supports_middlenet_direct: bool,
    pub supports_middlenet_html: bool,
    pub supports_middlenet_faithful_source: bool,
    pub supports_servo: bool,
    pub supports_wry: bool,
}

impl Default for HostCapabilities {
    fn default() -> Self {
        Self {
            supports_middlenet_direct: true,
            supports_middlenet_html: false,
            supports_middlenet_faithful_source: true,
            supports_servo: false,
            supports_wry: false,
        }
    }
}

impl HostCapabilities {
    pub fn middlenet(&self) -> MiddleNetHostCapabilities {
        MiddleNetHostCapabilities {
            supports_direct_lane: self.supports_middlenet_direct,
            supports_html_lane: self.supports_middlenet_html,
            supports_faithful_source_lane: self.supports_middlenet_faithful_source,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersoRequest {
    pub source: MiddleNetSource,
}

impl VersoRequest {
    pub fn new(source: MiddleNetSource) -> Self {
        Self { source }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DispatchOutcome {
    pub engine: EngineChoice,
    pub middlenet_lane: Option<LaneDecision>,
}

/// Typed handle for the viewer backends verso can route to.
///
/// Replaces the previous `viewer_id: &'static str` fields scattered
/// across routing decisions. Consumers that need the canonical
/// registry id call [`ViewerHandle::as_viewer_id`]; consumers that
/// want to branch on the backend match the enum directly instead of
/// comparing string literals like `"viewer:wry"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ViewerHandle {
    /// `viewer:middlenet` — Middlenet's direct/html/faithful-source lanes.
    Middlenet,
    /// `viewer:webview` — Servo-backed web viewer.
    Webview,
    /// `viewer:wry` — Wry (platform WebView) compatibility viewer.
    Wry,
}

impl ViewerHandle {
    /// Canonical registry id for this handle.
    pub const fn as_viewer_id(self) -> &'static str {
        match self {
            Self::Middlenet => "viewer:middlenet",
            Self::Webview => "viewer:webview",
            Self::Wry => "viewer:wry",
        }
    }

    /// Parse a canonical registry id back into a handle. Returns
    /// `None` for ids that verso does not route (specialized
    /// non-web viewers like `viewer:plaintext`, `viewer:pdf`).
    pub fn from_viewer_id(viewer_id: &str) -> Option<Self> {
        match viewer_id {
            "viewer:middlenet" => Some(Self::Middlenet),
            "viewer:webview" => Some(Self::Webview),
            "viewer:wry" => Some(Self::Wry),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ViewerRoutingDecision {
    pub handle: ViewerHandle,
    pub engine: EngineChoice,
    pub middlenet_lane: Option<LaneDecision>,
}

impl ViewerRoutingDecision {
    pub fn viewer_id(&self) -> &'static str {
        self.handle.as_viewer_id()
    }
}

/// Ownership tag for a pane's routing decision. Separates policy
/// picks from user pins so downstream UX can explain "why" without
/// comparing viewer-id strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VersoPaneOwner {
    /// Routing was chosen by verso's policy logic (no user override).
    Policy,
    /// User explicitly pinned the viewer via override.
    UserPin,
    /// No resolved route (initial or unsupported state).
    Unresolved,
}

/// Why a particular engine/lane was selected. Surface in debug UI and
/// diagnostics. Complements `VersoResolvedRoute` — the reason is the
/// "explain this decision" field that was previously embedded in
/// free-form strings (`fallback_reason_for_node_pane`, etc.).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum VersoRouteReason {
    /// Content matched a Middlenet source and was routed to the
    /// chosen lane.
    MiddlenetLane(LaneDecision),
    /// Browser-managed content routed to the host's preferred web
    /// engine (preference matched availability).
    WebEnginePreferred(WebEnginePreference),
    /// Browser-managed content routed to a fallback engine because
    /// the preferred one is not supported on this host.
    WebEngineFallback {
        preferred: WebEnginePreference,
        used: EngineChoice,
    },
    /// The user explicitly pinned the viewer via override.
    UserOverride,
    /// No engine could handle this content on this host.
    Unsupported,
}

/// A complete routing decision paired with its reason and ownership.
/// The intended replacement for bare `viewer_id` string comparisons
/// scattered across the workbench — consumers ask the resolved route
/// typed questions (`is_wry()`, `engine()`, `reason()`) instead of
/// `==` against `"viewer:wry"`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersoResolvedRoute {
    pub handle: ViewerHandle,
    pub engine: EngineChoice,
    pub middlenet_lane: Option<LaneDecision>,
    pub reason: VersoRouteReason,
    pub owner: VersoPaneOwner,
}

impl VersoResolvedRoute {
    pub fn engine(&self) -> EngineChoice {
        self.engine
    }

    pub fn is_middlenet(&self) -> bool {
        matches!(self.engine, EngineChoice::Middlenet)
    }

    pub fn is_servo(&self) -> bool {
        matches!(self.engine, EngineChoice::Servo)
    }

    pub fn is_wry(&self) -> bool {
        matches!(self.engine, EngineChoice::Wry)
    }

    pub fn is_unsupported(&self) -> bool {
        matches!(self.engine, EngineChoice::Unsupported)
    }

    pub fn handle(&self) -> ViewerHandle {
        self.handle
    }

    pub fn viewer_id(&self) -> &'static str {
        self.handle.as_viewer_id()
    }

    pub fn reason(&self) -> &VersoRouteReason {
        &self.reason
    }

    pub fn owner(&self) -> VersoPaneOwner {
        self.owner
    }
}

impl DispatchOutcome {
    pub fn unsupported() -> Self {
        Self {
            engine: EngineChoice::Unsupported,
            middlenet_lane: None,
        }
    }

    pub fn middlenet(lane: LaneDecision) -> Self {
        Self {
            engine: EngineChoice::Middlenet,
            middlenet_lane: Some(lane),
        }
    }
}

pub fn dispatch_request(
    request: &VersoRequest,
    host_caps: &HostCapabilities,
    override_engine: Option<EngineOverride>,
) -> DispatchOutcome {
    if let Some(override_engine) = override_engine {
        match override_engine {
            EngineOverride::Middlenet(override_lane) => {
                let lane = middlenet_engine::engine::MiddleNetEngine::select_lane_for_source(
                    &request.source,
                    &host_caps.middlenet(),
                    Some(override_lane),
                );
                if lane != LaneDecision::Unsupported {
                    return DispatchOutcome::middlenet(lane);
                }
            }
            EngineOverride::Servo if host_caps.supports_servo => {
                return DispatchOutcome {
                    engine: EngineChoice::Servo,
                    middlenet_lane: None,
                };
            }
            EngineOverride::Wry if host_caps.supports_wry => {
                return DispatchOutcome {
                    engine: EngineChoice::Wry,
                    middlenet_lane: None,
                };
            }
            _ => {}
        }
    }

    match request.source.content_kind {
        MiddleNetContentKind::Html => {
            let middlenet_lane = middlenet_engine::engine::MiddleNetEngine::select_lane_for_source(
                &request.source,
                &host_caps.middlenet(),
                None,
            );
            if middlenet_lane == LaneDecision::Html {
                DispatchOutcome::middlenet(middlenet_lane)
            } else if host_caps.supports_servo {
                DispatchOutcome {
                    engine: EngineChoice::Servo,
                    middlenet_lane: None,
                }
            } else if host_caps.supports_wry {
                DispatchOutcome {
                    engine: EngineChoice::Wry,
                    middlenet_lane: None,
                }
            } else if middlenet_lane != LaneDecision::Unsupported {
                DispatchOutcome::middlenet(middlenet_lane)
            } else {
                DispatchOutcome::unsupported()
            }
        }
        _ => {
            let lane = middlenet_engine::engine::MiddleNetEngine::select_lane_for_source(
                &request.source,
                &host_caps.middlenet(),
                None,
            );
            if lane != LaneDecision::Unsupported {
                DispatchOutcome::middlenet(lane)
            } else {
                DispatchOutcome::unsupported()
            }
        }
    }
}

pub fn dispatch_prepared(
    prepared: &PreparedDocument,
    host_caps: &HostCapabilities,
    override_engine: Option<EngineOverride>,
) -> DispatchOutcome {
    dispatch_request(
        &VersoRequest::new(prepared.source.clone()),
        host_caps,
        override_engine,
    )
}

pub fn select_viewer_for_content(
    uri: &str,
    mime_hint: Option<&str>,
    host_caps: &HostCapabilities,
    web_engine_preference: WebEnginePreference,
) -> Option<ViewerRoutingDecision> {
    if let Some(source) = MiddleNetSource::detect(uri, mime_hint) {
        let outcome = dispatch_request(&VersoRequest::new(source), host_caps, None);
        return match outcome.engine {
            EngineChoice::Middlenet => Some(ViewerRoutingDecision {
                handle: ViewerHandle::Middlenet,
                engine: outcome.engine,
                middlenet_lane: outcome.middlenet_lane,
            }),
            EngineChoice::Servo => Some(ViewerRoutingDecision {
                handle: ViewerHandle::Webview,
                engine: outcome.engine,
                middlenet_lane: None,
            }),
            EngineChoice::Wry => Some(ViewerRoutingDecision {
                handle: ViewerHandle::Wry,
                engine: outcome.engine,
                middlenet_lane: None,
            }),
            EngineChoice::Unsupported => None,
        };
    }

    if is_browser_managed_content(uri, mime_hint) {
        let engine = match web_engine_preference {
            WebEnginePreference::Wry if host_caps.supports_wry => EngineChoice::Wry,
            WebEnginePreference::Servo if host_caps.supports_servo => EngineChoice::Servo,
            WebEnginePreference::Wry if host_caps.supports_servo => EngineChoice::Servo,
            WebEnginePreference::Servo if host_caps.supports_wry => EngineChoice::Wry,
            _ => EngineChoice::Unsupported,
        };

        return match engine {
            EngineChoice::Servo => Some(ViewerRoutingDecision {
                handle: ViewerHandle::Webview,
                engine,
                middlenet_lane: None,
            }),
            EngineChoice::Wry => Some(ViewerRoutingDecision {
                handle: ViewerHandle::Wry,
                engine,
                middlenet_lane: None,
            }),
            _ => None,
        };
    }

    None
}

/// Resolve routing for `(uri, mime_hint)` into a `VersoResolvedRoute`
/// carrying decision + reason + owner. Preferred entry point for
/// shell pane/routing code that wants typed access to the decision's
/// explanation instead of string-comparing `viewer_id`s.
///
/// `owner` is supplied by the caller because ownership is external
/// to the routing decision itself (verso can't tell whether the
/// caller is acting on a user pin or a policy choice).
pub fn resolve_route_for_content(
    uri: &str,
    mime_hint: Option<&str>,
    host_caps: &HostCapabilities,
    web_engine_preference: WebEnginePreference,
    owner: VersoPaneOwner,
) -> Option<VersoResolvedRoute> {
    let decision = select_viewer_for_content(uri, mime_hint, host_caps, web_engine_preference)?;
    let reason = match decision.engine {
        EngineChoice::Middlenet => {
            let lane = decision.middlenet_lane.unwrap_or(LaneDecision::Unsupported);
            VersoRouteReason::MiddlenetLane(lane)
        }
        EngineChoice::Servo | EngineChoice::Wry => {
            let preferred_matches_used = matches!(
                (web_engine_preference, decision.engine),
                (WebEnginePreference::Servo, EngineChoice::Servo)
                    | (WebEnginePreference::Wry, EngineChoice::Wry)
            );
            if preferred_matches_used {
                VersoRouteReason::WebEnginePreferred(web_engine_preference)
            } else {
                VersoRouteReason::WebEngineFallback {
                    preferred: web_engine_preference,
                    used: decision.engine,
                }
            }
        }
        EngineChoice::Unsupported => VersoRouteReason::Unsupported,
    };
    Some(VersoResolvedRoute {
        handle: decision.handle,
        engine: decision.engine,
        middlenet_lane: decision.middlenet_lane,
        reason,
        owner,
    })
}

fn is_browser_managed_content(uri: &str, mime_hint: Option<&str>) -> bool {
    let lower_uri = uri.trim().to_ascii_lowercase();
    if lower_uri.starts_with("http://")
        || lower_uri.starts_with("https://")
        || lower_uri.starts_with("data:")
    {
        return true;
    }

    if let Some(mime_hint) = mime_hint.map(|mime| mime.trim().to_ascii_lowercase()) {
        if matches!(
            mime_hint.as_str(),
            "text/html" | "image/svg+xml" | "text/css" | "application/javascript"
        ) {
            return true;
        }
    }

    let no_fragment = lower_uri.split('#').next().unwrap_or(lower_uri.as_str());
    let no_query = no_fragment.split('?').next().unwrap_or(no_fragment);
    matches!(
        no_query.rsplit_once('.').map(|(_, ext)| ext),
        Some("html" | "htm" | "svg")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feeds_stay_in_middlenet_direct() {
        let request = VersoRequest::new(
            MiddleNetSource::new(MiddleNetContentKind::Rss).with_uri("https://example.com/feed.xml"),
        );

        let outcome = dispatch_request(&request, &HostCapabilities::default(), None);

        assert_eq!(outcome.engine, EngineChoice::Middlenet);
        assert_eq!(outcome.middlenet_lane, Some(LaneDecision::Direct));
    }

    #[test]
    fn html_escalates_to_servo_when_middlenet_html_is_unavailable() {
        let request =
            VersoRequest::new(MiddleNetSource::new(MiddleNetContentKind::Html).with_uri(
                "https://example.com/index.html",
            ));
        let caps = HostCapabilities {
            supports_servo: true,
            ..HostCapabilities::default()
        };

        let outcome = dispatch_request(&request, &caps, None);

        assert_eq!(outcome.engine, EngineChoice::Servo);
        assert_eq!(outcome.middlenet_lane, None);
    }

    #[test]
    fn html_falls_back_to_faithful_source_before_unsupported() {
        let request =
            VersoRequest::new(MiddleNetSource::new(MiddleNetContentKind::Html).with_uri(
                "https://example.com/index.html",
            ));

        let outcome = dispatch_request(&request, &HostCapabilities::default(), None);

        assert_eq!(outcome.engine, EngineChoice::Middlenet);
        assert_eq!(outcome.middlenet_lane, Some(LaneDecision::FaithfulSource));
    }

    #[test]
    fn explicit_wry_override_is_honored() {
        let request =
            VersoRequest::new(MiddleNetSource::new(MiddleNetContentKind::Html).with_uri(
                "https://example.com/index.html",
            ));
        let caps = HostCapabilities {
            supports_wry: true,
            ..HostCapabilities::default()
        };

        let outcome = dispatch_request(&request, &caps, Some(EngineOverride::Wry));

        assert_eq!(outcome.engine, EngineChoice::Wry);
        assert_eq!(outcome.middlenet_lane, None);
    }

    #[test]
    fn viewer_selection_prefers_middlenet_for_feeds() {
        let decision = select_viewer_for_content(
            "https://example.com/feed.xml",
            Some("application/rss+xml"),
            &HostCapabilities::default(),
            WebEnginePreference::Servo,
        )
        .expect("feed should resolve a viewer");

        assert_eq!(decision.handle, ViewerHandle::Middlenet);
        assert_eq!(decision.viewer_id(), "viewer:middlenet");
        assert_eq!(decision.engine, EngineChoice::Middlenet);
        assert_eq!(decision.middlenet_lane, Some(LaneDecision::Direct));
    }

    #[test]
    fn viewer_selection_prefers_wry_when_requested_and_available() {
        let decision = select_viewer_for_content(
            "https://example.com/index.html",
            Some("text/html"),
            &HostCapabilities {
                supports_wry: true,
                ..HostCapabilities::default()
            },
            WebEnginePreference::Wry,
        )
        .expect("html should resolve a viewer");

        assert_eq!(decision.handle, ViewerHandle::Wry);
        assert_eq!(decision.viewer_id(), "viewer:wry");
        assert_eq!(decision.engine, EngineChoice::Wry);
    }

    #[test]
    fn viewer_selection_falls_back_to_servo_when_wry_is_unavailable() {
        let decision = select_viewer_for_content(
            "https://example.com/index.html",
            Some("text/html"),
            &HostCapabilities {
                supports_servo: true,
                ..HostCapabilities::default()
            },
            WebEnginePreference::Wry,
        )
        .expect("html should still resolve a viewer");

        assert_eq!(decision.handle, ViewerHandle::Webview);
        assert_eq!(decision.viewer_id(), "viewer:webview");
        assert_eq!(decision.engine, EngineChoice::Servo);
    }

    #[test]
    fn resolved_route_for_middlenet_feed_carries_lane_reason() {
        let route = resolve_route_for_content(
            "https://example.com/feed.xml",
            Some("application/rss+xml"),
            &HostCapabilities::default(),
            WebEnginePreference::Servo,
            VersoPaneOwner::Policy,
        )
        .expect("feed should resolve a route");

        assert!(route.is_middlenet());
        assert_eq!(route.owner(), VersoPaneOwner::Policy);
        assert!(matches!(
            route.reason(),
            VersoRouteReason::MiddlenetLane(LaneDecision::Direct)
        ));
    }

    #[test]
    fn resolved_route_for_web_with_matching_preference_reports_preferred() {
        let route = resolve_route_for_content(
            "https://example.com/index.html",
            Some("text/html"),
            &HostCapabilities {
                supports_wry: true,
                ..HostCapabilities::default()
            },
            WebEnginePreference::Wry,
            VersoPaneOwner::Policy,
        )
        .expect("html should resolve a route");

        assert!(route.is_wry());
        assert_eq!(
            route.reason(),
            &VersoRouteReason::WebEnginePreferred(WebEnginePreference::Wry)
        );
    }

    #[test]
    fn resolved_route_for_web_preference_fallback_reports_fallback_reason() {
        let route = resolve_route_for_content(
            "https://example.com/index.html",
            Some("text/html"),
            &HostCapabilities {
                supports_servo: true,
                ..HostCapabilities::default()
            },
            WebEnginePreference::Wry,
            VersoPaneOwner::Policy,
        )
        .expect("html should still resolve a route");

        assert!(route.is_servo());
        assert_eq!(
            route.reason(),
            &VersoRouteReason::WebEngineFallback {
                preferred: WebEnginePreference::Wry,
                used: EngineChoice::Servo,
            }
        );
    }

    #[test]
    fn resolved_route_carries_user_pin_owner_when_requested() {
        let route = resolve_route_for_content(
            "https://example.com/index.html",
            Some("text/html"),
            &HostCapabilities {
                supports_servo: true,
                ..HostCapabilities::default()
            },
            WebEnginePreference::Servo,
            VersoPaneOwner::UserPin,
        )
        .expect("html should resolve a route");

        assert_eq!(route.owner(), VersoPaneOwner::UserPin);
    }

    #[test]
    fn viewer_handle_round_trips_through_viewer_id_strings() {
        for handle in [
            ViewerHandle::Middlenet,
            ViewerHandle::Webview,
            ViewerHandle::Wry,
        ] {
            assert_eq!(ViewerHandle::from_viewer_id(handle.as_viewer_id()), Some(handle));
        }
        assert_eq!(ViewerHandle::from_viewer_id("viewer:plaintext"), None);
    }

    #[test]
    fn resolved_route_handle_matches_viewer_id_accessor() {
        let route = resolve_route_for_content(
            "https://example.com/feed.xml",
            Some("application/rss+xml"),
            &HostCapabilities::default(),
            WebEnginePreference::Servo,
            VersoPaneOwner::Policy,
        )
        .expect("feed should resolve a route");

        assert_eq!(route.handle(), ViewerHandle::Middlenet);
        assert_eq!(route.viewer_id(), "viewer:middlenet");
    }

    #[test]
    fn resolved_route_returns_none_when_no_engine_supports_content() {
        let route = resolve_route_for_content(
            "https://example.com/index.html",
            Some("text/html"),
            &HostCapabilities::default(),
            WebEnginePreference::Servo,
            VersoPaneOwner::Policy,
        );
        assert!(route.is_none());
    }
}
