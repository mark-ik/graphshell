# Graphshell Versioning Policy

**Status**: Active / Canonical for release tagging decisions  
**Updated**: 2026-02-26

## Purpose

Define when Graphshell should bump versions and create releases, so lane completion and user-facing release signaling remain separate and consistent.

## Core Rule

Graphshell is **release-versioned, not lane-versioned**.

- A lane completion does **not** require a version bump by itself.
- A version bump should happen when we intend to publish a coherent release artifact/tag for users.

## Current SemVer Interpretation (pre-1.0)

Project is currently `0.y.z`, so minor bumps represent meaningful compatibility and milestone shifts.

- **Patch (`0.y.z` -> `0.y.z+1`)**
  - Bug fixes and small stabilization changes
  - No major architecture boundary changes
  - Safe default for routine releases

- **Minor (`0.y.z` -> `0.(y+1).0`)**
  - Significant architecture or platform milestone
  - Notable user-facing capability expansion
  - Example: retirement of servoshell inheritance boundary and canonical migration completion

- **Major (`0.y.z` -> `1.0.0`)**
  - Reserved for first stable contract declaration
  - Requires explicit readiness criteria and release notes quality gate

## Lane-to-Release Mapping

Use lanes for planning/execution; use versions for shipment.

- Merge lane PRs continuously.
- Cut releases on cadence (time-based) or milestone bundles (scope-based).
- Typical release bundle target: 2-5 merged lane slices plus stabilization validation.

## Recommended Release Cadence

- Default: weekly or biweekly release tags while active development continues.
- Allow out-of-band hotfix patch tags for critical regressions.

## Tagging and Artifact Policy

- Tags: `vX.Y.Z` (for example `v0.3.0`).
- Release workflow builds binaries per configured platform matrix.
- Each tag should include concise release notes:
  - lane slices included
  - key fixes/features
  - known limitations

## Servoshell Retirement Guidance

When servoshell inheritance retirement reaches declared done gate:

- Prefer a **minor bump** (`0.y.z` -> `0.(y+1).0`) to mark architectural milestone.
- Include migration-focused release notes (what changed, what remains transitional).

## Operational Checklist (per release)

1. Confirm merged scope and stabilization status.
2. Select bump type (patch/minor) using rules above.
3. Update `Cargo.toml` version.
4. Create and push tag `vX.Y.Z`.
5. Validate generated release artifacts.
6. Publish release notes with lane mapping.
