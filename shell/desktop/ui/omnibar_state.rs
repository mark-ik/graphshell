/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Omnibar session state.
//!
//! Per the M4 runtime extraction, omnibar state lives on `GraphshellRuntime`
//! and is host-neutral; the toolbar widget consumes it via a mutation handle
//! and a view-model projection. Previously these types lived inside the egui
//! widget module (`toolbar/toolbar_ui.rs`), producing a layering inversion
//! where the runtime imported its session type from the host.

use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::time::Instant;

use crate::graph::NodeKey;
use crate::shell::desktop::runtime::control_panel::HostRequestMailbox;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OmnibarSessionKind {
    Graph(OmnibarSearchMode),
    SearchProvider(SearchProviderKind),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum SearchProviderKind {
    DuckDuckGo,
    Bing,
    Google,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OmnibarSearchMode {
    Mixed,
    NodesLocal,
    NodesAll,
    TabsLocal,
    TabsAll,
    EdgesLocal,
    EdgesAll,
}

#[derive(Clone, Debug)]
pub(crate) struct HistoricalNodeMatch {
    pub(crate) url: String,
    pub(crate) display_label: Option<String>,
}

impl HistoricalNodeMatch {
    pub(crate) fn new(url: impl Into<String>, display_label: Option<String>) -> Self {
        Self {
            url: url.into(),
            display_label,
        }
    }

    pub(crate) fn without_label(url: impl Into<String>) -> Self {
        Self::new(url, None)
    }
}

impl PartialEq for HistoricalNodeMatch {
    fn eq(&self, other: &Self) -> bool {
        self.url == other.url
    }
}

impl Eq for HistoricalNodeMatch {}

impl Hash for HistoricalNodeMatch {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.url.hash(state);
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum OmnibarMatch {
    Node(NodeKey),
    NodeUrl(HistoricalNodeMatch),
    SearchQuery {
        query: String,
        provider: SearchProviderKind,
    },
    Edge {
        from: NodeKey,
        to: NodeKey,
    },
    /// A durable graphlet peer of a warm node that is currently `Cold` (no live tile).
    /// Shown with ○ in the `TabsLocal` empty-query roster; activating opens a tile via
    /// graphlet routing.
    ColdGraphletMember(NodeKey),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProviderSuggestionStatus {
    Idle,
    Loading,
    Ready,
    Failed(ProviderSuggestionError),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProviderSuggestionError {
    Network,
    HttpStatus(u16),
    Parse,
}

pub(crate) struct ProviderSuggestionFetchOutcome {
    pub(crate) matches: Vec<OmnibarMatch>,
    pub(crate) status: ProviderSuggestionStatus,
}

pub(crate) struct ProviderSuggestionMailbox {
    pub(crate) request_query: Option<String>,
    pub(crate) result_mailbox: HostRequestMailbox<ProviderSuggestionFetchOutcome>,
    pub(crate) debounce_deadline: Option<Instant>,
    pub(crate) status: ProviderSuggestionStatus,
}

impl ProviderSuggestionMailbox {
    pub(crate) fn idle() -> Self {
        Self {
            request_query: None,
            result_mailbox: HostRequestMailbox::idle(),
            debounce_deadline: None,
            status: ProviderSuggestionStatus::Idle,
        }
    }

    pub(crate) fn debounced(request_query: String, debounce_deadline: Instant) -> Self {
        Self {
            request_query: Some(request_query),
            result_mailbox: HostRequestMailbox::idle(),
            debounce_deadline: Some(debounce_deadline),
            status: ProviderSuggestionStatus::Loading,
        }
    }

    pub(crate) fn ready() -> Self {
        Self {
            status: ProviderSuggestionStatus::Ready,
            ..Self::idle()
        }
    }

    pub(crate) fn clear_pending(&mut self) {
        self.request_query = None;
        self.result_mailbox.clear();
        self.debounce_deadline = None;
    }
}

pub(crate) struct OmnibarSearchSession {
    pub(crate) kind: OmnibarSessionKind,
    pub(crate) query: String,
    pub(crate) matches: Vec<OmnibarMatch>,
    pub(crate) active_index: usize,
    pub(crate) selected_indices: HashSet<usize>,
    pub(crate) anchor_index: Option<usize>,
    pub(crate) provider_mailbox: ProviderSuggestionMailbox,
}

impl OmnibarSearchSession {
    pub(crate) fn new_graph(
        kind: OmnibarSearchMode,
        query: impl Into<String>,
        matches: Vec<OmnibarMatch>,
    ) -> Self {
        Self {
            kind: OmnibarSessionKind::Graph(kind),
            query: query.into(),
            matches,
            active_index: 0,
            selected_indices: HashSet::new(),
            anchor_index: None,
            provider_mailbox: ProviderSuggestionMailbox::idle(),
        }
    }

    pub(crate) fn new_search_provider(
        provider: SearchProviderKind,
        query: impl Into<String>,
        matches: Vec<OmnibarMatch>,
        provider_mailbox: ProviderSuggestionMailbox,
    ) -> Self {
        Self {
            kind: OmnibarSessionKind::SearchProvider(provider),
            query: query.into(),
            matches,
            active_index: 0,
            selected_indices: HashSet::new(),
            anchor_index: None,
            provider_mailbox,
        }
    }
}
