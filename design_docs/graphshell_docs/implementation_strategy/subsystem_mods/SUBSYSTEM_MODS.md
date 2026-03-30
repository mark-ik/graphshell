# MODS — Subsystem

**Date**: 2026-02-28
**Status**: Architectural subsystem note
**Priority**: Immediate architecture clarification

**Related**:

- `system/register/mod_registry_spec.md`
- `system/register/action_registry_spec.md`
- `system/registry_runtime_spec.md`
- `../subsystem_security/SUBSYSTEM_SECURITY.md`
- `2026-03-08_unified_mods_architecture_plan.md`
- `../../technical_architecture/2026-03-30_protocol_modularity_and_host_capability_model.md`

**Policy authority**: This file is the single canonical policy authority for the Mods subsystem.
Supporting mods docs may refine contracts, interfaces, and execution details, but must defer policy authority to this file.
Policy in this file should be distilled from canonical specs and accepted research conclusions.

**Adopted standards** (see [2026-03-04_standards_alignment_report.md](../../research/2026-03-04_standards_alignment_report.md) §§3.7, 3.17):
- **OSGi R8** — `ModManifest` `provides`/`requires` declarations, activation sequencing, capability lifecycle, and registry vocabulary follow the OSGi service component model
- **WASI Preview 1** (via extism) — normative contract for WASM mod sandbox; capability-restricted host interface; undeclared capabilities denied at load time

---

## 0A. Subsystem Policies

1. **Contracted-extension policy**: Mods extend existing registry contracts; they do not redefine core authority semantics.
2. **Manifest-integrity policy**: `provides/requires` declarations are required and validated before activation.
3. **Capability-boundary policy**: Native and WASM mods must remain within declared capability and sandbox constraints.
4. **Lifecycle-observability policy**: Load/activate/unload/dependency outcomes and denial paths must be diagnosable.
5. **Failure-containment policy**: Mod failure must degrade explicitly without silently corrupting registry state.

---

## 1. Purpose

This note defines the **Mods subsystem** as the architectural owner of mod lifecycle integrity.

It exists to keep one boundary explicit:

- registries define contracts,
- mods populate them,
- and the Mods subsystem guarantees that loading, activation, sandboxing, and unloading cannot silently corrupt registry state or violate capability grants.

---

## 2. Why This Is a Subsystem

Mods are a cross-cutting concern. Every registry can be populated by mods. Silent contract erosion from mod lifecycle failures — a mod registering into the wrong slot, a WASM mod escaping its capability grant, a native mod activating in the wrong order — is exactly the failure mode pattern that defines a Graphshell subsystem.

The Mods subsystem does not own registry semantics. It guarantees that the mechanics of loading, activating, and unloading mods cannot produce unobservable corruption.

---

## 3. What The Mods Subsystem Owns

- mod manifest validation (`ModManifest`: `provides`, `requires` declarations)
- mod loading pipeline (`mod_loader.rs`: native `inventory::submit!`; WASM runtime remains planned)
- mod activation sequencing (`mod_activation.rs`: dependency ordering, conflict detection)
- WASM sandbox enforcement (planned capability-restricted host interface via `extism`)
- mod health and activation diagnostics (which mods are loaded, which failed, which are deferred)
- core built-in definition (the minimal system-owned capability floor that keeps the app functional offline)
- mod unload / reload lifecycle (hot-reload path, if/when implemented)

---

## 4. Cross-Domain / Cross-Subsystem Policy Layer

The Mods subsystem does not define what any specific mod does — that belongs to the registry the mod populates.

- **Security subsystem**: capability grants and WASM sandbox enforcement are a shared boundary. Mods subsystem enforces that the declared `requires` match granted capabilities; Security subsystem owns the identity and grant store.
- **Diagnostics subsystem**: mod activation health and invariant violations are surfaced through the diagnostics channel schema.

---

## 5. Bridges

- Mods -> Registry Runtime: mod loader calls registry registration APIs at activation time
- Mods -> Security: capability grant checks before WASM mod activation
- Mods -> Diagnostics: activation failures, version conflicts, and sandbox violations emitted as diagnostic events
- Mods -> Control Panel: mod loader is a ControlPanel-supervised worker

---

## 6. Mod Tiers

| Tier | Mechanism | Sandboxed | Registered at |
|------|-----------|-----------|---------------|
| Native Mod | `inventory::submit!` | No | Startup |
| WASM Mod | `extism` dynamic load | Yes | Runtime (planned) |

Both tiers are intended to use the same `ModManifest` shape. However, the current runtime primarily implements the native tier; the WASM tier remains a planned track, not a landed runtime path. See `2026-03-08_unified_mods_architecture_plan.md` for the canonical split between built-ins, native mods, and future WASM plugins.

### 6.1 Relationship to protocol packaging

The Mods subsystem taxonomy and the protocol packaging taxonomy are related but
not identical.

- The Mods subsystem distinguishes built-ins, native feature mods, and planned
  WASM plugin mods.
- The protocol packaging model distinguishes `CoreBuiltins`,
  `DefaultPortableProtocolSet`, `OptionalPortableProtocolAdapters`,
  `NativeFeatureMods`, and `NonEngineNetworkLayers`.

Alignment rule:

- protocol modularity is **capability-scoped and host-aware**, not a claim that
  one binary mod package must load everywhere,
- portable protocol adapters and native feature mods are both valid extension
  units,
- only portable adapters are expected to span most host envelopes,
- host-bounded protocol absence must degrade explicitly through registry state
  and diagnostics.

---

## 7. Core Seed Invariant

The core floor (graph manipulation, local files, plaintext/metadata viewers, search, persistence) must remain functional with only core built-ins active. Optional feature mods must be disableable or fail without breaking the offline organizer floor.

During the current transition, some core built-ins are represented with manifest-like native entries. That implementation detail must not be mistaken for a claim that the runtime already has one uniform mod model.

### 7.1 Runtime Reality Gap

The current runtime is split across:

- system-owned core built-ins / composition seeds
- native inventory mods (`verso`, `verse`, `nostrcore`, etc.)
- planned but not yet implemented WASM plugins

The subsystem should be read through that three-part split. `2026-03-08_unified_mods_architecture_plan.md` is the canonical cleanup plan for closing the gap between the current runtime and the long-term two-tier extension architecture.

---

## 8. Architectural Rule

If a behavior answers "can this mod be loaded, activated, or unloaded without silently corrupting registry state?" it belongs to the **Mods subsystem**.

---

## 9. Deferred Spec: `mod_lifecycle_integrity_spec.md`

**Status**: Deferred — not yet written.

This deferred spec is now the primary architectural gap in the Mods subsystem. The unified architecture plan above should be treated as the staging document for that spec.

A `mod_lifecycle_integrity_spec.md` should be created once the registry specs that mods
actively populate are stable. Specifically, this spec is blocked on:

- `mod_registry_spec.md` — mod manifest registration contract,
- `action_registry_spec.md` — action registration by mods,
- `input_registry_spec.md` — input profile registration by mods,
- and any further registry specs whose registration lifecycle mods must participate in.

Until those specs define stable registration interfaces and invariants, writing the
mod lifecycle integrity spec would require re-specification as each registry hardens.

### What the deferred spec must cover

When written, `mod_lifecycle_integrity_spec.md` must define the normative contract for:

- manifest validation acceptance criteria (what `provides`/`requires` declarations are legal),
- activation sequencing invariants (topological order, conflict rules, deferred mod behavior),
- WASM capability grant enforcement at activation time,
- per-registry isolation contract (mod activation into one registry must not corrupt another),
- health diagnostics obligations (which channels emit for load/activate/deactivate failures),
- core seed protection invariant (core seed must remain functional if any mod activation fails),
- reload/hot-swap contract for WASM mods,
- acceptance criteria that gate readiness for production mod ecosystem support.

