# Test Infrastructure Improvement Plan (2026-02-26)

**Status**: Planned
**Lane**: `lane:runtime` (low-gui-churn; no hotspot file conflict)
**Linked subsystem**: `SUBSYSTEM_DIAGNOSTICS.md` §10 items T1, T2
**Linked plan**: `2026-02-22_test_harness_consolidation_plan.md` (operational context)

---

## Background

The current test infrastructure is correct and scales to the present test count with no issues.
Two latent problems appear at 10x scale:

1. `ACTIVE_CAPABILITIES` `OnceLock` is shared across all test threads — its initialization is
   env-driven and cached forever in the test process. Tests that exercise different disabled-mod
   configurations race on initialization order and may all see the same cached result.

2. All test code (`shell/desktop/tests/`, `_for_tests` helpers, `new_for_testing()`) is compiled into
   the library for every `cargo test` run. At 10x scale, changing any test file invalidates the
   entire library compile cache. Splitting to a separate Cargo `[[test]]` binary gives the
   compiler a boundary to cache across.

Neither problem causes wrong results in the current test suite. Both become correctness or
velocity problems as the suite grows.

---

## Item T1 — Make `ACTIVE_CAPABILITIES` test-safe

**File**: `registries/infrastructure/mod_loader.rs`

**Problem**: `runtime_has_capability(capability_id)` calls
`ACTIVE_CAPABILITIES.get_or_init(compute_active_capabilities)`. `compute_active_capabilities`
reads `GRAPHSHELL_DISABLE_MODS` and `GRAPHSHELL_DISABLE_VERSO` from env, then caches the result
in a process-global `OnceLock`. In a parallel test process, any test that sets env vars and then
calls `runtime_has_capability` wins or loses the initialization race. All subsequent calls in all
threads see the winning thread's cached set, regardless of what env vars other tests have set.

**Fix**:

1. Add a test-only entry point that bypasses the OnceLock:

```rust
#[cfg(test)]
pub(crate) fn compute_active_capabilities_with_disabled(
    disabled: &std::collections::HashSet<String>,
) -> std::collections::HashSet<String> {
    let mut registry = ModRegistry::new_with_disabled(disabled);
    let _ = registry.resolve_dependencies();
    let _ = registry.load_all();
    registry.active_capability_ids()
}
```

2. Update all tests that call `runtime_has_capability` with a specific disabled-mod configuration
   to call `compute_active_capabilities_with_disabled` directly instead of going through the
   OnceLock. The production `runtime_has_capability` path is unchanged.

3. Add a contract test asserting the two paths agree for the default (no-disabled) case.

**Scope**: `registries/infrastructure/mod_loader.rs` only. Zero changes to production call sites.
`cargo check` must stay green. Targeted test: `mod_registry_without_verso_disables_webview_capability`.

**Acceptance**:
- `cargo test mod_loader` green with `--test-threads=1` and `--test-threads=8`.
- No test touches `GRAPHSHELL_DISABLE_MODS` or `GRAPHSHELL_DISABLE_VERSO` env vars to influence
  the OnceLock path.

---

## Item T2 — Split integration tests to a separate `[[test]]` binary

**Files**:
- `Cargo.toml` (add `[[test]]` target)
- `shell/desktop/tests/` (move to `tests/integration/` at crate root, or add a shim entry point)
- `app.rs`, `shell/`, `registries/` (widen visibility of `_for_tests` helpers from `pub(crate)`
  to `pub` under a `test-utils` feature flag)

**Problem**: `shell/desktop/tests/` is compiled as part of the library (gated by `#[cfg(test)]`).
Every edit to any test file forces recompilation of the entire library. At 10x test scale with
frequent test edits this is a meaningful velocity tax. A Cargo `[[test]]` binary is a separate
compilation unit: editing it does not invalidate the library cache.

**The visibility problem**: `[[test]]` binaries are external consumers of the library. They only
see `pub` items. All current `_for_tests` helpers are `pub(crate)`. Two paths forward:

### Option A — `test-utils` feature flag (recommended)

Add a `test-utils` Cargo feature. Gate all `_for_tests` helpers and `new_for_testing()` with
`#[cfg(any(test, feature = "test-utils"))]` instead of `#[cfg(test)]`. Change their visibility
to `pub` under the same gate. The `[[test]]` binary adds `required-features = ["test-utils"]`.

Pros: Clean separation. Test-utils helpers are still absent in release builds.
Cons: All gated helpers need a visibility change from `pub(crate)` to `pub`. Mechanical but
     requires touching many files.

### Option B — Keep unit tests inline, add `[[test]]` only for new scenario tests

Leave existing `#[cfg(test)]` unit tests in place. Add a `[[test]]` binary only for new
scenario files written after this plan. New scenarios only call the `pub` surface of the app,
relying on `DiagnosticsState` and channel assertions (the observability-driven model documented
in `2026-02-22_test_harness_consolidation_plan.md`).

Pros: Zero changes to existing code. New scenarios naturally follow the black-box model.
Cons: Two test models coexist. Legacy unit tests still pollute the library compile cache.

**Recommended sequence**:

1. Land Option B first (zero churn, immediate benefit for new tests).
2. Migrate existing scenario tests from `shell/desktop/tests/` to the `[[test]]` binary incrementally,
   widening visibility of helpers as needed.
3. Once all scenarios are in the `[[test]]` binary, evaluate whether the remaining inline unit
   tests (in `app.rs`, `shell/`, etc.) are worth migrating. Most are pure unit logic and are
   fine inline.

**Cargo.toml addition** (Option B):

```toml
[[test]]
name = "scenarios"
path = "tests/scenarios/main.rs"
required-features = ["test-utils"]
```

**`test-utils` feature addition**:

```toml
[features]
test-utils = []
```

**Scope**: The `[[test]]` binary entry point is a new file. No existing files change in step 1.
Steps 2+ are incremental; each migration slice is a standalone PR.

**Acceptance** (step 1):
- `cargo test --features test-utils --test scenarios` runs the new binary and passes.
- `cargo test` (no feature flag) still runs all existing inline tests and passes.
- `cargo build --release` (no feature flag) contains no test code.

---

## Sequencing

```
T1 (OnceLock fix)     — standalone PR, lane:runtime, no hotspot conflict
T2 step 1 (new [[test]] binary, Option B)
                      — standalone PR, lane:runtime, no hotspot conflict
T2 step 2+ (incremental migration)
                      — one PR per scenario pouch migrated; mechanical
```

T1 and T2 step 1 can land in either order or as a single PR. They do not conflict.

T2 step 2+ depends on T2 step 1 being merged first (branch on the new binary target).

---

## Files Touched

| File | Item | Change |
|------|------|--------|
| `registries/infrastructure/mod_loader.rs` | T1 | Add `compute_active_capabilities_with_disabled` test helper |
| `Cargo.toml` | T2 | Add `[[test]]` target + `test-utils` feature |
| `tests/scenarios/main.rs` (new) | T2 | Entry point for integration test binary |
| `app.rs`, `shell/**`, `registries/**` | T2 step 2+ | Widen `_for_tests` visibility incrementally |

---

## Non-Goals

- This plan does not change how `cargo test` is invoked in CI.
- This plan does not change `DiagnosticsState`, channel schemas, or any production code path.
- This plan does not remove or rename `shell/desktop/tests/` — it coexists until migration is complete.
- "Pouches" as a terminology change is out of scope; the scenario-file organization is already
  functionally equivalent and renaming adds churn with no benefit.
