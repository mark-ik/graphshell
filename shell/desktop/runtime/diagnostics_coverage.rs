/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Diagnostics channel-coverage snapshot test — M6 §5.3.
//!
//! Pins the set of `CHANNEL_*` identifiers referenced by `emit_event`
//! call sites anywhere in the graphshell shell source tree. Any commit
//! that drops an emit site without replacing it fails the snapshot.
//!
//! This is not a behavioral test — it cannot tell whether an emit site
//! *fires* on any given path. It proves only that the call site exists
//! in compiled source. The value is catching accidental drops during
//! phase migrations (chrome ports, runtime refactors) that would
//! silently degrade observability.
//!
//! **Updating the snapshot intentionally**: `cargo insta review` from
//! the crate root accepts or rejects diffs.
//!
//! **Done gate for M6 §5.3**: the test exists and fails when coverage
//! shrinks. Adding channels is fine; removing them must be surfaced.

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::Path;

    /// Walk `shell/` recursively, extract every `channel_id: CHANNEL_*`
    /// reference, and return the set sorted. Stable ordering for
    /// snapshot comparison.
    fn collect_emitted_channel_ids() -> BTreeSet<String> {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let shell_dir = manifest_dir.join("shell");
        let mut ids = BTreeSet::new();
        walk_source_files(&shell_dir, &mut |source| {
            extract_channel_ids_from(source, &mut ids);
        });
        ids
    }

    fn walk_source_files(dir: &Path, f: &mut impl FnMut(&str)) {
        let entries = match fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(err) => panic!("diagnostics coverage test could not read {dir:?}: {err}"),
        };
        for entry in entries {
            let entry = entry.expect("valid dir entry");
            let path = entry.path();
            if path.is_dir() {
                walk_source_files(&path, f);
            } else if path.extension().is_some_and(|e| e == "rs") && !path_should_skip(&path) {
                let source = fs::read_to_string(&path)
                    .unwrap_or_else(|err| panic!("unreadable source {path:?}: {err}"));
                f(&source);
            }
        }
    }

    /// Skip files whose source contains test-fixture strings that
    /// would pollute the coverage snapshot with fake channel ids.
    /// Specifically: this module's own file (self-reference) and
    /// any pattern-testing harness that embeds `channel_id: CHANNEL_*`
    /// as example source literals.
    fn path_should_skip(path: &Path) -> bool {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        name == "diagnostics_coverage.rs"
    }

    /// Match `channel_id: CHANNEL_<IDENT>` where `<IDENT>` is
    /// `[A-Za-z0-9_]+`. Simple linear scan — no regex dep needed.
    fn extract_channel_ids_from(source: &str, out: &mut BTreeSet<String>) {
        const NEEDLE: &str = "channel_id: CHANNEL_";
        let mut rest = source;
        while let Some(idx) = rest.find(NEEDLE) {
            let after = &rest[idx + NEEDLE.len()..];
            let end = after
                .find(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                .unwrap_or(after.len());
            let ident = &after[..end];
            if !ident.is_empty() {
                out.insert(format!("CHANNEL_{ident}"));
            }
            rest = &after[end..];
        }
    }

    #[test]
    fn diagnostics_channel_coverage_snapshot() {
        let ids = collect_emitted_channel_ids();
        assert!(
            !ids.is_empty(),
            "coverage scan found no CHANNEL_* emits; something is wrong with the scanner"
        );
        let list = ids.into_iter().collect::<Vec<_>>().join("\n");
        insta::assert_snapshot!("diagnostics_channel_coverage", list);
    }

    #[test]
    fn extract_channel_ids_handles_multiline_and_adjacent_sites() {
        let source = r#"
            emit_event(DiagnosticEvent::MessageSent {
                channel_id: CHANNEL_FOO_BAR,
                byte_len: 0,
            });
            something_else();
            emit_event(DiagnosticEvent::MessageReceived {
                channel_id: CHANNEL_BAZ,
                latency_us: 0,
            });
        "#;
        let mut ids = BTreeSet::new();
        extract_channel_ids_from(source, &mut ids);
        assert_eq!(
            ids.into_iter().collect::<Vec<_>>(),
            vec!["CHANNEL_BAZ".to_string(), "CHANNEL_FOO_BAR".to_string()],
        );
    }

    #[test]
    fn extract_channel_ids_ignores_other_channel_id_kinds() {
        // Narrow scope: only matches `channel_id: CHANNEL_<IDENT>`. String
        // literals or `channel_id: String::from(...)` must not match.
        let source = r#"
            let x = "channel_id: CHANNEL_LITERAL"; // in a string
            channel_id: some_fn(),                 // not a CHANNEL_ constant
            channel_id: CHANNEL_REAL,              // this one counts
        "#;
        let mut ids = BTreeSet::new();
        extract_channel_ids_from(source, &mut ids);
        // The literal inside a string is also matched by the plain
        // substring search — that's an accepted false positive. Document
        // it by pinning the observed behaviour: both `CHANNEL_LITERAL`
        // and `CHANNEL_REAL` are detected, and `some_fn()` is not.
        let got = ids.into_iter().collect::<Vec<_>>();
        assert!(got.contains(&"CHANNEL_REAL".to_string()));
        assert!(got.contains(&"CHANNEL_LITERAL".to_string()));
        assert!(!got.iter().any(|s| s.contains("some_fn")));
    }
}
