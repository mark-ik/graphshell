//! Local, endpoint-scoped projection state.

use std::collections::BTreeMap;

use graphshell_protocol::{ProjectionSession, ProjectionSnapshot, SessionStatus};
use sceno::Scene;

/// The local cache entry for one disclosed remote projection.
#[derive(Clone, Debug, PartialEq)]
pub struct MountedScene {
    pub revision: u64,
    pub status: SessionStatus,
    pub scene: Scene,
}

/// Curation state. It receives scenes but never obtains source truth.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClientState {
    mounted: BTreeMap<ProjectionSession, MountedScene>,
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
            },
        );
    }

    /// Mark a mount stale without discarding permitted cached pixels/data.
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
    use graphshell_protocol::ProtocolVersion;

    #[test]
    fn snapshot_is_scoped_to_its_endpoint_session() {
        let session = ProjectionSession("loopback:one".into());
        let snapshot = ProjectionSnapshot {
            version: ProtocolVersion::V1,
            session: session.clone(),
            revision: 4,
            scene: Scene::new(),
        };
        let mut client = ClientState::default();
        client.apply_snapshot(snapshot);
        client.mark_stale(&session);
        assert_eq!(
            client.mounted(&session).unwrap().status,
            SessionStatus::Stale
        );
    }
}
