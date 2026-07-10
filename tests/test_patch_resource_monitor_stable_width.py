#!/usr/bin/env python3
"""Tests for patch_resource_monitor_stable_width.js — disk activity % width."""

import subprocess
from pathlib import Path


ROOT_DIR = Path(__file__).resolve().parents[1]
PATCH_SCRIPT = ROOT_DIR / "scripts" / "patch_resource_monitor_stable_width.js"

# Minimal upstream containers.js excerpt: just enough of DiskContainerSpace for
# the patcher to find and edit its _init() and add_element() snippets.
ORIGINAL_CONTAINERS = """import Clutter from "gi://Clutter";
import GObject from "gi://GObject";
import St from "gi://St";

class DiskContainer extends St.BoxLayout {
  set_element_width(width) {
    if (width === 0) {
      this._elementsPath.forEach((element) => {
        this._elementsValue[element].min_width = 0;
        this._elementsValue[element].natural_width = 0;
        this._elementsValue[element].min_width_set = false;
        this._elementsValue[element].natural_width_set = false;
      });
    } else {
      this._elementsPath.forEach((element) => {
        this._elementsValue[element].width = width;
      });
    }
  }
}

  class DiskContainerSpace extends DiskContainer {
    _init() {
      super._init();

      this._elementsSecondaryValue = [];
      this._elementsSecondaryUnit = [];
    }

  add_element(filesystem, label) {
    this._elementsPath.push(filesystem);

    this._elementsName[filesystem] = _createNameLabel(label);

    this._elementsValue[filesystem] = _createValueLabel("--");

    this._elementsUnit[filesystem] = _createUnitLabel("KB");

    this._elementsSecondaryValue[filesystem] = _createValueLabel("", [
      "resource-monitor-secondary-value",
    ]);

    this._elementsSecondaryUnit[filesystem] = _createUnitLabel("%", [
      "resource-monitor-secondary-unit",
    ]);

      const spaceSep = new St.Label({ text: "  " });

      this.add_child(this._elementsName[filesystem]);
      this.add_child(this._elementsValue[filesystem]);
      this.add_child(this._elementsUnit[filesystem]);
      this.add_child(spaceSep);
      this.add_child(this._elementsSecondaryValue[filesystem]);
      this.add_child(this._elementsSecondaryUnit[filesystem]);
  }
}

  class GpuContainer extends St.BoxLayout {
    _init() {
      super._init();

      this._elementsUuid = [];
      this._elementsName = [];
      this._elementsValue = [];
      this._elementsUnit = [];
      this._elementsMemoryValue = [];
      this._elementsMemoryUnit = [];
      this._elementsThermalValue = [];
      this._elementsThermalUnit = [];
      this._separatorPairs = [];
    }

    set_element_width(width) {
      if (width === 0) {
        this._elementsUuid.forEach((element) => {});
      } else {
        this._elementsUuid.forEach((element) => {
          if (this._elementsValue[element] !== undefined) {
            this._elementsValue[element].width = width;
          }

          if (this._elementsMemoryValue[element] !== undefined) {
            this._elementsMemoryValue[element].width = width;
          }
        });
      }
    }
  }

export { DiskContainer, DiskContainerSpace, GpuContainer };
"""


EXPECTED_MARKERS = [
    "this._diskActivityWidth = 24",
    "Space separator between disk-space activity percent and its unit (stable width)",
    "this._gpuMemoryWidth = 16",
    "VRAM value (0-99 GB, 2 digits) gets its own tighter reserved",
]


def _write_containers_fixture(tmp_path: Path) -> Path:
    panel_dir = tmp_path / "panel"
    panel_dir.mkdir(parents=True)
    containers = panel_dir / "containers.js"
    containers.write_text(ORIGINAL_CONTAINERS, encoding="utf-8")
    return containers


def _run_patch(containers_path: Path) -> subprocess.CompletedProcess:
    return subprocess.run(
        ["node", str(PATCH_SCRIPT), str(containers_path)],
        cwd=ROOT_DIR,
        capture_output=True,
        text=True,
        check=False,
    )


def test_patch_reserves_disk_activity_width(tmp_path):
    """Should reserve the secondary activity % value width and stay idempotent."""
    containers = _write_containers_fixture(tmp_path)

    result = _run_patch(containers)
    assert result.returncode == 0, f"Patch failed: {result.stderr}"

    patched = containers.read_text(encoding="utf-8")
    for marker in EXPECTED_MARKERS:
        assert marker in patched, f"Missing marker '{marker}' in patched file"


def test_patch_assigns_width_to_secondary_value(tmp_path):
    """The secondary value actor should be given the reserved width."""
    containers = _write_containers_fixture(tmp_path)

    result = _run_patch(containers)
    assert result.returncode == 0, f"Patch failed: {result.stderr}"

    patched = containers.read_text(encoding="utf-8")
    assert "this._elementsSecondaryValue[filesystem].width = this._diskActivityWidth;" in patched


def test_patch_splits_gpu_vram_width(tmp_path):
    """GPU VRAM must get its own tighter reserved width, not the shared usage width."""
    containers = _write_containers_fixture(tmp_path)

    result = _run_patch(containers)
    assert result.returncode == 0, f"Patch failed: {result.stderr}"

    patched = containers.read_text(encoding="utf-8")
    # VRAM reserved width constant declared.
    assert "this._gpuMemoryWidth = 16" in patched
    # VRAM value routed through that constant (not the shared `width`).
    assert "this._elementsMemoryValue[element].width = this._gpuMemoryWidth;" in patched
    # Usage value still uses the shared width.
    assert "this._elementsValue[element].width = width;" in patched


def test_patch_is_idempotent_on_gpu_split(tmp_path):
    """Running twice is a no-op and produces identical output."""
    containers = _write_containers_fixture(tmp_path)

    first = _run_patch(containers)
    assert first.returncode == 0, f"First patch failed: {first.stderr}"
    second = _run_patch(containers)
    assert second.returncode == 0, f"Second patch failed: {second.stderr}"

    again_dir = tmp_path / "other"
    again_dir.mkdir()
    again = _write_containers_fixture(again_dir)
    _run_patch(again)
    assert containers.read_text() == again.read_text()


def test_patch_is_idempotent(tmp_path):
    """Running the patch twice should be a no-op on the second run."""
    containers = _write_containers_fixture(tmp_path)

    first = _run_patch(containers)
    assert first.returncode == 0, f"First patch failed: {first.stderr}"

    second = _run_patch(containers)
    assert second.returncode == 0, f"Second patch failed: {second.stderr}"
    assert "already reserved" in second.stdout

    # Content must be identical to a single application.
    again_dir = tmp_path / "other"
    again_dir.mkdir()
    again = _write_containers_fixture(again_dir)
    _run_patch(again)
    assert containers.read_text() == again.read_text()

def _run_patch_mode(mode, containers_path):
    return subprocess.run(
        ["node", str(PATCH_SCRIPT), "--mode", mode, str(containers_path)],
        cwd=ROOT_DIR,
        capture_output=True,
        text=True,
        check=False,
    )


def test_patch_compact_releases_disk_activity_width(tmp_path):
    """Compact mode removes the disk-space secondary reservation."""
    containers = _write_containers_fixture(tmp_path)
    _run_patch(containers)

    result = _run_patch_mode("compact", containers)
    assert result.returncode == 0, f"Compact patch failed: {result.stderr}"

    patched = containers.read_text(encoding="utf-8")
    assert "this._diskActivityWidth = 24" not in patched
    assert "Space separator between disk-space activity percent and its unit (stable width)" not in patched


def test_patch_compact_merges_gpu_vram_back_to_shared_width(tmp_path):
    """Compact mode reverts GPU VRAM to share the GPU usage width."""
    containers = _write_containers_fixture(tmp_path)
    _run_patch(containers)

    result = _run_patch_mode("compact", containers)
    assert result.returncode == 0, f"Compact patch failed: {result.stderr}"

    patched = containers.read_text(encoding="utf-8")
    assert "this._gpuMemoryWidth = 16" not in patched
    assert "VRAM value (0-99 GB, 2 digits) gets its own tighter reserved" not in patched
    # VRAM value now uses the shared `width` again.
    assert "this._elementsMemoryValue[element].width = width;" in patched


def test_patch_compact_is_idempotent(tmp_path):
    """Running compact twice is a no-op."""
    containers = _write_containers_fixture(tmp_path)
    _run_patch(containers)

    first = _run_patch_mode("compact", containers)
    assert first.returncode == 0
    second = _run_patch_mode("compact", containers)
    assert second.returncode == 0
    assert "already compact" in second.stdout


def test_patch_compact_then_stable_roundtrips(tmp_path):
    """Compact then stable restores the reserved widths exactly."""
    containers = _write_containers_fixture(tmp_path)
    baseline = _write_containers_fixture(tmp_path / "baseline")
    _run_patch(baseline)

    _run_patch(containers)
    _run_patch_mode("compact", containers)
    _run_patch_mode("stable", containers)

    assert containers.read_text() == baseline.read_text()


def test_patch_rejects_invalid_mode(tmp_path):
    """An unknown mode exits non-zero with a usage error."""
    containers = _write_containers_fixture(tmp_path)
    result = _run_patch_mode("tiny", containers)
    assert result.returncode != 0
    assert "Invalid --mode" in result.stderr

