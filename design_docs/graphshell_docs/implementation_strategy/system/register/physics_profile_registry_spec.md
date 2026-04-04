# Physics Profile Registry Spec

**Doc role:** Canonical registry spec for `physics_profile_registry`.
**Status:** Active / canonical
**Kind:** Atomic registry
**Related docs:**

- [../2026-02-22_registry_layer_plan.md](../2026-02-22_registry_layer_plan.md) (registry ecosystem and ownership model)
- [SYSTEM_REGISTER.md](SYSTEM_REGISTER.md) (register hub and routing boundary)
- [../../graph/2026-04-03_damping_profile_follow_on_plan.md](../../graph/2026-04-03_damping_profile_follow_on_plan.md) (named damping curves and settle-shape policy)

**Policy authority**: This file is the canonical policy authority for `physics_profile_registry` semantics and boundaries.
Policy in this file should be distilled from canonical specs and accepted research conclusions.

**Adopted standards** (see [2026-03-04_standards_alignment_report.md](../../research/2026-03-04_standards_alignment_report.md) §§3.6, 3.7, 3.9)):

- **Fruchterman-Reingold 1991** — named presets (`Liquid`, `Gas`, `Solid`) are semantic parameter sets over the Fruchterman-Reingold force model; parameter semantics must be documented against the algorithm
- **OSGi R8** — capability registration, preset lookup, and fallback floor vocabulary
- **OpenTelemetry Semantic Conventions** — diagnostic channel naming/severity

## Registry Policies

1. **Named-preset policy**: Physics profiles are semantic named parameter sets with stable IDs and explicit fallback.
2. **Execution-separation policy**: Profile selection belongs here; physics engine execution remains in canvas territory.
3. **Lookup-determinism policy**: Profile resolution and legacy mapping behavior are deterministic and diagnosable.
4. **Override-policy**: Future user/mod overrides must preserve core fallback safety contracts.
5. **Shared-policy policy**: Physics profiles participate in a broader cross-system
   policy stack (`FamilyPhysicsPolicy`, lens switching, settings exposure,
   diagnostics reporting) and must not become canvas-only hidden state.

## Purpose and Scope

Defines named force/physics parameter presets such as Liquid, Gas, and Solid.

In scope:

- physics preset registration
- named profile lookup
- cross-domain semantic labels over numeric parameters
- shared preset semantics consumed by lens/settings/diagnostics surfaces

Out of scope:

- physics engine execution
- layout algorithm selection
- camera command dispatch

## Canonical Model

This registry is a named capability surface within the Register. It should expose stable lookup and capability contracts, keep failures explicit, and avoid hidden fallback semantics.

Canonical interfaces:

- `get_profile(id)`
- `describe_profile(id) -> PhysicsCapability`
- `resolve_preset(mode) -> PhysicsProfile`

## Normative Core

- Profiles are parameters only; execution remains elsewhere.
- Named presets are user-facing semantic contracts.
- Preset lookup must be deterministic and overrideable.
- The active profile should be inspectable and explainable outside the canvas via
  shared settings/diagnostics surfaces; registry semantics must support that reuse.

## Planned Extensions

- shared preset mapping across camera/layout/physics domains
- user-configurable preset overrides

## Prospective Capabilities

- adaptive presets driven by workflow or content density
- mod-shipped simulation bundles

## Acceptance Criteria

- Registration, lookup, and fallback behavior are covered by registry contract tests.
- At least one harness or scenario path exercises the registry through real app behavior.
- Diagnostics channels emitted by this registry follow the Register diagnostics contract.
- Registry ownership remains explicit and does not collapse into widget-local or ad hoc fallback logic.
