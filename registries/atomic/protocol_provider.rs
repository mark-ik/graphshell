/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// Slice 51: this module is now a re-export shim. The
// `ProtocolHandlerProvider` trait and `ProtocolHandlerProviders`
// bundle moved to the `register-protocol` crate as the natural
// sibling of `ProtocolContractRegistry` (the trait registers handlers
// *into* the registry — they belong together).
//
// New code should depend on `register-protocol` directly and import
// `register_protocol::{ProtocolHandlerProvider, ProtocolHandlerProviders}`.

pub(crate) use register_protocol::{ProtocolHandlerProvider, ProtocolHandlerProviders};
