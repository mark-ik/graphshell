<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Flaky Test Hygiene Plan (2026-04-19)

**Status**: **Archived 2026-04-20** — Complete. 2026-04-19 audit pass
plus follow-ups resolved all six originally-known flakes plus five
newly-surfaced flakes. Five consecutive full-suite runs clean:
`cargo test -p graphshell --lib` — 2144/2144 passing. The
navigator-specialty corridor flake turned out to be a **deterministic
bug in the test data**, not a parallelism race — `REGISTRY_RUNTIME`
was a red herring.
**Scope**: Two lib-tests fail non-deterministically under
`cargo test --lib` but pass when run alone or with `--test-threads=1`.
Both predate the 2026-04-19 egui_graphs retirement work; the retirement
just surfaced them because it reshuffled which tests run in which parallel
worker. This plan isolates the shared state causing the flakiness and
either fixes the offending globals or pins the affected tests to a
serialized worker.

---

## 1. Tests in scope

### 1.1 `navigator_specialty_corridor_uses_selected_pair_and_tree_layout`

- Location: [shell/desktop/ui/workbench_host.rs:7382](../../../../shell/desktop/ui/workbench_host.rs#L7382)
- Failure point: assertion `mask.contains(&middle)` at [workbench_host.rs:7437](../../../../shell/desktop/ui/workbench_host.rs#L7437)
- What it tests: corridor-projection graphlet with a left→middle→right
  chain, where the test selects `left` and `right` and expects the derived
  corridor mask to contain all three nodes.
- Failure shape: `middle` missing from the mask.

Suspected cause: a process-global that feeds graphlet derivation (likely
one of the phase3 registry singletons or a selection-scope global) is
left in a state from a prior test, so when this test queries "what
graphlet is derived from {left, right}?", the answer depends on test
ordering.

### 1.2 `radial_sector_count_violation_flags_overfull_radial_palette`

- Location: [shell/desktop/workbench/ux_tree.rs:2773](../../../../shell/desktop/workbench/ux_tree.rs#L2773)
- Failure point: `.expect("probe should detect overfull radial palette sector count")`
- What it tests: ux-probe that flags a radial palette with more sectors
  than a configured cap. The test calls `clear_semantic_snapshot()` between
  setup and query, but the setup-install of the probe fixture happens via
  a global the test expects to be clean.

Suspected cause: `clear_semantic_snapshot()` only clears one of several
globals touched during probe setup; a sibling test leaves a conflicting
fixture registered.

---

## 2. Investigation tasks

Each task is a small, isolated investigation — do them in order, stop
when the root cause is identified.

### Task A: enumerate shared-global surface

Grep for `thread_local!`, `static`, `lazy_static!`, `OnceLock`, `Mutex<_>`,
`RwLock<_>` in the paths touched by the two tests:

- `shell/desktop/ui/workbench_host.rs`
- `shell/desktop/workbench/ux_tree.rs`
- `shell/desktop/runtime/registries/mod.rs`
- `app/graph_views.rs`
- `app/focus_selection.rs`

Expect to find several registry runtime singletons (
`phase3_resolve_active_canvas_profile`, theme registry, etc.). Catalog
which ones get mutated during the two tests.

### Task B: narrow by pairing tests

Use `cargo test --lib -- --test-threads=2` with deliberately-chosen test
pairs to bisect: which *other* test needs to run concurrently with the
failing one to trigger the failure? Start by pairing each failing test
with tests in its own module, then siblings in the same file, then
registries tests.

A rustc-ordered test filter lets this go quickly:

```bash
cargo test --lib -- --test-threads=2 \
  navigator_specialty_corridor_uses_selected_pair_and_tree_layout \
  <candidate-other-test>
```

### Task C: classify the root cause

For each failing test, answer:

1. Does it read a global and get a polluted value?
2. Does it write a global and another test reads the leftover?
3. Does the sibling test under-set-up because an earlier test (in the
   same process) already bootstrapped a registry?

The fix for (1) is `tests/common/fixtures.rs` style test-scoped setup;
for (2) it's `clear_all()` in `Drop` of the test harness; for (3) it's
making registry init idempotent.

---

## 3. Remediation options

Pick per-test based on the root cause. Roughly ordered from cheapest to
most invasive:

### 3.1 Add explicit fixture reset before assertion

If the shared state is a named registry runtime, call its reset helper
at the top of the test (or inside its test harness `new()`). This is
usually enough for writer-pollutes-reader failures.

### 3.2 Scope the test to `serial_test`

Add a `#[serial]` attribute (from the `serial_test` crate) so just these
two tests run on a single worker. Small additive dependency; surgical;
does not fix the underlying shared state but removes the symptom.

This is the right choice if the shared state is intrinsically
process-global (e.g., a singleton registry that's by-design
process-wide).

### 3.3 Refactor the shared state to per-test

If the shared state is accidentally global (e.g., a `thread_local!` used
where a test-harness-scoped value would do), refactor. Biggest
remediation; highest long-term value.

### 3.4 `#[ignore]` with a reference to this plan

Last resort. Use only if (a) the test covers a feature that's itself
deferred, or (b) the remediation is too costly for this session. The
ignored test must reference the tracking issue or this plan so it isn't
forgotten.

---

## 4. Acceptance

- `cargo test --lib` passes consistently across five consecutive runs on
  the reporter's machine.
- No `#[ignore]` is added without a linked follow-on issue.
- The `serial_test` dependency, if added, is pinned to a minor version
  and its usage is documented in the test file.

---

## 5. Progress

### 2026-04-19

- Plan created after observing flakiness in the egui_graphs retirement
  session. Two tests identified; investigation not yet started.

- **Investigation + partial remediation** landed later the same day.

  - **§1.2 radial palette snapshot race — FIXED.** Root cause:
    `LATEST_RADIAL_SEMANTIC_SNAPSHOT` in
    [render/radial_menu.rs](../../../../render/radial_menu.rs) is a
    process-global `OnceLock<Mutex<Option<RadialPaletteSemanticSnapshot>>>`.
    Multiple tests (`radial_sector_count_violation_flags_overfull_radial_palette`,
    `snapshot_projects_radial_sector_metadata_when_available`,
    `evaluate_registered_probes_surfaces_radial_sector_count_violation`,
    `command_surface_radial_palette_case`) walked
    `publish_semantic_snapshot` → `build_snapshot`/`evaluate_registered_probes`
    → `clear_semantic_snapshot`, racing each other under the default
    `cargo test --lib` parallelism. The flake appears as one test's
    `clear_semantic_snapshot()` landing between another test's
    `publish_semantic_snapshot(...)` and its `build_snapshot(...)`,
    so the reader sees `None` and the `.expect("probe should detect ...")`
    panics.
  - Fix: added `lock_radial_palette_snapshot_tests() ->
    MutexGuard<'static, ()>` in `render/radial_menu.rs`, following the
    established `lock_command_surface_snapshot_tests()` pattern for
    the sibling command-surface snapshot global. Every test that runs
    the publish-read-clear sequence now binds `let _guard =
    lock_radial_palette_snapshot_tests();` at the top. Sites updated:
    [shell/desktop/workbench/ux_tree.rs](../../../../shell/desktop/workbench/ux_tree.rs)
    (two tests),
    [shell/desktop/workbench/ux_probes.rs](../../../../shell/desktop/workbench/ux_probes.rs)
    (one test, chained under the pre-existing `lock_probe_tests()` mutex),
    [shell/desktop/tests/scenarios/ux_tree_diff_gate.rs](../../../../shell/desktop/tests/scenarios/ux_tree_diff_gate.rs)
    (one scenario case).
  - The lock is `pub(crate)` and non-`cfg(test)` to match the
    existing sibling pattern; it has no runtime cost in production
    because the mutex is never acquired outside tests.

  - **§1.1 navigator specialty corridor race — INVESTIGATED, NOT FIXED.**
    Traced via an Agent sweep of the `SetNavigatorSpecialtyView` →
    workbench intent → corridor-mask derivation pipeline. Two globals
    along the path: `EMPTY_SELECTION` in
    [app/focus_selection.rs](../../../../app/focus_selection.rs) (`OnceLock<SelectionState>`,
    returned only as a fallback when a selection scope is not yet
    populated), and `REGISTRY_RUNTIME` in
    [shell/desktop/runtime/registries/mod.rs](../../../../shell/desktop/runtime/registries/mod.rs)
    (`OnceLock<Arc<RegistryRuntime>>` wrapping a tree of per-registry
    `Mutex`es coordinating intent dispatch).
  - Most likely culprit: `REGISTRY_RUNTIME`'s internal mutexes are held
    by other tests during dispatch, and one of those sibling tests
    leaves a specialty-view-affecting registry entry that perturbs the
    corridor-mask derivation for this test. The exact writer hasn't
    been isolated yet — doing so needs the §2 Task-B bisection
    (deliberate test-pair pairings) which burns enough CPU time to be
    a standalone session.
  - Measured flake rate under current parallelism: **~20 %** (1 of 5
    full-suite runs failed pre-fix; the radial-palette fix does not
    touch this test, so the rate after landing is unchanged).
  - **Pragmatic choice this session**: leave the test as-is rather
    than apply a coarse test mutex in `workbench_host.rs`. Rationale:
    (a) 80 % pass rate means CI reliability is marginal but not
    broken; (b) a mutex would paper over an actual state leak between
    tests that should be fixed at the source; (c) the §1.1 failure
    does not block the retirement / re-land sequence that motivated
    this plan. Ticket the proper fix as its own session.
  - **Next-session entry points** for the corridor race:
    1. Run `cargo test --lib -- --test-threads=2 \
       navigator_specialty_corridor_uses_selected_pair_and_tree_layout
       <candidate>` across the other workbench_host tests that call
       `SetNavigatorSpecialtyView` to identify the perturbing sibling.
    2. Inside the identified sibling, look for a
       `registries::atomic::*` registry mutation that is made before
       a matching reset runs.
    3. Prefer remediation §3.1 (explicit fixture reset) over §3.2
       (`serial_test`) since the registry runtime is the root of
       shared state, and making its reset explicit is far more
       valuable than serializing one test.

- **Receipts so far**: `cargo check --workspace --exclude servoshell
  --exclude webdriver_server` clean; `cargo test -p graphshell --lib`
  runs after the radial-palette fix show the two tests in §1.2 now
  pass every run; the navigator flake remains at roughly the same
  ~20 % rate, confirming the two issues were independent.

- **Out-of-scope flakes observed during verification** (not in §1
  originally; surfaced by repeated `cargo test -p graphshell --lib`
  runs). None of these relate to the radial-palette or corridor
  investigations and each needs its own root-cause work:
  - `app::tests::test_nostr_subscriptions_persist_across_restart`
  - `shell::desktop::host::webdriver_runtime::tests::ux_bridge_action_script_queues_node_pane_dismiss`
  - `shell::desktop::workbench::ux_bridge::tests::queued_bridge_action_maps_graph_surface_to_open_and_close_intents`
  - `shell::desktop::workbench::ux_bridge::tests::queued_bridge_action_maps_node_pane_to_open_and_dismiss_intents`
  - `shell::desktop::workbench::ux_bridge::tests::queued_bridge_action_maps_tool_pane_to_open_and_close_intents`

  The `ux_bridge` and `webdriver_runtime::ux_bridge_action_*` family
  likely share a queue/dispatcher global similar to `REGISTRY_RUNTIME`;
  `test_nostr_subscriptions_persist_across_restart` appears to depend
  on a persistence fixture across test boundaries. Worth a broader
  pass that enumerates every `OnceLock` / `static mut` / `Mutex<_>`
  in the crate and audits each for test-reset contracts, rather than
  patching flakes one at a time.

### 2026-04-19 (audit pass)

User said "proper audit!" → full enumeration of crate globals, mapping
of globals to each of the six flaky tests, and targeted fixes per
root cause. Summary of what changed and why.

**Global enumeration (context).** ~50 process-global state
declarations across the crate were enumerated and reviewed for
test-reset contracts. The ones touching flaky tests:
- `LATEST_UX_TREE_SNAPSHOT` (`shell/desktop/workbench/ux_tree.rs`) —
  UX-tree snapshot write slot; publish/read/clear dance.
- `REGISTRY_RUNTIME.nostr_core` (`shell/desktop/runtime/registries/mod.rs`)
  — master nostr state behind `phase3_nostr_*` accessors.

**Fix 1 — ux_bridge + webdriver_runtime snapshot race.** Promoted the
previously-local `UX_BRIDGE_TEST_LOCK` from inside `ux_bridge::tests`
to a crate-level `pub(crate) fn lock_ux_tree_snapshot_tests()` in
[shell/desktop/workbench/ux_tree.rs](../../../../shell/desktop/workbench/ux_tree.rs)
(same `OnceLock<Mutex<()>>` pattern as the sibling command-surface
and radial-palette locks). Applied at six test sites:
- `webdriver_runtime.rs` tests: `ux_bridge_query_script_reports_missing_snapshot`,
  `ux_bridge_action_script_queues_open_command_palette` (chained under
  the pre-existing `lock_command_surface_snapshot_tests`), and
  `ux_bridge_action_script_queues_node_pane_dismiss`.
- `ux_bridge.rs` tests: the three `queued_bridge_action_maps_*`
  tests now call the promoted lock via `lock_bridge_tests()`, which
  is now a one-line alias that defers to the module-level lock.

Also added `ux_tree::clear_snapshot()` to the end of the
`ux_bridge_action_script_queues_node_pane_dismiss` and
`ux_bridge_action_script_queues_open_command_palette` tests so they
leave the global clean for the next test, matching the sibling
tests' `clear_snapshot` cleanup.

**Fix 2 — nostr subscription race across three separate per-test
LOCKs.** The three `test_nostr_*_persist_across_restart` tests in
[graph_app_tests.rs](../../../../graph_app_tests.rs) each had their
own `static LOCK: OnceLock<Mutex<()>>`, so the tests did not actually
serialize against each other despite each *thinking* it was locking
out parallel runs. Consolidated them into a shared
`pub(crate) fn lock_phase3_nostr_tests()` in
[shell/desktop/runtime/registries/mod.rs](../../../../shell/desktop/runtime/registries/mod.rs);
every test calling `phase3_nostr_*` now binds that guard. Also
applied to `security_health_snapshot_reports_trust_and_nostr_runtime_state`
in [shell/desktop/runtime/diagnostics.rs](../../../../shell/desktop/runtime/diagnostics.rs),
which was the newly-surfaced 5/5 failure after the first consolidation
pass (it touches `phase3_trust_peer`, `phase3_nostr_use_local_signer`,
`phase3_nostr_set_nip07_permission`, `phase3_nostr_relay_subscribe_for_caller`).

**Fix 3 (the big one) — navigator-specialty corridor is a test-data
bug, not a parallelism race.** Direct isolation testing showed the
failure happens ~60 % of the time even with `--test-threads=1` and
the corridor test as the only selected test. That ruled out every
cross-test-state hypothesis. Adding debug `eprintln` around the
graph's `shortest_path(left, right)` call revealed the non-determinism:
sometimes the returned path was `[left, right]` (a direct 1-hop edge
that shouldn't exist), sometimes `[left, middle, right]` (the correct
two-hop path through the authored edges).

Tracing the 1-hop path to its source: `apply_graph_delta_and_sync`
calls `rebuild_derived_containment_relations` after every node-add.
That method scans nodes per URL host, picks a `domain_anchor` for
each host (lowest path depth, UUID as tiebreaker), then adds a
`ContainmentRelation::Domain` edge from every other node in the host
back to the anchor. The test's three URLs shared the `example.com`
host, so:

- If the anchor chosen by UUID was `middle`, edges `left → middle`
  and `right → middle` were added — same direction as the authored
  hyperlinks, no shorter path exists, mask is correct.
- If the anchor was `left` or `right`, a new domain-relation edge
  connected the two endpoints directly (via the anchor); undirected
  A* found the 1-hop path and returned the degenerate corridor
  `{left, right}` — the test's failure state.

UUID is `Uuid::new_v4()` (random), so the anchor — and hence whether
the flake fires — was 2/3 random each run. **The fix is a single
line of test data change**: distinct URL hosts (`left.test`,
`middle.test`, `right.test`). With distinct hosts, no domain anchor
ties the three nodes together, no bypassing containment edge is
synthesized, and the authored hyperlink path is the only path
between left and right. A comment in the test explains the subtlety
so future readers don't reintroduce it.

This one warranted the deep dig: the `REGISTRY_RUNTIME` hypothesis
from the earlier investigation and the "parallelism race" framing
were both wrong. The test was a ticking bomb that just happened to
be running alongside other tests when its bomb went off. Deterministic
isolation testing is what caught it.

**Verification.** Full suite was run 5 times post-fix. See the
run-by-run summary in the next progress entry (dated the same day,
added after the 5-run settled). The acceptance criterion in §4
("passes consistently across five consecutive runs") is met.

**Receipts**:
- `cargo check --workspace --exclude servoshell --exclude webdriver_server`
  clean.
- 5× isolation run of `navigator_specialty_corridor_uses_selected_pair_and_tree_layout`
  post-fix: 5/5 pass (vs 2/5 pre-fix under the same isolation conditions).
- Ux-bridge and webdriver-runtime targeted tests with the new lock:
  green.
- `test_nostr_*_persist_across_restart` + `security_health_snapshot_*`
  with consolidated lock: green.

### 2026-04-19 (audit pass follow-up)

After landing the first wave (§1.1 + §1.2 + nostr lock + ux_tree lock),
a 5× verify surfaced a second wave of flakes — none of them in the
original §1, but all exposed once the first wave's locks redistributed
parallelism. Each was root-caused and fixed the same day.

**Flake A — `security_health_snapshot_reports_trust_and_nostr_runtime_state`**
failed 4/5 runs even with the nostr lock applied. Root cause: a prior
panic of this same test could leave the `trusted_peers` / nostr state
dirty on the `REGISTRY_RUNTIME` singleton, so the next run's
`trusted_peer_count == 1` assertion saw 2+ and cascaded failures. Fix:
**defensive reset at the start of the test**, not just at the end.
Revoke every existing trusted peer, clear persisted nostr
subscriptions and NIP-07 permissions before the assertions run, so
the test is idempotent against its own past panics.

**Flake B — `host_neutral_builder_{matches_tiles_builder_on_empty_tree,
is_strict_prefix_of_tiles_builder}`** raced against parallel tests
that publish `LATEST_RADIAL_SEMANTIC_SNAPSHOT` and
`LATEST_COMMAND_SURFACE_SEMANTIC_SNAPSHOT`. The builders read both
globals as part of snapshot construction; "ambient" published state
from another test perturbs them. Fix: acquire both
`lock_radial_palette_snapshot_tests()` and
`lock_command_surface_snapshot_tests()` at the top, clear the
snapshots, then build — matching the pattern already used by the
third sibling test, `host_neutral_builder_includes_root_radial_command_workbench`.

**Flake C — five `phase3_*_profile*` tests in
`shell/desktop/runtime/registries/mod.rs`** mutate
`REGISTRY_RUNTIME`'s active workbench-surface, canvas, physics, and
theme profile state. The existing code even carried a "// Reset to
default first to avoid contamination from tests that change the
active profile" comment — the author knew — but no lock held the
profile constant between the reset and the assertion. Fix: new
`pub(crate) fn lock_phase3_profile_tests()` in
[shell/desktop/runtime/registries/mod.rs](../../../../shell/desktop/runtime/registries/mod.rs)
(matching the `lock_phase3_nostr_tests` pattern), applied to six tests:
`phase3_workbench_surface_resolution_returns_default_profile`,
`phase3_canvas_profile_switches_and_applies_workspace_preferences`,
`phase3_physics_profile_switches_and_falls_back`,
`phase3_presentation_profile_tracks_active_physics_and_theme`,
`phase3_workbench_surface_switches_and_describes_profiles`, and
`phase3_workflow_describes_stub_and_activates_runtime_defaults`
(which also flips active_physics + active_workbench_surface as a
side effect of workflow activation).

**Flake D — compositor_adapter trio
(`retire_stale_content_resources_only_prunes_unretained_nodes`,
`selected_bridge_metadata_flows_through_registration_into_diagnostics`,
`synthetic_viewer_can_register_generic_content_callback`)** all
failed together in run 4 of the verify. These tests clear the same
`COMPOSITOR_CONTENT_CALLBACKS` + `COMPOSITOR_NATIVE_TEXTURES`
globals. Two of them already held a `resource_retirement_test_lock`,
but three others (`compose_registered_content_pass_requires_registered_callback`,
`synthetic_viewer_can_register_generic_content_callback`, and
`selected_bridge_metadata_flows_through_registration_into_diagnostics`)
did not — even though they cleared the same globals. The third one
carried a different lock (`backend_bridge_test_env_lock`) which did
not protect the content-callback registry. Fix: each of the three
now acquires `resource_retirement_test_lock`; the third one chains
both locks.

**Flake E — `accessibility_inspector_snapshot_reports_selected_node_profile`**
asserts `snapshot.command_surface_semantic_node_count == 0`, reading
the `LATEST_COMMAND_SURFACE_SEMANTIC_SNAPSHOT` global. A parallel
test's published snapshot would push the count above zero. Fix:
acquire `lock_command_surface_snapshot_tests` and clear the global
at the top of the test, mirroring the pattern in the sibling
`accessibility_inspector_snapshot_counts_command_surface_semantics`.

**Final verification.** After all fixes land,
`cargo test -p graphshell --lib` over **5 consecutive runs** — all
clean:

```
run 1: 2144 passed; 0 failed
run 2: 2144 passed; 0 failed
run 3: 2144 passed; 0 failed
run 4: 2144 passed; 0 failed
run 5: 2144 passed; 0 failed
```

Acceptance criterion in §4 is met. `cargo check --workspace --exclude
servoshell --exclude webdriver_server` clean (pre-existing warnings
only). The plan's §3.1 ("Add explicit fixture reset before assertion")
and §3.2 ("Scope the test to `serial_test`" — substituted with
custom `OnceLock<Mutex<()>>` patterns already in the codebase) were
both used. §3.3 (refactor shared state to per-test) was not needed
for any flake. §3.4 (`#[ignore]`) was not used.

**Pattern catalogue landed in the crate** (for future flaky-test
work — all follow the `lock_*_tests() -> MutexGuard<'static, ()>`
shape with `OnceLock<Mutex<()>>` + poison-recovery):
- `lock_command_surface_snapshot_tests` (pre-existing)
- `lock_radial_palette_snapshot_tests` (first wave)
- `lock_ux_tree_snapshot_tests` (first wave — new)
- `lock_phase3_nostr_tests` (first wave — new)
- `lock_phase3_profile_tests` (this wave — new)
- `resource_retirement_test_lock` (pre-existing, applied more widely)
- `backend_bridge_test_env_lock` (pre-existing)
- `lock_probe_tests` (pre-existing)

Any new test that mutates a `REGISTRY_RUNTIME` singleton or a
module-level `OnceLock<Mutex<Option<T>>>` should bind the
corresponding `lock_*_tests()` guard before publish/clear/assert.
If the global is new, follow the existing pattern — stick a lock
next to the existing global's publish/clear helpers, and audit
every test that touches those helpers.
