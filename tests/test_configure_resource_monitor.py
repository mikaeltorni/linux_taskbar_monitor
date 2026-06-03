#!/usr/bin/env python3
"""Tests for configure_resource_monitor.py — simulation only, no system changes."""

import json
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path
from unittest.mock import patch, MagicMock

sys.path.insert(0, str(Path("scripts").resolve()))
from importlib.util import spec_from_file_location, module_from_spec

crm = spec_from_file_location(
    "configure_resource_monitor",
    "scripts/configure_resource_monitor.py",
)
mod = module_from_spec(crm)
crm.loader.exec_module(mod)


class TestDetectGpuDevices:
    """Tests for detect_gpu_devices function."""

    def test_returns_empty_when_nvidia_smi_missing(self):
        """Should return empty list when nvidia-smi is not available."""
        with patch("shutil.which", return_value=None):
            result = mod.detect_gpu_devices()
        assert result == []

    def test_parses_single_gpu(self):
        """Should parse a single GPU from nvidia-smi -L output."""
        sample_output = "GPU 0: NVIDIA GeForce RTX 4090 (UUID: GPU-abc123)\n"
        with patch("shutil.which", return_value="/usr/bin/nvidia-smi"), \
             patch("subprocess.check_output", return_value=sample_output):
            result = mod.detect_gpu_devices()
        assert len(result) == 1
        assert result[0]["device"] == "GPU-abc123"
        assert result[0]["name"] == "NVIDIA GeForce RTX 4090"
        assert result[0]["usage"] is True
        assert result[0]["memory"] is True

    def test_parses_multiple_gpus(self):
        """Should parse multiple GPUs from nvidia-smi -L output."""
        sample_output = (
            "GPU 0: NVIDIA GeForce RTX 4090 (UUID: GPU-abc123)\n"
            "GPU 1: NVIDIA GeForce RTX 3080 (UUID: GPU-def456)\n"
        )
        with patch("shutil.which", return_value="/usr/bin/nvidia-smi"), \
             patch("subprocess.check_output", return_value=sample_output):
            result = mod.detect_gpu_devices()
        assert len(result) == 2
        assert result[0]["device"] == "GPU-abc123"
        assert result[1]["device"] == "GPU-def456"


class TestDetectDiskDevices:
    """Tests for detect_disk_devices function."""

    def test_returns_empty_when_df_fails(self):
        """Should return empty list when df command fails."""
        with patch("subprocess.check_output", side_effect=Exception("df failed")):
            result = mod.detect_disk_devices()
        assert result == []

    def test_parses_mounted_filesystems(self):
        """Should parse mounted filesystems from df output."""
        sample_output = (
            "Filesystem      Size  Used Avail Use% Mounted on\n"
            "/dev/nvme1n1p5  492G   92G  376G  20% /\n"
            "/dev/nvme1n1p6  100G   50G   50G  50% /home\n"
        )
        with patch("subprocess.check_output", return_value=sample_output):
            result = mod.detect_disk_devices()
        assert len(result) == 2
        devices = [d["device"] for d in result]
        assert "/dev/nvme1n1p5" in devices
        assert "/dev/nvme1n1p6" in devices

    def test_adds_home_directory_when_home_shares_root_filesystem(self):
        """Should add /home as a disk row even when it is not a separate mount."""
        root_output = (
            "Filesystem      Size  Used Avail Use% Mounted on\n"
            "/dev/nvme1n1p5  492G   92G  376G  20% /\n"
        )
        home_output = (
            "Filesystem      Size  Used Avail Use% Mounted on\n"
            "/dev/nvme1n1p5  492G   92G  376G  20% /home\n"
        )
        with patch("subprocess.check_output", side_effect=[root_output, home_output]):
            result = mod.detect_disk_devices()

        assert [entry["mountPoint"] for entry in result] == ["/", "/home"]
        assert result[1]["displayName"] == "/home"

    def test_disk_entries_match_resource_monitor_schema(self):
        """Should emit Resource Monitor v2 disk entries that parse in GJS."""
        sample_output = (
            "Filesystem      Size  Used Avail Use% Mounted on\n"
            "/dev/nvme1n1p5  492G   92G  376G  20% /\n"
        )
        with patch("subprocess.check_output", return_value=sample_output):
            result = mod.detect_disk_devices()

        assert result[:1] == [{
            "version": 2,
            "type": "disk",
            "device": "/dev/nvme1n1p5",
            "stableId": "",
            "mountPoint": "/",
            "stats": False,
            "space": True,
            "displayName": "/",
        }]
        assert result[1]["mountPoint"] == "/home"

    def test_skips_non_block_devices(self):
        """Should skip tmpfs and other non-block-device filesystems."""
        sample_output = (
            "Filesystem      Size  Used Avail Use% Mounted on\n"
            "tmpfs            16G  1.2M   16G   1% /run\n"
            "/dev/sda1       500G  200G  300G  40% /\n"
        )
        with patch("subprocess.check_output", return_value=sample_output):
            result = mod.detect_disk_devices()
        assert len(result) == 2
        assert result[0]["device"] == "/dev/sda1"
        assert result[1]["mountPoint"] == "/home"


class TestBuildGsettingsArgs:
    """Tests for build_gsettings_args function."""

    def test_builds_correct_args_for_gpu_memory(self):
        """Should build correct gsettings args for GPU memory percentage mode."""
        schema = "org.gnome.shell.extensions.resource-monitor"
        ext_dir = "/fake/path/schemas"
        result = mod.build_gsettings_args(schema, ext_dir, gpu_memory_perc=True)
        # Result is a list of command lists; check first command contains expected values
        assert len(result) == 1
        cmd = result[0]
        assert "gsettings" in cmd
        assert "--schemadir" in cmd
        assert ext_dir in cmd
        assert schema in cmd
        assert "gpumemoryunit" in cmd
        assert "'perc'" in cmd

    def test_builds_correct_args_for_disk_space(self):
        """Should build correct gsettings args for GB disk space mode."""
        schema = "org.gnome.shell.extensions.resource-monitor"
        ext_dir = "/fake/path/schemas"
        result = mod.build_gsettings_args(schema, ext_dir, disk_space_gb=True)
        # Should include the default absolute VRAM command plus disk settings.
        assert len(result) == 5
        all_cmds = " ".join(" ".join(c) for c in result)
        assert "--schemadir" in all_cmds
        assert ext_dir in all_cmds
        assert schema in all_cmds
        assert "gpumemoryunit" in all_cmds
        assert "'numeric'" in all_cmds
        assert "diskspaceunit" in all_cmds
        assert "'perc'" not in all_cmds
        assert "diskspaceunitmeasure" in all_cmds
        assert "'g'" in all_cmds

    def test_disables_disk_throughput_stats_for_space_tray(self):
        """Should disable the KB/MB disk throughput item when showing disk space."""
        schema = "org.gnome.shell.extensions.resource-monitor"
        ext_dir = "/fake/path/schemas"
        result = mod.build_gsettings_args(schema, ext_dir, disk_space_gb=True)
        all_cmds = " ".join(" ".join(c) for c in result)
        assert "diskstatsstatus" in all_cmds
        assert "false" in all_cmds

    def test_builds_correct_args_for_disk_monitor(self):
        """Should set diskspacemonitor to 'free' for remaining-space GB."""
        schema = "org.gnome.shell.extensions.resource-monitor"
        ext_dir = "/fake/path/schemas"
        result = mod.build_gsettings_args(schema, ext_dir, disk_space_gb=True)
        all_cmds = " ".join(" ".join(c) for c in result)
        assert "diskspacemonitor" in all_cmds
        assert "'free'" in all_cmds

    def test_defaults_to_absolute_gpu_memory_when_neither_flag_set(self):
        """Should set GPU memory to absolute numeric mode by default."""
        schema = "org.gnome.shell.extensions.resource-monitor"
        ext_dir = "/fake/path/schemas"
        result = mod.build_gsettings_args(schema, ext_dir)
        assert len(result) == 1
        assert "gpumemoryunit" in result[0]
        assert "'numeric'" in result[0]

    def test_includes_gpu_devices_list(self):
        """Should include gpudeviceslist when gpu_devices is provided."""
        schema = "org.gnome.shell.extensions.resource-monitor"
        ext_dir = "/fake/path/schemas"
        devices = [{"device": "GPU-abc", "name": "RTX 4090"}]
        result = mod.build_gsettings_args(schema, ext_dir, gpu_memory_perc=True, gpu_devices=devices)
        assert len(result) == 2  # gpumemoryunit + gpudeviceslist
        devices_cmd = result[1]
        assert "gpudeviceslist" in devices_cmd

    def test_includes_disk_devices_list(self):
        """Should include diskdeviceslist when disk_devices is provided."""
        schema = "org.gnome.shell.extensions.resource-monitor"
        ext_dir = "/fake/path/schemas"
        devices = [{"device": "/dev/sda1", "mountPoint": "/"}]
        result = mod.build_gsettings_args(schema, ext_dir, disk_space_gb=True, disk_devices=devices)
        assert len(result) == 6  # gpumemoryunit + disk settings + diskdeviceslist


class TestFormatGsettingsList:
    """Tests for format_gsettings_list function."""

    def test_formats_gpu_devices_list(self):
        """Should format GPU devices as a GSettings string array with JSON."""
        devices = [
            {"device": "GPU-abc", "name": "RTX 4090", "usage": True, "memory": True},
            {"device": "GPU-def", "name": "RTX 3080", "usage": True, "memory": True},
        ]
        result = mod.format_gsettings_list(devices)
        assert isinstance(result, str)
        # Should contain JSON-encoded dicts wrapped in single quotes
        assert '"device"' in result
        assert "GPU-abc" in result
        assert "GPU-def" in result

    def test_formats_empty_list(self):
        """Should return '[]' for empty device list."""
        result = mod.format_gsettings_list([])
        assert result == "[]"

    def test_formats_disk_devices_list(self):
        """Should format disk devices as a GSettings string array with JSON."""
        devices = [
            {"device": "/dev/sda1", "mountPoint": "/", "stats": True, "space": True},
        ]
        result = mod.format_gsettings_list(devices)
        assert isinstance(result, str)
        assert '"device"' in result
        assert "/dev/sda1" in result

    def test_produces_valid_gsettings_value(self):
        """Should produce a value that gsettings can parse."""
        devices = [{"device": "GPU-abc", "name": "Test"}]
        result = mod.format_gsettings_list(devices)
        # The format should be: ['{"key": "value"}']
        assert result.startswith("[")
        assert result.endswith("]")


class TestApplySettings:
    """Tests for apply_settings function."""

    def test_success_returns_true(self):
        """Should return True when gsettings succeeds."""
        mock_result = MagicMock()
        mock_result.returncode = 0
        mock_result.stderr = ""
        with patch("subprocess.run", return_value=mock_result):
            result = mod.apply_settings(["gsettings", "set", "key", "value"])
        assert result is True

    def test_failure_returns_false(self):
        """Should return False when gsettings fails."""
        mock_result = MagicMock()
        mock_result.returncode = 1
        mock_result.stderr = "Error message"
        with patch("subprocess.run", return_value=mock_result):
            result = mod.apply_settings(["gsettings", "set", "key", "value"])
        assert result is False


class TestMain:
    """Tests for main function."""

    def test_main_no_options_returns_error(self):
        """Should return error when no options are specified."""
        result = mod.main([])
        assert result == 1

    def test_main_with_gpu_memory_only(self):
        """Should configure GPU memory percentage mode when requested."""
        mock_run_result = MagicMock()
        mock_run_result.returncode = 0
        mock_run_result.stderr = ""
        with patch("subprocess.run", return_value=mock_run_result), \
             patch("shutil.which", return_value="/usr/bin/nvidia-smi"), \
             patch("subprocess.check_output", return_value="GPU 0: Test (UUID: GPU-abc)\n"):
            result = mod.main(["--gpu-memory-perc"])
        assert result == 0

    def test_main_with_disk_space_only(self):
        """Should configure GB disk space mode when requested."""
        mock_run_result = MagicMock()
        mock_run_result.returncode = 0
        mock_run_result.stderr = ""
        with patch("subprocess.run", return_value=mock_run_result), \
             patch("subprocess.check_output", return_value="Filesystem  Size  Used  Use%  Mounted\n/dev/sda1  100G  50G  50%  /\n"):
            result = mod.main(["--disk-space-gb"])
        assert result == 0

    def test_main_accepts_legacy_disk_space_perc_alias(self):
        """Should preserve the old CLI flag while configuring GB disk space mode."""
        mock_run_result = MagicMock()
        mock_run_result.returncode = 0
        mock_run_result.stderr = ""
        with patch("subprocess.run", return_value=mock_run_result), \
             patch("subprocess.check_output", return_value="Filesystem  Size  Used  Use%  Mounted\n/dev/sda1  100G  50G  50%  /\n"):
            result = mod.main(["--disk-space-perc"])
        assert result == 0

    def test_main_with_both(self):
        """Should configure GPU memory and GB disk space when both flags are set."""
        mock_run_result = MagicMock()
        mock_run_result.returncode = 0
        mock_run_result.stderr = ""
        with patch("subprocess.run", return_value=mock_run_result), \
             patch("shutil.which", return_value="/usr/bin/nvidia-smi"), \
             patch("subprocess.check_output", side_effect=[
                 "GPU 0: Test (UUID: GPU-abc)\n",
                 "Filesystem  Size  Used  Use%  Mounted\n/dev/sda1  100G  50G  50%  /\n",
             ]):
            result = mod.main(["--gpu-memory-perc", "--disk-space-gb"])
        assert result == 0


class TestInstallerWiring:
    """Tests for installer integration with Resource Monitor configuration."""

    def test_gnome_extension_installer_applies_disk_device_list(self):
        """Installer should populate diskdeviceslist after enabling disk status."""
        source = Path("install.sh").read_text(encoding="utf-8")

        assert "scripts/configure_resource_monitor.py" in source
        assert "--disk-space-perc-home-only" in source
        assert '--schema-dir "$ext_dir/schemas"' in source
        assert "diskstatsstatus false" in source
        assert "diskspaceunit \"'perc'\"" in source
        assert "netethstatus true" in source
        assert "netwlanstatus false" in source
        assert "['cpu', 'ram', 'stats', 'space', 'eth', 'wlan', 'gpu']" in source

    def test_installer_sets_ethernet_to_megabytes(self):
        """Installer should set netunitmeasure to 'm' for MB/s display."""
        source = Path("install.sh").read_text(encoding="utf-8")
        assert "netunitmeasure \"'m'\"" in source

    def test_installer_sets_ethernet_decimals_to_one(self):
        """Installer should set netethdecimals to 1 for 0.1 precision."""
        source = Path("install.sh").read_text(encoding="utf-8")
        assert "netethdecimals 1" in source


if __name__ == "__main__":
    import pytest
    raise SystemExit(pytest.main([__file__, "-v"]))


class TestFilterDiskDevicesToMountPoint:
    """Tests for filter_disk_devices_to_mount_point function."""

    def test_filters_to_home_only(self):
        """Should keep only entries with mountPoint '/home'."""
        devices = [
            {"mountPoint": "/", "device": "/dev/nvme1n1p5"},
            {"mountPoint": "/home", "device": "/dev/nvme1n1p5"},
            {"mountPoint": "/boot", "device": "/dev/sda1"},
        ]
        result = mod.filter_disk_devices_to_mount_point(devices, "/home")
        assert len(result) == 1
        assert result[0]["mountPoint"] == "/home"

    def test_returns_empty_when_no_match(self):
        """Should return empty list when no entry matches the mount point."""
        devices = [
            {"mountPoint": "/", "device": "/dev/nvme1n1p5"},
            {"mountPoint": "/boot", "device": "/dev/sda1"},
        ]
        result = mod.filter_disk_devices_to_mount_point(devices, "/home")
        assert result == []

    def test_returns_all_when_no_filter(self):
        """Should return all entries when filter is None."""
        devices = [
            {"mountPoint": "/", "device": "/dev/nvme1n1p5"},
            {"mountPoint": "/home", "device": "/dev/nvme1n1p5"},
        ]
        result = mod.filter_disk_devices_to_mount_point(devices, None)
        assert len(result) == 2


class TestDetectDiskDevicesHomeOnly:
    """Tests for detect_disk_devices_home_only function."""

    def test_returns_only_home_when_root_shares_filesystem(self):
        """Should return only /home entry when root and home share the same device."""
        root_output = (
            "Filesystem      Size  Used Avail Use% Mounted on\n"
            "/dev/nvme1n1p5  492G   92G  376G  20% /\n"
        )
        home_output = (
            "Filesystem      Size  Used Avail Use% Mounted on\n"
            "/dev/nvme1n1p5  492G   92G  376G  20% /home\n"
        )
        with patch("subprocess.check_output", side_effect=[root_output, home_output]):
            result = mod.detect_disk_devices_home_only()

        assert len(result) == 1
        assert result[0]["mountPoint"] == "/home"

    def test_returns_empty_when_no_home_entry(self):
        """Should return empty list when /home is not a mount point."""
        root_output = (
            "Filesystem      Size  Used Avail Use% Mounted on\n"
            "/dev/nvme1n1p5  492G   92G  376G  20% /\n"
        )
        # side_effect: first call (df -P) returns root, second call (df -P /home) returns empty
        with patch("subprocess.check_output", side_effect=[root_output, ""]):
            result = mod.detect_disk_devices_home_only()

        assert result == []


class TestBuildGsettingsArgsPercHomeOnly:
    """Tests for build_gsettings_args with percentage + home-only mode."""

    def test_sets_diskspaceunit_to_perc(self):
        """Should set diskspaceunit to 'perc' when perc_home_only is True."""
        schema = "org.gnome.shell.extensions.resource-monitor"
        ext_dir = "/fake/path/schemas"
        result = mod.build_gsettings_args(
            schema, ext_dir, disk_space_perc_home_only=True
        )
        all_cmds = " ".join(" ".join(c) for c in result)
        assert "diskspaceunit" in all_cmds
        assert "'perc'" in all_cmds

    def test_does_not_set_diskspaceunitmeasure_for_perc_mode(self):
        """Should not set diskspaceunitmeasure when using percentage mode."""
        schema = "org.gnome.shell.extensions.resource-monitor"
        ext_dir = "/fake/path/schemas"
        result = mod.build_gsettings_args(
            schema, ext_dir, disk_space_perc_home_only=True
        )
        all_cmds = " ".join(" ".join(c) for c in result)
        assert "diskspaceunitmeasure" not in all_cmds

    def test_filters_disk_devices_to_home(self):
        """Should filter disk devices to only /home when perc_home_only is True."""
        schema = "org.gnome.shell.extensions.resource-monitor"
        ext_dir = "/fake/path/schemas"
        devices = [
            {"device": "/dev/nvme1n1p5", "mountPoint": "/", "displayName": "/"},
            {"device": "/dev/nvme1n1p5", "mountPoint": "/home", "displayName": "/home"},
        ]
        result = mod.build_gsettings_args(
            schema, ext_dir, disk_space_perc_home_only=True, disk_devices=devices
        )
        all_cmds = " ".join(" ".join(c) for c in result)
        # Should only contain /home in the devices list
        assert "/dev/nvme1n1p5" in all_cmds
        assert '"mountPoint": "/"' not in all_cmds
        assert '"mountPoint": "/home"' in all_cmds

    def test_sets_diskspacemonitor_to_used_for_perc_mode(self):
        """Should show disk percentage usage, not free percentage."""
        schema = "org.gnome.shell.extensions.resource-monitor"
        ext_dir = "/fake/path/schemas"
        result = mod.build_gsettings_args(
            schema, ext_dir, disk_space_perc_home_only=True
        )
        all_cmds = " ".join(" ".join(c) for c in result)
        assert "diskspacemonitor" in all_cmds
        assert "'used'" in all_cmds


class TestMainPercHomeOnly:
    """Tests for main function with --disk-space-perc-home-only flag."""

    def test_main_with_disk_space_perc_home_only(self):
        """Should configure percentage + home-only disk space mode when requested."""
        mock_run_result = MagicMock()
        mock_run_result.returncode = 0
        mock_run_result.stderr = ""
        with patch("subprocess.run", return_value=mock_run_result), \
             patch("subprocess.check_output", side_effect=[
                 "Filesystem      Size  Used Avail Use% Mounted on\n/dev/nvme1n1p5  492G   92G  376G  20% /\n",
                 "Filesystem      Size  Used Avail Use% Mounted on\n/dev/nvme1n1p5  492G   92G  376G  20% /home\n",
             ]):
            result = mod.main(["--disk-space-perc-home-only"])
        assert result == 0

    def test_main_rejects_both_gb_and_perc_home_only(self):
        """Should return error when both --disk-space-gb and --disk-space-perc-home-only are set."""
        mock_run_result = MagicMock()
        mock_run_result.returncode = 0
        with patch("subprocess.run", return_value=mock_run_result), \
             patch("subprocess.check_output", return_value="Filesystem  Size  Used  Use%  Mounted\n/dev/sda1  100G  50G  50%  /\n"):
            result = mod.main(["--disk-space-gb", "--disk-space-perc-home-only"])
        assert result == 1


class TestInstallerWiringPercHomeOnly:
    """Tests for installer integration with percentage + home-only configuration."""

    def test_installer_uses_perc_home_only_flag(self):
        """Installer should use --disk-space-perc-home-only flag instead of --disk-space-gb."""
        source = Path("install.sh").read_text(encoding="utf-8")
        assert "--disk-space-perc-home-only" in source

    def test_installer_sets_diskspaceunit_to_perc(self):
        """Installer should set diskspaceunit to 'perc' instead of 'numeric'."""
        source = Path("install.sh").read_text(encoding="utf-8")
        assert "diskspaceunit \"'perc'\"" in source

    def test_installer_no_longer_sets_diskspaceunitmeasure(self):
        """Installer should not set diskspaceunitmeasure when using percentage mode."""
        source = Path("install.sh").read_text(encoding="utf-8")
        assert "diskspaceunitmeasure" not in source

    def test_installer_sets_diskspacemonitor_to_used(self):
        """Installer should show disk percentage usage status."""
        source = Path("install.sh").read_text(encoding="utf-8")
        assert "diskspacemonitor \"'used'\"" in source

    def test_installer_still_configures_other_settings(self):
        """Installer should still configure other Resource Monitor settings correctly."""
        source = Path("install.sh").read_text(encoding="utf-8")
        assert "scripts/configure_resource_monitor.py" in source
        assert "diskstatsstatus false" in source
        assert "diskspacestatus true" in source
