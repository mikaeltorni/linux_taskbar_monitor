#!/usr/bin/env python3
"""
configure_resource_monitor.py — Configure Resource Monitor extension settings.

Components:
  - detect_gpu_devices(): Shared GPU discovery imported from report_cuda_devices.
  - Disk discovery helpers imported from resource_monitor_disks.
  - Settings helpers imported from resource_monitor_settings.
  - main(argv): CLI entry point — configures display settings.

Usage:
  python3 scripts/configure_resource_monitor.py --gpu-memory-perc
  python3 scripts/configure_resource_monitor.py --disk-space-gb
  python3 scripts/configure_resource_monitor.py --disk-space-perc-home-only
  python3 scripts/configure_resource_monitor.py --gpu-memory-perc --disk-space-gb
"""

import argparse
import os

from report_cuda_devices import get_gpu_devices as detect_gpu_devices
from resource_monitor_disks import (
    append_home_directory_entry,
    build_disk_device_entry,
    detect_disk_devices,
    detect_disk_devices_home_only,
    filter_disk_devices_to_mount_point,
    parse_df_output,
)
from resource_monitor_settings import (
    apply_settings,
    build_gsettings_args,
    format_gsettings_list,
    log,
)


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
        help="Set GPU memory to percentage mode (used/total %%).",
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
