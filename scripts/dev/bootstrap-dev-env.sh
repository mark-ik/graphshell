#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

MODE="check"
INSTALL_PWSH="0"

for arg in "$@"; do
  case "$arg" in
    --install)
      MODE="install"
      ;;
    --install-pwsh)
      INSTALL_PWSH="1"
      ;;
    -h|--help)
      ;;
    *)
      echo "Unknown argument: $arg" >&2
      echo "Use --help for usage." >&2
      exit 2
      ;;
  esac
done

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  cat <<'EOF'
Usage: scripts/dev/bootstrap-dev-env.sh [--install] [--install-pwsh]

Modes:
  (default)   Check which recommended tools are installed
  --install   Install Ubuntu/WSL baseline packages and Rust cargo helpers
  --install-pwsh  Install PowerShell (pwsh) on Ubuntu/WSL

Notes:
  - --install only performs apt/cargo installs on Linux.
  - Windows installs are printed as `winget` suggestions.
  - --install-pwsh is currently automated for Ubuntu/Debian apt systems.
EOF
  exit 0
fi

is_linux() {
  [[ "$(uname -s 2>/dev/null || echo unknown)" == "Linux" ]]
}

is_wsl() {
  [[ -n "${WSL_DISTRO_NAME:-}" || -n "${WSL_INTEROP:-}" ]] && return 0
  grep -qiE 'microsoft|wsl' /proc/sys/kernel/osrelease 2>/dev/null
}

print_header() {
  echo "[bootstrap] Graphshell dev environment helper"
  echo "[bootstrap] repo: $ROOT_DIR"
  echo "[bootstrap] mode: $MODE"
  if is_wsl; then
    echo "[bootstrap] platform: WSL"
  else
    echo "[bootstrap] platform: $(uname -s 2>/dev/null || echo unknown)"
  fi
}

report_tool() {
  local label="$1"
  local cmd="$2"
  if command -v "$cmd" >/dev/null 2>&1; then
    printf "  [ok]   %-12s -> %s\n" "$label" "$(command -v "$cmd")"
  else
    printf "  [miss] %-12s\n" "$label"
  fi
}

report_tool_any() {
  local label="$1"
  shift
  local cmd
  for cmd in "$@"; do
    if command -v "$cmd" >/dev/null 2>&1; then
      printf "  [ok]   %-12s -> %s (%s)\n" "$label" "$(command -v "$cmd")" "$cmd"
      return 0
    fi
  done
  printf "  [miss] %-12s\n" "$label"
}

check_tools() {
  echo "[bootstrap] checking core tools"
  report_tool git git
  report_tool gh gh
  report_tool jq jq
  report_tool rg rg
  report_tool_any fd fd fdfind
  report_tool fzf fzf
  report_tool_any bat bat batcat
  report_tool zoxide zoxide
  report_tool eza eza
  report_tool rustc rustc
  report_tool cargo cargo
  report_tool rustup rustup
  report_tool pwsh pwsh

  echo "[bootstrap] checking optional cargo helpers"
  report_tool cargo-binstall cargo-binstall
  report_tool sccache sccache
  report_tool cargo-nextest cargo-nextest
  report_tool cargo-watch cargo-watch
  report_tool cargo-add cargo-add
}

install_linux_baseline() {
  if ! is_linux; then
    echo "[bootstrap] --install is currently automated only for Linux/WSL."
    echo "[bootstrap] Suggested Windows command:"
    echo "  winget install Git.Git GitHub.cli Microsoft.PowerShell Rustlang.Rustup BurntSushi.ripgrep sharkdp.fd junegunn.fzf jqlang.jq"
    return 0
  fi

  echo "[bootstrap] installing Ubuntu/WSL baseline packages via apt"
  sudo apt update
  sudo apt install -y \
    git gh jq ripgrep fd-find fzf bat zoxide eza unzip zip

  if command -v fdfind >/dev/null 2>&1 && ! command -v fd >/dev/null 2>&1; then
    mkdir -p "$HOME/.local/bin"
    ln -sf "$(command -v fdfind)" "$HOME/.local/bin/fd"
    echo "[bootstrap] linked fdfind -> ~/.local/bin/fd"
    echo "[bootstrap] ensure ~/.local/bin is in PATH"
  fi

  if command -v batcat >/dev/null 2>&1 && ! command -v bat >/dev/null 2>&1; then
    mkdir -p "$HOME/.local/bin"
    ln -sf "$(command -v batcat)" "$HOME/.local/bin/bat"
    echo "[bootstrap] linked batcat -> ~/.local/bin/bat"
    echo "[bootstrap] ensure ~/.local/bin is in PATH"
  fi

  if command -v cargo >/dev/null 2>&1; then
    echo "[bootstrap] installing cargo helpers"
    cargo install cargo-binstall
    if command -v cargo-binstall >/dev/null 2>&1; then
      cargo binstall -y sccache cargo-nextest cargo-watch cargo-edit
    else
      cargo install sccache cargo-nextest cargo-watch cargo-edit
    fi
  else
    echo "[bootstrap] cargo not found, skipping cargo helper installation"
  fi
}

install_pwsh_linux() {
  if ! is_linux; then
    echo "[bootstrap] --install-pwsh is currently automated only for Linux/WSL."
    return 0
  fi

  if command -v pwsh >/dev/null 2>&1; then
    echo "[bootstrap] pwsh already installed: $(command -v pwsh)"
    return 0
  fi

  if [[ ! -f /etc/os-release ]]; then
    echo "[bootstrap] cannot determine distro (missing /etc/os-release), skipping pwsh install"
    return 0
  fi

  . /etc/os-release
  if [[ "${ID:-}" != "ubuntu" && "${ID:-}" != "debian" ]]; then
    echo "[bootstrap] automatic pwsh install currently supports ubuntu/debian only"
    echo "[bootstrap] for other distros, install from: https://learn.microsoft.com/powershell/scripting/install/installing-powershell-on-linux"
    return 0
  fi

  echo "[bootstrap] installing pwsh from Microsoft package repository"
  sudo apt-get update
  sudo apt-get install -y wget apt-transport-https software-properties-common gpg

  local version_full
  local major_version
  local repo_url
  version_full="${VERSION_ID:-}"
  major_version="${VERSION_ID%%.*}"
  if [[ -z "$major_version" ]]; then
    major_version="24"
  fi

  repo_url="https://packages.microsoft.com/config/${ID}/${version_full}/packages-microsoft-prod.deb"
  if ! wget -q "$repo_url" -O /tmp/packages-microsoft-prod.deb; then
    repo_url="https://packages.microsoft.com/config/${ID}/${major_version}/packages-microsoft-prod.deb"
    wget -q "$repo_url" -O /tmp/packages-microsoft-prod.deb
  fi
  sudo dpkg -i /tmp/packages-microsoft-prod.deb
  rm -f /tmp/packages-microsoft-prod.deb

  sudo apt-get update
  sudo apt-get install -y powershell
}

print_next_steps() {
  cat <<'EOF'

[bootstrap] recommended aliases (add to ~/.bashrc or ~/.zshrc):
  alias c='cargo'
  alias cc='cargo check -q'
  alias ct='cargo test -q'
  alias gs='git status -sb'
  alias gl='git log --oneline --decorate -20'

[bootstrap] Graphshell lane-safe commands:
  scripts/dev/smoke-matrix.sh status
  scripts/dev/smoke-matrix.sh quick
  scripts/dev/smoke-matrix.sh cargo build --release
EOF
}

print_header
if [[ "$INSTALL_PWSH" == "1" ]]; then
  install_pwsh_linux
fi
if [[ "$MODE" == "install" ]]; then
  install_linux_baseline
fi
check_tools
print_next_steps
