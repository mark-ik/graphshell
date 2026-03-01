# 2026-03-01 Backend Bridge Contract Receipt (`#183`)

## Scope

This receipt records the agreed backend migration strategy for composited-content paths: **C+F**.

- **C (Contract-first):** backend-agnostic bridge contract at compositor/content-pass boundaries.
- **F (Fallback-safe):** wgpu-primary implementation with capability-driven fallback path.

The goal is to avoid permanent Glow maintenance while preserving a controlled parity/benchmark window during migration.

## Canonical planning reference

- Planning Register update: `PLANNING_REGISTER.md` §0.11 “Backend Bridge Contract Rollout (C+F)”.
- Readiness row update: `PLANNING_REGISTER.md` §1A prerequisite table now marks surface-composition closure as `partial` with C+F evidence linked.

## Policy captured

1. Contract boundaries are landed before backend replacement closure.
2. Glow remains temporary baseline infrastructure only while wgpu bridge parity is proven.
3. Capability checks and fallback routing are required before Glow retirement.
4. Glow path retirement is allowed once wgpu + fallback are redundant for supported targets.

## Acceptance gates (for backend migration closure)

- Compositor replay diagnostics parity versus Glow baseline.
- No open stabilization regressions in pass-order, callback-state isolation, or overlay affordance visibility.
- Fallback path behavior validated in non-interop scenarios.
- Tracker-linked evidence proves required pass-contract scenarios are covered by wgpu-primary + fallback-safe paths.

## Tracker linkage

- Primary migration tracker: `#183`.
- Related lane hubs: `#88`, `#99`, `#92`, `#90` (where relevant to pass contract, policy parity, render mode authority, and callback-state invariants).

## Session evidence summary (already landed)

The following backend-seam commits were landed on `main` immediately prior to this receipt and support the contract-first direction:

- `a98236d` — backend graphics context aliasing at compositor seams.
- `3aaaaa8` — compositor scissor operations routed through backend helpers.
- `a898ed6` — compositor GL state ops routed through backend helper APIs.
- `7fe9168` — UI renderer wrapped behind backend handle type.

These slices reduce direct backend-shape coupling outside backend ownership boundaries.

## Suggested issue update payload (`#183`)

> Added planning evidence for the agreed C+F bridge strategy:
> - `design_docs/graphshell_docs/implementation_strategy/PLANNING_REGISTER.md` (§0.11)
> - `design_docs/graphshell_docs/implementation_strategy/2026-03-01_backend_bridge_contract_c_plus_f_receipt.md`
>
> This records: contract-first migration boundaries, wgpu-primary + fallback-safe closure policy, and explicit Glow retirement gates.
