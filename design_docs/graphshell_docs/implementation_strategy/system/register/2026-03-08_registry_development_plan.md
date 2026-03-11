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

Archive note:
- This master plan is not ready to archive. `RendererRegistry` (Sector B), Sector C's remaining
  real `UserIdentity` closure (`secp256k1` / NIP-46 after the new `UserIdentity`/`NodeId`
  split), and the remaining Sector G WASM/mod-theme follow-ons are still open.

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
| `ProtocolRegistry` | Atomic | ✅ | ✅ | ✅ | ✅ | ✅ | [A](#sector-a) |
| `ViewerRegistry` | Atomic | ✅ | ✅ | ✅ | ✅ | ✅ | [A](#sector-a) |
| `ViewerSurfaceRegistry` | Surface | ✅ | ✅ | ✅ | ✅ | ✅ | [A](#sector-a) |
| `LensRegistry` | Cross-domain | ✅ | ✅ | ✅ | ✅ | ✅ | [A](#sector-a) |
| `InputRegistry` | Domain | ✅ | ⚠️ partial | ✅ | ✅ | ✅ | [B](#sector-b) |
| `ActionRegistry` | Atomic | ✅ | ⚠️ partial | ✅ | ✅ | ✅ | [B](#sector-b) |
| `RendererRegistry` | Atomic (new) | ❌ | ❌ | ❌ | ❌ | ❌ | [B](#sector-b) |
| `IdentityRegistry` | Atomic | ✅ | ⚠️ stub crypto | ✅ | ✅ | ✅ | [C](#sector-c) |
| `NostrCoreRegistry` | Native mod | ✅ | ✅ | ✅ | ✅ | ✅ | [C](#sector-c) |
| `CanvasRegistry` | Surface | ✅ | ✅ | ✅ | ✅ | ✅ | [D](#sector-d) |
| `LayoutRegistry` | Atomic | ✅ | ✅ | ✅ | ✅ | ✅ | [D](#sector-d) |
| `PhysicsProfileRegistry` | Atomic | ✅ | ✅ | ✅ | ✅ | ✅ | [D](#sector-d) |
| `LayoutDomainRegistry` | Domain | ✅ | ✅ | ✅ | ✅ | ✅ | [D](#sector-d) |
| `PresentationDomainRegistry` | Domain | ✅ | ✅ | ✅ | ✅ | ✅ | [D](#sector-d) |
| `WorkbenchSurfaceRegistry` | Surface | ✅ | ✅ | ✅ | ✅ | ✅ | [E](#sector-e) |
| `WorkflowRegistry` | Domain | ✅ | ✅ | ✅ | ✅ | ✅ | [E](#sector-e) |
| `KnowledgeRegistry` | Atomic | ✅ | ✅ | ✅ | ✅ | ✅ | [F](#sector-f) |
| `IndexRegistry` | Atomic | ✅ | ✅ | ✅ | ✅ | ✅ | [F](#sector-f) |
| `DiagnosticsRegistry` | Atomic | ✅ | ✅ | ✅ | ✅ | ✅ | [F](#sector-f) |
| `ModRegistry` | Atomic | ✅ (atomic) | ✅ | ✅ | ✅ | ✅ | [G](#sector-g) |
| `AgentRegistry` | Atomic | ✅ | ✅ | ✅ | ✅ | ✅ | [G](#sector-g) |
| `ThemeRegistry` | Atomic | ✅ | ✅ | ✅ | ✅ | ✅ | [G](#sector-g) |
| `SignalRoutingLayer` → `SignalBus` | Infrastructure | ✅ | ✅ | ✅ | ✅ | ✅ | [H](#sector-h) |

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
F (diagnostics) must be complete before cross-sector test harness work; debt-clear only
needs the narrow renderer-boundary diagnostics slices it introduces.
B1 (RendererRegistry) is folded into servoshell debtclear Phases 1-2 and must be complete
before debtclear Phase 1 is done; B2-B3 are not debtclear blockers.
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

Current state: Sector A is implemented. `ProtocolRegistry` now drives URI-aware MIME inference and
cancellable content-type probes, `ViewerRegistry` exposes capability descriptions and canonical
fallback behavior, the existing layout-domain `ViewerSurfaceRegistry` now resolves viewer-specific
surface profiles at runtime, and `LensRegistry` now supports content-aware + semantic-overlay
resolution/composition. Remaining work in this area is refinement, not missing authority
existence.

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

Current state: Both `InputRegistry` and `ActionRegistry` have functional cores but are incomplete (no gamepad bindings, no cross-context resolution, no namespace enforcement, no capability guards). `RendererRegistry` does not exist and is the most urgent new registry. Only that `RendererRegistry` slice is a servoshell debt-clear prerequisite.

---

### Sector C — Identity & Verse {#sector-c}

**Registries:** `IdentityRegistry`, `NostrCoreRegistry`
**Plan:** [2026-03-08_sector_c_identity_verse_plan.md](2026-03-08_sector_c_identity_verse_plan.md)

Identity and Verse are co-dependent, but they no longer share one cryptographic lane: transport
trust and `NodeId` stay in `IdentityRegistry`, while public/user signing remains the unfinished
`UserIdentity` lane on the Nostr side.

Current state: `IdentityRegistry` now owns real Ed25519 node signing, key persistence, Verse trust
state, and signed presence-binding assertions. `NostrCoreRegistry` now has a supervised websocket
relay backend, restart-safe subscription persistence, relay connection diagnostics, and a local
secp256k1 user-signing lane. The remaining work in Sector C is delegated NIP-46 signing without
collapsing that `UserIdentity` lane back into the transport `NodeId` key.

---

### Sector D — Canvas Surface {#sector-d}

**Registries:** `CanvasRegistry`, `LayoutRegistry`, `PhysicsProfileRegistry`, `LayoutDomainRegistry`, `PresentationDomainRegistry`
**Plan:** [2026-03-08_sector_d_canvas_surface_plan.md](2026-03-08_sector_d_canvas_surface_plan.md)

The canvas surface registries now exist as live runtime authorities. The sector work moved graph
physics, canvas interaction policy, layout algorithm ownership, layout-domain coordination, and
presentation tokens out of ad hoc render logic and into registry-owned runtime paths.

These five registries are tightly coupled by the layout-first principle (layout resolves before presentation) and must be developed together.

---

### Sector E — Workbench Surface {#sector-e}

**Registries:** `WorkbenchSurfaceRegistry`, `WorkflowRegistry`
**Plan:** [2026-03-08_sector_e_workbench_surface_plan.md](2026-03-08_sector_e_workbench_surface_plan.md)

The workbench surface registries govern tile-tree layout policy and session modes. `WorkbenchSurfaceRegistry` owns the tile-tree policy authority that SYSTEM_REGISTER names as one of the two mutation authorities. `WorkflowRegistry` composes Lens × WorkbenchProfile into named session modes.

Both registries are implemented. Remaining work in this sector is rollback/stabilization depth, not
authority existence.

---

### Sector F — Knowledge & Index {#sector-f}

**Registries:** `KnowledgeRegistry`, `IndexRegistry`, `DiagnosticsRegistry`
**Plan:** [2026-03-08_sector_f_knowledge_index_plan.md](2026-03-08_sector_f_knowledge_index_plan.md)

The knowledge/index/diagnostics registries now exist as live runtime authorities. `KnowledgeRegistry`
owns bundled UDC seed data, validation, query APIs, semantic-distance helpers, and semantic-index
lifecycle signaling. `IndexRegistry` owns `index:local` / `index:history` / `index:knowledge`
fanout and now backs the omnibox submit/action path. `DiagnosticsRegistry` now carries schema,
severity, retention, sampling, invariant, and config-roundtrip contract surfaces.

Residual follow-ons are now explicit rather than hidden sector blockers:
- omnibar suggestion-dropdown UI still uses a legacy candidate pipeline instead of `IndexRegistry`
- semantic placement-anchor consumption still needs a semantic-tagged node-creation caller
- `index:timeline` remains a future history-coupled stub rather than a live provider

---

### Sector G — Mod & Agent Runtime {#sector-g}

**Registries:** `ModRegistry`, `AgentRegistry`, `ThemeRegistry`
**Plan:** [2026-03-08_sector_g_mod_agent_plan.md](2026-03-08_sector_g_mod_agent_plan.md)

Sector G now has real runtime-owned `AgentRegistry` and `ThemeRegistry` authorities. The GUI and
phase-helper surfaces now share one global `RegistryRuntime` authority, `ControlPanel` supervises
registered agents, and the built-in `agent:tag_suggester` uses the signal bus plus reducer intent
ingress instead of direct app-state mutation.

Remaining Sector G work is explicit rather than hidden:
- `ModRegistry` still lacks a real WASM host / intent bridge.
- `GraphIntent::ModDeactivated` still does not exist as the reducer-carried unload receipt from the
  original plan.
- Theme activation is runtime-owned, but startup OS-theme detection and mod-provided theme
  activation remain follow-on work.
- Theme token migration is substantial but not yet absolute across all `render/` literals.

---

### Sector H — Signal Infrastructure {#sector-h}

**Registries:** `SignalRoutingLayer` → `SignalBus`
**Plan:** [2026-03-08_sector_h_signal_infrastructure_plan.md](2026-03-08_sector_h_signal_infrastructure_plan.md)

Sector H is implemented. `SignalRoutingLayer` now backs an explicit `SignalBus` trait facade, the topic model includes navigation/lifecycle/sync/registry-event/input-event families, dead-letter and lag visibility are surfaced through diagnostics, and async subscribers are available through the runtime signal APIs. The main remaining follow-on is consumer adoption in Sector G, not missing signal-bus authority.

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
