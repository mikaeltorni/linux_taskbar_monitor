#!/usr/bin/env python3
"""CLI contract tests for rm-monitor gsettings-strv (replaces gsettings_strv.py)."""

from __future__ import annotations

import os
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BIN = ROOT / "dist" / "rm-monitor"


def _ensure_bin() -> Path:
    if BIN.is_file() and os.access(BIN, os.X_OK):
        return BIN
    subprocess.run(
        ["bash", str(ROOT / "scripts" / "build_rm_monitor.sh")],
        check=True,
        cwd=ROOT,
    )
    assert BIN.is_file()
    return BIN


def test_gsettings_strv_append_stdout_contract():
    bin_path = _ensure_bin()
    completed = subprocess.run(
        [str(bin_path), "gsettings-strv", "append", "two"],
        check=True,
        text=True,
        capture_output=True,
        env={**os.environ, "CURRENT": "@as ['one']"},
    )
    assert completed.stdout.strip() == "['one', 'two']"


def test_gsettings_strv_rejects_unparseable_current():
    bin_path = _ensure_bin()
    completed = subprocess.run(
        [str(bin_path), "gsettings-strv", "append", "x"],
        check=False,
        text=True,
        capture_output=True,
        env={**os.environ, "CURRENT": "not-a-list"},
    )
    assert completed.returncode == 1
    assert "parseable" in completed.stderr.lower() or "CURRENT" in completed.stderr


def test_installer_uses_rm_monitor_gsettings_strv():
    installer = (ROOT / "install.sh").read_text(encoding="utf-8")
    assert "rm_monitor gsettings-strv" in installer
    assert "scripts/gsettings_strv.py" not in installer


def test_append_gsettings_list_never_invents_empty_on_get_failure():
    """Failed gsettings get must not become CURRENT=[] (would wipe enabled-extensions)."""
    installer = (ROOT / "install.sh").read_text(encoding="utf-8")
    assert 'echo "[]"' not in installer
    assert "|| echo" not in installer.split("append_gsettings_list()")[1].split(
        "remove_gsettings_list()"
    )[0]
    assert "refusing to rewrite list" in installer
    assert "remove_gsettings_list()" in installer


def test_rm_monitor_forwards_current_under_sudo():
    """CURRENT must survive sudo env_reset or enabled-extensions can be wiped."""
    helper = (ROOT / "lib" / "rm_monitor_bin.sh").read_text(encoding="utf-8")
    assert 'CURRENT+set' in helper or '"${CURRENT+set}"' in helper
    assert 'env "CURRENT=$CURRENT"' in helper
