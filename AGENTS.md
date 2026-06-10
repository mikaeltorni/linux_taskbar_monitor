# Clean Installation Compatibility

All repository changes must remain compatible with a clean installation run
through `installation_scripts/install.sh` and this repository's `install.sh`.
Do not rely on packages, files, settings, or manual steps that exist only on
the current machine. Add every required dependency, asset, configuration
step, and migration to the installer so a fresh checkout can reproduce the
complete setup.

Keep installation steps idempotent and verify the clean-install path for every
change.

## Running Without Sudo

The GNOME Shell extension files live in `~/.local/share/gnome-shell/extensions/`
and are owned by the user — no root is needed to patch or reconfigure them.

### Apply a single setting change (no sudo)

```bash
gsettings set org.gnome.shell.extensions.resource-monitor netunit "'bits'"
gsettings set org.gnome.shell.extensions.resource-monitor netunitmeasure "'m'"
```

### Re-apply all patches without sudo

```bash
# Disable the extension first so changes take effect on reload
gnome-extensions disable Resource_Monitor@Ory0n

# Run each patcher directly (no sudo needed — files are user-owned)
node scripts/patch_resource_monitor_vram.js \
  ~/.local/share/gnome-shell/extensions/Resource_Monitor@Ory0n/panel/containers.js
node scripts/patch_resource_monitor_disk.js \
  ~/.local/share/gnome-shell/extensions/Resource_Monitor@Ory0n/panel/containers.js
node scripts/patch_resource_monitor_colors.js \
  ~/.local/share/gnome-shell/extensions/Resource_Monitor@Ory0n/extension.js
python3 scripts/patch_resource_monitor_refresh.py \
  ~/.local/share/gnome-shell/extensions/Resource_Monitor@Ory0n

# Re-enable the extension
gnome-extensions enable Resource_Monitor@Ory0n
```

### Apply gsettings without sudo

All `gsettings` commands work as the logged-in user — no `sudo` or `run_as_target`:

```bash
gsettings set org.gnome.shell.extensions.resource-monitor netunit "'bits'"
gsettings set org.gnome.shell.extensions.resource-monitor netunitmeasure "'m'"
# ... any other gsettings key
```

### When sudo IS required

- Installing the extension zip to system-wide locations (`/usr/share/gnome-shell/extensions/`)
- Modifying files owned by root (e.g., `/etc/`, `/usr/lib/`)
- Running `apt install` or system package management

For routine development and patching, **no sudo is needed**.

## Reloading GNOME Shell

When a change needs GNOME Shell to reload (extensions, themes, shell-side
configuration), reload it in place — never log the user out or terminate the
session.

- X11 session: run
  `busctl --user call org.gnome.Shell /org/gnome/Shell org.gnome.Shell Eval s 'Meta.restart("Restarting…")'`
  (equivalent to pressing Alt+F2 → `r`). This restarts gnome-shell while
  preserving the user's open windows and applications.
- Wayland session: there is no in-place reload. Ask the user to log out and
  back in themselves. Do NOT initiate it.

Never use any of the following to force changes through, regardless of session
type:

- `gnome-session-quit` / `--logout` / `--force`
- `loginctl terminate-session …` / `loginctl kill-user …`
- `systemctl --user stop gnome-session*` or similar unit stops
- `pkill -HUP gnome-shell`, `killall gnome-shell`, or any signal that ends the
  shell process on Wayland

Losing the user's open work is a worse outcome than waiting for them to reload
manually.
