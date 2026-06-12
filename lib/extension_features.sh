#!/usr/bin/env bash
# extension_features.sh — GNOME Shell extension feature configurations
#
# Components:
#   - install_pwa_icons(): Set up Chrome PWA icons and desktop entries.
#   - configure_dash_and_switchers(): Configure dash-to-panel extension settings.
#
# Sourced after lib/helpers.sh, lib/gsettings_helpers.sh, and lib/extension_installation.sh.
# Depends on: msg, run_as_target, enable_shell_extension, remove_gsettings_list, need_cmd, apt_install
# Requires variables: CHROME_PWAS

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

  # Build directory list from collected sizes
  local dir_list=()
  for sz in "${!needed_sizes[@]}"; do
    dir_list+=("${sz}x${sz}")
  done

  if [ ${#dir_list[@]} -gt 0 ]; then
    run_as_target mkdir -p "${icon_theme}/${dir_list[*]}/apps"
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

# ── configure_dash_and_switchers ─────────────────────────────────────────────
# Configure dash-to-panel extension and disable ubuntu-dock.
#
# Installs dash-to-panel if not present, disables the default ubuntu-dock,
# and applies dconf settings for favorites, workspace isolation, and panel behavior.

configure_dash_and_switchers() {
  msg "Configuring dash tweaks"
  local ext_id="dash-to-panel@jderose9.github.com"
  if ! run_as_target bash -lc 'command -v gnome-extensions >/dev/null 2>&1'; then
    msg "gnome-extensions CLI not found; cannot configure dash-to-panel."
    return 0
  fi
  if ! run_as_target bash -lc 'gnome-extensions list | grep -Fxq "$1"' _ "$ext_id"; then
    msg "dash-to-panel NOT installed or GNOME thinks it is incompatible; leaving ubuntu-dock enabled."
    return 0
  fi
  msg "dash-to-panel found, enabling and disabling ubuntu-dock"
  run_as_target gnome-extensions disable ubuntu-dock@ubuntu.com || true
  run_as_target gnome-extensions enable "$ext_id" || true
  enable_shell_extension "$ext_id"
  run_as_target dconf write /org/gnome/shell/extensions/dash-to-panel/show-favorites true || true
  run_as_target dconf write /org/gnome/shell/extensions/dash-to-panel/isolate-workspaces true || true
  run_as_target dconf write /org/gnome/shell/extensions/dash-to-panel/isolate-monitors false || true
  run_as_target dconf write /org/gnome/shell/extensions/dash-to-panel/stockgs-keep-top-panel false || true
  run_as_target dconf write /org/gnome/shell/extensions/dash-to-panel/show-activities-button false || true
  remove_gsettings_list org.gnome.shell enabled-extensions ubuntu-dock@ubuntu.com
}
