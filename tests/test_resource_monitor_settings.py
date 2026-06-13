#!/usr/bin/env python3
"""Tests for Resource Monitor GSettings serialization and command execution."""

import ast
import json
import subprocess
import sys
from pathlib import Path
from unittest.mock import MagicMock, patch


sys.path.insert(0, str(Path("scripts").resolve()))

import resource_monitor_settings as mod


SCHEMA = "org.gnome.shell.extensions.resource-monitor"
SCHEMA_DIR = "/tmp/resource-monitor/schemas"


def test_log_writes_uppercase_level_to_stderr(capsys):
    """Logging should retain the existing installer-compatible format."""
    mod.log("warn", "configuration failed")

    assert capsys.readouterr().err == "[WARN] configuration failed\n"


def test_format_gsettings_list_serializes_json_strings():
    """Device dictionaries should become a GSettings array of JSON strings."""
    devices = [{"device": "GPU-abc", "name": "RTX 4090"}]

    value = mod.format_gsettings_list(devices)
    parsed = [json.loads(item) for item in ast.literal_eval(value)]

    assert parsed == devices


def test_format_gsettings_list_handles_empty_devices():
    """An empty device collection should use the empty GSettings array."""
    assert mod.format_gsettings_list([]) == "[]"


def test_build_gsettings_args_defaults_to_numeric_gpu_memory():
    """The default command should preserve absolute GPU memory display."""
    assert mod.build_gsettings_args(SCHEMA, SCHEMA_DIR) == [[
        "gsettings",
        "--schemadir",
        SCHEMA_DIR,
        "set",
        SCHEMA,
        "gpumemoryunit",
        "'numeric'",
    ]]


def test_build_gsettings_args_configures_free_gb_and_home_device():
    """GB mode should disable throughput and retain only the /home disk row."""
    disk_devices = [
        {"device": "/dev/sda1", "mountPoint": "/"},
        {"device": "/dev/sda1", "mountPoint": "/home"},
    ]

    commands = mod.build_gsettings_args(
        SCHEMA,
        SCHEMA_DIR,
        disk_space_gb=True,
        disk_devices=disk_devices,
    )
    command_text = " ".join(" ".join(command) for command in commands)

    assert "diskstatsstatus false" in command_text
    assert "diskspaceunit 'numeric'" in command_text
    assert "diskspaceunitmeasure 'g'" in command_text
    assert "diskspacemonitor 'free'" in command_text
    assert '"mountPoint": "/"' not in command_text
    assert '"mountPoint": "/home"' in command_text


def test_build_gsettings_args_preserves_legacy_disk_space_alias():
    """The legacy disk-space flag should still configure free-GB mode."""
    commands = mod.build_gsettings_args(
        SCHEMA,
        SCHEMA_DIR,
        disk_space_perc=True,
    )
    command_text = " ".join(" ".join(command) for command in commands)

    assert "diskspaceunit 'numeric'" in command_text
    assert "diskspacemonitor 'free'" in command_text


def test_build_gsettings_args_configures_home_percentage_mode():
    """Home-only percentage mode should omit numeric unit measurement."""
    disk_devices = [
        {"device": "/dev/sda1", "mountPoint": "/"},
        {"device": "/dev/sda1", "mountPoint": "/home"},
    ]

    commands = mod.build_gsettings_args(
        SCHEMA,
        SCHEMA_DIR,
        disk_space_perc_home_only=True,
        disk_devices=disk_devices,
    )
    command_text = " ".join(" ".join(command) for command in commands)

    assert "diskspaceunit 'perc'" in command_text
    assert "diskspacemonitor 'used'" in command_text
    assert "diskspaceunitmeasure" not in command_text
    assert '"mountPoint": "/"' not in command_text
    assert '"mountPoint": "/home"' in command_text


def test_build_gsettings_args_includes_gpu_devices():
    """Detected GPUs should be included in the generated command list."""
    devices = [{"device": "GPU-abc", "name": "RTX 4090"}]

    commands = mod.build_gsettings_args(
        SCHEMA,
        SCHEMA_DIR,
        gpu_memory_perc=True,
        gpu_devices=devices,
    )

    assert commands[0][-2:] == ["gpumemoryunit", "'perc'"]
    assert commands[1][-2] == "gpudeviceslist"
    assert "GPU-abc" in commands[1][-1]


def test_apply_settings_returns_true_for_success():
    """A successful gsettings process should return true."""
    result = MagicMock(returncode=0, stderr="")

    with patch.object(mod.subprocess, "run", return_value=result) as run:
        assert mod.apply_settings(["gsettings", "set", "schema", "key"]) is True

    run.assert_called_once_with(
        ["gsettings", "set", "schema", "key"],
        capture_output=True,
        text=True,
        timeout=10,
    )


def test_apply_settings_logs_process_failure(capsys):
    """A nonzero gsettings process should return false and report stderr."""
    result = MagicMock(returncode=1, stderr="invalid value\n")

    with patch.object(mod.subprocess, "run", return_value=result):
        assert mod.apply_settings(["gsettings"]) is False

    assert capsys.readouterr().err == "[ERROR] gsettings failed: invalid value\n"


def test_apply_settings_logs_timeout(capsys):
    """A timed-out gsettings process should return false."""
    with patch.object(
        mod.subprocess,
        "run",
        side_effect=subprocess.TimeoutExpired("gsettings", 10),
    ):
        assert mod.apply_settings(["gsettings"]) is False

    assert capsys.readouterr().err == "[ERROR] gsettings command timed out\n"


def test_apply_settings_logs_unexpected_error(capsys):
    """Unexpected process errors should remain non-fatal to the caller."""
    with patch.object(mod.subprocess, "run", side_effect=OSError("missing")):
        assert mod.apply_settings(["gsettings"]) is False

    assert (
        capsys.readouterr().err
        == "[ERROR] Unexpected error running gsettings: missing\n"
    )
