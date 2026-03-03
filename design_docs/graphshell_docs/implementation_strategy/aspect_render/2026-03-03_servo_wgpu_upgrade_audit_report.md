# Servo wgpu Upgrade Audit Report

**Date**: 2026-03-03
**Status**: Living audit report
**Scope**: Servo-side `wgpu 26 -> 27` compatibility audit for the WebRender wgpu migration lane
**Primary workspace**: `../servo-graphshell` (sibling repo fork used for renderer migration work)
**Related plans**:
- `2026-03-01_webrender_wgpu_renderer_implementation_plan.md`
- `2026-03-01_webrender_readiness_gate_feature_guardrails.md`
- `2026-02-27_egui_wgpu_custom_canvas_migration_strategy.md`
- `render_backend_contract_spec.md`

---

## 1. Purpose

This report tracks the actual Servo-side version-audit work for the WebRender wgpu migration.

It exists to answer one narrow question with evidence:

- can the Servo fork move from its current `wgpu-core = 26` / `wgpu-types = 26` line to `27`
  without destabilizing the migration lane more than it helps?

This is a **working report**, not a frozen spec. Update it as the audit branch and post-WebRender
branch evolve.

---

## 2. Current Branch Topology

All branches below live in the sibling fork repository `../servo-graphshell`.

- `main`
  - clean mirror of upstream Servo with no Graphshell-specific renderer edits
- `renderer-webrender-wgpu-26-baseline`
  - reserved baseline branch for WebRender renderer work against Servo's original pinned `wgpu 26` line
- `renderer-wgpu-27-upgrade-audit`
  - active audit branch for the pure Servo-side `wgpu 27` compatibility pass
- `renderer-webrender-wgpu-27-post-upgrade`
  - reserved follow-on branch for WebRender renderer work after the `wgpu 27` audit is stable enough

**Current active branch during this report revision**: `renderer-webrender-wgpu-27-post-upgrade`

---

## 3. Baseline Before The Audit

Graphshell currently consumes Servo directly from `servo/servo.git`, but the renderer migration
work now uses the sibling fork `../servo-graphshell` as the controlled test fork.

Before the audit:

- Servo workspace root pinned:
  - `wgpu-core = "26"`
  - `wgpu-types = "26"`
- Graphshell lockfile currently resolves:
  - `wgpu 26.0.1`
- `egui-wgpu 0.33.3` depends on:
  - `wgpu 27.0.1`

This creates a real version-skew decision:

- either move Servo's WebGPU stack forward to `27`
- or keep Servo on `26` and patch `egui-wgpu` to the older line during the renderer migration

---

## 4. Audit Work Completed (2026-03-03)

### 4.1 Version pin bump

On `renderer-wgpu-27-upgrade-audit`, the Servo workspace root was changed from:

- `wgpu-core = "26"`
- `wgpu-types = "26"`

to:

- `wgpu-core = "27"`
- `wgpu-types = "27"`

This currently lives in `../servo-graphshell/Cargo.toml`.

### 4.2 First compile-pass finding

A direct component-level check was used instead of a full workspace check because the local Windows
environment blocks full Servo builds earlier on unrelated tool prerequisites.

Useful command:

```powershell
cargo check --manifest-path components\webgpu\Cargo.toml --message-format short
```

Initial direct breakage was concentrated in `components/webgpu`:

- removed `ImplicitPipelineIds` import path
- changed `command_encoder_finish` signature
- changed compute/render pipeline creation signatures
- `DeviceDescriptor` now requires `experimental_features`
- removed `into_command_encoder_id()` helper

### 4.3 Compatibility patch set landed on the audit branch

The following direct compatibility fixes were applied in `../servo-graphshell`:

- `components/webgpu/wgpu_thread.rs`
  - removed `ImplicitPipelineIds` usage
  - updated `command_encoder_finish(..., None)`
  - updated compute/render pipeline creation calls to the new 3-arg form
  - added `experimental_features: wgt::ExperimentalFeatures::default()`
  - replaced the removed command-buffer-to-encoder helper with an explicit `unzip()/zip()` conversion
- `components/webgpu/canvas_context.rs`
  - updated `command_encoder_finish(..., None)`
- `components/script/script_thread.rs`
  - replaced another stale `into_command_encoder_id()` call with the same explicit conversion pattern

### 4.4 Current compile evidence

Passing checks on the audit branch:

```powershell
cargo check --manifest-path components\webgpu\Cargo.toml --message-format short
cargo check --manifest-path components\shared\webgpu\Cargo.toml --message-format short
```

Meaning:

- the first-order Servo WebGPU compatibility layer now compiles against `wgpu 27`
- the direct `wgpu 27` fallout is real but so far manageable and localized

### 4.5 Post-upgrade branch handoff created

After the audit patch set was committed on `renderer-wgpu-27-upgrade-audit`, the same commit was
carried forward into:

- `renderer-webrender-wgpu-27-post-upgrade`

Current post-upgrade branch base:

- branch: `renderer-webrender-wgpu-27-post-upgrade`
- head commit: `1f9acedd366` (`Audit Servo wgpu 27 compatibility`)

This means the next renderer work does **not** need to replay the pure `wgpu 27` API audit.
It can start directly from the already-upgraded Servo-side WebGPU baseline.

### 4.6 Local WebRender patch path activated and verified

On `renderer-webrender-wgpu-27-post-upgrade`, Servo's root manifest now routes the crates.io
WebRender crates through a local sibling checkout:

- `webrender = { path = "../webrender/webrender" }`
- `webrender_api = { path = "../webrender/webrender_api" }`
- `wr_malloc_size_of = { path = "../webrender/wr_malloc_size_of" }`

This was activated in Servo's existing `[patch.crates-io]` section, not by adding a second
duplicate patch table.

The local editable checkout currently lives in:

- `../webrender/webrender`
- `../webrender/webrender_api`
- `../webrender/wr_malloc_size_of`

Verification command:

```powershell
cargo check --manifest-path components\shared\webgpu\Cargo.toml --message-format short
```

Observed confirmation:

- Cargo now checks `webrender`, `webrender_api`, and `wr_malloc_size_of` from `../webrender`
- the path override resolves cleanly on the post-upgrade branch

This closes the "can Servo be pointed at a local editable WebRender checkout on the `wgpu 27`
branch?" question enough to begin the actual renderer-side fork work.

---

## 5. What This Does And Does Not Prove

### Proven enough to proceed

- Servo's direct WebGPU layer is not fundamentally blocked by `wgpu 27`
- the initial break set is concentrated and fixable
- the version bump is not, by itself, a reason to abandon the `27` line

### Not yet proven

- full Servo workspace compatibility on this machine
- full `servo-script` compatibility under all feature sets
- WebRender renderer compatibility after introducing actual WGPU-renderer changes
- shared-device handoff behavior between patched WebRender and `egui_wgpu`

This report should not be read as "the renderer migration is solved."
It only closes the first Servo-side WebGPU API compatibility question enough to justify further work.

---

## 6. Local Environment Caveats

Servo's local Windows build guide in the fork README is:

1. `.\mach bootstrap`
2. `.\mach build`

This matters because direct `cargo check` from an arbitrary shell does **not** necessarily reflect
the environment that Servo expects on Windows.

In the current agent shell, the useful observed symptom is:

- `nasm` is not on `PATH`
- `cl` is not on `PATH`

So the present blocker should be interpreted as:

- the current shell session is not carrying the full Windows build environment Servo expects
  (very likely a path / shell-activation issue),
- not as proof that MozillaBuild itself is missing from the machine.

The earlier build failures (`mozjs_sys`, `aws-lc-sys`) therefore should be treated as
**environment-mismatch noise** unless they are reproduced from the proper Servo Windows build
entry path (`.\mach bootstrap`, then `.\mach build`, typically from the correctly activated
Windows toolchain shell).

Because of that, component-level `cargo check` remains useful for narrow API-audit work in this
session, but full-branch validation should be re-run later from the proper Servo Windows build
environment before drawing stronger conclusions.

---

## 7. Separation Of Concerns

This audit must stay conceptually separate from the actual WebRender renderer migration:

- **This audit branch** answers:
  - can Servo's current WebGPU-facing code survive the `26 -> 27` API move?
- **The post-WebRender branch** will answer:
  - what breaks when WebRender itself is patched to add a real wgpu renderer path?

These are related, but they are not the same task.

The expected next incompatibility class is not "Servo WebGPU API churn."
It is "WebRender renderer and compositor integration churn."

---

## 8. Recommended Next Steps

1. Start the first WebRender-side backend seam extraction on `renderer-webrender-wgpu-27-post-upgrade`
   now that the local editable checkout is in the build graph.
2. Focus the first renderer edits in WebRender's `device` / `renderer::init` boundary, where the
   current GL-backed `Device::new(...)` path is constructed.
3. Append new findings from that branch to this report under a new dated subsection instead of
   scattering notes across unrelated docs.

---

## 9. Update Discipline

When new migration evidence is gathered, append it here with:

- date
- branch name
- exact commands run
- observed break set
- whether the failure is:
  - Servo WebGPU API churn
  - WebRender renderer churn
  - host environment/tooling noise

This file is the running evidence log for the Servo-side version audit until the migration either:

- commits to `wgpu 27` as the mainline target, or
- explicitly retreats to the `wgpu 26` baseline strategy.
