/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// Slice 50b: this module is now a re-export shim. The
// `ProtocolContractRegistry` and friends moved to the
// `register-protocol` crate as the proof-of-concept for the
// per-registry split described in
// `design_docs/graphshell_docs/implementation_strategy/2026-05-01_workspace_architecture_proposal.md`.
//
// Existing call sites import via this path; the re-exports below keep
// them working unchanged. New code should depend on `register-protocol`
// directly.

pub(crate) use register_protocol::{
    ContentStream, ProtocolContractRegistry, ProtocolContractResolution, ProtocolError,
    ProtocolHandler,
};
