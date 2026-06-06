#!/usr/bin/env python3
"""Tests for delegating taskbar system status monitor installation."""

from pathlib import Path


ROOT_DIR = Path(__file__).resolve().parents[1]


def test_gnome_extension_module_installs_resource_monitor():
    """Resource Monitor setup should patch and configure the downloaded extension."""
    source = (ROOT_DIR / "lib" / "gnome_extensions.sh").read_text(encoding="utf-8")

    assert "patch_resource_monitor_refresh.py" in source
    assert "glib-compile-schemas" in source
    assert "refreshtime 0.5" in source
    assert "curl -fL" in source
    assert "netethstatus false" not in source
    assert "scripts/configure_resource_monitor.py" in source
