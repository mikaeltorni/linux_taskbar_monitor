#!/usr/bin/env python3
"""Structural tests for the rm-monitor Rust CLI used by Resource Monitor patches.

Patches and GSettings helpers run through the ``rm-monitor`` binary built from
this repository. A clean machine may lack cargo; the installer must build the
binary (local cargo, apt cargo, or container) before patching.
"""

from __future__ import annotations

from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
INSTALL = ROOT / "install.sh"
EXTENSIONS = ROOT / "lib" / "gnome_extensions.sh"
BIN_HELPER = ROOT / "lib" / "rm_monitor_bin.sh"


def test_ensure_rm_monitor_tools_builds_the_cli():
    """install.sh defines ensure_rm_monitor_tools and sources the bin helper."""
    source = INSTALL.read_text(encoding="utf-8")
    assert "ensure_rm_monitor_tools()" in source
    assert "lib/rm_monitor_bin.sh" in source
    assert "apt_install cargo" in source
    assert BIN_HELPER.is_file()
    helper = BIN_HELPER.read_text(encoding="utf-8")
    assert "ensure_rm_monitor_bin()" in helper
    assert "build_rm_monitor.sh" in helper


def test_patches_guard_on_rm_monitor_not_node():
    """Each Resource Monitor patch uses rm-monitor and never shells out to node."""
    source = EXTENSIONS.read_text(encoding="utf-8")
    assert "ensure_node" not in source
    assert " node " not in source
    assert "python3 " not in source
    for fn, subcommand in (
        ("patch_resource_monitor_gradient_colors", "patch-colors"),
        ("patch_resource_monitor_vram", "patch-vram"),
        ("patch_resource_monitor_per_disk", "patch-disk"),
        ("patch_resource_monitor_eth_icon", "patch-eth-icon"),
        ("patch_resource_monitor_process_popup", "patch-process-popup"),
        ("patch_resource_monitor_stable_width", "patch-stable-width"),
    ):
        assert f"{fn}()" in source
        body = source.split(f"{fn}()", 1)[1].split("\n}", 1)[0]
        assert "ensure_rm_monitor_bin ||" in body, f"{fn} must guard on ensure_rm_monitor_bin"
        assert f"rm_monitor {subcommand}" in body, f"{fn} must call rm_monitor {subcommand}"
