/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Spartan (`spartan://`) protocol parser — **scaffold crate**.
//!
//! [Spartan](https://spartan.mozz.us) is a small-web protocol in the
//! gemini family: text-only `text/gemini`-shaped responses, but the
//! request envelope is simpler (no TLS, prompt-driven input). The
//! response body is gemtext, so a future implementation will likely
//! delegate to [`middlenet-gemini`](../middlenet-gemini/index.html)
//! for the body parse and add Spartan-specific request/prompt
//! handling on top.
//!
//! Slice 61 scaffold: returns `Err` for now. The crate exists to
//! reserve the namespace and hold the `parse_spartan` signature so
//! the dispatcher in `middlenet-adapters` can route
//! `MiddleNetContentKind::SpartanText` here as soon as the parser
//! lands.

use middlenet_core::document::SemanticDocument;
use middlenet_core::source::MiddleNetSource;

pub fn parse_spartan(_source: &MiddleNetSource, _body: &str) -> Result<SemanticDocument, String> {
    Err(
        "middlenet-spartan: parser not yet implemented (Slice 61 scaffold). \
         Spartan response bodies are gemtext-shaped; full impl will likely \
         delegate to middlenet-gemini for the body parse."
            .to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use middlenet_core::source::MiddleNetContentKind;

    #[test]
    fn scaffold_returns_not_implemented_error() {
        let source = MiddleNetSource::new(MiddleNetContentKind::SpartanText);
        assert!(parse_spartan(&source, "# Hello\n").is_err());
    }
}
