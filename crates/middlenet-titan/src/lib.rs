/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Titan (`titan://`) protocol parser — **scaffold crate**.
//!
//! [Titan](https://transjovian.org/titan/) is the write companion
//! to gemini: clients submit a body to a `titan://` URL with
//! `;mime=text/gemini;size=N;token=…` parameters; servers respond
//! with a gemini-shaped status line (typically `30` redirecting to
//! the resulting `gemini://` URL). The submitted body is gemtext;
//! the parser concerns are mostly request-envelope shape (the
//! `;mime=…;size=…;token=…` parsing) plus body validation.
//!
//! Slice 61 scaffold: returns `Err` for now. A future
//! implementation distinguishes the request envelope from the
//! body, validates body length against `;size=`, and delegates to
//! [`middlenet-gemini`](../middlenet-gemini/index.html) for the
//! body parse.

use middlenet_core::document::SemanticDocument;
use middlenet_core::source::MiddleNetSource;

pub fn parse_titan(_source: &MiddleNetSource, _body: &str) -> Result<SemanticDocument, String> {
    Err(
        "middlenet-titan: parser not yet implemented (Slice 61 scaffold). \
         Titan is the write companion to gemini; full impl parses the \
         request envelope and delegates to middlenet-gemini for the body."
            .to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use middlenet_core::source::MiddleNetContentKind;

    #[test]
    fn scaffold_returns_not_implemented_error() {
        let source = MiddleNetSource::new(MiddleNetContentKind::TitanWrite);
        assert!(parse_titan(&source, "# Submission body\n").is_err());
    }
}
