#!/usr/bin/env python3
"""Build and apply GSettings values for the Resource Monitor extension.

This module owns Resource Monitor device serialization, display-mode command
construction, and execution of individual ``gsettings set`` commands.
"""

import json
import subprocess

from resource_monitor_disks import filter_disk_devices_to_mount_point
from rm_logging import LOGGER, log_call

__all__ = [
    "LOGGER",
    "format_gsettings_list",
    "build_gsettings_args",
    "apply_settings",
]


def format_gsettings_list(devices: list[dict]) -> str:
    """Format device dictionaries as a GSettings array of JSON strings.

    Args:
        devices: Resource Monitor GPU or disk device dictionaries.

    Returns:
        A value accepted by GSettings for an ``as`` schema key.
    """
    if not devices:
        return "[]"
    return "[" + ", ".join(repr(json.dumps(device)) for device in devices) + "]"


@log_call(LOGGER)
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
    """Build Resource Monitor ``gsettings set`` command arguments.

    Args:
        schema: Resource Monitor GSettings schema ID.
        ext_dir: Path to the extension's compiled schema directory.
        gpu_memory_perc: Display GPU memory as a percentage when true.
        disk_space_gb: Display free disk space in numeric GB.
        disk_space_perc: Legacy alias for ``disk_space_gb``.
        disk_space_perc_home_only: Display only ``/home`` usage percentage.
        gpu_devices: Optional GPU entries for ``gpudeviceslist``.
        disk_devices: Optional disk entries for ``diskdeviceslist``.

    Returns:
        Command argument lists ready for :func:`apply_settings`.
    """
    commands = []
    configure_disk_space = disk_space_gb or disk_space_perc

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
        if disk_devices is not None:
            disk_devices = filter_disk_devices_to_mount_point(
                disk_devices,
                "/home",
            )
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
        if disk_devices is not None:
            disk_devices = filter_disk_devices_to_mount_point(
                disk_devices,
                "/home",
            )

    if gpu_devices is not None and len(gpu_devices) > 0:
        commands.append([
            "gsettings", "--schemadir", ext_dir, "set", schema,
            "gpudeviceslist", format_gsettings_list(gpu_devices),
        ])

    if disk_devices is not None and len(disk_devices) > 0:
        commands.append([
            "gsettings", "--schemadir", ext_dir, "set", schema,
            "diskdeviceslist", format_gsettings_list(disk_devices),
        ])

    return commands


@log_call(LOGGER)
def apply_settings(args: list[str]) -> bool:
    """Execute one ``gsettings set`` command.

    Args:
        args: Complete command arguments.

    Returns:
        ``True`` when the process succeeds, otherwise ``False``.
    """
    try:
        result = subprocess.run(
            args,
            capture_output=True,
            text=True,
            timeout=10,
        )
        if result.returncode != 0:
            LOGGER.error("gsettings failed: %s", result.stderr.strip())
            return False
        return True
    except subprocess.TimeoutExpired:
        LOGGER.error("gsettings command timed out")
        return False
    except Exception as exc:
        LOGGER.error("Unexpected error running gsettings: %s", exc)
        return False
