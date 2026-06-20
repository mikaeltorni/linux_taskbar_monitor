#!/usr/bin/env python3
"""Detect and format disk devices for the Resource Monitor extension."""

import subprocess


from rm_logging import log


def build_disk_device_entry(filesystem: str, mount_point: str) -> dict:
    """Build a Resource Monitor v2 disk device entry."""
    return {
        "version": 2,
        "type": "disk",
        "device": filesystem,
        "stableId": "",
        "mountPoint": mount_point,
        "stats": False,
        "space": True,
        "displayName": mount_point,
    }


def parse_df_output(output: str) -> list[dict]:
    """Parse POSIX ``df`` output into Resource Monitor disk entries."""
    entries = []
    lines = output.strip().splitlines()
    if len(lines) < 2:
        return entries

    for line in lines[1:]:
        parts = line.split()
        if len(parts) < 6:
            continue

        filesystem = parts[0]
        mount_point = parts[5]
        if not filesystem.startswith("/dev/"):
            continue

        entries.append(build_disk_device_entry(filesystem, mount_point))

    return entries


def append_home_directory_entry(entries: list[dict]) -> list[dict]:
    """Append a ``/home`` row when it is not already a separate mount."""
    if any(entry.get("mountPoint") == "/home" for entry in entries):
        return entries

    try:
        output = subprocess.check_output(
            ["df", "-P", "/home"],
            text=True,
            stderr=subprocess.DEVNULL,
        )
    except Exception as exc:
        log("warn", f"df /home command failed: {exc}")
        return entries

    home_entries = parse_df_output(output)
    if not home_entries:
        return entries

    home_entry = home_entries[0]
    home_entry["mountPoint"] = "/home"
    home_entry["displayName"] = "/home"
    entries.append(home_entry)
    return entries


def detect_disk_devices() -> list[dict]:
    """Query ``df`` and return mounted block devices plus a ``/home`` row."""
    try:
        output = subprocess.check_output(
            ["df", "-P"],
            text=True,
            stderr=subprocess.DEVNULL,
        )
    except Exception as exc:
        log("warn", f"df command failed: {exc}")
        return []

    return append_home_directory_entry(parse_df_output(output))


def filter_disk_devices_to_mount_point(
    devices: list[dict],
    mount_point: str | None,
) -> list[dict]:
    """Return disk entries matching ``mount_point``, or all entries for ``None``."""
    if mount_point is None:
        return devices
    return [device for device in devices if device.get("mountPoint") == mount_point]


def detect_disk_devices_home_only() -> list[dict]:
    """Detect and return only the Resource Monitor entry for ``/home``."""
    try:
        output = subprocess.check_output(
            ["df", "-P"],
            text=True,
            stderr=subprocess.DEVNULL,
        )
    except Exception as exc:
        log("warn", f"df command failed: {exc}")
        return []

    entries = append_home_directory_entry(parse_df_output(output))
    return filter_disk_devices_to_mount_point(entries, "/home")
