# Ubuntu 24.04 Taskbar System Status Monitor

Standalone installer for the GNOME Shell Resource Monitor taskbar status setup
used on Ubuntu 24.04. Tested on Ubuntu 24.04 LTS.

It downloads Resource Monitor v27, patches the extension display for GPU VRAM,
disk usage rows, gradient colors, and configurable 100–2000 ms refreshes
(500 ms by default), and configures the panel to show CPU, RAM, `/home` disk
usage/activity, ethernet, and GPU status.

This repository is **fully standalone**: `bash install.sh` is enough. Soft
loading of
[`linux_installation_scripts_functions`](https://github.com/mikaeltorni/linux_installation_scripts_functions)
(sibling checkout or on-demand download) powers the optional component menu; a
missing sibling never blocks installation. The optional master orchestrator
`installation_scripts` may also invoke this installer — the component CLI
contract (`--list-components`, `--select`, …) stays stable for that path.

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

After installation, log out and back in before testing GNOME Shell extension
changes (on X11, agents may use the sanctioned in-place Shell reload instead).

## Development Workflow

```bash
# Rust unit/integration tests (primary)
source "$HOME/.cargo/env"   # if using rustup
cargo test
cargo build --release --bin rm-monitor

# Installer contract / shell checks
bash -n install.sh
bash scripts/build_rm_monitor.sh
python3 -m pytest tests -q
./install.sh --list-components
```

## Project Structure

- `install.sh` — downloads, patches, configures, and enables Resource Monitor.
- `src/` — Rust sources for the `rm-monitor` CLI.
- `scripts/build_rm_monitor.sh` — build into `dist/rm-monitor` (cargo or container).
- `lib/` — Bash installer modules (extension install, components, lifecycle).
- `installer/components.sh` — selectable component manifest for the shared menu.
- `tests/` — installer contract tests (behavior covered primarily by `cargo test`).

## Configuration

Environment overrides:

| Variable | Purpose |
|---|---|
| `RESOURCE_MONITOR_EXTENSION_ID` | Extension UUID (default `Resource_Monitor@Ory0n`) |
| `RESOURCE_MONITOR_EXTENSION_URL` | EGO zip URL |
| `RESOURCE_MONITOR_EXTENSION_SHA256` | Zip checksum |
| `RESOURCE_MONITOR_SPACING_MODE` | `stable` or `compact` |
| `RESOURCE_MONITOR_REFRESH_INTERVAL_MS` | 100–2000 (default 500) |
| `RM_MONITOR_RUST_IMAGE` | Container image for builds (default `rust:1-bookworm`) |

### Panel spacing (stable vs compact)

Every Resource Monitor value label is right-aligned and given a tight reserved
pixel width in **stable** mode so the panel does not jump when digit counts
change. **Compact** mode drops those widths for a narrower indicator. Choose
the mode via the installer's `rm_panel_spacing` component.

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
bash install.sh --detect
bash install.sh --reconfigure a,b
bash install.sh --uninstall a,b
```

The Resource Monitor extension is the mandatory core (`install_resource_monitor_core`
runs before component selection). Optional default-on components:

| Component id | Description | Default |
|---|---|---|
| `rm_refresh_interval` | Update time (100–2000 ms) | on, 500 ms |
| `rm_gradient_colors` | Gradient indicator colors | on |
| `rm_vram` | GPU VRAM display | on |
| `rm_per_disk` | Per-disk display | on |
| `rm_panel_spacing` | Panel spacing (stable/compact) | on, stable |
| `rm_hide_eth_icon` | Hide ethernet icon (keep Mbps) | on |
| `rm_process_popup` | Per-process CPU popup (left-click) | on |
| `window_rules` | App window-rules extension (Wayland) | on |

## Troubleshooting

If `nvidia-smi` is unavailable, GPU device list configuration is skipped.

If `rm-monitor` cannot build, install `cargo` (`sudo apt install cargo` or
rustup) or provide Docker/Podman for `scripts/build_rm_monitor.sh`.

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
fitness for any particular purpose.

This notice is intended to clarify the nature of the project and does not
impose additional restrictions beyond the MIT License.
