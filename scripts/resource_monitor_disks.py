#!/usr/bin/env python3
"""resource_monitor_disks.py — Detect and format disk devices for Resource Monitor.

Components:
  - build_disk_device_entry(filesystem, mount_point): Build a v2 disk entry dict.
  - parse_df_output(output): Parse ``df -P`` output into disk entries.
  - append_home_directory_entry(entries): Ensure a ``/home`` row is present.
  - detect_disk_devices(): Discover all mounted block devices plus ``/home``.
  - filter_disk_devices_to_mount_point(devices, mount_point): Filter by mount point.
  - detect_disk_devices_home_only(): Discover only the ``/home`` entry.

Logging is provided by the centralized :mod:`rm_logging` module.
"""

import subprocess

from rm_logging import log


def build_disk_device_entry(filesystem: str, mount_point: str) -> dict:
    """Build a Resource Monitor v2 disk device entry.

    Args:
        filesystem: Block device path reported by ``df`` (e.g. ``/dev/sda2``).
        mount_point: Filesystem mount point the device is mounted at (e.g. ``/``).

    Returns:
        A Resource Monitor schema-version-2 disk entry dict whose ``space`` flag
        is enabled and ``stats`` flag is disabled, keyed for the disk-space row.
    """
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
    """Parse POSIX ``df`` output into Resource Monitor disk entries.

    Skips the header row and any line that does not describe a real block device
    (only rows whose first column starts with ``/dev/`` are kept).

    Args:
        output: Raw text from ``df -P`` (or ``df -P <path>``).

    Returns:
        A list of disk device entries built by :func:`build_disk_device_entry`,
        one per ``/dev/*`` filesystem found. Empty when no such rows are present.
    """
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
    """Append a ``/home`` row when it is not already a separate mount.

    When ``/home`` lives on its own mount it is already present in ``entries`` and
    the list is returned unchanged. Otherwise ``df -P /home`` is queried to find
    the backing device, and a dedicated ``/home`` entry is appended so the panel
    can show home usage even when ``/home`` is part of the root filesystem.

    Args:
        entries: Existing disk entries from :func:`parse_df_output`.

    Returns:
        The same list, with a ``/home`` entry appended when one was missing and
        could be resolved. Returns ``entries`` unchanged if ``df`` fails.
    """
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
    """Query ``df`` and return mounted block devices plus a ``/home`` row.

    Returns:
        A list of Resource Monitor disk entries for every mounted ``/dev/*``
        filesystem, with a dedicated ``/home`` entry appended (see
        :func:`append_home_directory_entry`). Empty when ``df`` fails.
    """
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
    """Return disk entries matching ``mount_point``, or all entries for ``None``.

    Args:
        devices: Disk entries to filter (as built by :func:`build_disk_device_entry`).
        mount_point: Mount point to keep (e.g. ``/home``). When ``None``, no
            filtering is applied and ``devices`` is returned unchanged.

    Returns:
        The subset of ``devices`` whose ``mountPoint`` equals ``mount_point``, or
        all of ``devices`` when ``mount_point`` is ``None``.
    """
    if mount_point is None:
        return devices
    return [device for device in devices if device.get("mountPoint") == mount_point]


def detect_disk_devices_home_only() -> list[dict]:
    """Detect and return only the Resource Monitor entry for ``/home``.

    Returns:
        A single-element list containing the ``/home`` disk entry, or an empty
        list when ``df`` fails or ``/home`` cannot be resolved.
    """
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
