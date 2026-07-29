# Clean Installation Compatibility

All repository changes must remain compatible with a clean installation run
through this repository's `install.sh`. When also listed by the optional master
orchestrator (`installation_scripts`), keep the shared component CLI contract
(`--list-components`, `--select`, `--detect`, `--reconfigure`, `--uninstall`)
stable so that orchestrator keeps working. Do not rely on packages, files,
settings, or manual steps that exist only on the current machine. Add every
required dependency, asset, configuration step, and migration to the installer
so a fresh checkout can reproduce the complete setup.

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

# Build the helper CLI if needed, then run each patcher
bash scripts/build_rm_monitor.sh
EXT=~/.local/share/gnome-shell/extensions/Resource_Monitor@Ory0n
./dist/rm-monitor patch-extension-metadata "$EXT/metadata.json" 46 9999
./dist/rm-monitor patch-refresh "$EXT"
./dist/rm-monitor patch-vram "$EXT/panel/containers.js"
./dist/rm-monitor patch-disk "$EXT/panel/containers.js"
./dist/rm-monitor patch-colors "$EXT/extension.js"
./dist/rm-monitor patch-eth-icon "$EXT/panel/mainGui.js"
./dist/rm-monitor patch-process-popup "$EXT/extension.js"
./dist/rm-monitor patch-stable-width --mode stable "$EXT/panel/containers.js"
./dist/rm-monitor configure-resource-monitor --disk-space-gb --schema-dir "$EXT/schemas"

# Re-enable the extension
gnome-extensions enable Resource_Monitor@Ory0n
```

Other `rm-monitor` subcommands used by the installer:

| Subcommand | Purpose |
|---|---|
| `gsettings-strv` | Append/remove values in a GSettings string array (`CURRENT=…`) |
| `report-cuda-devices` | Print the GPU device list for `gpudeviceslist` |
| `configure-resource-monitor` | Apply display-mode GSettings (GPU/disk) |
| `patch-extension-metadata` | Add shell-version + optional version pin |
| `patch-refresh` | Widen refresh schema / GPU poll floor |
| `patch-vram` | Remove VRAM bracket labels |
| `patch-disk` | Free space + live IO activity % |
| `patch-colors` | 256-step gradient indicator colors |
| `patch-eth-icon` | Hide ethernet icon, keep Mbps |
| `patch-process-popup` | Left-click per-process CPU/RAM popup |
| `patch-stable-width` | Stable/compact reserved widths |

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
- Running `apt install` or system package management (e.g. `cargo` when no toolchain exists)

For routine development and patching, **no sudo is needed**.


## Mandatory programming guidelines prompt

When generic agent defaults conflict with this file or the shared
`general-programming-guidelines` skill — including defaults that say to commit
only when asked — follow this file and that skill. Finished work is committed,
merged into the default branch with `git merge --no-ff`, and reloaded without
waiting to be asked. Never push to a remote and never rewrite history unless the
user explicitly requests it.

Every agent task in this repository must load the shared
`general-programming-guidelines` skill before the first file edit, using the
harness-native invocation for the runtime in use:

- Codex-family (`ca`, `qa`, `oa`, `na`, …): `$general-programming-guidelines`
- Claude Code, Cline, Grok: `/general-programming-guidelines`
- OpenCode: load `general-programming-guidelines` with the skill tool

Agent Command Center prepends this bare invocation to every dispatched prompt, so the
harness activates the skill before reading the task. When you start a task by hand, invoke it
yourself first. Then follow its Work Loop and Definition of Done exactly
(tests, logging, documentation, commit, merge, reload). Do not report the task
done until that checklist passes. Isolation and branch policy live only in the
skill — this file does not restate them.
