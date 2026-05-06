/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Composition — the two files that describe the
//! arrangement → graph → canvas-scene composition pipeline:
//! arrangement state is reconciled into graph structure, and
//! graph state is then converted into a host-neutral scene
//! description for the canvas renderer.
//!
//! Slice 74 of the Phase 5 `app/` decomposition. The files were
//! flat siblings inside `app/` pre-Slice-74; grouping them under
//! `composition/` makes the conceptual boundary visible and is
//! the prerequisite for eventually extracting them to a
//! `graphshell-composition` crate.
//!
//! Members:
//! - [`arrangement_graph_bridge`] — the single authorised path from
//!   workbench arrangement state into graph structure mutations
//!   (ArrangementSnapshot reconciler).
//! - [`canvas_scene`] — host-neutral graph→canvas scene conversion
//!   (build_scene_input, scene_mode_to_canvas, view_id_to_canvas).
//!   Extracted from `render/canvas_bridge` so that no-Servo builds
//!   can share the conversion code.

pub(crate) mod arrangement_graph_bridge;
pub(crate) mod canvas_scene;
