/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Persistence — the three files that describe "how the app reads
//! and writes durable state to disk."
//!
//! Slice 71 of the Phase 5 `app/` decomposition. The files were
//! flat siblings inside `app/` pre-Slice-71; grouping them under
//! `persistence/` makes the conceptual boundary visible.
//!
//! Members:
//! - [`persistence_facade`] — GraphBrowserApp impls that wrap the
//!   underlying GraphStore (snapshot health, named-graph snapshot
//!   accessors, workspace-layout autosave).
//! - [`startup_persistence`] — boot-path graph recovery and store
//!   open/seed flow with diagnostic emission.
//! - [`settings_persistence`] — the user-preference type vocabulary
//!   (ThemeMode, FocusRingSettings, ThumbnailSettings, etc.) plus
//!   their on-disk read/write helpers.
//!
//! The `pub use` re-exports below preserve every existing
//! `crate::app::Foo` resolution.
//!
//! Note: the host-side webview-storage plumbing lives in the
//! sibling `storage_interop` module; it stays separate because
//! it bridges browser storage backends rather than graph state.

pub(crate) mod persistence_facade;
pub(crate) mod settings_persistence;
pub(crate) mod startup_persistence;

#[allow(unused_imports)]
pub use settings_persistence::{
    DefaultWebViewerBackend, FocusRingCurve, FocusRingSettings, NavigatorSidebarSidePreference,
    SettingsToolPage, ThemeMode, ThumbnailAspect, ThumbnailFilter, ThumbnailFormat,
    ThumbnailSettings, WorkspaceUserStylesheetSetting, WryRenderModePreference,
};
