#!/usr/bin/env bash
# extension_features.sh — GNOME Shell extension feature configurations
#
# Components:
#   - install_pwa_icons(): Set up Chrome PWA icons and desktop entries.
#   - configure_dash_and_switchers(): Configure dash-to-panel extension settings.
#
# Sourced after lib/helpers.sh, lib/gsettings_helpers.sh, and lib/extension_installation.sh.
# Depends on: msg, run_as_target, enable_shell_extension, remove_gsettings_list,
#   need_cmd, apt_install, install_gnome_ext_zip, patch_extension_metadata
# Requires variables: CHROME_PWAS, DASH_TO_PANEL_EXTENSION_URL,
#   DASH_TO_PANEL_EXTENSION_SHA256

# ── install_pwa_icons ────────────────────────────────────────────────────────
# Set up Chrome PWA (Progressive Web App) icons and desktop entries.
#
# Scans Chrome's Web Applications Manifest Resources directory for icon files,
# creates symlinks in the hicolor icon theme, and generates .desktop files
# for each PWA that doesn't already have one.
#
# Requires:
#   CHROME_PWAS: Array of pipe-delimited entries (app_id|name|desktop_file|workspace)

install_pwa_icons() {
  msg "Setting up Chrome PWA icons and desktop entries"

  local pwa_dir="$TARGET_HOME/.config/google-chrome/Default/Web Applications/Manifest Resources"
  local icon_base="$TARGET_HOME/.local/share/icons"
  local icon_theme="${icon_base}/hicolor"

  if [ ! -d "$pwa_dir" ]; then
    msg "Chrome Web Applications directory not found; skipping PWA icon setup."
    return 0
  fi

  # Fix ownership of icons directory if it's owned by root (common after initial install)
  local icons_owner
  icons_owner="$(stat -c '%U' "$icon_base" 2>/dev/null || echo unknown)"
  if [ "$icons_owner" = "root" ]; then
    if [ "$(id -u)" -eq 0 ]; then
      msg "Fixing ownership of $icon_base to $TARGET_USER"
      chown "$TARGET_USER:$TARGET_USER" "$icon_base"
    else
      msg "WARN: $icon_base is root-owned; run with sudo once to fix ownership"
    fi
  fi

  # Collect all icon sizes from Chrome PWA folders, then create directories for them
  local -A needed_sizes=()
  for entry in "${CHROME_PWAS[@]}"; do
    IFS='|' read -r app_id name desktop_file workspace <<< "$entry"
    local icons_dir="$pwa_dir/$app_id/Icons"
    [ -d "$icons_dir" ] || continue
    for icon_file in "$icons_dir"/*.png; do
      [ -f "$icon_file" ] || continue
      local size
      size="$(basename "$icon_file" .png)"
      needed_sizes["$size"]=1
    done
  done

  if [ ${#needed_sizes[@]} -gt 0 ]; then
    local created_dirs=0
    for sz in "${!needed_sizes[@]}"; do
      run_as_target mkdir -p "${icon_theme}/${sz}x${sz}/apps"
      created_dirs=$((created_dirs + 1))
    done
    msg "  prepared $created_dirs icon theme size directories"
  fi

  # Create symlinks for each PWA icon
  for entry in "${CHROME_PWAS[@]}"; do
    IFS='|' read -r app_id name desktop_file workspace <<< "$entry"

    local icons_dir="$pwa_dir/$app_id/Icons"
    if [ ! -d "$icons_dir" ]; then
      msg "  $name: icon directory not found at $icons_dir (PWA may not be installed yet)"
      continue
    fi

    # Create symlinks for each available icon size
    local linked=0
    for icon_file in "$icons_dir"/*.png; do
      [ -f "$icon_file" ] || continue
      local size
      size="$(basename "$icon_file" .png)"
      run_as_target ln -sf "$icon_file" "${icon_theme}/${size}x${size}/apps/chrome-${app_id}-Default.png"
      linked=$((linked + 1))
    done

    if [ "$linked" -gt 0 ]; then
      msg "  $name: linked $linked icon sizes"
    fi

    # Create desktop entry if it doesn't exist yet
    local desktop_path="$TARGET_HOME/.local/share/applications/chrome-${app_id}-Default.desktop"
    if [ ! -f "$desktop_path" ]; then
      run_as_target mkdir -p "$(dirname "$desktop_path")"
      run_as_target tee "$desktop_path" >/dev/null <<DESKTOP
[Desktop Entry]
Version=1.0
Terminal=false
Type=Application
Name=$name
Exec=/opt/google/chrome/google-chrome --profile-directory=Default --app-id=$app_id
Icon=chrome-${app_id}-Default
StartupWMClass=crx_${app_id}
DESKTOP
      msg "  $name: created desktop entry at $(basename "$desktop_path")"
    fi
  done

  # Refresh icon cache
  run_as_target gtk-update-icon-cache --force "${icon_theme}" 2>/dev/null || true
}

# ── dash_to_panel_installed ──────────────────────────────────────────────────
# True when the dash-to-panel extension is present on disk (target user's local
# extensions dir or a system-wide dir). Presence is checked on disk rather than
# via `gnome-extensions list`, because a freshly installed extension does not
# appear in the live list until GNOME Shell is reloaded — which deployment must
# never force.
dash_to_panel_installed() {
  local ext_id="$1"
  [ -f "$TARGET_HOME/.local/share/gnome-shell/extensions/$ext_id/metadata.json" ] && return 0
  [ -f "/usr/share/gnome-shell/extensions/$ext_id/metadata.json" ] && return 0
  return 1
}

# ── install_dash_to_panel ─────────────────────────────────────────────────────
# Download and install the pinned dash-to-panel EGO build into the target user's
# local extensions directory, patching its metadata for the running GNOME Shell
# and pinning the version high so a shell reload never auto-updates over it.
install_dash_to_panel() {
  local ext_id="$1"
  local ext_dir="$TARGET_HOME/.local/share/gnome-shell/extensions/$ext_id"
  local shell_version

  msg "dash-to-panel not installed; downloading pinned EGO build"
  install_gnome_ext_zip \
    "$DASH_TO_PANEL_EXTENSION_URL" "$ext_dir" \
    "$DASH_TO_PANEL_EXTENSION_SHA256" "$ext_id"

  shell_version="$(gnome-shell --version 2>/dev/null | awk '{print int($3)}')"
  if [ -n "$shell_version" ]; then
    patch_extension_metadata "$ext_dir" metadata.json "$shell_version" 9999 || true
  fi
  msg "dash-to-panel installed at $ext_dir"
}

# ── configure_dash_and_switchers ─────────────────────────────────────────────
# Configure dash-to-panel extension and disable ubuntu-dock.
#
# Installs dash-to-panel if not present, disables the default ubuntu-dock,
# and applies dconf settings for favorites, workspace isolation, and panel behavior.

configure_dash_and_switchers() {
  msg "Configuring dash tweaks"
  local ext_id="dash-to-panel@jderose9.github.com"
  if ! dash_to_panel_installed "$ext_id"; then
    install_dash_to_panel "$ext_id"
  fi
  if ! dash_to_panel_installed "$ext_id"; then
    msg "dash-to-panel install failed; leaving ubuntu-dock enabled."
    return 0
  fi
  msg "dash-to-panel present, enabling and disabling ubuntu-dock"
  # These are synchronous D-Bus calls into GNOME Shell that can hang for minutes
  # on a fresh install or an unresponsive session (the `|| true` only catches an
  # error exit, not a hang), so dispatch each in the BACKGROUND, time-boxed and
  # with stdio detached, so they can never block the installer. Persistent state
  # is handled by enable_shell_extension (gsettings) below.
  run_as_target sh -c 'timeout 5 gnome-extensions disable ubuntu-dock@ubuntu.com >/dev/null 2>&1 </dev/null &' || true
  run_as_target sh -c 'timeout 5 gnome-extensions enable "$1" >/dev/null 2>&1 </dev/null &' _ "$ext_id" || true
  enable_shell_extension "$ext_id"
  run_as_target dconf write /org/gnome/shell/extensions/dash-to-panel/show-favorites true || true
  run_as_target dconf write /org/gnome/shell/extensions/dash-to-panel/isolate-workspaces true || true
  run_as_target dconf write /org/gnome/shell/extensions/dash-to-panel/isolate-monitors false || true
  run_as_target dconf write /org/gnome/shell/extensions/dash-to-panel/stockgs-keep-top-panel false || true
  run_as_target dconf write /org/gnome/shell/extensions/dash-to-panel/show-activities-button false || true
  remove_gsettings_list org.gnome.shell enabled-extensions ubuntu-dock@ubuntu.com
}
