# Linux Taskbar Monitor

Standalone installer that patches the GNOME Shell **Resource Monitor** extension
into a compact taskbar status strip (CPU, RAM, `/home` disk usage/activity,
ethernet, GPU/VRAM).

## Supported platforms

**Tested on:** Ubuntu 24.04 LTS with GNOME Shell 46 (the version shipped by that
release).

**Not guaranteed elsewhere.** Other Ubuntu versions, other distributions, or
other GNOME Shell major versions may work if they can run Resource Monitor v27
and accept the same patch shapes, but they are untested. Treat anything outside
Ubuntu 24.04 + GNOME 46 as best-effort: run the installer on a throwaway session
first, and expect patch or schema mismatches if upstream Resource Monitor or
Shell APIs differ.

This repository is **fully standalone**: `bash install.sh` is enough. Soft
loading of
[`linux_installation_scripts_functions`](https://github.com/mikaeltorni/linux_installation_scripts_functions)
(sibling checkout or on-demand download) powers the interactive component menu
and config export. When that framework is unreachable (private raw URL, offline
host, or no sibling), the installer loads a built-in fallback that still
supports `--default` / `--all` / `--select` / `--list-components` / `--detect` /
`--reconfigure` / `--uninstall`. Set `ISC_FUNCTIONS_DIR` to an explicit checkout
to force the full framework, or to a missing path to force the fallback.

> **Rename note:** the intended GitHub repository name is `linux_taskbar_monitor`.
> Until that rename lands, clones and orchestrator entries may still use the
> historical directory name `ubuntu_2404_taskbar_system_status_monitor`. Machine
> identifiers (`ISC_REPO_NAME`, `installation_configs` `repo` field) stay on the
> historical name until the GitHub rename and orchestrator update happen together.

## What it installs

It downloads Resource Monitor v27 and configures the panel for CPU, RAM,
`/home` disk usage/activity, ethernet, and GPU status. The mandatory core also
enables configurable 100–2000 ms refreshes (500 ms by default). Optional
default-on components then patch GPU VRAM display, per-disk rows, gradient
colors, ethernet icon hide, process popup, and panel spacing — unless you
deselect them.

## Technology Stack

- Bash `install.sh` for GNOME Shell integration and the shared component menu.
- Rust `rm-monitor` CLI (Cargo) for every patch, GSettings helper, and GPU/disk
  discovery step — no Node.js or Python runtime required at install time.
- Optional Docker/Podman Rust image when local `cargo` is unavailable
  (`scripts/build_rm_monitor.sh`).

## Installation

```bash
bash install.sh                 # user-level; apt steps skipped and reported
sudo bash install.sh            # also apt-installs missing packages (e.g. cargo)
```

The installer builds `dist/rm-monitor` on demand (local cargo, apt `cargo`, or
container), downloads Resource Monitor, applies patches, and writes GSettings.
Core install requires a running `gnome-shell` whose version can be parsed: that
value is pinned into `metadata.json` (version `9999`) so extensions.gnome.org
cannot overwrite local patches on reload. Extract, refresh patch, and metadata
pin run in a staging directory first; the live extension tree is replaced only
after those steps succeed, so a patch/pin failure leaves a previous install
intact. Later GSettings/enable failures can still leave a freshly published
tree that needs a re-run.

After installation, log out and back in before testing GNOME Shell extension
changes (on X11, agents may use the sanctioned in-place Shell reload instead).

## Development Workflow

```bash
# Rust unit/integration tests (primary — covers every patcher and helper)
source "$HOME/.cargo/env"   # if using rustup
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release --bin rm-monitor

# Installer contract / shell checks
bash -n install.sh
bash scripts/build_rm_monitor.sh
bash tests/test_lifecycle.sh
python3 -m pytest tests -q
./install.sh --list-components
./install.sh --list-configurable-components
./install.sh --detect
```

## Project Structure

- `install.sh` — downloads, patches, configures, and enables Resource Monitor.
- `src/` — Rust sources for the `rm-monitor` CLI (see `rm-monitor --help`).
- `scripts/build_rm_monitor.sh` — build into `dist/rm-monitor` (cargo or container).
- `lib/` — Bash installer modules (extension install, patch wrappers, lifecycle).
- `installer/components.sh` — selectable component manifest for the shared menu.
- `installation_configs/` — default/empty selection snapshots for the orchestrator.
- `tests/` — installer contract tests; patch behavior is covered by `cargo test`.

### `rm-monitor` subcommands

| Subcommand | Role |
|---|---|
| `gsettings-strv` | Append/remove values in a GSettings `as` list (`CURRENT=…`) |
| `report-cuda-devices` | NVIDIA GPU device list for `gpudeviceslist` |
| `configure-resource-monitor` | Display-mode GSettings (GPU/disk); see flags below |
| `patch-extension-metadata` | Shell version + optional version pin |
| `patch-refresh` | Sub-second refresh capability |
| `patch-vram` | Remove VRAM bracket labels |
| `patch-disk` | Free space + live IO activity % |
| `patch-colors` | 256-step gradient indicator colors |
| `patch-eth-icon` | Hide ethernet icon, keep Mbps |
| `patch-process-popup` | Left-click per-process CPU/RAM popup |
| `patch-stable-width` | `--mode stable\|compact` reserved widths |

`configure-resource-monitor` flags:

| Flag | Effect |
|---|---|
| `--disk-space-gb` | Free space in GB for `/home` only (installer default) |
| `--disk-space-perc-home-only` | `/home` used percentage |
| `--gpu-memory-perc` | GPU memory as used/total % |
| `--schema-dir PATH` | Extension schemas directory (auto-detected when omitted) |

`--disk-space-perc` remains as a hidden legacy alias for `--disk-space-gb`.
Passing `--disk-space-gb` alone does not change `gpumemoryunit`; pass
`--gpu-memory-perc` when you want GPU memory shown as a percentage. The
installer sets `gpumemoryunit` to numeric itself after configure.

## Configuration

Environment overrides (used when no persisted file exists yet):

| Variable | Purpose |
|---|---|
| `RESOURCE_MONITOR_EXTENSION_ID` | Extension UUID (default `Resource_Monitor@Ory0n`) |
| `RESOURCE_MONITOR_EXTENSION_URL` | EGO zip URL |
| `RESOURCE_MONITOR_EXTENSION_SHA256` | Zip checksum |
| `RESOURCE_MONITOR_SPACING_MODE` | `stable` or `compact` (seed; file wins if present) |
| `RESOURCE_MONITOR_REFRESH_INTERVAL_MS` | 100–2000 (default 500; file wins if present) |
| `RM_MONITOR_RUST_IMAGE` | Container image for builds (default `rust:1-bookworm`) |
| `ISC_FUNCTIONS_DIR` | Explicit framework checkout (exclusive when set; missing path forces built-in fallback) |
| `ISC_FUNCTIONS_REF` | Git ref for the on-demand framework download (default `master`) |

Persisted under `~/.config/taskbar-system-status-monitor/` once chosen in the
installer menu (`refresh-interval-ms`, `panel-spacing-mode`).

### Panel spacing (stable vs compact)

Every Resource Monitor value label is right-aligned and given a tight reserved
pixel width in **stable** mode so the panel does not jump when digit counts
change. **Compact** mode drops those widths for a narrower indicator. Choose
the mode via the installer's `rm_panel_spacing` component (or
`RESOURCE_MONITOR_SPACING_MODE` / the persisted file under
`~/.config/taskbar-system-status-monitor/`).

The five upstream `*width` GSettings keys are applied by the mandatory core
(and again when the spacing component runs). The secondary disk-activity and
VRAM split reservations that have no upstream GSetting are applied by
`rm-monitor patch-stable-width` when `rm_panel_spacing` is selected.

Ethernet is placed first (leftmost). The ethernet icon is hidden while Mbps
values stay visible. Disk free space shows as colored GB with a secondary
activity %.

## Component selection

```bash
bash install.sh                 # interactive component selection (TTY), else defaults
bash install.sh --default       # install all default-on components, no prompts
bash install.sh --all           # install every component, no prompts
bash install.sh --select a,b    # install exactly these component ids
bash install.sh --list-components  # print: id<TAB>label<TAB>default
bash install.sh --list-configurable-components
bash install.sh --list-select-configure-components
bash install.sh --list-component-config-values
bash install.sh --configure-component ID
bash install.sh --export-selection
bash install.sh --detect
bash install.sh --reconfigure a,b
bash install.sh --uninstall a,b
```

Flags that never wipe the installed extension tree: `--list-*`, `--detect`,
`--help`, `--reconfigure`, and `--uninstall`. `--configure-component` and
`--export-selection` also skip core re-extract, but they require the full
`linux_installation_scripts_functions` framework (standalone fallback exits 1).
Fresh install modes (`--default`, `--all`, `--select`, interactive) always
re-run `install_resource_monitor_core`, which re-extracts a clean Resource
Monitor zip so deselected source patches revert.

The Resource Monitor extension is the mandatory core (`install_resource_monitor_core`
runs before component selection on fresh installs). Optional default-on components:

| Component id | Description | Default |
|---|---|---|
| `rm_refresh_interval` | Update time (100–2000 ms) | on, 500 ms |
| `rm_gradient_colors` | Gradient indicator colors | on |
| `rm_vram` | GPU VRAM display (no brackets) | on |
| `rm_per_disk` | Per-disk display | on |
| `rm_panel_spacing` | Panel spacing (stable/compact) | on, stable |
| `rm_hide_eth_icon` | Hide ethernet icon (keep Mbps) | on |
| `rm_process_popup` | Per-process CPU popup (left-click) | on |
| `window_rules` | App window-rules extension (Wayland; empty rules) | off |

## Troubleshooting

If `nvidia-smi` is unavailable, GPU device list configuration is skipped.
If `nvidia-smi` exists but `-L` fails, the installer leaves `gpudeviceslist`
unchanged (it does not write a false empty list).

After core install, the patched Resource Monitor schema is synced into
`~/.local/share/glib-2.0/schemas` so bare `gsettings` matches the extension's
double `refreshtime` range (0.1–60 s).

If `curl`, `unzip`, or `glib-compile-schemas` are missing, re-run with
`sudo bash install.sh` so apt can install `curl`, `unzip`, and `libglib2.0-bin`
(or install those packages yourself).

If `rm-monitor` cannot build, install `cargo` (`sudo apt install cargo` or
rustup) or provide Docker/Podman for `scripts/build_rm_monitor.sh`.

If enable fails with “refusing to rewrite list”, the user session D-Bus is not
reachable from the installer (common over SSH without the session bus). Run the
installer from the logged-in desktop session instead.

If GNOME Shell does not show the updated indicator immediately, log out and
back in. Do not reload GNOME Shell extensions from an active Wayland session
via destructive shortcuts.

## Extended Features

This repo also manages the Wayland **Window Rules Extension**
(`app-rules@local`). Desktop-wide behaviors such as Dash-to-Panel, PWA icons,
and auto-move-windows live in other repositories when used as part of a larger
desktop setup.

## Disclaimer

This software is provided under the MIT License on an **“as is”** basis, without
warranties of any kind. To the maximum extent permitted by applicable law, the
authors and copyright holders shall not be liable for any claims, damages,
losses, or other liability arising from the use of this software.

You are solely responsible for determining whether this software is suitable,
safe, lawful, and appropriate for your intended use. Unless explicitly stated
otherwise, this project is general-purpose software and is not designed,
tested, certified, or approved for safety-critical, medical, automotive,
aviation, industrial-control, life-support, cybersecurity-critical,
financial-critical, or other high-risk use cases.

The authors and copyright holders make no guarantees regarding security,
reliability, availability, correctness, compliance, non-infringement, or
fitness for any particular purpose — including on platforms other than the
tested Ubuntu 24.04 LTS / GNOME Shell 46 combination above.

This notice is intended to clarify the nature of the project and does not
impose additional restrictions beyond the MIT License.
