#!/usr/bin/env bash
# extension_installation.sh — GNOME Shell extension enable + metadata helpers
#
# Components:
#   - enable_shell_extension(ext_id): Append ext_id to enabled-extensions (idempotent).
#   - patch_extension_metadata(ext_dir, metadata_json, shell_ver, [pin]): Patch
#     metadata.json shell-version (and optional version pin) via rm-monitor.
#
# Sourced by install.sh and by lib/gnome_extensions.sh for standalone tests.
# Depends on: msg, append_gsettings_list, rm_monitor.

# enable_shell_extension EXT_ID — Append an extension UUID to enabled-extensions.
#   Idempotent: rm-monitor gsettings-strv append skips duplicates.
#   Args:
#     $1: Extension UUID (e.g. "Resource_Monitor@Ory0n")
enable_shell_extension() {
  local ext_id="$1"
  append_gsettings_list org.gnome.shell enabled-extensions "$ext_id"
}

# patch_extension_metadata EXT_DIR [METADATA_FILE] SHELL_VER [PIN_VERSION] — Patch
# extension metadata.json via rm-monitor patch-extension-metadata.
#   Updates shell-version compatibility and, when PIN_VERSION is given, raises
#   the "version" field above upstream so GNOME never auto-updates over local
#   patches on shell reload.
#   Args:
#     $1: Extension directory containing metadata.json
#     $2: Optional metadata filename (default: metadata.json)
#     $3: GNOME Shell version string
#     $4: Optional version-pin integer (e.g. 9999)
#   Returns:
#     0 on success, 1 on failure or missing file.
patch_extension_metadata() {
  local ext_dir="$1" meta_file="${2:-metadata.json}" shell_ver="$3" pin_ver="${4:-}"

  if [ ! -f "$ext_dir/$meta_file" ]; then
    msg "ERROR: metadata.json not found at $ext_dir/$meta_file; cannot pin against EGO overwrite."
    return 1
  fi

  if rm_monitor patch-extension-metadata \
     "$ext_dir/$meta_file" "$shell_ver" ${pin_ver:+"$pin_ver"}; then
    msg "Patched extension metadata for GNOME Shell ${shell_ver}"
    return 0
  else
    msg "ERROR: failed to patch $meta_file in $ext_dir"
    return 1
  fi
}
