#!/usr/bin/env bash
# window_rules_extension.sh — GNOME Shell window rules extension (Wayland)
#
# Components:
#   - configure_window_rules_extension(): Install and enable app-rules@local.
#
# Default-off optional component: ships with an empty RULES list so public
# installs do not apply personal desktop policy. Edit RULES in the installed
# extension.js (or this template) before enabling, or pass --select window_rules
# after customizing.
#
# Sourced by install.sh and by lib/gnome_extensions.sh for standalone tests.
# Depends on: msg, run_as_target, enable_shell_extension, SESSION_TYPE,
#             TARGET_HOME, TARGET_USER

# window_rules_skip_marker — Path written when X11 skips install so --detect
# treats the component as satisfied and does not loop forever.
window_rules_skip_marker() {
  printf '%s\n' "$TARGET_HOME/.config/taskbar-system-status-monitor/window-rules-skipped-x11"
}

# configure_window_rules_extension — Install the local window-rules extension.
# Skips on X11/XOrg (Wayland-only). Writes metadata.json + extension.js under
# ~/.local/share/gnome-shell/extensions/app-rules@local/ and enables it.

configure_window_rules_extension() {
  msg "Installing local GNOME Shell window rules (Wayland workspace/sticky)"

  if [[ "$SESSION_TYPE" =~ ^(x11|xorg)$ ]]; then
    msg "X11 session detected; skipping custom window-rules extension."
    local marker dir
    marker="$(window_rules_skip_marker)"
    dir="$(dirname "$marker")"
    run_as_target mkdir -p "$dir"
    printf 'skipped\n' | run_as_target tee "$marker" >/dev/null
    return 0
  fi

  # Installing on Wayland: clear any prior X11 skip marker.
  run_as_target rm -f "$(window_rules_skip_marker)" 2>/dev/null || true

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

// Empty by default — add entries such as:
//   { appIds: ['org.example.App.desktop'], workspace: 1 },
//   { appIds: ['spotify.desktop'], sticky: true },
const RULES = [];

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
