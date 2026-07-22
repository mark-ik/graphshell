//! Carrier-neutral Graphshell wire vocabulary.
//!
//! A message carries Scenograph's product-free score and scene types. Transport,
//! authorization, and application models are deliberately outside this crate.

use sceno::{InstanceId, Scene, Score};
use serde::{Deserialize, Serialize};

/// The first compatible Graphshell wire version.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolVersion {
    pub major: u16,
    pub minor: u16,
}

impl ProtocolVersion {
    pub const V1: Self = Self { major: 1, minor: 0 };
}

/// An endpoint-scoped projection session. It is opaque to Graphshell clients.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ProjectionSession(pub String);

/// A requested score plus the client's observed protocol version.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectionRequest {
    pub version: ProtocolVersion,
    pub session: ProjectionSession,
    pub score: Score,
}

/// A complete scene snapshot. Diffs wait for Scenotime's epoch/revision proof.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectionSnapshot {
    pub version: ProtocolVersion,
    pub session: ProjectionSession,
    pub revision: u64,
    pub scene: Scene,
}

/// A semantic intent invocation. `payload` is deliberately untyped bytes at
/// G0: each advertised intent will introduce a versioned schema in G1.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntentInvocation {
    pub session: ProjectionSession,
    pub target: InstanceId,
    pub observed_revision: u64,
    pub intent: String,
    pub payload: Vec<u8>,
}

/// The result of endpoint-side intent validation and dispatch.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum IntentResult {
    Accepted,
    Rejected { reason: String },
    Stale { current_revision: u64 },
}

/// The session status a client may render without inferring authority.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionStatus {
    Live,
    Stale,
    Disconnected,
    Expired,
    Revoked,
}

#[cfg(test)]
mod tests {
    use super::*;
    use sceno::{Arrangement, Spiral};

    #[test]
    fn request_serializes_a_product_free_score() {
        let request = ProjectionRequest {
            version: ProtocolVersion::V1,
            session: ProjectionSession("local:fixture".into()),
            score: Score::new(Arrangement::Spiral(Spiral::default())),
        };
        let json = serde_json::to_string(&request).unwrap();
        assert_eq!(
            serde_json::from_str::<ProjectionRequest>(&json).unwrap(),
            request
        );
        assert!(std::any::type_name::<Score>().starts_with("sceno::"));
    }
}
