#!/usr/bin/env bash
# Compile-matrix for the servo-into-verso engine selectivity lane
# (see design_docs/graphshell_docs/implementation_strategy/shell/
#  2026-04-25_servo_into_verso_plan.md).
#
# Checks the four feature combos that must stay green:
#   1. default                                            (servo + wry + iced-host + gl_compat)
#   2. --no-default-features --features wry              (no-Servo Wry only)
#   3. --no-default-features --features iced-host,wry    (no-Servo iced-host + wry)
#   4. servo-engine + production features minus gl_compat (Servo on, GL fallback off)
#
# Exits non-zero on any failure. Produces a one-line pass/fail summary at
# the end so this script is safe to wire into CI or a pre-push hook.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

# 2026-04-27 GL-callback gating: combo 4 mirrors the production default
# minus `gl_compat`, validating the wgpu-only build path. If `gl_compat`
# eventually becomes off-by-default (slice 2 in the GL-retirement
# ordering), this combo collapses into combo 1.
SERVO_NO_GL_COMPAT_FEATURES="servo-engine,gamepad,js_jit,max_log_level,webgpu,webxr,diagnostics,wry,ux-probes,ux-bridge"

declare -a NAMES=(
  "default"
  "no-default --features wry"
  "no-default --features iced-host,wry"
  "no-default --features servo-engine,...,(no gl_compat)"
)
declare -a CMDS=(
  "cargo check -p graphshell --lib"
  "cargo check --no-default-features --features wry"
  "cargo check --no-default-features --features iced-host,wry"
  "cargo check --no-default-features --features $SERVO_NO_GL_COMPAT_FEATURES"
)

declare -a RESULTS=()

for i in "${!NAMES[@]}"; do
  name="${NAMES[$i]}"
  cmd="${CMDS[$i]}"
  echo "==> $name"
  echo "    $cmd"
  if eval "$cmd"; then
    RESULTS+=("PASS  $name")
  else
    RESULTS+=("FAIL  $name")
  fi
done

echo
echo "== engine-feature matrix summary =="
fail=0
for r in "${RESULTS[@]}"; do
  echo "  $r"
  [[ "$r" == FAIL* ]] && fail=1
done

exit "$fail"
