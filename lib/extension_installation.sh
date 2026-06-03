#!/usr/bin/env bash
# extension_installation.sh — GNOME Shell extension installation helpers
#
# Components:
#   - enable_shell_extension(ext_id): Append ext_id to enabled-extensions list (idempotent).
#   - install_gnome_ext_zip(url, dest_dir, sha256, ext_id): Download zip, extract, chown.
#   - install_gnome_ext_from_src(src_dir, dest_dir, ext_id): Copy from source directory.
#   - patch_extension_metadata(ext_dir, metadata_json, shell_ver): Patch extension metadata.json.
#
# Sourced after lib/helpers.sh and lib/gsettings_helpers.sh.
# Depends on: msg, run_as_target, append_gsettings_list, need_cmd, apt_install, SCRIPT_DIR.

# ── Enable GNOME Shell extension (idempotent) ────────────────────────────────

# enable_shell_extension EXT_ID — Append an extension UUID to enabled-extensions.
#   Idempotent: does not add duplicates via Python ast.literal_eval parsing.
#   Args:
#     $1: Extension UUID (e.g., "dash-to-panel@jderose9.github.com")
enable_shell_extension() {
  local ext_id="$1"
  append_gsettings_list org.gnome.shell enabled-extensions "$ext_id"
}

# ── Extension installation helpers ───────────────────────────────────────────

# install_gnome_ext_zip URL DEST_DIR [SHA256] EXT_ID — Download a GNOME extension zip,
# extract it to DEST_DIR, and fix ownership.
#   Args:
#     $1: Download URL (e.g., https://extensions.gnome.org/...zip)
#     $2: Destination directory for the extracted extension
#     $3: Optional SHA256 checksum to verify
#     $4: Extension ID (for ownership chown)
install_gnome_ext_zip() {
  local url="$1" dest_dir="$2" sha256="${3:-}" ext_id="${4:-}"
  local tmpdir zip_file

  apt_install curl unzip

  tmpdir="$(mktemp -d)"
  zip_file="$tmpdir/$(basename "$url")"

  msg "Downloading GNOME extension from $url"
  curl -fL "$url" -o "$zip_file"

  if [ -n "$sha256" ]; then
    printf "%s  %s\n" "$sha256" "$zip_file" | sha256sum -c -
  fi

  run_as_target rm -rf "$dest_dir"
  mkdir -p "$(dirname "$dest_dir")"
  unzip -qo "$zip_file" -d "$(dirname "$dest_dir")"

  # Find the extracted directory (may have a different name than ext_id)
  local extracted_dir
  extracted_dir="$(find "$(dirname "$dest_dir")" -maxdepth 1 -type d -name "*${ext_id}*" | head -1)"
  if [ -n "$extracted_dir" ] && [ "$extracted_dir" != "$dest_dir" ]; then
    mv "$extracted_dir" "$dest_dir"
  fi

  run_as_target chown -R "${TARGET_USER}:${TARGET_USER}" "$dest_dir"
}

# install_gnome_ext_from_src SRC_DIR DEST_DIR EXT_ID — Copy an extension from a source directory.
#   Args:
#     $1: Source directory containing the extension files
#     $2: Destination directory (e.g., ~/.local/share/gnome-shell/extensions/)
#     $3: Extension ID (for ownership chown)
install_gnome_ext_from_src() {
  local src_dir="$1" dest_dir="$2" ext_id="${3:-}"

  msg "Installing GNOME extension from source: $src_dir → $dest_dir"

  run_as_target rm -rf "$dest_dir"
  mkdir -p "$(dirname "$dest_dir")"
  cp -a "$src_dir"/. "$dest_dir"

  if [ -n "$ext_id" ]; then
    run_as_target chown -R "${TARGET_USER}:${TARGET_USER}" "$dest_dir"
  fi
}

# patch_extension_metadata EXT_DIR [METADATA_FILE] SHELL_VER — Patch extension metadata.json.
#   Runs the Python patcher script to update shell-version compatibility.
#   Args:
#     $1: Extension directory containing metadata.json
#     $2: Optional metadata filename (default: metadata.json)
#     $3: GNOME Shell version string
#   Returns:
#     0 on success, 1 on failure or missing file.
patch_extension_metadata() {
  local ext_dir="$1" meta_file="${2:-metadata.json}" shell_ver="$3"

  if [ ! -f "$ext_dir/$meta_file" ]; then
    msg "WARNING: metadata.json not found at $ext_dir/$meta_file; skipping patch."
    return 1
  fi

  if python3 "$SCRIPT_DIR/scripts/patch_extension_metadata.py" \
     "$ext_dir/$meta_file" "$shell_ver"; then
    msg "Patched extension metadata for GNOME Shell ${shell_ver}"
    return 0
  else
    echo "WARNING: failed to patch $meta_file in $ext_dir" >&2
    return 1
  fi
}
