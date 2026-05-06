/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! App UX — the three files that describe "how the app exposes
//! navigation surfaces, action menus, and clip-inspector flows
//! to the user."
//!
//! Slice 73 of the Phase 5 `app/` decomposition. The files were
//! flat siblings inside `app/` pre-Slice-73; grouping them under
//! `app_ux/` makes the conceptual boundary visible.
//!
//! Members:
//! - [`ux_navigation`] — modal-surface state machine (ModalSurface
//!   enum + GraphBrowserApp impls that drive arrangement-projection
//!   and focus-capture diagnostics).
//! - [`action_surface`] — single-enum action-surface state
//!   (ActionSurfaceState + ActionScope/Anchor/ScopeTarget vocabulary)
//!   that replaced the legacy boolean flags; held in WorkbenchSession.
//! - [`clip_capture`] — clip-inspector data model + filter/query
//!   helpers (ClipCaptureData, ClipInspectorState, etc.).

pub(crate) mod action_surface;
pub(crate) mod clip_capture;
pub(crate) mod ux_navigation;
