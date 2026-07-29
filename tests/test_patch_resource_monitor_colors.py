#!/usr/bin/env python3
"""Tests for patch_resource_monitor_colors.js — fixture-based extension patching."""

import subprocess
import sys
from pathlib import Path


ROOT_DIR = Path(__file__).resolve().parents[1]
PATCH_SCRIPT = ROOT_DIR / "scripts" / "patch_resource_monitor_colors.js"


# Minimal original extension.js content that the patch script targets.
ORIGINAL_EXTENSION_HEADER = """/* extension.js — Resource Monitor GNOME Shell Extension */

const {Clutter, GLib, GObject, St} = imports.gi;
const Main = imports.ui.main;
const MessageTray = imports.ui.messageTray;
const Gettext = imports.gettext;
const _ = Gettext.gettext;

import * as Extension from 'resource:///org/gnome/shell/extensions/extension.js';

export default class ResourceMonitorExtension extends Extension.MetaWindowManagerExtension {
  constructor() {
    super();
    this._indicator = null;
  }

  enable() {
    this._indicator = new Indicator(this);
    Main.panel.addToStatusArea('resource-monitor', this._indicator);
  }

  disable() {
    if (this._indicator) {
      this._indicator.destroy();
      this._indicator = null;
    }
  }
}


"""

ORIGINAL_EXTENSION_GETUSAGECOLOR = """
    _getUsageColor(value, colors) {
      return getUsageColor(value, colors, COLOR_LIST_SEPARATOR);
    }
"""

# The patched version should contain gradient logic.
EXPECTED_PATCH_MARKERS = [
    "_gradientGetUsageColor",
    "getGradientColor",
    "GRADIENT_CONFIGS",
]


def _write_extension_fixture(tmp_path: Path) -> Path:
    """Create a minimal extension.js file for patching."""
    ext_dir = tmp_path / "extension"
    ext_dir.mkdir()
    ext_file = ext_dir / "extension.js"

    # Write the original header + getUsageColor method.
    content = ORIGINAL_EXTENSION_HEADER + ORIGINAL_EXTENSION_GETUSAGECOLOR
    ext_file.write_text(content, encoding="utf-8")
    return ext_file


def _run_patch(ext_path: Path) -> subprocess.CompletedProcess:
    """Run the color patch script against a fixture extension file."""
    return subprocess.run(
        ["node", str(PATCH_SCRIPT), str(ext_path)],
        cwd=ROOT_DIR,
        capture_output=True,
        text=True,
        check=False,
    )


def test_patch_overrides_getUsageColor(tmp_path):
    """Should replace the original _getUsageColor with gradient-based logic."""
    ext_path = _write_extension_fixture(tmp_path)

    result = _run_patch(ext_path)
    assert result.returncode == 0, f"Patch failed: {result.stderr}"

    patched = ext_path.read_text(encoding="utf-8")
    for marker in EXPECTED_PATCH_MARKERS:
        assert marker in patched, f"Missing marker '{marker}' in patched file"


def test_patch_preserves_extension_structure(tmp_path):
    """Should not remove the enable/disable lifecycle methods."""
    ext_path = _write_extension_fixture(tmp_path)

    result = _run_patch(ext_path)
    assert result.returncode == 0, f"Patch failed: {result.stderr}"

    patched = ext_path.read_text(encoding="utf-8")
    assert "enable()" in patched
    assert "disable()" in patched
    assert "new Indicator(this)" in patched


def test_patch_is_idempotent(tmp_path):
    """Running the patch twice should not break anything."""
    ext_path = _write_extension_fixture(tmp_path)

    result1 = _run_patch(ext_path)
    assert result1.returncode == 0, f"First patch failed: {result1.stderr}"

    # Run again on already-patched file.
    result2 = _run_patch(ext_path)
    assert result2.returncode == 0, f"Second patch failed: {result2.stderr}"


def test_patch_injects_gradient_color_functions(tmp_path):
    """Should inject gradient color computation logic."""
    ext_path = _write_extension_fixture(tmp_path)

    result = _run_patch(ext_path)
    assert result.returncode == 0, f"Patch failed: {result.stderr}"

    patched = ext_path.read_text(encoding="utf-8")
    # Should have gradient config and color computation.
    assert "GRADIENT_CONFIGS" in patched
    assert "getGradientColor" in patched


def test_patch_injects_ethernet_gradient_max_constant(tmp_path):
    """Should inject the configurable Ethernet MB/s gradient max into extension.js."""
    ext_path = _write_extension_fixture(tmp_path)

    result = _run_patch(ext_path)
    assert result.returncode == 0, f"Patch failed: {result.stderr}"

    patched = ext_path.read_text(encoding="utf-8")
    assert "const ETHERNET_MAX_MBPS = 2000;" in patched
    assert "maxVal: ETHERNET_MAX_MBPS" in patched


def test_patch_colors_disk_usage_percentage_from_green_to_red(tmp_path):
    """Disk space colors should use used percentage: 0% green, 100% red."""
    ext_path = _write_extension_fixture(tmp_path)

    result = _run_patch(ext_path)
    assert result.returncode == 0, f"Patch failed: {result.stderr}"

    patched = ext_path.read_text(encoding="utf-8")
    assert "const DISK_USAGE_MAX_PERCENT = 100;" in patched
    assert "maxVal: DISK_USAGE_MAX_PERCENT" in patched
    assert "diskSpace: {\n    minVal: 0,\n    maxVal: DISK_USAGE_MAX_PERCENT" in patched
    assert "inverted: true" not in patched


def test_patch_uses_green_yellow_red_gradient_with_255_steps(tmp_path):
    """Gradients should hit bright yellow at 50% instead of dark olive."""
    ext_path = _write_extension_fixture(tmp_path)

    result = _run_patch(ext_path)
    assert result.returncode == 0, f"Patch failed: {result.stderr}"

    patched = ext_path.read_text(encoding="utf-8")
    assert "function getGreenYellowRedGradientColor" in patched
    assert "const midpointRGB = [255, 255, 0];" in patched
    assert "return getGradientColor(value, minVal, midpoint, startRGB, midpointRGB);" in patched
    assert "return getGradientColor(value, midpoint, maxVal, midpointRGB, endRGB);" in patched


def test_patch_detects_disk_colors_by_indicator_property_identity(tmp_path):
    """Disk color detection should not depend only on startup marker injection."""
    ext_path = _write_extension_fixture(tmp_path)

    result = _run_patch(ext_path)
    assert result.returncode == 0, f"Patch failed: {result.stderr}"

    patched = ext_path.read_text(encoding="utf-8")
    assert "colors === this._diskSpaceColors" in patched
    assert "config = GRADIENT_CONFIGS.diskSpace;" in patched


def test_patch_migrates_existing_gradient_patch_to_identity_detection(tmp_path):
    """Already-patched files should be upgraded when identity checks are missing."""
    ext_path = _write_extension_fixture(tmp_path)

    result = _run_patch(ext_path)
    assert result.returncode == 0, f"Patch failed: {result.stderr}"

    patched = ext_path.read_text(encoding="utf-8")
    identity_block = '''      if (colors === this._diskSpaceColors) {
        config = GRADIENT_CONFIGS.diskSpace;
      } else if (colors === this._netEthColors || colorStr.includes("__eth")) {
        config = GRADIENT_CONFIGS.eth;
      } else if (colors === this._netWlanColors || colorStr.includes("__wlan")) {
        config = GRADIENT_CONFIGS.wlan;
      } else if (colors === this._gpuMemoryColors || colorStr.includes("__gpuMem")) {
        config = GRADIENT_CONFIGS.gpuMemory;
      } else if (colorStr.includes("__diskSpace")) {
        config = GRADIENT_CONFIGS.diskSpace;
      } else if (colors === this._gpuColors || colorStr.includes("__gpu")) {
        config = GRADIENT_CONFIGS.gpu;
      } else if (colors === this._ramColors || colorStr.includes("__ram")) {
        config = GRADIENT_CONFIGS.ram;
      } else if (colors === this._cpuColors || colorStr.includes("__cpu")) {
        config = GRADIENT_CONFIGS.cpu;'''
    marker_only_block = '''      if (colorStr.includes("__eth")) {
        config = GRADIENT_CONFIGS.eth;
      } else if (colorStr.includes("__wlan")) {
        config = GRADIENT_CONFIGS.wlan;
      } else if (colorStr.includes("__gpuMem")) {
        config = GRADIENT_CONFIGS.gpuMemory;
      } else if (colorStr.includes("__diskSpace")) {
        config = GRADIENT_CONFIGS.diskSpace;
      } else if (colorStr.includes("__gpu")) {
        config = GRADIENT_CONFIGS.gpu;
      } else if (colorStr.includes("__ram")) {
        config = GRADIENT_CONFIGS.ram;
      } else if (colorStr.includes("__cpu")) {
        config = GRADIENT_CONFIGS.cpu;'''
    legacy_patched = patched.replace(identity_block, marker_only_block)
    assert "colors === this._diskSpaceColors" not in legacy_patched
    ext_path.write_text(legacy_patched, encoding="utf-8")

    result = _run_patch(ext_path)
    assert result.returncode == 0, f"Migration failed: {result.stderr}"

    migrated = ext_path.read_text(encoding="utf-8")
    assert "colors === this._diskSpaceColors" in migrated
    assert "colors === this._netEthColors || colorStr.includes(\"__eth\")" in migrated


def test_patch_colors_gpu_memory_usage_from_green_to_red(tmp_path):
    """GPU memory colors should be green at 0GB used and red at max VRAM used."""
    ext_path = _write_extension_fixture(tmp_path)

    result = _run_patch(ext_path)
    assert result.returncode == 0, f"Patch failed: {result.stderr}"

    patched = ext_path.read_text(encoding="utf-8")
    assert "const GPU_MEMORY_MAX_GB = 24;" in patched
    assert "gpuMemory: {\n    minVal: 0,\n    maxVal: GPU_MEMORY_MAX_GB" in patched
    assert "endRGB: [255, 0, 0]" in patched


def test_patch_colors_array_valued_network_metrics(tmp_path):
    """Ethernet and Wi-Fi values are [download, upload] arrays and must not be rejected."""
    ext_path = _write_extension_fixture(tmp_path)

    result = _run_patch(ext_path)
    assert result.returncode == 0, f"Patch failed: {result.stderr}"

    patched = ext_path.read_text(encoding="utf-8")
    assert "const numericValue = Array.isArray(value)" in patched
    assert "if (!Number.isFinite(value)) return \"\";\n\n      const numericValue" not in patched


def test_patch_handles_missing_marker_gracefully(tmp_path):
    """If the original _getUsageColor is not found, should skip gracefully."""
    ext_dir = tmp_path / "extension"
    ext_dir.mkdir()
    ext_file = ext_dir / "extension.js"

    # Write a file without the target marker.
    ext_file.write_text("const x = 1;\n", encoding="utf-8")

    result = _run_patch(ext_file)
    # Should not crash; should log a warning or skip.
    assert result.returncode == 0


if __name__ == "__main__":
    import pytest
    raise SystemExit(pytest.main([__file__, "-v"]))


class TestInstallerWiringColors:
    """Tests for installer integration with gradient color patching."""

    def test_installer_calls_color_patch_script(self):
        """Installer should call rm-monitor patch-colors on extension.js."""
        source = Path("lib/gnome_extensions.sh").read_text(encoding="utf-8")
        assert "rm_monitor patch-colors" in source

    def test_installer_patches_extension_js(self):
        """Color patch component should target extension.js, not containers.js."""
        source = Path("lib/gnome_extensions.sh").read_text(encoding="utf-8")
        joined = source.replace("\\\n", " ")
        assert "rm_monitor patch-colors" in joined
        for line in joined.splitlines():
            if "rm_monitor patch-colors" in line:
                assert "extension.js" in line, f"Color patch should target extension.js: {line}"

    def test_vram_runs_before_per_disk(self):
        """Both containers.js patches are separate components; VRAM must precede per-disk."""
        components = Path("installer/components.sh").read_text(encoding="utf-8")
        assert components.index("rm_vram|") < components.index("rm_per_disk|")
