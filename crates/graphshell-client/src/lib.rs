//! Transport-independent, endpoint-scoped projection state.

use std::collections::BTreeMap;

use graphshell_protocol::{
    AdvertisedAction, CapabilityProfile, ContentHash, NativeGlyphV1, PortableCardV1,
    PresentationCodec, PresentationManifest, PresentationSemantics, ProjectionSession,
    ProjectionSnapshot, ResourceRequest, ResourceResponse, SemanticRole, SessionStatus,
};
use sceno::{InstanceId, Scene};

/// The local cache entry for one disclosed remote projection.
#[derive(Clone, Debug, PartialEq)]
pub struct MountedScene {
    pub revision: u64,
    pub status: SessionStatus,
    pub scene: Scene,
    pub presentation: PresentationManifest,
}

/// A decoded representation plus the semantics that survive fallback.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedPresentation {
    pub semantics: PresentationSemantics,
    pub content: ResolvedContent,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResolvedContent {
    NativeGlyph(NativeGlyphV1),
    PortableCard(PortableCardV1),
    Image { mime_type: String, bytes: Vec<u8> },
    LabeledPlaceholder,
}

/// Resolution is explicit about bytes that have not arrived yet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PresentationResolution {
    Ready(ResolvedPresentation),
    NeedsResource(ResourceRequest),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResourceCacheError {
    UnknownSession,
    UnadvertisedResource,
    AddressMismatch,
    SizeMismatch,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResolutionError {
    UnknownSession,
    UnknownPresentation,
    InvalidPayload,
}

/// A renderer-neutral accessibility projection owned by the Graphshell client.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccessibilityTree {
    pub label: String,
    pub children: Vec<AccessibleItem>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccessibleItem {
    pub instance: InstanceId,
    pub label: String,
    pub role: SemanticRole,
    pub actions: Vec<AdvertisedAction>,
}

/// Curation state. It receives scenes but never obtains source truth.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClientState {
    mounted: BTreeMap<ProjectionSession, MountedScene>,
    // Disclosure stays session-scoped even when two endpoints advertise the
    // same content hash. Cross-session reuse requires an explicit later policy.
    resources: BTreeMap<(ProjectionSession, ContentHash), Vec<u8>>,
}

impl ClientState {
    /// Replace one endpoint's last acknowledged snapshot.
    pub fn apply_snapshot(&mut self, snapshot: ProjectionSnapshot) {
        self.mounted.insert(
            snapshot.session,
            MountedScene {
                revision: snapshot.revision,
                status: SessionStatus::Live,
                scene: snapshot.scene,
                presentation: snapshot.presentation,
            },
        );
    }

    /// Verify and cache one resource only if this session advertised it.
    pub fn apply_resource(&mut self, response: ResourceResponse) -> Result<(), ResourceCacheError> {
        let mounted = self
            .mounted
            .get(&response.session)
            .ok_or(ResourceCacheError::UnknownSession)?;
        if !response.has_valid_address() {
            return Err(ResourceCacheError::AddressMismatch);
        }
        let offers = mounted.presentation.offers.values().flatten();
        let mut advertised = false;
        let mut size_matches = false;
        for offer in offers {
            if offer.resource == response.resource {
                advertised = true;
                size_matches |= offer.byte_size == response.bytes.len() as u64;
            }
        }
        if !advertised {
            return Err(ResourceCacheError::UnadvertisedResource);
        }
        if !size_matches {
            return Err(ResourceCacheError::SizeMismatch);
        }
        self.resources
            .insert((response.session, response.resource), response.bytes);
        Ok(())
    }

    /// Resolve the richest supported offer, or a semantic placeholder when no
    /// offered codec fits the client's capabilities.
    pub fn resolve(
        &self,
        session: &ProjectionSession,
        instance: InstanceId,
        profile: &CapabilityProfile,
    ) -> Result<PresentationResolution, ResolutionError> {
        let mounted = self
            .mounted
            .get(session)
            .ok_or(ResolutionError::UnknownSession)?;
        let offers = mounted
            .presentation
            .offers_for(instance)
            .ok_or(ResolutionError::UnknownPresentation)?;
        let Some(offer) = offers.iter().find(|offer| profile.supports(offer.requires)) else {
            let semantics = offers
                .first()
                .ok_or(ResolutionError::UnknownPresentation)?
                .semantics
                .clone();
            return Ok(PresentationResolution::Ready(ResolvedPresentation {
                semantics,
                content: ResolvedContent::LabeledPlaceholder,
            }));
        };

        let Some(bytes) = self.resources.get(&(session.clone(), offer.resource)) else {
            return Ok(PresentationResolution::NeedsResource(ResourceRequest {
                session: session.clone(),
                resource: offer.resource,
            }));
        };

        let content = match &offer.codec {
            PresentationCodec::NativeGlyphV1 => ResolvedContent::NativeGlyph(
                serde_json::from_slice(bytes).map_err(|_| ResolutionError::InvalidPayload)?,
            ),
            PresentationCodec::PortableCardV1 => ResolvedContent::PortableCard(
                serde_json::from_slice(bytes).map_err(|_| ResolutionError::InvalidPayload)?,
            ),
            PresentationCodec::ImageV1 { mime_type } => ResolvedContent::Image {
                mime_type: mime_type.clone(),
                bytes: bytes.clone(),
            },
        };
        Ok(PresentationResolution::Ready(ResolvedPresentation {
            semantics: offer.semantics.clone(),
            content,
        }))
    }

    /// The semantic tree is available from the manifest before optional bytes
    /// arrive, so fallback does not erase names or actions.
    pub fn accessibility_tree(
        &self,
        session: &ProjectionSession,
        profile: &CapabilityProfile,
    ) -> Result<AccessibilityTree, ResolutionError> {
        let mounted = self
            .mounted
            .get(session)
            .ok_or(ResolutionError::UnknownSession)?;
        let mut children = Vec::new();
        for (index, item) in mounted.scene.items.iter().enumerate() {
            if !item.visible {
                continue;
            }
            let instance = InstanceId(index as u32);
            let Some(offers) = mounted.presentation.offers_for(instance) else {
                continue;
            };
            let semantics = offers
                .iter()
                .find(|offer| profile.supports(offer.requires))
                .or_else(|| offers.first())
                .ok_or(ResolutionError::UnknownPresentation)?
                .semantics
                .clone();
            children.push(AccessibleItem {
                instance,
                label: semantics.label,
                role: semantics.role,
                actions: semantics.actions,
            });
        }
        Ok(AccessibilityTree {
            label: "Graphshell projection".into(),
            children,
        })
    }

    /// Mark a mount stale without discarding permitted cached pixels or data.
    pub fn mark_stale(&mut self, session: &ProjectionSession) {
        if let Some(mounted) = self.mounted.get_mut(session) {
            mounted.status = SessionStatus::Stale;
        }
    }

    pub fn mounted(&self, session: &ProjectionSession) -> Option<&MountedScene> {
        self.mounted.get(session)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use graphshell_protocol::{
        BoundsRelationship, PresentationBinding, PresentationCapability, PresentationKey,
        PresentationOffer, ProtocolVersion, SemanticRole,
    };

    fn semantics(label: &str, role: SemanticRole) -> PresentationSemantics {
        PresentationSemantics {
            label: label.into(),
            role,
            bounds: BoundsRelationship::FillFootprint,
            actions: Vec::new(),
        }
    }

    fn snapshot_with_offer(
        session: &ProjectionSession,
        codec: PresentationCodec,
        requires: PresentationCapability,
        semantics: PresentationSemantics,
        bytes: &[u8],
    ) -> ProjectionSnapshot {
        let key = PresentationKey("item:0".into());
        let hash = ContentHash::of(bytes);
        let mut presentation = PresentationManifest {
            bindings: vec![PresentationBinding {
                instance: InstanceId(0),
                key: key.clone(),
            }],
            ..PresentationManifest::default()
        };
        presentation.offers.insert(
            key,
            vec![PresentationOffer {
                codec,
                resource: hash,
                byte_size: bytes.len() as u64,
                requires,
                semantics,
            }],
        );
        ProjectionSnapshot {
            version: ProtocolVersion::V1,
            session: session.clone(),
            revision: 4,
            scene: Scene::new(),
            presentation,
        }
    }

    #[test]
    fn snapshot_and_resource_stay_scoped_to_their_session() {
        let session = ProjectionSession("loopback:one".into());
        let glyph = NativeGlyphV1 {
            label: "One".into(),
            icon: Some("1".into()),
            color: None,
        };
        let bytes = serde_json::to_vec(&glyph).unwrap();
        let snapshot = snapshot_with_offer(
            &session,
            PresentationCodec::NativeGlyphV1,
            PresentationCapability::NativeGlyph,
            semantics("One", SemanticRole::Graphic),
            &bytes,
        );
        let hash = ContentHash::of(&bytes);
        let mut client = ClientState::default();
        client.apply_snapshot(snapshot);
        client
            .apply_resource(ResourceResponse {
                session: session.clone(),
                resource: hash,
                bytes,
            })
            .unwrap();
        client.mark_stale(&session);
        assert_eq!(
            client.mounted(&session).unwrap().status,
            SessionStatus::Stale
        );
        assert!(matches!(
            client.resolve(
                &session,
                InstanceId(0),
                &CapabilityProfile::new([PresentationCapability::NativeGlyph])
            ),
            Ok(PresentationResolution::Ready(ResolvedPresentation {
                content: ResolvedContent::NativeGlyph(_),
                ..
            }))
        ));
    }

    #[test]
    fn unsupported_image_becomes_a_labeled_placeholder_without_fetching() {
        let session = ProjectionSession("loopback:image".into());
        let snapshot = snapshot_with_offer(
            &session,
            PresentationCodec::ImageV1 {
                mime_type: "image/svg+xml".into(),
            },
            PresentationCapability::Image,
            semantics("Map tile", SemanticRole::Image),
            b"<svg/>",
        );
        let mut client = ClientState::default();
        client.apply_snapshot(snapshot);
        assert_eq!(
            client
                .resolve(
                    &session,
                    InstanceId(0),
                    &CapabilityProfile::new([PresentationCapability::NativeGlyph])
                )
                .unwrap(),
            PresentationResolution::Ready(ResolvedPresentation {
                semantics: semantics("Map tile", SemanticRole::Image),
                content: ResolvedContent::LabeledPlaceholder,
            })
        );
    }

    #[test]
    fn unsolicited_resource_is_rejected() {
        let session = ProjectionSession("loopback:one".into());
        let mut client = ClientState::default();
        client.apply_snapshot(ProjectionSnapshot {
            version: ProtocolVersion::V1,
            session: session.clone(),
            revision: 1,
            scene: Scene::new(),
            presentation: PresentationManifest::default(),
        });
        assert_eq!(
            client.apply_resource(ResourceResponse::new(session, b"secret".to_vec())),
            Err(ResourceCacheError::UnadvertisedResource)
        );
    }
}
