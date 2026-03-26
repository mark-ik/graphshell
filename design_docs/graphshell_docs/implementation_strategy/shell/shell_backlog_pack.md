<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Shell Backlog Pack

**Date**: 2026-03-25
**Status**: Planning / handoff pack
**Scope**: Dependency-ordered backlog for Shell as Graphshell's only host, app-level orchestration boundary, overview surface authority, and control-surface router.

**Related docs**:

- [SHELL.md](SHELL.md) — Shell domain spec and authority boundaries
- [shell_overview_surface_spec.md](shell_overview_surface_spec.md) — concrete Shell overview UI and routing model
- [../domain_interaction_acceptance_matrix.md](../domain_interaction_acceptance_matrix.md) — cross-domain review matrix
- [../../technical_architecture/domain_interaction_scenarios.md](../../technical_architecture/domain_interaction_scenarios.md) — canonical cross-domain scenario flows
- [../navigator/NAVIGATOR.md](../navigator/NAVIGATOR.md) — Navigator projection/navigation peer domain
- [../workbench/WORKBENCH.md](../workbench/WORKBENCH.md) — Workbench arrangement/activation peer domain
- [../graph/GRAPH.md](../graph/GRAPH.md) — Graph truth/analysis peer domain; canvas remains the rendered graph surface

## Tracker mapping

- Hub issue: #306 (`Hub: five-domain architecture adoption — Shell host, graphlet model, cross-domain scenarios`)
- Primary implementation issue: #303 (`Implement Shell host and overview surface adoption`)
- Review/evidence issue: #305 (`Operationalize cross-domain scenario IDs and acceptance evidence`)

---

## Wave 1

1. `SH01` Shell Host Boundary. Depends: none. Done gate: one canonical doc defines Shell as the application's only host and names what it does not own.
2. `SH02` Shell Command Routing Contract. Depends: `SH01`. Done gate: Shell command entry points are explicitly mapped to Graph, Navigator, Workbench, Viewer, or runtime/control destinations.
3. `SH03` Shell Overview Module Contract. Depends: `SH01`. Done gate: overview modules, summary sources, and routing rules are defined without flattening ownership.
4. `SH04` Shell Ambient Status / Attention Contract. Depends: `SH01`, `SH03`. Done gate: runtime warnings, trust state, and background task surfacing are distinct from domain truth and have explicit return-context rules.
5. `SH05` Shell Diagnostics / Routing Evidence Pack. Depends: `SH02`, `SH04`. Done gate: failed handoff, blocked route, and interruption-return paths emit diagnosable evidence.
6. `SH06` Shell Milestone Closure Receipt. Depends: `SH01`-`SH05`. Done gate: one closure doc states what Shell host behavior is canonical and what downstream lanes can safely assume.

---

## Scenario Track

- `SHS01` `DI03` Graphlet-to-Workbench handoff. Depends: `SH02`, `SH03`. Done gate: Shell can route `open in workbench` from graphlet context to Navigator + Workbench without creating arrangement truth itself.
- `SHS02` `DI05` Shell overview reorientation. Depends: `SH03`. Done gate: overview summary chips/cards route to the correct owning domain and preserve domain-specific ownership semantics.
- `SHS03` `DI06` Runtime/trust interruption return path. Depends: `SH04`, `SH05`. Done gate: interruption handling preserves graphlet/workbench return context and exposes diagnostic evidence.
