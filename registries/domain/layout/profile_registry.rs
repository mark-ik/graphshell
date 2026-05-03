/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// Slice 55: this module is now a re-export shim. The body moved to
// the register-layout crate per the workspace architecture proposal.
// New code should depend on register-layout directly.

pub(crate) use register_layout::profile_registry::*;
