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


## Mandatory programming guidelines prompt

Every agent task in this repository must load the shared
`general-programming-guidelines` skill before the first file edit, using the
harness-native invocation for the runtime in use:

- Codex-family (`ca`, `qa`, `oa`, `na`, …): `$general-programming-guidelines`
- Claude Code, Cline, Grok: `/general-programming-guidelines`
- OpenCode: load `general-programming-guidelines` with the skill tool

Agent Command Center prepends this bare invocation to every dispatched prompt, so the
harness activates the skill before reading the task. When you start a task by hand, invoke it
yourself first. Then follow its Work Loop — dedicated worktree branch before
the first edit, tests, logging, documentation — and do not report the task done
until its Definition of Done checklist passes.
