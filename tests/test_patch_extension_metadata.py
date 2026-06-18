#!/usr/bin/env python3
"""Tests for patch_extension_metadata.py — simulation only, no system changes."""

import json
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path("scripts").resolve()))
from importlib.util import spec_from_file_location, module_from_spec

pem = spec_from_file_location("patch_extension_metadata", "scripts/patch_extension_metadata.py")
mod = module_from_spec(pem)
pem.loader.exec_module(mod)


def test_patch_shell_version_adds_missing():
    """patch_shell_version should add shell-version when missing."""
    with tempfile.NamedTemporaryFile(suffix=".json", mode="w", delete=False) as f:
        json.dump({"uuid": "test@ext"}, f)
        path = Path(f.name)

    result = mod.patch_shell_version(path, "46")
    assert result is True
    data = json.loads(path.read_text())
    assert data["shell-version"] == ["46"]


def test_patch_shell_version_appends_to_existing_list():
    """patch_shell_version should append to existing shell-version list."""
    with tempfile.NamedTemporaryFile(suffix=".json", mode="w", delete=False) as f:
        json.dump({"uuid": "test@ext", "shell-version": ["45"]}, f)
        path = Path(f.name)

    result = mod.patch_shell_version(path, "46")
    assert result is True
    data = json.loads(path.read_text())
    assert data["shell-version"] == ["45", "46"]


def test_patch_shell_version_no_change_when_present():
    """patch_shell_version should return False when version already present."""
    with tempfile.NamedTemporaryFile(suffix=".json", mode="w", delete=False) as f:
        json.dump({"uuid": "test@ext", "shell-version": ["46"]}, f)
        path = Path(f.name)

    result = mod.patch_shell_version(path, "46")
    assert result is False


def test_patch_shell_version_replaces_non_list():
    """patch_shell_version should replace non-list shell-version with list."""
    with tempfile.NamedTemporaryFile(suffix=".json", mode="w", delete=False) as f:
        json.dump({"uuid": "test@ext", "shell-version": "45"}, f)
        path = Path(f.name)

    result = mod.patch_shell_version(path, "46")
    assert result is True
    data = json.loads(path.read_text())
    assert data["shell-version"] == ["46"]


def test_patch_shell_version_file_not_found():
    """patch_shell_version should raise FileNotFoundError for missing file."""
    try:
        mod.patch_shell_version("/nonexistent/path.json", "46")
        assert False, "Should have raised FileNotFoundError"
    except FileNotFoundError:
        pass  # Expected


def test_pin_version_raises_lower_version():
    """pin_version should raise a lower version up to the pin value."""
    with tempfile.NamedTemporaryFile(suffix=".json", mode="w", delete=False) as f:
        json.dump({"uuid": "test@ext", "version": 27}, f)
        path = Path(f.name)

    result = mod.pin_version(path, 9999)
    assert result is True
    assert json.loads(path.read_text())["version"] == 9999


def test_pin_version_noop_when_already_high():
    """pin_version should not modify a version already at or above the pin."""
    with tempfile.NamedTemporaryFile(suffix=".json", mode="w", delete=False) as f:
        json.dump({"uuid": "test@ext", "version": 9999}, f)
        path = Path(f.name)

    result = mod.pin_version(path, 9999)
    assert result is False
    assert json.loads(path.read_text())["version"] == 9999


def test_pin_version_adds_missing_version():
    """pin_version should set version when the field is absent."""
    with tempfile.NamedTemporaryFile(suffix=".json", mode="w", delete=False) as f:
        json.dump({"uuid": "test@ext"}, f)
        path = Path(f.name)

    result = mod.pin_version(path, 9999)
    assert result is True
    assert json.loads(path.read_text())["version"] == 9999


if __name__ == "__main__":
    tests = [t for name, t in sorted(globals().items()) if name.startswith("test_") and callable(t)]
    passed = failed = 0
    for test_fn in tests:
        try:
            test_fn()
            print(f"  PASS: {test_fn.__doc__}")
            passed += 1
        except Exception as e:
            print(f"  FAIL: {test_fn.__doc__} — {e}")
            failed += 1
    print(f"\nResults: {passed} passed, {failed} failed")
    sys.exit(failed)
