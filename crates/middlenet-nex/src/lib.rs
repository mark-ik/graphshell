/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Nex (`nex://`) protocol parser — **scaffold crate**.
//!
//! [Nex](https://nex.nightfall.city) is a minimal small-web
//! protocol: directory-listing responses for index requests, plain
//! text for content requests. Unlike gopher there's no item-type
//! prefix on directory lines — links are detected by trailing `/`
//! (subdirectory) or by absence (file). A future implementation
//! distinguishes the two response shapes by inspecting the trailing
//! line shape (lines ending in `/` indicate a directory listing).
//!
//! Slice 61 scaffold: returns `Err` for now. The crate exists to
//! reserve the namespace and hold the `parse_nex` signature so the
//! dispatcher in `middlenet-adapters` can route
//! `MiddleNetContentKind::NexDirectory` here as soon as the parser
//! lands.

use middlenet_core::document::SemanticDocument;
use middlenet_core::source::MiddleNetSource;

pub fn parse_nex(_source: &MiddleNetSource, _body: &str) -> Result<SemanticDocument, String> {
    Err(
        "middlenet-nex: parser not yet implemented (Slice 61 scaffold). \
         Nex distinguishes directory listings from text content by line \
         shape; full impl handles both shapes."
            .to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use middlenet_core::source::MiddleNetContentKind;

    #[test]
    fn scaffold_returns_not_implemented_error() {
        let source = MiddleNetSource::new(MiddleNetContentKind::NexDirectory);
        assert!(parse_nex(&source, "subdir/\nfile.txt\n").is_err());
    }
}
