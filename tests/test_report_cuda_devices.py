#!/usr/bin/env python3
"""Tests for report_cuda_devices.py — simulation only, no system changes."""

import json
import re
import shutil
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path("scripts").resolve()))
from importlib.util import spec_from_file_location, module_from_spec

rc = spec_from_file_location("report_cuda_devices", "scripts/report_cuda_devices.py")
mod = module_from_spec(rc)
rc.loader.exec_module(mod)


def test_exits_cleanly_without_nvidia_smi():
    """Script should exit cleanly when nvidia-smi is not available."""
    original_which = shutil.which
    def mock_which(name):  # Fixed: was mock_ich
        if name == "nvidia-smi":
            return None
        return original_which(name)
    shutil.which = mock_which

    assert mod.get_gpu_devices() == []

    shutil.which = original_which


def test_parses_nvidia_smi_output():
    """Should parse nvidia-smi -L output correctly for multiple GPUs."""
    sample_output = """GPU 0: NVIDIA GeForce RTX 4090 (UUID: GPU-abc123def456)
GPU 1: NVIDIA GeForce RTX 4090 (UUID: GPU-789xyz000)"""

    entries = mod.parse_gpu_output(sample_output)

    assert len(entries) == 2
    assert entries[0]["name"] == "NVIDIA GeForce RTX 4090"
    assert entries[0]["device"] == "GPU-abc123def456"
    assert entries[1]["device"] == "GPU-789xyz000"


def test_formats_devices_as_gsettings_string_array():
    """Should format GPU devices as Resource Monitor's array-of-strings value."""
    devices = mod.parse_gpu_output("GPU 0: NVIDIA GeForce RTX 4090 (UUID: GPU-abc123)")
    value = mod.format_gsettings_list(devices)

    assert value.startswith("['{")
    assert value.endswith("}']")
    parsed = [json.loads(item) for item in eval(value, {"__builtins__": {}})]
    assert parsed[0]["device"] == "GPU-abc123"


def test_handles_empty_output():
    """Should handle empty nvidia-smi output gracefully."""
    assert mod.parse_gpu_output("") == []


def test_handles_malformed_lines():
    """Should handle malformed nvidia-smi output gracefully."""
    malformed = "this is not a valid line\nGPU X: incomplete"
    assert mod.parse_gpu_output(malformed) == []


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
