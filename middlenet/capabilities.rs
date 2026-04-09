/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum ProtocolCapability {
    DiscoverIdentity,
    ResolveIdentity,
    PublishArtifact,
    DeliverMessage,
    HttpFetch,
    KnownHosts,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum MiddlenetProtocol {
    WebFinger,
    Nip05,
    Matrix,
    ActivityPub,
    Gemini,
    Titan,
    Misfin,
}

impl MiddlenetProtocol {
    pub(crate) fn key(self) -> &'static str {
        match self {
            Self::WebFinger => "webfinger",
            Self::Nip05 => "nip05",
            Self::Matrix => "matrix",
            Self::ActivityPub => "activitypub",
            Self::Gemini => "gemini",
            Self::Titan => "titan",
            Self::Misfin => "misfin",
        }
    }

    pub(crate) fn from_key(key: &str) -> Option<Self> {
        match key {
            "webfinger" => Some(Self::WebFinger),
            "nip05" => Some(Self::Nip05),
            "matrix" => Some(Self::Matrix),
            "activitypub" => Some(Self::ActivityPub),
            "gemini" => Some(Self::Gemini),
            "titan" => Some(Self::Titan),
            "misfin" => Some(Self::Misfin),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProtocolFreshness {
    Fresh,
    Stale,
    NoPolicy,
}

impl ProtocolFreshness {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Fresh => "Fresh",
            Self::Stale => "Stale",
            Self::NoPolicy => "No TTL",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ProtocolDescriptor {
    pub(crate) protocol: MiddlenetProtocol,
    pub(crate) display_name: &'static str,
    pub(crate) identity_classification_kind: Option<&'static str>,
    pub(crate) identity_requirement_label: Option<&'static str>,
    pub(crate) action_name: Option<&'static str>,
    pub(crate) success_prefix: Option<&'static str>,
    pub(crate) freshness_ttl: Option<Duration>,
    pub(crate) capabilities: &'static [ProtocolCapability],
}

const WEBFINGER_FRESHNESS_TTL: Duration = Duration::from_secs(24 * 60 * 60);
const NIP05_FRESHNESS_TTL: Duration = Duration::from_secs(12 * 60 * 60);
const MATRIX_FRESHNESS_TTL: Duration = Duration::from_secs(6 * 60 * 60);
const ACTIVITYPUB_FRESHNESS_TTL: Duration = Duration::from_secs(6 * 60 * 60);

const ALL_PROTOCOLS: [MiddlenetProtocol; 7] = [
    MiddlenetProtocol::WebFinger,
    MiddlenetProtocol::Nip05,
    MiddlenetProtocol::Matrix,
    MiddlenetProtocol::ActivityPub,
    MiddlenetProtocol::Gemini,
    MiddlenetProtocol::Titan,
    MiddlenetProtocol::Misfin,
];

pub(crate) fn descriptor(protocol: MiddlenetProtocol) -> ProtocolDescriptor {
    match protocol {
        MiddlenetProtocol::WebFinger => ProtocolDescriptor {
            protocol,
            display_name: "WebFinger",
            identity_classification_kind: Some("webfinger"),
            identity_requirement_label: Some("WebFinger identity"),
            action_name: Some("WebFinger import"),
            success_prefix: Some("Imported WebFinger discovery"),
            freshness_ttl: Some(WEBFINGER_FRESHNESS_TTL),
            capabilities: &[
                ProtocolCapability::DiscoverIdentity,
                ProtocolCapability::HttpFetch,
            ],
        },
        MiddlenetProtocol::Nip05 => ProtocolDescriptor {
            protocol,
            display_name: "NIP-05",
            identity_classification_kind: Some("nip05"),
            identity_requirement_label: Some("NIP-05 identity"),
            action_name: Some("NIP-05 resolve"),
            success_prefix: Some("Resolved NIP-05 identity"),
            freshness_ttl: Some(NIP05_FRESHNESS_TTL),
            capabilities: &[
                ProtocolCapability::ResolveIdentity,
                ProtocolCapability::HttpFetch,
            ],
        },
        MiddlenetProtocol::Matrix => ProtocolDescriptor {
            protocol,
            display_name: "Matrix",
            identity_classification_kind: Some("matrix"),
            identity_requirement_label: Some("Matrix identity"),
            action_name: Some("Matrix resolve"),
            success_prefix: Some("Resolved Matrix profile"),
            freshness_ttl: Some(MATRIX_FRESHNESS_TTL),
            capabilities: &[
                ProtocolCapability::ResolveIdentity,
                ProtocolCapability::HttpFetch,
            ],
        },
        MiddlenetProtocol::ActivityPub => ProtocolDescriptor {
            protocol,
            display_name: "ActivityPub",
            identity_classification_kind: Some("activitypub"),
            identity_requirement_label: Some("ActivityPub actor identity"),
            action_name: Some("ActivityPub import"),
            success_prefix: Some("Imported ActivityPub actor"),
            freshness_ttl: Some(ACTIVITYPUB_FRESHNESS_TTL),
            capabilities: &[
                ProtocolCapability::ResolveIdentity,
                ProtocolCapability::HttpFetch,
            ],
        },
        MiddlenetProtocol::Gemini => ProtocolDescriptor {
            protocol,
            display_name: "Gemini",
            identity_classification_kind: Some("gemini"),
            identity_requirement_label: Some("Gemini endpoint"),
            action_name: None,
            success_prefix: None,
            freshness_ttl: None,
            capabilities: &[ProtocolCapability::KnownHosts],
        },
        MiddlenetProtocol::Titan => ProtocolDescriptor {
            protocol,
            display_name: "Titan",
            identity_classification_kind: Some("gemini"),
            identity_requirement_label: Some("Gemini/Titan publication endpoint"),
            action_name: None,
            success_prefix: None,
            freshness_ttl: None,
            capabilities: &[
                ProtocolCapability::PublishArtifact,
                ProtocolCapability::KnownHosts,
            ],
        },
        MiddlenetProtocol::Misfin => ProtocolDescriptor {
            protocol,
            display_name: "Misfin",
            identity_classification_kind: Some("misfin"),
            identity_requirement_label: Some("Misfin mailbox identity"),
            action_name: None,
            success_prefix: None,
            freshness_ttl: None,
            capabilities: &[
                ProtocolCapability::DeliverMessage,
                ProtocolCapability::KnownHosts,
            ],
        },
    }
}

pub(crate) fn protocol_for_identity_classification_kind(
    kind: &str,
) -> Option<MiddlenetProtocol> {
    ALL_PROTOCOLS.into_iter().find(|protocol| {
        descriptor(*protocol)
            .identity_classification_kind
            .is_some_and(|candidate| candidate == kind)
    })
}

pub(crate) fn supports(protocol: MiddlenetProtocol, capability: ProtocolCapability) -> bool {
    descriptor(protocol).capabilities.contains(&capability)
}

pub(crate) fn protocols_with_capability(
    capability: ProtocolCapability,
) -> impl Iterator<Item = MiddlenetProtocol> {
    ALL_PROTOCOLS
        .into_iter()
        .filter(move |protocol| supports(*protocol, capability))
}

pub(crate) fn primary_protocol_for_capability(
    capability: ProtocolCapability,
) -> Option<MiddlenetProtocol> {
    protocols_with_capability(capability).next()
}

pub(crate) fn freshness_state(
    protocol: MiddlenetProtocol,
    resolved_at_ms: u64,
    now_ms: u64,
) -> ProtocolFreshness {
    let Some(ttl) = descriptor(protocol).freshness_ttl else {
        return ProtocolFreshness::NoPolicy;
    };
    let age_ms = now_ms.saturating_sub(resolved_at_ms);
    if age_ms <= ttl.as_millis() as u64 {
        ProtocolFreshness::Fresh
    } else {
        ProtocolFreshness::Stale
    }
}

pub(crate) fn normalize_identity_action_resource(
    protocol: MiddlenetProtocol,
    resource: &str,
) -> Result<String, String> {
    match protocol {
        MiddlenetProtocol::WebFinger => crate::middlenet::webfinger::normalize_resource(resource),
        MiddlenetProtocol::Nip05 => crate::middlenet::identity::normalize_nip05_identifier(resource),
        MiddlenetProtocol::Matrix => crate::middlenet::identity::normalize_matrix_mxid(
            resource.trim_start_matches("mxid:"),
        ),
        MiddlenetProtocol::ActivityPub => {
            crate::middlenet::identity::normalize_activitypub_actor_url(resource)
        }
        MiddlenetProtocol::Gemini
        | MiddlenetProtocol::Titan
        | MiddlenetProtocol::Misfin => Err(format!(
            "{} is not an identity import protocol.",
            descriptor(protocol).display_name
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publish_and_delivery_capabilities_map_to_mutation_protocols() {
        assert_eq!(
            primary_protocol_for_capability(ProtocolCapability::PublishArtifact),
            Some(MiddlenetProtocol::Titan)
        );
        assert_eq!(
            primary_protocol_for_capability(ProtocolCapability::DeliverMessage),
            Some(MiddlenetProtocol::Misfin)
        );
    }

    #[test]
    fn normalize_identity_action_resource_uses_protocol_specific_rules() {
        assert_eq!(
            normalize_identity_action_resource(MiddlenetProtocol::WebFinger, "mark@example.net")
                .expect("webfinger resource should normalize"),
            "acct:mark@example.net"
        );
        assert_eq!(
            normalize_identity_action_resource(MiddlenetProtocol::Nip05, "nip05:mark@example.net")
                .expect("nip05 resource should normalize"),
            "mark@example.net"
        );
        assert_eq!(
            normalize_identity_action_resource(
                MiddlenetProtocol::Matrix,
                "mxid:@mark:matrix.example"
            )
                .expect("matrix resource should normalize"),
            "@mark:matrix.example"
        );
        assert_eq!(
            normalize_identity_action_resource(
                MiddlenetProtocol::ActivityPub,
                "https://social.example/users/mark"
            )
            .expect("activitypub actor should normalize"),
            "https://social.example/users/mark"
        );
    }

    #[test]
    fn freshness_state_respects_protocol_ttl() {
        assert_eq!(
            freshness_state(MiddlenetProtocol::Nip05, 1_000, 1_000 + 60_000),
            ProtocolFreshness::Fresh
        );
        assert_eq!(
            freshness_state(
                MiddlenetProtocol::Nip05,
                1_000,
                1_000 + NIP05_FRESHNESS_TTL.as_millis() as u64 + 1,
            ),
            ProtocolFreshness::Stale
        );
        assert_eq!(
            freshness_state(MiddlenetProtocol::Titan, 1_000, 9_000),
            ProtocolFreshness::NoPolicy
        );
    }
}