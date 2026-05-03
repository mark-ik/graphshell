/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// Slice 53: this module is now a re-export shim. The diagnostics
// channel descriptor catalog and `DiagnosticsRegistry` body moved
// to the `register-diagnostics` crate as the keystone for the
// registrar sweep — together with the 253 `CHANNEL_*` constants
// that were previously declared in
// `shell/desktop/runtime/registries/mod.rs` (now re-exported there
// as `pub(crate) use register_diagnostics::channels::*`).
//
// New code should depend on `register-diagnostics` directly.

pub(crate) use register_diagnostics::descriptor::*;
