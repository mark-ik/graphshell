# Graphshell Standalone Extraction Notes

This directory is a standalone Graphshell crate extracted from `ports/graphshell`.

## Goal

Keep Graphshell decoupled from Servo monorepo workspace layout while still tracking Servo quickly.

## Dependency model

`graphshell/Cargo.toml` now pulls Servo internals via git dependencies (branch `main`):

- `libservo`
- `webdriver_server`
- `servo_allocator`
- `base` (dev + android/ohos target support)

This allows frequent updates without vendoring Servo components locally.

## Build (standalone)

From repository root:

```powershell
cargo check --manifest-path graphshell/Cargo.toml
cargo run --manifest-path graphshell/Cargo.toml -p graphshell
```

## Keeping Servo current

Fast path (track latest Servo main):

```powershell
cargo update --manifest-path graphshell/Cargo.toml
```

If you want deterministic updates, pin `rev = "<sha>"` for Servo git deps and bump intentionally.

## Migration direction

Phase 1 (done):
- Extracted Graphshell source into top-level `graphshell/`
- Switched from workspace path dependencies to Servo git dependencies
- Removed `ports/graphshell` from root workspace members/default-members

Phase 2 (recommended):
- Move `graphshell/` into its own repository (`servo-graphshell`)
- Keep Servo as a git dependency only
- Add CI that runs:
  - `cargo check --manifest-path Cargo.toml`
  - periodic dependency refresh against `servo/main`

Phase 3 (stability hardening):
- Add a compatibility shim module for Servo API changes
- Run weekly servo-main bump job and auto-open fix PRs
