use super::*;
use crate::services::persistence::types::{
    PersistedArrangementSubKind, PersistedContainmentSubKind, PersistedEdgeAssertion,
    PersistedImportedSubKind, PersistedProvenanceSubKind, PersistedRelationSelector,
    PersistedSemanticSubKind,
};

const TAG_WEBFINGER: &str = "#webfinger";
const TAG_DISCOVERY: &str = "#discovery";
const TAG_IDENTITY: &str = "#identity";
const TAG_PERSON: &str = "#person";
const TAG_ALIAS: &str = "#alias";
const TAG_PROFILE: &str = "#profile";
const TAG_GEMINI: &str = "#gemini";
const TAG_GOPHER: &str = "#gopher";
const TAG_MISFIN: &str = "#misfin";
const TAG_NOSTR: &str = "#nostr";
const TAG_MATRIX: &str = "#matrix";
const TAG_NIP05: &str = "#nip05";
const TAG_ACTIVITYPUB: &str = "#activitypub";
const TAG_ENDPOINT: &str = "#endpoint";
const TAG_POST: &str = "#post";
const TAG_SHARED_DATA: &str = "#shared-data";
const TAG_MESSAGE_NOTIFICATION: &str = "#message-notification";
const TAG_PERSON_ARTIFACT: &str = "#person-artifact";
const WEBFINGER_CLUSTER_RADIUS: f32 = 180.0;

struct WebFingerNodeSpec {
    url: String,
    title: String,
    relation_label: &'static str,
    tags: &'static [&'static str],
}

#[derive(Clone, Copy)]
enum PersonIdentityRelation {
    SameEntityAs,
    CanonicalMirrorOf,
    GroupedOnly,
}

struct PersonIdentityNodeSpec {
    url: String,
    title: String,
    relation_label: &'static str,
    tags: &'static [&'static str],
    relation: PersonIdentityRelation,
}

pub(crate) struct PersonIdentityRefreshOutcome {
    pub(crate) person_key: NodeKey,
    pub(crate) refreshed_protocols: usize,
    pub(crate) changed: bool,
}

fn person_title(profile: &graphshell_comms::identity::PersonIdentityProfile) -> String {
    format!("Person: {}", webfinger_display_target(profile.preferred_label()))
}

fn person_identity_node_title(prefix: &str, target: &str) -> String {
    format!("{prefix}: {}", webfinger_display_target(target))
}

fn person_node_url(
    profile: &graphshell_comms::identity::PersonIdentityProfile,
) -> Result<String, String> {
    let canonical_seed = profile
        .canonical_seed()
        .ok_or_else(|| "Person identity profile must include at least one canonical identity.".to_string())?;
    let person_id = uuid::Uuid::new_v5(&uuid::Uuid::nil(), canonical_seed.as_bytes());
    Ok(
        crate::util::VersoAddress::Other {
            category: "person".to_string(),
            segments: vec![person_id.to_string()],
        }
        .to_string(),
    )
}

fn person_artifact_url(person_url: &str, kind: graphshell_comms::identity::PersonArtifactKind) -> String {
    let person_id = crate::util::VersoAddress::parse(person_url)
        .and_then(|address| match address {
            crate::util::VersoAddress::Other { category, segments }
                if category == "person" && !segments.is_empty() => segments.into_iter().next(),
            _ => None,
        })
        .unwrap_or_else(|| uuid::Uuid::new_v5(&uuid::Uuid::nil(), person_url.as_bytes()).to_string());
    crate::util::VersoAddress::Other {
        category: "person".to_string(),
        segments: vec![
            person_id,
            kind.route_segment().to_string(),
            uuid::Uuid::new_v4().to_string(),
        ],
    }
    .to_string()
}

fn person_identity_scheme(kind: &str) -> crate::model::graph::ClassificationScheme {
    crate::model::graph::ClassificationScheme::Custom(format!("identity:{kind}"))
}

fn person_resolution_scheme(
    protocol: graphshell_comms::capabilities::MiddlenetProtocol,
) -> crate::model::graph::ClassificationScheme {
    crate::model::graph::ClassificationScheme::Custom(format!(
        "resolution:{}",
        protocol.key()
    ))
}

fn parse_person_resolution_scheme(
    scheme: &crate::model::graph::ClassificationScheme,
) -> Option<graphshell_comms::capabilities::MiddlenetProtocol> {
    match scheme {
        crate::model::graph::ClassificationScheme::Custom(value) => value
            .strip_prefix("resolution:")
            .and_then(graphshell_comms::capabilities::MiddlenetProtocol::from_key),
        _ => None,
    }
}

fn person_identity_classification_candidates(
    profile: &graphshell_comms::identity::PersonIdentityProfile,
) -> Vec<(crate::model::graph::ClassificationScheme, String)> {
    let mut candidates = Vec::new();

    if let Some(handle) = &profile.human_handle {
        candidates.push((person_identity_scheme("handle"), handle.clone()));
    }
    if let Some(resource) = &profile.webfinger_resource {
        candidates.push((person_identity_scheme("webfinger"), resource.clone()));
    }
    if let Some(nip05_identifier) = &profile.nip05_identifier {
        candidates.push((person_identity_scheme("nip05"), nip05_identifier.clone()));
    }
    for mxid in &profile.matrix_mxids {
        candidates.push((person_identity_scheme("matrix"), mxid.clone()));
    }
    for identity in &profile.nostr_identities {
        candidates.push((person_identity_scheme("nostr"), identity.clone()));
    }
    for mailbox in &profile.misfin_mailboxes {
        candidates.push((person_identity_scheme("misfin"), mailbox.clone()));
    }
    for capsule in &profile.gemini_capsules {
        candidates.push((person_identity_scheme("gemini"), capsule.clone()));
    }
    for actor in &profile.activitypub_actors {
        candidates.push((person_identity_scheme("activitypub"), actor.clone()));
    }

    candidates
}

fn webfinger_display_target(target: &str) -> &str {
    target.strip_prefix("acct:").unwrap_or(target)
}

fn webfinger_subject_title(subject: &str) -> String {
    format!("Identity: {}", webfinger_display_target(subject))
}

fn webfinger_prefixed_title(prefix: &str, target: &str) -> String {
    format!("{prefix}: {}", webfinger_display_target(target))
}

fn webfinger_endpoint_title(endpoint: &graphshell_comms::webfinger::WebFingerEndpoint) -> String {
    let rel = endpoint.rel.trim();
    if rel.is_empty() {
        webfinger_prefixed_title("Endpoint", &endpoint.href)
    } else {
        format!("Endpoint ({rel}): {}", webfinger_display_target(&endpoint.href))
    }
}

fn webfinger_cluster_position(
    center: euclid::default::Point2D<f32>,
    index: usize,
    total: usize,
) -> euclid::default::Point2D<f32> {
    if total == 0 {
        return center;
    }

    let fraction = index as f32 / total as f32;
    let angle = fraction * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
    euclid::default::Point2D::new(
        center.x + WEBFINGER_CLUSTER_RADIUS * angle.cos(),
        center.y + WEBFINGER_CLUSTER_RADIUS * 0.72 * angle.sin(),
    )
}

fn edge_type_to_assertion(
    edge_type: crate::graph::EdgeType,
    edge_label: Option<String>,
) -> Option<crate::graph::EdgeAssertion> {
    match edge_type {
        crate::graph::EdgeType::Hyperlink => Some(crate::graph::EdgeAssertion::Semantic {
            sub_kind: crate::graph::SemanticSubKind::Hyperlink,
            label: edge_label,
            decay_progress: None,
        }),
        crate::graph::EdgeType::UserGrouped => Some(crate::graph::EdgeAssertion::Semantic {
            sub_kind: crate::graph::SemanticSubKind::UserGrouped,
            label: edge_label,
            decay_progress: None,
        }),
        crate::graph::EdgeType::AgentDerived { decay_progress } => {
            Some(crate::graph::EdgeAssertion::Semantic {
                sub_kind: crate::graph::SemanticSubKind::AgentDerived,
                label: edge_label,
                decay_progress: Some(decay_progress),
            })
        }
        crate::graph::EdgeType::ContainmentRelation(sub_kind) => {
            Some(crate::graph::EdgeAssertion::Containment { sub_kind })
        }
        crate::graph::EdgeType::ArrangementRelation(sub_kind) => {
            Some(crate::graph::EdgeAssertion::Arrangement { sub_kind })
        }
        crate::graph::EdgeType::History | crate::graph::EdgeType::ImportedRelation => None,
    }
}

fn edge_type_to_selector(
    edge_type: crate::graph::EdgeType,
) -> Option<crate::graph::RelationSelector> {
    match edge_type {
        crate::graph::EdgeType::Hyperlink => Some(crate::graph::RelationSelector::Semantic(
            crate::graph::SemanticSubKind::Hyperlink,
        )),
        crate::graph::EdgeType::UserGrouped => Some(crate::graph::RelationSelector::Semantic(
            crate::graph::SemanticSubKind::UserGrouped,
        )),
        crate::graph::EdgeType::AgentDerived { .. } => Some(
            crate::graph::RelationSelector::Semantic(crate::graph::SemanticSubKind::AgentDerived),
        ),
        crate::graph::EdgeType::ContainmentRelation(sub_kind) => {
            Some(crate::graph::RelationSelector::Containment(sub_kind))
        }
        crate::graph::EdgeType::ArrangementRelation(sub_kind) => {
            Some(crate::graph::RelationSelector::Arrangement(sub_kind))
        }
        crate::graph::EdgeType::ImportedRelation => None,
        crate::graph::EdgeType::History => Some(crate::graph::RelationSelector::Family(
            crate::graph::EdgeFamily::Traversal,
        )),
    }
}

fn persisted_assertion_from_graph_assertion(
    assertion: crate::graph::EdgeAssertion,
) -> PersistedEdgeAssertion {
    match assertion {
        crate::graph::EdgeAssertion::Semantic {
            sub_kind,
            label,
            decay_progress,
        } => PersistedEdgeAssertion::Semantic {
            sub_kind: match sub_kind {
                crate::graph::SemanticSubKind::Hyperlink => PersistedSemanticSubKind::Hyperlink,
                crate::graph::SemanticSubKind::UserGrouped => PersistedSemanticSubKind::UserGrouped,
                crate::graph::SemanticSubKind::AgentDerived => {
                    PersistedSemanticSubKind::AgentDerived
                }
                crate::graph::SemanticSubKind::Cites => PersistedSemanticSubKind::Cites,
                crate::graph::SemanticSubKind::Quotes => PersistedSemanticSubKind::Quotes,
                crate::graph::SemanticSubKind::Summarizes => PersistedSemanticSubKind::Summarizes,
                crate::graph::SemanticSubKind::Elaborates => PersistedSemanticSubKind::Elaborates,
                crate::graph::SemanticSubKind::ExampleOf => PersistedSemanticSubKind::ExampleOf,
                crate::graph::SemanticSubKind::Supports => PersistedSemanticSubKind::Supports,
                crate::graph::SemanticSubKind::Contradicts => PersistedSemanticSubKind::Contradicts,
                crate::graph::SemanticSubKind::Questions => PersistedSemanticSubKind::Questions,
                crate::graph::SemanticSubKind::SameEntityAs => {
                    PersistedSemanticSubKind::SameEntityAs
                }
                crate::graph::SemanticSubKind::DuplicateOf => PersistedSemanticSubKind::DuplicateOf,
                crate::graph::SemanticSubKind::CanonicalMirrorOf => {
                    PersistedSemanticSubKind::CanonicalMirrorOf
                }
                crate::graph::SemanticSubKind::DependsOn => PersistedSemanticSubKind::DependsOn,
                crate::graph::SemanticSubKind::Blocks => PersistedSemanticSubKind::Blocks,
                crate::graph::SemanticSubKind::NextStep => PersistedSemanticSubKind::NextStep,
            },
            label,
            agent_decay_progress: decay_progress,
        },
        crate::graph::EdgeAssertion::Containment { sub_kind } => {
            PersistedEdgeAssertion::Containment {
                sub_kind: match sub_kind {
                    crate::graph::ContainmentSubKind::UrlPath => {
                        PersistedContainmentSubKind::UrlPath
                    }
                    crate::graph::ContainmentSubKind::Domain => PersistedContainmentSubKind::Domain,
                    crate::graph::ContainmentSubKind::FileSystem => {
                        PersistedContainmentSubKind::FileSystem
                    }
                    crate::graph::ContainmentSubKind::UserFolder => {
                        PersistedContainmentSubKind::UserFolder
                    }
                    crate::graph::ContainmentSubKind::ClipSource => {
                        PersistedContainmentSubKind::ClipSource
                    }
                    crate::graph::ContainmentSubKind::NotebookSection => {
                        PersistedContainmentSubKind::NotebookSection
                    }
                    crate::graph::ContainmentSubKind::CollectionMember => {
                        PersistedContainmentSubKind::CollectionMember
                    }
                },
            }
        }
        crate::graph::EdgeAssertion::Arrangement { sub_kind } => {
            PersistedEdgeAssertion::Arrangement {
                sub_kind: match sub_kind {
                    crate::graph::ArrangementSubKind::FrameMember => {
                        PersistedArrangementSubKind::FrameMember
                    }
                    crate::graph::ArrangementSubKind::TileGroup => {
                        PersistedArrangementSubKind::TileGroup
                    }
                    crate::graph::ArrangementSubKind::SplitPair => {
                        PersistedArrangementSubKind::SplitPair
                    }
                },
            }
        }
        crate::graph::EdgeAssertion::Imported { sub_kind } => PersistedEdgeAssertion::Imported {
            sub_kind: match sub_kind {
                crate::graph::ImportedSubKind::BookmarkFolder => {
                    PersistedImportedSubKind::BookmarkFolder
                }
                crate::graph::ImportedSubKind::HistoryImport => {
                    PersistedImportedSubKind::HistoryImport
                }
                crate::graph::ImportedSubKind::SessionImport => {
                    PersistedImportedSubKind::SessionImport
                }
                crate::graph::ImportedSubKind::RssMembership => {
                    PersistedImportedSubKind::RssMembership
                }
                crate::graph::ImportedSubKind::FileSystemImport => {
                    PersistedImportedSubKind::FileSystemImport
                }
                crate::graph::ImportedSubKind::ArchiveMembership => {
                    PersistedImportedSubKind::ArchiveMembership
                }
                crate::graph::ImportedSubKind::SharedCollection => {
                    PersistedImportedSubKind::SharedCollection
                }
            },
        },
        crate::graph::EdgeAssertion::Provenance { sub_kind } => {
            PersistedEdgeAssertion::Provenance {
                sub_kind: match sub_kind {
                    crate::graph::ProvenanceSubKind::ClippedFrom => {
                        PersistedProvenanceSubKind::ClippedFrom
                    }
                    crate::graph::ProvenanceSubKind::ExcerptedFrom => {
                        PersistedProvenanceSubKind::ExcerptedFrom
                    }
                    crate::graph::ProvenanceSubKind::SummarizedFrom => {
                        PersistedProvenanceSubKind::SummarizedFrom
                    }
                    crate::graph::ProvenanceSubKind::TranslatedFrom => {
                        PersistedProvenanceSubKind::TranslatedFrom
                    }
                    crate::graph::ProvenanceSubKind::RewrittenFrom => {
                        PersistedProvenanceSubKind::RewrittenFrom
                    }
                    crate::graph::ProvenanceSubKind::GeneratedFrom => {
                        PersistedProvenanceSubKind::GeneratedFrom
                    }
                    crate::graph::ProvenanceSubKind::ExtractedFrom => {
                        PersistedProvenanceSubKind::ExtractedFrom
                    }
                    crate::graph::ProvenanceSubKind::ImportedFromSource => {
                        PersistedProvenanceSubKind::ImportedFromSource
                    }
                },
            }
        }
    }
}

fn persisted_selector_from_graph_selector(
    selector: crate::graph::RelationSelector,
) -> Option<PersistedRelationSelector> {
    Some(match selector {
        crate::graph::RelationSelector::Family(family) => {
            PersistedRelationSelector::Family(match family {
                crate::graph::EdgeFamily::Semantic => {
                    crate::services::persistence::types::PersistedEdgeFamily::Semantic
                }
                crate::graph::EdgeFamily::Traversal => {
                    crate::services::persistence::types::PersistedEdgeFamily::Traversal
                }
                crate::graph::EdgeFamily::Containment => {
                    crate::services::persistence::types::PersistedEdgeFamily::Containment
                }
                crate::graph::EdgeFamily::Arrangement => {
                    crate::services::persistence::types::PersistedEdgeFamily::Arrangement
                }
                crate::graph::EdgeFamily::Imported => {
                    crate::services::persistence::types::PersistedEdgeFamily::Imported
                }
                crate::graph::EdgeFamily::Provenance => {
                    crate::services::persistence::types::PersistedEdgeFamily::Provenance
                }
            })
        }
        crate::graph::RelationSelector::Semantic(sub_kind) => {
            PersistedRelationSelector::Semantic(match sub_kind {
                crate::graph::SemanticSubKind::Hyperlink => PersistedSemanticSubKind::Hyperlink,
                crate::graph::SemanticSubKind::UserGrouped => PersistedSemanticSubKind::UserGrouped,
                crate::graph::SemanticSubKind::AgentDerived => {
                    PersistedSemanticSubKind::AgentDerived
                }
                crate::graph::SemanticSubKind::Cites => PersistedSemanticSubKind::Cites,
                crate::graph::SemanticSubKind::Quotes => PersistedSemanticSubKind::Quotes,
                crate::graph::SemanticSubKind::Summarizes => PersistedSemanticSubKind::Summarizes,
                crate::graph::SemanticSubKind::Elaborates => PersistedSemanticSubKind::Elaborates,
                crate::graph::SemanticSubKind::ExampleOf => PersistedSemanticSubKind::ExampleOf,
                crate::graph::SemanticSubKind::Supports => PersistedSemanticSubKind::Supports,
                crate::graph::SemanticSubKind::Contradicts => PersistedSemanticSubKind::Contradicts,
                crate::graph::SemanticSubKind::Questions => PersistedSemanticSubKind::Questions,
                crate::graph::SemanticSubKind::SameEntityAs => {
                    PersistedSemanticSubKind::SameEntityAs
                }
                crate::graph::SemanticSubKind::DuplicateOf => PersistedSemanticSubKind::DuplicateOf,
                crate::graph::SemanticSubKind::CanonicalMirrorOf => {
                    PersistedSemanticSubKind::CanonicalMirrorOf
                }
                crate::graph::SemanticSubKind::DependsOn => PersistedSemanticSubKind::DependsOn,
                crate::graph::SemanticSubKind::Blocks => PersistedSemanticSubKind::Blocks,
                crate::graph::SemanticSubKind::NextStep => PersistedSemanticSubKind::NextStep,
            })
        }
        crate::graph::RelationSelector::Containment(sub_kind) => {
            PersistedRelationSelector::Containment(match sub_kind {
                crate::graph::ContainmentSubKind::UrlPath => PersistedContainmentSubKind::UrlPath,
                crate::graph::ContainmentSubKind::Domain => PersistedContainmentSubKind::Domain,
                crate::graph::ContainmentSubKind::FileSystem => {
                    PersistedContainmentSubKind::FileSystem
                }
                crate::graph::ContainmentSubKind::UserFolder => {
                    PersistedContainmentSubKind::UserFolder
                }
                crate::graph::ContainmentSubKind::ClipSource => {
                    PersistedContainmentSubKind::ClipSource
                }
                crate::graph::ContainmentSubKind::NotebookSection => {
                    PersistedContainmentSubKind::NotebookSection
                }
                crate::graph::ContainmentSubKind::CollectionMember => {
                    PersistedContainmentSubKind::CollectionMember
                }
            })
        }
        crate::graph::RelationSelector::Arrangement(sub_kind) => {
            PersistedRelationSelector::Arrangement(match sub_kind {
                crate::graph::ArrangementSubKind::FrameMember => {
                    PersistedArrangementSubKind::FrameMember
                }
                crate::graph::ArrangementSubKind::TileGroup => {
                    PersistedArrangementSubKind::TileGroup
                }
                crate::graph::ArrangementSubKind::SplitPair => {
                    PersistedArrangementSubKind::SplitPair
                }
            })
        }
        crate::graph::RelationSelector::Imported(sub_kind) => {
            PersistedRelationSelector::Imported(match sub_kind {
                crate::graph::ImportedSubKind::BookmarkFolder => {
                    PersistedImportedSubKind::BookmarkFolder
                }
                crate::graph::ImportedSubKind::HistoryImport => {
                    PersistedImportedSubKind::HistoryImport
                }
                crate::graph::ImportedSubKind::SessionImport => {
                    PersistedImportedSubKind::SessionImport
                }
                crate::graph::ImportedSubKind::RssMembership => {
                    PersistedImportedSubKind::RssMembership
                }
                crate::graph::ImportedSubKind::FileSystemImport => {
                    PersistedImportedSubKind::FileSystemImport
                }
                crate::graph::ImportedSubKind::ArchiveMembership => {
                    PersistedImportedSubKind::ArchiveMembership
                }
                crate::graph::ImportedSubKind::SharedCollection => {
                    PersistedImportedSubKind::SharedCollection
                }
            })
        }
        crate::graph::RelationSelector::Provenance(sub_kind) => {
            PersistedRelationSelector::Provenance(match sub_kind {
                crate::graph::ProvenanceSubKind::ClippedFrom => {
                    PersistedProvenanceSubKind::ClippedFrom
                }
                crate::graph::ProvenanceSubKind::ExcerptedFrom => {
                    PersistedProvenanceSubKind::ExcerptedFrom
                }
                crate::graph::ProvenanceSubKind::SummarizedFrom => {
                    PersistedProvenanceSubKind::SummarizedFrom
                }
                crate::graph::ProvenanceSubKind::TranslatedFrom => {
                    PersistedProvenanceSubKind::TranslatedFrom
                }
                crate::graph::ProvenanceSubKind::RewrittenFrom => {
                    PersistedProvenanceSubKind::RewrittenFrom
                }
                crate::graph::ProvenanceSubKind::GeneratedFrom => {
                    PersistedProvenanceSubKind::GeneratedFrom
                }
                crate::graph::ProvenanceSubKind::ExtractedFrom => {
                    PersistedProvenanceSubKind::ExtractedFrom
                }
                crate::graph::ProvenanceSubKind::ImportedFromSource => {
                    PersistedProvenanceSubKind::ImportedFromSource
                }
            })
        }
    })
}

/// Durable identifier for a rich note document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct NoteId(uuid::Uuid);

impl NoteId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }

    pub(crate) fn from_uuid(id: uuid::Uuid) -> Self {
        Self(id)
    }

    pub fn as_uuid(self) -> uuid::Uuid {
        self.0
    }
}

impl Default for NoteId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct NoteRecord {
    pub id: NoteId,
    pub title: String,
    pub linked_node: Option<NodeKey>,
    pub source_url: Option<String>,
    pub body: String,
    pub created_at: std::time::SystemTime,
    pub updated_at: std::time::SystemTime,
}

fn browser_family_key(family: &crate::services::import::BrowserFamily) -> String {
    match family {
        crate::services::import::BrowserFamily::Chrome => "chrome".to_string(),
        crate::services::import::BrowserFamily::Chromium => "chromium".to_string(),
        crate::services::import::BrowserFamily::Edge => "edge".to_string(),
        crate::services::import::BrowserFamily::Brave => "brave".to_string(),
        crate::services::import::BrowserFamily::Arc => "arc".to_string(),
        crate::services::import::BrowserFamily::Firefox => "firefox".to_string(),
        crate::services::import::BrowserFamily::Safari => "safari".to_string(),
        crate::services::import::BrowserFamily::Other(value) => value.trim().to_ascii_lowercase(),
    }
}

fn browser_import_source_kind_key(
    kind: &crate::services::import::BrowserImportSourceKind,
) -> &'static str {
    match kind {
        crate::services::import::BrowserImportSourceKind::BookmarkFile => "bookmark-file",
        crate::services::import::BrowserImportSourceKind::HistoryDatabase => "history-db",
        crate::services::import::BrowserImportSourceKind::SessionFile => "session-file",
        crate::services::import::BrowserImportSourceKind::NativeProfileReader => {
            "native-profile-reader"
        }
        crate::services::import::BrowserImportSourceKind::ExtensionBridge => "extension-bridge",
        crate::services::import::BrowserImportSourceKind::NativeMessagingBridge => {
            "native-messaging-bridge"
        }
    }
}

fn browser_import_source_id(run: &crate::services::import::BrowserImportRun) -> String {
    if let Some(stable_source_id) = &run.source.stable_source_id {
        let trimmed = stable_source_id.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }

    let mut source_id = format!(
        "browser-import:{}:{}",
        browser_family_key(&run.source.browser_family),
        browser_import_source_kind_key(&run.source.source_kind)
    );
    if let Some(profile_hint) = &run.source.profile_hint {
        let trimmed = profile_hint.trim();
        if !trimmed.is_empty() {
            source_id.push(':');
            source_id.push_str(trimmed);
        }
    }
    source_id
}

fn browser_import_source_label(run: &crate::services::import::BrowserImportRun) -> String {
    let trimmed = run.user_visible_label.trim();
    if !trimmed.is_empty() {
        return trimmed.to_string();
    }

    let family = browser_family_key(&run.source.browser_family);
    format!("{} {}", family, browser_import_source_kind_key(&run.source.source_kind))
}

fn browser_import_record_id(
    run: &crate::services::import::BrowserImportRun,
    source_id: &str,
) -> String {
    let trimmed = run.import_id.trim();
    if !trimmed.is_empty() {
        return trimmed.to_string();
    }
    format!("import-record:{source_id}:{}", run.observed_at_unix_secs.max(0))
}

fn browser_import_timestamp_secs(run: &crate::services::import::BrowserImportRun) -> u64 {
    run.observed_at_unix_secs.max(0) as u64
}

fn browser_import_node_url(kind: &str, seed: &str) -> String {
    crate::util::VersoAddress::Other {
        category: "import".to_string(),
        segments: vec![
            kind.to_string(),
            uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_URL, seed.as_bytes()).to_string(),
        ],
    }
    .to_string()
}

fn bookmark_folder_seed(
    source_id: &str,
    folder_path: &[crate::services::import::ImportedFolderSegment],
) -> String {
    let mut segments = Vec::with_capacity(folder_path.len());
    for segment in folder_path {
        segments.push(
            segment
                .stable_id
                .clone()
                .unwrap_or_else(|| segment.label.clone()),
        );
    }
    format!("bookmark-folder\n{source_id}\n{}", segments.join("/"))
}

fn imported_history_root_title(source_label: &str) -> String {
    format!("Imported History: {source_label}")
}

fn imported_session_root_title(
    source_label: &str,
    snapshot: &crate::services::import::ImportedBrowserSessionItem,
) -> String {
    format!("Imported Session Snapshot: {} ({})", source_label, snapshot.snapshot_id)
}

impl GraphBrowserApp {
    fn refresh_protocol_probe_for_node(&mut self, key: NodeKey, url: &str, enqueue_cancel: bool) {
        let protocol_resolution =
            crate::shell::desktop::runtime::registries::protocol::ProtocolRegistry::default()
                .resolve(url);
        let should_probe = matches!(
            crate::graph::address_kind_from_url(url),
            crate::graph::AddressKind::Http
        ) && protocol_resolution.inferred_mime_hint.is_none();
        if should_probe || enqueue_cancel {
            self.set_pending_protocol_probe(key, should_probe.then(|| url.to_string()));
        }
    }

    pub fn add_node_and_sync(
        &mut self,
        url: String,
        position: euclid::default::Point2D<f32>,
    ) -> NodeKey {
        let GraphDeltaResult::NodeAdded(key) =
            self.apply_graph_delta_and_sync(GraphDelta::AddNode {
                id: None,
                url: url.clone(),
                position,
            })
        else {
            unreachable!("add node delta must return NodeAdded");
        };
        if let Some(store) = &mut self.services.persistence
            && let Some(node) = self.workspace.domain.graph.get_node(key)
        {
            store.log_mutation(&LogEntry::AddNode {
                node_id: node.id.to_string(),
                url: url.clone(),
                position_x: position.x,
                position_y: position.y,
                timestamp_ms: Self::unix_timestamp_ms_now(),
            });
        }
        self.workspace.graph_runtime.physics.base.is_running = true;
        self.workspace.graph_runtime.drag_release_frames_remaining = 0;
        self.refresh_protocol_probe_for_node(key, &url, false);
        key
    }

    pub(crate) fn preferred_new_node_anchor(&self, anchor: Option<NodeKey>) -> Option<NodeKey> {
        anchor.or_else(|| {
            self.focused_selection().primary().and_then(|key| {
                crate::shell::desktop::runtime::registries::phase3_suggest_semantic_placement_anchor(
                    self, key,
                )
            })
        })
    }

    pub(crate) fn anchored_new_node_position(
        &self,
        anchor: NodeKey,
    ) -> Option<euclid::default::Point2D<f32>> {
        use rand::Rng;

        let base = self.domain_graph().node_projected_position(anchor)?;
        let mut rng = rand::thread_rng();
        let jitter_x = rng.gen_range(-50.0_f32..50.0_f32);
        let jitter_y = rng.gen_range(-50.0_f32..50.0_f32);
        Some(euclid::default::Point2D::new(
            base.x + 140.0 + jitter_x,
            base.y + 80.0 + jitter_y,
        ))
    }

    pub(crate) fn suggested_new_node_position(
        &self,
        anchor: Option<NodeKey>,
    ) -> euclid::default::Point2D<f32> {
        if let Some(position) = self
            .preferred_new_node_anchor(anchor)
            .and_then(|key| self.anchored_new_node_position(key))
        {
            return position;
        }

        let base = self
            .workspace
            .domain
            .graph
            .projected_centroid()
            .unwrap_or_else(|| euclid::default::Point2D::new(400.0, 300.0));
        let n = self.domain_graph().node_count() as f32;
        let angle = n * std::f32::consts::FRAC_PI_4;
        let radius = 90.0;
        euclid::default::Point2D::new(base.x + radius * angle.cos(), base.y + radius * angle.sin())
    }

    pub fn add_edge_and_sync(
        &mut self,
        from_key: NodeKey,
        to_key: NodeKey,
        edge_type: crate::graph::EdgeType,
        edge_label: Option<String>,
    ) -> Option<crate::graph::EdgeKey> {
        if let Some(assertion) = edge_type_to_assertion(edge_type, edge_label.clone()) {
            return self.assert_relation_and_sync(from_key, to_key, assertion);
        }
        let GraphDeltaResult::EdgeAdded(edge_key) =
            self.apply_graph_delta_and_sync(GraphDelta::AddEdge {
                from: from_key,
                to: to_key,
                edge_type,
                edge_label: edge_label.clone(),
            })
        else {
            unreachable!("add edge delta must return EdgeAdded");
        };
        if edge_key.is_some() {
            self.log_edge_mutation(from_key, to_key, edge_type, edge_label);
            self.workspace.graph_runtime.physics.base.is_running = true;
            self.workspace.graph_runtime.drag_release_frames_remaining = 0;
        }
        edge_key
    }

    pub(crate) fn import_webfinger_into_graph(
        &mut self,
        resource: &str,
        import: &graphshell_comms::webfinger::WebFingerImport,
        anchor: Option<NodeKey>,
    ) -> Result<NodeKey, String> {
        let normalized_resource = graphshell_comms::webfinger::normalize_resource(resource)?;
        let subject_url = graphshell_comms::webfinger::normalize_resource(&import.subject)
            .unwrap_or_else(|_| normalized_resource.clone());
        let subject_position = self.suggested_new_node_position(anchor);
        let subject_node = self.ensure_webfinger_import_node(
            subject_url.clone(),
            webfinger_subject_title(&subject_url),
            subject_position,
            &[TAG_WEBFINGER, TAG_DISCOVERY, TAG_IDENTITY],
        );

        let mut seen_urls = std::collections::HashSet::from([subject_url.clone()]);
        let mut alias_specs = Vec::new();
        let mut profile_specs = Vec::new();
        let mut gemini_specs = Vec::new();
        let mut gopher_specs = Vec::new();
        let mut misfin_specs = Vec::new();
        let mut nostr_specs = Vec::new();
        let mut activitypub_specs = Vec::new();
        let mut other_specs = Vec::new();

        let mut push_spec = |specs: &mut Vec<WebFingerNodeSpec>, spec: WebFingerNodeSpec| {
            let trimmed = spec.url.trim();
            if trimmed.is_empty() {
                return;
            }
            if seen_urls.insert(trimmed.to_string()) {
                specs.push(spec);
            }
        };

        if normalized_resource != subject_url {
            push_spec(
                &mut alias_specs,
                WebFingerNodeSpec {
                    title: webfinger_prefixed_title("Alias", &normalized_resource),
                    url: normalized_resource,
                    relation_label: "alias",
                    tags: &[TAG_WEBFINGER, TAG_DISCOVERY, TAG_ALIAS],
                },
            );
        }

        for alias in &import.aliases {
            push_spec(
                &mut alias_specs,
                WebFingerNodeSpec {
                    title: webfinger_prefixed_title("Alias", alias),
                    url: alias.clone(),
                    relation_label: "alias",
                    tags: &[TAG_WEBFINGER, TAG_DISCOVERY, TAG_ALIAS],
                },
            );
        }

        for profile in &import.profile_pages {
            push_spec(
                &mut profile_specs,
                WebFingerNodeSpec {
                    title: webfinger_prefixed_title("Profile", profile),
                    url: profile.clone(),
                    relation_label: "profile",
                    tags: &[TAG_WEBFINGER, TAG_DISCOVERY, TAG_PROFILE],
                },
            );
        }

        for capsule in &import.gemini_capsules {
            push_spec(
                &mut gemini_specs,
                WebFingerNodeSpec {
                    title: webfinger_prefixed_title("Gemini capsule", capsule),
                    url: capsule.clone(),
                    relation_label: "gemini",
                    tags: &[TAG_WEBFINGER, TAG_DISCOVERY, TAG_GEMINI],
                },
            );
        }

        for resource in &import.gopher_resources {
            push_spec(
                &mut gopher_specs,
                WebFingerNodeSpec {
                    title: webfinger_prefixed_title("Gopher resource", resource),
                    url: resource.clone(),
                    relation_label: "gopher",
                    tags: &[TAG_WEBFINGER, TAG_DISCOVERY, TAG_GOPHER],
                },
            );
        }

        for mailbox in &import.misfin_mailboxes {
            push_spec(
                &mut misfin_specs,
                WebFingerNodeSpec {
                    title: webfinger_prefixed_title("Misfin mailbox", mailbox),
                    url: mailbox.clone(),
                    relation_label: "misfin",
                    tags: &[TAG_WEBFINGER, TAG_DISCOVERY, TAG_MISFIN],
                },
            );
        }

        for identity in &import.nostr_identities {
            push_spec(
                &mut nostr_specs,
                WebFingerNodeSpec {
                    title: webfinger_prefixed_title("Nostr identity", identity),
                    url: identity.clone(),
                    relation_label: "nostr",
                    tags: &[TAG_WEBFINGER, TAG_DISCOVERY, TAG_NOSTR],
                },
            );
        }

        for actor in &import.activitypub_actors {
            push_spec(
                &mut activitypub_specs,
                WebFingerNodeSpec {
                    title: webfinger_prefixed_title("ActivityPub actor", actor),
                    url: actor.clone(),
                    relation_label: "activitypub",
                    tags: &[TAG_WEBFINGER, TAG_DISCOVERY, TAG_ACTIVITYPUB],
                },
            );
        }

        for endpoint in &import.other_endpoints {
            push_spec(
                &mut other_specs,
                WebFingerNodeSpec {
                    title: webfinger_endpoint_title(endpoint),
                    url: endpoint.href.clone(),
                    relation_label: "endpoint",
                    tags: &[TAG_WEBFINGER, TAG_DISCOVERY, TAG_ENDPOINT],
                },
            );
        }

        let total_specs = alias_specs.len()
            + profile_specs.len()
            + gemini_specs.len()
            + gopher_specs.len()
            + misfin_specs.len()
            + nostr_specs.len()
            + activitypub_specs.len()
            + other_specs.len();
        let mut index = 0usize;

        for specs in [
            &alias_specs,
            &profile_specs,
            &gemini_specs,
            &gopher_specs,
            &misfin_specs,
            &nostr_specs,
            &activitypub_specs,
            &other_specs,
        ] {
            for spec in specs.iter() {
                let position = webfinger_cluster_position(subject_position, index, total_specs);
                index += 1;
                let node_key = self.ensure_webfinger_import_node(
                    spec.url.clone(),
                    spec.title.clone(),
                    position,
                    spec.tags,
                );
                if node_key == subject_node {
                    continue;
                }
                let _ = self.add_edge_and_sync(subject_node, node_key, crate::graph::EdgeType::Hyperlink, None);
                let _ = self.add_edge_and_sync(
                    subject_node,
                    node_key,
                    crate::graph::EdgeType::UserGrouped,
                    Some(spec.relation_label.to_string()),
                );
            }
        }

        self.select_node(subject_node, false);
        Ok(subject_node)
    }

    pub(crate) fn apply_browser_import_batch(
        &mut self,
        batch: &crate::services::import::BrowserImportBatch,
        anchor: Option<NodeKey>,
    ) {
        if batch.items.is_empty() {
            return;
        }

        let source_id = browser_import_source_id(&batch.run);
        let source_label = browser_import_source_label(&batch.run);
        let record_id = browser_import_record_id(&batch.run, &source_id);
        let imported_at_secs = browser_import_timestamp_secs(&batch.run);
        let mut imported_node_ids = std::collections::BTreeSet::<String>::new();

        let history_root = if batch.items.iter().any(|item| {
            matches!(
                item,
                crate::services::import::BrowserImportPayload::HistoryVisit(_)
            )
        }) {
            let url = browser_import_node_url(
                "history",
                &format!("history-root\n{source_id}\n{record_id}"),
            );
            let key = self.ensure_browser_import_structure_node(
                url,
                imported_history_root_title(&source_label),
                anchor,
            );
            self.collect_browser_import_node_id(key, &mut imported_node_ids);
            Some(key)
        } else {
            None
        };

        for item in &batch.items {
            match item {
                crate::services::import::BrowserImportPayload::Bookmark(bookmark) => {
                    let page_key = self.ensure_browser_import_page_node(&bookmark.page, anchor);
                    self.apply_node_tags(page_key, &[GraphBrowserApp::TAG_STARRED]);
                    self.collect_browser_import_node_id(page_key, &mut imported_node_ids);

                    let mut parent_key = None;
                    for depth in 0..bookmark.folder_path.len() {
                        let folder_path = &bookmark.folder_path[..=depth];
                        let folder = folder_path.last().expect("folder path segment");
                        let folder_key = self.ensure_browser_import_structure_node(
                            browser_import_node_url(
                                "bookmark-folder",
                                &bookmark_folder_seed(&source_id, folder_path),
                            ),
                            folder.label.clone(),
                            anchor,
                        );
                        self.collect_browser_import_node_id(folder_key, &mut imported_node_ids);
                        if let Some(parent) = parent_key {
                            let _ = self.assert_relation_and_sync(
                                parent,
                                folder_key,
                                crate::graph::EdgeAssertion::Imported {
                                    sub_kind: crate::graph::ImportedSubKind::BookmarkFolder,
                                },
                            );
                        }
                        parent_key = Some(folder_key);
                    }

                    if let Some(parent) = parent_key {
                        let _ = self.assert_relation_and_sync(
                            parent,
                            page_key,
                            crate::graph::EdgeAssertion::Imported {
                                sub_kind: crate::graph::ImportedSubKind::BookmarkFolder,
                            },
                        );
                    }
                }
                crate::services::import::BrowserImportPayload::HistoryVisit(visit) => {
                    let page_key = self.ensure_browser_import_page_node(&visit.page, anchor);
                    self.collect_browser_import_node_id(page_key, &mut imported_node_ids);
                    if let Some(root_key) = history_root {
                        let _ = self.assert_relation_and_sync(
                            root_key,
                            page_key,
                            crate::graph::EdgeAssertion::Imported {
                                sub_kind: crate::graph::ImportedSubKind::HistoryImport,
                            },
                        );
                    }
                }
                crate::services::import::BrowserImportPayload::SessionSnapshot(snapshot) => {
                    let root_key = self.ensure_browser_import_structure_node(
                        browser_import_node_url(
                            "session",
                            &format!("session-root\n{source_id}\n{}", snapshot.snapshot_id),
                        ),
                        imported_session_root_title(&source_label, snapshot),
                        anchor,
                    );
                    self.collect_browser_import_node_id(root_key, &mut imported_node_ids);
                    for window in &snapshot.windows {
                        for tab in &window.tabs {
                            let page_key = self.ensure_browser_import_page_node(&tab.page, anchor);
                            self.collect_browser_import_node_id(page_key, &mut imported_node_ids);
                            let _ = self.assert_relation_and_sync(
                                root_key,
                                page_key,
                                crate::graph::EdgeAssertion::Imported {
                                    sub_kind: crate::graph::ImportedSubKind::SessionImport,
                                },
                            );
                        }
                    }
                }
            }
        }

        let mut import_records = self.workspace.domain.graph.import_records().to_vec();
        let mut updated = false;
        if !imported_node_ids.is_empty() {
            let memberships = imported_node_ids
                .into_iter()
                .map(|node_id| crate::graph::ImportRecordMembership {
                    node_id,
                    suppressed: false,
                })
                .collect::<Vec<_>>();

            if let Some(existing) = import_records
                .iter_mut()
                .find(|record| record.record_id == record_id)
            {
                if existing.source_id.is_empty() {
                    existing.source_id = source_id.clone();
                }
                if existing.source_label.is_empty() {
                    existing.source_label = source_label.clone();
                }
                if existing.imported_at_secs == 0 {
                    existing.imported_at_secs = imported_at_secs;
                }
                existing.memberships.extend(memberships);
            } else {
                import_records.push(crate::graph::ImportRecord {
                    record_id,
                    source_id,
                    source_label,
                    imported_at_secs,
                    memberships,
                });
            }
            updated = self.workspace.domain.graph.set_import_records(import_records);
        }

        if updated {
            self.workspace.graph_runtime.egui_state_dirty = true;
        }
    }

    pub(crate) fn fetch_and_import_webfinger_into_graph(
        &mut self,
        resource: &str,
        anchor: Option<NodeKey>,
    ) -> Result<NodeKey, String> {
        let import = graphshell_comms::webfinger::fetch_import(resource)?;
        self.import_webfinger_into_graph(resource, &import, anchor)
    }

    pub(crate) fn import_person_identity_into_graph(
        &mut self,
        profile: &graphshell_comms::identity::PersonIdentityProfile,
        anchor: Option<NodeKey>,
    ) -> Result<NodeKey, String> {
        let person_position = self.suggested_new_node_position(anchor);
        let person_key = if let Some(existing_key) = self.find_person_node_for_identity_profile(profile) {
            self.set_node_title_if_empty_or_url_and_log(existing_key, person_title(profile));
            self.apply_node_tags(existing_key, &[TAG_PERSON, TAG_IDENTITY]);
            existing_key
        } else {
            let person_url = person_node_url(profile)?;
            self.ensure_webfinger_import_node(
                person_url,
                person_title(profile),
                person_position,
                &[TAG_PERSON, TAG_IDENTITY],
            )
        };

        self.ensure_person_identity_classifications(person_key, profile);
        let mut person_tags = vec![TAG_PERSON, TAG_IDENTITY];
        if profile.webfinger_resource.is_some() || profile.human_handle.is_some() {
            person_tags.push(TAG_WEBFINGER);
        }
        if profile.nip05_identifier.is_some() {
            person_tags.push(TAG_NIP05);
        }
        if !profile.matrix_mxids.is_empty() {
            person_tags.push(TAG_MATRIX);
        }
        if !profile.nostr_identities.is_empty() {
            person_tags.push(TAG_NOSTR);
        }
        if !profile.misfin_mailboxes.is_empty() {
            person_tags.push(TAG_MISFIN);
        }
        if !profile.activitypub_actors.is_empty() {
            person_tags.push(TAG_ACTIVITYPUB);
        }
        if !profile.gemini_capsules.is_empty() {
            person_tags.push(TAG_GEMINI);
        }
        self.apply_node_tags(person_key, &person_tags);

        let mut specs = Vec::new();
        let mut push_spec = |spec: PersonIdentityNodeSpec| {
            if !spec.url.trim().is_empty() && !specs.iter().any(|existing: &PersonIdentityNodeSpec| existing.url == spec.url) {
                specs.push(spec);
            }
        };

        if let Some(resource) = &profile.webfinger_resource {
            push_spec(PersonIdentityNodeSpec {
                title: person_identity_node_title("WebFinger identity", resource),
                url: resource.clone(),
                relation_label: "webfinger",
                tags: &[TAG_WEBFINGER, TAG_IDENTITY],
                relation: PersonIdentityRelation::SameEntityAs,
            });
        }
        if let Some(nip05_identifier) = &profile.nip05_identifier {
            push_spec(PersonIdentityNodeSpec {
                title: person_identity_node_title("NIP-05 identity", nip05_identifier),
                url: format!("nip05:{nip05_identifier}"),
                relation_label: "nip05",
                tags: &[TAG_NIP05, TAG_IDENTITY, TAG_NOSTR],
                relation: PersonIdentityRelation::SameEntityAs,
            });
        }
        for mxid in &profile.matrix_mxids {
            push_spec(PersonIdentityNodeSpec {
                title: person_identity_node_title("Matrix identity", mxid),
                url: format!("mxid:{mxid}"),
                relation_label: "matrix",
                tags: &[TAG_MATRIX, TAG_IDENTITY],
                relation: PersonIdentityRelation::SameEntityAs,
            });
        }
        for identity in &profile.nostr_identities {
            push_spec(PersonIdentityNodeSpec {
                title: person_identity_node_title("Nostr identity", identity),
                url: identity.clone(),
                relation_label: "nostr",
                tags: &[TAG_NOSTR, TAG_IDENTITY],
                relation: PersonIdentityRelation::SameEntityAs,
            });
        }
        for mailbox in &profile.misfin_mailboxes {
            push_spec(PersonIdentityNodeSpec {
                title: person_identity_node_title("Misfin mailbox", mailbox),
                url: mailbox.clone(),
                relation_label: "misfin",
                tags: &[TAG_MISFIN, TAG_IDENTITY],
                relation: PersonIdentityRelation::SameEntityAs,
            });
        }
        for actor in &profile.activitypub_actors {
            push_spec(PersonIdentityNodeSpec {
                title: person_identity_node_title("ActivityPub actor", actor),
                url: actor.clone(),
                relation_label: "activitypub",
                tags: &[TAG_ACTIVITYPUB, TAG_IDENTITY],
                relation: PersonIdentityRelation::SameEntityAs,
            });
        }
        for alias in &profile.aliases {
            push_spec(PersonIdentityNodeSpec {
                title: person_identity_node_title("Alias", alias),
                url: alias.clone(),
                relation_label: "alias",
                tags: &[TAG_ALIAS, TAG_IDENTITY],
                relation: PersonIdentityRelation::SameEntityAs,
            });
        }
        for page in &profile.profile_pages {
            push_spec(PersonIdentityNodeSpec {
                title: person_identity_node_title("Profile", page),
                url: page.clone(),
                relation_label: "profile",
                tags: &[TAG_PROFILE, TAG_IDENTITY],
                relation: PersonIdentityRelation::CanonicalMirrorOf,
            });
        }
        for capsule in &profile.gemini_capsules {
            push_spec(PersonIdentityNodeSpec {
                title: person_identity_node_title("Gemini capsule", capsule),
                url: capsule.clone(),
                relation_label: "gemini",
                tags: &[TAG_GEMINI, TAG_PROFILE, TAG_IDENTITY],
                relation: PersonIdentityRelation::CanonicalMirrorOf,
            });
        }
        for resource in &profile.gopher_resources {
            push_spec(PersonIdentityNodeSpec {
                title: person_identity_node_title("Gopher resource", resource),
                url: resource.clone(),
                relation_label: "gopher",
                tags: &[TAG_GOPHER, TAG_PROFILE, TAG_IDENTITY],
                relation: PersonIdentityRelation::CanonicalMirrorOf,
            });
        }
        for endpoint in &profile.other_endpoints {
            push_spec(PersonIdentityNodeSpec {
                title: webfinger_endpoint_title(endpoint),
                url: endpoint.href.clone(),
                relation_label: "endpoint",
                tags: &[TAG_ENDPOINT, TAG_IDENTITY],
                relation: PersonIdentityRelation::GroupedOnly,
            });
        }

        let total_specs = specs.len();
        for (index, spec) in specs.iter().enumerate() {
            let position = webfinger_cluster_position(person_position, index, total_specs);
            let node_key = self.ensure_webfinger_import_node(
                spec.url.clone(),
                spec.title.clone(),
                position,
                spec.tags,
            );
            if node_key == person_key {
                continue;
            }
            let _ = self.add_edge_and_sync(person_key, node_key, crate::graph::EdgeType::Hyperlink, None);
            let _ = self.add_edge_and_sync(
                person_key,
                node_key,
                crate::graph::EdgeType::UserGrouped,
                Some(spec.relation_label.to_string()),
            );

            let semantic_relation = match spec.relation {
                PersonIdentityRelation::SameEntityAs => Some(crate::graph::SemanticSubKind::SameEntityAs),
                PersonIdentityRelation::CanonicalMirrorOf => Some(crate::graph::SemanticSubKind::CanonicalMirrorOf),
                PersonIdentityRelation::GroupedOnly => None,
            };
            if let Some(sub_kind) = semantic_relation {
                let _ = self.assert_relation_and_sync(
                    node_key,
                    person_key,
                    crate::graph::EdgeAssertion::Semantic {
                        sub_kind,
                        label: Some(spec.relation_label.to_string()),
                        decay_progress: None,
                    },
                );
            }
        }

        self.select_node(person_key, false);
        Ok(person_key)
    }

    pub(crate) fn import_person_identity_from_webfinger(
        &mut self,
        resource: &str,
        import: &graphshell_comms::webfinger::WebFingerImport,
        anchor: Option<NodeKey>,
    ) -> Result<NodeKey, String> {
        let profile = graphshell_comms::identity::PersonIdentityProfile::from_webfinger_import(
            resource,
            import,
        )?;
        self.import_person_identity_into_graph(&profile, anchor)
    }

    pub(crate) fn fetch_and_import_person_identity_from_webfinger(
        &mut self,
        resource: &str,
        anchor: Option<NodeKey>,
    ) -> Result<NodeKey, String> {
        self.resolve_and_import_person_identity(
            graphshell_comms::capabilities::MiddlenetProtocol::WebFinger,
            resource,
            anchor,
        )
    }

    pub(crate) fn resolve_and_import_person_identity_from_nip05(
        &mut self,
        identifier: &str,
        anchor: Option<NodeKey>,
    ) -> Result<NodeKey, String> {
        self.resolve_and_import_person_identity(
            graphshell_comms::capabilities::MiddlenetProtocol::Nip05,
            identifier,
            anchor,
        )
    }

    pub(crate) fn resolve_and_import_person_identity_from_matrix(
        &mut self,
        mxid: &str,
        anchor: Option<NodeKey>,
    ) -> Result<NodeKey, String> {
        self.resolve_and_import_person_identity(
            graphshell_comms::capabilities::MiddlenetProtocol::Matrix,
            mxid,
            anchor,
        )
    }

    pub(crate) fn resolve_and_import_person_identity_from_activitypub(
        &mut self,
        actor_url: &str,
        anchor: Option<NodeKey>,
    ) -> Result<NodeKey, String> {
        self.resolve_and_import_person_identity(
            graphshell_comms::capabilities::MiddlenetProtocol::ActivityPub,
            actor_url,
            anchor,
        )
    }

    pub(crate) fn refresh_person_identity_resolutions(
        &mut self,
        person_key: NodeKey,
    ) -> Result<PersonIdentityRefreshOutcome, String> {
        if !self.node_has_canonical_tag(person_key, TAG_PERSON) {
            return Err("Selected node is not a person node.".to_string());
        }
        let queries = self.person_resolution_queries(person_key);
        if queries.is_empty() {
            return Err("Person node has no recorded resolution provenance.".to_string());
        }

        let before = self.person_identity_refresh_fingerprint(person_key);
        let mut current_person = person_key;
        for (protocol, query) in &queries {
            let resolved = graphshell_comms::identity::refresh_person_identity_profile(*protocol, query)?;
            current_person = self.import_person_identity_into_graph(&resolved.profile, Some(current_person))?;
            self.record_identity_resolution_provenance(
                current_person,
                &resolved,
                graphshell_comms::identity::IdentityResolutionActionKind::Refresh,
                None,
            );
        }
        let after = self.person_identity_refresh_fingerprint(current_person);
        let changed = before != after;
        self.log_node_audit_event(
            current_person,
            crate::services::persistence::types::NodeAuditEventKind::ActionRecorded {
                action: "Identity refresh".to_string(),
                detail: format!(
                    "refreshed {} protocol(s); changed={}",
                    queries.len(),
                    if changed { "yes" } else { "no" }
                ),
            },
        );

        Ok(PersonIdentityRefreshOutcome {
            person_key: current_person,
            refreshed_protocols: queries.len(),
            changed,
        })
    }

    pub(crate) fn deliver_person_message_notification_via_misfin(
        &mut self,
        person_key: NodeKey,
        sender: &graphshell_comms::misfin::MisfinIdentitySpec,
        message: &str,
        anchor: Option<NodeKey>,
    ) -> Result<(
        NodeKey,
        graphshell_comms::misfin::MisfinSendOutcome,
    ), String> {
        let protocol = graphshell_comms::capabilities::primary_protocol_for_capability(
            graphshell_comms::capabilities::ProtocolCapability::DeliverMessage,
        )
        .ok_or_else(|| "No Middlenet delivery protocol is configured.".to_string())?;
        let mailbox = self
            .person_identity_value_for_capability(
                person_key,
                graphshell_comms::capabilities::ProtocolCapability::DeliverMessage,
            )
            .map(|(_, value)| value)
            .ok_or_else(|| {
                let label = graphshell_comms::capabilities::descriptor(protocol)
                    .identity_requirement_label
                    .unwrap_or("delivery identity");
                format!("Person node is missing a {label}.")
            })?;
        let mailbox_url = url::Url::parse(&mailbox)
            .map_err(|error| format!("Invalid Misfin mailbox '{mailbox}': {error}"))?;
        let outcome = graphshell_comms::misfin::send_message(&mailbox_url, sender, message)?;
        let artifact_url = graphshell_comms::misfin::url_string_for_address(
            &outcome.final_recipient,
            None,
        );
        let artifact_key = self.create_person_artifact_node(
            person_key,
            graphshell_comms::identity::PersonArtifactKind::MessageNotification,
            None,
            Some(artifact_url),
            anchor,
        )?;
        Ok((artifact_key, outcome))
    }

    pub(crate) fn publish_person_artifact_via_titan(
        &mut self,
        person_key: NodeKey,
        kind: graphshell_comms::identity::PersonArtifactKind,
        target_url: Option<&str>,
        content: &[u8],
        mime: Option<&str>,
        token: Option<&str>,
        title: Option<String>,
        anchor: Option<NodeKey>,
    ) -> Result<(
        NodeKey,
        graphshell_comms::transport::TitanUploadOutcome,
    ), String> {
        let resolved_target = if let Some(target_url) = target_url.map(str::trim).filter(|value| !value.is_empty()) {
            target_url.to_string()
        } else {
            let protocol = graphshell_comms::capabilities::primary_protocol_for_capability(
                graphshell_comms::capabilities::ProtocolCapability::PublishArtifact,
            )
            .ok_or_else(|| "No Middlenet publication protocol is configured.".to_string())?;
            self.person_identity_value_for_capability(
                person_key,
                graphshell_comms::capabilities::ProtocolCapability::PublishArtifact,
            )
            .map(|(_, value)| value)
            .ok_or_else(|| {
                let label = graphshell_comms::capabilities::descriptor(protocol)
                    .identity_requirement_label
                    .unwrap_or("publication endpoint");
                format!("Person node is missing a {label}.")
            })?
        };
        let parsed_target = url::Url::parse(&resolved_target)
            .map_err(|error| format!("Invalid Titan target URL '{resolved_target}': {error}"))?;
        let outcome = graphshell_comms::transport::titan_upload(
            &parsed_target,
            content,
            mime,
            token,
        )?;
        let artifact_key = self.create_person_artifact_node(
            person_key,
            kind,
            title,
            Some(resolved_target),
            anchor,
        )?;
        Ok((artifact_key, outcome))
    }

    #[cfg(test)]
    pub(crate) fn deliver_person_message_notification_via_misfin_for_tests(
        &mut self,
        person_key: NodeKey,
        sender: &graphshell_comms::misfin::MisfinIdentitySpec,
        message: &str,
        anchor: Option<NodeKey>,
        known_hosts_path: &std::path::Path,
        identity_root: &std::path::Path,
    ) -> Result<(
        NodeKey,
        graphshell_comms::misfin::MisfinSendOutcome,
    ), String> {
        let protocol = graphshell_comms::capabilities::primary_protocol_for_capability(
            graphshell_comms::capabilities::ProtocolCapability::DeliverMessage,
        )
        .ok_or_else(|| "No Middlenet delivery protocol is configured.".to_string())?;
        let mailbox = self
            .person_identity_value_for_capability(
                person_key,
                graphshell_comms::capabilities::ProtocolCapability::DeliverMessage,
            )
            .map(|(_, value)| value)
            .ok_or_else(|| {
                let label = graphshell_comms::capabilities::descriptor(protocol)
                    .identity_requirement_label
                    .unwrap_or("delivery identity");
                format!("Person node is missing a {label}.")
            })?;
        let mailbox_url = url::Url::parse(&mailbox)
            .map_err(|error| format!("Invalid Misfin mailbox '{mailbox}': {error}"))?;
        let outcome = graphshell_comms::misfin::send_message_for_tests(
            &mailbox_url,
            sender,
            message,
            known_hosts_path,
            identity_root,
        )?;
        let artifact_url = graphshell_comms::misfin::url_string_for_address(
            &outcome.final_recipient,
            None,
        );
        let artifact_key = self.create_person_artifact_node(
            person_key,
            graphshell_comms::identity::PersonArtifactKind::MessageNotification,
            None,
            Some(artifact_url),
            anchor,
        )?;
        Ok((artifact_key, outcome))
    }

    #[cfg(test)]
    pub(crate) fn publish_person_artifact_via_titan_for_tests(
        &mut self,
        person_key: NodeKey,
        kind: graphshell_comms::identity::PersonArtifactKind,
        target_url: Option<&str>,
        content: &[u8],
        mime: Option<&str>,
        token: Option<&str>,
        title: Option<String>,
        anchor: Option<NodeKey>,
        known_hosts_path: &std::path::Path,
    ) -> Result<(
        NodeKey,
        graphshell_comms::transport::TitanUploadOutcome,
    ), String> {
        let resolved_target = if let Some(target_url) = target_url.map(str::trim).filter(|value| !value.is_empty()) {
            target_url.to_string()
        } else {
            let protocol = graphshell_comms::capabilities::primary_protocol_for_capability(
                graphshell_comms::capabilities::ProtocolCapability::PublishArtifact,
            )
            .ok_or_else(|| "No Middlenet publication protocol is configured.".to_string())?;
            self.person_identity_value_for_capability(
                person_key,
                graphshell_comms::capabilities::ProtocolCapability::PublishArtifact,
            )
            .map(|(_, value)| value)
            .ok_or_else(|| {
                let label = graphshell_comms::capabilities::descriptor(protocol)
                    .identity_requirement_label
                    .unwrap_or("publication endpoint");
                format!("Person node is missing a {label}.")
            })?
        };
        let parsed_target = url::Url::parse(&resolved_target)
            .map_err(|error| format!("Invalid Titan target URL '{resolved_target}': {error}"))?;
        let outcome = graphshell_comms::transport::titan_upload_for_tests(
            &parsed_target,
            content,
            mime,
            token,
            known_hosts_path,
        )?;
        let artifact_key = self.create_person_artifact_node(
            person_key,
            kind,
            title,
            Some(resolved_target),
            anchor,
        )?;
        Ok((artifact_key, outcome))
    }

    pub(crate) fn create_person_artifact_node(
        &mut self,
        person_key: NodeKey,
        kind: graphshell_comms::identity::PersonArtifactKind,
        title: Option<String>,
        url: Option<String>,
        anchor: Option<NodeKey>,
    ) -> Result<NodeKey, String> {
        let (person_url, person_title_value) = self
            .domain_graph()
            .get_node(person_key)
            .map(|node| (node.url().to_string(), node.title.clone()))
            .ok_or_else(|| format!("Unknown person node {:?}.", person_key))?;
        if !self.node_has_canonical_tag(person_key, TAG_PERSON) {
            return Err(format!(
                "Node '{}' is not a canonical person node.",
                person_url
            ));
        }

        let artifact_url = url.unwrap_or_else(|| person_artifact_url(&person_url, kind));
        let artifact_position = self.suggested_new_node_position(anchor.or(Some(person_key)));
        let artifact_key = self.add_node_and_sync(artifact_url, artifact_position);
        let resolved_title = title.unwrap_or_else(|| {
            format!(
                "{} from {}",
                kind.title_prefix(),
                webfinger_display_target(&person_title_value)
            )
        });
        self.set_node_title_if_empty_or_url_and_log(artifact_key, resolved_title);
        let mut tags = vec![TAG_PERSON_ARTIFACT, TAG_IDENTITY];
        match kind {
            graphshell_comms::identity::PersonArtifactKind::Post => tags.push(TAG_POST),
            graphshell_comms::identity::PersonArtifactKind::SharedData => tags.push(TAG_SHARED_DATA),
            graphshell_comms::identity::PersonArtifactKind::MessageNotification => {
                tags.push(TAG_MESSAGE_NOTIFICATION)
            }
        }
        self.apply_node_tags(artifact_key, &tags);
        let _ = self.add_edge_and_sync(
            person_key,
            artifact_key,
            crate::graph::EdgeType::UserGrouped,
            Some(kind.relation_label().to_string()),
        );
        let _ = self.assert_relation_and_sync(
            artifact_key,
            person_key,
            crate::graph::EdgeAssertion::Provenance {
                sub_kind: crate::graph::ProvenanceSubKind::GeneratedFrom,
            },
        );
        Ok(artifact_key)
    }

    pub fn assert_relation_and_sync(
        &mut self,
        from_key: NodeKey,
        to_key: NodeKey,
        assertion: crate::graph::EdgeAssertion,
    ) -> Option<crate::graph::EdgeKey> {
        let GraphDeltaResult::EdgeAdded(edge_key) =
            self.apply_graph_delta_and_sync(GraphDelta::AssertRelation {
                from: from_key,
                to: to_key,
                assertion: assertion.clone(),
            })
        else {
            unreachable!("assert relation delta must return EdgeAdded");
        };
        if edge_key.is_some() {
            self.log_relation_assertion(from_key, to_key, assertion);
            self.workspace.graph_runtime.physics.base.is_running = true;
            self.workspace.graph_runtime.drag_release_frames_remaining = 0;
        }
        edge_key
    }

    pub fn remove_edges_and_log(
        &mut self,
        from_key: NodeKey,
        to_key: NodeKey,
        edge_type: crate::graph::EdgeType,
    ) -> usize {
        if edge_type == crate::graph::EdgeType::History {
            let mut emitted_dissolved_append = false;
            let removed = if let Some(store) = &mut self.services.persistence {
                let dissolved_before = store.dissolved_archive_len();
                let removed = store
                    .dissolve_and_remove_edges(
                        &mut self.workspace.domain.graph,
                        from_key,
                        to_key,
                        edge_type,
                    )
                    .unwrap_or_else(|e| {
                        log::warn!(
                            "Dissolution transfer failed, falling back to direct removal: {e}"
                        );
                        self.workspace
                            .domain
                            .graph
                            .remove_edges(from_key, to_key, edge_type)
                    });
                let dissolved_after = store.dissolved_archive_len();
                emitted_dissolved_append = dissolved_after > dissolved_before;
                removed
            } else {
                self.workspace
                    .domain
                    .graph
                    .remove_edges(from_key, to_key, edge_type)
            };

            if emitted_dissolved_append {
                self.workspace.graph_runtime.history_last_event_unix_ms =
                    Some(Self::unix_timestamp_ms_now());
                emit_event(DiagnosticEvent::MessageReceived {
                    channel_id: CHANNEL_HISTORY_ARCHIVE_DISSOLVED_APPENDED,
                    latency_us: 0,
                });
            }

            if removed > 0 {
                self.log_edge_removal_mutation(from_key, to_key, edge_type);
                self.workspace.graph_runtime.egui_state_dirty = true;
                self.workspace.graph_runtime.physics.base.is_running = true;
                self.workspace.graph_runtime.drag_release_frames_remaining = 0;
            }
            return removed;
        }

        if let Some(selector) = edge_type_to_selector(edge_type) {
            return self.retract_relations_and_log(from_key, to_key, selector);
        }
        let mut emitted_dissolved_append = false;
        let removed = if let Some(store) = &mut self.services.persistence {
            let dissolved_before = store.dissolved_archive_len();
            let removed = store
                .dissolve_and_remove_edges(
                    &mut self.workspace.domain.graph,
                    from_key,
                    to_key,
                    edge_type,
                )
                .unwrap_or_else(|e| {
                    log::warn!("Dissolution transfer failed, falling back to direct removal: {e}");
                    self.workspace
                        .domain
                        .graph
                        .remove_edges(from_key, to_key, edge_type)
                });
            let dissolved_after = store.dissolved_archive_len();
            emitted_dissolved_append = dissolved_after > dissolved_before;
            removed
        } else {
            self.workspace
                .domain
                .graph
                .remove_edges(from_key, to_key, edge_type)
        };

        if emitted_dissolved_append {
            self.workspace.graph_runtime.history_last_event_unix_ms =
                Some(Self::unix_timestamp_ms_now());
            emit_event(DiagnosticEvent::MessageReceived {
                channel_id: CHANNEL_HISTORY_ARCHIVE_DISSOLVED_APPENDED,
                latency_us: 0,
            });
        }

        if removed > 0 {
            self.log_edge_removal_mutation(from_key, to_key, edge_type);
            self.workspace.graph_runtime.egui_state_dirty = true;
            self.workspace.graph_runtime.physics.base.is_running = true;
            self.workspace.graph_runtime.drag_release_frames_remaining = 0;
        }
        removed
    }

    pub fn retract_relations_and_log(
        &mut self,
        from_key: NodeKey,
        to_key: NodeKey,
        selector: crate::graph::RelationSelector,
    ) -> usize {
        let GraphDeltaResult::EdgesRemoved(removed) =
            self.apply_graph_delta_and_sync(GraphDelta::RetractRelations {
                from: from_key,
                to: to_key,
                selector,
            })
        else {
            unreachable!("retract relations delta must return EdgesRemoved");
        };
        if removed > 0 {
            self.log_relation_retraction(from_key, to_key, selector);
            self.workspace.graph_runtime.egui_state_dirty = true;
            self.workspace.graph_runtime.physics.base.is_running = true;
            self.workspace.graph_runtime.drag_release_frames_remaining = 0;
        }
        removed
    }

    fn log_relation_assertion(
        &mut self,
        from_key: NodeKey,
        to_key: NodeKey,
        assertion: crate::graph::EdgeAssertion,
    ) {
        if let Some(store) = &mut self.services.persistence {
            let from_id = self
                .workspace
                .domain
                .graph
                .get_node(from_key)
                .map(|n| n.id.to_string());
            let to_id = self
                .workspace
                .domain
                .graph
                .get_node(to_key)
                .map(|n| n.id.to_string());
            let (Some(from_node_id), Some(to_node_id)) = (from_id, to_id) else {
                return;
            };
            store.log_mutation(&LogEntry::AddEdge {
                from_node_id,
                to_node_id,
                assertion: persisted_assertion_from_graph_assertion(assertion),
            });
        }
    }

    fn log_relation_retraction(
        &mut self,
        from_key: NodeKey,
        to_key: NodeKey,
        selector: crate::graph::RelationSelector,
    ) {
        if let Some(store) = &mut self.services.persistence {
            let from_id = self
                .workspace
                .domain
                .graph
                .get_node(from_key)
                .map(|n| n.id.to_string());
            let to_id = self
                .workspace
                .domain
                .graph
                .get_node(to_key)
                .map(|n| n.id.to_string());
            let (Some(from_node_id), Some(to_node_id)) = (from_id, to_id) else {
                return;
            };
            let Some(selector) = persisted_selector_from_graph_selector(selector) else {
                return;
            };
            store.log_mutation(&LogEntry::RemoveEdge {
                from_node_id,
                to_node_id,
                selector,
            });
        }
    }

    pub fn log_edge_mutation(
        &mut self,
        from_key: NodeKey,
        to_key: NodeKey,
        edge_type: crate::graph::EdgeType,
        edge_label: Option<String>,
    ) {
        if let Some(assertion) = edge_type_to_assertion(edge_type, edge_label) {
            self.log_relation_assertion(from_key, to_key, assertion);
        }
    }

    pub fn log_edge_removal_mutation(
        &mut self,
        from_key: NodeKey,
        to_key: NodeKey,
        edge_type: crate::graph::EdgeType,
    ) {
        if let Some(selector) = edge_type_to_selector(edge_type) {
            self.log_relation_retraction(from_key, to_key, selector);
        }
    }

    pub fn log_title_mutation(&mut self, node_key: NodeKey) {
        if let Some(store) = &mut self.services.persistence {
            if let Some(node) = self.workspace.domain.graph.get_node(node_key) {
                let node_id = node.id.to_string();
                let title = node.title.clone();
                store.log_mutation(&LogEntry::UpdateNodeTitle {
                    node_id: node_id.clone(),
                    title: title.clone(),
                });
                store.log_audit_event(
                    &node_id,
                    crate::services::persistence::types::NodeAuditEventKind::TitleChanged {
                        new_title: title,
                    },
                    Self::unix_timestamp_ms_now(),
                );
            }
        }
    }

    pub(crate) fn log_node_audit_event(
        &mut self,
        node_key: NodeKey,
        event: crate::services::persistence::types::NodeAuditEventKind,
    ) {
        if let Some(store) = &mut self.services.persistence
            && let Some(node) = self.workspace.domain.graph.get_node(node_key)
        {
            store.log_audit_event(&node.id.to_string(), event, Self::unix_timestamp_ms_now());
        }
    }

    pub(crate) fn maybe_add_history_traversal_edge(
        &mut self,
        node_key: NodeKey,
        old_entries: &[String],
        old_index: usize,
        new_entries: &[String],
        new_index: usize,
    ) {
        let Some(old_url) = old_entries.get(old_index).filter(|url| !url.is_empty()) else {
            self.record_history_failure(
                HistoryTraversalFailureReason::MissingOldUrl,
                "old history entry missing or empty",
            );
            return;
        };
        let Some(new_url) = new_entries.get(new_index).filter(|url| !url.is_empty()) else {
            self.record_history_failure(
                HistoryTraversalFailureReason::MissingNewUrl,
                "new history entry missing or empty",
            );
            return;
        };
        if old_url == new_url {
            self.record_history_failure(
                HistoryTraversalFailureReason::SameUrl,
                "history transition resolves to same URL",
            );
            return;
        }

        let is_back = new_index < old_index;
        let is_forward_same_list = new_index > old_index && new_entries.len() == old_entries.len();
        if !is_back && !is_forward_same_list {
            self.record_history_failure(
                HistoryTraversalFailureReason::NonHistoryTransition,
                "transition is not a back/forward history move",
            );
            return;
        }
        let trigger = if is_back {
            NavigationTrigger::Back
        } else {
            NavigationTrigger::Forward
        };

        let from_key = self
            .workspace
            .domain
            .graph
            .get_nodes_by_url(old_url)
            .into_iter()
            .find(|&key| key != node_key)
            .or(Some(node_key));
        let to_key = self
            .workspace
            .domain
            .graph
            .get_nodes_by_url(new_url)
            .into_iter()
            .find(|&key| key != node_key)
            .or(Some(node_key));
        let (Some(from_key), Some(to_key)) = (from_key, to_key) else {
            self.record_history_failure(
                HistoryTraversalFailureReason::MissingEndpoint,
                "could not resolve traversal endpoints",
            );
            return;
        };

        let _ = self.push_history_traversal_and_sync(from_key, to_key, trigger);
    }

    pub(crate) fn push_history_traversal_and_sync(
        &mut self,
        from_key: NodeKey,
        to_key: NodeKey,
        trigger: NavigationTrigger,
    ) -> bool {
        if from_key == to_key {
            self.record_history_failure(
                HistoryTraversalFailureReason::SelfLoop,
                "from_key equals to_key",
            );
            return false;
        }
        let existing_edge_key = self.workspace.domain.graph.find_edge_key(from_key, to_key);
        let history_semantic_existed = existing_edge_key
            .and_then(|edge_key| self.workspace.domain.graph.get_edge(edge_key))
            .map(|payload| payload.has_edge_type(EdgeType::History))
            .unwrap_or(false);

        let traversal = Traversal::now(trigger);
        let GraphDeltaResult::TraversalAppended(appended) =
            self.apply_graph_delta_and_sync(GraphDelta::AppendTraversal {
                from: from_key,
                to: to_key,
                traversal,
            })
        else {
            unreachable!("append traversal delta must return TraversalAppended");
        };
        if !appended {
            self.record_history_failure(
                HistoryTraversalFailureReason::GraphRejected,
                "graph push_traversal rejected append",
            );
            return false;
        }

        self.workspace.graph_runtime.history_last_event_unix_ms =
            Some(Self::unix_timestamp_ms_now());

        emit_event(DiagnosticEvent::MessageReceived {
            channel_id: CHANNEL_HISTORY_TRAVERSAL_RECORDED,
            latency_us: 0,
        });

        if !history_semantic_existed {
            self.log_edge_mutation(from_key, to_key, EdgeType::History, None);
        }
        self.log_traversal_mutation(from_key, to_key, traversal);
        self.workspace.graph_runtime.physics.base.is_running = true;
        self.workspace.graph_runtime.drag_release_frames_remaining = 0;
        true
    }

    fn log_traversal_mutation(&mut self, from_key: NodeKey, to_key: NodeKey, traversal: Traversal) {
        if let Some(store) = &mut self.services.persistence {
            let from_id = self
                .workspace
                .domain
                .graph
                .get_node(from_key)
                .map(|n| n.id.to_string());
            let to_id = self
                .workspace
                .domain
                .graph
                .get_node(to_key)
                .map(|n| n.id.to_string());
            let (Some(from_node_id), Some(to_node_id)) = (from_id, to_id) else {
                return;
            };
            let trigger = match traversal.trigger {
                NavigationTrigger::Unknown => PersistedNavigationTrigger::Unknown,
                NavigationTrigger::LinkClick => PersistedNavigationTrigger::LinkClick,
                NavigationTrigger::Back => PersistedNavigationTrigger::Back,
                NavigationTrigger::Forward => PersistedNavigationTrigger::Forward,
                NavigationTrigger::AddressBarEntry => PersistedNavigationTrigger::AddressBarEntry,
                NavigationTrigger::PanePromotion => PersistedNavigationTrigger::PanePromotion,
                NavigationTrigger::Programmatic => PersistedNavigationTrigger::Programmatic,
            };
            store.log_mutation(&LogEntry::AppendTraversal {
                from_node_id,
                to_node_id,
                timestamp_ms: traversal.timestamp_ms,
                trigger,
            });
        }
    }

    pub(crate) fn unix_timestamp_ms_now() -> u64 {
        SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    pub(crate) fn record_history_failure(
        &mut self,
        reason: HistoryTraversalFailureReason,
        detail: impl Into<String>,
    ) {
        self.update_history_failure(reason, detail)
    }

    pub(crate) fn add_user_grouped_edge_if_missing(
        &mut self,
        from: NodeKey,
        to: NodeKey,
        label: Option<String>,
    ) {
        if from == to {
            return;
        }
        if self.workspace.domain.graph.get_node(from).is_none()
            || self.workspace.domain.graph.get_node(to).is_none()
        {
            return;
        }
        let already_grouped = self
            .workspace
            .domain
            .graph
            .find_edge_key(from, to)
            .and_then(|edge_key| self.workspace.domain.graph.get_edge(edge_key))
            .is_some_and(|payload| {
                payload.has_relation(crate::graph::RelationSelector::Semantic(
                    crate::graph::SemanticSubKind::UserGrouped,
                ))
            });
        if !already_grouped {
            let _ = self.assert_relation_and_sync(
                from,
                to,
                crate::graph::EdgeAssertion::Semantic {
                    sub_kind: crate::graph::SemanticSubKind::UserGrouped,
                    label,
                    decay_progress: None,
                },
            );
        }
    }

    pub(crate) fn delete_import_record(&mut self, record_id: String) {
        if self.workspace.domain.graph.delete_import_record(&record_id) {
            self.workspace.graph_runtime.egui_state_dirty = true;
        }
    }

    pub(crate) fn suppress_import_record_membership(&mut self, record_id: String, key: NodeKey) {
        if self
            .workspace
            .domain
            .graph
            .set_import_record_membership_suppressed(&record_id, key, true)
        {
            self.workspace.graph_runtime.egui_state_dirty = true;
        }
    }

    pub(crate) fn promote_import_record_to_user_group(
        &mut self,
        record_id: String,
        anchor: NodeKey,
    ) {
        let member_keys = self
            .workspace
            .domain
            .graph
            .import_record_member_keys(&record_id);
        if !member_keys.contains(&anchor) {
            return;
        }
        for member in member_keys {
            if member == anchor {
                continue;
            }
            self.add_user_grouped_edge_if_missing(anchor, member, None);
            self.add_user_grouped_edge_if_missing(member, anchor, None);
        }
    }

    pub(crate) fn add_arrangement_relation_if_missing(
        &mut self,
        from: NodeKey,
        to: NodeKey,
        sub_kind: crate::graph::ArrangementSubKind,
    ) {
        if from == to {
            return;
        }
        if self.workspace.domain.graph.get_node(from).is_none()
            || self.workspace.domain.graph.get_node(to).is_none()
        {
            return;
        }
        let selector = crate::graph::RelationSelector::Arrangement(sub_kind);
        let has_opposite_durability = self
            .workspace
            .domain
            .graph
            .find_edge_key(from, to)
            .and_then(|edge_key| self.workspace.domain.graph.get_edge(edge_key))
            .and_then(|payload| payload.arrangement_data())
            .is_some_and(|arrangement| {
                arrangement
                    .sub_kinds
                    .iter()
                    .copied()
                    .any(|existing| existing.durability() != sub_kind.durability())
            });
        let already_exists = self
            .workspace
            .domain
            .graph
            .find_edge_key(from, to)
            .and_then(|edge_key| self.workspace.domain.graph.get_edge(edge_key))
            .is_some_and(|payload| payload.has_relation(selector));
        if !already_exists {
            let _ = self.assert_relation_and_sync(
                from,
                to,
                crate::graph::EdgeAssertion::Arrangement { sub_kind },
            );
            if has_opposite_durability {
                self.emit_arrangement_durability_transition();
            }
        }
    }

    pub(crate) fn promote_arrangement_relation_to_frame_membership(
        &mut self,
        from: NodeKey,
        to: NodeKey,
    ) {
        if from == to {
            return;
        }
        if self.workspace.domain.graph.get_node(from).is_none()
            || self.workspace.domain.graph.get_node(to).is_none()
        {
            return;
        }
        let edge_payload = self
            .workspace
            .domain
            .graph
            .find_edge_key(from, to)
            .and_then(|edge_key| self.workspace.domain.graph.get_edge(edge_key));
        let had_tile_group = edge_payload.is_some_and(|payload| {
            payload.has_relation(crate::graph::RelationSelector::Arrangement(
                crate::graph::ArrangementSubKind::TileGroup,
            ))
        });
        let had_split_pair = edge_payload.is_some_and(|payload| {
            payload.has_relation(crate::graph::RelationSelector::Arrangement(
                crate::graph::ArrangementSubKind::SplitPair,
            ))
        });
        let had_frame_member = edge_payload.is_some_and(|payload| {
            payload.has_relation(crate::graph::RelationSelector::Arrangement(
                crate::graph::ArrangementSubKind::FrameMember,
            ))
        });

        self.add_arrangement_relation_if_missing(
            from,
            to,
            crate::graph::ArrangementSubKind::FrameMember,
        );

        if had_tile_group {
            let _ = self.retract_relations_and_log(
                from,
                to,
                crate::graph::RelationSelector::Arrangement(
                    crate::graph::ArrangementSubKind::TileGroup,
                ),
            );
        }
        if had_split_pair {
            let _ = self.retract_relations_and_log(
                from,
                to,
                crate::graph::RelationSelector::Arrangement(
                    crate::graph::ArrangementSubKind::SplitPair,
                ),
            );
        }

        if (had_tile_group || had_split_pair) && had_frame_member {
            self.emit_arrangement_durability_transition();
        }
    }

    pub(crate) fn create_user_grouped_edge_from_primary_selection(&mut self) {
        let selection = self.focused_selection();
        let Some(from) = selection.primary() else {
            return;
        };
        let to = selection.iter().copied().find(|key| *key != from);
        if let Some(to) = to {
            self.add_user_grouped_edge_if_missing(from, to, None);
        }
    }

    pub(crate) fn group_nodes_by_semantic_tags(&mut self) {
        use std::collections::{HashMap, HashSet};

        let mut clusters: HashMap<u8, HashSet<NodeKey>> = HashMap::new();

        for (&node_key, vector) in &self.workspace.graph_runtime.semantic_index {
            for code in &vector.classes {
                if let Some(&first_digit) = code.0.first() {
                    clusters.entry(first_digit).or_default().insert(node_key);
                }
            }
        }

        let mut created_pairs = std::collections::HashSet::new();

        for (_subject_code, nodes) in clusters {
            let nodes: Vec<NodeKey> = nodes.into_iter().collect();
            if nodes.len() < 2 {
                continue;
            }

            for i in 0..nodes.len() {
                for j in (i + 1)..nodes.len() {
                    let (a, b) = (nodes[i], nodes[j]);
                    let pair = if a < b { (a, b) } else { (b, a) };
                    if !created_pairs.contains(&pair) {
                        created_pairs.insert(pair);
                        self.add_user_grouped_edge_if_missing(a, b, None);
                        self.add_user_grouped_edge_if_missing(b, a, None);
                    }
                }
            }
        }
    }

    pub(crate) fn selected_pair_in_order(&self) -> Option<(NodeKey, NodeKey)> {
        self.focused_selection().ordered_pair()
    }

    pub(crate) fn intents_for_edge_command(&self, command: EdgeCommand) -> Vec<GraphIntent> {
        match command {
            EdgeCommand::ConnectSelectedPair => self
                .selected_pair_in_order()
                .map(|(from, to)| {
                    vec![GraphIntent::CreateUserGroupedEdge {
                        from,
                        to,
                        label: None,
                    }]
                })
                .unwrap_or_default(),
            EdgeCommand::ConnectPair { from, to } => {
                vec![GraphIntent::CreateUserGroupedEdge {
                    from,
                    to,
                    label: None,
                }]
            }
            EdgeCommand::ConnectBothDirections => self
                .selected_pair_in_order()
                .map(|(from, to)| {
                    vec![
                        GraphIntent::CreateUserGroupedEdge {
                            from,
                            to,
                            label: None,
                        },
                        GraphIntent::CreateUserGroupedEdge {
                            from: to,
                            to: from,
                            label: None,
                        },
                    ]
                })
                .unwrap_or_default(),
            EdgeCommand::ConnectBothDirectionsPair { a, b } => {
                vec![
                    GraphIntent::CreateUserGroupedEdge {
                        from: a,
                        to: b,
                        label: None,
                    },
                    GraphIntent::CreateUserGroupedEdge {
                        from: b,
                        to: a,
                        label: None,
                    },
                ]
            }
            EdgeCommand::RemoveUserEdge => self
                .selected_pair_in_order()
                .map(|(from, to)| {
                    vec![
                        GraphIntent::RemoveEdge {
                            from,
                            to,
                            selector: crate::graph::RelationSelector::Semantic(
                                crate::graph::SemanticSubKind::UserGrouped,
                            ),
                        },
                        GraphIntent::RemoveEdge {
                            from: to,
                            to: from,
                            selector: crate::graph::RelationSelector::Semantic(
                                crate::graph::SemanticSubKind::UserGrouped,
                            ),
                        },
                    ]
                })
                .unwrap_or_default(),
            EdgeCommand::RemoveUserEdgePair { a, b } => {
                vec![
                    GraphIntent::RemoveEdge {
                        from: a,
                        to: b,
                        selector: crate::graph::RelationSelector::Semantic(
                            crate::graph::SemanticSubKind::UserGrouped,
                        ),
                    },
                    GraphIntent::RemoveEdge {
                        from: b,
                        to: a,
                        selector: crate::graph::RelationSelector::Semantic(
                            crate::graph::SemanticSubKind::UserGrouped,
                        ),
                    },
                ]
            }
            EdgeCommand::PinSelected => self
                .focused_selection()
                .iter()
                .copied()
                .map(|key| GraphIntent::SetNodePinned {
                    key,
                    is_pinned: true,
                })
                .collect(),
            EdgeCommand::UnpinSelected => self
                .focused_selection()
                .iter()
                .copied()
                .map(|key| GraphIntent::SetNodePinned {
                    key,
                    is_pinned: false,
                })
                .collect(),
        }
    }

    pub(crate) fn set_node_pinned_and_log(&mut self, key: NodeKey, is_pinned: bool) {
        let Some(current_state) = self
            .workspace
            .domain
            .graph
            .get_node(key)
            .map(|node| node.is_pinned)
        else {
            return;
        };
        let had_pin_tag = self
            .workspace
            .domain
            .graph
            .node_tags(key)
            .is_some_and(|tags| tags.contains(Self::TAG_PIN));
        if current_state == is_pinned && had_pin_tag == is_pinned {
            return;
        }

        let _ = self.apply_graph_delta_and_sync(GraphDelta::SetNodePinned { key, is_pinned });

        let tags_changed = if is_pinned {
            self.workspace
                .domain
                .graph
                .insert_node_tag(key, Self::TAG_PIN.to_string())
        } else {
            self.workspace
                .domain
                .graph
                .remove_node_tag(key, Self::TAG_PIN)
        };

        if tags_changed {
            self.workspace.graph_runtime.semantic_index_dirty = true;
        }

        if let Some(store) = &mut self.services.persistence {
            let node_id = self
                .workspace
                .domain
                .graph
                .get_node(key)
                .map(|node| node.id.to_string())
                .unwrap_or_default();
            store.log_mutation(&LogEntry::PinNode {
                node_id: node_id.clone(),
                is_pinned,
            });
            let audit_event = if is_pinned {
                crate::services::persistence::types::NodeAuditEventKind::Pinned
            } else {
                crate::services::persistence::types::NodeAuditEventKind::Unpinned
            };
            store.log_audit_event(&node_id, audit_event, Self::unix_timestamp_ms_now());
        }
    }

    pub fn create_new_node_near_center(&mut self) -> NodeKey {
        let position = self.suggested_new_node_position(None);
        let placeholder_url = self.next_placeholder_url();

        let key = self.add_node_and_sync(placeholder_url, position);
        self.select_node(key, false);
        key
    }

    pub fn remove_selected_nodes(&mut self) {
        let nodes_to_remove: Vec<NodeKey> = self.focused_selection().iter().copied().collect();

        for node_key in nodes_to_remove {
            let node_id = self
                .workspace
                .domain
                .graph
                .get_node(node_key)
                .map(|node| node.id);

            if let Some(store) = &mut self.services.persistence {
                if let Some(node_id) = node_id {
                    store.log_mutation(&LogEntry::RemoveNode {
                        node_id: node_id.to_string(),
                        timestamp_ms: Self::unix_timestamp_ms_now(),
                    });
                }
            }

            if let Some(webview_id) = self
                .workspace
                .graph_runtime
                .node_to_webview
                .get(&node_key)
                .copied()
            {
                let _ = self.unmap_webview(webview_id);
            }
            self.remove_active_node(node_key);
            self.remove_warm_cache_node(node_key);
            self.workspace
                .graph_runtime
                .runtime_block_state
                .remove(&node_key);
            self.workspace
                .graph_runtime
                .runtime_block_state
                .remove(&node_key);
            self.workspace
                .graph_runtime
                .suggested_semantic_tags
                .remove(&node_key);
            if let Some(node_id) = node_id {
                self.workspace.workbench_session.on_node_deleted(node_id);
            }

            if let Some(store) = &mut self.services.persistence {
                let dissolved_before = store.dissolved_archive_len();
                let _ = store.dissolve_and_remove_node(&mut self.workspace.domain.graph, node_key);
                let dissolved_after = store.dissolved_archive_len();
                if dissolved_after > dissolved_before {
                    self.workspace.graph_runtime.history_last_event_unix_ms =
                        Some(Self::unix_timestamp_ms_now());
                    emit_event(DiagnosticEvent::MessageReceived {
                        channel_id: CHANNEL_HISTORY_ARCHIVE_DISSOLVED_APPENDED,
                        latency_us: 0,
                    });
                }
            } else {
                let _ = self.apply_graph_delta_and_sync(GraphDelta::RemoveNode { key: node_key });
            }
        }

        self.clear_selection();
        self.prune_selection_to_existing_nodes();
        self.workspace.graph_runtime.highlighted_graph_edge = None;
        let pending_node_context_target = self
            .pending_node_context_target()
            .filter(|key| self.workspace.domain.graph.get_node(*key).is_some());
        self.set_pending_node_context_target(pending_node_context_target);
        self.sanitize_pending_frame_import_commands();
    }

    /// Soft-delete selected nodes: transitions them to `NodeLifecycle::Tombstone`
    /// (Ghost Node) without removing them from the graph.  Webview resources are
    /// freed; the node remains structurally present for topology preservation.
    pub fn mark_tombstone_for_selected(&mut self) {
        use crate::graph::NodeLifecycle;
        let nodes: Vec<NodeKey> = self.focused_selection().iter().copied().collect();
        for key in nodes {
            let already_tombstone = self
                .workspace
                .domain
                .graph
                .get_node(key)
                .is_some_and(|n| n.lifecycle == NodeLifecycle::Tombstone);
            if already_tombstone {
                continue;
            }
            // Free webview resources like a cold demotion.
            if let Some(webview_id) = self
                .workspace
                .graph_runtime
                .node_to_webview
                .get(&key)
                .copied()
            {
                let _ = self.unmap_webview(webview_id);
            }
            self.remove_active_node(key);
            self.remove_warm_cache_node(key);
            self.workspace
                .domain
                .graph
                .set_node_lifecycle(key, NodeLifecycle::Tombstone);
        }
        self.clear_selection();
        self.workspace.graph_runtime.egui_state_dirty = true;
    }

    /// Restore a single Ghost Node from `NodeLifecycle::Tombstone → Cold`.
    /// The node retains its preserved position and edges.
    pub fn restore_ghost_node(&mut self, key: NodeKey) {
        use crate::graph::NodeLifecycle;
        let is_tombstone = self
            .workspace
            .domain
            .graph
            .get_node(key)
            .is_some_and(|n| n.lifecycle == NodeLifecycle::Tombstone);
        if !is_tombstone {
            return;
        }
        self.workspace
            .domain
            .graph
            .set_node_lifecycle(key, NodeLifecycle::Cold);
        self.workspace.graph_runtime.egui_state_dirty = true;
    }

    pub fn get_single_selected_node(&self) -> Option<NodeKey> {
        let selected = self.focused_selection();
        if selected.len() == 1 {
            selected.primary()
        } else {
            None
        }
    }

    pub(crate) fn suggested_semantic_tags_for_node(&self, key: NodeKey) -> Vec<String> {
        self.workspace
            .graph_runtime
            .suggested_semantic_tags
            .get(&key)
            .cloned()
            .unwrap_or_default()
    }

    pub fn clear_graph(&mut self) {
        if let Some(store) = &mut self.services.persistence {
            store.log_mutation(&LogEntry::ClearGraph);
        }
        self.workspace.domain.graph = Graph::new();
        self.reset_selection_state();
        self.workspace.graph_runtime.highlighted_graph_edge = None;
        self.workspace.graph_runtime.navigator_projection_state =
            NavigatorProjectionState::default();
        self.clear_choose_frame_picker();
        self.workspace
            .workbench_session
            .pending_app_commands
            .clear();
        self.clear_pending_camera_command();
        self.clear_pending_wheel_zoom_delta();
        self.workspace.domain.notes.clear();
        self.workspace.graph_runtime.views.clear();
        self.workspace.graph_runtime.graph_view_frames.clear();
        self.workspace.graph_runtime.graph_view_canvas_rects.clear();
        self.workspace.graph_runtime.workbench_navigation_geometry = None;
        self.set_workspace_focused_view_with_transition(None);
        self.workspace.graph_runtime.webview_to_node.clear();
        self.workspace.graph_runtime.node_to_webview.clear();
        self.workspace.graph_runtime.active_lru.clear();
        self.workspace.graph_runtime.warm_cache_lru.clear();
        self.workspace.graph_runtime.runtime_block_state.clear();
        self.workspace.graph_runtime.runtime_block_state.clear();
        self.workspace.graph_runtime.suggested_semantic_tags.clear();
        self.workspace.graph_runtime.semantic_index.clear();
        self.workspace.graph_runtime.semantic_index_dirty = true;
        self.workspace
            .workbench_session
            .node_last_active_workspace
            .clear();
        self.workspace
            .workbench_session
            .node_workspace_membership
            .clear();
        self.workspace
            .workbench_session
            .last_session_workspace_layout_hash = None;
        self.workspace
            .workbench_session
            .last_session_workspace_layout_json = None;
        self.workspace.workbench_session.last_workspace_autosave_at = None;
        self.workspace
            .workbench_session
            .current_workspace_is_synthesized = false;
        self.workspace
            .workbench_session
            .workspace_has_unsaved_changes = false;
        self.workspace
            .workbench_session
            .unsaved_workspace_prompt_warned = false;
        self.workspace.graph_runtime.egui_state_dirty = true;
    }

    pub fn clear_graph_and_persistence(&mut self) {
        if let Some(store) = &mut self.services.persistence {
            if let Err(e) = store.clear_all() {
                warn!("Failed to clear persisted graph data: {e}");
            }
        }
        self.workspace.domain.graph = Graph::new();
        self.reset_selection_state();
        self.workspace.graph_runtime.highlighted_graph_edge = None;
        self.workspace.graph_runtime.navigator_projection_state =
            NavigatorProjectionState::default();
        self.clear_choose_frame_picker();
        self.workspace
            .workbench_session
            .pending_app_commands
            .clear();
        self.clear_pending_camera_command();
        self.clear_pending_wheel_zoom_delta();
        self.workspace.graph_runtime.views.clear();
        self.workspace.graph_runtime.graph_view_frames.clear();
        self.workspace.graph_runtime.graph_view_canvas_rects.clear();
        self.workspace.graph_runtime.workbench_navigation_geometry = None;
        self.set_workspace_focused_view_with_transition(None);
        self.workspace.graph_runtime.webview_to_node.clear();
        self.workspace.graph_runtime.node_to_webview.clear();
        self.workspace.graph_runtime.active_lru.clear();
        self.workspace.graph_runtime.warm_cache_lru.clear();
        self.workspace.graph_runtime.runtime_block_state.clear();
        self.workspace.graph_runtime.runtime_block_state.clear();
        self.workspace.graph_runtime.suggested_semantic_tags.clear();
        self.workspace
            .workbench_session
            .node_last_active_workspace
            .clear();
        self.workspace
            .workbench_session
            .node_workspace_membership
            .clear();
        self.workspace
            .workbench_session
            .current_workspace_is_synthesized = false;
        self.workspace
            .workbench_session
            .workspace_has_unsaved_changes = false;
        self.workspace
            .workbench_session
            .unsaved_workspace_prompt_warned = false;
        self.workspace.graph_runtime.active_webview_nodes.clear();
        self.workspace.domain.next_placeholder_id = 0;
        self.workspace.graph_runtime.egui_state_dirty = true;
        self.workspace.graph_runtime.semantic_index.clear();
        self.workspace.graph_runtime.semantic_index_dirty = true;
    }

    pub fn update_node_url_and_log(&mut self, key: NodeKey, new_url: String) -> Option<String> {
        let new_mime_hint = crate::graph::detect_mime(&new_url, None);

        let GraphDeltaResult::NodeUrlUpdated(old_url) =
            self.apply_graph_delta_and_sync(GraphDelta::SetNodeUrl {
                key,
                new_url: new_url.clone(),
            })
        else {
            unreachable!("url delta must return NodeUrlUpdated");
        };
        let old_url = old_url?;

        let _ = self.apply_graph_delta_and_sync(GraphDelta::SetNodeMimeHint {
            key,
            mime_hint: new_mime_hint.clone(),
        });

        if let Some(store) = &mut self.services.persistence {
            if let Some(node) = self.workspace.domain.graph.get_node(key) {
                let node_id = node.id.to_string();
                let ts = Self::unix_timestamp_ms_now();
                store.log_mutation(&LogEntry::NavigateNode {
                    node_id: node_id.clone(),
                    from_url: old_url.clone(),
                    to_url: new_url.clone(),
                    trigger: PersistedNavigationTrigger::Unknown,
                    timestamp_ms: ts,
                });
                store.log_mutation(&LogEntry::UpdateNodeUrl {
                    node_id: node_id.clone(),
                    new_url: new_url.clone(),
                });
                store.log_audit_event(
                    &node_id,
                    crate::services::persistence::types::NodeAuditEventKind::UrlChanged {
                        new_url: new_url.clone(),
                    },
                    ts,
                );
                store.log_mutation(&LogEntry::UpdateNodeMimeHint {
                    node_id: node_id.clone(),
                    mime_hint: new_mime_hint,
                });
            }
        }
        self.workspace.graph_runtime.egui_state_dirty = true;
        self.refresh_protocol_probe_for_node(key, &new_url, true);
        Some(old_url)
    }

    pub fn create_note_for_node(&mut self, key: NodeKey, title: Option<String>) -> Option<NoteId> {
        let node = self.workspace.domain.graph.get_node(key)?;
        let now = SystemTime::now();
        let note_id = NoteId::new();
        let resolved_title = title.unwrap_or_else(|| {
            let base = node.title.trim();
            if base.is_empty() {
                format!("Note for {}", node.url())
            } else {
                format!("Note for {base}")
            }
        });
        let note = NoteRecord {
            id: note_id,
            title: resolved_title,
            linked_node: Some(key),
            source_url: Some(node.url().to_string()),
            body: String::new(),
            created_at: now,
            updated_at: now,
        };

        self.workspace.domain.notes.insert(note_id, note);
        self.enqueue_app_command(AppCommand::OpenNote { note_id });
        self.request_open_node_tile_mode(key, PendingTileOpenMode::SplitHorizontal);
        Some(note_id)
    }

    pub fn note_record(&self, note_id: NoteId) -> Option<&NoteRecord> {
        self.workspace.domain.notes.get(&note_id)
    }

    fn ensure_webfinger_import_node(
        &mut self,
        url: String,
        title: String,
        position: euclid::default::Point2D<f32>,
        tags: &[&str],
    ) -> NodeKey {
        let key = if let Some((key, _)) = self.domain_graph().get_node_by_url(&url) {
            key
        } else {
            self.add_node_and_sync(url, position)
        };

        self.set_node_title_if_empty_or_url_and_log(key, title);
        self.apply_node_tags(key, tags);
        key
    }

    fn ensure_browser_import_structure_node(
        &mut self,
        url: String,
        title: String,
        anchor: Option<NodeKey>,
    ) -> NodeKey {
        let position = self.suggested_new_node_position(anchor);
        let key = if let Some((key, _)) = self.domain_graph().get_node_by_url(&url) {
            key
        } else {
            self.add_node_and_sync(url, position)
        };
        self.set_node_title_if_empty_or_url_and_log(key, title);
        key
    }

    fn ensure_browser_import_page_node(
        &mut self,
        page: &crate::services::import::ImportedPageSeed,
        anchor: Option<NodeKey>,
    ) -> NodeKey {
        let position = self.suggested_new_node_position(anchor);
        let key = if let Some((key, _)) = self.domain_graph().get_node_by_url(&page.canonical_url) {
            key
        } else {
            self.add_node_and_sync(page.canonical_url.clone(), position)
        };
        if let Some(title) = page
            .normalized_title
            .as_ref()
            .or(page.raw_title.as_ref())
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        {
            self.set_node_title_if_empty_or_url_and_log(key, title.to_string());
        }
        key
    }

    fn collect_browser_import_node_id(
        &self,
        key: NodeKey,
        ids: &mut std::collections::BTreeSet<String>,
    ) {
        if let Some(node) = self.workspace.domain.graph.get_node(key) {
            ids.insert(node.id.to_string());
        }
    }

    fn find_person_node_for_identity_profile(
        &self,
        profile: &graphshell_comms::identity::PersonIdentityProfile,
    ) -> Option<NodeKey> {
        let identity_candidates = person_identity_classification_candidates(profile);
        self.domain_graph().nodes().find_map(|(key, _)| {
            if !self.node_has_canonical_tag(key, TAG_PERSON) {
                return None;
            }
            let classifications = self.domain_graph().node_classifications(key)?;
            identity_candidates.iter().any(|(scheme, value)| {
                classifications.iter().any(|classification| {
                    classification.scheme == *scheme && classification.value == *value
                })
            }).then_some(key)
        })
    }

    fn person_identity_values(&self, key: NodeKey, kind: &str) -> Vec<String> {
        let scheme = person_identity_scheme(kind);
        self.domain_graph()
            .node_classifications(key)
            .map(|classifications| {
                classifications
                    .iter()
                    .filter(|classification| classification.scheme == scheme)
                    .map(|classification| classification.value.clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    fn resolve_and_import_person_identity(
        &mut self,
        protocol: graphshell_comms::capabilities::MiddlenetProtocol,
        resource: &str,
        anchor: Option<NodeKey>,
    ) -> Result<NodeKey, String> {
        let resolved = graphshell_comms::identity::resolve_person_identity_profile(protocol, resource)?;
        let person_key = self.import_person_identity_into_graph(&resolved.profile, anchor)?;
        self.record_identity_resolution_provenance(
            person_key,
            &resolved,
            graphshell_comms::identity::IdentityResolutionActionKind::Resolve,
            None,
        );
        Ok(person_key)
    }

    fn record_identity_resolution_provenance(
        &mut self,
        person_key: NodeKey,
        resolved: &graphshell_comms::identity::ResolvedPersonIdentityProfile,
        action_kind: graphshell_comms::identity::IdentityResolutionActionKind,
        changed: Option<bool>,
    ) {
        let descriptor = graphshell_comms::capabilities::descriptor(resolved.provenance.protocol);
        self.ensure_identity_classification(
            person_key,
            person_resolution_scheme(resolved.provenance.protocol),
            resolved.provenance.query_resource.clone(),
            Some(format!("Resolved via {}", descriptor.display_name)),
        );
        self.log_node_audit_event(
            person_key,
            crate::services::persistence::types::NodeAuditEventKind::ActionRecorded {
                action: action_kind.action_label().to_string(),
                detail: graphshell_comms::identity::format_identity_resolution_audit_detail(
                    &graphshell_comms::identity::IdentityResolutionAuditRecord {
                        action_kind,
                        protocol: resolved.provenance.protocol,
                        query_resource: resolved.provenance.query_resource.clone(),
                        cache_state: resolved.provenance.cache_state,
                        freshness: resolved.provenance.freshness,
                        resolved_at_ms: resolved.provenance.resolved_at_ms,
                        source_endpoints: resolved.provenance.source_endpoints.clone(),
                        changed,
                    },
                ),
            },
        );
    }

    fn person_resolution_queries(
        &self,
        key: NodeKey,
    ) -> Vec<(graphshell_comms::capabilities::MiddlenetProtocol, String)> {
        self.domain_graph()
            .node_classifications(key)
            .map(|classifications| {
                classifications
                    .iter()
                    .filter_map(|classification| {
                        parse_person_resolution_scheme(&classification.scheme)
                            .map(|protocol| (protocol, classification.value.clone()))
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    fn person_identity_refresh_fingerprint(
        &self,
        key: NodeKey,
    ) -> std::collections::BTreeSet<String> {
        let mut fingerprint = std::collections::BTreeSet::new();
        if let Some(classifications) = self.domain_graph().node_classifications(key) {
            for classification in classifications {
                fingerprint.insert(format!("c:{:?}:{}", classification.scheme, classification.value));
            }
        }
        for neighbor in self.domain_graph().out_neighbors(key) {
            if let Some(node) = self.domain_graph().get_node(neighbor) {
                fingerprint.insert(format!("out:{}", node.url()));
            }
        }
        for neighbor in self.domain_graph().in_neighbors(key) {
            if let Some(node) = self.domain_graph().get_node(neighbor) {
                fingerprint.insert(format!("in:{}", node.url()));
            }
        }
        fingerprint
    }

    fn person_identity_value_for_capability(
        &self,
        key: NodeKey,
        capability: graphshell_comms::capabilities::ProtocolCapability,
    ) -> Option<(graphshell_comms::capabilities::MiddlenetProtocol, String)> {
        graphshell_comms::capabilities::protocols_with_capability(capability).find_map(
            |protocol| {
                let kind = graphshell_comms::capabilities::descriptor(protocol)
                    .identity_classification_kind?;
                self.person_identity_values(key, kind)
                    .into_iter()
                    .next()
                    .map(|value| (protocol, value))
            },
        )
    }

    fn ensure_person_identity_classifications(
        &mut self,
        key: NodeKey,
        profile: &graphshell_comms::identity::PersonIdentityProfile,
    ) {
        if let Some(handle) = &profile.human_handle {
            self.ensure_identity_classification(
                key,
                person_identity_scheme("handle"),
                handle.clone(),
                Some("Human handle".to_string()),
            );
        }
        if let Some(resource) = &profile.webfinger_resource {
            self.ensure_identity_classification(
                key,
                person_identity_scheme("webfinger"),
                resource.clone(),
                Some("WebFinger resource".to_string()),
            );
        }
        if let Some(nip05_identifier) = &profile.nip05_identifier {
            self.ensure_identity_classification(
                key,
                person_identity_scheme("nip05"),
                nip05_identifier.clone(),
                Some("NIP-05 identifier".to_string()),
            );
        }
        for mxid in &profile.matrix_mxids {
            self.ensure_identity_classification(
                key,
                person_identity_scheme("matrix"),
                mxid.clone(),
                Some("Matrix MXID".to_string()),
            );
        }
        for identity in &profile.nostr_identities {
            self.ensure_identity_classification(
                key,
                person_identity_scheme("nostr"),
                identity.clone(),
                Some("Nostr identity".to_string()),
            );
        }
        for mailbox in &profile.misfin_mailboxes {
            self.ensure_identity_classification(
                key,
                person_identity_scheme("misfin"),
                mailbox.clone(),
                Some("Misfin mailbox".to_string()),
            );
        }
        for capsule in &profile.gemini_capsules {
            self.ensure_identity_classification(
                key,
                person_identity_scheme("gemini"),
                capsule.clone(),
                Some("Gemini capsule".to_string()),
            );
        }
        for actor in &profile.activitypub_actors {
            self.ensure_identity_classification(
                key,
                person_identity_scheme("activitypub"),
                actor.clone(),
                Some("ActivityPub actor".to_string()),
            );
        }
    }

    fn ensure_identity_classification(
        &mut self,
        key: NodeKey,
        scheme: crate::model::graph::ClassificationScheme,
        value: String,
        label: Option<String>,
    ) {
        let should_add = self
            .workspace
            .domain
            .graph
            .node_classifications(key)
            .is_none_or(|classifications| {
                !classifications.iter().any(|classification| {
                    classification.scheme == scheme && classification.value == value
                })
            });
        if should_add {
            let primary = self
                .workspace
                .domain
                .graph
                .node_classifications(key)
                .is_none_or(|classifications| {
                    !classifications.iter().any(|classification| classification.scheme == scheme)
                });
            self.apply_reducer_intents([GraphIntent::AssignClassification {
                key,
                classification: crate::model::graph::NodeClassification {
                    scheme: scheme.clone(),
                    value: value.clone(),
                    label,
                    confidence: 1.0,
                    provenance: crate::model::graph::ClassificationProvenance::Imported,
                    status: crate::model::graph::ClassificationStatus::Imported,
                    primary,
                },
            }]);
        }
    }

    fn set_node_title_if_empty_or_url_and_log(&mut self, key: NodeKey, title: String) {
        let should_update = self
            .workspace
            .domain
            .graph
            .get_node(key)
            .is_some_and(|node| {
                let current_title = node.title.trim();
                current_title.is_empty() || current_title == node.url()
            });
        if !should_update {
            return;
        }

        let GraphDeltaResult::NodeMetadataUpdated(changed) =
            self.apply_graph_delta_and_sync(GraphDelta::SetNodeTitle { key, title })
        else {
            unreachable!("title delta must return NodeMetadataUpdated");
        };
        if changed {
            self.log_title_mutation(key);
        }
    }

    fn apply_node_tags(&mut self, key: NodeKey, tags: &[&str]) {
        for tag in tags {
            self.apply_reducer_intents([GraphIntent::TagNode {
                key,
                tag: (*tag).to_string(),
            }]);
        }
    }

    pub(crate) fn apply_graph_delta_and_sync(&mut self, delta: GraphDelta) -> GraphDeltaResult {
        let result = apply_domain_graph_delta(&mut self.workspace.domain.graph, delta.clone());
        if Self::graph_structure_changed(&result) {
            self.clear_hop_distance_cache();
        }
        // Rebuild derived containment edges whenever the node set or a node's URL changes,
        // so ContainmentRelation edges stay consistent without requiring an explicit refresh.
        if Self::containment_affected(&result) {
            self.workspace
                .domain
                .graph
                .rebuild_derived_containment_relations();
        }
        if let Some(egui_state) = self.workspace.graph_runtime.egui_state.as_mut()
            && !egui_state.sync_from_delta(&self.workspace.domain.graph, &delta, &result)
        {
            self.workspace.graph_runtime.egui_state_dirty = true;
        }
        result
    }

    pub(crate) fn containment_affected(result: &GraphDeltaResult) -> bool {
        matches!(
            result,
            GraphDeltaResult::NodeAdded(_)
                | GraphDeltaResult::NodeMaybeAdded(Some(_))
                | GraphDeltaResult::NodeRemoved(true)
                | GraphDeltaResult::NodeUrlUpdated(Some(_))
        )
    }

    pub(crate) fn graph_structure_changed(result: &GraphDeltaResult) -> bool {
        match result {
            GraphDeltaResult::NodeAdded(_) => true,
            GraphDeltaResult::NodeMaybeAdded(maybe) => maybe.is_some(),
            GraphDeltaResult::EdgeAdded(maybe) => maybe.is_some(),
            GraphDeltaResult::NodeRemoved(changed) => *changed,
            GraphDeltaResult::EdgesRemoved(count) => *count > 0,
            GraphDeltaResult::TraversalAppended(_) => false,
            GraphDeltaResult::NodeMetadataUpdated(_) => false,
            GraphDeltaResult::NodeUrlUpdated(_) => false,
        }
    }
}

