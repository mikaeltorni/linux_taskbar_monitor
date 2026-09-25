# Linux Taskbar Monitor: GNOME System Status for Ubuntu

Linux Taskbar Monitor is a standalone Ubuntu 24.04 taskbar system monitor for
GNOME Shell 46. It patches the GNOME Shell **Resource Monitor** extension to
show CPU and RAM usage, `/home` free space and disk activity, ethernet
throughput, and GPU/VRAM in a compact GNOME panel strip.

<img width="805" height="50" alt="GNOME top panel displaying compact Resource Monitor system metrics" src="https://github.com/user-attachments/assets/980a757e-333b-420c-9379-ab38b7c7d082" />

## Contents

- [Supported platforms](#supported-platforms)
- [What it installs](#what-it-installs)
- [Technology stack](#technology-stack)
- [Installation](#installation)
- [Development workflow](#development-workflow)
- [Project structure](#project-structure)
  - [`rm-monitor` subcommands](#rm-monitor-subcommands)
- [Configuration](#configuration)
  - [Panel spacing (stable vs compact)](#panel-spacing-stable-vs-compact)
- [Component selection](#component-selection)
- [Troubleshooting](#troubleshooting)
  - [Top-users window](#top-users-window)
- [FAQ](#faq)
- [Extended features](#extended-features)
- [Disclaimer](#disclaimer)

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
loading of the optional `linux_installation_scripts_functions` helper framework
from a local checkout powers the interactive component menu and config export.
Without that checkout, the installer uses a built-in fallback that still
supports `--default` / `--all` / `--select` / `--list-components` / `--detect` /
`--reconfigure` / `--uninstall`. Set `ISC_FUNCTIONS_DIR` to an explicit checkout
to force the full framework, or to a missing path to force the fallback.
Set `ISC_FUNCTIONS_REF` only when you explicitly want to download and run the
shared framework from that Git ref.

> **Repository identity:** the GitHub repository, local checkout, installer
> `ISC_REPO_NAME`, installation-config `repo` field, and master orchestrator
> entry all use `linux_taskbar_monitor`. Existing clones can update their
> `origin` URL to `https://github.com/mikaeltorni/linux_taskbar_monitor.git`.

## What it installs

It downloads Resource Monitor v27 and configures the panel for CPU, RAM,
`/home` disk usage/activity, ethernet, and GPU status. The mandatory core also
enables configurable 100–2000 ms refreshes (500 ms by default). Optional
default-on components then patch GPU VRAM display, per-disk rows, gradient
colors, ethernet icon hide, process popup, and panel spacing — unless you
deselect them.

Resource Monitor is developed by [0ry0n](https://github.com/0ry0n/Resource_Monitor)
under GPL-3.0. The installer downloads and patches that extension; the
installed extension remains subject to its upstream license. This repository's
MIT license covers the files distributed here.

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
The tracked Rust dependencies support the Ubuntu 24.04 `cargo`/`rustc` 1.75
packages; builds use `Cargo.lock` without changing dependency versions.
Core install requires a running `gnome-shell` whose version can be parsed: that
value is pinned into `metadata.json` (version `9999`) so extensions.gnome.org
cannot overwrite local patches on reload. Extract, refresh patch, and metadata
pin run in a staging directory first; the live extension tree is replaced only
after those steps succeed, so a patch/pin failure leaves a previous install
intact. Later GSettings/enable failures can still leave a freshly published
tree that needs a re-run.

After installation on X11, press Alt+F2, type `r`, and press Enter to load the
edited extension code in place. On Wayland, log out and back in before testing
the extension changes.

## Development Workflow

```bash
# Full local / CI gate (fmt, clippy, cargo test, bash -n, pytest, lifecycle)
bash scripts/check.sh

# Build the helper CLI when iterating on patchers
bash scripts/build_rm_monitor.sh

# Installer surface smoke (after a successful check)
./install.sh --list-components
./install.sh --list-configurable-components
./install.sh --detect
```

## Project Structure

- `install.sh` — downloads, patches, configures, and enables Resource Monitor.
- `src/` — Rust sources for the `rm-monitor` CLI (see `rm-monitor --help`).
- `scripts/build_rm_monitor.sh` — build into `dist/rm-monitor` (cargo or container).
- `lib/` — Bash installer modules (extension install, patch wrappers, lifecycle,
  and `standalone_component_fallback.sh` when the shared framework is missing).
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
| `patch-process-popup` | Left-click top-users popup; `--window-minutes N` |
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
| `ISC_FUNCTIONS_REF` | Explicit Git ref that opts into downloading and running the shared framework when no local checkout exists |

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

Ethernet is placed first (leftmost) and remains the single visible network
column; Wi-Fi is disabled to avoid duplicating the connection reading. The
ethernet icon is hidden while Mbps values stay visible. Disk free space shows
as colored GB with a secondary activity %. Its gradient uses the filesystem's
current capacity: 0 GB free is red, half free is yellow, and fully free is
green.

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
Unknown or empty `--select` aborts **before** core re-extract. Fresh install
modes (`--default`, `--all`, `--select` with valid ids, interactive) always
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
| `rm_process_popup` | Top-users popup (left-click) | on, 10 min |
| `window_rules` | App window-rules (Wayland install; X11 skip marker) | off |

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

If GNOME Shell does not show the updated indicator immediately, use the X11
Alt+F2 `r` reload or, on Wayland, log out and back in. Do not reload GNOME
Shell extensions from an active Wayland session via destructive shortcuts.

If left-click opens the popup once and then stops responding, the installed
`extension.js` predates the `vfunc_event` toggle fix: `PanelMenu.Button` and
`_clickManager` each toggled the menu for the same click, so the two cancelled
out as soon as the menu had rows. Re-run `bash install.sh` (or
`./dist/rm-monitor patch-process-popup "$EXT/extension.js"`) and reload the
Shell.

### Top-users window

Left-click opens a popup listing the top process users of every panel
metric — CPU, RAM, disk IO, network, GPU and VRAM. The default window is the
past 10 minutes.

Every row has two columns:

| Column | Meaning |
|---|---|
| `now` | Live reading, refreshed every second for as long as the popup stays open — the value the rows are ranked by |
| `avg` | That process's mean over the rolling window |

Rows are ranked by `now`, so each section names the processes using the metric
at this moment, including one that just started and has no history yet. A
process that goes idle drops off its section, and `avg` reports what the
processes currently listed have been averaging. Because the ranking changes on
every tick, the rows are fixed slots whose text is rewritten in place — the
popup never rebuilds menu items under the pointer.

Each section is five rows tall by default and stays that tall. Anything with a
reading above zero is listed, down to the smallest byte, but the spare rows of
a quiet metric are left blank rather than removed: unplug the ethernet cable
and the Network section holds its space instead of collapsing and dragging
every section below it up the screen. A section with nothing running at all
says `No activity.` in its first row.

`U2TSSM_ROWS` changes how many rows every section gets (1–20), and a
per-metric variable overrides it for one section:

| Variable | Section |
|---|---|
| `U2TSSM_ROWS` | all sections, unless overridden below |
| `U2TSSM_ROWS_CPU` | CPU |
| `U2TSSM_ROWS_RAM` | RAM |
| `U2TSSM_ROWS_DISK` | Disk usage |
| `U2TSSM_ROWS_NET` | Network |
| `U2TSSM_ROWS_GPU` | GPU usage |
| `U2TSSM_ROWS_VRAM` | GPU VRAM |

Setting `U2TSSM_ROWS=10 U2TSSM_ROWS_VRAM=2` gives every section ten rows and
the VRAM section two.

Like `U2TSSM_LIVE_MS` these are read whenever the popup opens, so exporting
them into the desktop session retunes the next popup without re-patching. An
unset, empty, unparsable or out-of-range value falls back to the global value
and then to 5, so a typo can never produce a section with no rows in it.

The live sampler only exists between opening and closing the popup, and
deliberately does not feed the rolling averages — otherwise leaving the popup
open would bias every average towards that period. It sweeps every process
(~10 ms for ~700, handed out in ~2 ms slices between frames) because a
newcomer has no history to be found by. `now` for GPU, VRAM and network
refreshes as fast as `nvidia-smi` and `ss` return, which may be slower than
the live tick.

The popup ticks at its own rate, on purpose: the bar shows two numbers in
place and can flick along at its `refreshtime`, while a whole re-ranked table
at that speed is unreadable. `U2TSSM_LIVE_MS` sets the popup's live-column
interval in milliseconds (100–60000, default 1000). Like `U2TSSM` it is read
live — exporting it into the desktop session retunes the next popup without
re-patching — and it never touches how fast the bar itself refreshes.

`U2TSSM` is this repository's name condensed to its initials (**U**buntu
**2**404 **T**askbar **S**ystem **S**tatus **M**onitor) and carries the
window in minutes (1–1440):

```sh
U2TSSM=120 bash install.sh          # seed a clean install with 2 hours
bash install.sh --configure-component rm_process_popup   # typeable prompt
```

The selection is persisted to
`~/.config/taskbar-system-status-monitor/top-users-window-minutes`, so later
runs keep it without re-passing the variable. The value is baked into
`extension.js` and also re-read live from the environment, so exporting
`U2TSSM` into the desktop session changes the window without re-patching.
Sampling is bounded: the popup keeps per-minute buckets, so a long window
costs memory proportional to the window, not to uptime.

## FAQ

### Do I need sudo to install it?

No. Run `bash install.sh` for the user-level setup. Without root, the installer
reports any missing system packages it could not install; run
`sudo bash install.sh` only if you want it to install those packages through
apt.

### Which Ubuntu and GNOME Shell versions are tested?

Ubuntu 24.04 LTS with GNOME Shell 46 is the tested platform. Other Ubuntu
versions, distributions, and GNOME Shell major versions are best-effort.

### How do I list the available components?

Run `bash install.sh --list-components`. It prints each component ID, label,
and default selection without starting an installation.

### How do I install only selected components?

Pass comma-separated IDs from `--list-components`, for example
`bash install.sh --select rm_vram,rm_process_popup`. A fresh install still
installs the mandatory Resource Monitor core before applying the selected
components.

### What does `--reconfigure` do?

It idempotently reapplies configuration for the selected component IDs without
wiping the installed extension tree. For example,
`bash install.sh --reconfigure rm_refresh_interval,rm_vram` re-applies those
components' settings and patches.

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
