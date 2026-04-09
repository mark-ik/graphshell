<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Owner-Scoped Temporal Branching Follow-On

**Date**: 2026-04-09
**Status**: Implementation strategy / Track B follow-on
**Scope**: Extend the unified history architecture with owner-scoped temporal
branching so Graphshell can keep alternative local navigation and working
timelines without collapsing them into one global linear history.

**Related**:

- [2026-03-08_unified_history_architecture_plan.md](2026-03-08_unified_history_architecture_plan.md)
- [../system/2026-03-20_object_model_and_navigation_integration_strategy.md](../system/2026-03-20_object_model_and_navigation_integration_strategy.md)
- [../navigator/NAVIGATOR.md](../navigator/NAVIGATOR.md)
- [../../technical_architecture/2026-04-09_graph_object_classification_model.md](../../technical_architecture/2026-04-09_graph_object_classification_model.md)

---

## 1. Why This Exists

Graphshell needs richer history than a single back/forward stack, but it also
does not want one giant global history tree that ignores local ownership.

The practical need is closer to:

- this frame explored one branch,
- that tile kept another branch alive,
- this graph view forked into a different working line,
- a shared session may expose one active branch while a local user keeps a
  private alternate branch.

This note defines that model as **owner-scoped temporal branching**.

---

## 2. Core Rule

Temporal branching belongs to an owner scope, not to the universe as a whole.

Possible owners include:

- a frame,
- a tile or pane,
- a graph view,
- a navigator context,
- a co-op room or shared session when explicitly synchronized,
- a workspace-global surface only where that is the intended owner.

This avoids both extremes:

- purely global history,
- completely isolated history with no way to reason about branching.

---

## 3. What Branches

The first branching model should apply to navigation and working-context state,
not to every mutation in the system.

Suitable early branchable state includes:

- active item or node,
- active graphlet or projection scope,
- open constellation or route,
- selection/focus context,
- browsing path within a bounded owner surface.

Undo/redo for edits remains a separate concern even if later related.

---

## 4. Branch Events

A new branch is created when an owner:

- navigates away from a past point while preserving the older future,
- opens an alternate cluster or route from a prior state,
- explicitly duplicates or forks a working line,
- accepts a synchronized shared branch while preserving a local private line.

Branch creation should be explicit in the data model even if the initial UI is
simple.

---

## 5. Active-Branch Semantics

Each owner has:

- one active branch,
- one current head within that branch,
- optional inactive sibling branches.

Important rule:

- one owner's active branch selection must not silently rewrite another owner's
  active branch unless those owners are intentionally coupled.

---

## 6. Relationship to Shared Sessions

Shared sessions may expose a synchronized branch for common navigation, but
local owners should still be able to keep private alternates when policy allows
it.

This matters for:

- collaborative browsing,
- group wayfinding,
- review sessions,
- classroom or guided-tour modes.

Browser-envelope or degraded hosts may expose only a reduced subset of this
model, but the model itself should remain stable.

---

## 7. Minimal First Slice

Recommended first slice:

1. owner-scoped history IDs,
2. branch creation on alternate forward navigation,
3. active-branch tracking per owner,
4. branch metadata with timestamps and labels,
5. UI to inspect and switch local branches for a given owner.

This is enough to validate the model without overbuilding a full history-tree
editor immediately.

---

## 8. Reason for Track B Placement

This belongs in Track B because it closes an architectural seam before broader
feature expansion.

Without owner-scoped branching, future co-op, Navigator specialties, and richer
browser surfaces will keep inventing incompatible local history rules.