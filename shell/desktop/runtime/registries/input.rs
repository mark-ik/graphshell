/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// Slice 54: this module is now a re-export shim. The 1463-LOC input
// binding registry body moved to the `register-input` crate per the
// workspace architecture proposal. The original file had zero
// `crate::*` deps (only std + serde derives), making it the cleanest
// shell-side registry to extract once the keystone
// `register-diagnostics` (Slice 53) had landed.
//
// New code should depend on `register-input` directly.

pub(crate) use register_input::*;
