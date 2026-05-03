/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

// Slice 52: this module is now a re-export shim. The
// `KnowledgeRegistry` and supporting types moved to the
// `register-knowledge` crate per the workspace architecture proposal.
// The seed dataset (`udc_seed.json`) travels with the new crate at
// `crates/registrar/register-knowledge/assets/knowledge/`.
//
// New code should depend on `register-knowledge` directly.

pub(crate) use register_knowledge::{
    CompactCode, KnowledgeRegistry, SemanticClassVector, TagValidationResult, UdcEntry,
};
