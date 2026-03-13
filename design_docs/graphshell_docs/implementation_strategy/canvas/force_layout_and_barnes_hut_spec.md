# Force Layout and Barnes-Hut Spec

**Date:** 2026-03-12  
**Status:** Canonical algorithm and scaling contract  
**Priority:** Tier 3 / algorithm boundary and future-scale path

**Related docs:**

- [`layout_behaviors_and_physics_spec.md`](./layout_behaviors_and_physics_spec.md)
- [`2026-02-24_physics_engine_extensibility_plan.md`](./2026-02-24_physics_engine_extensibility_plan.md)
- [`CANVAS.md`](./CANVAS.md)
- [`../system/register/physics_profile_registry_spec.md`](../system/register/physics_profile_registry_spec.md)
- [`../../technical_architecture/2026-03-12_specification_coverage_register.md`](../../technical_architecture/2026-03-12_specification_coverage_register.md)

**External standards anchors:**

- Fruchterman-Reingold 1991
- Barnes-Hut n-body approximation literature

---

## 1. Purpose and Scope

This spec defines the algorithm contract for Graphshell’s force-directed layout path and the prospective Barnes-Hut scaling path.

It governs:

- the current baseline force-layout semantics,
- tunable parameter ownership,
- extension-force boundaries,
- determinism and stability expectations,
- the role of Barnes-Hut as a higher-scale alternative,
- diagnostics and acceptance criteria for algorithm switching.

It does not govern:

- command-surface or focus behavior,
- view/lens binding semantics beyond physics-profile integration,
- detailed visual overlay behavior.

---

## 2. Canonical Algorithm Role

Graphshell’s current baseline layout engine is a Graphshell-owned force-directed implementation with Fruchterman-Reingold-style behavior. `egui_graphs` is used for rendering only; the physics loop runs through Graphshell-owned `Layout<S>` implementations in `graph/layouts/`.

Current practical state:

- baseline FR-style state is live,
- tuning parameters are applied through Graphshell-owned adapters,
- semantic clustering currently exists as a post-step Graphshell extension,
- degree repulsion and domain clustering remain planned extensions,
- Barnes-Hut is not the production default path.

Normative rule:

- FR remains the default baseline for current small/medium graph interaction,
- Barnes-Hut is a scaling alternative behind the same higher-level layout/physics contracts,
- Graphshell-owned extras must not silently redefine the baseline force model.

---

## 3. Current Baseline Force Model

The current baseline force-layout path consists of:

- repulsion,
- attraction,
- center gravity,
- damping,
- step-bounded iterative simulation.

Current Graphshell tuning carrier:

- `GraphPhysicsTuning`
  - `repulsion_strength`
  - `attraction_strength`
  - `gravity_strength`
  - `damping`

Current baseline state initialization also sets:

- `k_scale`
- `dt`
- `max_step`

Normative rule:

- baseline tuning parameters must remain explicitly documented against the effective FR-style implementation,
- Graphshell may adapt naming and parameter surfaces locally, but must note when they deviate from upstream or canonical FR vocabulary.

---

## 4. Graphshell-Owned Extension Contract

Graphshell extends baseline force layout through explicit extension seams.

Current extension config carrier:

- `GraphPhysicsExtensionConfig`
  - `degree_repulsion`
  - `domain_clustering`
  - `semantic_clustering`
  - `semantic_strength`

Current implemented extension:

- semantic clustering as a post-step position adjustment based on semantic similarity.

Planned extension families:

- degree-dependent repulsion,
- domain clustering,
- other `ExtraForce` / post-physics behaviors.

Normative rule:

- Graphshell-specific extension forces are layered on top of the baseline algorithm,
- not hidden inside undocumented renderer or UI logic,
- extension effects must remain attributable to explicit profile/registry/config state.

---

## 5. Barnes-Hut Role

Barnes-Hut is the planned higher-scale approximation path for force-directed repulsion.

Canonical role:

- improve scaling for larger graphs by approximating many-body repulsion,
- preserve high-level force-layout semantics while changing repulsion evaluation strategy,
- remain substitutable behind the same `LayoutRegistry` / physics-profile surface.

Normative rule:

- Barnes-Hut is a scaling implementation choice, not a new user-facing semantics model,
- switching to Barnes-Hut must not silently change profile/lens/interaction contracts,
- any changed quality/performance tradeoff must be explicit and diagnosable.

---

## 6. Algorithm Selection Contract

The higher-level product contract should not expose raw algorithm internals as the primary user abstraction.

Selection layers:

- user-facing physics profile / lens / layout choice,
- registry-level algorithm binding,
- concrete baseline FR or Barnes-Hut implementation.

Normative rule:

- user-facing presets such as `Liquid`, `Gas`, `Solid` bind to profile semantics first,
- implementation-level choice of FR vs Barnes-Hut happens behind those semantics unless explicitly surfaced as an advanced choice.

---

## 7. Determinism and Stability Contract

The force-layout engine is interactive and iterative, but it still requires bounded stability behavior.

Required properties:

- pinned nodes do not drift,
- step integration remains bounded,
- reheat and extension-force application remain explicit,
- large-graph scaling paths must not introduce nondeterministic behavior that breaks testability beyond acceptable floating-point variance.

Normative rule:

- exact bitwise determinism is not required across backends or platforms,
- but measurable invariants and relative behavioral expectations must remain testable.

Examples:

- enabling semantic clustering measurably reduces semantic distance between related nodes,
- enabling degree repulsion measurably spreads hubs,
- Barnes-Hut switching preserves broad topological readability while reducing cost.

---

## 8. Performance and Degradation Contract

Current baseline expectation:

- standard FR is acceptable for current small/medium graphs.

Barnes-Hut expectation:

- should become the preferred scaling path when node count or frame cost makes all-pairs repulsion too expensive.

Normative rule:

- algorithm switching for scale must be explicit in policy or diagnostics,
- not an invisible change in behavior with no traceability.

If Barnes-Hut is introduced:

- quality/speed tradeoff parameter (for example `theta`) must be explicit,
- fallback to baseline FR for small graphs should remain allowed,
- diagnostics should reveal which algorithm path is active when performance or layout issues are investigated.

---

## 9. Diagnostics Contract

Required diagnostics coverage should include:

- active layout algorithm identity,
- active physics profile identity,
- extension-force enablement state,
- significant algorithm-mode switch (baseline FR vs Barnes-Hut),
- layout-step performance metrics where available.

Normative rule:

- algorithm choice must be inspectable,
- not only inferred from behavior.

---

## 10. Test Contract

Required coverage:

1. tuning application updates the underlying layout state,
2. pinned nodes remain stable under extension-force application,
3. semantic clustering measurably affects related-node positions,
4. extension config enablement is explicit and test-covered,
5. baseline force-layout behavior remains bounded under repeated stepping,
6. if Barnes-Hut is introduced, pairwise quality/performance comparison tests exist against baseline FR.

When Barnes-Hut lands, additional required coverage:

7. algorithm selection policy chooses baseline FR vs Barnes-Hut predictably,
8. Barnes-Hut approximation parameter is bounded and validated,
9. switching algorithms preserves higher-level profile semantics.

---

## 11. Acceptance Criteria

- [ ] baseline force-layout semantics remain explicitly documented against the current implementation.
- [ ] Graphshell-owned extension forces remain explicit and attributable.
- [ ] Barnes-Hut is treated as a scaling implementation path, not an alternate product semantics model.
- [ ] algorithm selection and active algorithm identity remain diagnosable.
- [ ] future Barnes-Hut adoption must preserve higher-level profile and layout contracts.
