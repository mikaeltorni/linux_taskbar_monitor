#!/usr/bin/env python3
"""Tests for delegating taskbar system status monitor installation."""

from pathlib import Path


ROOT_DIR = Path(__file__).resolve().parents[1]


def test_install_sh_clones_taskbar_system_status_monitor_repo():
    """install.sh should clone the standalone taskbar system status monitor repo."""
    source = (ROOT_DIR / "install.sh").read_text(encoding="utf-8")

    # TASKBAR_SYSTEM_STATUS_MONITOR_REPO removed — now uses direct string key
    assert "mikaeltorni/ubuntu_2404_taskbar_system_status_monitor" in source
    assert "TASKBAR_SYSTEM_STATUS_MONITOR_DIR" in source
    assert '["mikaeltorni/ubuntu_2404_taskbar_system_status_monitor"]="' + "$TARGET_HOME/projects/ubuntu_2404_taskbar_system_status_monitor" in source


def test_gnome_extension_module_delegates_resource_monitor_installation():
    """Resource Monitor setup should run the standalone repo installer."""
    source = (ROOT_DIR / "lib" / "gnome_extensions.sh").read_text(encoding="utf-8")

    assert 'bash "$TASKBAR_SYSTEM_STATUS_MONITOR_DIR/install.sh"' in source
    assert 'RESOURCE_MONITOR_EXTENSION_ID="$RESOURCE_MONITOR_EXTENSION_ID"' in source
    assert "netethstatus false" not in source
    assert "scripts/configure_resource_monitor.py" not in source
