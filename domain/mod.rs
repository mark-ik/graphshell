use std::collections::HashMap;

use crate::app::{NoteId, NoteRecord};
use crate::graph::Graph;

/// Durable domain state owned by the app but independent of workbench/runtime layout.
pub struct DomainState {
    /// The canonical durable graph truth.
    pub graph: Graph,
    /// Counter for unique placeholder URLs (about:blank#1, about:blank#2, ...).
    /// Prevents `url_to_node` clobbering when pressing N multiple times.
    pub(super) next_placeholder_id: u32,
    /// Durable note documents keyed by note identity.
    pub(super) notes: HashMap<NoteId, NoteRecord>,
}

