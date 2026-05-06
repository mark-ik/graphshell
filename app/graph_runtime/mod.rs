/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Graph runtime — the five files that describe "the in-process
//! graph state the app tracks: view state, layout dispatch,
//! mutations, projection invalidation, and the shared vocabulary
//! types those operate on."
//!
//! Slice 72 of the Phase 5 `app/` decomposition. The files were
//! flat siblings inside `app/` pre-Slice-72; grouping them under
//! `graph_runtime/` makes the conceptual boundary visible.
//!
//! Members:
//! - [`graph_app_types`] — vocabulary types (LifecycleCause,
//!   RuntimeBlockReason, RendererId, etc.) re-exported wholesale
//!   into `crate::app::*` because many app/ siblings depend on them.
//! - [`graph_views`] — graph view state machinery (GraphViewState,
//!   GraphViewFrame, Camera, ViewDimension, PolicyValueSource).
//! - [`graph_layout`] — layout-algorithm dispatch + the
//!   GRAPH_LAYOUT_* algorithm-id constants (~30 external call sites
//!   in registries/, render/, and intent_phases pull constants
//!   directly from this module's path).
//! - [`graph_mutations`] — graph mutation impls + NoteId/NoteRecord.
//! - [`graph_cartography`] — projection-invalidation adapters for
//!   the graph-cartography runtime.
//!
//! Re-exports below preserve every existing `crate::app::Foo` path.
//! The sibling `graph_app.rs` additionally re-exports `graph_layout`
//! and `graph_views` as modules so existing `crate::app::graph_layout::*`
//! call sites continue to resolve.

pub(crate) mod graph_app_types;
pub(crate) mod graph_cartography;
pub(crate) mod graph_layout;
pub(crate) mod graph_mutations;
pub(crate) mod graph_views;
