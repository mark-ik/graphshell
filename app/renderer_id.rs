/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! App-facing renderer identity seam.
//!
//! `GraphBrowserApp` uses `RendererId` as its opaque viewer handle. The
//! concrete host-owned renderer type now lives in `verso-host`; this module
//! stays behind as a thin app-local wrapper so submodules can keep a stable
//! path while the portable app layer stops owning the implementation.

pub(crate) use verso_host::RendererId;

#[cfg(test)]
pub(crate) fn test_renderer_id() -> RendererId {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    RendererId::from_raw(COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
}
