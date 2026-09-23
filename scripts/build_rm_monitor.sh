#!/usr/bin/env bash
# build_rm_monitor.sh — Build the rm-monitor Rust binary for installers and CI.
#
# Preference order:
#   1. Local cargo/rustc (PATH or ~/.cargo/bin) — always run incremental
#      `cargo build --release --locked` then install into dist/rm-monitor. Cargo's own
#      fingerprinting decides whether work is needed; never reuse a dist binary
#      that can be older-by-content than sources.
#   2. Existing fresh dist/rm-monitor only when cargo is unavailable and sources
#      are not newer than the binary (offline / no-toolchain fallback).
#   3. Docker/Podman rust:1-bookworm image when cargo is missing.
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

# reclaim_build_artifacts_for_invoker — When this script runs under sudo, cargo
# writes root-owned dist/ and target/. Hand them back to SUDO_USER so a later
# non-root `bash install.sh` can rebuild without permission errors. Fail hard
# if chown fails: a soft WARNING leaves root-owned trees that break the next
# non-root build with a confusing permission error far from this script.
reclaim_build_artifacts_for_invoker() {
  local owner
  [ "$(id -u)" -eq 0 ] || return 0
  owner="${SUDO_USER:-}"
  [ -n "$owner" ] && [ "$owner" != root ] || return 0
  for path in "$DIST_DIR" "$REPO_ROOT/target" "$REPO_ROOT/.cargo-container"; do
    [ -e "$path" ] || continue
    if ! chown -R "$owner:$owner" "$path"; then
      log "ERROR: could not chown $path to $owner (sudo-built artifacts would block later non-root rebuilds)"
      return 1
    fi
  done
}

# have_binary PATH — True when PATH is an executable file.
have_binary() { [ -x "$1" ]; }

# sources_newer_than_dist — True when Cargo.toml, Cargo.lock, or any src/*.rs
# is newer than dist/rm-monitor. Used only for the no-cargo fallback path.
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

# build_with_cargo — Compile the release binary with the local toolchain and
# install it into dist/. Cargo skips work when fingerprints are current.
build_with_cargo() {
  log "Building with local cargo…"
  (
    cd "$REPO_ROOT"
    cargo build --release --locked --bin rm-monitor
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
    -e CARGO_HOME=/src/.cargo-container \
    -e CARGO_TARGET_DIR=/src/target \
    -v "$REPO_ROOT:/src:rw" \
    -w /src \
    "$RUST_IMAGE" \
    cargo build --release --locked --bin rm-monitor
  install -m 0755 "$TARGET_BIN" "$DIST_BIN"
}

main() {
  if ensure_cargo_on_path; then
    # Always let cargo decide freshness, then refresh dist/ from target/release.
    build_with_cargo
  elif dist_is_fresh; then
    log "Using existing $DIST_BIN (cargo unavailable; sources not newer)"
  elif build_with_container; then
    :
  elif have_binary "$TARGET_BIN"; then
    # Only promote target/ when it is at least as new as sources (same freshness
    # rule as the no-cargo dist fallback).
    if find "$REPO_ROOT/src" "$REPO_ROOT/Cargo.toml" "$REPO_ROOT/Cargo.lock" \
         -type f -newer "$TARGET_BIN" 2>/dev/null | head -1 | grep -q .; then
      log "Cannot promote stale $TARGET_BIN (sources are newer; install cargo or docker/podman)."
      exit 1
    fi
    log "WARNING: promoting existing $TARGET_BIN without rebuild (no cargo/container)"
    mkdir -p "$DIST_DIR"
    install -m 0755 "$TARGET_BIN" "$DIST_BIN"
  else
    log "Cannot build rm-monitor: install cargo (rustup or apt install cargo) or docker/podman."
    exit 1
  fi

  if ! have_binary "$DIST_BIN"; then
    log "Build finished but $DIST_BIN is missing or not executable."
    exit 1
  fi

  reclaim_build_artifacts_for_invoker || exit 1

  if [ "$PRINT_ONLY" -eq 1 ]; then
    printf '%s\n' "$DIST_BIN"
  else
    log "Ready: $DIST_BIN"
  fi
}

main "$@"
