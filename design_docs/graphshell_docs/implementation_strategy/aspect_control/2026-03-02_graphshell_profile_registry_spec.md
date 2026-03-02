# Graphshell Profile Registry Spec

**Date**: 2026-03-02  
**Status**: Planned  
**Priority**: Core control-surface foundation (not a pre-renderer/WGPU blocker)

**Related**:
- `settings_and_control_surfaces_spec.md`
- `../2026-03-01_complete_feature_inventory.md`
- `../workbench/workbench_frame_tile_interaction_spec.md`
- `../aspect_input/input_interaction_spec.md`
- `../../TERMINOLOGY.md`

---

## Goal

Define a general, persisted profile system for Graphshell that can store and apply broad configuration sets across runtime surfaces.

The system must support:

1. a required default profile shipped by Graphshell,
2. user-created profiles,
3. future mod-contributed profile presets,
4. stable persistence + migration for evolving profile schema.

## Core Contract

### Profile identity

Every profile has:

- `profile_id` (stable UUID-like identifier),
- `display_name`,
- `origin` (`BuiltIn`, `User`, `Mod`),
- `created_at`, `updated_at`,
- `schema_version`.

### Profile payload model

`GraphshellProfile` is a composed payload with typed sections. MVP sections:

- `input` (keybinding maps, mouse policy, interaction toggles),
- `workbench` (layout policy defaults, pane behavior preferences),
- `viewer` (viewer/readability defaults, import mapping defaults),
- `appearance` (theme token choices via existing theme primitives),
- `safety` (permission and trust-boundary defaults for import/mapping actions).

Rules:

- Unknown sections/fields must be preserved for forward compatibility.
- Section-level partial updates must not erase unrelated sections.
- Defaults are resolved by merge order: `BuiltIn base -> active profile overrides -> session overrides`.

## Required Built-In Default Profile

Graphshell must ship one built-in profile:

- `profile_id = graphshell:default`,
- `display_name = Default`,
- immutable baseline payload used as fallback when no other profile is selected.

Behavior requirements:

- First run always has an active profile (`graphshell:default`).
- If active profile fails to load/validate, runtime falls back to `graphshell:default` and emits diagnostics.
- `graphshell:default` can be cloned but not deleted.

## User-Created Profiles

Users must be able to create profiles via control surfaces.

### Create flow (MVP)

1. User invokes `profile.create`.
2. User selects source:
   - clone active profile,
   - clone built-in default,
   - start from section templates.
3. System validates name + schema compatibility.
4. System persists profile and sets active profile by explicit user choice.

### Manage flow (MVP)

Required actions:

- activate profile,
- rename profile,
- duplicate profile,
- export/import profile document,
- delete profile (except built-in default).

Safety behavior:

- Deleting active profile triggers explicit replacement selection.
- Importing incompatible profile triggers migration attempt or non-destructive rejection with diagnostics.

## Persistence and Storage

Profile persistence requirements:

- Profiles are stored in Graphshell user data paths via existing platform policy.
- Active profile reference is persisted separately from profile payload records.
- Writes are atomic (temp + replace) to avoid partial-corruption states.
- Recovery path must tolerate malformed profile files by isolating bad entries and preserving valid profiles.

## Schema Evolution and Versioning

- `schema_version` is mandatory per profile document.
- Migrations are incremental and deterministic.
- Migration chain must be testable with fixture profiles from prior versions.
- Unsupported future schema versions remain inactive until compatible runtime is available.

## Diagnostics

Required diagnostic events:

- profile created/updated/deleted/activated,
- profile load failure + fallback-to-default,
- migration success/failure,
- import/export success/failure,
- schema validation warnings.

## UX Surface Requirements

Control-surface requirements:

- Profile selector in settings/control surfaces,
- explicit active profile indicator,
- profile diff preview before activation (at least section-level summary),
- one-click `Create New Profile` action,
- reset-current-profile-to-default action (non-destructive confirmation).

## Phase Breakdown

### Phase 1 — Registry + persistence foundation

- Introduce `GraphshellProfile` schema and profile registry storage.
- Ship built-in `graphshell:default` profile.
- Implement active profile resolution + fallback path.

### Phase 2 — Control surface wiring

- Add profile selector and create/manage actions in settings surfaces.
- Add section-level editing + save/apply semantics.

### Phase 3 — Import/export + migration hardening

- Add profile document import/export.
- Add schema migration engine and diagnostics coverage.

### Phase 4 — Cross-feature integration

- Bind mapping defaults, input policies, and workbench defaults to active profile sections.
- Add integration tests for profile-driven runtime behavior.

## Acceptance Criteria

1. Graphshell ships with `graphshell:default` and always has a valid active profile.
2. Users can create a new profile and persist it without modifying unrelated profile sections.
3. Users can activate, duplicate, rename, and delete (non-default) profiles from control surfaces.
4. Active profile survives restart and applies typed section defaults at runtime.
5. Corrupt or incompatible profile documents do not crash runtime and fall back safely to default with diagnostics.
6. Import/export and migration paths preserve known fields and forward-compatible unknown fields.
7. Profile-based settings for input/workbench/viewer can be consumed by feature modules without direct coupling to storage internals.

---

## Findings

- Existing planning already expects `WorkbenchProfile` behavior, but a broader profile model is needed to avoid fragmented per-feature config stores.
- A single `GraphshellProfile` contract provides the stable backbone for future workflow/lens/profile composition.
- Default profile + user-created profiles are required baseline capabilities before advanced profile composition features.

---

## Progress

- 2026-03-02: Created canonical general profile registry spec with required built-in default profile, user-created profile lifecycle, persistence contract, and migration/diagnostics requirements.
