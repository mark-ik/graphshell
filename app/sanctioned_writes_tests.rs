/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Cross-cutting contract tests that enforce the iced-host migration's
//! sanctioned-writes boundaries via repo-wide source scanning.
//!
//! Four boundaries are enforced here, all referenced by the iced-host
//! migration plan §12:
//!
//! - §12.3 — Persisted node navigation memory: `Graph::set_node_history_state`
//!   (Graph-level) and `Node::replace_history_state` (Node-level primitive).
//! - §12.1 — Arrangement→graph bridge: `add_arrangement_relation_if_missing`
//!   and `promote_arrangement_relation_to_frame_membership` may only be called
//!   from the bridge or their definition file.
//! - §12.2 — Durable graph mutation kernel: the lower-level kernel function
//!   `apply_graph_delta` may only be called from the kernel itself, kernel
//!   test fixtures, and the sanctioned WAL-replay path. Production durable
//!   mutations must route through `apply_graph_delta_and_sync` (which adds
//!   `post_apply_sync`); direct kernel calls bypass the sync.
//! - §12.17 — Host-owned mutation entrypoints: host adapter files
//!   (`iced_host.rs`, `iced_app.rs`, `iced_events.rs`, `iced_host_ports.rs`,
//!   `egui_host_ports.rs`) must not call the canonical mutation entrypoints
//!   `apply_graph_delta_and_sync` or `apply_arrangement_snapshot`.
//!
//! All needles are constructed via `concat!()` so this test source itself
//! does not match.

use std::fs;
use std::path::{Path, PathBuf};

// ── Shared scanning infrastructure ───────────────────────────────────────────

/// Directories never traversed during the repo-wide scan.
const SKIP_DIRS: &[&str] = &["target", ".git", "node_modules", "design_docs", "snapshots"];

fn walk_rs_files(root: &Path, into: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if SKIP_DIRS.contains(&name) {
                continue;
            }
            walk_rs_files(&path, into);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            into.push(path);
        }
    }
}

fn relative_to_repo(path: &Path, repo_root: &Path) -> String {
    path.strip_prefix(repo_root)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| path.to_string_lossy().to_string())
}

/// Repo-wide scanner. Fails if `needle` appears in any `.rs` file outside
/// `allowed_files`. Used when the protected identifier may legitimately
/// appear at a small known set of sanctioned sites repo-wide.
fn assert_no_unsanctioned_callers(needle: &str, allowed_files: &[&str], sanction_message: &str) {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut rs_files = Vec::new();
    walk_rs_files(&repo_root, &mut rs_files);

    let mut violations: Vec<String> = Vec::new();
    for file in &rs_files {
        let rel = relative_to_repo(file, &repo_root);
        if allowed_files.iter().any(|allowed| rel == *allowed) {
            continue;
        }
        let Ok(contents) = fs::read_to_string(file) else {
            continue;
        };
        for (line_idx, line) in contents.lines().enumerate() {
            if line.contains(needle) {
                violations.push(format!("{}:{}: {}", rel, line_idx + 1, line.trim()));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "{}\n\nViolations:\n  {}",
        sanction_message,
        violations.join("\n  ")
    );
}

/// Targeted scanner. Fails if `needle` appears in any of `target_files`.
/// Used when the rule is "this identifier must NOT appear in this small
/// fixed set of files at all" (the §12.17 host-adapter pattern).
fn assert_needle_absent_from_files(needle: &str, target_files: &[&str], sanction_message: &str) {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut violations: Vec<String> = Vec::new();
    for target in target_files {
        let path = repo_root.join(target);
        let Ok(contents) = fs::read_to_string(&path) else {
            continue;
        };
        for (line_idx, line) in contents.lines().enumerate() {
            if line.contains(needle) {
                violations.push(format!("{}:{}: {}", target, line_idx + 1, line.trim()));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "{}\n\nViolations:\n  {}",
        sanction_message,
        violations.join("\n  ")
    );
}

// ── §12.3 — Persisted node navigation memory ────────────────────────────────

/// Allowlist for the Graph-level setter. After the typed-delta migration
/// only two literal mention sites remain inside `graphshell-core`: the
/// function definition (`mod.rs`) and the `GraphDelta::UpdateNodeHistory`
/// match arm in `apply.rs`. The setter is `pub(crate)` inside
/// `graphshell-core`, so callers outside the kernel are compile-time
/// guaranteed to go through the typed delta.
const SET_NODE_HISTORY_ALLOWED_FILES: &[&str] = &[
    "crates/graphshell-core/src/graph/mod.rs",
    "crates/graphshell-core/src/graph/apply.rs",
];

/// Allowlist for the Node-level primitive. `Node::replace_history_state`
/// remains `pub` because it is the natural fixture-construction primitive
/// for `Node::test_stub(...)`-based detached unit tests, which can't go
/// through the typed delta (no Graph to apply against). The allowlist
/// captures the definition + every currently-known test caller; adding
/// a new file here is a deliberate signal in PR review that should be
/// scrutinized for whether the new caller is genuinely test-only.
const NODE_REPLACE_HISTORY_ALLOWED_FILES: &[&str] = &[
    "crates/graphshell-core/src/graph/mod.rs",
    "graph_app_tests.rs",
    "app/clip_capture.rs",
    "render/panels.rs",
    "shell/desktop/ui/workbench_host.rs",
    "shell/desktop/runtime/registries/index.rs",
    "shell/desktop/runtime/registries/action.rs",
    "shell/desktop/lifecycle/webview_backpressure.rs",
    "shell/desktop/tests/scenarios/navigation.rs",
];

#[test]
fn no_unsanctioned_set_node_history_state_writes() {
    let needle: &str = concat!("set_node_history", "_state(");
    assert_no_unsanctioned_callers(
        needle,
        SET_NODE_HISTORY_ALLOWED_FILES,
        "Unsanctioned direct writes to the Graph-level history setter.\n\
         All durable history writes must route through \
         `app::history::GraphBrowserApp::apply_node_history_change`,\n\
         which dispatches the typed `GraphDelta::UpdateNodeHistory` \
         variant. See iced-host migration plan \u{00A7}12.3.",
    );
}

#[test]
fn no_unsanctioned_node_replace_history_state_writes() {
    let needle: &str = concat!("replace_history", "_state(");
    assert_no_unsanctioned_callers(
        needle,
        NODE_REPLACE_HISTORY_ALLOWED_FILES,
        "Unsanctioned direct writes to the Node-level history primitive.\n\
         The Node-level primitive is preserved as `pub` for detached \
         fixture construction via `Node::test_stub(...)`,\n\
         but every currently-known caller is allowlisted. \
         A new file here means either:\n\
           (a) a new test fixture \u{2014} add the file to \
         `NODE_REPLACE_HISTORY_ALLOWED_FILES` after PR review confirms \
         test-only usage,\n\
           (b) a non-test caller \u{2014} route through \
         `GraphBrowserApp::apply_node_history_change` instead.\n\
         See iced-host migration plan \u{00A7}12.3.",
    );
}

// ── §12.1 — Arrangement-to-graph bridge sole-writer ─────────────────────────

/// Allowlist shared by both arrangement-helper guards. The two helpers live
/// in `app/graph_mutations.rs` (definition + internal composition) and are
/// only reached from `app/arrangement_graph_bridge.rs` on the production
/// path. A new file here means a new caller is entering the bridge \u2014
/// either re-route through `GraphBrowserApp::apply_arrangement_snapshot` or
/// justify the new bypass.
const ARRANGEMENT_HELPER_ALLOWED_FILES: &[&str] =
    &["app/graph_mutations.rs", "app/arrangement_graph_bridge.rs"];

#[test]
fn no_unsanctioned_add_arrangement_relation_calls() {
    let needle: &str = concat!(".add_arrangement_relation", "_if_missing(");
    assert_no_unsanctioned_callers(
        needle,
        ARRANGEMENT_HELPER_ALLOWED_FILES,
        "Unsanctioned direct calls to `add_arrangement_relation_if_missing`.\n\
         Arrangement-driven graph mutations must route through \
         `GraphBrowserApp::apply_arrangement_snapshot(&snapshot)` so the \
         plain-data `ArrangementSnapshot` boundary is preserved.\n\
         See iced-host migration plan \u{00A7}12.1.",
    );
}

#[test]
fn no_unsanctioned_promote_arrangement_relation_calls() {
    let needle: &str = concat!(".promote_arrangement_relation", "_to_frame_membership(");
    assert_no_unsanctioned_callers(
        needle,
        ARRANGEMENT_HELPER_ALLOWED_FILES,
        "Unsanctioned direct calls to \
         `promote_arrangement_relation_to_frame_membership`.\n\
         Arrangement-driven graph mutations must route through \
         `GraphBrowserApp::apply_arrangement_snapshot(&snapshot)`.\n\
         See iced-host migration plan \u{00A7}12.1.",
    );
}

// ── §12.2 — Durable graph mutation kernel sole-callers ──────────────────────

/// Allowlist for the kernel-level `apply_graph_delta` function (signature
/// `(&mut Graph, GraphDelta) -> GraphDeltaResult`). Production durable
/// mutations must route through `apply_graph_delta_and_sync` (in
/// `app/graph_mutations.rs`), which composes the typed kernel mutation
/// with `post_apply_sync`. Calling the kernel directly bypasses the sync.
/// The allowlist captures the only sites where a direct kernel call is
/// legitimate:
///
/// - `crates/graphshell-core/src/graph/apply.rs` — kernel definition itself.
/// - `crates/graphshell-core/src/graph/facet_projection.rs` — kernel-internal
///   test fixtures (mirrors the `Node::replace_history_state` rationale:
///   detached `Graph`-only test construction can't go through the app sync
///   wrapper because there's no `GraphBrowserApp` to call against).
/// - `graph/graphlet.rs`, `graph/frame_affinity.rs` — graph-crate test
///   fixtures, same rationale.
/// - `services/persistence/mod.rs` — WAL/snapshot replay path. Replay
///   re-applies persisted typed deltas to reconstruct the saved graph and
///   intentionally skips `post_apply_sync` (sync state is restored from
///   the snapshot, not re-derived from each replayed delta).
///
/// `app/graph_mutations.rs` is NOT in this list because its sanctioned
/// wrapper imports the kernel function under the renamed alias
/// `apply_domain_graph_delta`, so the protected literal does not appear
/// there. A new file in this allowlist is a deliberate review signal —
/// either it's a legitimate replay/test fixture path, or the new caller
/// should be re-routed through the sync wrapper.
const APPLY_GRAPH_DELTA_KERNEL_ALLOWED_FILES: &[&str] = &[
    "crates/graphshell-core/src/graph/apply.rs",
    "crates/graphshell-core/src/graph/facet_projection.rs",
    "graph/graphlet.rs",
    "graph/frame_affinity.rs",
    "services/persistence/mod.rs",
];

#[test]
fn no_unsanctioned_apply_graph_delta_kernel_calls() {
    let needle: &str = concat!("apply_graph", "_delta(");
    assert_no_unsanctioned_callers(
        needle,
        APPLY_GRAPH_DELTA_KERNEL_ALLOWED_FILES,
        "Unsanctioned direct call to the kernel function `apply_graph_delta`.\n\
         Production durable mutations must route through \
         `apply_graph_delta_and_sync` in `app/graph_mutations.rs`, which \
         composes the typed kernel mutation with `post_apply_sync`. Direct \
         kernel calls bypass the sync and leave derived state stale.\n\
         If this is a new replay/test-fixture path, add the file to \
         `APPLY_GRAPH_DELTA_KERNEL_ALLOWED_FILES` after PR review confirms \
         the bypass is intentional.\n\
         See iced-host migration plan \u{00A7}12.2.",
    );
}

// ── §12.17 — Host-owned mutation entrypoints ────────────────────────────────

/// Host adapter files that must not directly mutate domain state. These
/// translate input/render between OS/framework and the host-neutral runtime
/// (`runtime.tick(input, ports) -> view_model`); domain mutation belongs in
/// the runtime, intent reducer, or sanctioned helpers, not in host glue.
///
/// Two host-adjacent files are intentionally NOT in this list:
/// - `iced_graph_canvas.rs` \u2014 graph-canvas integration that legitimately
///   constructs test fixtures via `add_node_and_sync` in `#[test]` blocks.
/// - `iced_parity.rs` \u2014 parity-replay scaffold that may need fixture
///   construction.
///
/// A new file added here triggers the sanction; a new file added to
/// `shell/desktop/ui/{iced,egui}_*.rs` should be considered for inclusion
/// during PR review.
const HOST_ADAPTER_FILES: &[&str] = &[
    "shell/desktop/ui/iced_host.rs",
    "shell/desktop/ui/iced_app.rs",
    "shell/desktop/ui/iced_events.rs",
    "shell/desktop/ui/iced_host_ports.rs",
    "shell/desktop/ui/egui_host_ports.rs",
];

#[test]
fn host_adapters_do_not_call_apply_graph_delta_and_sync() {
    let needle: &str = concat!("apply_graph_delta", "_and_sync(");
    assert_needle_absent_from_files(
        needle,
        HOST_ADAPTER_FILES,
        "Host adapter file calls the canonical typed-mutation entrypoint.\n\
         Hosts must not own domain mutation. Translate the input event into \
         a `GraphIntent` or runtime command and route it through the \
         runtime (`runtime.tick(input, ports)`); the runtime owns the \
         decision to apply a graph delta.\n\
         See iced-host migration plan \u{00A7}12.17.",
    );
}

#[test]
fn host_adapters_do_not_call_apply_arrangement_snapshot() {
    let needle: &str = concat!("apply_arrangement", "_snapshot(");
    assert_needle_absent_from_files(
        needle,
        HOST_ADAPTER_FILES,
        "Host adapter file calls the arrangement\u{2192}graph bridge entrypoint.\n\
         Arrangement reconciliation is a runtime concern, not a host concern.\n\
         The host should produce arrangement intents/events the runtime \
         translates into a snapshot; the runtime then calls \
         `apply_arrangement_snapshot`.\n\
         See iced-host migration plan \u{00A7}12.17.",
    );
}
