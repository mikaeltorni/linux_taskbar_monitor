#!/usr/bin/env bash
# window_manager.sh — Window management and tiling configuration
#
# Components:
#   - install_auto_move_windows_extension(): Install auto-move-windows GNOME extension.
#   - configure_auto_move_windows(): Configure workspace rules via auto-move-windows.
#
# Must be sourced after SCRIPT_DIR, CONFIG_DIR, TARGET_USER, TARGET_HOME are set.


install_auto_move_windows_extension() {
  local ext_id="auto-move-windows@gnome-shell-extensions.gcampax.github.com"
  local ext_dir="$TARGET_HOME/.local/share/gnome-shell/extensions/$ext_id"
  local system_ext_dir="/usr/share/gnome-shell/extensions/$ext_id"
  if [ -d "$ext_dir" ] && [ -f "$ext_dir/metadata.json" ]; then
    return 0
  fi
  if [ -d "$system_ext_dir" ] && [ -f "$system_ext_dir/metadata.json" ]; then
    return 0
  fi
  if apt-cache show gnome-shell-extension-auto-move-windows >/dev/null 2>&1; then
    apt_install gnome-shell-extension-auto-move-windows || true
    [ -d "$system_ext_dir" ] || [ -d "$ext_dir" ] && return 0
  fi
  local shell_major branch tmpdir src
  shell_major="$(run_as_target gnome-shell --version 2>/dev/null | grep -oE '[0-9]+' | head -n1 || true)"
  shell_major="${shell_major:-46}"
  branch="gnome-${shell_major}"
  apt_install git
  tmpdir="$(mktemp -d)"
  for repo in \
    "${AUTO_MOVE_WINDOWS_GIT_URL:-https://gitlab.gnome.org/GNOME/gnome-shell-extensions.git}" \
    "https://github.com/GNOME/gnome-shell-extensions.git"; do
    if git clone --depth 1 --branch "$branch" "$repo" "$tmpdir/gse" 2>/dev/null; then
      src="$tmpdir/gse/extensions/$ext_id"
      if [ -d "$src" ]; then
        run_as_target mkdir -p "$(dirname "$ext_dir")"
        run_as_target rm -rf "$ext_dir"
        run_as_target cp -a "$src" "$ext_dir"
        chown -R "$TARGET_USER:$TARGET_USER" "$ext_dir"
        rm -rf "$tmpdir"
        return 0
      fi
    fi
  done
  rm -rf "$tmpdir"
  return 1
}

configure_auto_move_windows() {
  if [[ "$SESSION_TYPE" =~ ^(x11|xorg)$ ]]; then
    msg "X11 session detected; skipping auto-move-windows (Wayland-only path)."
    return 0
  fi
  msg "Configuring workspace rules via auto-move-windows extension (Wayland)"
  local ext_id="auto-move-windows@gnome-shell-extensions.gcampax.github.com"
  if ! install_auto_move_windows_extension; then
    msg "WARNING: auto-move-windows not available on this release; app-rules@local handles workspace placement."
    return 0
  fi
  enable_shell_extension "$ext_id"
  local gitkraken_desktop
  gitkraken_desktop="$(first_existing_desktop gitkraken.desktop gitkraken_gitkraken.desktop)"
  if [ -n "$gitkraken_desktop" ] && gsettings_key_exists org.gnome.shell.extensions.auto-move-windows application-list; then
    append_gsettings_list org.gnome.shell.extensions.auto-move-windows application-list "${gitkraken_desktop}:2"
  elif [ -z "$gitkraken_desktop" ]; then
    msg "GitKraken desktop file not found; skipping auto-move-windows GitKraken rule."
  fi
}
