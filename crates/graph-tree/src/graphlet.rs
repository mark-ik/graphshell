// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use crate::MemberId;
use serde::{Deserialize, Serialize};

/// Graphlet identity. Lightweight integer index within a single GraphTree.
pub type GraphletId = u32;

/// A graphlet is a connected sub-structure within the GraphTree.
/// Multiple graphlets exist in a graph view — like document groups
/// in a folder. Each tracks its own binding and anchor state.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(bound = "")]
pub struct GraphletRef<N: MemberId> {
    pub id: GraphletId,
    pub anchors: Vec<N>,
    pub primary_anchor: Option<N>,
    pub binding: GraphletBinding,
    pub kind: Option<GraphletKind>,
}

impl<N: MemberId> GraphletRef<N> {
    pub fn new_session(id: GraphletId) -> Self {
        Self {
            id,
            anchors: Vec::new(),
            primary_anchor: None,
            binding: GraphletBinding::UnlinkedSession,
            kind: Some(GraphletKind::Session),
        }
    }

    pub fn with_anchor(mut self, anchor: N) -> Self {
        self.primary_anchor = Some(anchor.clone());
        self.anchors.push(anchor);
        self
    }

    pub fn with_kind(mut self, kind: GraphletKind) -> Self {
        self.kind = Some(kind);
        self
    }
}

/// How a tile group binds to a graphlet definition.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum GraphletBinding {
    /// No link to a canonical graphlet definition. Pure session grouping.
    UnlinkedSession,
    /// Linked to a canonical graphlet spec. Roster updates from graph.
    Linked { spec: GraphletSpec },
    /// Was linked, but user override created a divergence.
    Forked {
        parent_spec: GraphletSpec,
        reason: String,
    },
}

/// Canonical graphlet specification (referenced by Linked bindings).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GraphletSpec {
    pub kind: GraphletKind,
    pub anchors: Vec<String>,
    pub primary_anchor: Option<String>,
    pub selectors: Vec<String>,
}

/// The 9 canonical graphlet shapes from `graphlet_model.md`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GraphletKind {
    Ego { radius: u8 },
    Corridor,
    Component,
    Loop,
    Frontier,
    Facet,
    Session,
    Bridge,
    WorkbenchCorrespondence,
}
