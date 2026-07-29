#!/usr/bin/env python3
"""Structural tests for the rm-monitor Rust CLI used by Resource Monitor patches.

Patches and GSettings helpers run through the ``rm-monitor`` binary built from
this repository. A clean machine may lack cargo; the installer must build the
binary (local cargo, apt cargo, or container) before patching. No Node.js or
Python runtime is required for patching.
"""

from __future__ import annotations

from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
INSTALL = ROOT / "install.sh"
EXTENSIONS = ROOT / "lib" / "gnome_extensions.sh"
BIN_HELPER = ROOT / "lib" / "rm_monitor_bin.sh"
BUILD = ROOT / "scripts" / "build_rm_monitor.sh"


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


def test_core_does_not_require_python_or_node():
    """Clean install must not hard-require python3 or node for patching."""
    source = EXTENSIONS.read_text(encoding="utf-8")
    assert "need_cmd python3" not in source
    assert "need_cmd node" not in source
    assert "ensure_node" not in source
    for token in (" node ", "python3 ", "python3\n"):
        assert token not in source, f"unexpected runtime reference: {token!r}"


def test_patches_guard_on_rm_monitor():
    """Each Resource Monitor patch uses rm-monitor and never shells out to node."""
    source = EXTENSIONS.read_text(encoding="utf-8")
    for fn, subcommand in (
        ("patch_resource_monitor_gradient_colors", "patch-colors"),
        ("patch_resource_monitor_vram", "patch-vram"),
        ("patch_resource_monitor_per_disk", "patch-disk"),
        ("patch_resource_monitor_eth_icon", "patch-eth-icon"),
        ("patch_resource_monitor_process_popup", "patch-process-popup"),
        ("apply_resource_monitor_spacing_mode", "patch-stable-width"),
    ):
        assert f"{fn}()" in source
        body = source.split(f"{fn}()", 1)[1].split("\n}", 1)[0]
        assert "ensure_rm_monitor_bin ||" in body, f"{fn} must guard on ensure_rm_monitor_bin"
        assert f"rm_monitor {subcommand}" in body, f"{fn} must call rm_monitor {subcommand}"


def test_build_script_rebuilds_when_sources_are_newer():
    """dist/rm-monitor must not be reused blindly when src/ or Cargo.toml is newer."""
    build = BUILD.read_text(encoding="utf-8")
    assert "sources_newer_than_dist" in build
    assert "dist_is_fresh" in build
    assert "stale" in build.lower()
    assert "build_with_cargo" in build
    assert "Using existing" in build
