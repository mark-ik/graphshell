# All-Docs Context Bootstrap (AI Session Primer)

**Date**: 2026-02-27  
**Status**: Active reference brief  
**Purpose**: If you need to behave as though you have already read the full documentation corpus, this is the shortest high-signal context to load first.

---

## 1) Canonical Authority Order

When docs conflict, use this precedence:

1. `design_docs/DOC_README.md` (canonical index + working principles)
2. `design_docs/DOC_POLICY.md` (governance rules)
3. `design_docs/PROJECT_DESCRIPTION.md` (maintainer-owned product vision)
4. `design_docs/TERMINOLOGY.md` (canonical vocabulary)
5. Active Graphshell and Verse docs under `design_docs/graphshell_docs/` and `design_docs/verse_docs/`
6. `design_docs/archive_docs/` (historical only; not authoritative)

Core implication: never let archive wording override active strategy/register docs.

---

## 2) Product Identity and Framing

- Graphshell is a local-first spatial graph browser.
- Workbench is global per graph dataset (`GraphId`); frames are local pane-layout containers inside a workbench.
- Verse is optional P2P/network capability, not required for core offline value.
- UX baseline reliability is currently higher priority than AI product expansion.

---

## 3) Non-Negotiable Terminology and Semantics

Use canonical terms exactly from `TERMINOLOGY.md`:

- Tile Tree primitives: Tile, Pane, Container (Tab Group / Split / Grid), Shares.
- Runtime/UI container term: Frame.
- Persistence term: Frame Snapshot.
- Render authority term: TileRenderMode (`CompositedTexture`, `NativeOverlay`, `EmbeddedEgui`, `Placeholder`).
- Cross-cutting architecture: Domain, Aspect, Surface, Subsystem.
- Register model: Atomic registries + Domain registries, supervised by Register runtime.

Avoid deprecated names (for example legacy ontology naming and old history panel naming).

---

## 4) Current Strategic Priority (Gate Before AI)

The active gate is the UX baseline in:

- `design_docs/graphshell_docs/implementation_strategy/2026-02-27_ux_baseline_done_definition.md`

AI-facing work should not move above maintenance priority until this gate is satisfied.

Baseline expectations, in plain terms:

- First activation should render real content (no blank-first-frame race).
- Focus handoff and split/tab behaviors must be deterministic.
- Viewer behavior must match render-mode policy.
- Lifecycle transitions (Active/Warm/Cold) must stay synchronized with pane/viewer mapping.
- Degradation/fallback must be explicit and observable.
- Docs/spec must not claim runtime behavior that is not actually implemented.

---

## 5) Execution Control Plane (What to Read for “What Next?”)

Primary execution register:

- `design_docs/graphshell_docs/implementation_strategy/PLANNING_REGISTER.md`

Current high-importance lanes:

- `lane:stabilization` (#88)
- `lane:embedder-debt` (#90)
- `lane:viewer-platform` (#92)
- `lane:spec-code-parity` (#99)

Operational meaning:

- Stabilization and viewer/runtime truth work are the immediate path to baseline close.
- Avoid adding new feature surface area that bypasses these lane outcomes.

---

## 6) Composited Viewer Contract (Critical Rendering Invariant)

Canonical contract:

- `design_docs/graphshell_docs/implementation_strategy/2026-02-26_composited_viewer_pass_contract.md`

Key invariant:

- Composition must be an explicit pass model, not incidental UI layer ordering.

Pass order:

1. UI layout/chrome pass
2. Content pass (backend render callback path)
3. Overlay affordance pass (must come after content for composited mode)

Policy nuance:

- `CompositedTexture`: overlays can render over content.
- `NativeOverlay`: overlays cannot occlude native web pixels; use chrome/gutter affordances.

---

## 7) Register Runtime and Routing Discipline

Canonical runtime hub:

- `design_docs/graphshell_docs/implementation_strategy/SYSTEM_REGISTER.md`

Do not blur these authority boundaries:

- Graph reducer authority: deterministic model mutations.
- Workbench authority: tile-tree layout/pane mutations.
- Signal path: decoupled cross-registry notifications.
- Direct calls: only inside the same ownership boundary.

Anti-pattern to avoid:

- Silent no-op routing failures for authority-mismatched intents.

---

## 8) Dependency and License Policy (Practical)

Dependency strategy (current):

- Existing crates first.
- Add/replace crates only when reliability, complexity, or measurable outcomes improve.
- Treat deep transitive dedupe as opportunistic unless metrics show real pain.

License posture:

- MPL-2.0 compatibility is enforced with CI license gate + policy script.
- Unknown license entries require explicit allowlist handling and review.

---

## 9) Documentation Governance Short Rules

- Prefer updating existing active docs over spawning redundant new docs.
- Keep `DOC_README.md` aligned whenever active docs are added/moved/removed.
- Do not edit `PROJECT_DESCRIPTION.md` unless explicitly requested by maintainer.
- Treat archive docs as historical context, not present-tense authority.

---

## 10) Archive Interpretation (What It Is Good For)

Archive checkpoints are useful for:

- decision history,
- rationale archaeology,
- deferred ideas.

Most archive planning docs are superseded by active strategy/register docs.

Notable historically unique detail still worth referencing when needed:

- `design_docs/archive_docs/checkpoint_2026-02-20/2026-02-19_ios_port_plan.md` (deferred iOS blueprint detail not mirrored 1:1 in active docs).

---

## 11) If You Only Remember 12 Things

1. UX baseline closure comes before AI prioritization.
2. `PLANNING_REGISTER.md` is the execution source of truth.
3. Use canonical terms from `TERMINOLOGY.md` exactly.
4. Workbench is global per graph; frames are local containers.
5. Enforce explicit composition passes for composited rendering.
6. Overlay behavior depends on TileRenderMode.
7. Graph mutations and tile-tree mutations have different authorities.
8. Signal routing is for decoupled cross-registry coordination.
9. Existing-first dependencies; selective replacement is allowed with clear ROI.
10. License policy is automated and must stay green in CI.
11. Keep docs/spec claims aligned with actual runtime behavior.
12. Archive informs history, not current truth.

---

## 12) Suggested Session Boot Sequence

For a fresh implementation session:

1. Read `DOC_README.md` and `TERMINOLOGY.md`.
2. Read `PLANNING_REGISTER.md` section 1A/1C and active lane items.
3. Read composited pass contract and current UX baseline gate doc.
4. Confirm current code reality before editing docs/spec claims.
5. Execute only slices that tighten baseline reliability or parity.
