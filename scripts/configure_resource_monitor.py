#!/usr/bin/env python3
"""
configure_resource_monitor.py — Configure Resource Monitor extension settings.

Components:
  - log(level, msg): Timestamped logging helper (prints to stderr).
  - detect_gpu_devices(): Query nvidia-smi and return structured GPU info.
  - Disk discovery helpers imported from resource_monitor_disks.
  - format_gsettings_list(devices): Format device list as GSettings string array.
  - build_gsettings_args(schema, ext_dir, ...): Build gsettings command arguments.
  - apply_settings(args): Execute a gsettings set command.
  - main(argv): CLI entry point — configures display settings.

Usage:
  python3 scripts/configure_resource_monitor.py --gpu-memory-perc
  python3 scripts/configure_resource_monitor.py --disk-space-gb
  python3 scripts/configure_resource_monitor.py --disk-space-perc-home-only
  python3 scripts/configure_resource_monitor.py --gpu-memory-perc --disk-space-gb
"""

import argparse
import json
import os
import re
import shutil
import subprocess
import sys

from resource_monitor_disks import (
    append_home_directory_entry,
    build_disk_device_entry,
    detect_disk_devices,
    detect_disk_devices_home_only,
    filter_disk_devices_to_mount_point,
    parse_df_output,
)


# ── Logging helper ───────────────────────────────────────────────────────────

def log(level: str, msg: str) -> None:
    """Print a timestamped log message to stderr.

    Args:
        level: Log level string (info, warn, error).
        msg: Message text.
    """
    print(f"[{level.upper()}] {msg}", file=sys.stderr)


# ── GPU detection ────────────────────────────────────────────────────────────

def detect_gpu_devices() -> list[dict]:
    """Query nvidia-smi and return structured GPU device information.

    Returns:
        List of GPU device dicts with keys: version, type, device, name,
        usage, memory, displayName. Empty list if nvidia-smi is unavailable.

    Example:
        >>> detect_gpu_devices()
        [{'version': 2, 'type': 'gpu', 'device': 'GPU-abc123', ...}]
    """
    nvidia_smi = shutil.which("nvidia-smi")
    if nvidia_smi is None:
        log("info", "nvidia-smi not found — no GPUs detected")
        return []

    try:
        output = subprocess.check_output(
            [nvidia_smi, "-L"],
            text=True,
            stderr=subprocess.DEVNULL,
        )
    except Exception as exc:
        log("warn", f"nvidia-smi -L failed: {exc}")
        return []

    entries = []
    for line in output.splitlines():
        match = re.search(
            r"GPU\s+\d+:\s+(.*?)\s+\(UUID:\s+([^)]+)\)",
            line.strip(),
        )
        if not match:
            continue
        name, uuid = match.groups()
        entries.append({
            "version": 2,
            "type": "gpu",
            "device": uuid,
            "name": name,
            "usage": True,
            "memory": True,
            "displayName": "",
        })

    return entries


# ── GSettings helpers ────────────────────────────────────────────────────────

def format_gsettings_list(devices: list[dict]) -> str:
    """Format a list of device dicts as a GSettings string array.

    Converts each device dict to its JSON repr and wraps in GSettings array syntax.
    Uses double-quoted strings for GSettings compatibility.

    Args:
        devices: List of device dicts (GPU or disk).

    Returns:
        GSettings-formatted string array, e.g.:
        "['{\"version\": 2, ...}', '{\"version\": 2, ...}']"

    Example:
        >>> format_gsettings_list([{"device": "GPU-abc"}])
        "['{\"device\": \"GPU-abc\"}']"
    """
    if not devices:
        return "[]"
    return "[" + ", ".join(repr(json.dumps(device)) for device in devices) + "]"


def build_gsettings_args(
    schema: str,
    ext_dir: str,
    gpu_memory_perc: bool = False,
    disk_space_gb: bool = False,
    disk_space_perc: bool = False,
    disk_space_perc_home_only: bool = False,
    gpu_devices: list[dict] | None = None,
    disk_devices: list[dict] | None = None,
) -> list[list[str]]:
    """Build gsettings set commands for Resource Monitor configuration.

    Args:
        schema: GSettings schema ID.
        ext_dir: Path to extension schemas directory.
        gpu_memory_perc: If True, set gpumemoryunit to 'perc'.
            (Default is 'numeric' — absolute VRAM values like "22.6gb".)
        disk_space_gb: If True, show remaining disk space in numeric GB.
        disk_space_perc: Legacy alias for disk_space_gb retained for old callers.
        disk_space_perc_home_only: If True, show /home disk usage as percentage
            of total NVMe capacity (diskspaceunit='perc', home-only filter).
        gpu_devices: Optional list of GPU device dicts for gpudeviceslist.
        disk_devices: Optional list of disk device dicts for diskdeviceslist.

    Returns:
        List of gsettings command argument lists, each ready for subprocess.run().

    Example:
        >>> build_gsettings_args("org.gnome.shell.extensions.resource-monitor",
        ...                      "/path/to/schemas", gpu_memory_perc=True)
        [['gsettings', '--schemadir', '/path/to/schemas', 'set', ..., 'gpumemoryunit', "'perc'"]]
    """
    commands = []
    configure_disk_space = disk_space_gb or disk_space_perc

    # Always set gpumemoryunit to 'numeric' (absolute VRAM values) unless
    # the caller explicitly requests percentage mode.
    if gpu_memory_perc:
        commands.append([
            "gsettings", "--schemadir", ext_dir, "set", schema,
            "gpumemoryunit", "'perc'",
        ])
    else:
        commands.append([
            "gsettings", "--schemadir", ext_dir, "set", schema,
            "gpumemoryunit", "'numeric'",
        ])

    if disk_space_perc_home_only:
        # Percentage mode — show /home usage as % of total NVMe capacity.
        commands.append([
            "gsettings", "--schemadir", ext_dir, "set", schema,
            "diskstatsstatus", "false",
        ])
        commands.append([
            "gsettings", "--schemadir", ext_dir, "set", schema,
            "diskspaceunit", "'perc'",
        ])
        commands.append([
            "gsettings", "--schemadir", ext_dir, "set", schema,
            "diskspacemonitor", "'used'",
        ])
        # Filter disk devices to /home only.
        if disk_devices is not None:
            disk_devices = filter_disk_devices_to_mount_point(disk_devices, "/home")

    elif configure_disk_space:
        commands.append([
            "gsettings", "--schemadir", ext_dir, "set", schema,
            "diskstatsstatus", "false",
        ])
        commands.append([
            "gsettings", "--schemadir", ext_dir, "set", schema,
            "diskspaceunit", "'numeric'",
        ])
        commands.append([
            "gsettings", "--schemadir", ext_dir, "set", schema,
            "diskspaceunitmeasure", "'g'",
        ])
        commands.append([
            "gsettings", "--schemadir", ext_dir, "set", schema,
            "diskspacemonitor", "'free'",
        ])
        # Keep the panel focused on the /home row while showing free GB.
        if disk_devices is not None:
            disk_devices = filter_disk_devices_to_mount_point(disk_devices, "/home")

    if gpu_devices is not None and len(gpu_devices) > 0:
        devices_list = format_gsettings_list(gpu_devices)
        commands.append([
            "gsettings", "--schemadir", ext_dir, "set", schema,
            "gpudeviceslist", devices_list,
        ])

    if disk_devices is not None and len(disk_devices) > 0:
        devices_list = format_gsettings_list(disk_devices)
        commands.append([
            "gsettings", "--schemadir", ext_dir, "set", schema,
            "diskdeviceslist", devices_list,
        ])

    return commands


def apply_settings(args: list[str]) -> bool:
    """Execute a single gsettings set command.

    Args:
        args: Command arguments (e.g., ['gsettings', '--schemadir=...', 'set', ...]).

    Returns:
        True if the command succeeded, False otherwise.
    """
    try:
        result = subprocess.run(
            args,
            capture_output=True,
            text=True,
            timeout=10,
        )
        if result.returncode != 0:
            log("error", f"gsettings failed: {result.stderr.strip()}")
            return False
        return True
    except subprocess.TimeoutExpired:
        log("error", "gsettings command timed out")
        return False
    except Exception as exc:
        log("error", f"Unexpected error running gsettings: {exc}")
        return False


# ── CLI entry point ─────────────────────────────────────────────────────────

def main(argv: list[str] | None = None) -> int:
    """CLI entry point for configure_resource_monitor.py.

    Configures Resource Monitor extension settings for the requested display mode.

    Args:
        argv: Command-line arguments (defaults to sys.argv[1:]).

    Returns:
        0 on success, non-zero on error.
    """
    parser = argparse.ArgumentParser(
        description="Configure Resource Monitor extension display settings.",
    )
    parser.add_argument(
        "--gpu-memory-perc",
        action="store_true",
        help="Set GPU memory to percentage mode (used/total %).",
    )
    parser.add_argument(
        "--disk-space-gb",
        action="store_true",
        help="Set disk space to used GB mode. The patch adds live IO percentage.",
    )
    parser.add_argument(
        "--disk-space-perc",
        action="store_true",
        help=argparse.SUPPRESS,
    )
    parser.add_argument(
        "--disk-space-perc-home-only",
        action="store_true",
        help="Show /home disk space as percentage of total.",
    )
    parser.add_argument(
        "--schema-dir",
        default=None,
        help="Path to extension schemas directory (auto-detected if omitted).",
    )

    args = parser.parse_args(argv)

    # Mutual exclusion: cannot use both GB and perc-home-only modes.
    if args.disk_space_gb and args.disk_space_perc_home_only:
        log("error", "--disk-space-gb and --disk-space-perc-home-only are mutually exclusive.")
        return 1

    configure_disk_space = (
        args.disk_space_gb or args.disk_space_perc or args.disk_space_perc_home_only
    )

    if not args.gpu_memory_perc and not configure_disk_space:
        log("error", "No options specified. Use --gpu-memory-perc and/or --disk-space-gb.")
        return 1

    # Auto-detect schema directory
    ext_dir = args.schema_dir
    if ext_dir is None:
        username = os.environ.get("SUDO_USER") or os.environ.get("USER", "mk")
        candidate = f"/home/{username}/.local/share/gnome-shell/extensions/Resource_Monitor@Ory0n/schemas"
        if os.path.isdir(candidate):
            ext_dir = candidate
        else:
            log("error", f"Could not auto-detect schema directory at {candidate}. Use --schema-dir.")
            return 1

    schema = "org.gnome.shell.extensions.resource-monitor"

    # Choose detection strategy based on requested mode.
    if args.disk_space_perc_home_only:
        disk_devices = detect_disk_devices_home_only()
    else:
        disk_devices = detect_disk_devices() if configure_disk_space else None

    gpu_devices = detect_gpu_devices() if args.gpu_memory_perc else None

    commands = build_gsettings_args(
        schema, ext_dir,
        gpu_memory_perc=args.gpu_memory_perc,
        disk_space_gb=configure_disk_space and not args.disk_space_perc_home_only,
        disk_space_perc=args.disk_space_perc,
        disk_space_perc_home_only=args.disk_space_perc_home_only,
        gpu_devices=gpu_devices,
        disk_devices=disk_devices,
    )

    if not commands:
        log("info", "No settings to apply.")
        return 0

    success = True
    for cmd_args in commands:
        log("info", f"Running: {' '.join(cmd_args)}")
        if not apply_settings(cmd_args):
            success = False

    return 0 if success else 1


if __name__ == "__main__":
    raise SystemExit(main())
