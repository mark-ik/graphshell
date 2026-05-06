/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Slice 51 (2026-05-04, Phase 2 of the workspace architecture proposal):
//! the 842-LOC signal-routing body moved to `graphshell-runtime::system::signal_bus`.
//! This file is now a `pub(crate) use` shim so existing call sites
//! (`crate::shell::desktop::runtime::registries::signal_routing::SignalKind`,
//! etc.) continue to resolve unchanged.
//!
//! New code should depend on `graphshell_runtime::system::signal_bus` directly.

pub(crate) use graphshell_runtime::system::signal_bus::*;
