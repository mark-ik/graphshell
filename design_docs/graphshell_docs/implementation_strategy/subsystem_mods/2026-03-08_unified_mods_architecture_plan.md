# Unified Mods Architecture Plan

**Date**: 2026-03-08  
**Status**: Active / architecture cleanup plan  
**Scope**: Reconcile the Mods subsystem docs with the actual runtime split between built-in composition, native inventory mods, and planned WASM plugins.

**Related**:
- `SUBSYSTEM_MODS.md`
- `../system/register/mod_registry_spec.md`
- `../system/registry_runtime_spec.md`
- `../subsystem_security/2026-03-08_unified_security_architecture_plan.md`

---

## 1. Why This Plan Exists

The current Mods subsystem docs describe the target extension platform, not the runtime model that actually exists today.

Right now the codebase contains three different mechanisms under the label “mod”:

- built-in core capability seeds represented as native manifests
- native feature bundles discovered via `inventory::submit!`
- planned but not yet implemented WASM plugins

Those are not the same thing. Treating them as one flat subsystem model obscures:

- what is actually implemented
- what lifecycle guarantees really exist
- what parts of the architecture are still scaffold-only
- where security and registry ownership boundaries actually sit

This plan defines the missing top-level taxonomy so the subsystem docs can become technically honest without abandoning the long-term extension architecture.

---

## 2. Canonical Mods Taxonomy

The Mods subsystem should explicitly distinguish five concerns:

1. `CoreBuiltins`
2. `NativeFeatureMods`
3. `WasmPluginMods`
4. `CapabilityDeclarations`
5. `LifecycleIntegrity`

“Mods” is the umbrella subsystem. These five concerns are the real architecture.

---

## 3. Canonical Tracks

### 3.1 `CoreBuiltins`

Owns:

- the minimal built-in capability floor required for the app to boot and remain useful offline
- built-in registry seeds and built-in runtime composition
- fallback behavior when optional feature mods are absent

Current reality:

- the runtime currently models some of these as built-in native manifests like `mod:core-protocol`, `mod:core-viewer`, and `mod:core-identity`
- that is acceptable as a transition device, but it is not the same as “zero mods loaded”

Architecture rule:

- core built-ins are system-owned composition units, even if represented with manifest-like structures during transition

### 3.2 `NativeFeatureMods`

Owns:

- compiled-in optional/native feature bundles such as `verso`, `verse`, and `nostrcore`
- manifest declarations for those bundles
- activation hooks and diagnostics side effects

Current reality:

- native discovery and dependency ordering exist
- activation hooks exist but are thin
- much real provider wiring still happens in runtime composition rather than through mod activation

Architecture rule:

- native feature mods are first-class extension units, but they are not yet a full dynamic plugin system

### 3.3 `WasmPluginMods`

Owns:

- dynamic third-party plugin loading
- sandboxed host ABI
- runtime load/unload/reload behavior
- plugin isolation and failure containment

Current reality:

- this track is planned only
- `ModType::Wasm` exists as a placeholder
- no real runtime `extism` integration, host ABI, or plugin lifecycle exists today

Architecture rule:

- subsystem docs must treat WASM mods as a planned track until the runtime actually exists

### 3.4 `CapabilityDeclarations`

Owns:

- `provides` / `requires` declarations
- declared security-sensitive capabilities
- the difference between availability, dependency satisfaction, and permission to execute

Current reality:

- dependency ordering based on `provides` / `requires` is implemented
- capability declarations exist but are only partially enforced as policy

Architecture rule:

- a declaration is not the same thing as approval or enforcement

### 3.5 `LifecycleIntegrity`

Owns:

- discovery
- dependency resolution
- activation
- failure handling
- unload/reload semantics
- rollback/quarantine expectations
- registry cleanup and diagnostics obligations

Current reality:

- discovery, dependency ordering, and basic load diagnostics exist
- unload, reload, rollback, quarantine, and registry cleanup semantics are missing

Architecture rule:

- this track is the actual heart of the Mods subsystem; the missing lifecycle contract is now the primary architecture gap

---

## 4. Current Implementation Snapshot

### 4.1 Landed

- native mod discovery via `inventory::submit!`
- manifest structures with `provides` / `requires` / capability declarations
- deterministic dependency resolution and cycle detection
- basic lifecycle status tracking and load diagnostics
- environment-based disable paths for selected native mods

### 4.2 Partial / Inconsistent

- built-in core seeds are represented as native manifests even though they are system-owned composition
- native mod activation exists, but much actual registry/provider wiring still occurs outside activation hooks
- capability declarations exist, but the enforcement model is thinner than the docs imply
- diagnostics exist for basic load lifecycle, but not for quarantine/rollback/unload because those flows do not yet exist

### 4.3 Missing

- runtime `load_mod(path)` behavior
- runtime `unload_mod(id)` behavior
- hot-reload or capability diffing
- actual WASM plugin runtime and sandbox
- rollback/quarantine semantics for partial activation
- canonical cleanup contract for removing providers from registries
- a dedicated lifecycle integrity spec

---

## 5. Architectural Corrections

### 5.1 Stop Flattening Builtins And Mods

The docs should explicitly distinguish:

- system-owned built-in composition
- optional native feature mods
- future dynamic plugins

The current “everything is a mod” language is too imprecise for the runtime that actually exists.

### 5.2 Reframe The Core Seed Invariant

Replace “the app functions with zero mods loaded” with the more accurate contract:

- the app functions with only core built-ins active, and optional feature mods may fail or be disabled without breaking the offline organizer floor

If built-in manifests continue to be used as an implementation device, the docs should say so explicitly.

### 5.3 Separate Capability Declaration From Capability Enforcement

The plan must distinguish:

- manifest declaration
- dependency satisfaction
- runtime availability
- security approval / granted execution

This is a key dependency with the Security subsystem.

### 5.4 Make Activation Honest

Current activation hooks are mostly scaffolding. The subsystem docs should not imply that all registry population flows through activation today.

The cleanup path is:

1. define what activation is supposed to own
2. identify what provider wiring still bypasses it
3. decide which direct runtime composition paths are permanent system-owned composition and which should move behind activation

### 5.5 Promote Lifecycle Integrity To A First-Class Spec

`mod_lifecycle_integrity_spec.md` is no longer an optional future cleanup. It is the missing canonical contract for:

- partial activation behavior
- unknown mod handling
- rollback/quarantine
- unload/reload semantics
- registry cleanup

---

## 6. Sequencing Plan

### Phase A. Taxonomy Cleanup

1. Update subsystem docs to distinguish `CoreBuiltins`, `NativeFeatureMods`, and `WasmPluginMods`.
2. Rewrite the core-seed invariant in technically accurate terms.
3. Mark WASM runtime support as planned, not current.

Done-gate:

- docs no longer imply one already-unified mod model

### Phase B. Lifecycle Contract Definition

1. Write `mod_lifecycle_integrity_spec.md`. ✅ Landed 2026-04-08.
2. Define unknown-mod, failed-activation, and partial-registration behavior.
3. Define rollback/quarantine expectations and diagnostics.

Done-gate:

- lifecycle semantics are explicit even if some flows are still unimplemented

### Phase C. Activation Boundary Cleanup

1. Audit native mod activation versus direct runtime composition.
2. Decide which provider registration paths belong in activation and which remain system-owned composition.
3. Normalize manifest naming/identity consistency for native mods.

Done-gate:

- mod activation no longer serves as a vague placeholder boundary

### Phase D. Capability Model Alignment

1. Align Mods and Security docs on declaration vs enforcement.
2. Define which capability checks are loader-time, activation-time, and call-time.
3. Add explicit cross-plan notes with the Security subsystem plan.

Done-gate:

- capability policy is coherent across mods and security

### Phase E. WASM Track Activation

1. Introduce the real runtime design for WASM loading and sandboxed host ABI.
2. Implement `load_mod(path)` and related lifecycle paths. Initial headless Extism-backed path landed 2026-04-08 with sidecar-manifest admission, `WasmModSource` tracking, required guest exports (`init`, `render`, `on_event`, optional `update`), activation-time `init`, callable headless `render` / `on_event` host paths, deny-by-default capability checks, rollback-aware activation, and unload bookkeeping/quarantine semantics.
3. Add unload/reload only after the load/register/rollback contract is stable.

Done-gate:

- subsystem docs can honestly describe a two-tier runtime, not just a planned one

---

## 7. Cross-Plan Dependencies

- Security subsystem alignment is required for capability enforcement and sandbox policy.
- RegistryRuntime and register-layer specs must align on which composition is system-owned versus mod-owned.
- servoshell and host-boundary cleanup may change which browser/runtime features remain optional mods versus core composition.
- `graphshell_core` extraction should not freeze the extension model before phases A-C clarify the built-in/mod split.

---

## 8. Recommended Immediate Actions

1. Update `SUBSYSTEM_MODS.md` to reference this architecture plan.
2. Add an explicit “runtime reality” section to the subsystem guide.
3. Create `mod_lifecycle_integrity_spec.md` as the next missing canonical contract. ✅ Landed 2026-04-08.
4. Add implementation follow-ons for:
   - unknown-mod activation handling
   - remaining registry cleanup/rollback diagnostics beyond the current rollback/quarantine core channels
   - built-in versus mod-owned provider wiring audit
   - accurate WASM-runtime status language across docs
   - richer WASM guest ABI beyond the current headless `init`/`render`/`on_event`-validated Extism activation slice

---

## 9. Done Definition

The Mods subsystem architecture is coherent when:

- built-ins, native mods, and WASM plugins are modeled as distinct tracks
- the core-seed invariant is stated in technically accurate terms
- activation boundaries are explicit and honest
- lifecycle integrity has a canonical contract
- capability declarations are clearly separated from enforcement
- the docs describe the real runtime model rather than a future composite of planned pieces
