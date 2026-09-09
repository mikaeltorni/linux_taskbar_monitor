# Clean Installation Compatibility

The `linux-configuration` skill owns the clean-install and root-optional
installer rules; they are not restated here. Repository-specific installer
facts:

This repository's `install.sh` is the primary entry point; the master
orchestrator (`installation_scripts`) lists it optionally. Keep the shared
component CLI contract (`--list-components`, `--select`, `--detect`,
`--reconfigure`, `--uninstall`, and the other readonly flags documented in
`README.md`) stable so that orchestrator keeps working.

User-facing install steps, component tables, `rm-monitor` subcommands, and
environment overrides live in [`README.md`](README.md). Keep that file the
single source of truth for CLI surfaces; update it whenever behavior changes.

## Running Without Sudo

This installer writes Resource Monitor under
`~/.local/share/gnome-shell/extensions/` (user-owned). Patching and
reconfiguring that tree needs no root. `gsettings` also runs as the logged-in
user.

### Apply a single setting change (no sudo)

```bash
gsettings set org.gnome.shell.extensions.resource-monitor netunit "'bits'"
gsettings set org.gnome.shell.extensions.resource-monitor netunitmeasure "'m'"
```

### Re-apply all patches without sudo

```bash
# Disable the extension first to stop callbacks while patching. Disabling and
# re-enabling alone does not reload edited extension source; follow the
# `linux-configuration` skill for the required session-specific reload.
gnome-extensions disable Resource_Monitor@Ory0n

# Build the helper CLI if needed, then run each patcher
bash scripts/build_rm_monitor.sh
EXT=~/.local/share/gnome-shell/extensions/Resource_Monitor@Ory0n
./dist/rm-monitor patch-extension-metadata "$EXT/metadata.json" 46 9999
./dist/rm-monitor patch-refresh "$EXT"
# patch-refresh also syncs the patched schema into ~/.local/share/glib-2.0/schemas.
# If you only recompile the extension schemadir by hand, mirror that sync:
#   SCHEMA_USER=~/.local/share/glib-2.0/schemas
#   mkdir -p "$SCHEMA_USER"
#   cp -f "$EXT/schemas/org.gnome.shell.extensions.resource-monitor.gschema.xml" "$SCHEMA_USER/"
#   glib-compile-schemas "$SCHEMA_USER"
./dist/rm-monitor patch-vram "$EXT/panel/containers.js"
./dist/rm-monitor patch-disk "$EXT/panel/containers.js"
./dist/rm-monitor patch-colors "$EXT/extension.js"
./dist/rm-monitor patch-eth-icon "$EXT/panel/mainGui.js"
./dist/rm-monitor patch-process-popup "$EXT/extension.js"
./dist/rm-monitor patch-stable-width --mode stable "$EXT/panel/containers.js"
./dist/rm-monitor configure-resource-monitor --disk-space-gb --schema-dir "$EXT/schemas"

# Re-enable the extension state after patching
gnome-extensions enable Resource_Monitor@Ory0n
```

See `README.md` for the full `rm-monitor` subcommand table and
`configure-resource-monitor` flags.

### When sudo IS required

- Running `apt install` or other system package management (for example `cargo`
  when no toolchain exists)
- Modifying root-owned paths under `/etc` or `/usr` (not part of this
  installer's normal path — extensions stay under `$HOME/.local`)

For routine development and patching, **no sudo is needed**.

## Shared workflow

This file intentionally contains project-specific ownership, data-safety,
component, integration, and deployment facts. The reusable workflow rules
live in the selected skills and must not be copied into every project file.

Before editing, read this project file, then load:

- `general-programming-guidelines` for the engineering workflow;
- `commits` for capability boundaries and commit verification;
- `worktree` for isolation, branch, merge, and consumer reapplication; and
- `linux-configuration` when the task touches GNOME, desktop settings,
  systemd user services, or any `install.sh`.

Those skills are the cross-project source of truth for workflow, delivery,
desktop deployment, clean-install compatibility, and root-optional installer
rules. Project-specific sections here may narrow ownership or add required
verification, but must not restate or contradict those shared policies.
