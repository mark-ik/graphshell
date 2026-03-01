# MODS — Subsystem

**Date**: 2026-02-28
**Status**: Architectural subsystem note
**Priority**: Immediate architecture clarification

**Related**:

- `system/register/mod_registry_spec.md`
- `system/register/action_registry_spec.md`
- `system/registry_runtime_spec.md`
- `../subsystem_security/SUBSYSTEM_SECURITY.md`

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
- mod loading pipeline (`mod_loader.rs`: native `inventory::submit!` + WASM `extism`)
- mod activation sequencing (`mod_activation.rs`: dependency ordering, conflict detection)
- WASM sandbox enforcement (capability-restricted host interface via `extism`)
- mod health and activation diagnostics (which mods are loaded, which failed, which are deferred)
- core seed definition (the minimal registry population that makes the app functional without any mods)
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
| WASM Mod | `extism` dynamic load | Yes | Runtime |

Both tiers use the same `ModManifest` format and activate through the same sequencing pipeline. The subsystem treats sandboxing as a capability enforcement boundary, not a trust boundary — native mods are trusted by virtue of compilation, not by bypassing validation.

---

## 7. Core Seed Invariant

The core seed (graph manipulation, local files, plaintext/metadata viewers, search, persistence) must remain functional with zero mods loaded. The Mods subsystem is responsible for ensuring the core seed is never broken by a mod activation failure.

---

## 8. Architectural Rule

If a behavior answers "can this mod be loaded, activated, or unloaded without silently corrupting registry state?" it belongs to the **Mods subsystem**.

---

## 9. Deferred Spec: `mod_lifecycle_integrity_spec.md`

**Status**: Deferred — not yet written.

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

