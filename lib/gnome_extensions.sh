#!/usr/bin/env bash
# gnome_extensions.sh — Resource Monitor core install and optional patch components.
#
# Components:
#   - ext_gsettings(ext_dir schema args...): Run gsettings with extension schemadir.
#   - install_resource_monitor_core: Download, patch-refresh, configure, enable.
#   - apply_resource_monitor_* / patch_resource_monitor_* / configure_*: selectable
#     components declared in installer/components.sh.
#
# Sourced by install.sh after rm_monitor_bin.sh and extension_installation.sh.
# When loaded alone (pytest / test_lifecycle.sh), source the helpers if missing.
# Depends on: msg, run_as_target, append_gsettings_list, need_cmd, rm_monitor.

if ! declare -F enable_shell_extension >/dev/null 2>&1; then
  source "$SCRIPT_DIR/lib/extension_installation.sh"
fi
if ! declare -F configure_window_rules_extension >/dev/null 2>&1; then
  source "$SCRIPT_DIR/lib/window_rules_extension.sh"
fi

# ── Enable GNOME Shell extension (idempotent) ────────────────────────────────
ext_gsettings() {
  local ext_dir="$1"
  shift
  run_as_target gsettings --schemadir "$ext_dir/schemas" "$@"
}

# RESOURCE_MONITOR_EXT_DIR is published by install_resource_monitor_core so the
# optional patch components (gradient colors, VRAM, per-disk) can locate the
# extracted extension after the mandatory core has installed it.
RESOURCE_MONITOR_EXT_DIR=""

# resource_monitor_refresh_interval_file - Print the target user's persisted
# refresh-interval file. The plain integer stored here is milliseconds.
resource_monitor_refresh_interval_file() {
  printf '%s\n' "$TARGET_HOME/.config/taskbar-system-status-monitor/refresh-interval-ms"
}

# ── Panel spacing mode (stable vs compact) ──────────────────────────────────
# "stable" reserves a tight per-value width so the taskbar does not shift as a
# reading changes digit count. "compact" drops those widths for a narrower
# indicator that shifts slightly. The mode is configurable in the installer and
# persisted like the refresh interval.

# resource_monitor_spacing_file - Print the persisted spacing-mode file path.
resource_monitor_spacing_file() {
  printf '%s\n' "$TARGET_HOME/.config/taskbar-system-status-monitor/panel-spacing-mode"
}

# resource_monitor_spacing_mode - Print the configured spacing mode.
# Prefers a persisted file when present; otherwise honors RESOURCE_MONITOR_SPACING_MODE
# (or defaults to "stable"). Invalid values fall back to "stable".
resource_monitor_spacing_mode() {
  local value="${RESOURCE_MONITOR_SPACING_MODE:-stable}" file
  file="$(resource_monitor_spacing_file)"
  if [ -f "$file" ]; then
    value="$(tr -d '[:space:]' < "$file")"
  fi
  case "$value" in
    stable|compact) ;;
    *) msg "Invalid Resource Monitor spacing mode '$value'; using stable." >&2; value=stable ;;
  esac
  printf '%s\n' "$value"
}

# persist_resource_monitor_spacing_mode MODE - Validate and save the spacing mode.
# Mirrors persist_resource_monitor_refresh_interval naming for the sibling setting.
persist_resource_monitor_spacing_mode() {
  local value="$1" file dir
  case "$value" in
    stable|compact) ;;
    *) msg "Spacing mode must be 'stable' or 'compact'." >&2; return 2 ;;
  esac
  file="$(resource_monitor_spacing_file)"
  dir="$(dirname "$file")"
  run_as_target mkdir -p "$dir"
  printf '%s\n' "$value" | run_as_target tee "$file" >/dev/null
  msg "Saved Resource Monitor panel spacing mode: ${value}."
}

# apply_resource_monitor_width_gsettings EXT_DIR MODE - Set the five upstream
# *width GSettings keys for stable (tight reserved widths) or compact (0).
# Shared by core install and the selectable spacing component so the values
# cannot drift.
apply_resource_monitor_width_gsettings() {
  local ext_dir="$1" mode="$2"
  case "$mode" in
    compact)
      ext_gsettings "$ext_dir" set org.gnome.shell.extensions.resource-monitor cpuwidth 0
      ext_gsettings "$ext_dir" set org.gnome.shell.extensions.resource-monitor ramwidth 0
      ext_gsettings "$ext_dir" set org.gnome.shell.extensions.resource-monitor diskspacewidth 0
      ext_gsettings "$ext_dir" set org.gnome.shell.extensions.resource-monitor netethwidth 0
      ext_gsettings "$ext_dir" set org.gnome.shell.extensions.resource-monitor gpuwidth 0
      ;;
    stable)
      # Sizes match the widest expected reading at the configured units
      # (measured in the panel font, digit ~8px): CPU 0-100 -> 24, RAM GB -> 20,
      # disk free GB -> 36, GPU usage/VRAM split -> 24, ethernet down|up -> 60.
      ext_gsettings "$ext_dir" set org.gnome.shell.extensions.resource-monitor cpuwidth 24
      ext_gsettings "$ext_dir" set org.gnome.shell.extensions.resource-monitor ramwidth 20
      ext_gsettings "$ext_dir" set org.gnome.shell.extensions.resource-monitor diskspacewidth 36
      ext_gsettings "$ext_dir" set org.gnome.shell.extensions.resource-monitor netethwidth 60
      ext_gsettings "$ext_dir" set org.gnome.shell.extensions.resource-monitor gpuwidth 24
      ;;
    *)
      msg "Invalid spacing mode for width GSettings: $mode" >&2
      return 2
      ;;
  esac
}

# apply_resource_monitor_spacing_mode - Apply the configured spacing mode to the
# installed Resource Monitor. Sets the *width GSettings (left to upstream
# defaults when compact) and runs the patch-stable-width CLI in the matching
# mode so the secondary disk-activity reservation (when rm_per_disk applied it)
# and GPU VRAM split follow. Safe before installation: only acts when the
# extension dir is present.
apply_resource_monitor_spacing_mode() {
  local ext_dir mode
  ext_dir="$(resource_monitor_ext_dir)"
  mode="$(resource_monitor_spacing_mode)"
  if [ ! -d "$ext_dir/schemas" ]; then
    msg "Resource Monitor is not installed yet; saved spacing mode will apply during installation."
    return 0
  fi
  msg "Applying Resource Monitor panel spacing mode: ${mode}"
  apply_resource_monitor_width_gsettings "$ext_dir" "$mode"
  ensure_rm_monitor_bin || { msg "rm-monitor unavailable; skipping stable-width patch (build with scripts/build_rm_monitor.sh and re-run)"; return 1; }
  rm_monitor patch-stable-width --mode "$mode" "$(resource_monitor_ext_dir)/panel/containers.js"
  _isc_mark_installed "rm_panel_spacing" || true
}

# configure_resource_monitor_spacing - Open a typeable-choice field for the
# spacing mode, persist it, and apply it live when the extension is installed.
configure_resource_monitor_spacing() {
  local current value
  current="$(resource_monitor_spacing_mode)"
  while true; do
    value=""
    read -r -e -i "$current" -p "Resource Monitor panel spacing [stable|compact]: " value </dev/tty || return 1
    case "$value" in
      stable|compact)
        if persist_resource_monitor_spacing_mode "$value"; then
          apply_resource_monitor_spacing_mode
          return 0
        fi
        ;;
      *) msg "Type 'stable' or 'compact'." >&2 ;;
    esac
  done
}

# resource_monitor_spacing_status - Print the menu-friendly current mode.
resource_monitor_spacing_status() {
  printf '%s\n' "$(resource_monitor_spacing_mode)"
}

# resource_monitor_refresh_interval_ms - Print the configured interval in ms.
# Prefers a persisted file when present; otherwise honors
# RESOURCE_MONITOR_REFRESH_INTERVAL_MS (default 500). Invalid values fall back
# to 500 so installation stays within the supported 100..2000 ms range.
resource_monitor_refresh_interval_ms() {
  local value="${RESOURCE_MONITOR_REFRESH_INTERVAL_MS:-500}" file
  file="$(resource_monitor_refresh_interval_file)"
  if [ -f "$file" ]; then
    value="$(tr -d '[:space:]' < "$file")"
  fi
  if [[ ! "$value" =~ ^[0-9]+$ ]] || (( value < 100 || value > 2000 )); then
    msg "Invalid Resource Monitor update time '$value'; using 500 ms." >&2
    value=500
  fi
  printf '%s\n' "$value"
}

# resource_monitor_refresh_seconds - Convert the configured integer
# milliseconds to the decimal seconds expected by Resource Monitor's schema.
resource_monitor_refresh_seconds() {
  awk -v ms="$(resource_monitor_refresh_interval_ms)" 'BEGIN { printf "%.3f", ms / 1000 }'
}

# persist_resource_monitor_refresh_interval VALUE - Validate and save an
# integer millisecond interval for future standalone and master installer runs.
persist_resource_monitor_refresh_interval() {
  local value="$1" file dir
  if [[ ! "$value" =~ ^[0-9]+$ ]] || (( value < 100 || value > 2000 )); then
    msg "Update time must be a whole number from 100 to 2000 ms." >&2
    return 2
  fi
  file="$(resource_monitor_refresh_interval_file)"
  dir="$(dirname "$file")"
  run_as_target mkdir -p "$dir"
  printf '%s\n' "$value" | run_as_target tee "$file" >/dev/null
  msg "Saved Resource Monitor update time: ${value} ms."
}

# apply_resource_monitor_refresh_interval - Apply the persisted interval to an
# installed Resource Monitor schema. It is safe before installation: the core
# installer will consume the persisted value when it creates the schema.
apply_resource_monitor_refresh_interval() {
  local ext_dir seconds
  ext_dir="$(resource_monitor_ext_dir)"
  seconds="$(resource_monitor_refresh_seconds)"
  if [ -d "$ext_dir/schemas" ]; then
    if ! ext_gsettings "$ext_dir" set org.gnome.shell.extensions.resource-monitor refreshtime "$seconds"; then
      msg "Failed to apply Resource Monitor update time; the installed schema may need reconfiguration." >&2
      return 1
    fi
    msg "Applied Resource Monitor update time: $(resource_monitor_refresh_interval_ms) ms."
    _isc_mark_installed "rm_refresh_interval" || true
  else
    msg "Resource Monitor is not installed yet; saved update time will apply during installation."
  fi
}

# configure_resource_monitor_refresh_interval - Open a typeable field prefilled
# with the current value. Re-prompts until an integer from 100 through 2000 is
# entered, persists it, and applies it live when the schema is installed.
configure_resource_monitor_refresh_interval() {
  local current value
  current="$(resource_monitor_refresh_interval_ms)"
  while true; do
    value=""
    read -r -e -i "$current" -p "Resource Monitor update time in ms (100-2000): " value </dev/tty || return 1
    if persist_resource_monitor_refresh_interval "$value"; then
      apply_resource_monitor_refresh_interval
      return 0
    fi
  done
}

# resource_monitor_refresh_interval_status - Print the menu-friendly current
# interval without changing desktop or repository state.
resource_monitor_refresh_interval_status() {
  printf '%s ms\n' "$(resource_monitor_refresh_interval_ms)"
}

# resource_monitor_ext_dir - Echo the installed Resource Monitor extension dir.
# Falls back to the canonical path derived from the extension id when the core
# has not exported it yet (e.g. a patch component invoked in isolation).
resource_monitor_ext_dir() {
  if [ -n "$RESOURCE_MONITOR_EXT_DIR" ]; then
    printf '%s\n' "$RESOURCE_MONITOR_EXT_DIR"
  else
    printf '%s\n' "$TARGET_HOME/.local/share/gnome-shell/extensions/$RESOURCE_MONITOR_EXTENSION_ID"
  fi
}

# install_resource_monitor_core - Mandatory core: download, verify, extract and
# configure the Resource Monitor extension so the taskbar indicator works on its
# own regardless of which optional components the user selects. The sub-second
# refresh *capability* patch lives here; the refresh *value* is configurable in
# milliseconds through the installer (default 500). The visual/VRAM/per-disk tweaks
# are split into separately selectable components below.
install_resource_monitor_core() {
  msg "Installing Resource Monitor taskbar CPU/RAM/disk/ethernet/GPU indicator (core)"
  need_cmd curl || { msg "ERROR: curl is required to download Resource Monitor"; return 1; }
  need_cmd unzip || { msg "ERROR: unzip is required to extract Resource Monitor"; return 1; }
  need_cmd gsettings || { msg "ERROR: gsettings is required to configure Resource Monitor"; return 1; }
  need_cmd glib-compile-schemas || { msg "ERROR: glib-compile-schemas is required after schema patches"; return 1; }
  need_cmd sha256sum || { msg "ERROR: sha256sum is required to verify the Resource Monitor zip"; return 1; }
  need_cmd gnome-shell || { msg "ERROR: gnome-shell is required to read the running Shell version"; return 1; }

  # Resolve the Shell version BEFORE wiping the live extension tree so a parse
  # failure cannot leave a half-applied (refresh-patched, unpinned) install.
  local shell_version
  shell_version="$(gnome-shell --version 2>/dev/null | awk '{print int($3)}')"
  if [ -z "$shell_version" ] || [ "$shell_version" = "0" ]; then
    msg "ERROR: could not parse GNOME Shell version from 'gnome-shell --version' (needed to pin metadata against EGO overwrite)"
    return 1
  fi

  local ext_id="$RESOURCE_MONITOR_EXTENSION_ID"
  local ext_dir="$TARGET_HOME/.local/share/gnome-shell/extensions/$ext_id"
  local tmpdir zip_file gpu_devices
  tmpdir="$(mktemp -d)"
  zip_file="$tmpdir/resource-monitor.zip"

  curl -fL "$RESOURCE_MONITOR_EXTENSION_URL" -o "$zip_file"
  if [ -n "$RESOURCE_MONITOR_EXTENSION_SHA256" ]; then
    printf "%s  %s\n" "$RESOURCE_MONITOR_EXTENSION_SHA256" "$zip_file" | sha256sum -c -
  fi

  run_as_target rm -rf "$ext_dir"
  run_as_target mkdir -p "$ext_dir"
  unzip -q "$zip_file" -d "$ext_dir"
  if [ "$(id -u)" -eq 0 ]; then
    chown -R "$TARGET_USER:$TARGET_USER" "$ext_dir"
  fi
  rm -rf "$tmpdir"

  # Sub-second refresh capability (schema/type widening + GPU poll floor). The
  # actual interval is applied below from the persisted installer setting.
  rm_monitor patch-refresh "$ext_dir"

  # Pin the version high (9999) so GNOME never auto-updates the EGO-sourced
  # extension over the local patches on shell reload, which previously
  # reverted the gradient colors back to upstream's threshold coloring.
  patch_extension_metadata "$ext_dir" metadata.json "$shell_version" 9999

  ext_gsettings "$ext_dir" set org.gnome.shell.extensions.resource-monitor refreshtime "$(resource_monitor_refresh_seconds)"
  ext_gsettings "$ext_dir" set org.gnome.shell.extensions.resource-monitor extensionposition "'right'"
  ext_gsettings "$ext_dir" set org.gnome.shell.extensions.resource-monitor displaymode "'primary'"
  ext_gsettings "$ext_dir" set org.gnome.shell.extensions.resource-monitor iconsstatus true
  ext_gsettings "$ext_dir" set org.gnome.shell.extensions.resource-monitor itemsposition "['eth', 'cpu', 'ram', 'stats', 'space', 'wlan', 'gpu']"
  ext_gsettings "$ext_dir" set org.gnome.shell.extensions.resource-monitor cpustatus true
  ext_gsettings "$ext_dir" set org.gnome.shell.extensions.resource-monitor cpufrequencystatus false
  ext_gsettings "$ext_dir" set org.gnome.shell.extensions.resource-monitor cpuloadaveragestatus false
  ext_gsettings "$ext_dir" set org.gnome.shell.extensions.resource-monitor ramstatus true
  ext_gsettings "$ext_dir" set org.gnome.shell.extensions.resource-monitor ramunit "'numeric'"
  ext_gsettings "$ext_dir" set org.gnome.shell.extensions.resource-monitor rammonitor "'used'"
  ext_gsettings "$ext_dir" set org.gnome.shell.extensions.resource-monitor swapstatus false
  ext_gsettings "$ext_dir" set org.gnome.shell.extensions.resource-monitor diskstatsstatus false
  ext_gsettings "$ext_dir" set org.gnome.shell.extensions.resource-monitor diskspacestatus true
  rm_monitor configure-resource-monitor \
    --disk-space-gb \
    --schema-dir "$ext_dir/schemas"
  ext_gsettings "$ext_dir" set org.gnome.shell.extensions.resource-monitor netethstatus true
  ext_gsettings "$ext_dir" set org.gnome.shell.extensions.resource-monitor netunit "'bits'"
  ext_gsettings "$ext_dir" set org.gnome.shell.extensions.resource-monitor netunitmeasure "'m'"
  ext_gsettings "$ext_dir" set org.gnome.shell.extensions.resource-monitor netwlanstatus false
  ext_gsettings "$ext_dir" set org.gnome.shell.extensions.resource-monitor gpustatus true
  ext_gsettings "$ext_dir" set org.gnome.shell.extensions.resource-monitor gpumemoryunit "'numeric'"
  ext_gsettings "$ext_dir" set org.gnome.shell.extensions.resource-monitor gpumemoryunitmeasure "'auto'"
  ext_gsettings "$ext_dir" set org.gnome.shell.extensions.resource-monitor gpumemorymonitor "'used'"
  ext_gsettings "$ext_dir" set org.gnome.shell.extensions.resource-monitor gpudisplaydevicename false

  # Reserve a tight per-value width so the taskbar does not jump as metric values
  # change digit count. The extension multiplies these pixel values by the display
  # scale factor and right-aligns every value label, so text grows leftward inside
  # a fixed box while the right edge stays put.
  #
  # The spacing mode selects whether these reserved widths are applied. "stable"
  # keeps the panel put as digits change; "compact" skips them so the indicator
  # takes less horizontal space but shifts slightly as values grow/shrink.
  # The secondary disk-activity width (no upstream GSetting) is handled by the
  # rm_panel_spacing component via rm-monitor patch-stable-width; the spacing
  # mode is passed straight through to it when that component runs.
  apply_resource_monitor_width_gsettings "$ext_dir" "$(resource_monitor_spacing_mode)"

  gpu_devices="$(rm_monitor report-cuda-devices)"
  if [ -n "$gpu_devices" ]; then
    ext_gsettings "$ext_dir" set org.gnome.shell.extensions.resource-monitor gpudeviceslist "$gpu_devices"
  else
    msg "No NVIDIA GPU reported by nvidia-smi; Resource Monitor GPU list left empty."
  fi

  enable_shell_extension "$ext_id"
  RESOURCE_MONITOR_EXT_DIR="$ext_dir"
  msg "Resource Monitor core installed with a $(resource_monitor_refresh_interval_ms) ms refresh interval."
}

# ── Optional Resource Monitor tweaks (selectable components) ──────────────────
# Each patcher edits the extension files extracted by install_resource_monitor_core.
# The core re-extracts a clean copy on every run, so deselecting a tweak on a
# later run reverts it. Re-running with the tweak selected is idempotent (the
# patch scripts skip already-applied edits).

# patch_resource_monitor_gradient_colors - Replace upstream threshold coloring
# with a smooth value-proportional gradient on the panel indicators.
patch_resource_monitor_gradient_colors() {
  msg "Applying Resource Monitor gradient colors patch"
  ensure_rm_monitor_bin || { msg "rm-monitor unavailable; skipping gradient colors patch"; return 1; }
  rm_monitor patch-colors "$(resource_monitor_ext_dir)/extension.js"
  _isc_mark_installed "rm_gradient_colors" || true
}

# patch_resource_monitor_vram - Show GPU VRAM usage without brackets in the panel.
patch_resource_monitor_vram() {
  msg "Applying Resource Monitor VRAM display patch"
  ensure_rm_monitor_bin || { msg "rm-monitor unavailable; skipping VRAM display patch"; return 1; }
  rm_monitor patch-vram "$(resource_monitor_ext_dir)/panel/containers.js"
  _isc_mark_installed "rm_vram" || true
}

# patch_resource_monitor_eth_icon - Remove the ethernet display icon while
# keeping the numeric Mbps value and unit. No upstream GSetting hides a single
# icon, so this source patch drops the eth icon argument at its wiring site.
patch_resource_monitor_eth_icon() {
  msg "Applying Resource Monitor ethernet-icon removal patch"
  ensure_rm_monitor_bin || { msg "rm-monitor unavailable; skipping ethernet-icon patch"; return 1; }
  rm_monitor patch-eth-icon "$(resource_monitor_ext_dir)/panel/mainGui.js"
  _isc_mark_installed "rm_hide_eth_icon" || true
}

# patch_resource_monitor_process_popup - Left-click shows a popup menu with
# total CPU%/RAM% aggregated per process name instead of launching the
# configured task manager (gnome-system-monitor). No upstream GSetting offers
# this, so the source patch rewires _clickManager to an in-panel PopupMenu.
patch_resource_monitor_process_popup() {
  msg "Applying Resource Monitor process-popup (left-click) patch"
  ensure_rm_monitor_bin || { msg "rm-monitor unavailable; skipping process-popup patch"; return 1; }
  rm_monitor patch-process-popup "$(resource_monitor_ext_dir)/extension.js"
  _isc_mark_installed "rm_process_popup" || true
}

# patch_resource_monitor_per_disk - Show each disk device separately in the panel.
patch_resource_monitor_per_disk() {
  msg "Applying Resource Monitor per-disk display patch"
  ensure_rm_monitor_bin || { msg "rm-monitor unavailable; skipping per-disk display patch"; return 1; }
  rm_monitor patch-disk "$(resource_monitor_ext_dir)/panel/containers.js"
  _isc_mark_installed "rm_per_disk" || true
}
