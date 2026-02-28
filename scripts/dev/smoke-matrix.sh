#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

TARGET_TEST="${TARGET_TEST:-graph_split_intent_tests}"

detect_host_lane() {
  if [[ -n "${GRAPHSHELL_CARGO_LANE:-}" ]]; then
    echo "$GRAPHSHELL_CARGO_LANE"
    return 0
  fi

  local uname_s
  uname_s="$(uname -s 2>/dev/null || echo unknown)"
  case "$uname_s" in
    Linux)
      echo "linux"
      ;;
    Darwin)
      echo "macos"
      ;;
    MINGW*|MSYS*|CYGWIN*|Windows_NT)
      echo "windows"
      ;;
    *)
      echo "unknown"
      ;;
  esac
}

resolve_target_dir() {
  local lane
  lane="$(detect_host_lane)"

  case "$lane" in
    linux)
      local linux_suffix=""
      if [[ -n "${GRAPHSHELL_LINUX_TARGET_FLAVOR:-}" ]]; then
        linux_suffix="-${GRAPHSHELL_LINUX_TARGET_FLAVOR}"
      elif is_wsl && [[ -n "${GRAPHSHELL_SPLIT_WSL_TARGET:-}" ]]; then
        linux_suffix="-wsl"
      fi
      echo "$ROOT_DIR/target/linux_target${linux_suffix}"
      ;;
    windows)
      echo "$ROOT_DIR/target/windows_target"
      ;;
    macos)
      echo "$ROOT_DIR/target/macos_target"
      ;;
    *)
      echo "$ROOT_DIR/target/host_target"
      ;;
  esac
}

prepare_cargo_target_dir() {
  if [[ -n "${CARGO_TARGET_DIR:-}" ]]; then
    mkdir -p "$CARGO_TARGET_DIR"
    echo "[smoke-matrix] Using caller-provided CARGO_TARGET_DIR=$CARGO_TARGET_DIR"
    return 0
  fi

  CARGO_TARGET_DIR="$(resolve_target_dir)"
  export CARGO_TARGET_DIR
  mkdir -p "$CARGO_TARGET_DIR"
  echo "[smoke-matrix] Using CARGO_TARGET_DIR=$CARGO_TARGET_DIR"
}

is_wsl() {
  [[ -n "${WSL_DISTRO_NAME:-}" || -n "${WSL_INTEROP:-}" ]] && return 0
  grep -qiE 'microsoft|wsl' /proc/sys/kernel/osrelease 2>/dev/null
}

apply_wsl_gl_fallback() {
  if ! is_wsl; then
    return 0
  fi

  if [[ -z "${GRAPHSHELL_DISABLE_WSL_SOFTWARE_FALLBACK:-}" ]]; then
    export LIBGL_ALWAYS_SOFTWARE="${LIBGL_ALWAYS_SOFTWARE:-1}"
    export MESA_LOADER_DRIVER_OVERRIDE="${MESA_LOADER_DRIVER_OVERRIDE:-llvmpipe}"
    export GALLIUM_DRIVER="${GALLIUM_DRIVER:-llvmpipe}"
    echo "[smoke-matrix] WSL detected, software GL fallback enabled."
  fi
}

usage() {
  cat <<'EOF'
Usage: scripts/dev/smoke-matrix.sh <command>

Commands:
  status   Print platform/runtime summary
  quick    Run non-GUI validation: cargo check --locked + one targeted lib test
  run      Start graphshell (applies WSL software GL fallback automatically)
  cargo    Run an arbitrary cargo subcommand with managed target dir

Environment knobs:
  TARGET_TEST=<test_name>  Override targeted test for quick mode
  GRAPHSHELL_CARGO_LANE=<linux|windows|macos>  Override host lane detection
  GRAPHSHELL_LINUX_TARGET_FLAVOR=<name>  Optional linux target suffix (e.g. ubuntu, wsl)
  GRAPHSHELL_SPLIT_WSL_TARGET=1  Auto-split WSL into linux_target-wsl
  CARGO_TARGET_DIR=<path>  Fully override target directory selection
EOF
}

cmd="${1:-quick}"
case "$cmd" in
  status)
    echo "repo: $ROOT_DIR"
    echo "uname: $(uname -a)"
    echo "lane: $(detect_host_lane)"
    echo "resolved target: $(resolve_target_dir)"
    echo "rust: $(rustc --version 2>/dev/null || echo 'missing')"
    echo "cargo: $(cargo --version 2>/dev/null || echo 'missing')"
    if is_wsl; then
      echo "platform: WSL"
    else
      echo "platform: non-WSL"
    fi
    echo "env LIBGL_ALWAYS_SOFTWARE=${LIBGL_ALWAYS_SOFTWARE:-<unset>}"
    echo "env MESA_LOADER_DRIVER_OVERRIDE=${MESA_LOADER_DRIVER_OVERRIDE:-<unset>}"
    echo "env GALLIUM_DRIVER=${GALLIUM_DRIVER:-<unset>}"
    ;;
  quick)
    prepare_cargo_target_dir
    cargo check --locked
    cargo test --locked --lib "$TARGET_TEST"
    ;;
  run)
    prepare_cargo_target_dir
    apply_wsl_gl_fallback
    cargo run
    ;;
  cargo)
    shift
    if [[ "$#" -eq 0 ]]; then
      echo "Usage: scripts/dev/smoke-matrix.sh cargo <cargo args...>" >&2
      exit 2
    fi
    prepare_cargo_target_dir
    cargo "$@"
    ;;
  -h|--help|help)
    usage
    ;;
  *)
    echo "Unknown command: $cmd" >&2
    usage >&2
    exit 2
    ;;
esac
