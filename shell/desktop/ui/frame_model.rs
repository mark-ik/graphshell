/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Host ↔ Runtime frame protocol.
//!
//! Per the M3.5 runtime boundary design
//! (`design_docs/graphshell_docs/implementation_strategy/shell/2026-04-16_runtime_boundary_design.md`
//! §5), this module defines the shapes that flow between a
//! `GraphshellRuntime` and whatever host (egui today, iced later) is
//! driving it. All types are now in
//! [`graphshell_core::shell_state::frame_model`] — M4 slice 10
//! (2026-04-22) completed the portable move for `FrameViewModel`
//! itself once `OverlayStrokePass` extracted to
//! [`graphshell_core::overlay`]. The re-exports below preserve the
//! shell-side import paths so no call-site churn landed.
//!
//! Post-M3.6, the boundary vocabulary uses portable types
//! (`PortableRect`, `PortablePoint`, `PortableSize` from
//! `graphshell_core::geometry`). Egui hosts convert at population
//! sites (`gui.rs::build_frame_host_input`,
//! `gui_state.rs::project_view_model`); iced hosts consume portable
//! types directly.

#[allow(unused_imports)]
pub(crate) use graphshell_core::shell_state::frame_model::{
    AccessibilityViewModel, CommandPaletteScopeView, CommandPaletteViewModel, DegradedReceiptSpec,
    DialogsViewModel, FocusRingCurve, FocusRingSettingsView, FocusRingSpec, FocusViewModel,
    FrameHostInput, FrameViewModel, GraphSearchViewModel, OmnibarProviderStatusView,
    OmnibarSessionKindView, OmnibarViewModel, SettingsViewModel, ThumbnailAspectView,
    ThumbnailFilterView, ThumbnailFormatView, ThumbnailSettingsView, ToastSeverity, ToastSpec,
    ToolbarViewModel,
};
