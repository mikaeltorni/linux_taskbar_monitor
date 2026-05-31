# Ubuntu 24.04 Taskbar System Status Monitor

Standalone installer for the GNOME Shell Resource Monitor taskbar status setup used on Ubuntu 24.04.

It installs Resource Monitor v27, patches the extension display for GPU VRAM and disk space rows, and configures the panel to show CPU, RAM, disk space/activity, ethernet, and GPU status.

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
- `scripts/configure_resource_monitor.py` - builds and applies Resource Monitor GSettings device lists.
- `scripts/report_cuda_devices.py` - emits NVIDIA GPU devices for Resource Monitor.
- `scripts/patch_resource_monitor_disk.js` - patches disk space display and activity percentage behavior.
- `scripts/patch_resource_monitor_vram.js` - patches GPU VRAM display formatting.
- `tests/` - simulation tests; no system settings are changed by tests.

## Configuration

The installer supports these environment overrides:

- `RESOURCE_MONITOR_EXTENSION_ID`
- `RESOURCE_MONITOR_EXTENSION_URL`
- `RESOURCE_MONITOR_EXTENSION_SHA256`

Ethernet status is enabled with `netethstatus true`; Wi-Fi status remains disabled with `netwlanstatus false`.

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

## Disclaimer

This software is provided under the MIT License on an **“as is”** basis, without warranties of any kind. To the maximum extent permitted by applicable law, the authors and copyright holders shall not be liable for any claims, damages, losses, or other liability arising from the use of this software.

You are solely responsible for determining whether this software is suitable, safe, lawful, and appropriate for your intended use. Unless explicitly stated otherwise, this project is general-purpose software and is not designed, tested, certified, or approved for safety-critical, medical, automotive, aviation, industrial-control, life-support, cybersecurity-critical, financial-critical, or other high-risk use cases.

The authors and copyright holders make no guarantees regarding security, reliability, availability, correctness, compliance, non-infringement, or fitness for any particular purpose.

This notice is intended to clarify the nature of the project and does not impose additional restrictions beyond the MIT License.
