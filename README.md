# Ubuntu 24.04 Taskbar System Status Monitor

> Part of [installation_scripts](https://github.com/mikaeltorni/installation_scripts) — the master installer that orchestrates a productivity-focused Ubuntu 24.04 desktop setup (workspaces, hotkeys, window tiling, programming tools, and more). Tested on Ubuntu 24.04.4 LTS.

Standalone installer for the GNOME Shell Resource Monitor taskbar status setup used on Ubuntu 24.04.

It installs Resource Monitor v27, patches the extension display for GPU VRAM, disk usage rows, gradient colors, and configurable 100–2000 ms refreshes (500 ms by default), and configures the panel to show CPU, RAM, `/home` disk usage/activity, ethernet, and GPU status.

## Repository dependencies

This repository installs and runs **standalone** — it has no hard dependency on
any sibling setup repository.

- **Build-time (soft):** the shared installer component framework
  [`linux_installation_scripts_functions`](https://github.com/mikaeltorni/linux_installation_scripts_functions) —
  loaded from a sibling checkout when present, otherwise downloaded on demand,
  so a missing checkout never blocks installation.

See the full cross-repository map in
[installation_scripts/DEPENDENCIES.md](https://github.com/mikaeltorni/installation_scripts/blob/master/DEPENDENCIES.md).

## Technology Stack

- Bash installer for system and GNOME Shell integration.
- Python helpers for GPU and disk device detection.
- Node.js patch scripts for Resource Monitor JavaScript files.
- Pytest tests for Python helpers and fixture-based extension patching.

## Installation

```bash
sudo bash install.sh
```

The installer must run with `sudo` because it installs files into the target user's GNOME Shell extension directory and fixes ownership. User-scoped GSettings writes are executed as the target desktop user.

After installation, log out and back in before testing GNOME Shell extension changes.

## Development Workflow

Run the test suite:

```bash
python3 -m pytest tests -q
```

Run shell syntax validation:

```bash
bash -n install.sh
```

## Project Structure

- `install.sh` - downloads, patches, configures, and enables Resource Monitor.
- `scripts/configure_resource_monitor.py` - builds and applies Resource Monitor GSettings device lists using the shared disk and GPU discovery modules.
- `scripts/resource_monitor_settings.py` - serializes device lists and builds and applies Resource Monitor GSettings commands.
- `scripts/resource_monitor_disks.py` - detects, parses, and filters Resource Monitor disk entries.
- `scripts/report_cuda_devices.py` - emits NVIDIA GPU devices for Resource Monitor.
- `scripts/patch_resource_monitor_disk.js` - patches disk space display and activity percentage behavior.
- `scripts/patch_resource_monitor_vram.js` - patches GPU VRAM display formatting.
- `scripts/patch_resource_monitor_colors.js` - patches Resource Monitor value colors with per-metric gradients.
- `scripts/patch_resource_monitor_refresh.py` - adds 100 ms-capable timer/schema support and a 500 ms default refresh interval.
- `tests/` - simulation tests; no system settings are changed by tests.

## Configuration

The installer supports these environment overrides:

## Stable panel width (no taskbar shift)

Every Resource Monitor value label is right-aligned and given a tight reserved pixel width, so the panel no longer jumps when a metric's digit count changes (e.g. CPU 9% → 10% → 100%, RAM 50 → 9 GB, or disk activity 5% → 100%). The reserved widths are sized to the widest reading at the configured units and the panel font (one character of slack each), and the extension multiplies them by the display scale factor:

| Metric | Reserved width (px, pre-scale) | Covers |
|---|---|---|
| CPU | 24 | 0–100% (3 digits, "100"=24px) |
| RAM | 20 | GB, integer (2 digits) |
| Disk free | 36 | GB, integer (3 digits) |
| GPU usage | 24 | 0–100% (3 digits) |
| GPU VRAM | 16 | GB, integer (2 digits; split off from usage by `rm_stable_width`) |
| Ethernet | 60 | down\|up, 3\|3 digits |

Ethernet is placed **first** (leftmost) in the indicator order so its wider and rarer over-range readings grow toward the screen center instead of shifting the clock. The ethernet **icon is hidden** (`rm_hide_eth_icon`) while the numeric Mbps value and unit stay visible. The disk-space secondary **activity %** and the GPU VRAM value have no upstream width setting, so the `rm_stable_width` component reserves them through a small source patch (`scripts/patch_resource_monitor_stable_width.js`).

If you change units or monitor large networks/VRAM, raise the relevant key with `gsettings set org.gnome.shell.extensions.resource-monitor <key>width <px>`.



- `RESOURCE_MONITOR_EXTENSION_ID`
- `RESOURCE_MONITOR_EXTENSION_URL`
- `RESOURCE_MONITOR_EXTENSION_SHA256`

Disk space is configured in GB mode, so the panel shows the `/home` disk row as colored free-space GB and keeps disk throughput stats disabled. The disk patch renders live disk load as a secondary colored percentage next to that primary GB value.

Ethernet status is enabled with `netethstatus true`; Wi-Fi status remains disabled with `netwlanstatus false`. Ethernet uses `netunitmeasure 'm'`, and the color gradient reaches red at the `ETHERNET_MAX_MBPS` displayed MB/s value in `scripts/patch_resource_monitor_colors.js`.

Resource values refresh every 500 ms by default. Open **Resource Monitor update
time → details** in either the master or standalone installer and edit the
pre-filled field to choose an integer from 100 through 2000 ms. The choice is
persisted and applied live through the extension's GSettings schema.

## Troubleshooting

If `nvidia-smi` is unavailable, GPU device list configuration is skipped and the installer continues.

If GNOME Shell does not show the updated indicator immediately, log out and back in. Do not reload GNOME Shell extensions from an active Wayland session.

## Deployment

Before deploying a change:

```bash
bash -n install.sh
python3 -m pytest tests -q
```

Then run the installer:

```bash
sudo bash install.sh
```

## Contributing

Keep installer behavior idempotent and keep GNOME Shell extension source patches covered by fixture tests. Avoid hard-coded checkout paths; resolve repository assets through `SCRIPT_DIR`.

## Part of 7-Repo Desktop Setup Chain

This repository is **Phase 6** in the desktop setup chain:

1. `linux_installations_setup` — Base packages, hardware, apps
2. `linux_configuration_setup` — GNOME desktop, autostart, GSettings
3. `linux_hotkey_setup` — GNOME hotkeys and workspace bindings
4. `agent_command_center` — Agent command center components
5. `linux_programming_setup` — Cursor IDE, VS Code extensions
6. **`ubuntu_2404_taskbar_system_status_monitor` (this repo)** — Taskbar system monitor extension
7. `phrase_automation` — Phrase automation tmux service

It is automatically cloned and run by the main installer (`installation_scripts/install.sh`).

## Extended Features

This repo also manages:
- **Window Rules Extension** (`app-rules@local`): Assigns apps to workspaces or makes them sticky on Wayland

> Previously this repo also bundled auto-move-windows placement, Dash-to-Panel,
> and Chrome PWA icons. Those are desktop-wide behaviors owned by other
> repositories and were moved there to avoid duplication: auto-move-windows →
> [`linux_workspaces_setup`](https://github.com/mikaeltorni/linux_workspaces_setup),
> Dash-to-Panel and PWA icons → [`linux_configuration_setup`](https://github.com/mikaeltorni/linux_configuration_setup).

## Component selection

This repository's `install.sh` participates in the shared installer
component menu (see the [`installation_scripts`](https://github.com/mikaeltorni/installation_scripts)
master installer). Run standalone, it offers an interactive submenu on a
terminal; the master installer drives it non-interactively.

```bash
bash install.sh                 # interactive component selection (TTY), else defaults
bash install.sh --default       # install all default-on components, no prompts
bash install.sh --all           # install every component, no prompts
bash install.sh --select a,b    # install exactly these component ids
bash install.sh --list-components  # print: id<TAB>label<TAB>default
```

In the interactive menu, **space** toggles a component, `a`/`n` select all
or none, and **Enter** installs the selection. Components:

The **Resource Monitor extension is the mandatory core**: `install.sh` installs
it unconditionally before the component selection runs, so the taskbar indicator
works no matter which components you pick. The components below are all optional
(default-on) and layer on top of the core:

| Component id | Description | Default |
|---|---|---|
| `rm_refresh_interval` | Resource Monitor update time (typeable 100–2000 ms field) | on, 500 ms |
| `rm_gradient_colors` | Resource Monitor gradient indicator colors | on |
| `rm_vram` | Resource Monitor GPU VRAM display | on |
| `rm_per_disk` | Resource Monitor per-disk display | on |
| `rm_stable_width` | Resource Monitor stable panel widths | on |
| `rm_hide_eth_icon` | Resource Monitor hide ethernet icon (keep Mbps) | on |
| `window_rules` | App window-rules extension (Wayland) | on |

### Customizing the core

The core widens the extension to accept sub-second refresh intervals; 500 ms is
only the default. Scripted installs can override it in milliseconds:

```bash
RESOURCE_MONITOR_REFRESH_INTERVAL_MS=1000 bash install.sh
```


## Detect, reconfigure, and uninstall

This installer tracks what it has installed and can re-apply or remove it, so you
can refresh configuration after a repo update or cleanly back a feature out.

```bash
bash install.sh --detect            # show each component as installed|absent
bash install.sh --reconfigure a,b   # re-apply (idempotent) these component ids
bash install.sh --uninstall a,b     # uninstall these component ids
```

In the interactive menu (run `bash install.sh` on a terminal, or via the master
installer), already-installed components show a green `✓`. Select one with
**space** to **reconfigure** it (`~`), press **`u`** to mark it for **uninstall**
(`✗`), or press **`r`** to reconfigure every installed component at once. Detection
uses a live check where deterministic and otherwise an install receipt under
`${XDG_STATE_HOME:-~/.local/state}/isc/receipts/`; a component without a reversal
step simply clears that receipt on uninstall.

## Disclaimer

This software is provided under the MIT License on an **“as is”** basis, without warranties of any kind. To the maximum extent permitted by applicable law, the authors and copyright holders shall not be liable for any claims, damages, losses, or other liability arising from the use of this software.

You are solely responsible for determining whether this software is suitable, safe, lawful, and appropriate for your intended use. Unless explicitly stated otherwise, this project is general-purpose software and is not designed, tested, certified, or approved for safety-critical, medical, automotive, aviation, industrial-control, life-support, cybersecurity-critical, financial-critical, or other high-risk use cases.

The authors and copyright holders make no guarantees regarding security, reliability, availability, correctness, compliance, non-infringement, or fitness for any particular purpose.

This notice is intended to clarify the nature of the project and does not impose additional restrictions beyond the MIT License.
