/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! `viewer:wry` engine implementation — owns the OS-WebView overlay
//! manager, frame-source bookkeeping, and per-platform capability
//! types.
//!
//! Migrated 2026-04-25 from graphshell's `mods/native/web_runtime/`
//! (Phase A2 of the wry-into-verso lane). The thread-local
//! `WRY_MANAGER` + the public `ensure_wry_overlay_for_node` etc.
//! functions stay in graphshell main since they coordinate with
//! graphshell's `NodeKey` / windowing model; this crate owns the
//! engine itself.

pub mod frame_source;
pub mod manager;
pub mod types;

// Re-export upstream `wry` so downstream consumers (iced-wry-viewer,
// graphshell's web_runtime) get the wry crate via `verso::wry_engine::wry`.
// Keeps the version pinning + dep ownership in verso.
pub use wry;
