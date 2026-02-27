# Freenet (freenet.org) Takeaways for Graphshell (2026-02-27)

**Status**: Research Notes / External Pattern Review  
**Scope**: Capture practical architecture patterns Graphshell can adopt now, and note what to avoid.

## Sources Reviewed

- https://freenet.org/
- https://freenet.org/faq/
- https://freenet.org/quickstart/
- https://freenet.org/resources/manual/components/overview/
- https://freenet.org/resources/manual/components/contracts/
- https://freenet.org/resources/manual/components/delegates/
- https://freenet.org/resources/manual/architecture/p2p-network/
- https://freenet.org/resources/manual/architecture/irouting/
- https://freenet.org/resources/manual/architecture/transport/

## Useful for Graphshell

### 1. Hard boundary between shared runtime state and secret/identity state

Freenet's contracts/delegates split is a useful reminder: not all state belongs in the same execution path.

Graphshell application:
- Keep graph/lifecycle/render state fully deterministic and testable.
- Keep identity/secrets/access checks behind explicit adapters and authority boundaries.
- Avoid identity concerns leaking into compositor or render passes.

Why:
- Reduces seam bleed and accidental coupling in migration paths.
- Keeps debugging scope smaller when runtime behavior diverges.

### 2. Capability-first integration contracts

Freenet is interface-forward across components. Graphshell should keep provider/mod boundaries similarly narrow and explicit.

Graphshell application:
- Treat storage, diagnostics, lifecycle control, and sync as separate capabilities.
- Prefer capability injection over implicit cross-module reach-through.
- Keep contract surfaces versioned and test-backed.

Why:
- Makes refactors safer while stabilizing lane work.
- Reduces "god module" growth in central runtime files.

### 3. Local-first deterministic developer loop

Freenet's local node + browser loop maps well to Graphshell's harness-first approach.

Graphshell application:
- Keep canonical scenario tests for lifecycle + compositor + diagnostics behavior.
- Validate architecture claims with executable tests before broader decomposition.

Why:
- Supports rapid iteration without distributed-debug complexity.
- Gives migration done-gates concrete evidence.

### 4. Docs should map to executable proof

Freenet's architecture manual is useful, but transport docs explicitly note ongoing spec/code mismatch.

Graphshell application:
- Every normative architecture claim should map to tests, snapshots, or harness scenarios.
- Mark proposed behavior as proposed, not implied implemented.

Why:
- Prevents doc drift during fast migration and lane parallelism.

## What Graphshell Should Avoid Copying

### 1. Spec/implementation drift

Do not allow architecture docs to imply implementation that is not test-backed.

### 2. Premature protocol/economic layering

Do not couple core Graphshell stabilization to higher-order network economics or token models.

### 3. Re-centralizing orchestration in single hint paths

Avoid introducing new central hint shortcuts that bypass reducer/reconciliation authority boundaries.

## Actionable Follow-Ons (Graphshell)

1. Add/maintain doc-to-test references in architecture docs for lifecycle/compositor claims.
2. Continue reducing direct render/compositor shortcut paths in favor of adapter boundaries.
3. Keep lane-specific compatibility shims explicitly documented with retirement conditions.

