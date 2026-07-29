"""Tests for gsettings string-array helpers used by install.sh."""

from __future__ import annotations

import importlib.util
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MODULE_PATH = ROOT / "scripts" / "gsettings_strv.py"


def load_module():
    spec = importlib.util.spec_from_file_location("taskbar_gsettings_strv", MODULE_PATH)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


def test_append_strv_handles_gsettings_prefix_and_deduplicates():
    mod = load_module()

    assert mod.append_strv("@as ['Resource_Monitor@Ory0n']", "Resource_Monitor@Ory0n") == [
        "Resource_Monitor@Ory0n"
    ]
    assert mod.append_strv("@as ['one']", "two") == ["one", "two"]


def test_remove_strv_deduplicates_remaining_values():
    mod = load_module()

    assert mod.remove_strv("['one', 'two', 'two']", "one") == ["two"]


def test_cli_formats_updated_array_from_current_environment(monkeypatch):
    monkeypatch.setenv("CURRENT", "@as ['one']")

    completed = subprocess.run(
        [sys.executable, str(MODULE_PATH), "append", "two"],
        check=True,
        text=True,
        capture_output=True,
    )

    assert completed.stdout.strip() == "['one', 'two']"


def test_installer_uses_rm_monitor_gsettings_strv():
    installer = (ROOT / "install.sh").read_text(encoding="utf-8")

    assert "rm_monitor gsettings-strv" in installer
    assert "scripts/gsettings_strv.py" not in installer
    assert "python3 - <<'PY'" not in installer
