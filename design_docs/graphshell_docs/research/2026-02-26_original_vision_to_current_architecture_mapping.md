# Original Vision to Current Architecture Mapping (2026-02-26)

**Status**: Product/architecture north-star note (interpretive mapping)
**Source basis**: User-provided historical notes (`code.pdf`), especially pages 11-18 (node-based browser, tabs/history/bookmarks replacement, spatial memory, curation/build mode, Servo-based implementation ideas)
**Purpose**: Preserve durable intent from early product thinking and map it to current Graphshell architecture/planning so execution stays aligned with the original value proposition.

## Summary

The original notes were directionally correct on the hardest parts:

- the core UX problem (`tabs/history/bookmarks` are the wrong shape),
- the importance of **spatial memory** and **curation**,
- the need to capture **navigation transitions** as first-class data,
- and the need for **LOD / clustering / unloading** for scale.

What changed over time is mostly architectural maturity:

- Graphshell now has a much stronger runtime/control-plane model,
- the project has decoupled core browsing/graph UX from economic/network layers,
- and implementation planning is more realistic (shell/runtime first, not "modify Servo everywhere" first).

## Original Ideas That Still Matter (High Signal)

### 1. Replace tabs/history/bookmarks with structural views

**Original concept (pages 11-12)**:
- Represent browsing artifacts as structured visual systems, not rows/lists.
- Support multiple views/modes for different user intents.

**Current architecture alignment**:
- Multi-pane + multi-surface architecture (`Pane`, `Surface`, `Aspect`) now supports this framing directly.
- Workbench/tile system and viewer/tool pane split are the right substrate for mode-specific experiences.

**Why this still matters**:
- This is the core product thesis, not an implementation detail.
- It should remain the primary filter for feature prioritization.

### 2. Three-mode framing (History / Home / Build-share)

**Original concept (page 11)**:
- History view
- Home/curated web-of-nodes view
- Build mode for sharable/sellable thematic collections

**Current architecture alignment**:
- Maps naturally to distinct surfaces/workflows instead of one overloaded graph canvas.
- Fits current terminology refactor (`Aspect` / `Surface`) and future pane-hosted multi-view plans.

**Insight**:
- This framing can reduce product ambiguity. Many current roadmap items become easier to prioritize when assigned to one of these user modes.

### 3. Curation as a first-class action (not just navigation)

**Original concept (page 11)**:
- Users create visual collections/chunks of web nodes that are meaningful and shareable.

**Current architecture alignment**:
- Connects strongly to:
  - UDC / universal content model work,
  - semantic tagging,
  - export/share workflows,
  - workspace persistence/history,
  - Verse collaboration/intelligence plans.

**Insight**:
- Graphshell's differentiator is not just "graph browsing"; it is **curated sense-making**.
- This is a useful guardrail against over-indexing on browser parity work.

### 4. Capture transition source and navigation semantics

**Original concept (pages 13-14)**:
- Track source/target navigation transitions and transition types as explicit data.

**Current architecture alignment**:
- Strongly aligns with current lifecycle/history/storage/diagnostics emphasis.
- Matches the architectural move toward intent-driven mutation boundaries and explicit provenance.

**Insight**:
- This was one of the most durable technical intuitions in the early notes.
- It remains foundational for:
  - history integrity,
  - replay/traversal semantics,
  - graph edge meaning,
  - future trust/economic layers.

### 5. Performance/scale concerns: LOD, clustering, unloading, snapshots

**Original concept (pages 15-18)**:
- Rendering many nodes requires visibility-aware rendering, clustering, aggressive unloading, and reduced-detail representations.

**Current architecture alignment**:
- Maps directly to current planning around:
  - layout behavior/performance tuning,
  - visual tombstones,
  - diagnostics,
  - pane/view lifecycle/backpressure,
  - storage/history integrity (snapshot/recovery implications).

**Insight**:
- Early notes correctly identified that Graphshell is a systems problem, not only a UI problem.

## What Changed (And Why It Was the Right Change)

### 1. Core UX and economic/tokenization concepts are now more decoupled

**Original notes** often bind node-browser UX to tokenization/blockchain/IPFS concepts.

**Current trajectory** is stronger because it separates:
- core graph browsing + curation UX,
- local/runtime correctness,
- optional network/economic layers (Verse and beyond).

**Why this is better**:
- It preserves the vision while reducing implementation risk.
- It allows the core product to become usable before advanced trust/economic layers are complete.

### 2. Servo integration approach matured

**Original notes** imply deeper direct Servo modifications early.

**Current architecture** increasingly treats Servo as a backend under a Graphshell runtime/surface system, with explicit registries and contracts.

**Why this is better**:
- Lower risk
- Better testability
- Cleaner future support for alternate viewer backends (including planned `viewer:wry`)

## Product North-Star Implications for Current Planning

### A. Reintroduce "mode" framing in roadmap language

Current plans are technically strong but can read as subsystem-first. The original notes suggest a user-facing framing that could improve prioritization clarity:

- **History mode**
- **Home / curation mode**
- **Build / share mode**

This can coexist with lane planning and subsystem guides; it is a product lens, not a replacement.

### B. Keep "spatial memory" as the primary UX metric

When evaluating UI/stabilization work, ask:
- Does this improve the user's ability to remember, revisit, and reorganize information spatially?

This helps prevent local polish work from drifting into generic browser UI parity that does not strengthen the core experience.

### C. Treat graph edges/history transitions as product-semantic data, not incidental telemetry

The original transition-capture idea supports current history/storage/security/diagnostics work. This should remain a first-class model concern in architecture docs and schema decisions.

## Current Gaps Relative to the Original Vision (Useful, Not Criticism)

1. **Mode-specific surfaces are not yet product-prominent**
- The architecture can support them, but the product framing is still mostly implicit.

2. **Curation/build workflows are underrepresented in active near-term lanes**
- There is strong infrastructure work, but fewer visible user-facing "curate/share a graph" slices.

3. **Spatial-memory UX principles are present in research, but not always explicit in done-gates**
- Could be made more concrete in UX/stabilization acceptance criteria.

4. **Render/compositor backend constraints are currently shaping UX affordances**
- This is normal for the stage, but it reinforces the need to keep the north-star visible while solving embedder debt.

## Practical Use of This Note

Use this note as a quick check during planning:

- Does a lane improve Graphshell as a **spatial sense-making browser**, or only make it more browser-like?
- Which original user mode does a proposed feature primarily serve (History / Home / Build-share)?
- Are we preserving the distinction between foundational data capture (transitions/history/edges) and optional economic/network layers?

## Related Current Docs

- `design_docs/graphshell_docs/implementation_strategy/PLANNING_REGISTER.md`
- `design_docs/graphshell_docs/implementation_strategy/SYSTEM_REGISTER.md`
- `design_docs/graphshell_docs/implementation_strategy/2026-02-24_universal_content_model_plan.md`
- `design_docs/graphshell_docs/implementation_strategy/2026-02-22_multi_graph_pane_plan.md`
- `design_docs/graphshell_docs/implementation_strategy/2026-02-24_control_ui_ux_plan.md`
- `design_docs/graphshell_docs/implementation_strategy/2026-02-24_performance_tuning_plan.md`
- `design_docs/graphshell_docs/research/2026-02-18_graph_ux_research_report.md`

