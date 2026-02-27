# UX Baseline Done Definition (Pre-AI Priority)

**Date**: 2026-02-27  
**Status**: Implementation-guiding  
**Priority**: Immediate (before AI-facing feature expansion)

**Related**:
- `PLANNING_REGISTER.md` (canonical lane sequencing and issue hubs)
- `2026-02-27_workbench_frame_tile_interaction_spec.md`
- `2026-02-26_composited_viewer_pass_contract.md`
- `2026-02-23_wry_integration_strategy.md`
- `2026-02-24_universal_content_model_plan.md`
- `../research/2026-02-27_viewer_state_matrix.md`
- `../technical_architecture/GRAPHSHELL_AS_BROWSER.md`

---

## 1) Purpose

Define a strict, testable UX baseline that must be complete before AI integration moves from optional scaffolding to product-priority work.

This document is intentionally execution-oriented: every baseline item has concrete pass criteria and lane ownership.

---

## 2) Scope and Non-Goals

### In scope

- Core navigation and pane interactions are deterministic and visible on first action.
- Viewer behavior is predictable across render modes (`CompositedTexture`, `NativeOverlay`, `EmbeddedEgui`, `Placeholder`).
- Declared viewer IDs are clearly classified as fully operational vs partially wired vs deferred.
- Diagnostics and tests prove behavior rather than relying on manual spot checks.

### Out of scope (until baseline complete)

- New AI-facing product interactions.
- AI-driven routing as a dependency for core browsing or pane workflows.
- Adding new viewer classes that bypass current baseline gaps.

---

## 3) UX Baseline Definition of Done

All items below must be true simultaneously.

### A. Interaction correctness

1. Opening a node into a pane renders usable content on first activation (no blank-first-frame race).
2. Split/merge/reflow operations preserve focus ownership and active-tile-selector visibility.
3. Closing a pane hands focus deterministically to the expected next pane/tile.
4. Keyboard and pointer navigation produce the same semantic outcomes for open/close/switch.

**Verification**
- `scripts/dev/smoke-matrix.ps1 quick` passes on Windows quick profile.
- Known focus-activation regression scenarios from stabilization register reproduce as fixed.

### B. Viewer baseline correctness

1. The stable baseline set is fully operational in node panes:
   - `viewer:webview` (canonical default),
   - `viewer:plaintext`,
   - `viewer:markdown`.
2. No legacy web-viewer aliases are required; `viewer:webview` is canonical.
3. Render-mode behavior is explicit and policy-conformant:
   - composited tiles: affordance overlays visible over content,
   - native overlay tiles: affordances rendered in chrome/gutter with documented limitations,
   - embedded/placeholder tiles: standard egui affordance path.
4. Every active node pane has runtime-authoritative render mode metadata.

**Verification**
- Render-mode dispatch tests pass for all active modes.
- Compositor pass ordering evidence exists in diagnostics output (content before overlay for composited path).

### C. Lifecycle and routing correctness

1. Active/Warm/Cold transitions do not desynchronize pane state and viewer mapping.
2. Opening from graph view to pane preserves node identity and expected viewer selection policy.
3. Declared viewer IDs with non-operational embedded paths are surfaced as partial/deferred status, not silent fallback ambiguity.
4. Settings routes are pane-authority: `graphshell://settings/{history,persistence,sync,physics}` resolve to tool-pane surfaces (not reducer-owned floating panels).

**Verification**
- Lifecycle reconcile tests cover promote/demote and re-open paths for baseline viewers.
- Viewer state matrix is updated with real status (`operational`, `partial`, `deferred`) and checked during release readiness.

### D. Performance and degradation baseline

1. Baseline interactions (open, switch tile, split, close) remain responsive under quick smoke workload.
2. Degradation transitions are explicit and observable (not silent failure):
   - fallback to placeholder path,
   - blocked/runtime error states surfaced in diagnostics.

**Verification**
- Quick smoke profile completes without regressions.
- Diagnostics channels emit reasoned degradation/fallback events.

### E. Spec/code parity baseline

1. Public viewer tables and strategy docs do not claim behavior that runtime cannot currently render.
2. Wry strategy references current runtime model (`TileRenderMode`) rather than legacy overlay tracking assumptions.

**Verification**
- Doc parity review done for viewer matrix + strategy docs before milestone close.

---

## 4) Lane and Issue Mapping (Execution Ownership)

### Primary lanes

- **`lane:stabilization` (`#88`)**: user-visible interaction regressions, focus/render activation correctness.
- **`lane:viewer-platform` (`#92`)**: render-mode authority, viewer routing/wiring correctness.
- **`lane:spec-code-parity` (`#99`)**: policy/spec alignment for affordances and viewer claims.
- **`lane:embedder-debt` (`#90`)**: GL-state and host/frame boundary hardening.

### UX baseline work package mapping

1. **Focus and first-frame activation reliability**  
   - Primary: `#88`  
   - Supporting: `#90`

2. **Render-mode authoritative pane dispatch**  
   - Primary: `#92`  
   - Supporting: `#99`

3. **Composited overlay affordance correctness**  
   - Primary: `#88`  
   - Supporting: `#99`

4. **GL-state contract hardening and regression tests**  
   - Primary: `#90`

5. **Viewer declaration vs runtime reality parity updates**  
   - Primary: `#99`  
   - Supporting: `#92`

### Existing issue alignment from planning register

- `#166` compositor replay traces (diagnostic depth; useful but not blocking for baseline close).
- `#167` differential composition (performance optimization; post-baseline unless needed for responsiveness gate).
- `#168` per-tile GPU budget/degradation diagnostics (important for baseline degradation observability).
- `#169` backend hot-swap contract (valuable, but can follow baseline if not required for core UX stability).

---

## 5) Dependency/Cargo Leverage Plan (Existing-First, Selective Replacement Allowed)

Use existing dependencies first, then add or replace only when it improves reliability, maintenance burden, or measurable performance.

### Decision policy

1. Prefer already-present crates and patterns in this repository.
2. If a new crate is proposed, require at least one of:
   - removes an existing custom/fragile implementation,
   - replaces multiple crates with one reliable crate,
   - closes a UX-baseline blocker with lower implementation risk.
3. Feature-gate heavyweight additions by default.
4. Treat transitive duplicate cleanup as lower priority unless it affects startup/build/runtime budgets materially.

### Testing and validation leverage

- `rstest` + `proptest`: table/matrix tests for render-mode policy and lifecycle transitions.
- `tracing-test` + optional `tracing` feature: verify pass ordering and fallback/degradation event emission.
- `insta`: stable snapshot assertions for diagnostics summaries and viewer-state matrix outputs.

### UI/runtime leverage

- `egui_tiles`: continue to centralize split/tab/focus semantics rather than ad hoc pane routing.
- `egui_graphs`: preserve graph-pane behavior boundaries so viewer fixes do not regress graph controls.
- `egui-notify`: explicit user-visible fallback/degradation notices for non-fatal runtime transitions.

### Content routing leverage

- `infer` + `mime_guess`: enforce deterministic MIME/address routing for baseline viewer selection.
- `image`: preserve embedded image/thumbnail fallback path quality without adding dependencies.

### Existing-first replacement/addition guidance

- **Markdown path**: prefer direct `pulldown-cmark` integration first; only add `egui_commonmark` if it demonstrably reduces code and remains compatible with current egui version.
- **SVG path**: use `resvg` as the canonical SVG renderer, but align version with the currently-resolved dependency graph before bumping to avoid duplicate major/minor lines.
- **Syntax highlighting**: use `syntect` with curated language scope and cached sets (`OnceLock`) to control compile time and binary impact.
- **PDF path**: keep `pdfium-render` feature-gated (`pdf`) so baseline UX is not blocked by native PDFium packaging on all platforms.
- **Web fallback path**: keep `wry` feature-gated and pane-only; do not make baseline UX correctness depend on `wry` being enabled.

### Observability and operational leverage

- `sysinfo`: baseline memory-pressure context for degradation diagnostics.
- Existing smoke scripts (`scripts/dev/smoke-matrix.ps1`, `scripts/dev/smoke-matrix.sh`): keep these as baseline entry gates before broader test suites.

### Consolidation opportunities (from `cargo tree -d` audit)

The current graph shows duplicate transitive lines including `hyper` (`0.14` + `1.x`), `tungstenite` (`0.21` + `0.28`), ICU (`1.x` + `2.x`), `kurbo` (`0.11` + `0.12`), `netwatch` (`0.2` + `0.3`), and `iroh-metrics` (`0.30` + `0.31`).

Recommended priority:

1. **Do now (high ROI, low risk)**
   - Avoid introducing new overlapping crates in app-owned code paths (especially HTTP/WebSocket/markdown/render helpers).
   - Keep one primary app-level HTTP client pattern (`reqwest` + `tokio`) in Graphshell-owned modules.

2. **Do soon (medium ROI)**
   - When adding direct dependencies (`resvg`, markdown/render helpers), pick versions that minimize duplicate resolution against current Servo-driven transitive set.
   - Audit optional feature flags to ensure heavy crates are not enabled in default baseline paths.

3. **Later (lower ROI unless perf/build pain is measured)**
   - Attempt deep transitive unification only when blocked by measurable regressions (build time, binary size, startup, memory), since much of this is inherited from Servo ecosystem version boundaries.

---

## 6) Milestone Exit Criteria (Pre-AI Gate)

UX baseline is considered complete only when:

1. Sections 3A–3E pass together in one checkpoint.
2. Viewer state matrix reflects current runtime truth with no known “declared as done, rendered as partial” mismatches for baseline viewers.
3. Stabilization and viewer-platform blockers affecting first-use workflows are closed or explicitly waived with documented constraints.

Only after this gate should AI-facing roadmap items move above maintenance priority.

---

## 7) Immediate Next Slices (Suggested 3-PR sequence)

1. **PR 1 — Interaction and affordance reliability**
   - Focus handoff + first-frame activation regressions
   - Composited overlay visibility checks

2. **PR 2 — Render-mode and viewer truth alignment**
   - Runtime-authoritative mode checks in pane paths
   - Viewer-state matrix/doc parity update

3. **PR 3 — Diagnostics and baseline test hardening**
   - Pass-order and degradation assertions
   - Snapshot-backed diagnostics output checks

These slices keep baseline closure small, parallel-safe, and measurable.
