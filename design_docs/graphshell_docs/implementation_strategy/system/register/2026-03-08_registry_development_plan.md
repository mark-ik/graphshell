<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

# Registry Development Plan — Master Index

**Doc role:** Master index and sequencing guide for registry sector development plans
**Status:** Active / canonical
**Date:** 2026-03-08
**Authority:** Subordinate to `SYSTEM_REGISTER.md`; coordinates across all registry spec files

---

## Purpose

This document organises the full registry surface into implementation sectors, describes the current implementation state of each registry, and links out to sector-specific development plans that contain the concrete phase-by-phase implementation work.

The goal is a fully operable Register layer in which every registry is:
- Explicitly structured with a real Rust type and stable public API
- Wired into the `RegistryRuntime` composition root
- Covered by diagnostics channels with appropriate severity
- Tested at the contract boundary via unit and scenario tests
- Connected to other registries only through the `SignalRoutingLayer` or `ControlPanel`, never via direct inter-registry calls

---

## Registry Inventory

All registries in the system spec family are listed here. Implementation state is assessed against:
- **Struct**: Does a real `XxxRegistry` struct exist?
- **API**: Are the canonical interfaces from the spec file implemented?
- **Wired**: Is it in `RegistryRuntime` and called through the runtime dispatch layer?
- **Tested**: Are there contract-level unit tests?
- **Diag**: Are diagnostics channels registered and emitting?

| Registry | Kind | Struct | API | Wired | Tested | Diag | Sector |
|---|---|---|---|---|---|---|---|
| `ProtocolRegistry` | Atomic | ✅ | ⚠️ partial | ✅ | ✅ | ✅ | [A](#sector-a) |
| `ViewerRegistry` | Atomic | ✅ (atomic) | ⚠️ partial | ✅ | ⚠️ | ✅ | [A](#sector-a) |
| `ViewerSurfaceRegistry` | Surface | ❌ | ❌ | ❌ | ❌ | ❌ | [A](#sector-a) |
| `LensRegistry` | Cross-domain | ⚠️ stub | ❌ | ⚠️ | ❌ | ❌ | [A](#sector-a) |
| `InputRegistry` | Domain | ✅ | ⚠️ partial | ✅ | ✅ | ✅ | [B](#sector-b) |
| `ActionRegistry` | Atomic | ✅ | ⚠️ partial | ✅ | ✅ | ✅ | [B](#sector-b) |
| `RendererRegistry` | Atomic (new) | ❌ | ❌ | ❌ | ❌ | ❌ | [B](#sector-b) |
| `IdentityRegistry` | Atomic | ✅ | ⚠️ stub crypto | ✅ | ✅ | ✅ | [C](#sector-c) |
| `NostrCoreRegistry` | Native mod | ✅ | ✅ | ✅ | ✅ | ✅ | [C](#sector-c) |
| `CanvasRegistry` | Surface | ❌ | ❌ | ❌ | ❌ | ❌ | [D](#sector-d) |
| `LayoutRegistry` | Atomic | ❌ | ❌ | ❌ | ❌ | ❌ | [D](#sector-d) |
| `PhysicsProfileRegistry` | Atomic | ❌ | ❌ | ❌ | ❌ | ❌ | [D](#sector-d) |
| `LayoutDomainRegistry` | Domain | ❌ | ❌ | ❌ | ❌ | ❌ | [D](#sector-d) |
| `PresentationDomainRegistry` | Domain | ❌ | ❌ | ❌ | ❌ | ❌ | [D](#sector-d) |
| `WorkbenchSurfaceRegistry` | Surface | ❌ | ❌ | ❌ | ❌ | ❌ | [E](#sector-e) |
| `WorkflowRegistry` | Domain | ❌ | ❌ | ❌ | ❌ | ❌ | [E](#sector-e) |
| `KnowledgeRegistry` | Atomic | ⚠️ shim | ⚠️ reconcile-only | ⚠️ | ✅ | ❌ | [F](#sector-f) |
| `IndexRegistry` | Atomic | ❌ | ❌ | ❌ | ❌ | ❌ | [F](#sector-f) |
| `DiagnosticsRegistry` | Atomic | ✅ (atomic) | ⚠️ | ✅ | ⚠️ | — | [F](#sector-f) |
| `ModRegistry` | Atomic | ✅ (atomic) | ✅ | ✅ | ✅ | ✅ | [G](#sector-g) |
| `AgentRegistry` | Atomic | ❌ | ❌ | ❌ | ❌ | ❌ | [G](#sector-g) |
| `ThemeRegistry` | Atomic | ❌ | ❌ | ❌ | ❌ | ❌ | [G](#sector-g) |
| `SignalRoutingLayer` → `SignalBus` | Infrastructure | ✅ skeleton | ⚠️ narrow topics | ✅ | ✅ | ✅ | [H](#sector-h) |

---

## Sector Map

The registries are grouped into eight development sectors. Each sector is a self-contained unit of work with its own implementation plan.

```
Sector A  — Content Pipeline         Protocol → ViewerSurface → Viewer → Lens
Sector B  — Input & Dispatch         Input → Action → Renderer
Sector C  — Identity & Verse         Identity → NostrCore
Sector D  — Canvas Surface           Canvas → Layout → Physics → LayoutDomain → PresentationDomain
Sector E  — Workbench Surface        WorkbenchSurface → Workflow
Sector F  — Knowledge & Index        Knowledge → Index → Diagnostics
Sector G  — Mod & Agent Runtime      Mod → Agent → Theme
Sector H  — Signal Infrastructure    SignalRoutingLayer → SignalBus
```

### Dependency ordering

Sectors are not fully independent. The following constraints govern sequencing:

```
H (signal infrastructure) must stabilise before D, E cross-registry signals are live.
F (diagnostics) must be complete before cross-sector test harness work.
B (RendererRegistry) must be complete before servoshell debtclear Phase 1 is done.
A (content pipeline) depends on B (ViewerRegistry selection depends on RendererRegistry attachment).
C can proceed in parallel with all other sectors.
G (AgentRegistry) depends on H for supervised intent ingress.
```

---

## Sector Plans

### Sector A — Content Pipeline {#sector-a}

**Registries:** `ProtocolRegistry`, `ViewerRegistry`, `ViewerSurfaceRegistry`, `LensRegistry`
**Plan:** [2026-03-08_sector_a_content_pipeline_plan.md](2026-03-08_sector_a_content_pipeline_plan.md)

The content pipeline is the chain that takes a URI and produces a rendered surface:

```
URI → ProtocolRegistry (scheme → MIME) → ViewerRegistry (MIME → ViewerId)
    → ViewerSurfaceRegistry (ViewerId → viewport policy)
    → LensRegistry (MIME + graph context → LensProfile)
```

This is the core rendering contract. Every node that displays content depends on it.

Current state: `ProtocolRegistry` has a good scaffold but lacks async MIME probing and mod-provided scheme handlers. `ViewerRegistry` exists in the atomic layer but lacks the domain surface wiring. `ViewerSurfaceRegistry` does not exist as a struct. `LensRegistry` is a one-line stub.

---

### Sector B — Input & Dispatch {#sector-b}

**Registries:** `InputRegistry`, `ActionRegistry`, `RendererRegistry`
**Plan:** [2026-03-08_sector_b_input_dispatch_plan.md](2026-03-08_sector_b_input_dispatch_plan.md)

Input → Action is the dispatch chain for all user interaction. `RendererRegistry` is a required new registry from the servoshell debtclear plan that enforces the renderer lifecycle boundary.

```
InputEvent → InputRegistry (binding → ActionId)
           → ActionRegistry (ActionId → Vec<GraphIntent> | WorkbenchIntent)
           → reducer / workbench authority

NodeKey + PaneId → RendererRegistry (attachment map)
                 → reconcile_webview_lifecycle (creation only after registry accepts)
```

Current state: Both `InputRegistry` and `ActionRegistry` have functional cores but are incomplete (no gamepad bindings, no cross-context resolution, no namespace enforcement, no capability guards). `RendererRegistry` does not exist and is the most urgent new registry.

---

### Sector C — Identity & Verse {#sector-c}

**Registries:** `IdentityRegistry`, `NostrCoreRegistry`
**Plan:** [2026-03-08_sector_c_identity_verse_plan.md](2026-03-08_sector_c_identity_verse_plan.md)

Identity and Verse are co-dependent: Nostr event signing draws on the identity keypair, device sync requires trusted peer identity, and NIP-46 delegation bridges the two registries.

Current state: `IdentityRegistry` is functional but uses SHA256 stub signing (no real ed25519 keypair). `NostrCoreRegistry` is the most complete registry in the system but lacks a real relay backend and NIP-46 signer implementation. Keypair ownership is duplicated across both registries.

---

### Sector D — Canvas Surface {#sector-d}

**Registries:** `CanvasRegistry`, `LayoutRegistry`, `PhysicsProfileRegistry`, `LayoutDomainRegistry`, `PresentationDomainRegistry`
**Plan:** [2026-03-08_sector_d_canvas_surface_plan.md](2026-03-08_sector_d_canvas_surface_plan.md)

The canvas surface registries govern everything about how the graph looks and moves. None of these registries have Rust implementations yet; the graph is driven by hardcoded constants in `render/mod.rs` and `graph_app.rs`.

These five registries are tightly coupled by the layout-first principle (layout resolves before presentation) and must be developed together.

---

### Sector E — Workbench Surface {#sector-e}

**Registries:** `WorkbenchSurfaceRegistry`, `WorkflowRegistry`
**Plan:** [2026-03-08_sector_e_workbench_surface_plan.md](2026-03-08_sector_e_workbench_surface_plan.md)

The workbench surface registries govern tile-tree layout policy and session modes. `WorkbenchSurfaceRegistry` owns the tile-tree policy authority that SYSTEM_REGISTER names as one of the two mutation authorities. `WorkflowRegistry` composes Lens × WorkbenchProfile into named session modes.

Both registries are not yet implemented. Their development enables the complete two-authority model to be enforced.

---

### Sector F — Knowledge & Index {#sector-f}

**Registries:** `KnowledgeRegistry`, `IndexRegistry`, `DiagnosticsRegistry`
**Plan:** [2026-03-08_sector_f_knowledge_index_plan.md](2026-03-08_sector_f_knowledge_index_plan.md)

The knowledge and index registries form the semantic layer. `KnowledgeRegistry` currently exists as a reconcile shim. `IndexRegistry` does not exist. `DiagnosticsRegistry` exists in the atomic layer but lacks the versioned payload schema contract from its spec.

Diagnostics must be advanced first in this sector as it is a prerequisite for cross-sector test harness confidence.

---

### Sector G — Mod & Agent Runtime {#sector-g}

**Registries:** `ModRegistry`, `AgentRegistry`, `ThemeRegistry`
**Plan:** [2026-03-08_sector_g_mod_agent_plan.md](2026-03-08_sector_g_mod_agent_plan.md)

`ModRegistry` is the most complete registry in the system (already wired, tested, discovery via `inventory`). The sector work is advancing the other two: `AgentRegistry` is the Register-owned version of supervised background capability, complementing the `ControlPanel`'s worker model. `ThemeRegistry` provides the visual token resolution that the presentation domain needs.

---

### Sector H — Signal Infrastructure {#sector-h}

**Registries:** `SignalRoutingLayer` → `SignalBus`
**Plan:** [2026-03-08_sector_h_signal_infrastructure_plan.md](2026-03-08_sector_h_signal_infrastructure_plan.md)

The signal routing layer exists as a functional skeleton (SR2/SR3 done gates met) but is narrow in scope. The full `SignalBus` abstraction with typed topics, async observers, dead-letter visibility, and backpressure policy is the SR3 → SR4 migration target.

---

## Completion Criteria for the Register Layer

The Register layer is considered complete when all of the following are true:

1. Every registry in the inventory table has ✅ across all columns.
2. The content pipeline chain (Sector A) is exercised end-to-end in a scenario test.
3. `RendererRegistry` enforces the creation boundary: no renderer created outside `reconcile_webview_lifecycle()`.
4. `InputRegistry` resolves gamepad bindings to actions with the same contract as keyboard bindings.
5. `IdentityRegistry` uses real ed25519 signing; `NostrCoreRegistry` draws keypair from it.
6. All canvas parameters (layout algorithm, physics profile, theme tokens) are resolved through their respective registries — no hardcoded constants in `render/mod.rs` or `graph_app.rs`.
7. `WorkbenchSurfaceRegistry` is the sole authority for tile-tree layout policy.
8. `DiagnosticsRegistry` has versioned channel schemas; all `DIAG_*` constants in `registries/mod.rs` are registered.
9. `AgentRegistry` supervises all background capabilities that are not `ControlPanel` workers.
10. `SignalBus` (or equivalent) replaces all remaining direct inter-registry wiring.

---

## Related Documents

- [SYSTEM_REGISTER.md](SYSTEM_REGISTER.md) — canonical Register hub and routing policy
- [../2026-02-22_registry_layer_plan.md](../2026-02-22_registry_layer_plan.md) — registry ecosystem and ownership model
- [../../../PLANNING_REGISTER.md](../../../PLANNING_REGISTER.md) — cross-subsystem execution register
- [../2026-03-08_servoshell_debtclear_plan.md](../2026-03-08_servoshell_debtclear_plan.md) — servoshell debt clearance (RendererRegistry dependency)
- [../2026-03-08_graph_app_decomposition_plan.md](../2026-03-08_graph_app_decomposition_plan.md) — graph_app decomposition (canvas/layout registry dependency)
