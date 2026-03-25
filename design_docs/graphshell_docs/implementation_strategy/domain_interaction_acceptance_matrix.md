# Domain Interaction Acceptance Matrix

**Date**: 2026-03-25
**Status**: Active / review aid
**Scope**: Compact acceptance matrix for reviewing implementation work against the canonical cross-domain scenarios.

**Related**:

- `../technical_architecture/domain_interaction_scenarios.md` — canonical scenario flows and ownership model
- `../technical_architecture/unified_view_model.md` — five-domain model
- `shell/shell_backlog_pack.md` — Shell scenario-track backlog IDs
- `navigator/navigator_backlog_pack.md` — Navigator scenario-track backlog IDs
- `workbench/workbench_backlog_pack.md` — Workbench scenario-track backlog IDs

---

## 1. Purpose

This matrix turns the cross-domain scenarios into a lightweight review artifact.

Use it when a PR or implementation slice touches behavior that spans more than one domain.

The goal is simple:

1. verify the user-visible flow still makes sense,
2. verify ownership has not been flattened,
3. verify at least one appropriate evidence path exists.

---

## 2. Review Rule

When a change materially affects one of the scenario IDs below, the review should state:

- which `DIxx` scenario(s) were touched,
- whether ownership remained correct,
- what evidence exists.

Minimum acceptable evidence is one of:

- scenario or integration test,
- focused contract/unit test plus doc parity update,
- diagnostics receipt for degraded/interruption paths,
- explicit manual validation note when automation is not yet available.

---

## 3. Acceptance Matrix

| Scenario ID | Short name | Core acceptance question | Ownership that must remain true | Minimum evidence | Backlog hooks |
|---|---|---|---|---|---|
| `DI01` | Graph-first local exploration | Does node selection lead to a derived local graphlet and visible frontier workflow without Workbench becoming implicit? | Graph owns selection truth; Navigator owns graphlet derivation; Shell only summarizes | graph/navigation scenario test or focused contract test + doc parity | `NVS01` |
| `DI02` | Corridor transition | Can the user move from selected anchors to a corridor/path view without Navigator pretending to own graph truth? | Graph owns selected anchors and path rendering; Navigator owns corridor derivation | graph + Navigator scenario or contract evidence | `NVS02` |
| `DI03` | Linked arrangement handoff | Can an active graphlet be opened into Workbench as a linked arrangement without Workbench owning graphlet truth? | Navigator owns graphlet identity; Workbench owns arrangement; Shell routes only | workbench routing scenario or linked-binding test | `SHS01`, `WBS01` |
| `DI04` | Viewer fallback in session | Does viewer fallback preserve workbench context and expose degraded state honestly? | Viewer owns fallback reason; Workbench owns context; Shell surfaces attention | fallback scenario, diagnostics receipt, or focused viewer/workbench test | `WBS02` |
| `DI05` | Shell overview reorientation | Can the user reorient from Shell overview and be routed to the correct owning domain? | Shell composes and routes; Graph/Navigator/Workbench/Viewer remain owners of facts | overview routing scenario or focused handoff test | `SHS02`, `NVS03`, `WBS03` |
| `DI06` | Runtime / trust interruption | Can an interruption be handled without losing graphlet/workbench return context? | Shell owns interruption surfacing and return routing; underlying domain truth remains unchanged | interruption scenario, focus-return test, or diagnostics evidence | `SHS03` |

---

## 4. PR Review Checklist

Use this short checklist when relevant:

1. Name the touched `DIxx` scenario IDs.
2. Confirm which domain owns each changed state transition.
3. Confirm Shell did not absorb domain truth that belongs elsewhere.
4. Confirm Graphlet identity did not silently become Workbench-owned.
5. Attach one evidence path per touched scenario.

---

## 5. Notes

- This matrix is intentionally small. It is a review aid, not a replacement for the domain specs.
- If a new cross-domain behavior does not fit `DI01` to `DI06`, add a new canonical scenario first, then extend this matrix and the relevant backlog packs.
