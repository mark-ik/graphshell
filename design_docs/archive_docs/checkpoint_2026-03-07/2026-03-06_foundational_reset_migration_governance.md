# Foundational Reset Migration Governance

**Date**: 2026-03-06
**Status**: Active governance policy
**Purpose**: Define the rules, gates, and enforcement mechanisms that prevent the foundational reset from becoming another partial migration with lingering duplicate authority.

**Related**:
- `2026-03-06_foundational_reset_architecture_vision.md`
- `2026-03-06_foundational_reset_demolition_plan.md`
- `2026-03-06_foundational_reset_implementation_plan.md`
- `2026-03-06_reducer_only_mutation_enforcement_plan.md`
- `../../../testing/test_guide.md`

---

## 1. Decision Summary

The foundational reset succeeds only if Graphshell treats deletion, authority transfer, and unknown-surface discovery as first-class deliverables.

This governance policy establishes five hard rules:

1. No dual authority.
2. No migration slice is complete without deletion.
3. Compatibility layers are explicit, narrow, and time-boxed.
4. Unknown or undocumented code paths must be actively searched for and classified.
5. The migration must be enforced by executable checks, not memory or doc prose alone.

Execution model:

- the foundational reset is governed globally
- actual implementation should proceed through narrow Component-Local Authority Transfers (CLATs)
- each CLAT transfers one authority boundary at a time

---

## 2. Core Failure Mode To Avoid

The main migration risk is not that the new architecture is wrong.

The main migration risk is:

- the new model is added
- the old model remains partially active
- both models acquire real callsites
- bugs appear at the seam
- the seam becomes normalized as "how the app works"

This governance policy exists to prevent that outcome.

---

## 3. Non-Negotiable Rules

### 3.1 No dual authority

For every architectural concept under migration, there must be one canonical owner at a time.

Examples:

- selection truth: one owner
- semantic truth: one owner
- frame membership: one owner
- traversal truth: one owner
- undo boundary ownership: one owner

Bridges may exist, but they must not create peer authorities.

### 3.2 No addition without subtraction

A migration slice is not complete when the new path works.

A migration slice is complete only when:

- the new canonical path exists
- the old path is deleted, blocked, or isolated behind a temporary bridge
- enforcement exists to prevent reintroduction

### 3.3 Temporary bridges must be named debt

Every compatibility layer must include:

- the old concept being bridged
- the new canonical concept
- the exact files/modules where the bridge exists
- a removal condition
- a linked demolition item

Unnamed compatibility glue is forbidden.

### 3.4 No feature work on retired foundations

Once a foundation is declared retired or retirement-in-progress:

- no new feature work may add callsites to it
- no new docs may present it as canonical
- no new tests may normalize it as default behavior

### 3.5 Unknown surfaces are part of scope

Undocumented or hard-to-find code paths do not count as "out of scope."

Every migration slice must actively search for:

- undocumented callsites
- stale comments
- stale tests
- stale specs
- stale helper APIs
- implicit runtime dependencies

---

## 4. Definition Of Done For A Migration Slice

A migration slice is `done` only when all of the following are true:

1. Canonical authority is declared in active specs.
2. The codebase has one canonical write/read path for the migrated concept.
3. Old entry points are deleted, made private, or blocked by tests/checks.
4. Boundary tests or banned-token checks fail on regression.
5. Active docs no longer describe the retired model as current.
6. Unknown-surface search was rerun and its findings were triaged.
7. Full compile and targeted test evidence is attached.

If any one of these is missing, the slice is still `in progress`.

### 4.1 CLAT rule

The preferred execution unit is one Component-Local Authority Transfer (CLAT).

Each CLAT must answer:

1. What exact authority boundary is being transferred?
2. What discovery/search receipt was produced?
3. What minimal code change makes the transfer real?
4. What regression check prevents reintroduction?
5. What canonical doc now names the new owner?

If a proposed slice cannot answer those five questions precisely, it is still too large.

---

## 5. Migration Artifact Set

Every foundational migration must maintain four artifacts:

1. Architecture vision
2. Governance policy
3. Demolition plan
4. Implementation plan

Optional but preferred:

5. Issue register / tracking hub
6. Banned-symbol or docs-parity check
7. Boundary-contract test

---

## 6. Migration Ledger

Every foundational concept under migration must be tracked in a ledger with one status:

- `active-legacy`
- `bridge-active`
- `bridged`
- `canonicalized`
- `deleted`

Each ledger row must include:

- concept name
- old authority
- new authority
- linked implementation phase
- bridge location if any
- enforcement location
- demolition owner

The ledger belongs in the demolition plan, not in scattered issue comments.

---

## 7. Unknown-Surface Accounting

### 7.1 Required discovery work

For each migration slice, perform all of the following:

1. Repo-wide symbol search for old APIs/terms.
2. Repo-wide comment/spec search for old semantics.
3. Targeted scan of tests for normalized legacy assumptions.
4. Compile validation.
5. Broad test validation plus slice-specific targeted tests.

### 7.2 Required search classes

Searches must include:

- function/method names
- field names
- enum variants
- doc phrases
- comments
- issue references if they define current semantics

### 7.3 Unknown-surface triage categories

Each discovery hit must be classified:

- `canonical`
- `bridge`
- `stale-doc`
- `stale-test`
- `legacy-runtime`
- `legacy-helper`
- `false-positive`

Anything not classified remains open work.

### 7.4 Dark-matter rule

If a file or module is materially adjacent to a migration slice but was never mentioned in specs or prior plans, it must still be reviewed.

Examples:

- helper scripts
- test-only builders
- persistence recovery code
- lifecycle adapters
- bridge/orchestration modules

Prototype code tends to accumulate hidden authority there.

---

## 8. Enforcement Mechanisms

### 8.1 Required enforcement types

Each migration slice should use at least two of:

- compile-time visibility restriction
- trusted-writer/boundary contract test
- docs-parity script
- banned-token grep test
- scenario/harness regression
- targeted unit test proving canonical authority

### 8.2 Preferred pattern

Best pattern:

1. narrow visibility
2. add boundary test
3. add docs-parity/banned-term check
4. delete old path

### 8.3 Regression rule

If an old API or semantic term is retired, the repo should contain at least one failing check that triggers if it reappears in active code/docs.

---

## 9. Review Policy

Foundational-reset changes should be reviewed with one question first:

> What old truth still survives after this change?

Secondary questions:

- Is there still duplicate authority?
- Is the bridge named and time-boxed?
- What unknown surfaces were searched?
- What exact files prove deletion?
- What test/check now fails if the old model returns?

---

## 10. Spec Governance

### 10.1 Canonical vs summary docs

Canonical subsystem specs define truth.
Overview docs summarize and link.

Overview docs must not restate data models in a way that can drift into competing authority.

### 10.2 Retired vocabulary

When a term is retired or narrowed:

- active specs must stop using it in the old sense
- archived docs may keep historical usage
- docs-parity checks should guard against reintroduction in active docs

### 10.3 Spec-change requirement

No foundational code migration closes without the corresponding active spec changes landing in the same slice or an explicitly linked prerequisite slice.

---

## 11. Code Governance

### 11.1 Single entry rule

Durable or semantically authoritative state changes should have one obvious entry path.

If reviewers can plausibly ask "which of these two paths is canonical?", the migration is not finished.

### 11.2 Helper rule

Prototype convenience helpers are acceptable only if they are clearly:

- constructors
- test builders
- read-only utilities

Helpers that mutate canonical state must either become canonical entry points or be removed.

### 11.3 Runtime rule

Runtime code may realize or project canonical state.
Runtime code must not quietly become semantic authority.

---

## 12. Escalation Rule

If a migration slice discovers that the new target model itself is unclear, implementation must pause long enough to resolve the semantic authority question in active specs.

Do not continue coding on top of ambiguous replacement architecture.

---

## 13. Required Governance Deliverables For The Foundational Reset

Before broad implementation begins, the reset must maintain:

1. A demolition ledger covering all targeted legacy foundations.
2. An implementation plan mapping each foundation to code/spec work.
3. A repo-level search and enforcement strategy for unknown surfaces.
4. Linked validation commands for compile and test evidence.

This document defines the policy.
The demolition and implementation documents define the execution.
