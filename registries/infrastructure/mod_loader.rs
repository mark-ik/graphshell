/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// Slice 68c: this module is now a re-export shim. The mod_loader
// body moved to the `register-mod-loader` crate per the workspace
// architecture proposal. The host-side native activation table
// (mod_activation.rs) stays in tree because it hardcodes
// `crate::mods::native::*::activate` function pointers.
//
// New code should depend on `register-mod-loader` directly.

pub(crate) use register_mod_loader::*;
// Test-utils gated: register-mod-loader has its own test-utils feature
// that exposes compute_active_capabilities_with_disabled. The root
// crate's test-utils feature propagates to it via Cargo.toml. Gate on
// the feature only — the `test` cfg is per-crate, so register-mod-loader
// (built as a dep) doesn't see it during this crate's `cargo test`.
#[cfg(feature = "test-utils")]
pub(crate) use register_mod_loader::loader::compute_active_capabilities_with_disabled;
