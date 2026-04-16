/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Graph data structures for the spatial browser — host shim.
//!
//! Canonical implementations live in `graphshell_core::graph`. This module
//! re-exports them and provides host-only sub-modules (badge, egui_adapter,
//! edge_style_registry) that depend on egui or platform I/O.

// Re-export everything from core's graph module.
pub use graphshell_core::graph::*;

// Re-export core sub-modules so host code can use `crate::graph::apply::*` etc.
pub use graphshell_core::graph::{apply, facet_projection, filter};

// Re-export leaf types from core (previously defined here).
pub use graphshell_core::types::{
    ArchivedClassificationProvenance, ArchivedClassificationScheme, ArchivedClassificationStatus,
    ClassificationProvenance, ClassificationScheme, ClassificationStatus, DominantEdge,
    FrameLayoutHint, FrameLayoutNodeId, ImportRecord, ImportRecordMembership, NodeClassification,
    NodeImportProvenance, NodeImportRecordSummary, SplitOrientation, format_imported_at_secs,
};

// Re-export address types from core (previously defined here).
pub use graphshell_core::address::{
    Address, AddressKind, address_from_url, address_kind_from_url, cached_host_from_url,
    detect_mime,
};

// Host-only sub-modules (depend on egui, platform I/O, etc.).
pub mod badge;
pub mod edge_style_registry;
pub mod egui_adapter;
