# Mod Lifecycle Integrity Spec

**Date**: 2026-04-08
**Status**: Canonical / active
**Priority**: Immediate architecture contract

**Related**:

- `SUBSYSTEM_MODS.md`
- `2026-03-08_unified_mods_architecture_plan.md`
- `../system/register/mod_registry_spec.md`
- `../system/registry_runtime_spec.md`
- `../subsystem_security/SUBSYSTEM_SECURITY.md`

---

## 1. Purpose

This spec defines the canonical lifecycle-integrity contract for Graphshell mods.

It exists to make one boundary explicit:

- discovery must not silently admit malformed or duplicate mod identities,
- activation must not silently corrupt registry state,
- partial registration and failure paths must be explicit and diagnosable,
- unload must not leave live registry residue behind.

This spec governs both native mods and future WASM mods. It does not require the
WASM runtime to already exist, but it does define the lifecycle rules that any
WASM runtime implementation must satisfy.

Implementation note (2026-04-08): the current runtime now admits path-backed
`.wasm` modules, validates the minimal headless guest contract (`init`,
`render`, `on_event`, optional `update`), runs `init` during activation, exposes
headless host-call paths for `render` and `on_event`, and uses rollback-first
activation failure handling with explicit quarantine when rollback or unload
cannot fully remove installed extension records.

---

## 2. Scope

This spec covers:

1. manifest admission and identity rules
2. dependency resolution and activation ordering
3. activation failure and partial-registration behavior
4. capability declaration versus granted execution
5. unload / rollback / quarantine behavior
6. diagnostics obligations

This spec does not define feature semantics inside protocol, viewer, action, or
other registry families. Those remain owned by their respective registry specs.

---

## 3. Canonical Lifecycle Model

Every mod lifecycle follows these phases:

1. **Discovery** — the runtime obtains one or more `ModManifest` candidates from
   built-in/native inventory or future WASM/plugin sources.
2. **Admission** — manifests are validated for stable identity, declared
   capabilities, and dependency vocabulary.
3. **Resolution** — dependency ordering is computed deterministically.
4. **Activation** — the mod attempts to attach registry extensions and runtime
   side effects.
5. **Active operation** — the mod may provide declared capabilities while its
   registry extensions remain installed.
6. **Unload / rollback / quarantine** — the runtime removes installed
   extensions, or marks the mod unavailable, without silently corrupting the
   registry surfaces.

No lifecycle phase may rely on hidden widget state, implicit fallback wiring, or
undocumented ordering behavior.

Current status vocabulary in code:

- `Discovered`
- `Loading`
- `Active`
- `Failed`
- `Quarantined`
- `Unloaded`

---

## 4. Manifest Admission Contract

### 4.1 Identity and Duplication

1. `mod_id` is the canonical identity for a mod lifecycle unit.
2. Duplicate `mod_id` entries are rejected before activation.
3. Admission failure due to duplicate identity is explicit and diagnosable.

### 4.2 Declared Surfaces

1. `provides` and `requires` declarations are part of manifest admission.
2. A mod may only activate after every declared requirement resolves to an
   admitted provider.
3. Requirement resolution must be deterministic for the same manifest set.

### 4.3 Capability Declaration vs Execution Approval

1. Declaring a capability is not the same as being granted execution rights.
2. Native mods may be available but still disabled by runtime policy.
3. WASM mods must satisfy sandbox/capability policy before activation.
4. Denial must be explicit; silent downgrade into active execution is forbidden.

---

## 5. Resolution and Activation Contract

### 5.1 Ordering

1. Activation order is dependency-driven and deterministic.
2. Cycles and missing requirements are explicit failure states.
3. A failure in one optional feature mod must not break the core built-in floor.

### 5.2 Activation Ownership

1. Activation owns installation of registry extensions attributable to that mod.
2. System-owned core composition may exist outside mod activation, but such
   composition must remain explicitly identified as system-owned.
3. A mod must not be considered `Active` until its registry contributions have
   either completed successfully or been explicitly recorded as empty.

### 5.3 Partial Registration

1. If activation installs some extensions and then fails, the runtime must not
   silently leave the registry in a partially attached state.
2. The runtime must either:
   - roll back all extensions installed during that activation attempt, or
   - quarantine the mod with explicit diagnostics and a stable record of which
     extensions remain installed.
3. The preferred contract is rollback-first for the initial implementation.

---

## 6. Unload, Rollback, and Quarantine Contract

### 6.1 Unload

1. A mod may only unload when no active dependent mod still requires one of its
   provided surfaces.
2. Unload removes registry extensions in reverse installation order.
3. Unload failure is explicit and must transition the mod into a diagnosable
   failure state.

### 6.2 Rollback

1. Rollback is the removal of extensions installed during a failed activation
   attempt before the mod ever reaches `Active`.
2. Rollback must preserve registry health and keep the runtime in a deterministic
   post-failure state.
3. Rollback must run in reverse installation order over the extension records
   already applied during the failed activation attempt.

### 6.3 Quarantine

1. Quarantine is reserved for cases where rollback cannot fully restore a safe
   state.
2. A quarantined mod must not advertise itself as active.
3. Quarantine must surface explicit diagnostics and remain visible to runtime
   inspection.

---

## 7. Native and WASM Tier Rules

### 7.1 Native Mods

1. Native mods may activate through inventory-discovered manifests and compiled
   activation hooks.
2. Native status must still follow the same lifecycle-integrity rules as WASM:
   explicit admission, deterministic ordering, explicit failure, and explicit
   unload semantics.

### 7.2 WASM Mods

1. WASM mods are not considered landed until the runtime can discover, admit,
   resolve, activate, and deny them under capability policy.
2. The first WASM runtime slice may be headless and registry-only; it does not
   need to include widget/UI embedding to satisfy lifecycle-integrity readiness.
3. Any WASM runtime must integrate with the same lifecycle status model as
   native mods.
4. The current minimal guest-surface contract is:
   - required exports: `init`, `render`, `on_event`
   - optional export: `update`
5. Richer widget/UI ABI, hot reload, and non-deny-by-default host capability
   grants remain follow-on work after the lifecycle contract is stable.

---

## 8. Diagnostics Obligations

The runtime must emit diagnosable events for at least:

1. discovery/admission failure
2. missing dependency
3. dependency cycle
4. activation started
5. activation succeeded
6. activation failed
7. capability denial
8. rollback attempted / rollback failed
9. unload succeeded / unload failed
10. quarantine entered

Diagnostics must identify the affected `mod_id` and the lifecycle phase.

Implementation note (2026-04-08): the current runtime now registers explicit
channels for `registry.mod.rollback_succeeded`, `registry.mod.rollback_failed`,
`registry.mod.quarantined`, and `registry.mod.unload_failed` in addition to the
existing load/dependency channels.

---

## 9. Acceptance Criteria

| Criterion | Verification |
| --- | --- |
| Duplicate `mod_id` values are rejected before activation | Unit test over manifest admission / dependency resolution |
| Missing requirements fail explicitly | Unit test over `resolve_dependencies()` |
| Mixed native/WASM manifest sets resolve deterministically | Unit test over manifest ordering with at least one WASM manifest |
| Failed activation does not silently mark a mod active | Unit test over `load_all_with_extensions()` failure path |
| Failed activation rolls back applied extension records or quarantines on rollback failure | Unit test over rollback-aware `load_all_with_extensions()` failure path |
| Active dependents block unload | Unit test over `unload_mod_with()` |
| Extension removal failures are explicit and preserve remaining records via quarantine | Unit test over `unload_mod_with()` failing callback |
| Capability denial is explicit | Runtime or unit tests in the future WASM activation path |
| Core built-ins remain functional if optional mod activation fails | Harness or scenario path exercising a failed optional mod |

---

## 10. Immediate Implementation Guidance

1. Normalize the loader around mixed manifest sets, even before the real WASM
   runtime exists.
2. Keep rollback-first semantics as the initial target for failed activation.
3. Close native scaffolds like `protocol:verse` registration without waiting for
   the broader WASM runtime.
4. Treat richer WASM plugin surfaces (widget rendering, hot reload, broader
   capability grants) as follow-on work after the basic lifecycle contract is
   implemented and rollback/quarantine behavior is proven.