# Roadmap Lane `#19` Readiness Plan

**Date**: 2026-02-27  
**Status**: Active (docs-only execution lane)  
**Lane**: `lane:roadmap`  
**Tracker focus**: `#19` (`2D↔3D` hotswitch)

---

## 1. Purpose

This plan defines a merge-safe roadmap lane that can proceed while stabilization and viewer/compositor work continue in parallel.

The lane is intentionally **docs/planning only** and avoids runtime hotspots:

- `app.rs`
- `render/mod.rs`
- `shell/desktop/ui/gui.rs`
- compositor implementation files under `shell/desktop/workbench/*`

---

## 2. Current Roadmap State

`#19` remains deferred because core prerequisites are not fully closed.

The blocker is not concept definition; the blocker is execution-order risk across active architecture lanes.

### 2.1 Prerequisites (for `#19`)

`#19` remains blocked until all four prerequisites below are closed.

1. **Stabilization closure on camera/input/focus**
  - Why it blocks: hotswitch semantics are not verifiable while core interaction is unstable.
  - Owner lane: `lane:stabilization` (`#88`).

2. **Surface composition pass contract + overlay policy closure**
  - Why it blocks: 3D mode switching must not regress per-mode overlay/focus visibility behavior.
  - Owner lanes: `lane:stabilization` (`#88`), `lane:spec-code-parity` (`#99`).

3. **Runtime-authoritative tile render mode behavior**
  - Why it blocks: viewer/render path state must remain deterministic during mode transitions.
  - Owner lane: `lane:viewer-platform` (`#92`).

4. **Persistence + degradation guarantees for dimension state**
  - Why it blocks: snapshot restore must degrade safely when 3D support is unavailable.
  - Owner lane: `lane:roadmap` (spec), then implementation lanes.

---

## 3. Docs-Only Work Queue (Parallel-Safe)

These slices are explicitly chosen because they do not require code changes in active hotspot files.

### R1 — `#19` readiness checklist

**Output**
- Add a readiness checklist to planning docs with one item per prerequisite.
- Track each item as `open / partial / closed`.
- Require explicit closure evidence links before moving `#19` out of blocked state.

**Done gate**
- `#19` has a visible prerequisite checklist with owners, evidence links, and closure criteria.

### R2 — `ViewDimension` acceptance contract

**Output**
- Define acceptance criteria for 2D↔3D parity as a single contract, including:
  - interaction continuity (pan/zoom/focus)
  - selection continuity
  - no silent state corruption on toggle
  - deterministic fallback to `TwoD` when 3D is unavailable

**Done gate**
- Contract is documented in strategy docs and referenced by the future implementation issue stack.

### R3 — persistence + degradation policy alignment

**Output**
- Align wording across planning docs and canonical terminology on:
  - persisted dimension state ownership
  - fallback behavior when 3D cannot render
  - what is ephemeral vs persisted (`z` positions remain derived/ephemeral)

**Done gate**
- No contradictory statements remain between roadmap-facing and terminology docs for dimension fallback behavior.

### R4 — issue stack seed for implementation phase

**Output**
- Prepare issue-ready child slices under `#19` (no code):
  1. state transition contract + reducer checkpoints
  2. render pipeline integration checks
  3. persistence/degradation tests
  4. UX/shortcut and user feedback polish

**Done gate**
- Child issue templates exist with scope, hotspots, and acceptance gates.

---

## 4. Merge-Safe Execution Rules

- Keep roadmap lane changes confined to `design_docs/**`.
- Do not bundle runtime refactors with roadmap docs updates.
- Prefer small doc PRs with one closure target each (`R1`..`R4`).
- If any item requires touching runtime hotspots, spin it out to the owning non-roadmap lane.

---

## 5. Exit Criteria for Roadmap Blocked State

`#19` can move from **blocked** to **implementation-ready** only when:

1. Stabilization evidence closes interaction regressions required to validate hotswitch behavior.
2. Viewer/compositor pass and render-mode contracts are no longer changing in incompatible ways.
3. The acceptance contract and persistence/degradation rules are canonical and non-contradictory.
4. Child implementation issues are created and sequenced to avoid hotspot collisions.

Until then, roadmap work proceeds as planning and issue-shaping only.
