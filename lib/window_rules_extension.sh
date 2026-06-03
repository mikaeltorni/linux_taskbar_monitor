#!/usr/bin/env bash
# window_rules_extension.sh — GNOME Shell window rules extension (Wayland workspace/sticky)
#
# Components:
#   - configure_window_rules_extension(): Install and enable the app-rules@local extension.
#     Creates a GNOME Shell extension that assigns specific apps to workspaces or makes them sticky.
#
# Sourced after lib/helpers.sh, lib/gsettings_helpers.sh, and lib/extension_installation.sh.
# Depends on: msg, run_as_target, enable_shell_extension, SESSION_TYPE, TARGET_HOME, TARGET_USER

# ── configure_window_rules_extension ─────────────────────────────────────────
# Install the local GNOME Shell window rules extension for Wayland sessions.
# This extension assigns specific applications to workspaces or marks them sticky.
#
# Behavior:
#   - Skips installation on X11/XOrg sessions (only works on Wayland).
#   - Creates metadata.json, extension.js in ~/.local/share/gnome-shell/extensions/app-rules@local/
#   - Enables the extension via enable_shell_extension().
#
# Rules defined in the extension:
#   gitkraken*              → workspace 2
#   monkeytype.desktop      → workspace 7
#   chatgpt.desktop         → workspace 0
#   obs*                    → workspace 7
#   discord*                → sticky
#   spotify*                → sticky

configure_window_rules_extension() {
  msg "Installing local GNOME Shell window rules (Wayland workspace/sticky)"

  if [[ "$SESSION_TYPE" =~ ^(x11|xorg)$ ]]; then
    msg "X11 session detected; skipping custom window-rules extension."
    return 0
  fi

  local ext_id="app-rules@local"
  local ext_dir="$TARGET_HOME/.local/share/gnome-shell/extensions/$ext_id"

  run_as_target mkdir -p "$ext_dir"
  run_as_target tee "$ext_dir/metadata.json" >/dev/null <<'EOF'
{
  "uuid": "app-rules@local",
  "name": "App Window Rules",
  "description": "Assign workspaces/sticky windows for specific apps",
  "shell-version": ["45", "46", "47", "48", "49", "50"],
  "version": 2
}
EOF

  run_as_target tee "$ext_dir/extension.js" >/dev/null <<'EOF'
import Shell from 'gi://Shell';
import Meta from 'gi://Meta';
import { Extension } from 'resource:///org/gnome/shell/extensions/extension.js';

const RULES = [
  { appIds: ['gitkraken.desktop', 'gitkraken_gitkraken.desktop'], workspace: 2 },
  { appIds: ['monkeytype.desktop'], workspace: 7 },
  { appIds: ['chatgpt.desktop'], workspace: 0 },
  { appIds: ['com.obsproject.Studio.desktop', 'obs.desktop'], workspace: 7 },
  { appIds: ['discord_discord.desktop'], sticky: true },
  { appIds: ['spotify.desktop', 'spotify_spotify.desktop'], sticky: true },
];

function getAppId(win) {
  try {
    const tracker = Shell.WindowTracker.get_default();
    const app = tracker.get_window_app(win);
    return app ? app.get_id() : null;
  } catch (e) {
    return null;
  }
}

function applyRule(win, rule) {
  if (!win || win.skip_taskbar) {
    return;
  }

  if (rule.workspace !== undefined) {
    const targetIndex = Math.max(0, rule.workspace);
    const wsMgr = global.workspace_manager;
    if (wsMgr.n_workspaces > targetIndex) {
      const ws = wsMgr.get_workspace_by_index(targetIndex);
      if (ws) {
        win.change_workspace(ws);
      }
    }
  }

  if (rule.sticky && typeof win.stick === 'function') {
    try {
      win.stick();
    } catch (e) {
      // ignore
    }
  }
}

function matchAndApply(win) {
  const appId = (getAppId(win) || '').toLowerCase();
  if (!appId) {
    return;
  }

  for (const rule of RULES) {
    if (rule.appIds.some((id) => appId === id.toLowerCase())) {
      applyRule(win, rule);
      break;
    }
  }
}

export default class AppRulesExtension extends Extension {
  enable() {
    this._sig = global.display.connect('window-created', (_disp, win) => {
      matchAndApply(win);
    });

    global.get_window_actors()
      .map((a) => a.meta_window)
      .forEach((w) => matchAndApply(w));
  }

  disable() {
    if (this._sig) {
      global.display.disconnect(this._sig);
      this._sig = null;
    }
  }
}
EOF

  enable_shell_extension "$ext_id"
}
