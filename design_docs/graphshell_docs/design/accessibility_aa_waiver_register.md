# Accessibility AA Waiver Register

**Date**: 2026-03-06  
**Status**: Active (v0.0.2 governance artifact)  
**Owner lane**: `lane:accessibility` (`#95`)

## Purpose

Track any remaining WCAG 2.2 AA gaps that are not fully remediated by the v0.0.2 release gate.

Policy alignment:
- WCAG 2.2 AA remains the normative target.
- v0.0.2 release gate requires Level A pass coverage across all 7 canonical surfaces.
- Any remaining AA gaps must be explicitly recorded here with owner, rationale, and deadline.

## Canonical Surfaces

- Graph Pane
- Node Pane
- Tool Pane
- Floating Windows
- Dialogs
- Omnibar
- Workbar

## Waiver Entries

| Waiver ID | Criterion | Level | Surface(s) | Current State | User Impact | Rationale | Owner | Target Fix Date | Issue Link | Exit Criteria | Status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| AAW-001 | 1.4.3 Contrast (Minimum) | AA | Floating Windows (radial disabled text) | Remediated in runtime; awaiting full multi-surface verification sweep | Reduced readability if regression reappears | Keep tracked until full checklist sweep closes AG0 evidence | `lane:accessibility` | 2026-03-15 | `#301` | Checklist row verified with pass evidence and regression remains green | Open |

## Entry Rules

1. Every open AA gap must have one row in this table.
2. `Owner`, `Target Fix Date`, and `Exit Criteria` are required fields.
3. `Status` values: `Open`, `Accepted Risk`, `In Progress`, `Closed`.
4. Rows can move to `Closed` only when linked evidence is merged and checklist parity is updated.

## Review Cadence

- Review on every AG0 status update.
- Review before any v0.0.2 release-candidate tag.
- Remove the file only after all rows are `Closed` and AA parity is demonstrated across canonical surfaces.
