#!/usr/bin/env python3
"""Structural tests for the Node.js dependency used by the Resource Monitor patches.

The gradient/VRAM/per-disk patches transform the extension's JavaScript with
``node`` and have no GJS-runtime equivalent. A clean machine has no Node.js, so
the installer must provision it before patching; otherwise the patches fail and
the components never report installed. These tests lock in that the patches go
through ensure_node (which apt-installs nodejs) rather than the old, ignored
``need_cmd node`` guard.
"""

from __future__ import annotations

from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
INSTALL = ROOT / "install.sh"
EXTENSIONS = ROOT / "lib" / "gnome_extensions.sh"


def test_ensure_node_helper_installs_nodejs():
    """install.sh defines ensure_node, which apt-installs nodejs when missing."""
    source = INSTALL.read_text(encoding="utf-8")
    assert "ensure_node()" in source
    body = source.split("ensure_node()", 1)[1].split("\n}", 1)[0]
    assert "need_cmd node" in body
    assert "apt_install nodejs" in body


def test_node_patches_use_ensure_node_not_ignored_need_cmd():
    """Each node-dependent patch guards on ensure_node and bails out cleanly.

    The previous ``need_cmd node`` statement was a no-op (its result was
    discarded), so the patches ran ``node`` regardless and failed on clean
    machines. They must now short-circuit via ensure_node."""
    source = EXTENSIONS.read_text(encoding="utf-8")
    for fn in (
        "patch_resource_monitor_gradient_colors",
        "patch_resource_monitor_vram",
        "patch_resource_monitor_per_disk",
    ):
        assert f"{fn}()" in source
        body = source.split(f"{fn}()", 1)[1].split("\n}", 1)[0]
        assert "ensure_node ||" in body, f"{fn} must guard on ensure_node"
        # The old ignored guard must be gone.
        assert "need_cmd node" not in body, f"{fn} still uses the ignored need_cmd node"
