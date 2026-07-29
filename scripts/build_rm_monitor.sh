#!/usr/bin/env bash
# build_rm_monitor.sh — Build the rm-monitor Rust binary for installers and CI.
#
# Preference order:
#   1. Fresh release binary at dist/rm-monitor (newer than sources)
#   2. Local cargo/rustc (PATH or ~/.cargo/bin) — rebuild when dist is missing/stale
#   3. Docker/Podman rust:1-bookworm image (no local toolchain required)
#
# Usage:
#   bash scripts/build_rm_monitor.sh           # build into dist/rm-monitor
#   bash scripts/build_rm_monitor.sh --print   # print path to binary (build if needed)
#
# Exit codes: 0 on success, 1 on failure. Logs to stderr; --print emits only the
# absolute binary path on stdout when successful.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
DIST_DIR="$REPO_ROOT/dist"
DIST_BIN="$DIST_DIR/rm-monitor"
TARGET_BIN="$REPO_ROOT/target/release/rm-monitor"
PRINT_ONLY=0
RUST_IMAGE="${RM_MONITOR_RUST_IMAGE:-rust:1-bookworm}"

log() { printf '[build_rm_monitor] %s\n' "$*" >&2; }

usage() {
  cat >&2 <<'EOF'
Usage: bash scripts/build_rm_monitor.sh [--print]

Build the rm-monitor release binary into dist/rm-monitor.
With --print, also write the absolute binary path to stdout.
EOF
}

while [ $# -gt 0 ]; do
  case "$1" in
    --print) PRINT_ONLY=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) log "Unknown argument: $1"; usage; exit 1 ;;
  esac
done

# have_binary PATH — True when PATH is an executable file.
have_binary() { [ -x "$1" ]; }

# sources_newer_than_dist — True when Cargo.toml, Cargo.lock, or any src/*.rs
# is newer than dist/rm-monitor. Used so a previously built dist binary is not
# reused after source changes (which would ship stale patchers).
sources_newer_than_dist() {
  local bin="$DIST_BIN" newest
  [ -f "$bin" ] || return 0
  newest="$(find "$REPO_ROOT/src" "$REPO_ROOT/Cargo.toml" "$REPO_ROOT/Cargo.lock" \
    -type f -newer "$bin" 2>/dev/null | head -1 || true)"
  [ -n "$newest" ]
}

# dist_is_fresh — True when dist/rm-monitor exists and is not older than sources.
dist_is_fresh() {
  have_binary "$DIST_BIN" && ! sources_newer_than_dist
}

# ensure_cargo_on_path — Prefer rustup cargo when present.
ensure_cargo_on_path() {
  if command -v cargo >/dev/null 2>&1; then
    return 0
  fi
  if [ -x "$HOME/.cargo/bin/cargo" ]; then
    # shellcheck disable=SC1091
    [ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
    export PATH="$HOME/.cargo/bin:$PATH"
  fi
  command -v cargo >/dev/null 2>&1
}

# build_with_cargo — Compile the release binary with the local toolchain.
build_with_cargo() {
  log "Building with local cargo…"
  (
    cd "$REPO_ROOT"
    cargo build --release --bin rm-monitor
  )
  mkdir -p "$DIST_DIR"
  install -m 0755 "$TARGET_BIN" "$DIST_BIN"
}

# container_engine — Prefer docker, then podman.
container_engine() {
  if command -v docker >/dev/null 2>&1; then
    printf '%s\n' docker
  elif command -v podman >/dev/null 2>&1; then
    printf '%s\n' podman
  else
    return 1
  fi
}

# build_with_container — Compile inside an official Rust container image.
build_with_container() {
  local engine
  engine="$(container_engine)" || {
    log "Neither docker nor podman is available for containerized build."
    return 1
  }
  log "Building with $engine ($RUST_IMAGE)…"
  mkdir -p "$DIST_DIR" "$REPO_ROOT/target"
  # Mount the repo read-write so cargo can write target/; copy the binary out to dist/.
  "$engine" run --rm \
    --user "$(id -u):$(id -g)" \
    -e CARGO_HOME=/usr/local/cargo \
    -v "$REPO_ROOT:/src:rw" \
    -w /src \
    "$RUST_IMAGE" \
    cargo build --release --bin rm-monitor
  install -m 0755 "$TARGET_BIN" "$DIST_BIN"
}

main() {
  if dist_is_fresh; then
    log "Using existing $DIST_BIN"
  else
    if have_binary "$DIST_BIN" && sources_newer_than_dist; then
      log "dist/rm-monitor is stale (sources newer than binary); rebuilding"
    fi
    if ensure_cargo_on_path; then
      build_with_cargo
    elif build_with_container; then
      :
    elif have_binary "$TARGET_BIN"; then
      log "WARNING: promoting existing $TARGET_BIN without rebuild (no cargo/container)"
      mkdir -p "$DIST_DIR"
      install -m 0755 "$TARGET_BIN" "$DIST_BIN"
    else
      log "Cannot build rm-monitor: install cargo (rustup or apt install cargo) or docker/podman."
      exit 1
    fi
  fi

  if ! have_binary "$DIST_BIN"; then
    log "Build finished but $DIST_BIN is missing or not executable."
    exit 1
  fi

  if [ "$PRINT_ONLY" -eq 1 ]; then
    printf '%s\n' "$DIST_BIN"
  else
    log "Ready: $DIST_BIN"
  fi
}

main "$@"
